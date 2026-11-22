use std::path::Path;
use std::time::Duration;

use lume_core::{PlaybackStatus, Track};
use raw_window_handle::RawWindowHandle;
use souvlaki::{MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition, PlatformConfig};
use tokio::sync::mpsc::UnboundedSender;

use crate::views::SmtcAction;

/// Extracts a Win32 HWND integer from an `iced` window handle.
pub fn extract_hwnd(window: &dyn iced::window::Window) -> Option<usize> {
    let handle = window.window_handle().ok()?;
    match handle.as_raw() {
        RawWindowHandle::Win32(win32) => Some(win32.hwnd.get() as usize),
        _ => None,
    }
}

/// Manages Windows System Media Transport Controls (SMTC) via `souvlaki`.
pub struct SmtcController {
    controls: Option<MediaControls>,
}

impl Default for SmtcController {
    fn default() -> Self {
        Self::new()
    }
}

impl SmtcController {
    pub fn new() -> Self {
        Self { controls: None }
    }

    pub fn attach_with_hwnd(&mut self, hwnd_val: usize, tx: UnboundedSender<SmtcAction>) {
        if self.controls.is_some() {
            return;
        }
        let config = PlatformConfig {
            display_name: "Lume Music",
            dbus_name: "com.lumemusic.app",
            hwnd: Some(hwnd_val as *mut std::ffi::c_void),
        };
        let Ok(mut controls) = MediaControls::new(config) else {
            return;
        };
        let _ = controls.attach(move |event| {
            let mapped = match event {
                MediaControlEvent::Play => Some(SmtcAction::Play),
                MediaControlEvent::Pause => Some(SmtcAction::Pause),
                MediaControlEvent::Toggle => Some(SmtcAction::Toggle),
                MediaControlEvent::Next => Some(SmtcAction::Next),
                MediaControlEvent::Previous => Some(SmtcAction::Previous),
                MediaControlEvent::Stop => Some(SmtcAction::Stop),
                _ => None,
            };
            if let Some(action) = mapped {
                let _ = tx.send(action);
            }
        });
        self.controls = Some(controls);
    }

    pub fn set_metadata(&mut self, track: Option<&Track>, cover_path: Option<&Path>) {
        let Some(controls) = &mut self.controls else {
            return;
        };
        let Some(track) = track else {
            let _ = controls.set_metadata(MediaMetadata::default());
            return;
        };

        let cover_url = cover_path.map(|p| {
            let normalized = p.to_string_lossy().replace('\\', "/");
            if normalized.starts_with('/') {
                format!("file://{normalized}")
            } else {
                format!("file:///{normalized}")
            }
        });
        let artist = track.artist_display();
        let duration = (track.duration_ms > 0).then(|| Duration::from_millis(track.duration_ms));

        let _ = controls.set_metadata(MediaMetadata {
            title: Some(track.title.as_str()),
            album: track.album.as_deref(),
            artist: Some(artist.as_str()),
            cover_url: cover_url.as_deref(),
            duration,
        });
    }

    pub fn set_playback(&mut self, status: PlaybackStatus, position_ms: u64) {
        let Some(controls) = &mut self.controls else {
            return;
        };
        let pos = Some(MediaPosition(Duration::from_millis(position_ms)));
        let pb = match status {
            PlaybackStatus::Playing | PlaybackStatus::Buffering => {
                MediaPlayback::Playing { progress: pos }
            }
            PlaybackStatus::Paused => MediaPlayback::Paused { progress: pos },
            PlaybackStatus::Stopped => MediaPlayback::Stopped,
        };
        let _ = controls.set_playback(pb);
    }
}
