use codex_app_server_protocol::AuthMode;
use codex_plugin::AppDeclaration;
use std::collections::HashMap;
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PluginCapabilityContext {
    auth_mode: Option<AuthMode>,
    plugin_active: bool,
}

impl PluginCapabilityContext {
    pub(crate) fn new(auth_mode: Option<AuthMode>, plugin_active: bool) -> Self {
        Self {
            auth_mode,
            plugin_active,
        }
    }

    pub(crate) fn apps_route_available(self) -> bool {
        self.auth_mode.is_some_and(AuthMode::uses_codex_backend)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PluginCapabilities<M> {
    pub(crate) apps: Vec<AppDeclaration>,
    pub(crate) mcp_servers: HashMap<String, M>,
}

impl<M> PluginCapabilities<M> {
    pub(crate) fn new(
        apps: Vec<AppDeclaration>,
        mcp_servers: HashMap<String, M>,
    ) -> PluginCapabilities<M> {
        Self { apps, mcp_servers }
    }
}

pub(crate) fn resolve_plugin_capabilities<M>(
    mut capabilities: PluginCapabilities<M>,
    context: PluginCapabilityContext,
) -> PluginCapabilities<M> {
    if context.apps_route_available() {
        if context.plugin_active && !capabilities.apps.is_empty() {
            let app_declaration_names = capabilities
                .apps
                .iter()
                .map(|app| app.name.as_str())
                .collect::<HashSet<_>>();
            capabilities
                .mcp_servers
                .retain(|name, _| !app_declaration_names.contains(name.as_str()));
        }
    } else {
        capabilities.apps.clear();
    }

    capabilities
}

#[cfg(test)]
#[path = "capabilities_tests.rs"]
mod tests;
