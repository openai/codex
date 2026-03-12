use std::collections::HashSet;

use codex_app_server_protocol::AppInfo;
use codex_app_server_protocol::AppSummary;
use codex_chatgpt::connectors;
use codex_core::config::Config;
use codex_core::plugins::AppConnectorId;
use tracing::warn;

pub(super) async fn load_plugin_app_summaries(
    config: &Config,
    plugin_apps: &[AppConnectorId],
) -> Vec<AppSummary> {
    if plugin_apps.is_empty() {
        return Vec::new();
    }

    let connectors = match connectors::list_all_connectors_with_options(config, false).await {
        Ok(connectors) => connectors,
        Err(err) => {
            warn!("failed to load app metadata for plugin/read: {err:#}");
            connectors::list_cached_all_connectors(config)
                .await
                .unwrap_or_default()
        }
    };

    connectors::connectors_for_plugin_apps(connectors, plugin_apps)
        .into_iter()
        .map(AppSummary::from)
        .collect()
}

pub(super) fn plugin_apps_needing_auth(
    all_connectors: &[AppInfo],
    accessible_connectors: &[AppInfo],
    plugin_apps: &[AppConnectorId],
    codex_apps_ready: bool,
) -> Vec<AppSummary> {
    if !codex_apps_ready {
        return Vec::new();
    }

    let accessible_ids = accessible_connectors
        .iter()
        .map(|connector| connector.id.as_str())
        .collect::<HashSet<_>>();
    let plugin_app_ids = plugin_apps
        .iter()
        .map(|connector_id| connector_id.0.as_str())
        .collect::<HashSet<_>>();

    all_connectors
        .iter()
        .filter(|connector| {
            plugin_app_ids.contains(connector.id.as_str())
                && !accessible_ids.contains(connector.id.as_str())
        })
        .cloned()
        .map(AppSummary::from)
        .collect()
}

#[cfg(test)]
mod tests {
    use codex_app_server_protocol::AppInfo;
    use codex_core::plugins::AppConnectorId;
    use pretty_assertions::assert_eq;

    use super::plugin_apps_needing_auth;

    #[test]
    fn plugin_apps_needing_auth_returns_empty_when_codex_apps_is_not_ready() {
        let all_connectors = vec![AppInfo {
            id: "alpha".to_string(),
            name: "Alpha".to_string(),
            description: Some("Alpha connector".to_string()),
            logo_url: None,
            logo_url_dark: None,
            distribution_channel: None,
            branding: None,
            app_metadata: None,
            labels: None,
            install_url: Some("https://chatgpt.com/apps/alpha/alpha".to_string()),
            is_accessible: false,
            is_enabled: true,
            plugin_display_names: Vec::new(),
        }];

        assert_eq!(
            plugin_apps_needing_auth(
                &all_connectors,
                &[],
                &[AppConnectorId("alpha".to_string())],
                false,
            ),
            Vec::new()
        );
    }
}
