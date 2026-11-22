use std::path::Path;
use std::time::Duration;

use rodio::Source;

/// Hardware-independent playback backend trait for deterministic testing and WASAPI playback.
pub trait AudioBackend: Send {
    fn load_local(&mut self, path: &Path) -> Result<Option<u64>, String>;
    fn play(&mut self);
    fn pause(&mut self);
    fn stop(&mut self);
    fn seek(&mut self, pos_ms: u64) -> Result<(), String>;
    fn set_volume(&mut self, volume: f32);
    fn position_ms(&self) -> u64;
    fn is_finished(&self) -> bool;
}

/// Real Windows audio output backend powered by `rodio 0.23` (`symphonia` + WASAPI).
pub struct RodioAudioBackend {
    _sink_handle: rodio::MixerDeviceSink,
    player: Option<rodio::Player>,
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
            volume: 1.0,
        })
    }
}

impl AudioBackend for RodioAudioBackend {
    fn load_local(&mut self, path: &Path) -> Result<Option<u64>, String> {
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
        if let Some(player) = self.player.take() {
            player.stop();
        }
    }

    fn seek(&mut self, pos_ms: u64) -> Result<(), String> {
        if let Some(player) = &self.player {
            player
                .try_seek(Duration::from_millis(pos_ms))
                .map_err(|e| format!("seek failed: {e:?}"))?;
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
