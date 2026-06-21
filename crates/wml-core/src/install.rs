//! Installing a catalog map into the local library.
//!
//! Ties together [`crate::catalog`], [`crate::download`], and [`crate::library`]:
//! create the map folder, write the metadata sidecar, fetch the preview, then
//! download and extract the release archive.

use std::path::{Path, PathBuf};

use crate::catalog::{MapResult, Release};
use crate::download::{self, Progress};
use crate::error::{Result, WmlError};
use crate::library::{self, MapMeta};

/// Progress reported during an install.
#[derive(Debug, Clone)]
pub enum InstallStage {
    /// Creating the folder and writing metadata.
    Preparing,
    /// Downloading the release archive.
    Downloading(Progress),
    /// Extracting the downloaded archive.
    Extracting,
    /// Finished; the map now lives at this folder.
    Done(PathBuf),
}

/// Download and install `release` of `map` into `maps_folder`.
///
/// `on_stage` is invoked as the install progresses. Returns the created map
/// folder. The zip is removed after a successful extraction.
pub async fn install_map<F>(
    client: &reqwest::Client,
    maps_folder: &Path,
    map: &MapResult,
    release: &Release,
    mut on_stage: F,
) -> Result<PathBuf>
where
    F: FnMut(InstallStage) + Send,
{
    if !maps_folder.is_dir() {
        return Err(WmlError::Config(format!(
            "maps folder does not exist: {}",
            maps_folder.display()
        )));
    }

    on_stage(InstallStage::Preparing);

    // Folder name, falling back to the project id if the name sanitizes to empty.
    let mut folder_name = library::safe_folder_name(&map.name);
    if folder_name.is_empty() {
        folder_name = format!("map_{}", map.id);
    }
    let dest = maps_folder.join(&folder_name);
    tokio::fs::create_dir_all(&dest).await?;

    // Metadata sidecar (`<folder_name>.json`), matching the library scanner.
    let meta = MapMeta {
        title: map.name.clone(),
        author: map.author.clone(),
        description: map.description.clone(),
        preview_url: map.preview_url.clone(),
    };
    library::write_meta(&dest, &folder_name, &meta)?;

    // Preview image is best-effort; failures don't abort the install.
    if !map.preview_url.is_empty() {
        let preview_path = dest.join(format!("{folder_name}.jfif"));
        let _ = download::download_to_file(client, &map.preview_url, &preview_path, |_| {}).await;
    }

    // Release archive.
    let zip_name = if release.zip_name.is_empty() {
        format!("{folder_name}.zip")
    } else {
        release.zip_name.clone()
    };
    let zip_path = dest.join(&zip_name);
    download::download_to_file(client, &release.download_url, &zip_path, |p| {
        on_stage(InstallStage::Downloading(p));
    })
    .await?;

    // Extraction is blocking; run it off the async worker.
    on_stage(InstallStage::Extracting);
    let zip_for_task = zip_path.clone();
    let dest_for_task = dest.clone();
    tokio::task::spawn_blocking(move || download::extract_zip(&zip_for_task, &dest_for_task))
        .await
        .map_err(|e| WmlError::Other(e.into()))??;

    // The archive is redundant once extracted.
    let _ = tokio::fs::remove_file(&zip_path).await;

    on_stage(InstallStage::Done(dest.clone()));
    Ok(dest)
}
