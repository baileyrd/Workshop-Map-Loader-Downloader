//! Typed application configuration, persisted as TOML.
//!
//! Replaces the plugin's hand-rolled `workshopmaploader.cfg` format. A migration
//! helper for that old format can be added later (see [`Config::migrate_legacy_cfg`]).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::Result;

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

    /// Load `config_path` if it exists; otherwise migrate the first existing
    /// legacy `.cfg` candidate (saving the result to `config_path`); otherwise
    /// return defaults.
    pub fn load_or_migrate(config_path: &Path, legacy_candidates: &[PathBuf]) -> Result<Self> {
        if config_path.exists() {
            return Self::load(config_path);
        }
        for candidate in legacy_candidates {
            if candidate.exists() {
                let cfg = Self::migrate_legacy_cfg(candidate)?;
                cfg.save(config_path)?;
                return Ok(cfg);
            }
        }
        Ok(Self::default())
    }

    /// Migrate the plugin's legacy `workshopmaploader.cfg` (lines of the form
    /// `Key = "value"`) into a [`Config`].
    pub fn migrate_legacy_cfg(legacy: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(legacy)?;
        Ok(Self::from_legacy_str(&text))
    }

    /// Parse the legacy cfg format. Unknown/missing keys keep their defaults, so
    /// this tolerates the several historical cfg layouts the plugin produced.
    fn from_legacy_str(text: &str) -> Self {
        let mut values: HashMap<&str, &str> = HashMap::new();
        for line in text.lines() {
            let Some((key, rest)) = line.split_once('=') else {
                continue;
            };
            // The value is whatever sits between the first and last quote.
            if let (Some(a), Some(b)) = (rest.find('"'), rest.rfind('"')) {
                if b > a {
                    values.insert(key.trim(), &rest[a + 1..b]);
                }
            }
        }

        let mut cfg = Self::default();
        if let Some(v) = values.get("MapsFolderPath") {
            cfg.maps_folder = PathBuf::from(v);
        }
        if values.get("Language") == Some(&"1") {
            cfg.language = Language::French;
        }
        if values.get("MapsDisplayMode") == Some(&"1") {
            cfg.display_mode = DisplayMode::Tiles;
        }
        if let Some(n) = values.get("nbTilesPerLine").and_then(|v| v.parse().ok()) {
            cfg.tiles_per_line = n;
        }
        if let Some(n) = values
            .get("ControllerSensitivity")
            .and_then(|v| v.parse().ok())
        {
            cfg.controller_sensitivity = n;
        }
        if let Some(n) = values
            .get("ControllerScrollSensitivity")
            .and_then(|v| v.parse().ok())
        {
            cfg.controller_scroll_sensitivity = n;
        }
        cfg.controller_enabled = values.get("UseController") == Some(&"1");
        cfg.antifreeze_fix = values.get("EnableAntiFreezeFix") == Some(&"1");
        cfg.dont_ask_textures = values.get("dontask") == Some(&"1");
        if let Some(v) = values.get("PluginVersion") {
            cfg.last_seen_version = v.to_string();
        }
        cfg
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

    #[test]
    fn migrates_legacy_cfg() {
        // A short legacy file (missing the later keys) to prove tolerance.
        let legacy = concat!(
            "MapsFolderPath = \"C:/Users\\snipj\\AppData\\Roaming\\bakkesmod\\WorkshopMaps\"\n",
            "Language = \"1\"\n",
            "UnzipMethod = \"Powershell\"\n",
            "HasSeeNewUpdateAlert = \"1\"\n",
            "dontask = \"1\"\n",
            "MapsDisplayMode = \"1\"\n",
            "nbTilesPerLine = \"5\"\n",
            "ControllerSensitivity = \"12\"\n",
            "ControllerScrollSensitivity = \"7\"\n",
        );
        let cfg = Config::from_legacy_str(legacy);
        assert_eq!(
            cfg.maps_folder,
            PathBuf::from("C:/Users\\snipj\\AppData\\Roaming\\bakkesmod\\WorkshopMaps")
        );
        assert_eq!(cfg.language, Language::French);
        assert_eq!(cfg.display_mode, DisplayMode::Tiles);
        assert_eq!(cfg.tiles_per_line, 5);
        assert_eq!(cfg.controller_sensitivity, 12);
        assert_eq!(cfg.controller_scroll_sensitivity, 7);
        assert!(cfg.dont_ask_textures);
        // Keys absent from this short file keep their defaults.
        assert!(!cfg.controller_enabled);
        assert!(!cfg.antifreeze_fix);
    }
}
