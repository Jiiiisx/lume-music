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
    LyricsData, PlaybackSourceUsed, PlaybackStatus, Playlist, RepeatMode, ResolvedStream, Track,
};
use lume_providers::{ScanReport, SpotifyAccountTier, SpotifyUserProfile};
use lume_storage::{GlassmorphismMode, LyricsVisualMode, SpotifyTokenSet};

use crate::theme::AlbumPalette;

#[derive(Debug, Clone)]
pub struct SpotifySyncReport {
    pub profile: Option<SpotifyUserProfile>,
    pub synced_playlists: usize,
    pub synced_liked: usize,
    pub flushed_mutations: usize,
    pub lapsed: bool,
    pub refreshed_token: Option<SpotifyTokenSet>,
}

#[derive(Debug, Clone)]
pub enum SpotifySearchError {
    LapsedOrForbidden,
    Other(String),
}

#[derive(Debug, Clone)]
pub struct SpotifySettingsProps<'a> {
    pub client_id_input: &'a str,
    pub web_api_connected: bool,
    pub playback_connected: bool,
    pub profile_name: Option<&'a str>,
    pub account_tier: SpotifyAccountTier,
    pub lapsed_fallback: bool,
    pub syncing: bool,
    pub logging_in_web: bool,
    pub logging_in_playback: bool,
    pub pending_mutations: usize,
    pub sync_summary: Option<&'a str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SourceFilter {
    #[default]
    All,
    Spotify,
    YouTubeMusic,
    Local,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlaylistSortField {
    #[default]
    Custom,
    RecentlyAdded,
    Title,
    Artist,
    Album,
    Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortDirection {
    #[default]
    Asc,
    Desc,
}

impl SortDirection {
    pub fn flip(self) -> Self {
        match self {
            Self::Asc => Self::Desc,
            Self::Desc => Self::Asc,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PlaylistSort {
    pub field: PlaylistSortField,
    pub direction: SortDirection,
}

impl PlaylistSort {
    pub fn select(&mut self, field: PlaylistSortField) {
        if field == PlaylistSortField::Custom {
            self.field = PlaylistSortField::Custom;
            self.direction = SortDirection::Asc;
            return;
        }
        if self.field == field {
            self.direction = self.direction.flip();
        } else {
            self.field = field;
            self.direction = match field {
                PlaylistSortField::RecentlyAdded => SortDirection::Desc,
                _ => SortDirection::Asc,
            };
        }
    }

    pub fn toggle_direction(&mut self) {
        self.direction = self.direction.flip();
    }
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
    SpotifySearchFinished {
        request_id: u64,
        result: Result<Vec<Track>, SpotifySearchError>,
        refreshed_token: Option<SpotifyTokenSet>,
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
    PickExportifyCsv,
    ExportifyCsvPicked(Option<PathBuf>),
    SpotifyImportFinished(Result<(Playlist, bool), String>),
    RematchYoutubeFallback(Track),
    RematchFinished {
        track: Track,
        result: Result<String, String>,
    },
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
    SetPlaylistSort(PlaylistSortField),
    TogglePlaylistSortDirection,
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
    ThumbnailsLoaded(Vec<(String, u32, u32, Vec<u8>)>),
    SpotifyArtworkEnriched(Vec<(String, String)>),

    // Settings, Spotify auth & local library
    SetGlassmorphismMode(GlassmorphismMode),
    SetLyricsVisualMode(LyricsVisualMode),
    SetReducedMotion(bool),
    SetPreferYoutubeFallback(bool),
    SpotifyClientIdChanged(String),
    SaveSpotifyClientId,
    ConnectSpotifyWebApi,
    SpotifyWebApiLoginFinished(Result<(SpotifyTokenSet, SpotifyUserProfile), String>),
    DisconnectSpotifyWebApi,
    ConnectSpotifyPlayback,
    SpotifyPlaybackLoginFinished(Result<SpotifyTokenSet, String>),
    DisconnectSpotifyPlayback,
    SyncSpotifyLibrary,
    SpotifySyncFinished(Result<SpotifySyncReport, String>),
    SpotifyPlaybackTokenReady {
        request_id: u64,
        track: Track,
        result: Result<(SpotifyTokenSet, bool), String>,
    },
    SpotifyLikeSynced {
        spotify_track_id: String,
        liked: bool,
        lapsed: bool,
        failed: bool,
        refreshed_token: Option<SpotifyTokenSet>,
    },
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
