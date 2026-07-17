//! Plugin mention capability enrichment for the TUI.
//!
//! Mention inventory comes from app-server `plugin/list`, matching the GUI
//! client. The current API exposes plugin-level mention metadata there, but not
//! effective per-session capability summaries.

use super::background_requests::request_plugin_list;
use super::*;
use codex_app_server_protocol::PluginAvailability;
use codex_app_server_protocol::PluginListResponse;
use codex_app_server_protocol::PluginSummary;
use codex_plugin::PluginCapabilitySummary;

pub(super) async fn fetch_plugin_mentions(
    request_handle: AppServerRequestHandle,
    cwd: PathBuf,
) -> Result<Vec<PluginCapabilitySummary>> {
    let response = request_plugin_list(request_handle, cwd).await?;
    Ok(plugin_mentions_from_list_response(response))
}

fn plugin_mentions_from_list_response(
    response: PluginListResponse,
) -> Vec<PluginCapabilitySummary> {
    response
        .marketplaces
        .into_iter()
        .flat_map(|marketplace| {
            let marketplace_name = marketplace.name;
            marketplace
                .plugins
                .into_iter()
                .filter_map(move |plugin| plugin_mention_from_summary(&marketplace_name, plugin))
        })
        .collect()
}

fn plugin_is_eligible_for_mentions(plugin: &PluginSummary) -> bool {
    plugin.installed && plugin.enabled && plugin.availability != PluginAvailability::DisabledByAdmin
}

fn plugin_mention_from_summary(
    marketplace_name: &str,
    plugin: PluginSummary,
) -> Option<PluginCapabilitySummary> {
    if !plugin_is_eligible_for_mentions(&plugin) {
        return None;
    }

    Some(PluginCapabilitySummary {
        config_name: plugin.id.clone(),
        display_name: plugin_mention_display_name(&plugin),
        description: plugin_mention_description(marketplace_name, &plugin),
        has_skills: false,
        mcp_server_names: Vec::new(),
        app_connector_ids: Vec::new(),
    })
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

fn plugin_mention_description(marketplace_name: &str, plugin: &PluginSummary) -> Option<String> {
    plugin
        .interface
        .as_ref()
        .and_then(|interface| interface.short_description.as_deref())
        .map(str::trim)
        .filter(|description| !description.is_empty())
        .map(str::to_string)
        .or_else(|| {
            let marketplace_name = marketplace_name.trim();
            (!marketplace_name.is_empty()).then(|| marketplace_name.to_string())
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_app_server_protocol::PluginAuthPolicy;
    use codex_app_server_protocol::PluginAvailability;
    use codex_app_server_protocol::PluginInstallPolicy;
    use codex_app_server_protocol::PluginListResponse;
    use codex_app_server_protocol::PluginMarketplaceEntry;
    use codex_app_server_protocol::PluginShareContext;
    use codex_app_server_protocol::PluginShareDiscoverability;
    use codex_app_server_protocol::PluginSource;
    use pretty_assertions::assert_eq;

    #[test]
    fn plugin_mentions_use_plugin_list_summaries_and_gui_eligibility() {
        let active = plugin_summary("active");
        let active_shared = shared_plugin_summary("active-shared");
        let mut disabled_by_admin = plugin_summary("disabled-by-admin");
        disabled_by_admin.availability = PluginAvailability::DisabledByAdmin;
        let mut disabled = plugin_summary("disabled");
        disabled.enabled = false;
        let mut uninstalled = plugin_summary("uninstalled");
        uninstalled.installed = false;

        let response = PluginListResponse {
            marketplaces: vec![PluginMarketplaceEntry {
                name: "server-marketplace".to_string(),
                path: None,
                interface: None,
                plugins: vec![
                    active,
                    active_shared,
                    disabled_by_admin,
                    disabled,
                    uninstalled,
                ],
            }],
            marketplace_load_errors: Vec::new(),
            featured_plugin_ids: Vec::new(),
        };

        assert_eq!(
            plugin_mentions_from_list_response(response),
            vec![
                PluginCapabilitySummary {
                    config_name: "active@server-marketplace".to_string(),
                    display_name: "active".to_string(),
                    description: Some("server-marketplace".to_string()),
                    has_skills: false,
                    mcp_server_names: Vec::new(),
                    app_connector_ids: Vec::new(),
                },
                PluginCapabilitySummary {
                    config_name: "active-shared@server-marketplace".to_string(),
                    display_name: "active-shared".to_string(),
                    description: Some("server-marketplace".to_string()),
                    has_skills: false,
                    mcp_server_names: Vec::new(),
                    app_connector_ids: Vec::new(),
                }
            ]
        );
    }

    fn shared_plugin_summary(name: &str) -> PluginSummary {
        PluginSummary {
            share_context: Some(PluginShareContext {
                remote_plugin_id: format!("plugins~{name}"),
                remote_version: Some("7".to_string()),
                discoverability: Some(PluginShareDiscoverability::Private),
                share_url: Some(format!("https://chatgpt.com/codex/plugins/share/{name}")),
                creator_account_user_id: None,
                creator_name: Some("Test User".to_string()),
                share_principals: None,
            }),
            ..plugin_summary(name)
        }
    }

    fn plugin_summary(name: &str) -> PluginSummary {
        PluginSummary {
            id: format!("{name}@server-marketplace"),
            remote_plugin_id: Some(format!("plugins~{name}")),
            version: None,
            local_version: None,
            name: name.to_string(),
            share_context: None,
            source: PluginSource::Remote,
            installed: true,
            enabled: true,
            install_policy: PluginInstallPolicy::Available,
            install_policy_source: None,
            must_show_installation_interstitial: None,
            auth_policy: PluginAuthPolicy::OnInstall,
            availability: PluginAvailability::Available,
            interface: None,
            keywords: Vec::new(),
        }
    }
}
