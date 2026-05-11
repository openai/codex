use codex_app_server_protocol::McpToolCallResult;
use codex_app_server_protocol::Thread;
use codex_app_server_protocol::ThreadItem;
use serde_json::Value as JsonValue;

// Temporary bandaid for remote clients: thread/resume can include large MCP and
// image-generation payloads. Keep this response-only so persisted rollout
// history, model resume history, and other APIs stay unchanged.
const REDACTED_PAYLOAD: &str = "[redacted]";
const CHATGPT_REMOTE_CLIENT_NAMES: &[&str] =
    &["codex_chatgpt_android_remote", "codex_chatgpt_ios_remote"];

pub(super) fn should_redact_thread_resume_payloads(client_name: Option<&str>) -> bool {
    client_name.is_some_and(|client_name| CHATGPT_REMOTE_CLIENT_NAMES.contains(&client_name))
}

pub(super) fn redact_thread_resume_payloads(thread: &mut Thread) {
    for turn in &mut thread.turns {
        turn.items.retain_mut(|item| match item {
            ThreadItem::McpToolCall {
                arguments,
                result,
                error,
                ..
            } => {
                *arguments = JsonValue::String(REDACTED_PAYLOAD.to_string());
                if result.is_some() {
                    *result = Some(Box::new(redacted_mcp_tool_call_result()));
                }
                if let Some(error) = error {
                    error.message = REDACTED_PAYLOAD.to_string();
                }
                true
            }
            ThreadItem::ImageGeneration { .. } => false,
            _ => true,
        });
    }
}

fn redacted_mcp_tool_call_result() -> McpToolCallResult {
    McpToolCallResult {
        content: vec![serde_json::json!({
            "type": "text",
            "text": REDACTED_PAYLOAD,
        })],
        structured_content: None,
        meta: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_app_server_protocol::McpToolCallError;
    use codex_app_server_protocol::McpToolCallStatus;
    use codex_app_server_protocol::SessionSource;
    use codex_app_server_protocol::ThreadStatus;
    use codex_app_server_protocol::Turn;
    use codex_app_server_protocol::TurnItemsView;
    use codex_app_server_protocol::TurnStatus;
    use codex_utils_absolute_path::test_support::PathBufExt;
    use codex_utils_absolute_path::test_support::test_path_buf;
    use pretty_assertions::assert_eq;

    #[test]
    fn redacts_mcp_success_result_and_removes_image_generation() {
        let mut thread = test_thread(vec![
            ThreadItem::AgentMessage {
                id: "agent-1".to_string(),
                text: "kept".to_string(),
                phase: None,
                memory_citation: None,
            },
            ThreadItem::McpToolCall {
                id: "mcp-1".to_string(),
                server: "docs".to_string(),
                tool: "lookup".to_string(),
                status: McpToolCallStatus::Completed,
                arguments: serde_json::json!({"secret":"argument"}),
                mcp_app_resource_uri: Some("ui://widget/lookup.html".to_string()),
                result: Some(Box::new(McpToolCallResult {
                    content: vec![serde_json::json!({
                        "type": "text",
                        "text": "secret result"
                    })],
                    structured_content: Some(serde_json::json!({"secret":"structured"})),
                    meta: Some(serde_json::json!({"secret":"meta"})),
                })),
                error: None,
                duration_ms: Some(8),
            },
            ThreadItem::ImageGeneration {
                id: "ig-1".to_string(),
                status: "completed".to_string(),
                revised_prompt: Some("revised".to_string()),
                result: "base64-result".to_string(),
                saved_path: Some(test_path_buf("/tmp/ig-1.png").abs()),
            },
        ]);

        redact_thread_resume_payloads(&mut thread);

        assert_eq!(thread.turns[0].items.len(), 2);
        assert_eq!(
            thread.turns[0].items[0],
            ThreadItem::AgentMessage {
                id: "agent-1".to_string(),
                text: "kept".to_string(),
                phase: None,
                memory_citation: None,
            }
        );
        assert_eq!(
            thread.turns[0].items[1],
            ThreadItem::McpToolCall {
                id: "mcp-1".to_string(),
                server: "docs".to_string(),
                tool: "lookup".to_string(),
                status: McpToolCallStatus::Completed,
                arguments: JsonValue::String(REDACTED_PAYLOAD.to_string()),
                mcp_app_resource_uri: Some("ui://widget/lookup.html".to_string()),
                result: Some(Box::new(redacted_mcp_tool_call_result())),
                error: None,
                duration_ms: Some(8),
            }
        );
    }

    #[test]
    fn redacts_mcp_error_message() {
        let mut thread = test_thread(vec![ThreadItem::McpToolCall {
            id: "mcp-1".to_string(),
            server: "docs".to_string(),
            tool: "lookup".to_string(),
            status: McpToolCallStatus::Failed,
            arguments: serde_json::json!({"secret":"argument"}),
            mcp_app_resource_uri: None,
            result: None,
            error: Some(McpToolCallError {
                message: "secret error".to_string(),
            }),
            duration_ms: Some(8),
        }]);

        redact_thread_resume_payloads(&mut thread);

        assert_eq!(
            thread.turns[0].items[0],
            ThreadItem::McpToolCall {
                id: "mcp-1".to_string(),
                server: "docs".to_string(),
                tool: "lookup".to_string(),
                status: McpToolCallStatus::Failed,
                arguments: JsonValue::String(REDACTED_PAYLOAD.to_string()),
                mcp_app_resource_uri: None,
                result: None,
                error: Some(McpToolCallError {
                    message: REDACTED_PAYLOAD.to_string(),
                }),
                duration_ms: Some(8),
            }
        );
    }

    fn test_thread(items: Vec<ThreadItem>) -> Thread {
        Thread {
            id: "thread-1".to_string(),
            session_id: "session-1".to_string(),
            forked_from_id: None,
            preview: "preview".to_string(),
            ephemeral: false,
            model_provider: "mock_provider".to_string(),
            created_at: 0,
            updated_at: 0,
            status: ThreadStatus::Idle,
            path: None,
            cwd: test_path_buf("/tmp").abs(),
            cli_version: "0.0.0".to_string(),
            source: SessionSource::Cli,
            thread_source: None,
            agent_nickname: None,
            agent_role: None,
            git_info: None,
            name: None,
            turns: vec![Turn {
                id: "turn-1".to_string(),
                items,
                items_view: TurnItemsView::Full,
                status: TurnStatus::Completed,
                error: None,
                started_at: None,
                completed_at: None,
                duration_ms: None,
            }],
        }
    }
}
