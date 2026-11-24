pub mod local;
pub mod lyrics;
pub mod ytm;

pub use local::{find_sidecar_lrc, read_embedded_lyrics, LocalProvider, ScanReport};
pub use lyrics::{parse_lrc, ChainedLyricsProvider, LrclibClient};
pub use ytm::{
    detect_yt_dlp_binary, map_track_item, parse_yt_dlp_stdout, select_best_audio_stream,
    upgrade_ytm_cover_url, AudioStreamCandidate, YouTubeMusicProvider, YT_DLP_FORMAT_SELECTOR,
};
