use anyhow::Context;
use std::collections::HashSet;
use tracing::warn;

use super::OPENAI_CURATED_MARKETPLACE_NAME;
use super::PluginCapabilitySummary;
use super::PluginLoadRequest;
use super::PluginReadRequest;
use super::PluginsManager;
use crate::config_types::ToolSuggestDiscoverable;
use crate::config_types::ToolSuggestDiscoverableType;

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

pub fn list_tool_suggest_discoverable_plugins(
    plugins_manager: &PluginsManager,
    request: &PluginLoadRequest,
    discoverables: &[ToolSuggestDiscoverable],
) -> anyhow::Result<Vec<PluginCapabilitySummary>> {
    if !request.plugins_enabled {
        return Ok(Vec::new());
    }

    let configured_plugin_ids = discoverables
        .iter()
        .filter(|discoverable| discoverable.kind == ToolSuggestDiscoverableType::Plugin)
        .map(|discoverable| discoverable.id.as_str())
        .collect::<HashSet<_>>();
    let marketplaces = plugins_manager
        .list_marketplaces(request, &[])
        .context("failed to list plugin marketplaces for tool suggestions")?;
    let Some(curated_marketplace) = marketplaces
        .into_iter()
        .find(|marketplace| marketplace.name == OPENAI_CURATED_MARKETPLACE_NAME)
    else {
        return Ok(Vec::new());
    };

    let mut discoverable_plugins = Vec::<PluginCapabilitySummary>::new();
    for plugin in curated_marketplace.plugins {
        if plugin.installed
            || (!TOOL_SUGGEST_DISCOVERABLE_PLUGIN_ALLOWLIST.contains(&plugin.id.as_str())
                && !configured_plugin_ids.contains(plugin.id.as_str()))
        {
            continue;
        }

        let plugin_id = plugin.id.clone();
        let plugin_name = plugin.name.clone();

        match plugins_manager.read_plugin(
            request,
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
