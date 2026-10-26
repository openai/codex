use std::collections::HashSet;
use std::sync::Arc;

use crate::AppInfo;
use crate::merge::plugin_connector_to_app_info;

/// Load the directory only when a candidate still needs to be suggested.
pub async fn filter_tool_suggest_discoverable_connectors(
    directory_connectors: impl Future<Output = Arc<[AppInfo]>>,
    accessible_connectors: &[AppInfo],
    mut discoverable_connector_ids: HashSet<String>,
) -> Vec<AppInfo> {
    for connector in accessible_connectors {
        if connector.is_accessible {
            discoverable_connector_ids.remove(&connector.id);
        }
    }
    if discoverable_connector_ids.is_empty() {
        return Vec::new();
    }

    let mut connectors = Vec::with_capacity(discoverable_connector_ids.len());
    connectors.extend(
        directory_connectors
            .await
            .iter()
            .filter(|connector| discoverable_connector_ids.contains(&connector.id))
            .cloned(),
    );
    for connector in &connectors {
        discoverable_connector_ids.remove(&connector.id);
    }
    connectors.extend(
        discoverable_connector_ids
            .into_iter()
            .map(plugin_connector_to_app_info),
    );
    connectors.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then_with(|| left.id.cmp(&right.id))
    });
    connectors
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::connector_install_url;
    use pretty_assertions::assert_eq;

    fn app(id: &str) -> AppInfo {
        AppInfo {
            id: id.to_string(),
            name: id.to_string(),
            description: None,
            logo_url: None,
            logo_url_dark: None,
            icon_assets: None,
            icon_dark_assets: None,
            distribution_channel: None,
            install_url: None,
            branding: None,
            app_metadata: None,
            labels: None,
            is_accessible: false,
            is_enabled: true,
            plugin_display_names: Vec::new(),
        }
    }

    fn named_app(id: &str, name: &str) -> AppInfo {
        AppInfo {
            id: id.to_string(),
            name: name.to_string(),
            install_url: Some(connector_install_url(name, id)),
            ..app(id)
        }
    }

    #[tokio::test]
    async fn filter_tool_suggest_discoverable_connectors_keeps_only_plugin_backed_uninstalled_apps()
    {
        let filtered = filter_tool_suggest_discoverable_connectors(
            std::future::ready(Arc::from([
                named_app(
                    "connector_2128aebfecb84f64a069897515042a44",
                    "Google Calendar",
                ),
                named_app("connector_68df038e0ba48191908c8434991bbac2", "Gmail"),
                named_app("connector_other", "Other"),
            ])),
            &[AppInfo {
                is_accessible: true,
                ..named_app(
                    "connector_2128aebfecb84f64a069897515042a44",
                    "Google Calendar",
                )
            }],
            HashSet::from([
                "connector_2128aebfecb84f64a069897515042a44".to_string(),
                "connector_68df038e0ba48191908c8434991bbac2".to_string(),
                "connector_missing".to_string(),
            ]),
        )
        .await;

        assert_eq!(
            filtered,
            vec![
                named_app("connector_68df038e0ba48191908c8434991bbac2", "Gmail"),
                plugin_connector_to_app_info("connector_missing".to_string()),
            ]
        );
    }

    #[tokio::test]
    async fn skips_directory_when_no_connectors_can_be_suggested() {
        let accessible = [
            AppInfo {
                is_accessible: true,
                ..app("calendar")
            },
            AppInfo {
                is_accessible: true,
                is_enabled: false,
                ..app("gmail")
            },
        ];
        for candidates in [
            HashSet::new(),
            HashSet::from(["calendar".to_string(), "gmail".to_string()]),
        ] {
            let filtered = filter_tool_suggest_discoverable_connectors(
                async { panic!("unexpected connector directory lookup") },
                &accessible,
                candidates,
            )
            .await;

            assert_eq!(filtered, Vec::<AppInfo>::new());
        }
    }
}
