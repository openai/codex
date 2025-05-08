//! Asynchronous worker that executes a **Codex** tool-call inside a spawned
//! Tokio task. Separated from `message_processor.rs` to keep that file small
//! and to make future feature-growth easier to manage.

use codex_core::codex_wrapper::init_codex;
use codex_core::config::Config as CodexConfig;
use codex_core::protocol::Event;
use codex_core::protocol::EventMsg;
use codex_core::protocol::InputItem;
use codex_core::protocol::Op;
use mcp_types::CallToolResult;
use mcp_types::CallToolResultContent;
use mcp_types::JSONRPC_VERSION;
use mcp_types::JSONRPCMessage;
use mcp_types::JSONRPCResponse;
use mcp_types::RequestId;
use mcp_types::TextContent;
use tokio::sync::mpsc::Sender;

/// Convert a Codex [`Event`] to an MCP notification.
fn codex_event_to_notification(event: &Event) -> JSONRPCMessage {
    JSONRPCMessage::Notification(mcp_types::JSONRPCNotification {
        jsonrpc: JSONRPC_VERSION.into(),
        method: "codex/event".into(),
        params: Some(serde_json::to_value(event).expect("Event must serialize")),
    })
}

/// Run a complete Codex session and stream events back to the client.
///
/// On completion (success or error) the function sends the appropriate
/// `tools/call` response so the LLM can continue the conversation.
pub async fn run_codex_tool_session(
    id: RequestId,
    initial_prompt: String,
    config: CodexConfig,
    outgoing: Sender<JSONRPCMessage>,
) {
    let (codex, first_event, _ctrl_c) = match init_codex(config).await {
        Ok(res) => res,
        Err(e) => {
            let result = CallToolResult {
                content: vec![CallToolResultContent::TextContent(TextContent {
                    r#type: "text".to_string(),
                    text: format!("Failed to start Codex session: {e}"),
                    annotations: None,
                })],
                is_error: Some(true),
            };
            let _ = outgoing
                .send(JSONRPCMessage::Response(JSONRPCResponse {
                    jsonrpc: JSONRPC_VERSION.into(),
                    id,
                    result: result.into(),
                }))
                .await;
            return;
        }
    };

    // Send initial SessionConfigured event.
    let _ = outgoing
        .send(codex_event_to_notification(&first_event))
        .await;

    if let Err(e) = codex
        .submit(Op::UserInput {
            items: vec![InputItem::Text {
                text: initial_prompt.clone(),
            }],
        })
        .await
    {
        tracing::error!("Failed to submit initial prompt: {e}");
    }

    let mut last_agent_message: Option<String> = None;

    // Stream events until the task needs to pause for user interaction or
    // completes.
    loop {
        match codex.next_event().await {
            Ok(event) => {
                let _ = outgoing.send(codex_event_to_notification(&event)).await;

                match &event.msg {
                    EventMsg::AgentMessage { message } => {
                        last_agent_message = Some(message.clone());
                    }
                    EventMsg::ExecApprovalRequest { .. } => {
                        let result = CallToolResult {
                            content: vec![CallToolResultContent::TextContent(TextContent {
                                r#type: "text".to_string(),
                                text: "EXEC_APPROVAL_REQUIRED".to_string(),
                                annotations: None,
                            })],
                            is_error: None,
                        };
                        let _ = outgoing
                            .send(JSONRPCMessage::Response(JSONRPCResponse {
                                jsonrpc: JSONRPC_VERSION.into(),
                                id: id.clone(),
                                result: result.into(),
                            }))
                            .await;
                        break;
                    }
                    EventMsg::ApplyPatchApprovalRequest { .. } => {
                        let result = CallToolResult {
                            content: vec![CallToolResultContent::TextContent(TextContent {
                                r#type: "text".to_string(),
                                text: "PATCH_APPROVAL_REQUIRED".to_string(),
                                annotations: None,
                            })],
                            is_error: None,
                        };
                        let _ = outgoing
                            .send(JSONRPCMessage::Response(JSONRPCResponse {
                                jsonrpc: JSONRPC_VERSION.into(),
                                id: id.clone(),
                                result: result.into(),
                            }))
                            .await;
                        break;
                    }
                    EventMsg::TaskComplete => {
                        let result = if let Some(msg) = last_agent_message {
                            CallToolResult {
                                content: vec![CallToolResultContent::TextContent(TextContent {
                                    r#type: "text".to_string(),
                                    text: msg,
                                    annotations: None,
                                })],
                                is_error: None,
                            }
                        } else {
                            CallToolResult {
                                content: vec![CallToolResultContent::TextContent(TextContent {
                                    r#type: "text".to_string(),
                                    text: String::new(),
                                    annotations: None,
                                })],
                                is_error: None,
                            }
                        };
                        let _ = outgoing
                            .send(JSONRPCMessage::Response(JSONRPCResponse {
                                jsonrpc: JSONRPC_VERSION.into(),
                                id: id.clone(),
                                result: result.into(),
                            }))
                            .await;
                        break;
                    }
                    EventMsg::SessionConfigured { .. } => {
                        tracing::error!("unexpected SessionConfigured event");
                    }
                    _ => {}
                }
            }
            Err(e) => {
                let result = CallToolResult {
                    content: vec![CallToolResultContent::TextContent(TextContent {
                        r#type: "text".to_string(),
                        text: format!("Codex runtime error: {e}"),
                        annotations: None,
                    })],
                    is_error: Some(true),
                };
                let _ = outgoing
                    .send(JSONRPCMessage::Response(JSONRPCResponse {
                        jsonrpc: JSONRPC_VERSION.into(),
                        id: id.clone(),
                        result: result.into(),
                    }))
                    .await;
                break;
            }
        }
    }
}
