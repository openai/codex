//! Plugin mention capability enrichment for the TUI.
//!
//! Mention inventory and capability details come from app-server `plugin/list` and `plugin/read`.

use super::background_requests::fetch_plugin_detail;
use super::background_requests::request_plugin_list;
use super::*;
use codex_app_server_protocol::PluginDetail;
use codex_app_server_protocol::PluginReadParams;
use codex_app_server_protocol::PluginSummary;
use codex_plugin::AppConnectorId;
use codex_plugin::PluginCapabilitySummary;

pub(super) async fn fetch_plugin_mentions(
    request_handle: AppServerRequestHandle,
    cwd: PathBuf,
) -> Result<Vec<PluginCapabilitySummary>> {
    let response = request_plugin_list(request_handle.clone(), cwd).await?;
    let mut summaries = Vec::new();
    for marketplace in response.marketplaces {
        let marketplace_name = marketplace.name;
        let marketplace_path = marketplace.path;
        for plugin in marketplace.plugins {
            if !plugin.installed || !plugin.enabled {
                continue;
            }
            let Some(read_params) =
                plugin_mention_read_params(&marketplace_name, marketplace_path.clone(), &plugin)
            else {
                continue;
            };
            match fetch_plugin_detail(request_handle.clone(), read_params).await {
                Ok(response) => {
                    if let Some(summary) = plugin_capability_summary_from_detail(response.plugin) {
                        summaries.push(summary);
                    }
                }
                Err(err) => {
                    tracing::warn!(
                        error = %err,
                        plugin = plugin.id,
                        "plugin/read failed while refreshing plugin mention capabilities"
                    );
                }
            }
        }
    }
    Ok(summaries)
}

fn plugin_capability_summary_from_detail(detail: PluginDetail) -> Option<PluginCapabilitySummary> {
    let summary = detail.summary;
    let display_name = plugin_mention_display_name(&summary);
    let description = plugin_mention_description(&summary);
    let mcp_server_names = detail.mcp_servers;
    let app_connector_ids = detail
        .apps
        .into_iter()
        .map(|app| AppConnectorId(app.id))
        .collect::<Vec<_>>();
    let has_skills = detail.skills.iter().any(|skill| skill.enabled);
    let summary = PluginCapabilitySummary {
        config_name: summary.id,
        display_name,
        description,
        has_skills,
        mcp_server_names,
        app_connector_ids,
    };
    (summary.has_skills
        || !summary.mcp_server_names.is_empty()
        || !summary.app_connector_ids.is_empty())
    .then_some(summary)
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
