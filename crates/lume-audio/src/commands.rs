use std::path::PathBuf;

use lume_core::PlaybackStatus;

/// Commands sent from UI/app state to the dedicated audio worker thread.
#[derive(Debug, Clone, PartialEq)]
pub enum AudioCommand {
    LoadLocal {
        path: PathBuf,
        track_uid: String,
        duration_ms: u64,
    },
    LoadHttpStream {
        url: String,
        track_uid: String,
        duration_ms: u64,
        mime_hint: Option<String>,
    },
    LoadYtDlpStream {
        binary: PathBuf,
        video_url: String,
        format_selector: String,
        track_uid: String,
        duration_ms: u64,
        mime_hint: Option<String>,
    },
    LoadSpotify {
        spotify_id: String,
        track_uid: String,
        duration_ms: u64,
        access_token: String,
    },
    DisconnectSpotify,
    Pause,
    Resume,
    Seek(u64),
    SetVolume(f32),
    SetMute(bool),
    Stop,
    Shutdown,
}

/// Asynchronous events emitted by the audio worker thread.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AudioEvent {
    StateChanged(PlaybackStatus),
    PositionTick {
        position_ms: u64,
        duration_ms: u64,
    },
    TrackEnded {
        track_uid: String,
    },
    SpotifyFallbackNeeded {
        track_uid: String,
        reason: String,
    },
    Error(String),
}
