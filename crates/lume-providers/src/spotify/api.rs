use std::time::Duration;

use lume_core::{Track, TrackSource};
use lume_storage::Database;
use reqwest::StatusCode;
use serde::Deserialize;
use thiserror::Error;

pub const SPOTIFY_API_BASE_URL: &str = "https://api.spotify.com/v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpotifyAccountTier {
    Premium,
    Free,
    Unknown,
}

impl SpotifyAccountTier {
    pub fn from_product(product: Option<&str>) -> Self {
        match product.map(|s| s.trim().to_ascii_lowercase()).as_deref() {
            Some("premium") => Self::Premium,
            Some("free") | Some("open") => Self::Free,
            _ => Self::Unknown,
        }
    }

    pub fn is_premium(self) -> bool {
        matches!(self, Self::Premium)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpotifyUserProfile {
    pub id: String,
    pub display_name: String,
    pub product: SpotifyAccountTier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpotifyPlaylistSummary {
    pub id: String,
    pub name: String,
    pub track_count: usize,
}

#[derive(Debug, Error)]
pub enum SpotifyApiError {
    #[error("Spotify session expired or unauthorized (401)")]
    Unauthorized,

    #[error("Spotify Web API restricted (403 — Premium required or Dev Mode lapsed): {0}")]
    PremiumRequiredOrForbidden(String),

    #[error("Spotify rate limited (429)")]
    RateLimited { retry_after_s: Option<u64> },

    #[error("Spotify HTTP error: {0}")]
    Http(String),

    #[error("Spotify JSON parse error: {0}")]
    Parse(String),
}

impl SpotifyApiError {
    pub fn triggers_fallback(&self) -> bool {
        matches!(
            self,
            Self::PremiumRequiredOrForbidden(_) | Self::Unauthorized
        )
    }
}

/// Spotify Web API HTTP client supporting profile tier checks, playlists, Liked Songs,
/// track search, and Feb 2026 unified `/me/library` mutations.
#[derive(Debug, Clone)]
pub struct SpotifyWebClient {
    http: reqwest::Client,
    base_url: String,
}

impl Default for SpotifyWebClient {
    fn default() -> Self {
        Self::new()
    }
}

impl SpotifyWebClient {
    pub fn new() -> Self {
        Self::with_base_url(SPOTIFY_API_BASE_URL)
    }

    pub fn with_base_url(base_url: impl Into<String>) -> Self {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .build()
            .expect("failed to construct Spotify HTTP client");
        Self {
            http,
            base_url: base_url.into().trim_end_matches('/').to_string(),
        }
    }

    async fn get_json<T: for<'de> Deserialize<'de>>(
        &self,
        url: &str,
        access_token: &str,
    ) -> Result<T, SpotifyApiError> {
        let resp = self
            .http
            .get(url)
            .bearer_auth(access_token)
            .send()
            .await
            .map_err(|e| SpotifyApiError::Http(e.to_string()))?;

        let status = resp.status();
        if status == StatusCode::UNAUTHORIZED {
            return Err(SpotifyApiError::Unauthorized);
        }
        if status == StatusCode::FORBIDDEN {
            let body = resp.text().await.unwrap_or_default();
            return Err(SpotifyApiError::PremiumRequiredOrForbidden(body));
        }
        if status == StatusCode::TOO_MANY_REQUESTS {
            let retry_after_s = resp
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok());
            return Err(SpotifyApiError::RateLimited { retry_after_s });
        }
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(SpotifyApiError::Http(format!("HTTP {status}: {body}")));
        }

        resp.json::<T>()
            .await
            .map_err(|e| SpotifyApiError::Parse(e.to_string()))
    }

    pub async fn get_profile(
        &self,
        access_token: &str,
    ) -> Result<SpotifyUserProfile, SpotifyApiError> {
        let url = format!("{}/me", self.base_url);
        let raw: ApiProfile = self.get_json(&url, access_token).await?;
        let display_name = raw
            .display_name
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| raw.id.clone());
        Ok(SpotifyUserProfile {
            id: raw.id,
            display_name,
            product: SpotifyAccountTier::from_product(raw.product.as_deref()),
        })
    }

    pub async fn get_playlists(
        &self,
        access_token: &str,
    ) -> Result<Vec<SpotifyPlaylistSummary>, SpotifyApiError> {
        let mut out = Vec::new();
        let mut next_url = Some(format!("{}/me/playlists?limit=50", self.base_url));
        let mut pages = 0usize;

        while let Some(url) = next_url.take() {
            pages += 1;
            if pages > 20 {
                break;
            }
            let page: ApiPage<ApiPlaylist> = self.get_json(&url, access_token).await?;
            for pl in page.items {
                if !pl.id.trim().is_empty() {
                    out.push(SpotifyPlaylistSummary {
                        id: pl.id,
                        name: pl.name,
                        track_count: pl.tracks.map(|t| t.total).unwrap_or(0),
                    });
                }
            }
            next_url = page.next;
        }
        Ok(out)
    }

    pub async fn get_playlist_tracks(
        &self,
        access_token: &str,
        playlist_id: &str,
    ) -> Result<Vec<Track>, SpotifyApiError> {
        let mut out = Vec::new();
        let mut next_url = Some(format!(
            "{}/playlists/{}/tracks?limit=100",
            self.base_url, playlist_id
        ));
        let mut pages = 0usize;

        while let Some(url) = next_url.take() {
            pages += 1;
            if pages > 30 {
                break;
            }
            let page: ApiPage<ApiPlaylistTrackItem> = self.get_json(&url, access_token).await?;
            for item in page.items {
                if let Some(raw_track) = item.track {
                    if let Some(track) = map_api_track(raw_track, false) {
                        out.push(track);
                    }
                }
            }
            next_url = page.next;
        }
        Ok(out)
    }

    pub async fn get_liked_tracks(
        &self,
        access_token: &str,
    ) -> Result<Vec<Track>, SpotifyApiError> {
        let mut out = Vec::new();
        let mut next_url = Some(format!("{}/me/tracks?limit=50", self.base_url));
        let mut pages = 0usize;

        while let Some(url) = next_url.take() {
            pages += 1;
            if pages > 40 {
                break;
            }
            let page: ApiPage<ApiSavedTrack> = self.get_json(&url, access_token).await?;
            for item in page.items {
                if let Some(track) = map_api_track(item.track, true) {
                    out.push(track);
                }
            }
            next_url = page.next;
        }
        Ok(out)
    }

    pub async fn search_tracks(
        &self,
        access_token: &str,
        query: &str,
        limit: usize,
    ) -> Result<Vec<Track>, SpotifyApiError> {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return Ok(Vec::new());
        }
        let capped = limit.clamp(1, 50);
        let mut url = url::Url::parse(&format!("{}/search", self.base_url))
            .map_err(|e| SpotifyApiError::Http(e.to_string()))?;
        url.query_pairs_mut()
            .append_pair("type", "track")
            .append_pair("q", trimmed)
            .append_pair("limit", &capped.to_string());

        let parsed: ApiSearchResponse = self.get_json(url.as_str(), access_token).await?;
        let tracks = parsed
            .tracks
            .map(|p| {
                p.items
                    .into_iter()
                    .filter_map(|t| map_api_track(t, false))
                    .collect()
            })
            .unwrap_or_default();
        Ok(tracks)
    }

    /// Saves tracks to the user's Spotify library via Feb 2026 `PUT /v1/me/library?uris=...`.
    pub async fn save_tracks(
        &self,
        access_token: &str,
        track_ids: &[String],
    ) -> Result<(), SpotifyApiError> {
        self.mutate_library(access_token, reqwest::Method::PUT, track_ids)
            .await
    }

    /// Removes tracks from the user's Spotify library via Feb 2026 `DELETE /v1/me/library?uris=...`.
    pub async fn remove_tracks(
        &self,
        access_token: &str,
        track_ids: &[String],
    ) -> Result<(), SpotifyApiError> {
        self.mutate_library(access_token, reqwest::Method::DELETE, track_ids)
            .await
    }

    async fn mutate_library(
        &self,
        access_token: &str,
        method: reqwest::Method,
        track_ids: &[String],
    ) -> Result<(), SpotifyApiError> {
        if track_ids.is_empty() {
            return Ok(());
        }
        let uris = track_ids
            .iter()
            .map(|id| format!("spotify:track:{id}"))
            .collect::<Vec<_>>()
            .join(",");

        let mut url = url::Url::parse(&format!("{}/me/library", self.base_url))
            .map_err(|e| SpotifyApiError::Http(e.to_string()))?;
        url.query_pairs_mut().append_pair("uris", &uris);

        let resp = self
            .http
            .request(method.clone(), url.as_str())
            .bearer_auth(access_token)
            .header("Content-Length", "0")
            .send()
            .await
            .map_err(|e| SpotifyApiError::Http(e.to_string()))?;

        let status = resp.status();
        if status.is_success() {
            return Ok(());
        }
        if status == StatusCode::UNAUTHORIZED {
            return Err(SpotifyApiError::Unauthorized);
        }
        if status == StatusCode::FORBIDDEN {
            let body = resp.text().await.unwrap_or_default();
            return Err(SpotifyApiError::PremiumRequiredOrForbidden(body));
        }
        if status == StatusCode::NOT_FOUND || status == StatusCode::METHOD_NOT_ALLOWED {
            // Compatibility fallback for older endpoints if `/me/library` is absent.
            let ids = track_ids.join(",");
            let mut legacy = url::Url::parse(&format!("{}/me/tracks", self.base_url))
                .map_err(|e| SpotifyApiError::Http(e.to_string()))?;
            legacy.query_pairs_mut().append_pair("ids", &ids);
            let legacy_resp = self
                .http
                .request(method, legacy.as_str())
                .bearer_auth(access_token)
                .header("Content-Length", "0")
                .send()
                .await
                .map_err(|e| SpotifyApiError::Http(e.to_string()))?;
            if legacy_resp.status().is_success() {
                return Ok(());
            }
            let st = legacy_resp.status();
            let body = legacy_resp.text().await.unwrap_or_default();
            if st == StatusCode::FORBIDDEN {
                return Err(SpotifyApiError::PremiumRequiredOrForbidden(body));
            }
            return Err(SpotifyApiError::Http(format!("HTTP {st}: {body}")));
        }
        let body = resp.text().await.unwrap_or_default();
        Err(SpotifyApiError::Http(format!("HTTP {status}: {body}")))
    }

    /// Replays queued `like` / `unlike` mutations from SQLite against `/me/library`.
    /// Deletes each row only after remote confirmation; stops cleanly on auth/403/network errors.
    pub async fn flush_pending_mutations(
        &self,
        db: &Database,
        access_token: &str,
    ) -> Result<usize, SpotifyApiError> {
        let pending = db
            .list_pending_mutations()
            .map_err(|e| SpotifyApiError::Http(e.to_string()))?;
        let mut flushed = 0usize;

        for item in pending {
            let ids = vec![item.spotify_track_id.clone()];
            let res = match item.action.as_str() {
                "like" => self.save_tracks(access_token, &ids).await,
                "unlike" => self.remove_tracks(access_token, &ids).await,
                _ => Ok(()),
            };
            match res {
                Ok(()) => {
                    let _ = db.delete_pending_mutation(item.id);
                    flushed += 1;
                }
                Err(err) => return Err(err),
            }
        }
        Ok(flushed)
    }
}

fn map_api_track(raw: ApiTrack, is_liked: bool) -> Option<Track> {
    if raw.is_local.unwrap_or(false) {
        return None;
    }
    let id = raw.id?.trim().to_string();
    if id.is_empty() {
        return None;
    }
    let source = TrackSource::Spotify { id };
    let uid = source.canonical_uid();
    let artists: Vec<String> = raw
        .artists
        .into_iter()
        .map(|a| a.name.trim().to_string())
        .filter(|n| !n.is_empty())
        .collect();
    let artists = if artists.is_empty() {
        vec!["Unknown Artist".to_string()]
    } else {
        artists
    };
    let (album_name, artwork_ref) = match raw.album {
        Some(alb) => {
            let name = (!alb.name.trim().is_empty()).then_some(alb.name);
            let art = alb
                .images
                .into_iter()
                .max_by_key(|img| img.width.unwrap_or(0) * img.height.unwrap_or(0))
                .map(|img| img.url);
            (name, art)
        }
        None => (None, None),
    };

    Some(Track {
        uid,
        source,
        title: raw.name,
        artists,
        album: album_name,
        duration_ms: raw.duration_ms.unwrap_or(0),
        artwork_ref,
        is_liked,
    })
}

#[derive(Debug, Deserialize)]
struct ApiProfile {
    id: String,
    display_name: Option<String>,
    product: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ApiPage<T> {
    #[serde(default = "Vec::new")]
    items: Vec<T>,
    next: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ApiPlaylist {
    id: String,
    name: String,
    tracks: Option<ApiPlaylistTracksRef>,
}

#[derive(Debug, Deserialize)]
struct ApiPlaylistTracksRef {
    total: usize,
}

#[derive(Debug, Deserialize)]
struct ApiPlaylistTrackItem {
    track: Option<ApiTrack>,
}

#[derive(Debug, Deserialize)]
struct ApiSavedTrack {
    track: ApiTrack,
}

#[derive(Debug, Deserialize)]
struct ApiSearchResponse {
    tracks: Option<ApiPage<ApiTrack>>,
}

#[derive(Debug, Deserialize)]
struct ApiTrack {
    id: Option<String>,
    name: String,
    #[serde(default)]
    artists: Vec<ApiArtist>,
    album: Option<ApiAlbum>,
    duration_ms: Option<u64>,
    is_local: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct ApiArtist {
    name: String,
}

#[derive(Debug, Deserialize)]
struct ApiAlbum {
    name: String,
    #[serde(default)]
    images: Vec<ApiImage>,
}

#[derive(Debug, Deserialize)]
struct ApiImage {
    url: String,
    width: Option<u32>,
    height: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    async fn spawn_mock_spotify_server(
        handler: impl Fn(String) -> (u16, String) + Send + Sync + 'static,
    ) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let handler = Arc::new(handler);

        tokio::spawn(async move {
            loop {
                let Ok((mut socket, _)) = listener.accept().await else {
                    break;
                };
                let handler = Arc::clone(&handler);
                tokio::spawn(async move {
                    let mut buf = [0u8; 4096];
                    let n = socket.read(&mut buf).await.unwrap_or(0);
                    let req = String::from_utf8_lossy(&buf[..n]);
                    let first_line = req.lines().next().unwrap_or_default().to_string();
                    let (status, body) = handler(first_line);
                    let reason = match status {
                        200 => "OK",
                        401 => "Unauthorized",
                        403 => "Forbidden",
                        _ => "Error",
                    };
                    let resp = format!(
                        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    let _ = socket.write_all(resp.as_bytes()).await;
                    let _ = socket.shutdown().await;
                });
            }
        });

        format!("http://{addr}/v1")
    }

    #[tokio::test]
    async fn web_client_parses_profile_playlists_search_and_flushes_me_library_mutations() {
        let seen_requests = Arc::new(Mutex::new(Vec::<String>::new()));
        let seen_clone = Arc::clone(&seen_requests);

        let base_url = spawn_mock_spotify_server(move |req_line| {
            seen_clone.lock().unwrap().push(req_line.clone());
            if req_line.starts_with("GET /v1/me ") {
                (
                    200,
                    r#"{"id":"u1","display_name":"Alex","product":"premium"}"#.to_string(),
                )
            } else if req_line.starts_with("GET /v1/me/playlists") {
                (
                    200,
                    r#"{"items":[{"id":"pl1","name":"Night Drive","tracks":{"total":1}}],"next":null}"#
                        .to_string(),
                )
            } else if req_line.starts_with("GET /v1/search") {
                (
                    200,
                    r#"{"tracks":{"items":[{"id":"tr1","name":"Blinding Lights","artists":[{"name":"The Weeknd"}],"album":{"name":"After Hours","images":[{"url":"https://i.scdn.co/art.jpg","width":640,"height":640}]},"duration_ms":200040,"is_local":false}],"next":null}}"#
                        .to_string(),
                )
            } else if req_line.starts_with("PUT /v1/me/library")
                || req_line.starts_with("DELETE /v1/me/library")
            {
                (200, String::new())
            } else {
                (403, r#"{"error":{"status":403,"message":"Premium required"}}"#.to_string())
            }
        })
        .await;

        let client = SpotifyWebClient::with_base_url(base_url);
        let profile = client.get_profile("tok").await.unwrap();
        assert_eq!(profile.display_name, "Alex");
        assert_eq!(profile.product, SpotifyAccountTier::Premium);

        let playlists = client.get_playlists("tok").await.unwrap();
        assert_eq!(playlists.len(), 1);
        assert_eq!(playlists[0].name, "Night Drive");

        let found = client.search_tracks("tok", "Blinding Lights", 10).await.unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].uid, "spotify:tr1");
        assert_eq!(found[0].artwork_ref.as_deref(), Some("https://i.scdn.co/art.jpg"));

        // Flush pending mutations through `/v1/me/library`.
        let db = Database::open_in_memory().unwrap();
        db.enqueue_pending_mutation("like", "tr1").unwrap();
        db.enqueue_pending_mutation("unlike", "tr2").unwrap();
        let flushed = client.flush_pending_mutations(&db, "tok").await.unwrap();
        assert_eq!(flushed, 2);
        assert_eq!(db.count_pending_mutations().unwrap(), 0);

        let recorded = seen_requests.lock().unwrap().clone();
        assert!(recorded
            .iter()
            .any(|r| r.starts_with("PUT /v1/me/library?uris=spotify%3Atrack%3Atr1")));
        assert!(recorded
            .iter()
            .any(|r| r.starts_with("DELETE /v1/me/library?uris=spotify%3Atrack%3Atr2")));
    }

    #[tokio::test]
    async fn forbidden_403_classifies_as_lapsed_premium_fallback_trigger() {
        let base_url = spawn_mock_spotify_server(|_| {
            (
                403,
                r#"{"error":{"status":403,"message":"User not in premium tier"}}"#.to_string(),
            )
        })
        .await;

        let client = SpotifyWebClient::with_base_url(base_url);
        let err = client.get_playlists("tok").await.unwrap_err();
        assert!(matches!(err, SpotifyApiError::PremiumRequiredOrForbidden(_)));
        assert!(err.triggers_fallback());
    }
}
