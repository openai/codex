use std::time::Duration;

use futures::future::BoxFuture;
use serde_json::Map;
use serde_json::Value;

/// One MCP tool call requested by a configured hook handler.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HookMcpCall {
    /// A bundled REPL cleanup may skip a private local transport whose startup was never requested.
    pub skip_dormant_repl_cleanup: bool,
    pub server: String,
    pub tool: String,
    pub environment_id: Option<String>,
    pub metadata: Option<Map<String, Value>>,
    pub input: Map<String, Value>,
    pub timeout: Duration,
}

/// The MCP hook's text, structured result, and originating callable namespace.
pub struct HookMcpOutput {
    pub text: String,
    pub structured_content: Option<serde_json::Value>,
    pub source_tool_namespace: Option<String>,
}

/// Executes already-connected MCP tools on behalf of hooks without coupling this crate to core.
///
/// Implementations own server readiness, policy enforcement, timeout handling, and elicitation.
pub trait HookMcpExecutor: Send + Sync {
    /// Returns text that is interpreted using ordinary command-hook output semantics.
    fn execute(&self, call: HookMcpCall) -> BoxFuture<'_, anyhow::Result<HookMcpOutput>>;
}
