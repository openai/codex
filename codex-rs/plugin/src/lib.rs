//! Shared plugin identifiers and telemetry-facing summaries.

pub use codex_utils_plugins::mention_syntax;
pub use codex_utils_plugins::plugin_namespace_for_skill_path;

mod load_outcome;
mod plugin_id;

use codex_config::HookEventsToml;
use codex_utils_absolute_path::AbsolutePathBuf;
pub use load_outcome::EffectiveSkillRoots;
pub use load_outcome::LoadedPlugin;
pub use load_outcome::PluginLoadOutcome;
pub use load_outcome::prompt_safe_plugin_description;
pub use plugin_id::PluginId;
pub use plugin_id::PluginIdError;
pub use plugin_id::validate_plugin_segment;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AppConnectorId(pub String);

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
    pub plugin_id: PluginId,
    /// Optional backend identifier for remote plugins, used when analytics
    /// should report the remote id instead of the local plugin cache id.
    pub remote_plugin_id: Option<String>,
    pub capability_summary: Option<PluginCapabilitySummary>,
}

impl PluginTelemetryMetadata {
    pub fn from_plugin_id(plugin_id: &PluginId) -> Self {
        Self {
            plugin_id: plugin_id.clone(),
            remote_plugin_id: None,
            capability_summary: None,
        }
    }
}

impl PluginCapabilitySummary {
    pub fn telemetry_metadata(&self) -> Option<PluginTelemetryMetadata> {
        PluginId::parse(&self.config_name)
            .ok()
            .map(|plugin_id| PluginTelemetryMetadata {
                plugin_id,
                remote_plugin_id: None,
                capability_summary: Some(self.clone()),
            })
    }
}
