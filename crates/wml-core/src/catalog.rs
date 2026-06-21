//! Client for the rocketleaguemaps.us map catalog.
//!
//! The backend is a GitLab instance (`celab.jetfox.ovh`). We list projects via
//! the search endpoint, then fetch each project's releases for the actual
//! download/preview asset links.
//!
//! NOTE: backend liveness must be verified; if the host moves, only [`DEFAULT_BASE`]
//! and the response structs below need to change.

use serde::Deserialize;

use crate::error::Result;

/// Base URL of the catalog API.
pub const DEFAULT_BASE: &str = "https://celab.jetfox.ovh/api/v4";

/// A downloadable release of a map.
#[derive(Debug, Clone)]
pub struct Release {
    pub name: String,
    pub tag_name: String,
    pub description: String,
    /// Sanitized archive file name.
    pub zip_name: String,
    pub download_url: String,
    pub preview_url: String,
}

/// A single search result (one map / project).
#[derive(Debug, Clone)]
pub struct MapResult {
    pub id: u64,
    pub name: String,
    pub description: String,
    pub author: String,
    pub preview_url: String,
    pub releases: Vec<Release>,
}

// --- Wire types (GitLab API shapes) ---------------------------------------

#[derive(Debug, Deserialize)]
struct GitlabProject {
    id: u64,
    name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    namespace: Namespace,
}

#[derive(Debug, Default, Deserialize)]
struct Namespace {
    #[serde(default)]
    path: String,
}

#[derive(Debug, Deserialize)]
struct GitlabRelease {
    #[serde(default)]
    name: String,
    #[serde(default)]
    tag_name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    assets: Assets,
}

#[derive(Debug, Default, Deserialize)]
struct Assets {
    #[serde(default)]
    links: Vec<AssetLink>,
}

#[derive(Debug, Deserialize)]
struct AssetLink {
    #[serde(default)]
    name: String,
    #[serde(default)]
    url: String,
}

// --------------------------------------------------------------------------

/// HTTP client for the catalog.
#[derive(Clone)]
pub struct CatalogClient {
    http: reqwest::Client,
    base: String,
}

impl Default for CatalogClient {
    fn default() -> Self {
        Self::new(DEFAULT_BASE)
    }
}

impl CatalogClient {
    pub fn new(base: impl Into<String>) -> Self {
        Self {
            http: reqwest::Client::new(),
            base: base.into(),
        }
    }

    /// Search for maps by keyword (1-based page index), resolving releases for each.
    pub async fn search(&self, keyword: &str, page: u32) -> Result<Vec<MapResult>> {
        let url = format!("{}/projects/", self.base);
        let projects: Vec<GitlabProject> = self
            .http
            .get(&url)
            .query(&[("search", keyword), ("page", &page.to_string())])
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        let mut results = Vec::with_capacity(projects.len());
        for project in projects {
            let releases = self.releases(project.id).await.unwrap_or_default();
            let preview_url = releases
                .first()
                .map(|r| r.preview_url.clone())
                .unwrap_or_default();
            results.push(MapResult {
                id: project.id,
                name: project.name,
                description: project.description.unwrap_or_default(),
                author: project.namespace.path,
                preview_url,
                releases,
            });
        }
        Ok(results)
    }

    /// Fetch the releases (download/preview asset links) for a project.
    pub async fn releases(&self, project_id: u64) -> Result<Vec<Release>> {
        let url = format!("{}/projects/{}/releases", self.base, project_id);
        let raw: Vec<GitlabRelease> = self
            .http
            .get(&url)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        Ok(raw.into_iter().map(into_release).collect())
    }
}

fn into_release(r: GitlabRelease) -> Release {
    // The plugin treats links[0] as the preview image and links[1] as the zip.
    let preview_url = r
        .assets
        .links
        .first()
        .map(|l| l.url.clone())
        .unwrap_or_default();
    let (download_url, raw_zip_name) = r
        .assets
        .links
        .get(1)
        .map(|l| (l.url.clone(), l.name.clone()))
        .unwrap_or_default();

    Release {
        name: r.name,
        tag_name: r.tag_name,
        description: r.description.unwrap_or_default(),
        zip_name: sanitize_zip_name(&raw_zip_name),
        download_url,
        preview_url,
    }
}

/// Strip characters that are unsafe in a file name.
fn sanitize_zip_name(name: &str) -> String {
    name.chars()
        .filter(|c| {
            !matches!(
                c,
                '/' | '\\' | '?' | ':' | '*' | '"' | '<' | '>' | '|' | '#' | '\'' | '`'
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_zip_names() {
        assert_eq!(sanitize_zip_name("my:map?.zip"), "mymap.zip");
    }
}
