use super::MarketplaceSource;
use anyhow::Context;
use anyhow::Result;
use codex_config::CONFIG_TOML_FILE;
use codex_core::plugins::validate_marketplace_root;
use std::io::ErrorKind;
use std::path::Path;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct MarketplaceInstallMetadata {
    source: InstalledMarketplaceSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum InstalledMarketplaceSource {
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
    codex_home: &Path,
    install_root: &Path,
    install_metadata: &MarketplaceInstallMetadata,
) -> Result<Option<PathBuf>> {
    let config_path = codex_home.join(CONFIG_TOML_FILE);
    let config = match std::fs::read_to_string(&config_path) {
        Ok(config) => config,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(None),
        Err(err) => {
            return Err(err)
                .with_context(|| format!("failed to read user config {}", config_path.display()));
        }
    };
    let config: toml::Value = toml::from_str(&config)
        .with_context(|| format!("failed to parse user config {}", config_path.display()))?;
    let Some(marketplaces) = config.get("marketplaces").and_then(toml::Value::as_table) else {
        return Ok(None);
    };

    for (marketplace_name, marketplace) in marketplaces {
        if !install_metadata.matches_config(marketplace) {
            continue;
        }
        let root = install_root.join(marketplace_name);
        if validate_marketplace_root(&root).is_ok() {
            return Ok(Some(root));
        }
    }

    Ok(None)
}

impl MarketplaceInstallMetadata {
    pub(super) fn from_source(source: &MarketplaceSource, sparse_paths: &[String]) -> Self {
        let source = match source {
            MarketplaceSource::LocalDirectory { path } => {
                InstalledMarketplaceSource::LocalDirectory { path: path.clone() }
            }
            MarketplaceSource::Git { url, ref_name } => InstalledMarketplaceSource::Git {
                url: url.clone(),
                ref_name: ref_name.clone(),
                sparse_paths: sparse_paths.to_vec(),
            },
        };
        Self { source }
    }

    pub(super) fn config_source_type(&self) -> &'static str {
        match &self.source {
            InstalledMarketplaceSource::LocalDirectory { .. } => "directory",
            InstalledMarketplaceSource::Git { .. } => "git",
        }
    }

    pub(super) fn config_source(&self) -> String {
        match &self.source {
            InstalledMarketplaceSource::LocalDirectory { path } => path.display().to_string(),
            InstalledMarketplaceSource::Git { url, .. } => url.clone(),
        }
    }

    pub(super) fn ref_name(&self) -> Option<&str> {
        match &self.source {
            InstalledMarketplaceSource::LocalDirectory { .. } => None,
            InstalledMarketplaceSource::Git { ref_name, .. } => ref_name.as_deref(),
        }
    }

    pub(super) fn sparse_paths(&self) -> &[String] {
        match &self.source {
            InstalledMarketplaceSource::LocalDirectory { .. } => &[],
            InstalledMarketplaceSource::Git { sparse_paths, .. } => sparse_paths,
        }
    }

    fn matches_config(&self, marketplace: &toml::Value) -> bool {
        marketplace.get("source_type").and_then(toml::Value::as_str)
            == Some(self.config_source_type())
            && marketplace.get("source").and_then(toml::Value::as_str)
                == Some(self.config_source().as_str())
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
    fn installed_marketplace_root_for_source_propagates_config_read_errors() -> Result<()> {
        let codex_home = TempDir::new()?;
        let config_path = codex_home.path().join(CONFIG_TOML_FILE);
        std::fs::create_dir(&config_path)?;

        let install_root = codex_home.path().join("marketplaces");
        let source = MarketplaceSource::LocalDirectory {
            path: codex_home.path().join("source"),
        };
        let install_metadata = MarketplaceInstallMetadata::from_source(&source, &[]);

        let err = installed_marketplace_root_for_source(
            codex_home.path(),
            &install_root,
            &install_metadata,
        )
        .unwrap_err();

        assert_eq!(
            err.to_string(),
            format!("failed to read user config {}", config_path.display())
        );

        Ok(())
    }
}
