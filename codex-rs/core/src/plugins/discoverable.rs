use anyhow::Context;
use tracing::warn;

use super::OPENAI_CURATED_MARKETPLACE_NAME;
use super::PluginDetailSummary;
use super::PluginReadRequest;
use super::PluginsManager;
use crate::config::Config;
use crate::features::Feature;

const TOOL_SUGGEST_DISCOVERABLE_PLUGIN_ALLOWLIST: &[&str] = &[
    "github@openai-curated",
    "notion@openai-curated",
    "slack@openai-curated",
    "gmail@openai-curated",
    "google-calendar@openai-curated",
    "google-docs@openai-curated",
    "google-drive@openai-curated",
    "google-sheets@openai-curated",
    "google-slides@openai-curated",
];

pub(crate) fn list_tool_suggest_discoverable_plugins(
    config: &Config,
) -> anyhow::Result<Vec<PluginDetailSummary>> {
    if !config.features.enabled(Feature::Plugins) {
        return Ok(Vec::new());
    }

    let plugins_manager = PluginsManager::new(config.codex_home.clone());
    let marketplaces = plugins_manager
        .list_marketplaces_for_config(config, &[])
        .context("failed to list plugin marketplaces for tool suggestions")?;
    let Some(curated_marketplace) = marketplaces
        .into_iter()
        .find(|marketplace| marketplace.name == OPENAI_CURATED_MARKETPLACE_NAME)
    else {
        return Ok(Vec::new());
    };

    let mut discoverable_plugins = Vec::new();
    for plugin in curated_marketplace.plugins {
        if plugin.installed
            || !TOOL_SUGGEST_DISCOVERABLE_PLUGIN_ALLOWLIST.contains(&plugin.id.as_str())
        {
            continue;
        }
        let plugin_id = plugin.id.clone();
        let plugin_name = plugin.name.clone();

        match plugins_manager.read_plugin_for_config(
            config,
            &PluginReadRequest {
                plugin_name,
                marketplace_path: curated_marketplace.path.clone(),
            },
        ) {
            Ok(plugin) => discoverable_plugins.push(plugin.plugin),
            Err(err) => warn!("failed to load curated plugin suggestion {plugin_id}: {err:#}"),
        }
    }
    discoverable_plugins.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then_with(|| left.id.cmp(&right.id))
    });
    Ok(discoverable_plugins)
}

#[cfg(test)]
#[path = "discoverable_tests.rs"]
mod tests;
