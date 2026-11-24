use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Origin source for a playable track.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TrackSource {
    Spotify { id: String },
    YouTubeMusic { video_id: String },
    Local { path: PathBuf },
}

impl TrackSource {
    /// Returns a deterministic canonical identifier for deduplication and persistence.
    pub fn canonical_uid(&self) -> String {
        match self {
            Self::Spotify { id } => format!("spotify:{id}"),
            Self::YouTubeMusic { video_id } => format!("ytm:{video_id}"),
            Self::Local { path } => {
                let normalized = normalize_local_path(path);
                let hash = fnv1a_64(normalized.as_bytes());
                format!("local:{hash:016x}")
            }
        }
    }
}

fn normalize_local_path(path: &std::path::Path) -> String {
    path.to_string_lossy()
        .replace('\\', "/")
        .to_lowercase()
}

fn fnv1a_64(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for &b in bytes {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

/// Core track metadata unified across Spotify, YouTube Music, and Local files.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Track {
    pub uid: String,
    pub source: TrackSource,
    pub title: String,
    pub artists: Vec<String>,
    pub album: Option<String>,
    pub duration_ms: u64,
    pub artwork_ref: Option<String>,
    pub is_liked: bool,
}

impl Track {
    /// Returns a display-ready comma-separated artist string.
    pub fn artist_display(&self) -> String {
        if self.artists.is_empty() {
            "Unknown Artist".to_string()
        } else {
            self.artists.join(", ")
        }
    }
}

/// Distinguishes read-only synced Spotify mirrors from locally editable playlists.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlaylistOrigin {
    SpotifySynced,
    SpotifyImported { source_url: String },
    LocalCustom,
}

impl PlaylistOrigin {
    /// Returns true if the user can rename, delete, or modify tracks locally.
    pub fn is_editable(&self) -> bool {
        match self {
            Self::SpotifySynced => false,
            Self::SpotifyImported { .. } | Self::LocalCustom => true,
        }
    }
}

/// A playlist entry in the user's library.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Playlist {
    pub id: String,
    pub name: String,
    pub origin: PlaylistOrigin,
    pub track_count: usize,
}

/// Word- or syllable-level timing span for enhanced karaoke-style synced lyrics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LyricsWord {
    pub start_ms: u64,
    pub end_ms: u64,
    pub text: String,
    /// True when this syllable connects directly to the next syllable without an intervening space
    /// (e.g. `"da"` + `"tang"` -> `"datang"`), mirroring Spicy Lyrics' `IsPartOfWord`.
    #[serde(default)]
    pub is_part_of_word: bool,
    /// True when this syllable belongs to a background vocal span.
    #[serde(default)]
    pub is_background: bool,
}

/// A single line of synced lyrics with optional word/syllable-level timing spans.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LyricsLine {
    pub time_ms: u64,
    /// Optional explicit line end timestamp (ms) from TTML / LyricsPlus / Spicy-style payloads.
    #[serde(default)]
    pub end_ms: Option<u64>,
    pub text: String,
    pub words: Option<Vec<LyricsWord>>,
    /// True for secondary duet singer lines (`OppositeAligned` in Spicy Lyrics).
    #[serde(default)]
    pub opposite_aligned: bool,
    /// True for dedicated background vocal lines (`bg-line` in Spicy Lyrics).
    #[serde(default)]
    pub is_background: bool,
}

/// Complete lyrics payload for a track.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LyricsData {
    pub synced: Vec<LyricsLine>,
    pub plain: Option<String>,
    pub provider: String,
}

impl LyricsData {
    /// Returns true if at least one synced line has native word/syllable timing spans.
    pub fn has_native_syllables(&self) -> bool {
        self.synced
            .iter()
            .any(|l| l.words.as_ref().is_some_and(|w| !w.is_empty()))
    }
}

/// Concrete backend used to stream the currently playing track.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlaybackSourceUsed {
    SpotifyNative,
    YouTubeFallback,
    YouTubeDirect,
    LocalFile,
}

impl PlaybackSourceUsed {
    pub fn badge_label(&self) -> &'static str {
        match self {
            Self::SpotifyNative => "Spotify",
            Self::YouTubeFallback => "YouTube Fallback",
            Self::YouTubeDirect => "YouTube",
            Self::LocalFile => "Local",
        }
    }
}

/// Repeat playback behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum RepeatMode {
    #[default]
    Off,
    All,
    One,
}

impl RepeatMode {
    pub fn next(self) -> Self {
        match self {
            Self::Off => Self::All,
            Self::All => Self::One,
            Self::One => Self::Off,
        }
    }
}

/// Current state of the audio playback engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum PlaybackStatus {
    #[default]
    Stopped,
    Buffering,
    Playing,
    Paused,
}

/// Resolved playback target returned by a `MusicProvider`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResolvedStream {
    LocalFile(PathBuf),
    HttpUrl {
        url: String,
        source: PlaybackSourceUsed,
    },
    YtDlpProcess {
        binary: PathBuf,
        watch_url: String,
        format_selector: String,
        source: PlaybackSourceUsed,
    },
    SpotifyTrack {
        id: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn canonical_uid_is_deterministic_per_source() {
        assert_eq!(
            TrackSource::Spotify { id: "abc".into() }.canonical_uid(),
            "spotify:abc"
        );
        assert_eq!(
            TrackSource::YouTubeMusic {
                video_id: "xyz".into()
            }
            .canonical_uid(),
            "ytm:xyz"
        );
        let local_a = TrackSource::Local {
            path: PathBuf::from(r"D:\Music\Song.flac"),
        }
        .canonical_uid();
        let local_b = TrackSource::Local {
            path: PathBuf::from(r"d:\music\song.flac"),
        }
        .canonical_uid();
        assert!(local_a.starts_with("local:"));
        assert_eq!(local_a, local_b);
    }

    #[test]
    fn playlist_origin_mutability_matches_spec() {
        assert!(!PlaylistOrigin::SpotifySynced.is_editable());
        assert!(PlaylistOrigin::SpotifyImported {
            source_url: "https://open.spotify.com/playlist/1".into()
        }
        .is_editable());
        assert!(PlaylistOrigin::LocalCustom.is_editable());
    }
}
