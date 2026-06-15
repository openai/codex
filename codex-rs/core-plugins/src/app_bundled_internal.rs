use std::path::PathBuf;
use std::sync::OnceLock;

use codex_desktop_installation::DesktopResources;
use codex_desktop_installation::locate_current_or_installed_resources;
use codex_plugin::PluginHookSource;
use codex_plugin::PluginId;
use codex_protocol::protocol::HookSource;
use codex_utils_absolute_path::AbsolutePathBuf;
use serde::Deserialize;
use tracing::warn;

use crate::OPENAI_BUNDLED_MARKETPLACE_NAME;
use crate::loader::load_plugin_hooks;
use crate::manifest::load_plugin_manifest;

const BUNDLED_MARKETPLACE_PATH: &str = "plugins/openai-bundled/.agents/plugins/marketplace.json";
// Standalone CLI discovery may invoke LaunchServices or PowerShell, so do it once per process.
static DESKTOP_RESOURCES: OnceLock<Result<DesktopResources, String>> = OnceLock::new();

pub(crate) fn is_app_bundled_plugin(plugin_id: &PluginId) -> bool {
    plugin_id.marketplace_name == OPENAI_BUNDLED_MARKETPLACE_NAME
}

pub(crate) async fn load_app_bundled_internal_hooks(
    plugin_id: &PluginId,
    plugin_data_root: &AbsolutePathBuf,
) -> Vec<PluginHookSource> {
    let worker_plugin_id = plugin_id.clone();
    let plugin_data_root = plugin_data_root.clone();
    let worker = tokio::task::spawn_blocking(move || {
        let resources = DESKTOP_RESOURCES
            .get_or_init(|| {
                locate_current_or_installed_resources().map_err(|error| error.to_string())
            })
            .as_ref()
            .map_err(Clone::clone)?;
        load_app_bundled_internal_hooks_from_resources(
            resources,
            &worker_plugin_id,
            &plugin_data_root,
        )
    })
    .await;
    let result = match worker {
        Ok(result) => result,
        Err(error) => Err(format!("Desktop discovery worker failed: {error}")),
    };
    match result {
        Ok(sources) => sources,
        Err(error) => {
            warn!(
                diagnostic_code = "app_bundled_internal_hook_load_failed",
                plugin_id = %plugin_id.as_key(),
                error,
                "app-bundled internal hooks failed closed"
            );
            Vec::new()
        }
    }
}

/// Loads this plugin's hooks only from the located Desktop resources root.
pub(crate) fn load_app_bundled_internal_hooks_from_resources(
    resources: &DesktopResources,
    plugin_id: &PluginId,
    plugin_data_root: &AbsolutePathBuf,
) -> Result<Vec<PluginHookSource>, String> {
    if !is_app_bundled_plugin(plugin_id) {
        return Err("plugin is not from the app-bundled marketplace".to_string());
    }

    let marketplace_path = resources
        .contained_file(BUNDLED_MARKETPLACE_PATH)
        .map_err(|error| format!("invalid bundled marketplace: {error}"))?;
    let contents = std::fs::read_to_string(marketplace_path.as_path())
        .map_err(|error| format!("failed to read bundled marketplace: {error}"))?;
    let marketplace: BundledMarketplace = serde_json::from_str(&contents)
        .map_err(|error| format!("failed to parse bundled marketplace: {error}"))?;
    if marketplace.name != OPENAI_BUNDLED_MARKETPLACE_NAME {
        return Err("bundled marketplace has the wrong name".to_string());
    }

    let mut matching_plugins = marketplace
        .plugins
        .into_iter()
        .filter(|plugin| plugin.name == plugin_id.plugin_name);
    let plugin = matching_plugins
        .next()
        .ok_or_else(|| "plugin is missing from the bundled marketplace".to_string())?;
    if matching_plugins.next().is_some() {
        return Err("bundled marketplace contains duplicate plugin entries".to_string());
    }
    let expected_source = format!("./plugins/{}", plugin_id.plugin_name);
    if plugin.source.source != "local" || plugin.source.path != expected_source {
        return Err("bundled plugin has an unexpected source path".to_string());
    }

    let plugin_relative = PathBuf::from("plugins")
        .join(OPENAI_BUNDLED_MARKETPLACE_NAME)
        .join("plugins")
        .join(&plugin_id.plugin_name);
    let plugin_root = resources
        .contained_directory(plugin_relative)
        .map_err(|error| format!("invalid bundled plugin root: {error}"))?;
    let manifest = load_plugin_manifest(plugin_root.as_path())
        .ok_or_else(|| "bundled plugin manifest is missing or invalid".to_string())?;
    if manifest.name != plugin_id.plugin_name {
        return Err("bundled plugin manifest has the wrong name".to_string());
    }

    let (mut sources, warnings) =
        load_plugin_hooks(&plugin_root, plugin_id, plugin_data_root, &manifest.paths);
    if !warnings.is_empty() {
        return Err(format!(
            "failed to load bundled plugin hooks: {}",
            warnings.join("; ")
        ));
    }
    for source in &mut sources {
        source.source = HookSource::AppBundledInternal;
    }
    Ok(sources)
}

#[derive(Deserialize)]
struct BundledMarketplace {
    name: String,
    plugins: Vec<BundledMarketplacePlugin>,
}

#[derive(Deserialize)]
struct BundledMarketplacePlugin {
    name: String,
    source: BundledMarketplacePluginSource,
}

#[derive(Deserialize)]
struct BundledMarketplacePluginSource {
    source: String,
    path: String,
}

#[cfg(test)]
#[path = "app_bundled_internal_tests.rs"]
mod tests;
