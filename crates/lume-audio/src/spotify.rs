use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use librespot_core::authentication::Credentials;
use librespot_core::config::SessionConfig;
use librespot_core::{Session, SpotifyId, SpotifyUri};
use librespot_playback::audio_backend::{Sink, SinkError, SinkResult};
use librespot_playback::config::{Bitrate, PlayerConfig};
use librespot_playback::convert::Converter;
use librespot_playback::decoder::AudioPacket;
use librespot_playback::mixer::softmixer::SoftMixer;
use librespot_playback::mixer::{Mixer, MixerConfig};
use librespot_playback::player::{Player, PlayerEvent};
use librespot_playback::{NUM_CHANNELS, SAMPLE_RATE};

/// Events forwarded asynchronously from the Spotify playback backend into the audio worker.
#[derive(Debug, Clone)]
pub enum SpotifyPlaybackEvent {
    Playing {
        gen: u64,
        position_ms: u64,
        duration_ms: Option<u64>,
    },
    Paused {
        gen: u64,
        position_ms: u64,
    },
    Position {
        gen: u64,
        position_ms: u64,
    },
    EndOfTrack {
        gen: u64,
    },
    FallbackNeeded {
        gen: u64,
        track_uid: String,
        reason: String,
    },
}

/// Callback used by `SpotifyBackend` implementations to forward async events into the audio worker.
pub type SpotifyEventSink = Arc<dyn Fn(SpotifyPlaybackEvent) + Send + Sync>;

/// Hardware-independent Spotify playback backend trait for mutually-exclusive WASAPI handoff
/// and deterministic engine testing.
pub trait SpotifyBackend: Send {
    #[allow(clippy::too_many_arguments)]
    fn load(
        &mut self,
        gen: u64,
        spotify_id: String,
        track_uid: String,
        duration_ms: u64,
        access_token: String,
        rt: &tokio::runtime::Handle,
        on_event: SpotifyEventSink,
    );
    fn play(&mut self);
    fn pause(&mut self);
    fn stop(&mut self);
    fn seek(&mut self, pos_ms: u64);
    fn set_volume(&mut self, volume: f32);
    fn disconnect(&mut self);
}

/// Custom `librespot-playback` Sink backed by Lume's own `rodio 0.23` WASAPI output.
/// Opens the WASAPI device lazily on `start()` and drops it cleanly on `stop()` so
/// mutually-exclusive handoff with `RodioAudioBackend` never holds idle audio streams.
struct LumeRodioSink {
    sink_handle: Option<rodio::MixerDeviceSink>,
    player: Option<rodio::Player>,
    stop_flag: Arc<AtomicBool>,
}

impl LumeRodioSink {
    fn new(stop_flag: Arc<AtomicBool>) -> Self {
        Self {
            sink_handle: None,
            player: None,
            stop_flag,
        }
    }

    fn ensure_open(&mut self) -> SinkResult<()> {
        if self.player.is_some() {
            return Ok(());
        }
        let mut handle = rodio::DeviceSinkBuilder::open_default_sink()
            .map_err(|e| SinkError::ConnectionRefused(e.to_string()))?;
        handle.log_on_drop(false);
        let player = rodio::Player::connect_new(handle.mixer());
        player.play();
        self.sink_handle = Some(handle);
        self.player = Some(player);
        Ok(())
    }
}

impl Sink for LumeRodioSink {
    fn start(&mut self) -> SinkResult<()> {
        self.stop_flag.store(false, Ordering::Release);
        self.ensure_open()?;
        if let Some(player) = &self.player {
            player.play();
        }
        Ok(())
    }

    fn stop(&mut self) -> SinkResult<()> {
        self.stop_flag.store(true, Ordering::Release);
        if let Some(player) = self.player.take() {
            player.stop();
        }
        self.sink_handle = None;
        Ok(())
    }

    fn write(&mut self, packet: AudioPacket, converter: &mut Converter) -> SinkResult<()> {
        if self.stop_flag.load(Ordering::Acquire) {
            return Ok(());
        }
        self.ensure_open()?;
        let samples = packet
            .samples()
            .map_err(|e| SinkError::OnWrite(e.to_string()))?;
        let samples_f32: Vec<f32> = converter.f64_to_f32(samples).to_vec();
        let channels = std::num::NonZeroU16::new(NUM_CHANNELS as u16)
            .ok_or_else(|| SinkError::InvalidParams("zero audio channels".into()))?;
        let sample_rate = std::num::NonZeroU32::new(SAMPLE_RATE)
            .ok_or_else(|| SinkError::InvalidParams("zero sample rate".into()))?;
        let source = rodio::buffer::SamplesBuffer::new(channels, sample_rate, samples_f32);
        if let Some(player) = &self.player {
            player.append(source);
            while player.len() > 26 && !self.stop_flag.load(Ordering::Acquire) {
                std::thread::sleep(Duration::from_millis(10));
            }
        }
        Ok(())
    }
}

/// Real `librespot 0.8` native Spotify playback backend.
pub struct LibrespotBackend {
    shared: Arc<Mutex<LibrespotState>>,
    active_gen: Arc<AtomicU64>,
    active_track_uid: Arc<Mutex<Option<String>>>,
    sink_stop_flag: Arc<AtomicBool>,
    mixer: Arc<SoftMixer>,
    volume: f32,
}

#[derive(Default)]
struct LibrespotState {
    session: Option<Session>,
    player: Option<Arc<Player>>,
    access_token: Option<String>,
    event_task: Option<tokio::task::JoinHandle<()>>,
    connect_task: Option<tokio::task::JoinHandle<()>>,
}

impl Default for LibrespotBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl LibrespotBackend {
    pub fn new() -> Self {
        let mixer = Arc::new(
            SoftMixer::open(MixerConfig::default()).expect("SoftMixer::open should never fail"),
        );
        mixer.set_volume(u16::MAX);
        Self {
            shared: Arc::new(Mutex::new(LibrespotState::default())),
            active_gen: Arc::new(AtomicU64::new(0)),
            active_track_uid: Arc::new(Mutex::new(None)),
            sink_stop_flag: Arc::new(AtomicBool::new(false)),
            mixer,
            volume: 1.0,
        }
    }

    fn apply_mixer_volume(&self, volume: f32) {
        let clamped = volume.clamp(0.0, 1.0);
        let scaled = (clamped * u16::MAX as f32).round() as u16;
        self.mixer.set_volume(scaled);
    }
}

impl SpotifyBackend for LibrespotBackend {
    fn load(
        &mut self,
        gen: u64,
        spotify_id: String,
        track_uid: String,
        _duration_ms: u64,
        access_token: String,
        rt: &tokio::runtime::Handle,
        on_event: SpotifyEventSink,
    ) {
        self.active_gen.store(gen, Ordering::Release);
        if let Ok(mut guard) = self.active_track_uid.lock() {
            *guard = Some(track_uid.clone());
        }
        self.apply_mixer_volume(self.volume);

        let track_uri = match SpotifyId::from_base62(&spotify_id) {
            Ok(id) => SpotifyUri::Track { id },
            Err(err) => {
                on_event(SpotifyPlaybackEvent::FallbackNeeded {
                    gen,
                    track_uid,
                    reason: format!("Invalid Spotify track ID '{spotify_id}': {err}"),
                });
                return;
            }
        };

        // Cancel any in-flight connect task before starting a new one.
        if let Ok(mut st) = self.shared.lock() {
            if let Some(prev) = st.connect_task.take() {
                prev.abort();
            }
            // Fast path: reuse healthy existing session & player if token matches.
            let can_reuse = st.access_token.as_deref() == Some(access_token.as_str())
                && st
                    .session
                    .as_ref()
                    .map(|s| !s.is_invalid())
                    .unwrap_or(false)
                && st
                    .player
                    .as_ref()
                    .map(|p| !p.is_invalid())
                    .unwrap_or(false);
            if can_reuse {
                if let Some(player) = &st.player {
                    self.sink_stop_flag.store(false, Ordering::Release);
                    player.load(track_uri, true, 0);
                    return;
                }
            }
        }

        let shared = Arc::clone(&self.shared);
        let active_gen = Arc::clone(&self.active_gen);
        let active_track_uid = Arc::clone(&self.active_track_uid);
        let sink_stop_flag = Arc::clone(&self.sink_stop_flag);
        let mixer = Arc::clone(&self.mixer);

        let connect_task = rt.spawn(async move {
            let session = Session::new(SessionConfig::default(), None);
            let creds = Credentials::with_access_token(access_token.clone());
            if let Err(err) = session.connect(creds, false).await {
                if active_gen.load(Ordering::Acquire) == gen {
                    on_event(SpotifyPlaybackEvent::FallbackNeeded {
                        gen,
                        track_uid,
                        reason: format!("Spotify session unavailable ({err}); switching to YouTube fallback"),
                    });
                }
                return;
            }

            if active_gen.load(Ordering::Acquire) != gen {
                session.shutdown();
                return;
            }

            let player_cfg = PlayerConfig {
                bitrate: Bitrate::Bitrate320,
                position_update_interval: Some(Duration::from_millis(100)),
                ..PlayerConfig::default()
            };
            let stop_flag_for_sink = Arc::clone(&sink_stop_flag);
            let player = Player::new(
                player_cfg,
                session.clone(),
                mixer.get_soft_volume(),
                move || Box::new(LumeRodioSink::new(stop_flag_for_sink)),
            );

            let mut ev_rx = player.get_player_event_channel();
            let on_event_loop = Arc::clone(&on_event);
            let active_gen_loop = Arc::clone(&active_gen);
            let active_uid_loop = Arc::clone(&active_track_uid);

            let event_task = tokio::spawn(async move {
                let mut current_duration_ms: Option<u64> = None;
                while let Some(ev) = ev_rx.recv().await {
                    let cur_gen = active_gen_loop.load(Ordering::Acquire);
                    match ev {
                        PlayerEvent::TrackChanged { audio_item } => {
                            current_duration_ms = Some(u64::from(audio_item.duration_ms));
                        }
                        PlayerEvent::Playing { position_ms, .. } => {
                            on_event_loop(SpotifyPlaybackEvent::Playing {
                                gen: cur_gen,
                                position_ms: u64::from(position_ms),
                                duration_ms: current_duration_ms,
                            });
                        }
                        PlayerEvent::Paused { position_ms, .. } => {
                            on_event_loop(SpotifyPlaybackEvent::Paused {
                                gen: cur_gen,
                                position_ms: u64::from(position_ms),
                            });
                        }
                        PlayerEvent::PositionChanged { position_ms, .. }
                        | PlayerEvent::PositionCorrection { position_ms, .. }
                        | PlayerEvent::Seeked { position_ms, .. } => {
                            on_event_loop(SpotifyPlaybackEvent::Position {
                                gen: cur_gen,
                                position_ms: u64::from(position_ms),
                            });
                        }
                        PlayerEvent::EndOfTrack { .. } => {
                            on_event_loop(SpotifyPlaybackEvent::EndOfTrack {
                                gen: cur_gen,
                            });
                        }
                        PlayerEvent::Unavailable { .. } => {
                            let uid = active_uid_loop
                                .lock()
                                .ok()
                                .and_then(|g| g.clone())
                                .unwrap_or_default();
                            on_event_loop(SpotifyPlaybackEvent::FallbackNeeded {
                                gen: cur_gen,
                                track_uid: uid,
                                reason: "Spotify track unavailable natively (Premium required or region-locked)"
                                    .to_string(),
                            });
                        }
                        _ => {}
                    }
                }
            });

            if let Ok(mut st) = shared.lock() {
                if let Some(old_ev) = st.event_task.take() {
                    old_ev.abort();
                }
                if let Some(old_player) = st.player.take() {
                    old_player.stop();
                }
                if let Some(old_session) = st.session.take() {
                    old_session.shutdown();
                }
                st.session = Some(session);
                st.player = Some(Arc::clone(&player));
                st.access_token = Some(access_token);
                st.event_task = Some(event_task);
            }

            sink_stop_flag.store(false, Ordering::Release);
            player.load(track_uri, true, 0);
        });

        if let Ok(mut st) = self.shared.lock() {
            st.connect_task = Some(connect_task);
        }
    }

    fn play(&mut self) {
        self.sink_stop_flag.store(false, Ordering::Release);
        if let Ok(st) = self.shared.lock() {
            if let Some(player) = &st.player {
                player.play();
            }
        }
    }

    fn pause(&mut self) {
        if let Ok(st) = self.shared.lock() {
            if let Some(player) = &st.player {
                player.pause();
            }
        }
    }

    fn stop(&mut self) {
        self.active_gen.fetch_add(1, Ordering::AcqRel);
        self.sink_stop_flag.store(true, Ordering::Release);
        if let Ok(mut st) = self.shared.lock() {
            if let Some(task) = st.connect_task.take() {
                task.abort();
            }
            if let Some(player) = &st.player {
                player.stop();
            }
        }
    }

    fn seek(&mut self, pos_ms: u64) {
        if let Ok(st) = self.shared.lock() {
            if let Some(player) = &st.player {
                player.seek(pos_ms.min(u32::MAX as u64) as u32);
            }
        }
    }

    fn set_volume(&mut self, volume: f32) {
        self.volume = volume.clamp(0.0, 1.0);
        self.apply_mixer_volume(self.volume);
    }

    fn disconnect(&mut self) {
        self.active_gen.fetch_add(1, Ordering::AcqRel);
        self.sink_stop_flag.store(true, Ordering::Release);
        if let Ok(mut st) = self.shared.lock() {
            if let Some(task) = st.connect_task.take() {
                task.abort();
            }
            if let Some(task) = st.event_task.take() {
                task.abort();
            }
            if let Some(player) = st.player.take() {
                player.stop();
            }
            if let Some(session) = st.session.take() {
                session.shutdown();
            }
            st.access_token = None;
        }
    }
}
