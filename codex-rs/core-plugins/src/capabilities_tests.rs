use super::*;
use codex_plugin::AppConnectorId;
use pretty_assertions::assert_eq;
use std::collections::HashMap;

fn app(name: &str) -> AppDeclaration {
    AppDeclaration {
        name: name.to_string(),
        connector_id: AppConnectorId(format!("connector_{name}")),
        category: None,
    }
}

fn capabilities(
    apps: Vec<AppDeclaration>,
    mcp_servers: impl IntoIterator<Item = (&'static str, i32)>,
) -> PluginCapabilities<i32> {
    PluginCapabilities::new(
        apps,
        mcp_servers
            .into_iter()
            .map(|(name, value)| (name.to_string(), value))
            .collect::<HashMap<_, _>>(),
    )
}

fn sorted_app_names(capabilities: &PluginCapabilities<i32>) -> Vec<String> {
    let mut names = capabilities
        .apps
        .iter()
        .map(|app| app.name.clone())
        .collect::<Vec<_>>();
    names.sort();
    names
}

fn sorted_mcp_server_names(capabilities: &PluginCapabilities<i32>) -> Vec<String> {
    let mut names = capabilities.mcp_servers.keys().cloned().collect::<Vec<_>>();
    names.sort();
    names
}

fn has_usable_capabilities(
    apps: Vec<AppDeclaration>,
    mcp_servers: impl IntoIterator<Item = (&'static str, i32)>,
    has_skills: bool,
    auth_mode: Option<AuthMode>,
) -> bool {
    plugin_has_usable_capabilities(
        capabilities(apps, mcp_servers),
        has_skills,
        PluginCapabilityContext::new(auth_mode, /*plugin_active*/ true),
    )
}

fn visible_in_marketplace(
    apps: Vec<AppDeclaration>,
    mcp_servers: impl IntoIterator<Item = (&'static str, i32)>,
    has_skills: bool,
    auth_mode: Option<AuthMode>,
) -> bool {
    plugin_is_visible_in_marketplace(
        capabilities(apps, mcp_servers),
        has_skills,
        PluginCapabilityContext::new(auth_mode, /*plugin_active*/ true),
    )
}

#[test]
fn apps_route_available_tracks_auth_mode_capabilities() {
    assert!(
        PluginCapabilityContext::new(Some(AuthMode::Chatgpt), /*plugin_active*/ true)
            .apps_route_available()
    );
    assert!(
        PluginCapabilityContext::new(Some(AuthMode::AgentIdentity), /*plugin_active*/ true)
            .apps_route_available()
    );
    assert!(
        !PluginCapabilityContext::new(Some(AuthMode::ApiKey), /*plugin_active*/ true)
            .apps_route_available()
    );
    assert!(
        !PluginCapabilityContext::new(/*auth_mode*/ None, /*plugin_active*/ true)
            .apps_route_available()
    );
}

#[test]
fn resolver_clears_apps_when_apps_route_is_unavailable() {
    let resolved = resolve_plugin_capabilities(
        capabilities(vec![app("linear")], [("linear", 1), ("docs", 2)]),
        PluginCapabilityContext::new(Some(AuthMode::ApiKey), /*plugin_active*/ true),
    );

    assert!(resolved.apps.is_empty());
    assert_eq!(
        sorted_mcp_server_names(&resolved),
        vec!["docs".to_string(), "linear".to_string()]
    );
}

#[test]
fn resolver_preserves_apps_and_removes_conflicting_mcp_with_apps_route() {
    let resolved = resolve_plugin_capabilities(
        capabilities(
            vec![app("linear"), app("notion")],
            [("linear", 1), ("docs", 2), ("notion", 3)],
        ),
        PluginCapabilityContext::new(Some(AuthMode::Chatgpt), /*plugin_active*/ true),
    );

    assert_eq!(
        sorted_app_names(&resolved),
        vec!["linear".to_string(), "notion".to_string()]
    );
    assert_eq!(sorted_mcp_server_names(&resolved), vec!["docs".to_string()]);
}

#[test]
fn resolver_preserves_mcp_conflicts_when_plugin_is_inactive() {
    let resolved = resolve_plugin_capabilities(
        capabilities(vec![app("linear")], [("linear", 1), ("docs", 2)]),
        PluginCapabilityContext::new(Some(AuthMode::Chatgpt), /*plugin_active*/ false),
    );

    assert_eq!(sorted_app_names(&resolved), vec!["linear".to_string()]);
    assert_eq!(
        sorted_mcp_server_names(&resolved),
        vec!["docs".to_string(), "linear".to_string()]
    );
}

#[test]
fn usability_requires_apps_to_be_covered_by_mcp_when_apps_route_is_unavailable() {
    assert!(!has_usable_capabilities(
        vec![app("linear")],
        [],
        /*has_skills*/ false,
        Some(AuthMode::ApiKey)
    ));
    assert!(!has_usable_capabilities(
        vec![app("linear")],
        [],
        /*has_skills*/ true,
        Some(AuthMode::ApiKey)
    ));
    assert!(!has_usable_capabilities(
        vec![app("linear")],
        [("other", 1)],
        /*has_skills*/ false,
        Some(AuthMode::ApiKey)
    ));
    assert!(has_usable_capabilities(
        vec![app("linear")],
        [("linear", 1)],
        /*has_skills*/ false,
        Some(AuthMode::ApiKey)
    ));
    assert!(has_usable_capabilities(
        vec![],
        [("other", 1)],
        /*has_skills*/ false,
        Some(AuthMode::ApiKey)
    ));
    assert!(has_usable_capabilities(
        vec![],
        [],
        /*has_skills*/ true,
        Some(AuthMode::ApiKey)
    ));
}

#[test]
fn usability_keeps_apps_when_apps_route_is_available() {
    assert!(has_usable_capabilities(
        vec![app("linear")],
        [],
        /*has_skills*/ false,
        Some(AuthMode::Chatgpt)
    ));
    assert!(has_usable_capabilities(
        vec![app("linear")],
        [("linear", 1)],
        /*has_skills*/ false,
        Some(AuthMode::Chatgpt)
    ));
}

#[test]
fn marketplace_visibility_filters_only_direct_auth_modes() {
    assert!(visible_in_marketplace(
        vec![app("linear")],
        [],
        /*has_skills*/ false,
        /*auth_mode*/ None
    ));
    assert!(visible_in_marketplace(
        vec![app("linear")],
        [],
        /*has_skills*/ false,
        Some(AuthMode::Chatgpt)
    ));
    assert!(!visible_in_marketplace(
        vec![app("linear")],
        [],
        /*has_skills*/ false,
        Some(AuthMode::ApiKey)
    ));
    assert!(visible_in_marketplace(
        vec![app("linear")],
        [("linear", 1)],
        /*has_skills*/ false,
        Some(AuthMode::ApiKey)
    ));
}
