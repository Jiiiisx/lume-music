pub mod local;
pub mod lyrics;

pub use local::{find_sidecar_lrc, read_embedded_lyrics, LocalProvider, ScanReport};
pub use lyrics::{parse_lrc, ChainedLyricsProvider, LrclibClient};
