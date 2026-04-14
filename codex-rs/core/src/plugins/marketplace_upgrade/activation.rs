use super::ConfiguredGitMarketplace;
use codex_config::types::MarketplaceSourceType;
use serde::Deserialize;
use serde::Serialize;
use std::path::Path;
use std::path::PathBuf;
use tempfile::TempDir;
use tracing::warn;

const MARKETPLACE_INSTALL_METADATA_FILE: &str = ".codex-marketplace-install.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
struct ActivatedMarketplaceMetadata {
    source_type: MarketplaceSourceType,
    source: String,
    ref_name: Option<String>,
    sparse_paths: Vec<String>,
    revision: String,
}

pub(super) fn activated_marketplace_metadata_matches(
    root: &Path,
    marketplace: &ConfiguredGitMarketplace,
    revision: &str,
) -> bool {
    let metadata = match std::fs::read_to_string(activated_marketplace_metadata_path(root)) {
        Ok(metadata) => metadata,
        Err(_) => return false,
    };
    let metadata = match serde_json::from_str::<ActivatedMarketplaceMetadata>(&metadata) {
        Ok(metadata) => metadata,
        Err(err) => {
            warn!(
                marketplace = marketplace.name,
                error = %err,
                "failed to parse activated marketplace metadata"
            );
            return false;
        }
    };
    metadata == activated_marketplace_metadata(marketplace, revision)
}

pub(super) fn write_activated_marketplace_metadata(
    root: &Path,
    marketplace: &ConfiguredGitMarketplace,
    revision: &str,
) -> Result<(), String> {
    let metadata = activated_marketplace_metadata(marketplace, revision);
    let contents = serde_json::to_string_pretty(&metadata)
        .map_err(|err| format!("failed to serialize activated marketplace metadata: {err}"))?;
    std::fs::write(activated_marketplace_metadata_path(root), contents)
        .map_err(|err| format!("failed to write activated marketplace metadata: {err}"))
}

pub(super) fn activate_marketplace_root(
    destination: &Path,
    staged_dir: TempDir,
    after_activate: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    let staged_root = staged_dir.path();
    let Some(parent) = destination.parent() else {
        return Err(format!(
            "failed to determine marketplace install parent for {}",
            destination.display()
        ));
    };
    std::fs::create_dir_all(parent).map_err(|err| {
        format!(
            "failed to create marketplace install parent {}: {err}",
            parent.display()
        )
    })?;

    if destination.exists() {
        let backup_dir = tempfile::Builder::new()
            .prefix("marketplace-backup-")
            .tempdir_in(parent)
            .map_err(|err| {
                format!(
                    "failed to create marketplace backup directory in {}: {err}",
                    parent.display()
                )
            })?;
        let backup_root = backup_dir.path().join("root");
        std::fs::rename(destination, &backup_root).map_err(|err| {
            format!(
                "failed to move previous marketplace root out of the way at {}: {err}",
                destination.display()
            )
        })?;

        if let Err(err) = std::fs::rename(staged_root, destination) {
            let rollback_result = std::fs::rename(&backup_root, destination);
            return match rollback_result {
                Ok(()) => Err(format!(
                    "failed to activate upgraded marketplace at {}: {err}",
                    destination.display()
                )),
                Err(rollback_err) => {
                    let backup_path = backup_dir.keep().join("root");
                    Err(format!(
                        "failed to activate upgraded marketplace at {}: {err}; failed to restore previous marketplace root (left at {}): {rollback_err}",
                        destination.display(),
                        backup_path.display()
                    ))
                }
            };
        }

        if let Err(err) = after_activate() {
            let remove_result = std::fs::remove_dir_all(destination);
            let rollback_result =
                remove_result.and_then(|()| std::fs::rename(&backup_root, destination));
            return match rollback_result {
                Ok(()) => Err(err),
                Err(rollback_err) => {
                    let backup_path = backup_dir.keep().join("root");
                    Err(format!(
                        "{err}; failed to restore previous marketplace root at {} (left at {}): {rollback_err}",
                        destination.display(),
                        backup_path.display()
                    ))
                }
            };
        }

        return Ok(());
    }

    std::fs::rename(staged_root, destination).map_err(|err| {
        format!(
            "failed to activate upgraded marketplace at {}: {err}",
            destination.display()
        )
    })?;
    if let Err(err) = after_activate() {
        let remove_result = std::fs::remove_dir_all(destination);
        return match remove_result {
            Ok(()) => Err(err),
            Err(remove_err) => Err(format!(
                "{err}; failed to remove newly activated marketplace root at {}: {remove_err}",
                destination.display()
            )),
        };
    }

    Ok(())
}

fn activated_marketplace_metadata(
    marketplace: &ConfiguredGitMarketplace,
    revision: &str,
) -> ActivatedMarketplaceMetadata {
    ActivatedMarketplaceMetadata {
        source_type: MarketplaceSourceType::Git,
        source: marketplace.source.clone(),
        ref_name: marketplace.ref_name.clone(),
        sparse_paths: marketplace.sparse_paths.clone(),
        revision: revision.to_string(),
    }
}

fn activated_marketplace_metadata_path(root: &Path) -> PathBuf {
    root.join(MARKETPLACE_INSTALL_METADATA_FILE)
}
