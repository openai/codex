use crate::plugins::marketplace_install_root;
use crate::plugins::validate_plugin_segment;
use codex_config::remove_user_marketplace;
use codex_utils_absolute_path::AbsolutePathBuf;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketplaceRemoveRequest {
    pub marketplace_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketplaceRemoveOutcome {
    pub marketplace_name: String,
    pub removed_installed_root: Option<AbsolutePathBuf>,
}

#[derive(Debug, thiserror::Error)]
pub enum MarketplaceRemoveError {
    #[error("{0}")]
    InvalidRequest(String),
    #[error("{0}")]
    Internal(String),
}

pub async fn remove_marketplace(
    codex_home: PathBuf,
    request: MarketplaceRemoveRequest,
) -> Result<MarketplaceRemoveOutcome, MarketplaceRemoveError> {
    tokio::task::spawn_blocking(move || remove_marketplace_sync(codex_home.as_path(), request))
        .await
        .map_err(|err| {
            MarketplaceRemoveError::Internal(format!("failed to remove marketplace: {err}"))
        })?
}

fn remove_marketplace_sync(
    codex_home: &Path,
    request: MarketplaceRemoveRequest,
) -> Result<MarketplaceRemoveOutcome, MarketplaceRemoveError> {
    let marketplace_name = request.marketplace_name;
    validate_plugin_segment(&marketplace_name, "marketplace name")
        .map_err(MarketplaceRemoveError::InvalidRequest)?;

    let destination = marketplace_install_root(codex_home).join(&marketplace_name);
    let removed_installed_root = remove_marketplace_root(&destination)?;
    let removed_config = remove_user_marketplace(codex_home, &marketplace_name).map_err(|err| {
        MarketplaceRemoveError::Internal(format!(
            "failed to remove marketplace '{marketplace_name}' from user config.toml: {err}"
        ))
    })?;

    if removed_installed_root.is_none() && !removed_config {
        return Err(MarketplaceRemoveError::InvalidRequest(format!(
            "marketplace `{marketplace_name}` is not configured or installed"
        )));
    }

    Ok(MarketplaceRemoveOutcome {
        marketplace_name,
        removed_installed_root,
    })
}

fn remove_marketplace_root(root: &Path) -> Result<Option<AbsolutePathBuf>, MarketplaceRemoveError> {
    if !root.exists() {
        return Ok(None);
    }

    let removed_root = AbsolutePathBuf::try_from(root.to_path_buf()).map_err(|err| {
        MarketplaceRemoveError::Internal(format!(
            "failed to resolve installed marketplace root {}: {err}",
            root.display()
        ))
    })?;
    fs::remove_dir_all(root).map_err(|err| {
        MarketplaceRemoveError::Internal(format!(
            "failed to remove installed marketplace root {}: {err}",
            root.display()
        ))
    })?;
    Ok(Some(removed_root))
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_config::MarketplaceConfigUpdate;
    use codex_config::record_user_marketplace;
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    #[test]
    fn remove_marketplace_sync_removes_config_and_installed_root() {
        let codex_home = TempDir::new().unwrap();
        record_user_marketplace(
            codex_home.path(),
            "debug",
            &MarketplaceConfigUpdate {
                last_updated: "2026-04-13T00:00:00Z",
                last_revision: None,
                source_type: "git",
                source: "https://github.com/owner/repo.git",
                ref_name: Some("main"),
                sparse_paths: &[],
            },
        )
        .unwrap();
        let installed_root = marketplace_install_root(codex_home.path()).join("debug");
        fs::create_dir_all(installed_root.join(".agents/plugins")).unwrap();
        fs::write(
            installed_root.join(".agents/plugins/marketplace.json"),
            "{}",
        )
        .unwrap();

        let outcome = remove_marketplace_sync(
            codex_home.path(),
            MarketplaceRemoveRequest {
                marketplace_name: "debug".to_string(),
            },
        )
        .unwrap();

        assert_eq!(outcome.marketplace_name, "debug");
        assert_eq!(
            outcome.removed_installed_root,
            Some(AbsolutePathBuf::try_from(installed_root.clone()).unwrap())
        );
        let config =
            fs::read_to_string(codex_home.path().join(codex_config::CONFIG_TOML_FILE)).unwrap();
        assert!(!config.contains("[marketplaces.debug]"));
        assert!(!installed_root.exists());
    }

    #[test]
    fn remove_marketplace_sync_rejects_unknown_marketplace() {
        let codex_home = TempDir::new().unwrap();

        let err = remove_marketplace_sync(
            codex_home.path(),
            MarketplaceRemoveRequest {
                marketplace_name: "debug".to_string(),
            },
        )
        .unwrap_err();

        assert_eq!(
            err.to_string(),
            "marketplace `debug` is not configured or installed"
        );
    }
}
