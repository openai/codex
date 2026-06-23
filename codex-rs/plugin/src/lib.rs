//! Shared plugin package models, source providers, identifiers, and telemetry summaries.

use std::collections::HashSet;

pub use codex_utils_plugins::mention_syntax;
pub use codex_utils_plugins::plugin_namespace_for_skill_path;

mod load_outcome;
pub mod manifest;
mod plugin_id;
mod provider;

use codex_config::HookEventsToml;
use codex_utils_absolute_path::AbsolutePathBuf;
pub use load_outcome::EffectiveSkillRoots;
pub use load_outcome::LoadedPlugin;
pub use load_outcome::PluginLoadOutcome;
pub use load_outcome::prompt_safe_plugin_description;
pub use plugin_id::PluginId;
pub use plugin_id::PluginIdError;
pub use plugin_id::validate_plugin_segment;
pub use provider::PluginProvider;
pub use provider::PluginResourceLocator;
pub use provider::ResolvedPlugin;
pub use provider::ResolvedPluginError;
pub use provider::ResolvedPluginLocation;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AppConnectorId(pub String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppDeclaration {
    pub name: String,
    pub connector_id: AppConnectorId,
    pub category: Option<String>,
}

pub fn app_connector_ids_from_declarations<'a>(
    app_declarations: impl IntoIterator<Item = &'a AppDeclaration>,
) -> Vec<AppConnectorId> {
    let mut connector_ids = Vec::new();
    let mut seen_connector_ids = HashSet::new();
    for app in app_declarations {
        if seen_connector_ids.insert(&app.connector_id) {
            connector_ids.push(app.connector_id.clone());
        }
    }
    connector_ids
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PluginCapabilitySummary {
    pub config_name: String,
    pub display_name: String,
    pub description: Option<String>,
    pub has_skills: bool,
    pub mcp_server_names: Vec<String>,
    pub app_connector_ids: Vec<AppConnectorId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginHookSource {
    pub plugin_id: PluginId,
    pub plugin_root: AbsolutePathBuf,
    pub plugin_data_root: AbsolutePathBuf,
    pub source_path: AbsolutePathBuf,
    pub source_relative_path: String,
    pub hooks: HookEventsToml,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginTelemetryMetadata {
    /// Local plugin identifier used by Codex configuration and the plugin cache,
    /// when it has been resolved.
    pub plugin_id: Option<PluginId>,
    /// Optional backend identifier for remote plugins.
    pub remote_plugin_id: Option<String>,
    pub capability_summary: Option<PluginCapabilitySummary>,
}
