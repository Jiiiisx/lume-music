use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use rodio::Source;

use crate::stream::{ProgressiveStreamReader, StreamBufferState};

/// Hardware-independent playback backend trait for deterministic testing and WASAPI playback.
pub trait AudioBackend: Send {
    fn load_local(&mut self, path: &Path) -> Result<Option<u64>, String>;
    fn load_stream(
        &mut self,
        reader: ProgressiveStreamReader,
        byte_len: Option<u64>,
        hint: Option<&str>,
    ) -> Result<Option<u64>, String>;
    fn play(&mut self);
    fn pause(&mut self);
    fn stop(&mut self);
    fn seek(&mut self, pos_ms: u64) -> Result<(), String>;
    fn set_volume(&mut self, volume: f32);
    fn position_ms(&self) -> u64;
    fn is_finished(&self) -> bool;
}

struct ActiveStreamSeekMeta {
    temp_path: PathBuf,
    state: Arc<StreamBufferState>,
    hint: Option<String>,
}

/// Real Windows audio output backend powered by `rodio 0.23` (`symphonia` + WASAPI).
pub struct RodioAudioBackend {
    _sink_handle: rodio::MixerDeviceSink,
    player: Option<rodio::Player>,
    stream_meta: Option<ActiveStreamSeekMeta>,
    volume: f32,
}

impl RodioAudioBackend {
    pub fn new() -> Result<Self, String> {
        let mut sink_handle =
            rodio::DeviceSinkBuilder::open_default_sink().map_err(|e| e.to_string())?;
        sink_handle.log_on_drop(false);
        Ok(Self {
            _sink_handle: sink_handle,
            player: None,
            stream_meta: None,
            volume: 1.0,
        })
    }
}

impl AudioBackend for RodioAudioBackend {
    fn load_local(&mut self, path: &Path) -> Result<Option<u64>, String> {
        self.stream_meta = None;
        let file = std::fs::File::open(path)
            .map_err(|e| format!("unable to open {}: {e}", path.display()))?;
        let decoder = rodio::Decoder::try_from(file)
            .map_err(|e| format!("unable to decode {}: {e}", path.display()))?;
        let probed_ms = decoder.total_duration().map(|d| d.as_millis() as u64);

        if let Some(prev) = self.player.take() {
            prev.stop();
        }

        let player = rodio::Player::connect_new(self._sink_handle.mixer());
        player.set_volume(self.volume);
        player.append(decoder);
        player.play();
        self.player = Some(player);

        Ok(probed_ms)
    }

    fn load_stream(
        &mut self,
        reader: ProgressiveStreamReader,
        byte_len: Option<u64>,
        hint: Option<&str>,
    ) -> Result<Option<u64>, String> {
        let meta = ActiveStreamSeekMeta {
            temp_path: reader.temp_path().to_path_buf(),
            state: reader.state(),
            hint: hint.map(ToString::to_string),
        };
        let known_len = byte_len
            .or_else(|| meta.state.total_bytes())
            .filter(|&len| len > 0);
        // Only enable `with_seekable(true)` upfront when the full stream is already on disk.
        // For progressive DASH MP4 streams still downloading, `symphonia`'s `IsoMp4Reader` scans
        // every `moof`/`mdat` atom to EOF when `is_seekable == true` (blocking playback until 100%
        // downloaded). Opening with `with_seekable(false)` breaks immediately after `sidx` (~1.2 KB)
        // for instant playback start, and `seek()` transparently upgrades to a seekable reader.
        let open_seekable = meta.state.is_finished() && known_len.is_some();

        let mut builder = rodio::Decoder::builder()
            .with_data(BufReader::new(reader))
            .with_seekable(open_seekable);
        if let Some(len) = known_len {
            builder = builder.with_byte_len(len);
        }
        if let Some(ext) = hint {
            builder = builder.with_hint(ext);
        }
        let decoder = builder
            .build()
            .map_err(|e| format!("unable to decode HTTP stream: {e:?}"))?;
        let probed_ms = decoder.total_duration().map(|d| d.as_millis() as u64);

        if let Some(prev) = self.player.take() {
            prev.stop();
        }

        let player = rodio::Player::connect_new(self._sink_handle.mixer());
        player.set_volume(self.volume);
        player.append(decoder);
        player.play();
        self.player = Some(player);
        self.stream_meta = Some(meta);

        Ok(probed_ms)
    }

    fn play(&mut self) {
        if let Some(player) = &self.player {
            player.play();
        }
    }

    fn pause(&mut self) {
        if let Some(player) = &self.player {
            player.pause();
        }
    }

    fn stop(&mut self) {
        self.stream_meta = None;
        if let Some(player) = self.player.take() {
            player.stop();
        }
    }

    fn seek(&mut self, pos_ms: u64) -> Result<(), String> {
        if let Some(player) = &self.player {
            if player.try_seek(Duration::from_millis(pos_ms)).is_ok() {
                return Ok(());
            }

            // Stream was opened progressively (unseekable) for instant startup; upgrade to a
            // seekable reader on the same temp buffer now that the user requested a seek.
            if let Some(meta) = &self.stream_meta {
                if !meta.state.is_finished() {
                    return Err(
                        "Stream is still buffering in background; seek will be ready in a few seconds."
                            .to_string(),
                    );
                }
                let was_paused = player.is_paused();
                let total_len = meta
                    .state
                    .total_bytes()
                    .or_else(|| Some(meta.state.downloaded_bytes()))
                    .filter(|&len| len > 0);
                if let Some(len) = total_len {
                    let reader = ProgressiveStreamReader::open(
                        meta.temp_path.clone(),
                        Arc::clone(&meta.state),
                    )
                    .map_err(|e| format!("seek reopen failed: {e}"))?;
                    let mut builder = rodio::Decoder::builder()
                        .with_data(BufReader::new(reader))
                        .with_byte_len(len)
                        .with_seekable(true);
                    if let Some(ext) = meta.hint.as_deref() {
                        builder = builder.with_hint(ext);
                    }
                    let mut decoder = builder
                        .build()
                        .map_err(|e| format!("seek decoder rebuild failed: {e:?}"))?;
                    decoder
                        .try_seek(Duration::from_millis(pos_ms))
                        .map_err(|e| format!("seek failed: {e:?}"))?;

                    let new_player = rodio::Player::connect_new(self._sink_handle.mixer());
                    new_player.set_volume(self.volume);
                    new_player.append(decoder);
                    if was_paused {
                        new_player.pause();
                    } else {
                        new_player.play();
                    }
                    if let Some(old) = self.player.replace(new_player) {
                        old.stop();
                    }
                    return Ok(());
                }
            }

            return Err("Seek is available once the stream finishes buffering.".to_string());
        }
        Ok(())
    }

    fn set_volume(&mut self, volume: f32) {
        self.volume = volume.clamp(0.0, 1.0);
        if let Some(player) = &self.player {
            player.set_volume(self.volume);
        }
    }

    fn position_ms(&self) -> u64 {
        self.player
            .as_ref()
            .map(|p| p.get_pos().as_millis() as u64)
            .unwrap_or(0)
    }

    fn is_finished(&self) -> bool {
        self.player.as_ref().map(|p| p.empty()).unwrap_or(false)
    }
}
