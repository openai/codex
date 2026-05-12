use std::sync::Arc;
use std::time::Instant;

use crate::function_tool::FunctionCallError;
use crate::mcp_tool_call::handle_mcp_tool_call;
use crate::original_image_detail::can_request_original_image_detail;
use crate::tools::context::McpToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::hook_names::HookToolName;
use crate::tools::registry::PostToolUsePayload;
use crate::tools::registry::PreToolUsePayload;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolTelemetryTags;
use codex_mcp::ToolInfo;
use codex_tools::ToolName;
use serde_json::Value;

pub struct McpHandler {
    tool_info: ToolInfo,
}

impl McpHandler {
    pub fn new(tool_info: ToolInfo) -> Self {
        Self { tool_info }
    }
}

impl ToolHandler for McpHandler {
    type Output = McpToolOutput;

    fn tool_name(&self) -> ToolName {
        self.tool_info.canonical_tool_name()
    }

    fn supports_parallel_tool_calls(&self) -> bool {
        self.tool_info.supports_parallel_tool_calls
    }

    async fn telemetry_tags(&self, _invocation: &ToolInvocation) -> ToolTelemetryTags {
        let mut tags = vec![("mcp_server", self.tool_info.server_name.clone())];
        if let Some(origin) = &self.tool_info.server_origin {
            tags.push(("mcp_server_origin", origin.clone()));
        }
        tags
    }

    fn pre_tool_use_payload(&self, invocation: &ToolInvocation) -> Option<PreToolUsePayload> {
        let ToolPayload::Function { arguments } = &invocation.payload else {
            return None;
        };

        Some(PreToolUsePayload {
            tool_name: HookToolName::new(self.tool_name().to_string()),
            tool_input: mcp_hook_tool_input(arguments),
        })
    }

    fn post_tool_use_payload(
        &self,
        invocation: &ToolInvocation,
        result: &Self::Output,
    ) -> Option<PostToolUsePayload> {
        let ToolPayload::Function { .. } = &invocation.payload else {
            return None;
        };

        let tool_response =
            result.post_tool_use_response(&invocation.call_id, &invocation.payload)?;
        Some(PostToolUsePayload {
            tool_name: HookToolName::new(self.tool_name().to_string()),
            tool_use_id: invocation.call_id.clone(),
            tool_input: result.tool_input.clone(),
            tool_response,
        })
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<Self::Output, FunctionCallError> {
        let ToolInvocation {
            session,
            turn,
            call_id,
            payload,
            ..
        } = invocation;

        let payload = match payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "mcp handler received unsupported payload".to_string(),
                ));
            }
        };

        let started = Instant::now();
        let result = handle_mcp_tool_call(
            Arc::clone(&session),
            &turn,
            call_id.clone(),
            self.tool_info.server_name.clone(),
            self.tool_info.tool.name.to_string(),
            self.tool_name().to_string(),
            payload,
        )
        .await;

        Ok(McpToolOutput {
            result: result.result,
            tool_input: result.tool_input,
            wall_time: started.elapsed(),
            original_image_detail_supported: can_request_original_image_detail(&turn.model_info),
            truncation_policy: turn.truncation_policy,
        })
    }
}

fn mcp_hook_tool_input(raw_arguments: &str) -> Value {
    if raw_arguments.trim().is_empty() {
        return Value::Object(serde_json::Map::new());
    }

    serde_json::from_str(raw_arguments).unwrap_or_else(|_| Value::String(raw_arguments.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::tests::make_session_and_context;
    use crate::tools::context::ToolCallSource;
    use crate::turn_diff_tracker::TurnDiffTracker;
    use pretty_assertions::assert_eq;
    use serde_json::json;
    use std::time::Duration;
    use tokio::sync::Mutex;

    #[tokio::test]
    async fn mcp_pre_tool_use_payload_uses_model_tool_name_and_raw_args() {
        let payload = ToolPayload::Function {
            arguments: json!({
                "entities": [{
                    "name": "Ada",
                    "entityType": "person"
                }]
            })
            .to_string(),
        };
        let (session, turn) = make_session_and_context().await;
        let handler = McpHandler::new(tool_info("memory", "mcp__memory__", "create_entities"));

        assert_eq!(
            handler.pre_tool_use_payload(&ToolInvocation {
                session: session.into(),
                turn: turn.into(),
                cancellation_token: tokio_util::sync::CancellationToken::new(),
                tracker: Arc::new(Mutex::new(TurnDiffTracker::new())),
                call_id: "call-mcp-pre".to_string(),
                tool_name: codex_tools::ToolName::namespaced("mcp__memory__", "create_entities"),
                source: ToolCallSource::Direct,
                payload,
            }),
            Some(PreToolUsePayload {
                tool_name: HookToolName::new("mcp__memory__create_entities"),
                tool_input: json!({
                    "entities": [{
                        "name": "Ada",
                        "entityType": "person"
                    }]
                }),
            })
        );
    }

    #[tokio::test]
    async fn mcp_post_tool_use_payload_uses_model_tool_name_args_and_result() {
        let payload = ToolPayload::Function {
            arguments: json!({ "path": "/tmp/notes.txt" }).to_string(),
        };
        let output = McpToolOutput {
            result: codex_protocol::mcp::CallToolResult {
                content: vec![json!({
                    "type": "text",
                    "text": "notes"
                })],
                structured_content: Some(json!({ "bytes": 5 })),
                is_error: None,
                meta: None,
            },
            tool_input: json!({
                "path": {
                    "file_id": "file_123"
                }
            }),
            wall_time: Duration::from_millis(42),
            original_image_detail_supported: true,
            truncation_policy: codex_utils_output_truncation::TruncationPolicy::Bytes(1024),
        };
        let (session, turn) = make_session_and_context().await;
        let handler = McpHandler::new(tool_info("filesystem", "mcp__filesystem__", "read_file"));
        let invocation = ToolInvocation {
            session: session.into(),
            turn: turn.into(),
            cancellation_token: tokio_util::sync::CancellationToken::new(),
            tracker: Arc::new(Mutex::new(TurnDiffTracker::new())),
            call_id: "call-mcp-post".to_string(),
            tool_name: codex_tools::ToolName::namespaced("mcp__filesystem__", "read_file"),
            source: ToolCallSource::Direct,
            payload,
        };
        assert_eq!(
            handler.post_tool_use_payload(&invocation, &output),
            Some(PostToolUsePayload {
                tool_name: HookToolName::new("mcp__filesystem__read_file"),
                tool_use_id: "call-mcp-post".to_string(),
                tool_input: json!({
                    "path": {
                        "file_id": "file_123"
                    }
                }),
                tool_response: json!({
                    "content": [{
                        "type": "text",
                        "text": "notes"
                    }],
                    "structuredContent": { "bytes": 5 }
                }),
            })
        );
    }

    #[test]
    fn mcp_hook_tool_input_defaults_empty_args_to_object() {
        assert_eq!(mcp_hook_tool_input("  "), json!({}));
    }

    fn tool_info(server_name: &str, callable_namespace: &str, tool_name: &str) -> ToolInfo {
        ToolInfo {
            server_name: server_name.to_string(),
            supports_parallel_tool_calls: false,
            server_origin: None,
            callable_name: tool_name.to_string(),
            callable_namespace: callable_namespace.to_string(),
            namespace_description: None,
            tool: rmcp::model::Tool {
                name: tool_name.to_string().into(),
                title: None,
                description: None,
                input_schema: Arc::new(rmcp::model::object(serde_json::json!({
                    "type": "object",
                }))),
                output_schema: None,
                annotations: None,
                execution: None,
                icons: None,
                meta: None,
            },
            connector_id: None,
            connector_name: None,
            plugin_display_names: Vec::new(),
        }
    }
}
