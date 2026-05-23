//! Plugin mention capability enrichment for the TUI.
//!
//! Mention inventory comes from app-server `plugin/list`. Local sessions reuse the older bulk
//! capability summaries, while remote sessions hydrate details through app-server `plugin/read`
//! instead of the TUI host's plugin config.

use super::background_requests::fetch_plugin_detail;
use super::background_requests::request_plugin_list;
use super::*;
use codex_app_server_protocol::PluginDetail;
use codex_app_server_protocol::PluginListResponse;
use codex_app_server_protocol::PluginMarketplaceEntry;
use codex_app_server_protocol::PluginReadParams;
use codex_app_server_protocol::PluginSummary;
use codex_core_plugins::PluginsManager;
use codex_plugin::AppConnectorId;
use codex_plugin::PluginCapabilitySummary;
use std::collections::HashMap;

#[derive(Debug, Clone)]
struct PluginMentionEntry {
    config_name: String,
    display_name: String,
    description: Option<String>,
    read_params: Option<PluginReadParams>,
}

impl PluginMentionEntry {
    fn capability_summary_from_detail(
        self,
        detail: PluginDetail,
    ) -> Option<PluginCapabilitySummary> {
        let mcp_server_names = detail.mcp_servers;
        let app_connector_ids = detail
            .apps
            .into_iter()
            .map(|app| AppConnectorId(app.id))
            .collect::<Vec<_>>();
        let has_skills = detail.skills.iter().any(|skill| skill.enabled);
        let summary = PluginCapabilitySummary {
            config_name: self.config_name,
            display_name: self.display_name,
            description: self.description,
            has_skills,
            mcp_server_names,
            app_connector_ids,
        };
        (summary.has_skills
            || !summary.mcp_server_names.is_empty()
            || !summary.app_connector_ids.is_empty())
        .then_some(summary)
    }

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

pub(super) async fn fetch_plugin_mentions_from_app_server_details(
    request_handle: AppServerRequestHandle,
    cwd: PathBuf,
) -> Result<Vec<PluginCapabilitySummary>> {
    let response = request_plugin_list(request_handle.clone(), cwd).await?;
    let mention_entries = plugin_mention_entries_from_list_response(response);
    let mut summaries = Vec::new();
    for entry in mention_entries {
        let Some(read_params) = entry.read_params.clone() else {
            continue;
        };
        match fetch_plugin_detail(request_handle.clone(), read_params).await {
            Ok(response) => {
                if let Some(summary) = entry.capability_summary_from_detail(response.plugin) {
                    summaries.push(summary);
                }
            }
            Err(err) => {
                tracing::warn!(
                    error = %err,
                    plugin = entry.config_name,
                    "plugin/read failed while refreshing plugin mention capabilities"
                );
            }
        }
    }
    Ok(summaries)
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
    let marketplace_path = marketplace.path;
    marketplace
        .plugins
        .into_iter()
        .filter_map(|plugin| {
            plugin_mention_entry(&marketplace_name, marketplace_path.clone(), plugin)
        })
        .collect()
}

fn plugin_mention_entry(
    marketplace_name: &str,
    marketplace_path: Option<AbsolutePathBuf>,
    plugin: PluginSummary,
) -> Option<PluginMentionEntry> {
    if !plugin_is_eligible_for_mentions(&plugin) {
        return None;
    }

    let config_name = plugin_mention_config_name(marketplace_name, &plugin)?;
    let read_params = plugin_mention_read_params(marketplace_name, marketplace_path, &plugin);
    Some(PluginMentionEntry {
        config_name,
        display_name: plugin_mention_display_name(&plugin),
        description: plugin_mention_description(&plugin),
        read_params,
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

fn plugin_mention_read_params(
    marketplace_name: &str,
    marketplace_path: Option<AbsolutePathBuf>,
    plugin: &PluginSummary,
) -> Option<PluginReadParams> {
    match marketplace_path {
        Some(marketplace_path) => Some(PluginReadParams {
            marketplace_path: Some(marketplace_path),
            remote_marketplace_name: None,
            plugin_name: plugin.name.clone(),
        }),
        None => Some(PluginReadParams {
            marketplace_path: None,
            remote_marketplace_name: Some(marketplace_name.to_string()),
            plugin_name: plugin.remote_plugin_id.clone()?,
        }),
    }
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
