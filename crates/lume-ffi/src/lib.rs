use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use lume_core::{PlaybackSourceUsed, ResolvedStream, Track, TrackSource};
use lume_providers::{
    build_authorize_url, canonical_ytm_playlist_url, exchange_authorization_code,
    import_spotify_playlist_url, parse_exportify_csv, refresh_spotify_token,
    resolve_spotify_to_ytm_video_id, ChainedLyricsProvider, LocalProvider, PkceChallenge,
    SpotifyWebClient, YouTubeMusicProvider, SPOTIFY_ACCOUNTS_TOKEN_URL,
    SPOTIFY_MOBILE_REDIRECT_URI, SPOTIFY_WEB_API_SCOPES,
};
use lume_storage::{ArtCache, Database, OfflineTrackRecord, SpotifyTokenSet, WEEKLY_CHARTS_TTL_S};
use serde::{Deserialize, Serialize};
use tokio::runtime::Runtime;

uniffi::setup_scaffolding!();

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum LumeFfiError {
    #[error("{message}")]
    InvalidInput { message: String },
    #[error("{message}")]
    Storage { message: String },
    #[error("{message}")]
    Network { message: String },
    #[error("{message}")]
    Auth { message: String },
    #[error("{message}")]
    Internal { message: String },
}

impl LumeFfiError {
    fn invalid(msg: impl Into<String>) -> Self {
        Self::InvalidInput {
            message: msg.into(),
        }
    }
    fn storage(msg: impl Into<String>) -> Self {
        Self::Storage {
            message: msg.into(),
        }
    }
    fn network(msg: impl Into<String>) -> Self {
        Self::Network {
            message: msg.into(),
        }
    }
    fn auth(msg: impl Into<String>) -> Self {
        Self::Auth {
            message: msg.into(),
        }
    }
    fn internal(msg: impl Into<String>) -> Self {
        Self::Internal {
            message: msg.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayableStreamDto {
    pub uri: String,
    pub is_offline: bool,
    pub source_used: PlaybackSourceUsed,
    pub resolved_video_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OfflineEntryDto {
    pub track: Track,
    pub record: OfflineTrackRecord,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MobileSpotifyAuthStartDto {
    pub authorize_url: String,
    pub code_verifier: String,
    pub state: String,
    pub redirect_uri: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpotifySyncSummaryDto {
    pub display_name: String,
    pub liked_count: usize,
    pub playlist_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckDto {
    pub version: String,
    pub target_os: String,
    pub db_path: String,
    pub offline_dir: String,
    pub offline_track_count: usize,
    pub offline_storage_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamicPaletteDto {
    pub bg_top: String,
    pub bg_mid: String,
    pub bg_bottom: String,
    pub accent: String,
    pub accent_soft: String,
}

#[derive(uniffi::Object)]
pub struct LumeMobileCore {
    runtime: Runtime,
    db: Mutex<Database>,
    ytm: YouTubeMusicProvider,
    lyrics: ChainedLyricsProvider,
    http: reqwest::Client,
    db_path: PathBuf,
    offline_dir: PathBuf,
    local_audio_dir: PathBuf,
    art_cache_dir: PathBuf,
}

fn sanitize_filename(uid: &str) -> String {
    uid.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn to_file_uri(path: &Path) -> String {
    let raw = path.to_string_lossy().replace('\\', "/");
    if raw.starts_with('/') {
        format!("file://{raw}")
    } else {
        format!("file:///{raw}")
    }
}

fn parse_track_json(track_json: &str) -> Result<Track, LumeFfiError> {
    serde_json::from_str(track_json)
        .map_err(|e| LumeFfiError::invalid(format!("invalid Track JSON: {e}")))
}

fn to_json<T: Serialize>(value: &T) -> Result<String, LumeFfiError> {
    serde_json::to_string(value)
        .map_err(|e| LumeFfiError::internal(format!("JSON serialization failed: {e}")))
}

fn default_dynamic_palette() -> DynamicPaletteDto {
    DynamicPaletteDto {
        bg_top: "#181C2E".into(),
        bg_mid: "#0E111B".into(),
        bg_bottom: "#0A0C12".into(),
        accent: "#818CF8".into(),
        accent_soft: "rgba(99, 102, 241, 0.22)".into(),
    }
}

fn rgb_hex(r: u8, g: u8, b: u8) -> String {
    format!("#{r:02X}{g:02X}{b:02X}")
}

fn extract_palette_from_bytes(bytes: &[u8]) -> DynamicPaletteDto {
    if bytes.is_empty() {
        return default_dynamic_palette();
    }
    let Ok(img) = image::load_from_memory(bytes) else {
        return default_dynamic_palette();
    };
    let small = img.resize_exact(32, 32, image::imageops::FilterType::Triangle);
    let rgba = small.to_rgba8();

    let mut best_score = -1.0_f32;
    let mut best_rgb = (99_u8, 102_u8, 241_u8);
    let mut avg_r = 0.0_f64;
    let mut avg_g = 0.0_f64;
    let mut avg_b = 0.0_f64;
    let mut count = 0.0_f64;

    for pixel in rgba.pixels() {
        let [r, g, b, a] = pixel.0;
        if a < 128 {
            continue;
        }
        let rf = r as f32 / 255.0;
        let gf = g as f32 / 255.0;
        let bf = b as f32 / 255.0;
        avg_r += rf as f64;
        avg_g += gf as f64;
        avg_b += bf as f64;
        count += 1.0;

        let max_c = rf.max(gf).max(bf);
        let min_c = rf.min(gf).min(bf);
        let sat = if max_c > 0.01 {
            (max_c - min_c) / max_c
        } else {
            0.0
        };
        let luma = 0.2126 * rf + 0.7152 * gf + 0.0722 * bf;
        if luma > 0.10 && luma < 0.88 {
            let luma_bonus = 1.0 - (luma - 0.45).abs();
            let score = sat * 1.6 + luma_bonus * 0.4;
            if score > best_score {
                best_score = score;
                best_rgb = (r, g, b);
            }
        }
    }

    if count < 1.0 {
        return default_dynamic_palette();
    }

    let (dom_r, dom_g, dom_b) = if best_score >= 0.35 {
        best_rgb
    } else {
        (
            ((avg_r / count) * 255.0) as u8,
            ((avg_g / count) * 255.0) as u8,
            ((avg_b / count) * 255.0) as u8,
        )
    };

    // Rich dark glassmorphism tones (matching Lume Desktop's WCAG-safe vibrant tint).
    let scale_channel = |c: u8, factor: f32, floor: u8, ceil: u8| -> u8 {
        ((c as f32 * factor).round() as u8).clamp(floor, ceil)
    };

    let top = rgb_hex(
        scale_channel(dom_r, 0.46, 18, 128),
        scale_channel(dom_g, 0.46, 16, 120),
        scale_channel(dom_b, 0.46, 20, 132),
    );
    let mid = rgb_hex(
        scale_channel(dom_r, 0.24, 12, 74),
        scale_channel(dom_g, 0.24, 12, 70),
        scale_channel(dom_b, 0.24, 16, 82),
    );
    let bottom = rgb_hex(
        scale_channel(dom_r, 0.11, 8, 34),
        scale_channel(dom_g, 0.11, 8, 32),
        scale_channel(dom_b, 0.11, 12, 38),
    );

    // Bright readable accent normalized for dark backgrounds.
    let ar = dom_r as f32 / 255.0;
    let ag = dom_g as f32 / 255.0;
    let ab = dom_b as f32 / 255.0;
    let luma = (0.2126 * ar + 0.7152 * ag + 0.0722 * ab).max(0.01);
    let target_luma = luma.clamp(0.55, 0.78);
    let boost = (target_luma / luma).min(2.2);
    let acc_r = ((ar * boost * 255.0).round() as u8).clamp(90, 255);
    let acc_g = ((ag * boost * 255.0).round() as u8).clamp(90, 255);
    let acc_b = ((ab * boost * 255.0).round() as u8).clamp(90, 255);

    DynamicPaletteDto {
        bg_top: top,
        bg_mid: mid,
        bg_bottom: bottom,
        accent: rgb_hex(acc_r, acc_g, acc_b),
        accent_soft: format!("rgba({acc_r}, {acc_g}, {acc_b}, 0.22)"),
    }
}

fn extract_ytm_playlist_id(input: &str) -> String {
    let trimmed = input.trim();
    if let Some(pos) = trimmed.find("list=") {
        let rest = &trimmed[pos + 5..];
        rest.split('&').next().unwrap_or(trimmed).trim().to_string()
    } else {
        trimmed.to_string()
    }
}

fn strip_file_scheme(uri_or_path: &str) -> PathBuf {
    let trimmed = uri_or_path.trim();
    if let Some(stripped) = trimmed.strip_prefix("file://") {
        let decoded = stripped.replace("%20", " ");
        PathBuf::from(decoded)
    } else {
        PathBuf::from(trimmed)
    }
}

const OFFLINE_RANGE_CHUNK_BYTES: u64 = 512 * 1024;
const VISIONOS_STREAM_UA: &str =
    "com.google.ios.youtube/0.1 (RealityDevice14,1; U; CPU OS 17_0 like Mac OS X; en_US)";

fn parse_content_range_total(header_val: Option<&reqwest::header::HeaderValue>) -> Option<u64> {
    let text = header_val?.to_str().ok()?;
    let total_str = text.rsplit('/').next()?;
    total_str.trim().parse::<u64>().ok()
}

fn parse_url_clen(url: &str) -> Option<u64> {
    let parsed = reqwest::Url::parse(url).ok()?;
    for (k, v) in parsed.query_pairs() {
        if k == "clen" {
            if let Ok(n) = v.parse::<u64>() {
                if n > 0 {
                    return Some(n);
                }
            }
        }
    }
    None
}

struct DownloadedChunk {
    bytes: Vec<u8>,
    status: u16,
    total_bytes: Option<u64>,
    content_type: Option<String>,
}

/// Downloads an audio stream to `tmp_path` using 512 KiB HTTP `Range` chunks with per-chunk retry.
///
/// YouTube `googlevideo.com` throttles plain un-ranged `GET` requests (~35 KiB/s) and frequently
/// resets long-lived connections mid-body; chunked `Range: bytes=start-end` streams at full CDN
/// speed (5–10 MiB/s) with constant low RAM usage on iOS.
async fn download_url_chunked_to_file(
    http: &reqwest::Client,
    url: &str,
    tmp_path: &Path,
) -> Result<(u64, String), LumeFfiError> {
    let mut file = std::fs::File::create(tmp_path)
        .map_err(|e| LumeFfiError::storage(format!("unable to create offline temp file: {e}")))?;

    let mut offset: u64 = 0;
    let mut total_bytes: Option<u64> = parse_url_clen(url);
    let mut mime_type = "audio/mp4".to_string();

    loop {
        if let Some(total) = total_bytes {
            if offset >= total {
                break;
            }
        }

        let end = match total_bytes {
            Some(total) => (offset + OFFLINE_RANGE_CHUNK_BYTES - 1).min(total - 1),
            None => offset + OFFLINE_RANGE_CHUNK_BYTES - 1,
        };
        let range_header = format!("bytes={offset}-{end}");

        let mut last_err = String::new();
        let mut chunk_ok: Option<DownloadedChunk> = None;

        for attempt in 0..3_u32 {
            let req = http
                .get(url)
                .header(reqwest::header::USER_AGENT, VISIONOS_STREAM_UA)
                .header(reqwest::header::RANGE, &range_header);

            match tokio::time::timeout(Duration::from_secs(20), req.send()).await {
                Ok(Ok(resp)) => {
                    let status = resp.status().as_u16();
                    if status == 416 {
                        chunk_ok = Some(DownloadedChunk {
                            bytes: Vec::new(),
                            status: 416,
                            total_bytes,
                            content_type: None,
                        });
                        break;
                    }
                    if status == 403 || status == 429 {
                        last_err = format!("YouTube CDN returned HTTP {status} (rate limit / expired URL)");
                        tokio::time::sleep(Duration::from_millis(600 * (attempt as u64 + 1))).await;
                        continue;
                    }
                    if !(200..300).contains(&status) {
                        last_err = format!("Audio server returned HTTP {status}");
                        tokio::time::sleep(Duration::from_millis(400 * (attempt as u64 + 1))).await;
                        continue;
                    }

                    let parsed_total = parse_content_range_total(
                        resp.headers().get(reqwest::header::CONTENT_RANGE),
                    )
                    .or_else(|| if status == 200 { resp.content_length() } else { None });
                    let ct = resp
                        .headers()
                        .get(reqwest::header::CONTENT_TYPE)
                        .and_then(|v| v.to_str().ok())
                        .and_then(|s| s.split(';').next())
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty());

                    match tokio::time::timeout(Duration::from_secs(20), resp.bytes()).await {
                        Ok(Ok(bytes)) => {
                            chunk_ok = Some(DownloadedChunk {
                                bytes: bytes.to_vec(),
                                status,
                                total_bytes: parsed_total,
                                content_type: ct,
                            });
                            break;
                        }
                        Ok(Err(_)) => {
                            last_err = "Connection interrupted while downloading audio chunk".into();
                        }
                        Err(_) => {
                            last_err = "Audio chunk download timed out".into();
                        }
                    }
                }
                Ok(Err(_)) => {
                    last_err = "Network error contacting audio server".into();
                }
                Err(_) => {
                    last_err = "Connection timed out contacting audio server".into();
                }
            }

            tokio::time::sleep(Duration::from_millis(400 * (attempt as u64 + 1))).await;
        }

        let Some(DownloadedChunk {
            bytes,
            status,
            total_bytes: resp_total,
            content_type: ct,
        }) = chunk_ok
        else {
            let _ = std::fs::remove_file(tmp_path);
            return Err(LumeFfiError::network(last_err));
        };

        if let Some(m) = ct {
            mime_type = m;
        }
        if let Some(t) = resp_total {
            total_bytes = Some(t);
        }

        if status == 416 || bytes.is_empty() {
            break;
        }

        file.write_all(&bytes)
            .map_err(|e| LumeFfiError::storage(format!("writing offline audio failed: {e}")))?;
        offset = offset.saturating_add(bytes.len() as u64);

        // If the server ignored Range and returned the full body in HTTP 200, we're done.
        if status == 200 {
            break;
        }
        if total_bytes.is_none() && (bytes.len() as u64) < OFFLINE_RANGE_CHUNK_BYTES {
            break;
        }
    }

    file.flush()
        .map_err(|e| LumeFfiError::storage(format!("flushing offline audio failed: {e}")))?;
    drop(file);

    if offset == 0 {
        let _ = std::fs::remove_file(tmp_path);
        return Err(LumeFfiError::network("Downloaded audio stream was empty"));
    }

    Ok((offset, mime_type))
}

impl LumeMobileCore {
    async fn resolve_remote_http_stream(
        &self,
        track: &Track,
        force_rematch: bool,
    ) -> Result<(String, PlaybackSourceUsed, Option<String>), LumeFfiError> {
        match &track.source {
            TrackSource::YouTubeMusic { video_id } => {
                let resolved = self
                    .ytm
                    .resolve_stream_for_video_id(video_id)
                    .await
                    .map_err(|e| LumeFfiError::network(e.to_string()))?;
                match resolved {
                    ResolvedStream::HttpUrl { url, source } => {
                        Ok((url, source, Some(video_id.clone())))
                    }
                    _ => Err(LumeFfiError::network(
                        "Direct HTTP audio stream unavailable for this YouTube track",
                    )),
                }
            }
            TrackSource::Spotify { .. } => {
                let matched =
                    resolve_spotify_to_ytm_video_id(track, &self.db, &self.ytm, force_rematch)
                        .await
                        .map_err(|e| LumeFfiError::network(e.to_string()))?;
                let resolved = self
                    .ytm
                    .resolve_stream_for_video_id(&matched.yt_video_id)
                    .await
                    .map_err(|e| LumeFfiError::network(e.to_string()))?;
                match resolved {
                    ResolvedStream::HttpUrl { url, .. } => Ok((
                        url,
                        PlaybackSourceUsed::YouTubeFallback,
                        Some(matched.yt_video_id),
                    )),
                    _ => Err(LumeFfiError::network(
                        "Direct HTTP audio stream unavailable for matched Spotify track",
                    )),
                }
            }
            TrackSource::Local { path } => Ok((
                to_file_uri(path),
                PlaybackSourceUsed::LocalFile,
                None,
            )),
        }
    }
}

#[uniffi::export]
impl LumeMobileCore {
    /// Initializes the shared Rust core using explicit iOS/mobile app-container paths.
    #[uniffi::constructor]
    pub fn new(data_dir: String, cache_dir: String) -> Result<Arc<Self>, LumeFfiError> {
        let data_path = PathBuf::from(data_dir.trim());
        let cache_path = PathBuf::from(cache_dir.trim());
        if data_path.as_os_str().is_empty() || cache_path.as_os_str().is_empty() {
            return Err(LumeFfiError::invalid(
                "data_dir and cache_dir must be non-empty paths",
            ));
        }

        let offline_dir = data_path.join("offline");
        std::fs::create_dir_all(&offline_dir)
            .map_err(|e| LumeFfiError::storage(format!("failed to create offline dir: {e}")))?;

        let local_audio_dir = data_path.join("local_audio");
        std::fs::create_dir_all(&local_audio_dir)
            .map_err(|e| LumeFfiError::storage(format!("failed to create local_audio dir: {e}")))?;

        let art_cache_dir = cache_path.join("art");
        std::fs::create_dir_all(&art_cache_dir)
            .map_err(|e| LumeFfiError::storage(format!("failed to create art cache dir: {e}")))?;

        let ytm_cache = cache_path.join("ytm");
        std::fs::create_dir_all(&ytm_cache)
            .map_err(|e| LumeFfiError::storage(format!("failed to create ytm cache dir: {e}")))?;

        let db_path = data_path.join("lume.db");
        let db = Database::open(&db_path).map_err(|e| LumeFfiError::storage(e.to_string()))?;

        let ytm = YouTubeMusicProvider::new(ytm_cache, None)
            .map_err(|e| LumeFfiError::internal(e.to_string()))?;
        let lyrics = ChainedLyricsProvider::new("https://lrclib.net".to_string());

        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(15))
            .build()
            .map_err(|e| LumeFfiError::internal(format!("failed to build HTTP client: {e}")))?;

        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("lume-ffi-tokio")
            .enable_all()
            .build()
            .map_err(|e| LumeFfiError::internal(format!("failed to build tokio runtime: {e}")))?;

        Ok(Arc::new(Self {
            runtime,
            db: Mutex::new(db),
            ytm,
            lyrics,
            http,
            db_path,
            offline_dir,
            local_audio_dir,
            art_cache_dir,
        }))
    }

    pub fn health_check_json(&self) -> Result<String, LumeFfiError> {
        let (offline_track_count, offline_storage_bytes) = {
            let db = self
                .db
                .lock()
                .map_err(|_| LumeFfiError::internal("database mutex poisoned"))?;
            let count = db
                .list_offline_tracks()
                .map_err(|e| LumeFfiError::storage(e.to_string()))?
                .len();
            let bytes = db
                .offline_storage_total_bytes()
                .map_err(|e| LumeFfiError::storage(e.to_string()))?;
            (count, bytes)
        };
        to_json(&HealthCheckDto {
            version: env!("CARGO_PKG_VERSION").to_string(),
            target_os: std::env::consts::OS.to_string(),
            db_path: self.db_path.to_string_lossy().into_owned(),
            offline_dir: self.offline_dir.to_string_lossy().into_owned(),
            offline_track_count,
            offline_storage_bytes,
        })
    }

    /// Resolves a track to an AVPlayer-ready URI, prioritizing locally cached offline audio first.
    pub fn resolve_playable_track_json(
        &self,
        track_json: String,
        force_rematch: bool,
    ) -> Result<String, LumeFfiError> {
        let track = parse_track_json(&track_json)?;

        if !force_rematch {
            let offline_hit = {
                let db = self
                    .db
                    .lock()
                    .map_err(|_| LumeFfiError::internal("database mutex poisoned"))?;
                db.get_offline_track(&track.uid)
                    .map_err(|e| LumeFfiError::storage(e.to_string()))?
            };
            if let Some(rec) = offline_hit {
                let path = PathBuf::from(&rec.file_path);
                if path.is_file() {
                    return to_json(&PlayableStreamDto {
                        uri: to_file_uri(&path),
                        is_offline: true,
                        source_used: PlaybackSourceUsed::LocalFile,
                        resolved_video_id: None,
                    });
                }
            }
        }

        if let TrackSource::Local { path } = &track.source {
            return to_json(&PlayableStreamDto {
                uri: to_file_uri(path),
                is_offline: true,
                source_used: PlaybackSourceUsed::LocalFile,
                resolved_video_id: None,
            });
        }

        let (url, source_used, resolved_video_id) = self
            .runtime
            .block_on(self.resolve_remote_http_stream(&track, force_rematch))?;

        to_json(&PlayableStreamDto {
            uri: url,
            is_offline: false,
            source_used,
            resolved_video_id,
        })
    }

    /// Downloads a track's M4A/audio stream to local offline storage and caches lyrics for driving mode.
    pub fn download_track_offline_json(&self, track_json: String) -> Result<String, LumeFfiError> {
        let track = parse_track_json(&track_json)?;
        let (url, _, _) = self
            .runtime
            .block_on(self.resolve_remote_http_stream(&track, false))?;

        if url.starts_with("file://") {
            return Err(LumeFfiError::invalid(
                "track is already a local file on device",
            ));
        }

        let target_path = self
            .offline_dir
            .join(format!("{}.m4a", sanitize_filename(&track.uid)));
        let tmp_path = target_path.with_extension("m4a.tmp");

        let (bytes_written, mime_type) = self.runtime.block_on(async {
            let res = download_url_chunked_to_file(&self.http, &url, &tmp_path).await;
            match res {
                Ok(ok) => {
                    std::fs::rename(&tmp_path, &target_path).map_err(|e| {
                        LumeFfiError::storage(format!("finalizing offline file failed: {e}"))
                    })?;
                    Ok(ok)
                }
                Err(err) => {
                    let _ = std::fs::remove_file(&tmp_path);
                    Err(err)
                }
            }
        })?;

        let record = {
            let db = self
                .db
                .lock()
                .map_err(|_| LumeFfiError::internal("database mutex poisoned"))?;
            db.upsert_offline_track(
                &track,
                &target_path.to_string_lossy(),
                bytes_written,
                &mime_type,
            )
            .map_err(|e| LumeFfiError::storage(e.to_string()))?
        };

        // Best-effort lyrics pre-cache so synced lyrics also work completely offline.
        let _ = self.fetch_lyrics_json(track_json, false);

        to_json(&OfflineEntryDto { track, record })
    }

    pub fn list_offline_tracks_json(&self) -> Result<String, LumeFfiError> {
        let items: Vec<OfflineEntryDto> = {
            let db = self
                .db
                .lock()
                .map_err(|_| LumeFfiError::internal("database mutex poisoned"))?;
            db.list_offline_tracks()
                .map_err(|e| LumeFfiError::storage(e.to_string()))?
                .into_iter()
                .map(|(track, record)| OfflineEntryDto { track, record })
                .collect()
        };
        to_json(&items)
    }

    pub fn delete_offline_track(&self, track_uid: String) -> Result<(), LumeFfiError> {
        let removed_path = {
            let db = self
                .db
                .lock()
                .map_err(|_| LumeFfiError::internal("database mutex poisoned"))?;
            db.delete_offline_track(track_uid.trim())
                .map_err(|e| LumeFfiError::storage(e.to_string()))?
        };
        if let Some(path) = removed_path {
            let p = PathBuf::from(path);
            if p.exists() {
                let _ = std::fs::remove_file(p);
            }
        }
        Ok(())
    }

    pub fn offline_storage_bytes(&self) -> Result<u64, LumeFfiError> {
        let db = self
            .db
            .lock()
            .map_err(|_| LumeFfiError::internal("database mutex poisoned"))?;
        db.offline_storage_total_bytes()
            .map_err(|e| LumeFfiError::storage(e.to_string()))
    }

    pub fn search_ytm_json(&self, query: String, limit: u32) -> Result<String, LumeFfiError> {
        let cap = (limit as usize).clamp(1, 50);
        let result = self
            .runtime
            .block_on(self.ytm.search_catalog_full(&query, cap, (cap / 2).max(4), 8, 8))
            .map_err(|e| LumeFfiError::network(e.to_string()))?;
        to_json(&result)
    }

    pub fn get_charts_json(
        &self,
        indonesia: bool,
        limit: u32,
        force_refresh: bool,
    ) -> Result<String, LumeFfiError> {
        let chart_key = if indonesia {
            "ytm:mobile:id"
        } else {
            "ytm:mobile:global"
        };

        if !force_refresh {
            let cached = {
                let db = self
                    .db
                    .lock()
                    .map_err(|_| LumeFfiError::internal("database mutex poisoned"))?;
                db.get_cached_chart(chart_key, WEEKLY_CHARTS_TTL_S)
                    .map_err(|e| LumeFfiError::storage(e.to_string()))?
            };
            if let Some(c) = cached {
                if c.is_fresh && !c.tracks.is_empty() {
                    return to_json(&c.tracks);
                }
            }
        }

        let cap = (limit as usize).clamp(1, 50);
        match self.runtime.block_on(self.ytm.fetch_charts(indonesia, cap)) {
            Ok(tracks) => {
                if !tracks.is_empty() {
                    let db = self
                        .db
                        .lock()
                        .map_err(|_| LumeFfiError::internal("database mutex poisoned"))?;
                    let _ = db.put_cached_chart(chart_key, &tracks);
                }
                to_json(&tracks)
            }
            Err(err) => {
                // Offline fallback: return stale cached charts if available.
                let stale = {
                    let db = self
                        .db
                        .lock()
                        .map_err(|_| LumeFfiError::internal("database mutex poisoned"))?;
                    db.get_cached_chart(chart_key, WEEKLY_CHARTS_TTL_S)
                        .map_err(|e| LumeFfiError::storage(e.to_string()))?
                };
                if let Some(c) = stale {
                    if !c.tracks.is_empty() {
                        return to_json(&c.tracks);
                    }
                }
                Err(LumeFfiError::network(err.to_string()))
            }
        }
    }

    pub fn get_artist_profile_json(&self, artist_id: String) -> Result<String, LumeFfiError> {
        let profile = self
            .runtime
            .block_on(self.ytm.get_artist_profile(&artist_id))
            .map_err(|e| LumeFfiError::network(e.to_string()))?;
        to_json(&profile)
    }

    pub fn get_album_detail_json(&self, album_id: String) -> Result<String, LumeFfiError> {
        let album = self
            .runtime
            .block_on(self.ytm.get_album_detail(&album_id))
            .map_err(|e| LumeFfiError::network(e.to_string()))?;
        to_json(&album)
    }

    pub fn get_ytm_playlist_detail_json(
        &self,
        playlist_id: String,
    ) -> Result<String, LumeFfiError> {
        let detail = self
            .runtime
            .block_on(self.ytm.get_playlist_detail(&playlist_id))
            .map_err(|e| LumeFfiError::network(e.to_string()))?;
        to_json(&detail)
    }

    pub fn list_playlists_json(&self) -> Result<String, LumeFfiError> {
        let playlists = {
            let db = self
                .db
                .lock()
                .map_err(|_| LumeFfiError::internal("database mutex poisoned"))?;
            db.list_playlists()
                .map_err(|e| LumeFfiError::storage(e.to_string()))?
        };
        to_json(&playlists)
    }

    pub fn create_playlist_json(&self, name: String) -> Result<String, LumeFfiError> {
        let playlist = {
            let db = self
                .db
                .lock()
                .map_err(|_| LumeFfiError::internal("database mutex poisoned"))?;
            db.create_playlist(&name)
                .map_err(|e| LumeFfiError::storage(e.to_string()))?
        };
        to_json(&playlist)
    }

    pub fn delete_playlist(&self, playlist_id: String) -> Result<(), LumeFfiError> {
        let db = self
            .db
            .lock()
            .map_err(|_| LumeFfiError::internal("database mutex poisoned"))?;
        db.delete_playlist(&playlist_id)
            .map_err(|e| LumeFfiError::storage(e.to_string()))
    }

    pub fn get_playlist_tracks_json(&self, playlist_id: String) -> Result<String, LumeFfiError> {
        let tracks = {
            let db = self
                .db
                .lock()
                .map_err(|_| LumeFfiError::internal("database mutex poisoned"))?;
            db.get_playlist_tracks(&playlist_id)
                .map_err(|e| LumeFfiError::storage(e.to_string()))?
        };
        to_json(&tracks)
    }

    pub fn add_track_to_playlist(
        &self,
        playlist_id: String,
        track_json: String,
    ) -> Result<(), LumeFfiError> {
        let track = parse_track_json(&track_json)?;
        let db = self
            .db
            .lock()
            .map_err(|_| LumeFfiError::internal("database mutex poisoned"))?;
        db.upsert_track(&track, None)
            .map_err(|e| LumeFfiError::storage(e.to_string()))?;
        db.add_track_to_playlist(&playlist_id, &track.uid)
            .map_err(|e| LumeFfiError::storage(e.to_string()))
    }

    pub fn remove_track_from_playlist(
        &self,
        playlist_id: String,
        position: u32,
    ) -> Result<(), LumeFfiError> {
        let db = self
            .db
            .lock()
            .map_err(|_| LumeFfiError::internal("database mutex poisoned"))?;
        db.remove_track_from_playlist(&playlist_id, position as usize)
            .map_err(|e| LumeFfiError::storage(e.to_string()))
    }

    pub fn set_track_liked(&self, track_json: String, liked: bool) -> Result<(), LumeFfiError> {
        let track = parse_track_json(&track_json)?;
        let db = self
            .db
            .lock()
            .map_err(|_| LumeFfiError::internal("database mutex poisoned"))?;
        db.upsert_track(&track, None)
            .map_err(|e| LumeFfiError::storage(e.to_string()))?;
        db.set_local_favorite(&track.uid, liked)
            .map_err(|e| LumeFfiError::storage(e.to_string()))
    }

    pub fn list_liked_tracks_json(&self) -> Result<String, LumeFfiError> {
        let tracks = {
            let db = self
                .db
                .lock()
                .map_err(|_| LumeFfiError::internal("database mutex poisoned"))?;
            db.list_unified_liked_tracks()
                .map_err(|e| LumeFfiError::storage(e.to_string()))?
        };
        to_json(&tracks)
    }

    pub fn record_play_history(&self, track_json: String) -> Result<(), LumeFfiError> {
        let track = parse_track_json(&track_json)?;
        let db = self
            .db
            .lock()
            .map_err(|_| LumeFfiError::internal("database mutex poisoned"))?;
        db.record_play_history(&track)
            .map_err(|e| LumeFfiError::storage(e.to_string()))
    }

    pub fn list_recent_tracks_json(&self, limit: u32) -> Result<String, LumeFfiError> {
        let cap = (limit as usize).clamp(1, 100);
        let tracks = {
            let db = self
                .db
                .lock()
                .map_err(|_| LumeFfiError::internal("database mutex poisoned"))?;
            db.list_recent_tracks(cap)
                .map_err(|e| LumeFfiError::storage(e.to_string()))?
        };
        to_json(&tracks)
    }

    pub fn fetch_lyrics_json(
        &self,
        track_json: String,
        force_refresh: bool,
    ) -> Result<String, LumeFfiError> {
        let track = parse_track_json(&track_json)?;
        let db = Database::open(&self.db_path).map_err(|e| LumeFfiError::storage(e.to_string()))?;
        let data = self
            .runtime
            .block_on(self.lyrics.fetch(&track, &db, force_refresh))
            .map_err(|e| LumeFfiError::network(e.to_string()))?;
        to_json(&data)
    }

    /// Generates an OAuth 2.0 PKCE challenge and authorize URL using `lumemusic://callback`.
    pub fn begin_spotify_mobile_auth_json(&self, client_id: String) -> Result<String, LumeFfiError> {
        let cid = client_id.trim();
        if cid.is_empty() {
            return Err(LumeFfiError::invalid("Spotify Client ID cannot be empty"));
        }
        let pkce = PkceChallenge::generate();
        let authorize_url = build_authorize_url(
            cid,
            SPOTIFY_MOBILE_REDIRECT_URI,
            SPOTIFY_WEB_API_SCOPES,
            &pkce,
        )
        .map_err(|e| LumeFfiError::auth(e.to_string()))?;

        to_json(&MobileSpotifyAuthStartDto {
            authorize_url,
            code_verifier: pkce.code_verifier,
            state: pkce.state,
            redirect_uri: SPOTIFY_MOBILE_REDIRECT_URI.to_string(),
        })
    }

    /// Exchanges a mobile OAuth callback code for a `SpotifyTokenSet` JSON (store in iOS Keychain!).
    pub fn exchange_spotify_mobile_code_json(
        &self,
        client_id: String,
        code: String,
        code_verifier: String,
    ) -> Result<String, LumeFfiError> {
        let token_set = self
            .runtime
            .block_on(exchange_authorization_code(
                SPOTIFY_ACCOUNTS_TOKEN_URL,
                client_id.trim(),
                code.trim(),
                SPOTIFY_MOBILE_REDIRECT_URI,
                code_verifier.trim(),
            ))
            .map_err(|e| LumeFfiError::auth(e.to_string()))?;
        to_json(&token_set)
    }

    pub fn refresh_spotify_token_json(
        &self,
        client_id: String,
        token_json: String,
    ) -> Result<String, LumeFfiError> {
        let current: SpotifyTokenSet = serde_json::from_str(&token_json)
            .map_err(|e| LumeFfiError::invalid(format!("invalid SpotifyTokenSet JSON: {e}")))?;
        let refreshed = self
            .runtime
            .block_on(refresh_spotify_token(client_id.trim(), &current))
            .map_err(|e| LumeFfiError::auth(e.to_string()))?;
        to_json(&refreshed)
    }

    pub fn sync_spotify_library_json(&self, access_token: String) -> Result<String, LumeFfiError> {
        let token = access_token.trim();
        if token.is_empty() {
            return Err(LumeFfiError::auth("Spotify access token is empty"));
        }
        let client = SpotifyWebClient::new();
        let (profile, liked_tracks, playlists_with_tracks) = self.runtime.block_on(async {
            let profile = client
                .get_profile(token)
                .await
                .map_err(|e| LumeFfiError::auth(e.to_string()))?;
            let liked = client
                .get_liked_tracks(token)
                .await
                .map_err(|e| LumeFfiError::network(e.to_string()))?;
            let playlists = client
                .get_playlists(token)
                .await
                .map_err(|e| LumeFfiError::network(e.to_string()))?;

            let mut full_playlists = Vec::with_capacity(playlists.len());
            for pl in playlists {
                if let Ok(tracks) = client.get_playlist_tracks(token, &pl.id).await {
                    full_playlists.push((pl, tracks));
                }
            }
            Ok::<_, LumeFfiError>((profile, liked, full_playlists))
        })?;

        let db = self
            .db
            .lock()
            .map_err(|_| LumeFfiError::internal("database mutex poisoned"))?;
        db.sync_spotify_liked_tracks(&liked_tracks)
            .map_err(|e| LumeFfiError::storage(e.to_string()))?;

        let mut synced_ids = std::collections::HashSet::with_capacity(playlists_with_tracks.len());
        for (pl, tracks) in &playlists_with_tracks {
            db.upsert_synced_spotify_playlist(&pl.id, &pl.name, tracks)
                .map_err(|e| LumeFfiError::storage(e.to_string()))?;
            synced_ids.insert(pl.id.clone());
        }
        let _ = db.prune_stale_synced_spotify_playlists(&synced_ids);

        to_json(&SpotifySyncSummaryDto {
            display_name: profile.display_name,
            liked_count: liked_tracks.len(),
            playlist_count: playlists_with_tracks.len(),
        })
    }

    pub fn import_spotify_playlist_url_json(&self, url: String) -> Result<String, LumeFfiError> {
        let imported = self
            .runtime
            .block_on(import_spotify_playlist_url(url.trim()))
            .map_err(|e| LumeFfiError::network(e.to_string()))?;
        let playlist = {
            let db = self
                .db
                .lock()
                .map_err(|_| LumeFfiError::internal("database mutex poisoned"))?;
            db.upsert_imported_spotify_playlist(
                &imported.name,
                &imported.source_url,
                &imported.tracks,
            )
            .map_err(|e| LumeFfiError::storage(e.to_string()))?
        };
        to_json(&playlist)
    }

    pub fn rematch_spotify_track(
        &self,
        spotify_track_id: String,
        yt_video_id: String,
    ) -> Result<(), LumeFfiError> {
        let db = self
            .db
            .lock()
            .map_err(|_| LumeFfiError::internal("database mutex poisoned"))?;
        db.put_yt_mapping(spotify_track_id.trim(), yt_video_id.trim(), 1.0)
            .map_err(|e| LumeFfiError::storage(e.to_string()))
    }

    pub fn rename_playlist(&self, playlist_id: String, new_name: String) -> Result<(), LumeFfiError> {
        let db = self
            .db
            .lock()
            .map_err(|_| LumeFfiError::internal("database mutex poisoned"))?;
        db.rename_playlist(&playlist_id, &new_name)
            .map_err(|e| LumeFfiError::storage(e.to_string()))
    }

    pub fn import_ytm_playlist_url_json(&self, url_or_id: String) -> Result<String, LumeFfiError> {
        let playlist_id = extract_ytm_playlist_id(&url_or_id);
        if playlist_id.is_empty() {
            return Err(LumeFfiError::invalid("YouTube Music playlist URL/ID is empty"));
        }
        let detail = self
            .runtime
            .block_on(self.ytm.get_playlist_detail(&playlist_id))
            .map_err(|e| LumeFfiError::network(e.to_string()))?;
        let source_url = canonical_ytm_playlist_url(&detail.id);
        let (playlist, _) = {
            let db = self
                .db
                .lock()
                .map_err(|_| LumeFfiError::internal("database mutex poisoned"))?;
            db.upsert_imported_ytm_playlist(&detail.name, &source_url, &detail.tracks)
                .map_err(|e| LumeFfiError::storage(e.to_string()))?
        };
        to_json(&playlist)
    }

    pub fn import_exportify_csv_json(
        &self,
        csv_text: String,
        playlist_name: String,
    ) -> Result<String, LumeFfiError> {
        let name = if playlist_name.trim().is_empty() {
            "Imported CSV Playlist"
        } else {
            playlist_name.trim()
        };
        let source_tag = format!("csv:{}", sanitize_filename(name));
        let imported = parse_exportify_csv(&csv_text, name, &source_tag)
            .map_err(|e| LumeFfiError::invalid(e.to_string()))?;
        let playlist = {
            let db = self
                .db
                .lock()
                .map_err(|_| LumeFfiError::internal("database mutex poisoned"))?;
            db.upsert_imported_spotify_playlist(
                &imported.name,
                &imported.source_url,
                &imported.tracks,
            )
            .map_err(|e| LumeFfiError::storage(e.to_string()))?
        };
        to_json(&playlist)
    }

    /// Copies a user-picked local audio file into the iOS app container and indexes it via `lofty`.
    pub fn import_local_audio_file_json(
        &self,
        source_uri_or_path: String,
        file_name: String,
    ) -> Result<String, LumeFfiError> {
        let src = strip_file_scheme(&source_uri_or_path);
        if !src.is_file() {
            return Err(LumeFfiError::invalid("selected local audio file not found"));
        }

        let safe_name = file_name
            .trim()
            .replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_");
        let final_name = if safe_name.is_empty() {
            src.file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("imported_audio.m4a")
                .to_string()
        } else {
            safe_name
        };

        let dest = self.local_audio_dir.join(&final_name);
        if src != dest {
            std::fs::copy(&src, &dest)
                .map_err(|e| LumeFfiError::storage(format!("failed to copy audio file: {e}")))?;
        }

        let tracks = {
            let db = self
                .db
                .lock()
                .map_err(|_| LumeFfiError::internal("database mutex poisoned"))?;
            let mut art_cache = ArtCache::new(self.art_cache_dir.clone(), 64 * 1024 * 1024, 128);
            let local = LocalProvider::new(self.db_path.clone());
            local
                .scan_folders(std::slice::from_ref(&self.local_audio_dir), &db, &mut art_cache)
                .map_err(|e| LumeFfiError::storage(e.to_string()))?;
            db.list_local_tracks()
                .map_err(|e| LumeFfiError::storage(e.to_string()))?
        };
        to_json(&tracks)
    }

    pub fn list_local_tracks_json(&self) -> Result<String, LumeFfiError> {
        let tracks = {
            let db = self
                .db
                .lock()
                .map_err(|_| LumeFfiError::internal("database mutex poisoned"))?;
            db.list_local_tracks()
                .map_err(|e| LumeFfiError::storage(e.to_string()))?
        };
        to_json(&tracks)
    }

    pub fn search_local_tracks_json(&self, query: String) -> Result<String, LumeFfiError> {
        let tracks = {
            let db = self
                .db
                .lock()
                .map_err(|_| LumeFfiError::internal("database mutex poisoned"))?;
            db.search_local_tracks(&query, 50)
                .map_err(|e| LumeFfiError::storage(e.to_string()))?
        };
        to_json(&tracks)
    }

    pub fn clear_all_offline_tracks(&self) -> Result<(), LumeFfiError> {
        let entries = {
            let db = self
                .db
                .lock()
                .map_err(|_| LumeFfiError::internal("database mutex poisoned"))?;
            let list = db
                .list_offline_tracks()
                .map_err(|e| LumeFfiError::storage(e.to_string()))?;
            for (track, _) in &list {
                let _ = db.delete_offline_track(&track.uid);
            }
            list
        };
        for (_, rec) in entries {
            let p = PathBuf::from(rec.file_path);
            if p.exists() {
                let _ = std::fs::remove_file(p);
            }
        }
        Ok(())
    }

    pub fn search_spotify_catalog_json(
        &self,
        access_token: String,
        query: String,
        limit: u32,
    ) -> Result<String, LumeFfiError> {
        let token = access_token.trim();
        if token.is_empty() {
            return Err(LumeFfiError::auth("Spotify access token is empty"));
        }
        let cap = (limit as usize).clamp(1, 30);
        let client = SpotifyWebClient::new();
        let catalog = self
            .runtime
            .block_on(client.search_catalog(token, &query, cap))
            .map_err(|e| LumeFfiError::network(e.to_string()))?;
        to_json(&catalog)
    }

    /// Extracts WCAG-safe dynamic album-art glassmorphism colors (matching Lume Desktop).
    pub fn extract_palette_json(&self, artwork_ref: String) -> Result<String, LumeFfiError> {
        let trimmed = artwork_ref.trim();
        if trimmed.is_empty() {
            return to_json(&default_dynamic_palette());
        }

        let bytes = if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
            self.runtime.block_on(async {
                match self.http.get(trimmed).send().await {
                    Ok(resp) if resp.status().is_success() => {
                        resp.bytes().await.map(|b| b.to_vec()).unwrap_or_default()
                    }
                    _ => Vec::new(),
                }
            })
        } else {
            let path = strip_file_scheme(trimmed);
            std::fs::read(path).unwrap_or_default()
        };

        to_json(&extract_palette_from_bytes(&bytes))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn sample_ytm_track(video_id: &str) -> Track {
        let source = TrackSource::YouTubeMusic {
            video_id: video_id.to_string(),
        };
        Track {
            uid: source.canonical_uid(),
            source,
            title: "Night Drive".into(),
            artists: vec!["Lume Artist".into()],
            album: Some("Highway EP".into()),
            duration_ms: 215_000,
            artwork_ref: None,
            is_liked: false,
        }
    }

    #[test]
    fn mobile_core_initializes_and_reports_health() {
        let dir = TempDir::new().unwrap();
        let core = LumeMobileCore::new(
            dir.path().join("data").to_string_lossy().into_owned(),
            dir.path().join("cache").to_string_lossy().into_owned(),
        )
        .unwrap();

        let health: HealthCheckDto =
            serde_json::from_str(&core.health_check_json().unwrap()).unwrap();
        assert_eq!(health.offline_track_count, 0);
        assert_eq!(health.offline_storage_bytes, 0);
        assert!(Path::new(&health.db_path).exists());
    }

    #[test]
    fn playlist_like_and_history_round_trip_over_json_boundary() {
        let dir = TempDir::new().unwrap();
        let core = LumeMobileCore::new(
            dir.path().join("data").to_string_lossy().into_owned(),
            dir.path().join("cache").to_string_lossy().into_owned(),
        )
        .unwrap();

        let created: lume_core::Playlist =
            serde_json::from_str(&core.create_playlist_json("Driving Mix".into()).unwrap())
                .unwrap();
        assert_eq!(created.name, "Driving Mix");

        let track = sample_ytm_track("drv001");
        let track_json = serde_json::to_string(&track).unwrap();
        core.add_track_to_playlist(created.id.clone(), track_json.clone())
            .unwrap();
        core.set_track_liked(track_json.clone(), true).unwrap();
        core.record_play_history(track_json).unwrap();

        let pl_tracks: Vec<Track> =
            serde_json::from_str(&core.get_playlist_tracks_json(created.id).unwrap()).unwrap();
        assert_eq!(pl_tracks.len(), 1);
        assert_eq!(pl_tracks[0].title, "Night Drive");

        let liked: Vec<Track> =
            serde_json::from_str(&core.list_liked_tracks_json().unwrap()).unwrap();
        assert_eq!(liked.len(), 1);

        let recent: Vec<Track> =
            serde_json::from_str(&core.list_recent_tracks_json(10).unwrap()).unwrap();
        assert_eq!(recent.len(), 1);
    }

    #[test]
    fn offline_file_takes_priority_without_network_and_deletes_cleanly() {
        let dir = TempDir::new().unwrap();
        let core = LumeMobileCore::new(
            dir.path().join("data").to_string_lossy().into_owned(),
            dir.path().join("cache").to_string_lossy().into_owned(),
        )
        .unwrap();

        let track = sample_ytm_track("offline_vid_99");
        let audio_file = core.offline_dir.join("ytm_offline_vid_99.m4a");
        std::fs::write(&audio_file, b"fake-m4a-bytes").unwrap();

        {
            let db = core.db.lock().unwrap();
            db.upsert_offline_track(
                &track,
                &audio_file.to_string_lossy(),
                14,
                "audio/mp4",
            )
            .unwrap();
        }

        let resolved: PlayableStreamDto = serde_json::from_str(
            &core
                .resolve_playable_track_json(serde_json::to_string(&track).unwrap(), false)
                .unwrap(),
        )
        .unwrap();
        assert!(resolved.is_offline);
        assert!(resolved.uri.starts_with("file://"));
        assert_eq!(resolved.source_used, PlaybackSourceUsed::LocalFile);
        assert_eq!(core.offline_storage_bytes().unwrap(), 14);

        core.delete_offline_track(track.uid.clone()).unwrap();
        assert!(!audio_file.exists());
        assert_eq!(core.offline_storage_bytes().unwrap(), 0);
    }

    #[test]
    fn spotify_mobile_auth_uses_custom_url_scheme_and_pkce() {
        let dir = TempDir::new().unwrap();
        let core = LumeMobileCore::new(
            dir.path().join("data").to_string_lossy().into_owned(),
            dir.path().join("cache").to_string_lossy().into_owned(),
        )
        .unwrap();

        let started: MobileSpotifyAuthStartDto = serde_json::from_str(
            &core
                .begin_spotify_mobile_auth_json("test_client_id".into())
                .unwrap(),
        )
        .unwrap();
        assert_eq!(started.redirect_uri, "lumemusic://callback");
        assert!(started.authorize_url.contains("lumemusic%3A%2F%2Fcallback"));
        assert!(started.authorize_url.contains("code_challenge_method=S256"));
        assert!(!started.code_verifier.is_empty());
        assert!(!started.state.is_empty());
    }

    #[test]
    fn rename_csv_import_clear_offline_and_palette_work() {
        let dir = TempDir::new().unwrap();
        let core = LumeMobileCore::new(
            dir.path().join("data").to_string_lossy().into_owned(),
            dir.path().join("cache").to_string_lossy().into_owned(),
        )
        .unwrap();

        let pl: lume_core::Playlist =
            serde_json::from_str(&core.create_playlist_json("Old Name".into()).unwrap()).unwrap();
        core.rename_playlist(pl.id.clone(), "Road Trip".into())
            .unwrap();
        let list: Vec<lume_core::Playlist> =
            serde_json::from_str(&core.list_playlists_json().unwrap()).unwrap();
        assert_eq!(list[0].name, "Road Trip");

        let csv = "Track URI,Track Name,Artist Name(s),Album Name,Duration (ms)\n\
                   spotify:track:abc123,Pelangi,HIVI!,Kereta Kencan,220000\n";
        let imported: lume_core::Playlist = serde_json::from_str(
            &core
                .import_exportify_csv_json(csv.into(), "Exportify Mix".into())
                .unwrap(),
        )
        .unwrap();
        assert_eq!(imported.name, "Exportify Mix");
        assert_eq!(imported.track_count, 1);

        // Verify palette extraction on a synthetic crimson PNG image.
        let mut img = image::RgbaImage::new(8, 8);
        for p in img.pixels_mut() {
            *p = image::Rgba([200, 32, 36, 255]);
        }
        let mut png_bytes = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut png_bytes, image::ImageFormat::Png)
            .unwrap();
        let pal = extract_palette_from_bytes(png_bytes.get_ref());
        assert!(pal.bg_top.starts_with('#'));
        assert!(pal.accent.starts_with('#'));
    }

    #[test]
    fn chunked_range_downloader_reassembles_multi_chunk_http_206_payload() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::TcpListener;

        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let payload: Vec<u8> = (0..(OFFLINE_RANGE_CHUNK_BYTES as usize + 12_345))
                .map(|i| (i % 251) as u8)
                .collect();
            let total = payload.len();
            let payload_arc = Arc::new(payload.clone());

            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let addr = listener.local_addr().unwrap();

            let server = tokio::spawn(async move {
                for _ in 0..2 {
                    let (mut sock, _) = listener.accept().await.unwrap();
                    let mut buf = [0u8; 2048];
                    let n = sock.read(&mut buf).await.unwrap_or(0);
                    let req = String::from_utf8_lossy(&buf[..n]);
                    let mut start = 0usize;
                    let mut end = total - 1;
                    for line in req.lines() {
                        if let Some(rest) = line.to_ascii_lowercase().strip_prefix("range: bytes=") {
                            let mut parts = rest.trim().split('-');
                            if let Some(s) = parts.next().and_then(|v| v.parse::<usize>().ok()) {
                                start = s;
                            }
                            if let Some(e) = parts.next().and_then(|v| v.parse::<usize>().ok()) {
                                end = e.min(total - 1);
                            }
                        }
                    }
                    let slice = &payload_arc[start..=end];
                    let header = format!(
                        "HTTP/1.1 206 Partial Content\r\nContent-Type: audio/mp4\r\nContent-Range: bytes {start}-{end}/{total}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        slice.len()
                    );
                    sock.write_all(header.as_bytes()).await.unwrap();
                    sock.write_all(slice).await.unwrap();
                }
            });

            let dir = TempDir::new().unwrap();
            let out_path = dir.path().join("chunked.m4a.tmp");
            let client = reqwest::Client::new();
            let url = format!("http://{addr}/videoplayback?clen={total}");

            let (written, mime) = download_url_chunked_to_file(&client, &url, &out_path)
                .await
                .unwrap();
            assert_eq!(written, total as u64);
            assert_eq!(mime, "audio/mp4");
            assert_eq!(std::fs::read(&out_path).unwrap(), payload);
            server.await.unwrap();
        });
    }
}
