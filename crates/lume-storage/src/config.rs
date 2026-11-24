use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::StorageError;

/// Sort field for tracks inside a playlist view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PlaylistSortField {
    #[default]
    Custom,
    RecentlyAdded,
    Title,
    Artist,
    Album,
    Duration,
}

/// Sort direction for playlist track ordering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SortDirection {
    #[default]
    Asc,
    Desc,
}

impl SortDirection {
    pub fn flip(self) -> Self {
        match self {
            Self::Asc => Self::Desc,
            Self::Desc => Self::Asc,
        }
    }
}

/// Per-playlist sort preference persisted in `config.toml`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct PlaylistSort {
    pub field: PlaylistSortField,
    pub direction: SortDirection,
}

impl PlaylistSort {
    pub fn select(&mut self, field: PlaylistSortField) {
        if self.field == field {
            self.direction = self.direction.flip();
        } else {
            self.field = field;
            self.direction = match field {
                PlaylistSortField::RecentlyAdded => SortDirection::Desc,
                _ => SortDirection::Asc,
            };
        }
    }

    pub fn toggle_direction(&mut self) {
        self.direction = self.direction.flip();
    }
}

/// Glassmorphism backdrop strategy in Browse mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum GlassmorphismMode {
    DynamicAlbumBlur,
    PureClear,
    #[default]
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
    pub prefer_youtube_fallback: bool,
    pub spotify_client_id: Option<String>,
    pub yt_dlp_path: Option<PathBuf>,
    pub local_folders: Vec<PathBuf>,
    pub playlist_sorts: BTreeMap<String, PlaylistSort>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            glassmorphism_mode: GlassmorphismMode::WindowsAcrylic,
            lyrics_visual_mode: LyricsVisualMode::SpicySmooth,
            reduced_motion: false,
            prefer_youtube_fallback: false,
            spotify_client_id: None,
            yt_dlp_path: None,
            local_folders: Vec::new(),
            playlist_sorts: BTreeMap::new(),
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
            GlassmorphismMode::WindowsAcrylic
        );
        assert_eq!(default_cfg.lyrics_visual_mode, LyricsVisualMode::SpicySmooth);

        let mut updated = default_cfg;
        updated.glassmorphism_mode = GlassmorphismMode::DynamicAlbumBlur;
        updated.lyrics_visual_mode = LyricsVisualMode::UltraLight;
        updated.local_folders.push(PathBuf::from(r"D:\Music"));
        updated.playlist_sorts.insert(
            "pl-1".to_string(),
            PlaylistSort {
                field: PlaylistSortField::Custom,
                direction: SortDirection::Desc,
            },
        );
        updated.save(&path).unwrap();

        let reloaded = AppConfig::load_or_default(&path).unwrap();
        assert_eq!(reloaded, updated);
        assert_eq!(
            reloaded.playlist_sorts.get("pl-1"),
            Some(&PlaylistSort {
                field: PlaylistSortField::Custom,
                direction: SortDirection::Desc,
            })
        );
    }

    #[test]
    fn app_config_ignores_legacy_close_to_tray_field() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            "glassmorphism_mode = \"windows_acrylic\"\nclose_to_tray = true\n",
        )
        .unwrap();

        let loaded = AppConfig::load_or_default(&path).unwrap();
        assert_eq!(loaded.glassmorphism_mode, GlassmorphismMode::WindowsAcrylic);
    }
}
