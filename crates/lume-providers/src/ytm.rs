use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use lume_core::{
    CoreError, MusicProvider, PlaybackSourceUsed, Playlist, ResolvedStream, Track, TrackSource,
};
use rustypipe::client::{ClientType, RustyPipe};
use rustypipe::model::{AudioCodec, AudioFormat, AudioStream, AudioTrackType, Thumbnail, TrackItem};
use tokio::sync::RwLock;

const VISITOR_DATA_TTL: Duration = Duration::from_secs(4 * 60 * 60);
const STREAM_CACHE_DEFAULT_TTL: Duration = Duration::from_secs(25 * 60);
const STREAM_CACHE_MAX_TTL: Duration = Duration::from_secs(30 * 60);
const STREAM_CACHE_MAX_ENTRIES: usize = 256;
const INNERTUBE_PLAYER_URL: &str = "https://www.youtube.com/youtubei/v1/player?prettyPrint=false";
const YOUTUBE_SW_JS_DATA_URL: &str = "https://www.youtube.com/sw.js_data";
const VISIONOS_USER_AGENT: &str =
    "com.google.ios.youtube/0.1 (RealityDevice14,1; U; CPU OS 17_0 like Mac OS X; en_US)";
const VISIONOS_CLIENT_NAME_HEADER: &str = "101";
const VISIONOS_CLIENT_VERSION: &str = "0.1";

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

/// Locate a usable `yt-dlp` binary, checking the user-configured path first, then `PATH`,
/// then standard Windows `WinGet\Links` so newly installed `yt-dlp` works without a reboot.
pub fn detect_yt_dlp_binary(configured: Option<&Path>) -> Option<PathBuf> {
    if let Some(cfg) = configured {
        if cfg.is_file() {
            return Some(cfg.to_path_buf());
        }
    }

    let candidates: &[&str] = if cfg!(windows) {
        &["yt-dlp.exe", "yt-dlp.cmd", "yt-dlp.bat", "yt-dlp"]
    } else {
        &["yt-dlp"]
    };

    if let Some(path_var) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path_var) {
            for name in candidates {
                let full = dir.join(name);
                if full.is_file() {
                    return Some(full);
                }
            }
        }
    }

    #[cfg(windows)]
    {
        if let Some(local_app) = dirs::data_local_dir() {
            let winget_link = local_app
                .join("Microsoft")
                .join("WinGet")
                .join("Links")
                .join("yt-dlp.exe");
            if winget_link.is_file() {
                return Some(winget_link);
            }
            let packages_dir = local_app
                .join("Microsoft")
                .join("WinGet")
                .join("Packages");
            if let Ok(entries) = std::fs::read_dir(&packages_dir) {
                for entry in entries.flatten() {
                    let name = entry.file_name();
                    if name.to_string_lossy().starts_with("yt-dlp.yt-dlp") {
                        let bin = entry.path().join("yt-dlp.exe");
                        if bin.is_file() {
                            return Some(bin);
                        }
                    }
                }
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

/// Extracts YouTube's `visitorData` token (starting with `Cg...`) from `sw.js_data` JSON payload.
pub fn extract_visitor_data_from_sw_js(body: &str) -> Option<String> {
    let bytes = body.as_bytes();
    let mut i = 0;
    while i + 4 < bytes.len() {
        if bytes[i] == b'"' && bytes[i + 1] == b'C' && bytes[i + 2] == b'g' {
            let start = i + 1;
            let mut end = start + 2;
            while end < bytes.len() && bytes[end] != b'"' {
                end += 1;
            }
            if end < bytes.len() && end - start >= 12 {
                let candidate = &body[start..end];
                if candidate
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'%'))
                {
                    return Some(candidate.to_string());
                }
            }
            i = end;
        }
        i += 1;
    }
    None
}

/// Parses InnerTube `/youtubei/v1/player` JSON into direct `AudioStreamCandidate`s.
///
/// Returns `Ok(candidates)` when `playabilityStatus.status == "OK"`, or `Err((retryable, reason))`
/// when YouTube rejects playback (e.g. stale `visitorData` returning `UNPLAYABLE`/`LOGIN_REQUIRED`).
pub fn parse_innertube_audio_candidates(
    json: &serde_json::Value,
) -> Result<Vec<AudioStreamCandidate>, (bool, String)> {
    let status = json
        .pointer("/playabilityStatus/status")
        .and_then(|v| v.as_str())
        .unwrap_or("UNKNOWN");
    if status != "OK" {
        let reason = json
            .pointer("/playabilityStatus/reason")
            .and_then(|v| v.as_str())
            .unwrap_or("unplayable");
        let retryable = matches!(status, "UNPLAYABLE" | "LOGIN_REQUIRED");
        return Err((retryable, format!("InnerTube status={status}: {reason}")));
    }

    let Some(formats) = json
        .pointer("/streamingData/adaptiveFormats")
        .and_then(|v| v.as_array())
    else {
        return Err((false, "InnerTube response missing adaptiveFormats".into()));
    };

    let mut out = Vec::new();
    for fmt in formats {
        let Some(url) = fmt.get("url").and_then(|v| v.as_str()) else {
            continue;
        };
        if url.trim().is_empty() {
            continue;
        }
        let mime = fmt
            .get("mimeType")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        let mime_lower = mime.to_ascii_lowercase();
        if !mime_lower.starts_with("audio/") {
            continue;
        }
        let is_m4a_aac = mime_lower.contains("audio/mp4") || mime_lower.contains("mp4a");
        let is_opus = mime_lower.contains("opus");
        let bitrate = fmt
            .get("bitrate")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;
        let size = fmt
            .get("contentLength")
            .and_then(|v| {
                v.as_str()
                    .and_then(|s| s.parse::<u64>().ok())
                    .or_else(|| v.as_u64())
            })
            .unwrap_or(0);
        let is_original_track = fmt
            .get("audioTrack")
            .map(|t| {
                t.get("audioIsDefault")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
                    || t.get("displayName")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_ascii_lowercase().contains("original"))
                        .unwrap_or(false)
            })
            .unwrap_or(true);
        let is_drm = fmt
            .get("drmFamilies")
            .and_then(|v| v.as_array())
            .map(|a| !a.is_empty())
            .unwrap_or(false);

        out.push(AudioStreamCandidate {
            url: url.to_string(),
            bitrate,
            is_m4a_aac,
            is_opus,
            is_original_track,
            is_drm,
            size,
        });
    }

    Ok(out)
}

fn compute_stream_cache_expiry(url: &str, now: Instant) -> Instant {
    if let Ok(parsed) = url::Url::parse(url) {
        if let Some((_, expire_str)) = parsed.query_pairs().find(|(k, _)| k == "expire") {
            if let Ok(expire_epoch) = expire_str.parse::<u64>() {
                let now_epoch = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                if expire_epoch > now_epoch.saturating_add(120) {
                    let remaining = Duration::from_secs(expire_epoch - now_epoch - 120);
                    return now + remaining.min(STREAM_CACHE_MAX_TTL);
                }
            }
        }
    }
    now + STREAM_CACHE_DEFAULT_TTL
}

#[derive(Debug, Clone)]
struct CachedVisitorData {
    value: String,
    fetched_at: Instant,
}

#[derive(Debug, Clone)]
struct CachedStreamUrl {
    url: String,
    expires_at: Instant,
}

/// Anonymous YouTube Music catalog & stream provider powered by direct InnerTube (`VISIONOS`),
/// `rustypipe`, and `yt-dlp` fallback, with session `visitorData` and stream URL caching.
#[derive(Clone)]
pub struct YouTubeMusicProvider {
    client: RustyPipe,
    http: reqwest::Client,
    yt_dlp_path: Option<PathBuf>,
    yt_dlp_resolved: Option<PathBuf>,
    visitor_data: Arc<RwLock<Option<CachedVisitorData>>>,
    stream_cache: Arc<Mutex<HashMap<String, CachedStreamUrl>>>,
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
            .connect_timeout(Duration::from_secs(6))
            .timeout(Duration::from_secs(12))
            .pool_max_idle_per_host(8)
            .build()
            .map_err(|e| CoreError::Network(format!("http client init failed: {e}")))?;
        let yt_dlp_resolved = detect_yt_dlp_binary(yt_dlp_path.as_deref());
        Ok(Self {
            client,
            http,
            yt_dlp_path,
            yt_dlp_resolved,
            visitor_data: Arc::new(RwLock::new(None)),
            stream_cache: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    /// Pre-warms YouTube `visitorData` in the background so the very first song click avoids
    /// an extra roundtrip.
    pub async fn warm_up(&self) {
        let _ = self.get_visitor_data(false).await;
    }

    /// Removes a cached stream URL for `video_id` (e.g. after an expired or HTTP 403 error).
    pub fn invalidate_stream_cache(&self, video_id: &str) {
        if let Ok(mut cache) = self.stream_cache.lock() {
            cache.remove(video_id.trim());
        }
    }

    /// Builds a `yt-dlp` progressive fallback handle for `video_id` when available.
    pub fn yt_dlp_fallback_for_video_id(&self, video_id: &str) -> Option<ResolvedStream> {
        let trimmed = video_id.trim();
        if trimmed.is_empty() {
            return None;
        }
        let binary = self
            .yt_dlp_resolved
            .clone()
            .or_else(|| detect_yt_dlp_binary(self.yt_dlp_path.as_deref()))?;
        Some(ResolvedStream::YtDlpProcess {
            binary,
            watch_url: format!("https://music.youtube.com/watch?v={trimmed}"),
            format_selector: YT_DLP_FORMAT_SELECTOR.to_string(),
            source: PlaybackSourceUsed::YouTubeDirect,
        })
    }

    fn get_cached_stream_url(&self, video_id: &str) -> Option<String> {
        let now = Instant::now();
        let mut cache = self.stream_cache.lock().ok()?;
        if let Some(entry) = cache.get(video_id) {
            if entry.expires_at > now {
                return Some(entry.url.clone());
            }
        }
        cache.remove(video_id);
        None
    }

    fn put_cached_stream_url(&self, video_id: &str, url: &str) {
        let now = Instant::now();
        let expires_at = compute_stream_cache_expiry(url, now);
        if let Ok(mut cache) = self.stream_cache.lock() {
            if cache.len() >= STREAM_CACHE_MAX_ENTRIES {
                cache.retain(|_, v| v.expires_at > now);
                if cache.len() >= STREAM_CACHE_MAX_ENTRIES {
                    if let Some(oldest_key) = cache
                        .iter()
                        .min_by_key(|(_, v)| v.expires_at)
                        .map(|(k, _)| k.clone())
                    {
                        cache.remove(&oldest_key);
                    }
                }
            }
            cache.insert(
                video_id.to_string(),
                CachedStreamUrl {
                    url: url.to_string(),
                    expires_at,
                },
            );
        }
    }

    async fn get_visitor_data(&self, force_refresh: bool) -> Option<String> {
        if !force_refresh {
            let guard = self.visitor_data.read().await;
            if let Some(cached) = guard.as_ref() {
                if cached.fetched_at.elapsed() < VISITOR_DATA_TTL {
                    return Some(cached.value.clone());
                }
            }
        }

        let resp = self
            .http
            .get(YOUTUBE_SW_JS_DATA_URL)
            .header(reqwest::header::USER_AGENT, "Mozilla/5.0")
            .send()
            .await
            .ok()?;
        if !resp.status().is_success() {
            return None;
        }
        let body = resp.text().await.ok()?;
        let token = extract_visitor_data_from_sw_js(&body)?;
        let mut guard = self.visitor_data.write().await;
        *guard = Some(CachedVisitorData {
            value: token.clone(),
            fetched_at: Instant::now(),
        });
        Some(token)
    }

    async fn call_innertube_visionos(
        &self,
        video_id: &str,
        visitor_data: Option<&str>,
    ) -> Result<serde_json::Value, CoreError> {
        let mut client_obj = serde_json::json!({
            "clientName": "VISIONOS",
            "clientVersion": VISIONOS_CLIENT_VERSION,
            "osName": "visionOS",
            "osVersion": "1.0",
            "hl": "en",
            "gl": "US",
        });
        if let Some(vd) = visitor_data {
            client_obj["visitorData"] = serde_json::Value::String(vd.to_string());
        }
        let payload = serde_json::json!({
            "videoId": video_id,
            "context": {
                "client": client_obj,
            },
            "contentCheckOk": true,
            "racyCheckOk": true,
        });

        let mut req = self
            .http
            .post(INNERTUBE_PLAYER_URL)
            .header(reqwest::header::USER_AGENT, VISIONOS_USER_AGENT)
            .header("X-YouTube-Client-Name", VISIONOS_CLIENT_NAME_HEADER)
            .header("X-YouTube-Client-Version", VISIONOS_CLIENT_VERSION)
            .json(&payload);
        if let Some(vd) = visitor_data {
            req = req.header("X-Goog-Visitor-Id", vd);
        }

        let resp = req
            .send()
            .await
            .map_err(|e| CoreError::Network(format!("InnerTube VISIONOS request failed: {e}")))?;
        let status = resp.status();
        if !status.is_success() {
            return Err(CoreError::ProviderUnavailable(format!(
                "InnerTube VISIONOS HTTP {}",
                status.as_u16()
            )));
        }
        resp.json::<serde_json::Value>()
            .await
            .map_err(|e| CoreError::Network(format!("InnerTube VISIONOS JSON decode failed: {e}")))
    }

    /// Fast direct InnerTube `VISIONOS` player API resolution (~120-200 ms, returns direct
    /// non-PO-token `audio/mp4` CDN URL without spawning `yt-dlp`).
    async fn resolve_via_innertube_visionos(&self, video_id: &str) -> Result<String, CoreError> {
        let vd = self.get_visitor_data(false).await;
        let json = self
            .call_innertube_visionos(video_id, vd.as_deref())
            .await?;

        match parse_innertube_audio_candidates(&json) {
            Ok(candidates) => select_best_audio_stream(&candidates).map(|b| b.url.clone()),
            Err((true, _)) => {
                // Retry once with a fresh visitorData token in case the cached token expired.
                let fresh_vd = self.get_visitor_data(true).await;
                let retry_json = self
                    .call_innertube_visionos(video_id, fresh_vd.as_deref())
                    .await?;
                let candidates = parse_innertube_audio_candidates(&retry_json)
                    .map_err(|(_, msg)| CoreError::ProviderUnavailable(msg))?;
                select_best_audio_stream(&candidates).map(|b| b.url.clone())
            }
            Err((false, msg)) => Err(CoreError::ProviderUnavailable(msg)),
        }
    }

    /// Lightweight plain-`GET` header probe (no `Range` header, body dropped immediately).
    ///
    /// YouTube's PO-token-restricted `c=IOS` URLs return `206` for tiny byte-range probes
    /// but immediately return `403 Forbidden` on a plain full-stream `GET` (and after 1 MiB
    /// of chunked transfer). Checking plain `GET` status catches the PO-token block in ~45 ms
    /// so playback immediately falls back to `yt-dlp`.
    async fn probe_stream_url(&self, url: &str) -> Result<(), CoreError> {
        let resp = self
            .http
            .get(url)
            .send()
            .await
            .map_err(|e| CoreError::Network(format!("stream probe failed: {e}")))?;
        let status = resp.status();
        if status.is_success() {
            Ok(())
        } else {
            Err(CoreError::ProviderUnavailable(format!(
                "YouTube stream rejected with HTTP {} (PO token required)",
                status.as_u16()
            )))
        }
    }

    async fn resolve_via_rustypipe(&self, video_id: &str, has_yt_dlp: bool) -> Result<String, CoreError> {
        // Prefer Ios first (no JS signature deobfuscation needed, ~180ms).
        // When yt-dlp is installed, avoid burning ~15s on Tv/Android/Desktop deobf failures if Ios is PO-blocked.
        let clients: &[ClientType] = if has_yt_dlp {
            &[ClientType::Ios]
        } else {
            &[
                ClientType::Ios,
                ClientType::Tv,
                ClientType::Android,
                ClientType::Desktop,
            ]
        };
        let mut last_err = CoreError::ProviderUnavailable("no rustypipe client succeeded".into());

        for &client_type in clients {
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

    /// Resolve a playable audio stream for a YouTube video ID:
    /// 1. In-memory stream URL cache (0 ms hit)
    /// 2. Direct InnerTube `VISIONOS` player API (~120-200 ms, direct M4A CDN URL)
    /// 3. `rustypipe` fallback
    /// 4. `yt-dlp` progressive pipe fallback
    pub async fn resolve_stream_for_video_id(
        &self,
        video_id: &str,
    ) -> Result<ResolvedStream, CoreError> {
        let trimmed = video_id.trim();
        if trimmed.is_empty() {
            return Err(CoreError::InvalidInput("empty YouTube video_id".into()));
        }

        if let Some(cached_url) = self.get_cached_stream_url(trimmed) {
            return Ok(ResolvedStream::HttpUrl {
                url: cached_url,
                source: PlaybackSourceUsed::YouTubeDirect,
            });
        }

        if let Ok(url) = self.resolve_via_innertube_visionos(trimmed).await {
            self.put_cached_stream_url(trimmed, &url);
            return Ok(ResolvedStream::HttpUrl {
                url,
                source: PlaybackSourceUsed::YouTubeDirect,
            });
        }

        let yt_dlp_bin = self
            .yt_dlp_resolved
            .clone()
            .or_else(|| detect_yt_dlp_binary(self.yt_dlp_path.as_deref()));
        match self.resolve_via_rustypipe(trimmed, yt_dlp_bin.is_some()).await {
            Ok(url) => {
                self.put_cached_stream_url(trimmed, &url);
                Ok(ResolvedStream::HttpUrl {
                    url,
                    source: PlaybackSourceUsed::YouTubeDirect,
                })
            }
            Err(primary_err) => {
                if let Some(binary) = yt_dlp_bin {
                    Ok(ResolvedStream::YtDlpProcess {
                        binary,
                        watch_url: format!("https://music.youtube.com/watch?v={trimmed}"),
                        format_selector: YT_DLP_FORMAT_SELECTOR.to_string(),
                        source: PlaybackSourceUsed::YouTubeDirect,
                    })
                } else {
                    Err(CoreError::ProviderUnavailable(format!(
                        "YouTube stream blocked ({primary_err}). Install yt-dlp (`winget install yt-dlp.yt-dlp`) or configure yt-dlp.exe in Settings."
                    )))
                }
            }
        }
    }

    /// Search YouTube Music videos (official music videos, official lyric videos, and uploads)
    /// as a fallback when a studio track is not distributed under YouTube Music's Songs catalog.
    pub async fn search_videos(&self, query: &str, limit: usize) -> Result<Vec<Track>, CoreError> {
        let trimmed = query.trim();
        if trimmed.is_empty() || limit == 0 {
            return Ok(Vec::new());
        }

        let res = self
            .client
            .query()
            .music_search_videos(trimmed)
            .await
            .map_err(|e| CoreError::Network(format!("YouTube Music video search failed: {e}")))?;

        let tracks = res
            .items
            .items
            .into_iter()
            .take(limit)
            .map(map_track_item)
            .collect();
        Ok(tracks)
    }

    /// Searches both YouTube Music Songs and YouTube Music Videos concurrently for user-facing
    /// Explore search so songs distributed only as official videos (e.g. *Waiting Room* by
    /// Phoebe Bridgers or *Pelangi* by HiVi!) still appear in search results.
    pub async fn search_catalog(
        &self,
        query: &str,
        song_limit: usize,
        video_limit: usize,
    ) -> Result<Vec<Track>, CoreError> {
        let trimmed = query.trim();
        if trimmed.is_empty() || (song_limit == 0 && video_limit == 0) {
            return Ok(Vec::new());
        }

        let (songs_res, videos_res) = tokio::join!(
            self.search(trimmed, song_limit),
            self.search_videos(trimmed, video_limit)
        );

        let mut merged = Vec::with_capacity(song_limit + video_limit);
        let mut seen = std::collections::HashSet::with_capacity(song_limit + video_limit);
        let mut first_err: Option<CoreError> = None;

        match songs_res {
            Ok(songs) => {
                for t in songs {
                    if seen.insert(t.uid.clone()) {
                        merged.push(t);
                    }
                }
            }
            Err(e) => first_err = Some(e),
        }

        match videos_res {
            Ok(videos) => {
                for t in videos {
                    if seen.insert(t.uid.clone()) {
                        merged.push(t);
                    }
                }
            }
            Err(e) => {
                if first_err.is_none() {
                    first_err = Some(e);
                }
            }
        }

        if merged.is_empty() {
            if let Some(err) = first_err {
                return Err(err);
            }
        }
        Ok(merged)
    }

    /// Fetches trending / top chart tracks from YouTube Music for Indonesia (`Country::Id`) or
    /// Global (`Country::Zz`), falling back gracefully to a curated regional chart search if the
    /// charts endpoint returns no items.
    pub async fn fetch_charts(
        &self,
        indonesia: bool,
        limit: usize,
    ) -> Result<Vec<Track>, CoreError> {
        if limit == 0 {
            return Ok(Vec::new());
        }

        let country = if indonesia {
            rustypipe::param::Country::Id
        } else {
            rustypipe::param::Country::Zz
        };

        let mut tracks = Vec::with_capacity(limit);
        let mut seen = std::collections::HashSet::with_capacity(limit);

        if let Ok(charts) = self.client.query().music_charts(Some(country)).await {
            for item in charts
                .top_tracks
                .into_iter()
                .chain(charts.trending_tracks.into_iter())
            {
                let t = map_track_item(item);
                if seen.insert(t.uid.clone()) {
                    tracks.push(t);
                    if tracks.len() >= limit {
                        return Ok(tracks);
                    }
                }
            }
        }

        if !tracks.is_empty() {
            return Ok(tracks);
        }

        // Graceful fallback if YouTube Music's charts browse layout returns empty items.
        let fallback_query = if indonesia {
            "lagu hits indonesia terbaru"
        } else {
            "top global hits songs"
        };
        self.search(fallback_query, limit).await
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

    #[test]
    fn extracts_visitor_data_token_from_sw_js_payload() {
        let payload = r#")]}'
[["something",null,"CgtHTTZENnkyanpyVSir0pTYBjIKCgJJRBIEGgAgZEgB%3D","other"]]"#;
        assert_eq!(
            extract_visitor_data_from_sw_js(payload).as_deref(),
            Some("CgtHTTZENnkyanpyVSir0pTYBjIKCgJJRBIEGgAgZEgB%3D")
        );
        assert_eq!(extract_visitor_data_from_sw_js(r#"{"empty": true}"#), None);
    }

    #[test]
    fn parses_innertube_adaptive_audio_formats_and_flags_retryable_status() {
        let ok_json = serde_json::json!({
            "playabilityStatus": { "status": "OK" },
            "streamingData": {
                "adaptiveFormats": [
                    {
                        "itag": 251,
                        "url": "https://rr1.googlevideo.com/videoplayback?itag=251",
                        "mimeType": "audio/webm; codecs=\"opus\"",
                        "bitrate": 160000,
                        "contentLength": "3500000"
                    },
                    {
                        "itag": 140,
                        "url": "https://rr1.googlevideo.com/videoplayback?itag=140",
                        "mimeType": "audio/mp4; codecs=\"mp4a.40.2\"",
                        "bitrate": 130677,
                        "contentLength": "3449447"
                    }
                ]
            }
        });
        let cands = parse_innertube_audio_candidates(&ok_json).expect("should parse OK");
        assert_eq!(cands.len(), 2);
        let best = select_best_audio_stream(&cands).expect("should pick m4a 140");
        assert_eq!(best.url, "https://rr1.googlevideo.com/videoplayback?itag=140");
        assert_eq!(best.size, 3_449_447);

        let bot_json = serde_json::json!({
            "playabilityStatus": {
                "status": "UNPLAYABLE",
                "reason": "Sign in to confirm you're not a bot"
            }
        });
        let (retryable, _) = parse_innertube_audio_candidates(&bot_json).unwrap_err();
        assert!(retryable);
    }

    #[test]
    fn stream_cache_hits_and_invalidates_cleanly() {
        let dir = tempfile::TempDir::new().unwrap();
        let provider = YouTubeMusicProvider::new(dir.path().to_path_buf(), None).unwrap();
        assert!(provider.get_cached_stream_url("abc123").is_none());

        provider.put_cached_stream_url("abc123", "https://rr1.googlevideo.com/audio.m4a");
        assert_eq!(
            provider.get_cached_stream_url("abc123").as_deref(),
            Some("https://rr1.googlevideo.com/audio.m4a")
        );

        provider.invalidate_stream_cache("abc123");
        assert!(provider.get_cached_stream_url("abc123").is_none());
    }

    #[tokio::test]
    #[ignore = "live network latency check"]
    async fn live_visionos_resolve_latency() {
        let dir = tempfile::TempDir::new().unwrap();
        let provider = YouTubeMusicProvider::new(dir.path().to_path_buf(), None).unwrap();

        let t_warm = Instant::now();
        provider.warm_up().await;
        println!("warm_up: {} ms", t_warm.elapsed().as_millis());

        for vid in ["JGwWNGJdvx8", "kJQP7kiw5Fk"] {
            let t0 = Instant::now();
            let resolved = provider.resolve_stream_for_video_id(vid).await.unwrap();
            let elapsed = t0.elapsed().as_millis();
            match &resolved {
                ResolvedStream::HttpUrl { url, .. } => {
                    println!("{vid}: HttpUrl in {elapsed} ms (len={})", url.len());
                }
                other => panic!("expected fast HttpUrl, got {other:?}"),
            }

            let t_cache = Instant::now();
            let cached = provider.resolve_stream_for_video_id(vid).await.unwrap();
            assert!(matches!(cached, ResolvedStream::HttpUrl { .. }));
            println!("{vid}: cached hit in {} us", t_cache.elapsed().as_micros());
        }
    }

    #[tokio::test]
    #[ignore = "live network charts check"]
    async fn live_fetch_home_charts_indonesia_and_global() {
        let dir = tempfile::TempDir::new().unwrap();
        let provider = YouTubeMusicProvider::new(dir.path().to_path_buf(), None).unwrap();

        let ytm_id = provider.fetch_charts(true, 10).await.unwrap();
        let ytm_gl = provider.fetch_charts(false, 10).await.unwrap();
        let sp_id = crate::spotify::fetch_spotify_chart_tracks(true, 10).await.unwrap();
        let sp_gl = crate::spotify::fetch_spotify_chart_tracks(false, 10).await.unwrap();

        println!(
            "YTM ID ({}): {:?}",
            ytm_id.len(),
            ytm_id.first().map(|t| (&t.title, t.artist_display()))
        );
        println!(
            "YTM Global ({}): {:?}",
            ytm_gl.len(),
            ytm_gl.first().map(|t| (&t.title, t.artist_display()))
        );
        println!(
            "Spotify ID ({}): {:?}",
            sp_id.len(),
            sp_id.first().map(|t| (&t.title, t.artist_display()))
        );
        println!(
            "Spotify Global ({}): {:?}",
            sp_gl.len(),
            sp_gl.first().map(|t| (&t.title, t.artist_display()))
        );

        assert!(!ytm_id.is_empty());
        assert!(!ytm_gl.is_empty());
        assert!(!sp_id.is_empty());
        assert!(!sp_gl.is_empty());
    }
}
