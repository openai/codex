use anyhow::Context;
use std::collections::HashSet;
use tracing::warn;

use super::OPENAI_CURATED_MARKETPLACE_NAME;
use super::PluginCapabilitySummary;
use super::PluginReadRequest;
use super::PluginsManager;
use crate::CodexAuth;
use crate::config::Config;
use crate::config::types::ToolSuggestDiscoverableType;
use codex_features::Feature;

pub(crate) async fn list_tool_suggest_discoverable_plugins(
    config: &Config,
    auth: Option<&CodexAuth>,
) -> anyhow::Result<Vec<PluginCapabilitySummary>> {
    if !config.features.enabled(Feature::Plugins) {
        return Ok(Vec::new());
    }

    let plugins_manager = PluginsManager::new(config.codex_home.clone());
    let configured_plugin_ids = config
        .tool_suggest
        .discoverables
        .iter()
        .filter(|discoverable| discoverable.kind == ToolSuggestDiscoverableType::Plugin)
        .map(|discoverable| discoverable.id.as_str())
        .collect::<HashSet<_>>();
    let featured_plugin_ids = match plugins_manager
        .featured_plugin_ids_for_config(config, auth)
        .await
    {
        Ok(featured_plugin_ids) => featured_plugin_ids.into_iter().collect::<HashSet<_>>(),
        Err(err) => {
            warn!("failed to load featured plugin suggestions: {err:#}");
            HashSet::new()
        }
    };
    let marketplaces = plugins_manager
        .list_marketplaces_for_config(config, &[])
        .context("failed to list plugin marketplaces for tool suggestions")?
        .marketplaces;
    let Some(curated_marketplace) = marketplaces
        .into_iter()
        .find(|marketplace| marketplace.name == OPENAI_CURATED_MARKETPLACE_NAME)
    else {
        return Ok(Vec::new());
    };

    let mut discoverable_plugins = Vec::<PluginCapabilitySummary>::new();
    for plugin in curated_marketplace.plugins {
        if plugin.installed
            || (!featured_plugin_ids.contains(plugin.id.as_str())
                && !configured_plugin_ids.contains(plugin.id.as_str()))
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
            Ok(plugin) => discoverable_plugins.push(plugin.plugin.into()),
            Err(err) => warn!("failed to load discoverable plugin suggestion {plugin_id}: {err:#}"),
        }
    }
    discoverable_plugins.sort_by(|left, right| {
        left.display_name
            .cmp(&right.display_name)
            .then_with(|| left.config_name.cmp(&right.config_name))
    });
    Ok(discoverable_plugins)
}

#[cfg(test)]
#[path = "discoverable_tests.rs"]
mod tests;
