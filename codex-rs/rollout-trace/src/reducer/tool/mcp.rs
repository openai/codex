//! Resolve MCP backend identity without replacing the caller-facing invocation.

use anyhow::Context;
use anyhow::Result;
use codex_protocol::protocol::McpInvocation;
use serde::Deserialize;

use crate::model::ToolCallKind;
use crate::payload::RawPayloadRef;
use crate::reducer::TraceReducer;

impl TraceReducer {
    /// Returns whether the runtime start identifies an MCP backend call.
    pub(super) fn classify_mcp_runtime_start(
        &mut self,
        tool_call_id: &str,
        runtime_payload: &RawPayloadRef,
    ) -> Result<bool> {
        // MCP begin events precede correlation assignment, and callable names
        // can match built-ins. Only the runtime payload identifies the backend.
        let payload = self.read_payload_json(runtime_payload)?;
        let Some(invocation) = payload.get("invocation") else {
            return Ok(false);
        };
        let invocation = McpInvocation::deserialize(invocation).with_context(|| {
            format!(
                "parse MCP runtime invocation in {}",
                runtime_payload.raw_payload_id
            )
        })?;
        // Callable namespaces are aliases, not backend names: `mcp__web` can
        // route to `web`, while other MCP namespaces have no `mcp__` prefix.
        // Keep the original callable in the summary and raw invocation payload.
        self.rollout
            .tool_calls
            .get_mut(tool_call_id)
            .with_context(|| format!("MCP runtime start referenced unknown call {tool_call_id}"))?
            .kind = ToolCallKind::Mcp {
            server: invocation.server,
            tool: invocation.tool,
        };
        Ok(true)
    }
}
