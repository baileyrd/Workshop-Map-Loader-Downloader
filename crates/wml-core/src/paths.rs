//! Locating on-disk locations: app config/data dirs, the Rocket League install,
//! and the `CookedPCConsole` folder used for workshop textures.

use std::path::{Path, PathBuf};

use directories::ProjectDirs;

use crate::error::{Result, WmlError};

const QUALIFIER: &str = "";
const ORGANIZATION: &str = "WorkshopMapLoader";
const APPLICATION: &str = "WorkshopMapLoader";

fn project_dirs() -> Result<ProjectDirs> {
    ProjectDirs::from(QUALIFIER, ORGANIZATION, APPLICATION)
        .ok_or_else(|| WmlError::Config("could not determine OS config directory".into()))
}

/// Directory for app data (cached preview images, downloaded assets, logos).
pub fn data_dir() -> Result<PathBuf> {
    Ok(project_dirs()?.data_dir().to_path_buf())
}

/// Path to the main config file (`config.toml`).
pub fn config_file() -> Result<PathBuf> {
    Ok(project_dirs()?.config_dir().join("config.toml"))
}

/// Directory used to cache downloaded preview thumbnails.
pub fn preview_cache_dir() -> Result<PathBuf> {
    Ok(data_dir()?.join("preview-cache"))
}

/// Best-effort detection of the Rocket League install directory.
///
/// To be implemented: parse Steam's `libraryfolders.vdf` and the Epic Games
/// launcher manifests. Returns `None` until then so callers fall back to a
/// user-configured path.
pub fn detect_rocket_league_install() -> Option<PathBuf> {
    None
}

/// Given a Rocket League install root, the path to `CookedPCConsole`.
pub fn cooked_pc_console(rl_install: &Path) -> PathBuf {
    rl_install.join("TAGame").join("CookedPCConsole")
}

/// Likely locations of the plugin's legacy `workshopmaploader.cfg`, for one-time
/// migration. Windows-only for now (BakkesMod's data dir lives under `%APPDATA%`);
/// other platforms return an empty list.
pub fn legacy_cfg_candidates() -> Vec<PathBuf> {
    #[cfg_attr(not(windows), allow(unused_mut))]
    let mut candidates = Vec::new();
    #[cfg(windows)]
    if let Ok(appdata) = std::env::var("APPDATA") {
        candidates.push(
            PathBuf::from(appdata)
                .join("bakkesmod")
                .join("bakkesmod")
                .join("data")
                .join("WorkshopMapLoader")
                .join("workshopmaploader.cfg"),
        );
    }
    candidates
}
