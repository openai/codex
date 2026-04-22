use std::collections::HashSet;

use codex_app_server_protocol::AppInfo;

pub fn filter_tool_suggest_discoverable_connectors(
    directory_connectors: Vec<AppInfo>,
    accessible_connectors: &[AppInfo],
    discoverable_connector_ids: &HashSet<String>,
    originator_value: &str,
    plugin_declared_connector_ids: &HashSet<String>,
) -> Vec<AppInfo> {
    let accessible_connector_ids: HashSet<&str> = accessible_connectors
        .iter()
        .filter(|connector| connector.is_accessible)
        .map(|connector| connector.id.as_str())
        .collect();

    let mut connectors = filter_disallowed_connectors(
        directory_connectors,
        originator_value,
        plugin_declared_connector_ids,
    )
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
const DISALLOWED_CONNECTOR_PREFIX: &str = "connector_openai_";

pub fn filter_disallowed_connectors(
    connectors: Vec<AppInfo>,
    originator_value: &str,
    plugin_declared_connector_ids: &HashSet<String>,
) -> Vec<AppInfo> {
    connectors
        .into_iter()
        .filter(|connector| {
            connector_id_passes_disallow_filter(
                connector.id.as_str(),
                originator_value,
                plugin_declared_connector_ids,
            )
        })
        .collect()
}

fn is_first_party_chat_originator(originator_value: &str) -> bool {
    originator_value == "codex_atlas" || originator_value == "codex_chatgpt_desktop"
}

pub fn connector_id_passes_disallow_filter(
    connector_id: &str,
    originator_value: &str,
    plugin_declared_connector_ids: &HashSet<String>,
) -> bool {
    let first_party_chat_originator = is_first_party_chat_originator(originator_value);
    let disallowed_connector_ids = if first_party_chat_originator {
        FIRST_PARTY_CHAT_DISALLOWED_CONNECTOR_IDS
    } else {
        DISALLOWED_CONNECTOR_IDS
    };

    if disallowed_connector_ids.contains(&connector_id) {
        return false;
    }

    !connector_id.starts_with(DISALLOWED_CONNECTOR_PREFIX)
        || plugin_declared_connector_ids.contains(connector_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn app(id: &str) -> AppInfo {
        AppInfo {
            id: id.to_string(),
            name: id.to_string(),
            description: None,
            logo_url: None,
            logo_url_dark: None,
            distribution_channel: None,
            branding: None,
            app_metadata: None,
            labels: None,
            install_url: None,
            is_accessible: false,
            is_enabled: true,
            plugin_display_names: Vec::new(),
        }
    }

    #[test]
    fn filter_disallowed_connectors_filters_openai_prefix_without_plugin_provenance() {
        let filtered = filter_disallowed_connectors(
            vec![app("connector_openai_foo"), app("gamma")],
            "codex_cli",
            &HashSet::new(),
        );

        assert_eq!(filtered, vec![app("gamma")]);
    }

    #[test]
    fn filter_disallowed_connectors_allows_plugin_declared_openai_prefix() {
        let plugin_declared_connector_ids =
            HashSet::from(["connector_openai_appgarden".to_string()]);

        let filtered = filter_disallowed_connectors(
            vec![
                app("connector_openai_appgarden"),
                app("connector_openai_hidden"),
                app("gamma"),
            ],
            "codex_cli",
            &plugin_declared_connector_ids,
        );

        assert_eq!(
            filtered,
            vec![app("connector_openai_appgarden"), app("gamma")]
        );
    }

    #[test]
    fn filter_disallowed_connectors_keeps_explicit_denylist_over_plugin_provenance() {
        let plugin_declared_connector_ids =
            HashSet::from(["asdk_app_6938a94a61d881918ef32cb999ff937c".to_string()]);

        let filtered = filter_disallowed_connectors(
            vec![
                app("asdk_app_6938a94a61d881918ef32cb999ff937c"),
                app("gamma"),
            ],
            "codex_cli",
            &plugin_declared_connector_ids,
        );

        assert_eq!(filtered, vec![app("gamma")]);
    }
}
