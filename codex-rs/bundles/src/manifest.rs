use crate::platform::RuntimeTarget;
use crate::platform::release_platform_key;
use crate::platform::url_platform_key;
use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use codex_client::build_reqwest_client_with_custom_ca;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;
use tokio::fs;

pub const DEFAULT_ARTIFACT_NAME: &str = "codex-primary-runtime";
pub const DEFAULT_STORAGE_BASE_URL: &str = "https://oaisidekickupdates.blob.core.windows.net/owl";
pub const LATEST_MANIFEST_FILE_NAME: &str = "LATEST.json";
pub const DEFAULT_RUNTIME_ROOT_DIRECTORY_NAME: &str = "codex-primary-runtime";

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexRuntimesConfig {
    pub runtimes: HashMap<String, InstallRuntimeOptions>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallRuntimeOptions {
    pub base_url: String,
    pub latest: Option<RuntimeReleaseManifest>,
    #[serde(rename = "latest-alpha")]
    pub latest_alpha: Option<RuntimeReleaseManifest>,
    #[serde(default)]
    pub versions: HashMap<String, RuntimeReleaseManifest>,
}

impl Default for InstallRuntimeOptions {
    fn default() -> Self {
        Self {
            base_url: DEFAULT_STORAGE_BASE_URL.to_string(),
            latest: None,
            latest_alpha: None,
            versions: HashMap::new(),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeReleaseManifest {
    pub bundle_format_version: Option<u32>,
    pub bundle_version: Option<String>,
    pub platforms: HashMap<String, RuntimePlatformManifest>,
    pub runtime_root_directory_name: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct RuntimePlatformManifest {
    pub digest: String,
    pub format: Option<String>,
    pub hash: String,
    pub path: Option<String>,
    pub providers: Vec<RuntimeProvider>,
    pub size: Option<u64>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct RuntimeProvider {
    pub url: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RuntimeArchiveManifest {
    pub(crate) archive_name: Option<String>,
    pub(crate) archive_sha256: String,
    pub(crate) archive_url: String,
    pub(crate) bundle_format_version: Option<u32>,
    pub(crate) bundle_version: Option<String>,
    pub(crate) runtime_root_directory_name: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BundleSelection {
    Channel(BundleChannel),
    Version(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BundleChannel {
    Latest,
    Alpha,
}

pub async fn read_runtimes_config(path: &Path) -> Result<CodexRuntimesConfig> {
    let raw = fs::read_to_string(path)
        .await
        .with_context(|| format!("failed to read runtime manifest {}", path.display()))?;
    serde_json::from_str(&raw)
        .with_context(|| format!("failed to parse runtime manifest {}", path.display()))
}

pub fn manifest_url(
    runtime_options: &InstallRuntimeOptions,
    artifact_name: &str,
    channel: BundleChannel,
    target: &RuntimeTarget,
) -> Result<String> {
    let channel_segments = match channel {
        BundleChannel::Latest => Vec::new(),
        BundleChannel::Alpha => vec!["alpha"],
    };
    let mut segments = vec![
        runtime_options.base_url.trim_end_matches('/').to_string(),
        artifact_name.to_string(),
    ];
    segments.extend(channel_segments.into_iter().map(ToOwned::to_owned));
    segments.push("latest".to_string());
    segments.push(url_platform_key(target)?);
    segments.push(LATEST_MANIFEST_FILE_NAME.to_string());
    Ok(segments.join("/"))
}

pub(crate) async fn resolve_archive_manifest(
    runtime_options: &InstallRuntimeOptions,
    selection: &BundleSelection,
    artifact_name: &str,
    target: &RuntimeTarget,
) -> Result<RuntimeArchiveManifest> {
    match selection {
        BundleSelection::Channel(BundleChannel::Latest) => {
            if let Some(release) = &runtime_options.latest {
                return release_manifest_for_target(release, target);
            }
            fetch_archive_manifest(&manifest_url(
                runtime_options,
                artifact_name,
                BundleChannel::Latest,
                target,
            )?)
            .await
        }
        BundleSelection::Channel(BundleChannel::Alpha) => {
            if let Some(release) = &runtime_options.latest_alpha {
                return release_manifest_for_target(release, target);
            }
            fetch_archive_manifest(&manifest_url(
                runtime_options,
                artifact_name,
                BundleChannel::Alpha,
                target,
            )?)
            .await
        }
        BundleSelection::Version(version) => {
            let release = runtime_options.versions.get(version).with_context(|| {
                format!("runtime manifest does not contain version `{version}`")
            })?;
            let mut manifest = release_manifest_for_target(release, target)?;
            if manifest.bundle_version.is_none() {
                manifest.bundle_version = Some(version.clone());
            }
            Ok(manifest)
        }
    }
}

fn release_manifest_for_target(
    release: &RuntimeReleaseManifest,
    target: &RuntimeTarget,
) -> Result<RuntimeArchiveManifest> {
    let platform_key = release_platform_key(target)?;
    let platform = release
        .platforms
        .get(&platform_key)
        .with_context(|| format!("runtime release does not contain platform `{platform_key}`"))?;
    if platform.hash != "sha256" {
        bail!("unsupported runtime archive hash `{}`", platform.hash);
    }
    let provider = platform
        .providers
        .first()
        .context("runtime platform manifest has no providers")?;
    Ok(RuntimeArchiveManifest {
        archive_name: platform.path.clone(),
        archive_sha256: platform.digest.clone(),
        archive_url: provider.url.clone(),
        bundle_format_version: release.bundle_format_version,
        bundle_version: release.bundle_version.clone(),
        runtime_root_directory_name: release.runtime_root_directory_name.clone(),
    })
}

async fn fetch_archive_manifest(url: &str) -> Result<RuntimeArchiveManifest> {
    let client = build_reqwest_client_with_custom_ca(reqwest::Client::builder())
        .context("failed to build HTTP client")?;
    let response = client
        .get(url)
        .header("User-Agent", "codex-bundles-installer")
        .send()
        .await
        .with_context(|| format!("failed to download runtime manifest {url}"))?;
    if !response.status().is_success() {
        bail!(
            "failed to download runtime manifest {url} ({} {})",
            response.status().as_u16(),
            response.status().canonical_reason().unwrap_or("unknown")
        );
    }
    response
        .json()
        .await
        .with_context(|| format!("failed to parse runtime manifest {url}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn test_target() -> RuntimeTarget {
        RuntimeTarget {
            arch: "arm64".to_string(),
            platform: "darwin".to_string(),
        }
    }

    #[test]
    fn builds_latest_manifest_url() {
        let options = InstallRuntimeOptions::default();
        let url = manifest_url(
            &options,
            DEFAULT_ARTIFACT_NAME,
            BundleChannel::Latest,
            &test_target(),
        )
        .expect("manifest url");
        assert_eq!(
            url,
            "https://oaisidekickupdates.blob.core.windows.net/owl/codex-primary-runtime/latest/darwin-arm64/LATEST.json"
        );
    }

    #[test]
    fn builds_alpha_manifest_url() {
        let options = InstallRuntimeOptions::default();
        let url = manifest_url(
            &options,
            DEFAULT_ARTIFACT_NAME,
            BundleChannel::Alpha,
            &test_target(),
        )
        .expect("manifest url");
        assert_eq!(
            url,
            "https://oaisidekickupdates.blob.core.windows.net/owl/codex-primary-runtime/alpha/latest/darwin-arm64/LATEST.json"
        );
    }
}
