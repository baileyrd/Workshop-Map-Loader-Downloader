//! The local map library: scanning the maps folder and modelling each map.
//!
//! Layout convention (inherited from the plugin): the maps folder contains one
//! subfolder per map. Each map folder may contain a `.upk`/`.udk` map file, a
//! preview image, a `.zip` (if not yet extracted), and a `<foldername>.json`
//! metadata sidecar.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::Result;

/// Metadata sidecar written next to a map (`<foldername>.json`).
///
/// Field names match the plugin's on-disk format for backward compatibility.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MapMeta {
    #[serde(rename = "Title", default)]
    pub title: String,
    #[serde(rename = "Author", default)]
    pub author: String,
    #[serde(rename = "Description", default)]
    pub description: String,
    #[serde(rename = "PreviewUrl", default)]
    pub preview_url: String,
}

/// A single map discovered in the library.
#[derive(Debug, Clone)]
pub struct Map {
    /// The map's own folder.
    pub folder: PathBuf,
    /// Display name (from sidecar, else derived from the folder name).
    pub name: String,
    pub author: String,
    pub description: String,
    /// The loadable map file, if present.
    pub upk_file: Option<PathBuf>,
    /// A not-yet-extracted archive, if present.
    pub zip_file: Option<PathBuf>,
    /// A preview image, if present.
    pub preview_image: Option<PathBuf>,
    /// Parsed sidecar, if one was found.
    pub meta: Option<MapMeta>,
}

impl Map {
    /// True when the map has a loadable `.upk`/`.udk` file.
    pub fn is_loadable(&self) -> bool {
        self.upk_file.is_some()
    }

    /// True when the map is only an archive that still needs extracting.
    pub fn needs_extraction(&self) -> bool {
        self.upk_file.is_none() && self.zip_file.is_some()
    }
}

const MAP_EXTS: &[&str] = &["upk", "udk"];
const IMAGE_EXTS: &[&str] = &["png", "jpg", "jpeg", "jfif"];

fn ext_matches(path: &Path, exts: &[&str]) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| exts.iter().any(|x| x.eq_ignore_ascii_case(e)))
        .unwrap_or(false)
}

fn folder_display_name(folder: &Path) -> String {
    folder
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .replace('_', " ")
}

/// Scan a single map folder into a [`Map`].
pub fn scan_map_folder(folder: &Path) -> Result<Map> {
    let folder_name = folder
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .to_string();

    let mut upk_file = None;
    let mut zip_file = None;
    let mut preview_image = None;
    let mut meta: Option<MapMeta> = None;

    for entry in std::fs::read_dir(folder)? {
        let path = entry?.path();
        if path.is_dir() {
            continue;
        }

        if upk_file.is_none() && ext_matches(&path, MAP_EXTS) {
            upk_file = Some(path.clone());
        }
        if zip_file.is_none() && ext_matches(&path, &["zip"]) {
            zip_file = Some(path.clone());
        }
        if preview_image.is_none() && ext_matches(&path, IMAGE_EXTS) {
            preview_image = Some(path.clone());
        }

        // The sidecar is named after the folder, e.g. `MyMap/MyMap.json`.
        if meta.is_none() && ext_matches(&path, &["json"]) {
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or_default();
            if stem == folder_name {
                if let Ok(text) = std::fs::read_to_string(&path) {
                    meta = serde_json::from_str::<MapMeta>(&text).ok();
                }
            }
        }
    }

    let (name, author, description) = match &meta {
        Some(m) if !m.title.is_empty() => {
            (m.title.clone(), m.author.clone(), m.description.clone())
        }
        _ => (folder_display_name(folder), String::new(), String::new()),
    };

    Ok(Map {
        folder: folder.to_path_buf(),
        name,
        author,
        description,
        upk_file,
        zip_file,
        preview_image,
        meta,
    })
}

/// Scan the maps folder, returning one [`Map`] per immediate subfolder.
pub fn scan_maps(maps_folder: &Path) -> Result<Vec<Map>> {
    let mut maps = Vec::new();
    if !maps_folder.is_dir() {
        return Ok(maps);
    }
    for entry in std::fs::read_dir(maps_folder)? {
        let path = entry?.path();
        if path.is_dir() {
            maps.push(scan_map_folder(&path)?);
        }
    }
    maps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(maps)
}

/// Case-insensitive substring filter over a map list (the plugin's Ctrl+F).
pub fn filter_maps<'a>(maps: &'a [Map], keyword: &str) -> Vec<&'a Map> {
    let needle = keyword.to_lowercase();
    if needle.is_empty() {
        return maps.iter().collect();
    }
    maps.iter()
        .filter(|m| m.name.to_lowercase().contains(&needle))
        .collect()
}

/// Write a metadata sidecar next to a map (`<name>.json`).
pub fn write_meta(map_folder: &Path, name: &str, meta: &MapMeta) -> Result<PathBuf> {
    let path = map_folder.join(format!("{name}.json"));
    let text = serde_json::to_string(meta)?;
    std::fs::write(&path, text)?;
    Ok(path)
}

/// Turn an arbitrary map name into a filesystem-safe folder name.
pub fn safe_folder_name(name: &str) -> String {
    let mut out: String = name.replace(' ', "_");
    out.retain(|c| {
        !matches!(
            c,
            '/' | '\\' | '?' | ':' | '*' | '"' | '<' | '>' | '|' | '-' | '#'
        )
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn scans_a_map_with_sidecar() {
        let tmp = std::env::temp_dir().join(format!("wml-test-{}", std::process::id()));
        let map_dir = tmp.join("Cool_Map");
        fs::create_dir_all(&map_dir).unwrap();
        fs::write(map_dir.join("Cool_Map.upk"), b"x").unwrap();
        fs::write(map_dir.join("Cool_Map.png"), b"x").unwrap();
        fs::write(
            map_dir.join("Cool_Map.json"),
            br#"{"Title":"Cool Map","Author":"Vync","Description":"fun"}"#,
        )
        .unwrap();

        let maps = scan_maps(&tmp).unwrap();
        assert_eq!(maps.len(), 1);
        let m = &maps[0];
        assert_eq!(m.name, "Cool Map");
        assert_eq!(m.author, "Vync");
        assert!(m.is_loadable());
        assert!(m.preview_image.is_some());

        fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn safe_folder_name_strips_specials() {
        assert_eq!(safe_folder_name("My: Cool/Map #1"), "My_CoolMap_1");
    }
}
