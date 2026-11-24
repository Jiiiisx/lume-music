use std::path::{Path, PathBuf};
use std::time::Duration;

use async_trait::async_trait;
use lume_core::{
    CoreError, MusicProvider, PlaybackSourceUsed, Playlist, ResolvedStream, Track, TrackSource,
};
use rustypipe::client::{ClientType, RustyPipe};
use rustypipe::model::{AudioCodec, AudioFormat, AudioStream, AudioTrackType, Thumbnail, TrackItem};

/// Format filter passed to `yt-dlp` so resolved streams decode cleanly in `symphonia`/`rodio`
/// (preferring M4A/AAC over WebM/Opus).
pub const YT_DLP_FORMAT_SELECTOR: &str = "bestaudio[ext=m4a]/bestaudio[acodec^=mp4a]/bestaudio";

/// Normalized representation of an audio stream candidate used for deterministic stream selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioStreamCandidate {
    pub url: String,
    pub bitrate: u32,
    pub is_m4a_aac: bool,
    pub is_opus: bool,
    pub is_original_track: bool,
    pub is_drm: bool,
    pub size: u64,
}

impl From<&AudioStream> for AudioStreamCandidate {
    fn from(s: &AudioStream) -> Self {
        let mime_lower = s.mime.to_ascii_lowercase();
        let is_m4a_aac = s.format == AudioFormat::M4a
            || s.codec == AudioCodec::Mp4a
            || mime_lower.contains("audio/mp4");
        let is_opus = s.codec == AudioCodec::Opus || mime_lower.contains("opus");
        let is_original_track = s
            .track
            .as_ref()
            .map(|t| t.track_type == Some(AudioTrackType::Original) || t.track_type.is_none())
            .unwrap_or(true);
        Self {
            url: s.url.clone(),
            bitrate: s.bitrate,
            is_m4a_aac,
            is_opus,
            is_original_track,
            is_drm: s.drm_track_type.is_some(),
            size: s.size,
        }
    }
}

/// Select the best `symphonia`-compatible audio stream from candidates.
pub fn select_best_audio_stream<'a>(
    candidates: &'a [AudioStreamCandidate],
) -> Result<&'a AudioStreamCandidate, CoreError> {
    let best = candidates
        .iter()
        .filter(|c| !c.url.trim().is_empty() && !c.is_drm && !c.is_opus)
        .max_by_key(|c| (c.is_m4a_aac, c.is_original_track, c.bitrate, c.size));

    best.ok_or_else(|| {
        CoreError::ProviderUnavailable(
            "no symphonia-compatible (non-DRM, non-Opus) audio stream found".into(),
        )
    })
}

/// Upgrade Google `lh3.googleusercontent.com` square cover thumbnails to crisp `w544-h544`.
pub fn upgrade_ytm_cover_url(url: &str) -> String {
    if !url.contains("googleusercontent.com") {
        return url.to_string();
    }
    if let Some(eq_idx) = url.rfind('=') {
        let prefix = &url[..eq_idx];
        let suffix = &url[eq_idx + 1..];
        if suffix.starts_with('w') && suffix.contains("-h") {
            let mut parts: Vec<String> = suffix.split('-').map(ToString::to_string).collect();
            for part in &mut parts {
                if part.starts_with('w') && part[1..].chars().all(|c| c.is_ascii_digit()) {
                    *part = "w544".to_string();
                } else if part.starts_with('h') && part[1..].chars().all(|c| c.is_ascii_digit()) {
                    *part = "h544".to_string();
                }
            }
            return format!("{prefix}={}", parts.join("-"));
        }
    }
    url.to_string()
}

/// Pick the best cover thumbnail URL from a RustyPipe thumbnail list.
pub fn select_best_thumbnail(thumbnails: &[Thumbnail]) -> Option<String> {
    thumbnails
        .iter()
        .max_by_key(|t| (t.width as u64) * (t.height as u64))
        .map(|t| upgrade_ytm_cover_url(&t.url))
}

/// Map a RustyPipe YouTube Music `TrackItem` into our core `Track`.
pub fn map_track_item(item: TrackItem) -> Track {
    let source = TrackSource::YouTubeMusic {
        video_id: item.id.clone(),
    };
    let uid = source.canonical_uid();
    let artists: Vec<String> = item
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
    let album = item
        .album
        .map(|a| a.name.trim().to_string())
        .filter(|n| !n.is_empty());
    let artwork_ref = select_best_thumbnail(&item.cover);
    let duration_ms = u64::from(item.duration.unwrap_or(0)) * 1000;

    Track {
        uid,
        source,
        title: item.name,
        artists,
        album,
        duration_ms,
        artwork_ref,
        is_liked: false,
    }
}

/// Locate a usable `yt-dlp` binary, checking the user-configured path first and then `PATH`.
pub fn detect_yt_dlp_binary(configured: Option<&Path>) -> Option<PathBuf> {
    if let Some(cfg) = configured {
        if cfg.is_file() {
            return Some(cfg.to_path_buf());
        }
    }

    let path_var = std::env::var_os("PATH")?;
    let candidates: &[&str] = if cfg!(windows) {
        &["yt-dlp.exe", "yt-dlp.cmd", "yt-dlp.bat", "yt-dlp"]
    } else {
        &["yt-dlp"]
    };

    for dir in std::env::split_paths(&path_var) {
        for name in candidates {
            let full = dir.join(name);
            if full.is_file() {
                return Some(full);
            }
        }
    }
    None
}

/// Extract the first valid `http://` or `https://` audio stream URL from `yt-dlp -g` stdout.
pub fn parse_yt_dlp_stdout(stdout: &str) -> Option<String> {
    stdout
        .lines()
        .map(str::trim)
        .find(|line| line.starts_with("https://") || line.starts_with("http://"))
        .map(ToString::to_string)
}

/// Anonymous YouTube Music catalog & stream provider powered by `rustypipe` + `yt-dlp` fallback.
#[derive(Clone)]
pub struct YouTubeMusicProvider {
    client: RustyPipe,
    http: reqwest::Client,
    yt_dlp_path: Option<PathBuf>,
}

impl YouTubeMusicProvider {
    pub fn default_cache_dir() -> PathBuf {
        let base = dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("."));
        base.join("LumeMusic").join("cache").join("rustypipe")
    }

    pub fn new(cache_dir: PathBuf, yt_dlp_path: Option<PathBuf>) -> Result<Self, CoreError> {
        let _ = std::fs::create_dir_all(&cache_dir);
        let client = RustyPipe::builder()
            .storage_dir(cache_dir)
            .build()
            .map_err(|e| CoreError::ProviderUnavailable(format!("rustypipe init failed: {e}")))?;
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| CoreError::Network(format!("http client init failed: {e}")))?;
        Ok(Self {
            client,
            http,
            yt_dlp_path,
        })
    }

    /// Lightweight HTTP range probe (`bytes=0-1`) to catch PO-token / HTTP 403 stream rejections
    /// before handing the URL to the audio engine.
    async fn probe_stream_url(&self, url: &str) -> Result<(), CoreError> {
        let resp = self
            .http
            .get(url)
            .header(reqwest::header::RANGE, "bytes=0-1")
            .send()
            .await
            .map_err(|e| CoreError::Network(format!("stream probe failed: {e}")))?;
        let status = resp.status();
        if status.is_success() || status == reqwest::StatusCode::PARTIAL_CONTENT {
            Ok(())
        } else {
            Err(CoreError::ProviderUnavailable(format!(
                "YouTube stream rejected with HTTP {}",
                status.as_u16()
            )))
        }
    }

    async fn resolve_via_rustypipe(&self, video_id: &str) -> Result<String, CoreError> {
        let clients = [
            ClientType::Tv,
            ClientType::Android,
            ClientType::Ios,
            ClientType::Desktop,
        ];
        let mut last_err = CoreError::ProviderUnavailable("no rustypipe client succeeded".into());

        for client_type in clients {
            let player = match self
                .client
                .query()
                .player_from_client(video_id, client_type)
                .await
            {
                Ok(p) => p,
                Err(e) => {
                    last_err = CoreError::ProviderUnavailable(format!(
                        "rustypipe ({client_type:?}) failed: {e}"
                    ));
                    continue;
                }
            };

            let candidates: Vec<AudioStreamCandidate> = player
                .audio_streams
                .iter()
                .map(AudioStreamCandidate::from)
                .collect();
            let best = match select_best_audio_stream(&candidates) {
                Ok(b) => b,
                Err(e) => {
                    last_err = e;
                    continue;
                }
            };

            match self.probe_stream_url(&best.url).await {
                Ok(()) => return Ok(best.url.clone()),
                Err(e) => {
                    last_err = e;
                    continue;
                }
            }
        }

        Err(last_err)
    }

    /// Fallback stream resolver invoking `yt-dlp -f ... -g --no-playlist <url>`.
    pub async fn resolve_via_yt_dlp(&self, video_id: &str) -> Result<String, CoreError> {
        let Some(binary) = detect_yt_dlp_binary(self.yt_dlp_path.as_deref()) else {
            return Err(CoreError::ProviderUnavailable(
                "yt-dlp not found on PATH or Settings; install yt-dlp (e.g. `winget install yt-dlp.yt-dlp`) or set its path in Settings".into(),
            ));
        };

        let watch_url = format!("https://music.youtube.com/watch?v={video_id}");
        let mut cmd = tokio::process::Command::new(&binary);
        cmd.args([
            "-f",
            YT_DLP_FORMAT_SELECTOR,
            "-g",
            "--no-playlist",
            "--no-warnings",
            &watch_url,
        ])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

        #[cfg(windows)]
        {
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }

        let output = tokio::time::timeout(Duration::from_secs(20), cmd.output())
            .await
            .map_err(|_| CoreError::Network("yt-dlp timed out resolving stream".into()))?
            .map_err(|e| CoreError::ProviderUnavailable(format!("failed to run yt-dlp: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let detail = stderr.lines().last().unwrap_or("unknown error").trim();
            return Err(CoreError::ProviderUnavailable(format!(
                "yt-dlp failed ({}): {detail}",
                output.status
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        parse_yt_dlp_stdout(&stdout).ok_or_else(|| {
            CoreError::ProviderUnavailable("yt-dlp returned no playable stream URL".into())
        })
    }

    /// Resolve a playable audio stream URL for a YouTube video ID (`rustypipe` primary,
    /// automatic `yt-dlp` fallback on extraction or HTTP 403 rejection).
    pub async fn resolve_stream_for_video_id(
        &self,
        video_id: &str,
    ) -> Result<ResolvedStream, CoreError> {
        let trimmed = video_id.trim();
        if trimmed.is_empty() {
            return Err(CoreError::InvalidInput("empty YouTube video_id".into()));
        }

        match self.resolve_via_rustypipe(trimmed).await {
            Ok(url) => Ok(ResolvedStream::HttpUrl {
                url,
                source: PlaybackSourceUsed::YouTubeDirect,
            }),
            Err(primary_err) => match self.resolve_via_yt_dlp(trimmed).await {
                Ok(url) => Ok(ResolvedStream::HttpUrl {
                    url,
                    source: PlaybackSourceUsed::YouTubeDirect,
                }),
                Err(fallback_err) => Err(CoreError::ProviderUnavailable(format!(
                    "YouTube stream failed ({primary_err}); fallback: {fallback_err}"
                ))),
            },
        }
    }
}

#[async_trait]
impl MusicProvider for YouTubeMusicProvider {
    async fn search(&self, query: &str, limit: usize) -> Result<Vec<Track>, CoreError> {
        let trimmed = query.trim();
        if trimmed.is_empty() || limit == 0 {
            return Ok(Vec::new());
        }

        let res = self
            .client
            .query()
            .music_search_tracks(trimmed)
            .await
            .map_err(|e| CoreError::Network(format!("YouTube Music search failed: {e}")))?;

        let tracks = res
            .items
            .items
            .into_iter()
            .take(limit)
            .map(map_track_item)
            .collect();
        Ok(tracks)
    }

    async fn get_playlists(&self) -> Result<Vec<Playlist>, CoreError> {
        Ok(Vec::new())
    }

    async fn get_playlist_tracks(&self, _playlist_id: &str) -> Result<Vec<Track>, CoreError> {
        Ok(Vec::new())
    }

    async fn resolve_stream(&self, track: &Track) -> Result<ResolvedStream, CoreError> {
        match &track.source {
            TrackSource::YouTubeMusic { video_id } => {
                self.resolve_stream_for_video_id(video_id).await
            }
            _ => Err(CoreError::InvalidInput(
                "YouTubeMusicProvider only resolves YouTubeMusic tracks".into(),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(
        url: &str,
        bitrate: u32,
        is_m4a_aac: bool,
        is_opus: bool,
        is_original_track: bool,
        is_drm: bool,
    ) -> AudioStreamCandidate {
        AudioStreamCandidate {
            url: url.to_string(),
            bitrate,
            is_m4a_aac,
            is_opus,
            is_original_track,
            is_drm,
            size: 1_000_000,
        }
    }

    #[test]
    fn select_best_audio_stream_prefers_highest_bitrate_m4a_over_opus() {
        let streams = vec![
            candidate("https://cdn/opus-160", 160_000, false, true, true, false),
            candidate("https://cdn/m4a-64", 64_000, true, false, true, false),
            candidate("https://cdn/m4a-128", 128_000, true, false, true, false),
            candidate("https://cdn/m4a-256-drm", 256_000, true, false, true, true),
            candidate("https://cdn/m4a-256-dubbed", 256_000, true, false, false, false),
        ];
        let best = select_best_audio_stream(&streams).expect("should select m4a-128");
        assert_eq!(best.url, "https://cdn/m4a-128");
    }

    #[test]
    fn select_best_audio_stream_rejects_empty_or_opus_only() {
        assert!(select_best_audio_stream(&[]).is_err());
        let opus_only = vec![candidate("https://cdn/opus", 160_000, false, true, true, false)];
        assert!(select_best_audio_stream(&opus_only).is_err());
    }

    #[test]
    fn upgrade_ytm_cover_url_normalizes_lh3_dimensions() {
        let raw = "https://lh3.googleusercontent.com/abc123=w60-h60-l90-rj";
        assert_eq!(
            upgrade_ytm_cover_url(raw),
            "https://lh3.googleusercontent.com/abc123=w544-h544-l90-rj"
        );
        let ytimg = "https://i.ytimg.com/vi/dQw4w9WgXcQ/hqdefault.jpg";
        assert_eq!(upgrade_ytm_cover_url(ytimg), ytimg);
    }

    #[test]
    fn maps_ytm_track_metadata_and_canonical_uid_correctly() {
        let item: TrackItem = serde_json::from_value(serde_json::json!({
            "id": "dQw4w9WgXcQ",
            "name": "Never Gonna Give You Up",
            "duration": 213,
            "cover": [
                {
                    "url": "https://lh3.googleusercontent.com/small=w60-h60-l90-rj",
                    "width": 60,
                    "height": 60
                },
                {
                    "url": "https://lh3.googleusercontent.com/big=w120-h120-l90-rj",
                    "width": 120,
                    "height": 120
                }
            ],
            "artists": [
                {
                    "id": "UC123",
                    "name": "Rick Astley"
                }
            ],
            "artist_id": "UC123",
            "album": {
                "id": "MPREb_1",
                "name": "Whenever You Need Somebody"
            },
            "view_count": 1000000,
            "track_type": "track",
            "track_nr": 1,
            "by_va": false,
            "unavailable": false
        }))
        .expect("deserialize TrackItem fixture");

        let track = map_track_item(item);
        assert_eq!(track.uid, "ytm:dQw4w9WgXcQ");
        assert_eq!(
            track.source,
            TrackSource::YouTubeMusic {
                video_id: "dQw4w9WgXcQ".into()
            }
        );
        assert_eq!(track.title, "Never Gonna Give You Up");
        assert_eq!(track.artists, vec!["Rick Astley".to_string()]);
        assert_eq!(track.album.as_deref(), Some("Whenever You Need Somebody"));
        assert_eq!(track.duration_ms, 213_000);
        assert_eq!(
            track.artwork_ref.as_deref(),
            Some("https://lh3.googleusercontent.com/big=w544-h544-l90-rj")
        );
        assert!(!track.is_liked);
    }

    #[test]
    fn parse_yt_dlp_stdout_extracts_first_http_url() {
        let out = "WARNING: something\n\nhttps://rr1---sn.googlevideo.com/videoplayback?itag=140\n";
        assert_eq!(
            parse_yt_dlp_stdout(out).as_deref(),
            Some("https://rr1---sn.googlevideo.com/videoplayback?itag=140")
        );
        assert_eq!(parse_yt_dlp_stdout("not a url\n"), None);
    }

    #[test]
    fn detect_yt_dlp_binary_prefers_existing_configured_path() {
        let dir = tempfile::TempDir::new().unwrap();
        let fake_bin = dir.path().join("yt-dlp.exe");
        std::fs::write(&fake_bin, b"stub").unwrap();

        let detected = detect_yt_dlp_binary(Some(&fake_bin));
        assert_eq!(detected.as_deref(), Some(fake_bin.as_path()));
    }

    #[tokio::test]
    async fn probe_stream_url_accepts_206_and_rejects_403() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let server = tokio::spawn(async move {
            for _ in 0..2 {
                let (mut sock, _) = listener.accept().await.unwrap();
                let mut buf = [0u8; 1024];
                let n = sock.read(&mut buf).await.unwrap();
                let req = String::from_utf8_lossy(&buf[..n]);
                let resp = if req.contains("GET /ok") {
                    "HTTP/1.1 206 Partial Content\r\nContent-Length: 2\r\n\r\nab"
                } else {
                    "HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\n\r\n"
                };
                sock.write_all(resp.as_bytes()).await.unwrap();
            }
        });

        let dir = tempfile::TempDir::new().unwrap();
        let provider = YouTubeMusicProvider::new(dir.path().to_path_buf(), None).unwrap();

        assert!(provider
            .probe_stream_url(&format!("http://{addr}/ok"))
            .await
            .is_ok());
        let err = provider
            .probe_stream_url(&format!("http://{addr}/forbidden"))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("403"));
        server.await.unwrap();
    }

    #[tokio::test]
    async fn resolve_via_yt_dlp_executes_configured_script_and_parses_url() {
        let dir = tempfile::TempDir::new().unwrap();
        #[cfg(windows)]
        let script = {
            let p = dir.path().join("mock-yt-dlp.cmd");
            std::fs::write(
                &p,
                "@echo off\r\necho https://mock.googlevideo.com/audio.m4a\r\n",
            )
            .unwrap();
            p
        };
        #[cfg(not(windows))]
        let script = {
            use std::os::unix::fs::PermissionsExt;
            let p = dir.path().join("mock-yt-dlp.sh");
            std::fs::write(&p, "#!/bin/sh\necho https://mock.googlevideo.com/audio.m4a\n").unwrap();
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
            p
        };

        let provider = YouTubeMusicProvider::new(dir.path().to_path_buf(), Some(script)).unwrap();
        let url = provider.resolve_via_yt_dlp("dQw4w9WgXcQ").await.unwrap();
        assert_eq!(url, "https://mock.googlevideo.com/audio.m4a");
    }
}
