use async_trait::async_trait;

use crate::error::CoreError;
use crate::models::{LyricsData, Playlist, ResolvedStream, Track};

/// Abstraction implemented by music catalog and stream providers.
#[async_trait]
pub trait MusicProvider: Send + Sync {
    async fn search(&self, query: &str, limit: usize) -> Result<Vec<Track>, CoreError>;
    async fn get_playlists(&self) -> Result<Vec<Playlist>, CoreError>;
    async fn get_playlist_tracks(&self, playlist_id: &str) -> Result<Vec<Track>, CoreError>;
    async fn resolve_stream(&self, track: &Track) -> Result<ResolvedStream, CoreError>;
}

/// Abstraction for lyrics retrieval.
#[async_trait]
pub trait LyricsProvider: Send + Sync {
    async fn fetch_lyrics(&self, track: &Track) -> Result<Option<LyricsData>, CoreError>;
}
