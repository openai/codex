use super::MarketplaceSource;
use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use codex_core::plugins::validate_marketplace_root;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

const MARKETPLACE_ADD_SOURCE_FILE: &str = ".codex-marketplace-source";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct MarketplaceInstallMetadata {
    pub(super) source_id: String,
    pub(super) source: InstalledMarketplaceSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum InstalledMarketplaceSource {
    LocalDirectory {
        path: PathBuf,
    },
    Git {
        url: String,
        ref_name: Option<String>,
        sparse_paths: Vec<String>,
    },
}

pub(super) fn installed_marketplace_root_for_source(
    install_root: &Path,
    source_id: &str,
) -> Result<Option<PathBuf>> {
    let entries = fs::read_dir(install_root).with_context(|| {
        format!(
            "failed to read marketplace install directory {}",
            install_root.display()
        )
    })?;
    for entry in entries {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let root = entry.path();
        let metadata_path = root.join(MARKETPLACE_ADD_SOURCE_FILE);
        if !metadata_path.is_file() {
            continue;
        }
        let metadata = read_marketplace_source_metadata(&root)?;
        if metadata
            .as_ref()
            .is_some_and(|metadata| metadata.source_id == source_id)
            && validate_marketplace_root(&root).is_ok()
        {
            return Ok(Some(root));
        }
    }
    Ok(None)
}

pub(super) fn write_marketplace_source_metadata(
    root: &Path,
    metadata: &MarketplaceInstallMetadata,
) -> Result<()> {
    let source = match &metadata.source {
        InstalledMarketplaceSource::LocalDirectory { path } => serde_json::json!({
            "kind": "directory",
            "path": path,
        }),
        InstalledMarketplaceSource::Git {
            url,
            ref_name,
            sparse_paths,
        } => serde_json::json!({
            "kind": "git",
            "url": url,
            "ref": ref_name,
            "sparsePaths": sparse_paths,
        }),
    };
    let content = serde_json::to_string_pretty(&serde_json::json!({
        "version": 1,
        "sourceId": metadata.source_id,
        "source": source,
    }))?;
    fs::write(root.join(MARKETPLACE_ADD_SOURCE_FILE), content).with_context(|| {
        format!(
            "failed to write marketplace source metadata in {}",
            root.display()
        )
    })
}

pub(super) fn read_marketplace_source_metadata(
    root: &Path,
) -> Result<Option<MarketplaceInstallMetadata>> {
    let path = root.join(MARKETPLACE_ADD_SOURCE_FILE);
    if !path.is_file() {
        return Ok(None);
    }
    let content = fs::read_to_string(&path).with_context(|| {
        format!(
            "failed to read marketplace source metadata {}",
            path.display()
        )
    })?;
    if !content.trim_start().starts_with('{') {
        return Ok(Some(MarketplaceInstallMetadata {
            source_id: content.trim().to_string(),
            source: InstalledMarketplaceSource::LocalDirectory {
                path: root.to_path_buf(),
            },
        }));
    }

    let json: serde_json::Value = serde_json::from_str(&content).with_context(|| {
        format!(
            "failed to parse marketplace source metadata {}",
            path.display()
        )
    })?;
    let source_id = json
        .get("sourceId")
        .and_then(serde_json::Value::as_str)
        .context("marketplace source metadata is missing sourceId")?
        .to_string();
    let source = json
        .get("source")
        .context("marketplace source metadata is missing source")?;
    let kind = source
        .get("kind")
        .and_then(serde_json::Value::as_str)
        .context("marketplace source metadata is missing source.kind")?;
    let source = match kind {
        "directory" => {
            let path = source
                .get("path")
                .and_then(serde_json::Value::as_str)
                .context("marketplace directory metadata is missing path")?;
            InstalledMarketplaceSource::LocalDirectory {
                path: PathBuf::from(path),
            }
        }
        "git" => {
            let url = source
                .get("url")
                .and_then(serde_json::Value::as_str)
                .context("marketplace git metadata is missing url")?
                .to_string();
            let ref_name = source
                .get("ref")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string);
            let sparse_paths = source
                .get("sparsePaths")
                .and_then(serde_json::Value::as_array)
                .map(|paths| {
                    paths
                        .iter()
                        .filter_map(serde_json::Value::as_str)
                        .map(str::to_string)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            InstalledMarketplaceSource::Git {
                url,
                ref_name,
                sparse_paths,
            }
        }
        other => bail!("unsupported marketplace source metadata kind `{other}`"),
    };
    Ok(Some(MarketplaceInstallMetadata { source_id, source }))
}

impl MarketplaceInstallMetadata {
    pub(super) fn from_source(source: &MarketplaceSource, sparse_paths: &[String]) -> Self {
        let source_id = if sparse_paths.is_empty() {
            source.source_id().to_string()
        } else {
            format!(
                "{}?sparse={}",
                source.source_id(),
                serde_json::to_string(sparse_paths).unwrap_or_else(|_| "[]".to_string())
            )
        };
        let source = match source {
            MarketplaceSource::LocalDirectory { path, .. } => {
                InstalledMarketplaceSource::LocalDirectory { path: path.clone() }
            }
            MarketplaceSource::Git { url, ref_name, .. } => InstalledMarketplaceSource::Git {
                url: url.clone(),
                ref_name: ref_name.clone(),
                sparse_paths: sparse_paths.to_vec(),
            },
        };
        Self { source_id, source }
    }
}
