//! Asynchronous worker that executes a **Codex** tool-call inside a spawned
//! Tokio task. Separated from `message_processor.rs` to keep that file small
//! and to make future feature-growth easier to manage.

use std::collections::HashMap;
use std::sync::Arc;

use codex_core::Codex;
use codex_core::codex_wrapper::CodexConversation;
use codex_core::codex_wrapper::init_codex;
use codex_core::config::Config as CodexConfig;
use codex_core::protocol::AgentMessageEvent;
use codex_core::protocol::ApplyPatchApprovalRequestEvent;
use codex_core::protocol::EventMsg;
use codex_core::protocol::ExecApprovalRequestEvent;
use codex_core::protocol::InputItem;
use codex_core::protocol::Op;
use codex_core::protocol::Submission;
use codex_core::protocol::TaskCompleteEvent;
use mcp_types::CallToolResult;
use mcp_types::ContentBlock;
use mcp_types::RequestId;
use mcp_types::TextContent;
use serde_json::json;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::exec_approval::handle_exec_approval_request;
use crate::outgoing_message::OutgoingMessageSender;
use crate::patch_approval::handle_patch_approval_request;

pub(crate) const INVALID_PARAMS_ERROR_CODE: i64 = -32602;

/// Run a complete Codex session and stream events back to the client.
///
/// On completion (success or error) the function sends the appropriate
/// `tools/call` response so the LLM can continue the conversation.
pub async fn run_codex_tool_session(
    id: RequestId,
    initial_prompt: String,
    config: CodexConfig,
    outgoing: Arc<OutgoingMessageSender>,
    session_map: Arc<Mutex<HashMap<Uuid, Arc<Codex>>>>,
    running_requests_id_to_codex_uuid: Arc<Mutex<HashMap<RequestId, Uuid>>>,
) {
    let CodexConversation {
        codex,
        session_configured,
        session_id,
        ..
    } = match init_codex(config).await {
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

    // update the session map so we can retrieve the session in a reply, and then drop it, since
    // we no longer need it for this function
    session_map.lock().await.insert(session_id, codex.clone());
    drop(session_map);

    // Send initial SessionConfigured event.
    outgoing
        .send_event_as_notification(&session_configured)
        .await;

    // Use the original MCP request ID as the `sub_id` for the Codex submission so that
    // any events emitted for this tool-call can be correlated with the
    // originating `tools/call` request.
    let sub_id = match &id {
        RequestId::String(s) => s.clone(),
        RequestId::Integer(n) => n.to_string(),
    };
    running_requests_id_to_codex_uuid
        .lock()
        .await
        .insert(id.clone(), session_id);
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
        // unregister the id so we don't keep it in the map
        running_requests_id_to_codex_uuid.lock().await.remove(&id);
        return;
    }

    run_codex_tool_session_inner(codex, outgoing, id, running_requests_id_to_codex_uuid).await;
}

pub async fn run_codex_tool_session_reply(
    codex: Arc<Codex>,
    outgoing: Arc<OutgoingMessageSender>,
    request_id: RequestId,
    prompt: String,
    running_requests_id_to_codex_uuid: Arc<Mutex<HashMap<RequestId, Uuid>>>,
    session_id: Uuid,
) {
    running_requests_id_to_codex_uuid
        .lock()
        .await
        .insert(request_id.clone(), session_id);
    if let Err(e) = codex
        .submit(Op::UserInput {
            items: vec![InputItem::Text { text: prompt }],
        })
        .await
    {
        tracing::error!("Failed to submit user input: {e}");
        // unregister the id so we don't keep it in the map
        running_requests_id_to_codex_uuid
            .lock()
            .await
            .remove(&request_id);
        return;
    }

    run_codex_tool_session_inner(
        codex,
        outgoing,
        request_id,
        running_requests_id_to_codex_uuid,
    )
    .await;
}

async fn run_codex_tool_session_inner(
    codex: Arc<Codex>,
    outgoing: Arc<OutgoingMessageSender>,
    request_id: RequestId,
    running_requests_id_to_codex_uuid: Arc<Mutex<HashMap<RequestId, Uuid>>>,
) {
    let request_id_str = match &request_id {
        RequestId::String(s) => s.clone(),
        RequestId::Integer(n) => n.to_string(),
    };

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
                        call_id,
                        reason: _,
                    }) => {
                        handle_exec_approval_request(
                            command,
                            cwd,
                            outgoing.clone(),
                            codex.clone(),
                            request_id.clone(),
                            request_id_str.clone(),
                            event.id.clone(),
                            call_id,
                        )
                        .await;
                        continue;
                    }
                    EventMsg::Error(err_event) => {
                        // Return a response to conclude the tool call when the Codex session reports an error (e.g., interruption).
                        let result = json!({
                            "error": err_event.message,
                        });
                        outgoing.send_response(request_id.clone(), result).await;
                        break;
                    }
                    EventMsg::ApplyPatchApprovalRequest(ApplyPatchApprovalRequestEvent {
                        call_id,
                        reason,
                        grant_root,
                        changes,
                    }) => {
                        handle_patch_approval_request(
                            call_id,
                            reason,
                            grant_root,
                            changes,
                            outgoing.clone(),
                            codex.clone(),
                            request_id.clone(),
                            request_id_str.clone(),
                            event.id.clone(),
                        )
                        .await;
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
                        outgoing
                            .send_response(request_id.clone(), result.into())
                            .await;
                        // unregister the id so we don't keep it in the map
                        running_requests_id_to_codex_uuid
                            .lock()
                            .await
                            .remove(&request_id);
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
                    EventMsg::TaskStarted
                    | EventMsg::TokenCount(_)
                    | EventMsg::AgentReasoning(_)
                    | EventMsg::McpToolCallBegin(_)
                    | EventMsg::McpToolCallEnd(_)
                    | EventMsg::ExecCommandBegin(_)
                    | EventMsg::ExecCommandEnd(_)
                    | EventMsg::BackgroundEvent(_)
                    | EventMsg::PatchApplyBegin(_)
                    | EventMsg::PatchApplyEnd(_)
                    | EventMsg::GetHistoryEntryResponse(_)
                    | EventMsg::ShutdownComplete => {
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
                outgoing
                    .send_response(request_id.clone(), result.into())
                    .await;
                break;
            }
        }
    }
}
