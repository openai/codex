use codex_app_server_protocol::AuthMode;
use codex_plugin::AppDeclaration;
use std::collections::HashMap;
use std::collections::HashSet;

pub(crate) fn apps_route_available(auth_mode: Option<AuthMode>) -> bool {
    auth_mode.is_some_and(AuthMode::uses_codex_backend)
}

pub(crate) fn resolve_app_and_mcp_capabilities<M>(
    apps: &mut Vec<AppDeclaration>,
    mcp_servers: &mut HashMap<String, M>,
    auth_mode: Option<AuthMode>,
    plugin_active: bool,
) {
    if !apps_route_available(auth_mode) {
        apps.clear();
        return;
    }

    if plugin_active && !apps.is_empty() {
        let app_declaration_names = apps
            .iter()
            .map(|app| app.name.as_str())
            .collect::<HashSet<_>>();
        mcp_servers.retain(|name, _| !app_declaration_names.contains(name.as_str()));
    }
}

#[cfg(test)]
#[path = "capabilities_tests.rs"]
mod tests;
