use std::collections::HashSet;

use codex_app_server_protocol::AppInfo;

pub fn filter_tool_suggest_discoverable_connectors(
    directory_connectors: Vec<AppInfo>,
    accessible_connectors: &[AppInfo],
    discoverable_connector_ids: &HashSet<String>,
    originator_value: &str,
) -> Vec<AppInfo> {
    let accessible_connector_ids: HashSet<&str> = accessible_connectors
        .iter()
        .filter(|connector| connector.is_accessible)
        .map(|connector| connector.id.as_str())
        .collect();

    let mut connectors = filter_disallowed_connectors(directory_connectors, originator_value)
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

const DISALLOWED_CONNECTOR_IDS: &[&str] = &[
    "asdk_app_6938a94a61d881918ef32cb999ff937c",
    "connector_2b0a9009c9c64bf9933a3dae3f2b1254",
    "connector_3f8d1a79f27c4c7ba1a897ab13bf37dc",
    "connector_68de829bf7648191acd70a907364c67c",
    "connector_68e004f14af881919eb50893d3d9f523",
    "connector_69272cb413a081919685ec3c88d1744e",
];
const FIRST_PARTY_CHAT_DISALLOWED_CONNECTOR_IDS: &[&str] =
    &["connector_0f9c9d4592e54d0a9a12b3f44a1e2010"];

pub fn filter_disallowed_connectors(
    connectors: Vec<AppInfo>,
    originator_value: &str,
) -> Vec<AppInfo> {
    let first_party_chat_originator = is_first_party_chat_originator(originator_value);
    connectors
        .into_iter()
        .filter(|connector| {
            is_connector_id_allowed(connector.id.as_str(), first_party_chat_originator)
        })
        .collect()
}

fn is_first_party_chat_originator(originator_value: &str) -> bool {
    originator_value == "codex_atlas" || originator_value == "codex_chatgpt_desktop"
}

fn is_connector_id_allowed(connector_id: &str, first_party_chat_originator: bool) -> bool {
    let disallowed_connector_ids = if first_party_chat_originator {
        FIRST_PARTY_CHAT_DISALLOWED_CONNECTOR_IDS
    } else {
        DISALLOWED_CONNECTOR_IDS
    };

    !disallowed_connector_ids.contains(&connector_id)
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
    fn filter_disallowed_connectors_allows_non_disallowed_connectors() {
        let filtered =
            filter_disallowed_connectors(vec![app("asdk_app_hidden"), app("alpha")], "codex_cli");
        assert_eq!(filtered, vec![app("asdk_app_hidden"), app("alpha")]);
    }

    #[test]
    fn filter_disallowed_connectors_allows_openai_prefix() {
        let filtered = filter_disallowed_connectors(
            vec![
                app("connector_openai_foo"),
                app("connector_openai_bar"),
                app("gamma"),
            ],
            "codex_cli",
        );
        assert_eq!(
            filtered,
            vec![
                app("connector_openai_foo"),
                app("connector_openai_bar"),
                app("gamma")
            ]
        );
    }

    #[test]
    fn filter_disallowed_connectors_filters_disallowed_connector_ids() {
        let filtered = filter_disallowed_connectors(
            vec![
                app("asdk_app_6938a94a61d881918ef32cb999ff937c"),
                app("connector_3f8d1a79f27c4c7ba1a897ab13bf37dc"),
                app("delta"),
            ],
            "codex_cli",
        );
        assert_eq!(filtered, vec![app("delta")]);
    }

    #[test]
    fn first_party_chat_originator_filters_target_connector_ids() {
        let filtered = filter_disallowed_connectors(
            vec![
                app("connector_openai_foo"),
                app("asdk_app_6938a94a61d881918ef32cb999ff937c"),
                app("connector_0f9c9d4592e54d0a9a12b3f44a1e2010"),
            ],
            "codex_atlas",
        );
        assert_eq!(
            filtered,
            vec![
                app("connector_openai_foo"),
                app("asdk_app_6938a94a61d881918ef32cb999ff937c")
            ]
        );
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
            "codex_cli",
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
            "codex_cli",
        );

        assert_eq!(filtered, Vec::<AppInfo>::new());
    }
}
