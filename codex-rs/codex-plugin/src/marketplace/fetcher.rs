//! Marketplace fetching.

use super::schema::MarketplaceManifest;
use super::schema::MarketplaceSource;
use crate::MARKETPLACE_MANIFEST_FILE;
use crate::error::PluginError;
use crate::error::Result;
use std::path::Path;
use tokio::fs;
use tracing::debug;
use tracing::info;

/// Fetch a marketplace manifest from a source.
pub async fn fetch_marketplace(source: &MarketplaceSource) -> Result<MarketplaceManifest> {
    match source {
        MarketplaceSource::Url { url, headers } => fetch_from_url(url, headers).await,
        MarketplaceSource::GitHub {
            repo,
            ref_spec,
            path,
        } => fetch_from_github(repo, ref_spec.as_deref(), path.as_deref()).await,
        MarketplaceSource::Git {
            url,
            ref_spec,
            path,
        } => fetch_from_git(url, ref_spec.as_deref(), path.as_deref()).await,
        MarketplaceSource::File { path } => fetch_from_file(Path::new(path)).await,
        MarketplaceSource::Directory { path } => fetch_from_directory(Path::new(path)).await,
    }
}

/// Fetch marketplace from a URL.
async fn fetch_from_url(
    url: &str,
    headers: &std::collections::HashMap<String, String>,
) -> Result<MarketplaceManifest> {
    info!("Fetching marketplace from URL: {url}");

    let client = reqwest::Client::new();
    let mut request = client.get(url);

    for (key, value) in headers {
        request = request.header(key, value);
    }

    let response = request.send().await?;

    if !response.status().is_success() {
        return Err(PluginError::Marketplace(format!(
            "Failed to fetch marketplace: HTTP {}",
            response.status()
        )));
    }

    let content = response.text().await?;
    let manifest: MarketplaceManifest = serde_json::from_str(&content).map_err(|e| {
        PluginError::Marketplace(format!("Failed to parse marketplace manifest: {e}"))
    })?;

    debug!(
        "Fetched marketplace '{}' with {} plugins",
        manifest.name,
        manifest.plugins.len()
    );

    Ok(manifest)
}

/// Fetch marketplace from GitHub.
async fn fetch_from_github(
    repo: &str,
    ref_spec: Option<&str>,
    path: Option<&str>,
) -> Result<MarketplaceManifest> {
    let branch = ref_spec.unwrap_or("main");
    let file_path = path.unwrap_or(".codex-plugin/marketplace.json");

    // Use raw.githubusercontent.com for direct file access
    let url = format!("https://raw.githubusercontent.com/{repo}/{branch}/{file_path}");

    fetch_from_url(&url, &std::collections::HashMap::new()).await
}

/// Fetch marketplace from a Git URL.
async fn fetch_from_git(
    url: &str,
    ref_spec: Option<&str>,
    path: Option<&str>,
) -> Result<MarketplaceManifest> {
    // Clone to temp directory
    let temp_dir = std::env::temp_dir().join(format!("codex-mp-{}", std::process::id()));

    let mut cmd = tokio::process::Command::new("git");
    cmd.arg("clone")
        .arg("--depth")
        .arg("1")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());

    if let Some(r) = ref_spec {
        cmd.arg("--branch").arg(r);
    }

    cmd.arg(url).arg(&temp_dir);

    let status = cmd
        .status()
        .await
        .map_err(|e| PluginError::Git(format!("Failed to clone: {e}")))?;

    if !status.success() {
        return Err(PluginError::Git("Git clone failed".to_string()));
    }

    // Read manifest
    let manifest_path = if let Some(p) = path {
        temp_dir.join(p)
    } else {
        temp_dir
            .join(".codex-plugin")
            .join(MARKETPLACE_MANIFEST_FILE)
    };

    let result = fetch_from_file(&manifest_path).await;

    // Cleanup
    let _ = fs::remove_dir_all(&temp_dir).await;

    result
}

/// Fetch marketplace from a local file.
async fn fetch_from_file(path: &Path) -> Result<MarketplaceManifest> {
    debug!("Loading marketplace from file: {}", path.display());

    let content = fs::read_to_string(path).await.map_err(|e| {
        PluginError::Marketplace(format!(
            "Failed to read marketplace file {}: {e}",
            path.display()
        ))
    })?;

    let manifest: MarketplaceManifest = serde_json::from_str(&content).map_err(|e| {
        PluginError::Marketplace(format!(
            "Failed to parse marketplace manifest at {}: {e}",
            path.display()
        ))
    })?;

    Ok(manifest)
}

/// Fetch marketplace from a local directory.
async fn fetch_from_directory(dir: &Path) -> Result<MarketplaceManifest> {
    let manifest_path = dir.join(".codex-plugin").join(MARKETPLACE_MANIFEST_FILE);

    if manifest_path.exists() {
        return fetch_from_file(&manifest_path).await;
    }

    // Try root-level marketplace.json
    let root_manifest = dir.join(MARKETPLACE_MANIFEST_FILE);
    if root_manifest.exists() {
        return fetch_from_file(&root_manifest).await;
    }

    Err(PluginError::Marketplace(format!(
        "No marketplace manifest found in {}",
        dir.display()
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_fetch_from_file() {
        let dir = tempdir().unwrap();
        let manifest_path = dir.path().join("marketplace.json");

        std::fs::write(
            &manifest_path,
            r#"{
                "name": "test-marketplace",
                "owner": {"name": "Test"},
                "plugins": []
            }"#,
        )
        .unwrap();

        let manifest = fetch_from_file(&manifest_path).await.unwrap();
        assert_eq!(manifest.name, "test-marketplace");
    }

    #[tokio::test]
    async fn test_fetch_from_directory() {
        let dir = tempdir().unwrap();
        let manifest_dir = dir.path().join(".codex-plugin");
        std::fs::create_dir_all(&manifest_dir).unwrap();

        std::fs::write(
            manifest_dir.join("marketplace.json"),
            r#"{
                "name": "dir-marketplace",
                "owner": {"name": "Test"},
                "plugins": []
            }"#,
        )
        .unwrap();

        let manifest = fetch_from_directory(dir.path()).await.unwrap();
        assert_eq!(manifest.name, "dir-marketplace");
    }
}
