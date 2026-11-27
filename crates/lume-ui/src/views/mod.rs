pub mod browse;
pub mod catalog;
pub mod lyrics_view;
pub mod player_bar;
pub mod queue_drawer;
pub mod settings;
pub mod titlebar;

use std::path::PathBuf;
use std::sync::Arc;

use lume_audio::AudioEvent;
use lume_core::{
    AlbumDetail, AlbumSummary, ArtistProfile, ArtistSummary, CatalogSource, LyricsData,
    PlaybackSourceUsed, PlaybackStatus, Playlist, RemotePlaylistDetail, RemotePlaylistSummary,
    RepeatMode, ResolvedStream, Track, TrackSource,
};
use lume_providers::{
    ScanReport, SpotifyAccountTier, SpotifyCatalogSearchResult, SpotifyUserProfile,
    YtmCatalogSearchResult,
};
use lume_storage::{GlassmorphismMode, LyricsVisualMode, SpotifyTokenSet};
pub use lume_storage::{PlaylistSort, PlaylistSortField, SortDirection};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ChartRegion {
    #[default]
    Indonesia,
    Global,
}

impl ChartRegion {
    pub fn label(self) -> &'static str {
        match self {
            Self::Indonesia => "Indonesia",
            Self::Global => "Global",
        }
    }

    pub fn is_indonesia(self) -> bool {
        matches!(self, Self::Indonesia)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SearchTab {
    #[default]
    All,
    Songs,
    Artists,
    Playlists,
}

impl SearchTab {
    pub fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Songs => "Songs",
            Self::Artists => "Artists",
            Self::Playlists => "Playlists",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ActiveNav {
    #[default]
    Home,
    Search,
    LikedSongs,
    LocalFiles,
    Playlist(String),
    ArtistProfile {
        source: CatalogSource,
        id: String,
        name: String,
    },
    AlbumDetail {
        source: CatalogSource,
        id: String,
        title: String,
    },
    RemotePlaylist {
        source: CatalogSource,
        id: String,
        name: String,
    },
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
    NavigateBack,
    SelectHomeRegion(ChartRegion),
    RefreshHomeCharts,
    ToggleExpandSpotifyTrending,
    ToggleExpandYtmTrending,
    HomeSpotifyChartsLoaded {
        region: ChartRegion,
        result: Result<Vec<Track>, String>,
    },
    HomeYtmChartsLoaded {
        region: ChartRegion,
        result: Result<Vec<Track>, String>,
    },
    SetViewMode(ViewMode),
    ToggleQueueDrawer,
    OpenNewPlaylistModal,
    OpenImportUrlModal,
    CloseModal,

    // Search & filters
    SearchInputChanged(String),
    SearchDebounceFired {
        seq: u64,
        query: String,
    },
    SearchSubmitted,
    SearchTabChanged(SearchTab),
    SourceFilterChanged(SourceFilter),
    YtmSearchFinished {
        request_id: u64,
        result: Result<YtmCatalogSearchResult, String>,
    },
    SpotifySearchFinished {
        request_id: u64,
        result: Result<SpotifyCatalogSearchResult, SpotifySearchError>,
        refreshed_token: Option<SpotifyTokenSet>,
    },

    // Catalog detail views (Artist Profile, Album Detail, Remote Playlist Preview)
    OpenArtist(ArtistSummary),
    OpenArtistByName {
        name: String,
        source_hint: TrackSource,
    },
    ToggleExpandArtistTracks,
    ArtistProfileLoaded {
        request_id: u64,
        result: Result<ArtistProfile, String>,
        refreshed_token: Option<SpotifyTokenSet>,
        fallback_notice: Option<String>,
        spotify_lapsed: bool,
    },
    OpenAlbum(AlbumSummary),
    AlbumDetailLoaded {
        request_id: u64,
        result: Result<AlbumDetail, String>,
        refreshed_token: Option<SpotifyTokenSet>,
        spotify_lapsed: bool,
    },
    OpenRemotePlaylist(RemotePlaylistSummary),
    RemotePlaylistLoaded {
        request_id: u64,
        result: Result<RemotePlaylistDetail, String>,
        refreshed_token: Option<SpotifyTokenSet>,
        spotify_lapsed: bool,
    },
    SaveRemotePlaylistToLibrary,
    RemotePlaylistSaved(Result<(Playlist, bool), String>),

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
    TogglePlayerMenu,
    ClosePlayerMenu,

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
        video_id: Option<String>,
        result: Result<ResolvedStream, String>,
    },
    PaletteLoaded {
        track_uid: String,
        art_key: Option<String>,
        palette: AlbumPalette,
        cover_rgba: Option<(u32, u32, Vec<u8>)>,
        thumb_rgba: Option<(u32, u32, Vec<u8>)>,
        art_blur_rgba: Option<Arc<Vec<u8>>>,
    },
    ThumbnailsLoaded(Vec<(String, u32, u32, Vec<u8>)>),
    SpotifyArtworkEnriched {
        attempted_uids: Vec<String>,
        resolved: Vec<(String, String)>,
    },

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
    ExpireToast(u64),
    BackgroundTaskDone,
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

/// Computes sidebar width (and matching titlebar left brand width) for the current window size.
pub fn responsive_sidebar_width(window_width: f32) -> f32 {
    if window_width < 1020.0 {
        192.0
    } else {
        224.0
    }
}

/// Whether track tables have enough width to display the Album column cleanly alongside Title/Source/Time.
pub fn show_track_album_column(window_width: f32) -> bool {
    window_width >= 960.0
}

/// Whether track tables have enough width to display the Source badge column.
pub fn show_track_source_column(window_width: f32) -> bool {
    window_width >= 760.0
}

/// Computes responsive card-grid column count for Artists, Playlists, Albums, and Singles.
pub fn responsive_grid_columns(
    window_width: f32,
    sidebar_width: f32,
    card_width: f32,
    spacing: f32,
) -> usize {
    let available = (window_width - sidebar_width - 116.0).max(card_width);
    let cols = ((available + spacing) / (card_width + spacing)).floor() as usize;
    cols.clamp(2, 9)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn responsive_helpers_scale_cleanly_across_breakpoints() {
        assert_eq!(responsive_sidebar_width(820.0), 192.0);
        assert_eq!(responsive_sidebar_width(1280.0), 224.0);

        assert!(!show_track_album_column(860.0));
        assert!(show_track_album_column(1024.0));
        assert!(!show_track_source_column(700.0));
        assert!(show_track_source_column(820.0));

        assert_eq!(responsive_grid_columns(820.0, 192.0, 136.0, 10.0), 3);
        assert_eq!(responsive_grid_columns(1180.0, 224.0, 136.0, 10.0), 5);
        assert!(responsive_grid_columns(1920.0, 224.0, 136.0, 10.0) >= 8);
    }
}
