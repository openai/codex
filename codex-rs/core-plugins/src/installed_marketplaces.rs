use std::path::Path;
use std::path::PathBuf;

use codex_config::ConfigLayerStack;
use codex_plugin::validate_plugin_segment;
use codex_utils_absolute_path::AbsolutePathBuf;
use tracing::warn;

use crate::marketplace::find_marketplace_manifest_path;
use crate::marketplace_upgrade::installed_marketplace_revision;
use crate::plugin_catalog_revision::PluginCatalogRevision;

pub const INSTALLED_MARKETPLACES_DIR: &str = ".tmp/marketplaces";

pub fn marketplace_install_root(codex_home: &Path) -> PathBuf {
    codex_home.join(INSTALLED_MARKETPLACES_DIR)
}

pub fn installed_marketplace_roots_from_layer_stack(
    config_layer_stack: &ConfigLayerStack,
    codex_home: &Path,
) -> Vec<AbsolutePathBuf> {
    installed_marketplace_roots_with_revisions_from_layer_stack(config_layer_stack, codex_home)
        .into_iter()
        .map(|(root, _revision)| root)
        .collect()
}

pub(crate) fn installed_marketplace_roots_with_revisions_from_layer_stack(
    config_layer_stack: &ConfigLayerStack,
    codex_home: &Path,
) -> Vec<(AbsolutePathBuf, Option<PluginCatalogRevision>)> {
    let Some(user_config) = config_layer_stack.effective_user_config() else {
        return Vec::new();
    };
    let Some(marketplaces_value) = user_config.get("marketplaces") else {
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
            let root = AbsolutePathBuf::try_from(path).ok()?;
            let revision = if marketplace.get("source_type").and_then(toml::Value::as_str)
                == Some("git")
            {
                (|| {
                    let source = marketplace.get("source").and_then(toml::Value::as_str)?;
                    let ref_name = marketplace.get("ref").and_then(toml::Value::as_str);
                    let sparse_paths = match marketplace.get("sparse_paths") {
                        Some(paths) => paths
                            .as_array()?
                            .iter()
                            .map(|path| path.as_str().map(ToString::to_string))
                            .collect::<Option<Vec<_>>>()?,
                        None => Vec::new(),
                    };
                    installed_marketplace_revision(root.as_path(), source, ref_name, &sparse_paths)
                })()
            } else {
                None
            };
            Some((root, revision))
        })
        .collect::<Vec<_>>();
    roots.sort_unstable_by(|(left, _), (right, _)| left.as_path().cmp(right.as_path()));
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
