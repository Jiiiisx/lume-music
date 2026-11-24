pub mod browse;
pub mod lyrics_view;
pub mod player_bar;
pub mod queue_drawer;
pub mod settings;
pub mod titlebar;

use std::path::PathBuf;
use std::sync::Arc;

use lume_audio::AudioEvent;
use lume_core::{
    LyricsData, PlaybackSourceUsed, PlaybackStatus, RepeatMode, ResolvedStream, Track,
};
use lume_providers::ScanReport;
use lume_storage::{GlassmorphismMode, LyricsVisualMode};

use crate::theme::AlbumPalette;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SourceFilter {
    #[default]
    All,
    Spotify,
    YouTubeMusic,
    Local,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ActiveNav {
    Search,
    LikedSongs,
    #[default]
    LocalFiles,
    Playlist(String),
    Settings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewMode {
    #[default]
    Browse,
    NowPlayingLyrics,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmtcAction {
    Play,
    Pause,
    Toggle,
    Next,
    Previous,
    Stop,
}

#[derive(Debug, Clone)]
pub enum Message {
    // Window chrome & lifecycle
    WindowOpened(iced::window::Id),
    WindowEvent(iced::window::Id, iced::window::Event),
    DragWindow,
    MinimizeWindow,
    ToggleMaximizeWindow,
    CloseRequested,
    FrameTick(std::time::Instant),

    // Navigation & modals
    SelectNav(ActiveNav),
    SetViewMode(ViewMode),
    ToggleQueueDrawer,
    OpenNewPlaylistModal,
    OpenImportUrlModal,
    CloseModal,

    // Search & filters
    SearchInputChanged(String),
    SearchSubmitted,
    SourceFilterChanged(SourceFilter),
    YtmSearchFinished {
        request_id: u64,
        result: Result<Vec<Track>, String>,
    },

    // Playlists & track actions
    NewPlaylistNameChanged(String),
    CreatePlaylistSubmitted,
    StartRenamePlaylist(String, String),
    RenamePlaylistInputChanged(String),
    SubmitRenamePlaylist,
    CancelRenamePlaylist,
    DeletePlaylist(String),
    ImportUrlInputChanged(String),
    ImportSpotifyUrlSubmitted,
    AddTrackToPlaylist {
        playlist_id: String,
        track: Track,
    },
    RemoveTrackFromPlaylist {
        playlist_id: String,
        position: usize,
    },
    ReorderPlaylistTrack {
        playlist_id: String,
        from: usize,
        to: usize,
    },
    ToggleTrackMenu(String),
    CloseTrackMenu,

    // Playback & queue controls
    PlayTrackList {
        tracks: Vec<Track>,
        start_index: usize,
    },
    PlayNext(Track),
    AddToQueue(Track),
    ToggleLike(Track),
    TogglePlayPause,
    NextTrack,
    PreviousTrack,
    ToggleShuffle,
    CycleRepeat,
    SeekSliderChanged(u64),
    SeekTo(u64),
    SetVolume(f32),
    ToggleMute,
    QueueJumpTo(usize),
    QueueMove {
        from: usize,
        to: usize,
    },
    QueueRemoveAt(usize),
    ClearQueue,

    // Lyrics, streaming & artwork async completions
    RetryLyrics,
    LyricsLoaded {
        track_uid: String,
        result: Result<Option<LyricsData>, String>,
    },
    StreamResolved {
        request_id: u64,
        track: Track,
        result: Result<ResolvedStream, String>,
    },
    PaletteLoaded {
        track_uid: String,
        art_key: Option<String>,
        palette: AlbumPalette,
        art_bytes: Option<Vec<u8>>,
        art_blur_rgba: Option<Arc<Vec<u8>>>,
    },
    ThumbnailsLoaded(Vec<(String, Vec<u8>)>),

    // Settings & local library
    SetGlassmorphismMode(GlassmorphismMode),
    SetLyricsVisualMode(LyricsVisualMode),
    SetReducedMotion(bool),
    SetPreferYoutubeFallback(bool),
    PickYtDlpBinary,
    YtDlpPicked(Option<PathBuf>),
    ClearYtDlpBinary,
    PickLocalFolder,
    FolderPicked(Option<PathBuf>),
    RemoveLocalFolder(PathBuf),
    RescanLocalFolders,
    LocalScanFinished(Result<(ScanReport, Vec<Track>), String>),

    // Background platform & audio events
    Audio(AudioEvent),
    Smtc(SmtcAction),
    SmtcHwndReady(Option<usize>),
    DismissToast,
}

#[derive(Debug, Clone)]
pub struct NowPlayingInfo<'a> {
    pub track: Option<&'a Track>,
    pub status: PlaybackStatus,
    pub source_used: Option<PlaybackSourceUsed>,
    pub position_ms: u64,
    pub duration_ms: u64,
    pub shuffle: bool,
    pub repeat: RepeatMode,
    pub volume: f32,
    pub muted: bool,
}
