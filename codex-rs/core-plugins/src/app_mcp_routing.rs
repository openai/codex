use crate::manifest::PluginManifestFormat;
use crate::remote::REMOTE_GLOBAL_MARKETPLACE_NAME;
use codex_plugin::AppDeclaration;
use codex_plugin::PluginId;
use codex_protocol::auth::AuthMode;
use std::collections::HashMap;
use std::collections::HashSet;

pub fn apps_route_available(auth_mode: Option<AuthMode>) -> bool {
    auth_mode.is_some_and(AuthMode::uses_codex_backend)
}

pub(crate) fn plugin_supports_apps(format: PluginManifestFormat, plugin_id: &PluginId) -> bool {
    // The remote curated installer overwrites app declarations with plugin-service metadata.
    // Keep portable app declarations inactive for other Agent Plugins.
    format == PluginManifestFormat::Legacy
        || plugin_id.marketplace_name == REMOTE_GLOBAL_MARKETPLACE_NAME
}

pub(crate) fn apply_app_mcp_routing_policy<M>(
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
#[path = "app_mcp_routing_tests.rs"]
mod tests;
