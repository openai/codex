//! Plugin path resolution, plaintext mention sigils, and MCP connector helpers shared across Codex
//! crates.

use codex_utils_absolute_path::AbsolutePathBuf;

pub mod mcp_connector;
pub mod mention_syntax;
pub mod plugin_namespace;

pub use codex_exec_server_protocol::DISCOVERABLE_PLUGIN_MANIFEST_PATHS;
pub use plugin_namespace::AGENT_PLUGIN_MANIFEST_RELATIVE_PATH;
pub use plugin_namespace::AGENT_PLUGIN_SCHEMA_PREFIX;
pub use plugin_namespace::AGENT_PLUGIN_SCHEMA_URI;
pub use plugin_namespace::AgentPluginSchemaStatus;
pub use plugin_namespace::SUPPORTED_AGENT_PLUGIN_SCHEMA_URIS;
pub use plugin_namespace::agent_plugin_schema_status;
pub use plugin_namespace::find_plugin_manifest_path;
pub use plugin_namespace::plugin_namespace_for_root_uri;
pub use plugin_namespace::plugin_namespace_for_skill_path;
pub use plugin_namespace::plugin_namespace_for_skill_uri;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum SkillDiscoveryMode {
    #[default]
    Recursive,
    DirectChildren,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PluginSkillRoot {
    pub path: AbsolutePathBuf,
    pub plugin_id: String,
    pub plugin_namespace: String,
    pub plugin_root: AbsolutePathBuf,
    pub discovery_mode: SkillDiscoveryMode,
}
