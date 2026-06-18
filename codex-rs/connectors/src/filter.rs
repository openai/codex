use std::collections::HashSet;

use codex_app_server_protocol::AppInfo;

pub fn filter_tool_suggest_discoverable_connectors(
    directory_connectors: Vec<AppInfo>,
    accessible_connectors: &[AppInfo],
    discoverable_connector_ids: &HashSet<String>,
) -> Vec<AppInfo> {
    let accessible_connector_ids: HashSet<&str> = accessible_connectors
        .iter()
        .filter(|connector| connector.is_accessible)
        .map(|connector| connector.id.as_str())
        .collect();

    let mut connectors = directory_connectors
        .into_iter()
        .filter(|connector| !accessible_connector_ids.contains(connector.id.as_str()))
        .filter(|connector| discoverable_connector_ids.contains(connector.id.as_str()))
        .collect::<Vec<_>>();
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

    #[test]
    fn filter_tool_suggest_discoverable_connectors_keeps_only_plugin_backed_uninstalled_apps() {
        let filtered = filter_tool_suggest_discoverable_connectors(
            vec![
                named_app(
                    "connector_2128aebfecb84f64a069897515042a44",
                    "Google Calendar",
                ),
                named_app("connector_68df038e0ba48191908c8434991bbac2", "Gmail"),
                named_app("connector_other", "Other"),
            ],
            &[AppInfo {
                is_accessible: true,
                ..named_app(
                    "connector_2128aebfecb84f64a069897515042a44",
                    "Google Calendar",
                )
            }],
            &HashSet::from([
                "connector_2128aebfecb84f64a069897515042a44".to_string(),
                "connector_68df038e0ba48191908c8434991bbac2".to_string(),
            ]),
        );

        assert_eq!(
            filtered,
            vec![named_app(
                "connector_68df038e0ba48191908c8434991bbac2",
                "Gmail",
            )]
        );
    }

    #[test]
    fn filter_tool_suggest_discoverable_connectors_excludes_accessible_apps_even_when_disabled() {
        let filtered = filter_tool_suggest_discoverable_connectors(
            vec![
                named_app(
                    "connector_2128aebfecb84f64a069897515042a44",
                    "Google Calendar",
                ),
                named_app("connector_68df038e0ba48191908c8434991bbac2", "Gmail"),
            ],
            &[
                AppInfo {
                    is_accessible: true,
                    ..named_app(
                        "connector_2128aebfecb84f64a069897515042a44",
                        "Google Calendar",
                    )
                },
                AppInfo {
                    is_accessible: true,
                    is_enabled: false,
                    ..named_app("connector_68df038e0ba48191908c8434991bbac2", "Gmail")
                },
            ],
            &HashSet::from([
                "connector_2128aebfecb84f64a069897515042a44".to_string(),
                "connector_68df038e0ba48191908c8434991bbac2".to_string(),
            ]),
        );

        assert_eq!(filtered, Vec::<AppInfo>::new());
    }
}
