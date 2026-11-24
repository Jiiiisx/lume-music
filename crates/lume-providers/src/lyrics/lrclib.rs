use std::time::Duration;

use lume_core::{CoreError, LyricsData, Track};
use reqwest::{Client, StatusCode};
use serde::Deserialize;

use crate::lyrics::lrc::parse_lrc;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LrclibRecord {
    synced_lyrics: Option<String>,
    plain_lyrics: Option<String>,
    duration: Option<f64>,
    #[serde(default)]
    instrumental: bool,
}

impl LrclibRecord {
    fn into_lyrics_data(mut self) -> Option<LyricsData> {
        if self.instrumental {
            return None;
        }
        if let Some(synced_raw) = self.synced_lyrics.take().filter(|s| !s.trim().is_empty()) {
            let mut parsed = parse_lrc(&synced_raw, "lrclib");
            if parsed.plain.is_none() {
                parsed.plain = self.plain_lyrics.take().filter(|s| !s.trim().is_empty());
            }
            if !parsed.synced.is_empty() || parsed.plain.is_some() {
                return Some(parsed);
            }
        }
        self.plain_lyrics
            .filter(|s| !s.trim().is_empty())
            .map(|plain| LyricsData {
                synced: Vec::new(),
                plain: Some(plain),
                provider: "lrclib".to_string(),
            })
    }
}

/// HTTP client for LRCLIB (`https://lrclib.net`).
#[derive(Debug, Clone)]
pub struct LrclibClient {
    base_url: String,
    http: Client,
}

impl LrclibClient {
    pub fn new(base_url: String) -> Self {
        let http = Client::builder()
            .user_agent("LumeMusic/1.0")
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_else(|_| Client::new());
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            http,
        }
    }

    pub async fn get_exact(&self, track: &Track) -> Result<Option<LyricsData>, CoreError> {
        let artist = track.artist_display();
        let mut params = vec![
            ("track_name", track.title.clone()),
            ("artist_name", artist),
        ];
        if let Some(album) = &track.album {
            if !album.trim().is_empty() {
                params.push(("album_name", album.clone()));
            }
        }
        if track.duration_ms > 0 {
            let secs = (track.duration_ms as f64 / 1000.0).round() as u64;
            params.push(("duration", secs.to_string()));
        }

        let url = format!("{}/api/get", self.base_url);
        let resp = self
            .http
            .get(&url)
            .query(&params)
            .send()
            .await
            .map_err(|e| CoreError::Network(e.to_string()))?;

        if resp.status() == StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !resp.status().is_success() {
            return Err(CoreError::Network(format!(
                "LRCLIB /api/get status {}",
                resp.status()
            )));
        }

        let rec: LrclibRecord = resp
            .json()
            .await
            .map_err(|e| CoreError::Network(e.to_string()))?;
        Ok(rec.into_lyrics_data())
    }

    pub async fn search_best_match(&self, track: &Track) -> Result<Option<LyricsData>, CoreError> {
        let artist = track.artist_display();
        let url = format!("{}/api/search", self.base_url);
        let resp = self
            .http
            .get(&url)
            .query(&[
                ("track_name", track.title.as_str()),
                ("artist_name", artist.as_str()),
            ])
            .send()
            .await
            .map_err(|e| CoreError::Network(e.to_string()))?;

        if resp.status() == StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !resp.status().is_success() {
            return Err(CoreError::Network(format!(
                "LRCLIB /api/search status {}",
                resp.status()
            )));
        }

        let candidates: Vec<LrclibRecord> = resp
            .json()
            .await
            .map_err(|e| CoreError::Network(e.to_string()))?;

        let target_secs = track.duration_ms as f64 / 1000.0;
        let mut best_synced: Option<(f64, LrclibRecord)> = None;
        let mut best_plain: Option<(f64, LrclibRecord)> = None;

        for rec in candidates {
            if rec.instrumental {
                continue;
            }
            let diff = match (track.duration_ms > 0, rec.duration) {
                (true, Some(dur)) => (dur - target_secs).abs(),
                (true, None) => 5.0,
                (false, _) => 0.0,
            };
            if track.duration_ms > 0 && diff > 5.0 {
                continue;
            }
            let has_synced = rec
                .synced_lyrics
                .as_ref()
                .map(|s| !s.trim().is_empty())
                .unwrap_or(false);
            if has_synced {
                if best_synced
                    .as_ref()
                    .map(|(d, _)| diff < *d)
                    .unwrap_or(true)
                {
                    best_synced = Some((diff, rec));
                }
            } else if rec
                .plain_lyrics
                .as_ref()
                .map(|s| !s.trim().is_empty())
                .unwrap_or(false)
                && best_plain.as_ref().map(|(d, _)| diff < *d).unwrap_or(true)
            {
                best_plain = Some((diff, rec));
            }
        }

        Ok(best_synced
            .or(best_plain)
            .and_then(|(_, rec)| rec.into_lyrics_data()))
    }

    pub async fn fetch(&self, track: &Track) -> Result<Option<LyricsData>, CoreError> {
        if let Some(exact) = self.get_exact(track).await? {
            return Ok(Some(exact));
        }
        self.search_best_match(track).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lume_core::TrackSource;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn lrclib_falls_back_from_exact_get_404_to_search_duration_match() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let server = tokio::spawn(async move {
            for _ in 0..2 {
                let (mut sock, _) = listener.accept().await.unwrap();
                let mut buf = [0u8; 4096];
                let n = sock.read(&mut buf).await.unwrap();
                let req = String::from_utf8_lossy(&buf[..n]);
                if req.starts_with("GET /api/get") {
                    let resp = "HTTP/1.1 404 Not Found\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}";
                    sock.write_all(resp.as_bytes()).await.unwrap();
                } else if req.starts_with("GET /api/search") {
                    let body = r#"[
                        {"syncedLyrics":"[00:01.00]Wrong Duration","plainLyrics":"Wrong","duration":240},
                        {"syncedLyrics":"[00:02.00]Matched Line","plainLyrics":"Matched","duration":201}
                    ]"#;
                    let resp = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        body.len(),
                        body
                    );
                    sock.write_all(resp.as_bytes()).await.unwrap();
                }
            }
        });

        let track = Track {
            uid: "spotify:s1".into(),
            source: TrackSource::Spotify { id: "s1".into() },
            title: "Test Song".into(),
            artists: vec!["Test Artist".into()],
            album: Some("Test Album".into()),
            duration_ms: 200_000,
            artwork_ref: None,
            is_liked: false,
        };

        let client = LrclibClient::new(format!("http://{addr}"));
        let result = client.fetch(&track).await.unwrap().unwrap();
        assert_eq!(result.synced.len(), 1);
        assert_eq!(result.synced[0].text, "Matched Line");
        assert_eq!(result.provider, "lrclib");

        server.await.unwrap();
    }
}
