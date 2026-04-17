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
use crate::tools::registry::ToolKind;
use serde_json::Value;

pub struct McpHandler;
impl ToolHandler for McpHandler {
    type Output = McpToolOutput;

    fn kind(&self) -> ToolKind {
        ToolKind::Mcp
    }

    fn pre_tool_use_payload(&self, invocation: &ToolInvocation) -> Option<PreToolUsePayload> {
        let ToolPayload::Mcp { raw_arguments, .. } = &invocation.payload else {
            return None;
        };

        Some(PreToolUsePayload {
            tool_name: HookToolName::new(invocation.tool_name.display()),
            tool_input: mcp_hook_tool_input(raw_arguments),
        })
    }

    fn post_tool_use_payload(
        &self,
        invocation: &ToolInvocation,
        result: &dyn ToolOutput,
    ) -> Option<PostToolUsePayload> {
        let ToolPayload::Mcp { raw_arguments, .. } = &invocation.payload else {
            return None;
        };

        let tool_response =
            result.post_tool_use_response(&invocation.call_id, &invocation.payload)?;
        Some(PostToolUsePayload {
            tool_name: HookToolName::new(invocation.tool_name.display()),
            tool_input: mcp_hook_tool_input(raw_arguments),
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
            ToolPayload::Mcp {
                server,
                tool,
                raw_arguments,
            } => (server, tool, raw_arguments),
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "mcp handler received unsupported payload".to_string(),
                ));
            }
        };

        let (server, tool, raw_arguments) = payload;
        let arguments_str = raw_arguments;

        let started = Instant::now();
        let result = handle_mcp_tool_call(
            Arc::clone(&session),
            &turn,
            call_id.clone(),
            server,
            tool,
            arguments_str,
        )
        .await;

        Ok(McpToolOutput {
            result,
            wall_time: started.elapsed(),
            original_image_detail_supported: can_request_original_image_detail(&turn.model_info),
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
    use crate::turn_diff_tracker::TurnDiffTracker;
    use pretty_assertions::assert_eq;
    use serde_json::json;
    use std::time::Duration;
    use tokio::sync::Mutex;

    #[tokio::test]
    async fn mcp_pre_tool_use_payload_uses_model_tool_name_and_raw_args() {
        let payload = ToolPayload::Mcp {
            server: "memory".to_string(),
            tool: "create_entities".to_string(),
            raw_arguments: json!({
                "entities": [{
                    "name": "Ada",
                    "entityType": "person"
                }]
            })
            .to_string(),
        };
        let (session, turn) = make_session_and_context().await;

        assert_eq!(
            McpHandler.pre_tool_use_payload(&ToolInvocation {
                session: session.into(),
                turn: turn.into(),
                tracker: Arc::new(Mutex::new(TurnDiffTracker::new())),
                call_id: "call-mcp-pre".to_string(),
                tool_name: codex_tools::ToolName::namespaced("mcp__memory__", "create_entities"),
                payload,
            }),
            Some(PreToolUsePayload {
                tool_name: "mcp__memory__create_entities".to_string(),
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
        let payload = ToolPayload::Mcp {
            server: "filesystem".to_string(),
            tool: "read_file".to_string(),
            raw_arguments: json!({ "path": "/tmp/notes.txt" }).to_string(),
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
            wall_time: Duration::from_millis(42),
            original_image_detail_supported: true,
        };
        let (session, turn) = make_session_and_context().await;

        assert_eq!(
            McpHandler.post_tool_use_payload(
                &ToolInvocation {
                    session: session.into(),
                    turn: turn.into(),
                    tracker: Arc::new(Mutex::new(TurnDiffTracker::new())),
                    call_id: "call-mcp-post".to_string(),
                    tool_name: codex_tools::ToolName::namespaced("mcp__filesystem__", "read_file"),
                    payload,
                },
                &output,
            ),
            Some(PostToolUsePayload {
                tool_name: "mcp__filesystem__read_file".to_string(),
                tool_input: json!({ "path": "/tmp/notes.txt" }),
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
}
