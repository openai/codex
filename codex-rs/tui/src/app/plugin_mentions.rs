//! Plugin mention capability enrichment for the TUI.
//!
//! Mention inventory comes from app-server `plugin/list`, then eligible plugins are hydrated with
//! `plugin/read` so the TUI does not derive mention capabilities from the client machine's local
//! config or plugin cache.

use super::background_requests::fetch_plugin_detail;
use super::background_requests::request_plugin_list;
use super::*;
use codex_app_server_protocol::PluginDetail;
use codex_app_server_protocol::PluginListResponse;
use codex_app_server_protocol::PluginReadParams;
use codex_app_server_protocol::PluginSummary;
use codex_plugin::AppConnectorId;
use codex_plugin::PluginCapabilitySummary;

pub(super) async fn fetch_plugin_mentions(
    request_handle: AppServerRequestHandle,
    cwd: PathBuf,
) -> Result<Vec<PluginCapabilitySummary>> {
    let response = request_plugin_list(request_handle.clone(), cwd).await?;
    let mention_reads = plugin_mention_reads_from_list_response(response);

    let mut capabilities = Vec::new();
    for read_params in mention_reads {
        let plugin_name = read_params.plugin_name.clone();
        match fetch_plugin_detail(request_handle.clone(), read_params).await {
            Ok(response) => {
                if let Some(summary) = plugin_detail_capability_summary(response.plugin) {
                    capabilities.push(summary);
                }
            }
            Err(err) => {
                tracing::warn!(
                    error = %err,
                    plugin_name,
                    "plugin/read failed while refreshing plugin mention candidates"
                );
            }
        }
    }
    Ok(capabilities)
}

fn plugin_detail_capability_summary(detail: PluginDetail) -> Option<PluginCapabilitySummary> {
    let config_name = plugin_mention_config_name(&detail.marketplace_name, &detail.summary)?;
    Some(PluginCapabilitySummary {
        config_name,
        display_name: plugin_mention_display_name(&detail.summary),
        description: plugin_mention_description(&detail.summary),
        has_skills: detail.skills.iter().any(|skill| skill.enabled),
        mcp_server_names: detail.mcp_servers,
        app_connector_ids: detail
            .apps
            .into_iter()
            .map(|app| AppConnectorId(app.id))
            .collect(),
    })
}

fn plugin_mention_reads_from_list_response(response: PluginListResponse) -> Vec<PluginReadParams> {
    response
        .marketplaces
        .into_iter()
        .flat_map(|marketplace| {
            let marketplace_name = marketplace.name;
            let marketplace_path = marketplace.path;
            marketplace.plugins.into_iter().filter_map(move |plugin| {
                plugin_mention_read(&marketplace_name, marketplace_path.clone(), plugin)
            })
        })
        .collect()
}

fn plugin_mention_read(
    marketplace_name: &str,
    marketplace_path: Option<AbsolutePathBuf>,
    plugin: PluginSummary,
) -> Option<PluginReadParams> {
    if !plugin.installed || !plugin.enabled {
        return None;
    }

    plugin_mention_config_name(marketplace_name, &plugin)?;
    if let Some(marketplace_path) = marketplace_path {
        Some(PluginReadParams {
            marketplace_path: Some(marketplace_path),
            remote_marketplace_name: None,
            plugin_name: plugin.name,
        })
    } else {
        let Some(remote_plugin_id) = plugin.remote_plugin_id else {
            tracing::warn!(
                plugin_name = plugin.name,
                marketplace_name,
                "skipping remote plugin mention without remote plugin id"
            );
            return None;
        };
        Some(PluginReadParams {
            marketplace_path: None,
            remote_marketplace_name: Some(marketplace_name.to_string()),
            plugin_name: remote_plugin_id,
        })
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use codex_app_server_protocol::AppSummary;
    use codex_app_server_protocol::PluginAuthPolicy;
    use codex_app_server_protocol::PluginAvailability;
    use codex_app_server_protocol::PluginInstallPolicy;
    use codex_app_server_protocol::PluginMarketplaceEntry;
    use codex_app_server_protocol::PluginSource;
    use codex_app_server_protocol::SkillSummary;
    use pretty_assertions::assert_eq;

    fn test_absolute_path(path: &str) -> AbsolutePathBuf {
        AbsolutePathBuf::try_from(PathBuf::from(path)).expect("absolute test path")
    }

    fn plugin_summary(name: &str, installed: bool, enabled: bool) -> PluginSummary {
        PluginSummary {
            id: format!("{name}@test-marketplace"),
            remote_plugin_id: None,
            local_version: None,
            name: name.to_string(),
            share_context: None,
            source: PluginSource::Remote,
            installed,
            enabled,
            install_policy: PluginInstallPolicy::Available,
            auth_policy: PluginAuthPolicy::OnUse,
            availability: PluginAvailability::Available,
            interface: None,
            keywords: Vec::new(),
        }
    }

    #[test]
    fn plugin_mentions_use_app_server_detail_capabilities() {
        let summary = plugin_detail_capability_summary(PluginDetail {
            marketplace_name: "test-marketplace".to_string(),
            marketplace_path: Some(test_absolute_path("/marketplace")),
            summary: plugin_summary("calendar", /*installed*/ true, /*enabled*/ true),
            description: None,
            skills: vec![SkillSummary {
                name: "schedule".to_string(),
                description: "Schedule things".to_string(),
                short_description: None,
                interface: None,
                path: None,
                enabled: true,
            }],
            hooks: Vec::new(),
            apps: vec![AppSummary {
                id: "calendar-app".to_string(),
                name: "Calendar".to_string(),
                description: None,
                install_url: None,
                needs_auth: false,
            }],
            mcp_servers: vec!["calendar-mcp".to_string()],
        })
        .expect("valid plugin detail");

        assert_eq!(
            summary,
            PluginCapabilitySummary {
                config_name: "calendar@test-marketplace".to_string(),
                display_name: "calendar".to_string(),
                description: None,
                has_skills: true,
                mcp_server_names: vec!["calendar-mcp".to_string()],
                app_connector_ids: vec![AppConnectorId("calendar-app".to_string())],
            }
        );
    }

    #[test]
    fn plugin_mention_reads_skip_disabled_and_handle_local_and_remote_sources() {
        let marketplace_path = test_absolute_path("/marketplace");
        let mut remote_plugin = plugin_summary(
            "remote_name",
            /*installed*/ true,
            /*enabled*/ true,
        );
        remote_plugin.remote_plugin_id = Some("plugins~remote_id".to_string());

        let reads = plugin_mention_reads_from_list_response(PluginListResponse {
            marketplaces: vec![
                PluginMarketplaceEntry {
                    name: "test-marketplace".to_string(),
                    path: Some(marketplace_path.clone()),
                    interface: None,
                    plugins: vec![
                        plugin_summary("enabled", /*installed*/ true, /*enabled*/ true),
                        plugin_summary("disabled", /*installed*/ true, /*enabled*/ false),
                    ],
                },
                PluginMarketplaceEntry {
                    name: "chatgpt-global".to_string(),
                    path: None,
                    interface: None,
                    plugins: vec![remote_plugin],
                },
            ],
            marketplace_load_errors: Vec::new(),
            featured_plugin_ids: Vec::new(),
        });

        assert_eq!(
            reads,
            vec![
                PluginReadParams {
                    marketplace_path: Some(marketplace_path),
                    remote_marketplace_name: None,
                    plugin_name: "enabled".to_string(),
                },
                PluginReadParams {
                    marketplace_path: None,
                    remote_marketplace_name: Some("chatgpt-global".to_string()),
                    plugin_name: "plugins~remote_id".to_string(),
                },
            ]
        );
    }
}
