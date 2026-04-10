use super::MarketplaceSource;
use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use codex_config::CONFIG_TOML_FILE;
use codex_config::config_toml::ConfigToml;
use codex_config::types::MarketplaceConfig;
use codex_config::types::MarketplaceSourceType;
use codex_core::plugins::validate_marketplace_root;
use std::path::Path;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ConfiguredMarketplace {
    pub(super) name: String,
    pub(super) install_root: PathBuf,
    pub(super) install_metadata: MarketplaceInstallMetadata,
}

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
    let Ok(config) = std::fs::read_to_string(&config_path) else {
        return Ok(None);
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

pub(super) fn configured_marketplaces(
    codex_home: &Path,
    install_root: &Path,
) -> Result<Vec<ConfiguredMarketplace>> {
    let config_path = codex_home.join(CONFIG_TOML_FILE);
    let config = read_user_config(&config_path)?;
    let mut marketplaces = config
        .marketplaces
        .into_iter()
        .map(|(name, config)| {
            let install_metadata = MarketplaceInstallMetadata::from_config(&name, &config)?;
            Ok(ConfiguredMarketplace {
                install_root: install_root.join(&name),
                name,
                install_metadata,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    marketplaces.sort_unstable_by(|left, right| left.name.cmp(&right.name));
    Ok(marketplaces)
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

    fn from_config(marketplace_name: &str, config: &MarketplaceConfig) -> Result<Self> {
        let Some(source_type) = config.source_type else {
            bail!("marketplace `{marketplace_name}` is missing source_type in user config.toml");
        };
        let Some(source) = config.source.as_ref() else {
            bail!("marketplace `{marketplace_name}` is missing source in user config.toml");
        };

        let source = match source_type {
            MarketplaceSourceType::Directory => InstalledMarketplaceSource::LocalDirectory {
                path: PathBuf::from(source),
            },
            MarketplaceSourceType::Git => InstalledMarketplaceSource::Git {
                url: source.clone(),
                ref_name: config.ref_name.clone(),
                sparse_paths: config.sparse_paths.clone().unwrap_or_default(),
            },
        };
        Ok(Self { source })
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

    pub(super) fn source_display(&self) -> String {
        match &self.source {
            InstalledMarketplaceSource::LocalDirectory { path } => path.display().to_string(),
            InstalledMarketplaceSource::Git { url, ref_name, .. } => match ref_name {
                Some(ref_name) => format!("{url}#{ref_name}"),
                None => url.clone(),
            },
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

fn read_user_config(config_path: &Path) -> Result<ConfigToml> {
    match std::fs::read_to_string(config_path) {
        Ok(raw) => toml::from_str::<ConfigToml>(&raw)
            .with_context(|| format!("failed to parse user config {}", config_path.display())),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(ConfigToml::default()),
        Err(err) => Err(err)
            .with_context(|| format!("failed to read user config {}", config_path.display())),
    }
}
