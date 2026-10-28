//! Plugin mention capability enrichment for the TUI.
//!
//! Mention inventory comes from app-server `plugin/installed` so loading
//! mentions does not fetch the browsable plugin catalog. The API exposes
//! plugin-level mention metadata, but not effective per-session capability summaries.

use super::*;
use codex_app_server_protocol::PluginAvailability;
use codex_app_server_protocol::PluginInstalledParams;
use codex_app_server_protocol::PluginInstalledResponse;
use codex_app_server_protocol::PluginSummary;
use codex_app_server_protocol::RequestId;
use codex_plugin::PluginCapabilitySummary;

pub(super) async fn fetch_plugin_mentions(
    request_handle: AppServerRequestHandle,
    cwd: PathBuf,
) -> Result<Vec<PluginCapabilitySummary>> {
    let cwd = AbsolutePathBuf::try_from(cwd).wrap_err("plugin installed cwd must be absolute")?;
    let request_id = RequestId::String(format!("plugin-installed-{}", Uuid::new_v4()));
    let response = request_handle
        .request_typed(ClientRequest::PluginInstalled {
            request_id,
            params: PluginInstalledParams {
                cwds: Some(vec![cwd]),
                install_suggestion_plugin_names: None,
            },
        })
        .await
        .wrap_err("plugin/installed failed in TUI")?;
    Ok(plugin_mentions_from_installed_response(response))
}

fn plugin_mentions_from_installed_response(
    response: PluginInstalledResponse,
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
        plugin_namespace: Some(plugin.name.clone()),
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
    use codex_app_server_protocol::PluginMarketplaceEntry;
    use codex_app_server_protocol::PluginShareContext;
    use codex_app_server_protocol::PluginShareDiscoverability;
    use codex_app_server_protocol::PluginSource;
    use pretty_assertions::assert_eq;

    #[test]
    fn plugin_mentions_use_installed_summaries_and_gui_eligibility() {
        let active = plugin_summary("active");
        let active_shared = shared_plugin_summary("active-shared");
        let mut disabled_by_admin = plugin_summary("disabled-by-admin");
        disabled_by_admin.availability = PluginAvailability::DisabledByAdmin;
        let mut disabled = plugin_summary("disabled");
        disabled.enabled = false;
        let mut uninstalled = plugin_summary("uninstalled");
        uninstalled.installed = false;

        let response = PluginInstalledResponse {
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
        };

        assert_eq!(
            plugin_mentions_from_installed_response(response),
            ["active", "active-shared"].map(|name| PluginCapabilitySummary {
                config_name: format!("{name}@server-marketplace"),
                display_name: name.to_string(),
                plugin_namespace: Some(name.to_string()),
                description: Some("server-marketplace".to_string()),
                has_skills: false,
                mcp_server_names: Vec::new(),
                app_connector_ids: Vec::new(),
            })
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
                can_publish_to_workspace: None,
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
            installed_at: None,
            enabled: true,
            install_policy: PluginInstallPolicy::Available,
            install_policy_source: None,
            must_show_installation_interstitial: None,
            auth_policy: PluginAuthPolicy::OnInstall,
            availability: PluginAvailability::Available,
            disabled_reason: None,
            eligible_plan_types: None,
            interface: None,
            keywords: Vec::new(),
        }
    }
}
