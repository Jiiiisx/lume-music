use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use iced::futures::SinkExt;
use iced::widget::{container, image, mouse_area, operation, row, shader, stack};
use iced::{window, Background, Color, Element, Length, Subscription, Task, Theme};
use lume_audio::{AudioCommand, AudioEngineHandle, AudioEvent};
use lume_core::{
    AlbumDetail, AlbumSummary, ArtistProfile, ArtistSummary, CatalogSource, LyricsData,
    MusicProvider, PlaybackSourceUsed, PlaybackStatus, Playlist, QueueActionResult, QueueEngine,
    RemotePlaylistDetail, RemotePlaylistSummary, ResolvedStream, Track, TrackSource,
};
use lume_providers::{
    canonical_playlist_url, detect_yt_dlp_binary, ensure_fresh_token, fetch_spotify_album_embed,
    fetch_spotify_artist_embed, fetch_spotify_chart_tracks, fetch_spotify_track_oembed_cover,
    find_sidecar_lrc, find_sidecar_ttml, import_spotify_playlist_url, login_spotify_playback,
    login_spotify_web_api, needs_spotify_album_artwork, parse_exportify_csv, parse_lrc, parse_ttml,
    read_embedded_lyrics, resolve_spotify_to_ytm_video_id, LocalProvider, LrclibClient,
    LyricsPlusClient, SpotifyAccountTier, SpotifyApiError, SpotifyCatalogSearchResult,
    SpotifyUserProfile, SpotifyWebClient, YouTubeMusicProvider, SPOTIFY_PLAYBACK_CLIENT_ID,
};
use lume_storage::{
    AppConfig, ArtCache, Database, GlassmorphismMode, LyricsCacheLookup, SpotifyTokenBundle,
    TokenStore, WEEKLY_CHARTS_TTL_S,
};
use tokio::sync::mpsc::{unbounded_channel, Receiver, UnboundedReceiver, UnboundedSender};
use tokio::sync::Semaphore;

use crate::lyrics_anim::{
    active_display_index, build_display_timeline, target_scroll_y_for_item, LyricsDisplayItem,
    LyricsSpringState,
};
use crate::platform::smtc::{extract_hwnd, SmtcController};
use crate::platform::apply_window_glass_mode;
use crate::theme::{
    decode_art_blur_rgba, extract_palette, AccentTheme, AlbumPalette, DynamicBackdropProgram,
};
use crate::views::browse::{
    sort_playlist_tracks, validate_playlist_name, view_browse, view_sidebar, BrowseViewProps,
};
use crate::views::lyrics_view::{lyrics_scroll_id, view_now_playing_lyrics, LyricsViewProps};
use crate::views::player_bar::{view_player_bar, view_player_menu_popover};
use crate::views::queue_drawer::view_queue_drawer;
use crate::views::settings::view_settings;
use crate::views::titlebar::view_titlebar;
use crate::views::{
    ActiveNav, ChartRegion, Message, NowPlayingInfo, PlaylistSort, SearchTab, SmtcAction,
    SourceFilter, SpotifySearchError, SpotifySettingsProps, SpotifySyncReport, ViewMode,
};

/// Bounded in-memory Browse thumbnail cache (`800 × 96×96×4 ≈ 28.8 MiB` max RAM),
/// large enough to hold 1,000+ songs across playlists without LRU eviction flicker.
const MAX_IN_MEM_ART_HANDLES: usize = 800;
const THUMBNAIL_SIZE_PX: u32 = 96;
/// Max concurrent remote artwork HTTP downloads to stay polite on CDN rate limits.
const MAX_CONCURRENT_ART_DOWNLOADS: usize = 6;
/// Max concurrent Spotify oEmbed enrichment requests to stay below rate limits.
const OEMBED_ENRICH_CONCURRENCY: usize = 4;
/// Maximum decoded dimension for the single active cinema cover (`512×512×4 = 1 MiB` max RAM).
const MAX_ACTIVE_COVER_PX: u32 = 512;
const MAX_REMOTE_ART_BYTES: usize = 10 * 1024 * 1024;
const PALETTE_TRANSITION_SECS: f32 = 0.6;
const LRCLIB_BASE_URL: &str = "https://lrclib.net";
const TOAST_SHORT_DURATION_MS: u64 = 3_200;
const TOAST_LONG_DURATION_MS: u64 = 5_000;
/// Live search waits 650 ms after typing pauses and requires 3+ chars (Enter still searches
/// immediately for any query) to avoid bursting partial-word API calls on every keystroke.
const SEARCH_DEBOUNCE_MS: u64 = 650;
const SEARCH_MIN_LIVE_CHARS: usize = 3;
/// Wait until the current track has played for at least 15 s (or <= 25 s remain) before
/// prefetching the next track's stream so rapid track skipping fires zero wasted API calls.
const PREFETCH_MIN_PLAYED_MS: u64 = 15_000;
const PREFETCH_REMAINING_WINDOW_MS: u64 = 25_000;
const SPOTIFY_SEARCH_CACHE_TTL: Duration = Duration::from_secs(10 * 60);
const SPOTIFY_SEARCH_CACHE_MAX_ENTRIES: usize = 48;
const MAX_NAV_HISTORY: usize = 30;

pub fn push_nav_history(history: &mut Vec<ActiveNav>, current: ActiveNav, next: &ActiveNav) {
    if &current == next {
        return;
    }
    if history.last() != Some(&current) {
        history.push(current);
        if history.len() > MAX_NAV_HISTORY {
            let overflow = history.len() - MAX_NAV_HISTORY;
            history.drain(0..overflow);
        }
    }
}

pub fn pick_best_artist_match(
    query_name: &str,
    candidates: &[ArtistSummary],
) -> Option<ArtistSummary> {
    let q = query_name.trim().to_ascii_lowercase();
    if q.is_empty() {
        return candidates.first().cloned();
    }
    if let Some(exact) = candidates
        .iter()
        .find(|a| a.name.trim().to_ascii_lowercase() == q)
    {
        return Some(exact.clone());
    }
    if let Some(prefix) = candidates
        .iter()
        .find(|a| a.name.trim().to_ascii_lowercase().starts_with(&q))
    {
        return Some(prefix.clone());
    }
    if let Some(contains) = candidates
        .iter()
        .find(|a| a.name.trim().to_ascii_lowercase().contains(&q))
    {
        return Some(contains.clone());
    }
    candidates.first().cloned()
}

fn format_followers_label(count: u64) -> String {
    if count >= 1_000_000 {
        format!("{:.1}M followers", count as f64 / 1_000_000.0)
    } else if count >= 1_000 {
        format!("{:.1}K followers", count as f64 / 1_000.0)
    } else {
        format!("{count} followers")
    }
}

fn chart_cache_key(provider: &str, region: ChartRegion) -> String {
    let suffix = if region.is_indonesia() { "id" } else { "global" };
    format!("{provider}:{suffix}")
}

fn toast_auto_dismiss_duration(text: &str) -> Duration {
    if text.chars().count() > 65 {
        Duration::from_millis(TOAST_LONG_DURATION_MS)
    } else {
        Duration::from_millis(TOAST_SHORT_DURATION_MS)
    }
}

fn decode_thumbnail_rgba_96(bytes: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    let img = ::image::load_from_memory(bytes).ok()?;
    let thumb = img
        .resize_to_fill(
            THUMBNAIL_SIZE_PX,
            THUMBNAIL_SIZE_PX,
            ::image::imageops::FilterType::Triangle,
        )
        .to_rgba8();
    let (w, h) = thumb.dimensions();
    Some((w, h, thumb.into_raw()))
}

/// Decodes and caps the active cinema album cover to `max_dim × max_dim` RGBA so huge local FLAC
/// covers (e.g., 2000×2000) never bloat RAM or GPU texture memory.
pub fn decode_bounded_cover_rgba(bytes: &[u8], max_dim: u32) -> Option<(u32, u32, Vec<u8>)> {
    if bytes.is_empty() || max_dim == 0 {
        return None;
    }
    let img = ::image::load_from_memory(bytes).ok()?;
    let rgba = if img.width() > max_dim || img.height() > max_dim {
        img.thumbnail(max_dim, max_dim).to_rgba8()
    } else {
        img.to_rgba8()
    };
    let (w, h) = rgba.dimensions();
    Some((w, h, rgba.into_raw()))
}

/// Classifies `librespot` / Spotify playback fallback reasons that indicate a lapsed/free Premium
/// subscription or revoked auth so subsequent tracks skip `librespot` and route straight to YTM.
pub fn is_spotify_lapsed_reason(reason: &str) -> bool {
    let lower = reason.to_ascii_lowercase();
    lower.contains("premium")
        || lower.contains("lapsed")
        || lower.contains("403")
        || lower.contains("forbidden")
        || lower.contains("unauthorized")
        || lower.contains("account")
        || lower.contains("not signed in")
}

/// Detects transient offline / DNS / connection-refused network errors so cached SQLite views
/// stay calm and avoid spamming raw stack/reqwest strings.
pub fn is_likely_offline_error(err: &str) -> bool {
    let lower = err.to_ascii_lowercase();
    lower.contains("dns error")
        || lower.contains("failed to lookup address")
        || lower.contains("no such host")
        || lower.contains("network is unreachable")
        || lower.contains("error sending request")
        || lower.contains("timed out")
        || lower.contains("connection refused")
        || lower.contains("connection reset")
        || lower.contains("offline")
}

/// Formats an actionable, non-blocking toast when YouTube Music playback fails (e.g. IP-level
/// anti-bot check, HTTP 403 PO-token rejection, or missing `yt-dlp` binary).
pub fn format_ytm_stream_error_toast(err: &str, yt_dlp_available: bool) -> String {
    if is_likely_offline_error(err) {
        return "Offline — cannot stream remote track right now. Local files remain playable."
            .to_string();
    }
    let lower = err.to_ascii_lowercase();
    if lower.contains("sign in to confirm") || lower.contains("not a bot") {
        return "YouTube temporarily flagged this network IP ('Sign in to confirm you're not a bot'). Try restarting your router, switching networks/hotspot, disabling VPN, or using Spotify Premium in Settings.".to_string();
    }
    let is_403_or_bot = lower.contains("403")
        || lower.contains("po token")
        || lower.contains("po-token")
        || lower.contains("forbidden")
        || lower.contains("yt-dlp");
    if is_403_or_bot && !yt_dlp_available {
        return "YouTube blocked direct audio (HTTP 403). Install yt-dlp (winget install yt-dlp.yt-dlp) or set its path in Settings.".to_string();
    }
    if is_403_or_bot {
        return format!(
            "YouTube stream blocked (403). Try updating yt-dlp (`yt-dlp -U`) or re-matching the track. ({err})"
        );
    }
    err.to_string()
}

pub fn is_remote_art_url(key: &str) -> bool {
    key.starts_with("https://") || key.starts_with("http://")
}

/// Decides whether a Spotify track should attempt native `librespot` playback or go straight
/// to Smart YouTube Music Auto-Fallback.
pub fn should_attempt_spotify_native(
    prefer_youtube_fallback: bool,
    has_playback_token: bool,
    account_tier: SpotifyAccountTier,
    spotify_lapsed: bool,
) -> bool {
    if prefer_youtube_fallback || !has_playback_token || spotify_lapsed {
        return false;
    }
    account_tier != SpotifyAccountTier::Free
}

/// Concise sidebar status pill label reflecting Spotify + fallback connectivity.
pub fn format_connection_status_pill(
    web_connected: bool,
    playback_connected: bool,
    account_tier: SpotifyAccountTier,
    spotify_lapsed: bool,
    prefer_youtube_fallback: bool,
) -> &'static str {
    if !web_connected && !playback_connected {
        return "Local + YouTube Music";
    }
    if spotify_lapsed {
        return "Spotify Lapsed • YTM";
    }
    if account_tier == SpotifyAccountTier::Free {
        return "Spotify Free • YTM";
    }
    if prefer_youtube_fallback {
        return "Spotify • YTM Fallback";
    }
    if playback_connected {
        return "Spotify Premium";
    }
    "Spotify Sync • YTM"
}

fn rewrite_stream_as_youtube_fallback(stream: ResolvedStream) -> ResolvedStream {
    match stream {
        ResolvedStream::HttpUrl { url, .. } => ResolvedStream::HttpUrl {
            url,
            source: PlaybackSourceUsed::YouTubeFallback,
        },
        ResolvedStream::YtDlpProcess {
            binary,
            watch_url,
            format_selector,
            ..
        } => ResolvedStream::YtDlpProcess {
            binary,
            watch_url,
            format_selector,
            source: PlaybackSourceUsed::YouTubeFallback,
        },
        other => other,
    }
}

pub fn merge_search_results(cached: Vec<Track>, remote: Vec<Track>) -> Vec<Track> {
    let mut seen = HashSet::with_capacity(cached.len() + remote.len());
    let mut merged = Vec::with_capacity(cached.len() + remote.len());
    for track in cached {
        if seen.insert(track.uid.clone()) {
            merged.push(track);
        }
    }
    for track in remote {
        if seen.insert(track.uid.clone()) {
            merged.push(track);
        }
    }
    merged
}

static ART_HTTP_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

fn shared_art_http_client() -> &'static reqwest::Client {
    ART_HTTP_CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(12))
            .pool_max_idle_per_host(16)
            .build()
            .unwrap_or_else(|_| reqwest::Client::new())
    })
}

async fn fetch_or_cache_art(art_cache: Arc<Mutex<ArtCache>>, key: String) -> Option<Vec<u8>> {
    let cache_read = Arc::clone(&art_cache);
    let key_read = key.clone();
    let cached = tokio::task::spawn_blocking(move || {
        cache_read
            .lock()
            .ok()
            .and_then(|mut c| c.get(&key_read).ok().flatten())
    })
    .await
    .ok()
    .flatten();

    if cached.is_some() {
        return cached;
    }

    if !is_remote_art_url(&key) {
        return None;
    }

    let mut fetched: Option<Vec<u8>> = None;
    for attempt in 0..2 {
        if attempt > 0 {
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
        let Ok(resp) = shared_art_http_client().get(&key).send().await else {
            continue;
        };
        if !resp.status().is_success() {
            if resp.status().as_u16() == 429 || resp.status().is_server_error() {
                continue;
            }
            return None;
        }
        let Ok(bytes) = resp.bytes().await else {
            continue;
        };
        if bytes.is_empty() || bytes.len() > MAX_REMOTE_ART_BYTES {
            return None;
        }
        fetched = Some(bytes.to_vec());
        break;
    }

    let vec = fetched?;
    let vec_for_cache = vec.clone();
    let _ = tokio::task::spawn_blocking(move || {
        if let Ok(mut cache) = art_cache.lock() {
            let _ = cache.put(&key, &vec_for_cache);
        }
    })
    .await;
    Some(vec)
}

static AUDIO_RX_SLOT: OnceLock<Mutex<Option<Receiver<AudioEvent>>>> = OnceLock::new();
static SMTC_RX_SLOT: OnceLock<Mutex<Option<UnboundedReceiver<SmtcAction>>>> = OnceLock::new();

#[allow(clippy::too_many_arguments)]
pub fn should_tick_frames(
    window_visible: bool,
    minimized: bool,
    reduced_motion: bool,
    view_mode: ViewMode,
    is_playing: bool,
    lyrics_loading: bool,
    transition_active: bool,
    spring_settled: bool,
) -> bool {
    if !window_visible || minimized || reduced_motion {
        return false;
    }
    if transition_active {
        return true;
    }
    match view_mode {
        ViewMode::NowPlayingLyrics => is_playing || lyrics_loading || !spring_settled,
        // Browse backdrop shader is static; seekbar updates via 100 ms AudioEvent::PositionTick.
        ViewMode::Browse => false,
    }
}

fn audio_subscription() -> impl iced::futures::Stream<Item = Message> {
    iced::stream::channel(64, async move |mut output| {
        let mut rx = AUDIO_RX_SLOT
            .get()
            .and_then(|m| m.lock().ok().and_then(|mut g| g.take()));
        let Some(ref mut receiver) = rx else {
            std::future::pending::<()>().await;
            return;
        };
        while let Some(ev) = receiver.recv().await {
            if output.send(Message::Audio(ev)).await.is_err() {
                break;
            }
        }
    })
}

fn smtc_subscription() -> impl iced::futures::Stream<Item = Message> {
    iced::stream::channel(32, async move |mut output| {
        let mut rx = SMTC_RX_SLOT
            .get()
            .and_then(|m| m.lock().ok().and_then(|mut g| g.take()));
        let Some(ref mut receiver) = rx else {
            std::future::pending::<()>().await;
            return;
        };
        while let Some(action) = receiver.recv().await {
            if output.send(Message::Smtc(action)).await.is_err() {
                break;
            }
        }
    })
}

pub struct LumeApp {
    config: AppConfig,
    config_path: PathBuf,
    db_path: PathBuf,
    db: Arc<Mutex<Database>>,
    art_cache: Arc<Mutex<ArtCache>>,
    token_store: TokenStore,
    tokens: SpotifyTokenBundle,
    audio: Option<AudioEngineHandle>,
    ytm: Option<Arc<YouTubeMusicProvider>>,
    active_http_fallback: Option<(u64, String, Track)>,

    main_window: Option<window::Id>,
    window_visible: bool,
    window_focused: bool,
    minimized: bool,
    maximized: bool,
    window_width: f32,
    window_height: f32,

    view_mode: ViewMode,
    nav: ActiveNav,
    nav_history: Vec<ActiveNav>,
    queue_open: bool,

    search_query: String,
    search_tab: SearchTab,
    source_filter: SourceFilter,
    playlist_sorts: HashMap<String, PlaylistSort>,
    playlists: Vec<Playlist>,
    local_tracks: Vec<Track>,
    liked_tracks: Vec<Track>,
    recent_tracks: Vec<Track>,
    home_region: ChartRegion,
    home_spotify_charts: HashMap<ChartRegion, Vec<Track>>,
    home_ytm_charts: HashMap<ChartRegion, Vec<Track>>,
    home_loading_spotify: bool,
    home_loading_ytm: bool,
    home_spotify_expanded: bool,
    home_ytm_expanded: bool,
    playlist_tracks: Vec<Track>,
    search_results: Vec<Track>,
    spotify_search_results: Vec<Track>,
    ytm_search_results: Vec<Track>,
    spotify_search_artists: Vec<ArtistSummary>,
    spotify_search_playlists: Vec<RemotePlaylistSummary>,
    ytm_search_artists: Vec<ArtistSummary>,
    ytm_search_playlists: Vec<RemotePlaylistSummary>,
    searching_spotify: bool,
    searching_ytm: bool,
    resolving_stream: bool,
    search_request_id: u64,
    search_debounce_seq: u64,
    last_remote_search_query: String,
    spotify_search_cache: HashMap<String, (Instant, SpotifyCatalogSearchResult)>,
    play_request_id: u64,
    prefetched_for_play_id: u64,
    yt_dlp_detected: Option<PathBuf>,

    active_artist_profile: Option<ArtistProfile>,
    loading_artist: bool,
    artist_error: Option<String>,
    artist_tracks_expanded: bool,
    artist_request_id: u64,
    active_album_detail: Option<AlbumDetail>,
    loading_album: bool,
    album_error: Option<String>,
    album_request_id: u64,
    active_remote_playlist: Option<RemotePlaylistDetail>,
    loading_remote_playlist: bool,
    remote_playlist_error: Option<String>,
    saving_remote_playlist: bool,
    remote_playlist_request_id: u64,
    artist_name_cache: HashMap<String, ArtistSummary>,

    spotify_client_id_input: String,
    spotify_profile: Option<SpotifyUserProfile>,
    spotify_lapsed: bool,
    spotify_logging_in_web: bool,
    spotify_logging_in_playback: bool,
    spotify_syncing: bool,
    spotify_sync_summary: Option<String>,
    pending_mutations_count: usize,
    importing_url: bool,

    queue: QueueEngine,
    status: PlaybackStatus,
    source_used: Option<PlaybackSourceUsed>,
    position_ms: u64,
    last_audio_pos_ms: u64,
    last_audio_pos_at: Instant,
    duration_ms: u64,
    volume: f32,
    muted: bool,

    current_lyrics_uid: Option<String>,
    current_lyrics: Option<LyricsData>,
    cached_lyrics_timeline: Vec<LyricsDisplayItem>,
    lyrics_loading: bool,
    lyrics_spring: LyricsSpringState,
    last_scrolled_line: Option<usize>,

    active_cover_handle: Option<(String, image::Handle)>,
    art_handles: HashMap<String, image::Handle>,
    art_handle_lru: VecDeque<String>,
    art_blur_rgba: Option<Arc<Vec<u8>>>,
    art_blur_version: u64,
    enriching_spotify_art: HashSet<String>,

    prev_palette: AlbumPalette,
    target_palette: AlbumPalette,
    display_palette: AlbumPalette,
    palette_transition_start: Option<Instant>,
    anim_start: Instant,
    last_frame_at: Instant,
    time_s: f32,

    open_track_menu_uid: Option<String>,
    player_menu_open: bool,
    renaming_playlist: Option<(String, String)>,
    show_new_playlist_modal: bool,
    new_playlist_input: String,
    show_import_url_modal: bool,
    import_url_input: String,
    modal_error: Option<String>,
    status_toast: Option<String>,
    toast_seq: u64,
    scanning_folders: bool,
    scan_summary: Option<String>,

    smtc: SmtcController,
    smtc_tx: UnboundedSender<SmtcAction>,
}

impl LumeApp {
    pub fn boot() -> (Self, Task<Message>) {
        let config_path = AppConfig::default_config_path();
        let config = AppConfig::load_or_default(&config_path).unwrap_or_default();
        let db_path = Database::default_db_path();
        let db_raw = Database::open(&db_path)
            .or_else(|_| Database::open_in_memory())
            .expect("SQLite initialization failed");

        let playlists = db_raw.list_playlists().unwrap_or_default();
        let local_tracks = db_raw.list_local_tracks().unwrap_or_default();
        let liked_tracks = db_raw.list_unified_liked_tracks().unwrap_or_default();
        let recent_tracks = db_raw.list_recent_tracks(12).unwrap_or_default();
        let session = db_raw.load_playback_session().unwrap_or(None);
        let pending_mutations_count = db_raw.count_pending_mutations().unwrap_or(0);

        let token_store = TokenStore::open_default();
        let tokens = token_store.load().unwrap_or_default();
        let spotify_client_id_input = config.spotify_client_id.clone().unwrap_or_default();

        let art_cache = Arc::new(Mutex::new(ArtCache::new(
            ArtCache::default_cache_dir(),
            ArtCache::default_disk_budget(),
            ArtCache::default_mem_entries(),
        )));

        let mut queue = QueueEngine::new();
        let mut volume = 0.85_f32;
        let mut muted = false;
        if let Some(sess) = session {
            queue = QueueEngine::from_snapshot(sess.queue);
            volume = sess.volume.clamp(0.0, 1.0);
            muted = sess.muted;
        }

        let audio = match AudioEngineHandle::spawn() {
            Ok((handle, rx)) => {
                let _ = handle.send(AudioCommand::SetVolume(volume));
                let _ = handle.send(AudioCommand::SetMute(muted));
                let slot = AUDIO_RX_SLOT.get_or_init(|| Mutex::new(None));
                *slot.lock().unwrap() = Some(rx);
                Some(handle)
            }
            Err(_) => None,
        };

        let (smtc_tx, smtc_rx) = unbounded_channel();
        *SMTC_RX_SLOT
            .get_or_init(|| Mutex::new(None))
            .lock()
            .unwrap() = Some(smtc_rx);

        let now = Instant::now();
        let default_pal = AlbumPalette::default_dark();
        let db = Arc::new(Mutex::new(db_raw));
        let yt_dlp_detected = detect_yt_dlp_binary(config.yt_dlp_path.as_deref());
        let ytm = YouTubeMusicProvider::new(
            YouTubeMusicProvider::default_cache_dir(),
            config.yt_dlp_path.clone(),
        )
        .ok()
        .map(Arc::new);
        let playlist_sorts: HashMap<String, PlaylistSort> = config
            .playlist_sorts
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect();

        let mut app = Self {
            config,
            config_path,
            db_path,
            db,
            art_cache,
            token_store,
            tokens,
            audio,
            ytm,
            active_http_fallback: None,
            main_window: None,
            window_visible: true,
            window_focused: true,
            minimized: false,
            maximized: false,
            window_width: 1180.0,
            window_height: 760.0,
            view_mode: ViewMode::Browse,
            nav: ActiveNav::Home,
            nav_history: Vec::new(),
            queue_open: false,
            search_query: String::new(),
            search_tab: SearchTab::All,
            source_filter: SourceFilter::All,
            playlist_sorts,
            playlists,
            local_tracks,
            liked_tracks,
            recent_tracks,
            home_region: ChartRegion::Indonesia,
            home_spotify_charts: HashMap::new(),
            home_ytm_charts: HashMap::new(),
            home_loading_spotify: false,
            home_loading_ytm: false,
            home_spotify_expanded: false,
            home_ytm_expanded: false,
            playlist_tracks: Vec::new(),
            search_results: Vec::new(),
            spotify_search_results: Vec::new(),
            ytm_search_results: Vec::new(),
            spotify_search_artists: Vec::new(),
            spotify_search_playlists: Vec::new(),
            ytm_search_artists: Vec::new(),
            ytm_search_playlists: Vec::new(),
            searching_spotify: false,
            searching_ytm: false,
            resolving_stream: false,
            search_request_id: 0,
            search_debounce_seq: 0,
            last_remote_search_query: String::new(),
            spotify_search_cache: HashMap::new(),
            play_request_id: 0,
            prefetched_for_play_id: 0,
            yt_dlp_detected,
            active_artist_profile: None,
            loading_artist: false,
            artist_error: None,
            artist_tracks_expanded: false,
            artist_request_id: 0,
            active_album_detail: None,
            loading_album: false,
            album_error: None,
            album_request_id: 0,
            active_remote_playlist: None,
            loading_remote_playlist: false,
            remote_playlist_error: None,
            saving_remote_playlist: false,
            remote_playlist_request_id: 0,
            artist_name_cache: HashMap::new(),
            spotify_client_id_input,
            spotify_profile: None,
            spotify_lapsed: false,
            spotify_logging_in_web: false,
            spotify_logging_in_playback: false,
            spotify_syncing: false,
            spotify_sync_summary: None,
            pending_mutations_count,
            importing_url: false,
            queue,
            status: PlaybackStatus::Stopped,
            source_used: None,
            position_ms: 0,
            last_audio_pos_ms: 0,
            last_audio_pos_at: now,
            duration_ms: 0,
            volume,
            muted,
            current_lyrics_uid: None,
            current_lyrics: None,
            cached_lyrics_timeline: Vec::new(),
            lyrics_loading: false,
            lyrics_spring: LyricsSpringState::new(),
            last_scrolled_line: None,
            active_cover_handle: None,
            art_handles: HashMap::new(),
            art_handle_lru: VecDeque::new(),
            art_blur_rgba: None,
            art_blur_version: 0,
            enriching_spotify_art: HashSet::new(),
            prev_palette: default_pal,
            target_palette: default_pal,
            display_palette: default_pal,
            palette_transition_start: None,
            anim_start: now,
            last_frame_at: now,
            time_s: 0.0,
            open_track_menu_uid: None,
            player_menu_open: false,
            renaming_playlist: None,
            show_new_playlist_modal: false,
            new_playlist_input: String::new(),
            show_import_url_modal: false,
            import_url_input: String::new(),
            modal_error: None,
            status_toast: None,
            toast_seq: 0,
            scanning_folders: false,
            scan_summary: None,
            smtc: SmtcController::new(),
            smtc_tx,
        };

        let mut startup_tasks = vec![window::oldest().map(|id| {
            id.map(Message::WindowOpened)
                .unwrap_or(Message::DismissToast)
        })];

        if let Some(ytm) = app.ytm.clone() {
            startup_tasks.push(Task::perform(
                async move {
                    ytm.warm_up().await;
                },
                |_| Message::BackgroundTaskDone,
            ));
        }

        if let Some(current) = app.queue.current().cloned() {
            app.duration_ms = current.duration_ms;
            startup_tasks.push(app.load_palette_task(&current));
            if needs_spotify_album_artwork(&current) {
                startup_tasks.push(app.spawn_spotify_artwork_enrichment_task(&[current]));
            }
        }
        if !app.config.local_folders.is_empty() {
            app.scanning_folders = true;
            startup_tasks.push(app.spawn_local_scan_task());
        } else if !app.local_tracks.is_empty() {
            let keys: Vec<String> = app
                .local_tracks
                .iter()
                .filter_map(|t| t.artwork_ref.clone())
                .take(MAX_IN_MEM_ART_HANDLES)
                .collect();
            startup_tasks.push(app.preload_thumbnails_task(keys));
        }

        if app.tokens.web_api.is_some()
            && app
                .config
                .spotify_client_id
                .as_deref()
                .is_some_and(|id| !id.trim().is_empty())
        {
            app.spotify_syncing = true;
            startup_tasks.push(app.spawn_spotify_sync_task());
        }

        if !app.recent_tracks.is_empty() {
            let recent_keys = app.missing_art_keys(&app.recent_tracks);
            let recent_snapshot = app.recent_tracks.clone();
            startup_tasks.push(app.preload_thumbnails_task(recent_keys));
            startup_tasks.push(app.spawn_spotify_artwork_enrichment_task(&recent_snapshot));
        }
        startup_tasks.push(app.spawn_home_charts_tasks(app.home_region, false));

        (app, Task::batch(startup_tasks))
    }

    pub fn title(&self) -> String {
        if let Some(track) = self.queue.current() {
            format!("{} — {} | Lume Music", track.title, track.artist_display())
        } else {
            "Lume Music".to_string()
        }
    }

    pub fn theme(&self) -> Theme {
        Theme::Dark
    }

    pub fn style(&self, _theme: &Theme) -> iced::theme::Style {
        iced::theme::Style {
            background_color: Color::TRANSPARENT,
            text_color: Color::WHITE,
        }
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let mut subs = vec![
            window::events().map(|(id, ev)| Message::WindowEvent(id, ev)),
            window::close_requests().map(|_| Message::CloseRequested),
            Subscription::run(audio_subscription),
            Subscription::run(smtc_subscription),
        ];

        let is_playing = self.status == PlaybackStatus::Playing;
        let transition_active = self.palette_transition_start.is_some();
        let spring_settled =
            self.view_mode != ViewMode::NowPlayingLyrics || self.lyrics_spring.is_settled();

        if should_tick_frames(
            self.window_visible,
            self.minimized,
            self.config.reduced_motion,
            self.view_mode,
            is_playing,
            self.lyrics_loading,
            transition_active,
            spring_settled,
        ) {
            subs.push(window::frames().map(Message::FrameTick));
        }

        Subscription::batch(subs)
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        let prev_toast = self.status_toast.clone();
        let task = self.update_inner(message);
        if self.status_toast.is_some() && self.status_toast != prev_toast {
            self.toast_seq = self.toast_seq.wrapping_add(1);
            let seq = self.toast_seq;
            let delay = toast_auto_dismiss_duration(self.status_toast.as_deref().unwrap_or(""));
            let expire_task = Task::perform(
                async move {
                    tokio::time::sleep(delay).await;
                    seq
                },
                Message::ExpireToast,
            );
            return Task::batch([task, expire_task]);
        }
        task
    }

    fn update_inner(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::WindowOpened(id) => {
                self.main_window = Some(id);
                return Task::batch([
                    apply_window_glass_mode(id, self.config.glassmorphism_mode, self.window_focused),
                    self.init_smtc_task(id),
                ]);
            }
            Message::SmtcHwndReady(Some(hwnd)) => {
                self.smtc.attach_with_hwnd(hwnd, self.smtc_tx.clone());
                if let Some(track) = self.queue.current() {
                    let cover_path = track
                        .artwork_ref
                        .as_deref()
                        .and_then(|k| self.art_cache.lock().ok().and_then(|c| c.art_path(k)));
                    self.smtc.set_metadata(Some(track), cover_path.as_deref());
                    self.smtc.set_playback(self.status, self.position_ms);
                }
            }
            Message::SmtcHwndReady(None) => {}
            Message::WindowEvent(id, event) => match event {
                window::Event::Opened { size, .. } => {
                    self.main_window = Some(id);
                    self.window_visible = true;
                    self.window_focused = true;
                    self.minimized = false;
                    if size.width > 0.0 && size.height > 0.0 {
                        self.window_width = size.width;
                        self.window_height = size.height;
                    }
                    return Task::batch([
                        apply_window_glass_mode(id, self.config.glassmorphism_mode, true),
                        self.init_smtc_task(id),
                    ]);
                }
                window::Event::Resized(size) => {
                    if size.width <= 0.0 || size.height <= 0.0 {
                        // Windows taskbar / Win+Down minimize reports a zero-area surface.
                        self.minimized = true;
                    } else {
                        self.minimized = false;
                        self.window_width = size.width;
                        self.window_height = size.height;
                        self.last_scrolled_line = None;
                    }
                }
                window::Event::Focused => {
                    self.window_visible = true;
                    self.window_focused = true;
                    self.minimized = false;
                    return apply_window_glass_mode(id, self.config.glassmorphism_mode, true);
                }
                window::Event::Unfocused => {
                    self.window_focused = false;
                    return apply_window_glass_mode(id, self.config.glassmorphism_mode, false);
                }
                _ => {}
            },
            Message::DragWindow => {
                if let Some(id) = self.main_window {
                    return window::drag(id);
                }
            }
            Message::MinimizeWindow => {
                self.minimized = true;
                if let Some(id) = self.main_window {
                    return window::minimize(id, true);
                }
            }
            Message::ToggleMaximizeWindow => {
                self.maximized = !self.maximized;
                if let Some(id) = self.main_window {
                    return window::toggle_maximize(id);
                }
            }
            Message::CloseRequested => {
                return self.shutdown_and_exit();
            }
            Message::FrameTick(now) => {
                let dt = (now - self.last_frame_at).as_secs_f32().clamp(0.0, 0.05);
                self.last_frame_at = now;
                self.time_s = (now - self.anim_start).as_secs_f32();

                if self.status == PlaybackStatus::Playing {
                    let extra_ms = (now
                        .saturating_duration_since(self.last_audio_pos_at)
                        .as_millis() as u64)
                        .min(250);
                    let interp = self.last_audio_pos_ms.saturating_add(extra_ms);
                    self.position_ms = if self.duration_ms > 0 {
                        interp.min(self.duration_ms)
                    } else {
                        interp
                    };
                }

                if let Some(start) = self.palette_transition_start {
                    let progress =
                        ((now - start).as_secs_f32() / PALETTE_TRANSITION_SECS).clamp(0.0, 1.0);
                    self.display_palette = self.prev_palette.lerp(&self.target_palette, progress);
                    if progress >= 1.0 {
                        self.display_palette = self.target_palette;
                        self.palette_transition_start = None;
                    }
                }

                if self.view_mode == ViewMode::NowPlayingLyrics {
                    if let Some(lyrics) = &self.current_lyrics {
                        let timeline = &self.cached_lyrics_timeline;
                        let idx = active_display_index(timeline, self.position_ms);
                        self.lyrics_spring.notify_active_item(idx);
                        if let Some(idx) = idx {
                            let target_y = target_scroll_y_for_item(
                                timeline,
                                &lyrics.synced,
                                idx,
                                self.config.lyrics_visual_mode,
                                self.window_width,
                                self.window_height,
                            );
                            self.lyrics_spring.update(dt, target_y);
                            if !self.lyrics_spring.is_settled()
                                || self.last_scrolled_line != Some(idx)
                            {
                                self.last_scrolled_line = Some(idx);
                                return operation::scroll_to(
                                    lyrics_scroll_id(),
                                    operation::AbsoluteOffset {
                                        x: None,
                                        y: Some(self.lyrics_spring.scroll_y().max(0.0)),
                                    },
                                );
                            }
                        }
                    }
                }
            }
            Message::SelectNav(nav) => {
                self.open_track_menu_uid = None;
                self.player_menu_open = false;
                self.nav_history.clear();
                self.nav = nav.clone();
                if matches!(self.nav, ActiveNav::Home) {
                    if let Ok(db) = self.db.lock() {
                        self.recent_tracks = db.list_recent_tracks(12).unwrap_or_default();
                    }
                    let keys = self.missing_art_keys(&self.recent_tracks);
                    let recent_snapshot = self.recent_tracks.clone();
                    return Task::batch([
                        self.preload_thumbnails_task(keys),
                        self.spawn_spotify_artwork_enrichment_task(&recent_snapshot),
                        self.spawn_home_charts_tasks(self.home_region, false),
                    ]);
                } else if let ActiveNav::Playlist(pl_id) = nav {
                    if let Ok(db) = self.db.lock() {
                        self.playlist_tracks = db.get_playlist_tracks(&pl_id).unwrap_or_default();
                    }
                    let ordered = self.ordered_playlist_tracks(&pl_id);
                    let keys = self.missing_art_keys(&ordered);
                    return Task::batch([
                        self.preload_thumbnails_task(keys),
                        self.spawn_spotify_artwork_enrichment_task(&ordered),
                    ]);
                } else if matches!(self.nav, ActiveNav::LikedSongs) {
                    if let Ok(db) = self.db.lock() {
                        self.liked_tracks = db.list_unified_liked_tracks().unwrap_or_default();
                    }
                    let keys = self.missing_art_keys(&self.liked_tracks);
                    let tracks_snapshot = self.liked_tracks.clone();
                    return Task::batch([
                        self.preload_thumbnails_task(keys),
                        self.spawn_spotify_artwork_enrichment_task(&tracks_snapshot),
                    ]);
                } else if matches!(self.nav, ActiveNav::Search) {
                    self.refresh_search_results();
                } else if matches!(self.nav, ActiveNav::Settings) {
                    self.yt_dlp_detected = detect_yt_dlp_binary(self.config.yt_dlp_path.as_deref());
                }
            }
            Message::NavigateBack => {
                self.open_track_menu_uid = None;
                self.player_menu_open = false;
                if let Some(prev) = self.nav_history.pop() {
                    self.nav = prev.clone();
                    match prev {
                        ActiveNav::ArtistProfile { source, id, name } => {
                            let needs_reload = self
                                .active_artist_profile
                                .as_ref()
                                .map(|p| p.id != id || p.source != source)
                                .unwrap_or(true);
                            if needs_reload {
                                return self.start_loading_artist(ArtistSummary {
                                    id,
                                    source,
                                    name,
                                    avatar_url: None,
                                    subtitle: None,
                                });
                            }
                        }
                        ActiveNav::AlbumDetail { source, id, title } => {
                            let needs_reload = self
                                .active_album_detail
                                .as_ref()
                                .map(|a| a.id != id || a.source != source)
                                .unwrap_or(true);
                            if needs_reload {
                                return self.start_loading_album(AlbumSummary {
                                    id,
                                    source,
                                    title,
                                    artist_name: String::new(),
                                    year: None,
                                    album_type: None,
                                    cover_url: None,
                                });
                            }
                        }
                        ActiveNav::RemotePlaylist { source, id, name } => {
                            let needs_reload = self
                                .active_remote_playlist
                                .as_ref()
                                .map(|p| p.id != id || p.source != source)
                                .unwrap_or(true);
                            if needs_reload {
                                return self.start_loading_remote_playlist(RemotePlaylistSummary {
                                    id,
                                    source,
                                    name,
                                    owner: None,
                                    track_count: None,
                                    cover_url: None,
                                });
                            }
                        }
                        ActiveNav::Search => self.refresh_search_results(),
                        _ => {}
                    }
                } else {
                    self.nav = ActiveNav::Search;
                }
            }
            Message::SelectHomeRegion(region) => {
                if self.home_region != region {
                    self.home_region = region;
                    self.home_spotify_expanded = false;
                    self.home_ytm_expanded = false;
                    return self.spawn_home_charts_tasks(region, false);
                }
            }
            Message::RefreshHomeCharts => {
                return self.spawn_home_charts_tasks(self.home_region, true);
            }
            Message::ToggleExpandSpotifyTrending => {
                self.home_spotify_expanded = !self.home_spotify_expanded;
            }
            Message::ToggleExpandYtmTrending => {
                self.home_ytm_expanded = !self.home_ytm_expanded;
            }
            Message::HomeSpotifyChartsLoaded { region, result } => {
                if region == self.home_region {
                    self.home_loading_spotify = false;
                }
                match result {
                    Ok(mut tracks) => {
                        let liked_uids: HashSet<String> = self
                            .liked_tracks
                            .iter()
                            .map(|t| t.uid.clone())
                            .collect();
                        for t in &mut tracks {
                            if liked_uids.contains(&t.uid) {
                                t.is_liked = true;
                            }
                        }
                        if !tracks.is_empty() {
                            if let Ok(db) = self.db.lock() {
                                let _ = db.put_cached_chart(
                                    &chart_cache_key("spotify", region),
                                    &tracks,
                                );
                            }
                        }
                        let keys = self.missing_art_keys(&tracks);
                        let snapshot = tracks.clone();
                        self.home_spotify_charts.insert(region, tracks);
                        return Task::batch([
                            self.preload_thumbnails_task(keys),
                            self.spawn_spotify_artwork_enrichment_task(&snapshot),
                        ]);
                    }
                    Err(err) => {
                        if region == self.home_region
                            && !self.home_spotify_charts.contains_key(&region)
                        {
                            self.status_toast = Some(if is_likely_offline_error(&err) {
                                "Offline — showing cached local library.".to_string()
                            } else {
                                format!("Spotify charts unavailable: {err}")
                            });
                        }
                    }
                }
            }
            Message::HomeYtmChartsLoaded { region, result } => {
                if region == self.home_region {
                    self.home_loading_ytm = false;
                }
                match result {
                    Ok(mut tracks) => {
                        let liked_uids: HashSet<String> = self
                            .liked_tracks
                            .iter()
                            .map(|t| t.uid.clone())
                            .collect();
                        for t in &mut tracks {
                            if liked_uids.contains(&t.uid) {
                                t.is_liked = true;
                            }
                        }
                        if !tracks.is_empty() {
                            if let Ok(db) = self.db.lock() {
                                let _ = db.put_cached_chart(
                                    &chart_cache_key("ytm", region),
                                    &tracks,
                                );
                            }
                        }
                        let keys = self.missing_art_keys(&tracks);
                        self.home_ytm_charts.insert(region, tracks);
                        return self.preload_thumbnails_task(keys);
                    }
                    Err(err) => {
                        if region == self.home_region
                            && !self.home_ytm_charts.contains_key(&region)
                        {
                            self.status_toast = Some(if is_likely_offline_error(&err) {
                                "Offline — showing cached local library.".to_string()
                            } else {
                                format!("YouTube Music charts unavailable: {err}")
                            });
                        }
                    }
                }
            }
            Message::SetViewMode(mode) => {
                self.player_menu_open = false;
                self.queue_open = false;
                self.view_mode = mode;
                self.last_scrolled_line = None;
                let mut tasks = Vec::new();
                if let Some(id) = self.main_window {
                    tasks.push(apply_window_glass_mode(
                        id,
                        self.config.glassmorphism_mode,
                        self.window_focused,
                    ));
                }
                if mode == ViewMode::NowPlayingLyrics {
                    if let Some(track) = self.queue.current().cloned() {
                        if self.current_lyrics_uid.as_deref() != Some(track.uid.as_str()) {
                            tasks.push(self.fetch_lyrics_task(track, false));
                        }
                    }
                }
                return Task::batch(tasks);
            }
            Message::ToggleQueueDrawer => {
                self.player_menu_open = false;
                self.queue_open = !self.queue_open;
                if self.queue_open {
                    let snapshot: Vec<Track> = self
                        .queue
                        .active_list()
                        .into_iter()
                        .cloned()
                        .collect();
                    let keys: Vec<String> = snapshot
                        .iter()
                        .filter_map(|t| t.artwork_ref.clone())
                        .collect();
                    return Task::batch([
                        self.preload_thumbnails_task(keys),
                        self.spawn_spotify_artwork_enrichment_task(&snapshot),
                    ]);
                }
            }
            Message::OpenNewPlaylistModal => {
                self.player_menu_open = false;
                self.show_new_playlist_modal = true;
                self.new_playlist_input.clear();
                self.modal_error = None;
            }
            Message::OpenImportUrlModal => {
                self.show_import_url_modal = true;
                self.import_url_input.clear();
                self.modal_error = None;
            }
            Message::CloseModal => {
                self.show_new_playlist_modal = false;
                self.show_import_url_modal = false;
                self.modal_error = None;
            }
            Message::NewPlaylistNameChanged(val) => {
                self.new_playlist_input = val;
                self.modal_error = None;
            }
            Message::CreatePlaylistSubmitted => {
                match validate_playlist_name(&self.new_playlist_input) {
                    Ok(name) => {
                        if let Ok(db) = self.db.lock() {
                            match db.create_playlist(&name) {
                                Ok(pl) => {
                                    self.playlists = db.list_playlists().unwrap_or_default();
                                    self.show_new_playlist_modal = false;
                                    self.modal_error = None;
                                    self.nav = ActiveNav::Playlist(pl.id);
                                    self.playlist_tracks.clear();
                                }
                                Err(e) => self.modal_error = Some(e.to_string()),
                            }
                        }
                    }
                    Err(msg) => self.modal_error = Some(msg.to_string()),
                }
            }
            Message::StartRenamePlaylist(id, name) => {
                self.renaming_playlist = Some((id, name));
            }
            Message::RenamePlaylistInputChanged(val) => {
                if let Some((_, ref mut draft)) = self.renaming_playlist {
                    *draft = val;
                }
            }
            Message::SubmitRenamePlaylist => {
                if let Some((id, draft)) = self.renaming_playlist.take() {
                    if let Ok(valid) = validate_playlist_name(&draft) {
                        if let Ok(db) = self.db.lock() {
                            match db.rename_playlist(&id, &valid) {
                                Ok(()) => {
                                    self.playlists = db.list_playlists().unwrap_or_default();
                                }
                                Err(e) => self.status_toast = Some(e.to_string()),
                            }
                        }
                    }
                }
            }
            Message::CancelRenamePlaylist => {
                self.renaming_playlist = None;
            }
            Message::DeletePlaylist(id) => {
                if let Ok(db) = self.db.lock() {
                    match db.delete_playlist(&id) {
                        Ok(()) => {
                            self.playlists = db.list_playlists().unwrap_or_default();
                            if self.playlist_sorts.remove(&id).is_some()
                                | self.config.playlist_sorts.remove(&id).is_some()
                            {
                                let _ = self.config.save(&self.config_path);
                            }
                            if matches!(&self.nav, ActiveNav::Playlist(cur) if cur == &id) {
                                self.nav = ActiveNav::LocalFiles;
                            }
                        }
                        Err(e) => self.status_toast = Some(e.to_string()),
                    }
                }
            }
            Message::ImportUrlInputChanged(val) => {
                self.import_url_input = val;
                self.modal_error = None;
            }
            Message::ImportSpotifyUrlSubmitted => {
                if self.importing_url {
                    return Task::none();
                }
                let url = self.import_url_input.trim().to_string();
                let playlist_id = match lume_providers::extract_spotify_playlist_id(&url) {
                    Ok(id) => id,
                    Err(err) => {
                        self.modal_error = Some(err.to_string());
                        return Task::none();
                    }
                };
                self.importing_url = true;
                self.modal_error = None;
                let db_path = self.db_path.clone();
                let web_api_ctx = if self.can_query_spotify_web_api() {
                    self.config
                        .spotify_client_id
                        .clone()
                        .zip(self.tokens.web_api.clone())
                } else {
                    None
                };

                return Task::perform(
                    async move {
                        let (name, source_url, tracks, used_embed_cap) =
                            if let Some((client_id, tok)) = web_api_ctx {
                                if let Ok((fresh, _)) = ensure_fresh_token(&client_id, tok).await {
                                    if let Ok((pl_name, pl_tracks)) = SpotifyWebClient::new()
                                        .get_playlist_full(&fresh.access_token, &playlist_id)
                                        .await
                                    {
                                        if !pl_tracks.is_empty() {
                                            (
                                                pl_name,
                                                canonical_playlist_url(&playlist_id),
                                                pl_tracks,
                                                false,
                                            )
                                        } else {
                                            let imp = import_spotify_playlist_url(&url)
                                                .await
                                                .map_err(|e| e.to_string())?;
                                            (imp.name, imp.source_url, imp.tracks, true)
                                        }
                                    } else {
                                        let imp = import_spotify_playlist_url(&url)
                                            .await
                                            .map_err(|e| e.to_string())?;
                                        (imp.name, imp.source_url, imp.tracks, true)
                                    }
                                } else {
                                    let imp = import_spotify_playlist_url(&url)
                                        .await
                                        .map_err(|e| e.to_string())?;
                                    (imp.name, imp.source_url, imp.tracks, true)
                                }
                            } else {
                                let imp = import_spotify_playlist_url(&url)
                                    .await
                                    .map_err(|e| e.to_string())?;
                                (imp.name, imp.source_url, imp.tracks, true)
                            };

                        let playlist = tokio::task::spawn_blocking(move || {
                            let db = Database::open(&db_path).map_err(|e| e.to_string())?;
                            db.upsert_imported_spotify_playlist(&name, &source_url, &tracks)
                                .map_err(|e| e.to_string())
                        })
                        .await
                        .map_err(|e| e.to_string())??;
                        Ok((playlist, used_embed_cap))
                    },
                    Message::SpotifyImportFinished,
                );
            }
            Message::PickExportifyCsv => {
                if self.importing_url {
                    return Task::none();
                }
                return Task::perform(
                    async move {
                        rfd::AsyncFileDialog::new()
                            .add_filter("Exportify CSV", &["csv"])
                            .pick_file()
                            .await
                            .map(|h| h.path().to_path_buf())
                    },
                    Message::ExportifyCsvPicked,
                );
            }
            Message::ExportifyCsvPicked(Some(path)) => {
                self.importing_url = true;
                self.modal_error = None;
                let db_path = self.db_path.clone();
                return Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            let csv_text = std::fs::read_to_string(&path)
                                .map_err(|e| format!("Failed to read CSV: {e}"))?;
                            let stem = path
                                .file_stem()
                                .and_then(|s| s.to_str())
                                .unwrap_or("Imported CSV Playlist");
                            let file_tag = path
                                .file_name()
                                .and_then(|s| s.to_str())
                                .unwrap_or("playlist.csv");
                            let imported = parse_exportify_csv(&csv_text, stem, file_tag)
                                .map_err(|e| e.to_string())?;
                            let db = Database::open(&db_path).map_err(|e| e.to_string())?;
                            let pl = db
                                .upsert_imported_spotify_playlist(
                                    &imported.name,
                                    &imported.source_url,
                                    &imported.tracks,
                                )
                                .map_err(|e| e.to_string())?;
                            Ok((pl, false))
                        })
                        .await
                        .map_err(|e| e.to_string())?
                    },
                    Message::SpotifyImportFinished,
                );
            }
            Message::ExportifyCsvPicked(None) => {}
            Message::SpotifyImportFinished(res) => {
                self.importing_url = false;
                match res {
                    Ok((playlist, used_embed_cap)) => {
                        self.show_import_url_modal = false;
                        self.import_url_input.clear();
                        self.modal_error = None;
                        self.enriching_spotify_art.clear();
                        if let Ok(db) = self.db.lock() {
                            self.playlists = db.list_playlists().unwrap_or_default();
                            self.playlist_tracks =
                                db.get_playlist_tracks(&playlist.id).unwrap_or_default();
                        }
                        self.nav = ActiveNav::Playlist(playlist.id.clone());
                        self.status_toast = Some(if used_embed_cap && playlist.track_count >= 95 {
                            format!(
                                "Imported \"{}\" ({} tracks — Spotify caps anonymous URLs at 100; connect Web API or use Exportify .csv for full list)",
                                playlist.name, playlist.track_count
                            )
                        } else {
                            format!(
                                "Imported \"{}\" ({} tracks) — loading original album covers...",
                                playlist.name, playlist.track_count
                            )
                        });
                        let keys = self.missing_art_keys(&self.playlist_tracks);
                        let tracks_snapshot = self.playlist_tracks.clone();
                        return Task::batch([
                            self.preload_thumbnails_task(keys),
                            self.spawn_spotify_artwork_enrichment_task(&tracks_snapshot),
                        ]);
                    }
                    Err(err) => {
                        self.modal_error = Some(err);
                    }
                }
            }
            Message::RematchYoutubeFallback(track) => {
                self.open_track_menu_uid = None;
                self.player_menu_open = false;
                if !matches!(track.source, TrackSource::Spotify { .. }) {
                    return Task::none();
                }
                let is_current = self
                    .queue
                    .current()
                    .map(|c| c.uid == track.uid)
                    .unwrap_or(false);
                if is_current {
                    self.play_request_id = self.play_request_id.wrapping_add(1);
                    let req_id = self.play_request_id;
                    self.resolving_stream = true;
                    self.source_used = Some(PlaybackSourceUsed::YouTubeFallback);
                    self.status = PlaybackStatus::Buffering;
                    self.status_toast =
                        Some(format!("Re-matching \"{}\" on YouTube Music...", track.title));
                    if let Some(audio) = &self.audio {
                        let _ = audio.send(AudioCommand::Stop);
                    }
                    self.smtc.set_playback(PlaybackStatus::Buffering, 0);
                    return self.resolve_spotify_fallback_stream_task(req_id, track, true);
                }

                self.status_toast =
                    Some(format!("Re-matching \"{}\" on YouTube Music...", track.title));
                let db = Arc::clone(&self.db);
                let ytm_res = self.get_ytm_provider();
                let track_for_msg = track.clone();
                return Task::perform(
                    async move {
                        let ytm = ytm_res?;
                        let matched = resolve_spotify_to_ytm_video_id(&track, &db, &ytm, true)
                            .await
                            .map_err(|e| e.to_string())?;
                        Ok(matched.yt_video_id)
                    },
                    move |result| Message::RematchFinished {
                        track: track_for_msg,
                        result,
                    },
                );
            }
            Message::RematchFinished { track, result } => match result {
                Ok(video_id) => {
                    self.status_toast = Some(format!(
                        "Updated YouTube fallback match for \"{}\" ({video_id})",
                        track.title
                    ));
                }
                Err(err) => {
                    self.status_toast = Some(format!("Re-match failed: {err}"));
                }
            },
            Message::AddTrackToPlaylist { playlist_id, track } => {
                self.open_track_menu_uid = None;
                self.player_menu_open = false;
                if let Ok(db) = self.db.lock() {
                    let _ = db.upsert_track(&track, None);
                    match db.add_track_to_playlist(&playlist_id, &track.uid) {
                        Ok(()) => {
                            self.playlists = db.list_playlists().unwrap_or_default();
                            if matches!(&self.nav, ActiveNav::Playlist(cur) if cur == &playlist_id) {
                                self.playlist_tracks =
                                    db.get_playlist_tracks(&playlist_id).unwrap_or_default();
                            }
                            self.status_toast = Some(format!("Added \"{}\" to playlist", track.title));
                        }
                        Err(e) => self.status_toast = Some(e.to_string()),
                    }
                }
            }
            Message::RemoveTrackFromPlaylist {
                playlist_id,
                position,
            } => {
                self.open_track_menu_uid = None;
                if let Ok(db) = self.db.lock() {
                    match db.remove_track_from_playlist(&playlist_id, position) {
                        Ok(()) => {
                            self.playlists = db.list_playlists().unwrap_or_default();
                            self.playlist_tracks =
                                db.get_playlist_tracks(&playlist_id).unwrap_or_default();
                        }
                        Err(e) => self.status_toast = Some(e.to_string()),
                    }
                }
            }
            Message::ReorderPlaylistTrack {
                playlist_id,
                from,
                to,
            } => {
                if let Ok(db) = self.db.lock() {
                    match db.reorder_playlist_track(&playlist_id, from, to) {
                        Ok(()) => {
                            self.playlist_tracks =
                                db.get_playlist_tracks(&playlist_id).unwrap_or_default();
                        }
                        Err(e) => self.status_toast = Some(e.to_string()),
                    }
                }
            }
            Message::SetPlaylistSort(field) => {
                if let ActiveNav::Playlist(pl_id) = self.nav.clone() {
                    let sort = self.playlist_sorts.entry(pl_id.clone()).or_default();
                    sort.select(field);
                    self.config.playlist_sorts.insert(pl_id.clone(), *sort);
                    let _ = self.config.save(&self.config_path);
                    let ordered = self.ordered_playlist_tracks(&pl_id);
                    let keys = self.missing_art_keys(&ordered);
                    return Task::batch([
                        self.preload_thumbnails_task(keys),
                        self.spawn_spotify_artwork_enrichment_task(&ordered),
                    ]);
                }
            }
            Message::TogglePlaylistSortDirection => {
                if let ActiveNav::Playlist(pl_id) = self.nav.clone() {
                    let sort = self.playlist_sorts.entry(pl_id.clone()).or_default();
                    sort.toggle_direction();
                    self.config.playlist_sorts.insert(pl_id.clone(), *sort);
                    let _ = self.config.save(&self.config_path);
                    let ordered = self.ordered_playlist_tracks(&pl_id);
                    let keys = self.missing_art_keys(&ordered);
                    return Task::batch([
                        self.preload_thumbnails_task(keys),
                        self.spawn_spotify_artwork_enrichment_task(&ordered),
                    ]);
                }
            }
            Message::ToggleTrackMenu(uid) => {
                if self.open_track_menu_uid.as_deref() == Some(uid.as_str()) {
                    self.open_track_menu_uid = None;
                } else {
                    self.open_track_menu_uid = Some(uid);
                }
            }
            Message::CloseTrackMenu => {
                self.open_track_menu_uid = None;
            }
            Message::TogglePlayerMenu => {
                self.open_track_menu_uid = None;
                if self.queue.current().is_some() {
                    self.player_menu_open = !self.player_menu_open;
                } else {
                    self.player_menu_open = false;
                }
            }
            Message::ClosePlayerMenu => {
                self.player_menu_open = false;
            }
            Message::SearchInputChanged(val) => {
                self.search_query = val;
                self.search_debounce_seq = self.search_debounce_seq.wrapping_add(1);
                let trimmed = self.search_query.trim().to_string();
                if trimmed.is_empty() {
                    self.search_request_id = self.search_request_id.wrapping_add(1);
                    self.last_remote_search_query.clear();
                    self.spotify_search_results.clear();
                    self.ytm_search_results.clear();
                    self.spotify_search_artists.clear();
                    self.spotify_search_playlists.clear();
                    self.ytm_search_artists.clear();
                    self.ytm_search_playlists.clear();
                    self.searching_spotify = false;
                    self.searching_ytm = false;
                    self.refresh_search_results();
                    return Task::none();
                }

                self.refresh_search_results();
                if !matches!(self.nav, ActiveNav::Search) {
                    self.nav = ActiveNav::Search;
                }

                if trimmed.chars().count() >= SEARCH_MIN_LIVE_CHARS
                    && !trimmed.eq_ignore_ascii_case(&self.last_remote_search_query)
                {
                    let seq = self.search_debounce_seq;
                    return Task::perform(
                        async move {
                            tokio::time::sleep(Duration::from_millis(SEARCH_DEBOUNCE_MS)).await;
                            (seq, trimmed)
                        },
                        |(seq, query)| Message::SearchDebounceFired { seq, query },
                    );
                }
            }
            Message::SearchDebounceFired { seq, query } => {
                if seq != self.search_debounce_seq {
                    return Task::none();
                }
                let current = self.search_query.trim();
                if current.is_empty()
                    || current != query
                    || current.eq_ignore_ascii_case(&self.last_remote_search_query)
                {
                    return Task::none();
                }
                self.last_remote_search_query = query.clone();
                return self.trigger_remote_search_tasks(query);
            }
            Message::SearchSubmitted => {
                self.search_debounce_seq = self.search_debounce_seq.wrapping_add(1);
                self.nav = ActiveNav::Search;
                self.refresh_search_results();
                let query = self.search_query.trim().to_string();
                if !query.is_empty() {
                    self.last_remote_search_query = query.clone();
                    return self.trigger_remote_search_tasks(query);
                }
            }
            Message::SearchTabChanged(tab) => {
                self.search_tab = tab;
            }
            Message::SourceFilterChanged(filter) => {
                self.source_filter = filter;
                let query = self.search_query.trim().to_string();
                if matches!(self.nav, ActiveNav::Search) && !query.is_empty() {
                    let spotify_ok = self.can_query_spotify_web_api();
                    let need_ytm = (matches!(filter, SourceFilter::YouTubeMusic | SourceFilter::All)
                        || (filter == SourceFilter::Spotify && !spotify_ok))
                        && self.ytm_search_results.is_empty()
                        && self.ytm_search_artists.is_empty()
                        && self.ytm_search_playlists.is_empty()
                        && !self.searching_ytm;
                    let need_spotify = matches!(filter, SourceFilter::Spotify | SourceFilter::All)
                        && spotify_ok
                        && self.spotify_search_results.is_empty()
                        && self.spotify_search_artists.is_empty()
                        && self.spotify_search_playlists.is_empty()
                        && !self.searching_spotify;
                    if need_ytm || need_spotify {
                        return self.trigger_remote_search_tasks(query);
                    }
                }
            }
            Message::YtmSearchFinished { request_id, result } => {
                if request_id != self.search_request_id {
                    return Task::none();
                }
                self.searching_ytm = false;
                match result {
                    Ok(mut catalog) => {
                        self.apply_liked_flags(&mut catalog.tracks);
                        let mut keys = self.missing_art_keys(&catalog.tracks);
                        keys.extend(self.missing_art_urls(
                            catalog
                                .artists
                                .iter()
                                .filter_map(|a| a.avatar_url.as_deref())
                                .chain(
                                    catalog
                                        .playlists
                                        .iter()
                                        .filter_map(|p| p.cover_url.as_deref()),
                                ),
                        ));
                        self.ytm_search_results = catalog.tracks;
                        self.ytm_search_artists = catalog.artists;
                        self.ytm_search_playlists = catalog.playlists;
                        self.refresh_search_results();
                        return self.preload_thumbnails_task(keys);
                    }
                    Err(err) => {
                        self.status_toast = Some(format!("YouTube Music search failed: {err}"));
                    }
                }
            }
            Message::SpotifySearchFinished {
                request_id,
                result,
                refreshed_token,
            } => {
                if let Some(tok) = refreshed_token {
                    self.tokens.web_api = Some(tok);
                    let _ = self.token_store.save(&self.tokens);
                }
                if request_id != self.search_request_id {
                    return Task::none();
                }
                self.searching_spotify = false;
                match result {
                    Ok(mut catalog) => {
                        let cache_key = self.search_query.trim().to_ascii_lowercase();
                        if !cache_key.is_empty()
                            && (!catalog.tracks.is_empty()
                                || !catalog.artists.is_empty()
                                || !catalog.playlists.is_empty())
                        {
                            let now = Instant::now();
                            if self.spotify_search_cache.len() >= SPOTIFY_SEARCH_CACHE_MAX_ENTRIES {
                                self.spotify_search_cache
                                    .retain(|_, (exp, _)| *exp > now);
                                if self.spotify_search_cache.len()
                                    >= SPOTIFY_SEARCH_CACHE_MAX_ENTRIES
                                {
                                    if let Some(oldest) = self
                                        .spotify_search_cache
                                        .iter()
                                        .min_by_key(|(_, (exp, _))| *exp)
                                        .map(|(k, _)| k.clone())
                                    {
                                        self.spotify_search_cache.remove(&oldest);
                                    }
                                }
                            }
                            self.spotify_search_cache.insert(
                                cache_key,
                                (now + SPOTIFY_SEARCH_CACHE_TTL, catalog.clone()),
                            );
                        }
                        self.apply_liked_flags(&mut catalog.tracks);
                        let mut keys = self.missing_art_keys(&catalog.tracks);
                        keys.extend(self.missing_art_urls(
                            catalog
                                .artists
                                .iter()
                                .filter_map(|a| a.avatar_url.as_deref())
                                .chain(
                                    catalog
                                        .playlists
                                        .iter()
                                        .filter_map(|p| p.cover_url.as_deref()),
                                ),
                        ));
                        self.spotify_search_results = catalog.tracks;
                        self.spotify_search_artists = catalog.artists;
                        self.spotify_search_playlists = catalog.playlists;
                        self.refresh_search_results();
                        return self.preload_thumbnails_task(keys);
                    }
                    Err(SpotifySearchError::LapsedOrForbidden) => {
                        self.spotify_lapsed = true;
                        self.spotify_search_results.clear();
                        self.spotify_search_artists.clear();
                        self.spotify_search_playlists.clear();
                        self.refresh_search_results();
                        self.status_toast = Some(
                            "Spotify Web API unavailable (Premium lapsed/403) — falling back to YouTube Music."
                                .to_string(),
                        );
                        // If user had filtered exclusively to Spotify, automatically query YTM fallback.
                        let query = self.search_query.trim().to_string();
                        if self.source_filter == SourceFilter::Spotify
                            && !query.is_empty()
                            && self.ytm_search_results.is_empty()
                            && !self.searching_ytm
                        {
                            self.searching_ytm = true;
                            return self.spawn_ytm_search_task(query, request_id);
                        }
                    }
                    Err(SpotifySearchError::Other(err)) => {
                        self.status_toast = Some(format!("Spotify search failed: {err}"));
                    }
                }
            }
            Message::OpenArtist(summary) => {
                self.open_track_menu_uid = None;
                self.player_menu_open = false;
                self.view_mode = ViewMode::Browse;
                let next = ActiveNav::ArtistProfile {
                    source: summary.source,
                    id: summary.id.clone(),
                    name: summary.name.clone(),
                };
                push_nav_history(&mut self.nav_history, self.nav.clone(), &next);
                self.nav = next;
                return self.start_loading_artist(summary);
            }
            Message::OpenArtistByName { name, source_hint } => {
                let clean_name = name.trim().to_string();
                if clean_name.is_empty() || clean_name.eq_ignore_ascii_case("unknown artist") {
                    return Task::none();
                }
                self.open_track_menu_uid = None;
                self.player_menu_open = false;
                self.view_mode = ViewMode::Browse;

                let cache_key = clean_name.to_ascii_lowercase();
                if let Some(cached) = self.artist_name_cache.get(&cache_key).cloned() {
                    let usable = cached.source == CatalogSource::YouTubeMusic
                        || self.can_query_spotify_web_api();
                    if usable {
                        let next = ActiveNav::ArtistProfile {
                            source: cached.source,
                            id: cached.id.clone(),
                            name: cached.name.clone(),
                        };
                        push_nav_history(&mut self.nav_history, self.nav.clone(), &next);
                        self.nav = next;
                        return self.start_loading_artist(cached);
                    }
                }

                let prefer_spotify = matches!(source_hint, TrackSource::Spotify { .. })
                    && self.can_query_spotify_web_api();
                let next_source = if prefer_spotify {
                    CatalogSource::Spotify
                } else {
                    CatalogSource::YouTubeMusic
                };
                let next = ActiveNav::ArtistProfile {
                    source: next_source,
                    id: format!("lookup:{cache_key}"),
                    name: clean_name.clone(),
                };
                push_nav_history(&mut self.nav_history, self.nav.clone(), &next);
                self.nav = next;
                self.active_artist_profile = None;
                self.artist_error = None;
                self.artist_tracks_expanded = false;
                self.loading_artist = true;
                self.artist_request_id = self.artist_request_id.wrapping_add(1);
                return self.spawn_load_artist_by_name_task(
                    self.artist_request_id,
                    clean_name,
                    prefer_spotify,
                );
            }
            Message::ToggleExpandArtistTracks => {
                self.artist_tracks_expanded = !self.artist_tracks_expanded;
            }
            Message::ArtistProfileLoaded {
                request_id,
                result,
                refreshed_token,
                fallback_notice,
                spotify_lapsed,
            } => {
                if let Some(tok) = refreshed_token {
                    self.tokens.web_api = Some(tok);
                    let _ = self.token_store.save(&self.tokens);
                }
                if spotify_lapsed {
                    self.spotify_lapsed = true;
                }
                if request_id != self.artist_request_id {
                    return Task::none();
                }
                self.loading_artist = false;
                if let Some(notice) = fallback_notice {
                    self.status_toast = Some(notice);
                }
                match result {
                    Ok(mut profile) => {
                        self.apply_liked_flags(&mut profile.top_tracks);
                        self.artist_name_cache.insert(
                            profile.name.trim().to_ascii_lowercase(),
                            ArtistSummary {
                                id: profile.id.clone(),
                                source: profile.source,
                                name: profile.name.clone(),
                                avatar_url: profile.avatar_url.clone(),
                                subtitle: profile.subscriber_label.clone(),
                            },
                        );
                        if matches!(self.nav, ActiveNav::ArtistProfile { .. }) {
                            self.nav = ActiveNav::ArtistProfile {
                                source: profile.source,
                                id: profile.id.clone(),
                                name: profile.name.clone(),
                            };
                        }
                        let mut keys = self.missing_art_keys(&profile.top_tracks);
                        keys.extend(self.missing_art_urls(
                            profile
                                .avatar_url
                                .as_deref()
                                .into_iter()
                                .chain(profile.albums.iter().filter_map(|a| a.cover_url.as_deref()))
                                .chain(profile.singles.iter().filter_map(|a| a.cover_url.as_deref()))
                                .chain(
                                    profile
                                        .playlists
                                        .iter()
                                        .filter_map(|p| p.cover_url.as_deref()),
                                ),
                        ));
                        let enrich_snapshot = profile.top_tracks.clone();
                        self.active_artist_profile = Some(profile);
                        self.artist_error = None;
                        return Task::batch([
                            self.preload_thumbnails_task(keys),
                            self.spawn_spotify_artwork_enrichment_task(&enrich_snapshot),
                        ]);
                    }
                    Err(err) => {
                        self.artist_error = Some(err);
                    }
                }
            }
            Message::OpenAlbum(summary) => {
                self.open_track_menu_uid = None;
                self.player_menu_open = false;
                let next = ActiveNav::AlbumDetail {
                    source: summary.source,
                    id: summary.id.clone(),
                    title: summary.title.clone(),
                };
                push_nav_history(&mut self.nav_history, self.nav.clone(), &next);
                self.nav = next;
                return self.start_loading_album(summary);
            }
            Message::AlbumDetailLoaded {
                request_id,
                result,
                refreshed_token,
                spotify_lapsed,
            } => {
                if let Some(tok) = refreshed_token {
                    self.tokens.web_api = Some(tok);
                    let _ = self.token_store.save(&self.tokens);
                }
                if spotify_lapsed {
                    self.spotify_lapsed = true;
                }
                if request_id != self.album_request_id {
                    return Task::none();
                }
                self.loading_album = false;
                match result {
                    Ok(mut detail) => {
                        self.apply_liked_flags(&mut detail.tracks);
                        let mut keys = self.missing_art_keys(&detail.tracks);
                        keys.extend(self.missing_art_urls(detail.cover_url.as_deref().into_iter()));
                        self.active_album_detail = Some(detail);
                        self.album_error = None;
                        return self.preload_thumbnails_task(keys);
                    }
                    Err(err) => {
                        self.album_error = Some(err);
                    }
                }
            }
            Message::OpenRemotePlaylist(summary) => {
                self.open_track_menu_uid = None;
                self.player_menu_open = false;
                let next = ActiveNav::RemotePlaylist {
                    source: summary.source,
                    id: summary.id.clone(),
                    name: summary.name.clone(),
                };
                push_nav_history(&mut self.nav_history, self.nav.clone(), &next);
                self.nav = next;
                return self.start_loading_remote_playlist(summary);
            }
            Message::RemotePlaylistLoaded {
                request_id,
                result,
                refreshed_token,
                spotify_lapsed,
            } => {
                if let Some(tok) = refreshed_token {
                    self.tokens.web_api = Some(tok);
                    let _ = self.token_store.save(&self.tokens);
                }
                if spotify_lapsed {
                    self.spotify_lapsed = true;
                }
                if request_id != self.remote_playlist_request_id {
                    return Task::none();
                }
                self.loading_remote_playlist = false;
                match result {
                    Ok(mut detail) => {
                        self.apply_liked_flags(&mut detail.tracks);
                        let mut keys = self.missing_art_keys(&detail.tracks);
                        keys.extend(self.missing_art_urls(detail.cover_url.as_deref().into_iter()));
                        let enrich_snapshot = detail.tracks.clone();
                        self.active_remote_playlist = Some(detail);
                        self.remote_playlist_error = None;
                        return Task::batch([
                            self.preload_thumbnails_task(keys),
                            self.spawn_spotify_artwork_enrichment_task(&enrich_snapshot),
                        ]);
                    }
                    Err(err) => {
                        self.remote_playlist_error = Some(err);
                    }
                }
            }
            Message::SaveRemotePlaylistToLibrary => {
                let Some(detail) = self.active_remote_playlist.clone() else {
                    return Task::none();
                };
                if self.saving_remote_playlist || detail.tracks.is_empty() {
                    return Task::none();
                }
                self.saving_remote_playlist = true;
                let db_path = self.db_path.clone();
                return Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            let db = Database::open(&db_path).map_err(|e| e.to_string())?;
                            match detail.source {
                                CatalogSource::Spotify => {
                                    let existed = db.list_playlists().ok().is_some_and(|pls| {
                                        pls.iter().any(|p| {
                                            matches!(
                                                &p.origin,
                                                lume_core::PlaylistOrigin::SpotifyImported {
                                                    source_url
                                                } if source_url == &detail.source_url
                                            )
                                        })
                                    });
                                    let pl = db
                                        .upsert_imported_spotify_playlist(
                                            &detail.name,
                                            &detail.source_url,
                                            &detail.tracks,
                                        )
                                        .map_err(|e| e.to_string())?;
                                    Ok((pl, existed))
                                }
                                CatalogSource::YouTubeMusic => db
                                    .upsert_imported_ytm_playlist(
                                        &detail.name,
                                        &detail.source_url,
                                        &detail.tracks,
                                    )
                                    .map_err(|e| e.to_string()),
                            }
                        })
                        .await
                        .map_err(|e| e.to_string())?
                    },
                    Message::RemotePlaylistSaved,
                );
            }
            Message::RemotePlaylistSaved(res) => {
                self.saving_remote_playlist = false;
                match res {
                    Ok((pl, updated)) => {
                        if let Ok(db) = self.db.lock() {
                            self.playlists = db.list_playlists().unwrap_or_default();
                        }
                        let verb = if updated { "Updated" } else { "Saved" };
                        self.status_toast = Some(format!(
                            "{verb} \"{}\" ({} songs) in your playlists.",
                            pl.name, pl.track_count
                        ));
                    }
                    Err(err) => {
                        self.status_toast = Some(format!("Failed to save playlist: {err}"));
                    }
                }
            }
            Message::PlayTrackList {
                tracks,
                start_index,
            } => {
                self.open_track_menu_uid = None;
                if let Some(track) = self.queue.set_tracks(tracks, start_index).cloned() {
                    return self.start_playing_track(track);
                }
            }
            Message::PlayNext(track) => {
                self.open_track_menu_uid = None;
                self.player_menu_open = false;
                let was_empty = self.queue.current().is_none();
                let title = track.title.clone();
                self.queue.play_next(track.clone());
                self.persist_session();
                if was_empty {
                    return self.start_playing_track(track);
                }
                self.status_toast = Some(format!("Playing \"{title}\" next"));
            }
            Message::AddToQueue(track) => {
                self.open_track_menu_uid = None;
                self.player_menu_open = false;
                let was_empty = self.queue.current().is_none();
                let title = track.title.clone();
                self.queue.add_to_queue(track.clone());
                self.persist_session();
                if was_empty {
                    return self.start_playing_track(track);
                }
                self.status_toast = Some(format!("Added \"{title}\" to queue"));
            }
            Message::ToggleLike(track) => {
                let next_liked = !track.is_liked;
                if let Ok(db) = self.db.lock() {
                    let _ = db.upsert_track(&track, None);
                    let _ = db.set_local_favorite(&track.uid, next_liked);
                    self.local_tracks = db.list_local_tracks().unwrap_or_default();
                    self.liked_tracks = db.list_unified_liked_tracks().unwrap_or_default();
                    if let ActiveNav::Playlist(pl_id) = &self.nav {
                        self.playlist_tracks = db.get_playlist_tracks(pl_id).unwrap_or_default();
                    }
                }
                for t in &mut self.ytm_search_results {
                    if t.uid == track.uid {
                        t.is_liked = next_liked;
                    }
                }
                for t in &mut self.spotify_search_results {
                    if t.uid == track.uid {
                        t.is_liked = next_liked;
                    }
                }
                for t in &mut self.recent_tracks {
                    if t.uid == track.uid {
                        t.is_liked = next_liked;
                    }
                }
                for list in self.home_spotify_charts.values_mut() {
                    for t in list {
                        if t.uid == track.uid {
                            t.is_liked = next_liked;
                        }
                    }
                }
                for list in self.home_ytm_charts.values_mut() {
                    for t in list {
                        if t.uid == track.uid {
                            t.is_liked = next_liked;
                        }
                    }
                }
                if let Some(profile) = &mut self.active_artist_profile {
                    for t in &mut profile.top_tracks {
                        if t.uid == track.uid {
                            t.is_liked = next_liked;
                        }
                    }
                }
                if let Some(album) = &mut self.active_album_detail {
                    for t in &mut album.tracks {
                        if t.uid == track.uid {
                            t.is_liked = next_liked;
                        }
                    }
                }
                if let Some(pl) = &mut self.active_remote_playlist {
                    for t in &mut pl.tracks {
                        if t.uid == track.uid {
                            t.is_liked = next_liked;
                        }
                    }
                }
                self.refresh_search_results();
                // Reflect on currently active queue snapshot if matching.
                let mut snap = self.queue.to_snapshot();
                for t in &mut snap.natural_order {
                    if t.uid == track.uid {
                        t.is_liked = next_liked;
                    }
                }
                self.queue = QueueEngine::from_snapshot(snap);

                // Sync Spotify Like/Unlike via `/me/library` or queue in `pending_mutations`.
                if let TrackSource::Spotify { id: sp_id } = track.source {
                    if self.can_query_spotify_web_api() {
                        if let (Some(client_id), Some(tok)) = (
                            self.config.spotify_client_id.clone(),
                            self.tokens.web_api.clone(),
                        ) {
                            return Task::perform(
                                async move {
                                    let (tok, refreshed) =
                                        match ensure_fresh_token(&client_id, tok).await {
                                            Ok(pair) => pair,
                                            Err(_) => {
                                                return Message::SpotifyLikeSynced {
                                                    spotify_track_id: sp_id,
                                                    liked: next_liked,
                                                    lapsed: false,
                                                    failed: true,
                                                    refreshed_token: None,
                                                };
                                            }
                                        };
                                    let client = SpotifyWebClient::new();
                                    let ids = vec![sp_id.clone()];
                                    let res = if next_liked {
                                        client.save_tracks(&tok.access_token, &ids).await
                                    } else {
                                        client.remove_tracks(&tok.access_token, &ids).await
                                    };
                                    let refreshed_token = refreshed.then_some(tok);
                                    match res {
                                        Ok(()) => Message::SpotifyLikeSynced {
                                            spotify_track_id: sp_id,
                                            liked: next_liked,
                                            lapsed: false,
                                            failed: false,
                                            refreshed_token,
                                        },
                                        Err(SpotifyApiError::PremiumRequiredOrForbidden(_)) => {
                                            Message::SpotifyLikeSynced {
                                                spotify_track_id: sp_id,
                                                liked: next_liked,
                                                lapsed: true,
                                                failed: true,
                                                refreshed_token,
                                            }
                                        }
                                        Err(_) => Message::SpotifyLikeSynced {
                                            spotify_track_id: sp_id,
                                            liked: next_liked,
                                            lapsed: false,
                                            failed: true,
                                            refreshed_token,
                                        },
                                    }
                                },
                                |msg| msg,
                            );
                        }
                    } else if let Ok(db) = self.db.lock() {
                        let action = if next_liked { "like" } else { "unlike" };
                        let _ = db.enqueue_pending_mutation(action, &sp_id);
                        self.pending_mutations_count = db.count_pending_mutations().unwrap_or(0);
                    }
                }
            }
            Message::TogglePlayPause => {
                return self.handle_play_pause_toggle();
            }
            Message::NextTrack => match self.queue.next() {
                QueueActionResult::Play(track) => return self.start_playing_track(track),
                QueueActionResult::RestartCurrent => {
                    self.position_ms = 0;
                    if let Some(audio) = &self.audio {
                        let _ = audio.send(AudioCommand::Seek(0));
                    }
                }
                QueueActionResult::Stopped => {
                    self.stop_playback();
                }
            },
            Message::PreviousTrack => match self.queue.previous(self.position_ms) {
                QueueActionResult::Play(track) => return self.start_playing_track(track),
                QueueActionResult::RestartCurrent => {
                    self.position_ms = 0;
                    if let Some(audio) = &self.audio {
                        let _ = audio.send(AudioCommand::Seek(0));
                    }
                    self.smtc.set_playback(self.status, 0);
                }
                QueueActionResult::Stopped => self.stop_playback(),
            },
            Message::ToggleShuffle => {
                let next = !self.queue.is_shuffle();
                self.queue.set_shuffle(next);
                self.persist_session();
            }
            Message::CycleRepeat => {
                let next = self.queue.repeat_mode().next();
                self.queue.set_repeat(next);
                self.persist_session();
            }
            Message::SeekSliderChanged(pos_ms) | Message::SeekTo(pos_ms) => {
                self.position_ms = pos_ms;
                self.last_audio_pos_ms = pos_ms;
                self.last_audio_pos_at = Instant::now();
                if let Some(audio) = &self.audio {
                    let _ = audio.send(AudioCommand::Seek(pos_ms));
                }
                self.smtc.set_playback(self.status, pos_ms);
            }
            Message::SetVolume(vol) => {
                self.volume = vol.clamp(0.0, 1.0);
                if self.muted && self.volume > 0.0 {
                    self.muted = false;
                    if let Some(audio) = &self.audio {
                        let _ = audio.send(AudioCommand::SetMute(false));
                    }
                }
                if let Some(audio) = &self.audio {
                    let _ = audio.send(AudioCommand::SetVolume(self.volume));
                }
                self.persist_session();
            }
            Message::ToggleMute => {
                self.muted = !self.muted;
                if let Some(audio) = &self.audio {
                    let _ = audio.send(AudioCommand::SetMute(self.muted));
                }
                self.persist_session();
            }
            Message::QueueJumpTo(idx) => {
                if let QueueActionResult::Play(track) = self.queue.jump_to(idx) {
                    return self.start_playing_track(track);
                }
            }
            Message::QueueMove { from, to } => {
                self.queue.reorder(from, to);
                self.persist_session();
            }
            Message::QueueRemoveAt(idx) => {
                let was_active = self.queue.current_pos() == Some(idx);
                let res = self.queue.remove_at(idx);
                self.persist_session();
                if was_active {
                    match res {
                        QueueActionResult::Play(track) => return self.start_playing_track(track),
                        QueueActionResult::RestartCurrent => {}
                        QueueActionResult::Stopped => self.stop_playback(),
                    }
                }
            }
            Message::ClearQueue => {
                self.queue.clear();
                self.stop_playback();
                self.persist_session();
            }
            Message::RetryLyrics => {
                if let Some(track) = self.queue.current().cloned() {
                    return self.fetch_lyrics_task(track, true);
                }
            }
            Message::LyricsLoaded { track_uid, result } => {
                if self.queue.current().map(|t| t.uid.as_str()) == Some(track_uid.as_str()) {
                    self.lyrics_loading = false;
                    match result {
                        Ok(data) => {
                            self.cached_lyrics_timeline = data
                                .as_ref()
                                .map(|d| build_display_timeline(&d.synced))
                                .unwrap_or_default();
                            self.current_lyrics = data;
                            self.current_lyrics_uid = Some(track_uid);
                            self.last_scrolled_line = None;
                        }
                        Err(err) => {
                            self.current_lyrics = None;
                            self.cached_lyrics_timeline.clear();
                            self.status_toast = Some(if is_likely_offline_error(&err) {
                                "Offline — lyrics unavailable (not cached yet).".to_string()
                            } else {
                                format!("Lyrics unavailable: {err}")
                            });
                        }
                    }
                }
            }
            Message::StreamResolved {
                request_id,
                track,
                video_id,
                result,
            } => {
                if request_id != self.play_request_id {
                    return Task::none();
                }
                self.resolving_stream = false;
                match result {
                    Ok(ResolvedStream::HttpUrl { url, source }) => {
                        self.source_used = Some(source);
                        self.active_http_fallback =
                            video_id.map(|vid| (request_id, vid, track.clone()));
                        if let Some(audio) = &self.audio {
                            let _ = audio.send(AudioCommand::LoadHttpStream {
                                url,
                                track_uid: track.uid,
                                duration_ms: track.duration_ms,
                                mime_hint: Some("m4a".to_string()),
                            });
                        }
                    }
                    Ok(ResolvedStream::YtDlpProcess {
                        binary,
                        watch_url,
                        format_selector,
                        source,
                    }) => {
                        self.active_http_fallback = None;
                        self.source_used = Some(source);
                        if let Some(audio) = &self.audio {
                            let _ = audio.send(AudioCommand::LoadYtDlpStream {
                                binary,
                                video_url: watch_url,
                                format_selector,
                                track_uid: track.uid,
                                duration_ms: track.duration_ms,
                                mime_hint: Some("m4a".to_string()),
                            });
                        }
                    }
                    Ok(ResolvedStream::LocalFile(path)) => {
                        self.active_http_fallback = None;
                        self.source_used = Some(PlaybackSourceUsed::LocalFile);
                        if let Some(audio) = &self.audio {
                            let _ = audio.send(AudioCommand::LoadLocal {
                                path,
                                track_uid: track.uid,
                                duration_ms: track.duration_ms,
                            });
                        }
                    }
                    Ok(ResolvedStream::SpotifyTrack { .. }) => {
                        self.active_http_fallback = None;
                        self.resolving_stream = true;
                        return self.prepare_spotify_playback_token_task(request_id, track);
                    }
                    Err(err) => {
                        self.active_http_fallback = None;
                        self.status = PlaybackStatus::Stopped;
                        self.smtc.set_playback(PlaybackStatus::Stopped, 0);
                        self.status_toast = Some(format_ytm_stream_error_toast(
                            &err,
                            self.yt_dlp_detected.is_some(),
                        ));
                    }
                }
            }
            Message::PaletteLoaded {
                track_uid,
                art_key,
                palette,
                cover_rgba,
                thumb_rgba,
                art_blur_rgba,
            } => {
                let handle_key = art_key.clone().unwrap_or_else(|| track_uid.clone());
                if let Some((w, h, rgba)) = thumb_rgba {
                    self.insert_art_handle(handle_key.clone(), image::Handle::from_rgba(w, h, rgba));
                }
                if self.queue.current().map(|t| t.uid.as_str()) == Some(track_uid.as_str()) {
                    self.active_cover_handle = cover_rgba
                        .map(|(w, h, rgba)| (handle_key.clone(), image::Handle::from_rgba(w, h, rgba)));
                    if let Some(track) = self.queue.current() {
                        let cover_path = self
                            .art_cache
                            .lock()
                            .ok()
                            .and_then(|c| c.art_path(&handle_key));
                        if cover_path.is_some() {
                            self.smtc.set_metadata(Some(track), cover_path.as_deref());
                        }
                    }
                    self.art_blur_rgba = art_blur_rgba;
                    self.art_blur_version = self.art_blur_version.wrapping_add(1);
                    self.prev_palette = self.display_palette;
                    self.target_palette = palette;
                    if self.config.reduced_motion {
                        self.display_palette = palette;
                        self.palette_transition_start = None;
                    } else {
                        self.palette_transition_start = Some(Instant::now());
                    }
                }
            }
            Message::ThumbnailsLoaded(items) => {
                for (key, w, h, rgba) in items {
                    self.insert_art_handle(key, image::Handle::from_rgba(w, h, rgba));
                }
            }
            Message::SpotifyArtworkEnriched {
                attempted_uids,
                resolved: pairs,
            } => {
                let updated_map: HashMap<String, String> = pairs.into_iter().collect();
                // Release any UID that failed (e.g. HTTP 429 or transient offline) so it can retry later.
                for uid in attempted_uids {
                    if !updated_map.contains_key(&uid) {
                        self.enriching_spotify_art.remove(&uid);
                    }
                }
                if updated_map.is_empty() {
                    return Task::none();
                }
                let mut new_keys = Vec::with_capacity(updated_map.len());

                for (idx, t) in self.playlist_tracks.iter_mut().enumerate() {
                    if let Some(url) = updated_map.get(&t.uid) {
                        t.artwork_ref = Some(url.clone());
                        if idx < MAX_IN_MEM_ART_HANDLES {
                            new_keys.push(url.clone());
                        }
                    }
                }
                for (idx, t) in self.liked_tracks.iter_mut().enumerate() {
                    if let Some(url) = updated_map.get(&t.uid) {
                        t.artwork_ref = Some(url.clone());
                        if idx < MAX_IN_MEM_ART_HANDLES {
                            new_keys.push(url.clone());
                        }
                    }
                }
                if let Some(profile) = &mut self.active_artist_profile {
                    for t in &mut profile.top_tracks {
                        if let Some(url) = updated_map.get(&t.uid) {
                            t.artwork_ref = Some(url.clone());
                            new_keys.push(url.clone());
                        }
                    }
                }
                if let Some(pl) = &mut self.active_remote_playlist {
                    for t in &mut pl.tracks {
                        if let Some(url) = updated_map.get(&t.uid) {
                            t.artwork_ref = Some(url.clone());
                            new_keys.push(url.clone());
                        }
                    }
                }
                for t in &mut self.spotify_search_results {
                    if let Some(url) = updated_map.get(&t.uid) {
                        t.artwork_ref = Some(url.clone());
                    }
                }
                for t in &mut self.recent_tracks {
                    if let Some(url) = updated_map.get(&t.uid) {
                        t.artwork_ref = Some(url.clone());
                        new_keys.push(url.clone());
                    }
                }
                let mut enriched_chart_regions = Vec::new();
                for (&region, list) in self.home_spotify_charts.iter_mut() {
                    let mut changed = false;
                    for t in list {
                        if let Some(url) = updated_map.get(&t.uid) {
                            t.artwork_ref = Some(url.clone());
                            new_keys.push(url.clone());
                            changed = true;
                        }
                    }
                    if changed {
                        enriched_chart_regions.push(region);
                    }
                }
                if !enriched_chart_regions.is_empty() {
                    if let Ok(db) = self.db.lock() {
                        for region in enriched_chart_regions {
                            if let Some(tracks) = self.home_spotify_charts.get(&region) {
                                let _ = db.update_cached_chart_tracks(
                                    &chart_cache_key("spotify", region),
                                    tracks,
                                );
                            }
                        }
                    }
                }
                self.refresh_search_results();

                let mut snap = self.queue.to_snapshot();
                let mut active_updated = false;
                let active_uid = self.queue.current().map(|t| t.uid.clone());
                for t in &mut snap.natural_order {
                    if let Some(url) = updated_map.get(&t.uid) {
                        t.artwork_ref = Some(url.clone());
                        if active_uid.as_deref() == Some(t.uid.as_str()) {
                            active_updated = true;
                        }
                    }
                }
                self.queue = QueueEngine::from_snapshot(snap);

                let mut tasks = Vec::new();
                new_keys.sort();
                new_keys.dedup();
                if !new_keys.is_empty() {
                    tasks.push(self.preload_thumbnails_task(new_keys));
                }
                if active_updated {
                    if let Some(current) = self.queue.current().cloned() {
                        tasks.push(self.load_palette_task(&current));
                    }
                }
                // Continue enriching any remaining visible tracks in the current view.
                let remaining_snapshot = match &self.nav {
                    ActiveNav::Home => self
                        .home_spotify_charts
                        .get(&self.home_region)
                        .cloned()
                        .unwrap_or_default(),
                    ActiveNav::Playlist(pl_id) => self.ordered_playlist_tracks(pl_id),
                    ActiveNav::LikedSongs => self.liked_tracks.clone(),
                    _ => Vec::new(),
                };
                if !remaining_snapshot.is_empty() {
                    tasks.push(self.spawn_spotify_artwork_enrichment_task(&remaining_snapshot));
                }
                return Task::batch(tasks);
            }
            Message::SetGlassmorphismMode(mode) => {
                self.config.glassmorphism_mode = mode;
                let _ = self.config.save(&self.config_path);
                if let Some(id) = self.main_window {
                    return apply_window_glass_mode(id, mode, self.window_focused);
                }
            }
            Message::SetLyricsVisualMode(mode) => {
                self.config.lyrics_visual_mode = mode;
                let _ = self.config.save(&self.config_path);
            }
            Message::SetReducedMotion(enabled) => {
                self.config.reduced_motion = enabled;
                if enabled {
                    self.display_palette = self.target_palette;
                    self.palette_transition_start = None;
                }
                let _ = self.config.save(&self.config_path);
            }
            Message::SetPreferYoutubeFallback(enabled) => {
                self.config.prefer_youtube_fallback = enabled;
                let _ = self.config.save(&self.config_path);
            }
            Message::SpotifyClientIdChanged(val) => {
                self.spotify_client_id_input = val;
            }
            Message::SaveSpotifyClientId => {
                let trimmed = self.spotify_client_id_input.trim().to_string();
                self.config.spotify_client_id = (!trimmed.is_empty()).then_some(trimmed);
                let _ = self.config.save(&self.config_path);
                self.status_toast = Some(if self.config.spotify_client_id.is_some() {
                    "Saved Spotify Developer Client ID.".to_string()
                } else {
                    "Cleared Spotify Developer Client ID.".to_string()
                });
            }
            Message::ConnectSpotifyWebApi => {
                if self.spotify_logging_in_web {
                    return Task::none();
                }
                let trimmed = self.spotify_client_id_input.trim().to_string();
                if trimmed.is_empty() {
                    self.status_toast = Some(
                        "Enter your Spotify Developer Client ID in Settings first.".to_string(),
                    );
                    return Task::none();
                }
                self.config.spotify_client_id = Some(trimmed.clone());
                let _ = self.config.save(&self.config_path);
                self.spotify_logging_in_web = true;
                self.status_toast =
                    Some("Opened browser for Spotify Web API sign-in...".to_string());
                return Task::perform(
                    async move {
                        let tok = login_spotify_web_api(&trimmed)
                            .await
                            .map_err(|e| e.to_string())?;
                        let profile = SpotifyWebClient::new()
                            .get_profile(&tok.access_token)
                            .await
                            .unwrap_or(SpotifyUserProfile {
                                id: "spotify".into(),
                                display_name: "Spotify User".into(),
                                product: SpotifyAccountTier::Unknown,
                            });
                        Ok((tok, profile))
                    },
                    Message::SpotifyWebApiLoginFinished,
                );
            }
            Message::SpotifyWebApiLoginFinished(res) => {
                self.spotify_logging_in_web = false;
                match res {
                    Ok((tok, profile)) => {
                        self.tokens.web_api = Some(tok);
                        let _ = self.token_store.save(&self.tokens);
                        self.spotify_lapsed = false;
                        self.status_toast = Some(format!(
                            "Connected Spotify Library as {} — syncing playlists...",
                            profile.display_name
                        ));
                        self.spotify_profile = Some(profile);
                        self.spotify_syncing = true;
                        return self.spawn_spotify_sync_task();
                    }
                    Err(err) => {
                        self.status_toast = Some(format!("Spotify Web API login failed: {err}"));
                    }
                }
            }
            Message::DisconnectSpotifyWebApi => {
                self.tokens.web_api = None;
                let _ = self.token_store.save(&self.tokens);
                self.spotify_profile = None;
                self.spotify_lapsed = false;
                self.spotify_search_results.clear();
                self.spotify_sync_summary = None;
                self.refresh_search_results();
                self.status_toast = Some(
                    "Disconnected Spotify Web API (cached playlists remain playable via YTM Fallback)."
                        .to_string(),
                );
            }
            Message::ConnectSpotifyPlayback => {
                if self.spotify_logging_in_playback {
                    return Task::none();
                }
                self.spotify_logging_in_playback = true;
                self.status_toast = Some(
                    "Opened browser for Spotify native playback (librespot) sign-in...".to_string(),
                );
                return Task::perform(
                    async move { login_spotify_playback().await.map_err(|e| e.to_string()) },
                    Message::SpotifyPlaybackLoginFinished,
                );
            }
            Message::SpotifyPlaybackLoginFinished(res) => {
                self.spotify_logging_in_playback = false;
                match res {
                    Ok(tok) => {
                        self.tokens.playback = Some(tok);
                        let _ = self.token_store.save(&self.tokens);
                        self.spotify_lapsed = false;
                        self.status_toast =
                            Some("Connected Spotify native playback (librespot 320 kbps).".to_string());
                    }
                    Err(err) => {
                        self.status_toast =
                            Some(format!("Spotify playback login failed: {err}"));
                    }
                }
            }
            Message::DisconnectSpotifyPlayback => {
                self.tokens.playback = None;
                let _ = self.token_store.save(&self.tokens);
                if let Some(audio) = &self.audio {
                    let _ = audio.send(AudioCommand::DisconnectSpotify);
                }
                self.status_toast = Some(
                    "Disconnected Spotify native playback (Spotify tracks will use Smart YTM Fallback)."
                        .to_string(),
                );
            }
            Message::SyncSpotifyLibrary => {
                if self.spotify_syncing {
                    return Task::none();
                }
                if self.tokens.web_api.is_none() {
                    self.status_toast = Some("Connect Spotify Library first.".to_string());
                    return Task::none();
                }
                self.spotify_syncing = true;
                return self.spawn_spotify_sync_task();
            }
            Message::SpotifySyncFinished(res) => {
                self.spotify_syncing = false;
                match res {
                    Ok(report) => {
                        if let Some(tok) = report.refreshed_token {
                            self.tokens.web_api = Some(tok);
                            let _ = self.token_store.save(&self.tokens);
                        }
                        if let Some(profile) = report.profile {
                            self.spotify_profile = Some(profile);
                        }
                        self.spotify_lapsed = report.lapsed;
                        if let Ok(db) = self.db.lock() {
                            self.playlists = db.list_playlists().unwrap_or_default();
                            self.liked_tracks = db.list_unified_liked_tracks().unwrap_or_default();
                            self.pending_mutations_count =
                                db.count_pending_mutations().unwrap_or(0);
                            if let ActiveNav::Playlist(pl_id) = &self.nav {
                                self.playlist_tracks =
                                    db.get_playlist_tracks(pl_id).unwrap_or_default();
                            }
                        }
                        self.refresh_search_results();
                        if report.lapsed {
                            self.spotify_sync_summary = Some(
                                "Spotify API returned 403 (Premium lapsed/restricted) — serving cached SQLite library via Smart YTM Fallback."
                                    .to_string(),
                            );
                        } else {
                            self.spotify_sync_summary = Some(format!(
                                "Synced {} playlists • {} liked songs • {} queued likes flushed",
                                report.synced_playlists,
                                report.synced_liked,
                                report.flushed_mutations
                            ));
                        }
                        let keys = self.missing_art_keys(&self.liked_tracks);
                        return self.preload_thumbnails_task(keys);
                    }
                    Err(err) => {
                        self.spotify_sync_summary = Some(format!("Sync error: {err}"));
                    }
                }
            }
            Message::SpotifyPlaybackTokenReady {
                request_id,
                track,
                result,
            } => {
                if request_id != self.play_request_id {
                    return Task::none();
                }
                match result {
                    Ok((tok, refreshed)) => {
                        let access = tok.access_token.clone();
                        if refreshed {
                            self.tokens.playback = Some(tok);
                            let _ = self.token_store.save(&self.tokens);
                        }
                        if let TrackSource::Spotify { id: sp_id } = &track.source {
                            self.resolving_stream = false;
                            self.source_used = Some(PlaybackSourceUsed::SpotifyNative);
                            if let Some(audio) = &self.audio {
                                let _ = audio.send(AudioCommand::LoadSpotify {
                                    spotify_id: sp_id.clone(),
                                    track_uid: track.uid,
                                    duration_ms: track.duration_ms,
                                    access_token: access,
                                });
                            }
                        }
                    }
                    Err(err) => {
                        self.source_used = Some(PlaybackSourceUsed::YouTubeFallback);
                        self.status_toast = Some(format!(
                            "Spotify playback token unavailable ({err}) — using Smart YouTube Fallback."
                        ));
                        return self.resolve_spotify_fallback_stream_task(request_id, track, false);
                    }
                }
            }
            Message::SpotifyLikeSynced {
                spotify_track_id,
                liked,
                lapsed,
                failed,
                refreshed_token,
            } => {
                if let Some(tok) = refreshed_token {
                    self.tokens.web_api = Some(tok);
                    let _ = self.token_store.save(&self.tokens);
                }
                if lapsed {
                    self.spotify_lapsed = true;
                }
                if failed {
                    if let Ok(db) = self.db.lock() {
                        let action = if liked { "like" } else { "unlike" };
                        let _ = db.enqueue_pending_mutation(action, &spotify_track_id);
                        self.pending_mutations_count = db.count_pending_mutations().unwrap_or(0);
                    }
                }
            }
            Message::PickYtDlpBinary => {
                return Task::perform(
                    async move {
                        rfd::AsyncFileDialog::new()
                            .add_filter("yt-dlp executable", &["exe", "cmd", "bat"])
                            .pick_file()
                            .await
                            .map(|h| h.path().to_path_buf())
                    },
                    Message::YtDlpPicked,
                );
            }
            Message::YtDlpPicked(Some(path)) => {
                self.config.yt_dlp_path = Some(path);
                let _ = self.config.save(&self.config_path);
                self.yt_dlp_detected = detect_yt_dlp_binary(self.config.yt_dlp_path.as_deref());
                self.rebuild_ytm_provider();
                self.status_toast = Some("Saved yt-dlp fallback path.".to_string());
            }
            Message::YtDlpPicked(None) => {}
            Message::ClearYtDlpBinary => {
                self.config.yt_dlp_path = None;
                let _ = self.config.save(&self.config_path);
                self.yt_dlp_detected = detect_yt_dlp_binary(None);
                self.rebuild_ytm_provider();
                self.status_toast = Some("Cleared custom yt-dlp path (using PATH auto-detect).".to_string());
            }
            Message::PickLocalFolder => {
                return Task::perform(
                    async move {
                        rfd::AsyncFileDialog::new()
                            .pick_folder()
                            .await
                            .map(|h| h.path().to_path_buf())
                    },
                    Message::FolderPicked,
                );
            }
            Message::FolderPicked(Some(folder)) => {
                if !self.config.local_folders.contains(&folder) {
                    self.config.local_folders.push(folder.clone());
                    let _ = self.config.save(&self.config_path);
                }
                if let Ok(db) = self.db.lock() {
                    let _ = db.add_local_folder(&folder);
                }
                self.scanning_folders = true;
                return self.spawn_local_scan_task();
            }
            Message::FolderPicked(None) => {}
            Message::RemoveLocalFolder(folder) => {
                self.config.local_folders.retain(|f| f != &folder);
                let _ = self.config.save(&self.config_path);
                if let Ok(db) = self.db.lock() {
                    let _ = db.remove_local_folder(&folder);
                }
                self.scanning_folders = true;
                return self.spawn_local_scan_task();
            }
            Message::RescanLocalFolders => {
                if !self.scanning_folders {
                    self.scanning_folders = true;
                    return self.spawn_local_scan_task();
                }
            }
            Message::LocalScanFinished(res) => {
                self.scanning_folders = false;
                match res {
                    Ok((report, tracks)) => {
                        let thumb_keys: Vec<String> = tracks
                            .iter()
                            .filter_map(|t| t.artwork_ref.clone())
                            .take(MAX_IN_MEM_ART_HANDLES)
                            .collect();
                        self.local_tracks = tracks;
                        self.refresh_search_results();
                        self.scan_summary = Some(format!(
                            "Scanned {} files • {} indexed • {} corrupt skipped • {} pruned",
                            report.scanned,
                            report.added_or_updated,
                            report.skipped_corrupt,
                            report.pruned
                        ));
                        return self.preload_thumbnails_task(thumb_keys);
                    }
                    Err(err) => {
                        self.scan_summary = Some(format!("Scan failed: {err}"));
                    }
                }
            }
            Message::Audio(ev) => match ev {
                AudioEvent::StateChanged(st) => {
                    if st == PlaybackStatus::Stopped && self.resolving_stream {
                        // Ignore stale Stop event from cancelling the previous track while resolving next stream.
                        return Task::none();
                    }
                    if st == PlaybackStatus::Playing {
                        self.active_http_fallback = None;
                    }
                    self.status = st;
                    self.smtc.set_playback(st, self.position_ms);
                }
                AudioEvent::PositionTick {
                    position_ms,
                    duration_ms,
                } => {
                    // Monotonic clock smoothing: avoid tiny 10-80ms backward snaps between
                    // 100ms audio engine ticks and 60 FPS UI interpolation so Bloom lyric
                    // sweeps never micro-stutter, while still honoring genuine seeks/drifts.
                    let smoothed = if self.status == PlaybackStatus::Playing
                        && position_ms < self.position_ms
                        && self.position_ms.saturating_sub(position_ms) <= 90
                    {
                        self.position_ms
                    } else {
                        position_ms
                    };
                    self.position_ms = smoothed;
                    self.last_audio_pos_ms = smoothed;
                    self.last_audio_pos_at = Instant::now();
                    if duration_ms > 0 {
                        self.duration_ms = duration_ms;
                    }
                    // Deferred next-track prefetch: only pre-resolve once the current song has
                    // actually played for >= 15 s (or <= 25 s remain), avoiding wasted YouTube
                    // API requests when rapidly skipping through a playlist.
                    if self.status == PlaybackStatus::Playing
                        && self.play_request_id > 0
                        && self.prefetched_for_play_id != self.play_request_id
                        && (self.position_ms >= PREFETCH_MIN_PLAYED_MS
                            || (self.duration_ms > 10_000
                                && self.duration_ms.saturating_sub(self.position_ms)
                                    <= PREFETCH_REMAINING_WINDOW_MS))
                    {
                        self.prefetched_for_play_id = self.play_request_id;
                        if let Some(next_track) = self.queue.peek_next().cloned() {
                            if self
                                .queue
                                .current()
                                .is_none_or(|cur| cur.uid != next_track.uid)
                            {
                                return self.spawn_prefetch_next_track_task(next_track);
                            }
                        }
                    }
                }
                AudioEvent::TrackEnded { .. } => match self.queue.next() {
                    QueueActionResult::Play(track) => return self.start_playing_track(track),
                    QueueActionResult::RestartCurrent => {
                        if let Some(track) = self.queue.current().cloned() {
                            return self.start_playing_track(track);
                        }
                    }
                    QueueActionResult::Stopped => self.stop_playback(),
                },
                AudioEvent::SpotifyFallbackNeeded { track_uid, reason } => {
                    if is_spotify_lapsed_reason(&reason) {
                        self.spotify_lapsed = true;
                    }
                    if let Some(current) = self.queue.current().cloned() {
                        if current.uid == track_uid
                            && matches!(current.source, TrackSource::Spotify { .. })
                        {
                            self.play_request_id = self.play_request_id.wrapping_add(1);
                            let req_id = self.play_request_id;
                            self.resolving_stream = true;
                            self.source_used = Some(PlaybackSourceUsed::YouTubeFallback);
                            self.status = PlaybackStatus::Buffering;
                            self.status_toast = Some(reason);
                            self.smtc.set_playback(PlaybackStatus::Buffering, 0);
                            return self.resolve_spotify_fallback_stream_task(req_id, current, false);
                        }
                    }
                }
                AudioEvent::Error(err) => {
                    if let Some((req_id, video_id, track)) = self.active_http_fallback.take() {
                        if req_id == self.play_request_id {
                            if let Ok(ytm) = self.get_ytm_provider() {
                                ytm.invalidate_stream_cache(&video_id);
                                if let Some(ResolvedStream::YtDlpProcess {
                                    binary,
                                    watch_url,
                                    format_selector,
                                    ..
                                }) = ytm.yt_dlp_fallback_for_video_id(&video_id)
                                {
                                    self.status = PlaybackStatus::Buffering;
                                    self.smtc.set_playback(PlaybackStatus::Buffering, 0);
                                    if let Some(audio) = &self.audio {
                                        let _ = audio.send(AudioCommand::LoadYtDlpStream {
                                            binary,
                                            video_url: watch_url,
                                            format_selector,
                                            track_uid: track.uid,
                                            duration_ms: track.duration_ms,
                                            mime_hint: Some("m4a".to_string()),
                                        });
                                    }
                                    return Task::none();
                                }
                            }
                        }
                    }
                    self.status = PlaybackStatus::Stopped;
                    self.smtc.set_playback(PlaybackStatus::Stopped, 0);
                    self.status_toast = Some(format_ytm_stream_error_toast(
                        &err,
                        self.yt_dlp_detected.is_some(),
                    ));
                }
            },
            Message::Smtc(action) => match action {
                SmtcAction::Play => {
                    if self.status == PlaybackStatus::Paused {
                        if let Some(audio) = &self.audio {
                            let _ = audio.send(AudioCommand::Resume);
                        }
                    } else if self.status == PlaybackStatus::Stopped {
                        if let Some(track) = self.queue.current().cloned() {
                            return self.start_playing_track(track);
                        }
                    }
                }
                SmtcAction::Pause => {
                    if self.status == PlaybackStatus::Playing {
                        if let Some(audio) = &self.audio {
                            let _ = audio.send(AudioCommand::Pause);
                        }
                    }
                }
                SmtcAction::Toggle => return self.handle_play_pause_toggle(),
                SmtcAction::Next => return self.update(Message::NextTrack),
                SmtcAction::Previous => return self.update(Message::PreviousTrack),
                SmtcAction::Stop => self.stop_playback(),
            },
            Message::DismissToast => {
                self.status_toast = None;
                self.toast_seq = self.toast_seq.wrapping_add(1);
            }
            Message::ExpireToast(seq) => {
                if seq == self.toast_seq {
                    self.status_toast = None;
                }
            }
            Message::BackgroundTaskDone => {}
        }
        Task::none()
    }

    pub fn view(&self) -> Element<'_, Message> {
        let has_active_playback = self.queue.current().is_some()
            && matches!(
                self.status,
                PlaybackStatus::Playing | PlaybackStatus::Buffering | PlaybackStatus::Paused
            );
        let accent = if has_active_playback {
            self.display_palette.accent_theme()
        } else {
            AccentTheme::default_cool()
        };

        let info = NowPlayingInfo {
            track: self.queue.current(),
            status: self.status,
            source_used: self.source_used,
            position_ms: self.position_ms,
            duration_ms: self.duration_ms,
            shuffle: self.queue.is_shuffle(),
            repeat: self.queue.repeat_mode(),
            volume: self.volume,
            muted: self.muted,
        };

        let active_cover = self.active_cover_handle.as_ref().and_then(|(k, h)| {
            let cur_key = self
                .queue
                .current()
                .and_then(|t| t.artwork_ref.as_deref().or(Some(t.uid.as_str())));
            (cur_key == Some(k.as_str())).then_some(h)
        });

        let foreground: Element<'_, Message> = match self.view_mode {
            ViewMode::NowPlayingLyrics => {
                let base_lyrics: Element<'_, Message> = view_now_playing_lyrics(LyricsViewProps {
                    info,
                    lyrics: self.current_lyrics.as_ref(),
                    lyrics_timeline: &self.cached_lyrics_timeline,
                    lyrics_loading: self.lyrics_loading,
                    lyrics_mode: self.config.lyrics_visual_mode,
                    active_scale: self.lyrics_spring.active_scale(),
                    visual_active_idx: self.lyrics_spring.visual_active_idx(),
                    window_width: self.window_width,
                    window_height: self.window_height,
                    active_cover,
                    art_handles: &self.art_handles,
                    time_s: self.time_s,
                    queue_open: self.queue_open,
                    queue_tracks: self.queue.active_list(),
                    queue_current_pos: self.queue.current_pos(),
                    status_banner: self.status_toast.as_deref(),
                    accent,
                });

                if self.player_menu_open {
                    if let Some(current_track) = self.queue.current() {
                        stack![
                            base_lyrics,
                            mouse_area(
                                container(iced::widget::space().width(Length::Fill).height(Length::Fill))
                                    .width(Length::Fill)
                                    .height(Length::Fill)
                            )
                            .on_press(Message::ClosePlayerMenu),
                            container(view_player_menu_popover(
                                current_track,
                                &self.playlists,
                                accent,
                            ))
                            .width(Length::Fill)
                            .height(Length::Fill)
                            .align_x(iced::alignment::Horizontal::Left)
                            .align_y(iced::alignment::Vertical::Bottom)
                            .padding(iced::Padding {
                                top: 48.0,
                                right: 16.0,
                                bottom: 96.0,
                                left: 48.0,
                            })
                        ]
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .into()
                    } else {
                        base_lyrics
                    }
                } else {
                    base_lyrics
                }
            }
            ViewMode::Browse => {
                let titlebar = view_titlebar(
                    self.source_used.map(|s| s.badge_label()),
                    &self.search_query,
                    self.source_filter,
                    self.queue_open,
                    self.window_width,
                    accent,
                );

                let connection_status = format_connection_status_pill(
                    self.tokens.web_api.is_some(),
                    self.tokens.playback.is_some(),
                    self.spotify_account_tier(),
                    self.spotify_lapsed,
                    self.config.prefer_youtube_fallback,
                );

                let center_content: Element<'_, Message> = if matches!(self.nav, ActiveNav::Settings)
                {
                    row![
                        view_sidebar(
                            &self.nav,
                            &self.playlists,
                            connection_status,
                            self.window_width,
                            accent,
                        ),
                        view_settings(
                            &self.config,
                            self.scanning_folders,
                            self.scan_summary.as_deref(),
                            self.yt_dlp_detected.as_deref(),
                            SpotifySettingsProps {
                                client_id_input: &self.spotify_client_id_input,
                                web_api_connected: self.tokens.web_api.is_some(),
                                playback_connected: self.tokens.playback.is_some(),
                                profile_name: self
                                    .spotify_profile
                                    .as_ref()
                                    .map(|p| p.display_name.as_str()),
                                account_tier: self.spotify_account_tier(),
                                lapsed_fallback: self.spotify_lapsed,
                                syncing: self.spotify_syncing,
                                logging_in_web: self.spotify_logging_in_web,
                                logging_in_playback: self.spotify_logging_in_playback,
                                pending_mutations: self.pending_mutations_count,
                                sync_summary: self.spotify_sync_summary.as_deref(),
                            },
                            accent,
                        ),
                    ]
                    .spacing(14)
                    .padding([2, 16])
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .into()
                } else {
                    let visible_tracks = self.visible_tracks();
                    let is_playing = matches!(
                        self.status,
                        PlaybackStatus::Playing | PlaybackStatus::Buffering
                    );
                    let playlist_sort = match &self.nav {
                        ActiveNav::Playlist(pl_id) => self
                            .playlist_sorts
                            .get(pl_id)
                            .copied()
                            .unwrap_or_default(),
                        _ => PlaylistSort::default(),
                    };
                    let home_spotify_trending = self
                        .home_spotify_charts
                        .get(&self.home_region)
                        .map(Vec::as_slice)
                        .unwrap_or(&[]);
                    let home_ytm_trending = self
                        .home_ytm_charts
                        .get(&self.home_region)
                        .map(Vec::as_slice)
                        .unwrap_or(&[]);
                    let remote_playlist_is_saved = self
                        .active_remote_playlist
                        .as_ref()
                        .is_some_and(|rp| {
                            self.playlists.iter().any(|p| match &p.origin {
                                lume_core::PlaylistOrigin::SpotifyImported { source_url }
                                | lume_core::PlaylistOrigin::YouTubeImported { source_url } => {
                                    source_url == &rp.source_url
                                }
                                _ => false,
                            })
                        });
                    view_browse(BrowseViewProps {
                        nav: &self.nav,
                        window_width: self.window_width,
                        can_go_back: !self.nav_history.is_empty(),
                        search_query: &self.search_query,
                        search_tab: self.search_tab,
                        source_filter: self.source_filter,
                        spotify_web_api_active: self.can_query_spotify_web_api(),
                        playlist_sort,
                        searching_remote: self.searching_ytm || self.searching_spotify,
                        playlists: &self.playlists,
                        visible_tracks,
                        spotify_search_artists: &self.spotify_search_artists,
                        spotify_search_playlists: &self.spotify_search_playlists,
                        ytm_search_artists: &self.ytm_search_artists,
                        ytm_search_playlists: &self.ytm_search_playlists,
                        active_artist_profile: self.active_artist_profile.as_ref(),
                        loading_artist: self.loading_artist,
                        artist_error: self.artist_error.as_deref(),
                        artist_tracks_expanded: self.artist_tracks_expanded,
                        active_album_detail: self.active_album_detail.as_ref(),
                        loading_album: self.loading_album,
                        album_error: self.album_error.as_deref(),
                        active_remote_playlist: self.active_remote_playlist.as_ref(),
                        loading_remote_playlist: self.loading_remote_playlist,
                        remote_playlist_error: self.remote_playlist_error.as_deref(),
                        saving_remote_playlist: self.saving_remote_playlist,
                        remote_playlist_is_saved,
                        recent_tracks: &self.recent_tracks,
                        home_region: self.home_region,
                        home_spotify_trending,
                        home_ytm_trending,
                        home_loading_spotify: self.home_loading_spotify,
                        home_loading_ytm: self.home_loading_ytm,
                        home_spotify_expanded: self.home_spotify_expanded,
                        home_ytm_expanded: self.home_ytm_expanded,
                        active_track_uid: self.queue.current().map(|t| t.uid.as_str()),
                        active_track: self.queue.current(),
                        is_playing,
                        open_track_menu_uid: self.open_track_menu_uid.as_deref(),
                        renaming_playlist: self.renaming_playlist.as_ref(),
                        art_handles: &self.art_handles,
                        show_new_playlist_modal: self.show_new_playlist_modal,
                        new_playlist_input: &self.new_playlist_input,
                        show_import_url_modal: self.show_import_url_modal,
                        import_url_input: &self.import_url_input,
                        importing_url: self.importing_url,
                        connection_status,
                        modal_error: self.modal_error.as_deref(),
                        status_banner: self.status_toast.as_deref(),
                        accent,
                    })
                };

                let player_bar = view_player_bar(
                    info,
                    active_cover,
                    &self.art_handles,
                    self.queue_open,
                    self.player_menu_open,
                    self.window_width,
                    accent,
                );

                let main_col = iced::widget::column![
                    titlebar,
                    container(center_content)
                        .width(Length::Fill)
                        .height(Length::Fill),
                    player_bar,
                ]
                .width(Length::Fill)
                .height(Length::Fill);

                let base_browse: Element<'_, Message> = if self.queue_open {
                    let active_list = self.queue.active_list();
                    stack![
                        main_col,
                        mouse_area(
                            container(
                                iced::widget::space()
                                    .width(Length::Fill)
                                    .height(Length::Fill)
                            )
                            .width(Length::Fill)
                            .height(Length::Fill)
                        )
                        .on_press(Message::ToggleQueueDrawer),
                        container(iced::widget::opaque(view_queue_drawer(
                            &active_list,
                            self.queue.current_pos(),
                            &self.art_handles,
                            self.window_width,
                            accent,
                        )))
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .align_x(iced::alignment::Horizontal::Right)
                        .padding(iced::Padding {
                            top: 52.0,
                            right: 16.0,
                            bottom: 92.0,
                            left: 16.0,
                        })
                    ]
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .into()
                } else {
                    main_col.into()
                };

                if self.player_menu_open {
                    if let Some(current_track) = self.queue.current() {
                        stack![
                            base_browse,
                            mouse_area(
                                container(iced::widget::space().width(Length::Fill).height(Length::Fill))
                                    .width(Length::Fill)
                                    .height(Length::Fill)
                            )
                            .on_press(Message::ClosePlayerMenu),
                            container(view_player_menu_popover(
                                current_track,
                                &self.playlists,
                                accent,
                            ))
                            .width(Length::Fill)
                            .height(Length::Fill)
                            .align_x(iced::alignment::Horizontal::Left)
                            .align_y(iced::alignment::Vertical::Bottom)
                            .padding(iced::Padding {
                                top: 48.0,
                                right: 16.0,
                                bottom: 92.0,
                                left: 24.0,
                            })
                        ]
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .into()
                    } else {
                        base_browse
                    }
                } else {
                    base_browse
                }
            }
        };

        let outer_pad = if self.maximized { 0 } else { 4 };
        let has_playing_art = has_active_playback && self.art_blur_rgba.is_some();

        // Ultra-clear crystal glass in Browse; lush full-bleed Spicy Lyrics album blur in NowPlayingLyrics.
        let backdrop_cfg: Option<(f32, f32)> = match self.config.glassmorphism_mode {
            GlassmorphismMode::PureClear => match self.view_mode {
                ViewMode::Browse => {
                    if has_playing_art {
                        Some((0.07, 0.12))
                    } else {
                        None
                    }
                }
                ViewMode::NowPlayingLyrics => {
                    Some((0.50, if has_playing_art { 0.62 } else { 0.0 }))
                }
            },
            GlassmorphismMode::WindowsAcrylic => match self.view_mode {
                ViewMode::Browse => Some((
                    if has_playing_art { 0.11 } else { 0.045 },
                    if has_playing_art { 0.15 } else { 0.0 },
                )),
                ViewMode::NowPlayingLyrics => {
                    Some((0.68, if has_playing_art { 0.76 } else { 0.0 }))
                }
            },
            GlassmorphismMode::DynamicAlbumBlur => match self.view_mode {
                ViewMode::Browse => Some((
                    if has_playing_art { 0.34 } else { 0.08 },
                    if has_playing_art { 0.48 } else { 0.0 },
                )),
                ViewMode::NowPlayingLyrics => {
                    Some((0.86, if has_playing_art { 0.88 } else { 0.0 }))
                }
            },
        };

        let root_content: Element<'_, Message> = if let Some((backdrop_alpha, art_tint_strength)) =
            backdrop_cfg
        {
            let backdrop = shader(DynamicBackdropProgram::new(
                self.display_palette,
                Some(backdrop_alpha),
                if has_playing_art {
                    self.art_blur_rgba.clone()
                } else {
                    None
                },
                self.art_blur_version,
                art_tint_strength,
            ))
            .width(Length::Fill)
            .height(Length::Fill);

            let padded_fg = container(foreground)
                .width(Length::Fill)
                .height(Length::Fill)
                .padding(outer_pad);

            stack![backdrop, padded_fg]
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
        } else {
            container(foreground)
                .width(Length::Fill)
                .height(Length::Fill)
                .padding(outer_pad)
                .into()
        };

        container(root_content)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(|_| container::Style {
                background: Some(Background::Color(Color::TRANSPARENT)),
                ..Default::default()
            })
            .into()
    }

    fn visible_tracks(&self) -> &[Track] {
        match &self.nav {
            ActiveNav::Home => &self.recent_tracks,
            ActiveNav::Search => &self.search_results,
            ActiveNav::LikedSongs => &self.liked_tracks,
            ActiveNav::LocalFiles => &self.local_tracks,
            ActiveNav::Playlist(_) => &self.playlist_tracks,
            ActiveNav::ArtistProfile { .. } => self
                .active_artist_profile
                .as_ref()
                .map(|p| p.top_tracks.as_slice())
                .unwrap_or(&[]),
            ActiveNav::AlbumDetail { .. } => self
                .active_album_detail
                .as_ref()
                .map(|a| a.tracks.as_slice())
                .unwrap_or(&[]),
            ActiveNav::RemotePlaylist { .. } => self
                .active_remote_playlist
                .as_ref()
                .map(|p| p.tracks.as_slice())
                .unwrap_or(&[]),
            ActiveNav::Settings => &self.local_tracks,
        }
    }

    fn apply_liked_flags(&self, tracks: &mut [Track]) {
        let liked_uids: HashSet<&str> = self
            .liked_tracks
            .iter()
            .map(|t| t.uid.as_str())
            .collect();
        for t in tracks {
            if liked_uids.contains(t.uid.as_str()) {
                t.is_liked = true;
            }
        }
    }

    fn missing_art_urls<'a>(&self, urls: impl Iterator<Item = &'a str>) -> Vec<String> {
        let mut seen = HashSet::new();
        let mut out = Vec::new();
        for url in urls {
            let trimmed = url.trim();
            if !trimmed.is_empty()
                && !self.art_handles.contains_key(trimmed)
                && seen.insert(trimmed.to_string())
            {
                out.push(trimmed.to_string());
                if out.len() >= MAX_IN_MEM_ART_HANDLES {
                    break;
                }
            }
        }
        out
    }

    fn start_loading_artist(&mut self, summary: ArtistSummary) -> Task<Message> {
        self.active_artist_profile = None;
        self.artist_error = None;
        self.artist_tracks_expanded = false;
        self.loading_artist = true;
        self.artist_request_id = self.artist_request_id.wrapping_add(1);
        self.spawn_load_artist_task(self.artist_request_id, summary)
    }

    fn start_loading_album(&mut self, summary: AlbumSummary) -> Task<Message> {
        self.active_album_detail = None;
        self.album_error = None;
        self.loading_album = true;
        self.album_request_id = self.album_request_id.wrapping_add(1);
        self.spawn_load_album_task(self.album_request_id, summary)
    }

    fn start_loading_remote_playlist(&mut self, summary: RemotePlaylistSummary) -> Task<Message> {
        self.active_remote_playlist = None;
        self.remote_playlist_error = None;
        self.saving_remote_playlist = false;
        self.loading_remote_playlist = true;
        self.remote_playlist_request_id = self.remote_playlist_request_id.wrapping_add(1);
        self.spawn_load_remote_playlist_task(self.remote_playlist_request_id, summary)
    }

    fn spotify_account_tier(&self) -> SpotifyAccountTier {
        self.spotify_profile
            .as_ref()
            .map(|p| p.product)
            .unwrap_or(SpotifyAccountTier::Unknown)
    }

    fn can_query_spotify_web_api(&self) -> bool {
        !self.spotify_lapsed
            && self.tokens.web_api.is_some()
            && self
                .config
                .spotify_client_id
                .as_deref()
                .is_some_and(|id| !id.trim().is_empty())
    }

    fn trigger_remote_search_tasks(&mut self, query: String) -> Task<Message> {
        self.search_request_id = self.search_request_id.wrapping_add(1);
        let req_id = self.search_request_id;
        let mut tasks = Vec::new();
        let spotify_ok = self.can_query_spotify_web_api();

        if matches!(self.source_filter, SourceFilter::All | SourceFilter::Spotify) && spotify_ok {
            let cache_key = query.trim().to_ascii_lowercase();
            let cached_spotify = self
                .spotify_search_cache
                .get(&cache_key)
                .filter(|(exp, _)| *exp > Instant::now())
                .map(|(_, c)| c.clone());
            if let Some(mut catalog) = cached_spotify {
                self.searching_spotify = false;
                self.apply_liked_flags(&mut catalog.tracks);
                self.spotify_search_results = catalog.tracks;
                self.spotify_search_artists = catalog.artists;
                self.spotify_search_playlists = catalog.playlists;
                self.refresh_search_results();
            } else {
                self.searching_spotify = true;
                tasks.push(self.spawn_spotify_search_task(query.clone(), req_id));
            }
        }

        let need_ytm = matches!(
            self.source_filter,
            SourceFilter::All | SourceFilter::YouTubeMusic
        ) || (self.source_filter == SourceFilter::Spotify && !spotify_ok);
        if need_ytm {
            if self.source_filter == SourceFilter::Spotify && !spotify_ok {
                self.status_toast = Some(
                    "Spotify Web API is not active — showing YouTube Music results instead."
                        .to_string(),
                );
            }
            self.searching_ytm = true;
            tasks.push(self.spawn_ytm_search_task(query, req_id));
        }

        if tasks.is_empty() {
            Task::none()
        } else {
            Task::batch(tasks)
        }
    }

    fn refresh_search_results(&mut self) {
        let cached = if let Ok(db) = self.db.lock() {
            db.search_cached_tracks(&self.search_query, 200)
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        let with_spotify = merge_search_results(cached, self.spotify_search_results.clone());
        self.search_results = merge_search_results(with_spotify, self.ytm_search_results.clone());
    }

    fn ordered_playlist_tracks(&self, pl_id: &str) -> Vec<Track> {
        let sort = self
            .playlist_sorts
            .get(pl_id)
            .copied()
            .unwrap_or_default();
        sort_playlist_tracks(&self.playlist_tracks, sort)
            .into_iter()
            .map(|(_, t)| t.clone())
            .collect()
    }

    fn missing_art_keys(&self, tracks: &[Track]) -> Vec<String> {
        let mut seen = HashSet::new();
        let mut out = Vec::new();
        for t in tracks {
            if let Some(key) = &t.artwork_ref {
                if !self.art_handles.contains_key(key) && seen.insert(key.clone()) {
                    out.push(key.clone());
                    if out.len() >= MAX_IN_MEM_ART_HANDLES {
                        break;
                    }
                }
            }
        }
        out
    }

    fn handle_play_pause_toggle(&mut self) -> Task<Message> {
        match self.status {
            PlaybackStatus::Playing => {
                if let Some(audio) = &self.audio {
                    let _ = audio.send(AudioCommand::Pause);
                }
                self.status = PlaybackStatus::Paused;
                self.smtc.set_playback(self.status, self.position_ms);
            }
            PlaybackStatus::Paused => {
                if let Some(audio) = &self.audio {
                    let _ = audio.send(AudioCommand::Resume);
                }
                self.status = PlaybackStatus::Playing;
                self.smtc.set_playback(self.status, self.position_ms);
            }
            PlaybackStatus::Buffering => {
                self.stop_playback();
            }
            PlaybackStatus::Stopped => {
                if let Some(track) = self.queue.current().cloned() {
                    return self.start_playing_track(track);
                }
            }
        }
        Task::none()
    }

    fn start_playing_track(&mut self, track: Track) -> Task<Message> {
        self.play_request_id = self.play_request_id.wrapping_add(1);
        let req_id = self.play_request_id;
        self.active_http_fallback = None;
        self.position_ms = 0;
        self.last_audio_pos_ms = 0;
        self.last_audio_pos_at = Instant::now();
        self.duration_ms = track.duration_ms;
        self.lyrics_spring.snap_to(0.0);
        self.last_scrolled_line = None;
        if let Ok(db) = self.db.lock() {
            let _ = db.record_play_history(&track);
            self.recent_tracks = db.list_recent_tracks(12).unwrap_or_default();
        }
        self.persist_session();

        let cover_path = track
            .artwork_ref
            .as_deref()
            .and_then(|k| self.art_cache.lock().ok().and_then(|c| c.art_path(k)));
        self.smtc.set_metadata(Some(&track), cover_path.as_deref());

        let mut tasks = vec![self.load_palette_task(&track)];
        if needs_spotify_album_artwork(&track) {
            tasks.push(self.spawn_spotify_artwork_enrichment_task(std::slice::from_ref(&track)));
        }
        if self.view_mode == ViewMode::NowPlayingLyrics {
            tasks.push(self.fetch_lyrics_task(track.clone(), false));
        } else {
            self.current_lyrics = None;
            self.cached_lyrics_timeline.clear();
            self.current_lyrics_uid = None;
        }

        match &track.source {
            TrackSource::Local { path } => {
                self.resolving_stream = false;
                self.source_used = Some(PlaybackSourceUsed::LocalFile);
                if let Some(audio) = &self.audio {
                    let _ = audio.send(AudioCommand::LoadLocal {
                        path: path.clone(),
                        track_uid: track.uid.clone(),
                        duration_ms: track.duration_ms,
                    });
                }
            }
            TrackSource::YouTubeMusic { .. } => {
                if let Ok(db) = self.db.lock() {
                    let _ = db.upsert_track(&track, None);
                }
                self.resolving_stream = true;
                self.source_used = Some(PlaybackSourceUsed::YouTubeDirect);
                self.status = PlaybackStatus::Buffering;
                self.status_toast = None;
                if let Some(audio) = &self.audio {
                    let _ = audio.send(AudioCommand::Stop);
                }
                self.smtc.set_playback(PlaybackStatus::Buffering, 0);
                tasks.push(self.resolve_ytm_stream_task(req_id, track.clone()));
            }
            TrackSource::Spotify { .. } => {
                if let Ok(db) = self.db.lock() {
                    let _ = db.upsert_track(&track, None);
                }
                self.resolving_stream = true;
                self.status = PlaybackStatus::Buffering;
                self.status_toast = None;
                if let Some(audio) = &self.audio {
                    let _ = audio.send(AudioCommand::Stop);
                }
                self.smtc.set_playback(PlaybackStatus::Buffering, 0);

                if should_attempt_spotify_native(
                    self.config.prefer_youtube_fallback,
                    self.tokens.playback.is_some(),
                    self.spotify_account_tier(),
                    self.spotify_lapsed,
                ) {
                    self.source_used = Some(PlaybackSourceUsed::SpotifyNative);
                    tasks.push(self.prepare_spotify_playback_token_task(req_id, track.clone()));
                } else {
                    self.source_used = Some(PlaybackSourceUsed::YouTubeFallback);
                    tasks.push(self.resolve_spotify_fallback_stream_task(
                        req_id,
                        track.clone(),
                        false,
                    ));
                }
            }
        }

        Task::batch(tasks)
    }

    fn stop_playback(&mut self) {
        self.play_request_id = self.play_request_id.wrapping_add(1);
        self.resolving_stream = false;
        self.active_http_fallback = None;
        self.status = PlaybackStatus::Stopped;
        self.position_ms = 0;
        if let Some(audio) = &self.audio {
            let _ = audio.send(AudioCommand::Stop);
        }
        self.smtc.set_playback(PlaybackStatus::Stopped, 0);
    }

    fn persist_session(&self) {
        if let Ok(db) = self.db.lock() {
            let _ = db.save_playback_session(&self.queue.to_snapshot(), self.volume, self.muted);
        }
    }

    fn shutdown_and_exit(&mut self) -> Task<Message> {
        self.persist_session();
        if let Some(audio) = &self.audio {
            let _ = audio.send(AudioCommand::Shutdown);
        }
        iced::exit()
    }

    fn insert_art_handle(&mut self, key: String, handle: image::Handle) {
        self.art_handle_lru.retain(|k| k != &key);
        self.art_handle_lru.push_back(key.clone());
        self.art_handles.insert(key, handle);
        while self.art_handles.len() > MAX_IN_MEM_ART_HANDLES {
            if let Some(oldest) = self.art_handle_lru.pop_front() {
                self.art_handles.remove(&oldest);
            } else {
                break;
            }
        }
    }

    fn load_palette_task(&self, track: &Track) -> Task<Message> {
        let track_uid = track.uid.clone();
        let art_key = track.artwork_ref.clone();
        let art_cache = Arc::clone(&self.art_cache);

        Task::perform(
            async move {
                let bytes = match art_key.clone() {
                    Some(key) => fetch_or_cache_art(art_cache, key).await,
                    None => None,
                };
                tokio::task::spawn_blocking(move || {
                    let palette = bytes
                        .as_deref()
                        .map(extract_palette)
                        .unwrap_or_else(AlbumPalette::default_dark);
                    let cover_rgba = bytes
                        .as_deref()
                        .and_then(|b| decode_bounded_cover_rgba(b, MAX_ACTIVE_COVER_PX));
                    let thumb_rgba = bytes.as_deref().and_then(decode_thumbnail_rgba_96);
                    let blur_rgba = bytes.as_deref().and_then(decode_art_blur_rgba).map(Arc::new);
                    (track_uid, art_key, palette, cover_rgba, thumb_rgba, blur_rgba)
                })
                .await
                .unwrap_or_else(|_| {
                    (
                        String::new(),
                        None,
                        AlbumPalette::default_dark(),
                        None,
                        None,
                        None,
                    )
                })
            },
            |(track_uid, art_key, palette, cover_rgba, thumb_rgba, art_blur_rgba)| {
                Message::PaletteLoaded {
                    track_uid,
                    art_key,
                    palette,
                    cover_rgba,
                    thumb_rgba,
                    art_blur_rgba,
                }
            },
        )
    }

    fn preload_thumbnails_task(&self, keys: Vec<String>) -> Task<Message> {
        if keys.is_empty() {
            return Task::none();
        }
        // Emit chunks of 16 progressively while capping total concurrent HTTP downloads via a
        // shared semaphore so large playlists never flood the CDN into timeouts or drops.
        let sem = Arc::new(Semaphore::new(MAX_CONCURRENT_ART_DOWNLOADS));
        let mut tasks = Vec::new();
        for group in keys.chunks(16) {
            let group_keys: Vec<String> = group.to_vec();
            let art_cache = Arc::clone(&self.art_cache);
            let sem = Arc::clone(&sem);
            tasks.push(Task::perform(
                async move {
                    let mut set = tokio::task::JoinSet::new();
                    for key in group_keys {
                        let cache = Arc::clone(&art_cache);
                        let sem = Arc::clone(&sem);
                        set.spawn(async move {
                            let _permit = sem.acquire_owned().await.ok()?;
                            let bytes = fetch_or_cache_art(cache, key.clone()).await?;
                            drop(_permit);
                            let (w, h, rgba) = tokio::task::spawn_blocking(move || {
                                decode_thumbnail_rgba_96(&bytes)
                            })
                            .await
                            .ok()??;
                            Some((key, w, h, rgba))
                        });
                    }
                    let mut out = Vec::new();
                    while let Some(joined) = set.join_next().await {
                        if let Ok(Some(item)) = joined {
                            out.push(item);
                        }
                    }
                    out
                },
                Message::ThumbnailsLoaded,
            ));
        }
        Task::batch(tasks)
    }

    fn spawn_spotify_artwork_enrichment_task(&mut self, tracks: &[Track]) -> Task<Message> {
        // First visible rows use a fast 12-item batch so covers pop in immediately (~0.4s);
        // subsequent background batches stream 32 at a time over the shared HTTP/2 pool.
        let first_visible_missing = tracks
            .iter()
            .take(12)
            .any(|t| needs_spotify_album_artwork(t) && !self.enriching_spotify_art.contains(&t.uid));
        let batch_size = if first_visible_missing { 12 } else { 32 };

        let mut batch: Vec<(String, String)> = Vec::new();
        for t in tracks {
            if !needs_spotify_album_artwork(t) {
                continue;
            }
            if let TrackSource::Spotify { id } = &t.source {
                if self.enriching_spotify_art.insert(t.uid.clone()) {
                    batch.push((t.uid.clone(), id.clone()));
                    if batch.len() >= batch_size {
                        break;
                    }
                }
            }
        }
        if batch.is_empty() {
            return Task::none();
        }

        let db_path = self.db_path.clone();
        let attempted_uids: Vec<String> = batch.iter().map(|(uid, _)| uid.clone()).collect();
        Task::perform(
            async move {
                let client = shared_art_http_client().clone();

                let mut resolved = Vec::with_capacity(batch.len());
                for chunk in batch.chunks(OEMBED_ENRICH_CONCURRENCY) {
                    let mut set = tokio::task::JoinSet::new();
                    for (uid, sp_id) in chunk {
                        let http = client.clone();
                        let uid = uid.clone();
                        let sp_id = sp_id.clone();
                        set.spawn(async move {
                            fetch_spotify_track_oembed_cover(&http, &sp_id)
                                .await
                                .map(|url| (uid, url))
                        });
                    }
                    while let Some(joined) = set.join_next().await {
                        if let Ok(Some(pair)) = joined {
                            resolved.push(pair);
                        }
                    }
                }

                if !resolved.is_empty() {
                    let to_save = resolved.clone();
                    let _ = tokio::task::spawn_blocking(move || {
                        if let Ok(db) = Database::open(&db_path) {
                            let _ = db.update_tracks_artwork_batch(&to_save);
                        }
                    })
                    .await;
                }

                (attempted_uids, resolved)
            },
            |(attempted_uids, resolved)| Message::SpotifyArtworkEnriched {
                attempted_uids,
                resolved,
            },
        )
    }

    fn get_ytm_provider(&self) -> Result<Arc<YouTubeMusicProvider>, String> {
        if let Some(ytm) = &self.ytm {
            return Ok(Arc::clone(ytm));
        }
        YouTubeMusicProvider::new(
            YouTubeMusicProvider::default_cache_dir(),
            self.config.yt_dlp_path.clone(),
        )
        .map(Arc::new)
        .map_err(|e| e.to_string())
    }

    fn rebuild_ytm_provider(&mut self) {
        self.ytm = YouTubeMusicProvider::new(
            YouTubeMusicProvider::default_cache_dir(),
            self.config.yt_dlp_path.clone(),
        )
        .ok()
        .map(Arc::new);
    }

    fn spawn_prefetch_next_track_task(&self, next_track: Track) -> Task<Message> {
        let Ok(ytm) = self.get_ytm_provider() else {
            return Task::none();
        };
        let db = Arc::clone(&self.db);
        let try_native_spotify = should_attempt_spotify_native(
            self.config.prefer_youtube_fallback,
            self.tokens.playback.is_some(),
            self.spotify_account_tier(),
            self.spotify_lapsed,
        );
        Task::perform(
            async move {
                // Brief yield so the currently clicked track gets priority on the network first.
                tokio::time::sleep(Duration::from_millis(450)).await;
                match &next_track.source {
                    TrackSource::YouTubeMusic { video_id } => {
                        let _ = ytm.resolve_stream_for_video_id(video_id).await;
                    }
                    TrackSource::Spotify { .. } if !try_native_spotify => {
                        if let Ok(matched) =
                            resolve_spotify_to_ytm_video_id(&next_track, &db, &ytm, false).await
                        {
                            let _ = ytm.resolve_stream_for_video_id(&matched.yt_video_id).await;
                        }
                    }
                    _ => {}
                }
            },
            |_| Message::BackgroundTaskDone,
        )
    }

    fn spawn_ytm_search_task(&self, query: String, request_id: u64) -> Task<Message> {
        let ytm_res = self.get_ytm_provider();
        Task::perform(
            async move {
                let provider = ytm_res?;
                provider
                    .search_catalog_full(&query, 14, 6, 8, 8)
                    .await
                    .map_err(|e| e.to_string())
            },
            move |result| Message::YtmSearchFinished { request_id, result },
        )
    }

    fn spawn_home_charts_tasks(&mut self, region: ChartRegion, force: bool) -> Task<Message> {
        let mut tasks = Vec::new();
        let indonesia = region.is_indonesia();

        // Spotify weekly charts: serve directly from SQLite cache for 7 days (0 network calls).
        let mut need_spotify_fetch = force;
        if !force && !self.home_spotify_charts.contains_key(&region) {
            let cached = self
                .db
                .lock()
                .ok()
                .and_then(|db| {
                    db.get_cached_chart(&chart_cache_key("spotify", region), WEEKLY_CHARTS_TTL_S)
                        .ok()
                        .flatten()
                })
                .filter(|c| !c.tracks.is_empty());

            if let Some(mut entry) = cached {
                self.apply_liked_flags(&mut entry.tracks);
                let keys = self.missing_art_keys(&entry.tracks);
                let snapshot = entry.tracks.clone();
                self.home_spotify_charts.insert(region, entry.tracks);
                tasks.push(self.preload_thumbnails_task(keys));
                tasks.push(self.spawn_spotify_artwork_enrichment_task(&snapshot));
                need_spotify_fetch = !entry.is_fresh;
            } else {
                need_spotify_fetch = true;
            }
        }

        if need_spotify_fetch {
            self.home_loading_spotify = !self.home_spotify_charts.contains_key(&region) || force;
            tasks.push(Task::perform(
                async move {
                    fetch_spotify_chart_tracks(indonesia, 25)
                        .await
                        .map_err(|e| e.to_string())
                },
                move |result| Message::HomeSpotifyChartsLoaded { region, result },
            ));
        } else {
            self.home_loading_spotify = false;
        }

        // YouTube Music weekly charts: serve directly from SQLite cache for 7 days (0 network calls).
        let mut need_ytm_fetch = force;
        if !force && !self.home_ytm_charts.contains_key(&region) {
            let cached = self
                .db
                .lock()
                .ok()
                .and_then(|db| {
                    db.get_cached_chart(&chart_cache_key("ytm", region), WEEKLY_CHARTS_TTL_S)
                        .ok()
                        .flatten()
                })
                .filter(|c| !c.tracks.is_empty());

            if let Some(mut entry) = cached {
                self.apply_liked_flags(&mut entry.tracks);
                let keys = self.missing_art_keys(&entry.tracks);
                self.home_ytm_charts.insert(region, entry.tracks);
                tasks.push(self.preload_thumbnails_task(keys));
                need_ytm_fetch = !entry.is_fresh;
            } else {
                need_ytm_fetch = true;
            }
        }

        if need_ytm_fetch {
            self.home_loading_ytm = !self.home_ytm_charts.contains_key(&region) || force;
            let ytm_res = self.get_ytm_provider();
            tasks.push(Task::perform(
                async move {
                    let provider = ytm_res?;
                    if force {
                        provider.invalidate_charts_cache();
                    }
                    provider
                        .fetch_charts(indonesia, 25)
                        .await
                        .map_err(|e| e.to_string())
                },
                move |result| Message::HomeYtmChartsLoaded { region, result },
            ));
        } else {
            self.home_loading_ytm = false;
        }

        if tasks.is_empty() {
            Task::none()
        } else {
            Task::batch(tasks)
        }
    }

    fn resolve_ytm_stream_task(&self, request_id: u64, track: Track) -> Task<Message> {
        let ytm_res = self.get_ytm_provider();
        let video_id = match &track.source {
            TrackSource::YouTubeMusic { video_id } => Some(video_id.clone()),
            _ => None,
        };
        let track_for_msg = track.clone();
        Task::perform(
            async move {
                let provider = ytm_res?;
                provider
                    .resolve_stream(&track)
                    .await
                    .map_err(|e| e.to_string())
            },
            move |result| Message::StreamResolved {
                request_id,
                track: track_for_msg,
                video_id,
                result,
            },
        )
    }

    fn prepare_spotify_playback_token_task(&self, request_id: u64, track: Track) -> Task<Message> {
        let current_tok = self.tokens.playback.clone();
        let track_for_msg = track.clone();
        Task::perform(
            async move {
                let Some(tok) = current_tok else {
                    return Err("Spotify playback not signed in".to_string());
                };
                ensure_fresh_token(SPOTIFY_PLAYBACK_CLIENT_ID, tok)
                    .await
                    .map_err(|e| e.to_string())
            },
            move |result| Message::SpotifyPlaybackTokenReady {
                request_id,
                track: track_for_msg,
                result,
            },
        )
    }

    fn resolve_spotify_fallback_stream_task(
        &self,
        request_id: u64,
        track: Track,
        force_rematch: bool,
    ) -> Task<Message> {
        let db = Arc::clone(&self.db);
        let ytm_res = self.get_ytm_provider();
        let track_for_msg = track.clone();
        Task::perform(
            async move {
                let ytm = ytm_res?;
                let matched = resolve_spotify_to_ytm_video_id(&track, &db, &ytm, force_rematch)
                    .await
                    .map_err(|e| e.to_string())?;
                let matched_vid = matched.yt_video_id.clone();
                let resolved = ytm
                    .resolve_stream_for_video_id(&matched_vid)
                    .await
                    .map_err(|e| e.to_string())?;
                Ok((matched_vid, rewrite_stream_as_youtube_fallback(resolved)))
            },
            move |res| match res {
                Ok((vid, stream)) => Message::StreamResolved {
                    request_id,
                    track: track_for_msg,
                    video_id: Some(vid),
                    result: Ok(stream),
                },
                Err(err) => Message::StreamResolved {
                    request_id,
                    track: track_for_msg,
                    video_id: None,
                    result: Err(err),
                },
            },
        )
    }

    fn spawn_spotify_search_task(&self, query: String, request_id: u64) -> Task<Message> {
        let client_id = self.config.spotify_client_id.clone().unwrap_or_default();
        let tok = self.tokens.web_api.clone();
        Task::perform(
            async move {
                let Some(tok) = tok else {
                    return Message::SpotifySearchFinished {
                        request_id,
                        result: Err(SpotifySearchError::LapsedOrForbidden),
                        refreshed_token: None,
                    };
                };
                let (tok, refreshed) = match ensure_fresh_token(&client_id, tok).await {
                    Ok(pair) => pair,
                    Err(e) => {
                        return Message::SpotifySearchFinished {
                            request_id,
                            result: Err(SpotifySearchError::Other(e.to_string())),
                            refreshed_token: None,
                        };
                    }
                };
                let client = SpotifyWebClient::new();
                let res = client.search_catalog(&tok.access_token, &query, 10).await;
                let refreshed_token = refreshed.then_some(tok);
                match res {
                    Ok(catalog) => Message::SpotifySearchFinished {
                        request_id,
                        result: Ok(catalog),
                        refreshed_token,
                    },
                    Err(SpotifyApiError::PremiumRequiredOrForbidden(_))
                    | Err(SpotifyApiError::Unauthorized) => Message::SpotifySearchFinished {
                        request_id,
                        result: Err(SpotifySearchError::LapsedOrForbidden),
                        refreshed_token,
                    },
                    Err(err) => Message::SpotifySearchFinished {
                        request_id,
                        result: Err(SpotifySearchError::Other(err.to_string())),
                        refreshed_token,
                    },
                }
            },
            |msg| msg,
        )
    }

    fn spawn_load_artist_task(&self, request_id: u64, summary: ArtistSummary) -> Task<Message> {
        let client_id = self.config.spotify_client_id.clone().unwrap_or_default();
        let tok = self.tokens.web_api.clone();
        let can_spotify = self.can_query_spotify_web_api();
        let ytm_res = self.get_ytm_provider();

        Task::perform(
            async move {
                if summary.source == CatalogSource::YouTubeMusic {
                    let res = async {
                        let ytm = ytm_res?;
                        ytm.get_artist_profile(&summary.id)
                            .await
                            .map_err(|e| e.to_string())
                    }
                    .await;
                    return Message::ArtistProfileLoaded {
                        request_id,
                        result: res,
                        refreshed_token: None,
                        fallback_notice: None,
                        spotify_lapsed: false,
                    };
                }

                // Spotify-first: load Web API artist + albums concurrently with Embed top tracks
                // (Feb 2026 Dev Mode removed `/v1/artists/{id}/top-tracks`).
                let mut refreshed_token = None;
                let mut spotify_lapsed = false;

                if can_spotify {
                    if let Some(t) = tok {
                        if let Ok((fresh_tok, refreshed)) =
                            ensure_fresh_token(&client_id, t).await
                        {
                            let access = fresh_tok.access_token.clone();
                            if refreshed {
                                refreshed_token = Some(fresh_tok);
                            }
                            let client = SpotifyWebClient::new();
                            let (meta_res, albums_res, embed_res) = tokio::join!(
                                client.get_artist(&access, &summary.id),
                                client.get_artist_albums(&access, &summary.id, 3),
                                fetch_spotify_artist_embed(&summary.id)
                            );

                            let lapsed_err = matches!(
                                &meta_res,
                                Err(SpotifyApiError::PremiumRequiredOrForbidden(_))
                                    | Err(SpotifyApiError::Unauthorized)
                            ) || matches!(
                                &albums_res,
                                Err(SpotifyApiError::PremiumRequiredOrForbidden(_))
                                    | Err(SpotifyApiError::Unauthorized)
                            );

                            if lapsed_err {
                                spotify_lapsed = true;
                            } else if meta_res.is_ok() || albums_res.is_ok() || embed_res.is_ok() {
                                let meta = meta_res.ok();
                                let (albums, singles) = albums_res.unwrap_or_default();
                                let embed = embed_res.ok();
                                let name = meta
                                    .as_ref()
                                    .map(|m| m.name.clone())
                                    .or_else(|| embed.as_ref().map(|e| e.name.clone()))
                                    .unwrap_or_else(|| summary.name.clone());
                                let avatar_url = meta
                                    .as_ref()
                                    .and_then(|m| m.avatar_url.clone())
                                    .or_else(|| embed.as_ref().and_then(|e| e.avatar_url.clone()))
                                    .or_else(|| summary.avatar_url.clone());
                                let subscriber_label = meta
                                    .as_ref()
                                    .and_then(|m| m.followers.map(format_followers_label))
                                    .or_else(|| {
                                        meta.as_ref()
                                            .filter(|m| !m.genres.is_empty())
                                            .map(|m| m.genres.iter().take(3).cloned().collect::<Vec<_>>().join(" • "))
                                    });
                                let top_tracks =
                                    embed.map(|e| e.top_tracks).unwrap_or_default();

                                if !top_tracks.is_empty()
                                    || !albums.is_empty()
                                    || !singles.is_empty()
                                {
                                    return Message::ArtistProfileLoaded {
                                        request_id,
                                        result: Ok(ArtistProfile {
                                            id: summary.id,
                                            source: CatalogSource::Spotify,
                                            name,
                                            avatar_url,
                                            description: None,
                                            subscriber_label,
                                            top_tracks,
                                            albums,
                                            singles,
                                            playlists: Vec::new(),
                                        }),
                                        refreshed_token,
                                        fallback_notice: None,
                                        spotify_lapsed: false,
                                    };
                                }
                            }
                        }
                    }
                }

                // Automatic fallback to YouTube Music artist profile by artist name.
                let fallback_res = async {
                    let ytm = ytm_res?;
                    let found = ytm
                        .search_artists(&summary.name, 6)
                        .await
                        .map_err(|e| e.to_string())?;
                    let best = pick_best_artist_match(&summary.name, &found).ok_or_else(|| {
                        format!("Could not find artist \"{}\" on YouTube Music.", summary.name)
                    })?;
                    ytm.get_artist_profile(&best.id)
                        .await
                        .map_err(|e| e.to_string())
                }
                .await;

                let notice = fallback_res.is_ok().then(|| {
                    format!(
                        "Loaded \"{}\" profile from YouTube Music (Spotify API unavailable).",
                        summary.name
                    )
                });
                Message::ArtistProfileLoaded {
                    request_id,
                    result: fallback_res,
                    refreshed_token,
                    fallback_notice: notice,
                    spotify_lapsed,
                }
            },
            |msg| msg,
        )
    }

    fn spawn_load_artist_by_name_task(
        &self,
        request_id: u64,
        artist_name: String,
        prefer_spotify: bool,
    ) -> Task<Message> {
        let client_id = self.config.spotify_client_id.clone().unwrap_or_default();
        let tok = self.tokens.web_api.clone();
        let ytm_res = self.get_ytm_provider();

        Task::perform(
            async move {
                let mut refreshed_token = None;
                let mut spotify_lapsed = false;

                if prefer_spotify {
                    if let Some(t) = tok {
                        if let Ok((fresh_tok, refreshed)) =
                            ensure_fresh_token(&client_id, t).await
                        {
                            let access = fresh_tok.access_token.clone();
                            if refreshed {
                                refreshed_token = Some(fresh_tok);
                            }
                            let client = SpotifyWebClient::new();
                            match client.search_artists(&access, &artist_name, 6).await {
                                Ok(candidates) => {
                                    if let Some(best) =
                                        pick_best_artist_match(&artist_name, &candidates)
                                    {
                                        let (meta_res, albums_res, embed_res) = tokio::join!(
                                            client.get_artist(&access, &best.id),
                                            client.get_artist_albums(&access, &best.id, 3),
                                            fetch_spotify_artist_embed(&best.id)
                                        );
                                        let meta = meta_res.ok();
                                        let (albums, singles) = albums_res.unwrap_or_default();
                                        let embed = embed_res.ok();
                                        let top_tracks =
                                            embed.as_ref().map(|e| e.top_tracks.clone()).unwrap_or_default();
                                        if !top_tracks.is_empty()
                                            || !albums.is_empty()
                                            || !singles.is_empty()
                                        {
                                            let avatar_url = meta
                                                .as_ref()
                                                .and_then(|m| m.avatar_url.clone())
                                                .or_else(|| embed.and_then(|e| e.avatar_url))
                                                .or(best.avatar_url);
                                            let subscriber_label = meta
                                                .as_ref()
                                                .and_then(|m| m.followers.map(format_followers_label))
                                                .or(best.subtitle);
                                            return Message::ArtistProfileLoaded {
                                                request_id,
                                                result: Ok(ArtistProfile {
                                                    id: best.id,
                                                    source: CatalogSource::Spotify,
                                                    name: meta.map(|m| m.name).unwrap_or(best.name),
                                                    avatar_url,
                                                    description: None,
                                                    subscriber_label,
                                                    top_tracks,
                                                    albums,
                                                    singles,
                                                    playlists: Vec::new(),
                                                }),
                                                refreshed_token,
                                                fallback_notice: None,
                                                spotify_lapsed: false,
                                            };
                                        }
                                    }
                                }
                                Err(SpotifyApiError::PremiumRequiredOrForbidden(_))
                                | Err(SpotifyApiError::Unauthorized) => {
                                    spotify_lapsed = true;
                                }
                                Err(_) => {}
                            }
                        }
                    }
                }

                let ytm_result = async {
                    let ytm = ytm_res?;
                    let candidates = ytm
                        .search_artists(&artist_name, 6)
                        .await
                        .map_err(|e| e.to_string())?;
                    let best = pick_best_artist_match(&artist_name, &candidates).ok_or_else(|| {
                        format!("Artist \"{artist_name}\" was not found on YouTube Music.")
                    })?;
                    ytm.get_artist_profile(&best.id)
                        .await
                        .map_err(|e| e.to_string())
                }
                .await;

                Message::ArtistProfileLoaded {
                    request_id,
                    result: ytm_result,
                    refreshed_token,
                    fallback_notice: None,
                    spotify_lapsed,
                }
            },
            |msg| msg,
        )
    }

    fn spawn_load_album_task(&self, request_id: u64, summary: AlbumSummary) -> Task<Message> {
        let client_id = self.config.spotify_client_id.clone().unwrap_or_default();
        let tok = self.tokens.web_api.clone();
        let can_spotify = self.can_query_spotify_web_api();
        let ytm_res = self.get_ytm_provider();

        Task::perform(
            async move {
                if summary.source == CatalogSource::YouTubeMusic {
                    let res = async {
                        let ytm = ytm_res?;
                        ytm.get_album_detail(&summary.id)
                            .await
                            .map_err(|e| e.to_string())
                    }
                    .await;
                    return Message::AlbumDetailLoaded {
                        request_id,
                        result: res,
                        refreshed_token: None,
                        spotify_lapsed: false,
                    };
                }

                let mut refreshed_token = None;
                let mut spotify_lapsed = false;

                if can_spotify {
                    if let Some(t) = tok {
                        if let Ok((fresh_tok, refreshed)) =
                            ensure_fresh_token(&client_id, t).await
                        {
                            let access = fresh_tok.access_token.clone();
                            if refreshed {
                                refreshed_token = Some(fresh_tok);
                            }
                            let client = SpotifyWebClient::new();
                            match client.get_album_detail(&access, &summary.id).await {
                                Ok(detail) if !detail.tracks.is_empty() => {
                                    return Message::AlbumDetailLoaded {
                                        request_id,
                                        result: Ok(detail),
                                        refreshed_token,
                                        spotify_lapsed: false,
                                    };
                                }
                                Err(SpotifyApiError::PremiumRequiredOrForbidden(_))
                                | Err(SpotifyApiError::Unauthorized) => {
                                    spotify_lapsed = true;
                                }
                                _ => {}
                            }
                        }
                    }
                }

                // Fallback to Spotify public album embed page (no auth needed).
                let embed_res = fetch_spotify_album_embed(&summary.id)
                    .await
                    .map_err(|e| e.to_string());
                Message::AlbumDetailLoaded {
                    request_id,
                    result: embed_res,
                    refreshed_token,
                    spotify_lapsed,
                }
            },
            |msg| msg,
        )
    }

    fn spawn_load_remote_playlist_task(
        &self,
        request_id: u64,
        summary: RemotePlaylistSummary,
    ) -> Task<Message> {
        let client_id = self.config.spotify_client_id.clone().unwrap_or_default();
        let tok = self.tokens.web_api.clone();
        let can_spotify = self.can_query_spotify_web_api();
        let ytm_res = self.get_ytm_provider();

        Task::perform(
            async move {
                if summary.source == CatalogSource::YouTubeMusic {
                    let res = async {
                        let ytm = ytm_res?;
                        ytm.get_playlist_detail(&summary.id)
                            .await
                            .map_err(|e| e.to_string())
                    }
                    .await;
                    return Message::RemotePlaylistLoaded {
                        request_id,
                        result: res,
                        refreshed_token: None,
                        spotify_lapsed: false,
                    };
                }

                let mut refreshed_token = None;
                let mut spotify_lapsed = false;

                // Try Spotify Web API first (returns unlimited tracks for user-owned/collab playlists).
                if can_spotify {
                    if let Some(t) = tok {
                        if let Ok((fresh_tok, refreshed)) =
                            ensure_fresh_token(&client_id, t).await
                        {
                            let access = fresh_tok.access_token.clone();
                            if refreshed {
                                refreshed_token = Some(fresh_tok);
                            }
                            let client = SpotifyWebClient::new();
                            match client.get_playlist_detail(&access, &summary.id).await {
                                Ok(mut detail) if !detail.tracks.is_empty() => {
                                    if detail.cover_url.is_none() {
                                        detail.cover_url = summary.cover_url.clone();
                                    }
                                    if detail.owner.is_none() {
                                        detail.owner = summary.owner.clone();
                                    }
                                    return Message::RemotePlaylistLoaded {
                                        request_id,
                                        result: Ok(detail),
                                        refreshed_token,
                                        spotify_lapsed: false,
                                    };
                                }
                                Err(SpotifyApiError::Unauthorized) => {
                                    spotify_lapsed = true;
                                }
                                // Note: 403 or empty items on third-party public playlists is expected
                                // under Feb 2026 Dev Mode -> fall through cleanly to embed parser
                                // without flagging the whole session as lapsed.
                                _ => {}
                            }
                        }
                    }
                }

                // Fallback to Spotify public embed parser (works for editorial & third-party public playlists, up to 100 tracks).
                let url = canonical_playlist_url(&summary.id);
                let res = import_spotify_playlist_url(&url)
                    .await
                    .map(|imp| {
                        let is_truncated = imp.tracks.len() >= 100;
                        RemotePlaylistDetail {
                            id: imp.spotify_id,
                            source: CatalogSource::Spotify,
                            name: if imp.name.trim().is_empty() {
                                summary.name.clone()
                            } else {
                                imp.name
                            },
                            owner: summary.owner.clone(),
                            cover_url: imp.cover_url.or(summary.cover_url.clone()),
                            source_url: imp.source_url,
                            tracks: imp.tracks,
                            is_truncated_embed: is_truncated,
                        }
                    })
                    .map_err(|e| e.to_string());

                Message::RemotePlaylistLoaded {
                    request_id,
                    result: res,
                    refreshed_token,
                    spotify_lapsed,
                }
            },
            |msg| msg,
        )
    }

    fn spawn_spotify_sync_task(&self) -> Task<Message> {
        let client_id = self.config.spotify_client_id.clone().unwrap_or_default();
        let tok = self.tokens.web_api.clone();
        let db_path = self.db_path.clone();

        Task::perform(
            async move {
                let Some(tok) = tok else {
                    return Err("Spotify Web API is not connected.".to_string());
                };
                let (tok, refreshed) = ensure_fresh_token(&client_id, tok)
                    .await
                    .map_err(|e| e.to_string())?;
                let access = tok.access_token.clone();
                let refreshed_token = refreshed.then_some(tok);

                let client = SpotifyWebClient::new();
                let profile = match client.get_profile(&access).await {
                    Ok(p) => Some(p),
                    Err(SpotifyApiError::PremiumRequiredOrForbidden(_)) => {
                        return Ok(SpotifySyncReport {
                            profile: None,
                            synced_playlists: 0,
                            synced_liked: 0,
                            flushed_mutations: 0,
                            lapsed: true,
                            refreshed_token,
                        });
                    }
                    Err(err) => return Err(err.to_string()),
                };

                // Flush queued offline Like/Unlike mutations first (never hold !Sync Database across .await).
                let db_path_flush = db_path.clone();
                let pending = tokio::task::spawn_blocking(move || {
                    Database::open(&db_path_flush)
                        .and_then(|db| db.list_pending_mutations())
                        .unwrap_or_default()
                })
                .await
                .unwrap_or_default();

                let mut flushed_ids = Vec::new();
                for item in pending {
                    let ids = vec![item.spotify_track_id.clone()];
                    let res = match item.action.as_str() {
                        "like" => client.save_tracks(&access, &ids).await,
                        "unlike" => client.remove_tracks(&access, &ids).await,
                        _ => Ok(()),
                    };
                    match res {
                        Ok(()) => flushed_ids.push(item.id),
                        Err(SpotifyApiError::PremiumRequiredOrForbidden(_)) => {
                            if !flushed_ids.is_empty() {
                                let p = db_path.clone();
                                let done = flushed_ids.clone();
                                let _ = tokio::task::spawn_blocking(move || {
                                    if let Ok(db) = Database::open(&p) {
                                        for id in done {
                                            let _ = db.delete_pending_mutation(id);
                                        }
                                    }
                                })
                                .await;
                            }
                            return Ok(SpotifySyncReport {
                                profile,
                                synced_playlists: 0,
                                synced_liked: 0,
                                flushed_mutations: flushed_ids.len(),
                                lapsed: true,
                                refreshed_token,
                            });
                        }
                        Err(_) => break,
                    }
                }
                let flushed_mutations = flushed_ids.len();
                if !flushed_ids.is_empty() {
                    let p = db_path.clone();
                    let _ = tokio::task::spawn_blocking(move || {
                        if let Ok(db) = Database::open(&p) {
                            for id in flushed_ids {
                                let _ = db.delete_pending_mutation(id);
                            }
                        }
                    })
                    .await;
                }

                let playlists = match client.get_playlists(&access).await {
                    Ok(pls) => pls,
                    Err(SpotifyApiError::PremiumRequiredOrForbidden(_)) => {
                        return Ok(SpotifySyncReport {
                            profile,
                            synced_playlists: 0,
                            synced_liked: 0,
                            flushed_mutations,
                            lapsed: true,
                            refreshed_token,
                        });
                    }
                    Err(err) => return Err(err.to_string()),
                };

                let mut playlist_payloads = Vec::with_capacity(playlists.len());
                for pl in playlists {
                    match client.get_playlist_tracks(&access, &pl.id).await {
                        Ok(tracks) => playlist_payloads.push((pl.id, pl.name, tracks)),
                        Err(SpotifyApiError::PremiumRequiredOrForbidden(_)) => continue,
                        Err(_) => continue,
                    }
                }

                let liked_tracks = match client.get_liked_tracks(&access).await {
                    Ok(t) => t,
                    Err(SpotifyApiError::PremiumRequiredOrForbidden(_)) => Vec::new(),
                    Err(_) => Vec::new(),
                };

                let synced_playlists = playlist_payloads.len();
                let synced_liked = liked_tracks.len();

                tokio::task::spawn_blocking(move || -> Result<(), String> {
                    let db = Database::open(&db_path).map_err(|e| e.to_string())?;
                    let mut keep_ids = HashSet::with_capacity(playlist_payloads.len());
                    for (id, name, tracks) in playlist_payloads {
                        keep_ids.insert(id.clone());
                        db.upsert_synced_spotify_playlist(&id, &name, &tracks)
                            .map_err(|e| e.to_string())?;
                    }
                    let _ = db.prune_stale_synced_spotify_playlists(&keep_ids);
                    if !liked_tracks.is_empty() {
                        db.sync_spotify_liked_tracks(&liked_tracks)
                            .map_err(|e| e.to_string())?;
                    }
                    Ok(())
                })
                .await
                .map_err(|e| e.to_string())??;

                Ok(SpotifySyncReport {
                    profile,
                    synced_playlists,
                    synced_liked,
                    flushed_mutations,
                    lapsed: false,
                    refreshed_token,
                })
            },
            Message::SpotifySyncFinished,
        )
    }

    fn fetch_lyrics_task(&mut self, track: Track, force_refresh: bool) -> Task<Message> {
        self.lyrics_loading = true;
        let db_path = self.db_path.clone();
        let track_uid = track.uid.clone();

        Task::perform(
            async move {
                enum PreFetch {
                    Resolved(Option<LyricsData>),
                    UpgradeLegacyLrclib(LyricsData),
                    FetchRemote,
                }

                let track_pre = track.clone();
                let db_path_pre = db_path.clone();
                let pre = tokio::task::spawn_blocking(move || -> Result<PreFetch, String> {
                    let db = Database::open(&db_path_pre).map_err(|e| e.to_string())?;
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_secs() as i64)
                        .unwrap_or(0);

                    if !force_refresh {
                        match db
                            .get_cached_lyrics(&track_pre.uid, now)
                            .map_err(|e| e.to_string())?
                        {
                            LyricsCacheLookup::Hit(data) => {
                                // Auto-upgrade legacy line-only LRCLIB cache entries once to
                                // syllable-synced LyricsPlus (Apple Music / Musixmatch).
                                if !data.has_native_syllables()
                                    && matches!(data.provider.as_str(), "lrclib" | "lrclib_checked")
                                {
                                    return Ok(PreFetch::UpgradeLegacyLrclib(data));
                                }
                                return Ok(PreFetch::Resolved(Some(data)));
                            }
                            LyricsCacheLookup::NegativeHit => {
                                return Ok(PreFetch::Resolved(None));
                            }
                            LyricsCacheLookup::Miss => {}
                        }
                    }

                    if let TrackSource::Local { path } = &track_pre.source {
                        if let Some(ttml_path) = find_sidecar_ttml(path) {
                            if let Ok(raw) = std::fs::read_to_string(&ttml_path) {
                                let parsed = parse_ttml(&raw, "sidecar_ttml");
                                if !parsed.synced.is_empty() || parsed.plain.is_some() {
                                    let _ = db.put_cached_lyrics(&track_pre.uid, &parsed, now);
                                    return Ok(PreFetch::Resolved(Some(parsed)));
                                }
                            }
                        }
                        if let Some(lrc_path) = find_sidecar_lrc(path) {
                            if let Ok(raw) = std::fs::read_to_string(&lrc_path) {
                                let parsed = parse_lrc(&raw, "sidecar_lrc");
                                if !parsed.synced.is_empty() || parsed.plain.is_some() {
                                    let _ = db.put_cached_lyrics(&track_pre.uid, &parsed, now);
                                    return Ok(PreFetch::Resolved(Some(parsed)));
                                }
                            }
                        }
                        if let Some(embedded) = read_embedded_lyrics(path) {
                            let parsed = parse_lrc(&embedded, "embedded");
                            if !parsed.synced.is_empty() || parsed.plain.is_some() {
                                let _ = db.put_cached_lyrics(&track_pre.uid, &parsed, now);
                                return Ok(PreFetch::Resolved(Some(parsed)));
                            }
                        }
                    }
                    Ok(PreFetch::FetchRemote)
                })
                .await
                .map_err(|e| e.to_string())??;

                let lyricsplus = LyricsPlusClient::default_public();

                let fetched: Option<LyricsData> = match pre {
                    PreFetch::Resolved(resolved) => return Ok(resolved),
                    PreFetch::UpgradeLegacyLrclib(mut legacy) => {
                        match lyricsplus.fetch(&track).await {
                            Ok(Some(upgraded))
                                if upgraded.has_native_syllables()
                                    || upgraded.synced.iter().any(|l| l.end_ms.is_some()) =>
                            {
                                Some(upgraded)
                            }
                            _ => {
                                // Mark verified so we don't re-hit LyricsPlus on every play unless user refreshes.
                                legacy.provider = "lrclib_v2".to_string();
                                Some(legacy)
                            }
                        }
                    }
                    PreFetch::FetchRemote => {
                        let mut lp_line_fallback: Option<LyricsData> = None;
                        let mut resolved: Option<LyricsData> = None;

                        if let Ok(Some(lp_data)) = lyricsplus.fetch(&track).await {
                            if lp_data.has_native_syllables()
                                || lp_data.synced.iter().any(|l| l.end_ms.is_some())
                            {
                                resolved = Some(lp_data);
                            } else {
                                lp_line_fallback = Some(lp_data);
                            }
                        }

                        if resolved.is_none() {
                            let client = LrclibClient::new(LRCLIB_BASE_URL.to_string());
                            match client.fetch(&track).await {
                                Ok(Some(mut lrc_data)) => {
                                    if lrc_data.provider == "lrclib" {
                                        lrc_data.provider = "lrclib_v2".to_string();
                                    }
                                    resolved = Some(lrc_data);
                                }
                                Ok(None) => {
                                    resolved = lp_line_fallback;
                                }
                                Err(err) => {
                                    if let Some(lp) = lp_line_fallback {
                                        resolved = Some(lp);
                                    } else {
                                        return Err(err.to_string());
                                    }
                                }
                            }
                        }

                        resolved
                    }
                };

                let fetched_clone = fetched.clone();
                let uid_for_db = track.uid.clone();
                let _ = tokio::task::spawn_blocking(move || {
                    if let Ok(db) = Database::open(&db_path) {
                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map(|d| d.as_secs() as i64)
                            .unwrap_or(0);
                        match &fetched_clone {
                            Some(data) => {
                                let _ = db.put_cached_lyrics(&uid_for_db, data, now);
                            }
                            None => {
                                let _ = db.put_negative_lyrics_hit(&uid_for_db, now);
                            }
                        }
                    }
                })
                .await;

                Ok(fetched)
            },
            move |result| Message::LyricsLoaded { track_uid, result },
        )
    }

    fn spawn_local_scan_task(&self) -> Task<Message> {
        let folders = self.config.local_folders.clone();
        let db_path = self.db_path.clone();
        let art_cache = Arc::clone(&self.art_cache);

        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    let db = Database::open(&db_path).map_err(|e| e.to_string())?;
                    let provider = LocalProvider::new(db_path.clone());
                    let mut cache = art_cache.lock().map_err(|e| e.to_string())?;
                    let report = provider
                        .scan_folders(&folders, &db, &mut cache)
                        .map_err(|e| e.to_string())?;
                    let tracks = db.list_local_tracks().map_err(|e| e.to_string())?;
                    Ok((report, tracks))
                })
                .await
                .map_err(|e| e.to_string())?
            },
            Message::LocalScanFinished,
        )
    }

    fn init_smtc_task(&self, id: window::Id) -> Task<Message> {
        window::run(id, extract_hwnd).map(Message::SmtcHwndReady)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn never_ticks_when_hidden_or_minimized_even_while_playing() {
        assert!(!should_tick_frames(
            false,
            false,
            false,
            ViewMode::NowPlayingLyrics,
            true,
            true,
            true,
            false
        ));
        assert!(!should_tick_frames(
            true,
            true,
            false,
            ViewMode::NowPlayingLyrics,
            true,
            true,
            true,
            false
        ));
    }

    #[test]
    fn never_ticks_when_reduced_motion_enabled() {
        assert!(!should_tick_frames(
            true,
            false,
            true,
            ViewMode::NowPlayingLyrics,
            true,
            true,
            true,
            false
        ));
    }

    #[test]
    fn stops_ticking_when_paused_and_transitions_and_spring_settled() {
        assert!(!should_tick_frames(
            true,
            false,
            false,
            ViewMode::NowPlayingLyrics,
            false,
            false,
            false,
            true
        ));
    }

    #[test]
    fn browse_mode_skips_60fps_ticks_while_playing_unless_palette_crossfading() {
        assert!(!should_tick_frames(
            true,
            false,
            false,
            ViewMode::Browse,
            true,
            false,
            false,
            true
        ));
        assert!(should_tick_frames(
            true,
            false,
            false,
            ViewMode::Browse,
            true,
            false,
            true,
            true
        ));
    }

    #[test]
    fn lyrics_mode_ticks_when_playing_loading_transitioning_or_spring_unsettled() {
        assert!(should_tick_frames(
            true,
            false,
            false,
            ViewMode::NowPlayingLyrics,
            true,
            false,
            false,
            true
        ));
        assert!(should_tick_frames(
            true,
            false,
            false,
            ViewMode::NowPlayingLyrics,
            false,
            true,
            false,
            true
        ));
        assert!(should_tick_frames(
            true,
            false,
            false,
            ViewMode::NowPlayingLyrics,
            false,
            false,
            true,
            true
        ));
        assert!(should_tick_frames(
            true,
            false,
            false,
            ViewMode::NowPlayingLyrics,
            false,
            false,
            false,
            false
        ));
    }

    #[test]
    fn bounded_cover_decoder_downscales_oversized_art_to_max_dim() {
        let img = ::image::RgbaImage::from_pixel(1024, 1024, ::image::Rgba([200, 120, 80, 255]));
        let mut png = std::io::Cursor::new(Vec::new());
        img.write_to(&mut png, ::image::ImageFormat::Png).unwrap();
        let (w, h, rgba) = decode_bounded_cover_rgba(&png.into_inner(), 512).unwrap();
        assert_eq!((w, h), (512, 512));
        assert_eq!(rgba.len(), 512 * 512 * 4);
    }

    #[test]
    fn phase4_edge_case_classifiers_handle_lapsed_premium_offline_and_ytm_403() {
        assert!(is_spotify_lapsed_reason("Spotify Premium required for native playback"));
        assert!(is_spotify_lapsed_reason("HTTP 403 Forbidden"));
        assert!(!is_spotify_lapsed_reason("Temporary track decode glitch"));

        assert!(is_likely_offline_error("error sending request for url: dns error"));
        assert!(is_likely_offline_error("Network is unreachable"));
        assert!(!is_likely_offline_error("HTTP 403 Forbidden"));

        let no_ytdlp = format_ytm_stream_error_toast("HTTP 403 Forbidden (PO token)", false);
        assert!(no_ytdlp.contains("winget install yt-dlp.yt-dlp"));
        let with_ytdlp = format_ytm_stream_error_toast("HTTP 403 Forbidden", true);
        assert!(with_ytdlp.contains("yt-dlp -U"));
        let bot_check = format_ytm_stream_error_toast(
            "yt-dlp stream failed (exit code: 1): ERROR: [youtube] abc: Sign in to confirm you're not a bot.",
            true,
        );
        assert!(bot_check.contains("temporarily flagged this network IP"));
        let offline = format_ytm_stream_error_toast("dns error: no such host", false);
        assert!(offline.contains("Offline"));
    }

    #[test]
    fn remote_art_url_helper_detects_http_and_https_only() {
        assert!(is_remote_art_url("https://lh3.googleusercontent.com/abc=w544-h544"));
        assert!(is_remote_art_url("http://127.0.0.1:8080/cover.jpg"));
        assert!(!is_remote_art_url("local:abcdef1234"));
        assert!(!is_remote_art_url("ytm:dQw4w9WgXcQ"));
    }

    #[test]
    fn merge_search_results_deduplicates_uid_preserving_cached_liked_state() {
        let local = Track {
            uid: "local:1".into(),
            source: TrackSource::Local {
                path: PathBuf::from(r"D:\Music\A.mp3"),
            },
            title: "Song A".into(),
            artists: vec!["Artist".into()],
            album: None,
            duration_ms: 120_000,
            artwork_ref: None,
            is_liked: false,
        };
        let cached_ytm = Track {
            uid: "ytm:vid1".into(),
            source: TrackSource::YouTubeMusic {
                video_id: "vid1".into(),
            },
            title: "YTM Liked".into(),
            artists: vec!["Band".into()],
            album: None,
            duration_ms: 180_000,
            artwork_ref: Some("https://lh3.googleusercontent.com/cover".into()),
            is_liked: true,
        };
        let fresh_ytm_dup = Track {
            is_liked: false,
            ..cached_ytm.clone()
        };
        let fresh_ytm_new = Track {
            uid: "ytm:vid2".into(),
            source: TrackSource::YouTubeMusic {
                video_id: "vid2".into(),
            },
            title: "YTM Fresh".into(),
            artists: vec!["Band".into()],
            album: None,
            duration_ms: 200_000,
            artwork_ref: None,
            is_liked: false,
        };

        let merged = merge_search_results(
            vec![local, cached_ytm],
            vec![fresh_ytm_dup, fresh_ytm_new],
        );
        assert_eq!(merged.len(), 3);
        let ytm1 = merged.iter().find(|t| t.uid == "ytm:vid1").unwrap();
        assert!(ytm1.is_liked, "cached liked state must be preserved");
    }

    #[test]
    fn spotify_native_routing_respects_fallback_preference_tier_and_lapsed_state() {
        assert!(should_attempt_spotify_native(
            false,
            true,
            SpotifyAccountTier::Premium,
            false
        ));
        assert!(should_attempt_spotify_native(
            false,
            true,
            SpotifyAccountTier::Unknown,
            false
        ));
        assert!(!should_attempt_spotify_native(
            true,
            true,
            SpotifyAccountTier::Premium,
            false
        ));
        assert!(!should_attempt_spotify_native(
            false,
            false,
            SpotifyAccountTier::Premium,
            false
        ));
        assert!(!should_attempt_spotify_native(
            false,
            true,
            SpotifyAccountTier::Free,
            false
        ));
        assert!(!should_attempt_spotify_native(
            false,
            true,
            SpotifyAccountTier::Premium,
            true
        ));
    }

    #[test]
    fn connection_status_pill_reflects_active_spotify_and_fallback_modes() {
        assert_eq!(
            format_connection_status_pill(false, false, SpotifyAccountTier::Unknown, false, false),
            "Local + YouTube Music"
        );
        assert_eq!(
            format_connection_status_pill(true, true, SpotifyAccountTier::Premium, false, false),
            "Spotify Premium"
        );
        assert_eq!(
            format_connection_status_pill(true, true, SpotifyAccountTier::Premium, true, false),
            "Spotify Lapsed • YTM"
        );
        assert_eq!(
            format_connection_status_pill(true, false, SpotifyAccountTier::Free, false, false),
            "Spotify Free • YTM"
        );
        assert_eq!(
            format_connection_status_pill(true, true, SpotifyAccountTier::Premium, false, true),
            "Spotify • YTM Fallback"
        );
    }

    #[test]
    fn nav_history_pushes_deduplicates_and_caps_cleanly() {
        let mut hist = Vec::new();
        push_nav_history(&mut hist, ActiveNav::Search, &ActiveNav::Search);
        assert!(hist.is_empty(), "same nav should not be pushed");

        let artist_nav = ActiveNav::ArtistProfile {
            source: CatalogSource::YouTubeMusic,
            id: "UC1".into(),
            name: "Hindia".into(),
        };
        push_nav_history(&mut hist, ActiveNav::Search, &artist_nav);
        assert_eq!(hist, vec![ActiveNav::Search]);

        // Cap check.
        for i in 0..40 {
            push_nav_history(
                &mut hist,
                ActiveNav::Playlist(format!("pl_{i}")),
                &ActiveNav::Playlist(format!("pl_{}", i + 1)),
            );
        }
        assert_eq!(hist.len(), MAX_NAV_HISTORY);
    }

    #[test]
    fn pick_best_artist_match_prefers_exact_then_prefix_then_first() {
        let mk = |id: &str, name: &str| ArtistSummary {
            id: id.into(),
            source: CatalogSource::YouTubeMusic,
            name: name.into(),
            avatar_url: None,
            subtitle: None,
        };
        let list = vec![
            mk("1", "Hindia Official Fan"),
            mk("2", "Hindia"),
            mk("3", "The Hindia"),
        ];
        assert_eq!(pick_best_artist_match("hindia", &list).unwrap().id, "2");
        assert_eq!(
            pick_best_artist_match("hindia official", &list).unwrap().id,
            "1"
        );
        assert_eq!(pick_best_artist_match("unrelated", &list).unwrap().id, "1");
    }
}
