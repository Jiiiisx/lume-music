pub mod error;
pub mod models;
pub mod traits;

pub use error::CoreError;
pub use models::*;
pub use traits::{LyricsProvider, MusicProvider};
