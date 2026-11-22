use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

use iced::futures::SinkExt;
use iced::widget::{container, image, operation, row, shader, stack};
use iced::{window, Background, Color, Element, Length, Subscription, Task, Theme};
use lume_audio::{AudioCommand, AudioEngineHandle, AudioEvent};
use lume_core::{
    LyricsData, PlaybackSourceUsed, PlaybackStatus, Playlist, QueueActionResult, QueueEngine,
    Track, TrackSource,
};
use lume_providers::{find_sidecar_lrc, parse_lrc, read_embedded_lyrics, LocalProvider, LrclibClient};
use lume_storage::{AppConfig, ArtCache, Database, GlassmorphismMode, LyricsCacheLookup};
use tokio::sync::mpsc::{unbounded_channel, Receiver, UnboundedReceiver, UnboundedSender};

use crate::lyrics_anim::{active_line_index, LyricsSpringState};
use crate::platform::smtc::{extract_hwnd, SmtcController};
use crate::platform::{apply_window_glass_mode, TrayController};
use crate::theme::{decode_art_blur_rgba, extract_palette, AlbumPalette, DynamicBackdropProgram};
use crate::views::browse::{validate_playlist_name, view_browse, view_sidebar, BrowseViewProps};
use crate::views::lyrics_view::{
    lyrics_scroll_id, view_now_playing_lyrics, LyricsViewProps, LYRIC_LINE_STRIDE_PX,
};
use crate::views::player_bar::view_player_bar;
use crate::views::queue_drawer::view_queue_drawer;
use crate::views::settings::view_settings;
use crate::views::titlebar::view_titlebar;
use crate::views::{
    ActiveNav, Message, NowPlayingInfo, SmtcAction, SourceFilter, TrayAction, ViewMode,
};

const MAX_IN_MEM_ART_HANDLES: usize = 30;
const PALETTE_TRANSITION_SECS: f32 = 0.6;
const LRCLIB_BASE_URL: &str = "https://lrclib.net";

static AUDIO_RX_SLOT: OnceLock<Mutex<Option<Receiver<AudioEvent>>>> = OnceLock::new();
static TRAY_RX_SLOT: OnceLock<Mutex<Option<UnboundedReceiver<TrayAction>>>> = OnceLock::new();
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

fn tray_subscription() -> impl iced::futures::Stream<Item = Message> {
    iced::stream::channel(32, async move |mut output| {
        let mut rx = TRAY_RX_SLOT
            .get()
            .and_then(|m| m.lock().ok().and_then(|mut g| g.take()));
        let Some(ref mut receiver) = rx else {
            std::future::pending::<()>().await;
            return;
        };
        while let Some(action) = receiver.recv().await {
            if output.send(Message::Tray(action)).await.is_err() {
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
    audio: Option<AudioEngineHandle>,

    main_window: Option<window::Id>,
    window_visible: bool,
    minimized: bool,
    maximized: bool,

    view_mode: ViewMode,
    nav: ActiveNav,
    queue_open: bool,

    search_query: String,
    source_filter: SourceFilter,
    playlists: Vec<Playlist>,
    local_tracks: Vec<Track>,
    liked_tracks: Vec<Track>,
    playlist_tracks: Vec<Track>,
    search_results: Vec<Track>,

    queue: QueueEngine,
    status: PlaybackStatus,
    source_used: Option<PlaybackSourceUsed>,
    position_ms: u64,
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
    _tray: Option<TrayController>,
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

        let (tray_tx, tray_rx) = unbounded_channel();
        *TRAY_RX_SLOT
            .get_or_init(|| Mutex::new(None))
            .lock()
            .unwrap() = Some(tray_rx);
        let tray = TrayController::new(tray_tx);

        let (smtc_tx, smtc_rx) = unbounded_channel();
        *SMTC_RX_SLOT
            .get_or_init(|| Mutex::new(None))
            .lock()
            .unwrap() = Some(smtc_rx);

        let now = Instant::now();
        let default_pal = AlbumPalette::default_dark();
        let db = Arc::new(Mutex::new(db_raw));

        let mut app = Self {
            config,
            config_path,
            db_path,
            db,
            art_cache,
            audio,
            main_window: None,
            window_visible: true,
            minimized: false,
            maximized: false,
            view_mode: ViewMode::Browse,
            nav: ActiveNav::LocalFiles,
            queue_open: false,
            search_query: String::new(),
            source_filter: SourceFilter::All,
            playlists,
            local_tracks,
            liked_tracks,
            playlist_tracks: Vec::new(),
            search_results: Vec::new(),
            queue,
            status: PlaybackStatus::Stopped,
            source_used: None,
            position_ms: 0,
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
            _tray: tray,
        };

        let mut startup_tasks = vec![window::oldest().map(|id| {
            id.map(Message::WindowOpened)
                .unwrap_or(Message::DismissToast)
        })];

        if let Some(current) = app.queue.current().cloned() {
            app.duration_ms = current.duration_ms;
            startup_tasks.push(app.load_palette_task(&current));
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
        let acrylic_browse = self.config.glassmorphism_mode == GlassmorphismMode::WindowsAcrylic
            && self.view_mode == ViewMode::Browse;
        iced::theme::Style {
            background_color: if acrylic_browse {
                Color::TRANSPARENT
            } else {
                Color::from_rgb8(14, 14, 20)
            },
            text_color: Color::WHITE,
        }
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let mut subs = vec![
            window::events().map(|(id, ev)| Message::WindowEvent(id, ev)),
            window::close_requests().map(|_| Message::CloseRequested),
            Subscription::run(audio_subscription),
            Subscription::run(tray_subscription),
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
                    apply_window_glass_mode(id, self.config.glassmorphism_mode, self.view_mode),
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
                window::Event::Opened { .. } => {
                    self.main_window = Some(id);
                    self.window_visible = true;
                    self.minimized = false;
                    return Task::batch([
                        apply_window_glass_mode(id, self.config.glassmorphism_mode, self.view_mode),
                        self.init_smtc_task(id),
                    ]);
                }
                window::Event::Resized(size) => {
                    if size.width > 0.0 && size.height > 0.0 {
                        self.minimized = false;
                    }
                }
                window::Event::Focused => {
                    self.window_visible = true;
                    self.minimized = false;
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
                if self.config.close_to_tray {
                    self.window_visible = false;
                    if let Some(id) = self.main_window {
                        return window::set_mode(id, window::Mode::Hidden);
                    }
                } else {
                    return self.shutdown_and_exit();
                }
            }
            Message::FrameTick(now) => {
                let dt = (now - self.last_frame_at).as_secs_f32().clamp(0.0, 0.05);
                self.last_frame_at = now;
                self.time_s = (now - self.anim_start).as_secs_f32();

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
                        if let Some(idx) = active_line_index(&lyrics.synced, self.position_ms) {
                            let target_y = (idx as f32) * LYRIC_LINE_STRIDE_PX;
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
                } else if matches!(self.nav, ActiveNav::LikedSongs) {
                    if let Ok(db) = self.db.lock() {
                        self.liked_tracks = db.list_unified_liked_tracks().unwrap_or_default();
                    }
                } else if matches!(self.nav, ActiveNav::Search) {
                    self.refresh_search_results();
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
                        self.view_mode,
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
                let url = self.import_url_input.trim();
                if url.contains("open.spotify.com/playlist/") {
                    self.modal_error = Some(
                        "Spotify URL Importer is ready to connect in Phase 3.".to_string(),
                    );
                } else {
                    self.modal_error =
                        Some("Enter a valid https://open.spotify.com/playlist/... URL.".to_string());
                }
            }
            Message::AddTrackToPlaylist {
                playlist_id,
                track_uid,
            } => {
                self.open_track_menu_uid = None;
                if let Ok(db) = self.db.lock() {
                    match db.add_track_to_playlist(&playlist_id, &track_uid) {
                        Ok(()) => {
                            self.playlists = db.list_playlists().unwrap_or_default();
                            if matches!(&self.nav, ActiveNav::Playlist(cur) if cur == &playlist_id) {
                                self.playlist_tracks =
                                    db.get_playlist_tracks(&playlist_id).unwrap_or_default();
                            }
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
            }
            Message::SourceFilterChanged(filter) => {
                self.source_filter = filter;
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
                    let _ = db.set_local_favorite(&track.uid, next_liked);
                    self.local_tracks = db.list_local_tracks().unwrap_or_default();
                    self.liked_tracks = db.list_unified_liked_tracks().unwrap_or_default();
                    if let ActiveNav::Playlist(pl_id) = &self.nav {
                        self.playlist_tracks = db.get_playlist_tracks(pl_id).unwrap_or_default();
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
            Message::PaletteLoaded {
                track_uid,
                palette,
                art_bytes,
                art_blur_rgba,
            } => {
                if let Some(bytes) = art_bytes {
                    self.insert_art_handle(track_uid.clone(), image::Handle::from_bytes(bytes));
                }
                if self.queue.current().map(|t| t.uid.as_str()) == Some(track_uid.as_str()) {
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
                for (key, bytes) in items {
                    self.insert_art_handle(key, image::Handle::from_bytes(bytes));
                }
            }
            Message::SetGlassmorphismMode(mode) => {
                self.config.glassmorphism_mode = mode;
                let _ = self.config.save(&self.config_path);
                if let Some(id) = self.main_window {
                    return apply_window_glass_mode(id, mode, self.view_mode);
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
            Message::SetCloseToTray(enabled) => {
                self.config.close_to_tray = enabled;
                let _ = self.config.save(&self.config_path);
            }
            Message::SetPreferYoutubeFallback(enabled) => {
                self.config.prefer_youtube_fallback = enabled;
                let _ = self.config.save(&self.config_path);
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
                    self.status = st;
                    self.smtc.set_playback(st, self.position_ms);
                }
                AudioEvent::PositionTick {
                    position_ms,
                    duration_ms,
                } => {
                    self.position_ms = position_ms;
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
                AudioEvent::Error(err) => {
                    self.status = PlaybackStatus::Stopped;
                    self.status_toast = Some(err);
                }
            },
            Message::Tray(action) => match action {
                TrayAction::ToggleWindow => {
                    if let Some(id) = self.main_window {
                        if self.window_visible && !self.minimized {
                            self.window_visible = false;
                            return window::set_mode(id, window::Mode::Hidden);
                        } else {
                            self.window_visible = true;
                            self.minimized = false;
                            return Task::batch([
                                window::set_mode(id, window::Mode::Windowed),
                                window::minimize(id, false),
                                window::gain_focus(id),
                            ]);
                        }
                    }
                }
                TrayAction::PlayPause => return self.handle_play_pause_toggle(),
                TrayAction::Next => return self.update(Message::NextTrack),
                TrayAction::Previous => return self.update(Message::PreviousTrack),
                TrayAction::Quit => return self.shutdown_and_exit(),
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
        let acrylic_browse = self.config.glassmorphism_mode == GlassmorphismMode::WindowsAcrylic
            && self.view_mode == ViewMode::Browse;

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
                    art_handles: &self.art_handles,
                    time_s: self.time_s,
                    maximized: self.maximized,
                });

                if self.queue_open {
                    let active_list = self.queue.active_list();
                    stack![
                        lyrics_view,
                        container(view_queue_drawer(&active_list, self.queue.current_pos()))
                            .width(Length::Fill)
                            .height(Length::Fill)
                            .align_x(iced::alignment::Horizontal::Right)
                            .padding([42, 14])
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
                    "Lume Music",
                    self.maximized,
                    self.source_used.map(|s| s.badge_label()),
                );

                let center_content: Element<'_, Message> = if matches!(self.nav, ActiveNav::Settings)
                {
                    row![
                        view_sidebar(&self.nav, &self.playlists),
                        view_settings(
                            &self.config,
                            self.scanning_folders,
                            self.scan_summary.as_deref()
                        ),
                    ]
                    .spacing(16)
                    .padding([2, 16])
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .into()
                } else {
                    let visible_tracks = self.visible_tracks();
                    view_browse(BrowseViewProps {
                        nav: &self.nav,
                        search_query: &self.search_query,
                        source_filter: self.source_filter,
                        playlists: &self.playlists,
                        visible_tracks,
                        active_track_uid: self.queue.current().map(|t| t.uid.as_str()),
                        open_track_menu_uid: self.open_track_menu_uid.as_deref(),
                        renaming_playlist: self.renaming_playlist.as_ref(),
                        art_handles: &self.art_handles,
                        show_new_playlist_modal: self.show_new_playlist_modal,
                        new_playlist_input: &self.new_playlist_input,
                        show_import_url_modal: self.show_import_url_modal,
                        import_url_input: &self.import_url_input,
                        modal_error: self.modal_error.as_deref(),
                        status_banner: self.status_toast.as_deref(),
                    })
                };

                let player_bar = view_player_bar(info, &self.art_handles, self.queue_open);

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
                        container(view_queue_drawer(&active_list, self.queue.current_pos()))
                            .width(Length::Fill)
                            .height(Length::Fill)
                            .align_x(iced::alignment::Horizontal::Right)
                            .padding([42, 14])
                    ]
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .into()
                } else {
                    main_col.into()
                }
            }
        };

        let root_bg = if acrylic_browse {
            Color::TRANSPARENT
        } else {
            Color::from_rgb8(10, 10, 14)
        };

        let root_content: Element<'_, Message> = if acrylic_browse {
            foreground
        } else {
            let backdrop = shader(DynamicBackdropProgram::new(
                self.display_palette,
                if self.config.reduced_motion {
                    0.0
                } else {
                    self.time_s
                },
                None,
                self.art_blur_rgba.clone(),
                self.art_blur_version,
            ))
            .width(Length::Fill)
            .height(Length::Fill);

            stack![backdrop, foreground]
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
        };

        container(root_content)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_| {
                let border = if acrylic_browse {
                    iced::border::Border {
                        color: Color::from_rgba(1.0, 1.0, 1.0, 0.16),
                        width: 1.0,
                        radius: 12.0.into(),
                    }
                } else {
                    iced::border::Border::default()
                };
                container::Style {
                    background: Some(Background::Color(root_bg)),
                    border,
                    ..Default::default()
                }
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

    fn refresh_search_results(&mut self) {
        if let Ok(db) = self.db.lock() {
            self.search_results = db
                .search_cached_tracks(&self.search_query, 200)
                .unwrap_or_default();
        }
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
            PlaybackStatus::Stopped | PlaybackStatus::Buffering => {
                if let Some(track) = self.queue.current().cloned() {
                    return self.start_playing_track(track);
                }
            }
        }
        Task::none()
    }

    fn start_playing_track(&mut self, track: Track) -> Task<Message> {
        self.position_ms = 0;
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
        if self.view_mode == ViewMode::NowPlayingLyrics {
            tasks.push(self.fetch_lyrics_task(track.clone(), false));
        } else {
            self.current_lyrics = None;
            self.current_lyrics_uid = None;
        }

        match &track.source {
            TrackSource::Local { path } => {
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
                self.source_used = Some(PlaybackSourceUsed::YouTubeDirect);
                self.status_toast = Some("YouTube Music playback arrives in Phase 2.".to_string());
            }
            TrackSource::Spotify { .. } => {
                self.source_used = Some(PlaybackSourceUsed::SpotifyNative);
                self.status_toast =
                    Some("Spotify playback & auto-fallback arrive in Phase 3.".to_string());
            }
        }

        Task::batch(tasks)
    }

    fn stop_playback(&mut self) {
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
        self._tray.take();
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
                tokio::task::spawn_blocking(move || {
                    let bytes = art_key.and_then(|key| {
                        art_cache
                            .lock()
                            .ok()
                            .and_then(|mut cache| cache.get(&key).ok().flatten())
                    });
                    let palette = bytes
                        .as_deref()
                        .map(extract_palette)
                        .unwrap_or_else(AlbumPalette::default_dark);
                    let blur_rgba = bytes.as_deref().and_then(decode_art_blur_rgba).map(Arc::new);
                    (track_uid, palette, bytes, blur_rgba)
                })
                .await
                .unwrap_or_else(|_| (String::new(), AlbumPalette::default_dark(), None, None))
            },
            |(track_uid, palette, art_bytes, art_blur_rgba)| Message::PaletteLoaded {
                track_uid,
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
        let art_cache = Arc::clone(&self.art_cache);
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    let mut out = Vec::new();
                    if let Ok(mut cache) = art_cache.lock() {
                        for key in keys {
                            if let Ok(Some(bytes)) = cache.get(&key) {
                                out.push((key, bytes));
                            }
                        }
                    }
                    out
                })
                .await
                .unwrap_or_default()
            },
            Message::ThumbnailsLoaded,
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
}
