use anyhow::Context;
use std::collections::HashSet;
use tracing::warn;

use super::PluginCapabilitySummary;
use crate::config::Config;
use codex_config::types::ToolSuggestDiscoverableType;
use codex_core_plugins::OPENAI_BUNDLED_MARKETPLACE_NAME;
use codex_core_plugins::OPENAI_CURATED_MARKETPLACE_NAME;
use codex_core_plugins::PluginsManager;
use codex_core_plugins::TOOL_SUGGEST_DISCOVERABLE_PLUGIN_ALLOWLIST as TOOL_SUGGEST_DISCOVERABLE_PLUGIN_FALLBACK_ALLOWLIST;
use codex_core_plugins::marketplace::MarketplacePluginInstallPolicy;
use codex_features::Feature;
use codex_tools::DiscoverablePluginInfo;

const TOOL_SUGGEST_DISCOVERABLE_MARKETPLACE_ALLOWLIST: &[&str] = &[
    OPENAI_BUNDLED_MARKETPLACE_NAME,
    OPENAI_CURATED_MARKETPLACE_NAME,
];

pub(crate) async fn list_tool_suggest_discoverable_plugins(
    config: &Config,
    loaded_plugin_app_connector_ids: &[String],
) -> anyhow::Result<Vec<DiscoverablePluginInfo>> {
    if !config.features.enabled(Feature::Plugins) {
        return Ok(Vec::new());
    }

    let plugins_manager = PluginsManager::new(config.codex_home.to_path_buf());
    let plugins_input = config.plugins_config_input();
    let configured_plugin_ids = config
        .tool_suggest
        .discoverables
        .iter()
        .filter(|discoverable| discoverable.kind == ToolSuggestDiscoverableType::Plugin)
        .map(|discoverable| discoverable.id.as_str())
        .collect::<HashSet<_>>();
    let disabled_plugin_ids = config
        .tool_suggest
        .disabled_tools
        .iter()
        .filter(|disabled_tool| disabled_tool.kind == ToolSuggestDiscoverableType::Plugin)
        .map(|disabled_tool| disabled_tool.id.as_str())
        .collect::<HashSet<_>>();
    let marketplaces = plugins_manager
        .list_marketplaces_for_config(&plugins_input, &[])
        .context("failed to list plugin marketplaces for tool suggestions")?
        .marketplaces;
    let mut installed_app_connector_ids = plugins_manager
        .plugins_for_config(&plugins_input)
        .await
        .capability_summaries()
        .iter()
        .flat_map(|plugin| plugin.app_connector_ids.iter())
        .map(|connector_id| connector_id.0.clone())
        .collect::<HashSet<_>>();
    installed_app_connector_ids.extend(loaded_plugin_app_connector_ids.iter().cloned());

    let mut discoverable_plugins = Vec::<DiscoverablePluginInfo>::new();
    for marketplace in marketplaces {
        let marketplace_name = marketplace.name;
        let is_allowlisted_marketplace =
            TOOL_SUGGEST_DISCOVERABLE_MARKETPLACE_ALLOWLIST.contains(&marketplace_name.as_str());

        for plugin in marketplace.plugins {
            let is_configured_plugin = configured_plugin_ids.contains(plugin.id.as_str());
            let is_fallback_plugin =
                TOOL_SUGGEST_DISCOVERABLE_PLUGIN_FALLBACK_ALLOWLIST.contains(&plugin.id.as_str());
            if plugin.installed
                || plugin.policy.installation == MarketplacePluginInstallPolicy::NotAvailable
                || disabled_plugin_ids.contains(plugin.id.as_str())
                || (!is_allowlisted_marketplace && !is_configured_plugin)
            {
                continue;
            }

            let plugin_id = plugin.id.clone();

            match plugins_manager
                .read_plugin_detail_for_marketplace_plugin(
                    &plugins_input,
                    &marketplace_name,
                    plugin,
                )
                .await
            {
                Ok(plugin) => {
                    let plugin: PluginCapabilitySummary = plugin.into();
                    let matches_installed_app =
                        plugin.app_connector_ids.iter().any(|connector_id| {
                            installed_app_connector_ids.contains(connector_id.0.as_str())
                        });
                    if !is_configured_plugin && !is_fallback_plugin && !matches_installed_app {
                        continue;
                    }

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
