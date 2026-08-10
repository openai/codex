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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum SkillDiscoveryMode {
    #[default]
    Recursive,
    DirectChildren,
}

/// The local identifier and optional remote identifier for a plugin.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PluginIdentity {
    pub plugin_id: String,
    pub remote_plugin_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PluginSkillRoot {
    pub path: AbsolutePathBuf,
    pub plugin_identity: PluginIdentity,
    pub plugin_namespace: String,
    pub plugin_root: AbsolutePathBuf,
    pub discovery_mode: SkillDiscoveryMode,
}

/// Directory containing private plugin metadata.
const PLUGIN_METADATA_DIR: &str = ".codex-plugin";
/// Directory containing commands converted into skills during plugin installation.
const MIGRATED_COMMAND_SKILLS_DIR: &str = "migrated-command-skills";

/// Returns the install-time command migration output directory for a plugin.
pub fn migrated_command_skills_root(plugin_root: &AbsolutePathBuf) -> AbsolutePathBuf {
    plugin_root
        .join(PLUGIN_METADATA_DIR)
        .join(MIGRATED_COMMAND_SKILLS_DIR)
}
