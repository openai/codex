//! Hot-path helpers for correlating concrete MCP executions with rollout traces.
//!
//! Core decides when an MCP request is actually going to execute. The trace
//! crate owns the globally unique ID, the trace event that preserves it in the
//! reduced artifact, and the bridge-private MCP request metadata key.

use crate::McpCallId;
use serde_json::Value as JsonValue;

const MCP_CALL_ID_META_KEY: &str = "codex_bridge_mcp_call_id";

/// No-op capable handle for one concrete MCP backend call.
#[derive(Clone, Debug)]
pub struct McpCallTraceContext {
    mcp_call_id: Option<McpCallId>,
}

impl McpCallTraceContext {
    /// Builds a context that records nothing and leaves request metadata unchanged.
    pub fn disabled() -> Self {
        Self { mcp_call_id: None }
    }

    /// Builds the trace handle for one concrete MCP execution.
    pub(crate) fn enabled(mcp_call_id: McpCallId) -> Self {
        Self {
            mcp_call_id: Some(mcp_call_id),
        }
    }

    /// Returns the trace-owned MCP call ID when rollout tracing is enabled.
    pub(crate) fn mcp_call_id(&self) -> Option<&str> {
        self.mcp_call_id.as_deref()
    }

    /// Adds bridge-private MCP correlation metadata to one outgoing request.
    pub fn add_request_meta(&self, meta: Option<JsonValue>) -> Option<JsonValue> {
        let Some(mcp_call_id) = self.mcp_call_id() else {
            return meta;
        };

        match meta {
            Some(JsonValue::Object(mut map)) => {
                map.insert(
                    MCP_CALL_ID_META_KEY.to_string(),
                    JsonValue::String(mcp_call_id.to_string()),
                );
                Some(JsonValue::Object(map))
            }
            None => {
                let mut map = serde_json::Map::new();
                map.insert(
                    MCP_CALL_ID_META_KEY.to_string(),
                    JsonValue::String(mcp_call_id.to_string()),
                );
                Some(JsonValue::Object(map))
            }
            // This should never happen but if it does then we'll fallback to
            // a noop rather than any breaking behavior. The tracing is best
            // effort after all.
            Some(_) => meta,
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::MCP_CALL_ID_META_KEY;
    use super::McpCallTraceContext;

    #[test]
    fn disabled_mcp_trace_leaves_request_meta_unchanged() {
        let meta = Some(json!({"source": "test"}));

        assert_eq!(
            McpCallTraceContext::disabled().add_request_meta(meta.clone()),
            meta
        );
    }

    #[test]
    fn enabled_mcp_trace_adds_bridge_correlation_meta() {
        let trace = McpCallTraceContext::enabled("mcp-call-id".to_string());
        let meta = trace
            .add_request_meta(Some(json!({"source": "test"})))
            .expect("enabled trace keeps request metadata");
        let object = meta
            .as_object()
            .expect("MCP request metadata remains an object");

        assert_eq!(object["source"], json!("test"));
        assert_eq!(
            object[MCP_CALL_ID_META_KEY],
            json!(trace.mcp_call_id().expect("enabled trace has an ID"))
        );
    }
}
