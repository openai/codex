use std::time::Duration;

use futures::future::BoxFuture;
use serde_json::Map;
use serde_json::Value;

/// One MCP tool call requested by a configured hook handler.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HookMcpCall {
    pub server: String,
    pub tool: String,
    pub input: Map<String, Value>,
    pub timeout: Duration,
}

/// Executes already-connected MCP tools on behalf of hooks without coupling this crate to core.
///
/// Implementations own server readiness, policy enforcement, timeout handling, and elicitation.
pub trait HookMcpExecutor: Send + Sync {
    /// Returns text that is interpreted using ordinary command-hook output semantics.
    fn execute(&self, call: HookMcpCall) -> BoxFuture<'_, anyhow::Result<String>>;
}
