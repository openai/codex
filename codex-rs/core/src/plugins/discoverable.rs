use anyhow::Context;
use std::collections::HashSet;
use tracing::warn;

use super::OPENAI_BUNDLED_MARKETPLACE_NAME;
use super::OPENAI_CURATED_MARKETPLACE_NAME;
use super::PluginCapabilitySummary;
use super::PluginsManager;
use crate::config::Config;
use codex_config::types::ToolSuggestDiscoverableType;
use codex_features::Feature;
use codex_tools::DiscoverablePluginInfo;

const TOOL_SUGGEST_DISCOVERABLE_PLUGIN_ALLOWLIST: &[&str] = &[
    "github@openai-curated",
    "notion@openai-curated",
    "slack@openai-curated",
    "gmail@openai-curated",
    "google-calendar@openai-curated",
    "google-drive@openai-curated",
    "linear@openai-curated",
    "figma@openai-curated",
    "computer-use@openai-bundled",
];

const TOOL_SUGGEST_DISCOVERABLE_MARKETPLACE_ALLOWLIST: &[&str] = &[
    OPENAI_BUNDLED_MARKETPLACE_NAME,
    OPENAI_CURATED_MARKETPLACE_NAME,
];

pub(crate) async fn list_tool_suggest_discoverable_plugins(
    config: &Config,
) -> anyhow::Result<Vec<DiscoverablePluginInfo>> {
    if !config.features.enabled(Feature::Plugins) {
        return Ok(Vec::new());
    }

    let plugins_manager = PluginsManager::new(config.codex_home.to_path_buf());
    let configured_plugin_ids = config
        .tool_suggest
        .discoverables
        .iter()
        .filter(|discoverable| discoverable.kind == ToolSuggestDiscoverableType::Plugin)
        .map(|discoverable| discoverable.id.as_str())
        .collect::<HashSet<_>>();
    let marketplaces = plugins_manager
        .list_marketplaces_for_config(config, &[])
        .context("failed to list plugin marketplaces for tool suggestions")?
        .marketplaces;
    let mut discoverable_plugins = Vec::<DiscoverablePluginInfo>::new();
    for marketplace in marketplaces {
        let marketplace_name = marketplace.name;
        if !TOOL_SUGGEST_DISCOVERABLE_MARKETPLACE_ALLOWLIST.contains(&marketplace_name.as_str()) {
            continue;
        }

        for plugin in marketplace.plugins {
            if plugin.installed
                || (!TOOL_SUGGEST_DISCOVERABLE_PLUGIN_ALLOWLIST.contains(&plugin.id.as_str())
                    && !configured_plugin_ids.contains(plugin.id.as_str()))
            {
                continue;
            }

            let plugin_id = plugin.id.clone();

            match plugins_manager
                .read_plugin_detail_for_marketplace_plugin(config, &marketplace_name, plugin)
                .await
            {
                Ok(plugin) => {
                    let plugin: PluginCapabilitySummary = plugin.into();
                    discoverable_plugins.push(DiscoverablePluginInfo {
                        id: plugin.config_name,
                        name: plugin.display_name,
                        description: plugin.description,
                        has_skills: plugin.has_skills,
                        mcp_server_names: plugin.mcp_server_names,
                        app_connector_ids: plugin
                            .app_connector_ids
                            .into_iter()
                            .map(|connector_id| connector_id.0)
                            .collect(),
                    });
                }
                Err(err) => {
                    warn!("failed to load discoverable plugin suggestion {plugin_id}: {err:#}")
                }
            }
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
