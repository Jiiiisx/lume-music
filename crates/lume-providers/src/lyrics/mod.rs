pub mod lrc;
pub mod lrclib;
pub mod ttml;

use std::time::{SystemTime, UNIX_EPOCH};

use lume_core::{CoreError, LyricsData, Track, TrackSource};
use lume_storage::{Database, LyricsCacheLookup};

use crate::local::{find_sidecar_lrc, find_sidecar_ttml, read_embedded_lyrics};
pub use lrc::parse_lrc;
pub use lrclib::LrclibClient;
pub use ttml::parse_ttml;

fn now_epoch_s() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Multi-step lyrics resolver: SQLite cache -> local sidecar/embedded -> LRCLIB.
#[derive(Debug, Clone)]
pub struct ChainedLyricsProvider {
    lrclib: LrclibClient,
}

impl ChainedLyricsProvider {
    pub fn new(lrclib_base_url: String) -> Self {
        Self {
            lrclib: LrclibClient::new(lrclib_base_url),
        }
    }

    pub async fn fetch(
        &self,
        track: &Track,
        db: &Database,
        force_refresh: bool,
    ) -> Result<Option<LyricsData>, CoreError> {
        let now = now_epoch_s();

        if !force_refresh {
            match db
                .get_cached_lyrics(&track.uid, now)
                .map_err(|e| CoreError::Storage(e.to_string()))?
            {
                LyricsCacheLookup::Hit(data) => return Ok(Some(data)),
                LyricsCacheLookup::NegativeHit => return Ok(None),
                LyricsCacheLookup::Miss => {}
            }
        }

        if let TrackSource::Local { path } = &track.source {
            if let Some(ttml_path) = find_sidecar_ttml(path) {
                if let Ok(raw) = std::fs::read_to_string(&ttml_path) {
                    let parsed = parse_ttml(&raw, "sidecar_ttml");
                    if !parsed.synced.is_empty() || parsed.plain.is_some() {
                        db.put_cached_lyrics(&track.uid, &parsed, now)
                            .map_err(|e| CoreError::Storage(e.to_string()))?;
                        return Ok(Some(parsed));
                    }
                }
            }

            if let Some(lrc_path) = find_sidecar_lrc(path) {
                if let Ok(raw) = std::fs::read_to_string(&lrc_path) {
                    let parsed = parse_lrc(&raw, "sidecar_lrc");
                    if !parsed.synced.is_empty() || parsed.plain.is_some() {
                        db.put_cached_lyrics(&track.uid, &parsed, now)
                            .map_err(|e| CoreError::Storage(e.to_string()))?;
                        return Ok(Some(parsed));
                    }
                }
            }

            if let Some(embedded_raw) = read_embedded_lyrics(path) {
                let parsed = parse_lrc(&embedded_raw, "embedded");
                if !parsed.synced.is_empty() || parsed.plain.is_some() {
                    db.put_cached_lyrics(&track.uid, &parsed, now)
                        .map_err(|e| CoreError::Storage(e.to_string()))?;
                    return Ok(Some(parsed));
                }
            }
        }

        match self.lrclib.fetch(track).await? {
            Some(data) => {
                db.put_cached_lyrics(&track.uid, &data, now)
                    .map_err(|e| CoreError::Storage(e.to_string()))?;
                Ok(Some(data))
            }
            None => {
                db.put_negative_lyrics_hit(&track.uid, now)
                    .map_err(|e| CoreError::Storage(e.to_string()))?;
                Ok(None)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lume_core::LyricsLine;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    fn sample_track(id: &str) -> Track {
        let source = TrackSource::Spotify { id: id.into() };
        Track {
            uid: source.canonical_uid(),
            source,
            title: "Song Title".into(),
            artists: vec!["Artist".into()],
            album: Some("Album".into()),
            duration_ms: 200_000,
            artwork_ref: None,
            is_liked: false,
        }
    }

    #[tokio::test]
    async fn chained_provider_returns_sqlite_cache_hit_without_http() {
        let db = Database::open_in_memory().unwrap();
        let track = sample_track("cached_1");
        let cached = LyricsData {
            synced: vec![LyricsLine {
                time_ms: 5000,
                text: "From SQLite".into(),
                words: None,
            }],
            plain: Some("From SQLite".into()),
            provider: "lrclib".into(),
        };
        db.put_cached_lyrics(&track.uid, &cached, now_epoch_s())
            .unwrap();

        // Closed/unreachable port proves no HTTP request is made on cache hit.
        let provider = ChainedLyricsProvider::new("http://127.0.0.1:9".into());
        let got = provider.fetch(&track, &db, false).await.unwrap();
        assert_eq!(got, Some(cached));
    }

    #[tokio::test]
    async fn chained_provider_respects_24h_negative_cache_unless_force_refresh() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let server = tokio::spawn(async move {
            let (mut sock, _) = listener.accept().await.unwrap();
            let mut buf = [0u8; 2048];
            let _ = sock.read(&mut buf).await;
            let body = r#"{"syncedLyrics":"[00:01.00]Refreshed","plainLyrics":"Refreshed","duration":200}"#;
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            sock.write_all(resp.as_bytes()).await.unwrap();
        });

        let db = Database::open_in_memory().unwrap();
        let track = sample_track("neg_1");
        db.put_negative_lyrics_hit(&track.uid, now_epoch_s())
            .unwrap();

        let provider = ChainedLyricsProvider::new(format!("http://{addr}"));
        // Without force_refresh, returns None immediately from negative cache.
        assert_eq!(provider.fetch(&track, &db, false).await.unwrap(), None);

        // With force_refresh = true, bypasses negative cache and hits server.
        let refreshed = provider.fetch(&track, &db, true).await.unwrap().unwrap();
        assert_eq!(refreshed.synced[0].text, "Refreshed");
        server.await.unwrap();
    }
}
