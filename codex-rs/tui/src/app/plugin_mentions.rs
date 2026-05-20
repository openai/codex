//! Plugin mention capability enrichment for the TUI.
//!
//! Mention inventory comes from app-server `plugin/list`, while mention eligibility still reuses
//! the older local bulk capability summaries. That keeps the feature app-server-shaped without
//! paying for a `plugin/read` per plugin.

use super::background_requests::request_plugin_list;
use super::*;
use codex_app_server_protocol::PluginListResponse;
use codex_app_server_protocol::PluginMarketplaceEntry;
use codex_app_server_protocol::PluginSummary;
use codex_core_plugins::PluginsManager;
use codex_plugin::PluginCapabilitySummary;
use std::collections::HashMap;

#[derive(Debug, Clone)]
struct PluginMentionEntry {
    config_name: String,
    display_name: String,
    description: Option<String>,
}

impl PluginMentionEntry {
    fn capability_summary(
        self,
        capabilities_by_config_name: &HashMap<String, PluginCapabilitySummary>,
    ) -> Option<PluginCapabilitySummary> {
        let capabilities = capabilities_by_config_name.get(&self.config_name)?;
        Some(PluginCapabilitySummary {
            config_name: self.config_name,
            display_name: self.display_name,
            description: self.description,
            has_skills: capabilities.has_skills,
            mcp_server_names: capabilities.mcp_server_names.clone(),
            app_connector_ids: capabilities.app_connector_ids.clone(),
        })
    }
}

pub(super) async fn fetch_plugin_mentions(
    request_handle: AppServerRequestHandle,
    config: crate::legacy_core::config::Config,
) -> Result<Vec<PluginCapabilitySummary>> {
    let response = request_plugin_list(request_handle, config.cwd.to_path_buf()).await?;
    let mention_entries = plugin_mention_entries_from_list_response(response);
    let capabilities_by_config_name = load_plugin_mention_capabilities(&config).await;

    Ok(mention_entries
        .into_iter()
        .filter_map(|entry| entry.capability_summary(&capabilities_by_config_name))
        .collect())
}

async fn load_plugin_mention_capabilities(
    config: &crate::legacy_core::config::Config,
) -> HashMap<String, PluginCapabilitySummary> {
    let plugins_input = config.plugins_config_input();
    PluginsManager::new(config.codex_home.to_path_buf())
        .plugins_for_config(&plugins_input)
        .await
        .capability_summaries()
        .iter()
        .cloned()
        .map(|summary| (summary.config_name.clone(), summary))
        .collect()
}

fn plugin_mention_entries_from_list_response(
    response: PluginListResponse,
) -> Vec<PluginMentionEntry> {
    response
        .marketplaces
        .into_iter()
        .flat_map(plugin_mention_entries_from_marketplace)
        .collect()
}

fn plugin_mention_entries_from_marketplace(
    marketplace: PluginMarketplaceEntry,
) -> Vec<PluginMentionEntry> {
    let marketplace_name = marketplace.name;
    marketplace
        .plugins
        .into_iter()
        .filter_map(|plugin| plugin_mention_entry(&marketplace_name, plugin))
        .collect()
}

fn plugin_mention_entry(
    marketplace_name: &str,
    plugin: PluginSummary,
) -> Option<PluginMentionEntry> {
    if !plugin_is_eligible_for_mentions(&plugin) {
        return None;
    }

    let config_name = plugin_mention_config_name(marketplace_name, &plugin)?;
    Some(PluginMentionEntry {
        config_name,
        display_name: plugin_mention_display_name(&plugin),
        description: plugin_mention_description(&plugin),
    })
}

fn plugin_is_eligible_for_mentions(plugin: &PluginSummary) -> bool {
    plugin.installed && plugin.enabled
}

fn plugin_mention_config_name(marketplace_name: &str, plugin: &PluginSummary) -> Option<String> {
    codex_plugin::PluginId::new(plugin.name.clone(), marketplace_name.to_string())
        .map(|plugin_id| plugin_id.as_key())
        .map_err(|err| {
            tracing::warn!(
                plugin_name = plugin.name,
                marketplace_name,
                error = %err,
                "skipping plugin mention with invalid identity"
            );
        })
        .ok()
}

fn plugin_mention_display_name(plugin: &PluginSummary) -> String {
    plugin
        .interface
        .as_ref()
        .and_then(|interface| interface.display_name.as_deref())
        .map(str::trim)
        .filter(|display_name| !display_name.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| plugin.name.clone())
}

fn plugin_mention_description(plugin: &PluginSummary) -> Option<String> {
    plugin
        .interface
        .as_ref()
        .and_then(|interface| {
            interface
                .short_description
                .as_deref()
                .or(interface.long_description.as_deref())
        })
        .map(str::trim)
        .filter(|description| !description.is_empty())
        .map(str::to_string)
}
