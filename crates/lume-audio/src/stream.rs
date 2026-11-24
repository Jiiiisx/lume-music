use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use tokio::task::JoinHandle;

/// Default bytes downloaded before handing the stream to `symphonia`/`rodio` for probing.
pub const DEFAULT_PREBUFFER_BYTES: u64 = 128 * 1024;

static STREAM_FILE_GEN: AtomicU64 = AtomicU64::new(1);

/// Default cache directory for temporary streaming buffers (`%LOCALAPPDATA%\LumeMusic\cache`).
pub fn default_stream_cache_dir() -> PathBuf {
    let base = dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("."));
    base.join("LumeMusic").join("cache")
}

/// Remove any leftover temporary stream buffer files (`stream.tmp` or `stream-*.tmp`).
pub fn cleanup_stale_stream_files(cache_dir: &Path) {
    let Ok(entries) = std::fs::read_dir(cache_dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if name == "stream.tmp" || (name.starts_with("stream-") && name.ends_with(".tmp")) {
                let _ = std::fs::remove_file(path);
            }
        }
    }
}

#[derive(Debug, Default)]
struct BufferProgress {
    downloaded_bytes: u64,
    total_bytes: Option<u64>,
    finished: bool,
    cancelled: bool,
    error: Option<String>,
}

/// Shared synchronization state between the async HTTP writer and `ProgressiveStreamReader`.
#[derive(Debug, Default)]
pub struct StreamBufferState {
    progress: Mutex<BufferProgress>,
    cvar: Condvar,
}

impl StreamBufferState {
    pub fn new(total_bytes: Option<u64>) -> Arc<Self> {
        Arc::new(Self {
            progress: Mutex::new(BufferProgress {
                downloaded_bytes: 0,
                total_bytes,
                finished: false,
                cancelled: false,
                error: None,
            }),
            cvar: Condvar::new(),
        })
    }

    pub fn set_total_bytes(&self, total: Option<u64>) {
        if let Ok(mut p) = self.progress.lock() {
            p.total_bytes = total;
            self.cvar.notify_all();
        }
    }

    pub fn advance(&self, added_bytes: u64) -> u64 {
        if let Ok(mut p) = self.progress.lock() {
            p.downloaded_bytes = p.downloaded_bytes.saturating_add(added_bytes);
            let total = p.downloaded_bytes;
            self.cvar.notify_all();
            total
        } else {
            0
        }
    }

    pub fn mark_finished(&self) {
        if let Ok(mut p) = self.progress.lock() {
            p.finished = true;
            if p.total_bytes.is_none() {
                p.total_bytes = Some(p.downloaded_bytes);
            }
            self.cvar.notify_all();
        }
    }

    pub fn mark_error(&self, message: String) {
        if let Ok(mut p) = self.progress.lock() {
            p.error = Some(message);
            p.finished = true;
            self.cvar.notify_all();
        }
    }

    pub fn cancel(&self) {
        if let Ok(mut p) = self.progress.lock() {
            p.cancelled = true;
            self.cvar.notify_all();
        }
    }

    pub fn is_cancelled(&self) -> bool {
        self.progress.lock().map(|p| p.cancelled).unwrap_or(true)
    }

    pub fn total_bytes(&self) -> Option<u64> {
        self.progress.lock().ok().and_then(|p| p.total_bytes)
    }
}

/// Handle held by the audio worker to cancel and clean up an active HTTP download.
pub struct ActiveHttpStream {
    state: Arc<StreamBufferState>,
    task: Option<JoinHandle<()>>,
    temp_path: PathBuf,
}

impl ActiveHttpStream {
    pub fn new(state: Arc<StreamBufferState>, task: JoinHandle<()>, temp_path: PathBuf) -> Self {
        Self {
            state,
            task: Some(task),
            temp_path,
        }
    }

    pub fn temp_path(&self) -> &Path {
        &self.temp_path
    }

    pub fn cancel(&mut self) {
        self.state.cancel();
        if let Some(handle) = self.task.take() {
            handle.abort();
        }
    }
}

impl Drop for ActiveHttpStream {
    fn drop(&mut self) {
        self.cancel();
        let _ = std::fs::remove_file(&self.temp_path);
    }
}

/// Seekable, progressively-fed reader backed by a temporary file on disk.
///
/// Blocks `read()` via `Condvar` when catching up to the active HTTP writer, wakes immediately
/// on cancellation, and removes its temporary file when dropped.
pub struct ProgressiveStreamReader {
    file: Option<File>,
    pos: u64,
    state: Arc<StreamBufferState>,
    temp_path: PathBuf,
}

impl ProgressiveStreamReader {
    pub fn open(temp_path: PathBuf, state: Arc<StreamBufferState>) -> io::Result<Self> {
        let file = File::open(&temp_path)?;
        Ok(Self {
            file: Some(file),
            pos: 0,
            state,
            temp_path,
        })
    }

    pub fn temp_path(&self) -> &Path {
        &self.temp_path
    }

    pub fn total_bytes(&self) -> Option<u64> {
        self.state.total_bytes()
    }
}

impl Read for ProgressiveStreamReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }

        let available = {
            let mut p = self
                .state
                .progress
                .lock()
                .map_err(|_| io::Error::other("stream buffer lock poisoned"))?;
            while self.pos >= p.downloaded_bytes {
                if p.cancelled {
                    return Ok(0);
                }
                if let Some(ref err) = p.error {
                    return Err(io::Error::other(err.clone()));
                }
                if p.finished {
                    return Ok(0);
                }
                p = self
                    .state
                    .cvar
                    .wait(p)
                    .map_err(|_| io::Error::other("stream buffer lock poisoned"))?;
            }
            (p.downloaded_bytes - self.pos) as usize
        };

        let file = self
            .file
            .as_mut()
            .ok_or_else(|| io::Error::other("stream reader closed"))?;
        file.seek(SeekFrom::Start(self.pos))?;
        let to_read = buf.len().min(available);
        let n = file.read(&mut buf[..to_read])?;
        self.pos = self.pos.saturating_add(n as u64);
        Ok(n)
    }
}

impl Seek for ProgressiveStreamReader {
    fn seek(&mut self, style: SeekFrom) -> io::Result<u64> {
        let target: i64 = {
            let mut p = self
                .state
                .progress
                .lock()
                .map_err(|_| io::Error::other("stream buffer lock poisoned"))?;
            match style {
                SeekFrom::Start(offset) => offset as i64,
                SeekFrom::Current(delta) => (self.pos as i64).saturating_add(delta),
                SeekFrom::End(delta) => {
                    while p.total_bytes.is_none() && !p.finished && !p.cancelled {
                        p = self
                            .state
                            .cvar
                            .wait(p)
                            .map_err(|_| io::Error::other("stream buffer lock poisoned"))?;
                    }
                    let end = p.total_bytes.unwrap_or(p.downloaded_bytes);
                    (end as i64).saturating_add(delta)
                }
            }
        };

        if target < 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "cannot seek before start of stream",
            ));
        }

        self.pos = target as u64;
        Ok(self.pos)
    }
}

impl Drop for ProgressiveStreamReader {
    fn drop(&mut self) {
        // Close the file handle first so Windows permits immediate deletion.
        self.file.take();
        let _ = std::fs::remove_file(&self.temp_path);
    }
}

/// Allocate a fresh temporary file path inside `cache_dir` (`stream-<pid>-<gen>.tmp`).
pub fn allocate_stream_temp_path(cache_dir: &Path) -> io::Result<PathBuf> {
    std::fs::create_dir_all(cache_dir)?;
    let pid = std::process::id();
    let gen = STREAM_FILE_GEN.fetch_add(1, Ordering::Relaxed);
    Ok(cache_dir.join(format!("stream-{pid}-{gen}.tmp")))
}

/// Chunk size used when falling back to HTTP `Range` streaming (256 KiB).
pub const HTTP_RANGE_CHUNK_BYTES: u64 = 256 * 1024;

fn parse_content_range_total(header_val: Option<&reqwest::header::HeaderValue>) -> Option<u64> {
    let text = header_val?.to_str().ok()?;
    let total_str = text.rsplit('/').next()?;
    total_str.trim().parse::<u64>().ok()
}

/// Download `url` progressively into `temp_path`, invoking `on_ready` as soon as the initial
/// prebuffer threshold is reached (or the full response finishes if smaller).
///
/// Tries a standard streaming `GET` first; if the CDN rejects full-file `GET` with HTTP 403,
/// falls back automatically to chunked `Range: bytes=start-end` (`256 KiB`) requests.
pub async fn download_http_stream<F>(
    client: reqwest::Client,
    url: String,
    temp_path: PathBuf,
    state: Arc<StreamBufferState>,
    prebuffer_bytes: u64,
    on_ready: F,
) -> Result<(), String>
where
    F: FnOnce(ProgressiveStreamReader, Option<u64>) + Send + 'static,
{
    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("stream request failed: {e}"))?;

    if resp.status() == reqwest::StatusCode::FORBIDDEN {
        return download_http_stream_chunked_range(
            client,
            url,
            temp_path,
            state,
            prebuffer_bytes,
            on_ready,
        )
        .await;
    }

    let status = resp.status();
    if !status.is_success() {
        return Err(format!("stream HTTP error {}", status.as_u16()));
    }

    let mut resp = resp;
    let total_bytes = resp.content_length();
    state.set_total_bytes(total_bytes);

    let mut writer = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&temp_path)
        .map_err(|e| format!("unable to create temp stream file: {e}"))?;

    let mut on_ready = Some(on_ready);
    let target_prebuffer = total_bytes
        .map(|len| len.min(prebuffer_bytes))
        .unwrap_or(prebuffer_bytes);

    loop {
        if state.is_cancelled() {
            drop(writer);
            let _ = std::fs::remove_file(&temp_path);
            return Ok(());
        }

        match resp.chunk().await {
            Ok(Some(chunk)) => {
                if !chunk.is_empty() {
                    writer
                        .write_all(&chunk)
                        .map_err(|e| format!("temp stream write failed: {e}"))?;
                    let downloaded = state.advance(chunk.len() as u64);

                    if downloaded >= target_prebuffer {
                        if let Some(cb) = on_ready.take() {
                            let _ = writer.flush();
                            let reader =
                                ProgressiveStreamReader::open(temp_path.clone(), Arc::clone(&state))
                                    .map_err(|e| format!("temp stream open failed: {e}"))?;
                            cb(reader, total_bytes);
                        }
                    }
                }
            }
            Ok(None) => {
                let _ = writer.flush();
                state.mark_finished();
                if let Some(cb) = on_ready.take() {
                    let final_len = state.total_bytes();
                    let reader =
                        ProgressiveStreamReader::open(temp_path.clone(), Arc::clone(&state))
                            .map_err(|e| format!("temp stream open failed: {e}"))?;
                    cb(reader, final_len);
                }
                return Ok(());
            }
            Err(e) => {
                let msg = format!("stream download interrupted: {e}");
                state.mark_error(msg.clone());
                return Err(msg);
            }
        }
    }
}

async fn download_http_stream_chunked_range<F>(
    client: reqwest::Client,
    url: String,
    temp_path: PathBuf,
    state: Arc<StreamBufferState>,
    prebuffer_bytes: u64,
    on_ready: F,
) -> Result<(), String>
where
    F: FnOnce(ProgressiveStreamReader, Option<u64>) + Send + 'static,
{
    let mut writer = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&temp_path)
        .map_err(|e| format!("unable to create temp stream file: {e}"))?;

    let mut on_ready = Some(on_ready);
    let mut offset = 0u64;
    let mut total_bytes: Option<u64> = None;

    loop {
        if state.is_cancelled() {
            drop(writer);
            let _ = std::fs::remove_file(&temp_path);
            return Ok(());
        }
        if let Some(total) = total_bytes {
            if offset >= total {
                break;
            }
        }

        let end = match total_bytes {
            Some(total) => (offset + HTTP_RANGE_CHUNK_BYTES - 1).min(total.saturating_sub(1)),
            None => offset + HTTP_RANGE_CHUNK_BYTES - 1,
        };
        let mut resp = client
            .get(&url)
            .header(reqwest::header::RANGE, format!("bytes={offset}-{end}"))
            .send()
            .await
            .map_err(|e| format!("chunked stream request failed: {e}"))?;

        let status = resp.status();
        if !status.is_success() && status != reqwest::StatusCode::PARTIAL_CONTENT {
            let msg = format!("stream HTTP error {}", status.as_u16());
            state.mark_error(msg.clone());
            return Err(msg);
        }

        if total_bytes.is_none() {
            total_bytes = parse_content_range_total(resp.headers().get(reqwest::header::CONTENT_RANGE))
                .or_else(|| {
                    if status == reqwest::StatusCode::OK {
                        resp.content_length()
                    } else {
                        None
                    }
                });
            state.set_total_bytes(total_bytes);
        }

        let target_prebuffer = total_bytes
            .map(|len| len.min(prebuffer_bytes))
            .unwrap_or(prebuffer_bytes);

        let mut chunk_read = 0u64;
        while let Some(chunk) = resp
            .chunk()
            .await
            .map_err(|e| format!("chunked stream interrupted: {e}"))?
        {
            if state.is_cancelled() {
                drop(writer);
                let _ = std::fs::remove_file(&temp_path);
                return Ok(());
            }
            if !chunk.is_empty() {
                writer
                    .write_all(&chunk)
                    .map_err(|e| format!("temp stream write failed: {e}"))?;
                chunk_read += chunk.len() as u64;
                let downloaded = state.advance(chunk.len() as u64);
                if downloaded >= target_prebuffer {
                    if let Some(cb) = on_ready.take() {
                        let _ = writer.flush();
                        let reader =
                            ProgressiveStreamReader::open(temp_path.clone(), Arc::clone(&state))
                                .map_err(|e| format!("temp stream open failed: {e}"))?;
                        cb(reader, total_bytes);
                    }
                }
            }
        }

        offset = offset.saturating_add(chunk_read);
        if chunk_read == 0 || status == reqwest::StatusCode::OK {
            break;
        }
    }

    let _ = writer.flush();
    state.mark_finished();
    if let Some(cb) = on_ready.take() {
        let final_len = state.total_bytes();
        let reader = ProgressiveStreamReader::open(temp_path.clone(), Arc::clone(&state))
            .map_err(|e| format!("temp stream open failed: {e}"))?;
        cb(reader, final_len);
    }
    Ok(())
}

pub fn build_stream_http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(12))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

/// Stream audio from `yt-dlp -f <selector> --no-playlist --no-part --fixup never -o - <url>`
/// into `temp_path`, killing the child process automatically if cancelled/aborted.
pub async fn download_yt_dlp_stream<F>(
    binary: PathBuf,
    video_url: String,
    format_selector: String,
    temp_path: PathBuf,
    state: Arc<StreamBufferState>,
    on_ready: F,
) -> Result<(), String>
where
    F: FnOnce(ProgressiveStreamReader, Option<u64>) + Send + 'static,
{
    use tokio::io::AsyncReadExt;

    let mut cmd = tokio::process::Command::new(&binary);
    cmd.args([
        "-f",
        &format_selector,
        "--no-playlist",
        "--no-part",
        "--fixup",
        "never",
        "--no-warnings",
        "-o",
        "-",
        &video_url,
    ])
    .stdin(std::process::Stdio::null())
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped())
    .kill_on_drop(true);

    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("failed to start yt-dlp ({}): {e}", binary.display()))?;

    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| "yt-dlp stdout pipe unavailable".to_string())?;
    let mut stderr = child.stderr.take();

    let stderr_task = tokio::spawn(async move {
        let mut buf = Vec::new();
        if let Some(ref mut err_pipe) = stderr {
            let _ = err_pipe.read_to_end(&mut buf).await;
        }
        String::from_utf8_lossy(&buf).into_owned()
    });

    let mut writer = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&temp_path)
        .map_err(|e| format!("unable to create temp stream file: {e}"))?;

    let mut buf = vec![0u8; 32 * 1024];
    let mut downloaded = 0u64;

    loop {
        if state.is_cancelled() {
            let _ = child.kill().await;
            drop(writer);
            let _ = std::fs::remove_file(&temp_path);
            return Ok(());
        }

        let n = stdout
            .read(&mut buf)
            .await
            .map_err(|e| format!("yt-dlp stdout read failed: {e}"))?;
        if n == 0 {
            break;
        }
        writer
            .write_all(&buf[..n])
            .map_err(|e| format!("temp stream write failed: {e}"))?;
        downloaded = state.advance(n as u64);
    }

    let _ = writer.flush();
    drop(writer);

    let status = child
        .wait()
        .await
        .map_err(|e| format!("yt-dlp wait failed: {e}"))?;

    if state.is_cancelled() {
        let _ = std::fs::remove_file(&temp_path);
        return Ok(());
    }

    if !status.success() || downloaded == 0 {
        let stderr_text = stderr_task.await.unwrap_or_default();
        let detail = stderr_text
            .lines()
            .rev()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("no audio data received")
            .trim()
            .to_string();
        let _ = std::fs::remove_file(&temp_path);
        let msg = format!("yt-dlp stream failed ({status}): {detail}");
        state.mark_error(msg.clone());
        return Err(msg);
    }

    state.set_total_bytes(Some(downloaded));
    state.mark_finished();
    let reader = ProgressiveStreamReader::open(temp_path.clone(), Arc::clone(&state))
        .map_err(|e| format!("temp stream open failed: {e}"))?;
    on_ready(reader, Some(downloaded));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn parses_content_range_total_bytes() {
        let hv = reqwest::header::HeaderValue::from_static("bytes 0-131071/3512688");
        assert_eq!(parse_content_range_total(Some(&hv)), Some(3_512_688));
        let invalid = reqwest::header::HeaderValue::from_static("bytes 0-10/*");
        assert_eq!(parse_content_range_total(Some(&invalid)), None);
    }

    #[test]
    fn progressive_reader_waits_for_chunks_and_supports_seeking() {
        let dir = TempDir::new().unwrap();
        let path = allocate_stream_temp_path(dir.path()).unwrap();
        let state = StreamBufferState::new(Some(10));

        let mut writer = File::create(&path).unwrap();
        writer.write_all(b"hello").unwrap();
        writer.flush().unwrap();
        state.advance(5);

        let mut reader = ProgressiveStreamReader::open(path.clone(), Arc::clone(&state)).unwrap();

        // Spawn writer finishing the remaining 5 bytes slightly later.
        let state_bg = Arc::clone(&state);
        let writer_thread = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(30));
            writer.write_all(b"world").unwrap();
            writer.flush().unwrap();
            state_bg.advance(5);
            state_bg.mark_finished();
        });

        let mut first = [0u8; 5];
        reader.read_exact(&mut first).unwrap();
        assert_eq!(&first, b"hello");

        let mut second = [0u8; 5];
        reader.read_exact(&mut second).unwrap();
        assert_eq!(&second, b"world");

        // Seek back to offset 2 and read "llo"
        assert_eq!(reader.seek(SeekFrom::Start(2)).unwrap(), 2);
        let mut mid = [0u8; 3];
        reader.read_exact(&mut mid).unwrap();
        assert_eq!(&mid, b"llo");

        // Seek from end (-5) -> "world"
        assert_eq!(reader.seek(SeekFrom::End(-5)).unwrap(), 5);
        let mut tail = [0u8; 5];
        reader.read_exact(&mut tail).unwrap();
        assert_eq!(&tail, b"world");

        writer_thread.join().unwrap();
        drop(reader);
        assert!(!path.exists(), "temp file must be deleted on drop");
    }

    #[test]
    fn cancelling_stream_wakes_blocked_reader_and_deletes_temp_file() {
        let dir = TempDir::new().unwrap();
        let path = allocate_stream_temp_path(dir.path()).unwrap();
        std::fs::write(&path, b"").unwrap();
        let state = StreamBufferState::new(Some(1024));
        let mut reader = ProgressiveStreamReader::open(path.clone(), Arc::clone(&state)).unwrap();

        let state_cancel = Arc::clone(&state);
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(30));
            state_cancel.cancel();
        });

        let mut buf = [0u8; 16];
        let n = reader.read(&mut buf).unwrap();
        assert_eq!(n, 0, "cancelled reader should unblock and return EOF");

        drop(reader);
        assert!(!path.exists(), "temp file must be removed after cancel/drop");
    }

    #[test]
    fn cleanup_stale_stream_files_only_removes_stream_tmps() {
        let dir = TempDir::new().unwrap();
        let stale1 = dir.path().join("stream.tmp");
        let stale2 = dir.path().join("stream-1234-1.tmp");
        let keep = dir.path().join("other.art");
        std::fs::write(&stale1, b"1").unwrap();
        std::fs::write(&stale2, b"2").unwrap();
        std::fs::write(&keep, b"3").unwrap();

        cleanup_stale_stream_files(dir.path());
        assert!(!stale1.exists());
        assert!(!stale2.exists());
        assert!(keep.exists());
    }
}
