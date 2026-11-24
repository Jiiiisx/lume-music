use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use iced::futures::SinkExt;
use iced::widget::{container, image, operation, row, shader, stack};
use iced::{window, Background, Color, Element, Length, Subscription, Task, Theme};
use lume_audio::{AudioCommand, AudioEngineHandle, AudioEvent};
use lume_core::{
    LyricsData, MusicProvider, PlaybackSourceUsed, PlaybackStatus, Playlist, QueueActionResult,
    QueueEngine, ResolvedStream, Track, TrackSource,
};
use lume_providers::{
    canonical_playlist_url, detect_yt_dlp_binary, ensure_fresh_token,
    fetch_spotify_track_oembed_cover, find_sidecar_lrc, find_sidecar_ttml,
    import_spotify_playlist_url, login_spotify_playback, login_spotify_web_api,
    needs_spotify_album_artwork, parse_exportify_csv, parse_lrc, parse_ttml, read_embedded_lyrics,
    resolve_spotify_to_ytm_video_id, LocalProvider, LrclibClient, SpotifyAccountTier,
    SpotifyApiError, SpotifyUserProfile, SpotifyWebClient, YouTubeMusicProvider,
    SPOTIFY_PLAYBACK_CLIENT_ID,
};
use lume_storage::{
    AppConfig, ArtCache, Database, GlassmorphismMode, LyricsCacheLookup, SpotifyTokenBundle,
    TokenStore,
};
use tokio::sync::mpsc::{unbounded_channel, Receiver, UnboundedReceiver, UnboundedSender};

use crate::lyrics_anim::{
    active_display_index, build_display_timeline, target_scroll_y_for_item, LyricsSpringState,
};
use crate::platform::smtc::{extract_hwnd, SmtcController};
use crate::platform::apply_window_glass_mode;
use crate::theme::{
    decode_art_blur_rgba, extract_palette, AccentTheme, AlbumPalette, DynamicBackdropProgram,
};
use crate::views::browse::{validate_playlist_name, view_browse, view_sidebar, BrowseViewProps};
use crate::views::lyrics_view::{lyrics_scroll_id, view_now_playing_lyrics, LyricsViewProps};
use crate::views::player_bar::view_player_bar;
use crate::views::queue_drawer::view_queue_drawer;
use crate::views::settings::view_settings;
use crate::views::titlebar::view_titlebar;
use crate::views::{
    ActiveNav, Message, NowPlayingInfo, PlaylistSort, SmtcAction, SourceFilter, SpotifySearchError,
    SpotifySettingsProps, SpotifySyncReport, ViewMode,
};

const MAX_IN_MEM_ART_HANDLES: usize = 250;
const THUMBNAIL_SIZE_PX: u32 = 96;
const MAX_REMOTE_ART_BYTES: usize = 10 * 1024 * 1024;
const PALETTE_TRANSITION_SECS: f32 = 0.6;
const LRCLIB_BASE_URL: &str = "https://lrclib.net";

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
            .timeout(Duration::from_secs(8))
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

    let resp = shared_art_http_client().get(&key).send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let bytes = resp.bytes().await.ok()?;
    if bytes.is_empty() || bytes.len() > MAX_REMOTE_ART_BYTES {
        return None;
    }
    let vec = bytes.to_vec();
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

pub fn should_tick_frames(
    window_visible: bool,
    minimized: bool,
    reduced_motion: bool,
    is_playing: bool,
    transition_active: bool,
    spring_settled: bool,
) -> bool {
    if !window_visible || minimized || reduced_motion {
        return false;
    }
    is_playing || transition_active || !spring_settled
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

    main_window: Option<window::Id>,
    window_visible: bool,
    window_focused: bool,
    minimized: bool,
    maximized: bool,
    window_width: f32,
    window_height: f32,

    view_mode: ViewMode,
    nav: ActiveNav,
    queue_open: bool,

    search_query: String,
    source_filter: SourceFilter,
    playlist_sorts: HashMap<String, PlaylistSort>,
    playlists: Vec<Playlist>,
    local_tracks: Vec<Track>,
    liked_tracks: Vec<Track>,
    playlist_tracks: Vec<Track>,
    search_results: Vec<Track>,
    spotify_search_results: Vec<Track>,
    ytm_search_results: Vec<Track>,
    searching_spotify: bool,
    searching_ytm: bool,
    resolving_stream: bool,
    search_request_id: u64,
    play_request_id: u64,
    yt_dlp_detected: Option<PathBuf>,

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
    lyrics_loading: bool,
    lyrics_spring: LyricsSpringState,
    last_scrolled_line: Option<usize>,

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
    renaming_playlist: Option<(String, String)>,
    show_new_playlist_modal: bool,
    new_playlist_input: String,
    show_import_url_modal: bool,
    import_url_input: String,
    modal_error: Option<String>,
    status_toast: Option<String>,
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

        let mut app = Self {
            config,
            config_path,
            db_path,
            db,
            art_cache,
            token_store,
            tokens,
            audio,
            main_window: None,
            window_visible: true,
            window_focused: true,
            minimized: false,
            maximized: false,
            window_width: 1180.0,
            window_height: 760.0,
            view_mode: ViewMode::Browse,
            nav: ActiveNav::LocalFiles,
            queue_open: false,
            search_query: String::new(),
            source_filter: SourceFilter::All,
            playlist_sorts: HashMap::new(),
            playlists,
            local_tracks,
            liked_tracks,
            playlist_tracks: Vec::new(),
            search_results: Vec::new(),
            spotify_search_results: Vec::new(),
            ytm_search_results: Vec::new(),
            searching_spotify: false,
            searching_ytm: false,
            resolving_stream: false,
            search_request_id: 0,
            play_request_id: 0,
            yt_dlp_detected,
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
            lyrics_loading: false,
            lyrics_spring: LyricsSpringState::new(),
            last_scrolled_line: None,
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
            renaming_playlist: None,
            show_new_playlist_modal: false,
            new_playlist_input: String::new(),
            show_import_url_modal: false,
            import_url_input: String::new(),
            modal_error: None,
            status_toast: None,
            scanning_folders: false,
            scan_summary: None,
            smtc: SmtcController::new(),
            smtc_tx,
        };

        let mut startup_tasks = vec![window::oldest().map(|id| {
            id.map(Message::WindowOpened)
                .unwrap_or(Message::DismissToast)
        })];

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
            is_playing,
            transition_active,
            spring_settled,
        ) {
            subs.push(window::frames().map(Message::FrameTick));
        }

        Subscription::batch(subs)
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
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
                    if size.width > 0.0 && size.height > 0.0 {
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
                        let timeline = build_display_timeline(&lyrics.synced);
                        let idx = active_display_index(&timeline, self.position_ms);
                        self.lyrics_spring.notify_active_item(idx);
                        if let Some(idx) = idx {
                            let target_y = target_scroll_y_for_item(
                                &timeline,
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
                self.nav = nav.clone();
                if let ActiveNav::Playlist(pl_id) = nav {
                    if let Ok(db) = self.db.lock() {
                        self.playlist_tracks = db.get_playlist_tracks(&pl_id).unwrap_or_default();
                    }
                    let keys = self.missing_art_keys(&self.playlist_tracks);
                    let tracks_snapshot = self.playlist_tracks.clone();
                    return Task::batch([
                        self.preload_thumbnails_task(keys),
                        self.spawn_spotify_artwork_enrichment_task(&tracks_snapshot),
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
            Message::SetViewMode(mode) => {
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
                self.queue_open = !self.queue_open;
            }
            Message::OpenNewPlaylistModal => {
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
                let yt_dlp_path = self.config.yt_dlp_path.clone();
                let track_for_msg = track.clone();
                return Task::perform(
                    async move {
                        let ytm = YouTubeMusicProvider::new(
                            YouTubeMusicProvider::default_cache_dir(),
                            yt_dlp_path,
                        )
                        .map_err(|e| e.to_string())?;
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
                if let ActiveNav::Playlist(pl_id) = &self.nav {
                    self.playlist_sorts
                        .entry(pl_id.clone())
                        .or_default()
                        .select(field);
                }
            }
            Message::TogglePlaylistSortDirection => {
                if let ActiveNav::Playlist(pl_id) = &self.nav {
                    self.playlist_sorts
                        .entry(pl_id.clone())
                        .or_default()
                        .toggle_direction();
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
            Message::SearchInputChanged(val) => {
                self.search_query = val;
                if self.search_query.trim().is_empty() {
                    self.spotify_search_results.clear();
                    self.ytm_search_results.clear();
                    self.searching_spotify = false;
                    self.searching_ytm = false;
                }
                self.refresh_search_results();
                if !self.search_query.trim().is_empty()
                    && matches!(self.nav, ActiveNav::LocalFiles | ActiveNav::LikedSongs)
                {
                    self.nav = ActiveNav::Search;
                }
            }
            Message::SearchSubmitted => {
                self.nav = ActiveNav::Search;
                self.refresh_search_results();
                let query = self.search_query.trim().to_string();
                if !query.is_empty() {
                    return self.trigger_remote_search_tasks(query);
                }
            }
            Message::SourceFilterChanged(filter) => {
                self.source_filter = filter;
                let query = self.search_query.trim().to_string();
                if matches!(self.nav, ActiveNav::Search) && !query.is_empty() {
                    let need_ytm = matches!(filter, SourceFilter::YouTubeMusic | SourceFilter::All)
                        && self.ytm_search_results.is_empty()
                        && !self.searching_ytm;
                    let need_spotify = matches!(filter, SourceFilter::Spotify | SourceFilter::All)
                        && self.can_query_spotify_web_api()
                        && self.spotify_search_results.is_empty()
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
                        let keys = self.missing_art_keys(&tracks);
                        self.ytm_search_results = tracks;
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
                        let keys = self.missing_art_keys(&tracks);
                        self.spotify_search_results = tracks;
                        self.refresh_search_results();
                        return self.preload_thumbnails_task(keys);
                    }
                    Err(SpotifySearchError::LapsedOrForbidden) => {
                        self.spotify_lapsed = true;
                        self.spotify_search_results.clear();
                        self.refresh_search_results();
                        self.status_toast = Some(
                            "Spotify Web API unavailable (Premium lapsed/403) — showing Local + YouTube Music."
                                .to_string(),
                        );
                    }
                    Err(SpotifySearchError::Other(err)) => {
                        self.status_toast = Some(format!("Spotify search failed: {err}"));
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
                self.queue.play_next(track);
                self.persist_session();
            }
            Message::AddToQueue(track) => {
                self.open_track_menu_uid = None;
                self.queue.add_to_queue(track);
                self.persist_session();
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
                            self.current_lyrics = data;
                            self.current_lyrics_uid = Some(track_uid);
                            self.last_scrolled_line = None;
                        }
                        Err(err) => {
                            self.current_lyrics = None;
                            self.status_toast = Some(format!("Lyrics error: {err}"));
                        }
                    }
                }
            }
            Message::StreamResolved {
                request_id,
                track,
                result,
            } => {
                if request_id != self.play_request_id {
                    return Task::none();
                }
                self.resolving_stream = false;
                match result {
                    Ok(ResolvedStream::HttpUrl { url, source }) => {
                        self.source_used = Some(source);
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
                        self.resolving_stream = true;
                        return self.prepare_spotify_playback_token_task(request_id, track);
                    }
                    Err(err) => {
                        self.status = PlaybackStatus::Stopped;
                        self.smtc.set_playback(PlaybackStatus::Stopped, 0);
                        self.status_toast = Some(err);
                    }
                }
            }
            Message::PaletteLoaded {
                track_uid,
                art_key,
                palette,
                art_bytes,
                art_blur_rgba,
            } => {
                let handle_key = art_key.clone().unwrap_or_else(|| track_uid.clone());
                if let Some(bytes) = art_bytes {
                    self.insert_art_handle(handle_key.clone(), image::Handle::from_bytes(bytes));
                }
                if self.queue.current().map(|t| t.uid.as_str()) == Some(track_uid.as_str()) {
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
                let active_art_key = self
                    .queue
                    .current()
                    .and_then(|t| t.artwork_ref.as_deref())
                    .map(ToString::to_string);
                for (key, w, h, rgba) in items {
                    if active_art_key.as_deref() == Some(key.as_str())
                        && self.art_handles.contains_key(&key)
                    {
                        continue;
                    }
                    self.insert_art_handle(key, image::Handle::from_rgba(w, h, rgba));
                }
            }
            Message::SpotifyArtworkEnriched(pairs) => {
                if pairs.is_empty() {
                    return Task::none();
                }
                let updated_map: HashMap<String, String> = pairs.into_iter().collect();
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
                for t in &mut self.spotify_search_results {
                    if let Some(url) = updated_map.get(&t.uid) {
                        t.artwork_ref = Some(url.clone());
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
                let remaining_snapshot = match self.nav {
                    ActiveNav::Playlist(_) => self.playlist_tracks.clone(),
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
                self.status_toast = Some("Saved yt-dlp fallback path.".to_string());
            }
            Message::YtDlpPicked(None) => {}
            Message::ClearYtDlpBinary => {
                self.config.yt_dlp_path = None;
                let _ = self.config.save(&self.config_path);
                self.yt_dlp_detected = detect_yt_dlp_binary(None);
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
                    self.status = st;
                    self.smtc.set_playback(st, self.position_ms);
                }
                AudioEvent::PositionTick {
                    position_ms,
                    duration_ms,
                } => {
                    self.position_ms = position_ms;
                    self.last_audio_pos_ms = position_ms;
                    self.last_audio_pos_at = Instant::now();
                    if duration_ms > 0 {
                        self.duration_ms = duration_ms;
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
                    self.status = PlaybackStatus::Stopped;
                    self.status_toast = Some(err);
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
            }
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

        let foreground: Element<'_, Message> = match self.view_mode {
            ViewMode::NowPlayingLyrics => {
                let lyrics_view = view_now_playing_lyrics(LyricsViewProps {
                    info,
                    lyrics: self.current_lyrics.as_ref(),
                    lyrics_loading: self.lyrics_loading,
                    lyrics_mode: self.config.lyrics_visual_mode,
                    active_scale: self.lyrics_spring.active_scale(),
                    window_width: self.window_width,
                    window_height: self.window_height,
                    art_handles: &self.art_handles,
                    time_s: self.time_s,
                    queue_open: self.queue_open,
                    status_banner: self.status_toast.as_deref(),
                    accent,
                });

                if self.queue_open {
                    let active_list = self.queue.active_list();
                    stack![
                        lyrics_view,
                        container(view_queue_drawer(
                            &active_list,
                            self.queue.current_pos(),
                            accent,
                        ))
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .align_x(iced::alignment::Horizontal::Right)
                        .padding([48, 16])
                    ]
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .into()
                } else {
                    lyrics_view
                }
            }
            ViewMode::Browse => {
                let titlebar = view_titlebar(
                    self.source_used.map(|s| s.badge_label()),
                    &self.search_query,
                    self.source_filter,
                    self.queue_open,
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
                        view_sidebar(&self.nav, &self.playlists, connection_status, accent),
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
                    view_browse(BrowseViewProps {
                        nav: &self.nav,
                        search_query: &self.search_query,
                        source_filter: self.source_filter,
                        playlist_sort,
                        searching_remote: self.searching_ytm || self.searching_spotify,
                        playlists: &self.playlists,
                        visible_tracks,
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

                let player_bar = view_player_bar(info, &self.art_handles, self.queue_open, accent);

                let main_col = iced::widget::column![
                    titlebar,
                    container(center_content)
                        .width(Length::Fill)
                        .height(Length::Fill),
                    player_bar,
                ]
                .width(Length::Fill)
                .height(Length::Fill);

                if self.queue_open {
                    let active_list = self.queue.active_list();
                    stack![
                        main_col,
                        container(view_queue_drawer(
                            &active_list,
                            self.queue.current_pos(),
                            accent,
                        ))
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .align_x(iced::alignment::Horizontal::Right)
                        .padding([48, 16])
                    ]
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .into()
                } else {
                    main_col.into()
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
            ActiveNav::Search => &self.search_results,
            ActiveNav::LikedSongs => &self.liked_tracks,
            ActiveNav::LocalFiles => &self.local_tracks,
            ActiveNav::Playlist(_) => &self.playlist_tracks,
            ActiveNav::Settings => &self.local_tracks,
        }
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

        if matches!(
            self.source_filter,
            SourceFilter::All | SourceFilter::YouTubeMusic
        ) {
            self.searching_ytm = true;
            tasks.push(self.spawn_ytm_search_task(query.clone(), req_id));
        }

        if matches!(self.source_filter, SourceFilter::All | SourceFilter::Spotify)
            && self.can_query_spotify_web_api()
        {
            self.searching_spotify = true;
            tasks.push(self.spawn_spotify_search_task(query, req_id));
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
        self.position_ms = 0;
        self.last_audio_pos_ms = 0;
        self.last_audio_pos_at = Instant::now();
        self.duration_ms = track.duration_ms;
        self.lyrics_spring.snap_to(0.0);
        self.last_scrolled_line = None;
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
                tasks.push(self.resolve_ytm_stream_task(req_id, track));
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
                    tasks.push(self.prepare_spotify_playback_token_task(req_id, track));
                } else {
                    self.source_used = Some(PlaybackSourceUsed::YouTubeFallback);
                    tasks.push(self.resolve_spotify_fallback_stream_task(req_id, track, false));
                }
            }
        }

        Task::batch(tasks)
    }

    fn stop_playback(&mut self) {
        self.play_request_id = self.play_request_id.wrapping_add(1);
        self.resolving_stream = false;
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
                    let blur_rgba = bytes.as_deref().and_then(decode_art_blur_rgba).map(Arc::new);
                    (track_uid, art_key, palette, bytes, blur_rgba)
                })
                .await
                .unwrap_or_else(|_| {
                    (
                        String::new(),
                        None,
                        AlbumPalette::default_dark(),
                        None,
                        None,
                    )
                })
            },
            |(track_uid, art_key, palette, art_bytes, art_blur_rgba)| Message::PaletteLoaded {
                track_uid,
                art_key,
                palette,
                art_bytes,
                art_blur_rgba,
            },
        )
    }

    fn preload_thumbnails_task(&self, keys: Vec<String>) -> Task<Message> {
        if keys.is_empty() {
            return Task::none();
        }
        // Emit chunks of 16 progressively so visible covers appear in ~150 ms instead of waiting
        // for all 200+ images in a large playlist to finish downloading and decoding.
        let mut tasks = Vec::new();
        for group in keys.chunks(16) {
            let group_keys: Vec<String> = group.to_vec();
            let art_cache = Arc::clone(&self.art_cache);
            tasks.push(Task::perform(
                async move {
                    let mut set = tokio::task::JoinSet::new();
                    for key in group_keys {
                        let cache = Arc::clone(&art_cache);
                        set.spawn(async move {
                            let bytes = fetch_or_cache_art(cache, key.clone()).await?;
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
        Task::perform(
            async move {
                let client = shared_art_http_client().clone();

                let mut resolved = Vec::with_capacity(batch.len());
                for chunk in batch.chunks(16) {
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

                resolved
            },
            Message::SpotifyArtworkEnriched,
        )
    }

    fn spawn_ytm_search_task(&self, query: String, request_id: u64) -> Task<Message> {
        let yt_dlp_path = self.config.yt_dlp_path.clone();
        Task::perform(
            async move {
                let provider = YouTubeMusicProvider::new(
                    YouTubeMusicProvider::default_cache_dir(),
                    yt_dlp_path,
                )
                .map_err(|e| e.to_string())?;
                provider.search(&query, 20).await.map_err(|e| e.to_string())
            },
            move |result| Message::YtmSearchFinished { request_id, result },
        )
    }

    fn resolve_ytm_stream_task(&self, request_id: u64, track: Track) -> Task<Message> {
        let yt_dlp_path = self.config.yt_dlp_path.clone();
        let track_for_msg = track.clone();
        Task::perform(
            async move {
                let provider = YouTubeMusicProvider::new(
                    YouTubeMusicProvider::default_cache_dir(),
                    yt_dlp_path,
                )
                .map_err(|e| e.to_string())?;
                provider
                    .resolve_stream(&track)
                    .await
                    .map_err(|e| e.to_string())
            },
            move |result| Message::StreamResolved {
                request_id,
                track: track_for_msg,
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
        let yt_dlp_path = self.config.yt_dlp_path.clone();
        let track_for_msg = track.clone();
        Task::perform(
            async move {
                let ytm = YouTubeMusicProvider::new(
                    YouTubeMusicProvider::default_cache_dir(),
                    yt_dlp_path,
                )
                .map_err(|e| e.to_string())?;
                let matched = resolve_spotify_to_ytm_video_id(&track, &db, &ytm, force_rematch)
                    .await
                    .map_err(|e| e.to_string())?;
                let ytm_proxy = Track {
                    uid: format!("ytm:{}", matched.yt_video_id),
                    source: TrackSource::YouTubeMusic {
                        video_id: matched.yt_video_id,
                    },
                    title: track.title.clone(),
                    artists: track.artists.clone(),
                    album: track.album.clone(),
                    duration_ms: track.duration_ms,
                    artwork_ref: track.artwork_ref.clone(),
                    is_liked: track.is_liked,
                };
                let resolved = ytm
                    .resolve_stream(&ytm_proxy)
                    .await
                    .map_err(|e| e.to_string())?;
                Ok(rewrite_stream_as_youtube_fallback(resolved))
            },
            move |result| Message::StreamResolved {
                request_id,
                track: track_for_msg,
                result,
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
                let res = client.search_tracks(&tok.access_token, &query, 20).await;
                let refreshed_token = refreshed.then_some(tok);
                match res {
                    Ok(tracks) => Message::SpotifySearchFinished {
                        request_id,
                        result: Ok(tracks),
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
                let track_pre = track.clone();
                let db_path_pre = db_path.clone();
                let pre = tokio::task::spawn_blocking(move || -> Result<Option<Option<LyricsData>>, String> {
                    let db = Database::open(&db_path_pre).map_err(|e| e.to_string())?;
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_secs() as i64)
                        .unwrap_or(0);

                    if !force_refresh {
                        match db.get_cached_lyrics(&track_pre.uid, now).map_err(|e| e.to_string())? {
                            LyricsCacheLookup::Hit(data) => return Ok(Some(Some(data))),
                            LyricsCacheLookup::NegativeHit => return Ok(Some(None)),
                            LyricsCacheLookup::Miss => {}
                        }
                    }

                    if let TrackSource::Local { path } = &track_pre.source {
                        if let Some(ttml_path) = find_sidecar_ttml(path) {
                            if let Ok(raw) = std::fs::read_to_string(&ttml_path) {
                                let parsed = parse_ttml(&raw, "sidecar_ttml");
                                if !parsed.synced.is_empty() || parsed.plain.is_some() {
                                    let _ = db.put_cached_lyrics(&track_pre.uid, &parsed, now);
                                    return Ok(Some(Some(parsed)));
                                }
                            }
                        }
                        if let Some(lrc_path) = find_sidecar_lrc(path) {
                            if let Ok(raw) = std::fs::read_to_string(&lrc_path) {
                                let parsed = parse_lrc(&raw, "sidecar_lrc");
                                if !parsed.synced.is_empty() || parsed.plain.is_some() {
                                    let _ = db.put_cached_lyrics(&track_pre.uid, &parsed, now);
                                    return Ok(Some(Some(parsed)));
                                }
                            }
                        }
                        if let Some(embedded) = read_embedded_lyrics(path) {
                            let parsed = parse_lrc(&embedded, "embedded");
                            if !parsed.synced.is_empty() || parsed.plain.is_some() {
                                let _ = db.put_cached_lyrics(&track_pre.uid, &parsed, now);
                                return Ok(Some(Some(parsed)));
                            }
                        }
                    }
                    Ok(None)
                })
                .await
                .map_err(|e| e.to_string())??;

                if let Some(resolved) = pre {
                    return Ok(resolved);
                }

                let client = LrclibClient::new(LRCLIB_BASE_URL.to_string());
                let fetched = client.fetch(&track).await.map_err(|e| e.to_string())?;
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
        assert!(!should_tick_frames(false, false, false, true, true, false));
        assert!(!should_tick_frames(true, true, false, true, true, false));
    }

    #[test]
    fn never_ticks_when_reduced_motion_enabled() {
        assert!(!should_tick_frames(true, false, true, true, true, false));
    }

    #[test]
    fn stops_ticking_when_paused_and_transitions_and_spring_settled() {
        assert!(!should_tick_frames(true, false, false, false, false, true));
    }

    #[test]
    fn ticks_when_visible_and_playing_or_palette_transitioning() {
        assert!(should_tick_frames(true, false, false, true, false, true));
        assert!(should_tick_frames(true, false, false, false, true, true));
        assert!(should_tick_frames(true, false, false, false, false, false));
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
}
