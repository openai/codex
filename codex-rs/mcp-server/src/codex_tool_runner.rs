//! Asynchronous worker that executes a **Codex** tool-call inside a spawned
//! Tokio task. Separated from `message_processor.rs` to keep that file small
//! and to make future feature-growth easier to manage.

use std::collections::HashMap;
use std::sync::Arc;

use crate::exec_approval::handle_exec_approval_request;
use crate::outgoing_message::OutgoingMessageSender;
use crate::outgoing_message::OutgoingNotificationMeta;
use crate::patch_approval::handle_patch_approval_request;
use codex_core::CodexThread;
use codex_core::NewThread;
use codex_core::ThreadManager;
use codex_core::config::Config as CodexConfig;
use codex_core::protocol::AgentMessageEvent;
use codex_core::protocol::ApplyPatchApprovalRequestEvent;
use codex_core::protocol::Event;
use codex_core::protocol::EventMsg;
use codex_core::protocol::ExecApprovalRequestEvent;
use codex_core::protocol::Op;
use codex_core::protocol::Submission;
use codex_core::protocol::TurnCompleteEvent;
use codex_protocol::ThreadId;
use codex_protocol::user_input::UserInput;
use mcp_types::CallToolResult;
use mcp_types::ContentBlock;
use mcp_types::RequestId;
use mcp_types::TextContent;
use serde_json::json;
use tokio::sync::Mutex;

pub(crate) const INVALID_PARAMS_ERROR_CODE: i64 = -32602;

/// To adhere to MCP `tools/call` response format, include the Codex
/// `threadId` in the `structured_content` field of the response.
/// Some MCP clients ignore `content` when `structuredContent` is present, so
/// mirror the text there as well.
pub(crate) fn create_call_tool_result_with_thread_id(
    thread_id: ThreadId,
    text: String,
    is_error: Option<bool>,
) -> CallToolResult {
    let content_text = text;
    let content = vec![ContentBlock::TextContent(TextContent {
        r#type: "text".to_string(),
        text: content_text.clone(),
        annotations: None,
    })];
    let structured_content = json!({
        "threadId": thread_id,
        "content": content_text,
    });
    CallToolResult {
        content,
        is_error,
        structured_content: Some(structured_content),
    }
}

/// Run a complete Codex session and stream events back to the client.
///
/// On completion (success or error) the function sends the appropriate
/// `tools/call` response so the LLM can continue the conversation.
pub async fn run_codex_tool_session(
    id: RequestId,
    initial_prompt: String,
    config: CodexConfig,
    outgoing: Arc<OutgoingMessageSender>,
    thread_manager: Arc<ThreadManager>,
    running_requests_id_to_codex_uuid: Arc<Mutex<HashMap<RequestId, ThreadId>>>,
) {
    let NewThread {
        thread_id,
        thread,
        session_configured,
    } = match thread_manager.start_thread(config).await {
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
            outgoing.send_response(id.clone(), result).await;
            return;
        }
    };

    let session_configured_event = Event {
        // Use a fake id value for now.
        id: "".to_string(),
        msg: EventMsg::SessionConfigured(session_configured.clone()),
    };
    outgoing
        .send_event_as_notification(
            &session_configured_event,
            Some(OutgoingNotificationMeta {
                request_id: Some(id.clone()),
                thread_id: Some(thread_id),
            }),
        )
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
        .insert(id.clone(), thread_id);
    let submission = Submission {
        id: sub_id.clone(),
        op: Op::UserInput {
            items: vec![UserInput::Text {
                text: initial_prompt.clone(),
                // MCP tool prompts are plain text with no UI element ranges.
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        },
    };

    if let Err(e) = thread.submit_with_id(submission).await {
        tracing::error!("Failed to submit initial prompt: {e}");
        let result = create_call_tool_result_with_thread_id(
            thread_id,
            format!("Failed to submit initial prompt: {e}"),
            Some(true),
        );
        outgoing.send_response(id.clone(), result).await;
        // unregister the id so we don't keep it in the map
        running_requests_id_to_codex_uuid.lock().await.remove(&id);
        return;
    }

    run_codex_tool_session_inner(
        thread_id,
        thread,
        outgoing,
        id,
        running_requests_id_to_codex_uuid,
    )
    .await;
}

pub async fn run_codex_tool_session_reply(
    thread_id: ThreadId,
    thread: Arc<CodexThread>,
    outgoing: Arc<OutgoingMessageSender>,
    request_id: RequestId,
    prompt: String,
    running_requests_id_to_codex_uuid: Arc<Mutex<HashMap<RequestId, ThreadId>>>,
) {
    running_requests_id_to_codex_uuid
        .lock()
        .await
        .insert(request_id.clone(), thread_id);
    if let Err(e) = thread
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: prompt,
                // MCP tool prompts are plain text with no UI element ranges.
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await
    {
        tracing::error!("Failed to submit user input: {e}");
        let result = create_call_tool_result_with_thread_id(
            thread_id,
            format!("Failed to submit user input: {e}"),
            Some(true),
        );
        outgoing.send_response(request_id.clone(), result).await;
        // unregister the id so we don't keep it in the map
        running_requests_id_to_codex_uuid
            .lock()
            .await
            .remove(&request_id);
        return;
    }

    run_codex_tool_session_inner(
        thread_id,
        thread,
        outgoing,
        request_id,
        running_requests_id_to_codex_uuid,
    )
    .await;
}

async fn run_codex_tool_session_inner(
    thread_id: ThreadId,
    thread: Arc<CodexThread>,
    outgoing: Arc<OutgoingMessageSender>,
    request_id: RequestId,
    running_requests_id_to_codex_uuid: Arc<Mutex<HashMap<RequestId, ThreadId>>>,
) {
    let request_id_str = match &request_id {
        RequestId::String(s) => s.clone(),
        RequestId::Integer(n) => n.to_string(),
    };

    // Stream events until the task needs to pause for user interaction or
    // completes.
    loop {
        match thread.next_event().await {
            Ok(event) => {
                outgoing
                    .send_event_as_notification(
                        &event,
                        Some(OutgoingNotificationMeta {
                            request_id: Some(request_id.clone()),
                            thread_id: Some(thread_id),
                        }),
                    )
                    .await;

                match event.msg {
                    EventMsg::ExecApprovalRequest(ExecApprovalRequestEvent {
                        turn_id: _,
                        command,
                        cwd,
                        call_id,
                        reason: _,
                        proposed_execpolicy_amendment: _,
                        parsed_cmd,
                    }) => {
                        handle_exec_approval_request(
                            command,
                            cwd,
                            outgoing.clone(),
                            thread.clone(),
                            request_id.clone(),
                            request_id_str.clone(),
                            event.id.clone(),
                            call_id,
                            parsed_cmd,
                            thread_id,
                        )
                        .await;
                        continue;
                    }
                    EventMsg::PlanDelta(_) => {
                        continue;
                    }
                    EventMsg::Error(err_event) => {
                        // Always respond in tools/call's expected shape, and include conversationId so the client can resume.
                        let result = create_call_tool_result_with_thread_id(
                            thread_id,
                            err_event.message,
                            Some(true),
                        );
                        outgoing.send_response(request_id.clone(), result).await;
                        break;
                    }
                    EventMsg::Warning(_) => {
                        continue;
                    }
                    EventMsg::ElicitationRequest(_) => {
                        // TODO: forward elicitation requests to the client?
                        continue;
                    }
                    EventMsg::ApplyPatchApprovalRequest(ApplyPatchApprovalRequestEvent {
                        call_id,
                        turn_id: _,
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
                            thread.clone(),
                            request_id.clone(),
                            request_id_str.clone(),
                            event.id.clone(),
                            thread_id,
                        )
                        .await;
                        continue;
                    }
                    EventMsg::TurnComplete(TurnCompleteEvent { last_agent_message }) => {
                        let text = match last_agent_message {
                            Some(msg) => msg,
                            None => "".to_string(),
                        };
                        let result = create_call_tool_result_with_thread_id(thread_id, text, None);
                        outgoing.send_response(request_id.clone(), result).await;
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
                    EventMsg::ThreadNameUpdated(_) => {
                        // Ignore session metadata updates in MCP tool runner.
                    }
                    EventMsg::AgentMessageDelta(_) => {
                        // TODO: think how we want to support this in the MCP
                    }
                    EventMsg::AgentReasoningDelta(_) => {
                        // TODO: think how we want to support this in the MCP
                    }
                    EventMsg::McpStartupUpdate(_) | EventMsg::McpStartupComplete(_) => {
                        // Ignored in MCP tool runner.
                    }
                    EventMsg::AgentMessage(AgentMessageEvent { .. }) => {
                        // TODO: think how we want to support this in the MCP
                    }
                    EventMsg::AgentReasoningRawContent(_)
                    | EventMsg::AgentReasoningRawContentDelta(_)
                    | EventMsg::TurnStarted(_)
                    | EventMsg::TokenCount(_)
                    | EventMsg::AgentReasoning(_)
                    | EventMsg::AgentReasoningSectionBreak(_)
                    | EventMsg::McpToolCallBegin(_)
                    | EventMsg::McpToolCallEnd(_)
                    | EventMsg::McpListToolsResponse(_)
                    | EventMsg::ListCustomPromptsResponse(_)
                    | EventMsg::ListSkillsResponse(_)
                    | EventMsg::ExecCommandBegin(_)
                    | EventMsg::TerminalInteraction(_)
                    | EventMsg::ExecCommandOutputDelta(_)
                    | EventMsg::ExecCommandEnd(_)
                    | EventMsg::BackgroundEvent(_)
                    | EventMsg::StreamError(_)
                    | EventMsg::PatchApplyBegin(_)
                    | EventMsg::PatchApplyEnd(_)
                    | EventMsg::TurnDiff(_)
                    | EventMsg::WebSearchBegin(_)
                    | EventMsg::WebSearchEnd(_)
                    | EventMsg::GetHistoryEntryResponse(_)
                    | EventMsg::PlanUpdate(_)
                    | EventMsg::TurnAborted(_)
                    | EventMsg::UserMessage(_)
                    | EventMsg::ShutdownComplete
                    | EventMsg::ViewImageToolCall(_)
                    | EventMsg::RawResponseItem(_)
                    | EventMsg::EnteredReviewMode(_)
                    | EventMsg::ItemStarted(_)
                    | EventMsg::ItemCompleted(_)
                    | EventMsg::AgentMessageContentDelta(_)
                    | EventMsg::ReasoningContentDelta(_)
                    | EventMsg::ReasoningRawContentDelta(_)
                    | EventMsg::SkillsUpdateAvailable
                    | EventMsg::UndoStarted(_)
                    | EventMsg::UndoCompleted(_)
                    | EventMsg::ExitedReviewMode(_)
                    | EventMsg::RequestUserInput(_)
                    | EventMsg::DynamicToolCallRequest(_)
                    | EventMsg::ContextCompacted(_)
                    | EventMsg::ThreadRolledBack(_)
                    | EventMsg::CollabAgentSpawnBegin(_)
                    | EventMsg::CollabAgentSpawnEnd(_)
                    | EventMsg::CollabAgentInteractionBegin(_)
                    | EventMsg::CollabAgentInteractionEnd(_)
                    | EventMsg::CollabWaitingBegin(_)
                    | EventMsg::CollabWaitingEnd(_)
                    | EventMsg::CollabCloseBegin(_)
                    | EventMsg::CollabCloseEnd(_)
                    | EventMsg::DeprecationNotice(_) => {
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
                let result = create_call_tool_result_with_thread_id(
                    thread_id,
                    format!("Codex runtime error: {e}"),
                    Some(true),
                );
                outgoing.send_response(request_id.clone(), result).await;
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn call_tool_result_includes_thread_id_in_structured_content() {
        let thread_id = ThreadId::new();
        let result = create_call_tool_result_with_thread_id(thread_id, "done".to_string(), None);
        assert_eq!(
            result.structured_content,
            Some(json!({
                "threadId": thread_id,
                "content": "done",
            }))
        );
    }
}
