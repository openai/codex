//! Plugin path resolution, plaintext mention sigils, and MCP connector helpers shared across Codex
//! crates.

pub mod mcp_connector;
pub mod mention_syntax;
pub mod plugin_namespace;

pub use plugin_namespace::PLUGIN_MANIFEST_PATH;
pub use plugin_namespace::plugin_namespace_for_skill_path;
