//! Plugin path resolution and plaintext mention sigils shared across Codex crates.

pub mod mention_syntax;
pub mod plugin_namespace;

pub use plugin_namespace::PLUGIN_MANIFEST_PATH;
pub use plugin_namespace::plugin_namespace_for_skill_path;
