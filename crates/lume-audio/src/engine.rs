use std::sync::mpsc::RecvTimeoutError;
use std::time::{Duration, Instant};

use lume_core::PlaybackStatus;
use tokio::sync::mpsc::{channel, Receiver, Sender};

use crate::backend::{AudioBackend, RodioAudioBackend};
use crate::commands::{AudioCommand, AudioEvent};

/// Non-blocking handle to the background audio engine thread.
#[derive(Debug, Clone)]
pub struct AudioEngineHandle {
    tx: std::sync::mpsc::SyncSender<AudioCommand>,
}

impl AudioEngineHandle {
    pub fn spawn() -> Result<(Self, Receiver<AudioEvent>), String> {
        let backend = RodioAudioBackend::new()?;
        Ok(Self::spawn_with_backend(Box::new(backend)))
    }

    pub fn spawn_with_backend(
        backend: Box<dyn AudioBackend + Send>,
    ) -> (Self, Receiver<AudioEvent>) {
        let (cmd_tx, cmd_rx) = std::sync::mpsc::sync_channel::<AudioCommand>(128);
        let (evt_tx, evt_rx) = channel::<AudioEvent>(256);

        std::thread::Builder::new()
            .name("lume-audio-worker".to_string())
            .spawn(move || run_worker(backend, cmd_rx, evt_tx))
            .expect("failed to spawn lume-audio-worker thread");

        (Self { tx: cmd_tx }, evt_rx)
    }

    pub fn send(&self, cmd: AudioCommand) -> Result<(), String> {
        self.tx.send(cmd).map_err(|e| e.to_string())
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
        }
    }
}

fn emit(evt_tx: &Sender<AudioEvent>, event: AudioEvent) {
    let _ = evt_tx.try_send(event);
}

fn handle_command(
    backend: &mut dyn AudioBackend,
    state: &mut WorkerState,
    evt_tx: &Sender<AudioEvent>,
    cmd: AudioCommand,
) -> bool {
    match cmd {
        AudioCommand::LoadLocal {
            path,
            track_uid,
            duration_ms,
        } => {
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
            backend.stop();
            state.current_uid = None;
            state.duration_ms = 0;
            state.ended_emitted = false;
            state.status = PlaybackStatus::Stopped;
            emit(evt_tx, AudioEvent::StateChanged(PlaybackStatus::Stopped));
            true
        }
        AudioCommand::Shutdown => {
            backend.stop();
            false
        }
    }
}

fn run_worker(
    mut backend: Box<dyn AudioBackend + Send>,
    cmd_rx: std::sync::mpsc::Receiver<AudioCommand>,
    evt_tx: Sender<AudioEvent>,
) {
    let mut state = WorkerState::default();
    let tick_interval = Duration::from_millis(100);

    loop {
        if state.status == PlaybackStatus::Playing {
            match cmd_rx.recv_timeout(tick_interval) {
                Ok(cmd) => {
                    if !handle_command(backend.as_mut(), &mut state, &evt_tx, cmd) {
                        break;
                    }
                    while let Ok(next_cmd) = cmd_rx.try_recv() {
                        if !handle_command(backend.as_mut(), &mut state, &evt_tx, next_cmd) {
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
            match cmd_rx.recv() {
                Ok(cmd) => {
                    if !handle_command(backend.as_mut(), &mut state, &evt_tx, cmd) {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lume_core::PlaybackStatus;
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use tempfile::TempDir;

    #[derive(Debug, Default)]
    struct MockState {
        loaded: Vec<PathBuf>,
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
    }

    impl MockAudioBackend {
        fn new() -> (Self, Arc<Mutex<MockState>>) {
            let state = Arc::new(Mutex::new(MockState::default()));
            (
                Self {
                    state: Arc::clone(&state),
                },
                state,
            )
        }
    }

    impl AudioBackend for MockAudioBackend {
        fn load_local(&mut self, path: &Path) -> Result<Option<u64>, String> {
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

    fn write_wav_file(path: &Path) {
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
        std::fs::write(path, buf).unwrap();
    }

    #[tokio::test]
    async fn load_pause_resume_seek_stop_emit_expected_events() {
        let dir = TempDir::new().unwrap();
        let wav = dir.path().join("track.wav");
        write_wav_file(&wav);

        let (backend, state) = MockAudioBackend::new();
        let (handle, mut rx) = AudioEngineHandle::spawn_with_backend(Box::new(backend));

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
        let (backend, state) = MockAudioBackend::new();
        let (handle, _rx) = AudioEngineHandle::spawn_with_backend(Box::new(backend));

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
        let (handle, mut rx) = AudioEngineHandle::spawn_with_backend(Box::new(backend));

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
        let (handle, mut rx) = AudioEngineHandle::spawn_with_backend(Box::new(backend));

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
}
