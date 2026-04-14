use crate::config::Config;
use codex_utils_absolute_path::AbsolutePathBuf;
use std::path::Path;
use std::path::PathBuf;
use tracing::warn;

use super::validate_plugin_segment;

pub const INSTALLED_MARKETPLACES_DIR: &str = ".tmp/marketplaces";

pub fn marketplace_install_root(codex_home: &Path) -> PathBuf {
    codex_home.join(INSTALLED_MARKETPLACES_DIR)
}

pub(crate) fn installed_marketplace_roots_from_config(
    config: &Config,
    codex_home: &Path,
) -> Vec<AbsolutePathBuf> {
    let Some(user_layer) = config.config_layer_stack.get_user_layer() else {
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
            path.join(".agents/plugins/marketplace.json")
                .is_file()
                .then_some(path)
        })
        .filter_map(|path| AbsolutePathBuf::try_from(path).ok())
        .collect::<Vec<_>>();
    roots.sort_unstable_by(|left, right| left.as_path().cmp(right.as_path()));
    roots
}

pub(crate) fn resolve_configured_marketplace_root(
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
