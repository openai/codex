//! Asynchronous worker that executes a **Codex** tool-call inside a spawned
//! Tokio task. Separated from `message_processor.rs` to keep that file small
//! and to make future feature-growth easier to manage.

use std::sync::Arc;

use codex_core::Codex;
use codex_core::codex_wrapper::init_codex;
use codex_core::config::Config as CodexConfig;
use codex_core::protocol::AgentMessageEvent;
use codex_core::protocol::EventMsg;
use codex_core::protocol::ExecApprovalRequestEvent;
use codex_core::protocol::InputItem;
use codex_core::protocol::Op;
use codex_core::protocol::ReviewDecision;
use codex_core::protocol::Submission;
use codex_core::protocol::TaskCompleteEvent;
use mcp_types::CallToolResult;
use mcp_types::ContentBlock;
use mcp_types::ElicitRequest;
use mcp_types::ElicitRequestParamsRequestedSchema;
use mcp_types::ModelContextProtocolRequest;
use mcp_types::RequestId;
use mcp_types::TextContent;
use serde::Deserialize;
use serde_json::json;
use tracing::error;

use crate::outgoing_message::OutgoingMessageSender;

/// Run a complete Codex session and stream events back to the client.
///
/// On completion (success or error) the function sends the appropriate
/// `tools/call` response so the LLM can continue the conversation.
pub async fn run_codex_tool_session(
    id: RequestId,
    initial_prompt: String,
    config: CodexConfig,
    outgoing: Arc<OutgoingMessageSender>,
) {
    let (codex, first_event, _ctrl_c) = match init_codex(config).await {
        Ok(res) => res,
        Err(e) => {
            let result = CallToolResult {
                content: vec![ContentBlock::TextContent(TextContent {
                    r#type: "text".to_string(),
                    text: format!("Failed to start Codex session: {e}"),
                    annotations: None,
                })],
                is_error: Some(true),
                structured_content: None,
            };
            outgoing.send_response(id.clone(), result.into()).await;
            return;
        }
    };
    let codex = Arc::new(codex);

    // Send initial SessionConfigured event.
    outgoing.send_event_as_notification(&first_event).await;

    // Use the original MCP request ID as the `sub_id` for the Codex submission so that
    // any events emitted for this tool-call can be correlated with the
    // originating `tools/call` request.
    let sub_id = match &id {
        RequestId::String(s) => s.clone(),
        RequestId::Integer(n) => n.to_string(),
    };

    let submission = Submission {
        id: sub_id.clone(),
        op: Op::UserInput {
            items: vec![InputItem::Text {
                text: initial_prompt.clone(),
            }],
        },
    };

    if let Err(e) = codex.submit_with_id(submission).await {
        tracing::error!("Failed to submit initial prompt: {e}");
    }

    // Stream events until the task needs to pause for user interaction or
    // completes.
    loop {
        match codex.next_event().await {
            Ok(event) => {
                outgoing.send_event_as_notification(&event).await;

                match event.msg {
                    EventMsg::ExecApprovalRequest(ExecApprovalRequestEvent {
                        command,
                        cwd,
                        reason: _,
                    }) => {
                        let escaped_command = shlex::try_join(command.iter().map(|s| s.as_str()))
                            .unwrap_or_else(|_| command.join(" "));
                        let message = format!("Allow Codex to run `{escaped_command}` in {cwd:?}?");

                        let params = json!({
                            // These fields are required so that `params`
                            // conforms to ElicitRequestParams.
                            "message": message,
                            "requestedSchema": ElicitRequestParamsRequestedSchema {
                                r#type: "object".to_string(),
                                properties: json!({}),
                                required: None,
                            },

                            // These are additional fields the client can use to
                            // correlate the request with the codex tool call.
                            "codex_elicitation": "exec-approval",
                            "codex_mcp_tool_call_id": sub_id,
                            "codex_event_id": event.id,
                            "codex_command": command,
                            // Could convert it to base64 encoded bytes if we
                            // don't want to use to_string_lossy() here?
                            "codex_cwd": cwd.to_string_lossy().to_string()
                        });
                        let on_response = outgoing
                            .send_request(ElicitRequest::METHOD, Some(params))
                            .await;

                        // Listen for the response on a separate task so we do
                        // not block the main loop of this function.
                        {
                            let codex = codex.clone();
                            let event_id = event.id.clone();
                            tokio::spawn(async move {
                                on_exec_approval_response(event_id, on_response, codex).await;
                            });
                        }

                        // Continue, don't break so the session continues.
                        continue;
                    }
                    EventMsg::ApplyPatchApprovalRequest(_) => {
                        let result = CallToolResult {
                            content: vec![ContentBlock::TextContent(TextContent {
                                r#type: "text".to_string(),
                                text: "PATCH_APPROVAL_REQUIRED".to_string(),
                                annotations: None,
                            })],
                            is_error: None,
                            structured_content: None,
                        };
                        outgoing.send_response(id.clone(), result.into()).await;
                        // Continue, don't break so the session continues.
                        continue;
                    }
                    EventMsg::TaskComplete(TaskCompleteEvent { last_agent_message }) => {
                        let text = match last_agent_message {
                            Some(msg) => msg.clone(),
                            None => "".to_string(),
                        };
                        let result = CallToolResult {
                            content: vec![ContentBlock::TextContent(TextContent {
                                r#type: "text".to_string(),
                                text,
                                annotations: None,
                            })],
                            is_error: None,
                            structured_content: None,
                        };
                        outgoing.send_response(id.clone(), result.into()).await;
                        break;
                    }
                    EventMsg::SessionConfigured(_) => {
                        tracing::error!("unexpected SessionConfigured event");
                    }
                    EventMsg::AgentMessageDelta(_) => {
                        // TODO: think how we want to support this in the MCP
                    }
                    EventMsg::AgentReasoningDelta(_) => {
                        // TODO: think how we want to support this in the MCP
                    }
                    EventMsg::AgentMessage(AgentMessageEvent { .. }) => {
                        // TODO: think how we want to support this in the MCP
                    }
                    EventMsg::Error(_)
                    | EventMsg::TaskStarted
                    | EventMsg::TokenCount(_)
                    | EventMsg::AgentReasoning(_)
                    | EventMsg::McpToolCallBegin(_)
                    | EventMsg::McpToolCallEnd(_)
                    | EventMsg::ExecCommandBegin(_)
                    | EventMsg::ExecCommandEnd(_)
                    | EventMsg::BackgroundEvent(_)
                    | EventMsg::PatchApplyBegin(_)
                    | EventMsg::PatchApplyEnd(_)
                    | EventMsg::GetHistoryEntryResponse(_) => {
                        // For now, we do not do anything extra for these
                        // events. Note that
                        // send(codex_event_to_notification(&event)) above has
                        // already dispatched these events as notifications,
                        // though we may want to do give different treatment to
                        // individual events in the future.
                    }
                }
            }
            Err(e) => {
                let result = CallToolResult {
                    content: vec![ContentBlock::TextContent(TextContent {
                        r#type: "text".to_string(),
                        text: format!("Codex runtime error: {e}"),
                        annotations: None,
                    })],
                    is_error: Some(true),
                    // TODO(mbolin): Could present the error in a more
                    // structured way.
                    structured_content: None,
                };
                outgoing.send_response(id.clone(), result.into()).await;
                break;
            }
        }
    }
}

async fn on_exec_approval_response(
    event_id: String,
    receiver: tokio::sync::oneshot::Receiver<mcp_types::Result>,
    codex: Arc<Codex>,
) {
    let response = receiver.await;
    let value = match response {
        Ok(value) => value,
        Err(err) => {
            error!("request failed: {err:?}");
            return;
        }
    };

    // Try to deserialize `value` and then make the appropriate call to `codex`.
    let response = match serde_json::from_value::<ExecApprovalResponse>(value) {
        Ok(response) => response,
        Err(err) => {
            error!("failed to deserialize ExecApprovalResponse: {err}");
            return;
        }
    };

    if let Err(err) = codex
        .submit(Op::ExecApproval {
            id: event_id,
            decision: response.decision,
        })
        .await
    {
        error!("failed to submit ExecApproval: {err}");
    }
}

#[derive(Debug, Deserialize)]
pub struct ExecApprovalResponse {
    pub decision: ReviewDecision,
}
