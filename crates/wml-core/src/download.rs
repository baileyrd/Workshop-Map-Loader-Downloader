//! Streamed downloads (with progress) and in-process zip extraction.
//!
//! This replaces the plugin's fragile "shell out to PowerShell / a `.bat` +
//! VBScript" unzip path with the cross-platform [`zip`] crate.

use std::path::{Path, PathBuf};

use futures_util::StreamExt;
use tokio::io::AsyncWriteExt;

use crate::error::Result;

/// Progress for an in-flight download.
#[derive(Debug, Clone, Copy)]
pub struct Progress {
    /// Bytes received so far.
    pub downloaded: u64,
    /// Total size if the server reported `Content-Length`.
    pub total: Option<u64>,
}

impl Progress {
    /// Fraction in `0.0..=1.0`, or `None` if total is unknown.
    pub fn fraction(&self) -> Option<f32> {
        self.total.map(|t| {
            if t == 0 {
                0.0
            } else {
                (self.downloaded as f32 / t as f32).clamp(0.0, 1.0)
            }
        })
    }
}

/// Download `url` to `dest`, invoking `on_progress` as bytes arrive.
pub async fn download_to_file<F>(
    client: &reqwest::Client,
    url: &str,
    dest: &Path,
    mut on_progress: F,
) -> Result<()>
where
    F: FnMut(Progress),
{
    let resp = client.get(url).send().await?.error_for_status()?;
    let total = resp.content_length();

    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let mut file = tokio::fs::File::create(dest).await?;

    let mut downloaded: u64 = 0;
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        downloaded += chunk.len() as u64;
        file.write_all(&chunk).await?;
        on_progress(Progress { downloaded, total });
    }
    file.flush().await?;
    Ok(())
}

/// Extract every entry of `zip_path` into `dest_dir`. Blocking; call via
/// [`tokio::task::spawn_blocking`] from async contexts.
pub fn extract_zip(zip_path: &Path, dest_dir: &Path) -> Result<Vec<PathBuf>> {
    let file = std::fs::File::open(zip_path)?;
    let mut archive = zip::ZipArchive::new(file)?;
    let mut written = Vec::new();

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        // `enclosed_name` rejects path-traversal entries (e.g. `../`).
        let Some(rel) = entry.enclosed_name() else {
            continue;
        };
        let out_path = dest_dir.join(rel);

        if entry.is_dir() {
            std::fs::create_dir_all(&out_path)?;
        } else {
            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut out = std::fs::File::create(&out_path)?;
            std::io::copy(&mut entry, &mut out)?;
            written.push(out_path);
        }
    }
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_fraction() {
        let p = Progress { downloaded: 50, total: Some(100) };
        assert_eq!(p.fraction(), Some(0.5));
        let p = Progress { downloaded: 50, total: None };
        assert_eq!(p.fraction(), None);
    }
}
