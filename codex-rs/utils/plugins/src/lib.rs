//! Plugin path resolution, plaintext mention sigils, and MCP connector helpers shared across Codex
//! crates.

use codex_utils_absolute_path::AbsolutePathBuf;

pub mod mcp_connector;
pub mod mention_syntax;
pub mod plugin_namespace;

pub use plugin_namespace::find_plugin_manifest_path;
pub use plugin_namespace::plugin_namespace_for_skill_path;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PluginSkillRoot {
    pub path: AbsolutePathBuf,
    pub plugin_id: String,
}
