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

    pub(crate) fn filters_marketplace_plugins(self) -> bool {
        self.auth_mode
            .is_some_and(|auth_mode| !auth_mode.uses_codex_backend())
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

fn app_declaration_names(apps: &[AppDeclaration]) -> HashSet<&str> {
    apps.iter().map(|app| app.name.as_str()).collect()
}

fn app_declarations_are_covered_by_mcp<M>(capabilities: &PluginCapabilities<M>) -> bool {
    capabilities
        .apps
        .iter()
        .all(|app| capabilities.mcp_servers.contains_key(app.name.as_str()))
}

pub(crate) fn resolve_plugin_capabilities<M>(
    mut capabilities: PluginCapabilities<M>,
    context: PluginCapabilityContext,
) -> PluginCapabilities<M> {
    if context.apps_route_available() {
        if context.plugin_active && !capabilities.apps.is_empty() {
            let app_declaration_names = app_declaration_names(&capabilities.apps);
            capabilities
                .mcp_servers
                .retain(|name, _| !app_declaration_names.contains(name.as_str()));
        }
    } else {
        capabilities.apps.clear();
    }

    capabilities
}

fn plugin_has_usable_capabilities<M>(
    capabilities: PluginCapabilities<M>,
    has_skills: bool,
    context: PluginCapabilityContext,
) -> bool {
    if !context.apps_route_available() && !app_declarations_are_covered_by_mcp(&capabilities) {
        return false;
    }

    let capabilities = resolve_plugin_capabilities(capabilities, context);
    has_skills || !capabilities.apps.is_empty() || !capabilities.mcp_servers.is_empty()
}

pub(crate) fn plugin_is_visible_in_marketplace<M>(
    capabilities: PluginCapabilities<M>,
    has_skills: bool,
    context: PluginCapabilityContext,
) -> bool {
    if !context.filters_marketplace_plugins() {
        return true;
    }

    plugin_has_usable_capabilities(capabilities, has_skills, context)
}

#[cfg(test)]
#[path = "capabilities_tests.rs"]
mod tests;
