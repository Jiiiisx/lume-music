use std::path::PathBuf;
use std::sync::mpsc::RecvTimeoutError;
use std::sync::Arc;
use std::time::{Duration, Instant};

use lume_core::PlaybackStatus;
use tokio::sync::mpsc::{channel, Receiver, Sender};

use crate::backend::{AudioBackend, RodioAudioBackend};
use crate::commands::{AudioCommand, AudioEvent};
use crate::stream::{
    allocate_stream_temp_path, build_stream_http_client, cleanup_stale_stream_files,
    default_stream_cache_dir, download_http_stream, ActiveHttpStream, ProgressiveStreamReader,
    StreamBufferState, DEFAULT_PREBUFFER_BYTES,
};

enum WorkerMsg {
    Command(AudioCommand),
    StreamReady {
        gen: u64,
        track_uid: String,
        duration_ms: u64,
        reader: ProgressiveStreamReader,
        byte_len: Option<u64>,
        hint: Option<String>,
    },
    StreamError {
        gen: u64,
        error: String,
    },
}

/// Non-blocking handle to the background audio engine thread.
#[derive(Debug, Clone)]
pub struct AudioEngineHandle {
    tx: std::sync::mpsc::SyncSender<WorkerMsg>,
}

impl AudioEngineHandle {
    pub fn spawn() -> Result<(Self, Receiver<AudioEvent>), String> {
        let backend = RodioAudioBackend::new()?;
        Ok(Self::spawn_with_backend_and_cache_dir(
            Box::new(backend),
            default_stream_cache_dir(),
        ))
    }

    pub fn spawn_with_backend(
        backend: Box<dyn AudioBackend + Send>,
    ) -> (Self, Receiver<AudioEvent>) {
        Self::spawn_with_backend_and_cache_dir(backend, default_stream_cache_dir())
    }

    pub fn spawn_with_backend_and_cache_dir(
        backend: Box<dyn AudioBackend + Send>,
        cache_dir: PathBuf,
    ) -> (Self, Receiver<AudioEvent>) {
        let (msg_tx, msg_rx) = std::sync::mpsc::sync_channel::<WorkerMsg>(128);
        let (evt_tx, evt_rx) = channel::<AudioEvent>(256);
        let worker_tx = msg_tx.clone();

        std::thread::Builder::new()
            .name("lume-audio-worker".to_string())
            .spawn(move || run_worker(backend, cache_dir, worker_tx, msg_rx, evt_tx))
            .expect("failed to spawn lume-audio-worker thread");

        (Self { tx: msg_tx }, evt_rx)
    }

    pub fn send(&self, cmd: AudioCommand) -> Result<(), String> {
        self.tx
            .send(WorkerMsg::Command(cmd))
            .map_err(|e| e.to_string())
    }
}

struct WorkerState {
    status: PlaybackStatus,
    current_uid: Option<String>,
    duration_ms: u64,
    volume: f32,
    muted: bool,
    ended_emitted: bool,
    last_tick: Instant,
    load_gen: u64,
    active_stream: Option<ActiveHttpStream>,
}

impl Default for WorkerState {
    fn default() -> Self {
        Self {
            status: PlaybackStatus::Stopped,
            current_uid: None,
            duration_ms: 0,
            volume: 1.0,
            muted: false,
            ended_emitted: false,
            last_tick: Instant::now(),
            load_gen: 0,
            active_stream: None,
        }
    }
}

impl WorkerState {
    fn cancel_active_stream(&mut self) {
        if let Some(mut stream) = self.active_stream.take() {
            stream.cancel();
            // `Drop` cleans up the temp file once readers release it.
        }
    }
}

fn emit(evt_tx: &Sender<AudioEvent>, event: AudioEvent) {
    let _ = evt_tx.try_send(event);
}

struct StreamRuntime {
    rt: tokio::runtime::Runtime,
    http: reqwest::Client,
    cache_dir: PathBuf,
    worker_tx: std::sync::mpsc::SyncSender<WorkerMsg>,
}

fn handle_worker_msg(
    backend: &mut dyn AudioBackend,
    state: &mut WorkerState,
    stream_rt: &StreamRuntime,
    evt_tx: &Sender<AudioEvent>,
    msg: WorkerMsg,
) -> bool {
    match msg {
        WorkerMsg::Command(cmd) => handle_command(backend, state, stream_rt, evt_tx, cmd),
        WorkerMsg::StreamReady {
            gen,
            track_uid,
            duration_ms,
            reader,
            byte_len,
            hint,
        } => {
            if gen != state.load_gen {
                // Stale stream from a superseded request — drop immediately so its temp file is removed.
                drop(reader);
                return true;
            }
            match backend.load_stream(reader, byte_len, hint.as_deref()) {
                Ok(probed_ms) => {
                    let effective_vol = if state.muted { 0.0 } else { state.volume };
                    backend.set_volume(effective_vol);
                    state.current_uid = Some(track_uid);
                    state.duration_ms = probed_ms.filter(|&d| d > 0).unwrap_or(duration_ms);
                    state.ended_emitted = false;
                    state.status = PlaybackStatus::Playing;
                    state.last_tick = Instant::now();
                    emit(evt_tx, AudioEvent::StateChanged(PlaybackStatus::Playing));
                }
                Err(err) => {
                    state.cancel_active_stream();
                    backend.stop();
                    state.current_uid = None;
                    state.duration_ms = 0;
                    state.ended_emitted = false;
                    state.status = PlaybackStatus::Stopped;
                    emit(evt_tx, AudioEvent::Error(err));
                    emit(evt_tx, AudioEvent::StateChanged(PlaybackStatus::Stopped));
                }
            }
            true
        }
        WorkerMsg::StreamError { gen, error } => {
            if gen != state.load_gen {
                return true;
            }
            state.cancel_active_stream();
            backend.stop();
            state.current_uid = None;
            state.duration_ms = 0;
            state.ended_emitted = false;
            state.status = PlaybackStatus::Stopped;
            emit(evt_tx, AudioEvent::Error(error));
            emit(evt_tx, AudioEvent::StateChanged(PlaybackStatus::Stopped));
            true
        }
    }
}

fn handle_command(
    backend: &mut dyn AudioBackend,
    state: &mut WorkerState,
    stream_rt: &StreamRuntime,
    evt_tx: &Sender<AudioEvent>,
    cmd: AudioCommand,
) -> bool {
    match cmd {
        AudioCommand::LoadLocal {
            path,
            track_uid,
            duration_ms,
        } => {
            state.load_gen = state.load_gen.wrapping_add(1);
            state.cancel_active_stream();

            match backend.load_local(&path) {
                Ok(probed_ms) => {
                    let effective_vol = if state.muted { 0.0 } else { state.volume };
                    backend.set_volume(effective_vol);
                    state.current_uid = Some(track_uid);
                    state.duration_ms = probed_ms.filter(|&d| d > 0).unwrap_or(duration_ms);
                    state.ended_emitted = false;
                    state.status = PlaybackStatus::Playing;
                    state.last_tick = Instant::now();
                    emit(evt_tx, AudioEvent::StateChanged(PlaybackStatus::Playing));
                }
                Err(err) => {
                    backend.stop();
                    state.current_uid = None;
                    state.duration_ms = 0;
                    state.ended_emitted = false;
                    state.status = PlaybackStatus::Stopped;
                    emit(evt_tx, AudioEvent::Error(err));
                }
            }
            true
        }
        AudioCommand::LoadHttpStream {
            url,
            track_uid,
            duration_ms,
            mime_hint,
        } => {
            state.load_gen = state.load_gen.wrapping_add(1);
            let gen = state.load_gen;
            state.cancel_active_stream();
            backend.stop();

            let temp_path = match allocate_stream_temp_path(&stream_rt.cache_dir) {
                Ok(p) => p,
                Err(e) => {
                    state.status = PlaybackStatus::Stopped;
                    emit(
                        evt_tx,
                        AudioEvent::Error(format!("unable to allocate stream buffer: {e}")),
                    );
                    emit(evt_tx, AudioEvent::StateChanged(PlaybackStatus::Stopped));
                    return true;
                }
            };

            state.current_uid = Some(track_uid.clone());
            state.duration_ms = duration_ms;
            state.ended_emitted = false;
            state.status = PlaybackStatus::Buffering;
            emit(evt_tx, AudioEvent::StateChanged(PlaybackStatus::Buffering));

            let buf_state = StreamBufferState::new(None);
            let http = stream_rt.http.clone();
            let tx_ready = stream_rt.worker_tx.clone();
            let tx_err = stream_rt.worker_tx.clone();
            let path_for_task = temp_path.clone();
            let state_for_task = Arc::clone(&buf_state);

            let task = stream_rt.rt.spawn(async move {
                let ready_flag = Arc::new(std::sync::atomic::AtomicBool::new(false));
                let ready_flag_cb = Arc::clone(&ready_flag);
                let result = download_http_stream(
                    http,
                    url,
                    path_for_task,
                    Arc::clone(&state_for_task),
                    DEFAULT_PREBUFFER_BYTES,
                    move |reader, byte_len| {
                        ready_flag_cb.store(true, std::sync::atomic::Ordering::Release);
                        let _ = tx_ready.send(WorkerMsg::StreamReady {
                            gen,
                            track_uid,
                            duration_ms,
                            reader,
                            byte_len,
                            hint: mime_hint,
                        });
                    },
                )
                .await;

                if let Err(error) = result {
                    let sent_ready = ready_flag.load(std::sync::atomic::Ordering::Acquire);
                    if !sent_ready && !state_for_task.is_cancelled() {
                        let _ = tx_err.send(WorkerMsg::StreamError { gen, error });
                    }
                }
            });

            state.active_stream = Some(ActiveHttpStream::new(buf_state, task, temp_path));
            true
        }
        AudioCommand::Pause => {
            if state.status == PlaybackStatus::Playing {
                backend.pause();
                state.status = PlaybackStatus::Paused;
                emit(evt_tx, AudioEvent::StateChanged(PlaybackStatus::Paused));
            }
            true
        }
        AudioCommand::Resume => {
            if state.status == PlaybackStatus::Paused {
                backend.play();
                state.status = PlaybackStatus::Playing;
                state.last_tick = Instant::now();
                emit(evt_tx, AudioEvent::StateChanged(PlaybackStatus::Playing));
            }
            true
        }
        AudioCommand::Seek(pos_ms) => {
            let clamped = if state.duration_ms > 0 {
                pos_ms.min(state.duration_ms)
            } else {
                pos_ms
            };
            if let Err(err) = backend.seek(clamped) {
                emit(evt_tx, AudioEvent::Error(err));
            } else if state.current_uid.is_some() {
                emit(
                    evt_tx,
                    AudioEvent::PositionTick {
                        position_ms: clamped,
                        duration_ms: state.duration_ms,
                    },
                );
            }
            true
        }
        AudioCommand::SetVolume(vol) => {
            state.volume = vol.clamp(0.0, 1.0);
            if !state.muted {
                backend.set_volume(state.volume);
            }
            true
        }
        AudioCommand::SetMute(muted) => {
            state.muted = muted;
            let effective = if muted { 0.0 } else { state.volume };
            backend.set_volume(effective);
            true
        }
        AudioCommand::Stop => {
            state.load_gen = state.load_gen.wrapping_add(1);
            state.cancel_active_stream();
            backend.stop();
            state.current_uid = None;
            state.duration_ms = 0;
            state.ended_emitted = false;
            state.status = PlaybackStatus::Stopped;
            emit(evt_tx, AudioEvent::StateChanged(PlaybackStatus::Stopped));
            true
        }
        AudioCommand::Shutdown => {
            state.load_gen = state.load_gen.wrapping_add(1);
            state.cancel_active_stream();
            backend.stop();
            false
        }
    }
}

fn run_worker(
    mut backend: Box<dyn AudioBackend + Send>,
    cache_dir: PathBuf,
    worker_tx: std::sync::mpsc::SyncSender<WorkerMsg>,
    msg_rx: std::sync::mpsc::Receiver<WorkerMsg>,
    evt_tx: Sender<AudioEvent>,
) {
    cleanup_stale_stream_files(&cache_dir);

    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .thread_name("lume-audio-http")
        .enable_all()
        .build()
        .expect("failed to build audio HTTP runtime");

    let stream_rt = StreamRuntime {
        rt,
        http: build_stream_http_client(),
        cache_dir: cache_dir.clone(),
        worker_tx,
    };

    let mut state = WorkerState::default();
    let tick_interval = Duration::from_millis(100);

    loop {
        if state.status == PlaybackStatus::Playing {
            match msg_rx.recv_timeout(tick_interval) {
                Ok(msg) => {
                    if !handle_worker_msg(backend.as_mut(), &mut state, &stream_rt, &evt_tx, msg) {
                        break;
                    }
                    while let Ok(next_msg) = msg_rx.try_recv() {
                        if !handle_worker_msg(
                            backend.as_mut(),
                            &mut state,
                            &stream_rt,
                            &evt_tx,
                            next_msg,
                        ) {
                            state.cancel_active_stream();
                            backend.stop();
                            cleanup_stale_stream_files(&cache_dir);
                            return;
                        }
                    }
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break,
            }

            if state.status == PlaybackStatus::Playing {
                if backend.is_finished() {
                    if !state.ended_emitted {
                        state.ended_emitted = true;
                        state.status = PlaybackStatus::Stopped;
                        state.cancel_active_stream();
                        if let Some(uid) = state.current_uid.take() {
                            emit(&evt_tx, AudioEvent::TrackEnded { track_uid: uid });
                        }
                        emit(&evt_tx, AudioEvent::StateChanged(PlaybackStatus::Stopped));
                    }
                } else if state.last_tick.elapsed() >= tick_interval {
                    state.last_tick = Instant::now();
                    emit(
                        &evt_tx,
                        AudioEvent::PositionTick {
                            position_ms: backend.position_ms(),
                            duration_ms: state.duration_ms,
                        },
                    );
                }
            }
        } else {
            match msg_rx.recv() {
                Ok(msg) => {
                    if !handle_worker_msg(backend.as_mut(), &mut state, &stream_rt, &evt_tx, msg) {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    }

    state.cancel_active_stream();
    backend.stop();
    drop(stream_rt);
    cleanup_stale_stream_files(&cache_dir);
}

#[cfg(test)]
mod tests {
    use super::*;
    use lume_core::PlaybackStatus;
    use std::io::Read;
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use tempfile::TempDir;

    #[derive(Debug, Default)]
    struct MockState {
        loaded: Vec<PathBuf>,
        stream_loads: usize,
        last_stream_bytes_read: usize,
        playing: bool,
        paused: bool,
        stopped: bool,
        volume_history: Vec<f32>,
        seek_pos_ms: u64,
        pos_ms: u64,
        finished: bool,
        fail_next_load: Option<String>,
    }

    struct MockAudioBackend {
        state: Arc<Mutex<MockState>>,
        active_reader: Option<ProgressiveStreamReader>,
    }

    impl MockAudioBackend {
        fn new() -> (Self, Arc<Mutex<MockState>>) {
            let state = Arc::new(Mutex::new(MockState::default()));
            (
                Self {
                    state: Arc::clone(&state),
                    active_reader: None,
                },
                state,
            )
        }
    }

    impl AudioBackend for MockAudioBackend {
        fn load_local(&mut self, path: &Path) -> Result<Option<u64>, String> {
            self.active_reader = None;
            let mut st = self.state.lock().unwrap();
            if let Some(err) = st.fail_next_load.take() {
                return Err(err);
            }
            if !path.exists() {
                return Err(format!("missing file: {}", path.display()));
            }
            st.loaded.push(path.to_path_buf());
            st.playing = true;
            st.paused = false;
            st.stopped = false;
            st.finished = false;
            st.pos_ms = 0;
            Ok(Some(120_000))
        }

        fn load_stream(
            &mut self,
            mut reader: ProgressiveStreamReader,
            _byte_len: Option<u64>,
            _hint: Option<&str>,
        ) -> Result<Option<u64>, String> {
            let mut sample = [0u8; 44];
            let n = reader.read(&mut sample).map_err(|e| e.to_string())?;
            self.active_reader = Some(reader);
            let mut st = self.state.lock().unwrap();
            st.stream_loads += 1;
            st.last_stream_bytes_read = n;
            st.playing = true;
            st.paused = false;
            st.stopped = false;
            st.finished = false;
            st.pos_ms = 0;
            Ok(Some(1_000))
        }

        fn play(&mut self) {
            let mut st = self.state.lock().unwrap();
            st.playing = true;
            st.paused = false;
        }

        fn pause(&mut self) {
            let mut st = self.state.lock().unwrap();
            st.playing = false;
            st.paused = true;
        }

        fn stop(&mut self) {
            self.active_reader = None;
            let mut st = self.state.lock().unwrap();
            st.playing = false;
            st.paused = false;
            st.stopped = true;
            st.pos_ms = 0;
        }

        fn seek(&mut self, pos_ms: u64) -> Result<(), String> {
            let mut st = self.state.lock().unwrap();
            st.seek_pos_ms = pos_ms;
            st.pos_ms = pos_ms;
            Ok(())
        }

        fn set_volume(&mut self, volume: f32) {
            self.state.lock().unwrap().volume_history.push(volume);
        }

        fn position_ms(&self) -> u64 {
            self.state.lock().unwrap().pos_ms
        }

        fn is_finished(&self) -> bool {
            self.state.lock().unwrap().finished
        }
    }

    fn build_wav_bytes() -> Vec<u8> {
        let sample_rate: u32 = 8000;
        let channels: u16 = 1;
        let bits: u16 = 16;
        let num_samples = sample_rate;
        let data_size = num_samples * 2;
        let mut buf = Vec::with_capacity(44 + data_size as usize);
        buf.extend_from_slice(b"RIFF");
        buf.extend_from_slice(&(36 + data_size).to_le_bytes());
        buf.extend_from_slice(b"WAVEfmt ");
        buf.extend_from_slice(&16u32.to_le_bytes());
        buf.extend_from_slice(&1u16.to_le_bytes());
        buf.extend_from_slice(&channels.to_le_bytes());
        buf.extend_from_slice(&sample_rate.to_le_bytes());
        buf.extend_from_slice(&(sample_rate * 2).to_le_bytes());
        buf.extend_from_slice(&2u16.to_le_bytes());
        buf.extend_from_slice(&bits.to_le_bytes());
        buf.extend_from_slice(b"data");
        buf.extend_from_slice(&data_size.to_le_bytes());
        buf.resize(44 + data_size as usize, 0u8);
        buf
    }

    fn write_wav_file(path: &Path) {
        std::fs::write(path, build_wav_bytes()).unwrap();
    }

    #[tokio::test]
    async fn load_pause_resume_seek_stop_emit_expected_events() {
        let dir = TempDir::new().unwrap();
        let wav = dir.path().join("track.wav");
        write_wav_file(&wav);

        let (backend, state) = MockAudioBackend::new();
        let (handle, mut rx) =
            AudioEngineHandle::spawn_with_backend_and_cache_dir(Box::new(backend), dir.path().join("cache"));

        handle
            .send(AudioCommand::LoadLocal {
                path: wav,
                track_uid: "local:1".into(),
                duration_ms: 1000,
            })
            .unwrap();
        assert_eq!(
            rx.recv().await.unwrap(),
            AudioEvent::StateChanged(PlaybackStatus::Playing)
        );

        handle.send(AudioCommand::Pause).unwrap();
        loop {
            match rx.recv().await.unwrap() {
                AudioEvent::StateChanged(PlaybackStatus::Paused) => break,
                AudioEvent::PositionTick { .. } => continue,
                other => panic!("unexpected event: {other:?}"),
            }
        }

        handle.send(AudioCommand::Seek(45_000)).unwrap();
        handle.send(AudioCommand::Resume).unwrap();
        loop {
            match rx.recv().await.unwrap() {
                AudioEvent::StateChanged(PlaybackStatus::Playing) => break,
                AudioEvent::PositionTick { .. } => continue,
                other => panic!("unexpected event: {other:?}"),
            }
        }
        assert_eq!(state.lock().unwrap().seek_pos_ms, 45_000);

        handle.send(AudioCommand::Stop).unwrap();
        loop {
            match rx.recv().await.unwrap() {
                AudioEvent::StateChanged(PlaybackStatus::Stopped) => break,
                AudioEvent::PositionTick { .. } => continue,
                other => panic!("unexpected event: {other:?}"),
            }
        }

        handle.send(AudioCommand::Shutdown).unwrap();
    }

    #[tokio::test]
    async fn mute_sets_effective_volume_to_zero_and_unmute_restores_previous_level() {
        let dir = TempDir::new().unwrap();
        let (backend, state) = MockAudioBackend::new();
        let (handle, _rx) =
            AudioEngineHandle::spawn_with_backend_and_cache_dir(Box::new(backend), dir.path().join("cache"));

        handle.send(AudioCommand::SetVolume(0.75)).unwrap();
        handle.send(AudioCommand::SetMute(true)).unwrap();
        handle.send(AudioCommand::SetMute(false)).unwrap();
        handle.send(AudioCommand::Shutdown).unwrap();

        tokio::time::sleep(Duration::from_millis(50)).await;
        let history = state.lock().unwrap().volume_history.clone();
        assert_eq!(history, vec![0.75, 0.0, 0.75]);
    }

    #[tokio::test]
    async fn missing_or_corrupt_file_emits_error_without_stopping_worker() {
        let dir = TempDir::new().unwrap();
        let wav = dir.path().join("valid.wav");
        write_wav_file(&wav);

        let (backend, _state) = MockAudioBackend::new();
        let (handle, mut rx) =
            AudioEngineHandle::spawn_with_backend_and_cache_dir(Box::new(backend), dir.path().join("cache"));

        handle
            .send(AudioCommand::LoadLocal {
                path: dir.path().join("does_not_exist.mp3"),
                track_uid: "local:bad".into(),
                duration_ms: 0,
            })
            .unwrap();

        assert!(matches!(rx.recv().await.unwrap(), AudioEvent::Error(_)));

        // Worker remains responsive for subsequent loads.
        handle
            .send(AudioCommand::LoadLocal {
                path: wav,
                track_uid: "local:good".into(),
                duration_ms: 1000,
            })
            .unwrap();
        assert_eq!(
            rx.recv().await.unwrap(),
            AudioEvent::StateChanged(PlaybackStatus::Playing)
        );
        handle.send(AudioCommand::Shutdown).unwrap();
    }

    #[tokio::test]
    async fn emits_track_ended_once_when_backend_finishes() {
        let dir = TempDir::new().unwrap();
        let wav = dir.path().join("short.wav");
        write_wav_file(&wav);

        let (backend, state) = MockAudioBackend::new();
        let (handle, mut rx) =
            AudioEngineHandle::spawn_with_backend_and_cache_dir(Box::new(backend), dir.path().join("cache"));

        handle
            .send(AudioCommand::LoadLocal {
                path: wav,
                track_uid: "local:end".into(),
                duration_ms: 1000,
            })
            .unwrap();
        assert_eq!(
            rx.recv().await.unwrap(),
            AudioEvent::StateChanged(PlaybackStatus::Playing)
        );

        state.lock().unwrap().finished = true;

        let mut ended_count = 0;
        let deadline = tokio::time::Instant::now() + Duration::from_millis(450);
        while tokio::time::Instant::now() < deadline {
            if let Ok(Some(evt)) =
                tokio::time::timeout(Duration::from_millis(120), rx.recv()).await
            {
                if matches!(evt, AudioEvent::TrackEnded { ref track_uid } if track_uid == "local:end")
                {
                    ended_count += 1;
                }
            }
        }
        assert_eq!(ended_count, 1);
        handle.send(AudioCommand::Shutdown).unwrap();
    }

    #[tokio::test]
    async fn load_http_stream_emits_buffering_then_playing_and_cleans_up_on_stop() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::TcpListener;

        let wav = build_wav_bytes();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let server = tokio::spawn(async move {
            let (mut sock, _) = listener.accept().await.unwrap();
            let mut req_buf = [0u8; 1024];
            let _ = sock.read(&mut req_buf).await;
            let header = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: audio/wav\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                wav.len()
            );
            sock.write_all(header.as_bytes()).await.unwrap();
            sock.write_all(&wav).await.unwrap();
        });

        let dir = TempDir::new().unwrap();
        let cache_dir = dir.path().join("stream_cache");
        let (backend, state) = MockAudioBackend::new();
        let (handle, mut rx) =
            AudioEngineHandle::spawn_with_backend_and_cache_dir(Box::new(backend), cache_dir.clone());

        handle
            .send(AudioCommand::LoadHttpStream {
                url: format!("http://{addr}/song.wav"),
                track_uid: "ytm:test123".into(),
                duration_ms: 1_000,
                mime_hint: Some("wav".into()),
            })
            .unwrap();

        assert_eq!(
            rx.recv().await.unwrap(),
            AudioEvent::StateChanged(PlaybackStatus::Buffering)
        );
        assert_eq!(
            rx.recv().await.unwrap(),
            AudioEvent::StateChanged(PlaybackStatus::Playing)
        );
        assert_eq!(state.lock().unwrap().stream_loads, 1);
        assert_eq!(state.lock().unwrap().last_stream_bytes_read, 44);

        handle.send(AudioCommand::Stop).unwrap();
        loop {
            match rx.recv().await.unwrap() {
                AudioEvent::StateChanged(PlaybackStatus::Stopped) => break,
                AudioEvent::PositionTick { .. } => continue,
                other => panic!("unexpected event: {other:?}"),
            }
        }
        handle.send(AudioCommand::Shutdown).unwrap();
        server.await.unwrap();

        tokio::time::sleep(Duration::from_millis(60)).await;
        let remaining: Vec<_> = std::fs::read_dir(&cache_dir)
            .unwrap()
            .flatten()
            .collect();
        assert!(
            remaining.is_empty(),
            "temporary stream file must be deleted after Stop/Shutdown"
        );
    }

    #[tokio::test]
    async fn http_403_stream_emits_error_without_stopping_worker() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let server = tokio::spawn(async move {
            let (mut sock, _) = listener.accept().await.unwrap();
            let mut req_buf = [0u8; 1024];
            let _ = sock.read(&mut req_buf).await;
            let resp = "HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
            sock.write_all(resp.as_bytes()).await.unwrap();
        });

        let dir = TempDir::new().unwrap();
        let wav = dir.path().join("fallback.wav");
        write_wav_file(&wav);

        let (backend, _) = MockAudioBackend::new();
        let (handle, mut rx) = AudioEngineHandle::spawn_with_backend_and_cache_dir(
            Box::new(backend),
            dir.path().join("cache"),
        );

        handle
            .send(AudioCommand::LoadHttpStream {
                url: format!("http://{addr}/blocked.m4a"),
                track_uid: "ytm:blocked".into(),
                duration_ms: 1_000,
                mime_hint: Some("m4a".into()),
            })
            .unwrap();

        assert_eq!(
            rx.recv().await.unwrap(),
            AudioEvent::StateChanged(PlaybackStatus::Buffering)
        );
        match rx.recv().await.unwrap() {
            AudioEvent::Error(msg) => assert!(msg.contains("403")),
            other => panic!("expected Error(403), got {other:?}"),
        }
        assert_eq!(
            rx.recv().await.unwrap(),
            AudioEvent::StateChanged(PlaybackStatus::Stopped)
        );

        // Worker remains responsive for subsequent loads.
        handle
            .send(AudioCommand::LoadLocal {
                path: wav,
                track_uid: "local:ok".into(),
                duration_ms: 1_000,
            })
            .unwrap();
        assert_eq!(
            rx.recv().await.unwrap(),
            AudioEvent::StateChanged(PlaybackStatus::Playing)
        );
        handle.send(AudioCommand::Shutdown).unwrap();
        server.await.unwrap();
    }

    #[test]
    fn decodes_valid_wav_buffer_with_symphonia_decoder() {
        let dir = TempDir::new().unwrap();
        let wav = dir.path().join("decode.wav");
        write_wav_file(&wav);
        let file = std::fs::File::open(&wav).unwrap();
        let decoder = rodio::Decoder::try_from(file).unwrap();
        use rodio::Source;
        assert!(decoder.total_duration().is_some());
    }

    #[test]
    fn decodes_progressive_stream_reader_with_rodio_seekable_builder() {
        let dir = TempDir::new().unwrap();
        let wav_bytes = build_wav_bytes();
        let path = allocate_stream_temp_path(dir.path()).unwrap();
        std::fs::write(&path, &wav_bytes).unwrap();
        let state = StreamBufferState::new(Some(wav_bytes.len() as u64));
        state.advance(wav_bytes.len() as u64);
        state.mark_finished();

        let reader = ProgressiveStreamReader::open(path.clone(), state).unwrap();
        let mut decoder = rodio::Decoder::builder()
            .with_data(std::io::BufReader::new(reader))
            .with_byte_len(wav_bytes.len() as u64)
            .with_seekable(true)
            .with_hint("wav")
            .build()
            .expect("rodio Decoder::builder should decode ProgressiveStreamReader");
        use rodio::Source;
        assert!(decoder.total_duration().is_some());
        decoder
            .try_seek(Duration::from_millis(500))
            .expect("seek on ProgressiveStreamReader should succeed");
        drop(decoder);
        assert!(!path.exists(), "temp file must be deleted when decoder drops");
    }
}
