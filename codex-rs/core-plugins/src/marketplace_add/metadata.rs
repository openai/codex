use super::MarketplaceAddError;
use super::source::MarketplaceSource;
use crate::installed_marketplaces::resolve_configured_marketplace_root;
use crate::marketplace::validate_marketplace_root;
use codex_config::CONFIG_TOML_FILE;
use codex_config::MarketplaceConfigUpdate;
use codex_config::record_user_marketplace;
use codex_utils_path::paths_match_after_normalization;
use std::fs;
use std::io::ErrorKind;
use std::path::Path;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct MarketplaceInstallMetadata {
    source: InstalledMarketplaceSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum InstalledMarketplaceSource {
    Git {
        url: String,
        ref_name: Option<String>,
        sparse_paths: Vec<String>,
    },
    Local {
        path: String,
    },
}

pub(super) fn record_added_marketplace_entry(
    codex_home: &Path,
    marketplace_name: &str,
    install_metadata: &MarketplaceInstallMetadata,
) -> Result<(), MarketplaceAddError> {
    let source = install_metadata.config_source();
    let update = MarketplaceConfigUpdate {
        source_type: install_metadata.config_source_type(),
        source: &source,
        ref_name: install_metadata.ref_name(),
        sparse_paths: install_metadata.sparse_paths(),
    };

    record_user_marketplace(codex_home, marketplace_name, &update).map_err(|err| {
        MarketplaceAddError::Internal(format!(
            "failed to add marketplace '{marketplace_name}' to user config.toml: {err}"
        ))
    })
}

pub(super) fn installed_marketplace_root_for_source(
    codex_home: &Path,
    install_root: &Path,
    install_metadata: &MarketplaceInstallMetadata,
) -> Result<Option<PathBuf>, MarketplaceAddError> {
    let config_path = codex_home.join(CONFIG_TOML_FILE);
    let config = match fs::read_to_string(&config_path) {
        Ok(config) => config,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(None),
        Err(err) => {
            return Err(MarketplaceAddError::Internal(format!(
                "failed to read user config {}: {err}",
                config_path.display()
            )));
        }
    };
    let config: toml::Value = toml::from_str(&config).map_err(|err| {
        MarketplaceAddError::Internal(format!(
            "failed to parse user config {}: {err}",
            config_path.display()
        ))
    })?;
    let Some(marketplaces) = config.get("marketplaces").and_then(toml::Value::as_table) else {
        return Ok(None);
    };

    for (marketplace_name, marketplace) in marketplaces {
        if !install_metadata.matches_config(marketplace) {
            continue;
        }
        let root = match &install_metadata.source {
            // Keep using the resolved request path if the configured alias is retargeted.
            InstalledMarketplaceSource::Local { path } => PathBuf::from(path),
            InstalledMarketplaceSource::Git { .. } => install_root.join(marketplace_name),
        };
        if validate_marketplace_root(&root).is_ok() {
            return Ok(Some(root));
        }
    }

    Ok(None)
}

pub(super) fn find_marketplace_root_by_name(
    codex_home: &Path,
    install_root: &Path,
    marketplace_name: &str,
) -> Result<Option<PathBuf>, MarketplaceAddError> {
    let config_path = codex_home.join(CONFIG_TOML_FILE);
    let config = match fs::read_to_string(&config_path) {
        Ok(config) => config,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(None),
        Err(err) => {
            return Err(MarketplaceAddError::Internal(format!(
                "failed to read user config {}: {err}",
                config_path.display()
            )));
        }
    };
    let config: toml::Value = toml::from_str(&config).map_err(|err| {
        MarketplaceAddError::Internal(format!(
            "failed to parse user config {}: {err}",
            config_path.display()
        ))
    })?;
    let Some(marketplace) = config
        .get("marketplaces")
        .and_then(toml::Value::as_table)
        .and_then(|marketplaces| marketplaces.get(marketplace_name))
    else {
        return Ok(None);
    };

    let Some(root) =
        resolve_configured_marketplace_root(marketplace_name, marketplace, install_root)
    else {
        return Ok(None);
    };
    if validate_marketplace_root(&root).is_ok() {
        Ok(Some(root))
    } else {
        Ok(None)
    }
}

impl MarketplaceInstallMetadata {
    pub(super) fn from_source(source: &MarketplaceSource, sparse_paths: &[String]) -> Self {
        let source = match source {
            MarketplaceSource::Git { url, ref_name } => InstalledMarketplaceSource::Git {
                url: url.clone(),
                ref_name: ref_name.clone(),
                sparse_paths: sparse_paths.to_vec(),
            },
            MarketplaceSource::Local { path } => InstalledMarketplaceSource::Local {
                path: path.display().to_string(),
            },
        };
        Self { source }
    }

    fn config_source_type(&self) -> &'static str {
        match &self.source {
            InstalledMarketplaceSource::Git { .. } => "git",
            InstalledMarketplaceSource::Local { .. } => "local",
        }
    }

    fn config_source(&self) -> String {
        match &self.source {
            InstalledMarketplaceSource::Git { url, .. } => url.clone(),
            InstalledMarketplaceSource::Local { path } => path.clone(),
        }
    }

    fn ref_name(&self) -> Option<&str> {
        match &self.source {
            InstalledMarketplaceSource::Git { ref_name, .. } => ref_name.as_deref(),
            InstalledMarketplaceSource::Local { .. } => None,
        }
    }

    fn sparse_paths(&self) -> &[String] {
        match &self.source {
            InstalledMarketplaceSource::Git { sparse_paths, .. } => sparse_paths,
            InstalledMarketplaceSource::Local { .. } => &[],
        }
    }

    fn matches_config(&self, marketplace: &toml::Value) -> bool {
        marketplace.get("source_type").and_then(toml::Value::as_str)
            == Some(self.config_source_type())
            && marketplace
                .get("source")
                .and_then(toml::Value::as_str)
                .is_some_and(|source| match &self.source {
                    InstalledMarketplaceSource::Local { path } => {
                        paths_match_after_normalization(source, path)
                    }
                    InstalledMarketplaceSource::Git { url, .. } => source == url,
                })
            && marketplace.get("ref").and_then(toml::Value::as_str) == self.ref_name()
            && config_sparse_paths(marketplace) == self.sparse_paths()
    }
}

fn config_sparse_paths(marketplace: &toml::Value) -> Vec<String> {
    marketplace
        .get("sparse_paths")
        .and_then(toml::Value::as_array)
        .map(|paths| {
            paths
                .iter()
                .filter_map(toml::Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    #[test]
    fn installed_marketplace_root_for_source_propagates_config_read_errors() {
        let codex_home = TempDir::new().unwrap();
        let config_path = codex_home.path().join(CONFIG_TOML_FILE);
        fs::create_dir(&config_path).unwrap();

        let install_root = codex_home.path().join("marketplaces");
        let source = MarketplaceSource::Git {
            url: "https://github.com/owner/repo.git".to_string(),
            ref_name: None,
        };
        let install_metadata = MarketplaceInstallMetadata::from_source(&source, &[]);

        let err = installed_marketplace_root_for_source(
            codex_home.path(),
            &install_root,
            &install_metadata,
        )
        .unwrap_err();

        assert!(
            err.to_string().contains(&format!(
                "failed to read user config {}:",
                config_path.display()
            )),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn installed_marketplace_root_for_source_uses_local_source_root() {
        let codex_home = TempDir::new().unwrap();
        let install_root = codex_home.path().join("marketplaces");
        let source_root = codex_home.path().join("source");
        fs::create_dir_all(source_root.join(".agents/plugins")).unwrap();
        fs::write(
            source_root.join(".agents/plugins/marketplace.json"),
            r#"{"name":"debug","plugins":[]}"#,
        )
        .unwrap();
        let source = MarketplaceSource::Local {
            path: source_root.clone(),
        };
        let install_metadata = MarketplaceInstallMetadata::from_source(&source, &[]);
        record_added_marketplace_entry(codex_home.path(), "debug", &install_metadata).unwrap();

        let root = installed_marketplace_root_for_source(
            codex_home.path(),
            &install_root,
            &install_metadata,
        )
        .unwrap();

        assert_eq!(root, Some(source_root));
    }

    #[cfg(windows)]
    #[test]
    fn local_marketplace_matches_canonical_and_legacy_windows_sources() {
        let source = TempDir::new().unwrap();
        let other = TempDir::new().unwrap();
        let metadata = MarketplaceInstallMetadata::from_source(
            &MarketplaceSource::Local {
                path: source.path().canonicalize().unwrap(),
            },
            &[],
        );
        let canonical = source.path().canonicalize().unwrap();
        let legacy =
            codex_utils_absolute_path::normalize_windows_device_path(&canonical.to_string_lossy())
                .unwrap();
        for spelling in [
            canonical.to_string_lossy().into_owned(),
            legacy.replace('\\', "/"),
        ] {
            let config: toml::Value =
                toml::from_str(&format!("source_type = \"local\"\nsource = {spelling:?}\n"))
                    .unwrap();
            assert!(metadata.matches_config(&config));
        }
        let different = other.path().to_string_lossy();
        let config: toml::Value = toml::from_str(&format!(
            "source_type = \"local\"\nsource = {different:?}\n"
        ))
        .unwrap();
        assert!(!metadata.matches_config(&config));
    }

    #[cfg(windows)]
    #[test]
    fn installed_local_root_does_not_follow_retargeted_configured_junction() {
        let codex_home = TempDir::new().unwrap();
        let source_root = codex_home.path().join("source");
        let other_root = codex_home.path().join("other");
        for (root, name) in [(&source_root, "source"), (&other_root, "other")] {
            fs::create_dir_all(root.join(".agents/plugins")).unwrap();
            fs::write(
                root.join(".agents/plugins/marketplace.json"),
                format!(r#"{{"name":"{name}","plugins":[]}}"#),
            )
            .unwrap();
        }
        let alias = codex_home.path().join("configured-source");
        crate::test_support::create_directory_junction(&source_root, &alias);
        let configured_metadata = MarketplaceInstallMetadata::from_source(
            &MarketplaceSource::Local {
                path: alias.clone(),
            },
            &[],
        );
        record_added_marketplace_entry(codex_home.path(), "source", &configured_metadata).unwrap();
        let canonical_source = source_root.canonicalize().unwrap();
        let request_metadata = MarketplaceInstallMetadata::from_source(
            &MarketplaceSource::Local {
                path: canonical_source.clone(),
            },
            &[],
        );
        let root = installed_marketplace_root_for_source(
            codex_home.path(),
            &codex_home.path().join("marketplaces"),
            &request_metadata,
        )
        .unwrap()
        .unwrap();

        // Model an updater switching the alias before the caller validates the returned root.
        fs::remove_dir(&alias).unwrap();
        crate::test_support::create_directory_junction(&other_root, &alias);
        assert_eq!(root, canonical_source);
        assert_eq!(validate_marketplace_root(&root).unwrap(), "source");
    }
}
