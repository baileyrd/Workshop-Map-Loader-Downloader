//! Typed application configuration, persisted as TOML.
//!
//! Replaces the plugin's hand-rolled `workshopmaploader.cfg` format. A migration
//! helper for that old format can be added later (see [`Config::migrate_legacy_cfg`]).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{Result, WmlError};

/// UI language.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Language {
    #[default]
    English,
    French,
}

/// How the local map library is rendered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum DisplayMode {
    /// One map per row, with description (plugin mode 0).
    #[default]
    List,
    /// Grid of tiles (plugin mode 1).
    Tiles,
}

/// Connection settings for the BakkesMod RCON bridge.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct BakkesModConfig {
    pub host: String,
    pub port: u16,
    pub password: String,
    /// When false, the app never attempts the RCON bridge (manager-only mode).
    pub enabled: bool,
}

impl Default for BakkesModConfig {
    fn default() -> Self {
        // BakkesMod's RCON server defaults.
        Self {
            host: "127.0.0.1".to_string(),
            port: 9876,
            password: "password".to_string(),
            enabled: true,
        }
    }
}

/// Top-level persisted configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Folder the user keeps downloaded/managed maps in.
    pub maps_folder: PathBuf,
    pub language: Language,
    pub display_mode: DisplayMode,
    pub tiles_per_line: u32,
    pub controller_enabled: bool,
    pub controller_sensitivity: u32,
    pub controller_scroll_sensitivity: u32,
    pub antifreeze_fix: bool,
    /// Suppress the "download textures" prompt.
    pub dont_ask_textures: bool,
    /// Last app version whose changelog the user acknowledged.
    pub last_seen_version: String,
    pub bakkesmod: BakkesModConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            maps_folder: PathBuf::new(),
            language: Language::default(),
            display_mode: DisplayMode::default(),
            tiles_per_line: 6,
            controller_enabled: false,
            controller_sensitivity: 10,
            controller_scroll_sensitivity: 10,
            antifreeze_fix: false,
            dont_ask_textures: false,
            last_seen_version: String::new(),
            bakkesmod: BakkesModConfig::default(),
        }
    }
}

impl Config {
    /// Load config from `path`, or return defaults if the file does not exist.
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let text = std::fs::read_to_string(path)?;
        let cfg = toml::from_str(&text)?;
        Ok(cfg)
    }

    /// Persist config to `path`, creating parent directories as needed.
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = toml::to_string_pretty(self)?;
        std::fs::write(path, text)?;
        Ok(())
    }

    /// Migrate the plugin's legacy `workshopmaploader.cfg` (key = "value" lines)
    /// into a [`Config`]. Not yet implemented.
    pub fn migrate_legacy_cfg(_legacy: &Path) -> Result<Self> {
        Err(WmlError::NotImplemented("Config::migrate_legacy_cfg"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_toml() {
        let cfg = Config {
            tiles_per_line: 8,
            language: Language::French,
            display_mode: DisplayMode::Tiles,
            ..Config::default()
        };
        let text = toml::to_string_pretty(&cfg).unwrap();
        let parsed: Config = toml::from_str(&text).unwrap();
        assert_eq!(parsed.tiles_per_line, 8);
        assert_eq!(parsed.language, Language::French);
        assert_eq!(parsed.display_mode, DisplayMode::Tiles);
        assert_eq!(parsed.bakkesmod.port, 9876);
    }

    #[test]
    fn missing_file_yields_defaults() {
        let cfg = Config::load(Path::new("/nonexistent/path/wml.toml")).unwrap();
        assert_eq!(cfg.tiles_per_line, 6);
    }
}
