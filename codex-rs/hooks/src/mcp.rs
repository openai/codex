use std::time::Duration;

use futures::future::BoxFuture;
use serde_json::Map;
use serde_json::Value;

/// One MCP tool call requested by a configured hook handler.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HookMcpCall {
    /// A bundled REPL cleanup may skip a private local transport whose startup was never requested.
    pub skip_dormant_repl_cleanup: bool,
    /// Exact bundled App identity; executors must enforce live connector and approval policy.
    pub expected_app_connector_id: Option<&'static str>,
    pub server: String,
    pub tool: String,
    pub environment_id: Option<String>,
    pub metadata: Option<Map<String, Value>>,
    pub input: Map<String, Value>,
    pub timeout: Duration,
}

/// The original response returned by an MCP hook and the callable namespace that supplied it.
pub struct HookMcpOutput {
    pub result: codex_protocol::mcp::CallToolResult,
    pub source_tool_namespace: Option<String>,
}

/// Executes MCP tools on behalf of hooks without coupling this crate to core.
///
/// Implementations own server readiness, policy enforcement, timeout handling, and elicitation.
pub trait HookMcpExecutor: Send + Sync {
    /// Preserves MCP content metadata so the hook runner can interpret the result.
    fn execute(&self, call: HookMcpCall) -> BoxFuture<'_, anyhow::Result<HookMcpOutput>>;
}
