use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::StorageError;

/// Glassmorphism backdrop strategy in Browse mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum GlassmorphismMode {
    #[default]
    DynamicAlbumBlur,
    WindowsAcrylic,
}

/// Lyrics visual intensity preset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum LyricsVisualMode {
    #[default]
    SpicySmooth,
    UltraLight,
}

/// Persistent user preferences stored in `%APPDATA%\LumeMusic\config.toml`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub glassmorphism_mode: GlassmorphismMode,
    pub lyrics_visual_mode: LyricsVisualMode,
    pub reduced_motion: bool,
    pub close_to_tray: bool,
    pub prefer_youtube_fallback: bool,
    pub spotify_client_id: Option<String>,
    pub yt_dlp_path: Option<PathBuf>,
    pub local_folders: Vec<PathBuf>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            glassmorphism_mode: GlassmorphismMode::DynamicAlbumBlur,
            lyrics_visual_mode: LyricsVisualMode::SpicySmooth,
            reduced_motion: false,
            close_to_tray: true,
            prefer_youtube_fallback: false,
            spotify_client_id: None,
            yt_dlp_path: None,
            local_folders: Vec::new(),
        }
    }
}

impl AppConfig {
    pub fn default_config_path() -> PathBuf {
        let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        base.join("LumeMusic").join("config.toml")
    }

    pub fn load_or_default(path: &Path) -> Result<Self, StorageError> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let raw = std::fs::read_to_string(path)?;
        toml::from_str(&raw).map_err(|e| StorageError::Serialization(e.to_string()))
    }

    pub fn save(&self, path: &Path) -> Result<(), StorageError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let body =
            toml::to_string_pretty(self).map_err(|e| StorageError::Serialization(e.to_string()))?;
        std::fs::write(path, body)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn app_config_toml_round_trip_defaults_match_spec() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.toml");

        let default_cfg = AppConfig::load_or_default(&path).unwrap();
        assert_eq!(
            default_cfg.glassmorphism_mode,
            GlassmorphismMode::DynamicAlbumBlur
        );
        assert_eq!(default_cfg.lyrics_visual_mode, LyricsVisualMode::SpicySmooth);
        assert!(default_cfg.close_to_tray);

        let mut updated = default_cfg;
        updated.glassmorphism_mode = GlassmorphismMode::WindowsAcrylic;
        updated.lyrics_visual_mode = LyricsVisualMode::UltraLight;
        updated.close_to_tray = false;
        updated.local_folders.push(PathBuf::from(r"D:\Music"));
        updated.save(&path).unwrap();

        let reloaded = AppConfig::load_or_default(&path).unwrap();
        assert_eq!(reloaded, updated);
    }
}
