//! MCP context passed to plugin catalog providers.

use codex_mcp::McpResourceClient;
use std::sync::Arc;

pub use codex_plugin::PluginProvider;
pub use codex_plugin::PluginProviderError;
pub use codex_plugin::PluginProviderFuture;

/// Turn identifiers and MCP client used to request one catalog snapshot.
#[derive(Clone, Debug)]
pub struct PluginListQuery {
    pub thread_id: String,
    pub turn_id: String,
    pub mcp_resources: Option<Arc<McpResourceClient>>,
}
