use std::path::Path;
use std::path::PathBuf;

use codex_config::ConfigLayerStack;
use codex_config::StrictKnownMarketplaceToml;
use codex_config::types::MarketplaceSourceType;
use codex_plugin::validate_plugin_segment;
use codex_utils_absolute_path::AbsolutePathBuf;
use tracing::warn;

use crate::marketplace::find_marketplace_manifest_path;

pub const INSTALLED_MARKETPLACES_DIR: &str = ".tmp/marketplaces";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfiguredUserMarketplace {
    pub name: String,
    pub root: AbsolutePathBuf,
    pub source_type: MarketplaceSourceType,
    pub source: String,
    pub ref_name: Option<String>,
    pub sparse_paths: Vec<String>,
}

impl ConfiguredUserMarketplace {
    pub fn allowed_by(&self, allowlist: &[StrictKnownMarketplaceToml]) -> bool {
        allowlist.iter().any(|allowed| {
            allowed.matches_source(
                self.source_type,
                &self.source,
                self.ref_name.as_deref(),
                &self.sparse_paths,
            )
        })
    }
}

pub fn marketplace_install_root(codex_home: &Path) -> PathBuf {
    codex_home.join(INSTALLED_MARKETPLACES_DIR)
}

pub fn installed_marketplace_roots_from_layer_stack(
    config_layer_stack: &ConfigLayerStack,
    codex_home: &Path,
) -> Vec<AbsolutePathBuf> {
    configured_user_marketplaces_from_layer_stack(config_layer_stack, codex_home)
        .into_iter()
        .map(|marketplace| marketplace.root)
        .collect()
}

pub fn configured_user_marketplaces_from_layer_stack(
    config_layer_stack: &ConfigLayerStack,
    codex_home: &Path,
) -> Vec<ConfiguredUserMarketplace> {
    let Some(user_layer) = config_layer_stack.get_user_layer() else {
        return Vec::new();
    };
    let Some(marketplaces_value) = user_layer.config.get("marketplaces") else {
        return Vec::new();
    };
    let Some(marketplaces) = marketplaces_value.as_table() else {
        warn!("invalid marketplaces config: expected table");
        return Vec::new();
    };
    let default_install_root = marketplace_install_root(codex_home);
    let mut roots = marketplaces
        .iter()
        .filter_map(|(marketplace_name, marketplace)| {
            if !marketplace.is_table() {
                warn!(
                    marketplace_name,
                    "ignoring invalid configured marketplace entry"
                );
                return None;
            }
            if let Err(err) = validate_plugin_segment(marketplace_name, "marketplace name") {
                warn!(
                    marketplace_name,
                    error = %err,
                    "ignoring invalid configured marketplace name"
                );
                return None;
            }
            let path = resolve_configured_marketplace_root(
                marketplace_name,
                marketplace,
                &default_install_root,
            )?;
            find_marketplace_manifest_path(&path)?;
            let source_type = marketplace
                .get("source_type")
                .and_then(toml::Value::as_str)
                .and_then(parse_source_type)?;
            let source = marketplace
                .get("source")
                .and_then(toml::Value::as_str)?
                .to_string();
            let ref_name = marketplace
                .get("ref")
                .and_then(toml::Value::as_str)
                .map(str::to_string);
            let sparse_paths = marketplace
                .get("sparse_paths")
                .and_then(toml::Value::as_array)
                .map(|paths| {
                    paths
                        .iter()
                        .filter_map(toml::Value::as_str)
                        .map(str::to_string)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let root = AbsolutePathBuf::try_from(path).ok()?;
            Some(ConfiguredUserMarketplace {
                name: marketplace_name.to_string(),
                root,
                source_type,
                source,
                ref_name,
                sparse_paths,
            })
        })
        .collect::<Vec<_>>();
    roots.sort_unstable_by(|left, right| left.root.as_path().cmp(right.root.as_path()));
    roots
}

pub fn resolve_configured_marketplace_root(
    marketplace_name: &str,
    marketplace: &toml::Value,
    default_install_root: &Path,
) -> Option<PathBuf> {
    match marketplace.get("source_type").and_then(toml::Value::as_str) {
        Some("local") => marketplace
            .get("source")
            .and_then(toml::Value::as_str)
            .filter(|source| !source.is_empty())
            .map(PathBuf::from),
        _ => Some(default_install_root.join(marketplace_name)),
    }
}

fn parse_source_type(source_type: &str) -> Option<MarketplaceSourceType> {
    match source_type {
        "git" => Some(MarketplaceSourceType::Git),
        "local" => Some(MarketplaceSourceType::Local),
        _ => None,
    }
}
