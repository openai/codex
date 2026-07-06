mod activation;
mod git;

use self::activation::activate_marketplace_root;
use self::activation::installed_marketplace_metadata_matches;
use self::activation::write_installed_marketplace_metadata;
use self::git::clone_git_source;
use self::git::git_remote_revision;
use crate::installed_marketplaces::marketplace_install_root;
use crate::marketplace::validate_marketplace_root;
use crate::marketplace_add::MarketplaceSource;
use crate::marketplace_policy::MarketplacePolicy;
use codex_config::CONFIG_TOML_FILE;
use codex_config::ConfigLayerStack;
use codex_config::MarketplaceConfigUpdate;
use codex_config::record_user_marketplace;
use codex_config::types::MarketplaceConfig;
use codex_config::types::MarketplaceSourceType;
use codex_plugin::validate_plugin_segment;
use codex_utils_absolute_path::AbsolutePathBuf;
use std::path::Path;
use std::time::Duration;

const MARKETPLACE_UPGRADE_GIT_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfiguredMarketplaceUpgradeError {
    pub marketplace_name: String,
    pub message: String,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ConfiguredMarketplaceUpgradeOutcome {
    pub selected_marketplaces: Vec<String>,
    pub upgraded_roots: Vec<AbsolutePathBuf>,
    pub errors: Vec<ConfiguredMarketplaceUpgradeError>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ConfiguredGitMarketplace {
    name: String,
    source: String,
    ref_name: Option<String>,
    sparse_paths: Vec<String>,
    last_revision: Option<String>,
}

#[derive(Default)]
struct ConfiguredGitMarketplaceLoadOutcome {
    marketplaces: Vec<ConfiguredGitMarketplace>,
    errors: Vec<ConfiguredMarketplaceUpgradeError>,
}

impl ConfiguredMarketplaceUpgradeOutcome {
    pub fn all_succeeded(&self) -> bool {
        self.errors.is_empty()
    }
}

pub fn configured_git_marketplace_names(config_layer_stack: &ConfigLayerStack) -> Vec<String> {
    let mut names = load_configured_git_marketplaces(config_layer_stack)
        .marketplaces
        .into_iter()
        .map(|marketplace| marketplace.name)
        .collect::<Vec<_>>();
    names.sort_unstable();
    names
}

pub fn upgrade_configured_git_marketplaces(
    codex_home: &Path,
    config_layer_stack: &ConfigLayerStack,
    marketplace_name: Option<&str>,
) -> ConfiguredMarketplaceUpgradeOutcome {
    let loaded = load_configured_git_marketplaces(config_layer_stack);
    let marketplaces = loaded
        .marketplaces
        .into_iter()
        .filter(|marketplace| marketplace_name.is_none_or(|name| marketplace.name.as_str() == name))
        .collect::<Vec<_>>();
    let mut errors = loaded
        .errors
        .into_iter()
        .filter(|error| marketplace_name.is_none_or(|name| error.marketplace_name.as_str() == name))
        .collect::<Vec<_>>();
    if marketplaces.is_empty() && errors.is_empty() {
        return ConfiguredMarketplaceUpgradeOutcome::default();
    }

    let install_root = marketplace_install_root(codex_home);
    let mut selected_marketplaces = marketplaces
        .iter()
        .map(|marketplace| marketplace.name.clone())
        .chain(errors.iter().map(|error| error.marketplace_name.clone()))
        .collect::<Vec<_>>();
    selected_marketplaces.sort_unstable();
    selected_marketplaces.dedup();
    let mut upgraded_roots = Vec::new();
    let policy = MarketplacePolicy::from_requirements(config_layer_stack.requirements());
    for marketplace in marketplaces {
        let normalized_source =
            match policy.validate_git_source(&marketplace.source, marketplace.ref_name.clone()) {
                Ok(normalized_source) => normalized_source,
                Err(message) => {
                    errors.push(ConfiguredMarketplaceUpgradeError {
                        marketplace_name: marketplace.name,
                        message,
                    });
                    continue;
                }
            };
        match upgrade_configured_git_marketplace(
            codex_home,
            &install_root,
            &marketplace,
            normalized_source.as_ref(),
        ) {
            Ok(Some(upgraded_root)) => upgraded_roots.push(upgraded_root),
            Ok(None) => {}
            Err(err) => {
                errors.push(ConfiguredMarketplaceUpgradeError {
                    marketplace_name: marketplace.name,
                    message: err,
                });
            }
        }
    }

    ConfiguredMarketplaceUpgradeOutcome {
        selected_marketplaces,
        upgraded_roots,
        errors,
    }
}

fn load_configured_git_marketplaces(
    config_layer_stack: &ConfigLayerStack,
) -> ConfiguredGitMarketplaceLoadOutcome {
    let Some(user_config) = config_layer_stack.effective_user_config() else {
        return ConfiguredGitMarketplaceLoadOutcome::default();
    };
    let Some(marketplaces) = user_config
        .get("marketplaces")
        .and_then(toml::Value::as_table)
    else {
        return ConfiguredGitMarketplaceLoadOutcome::default();
    };

    let mut outcome = ConfiguredGitMarketplaceLoadOutcome::default();
    for (name, marketplace) in marketplaces {
        match parse_configured_git_marketplace(name, marketplace) {
            Ok(Some(marketplace)) => outcome.marketplaces.push(marketplace),
            Ok(None) => {}
            Err(message) => outcome.errors.push(ConfiguredMarketplaceUpgradeError {
                marketplace_name: name.clone(),
                message,
            }),
        }
    }
    outcome
        .marketplaces
        .sort_unstable_by(|left, right| left.name.cmp(&right.name));
    outcome
        .errors
        .sort_unstable_by(|left, right| left.marketplace_name.cmp(&right.marketplace_name));
    outcome
}

fn parse_configured_git_marketplace(
    name: &str,
    marketplace: &toml::Value,
) -> Result<Option<ConfiguredGitMarketplace>, String> {
    if marketplace.get("source_type").and_then(toml::Value::as_str) != Some("git") {
        return Ok(None);
    }
    let marketplace = marketplace
        .clone()
        .try_into::<MarketplaceConfig>()
        .map_err(|err| format!("invalid configured Git marketplace: {err}"))?;
    let MarketplaceConfig {
        last_updated: _,
        last_revision,
        source_type,
        source,
        ref_name,
        sparse_paths,
    } = marketplace;
    if source_type != Some(MarketplaceSourceType::Git) {
        return Ok(None);
    }
    let source =
        source.ok_or_else(|| "configured Git marketplace is missing source".to_string())?;
    Ok(Some(ConfiguredGitMarketplace {
        name: name.to_string(),
        source,
        ref_name,
        sparse_paths: sparse_paths.unwrap_or_default(),
        last_revision,
    }))
}

fn upgrade_configured_git_marketplace(
    codex_home: &Path,
    install_root: &Path,
    marketplace: &ConfiguredGitMarketplace,
    normalized_source: Option<&MarketplaceSource>,
) -> Result<Option<AbsolutePathBuf>, String> {
    validate_plugin_segment(&marketplace.name, "marketplace name")?;
    let (source, ref_name) = match normalized_source {
        Some(MarketplaceSource::Git { url, ref_name }) => (url.as_str(), ref_name.as_deref()),
        Some(MarketplaceSource::Local { .. }) => {
            return Err("validated Git marketplace source resolved to a local path".to_string());
        }
        None => (marketplace.source.as_str(), marketplace.ref_name.as_deref()),
    };
    let remote_revision = git_remote_revision(source, ref_name, MARKETPLACE_UPGRADE_GIT_TIMEOUT)?;
    let destination = install_root.join(&marketplace.name);
    if validate_marketplace_root(&destination)
        .is_ok_and(|marketplace_name| marketplace_name == marketplace.name)
        && marketplace.last_revision.as_deref() == Some(remote_revision.as_str())
        && installed_marketplace_metadata_matches(&destination, marketplace, &remote_revision)
    {
        return Ok(None);
    }

    let staging_parent = install_root.join(".staging");
    std::fs::create_dir_all(&staging_parent).map_err(|err| {
        format!(
            "failed to create marketplace upgrade staging directory {}: {err}",
            staging_parent.display()
        )
    })?;
    let staged_dir = tempfile::Builder::new()
        .prefix("marketplace-upgrade-")
        .tempdir_in(&staging_parent)
        .map_err(|err| {
            format!(
                "failed to create temporary marketplace upgrade directory in {}: {err}",
                staging_parent.display()
            )
        })?;

    let activated_revision = clone_git_source(
        source,
        ref_name,
        &marketplace.sparse_paths,
        staged_dir.path(),
        MARKETPLACE_UPGRADE_GIT_TIMEOUT,
    )?;
    let marketplace_name = validate_marketplace_root(staged_dir.path())
        .map_err(|err| format!("failed to validate upgraded marketplace root: {err}"))?;
    if marketplace_name != marketplace.name {
        return Err(format!(
            "upgraded marketplace name `{marketplace_name}` does not match configured marketplace `{}`",
            marketplace.name
        ));
    }
    write_installed_marketplace_metadata(staged_dir.path(), marketplace, &activated_revision)?;

    let last_updated = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let update = MarketplaceConfigUpdate {
        last_updated: &last_updated,
        last_revision: Some(&activated_revision),
        source_type: "git",
        source: &marketplace.source,
        ref_name: marketplace.ref_name.as_deref(),
        sparse_paths: &marketplace.sparse_paths,
    };
    activate_marketplace_root(&destination, staged_dir, || {
        ensure_configured_git_marketplace_unchanged(codex_home, marketplace)?;
        record_user_marketplace(codex_home, &marketplace.name, &update).map_err(|err| {
            format!(
                "failed to record upgraded marketplace `{}` in user config.toml: {err}",
                marketplace.name
            )
        })
    })?;

    AbsolutePathBuf::try_from(destination)
        .map(Some)
        .map_err(|err| format!("upgraded marketplace path is not absolute: {err}"))
}
fn ensure_configured_git_marketplace_unchanged(
    codex_home: &Path,
    expected: &ConfiguredGitMarketplace,
) -> Result<(), String> {
    let current = read_configured_git_marketplace(codex_home, &expected.name)?;
    match current {
        Some(current) if current == *expected => Ok(()),
        Some(_) => Err(format!(
            "configured marketplace `{}` changed while auto-upgrade was in flight",
            expected.name
        )),
        None => Err(format!(
            "configured marketplace `{}` was removed or is no longer a Git marketplace",
            expected.name
        )),
    }
}

fn read_configured_git_marketplace(
    codex_home: &Path,
    marketplace_name: &str,
) -> Result<Option<ConfiguredGitMarketplace>, String> {
    let config_path = codex_home.join(CONFIG_TOML_FILE);
    let raw_config = match std::fs::read_to_string(&config_path) {
        Ok(raw_config) => raw_config,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => {
            return Err(format!(
                "failed to read user config {} while checking marketplace auto-upgrade: {err}",
                config_path.display()
            ));
        }
    };
    let config: toml::Value = toml::from_str(&raw_config).map_err(|err| {
        format!(
            "failed to parse user config {} while checking marketplace auto-upgrade: {err}",
            config_path.display()
        )
    })?;
    let Some(marketplace) = config
        .get("marketplaces")
        .and_then(toml::Value::as_table)
        .and_then(|marketplaces| marketplaces.get(marketplace_name))
    else {
        return Ok(None);
    };
    parse_configured_git_marketplace(marketplace_name, marketplace)
}

#[cfg(test)]
#[path = "marketplace_upgrade_tests.rs"]
mod tests;
