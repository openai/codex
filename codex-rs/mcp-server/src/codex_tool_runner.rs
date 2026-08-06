//! Asynchronous worker that executes a **Codex** tool-call inside a spawned
//! Tokio task. Separated from `message_processor.rs` to keep that file small
//! and to make future feature-growth easier to manage.

use std::sync::Arc;

use crate::active_turn_registry::ActiveTurnRegistry;
use crate::exec_approval::handle_exec_approval_request;
use crate::outgoing_message::OutgoingMessageSender;
use crate::outgoing_message::OutgoingNotificationMeta;
use crate::patch_approval::handle_patch_approval_request;
use codex_core::CodexThread;
use codex_core::NewThread;
use codex_core::StartThreadOptions;
use codex_core::ThreadManager;
use codex_core::config::Config as CodexConfig;
use codex_protocol::ThreadId;
use codex_protocol::protocol::AgentMessageEvent;
use codex_protocol::protocol::ApplyPatchApprovalRequestEvent;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ExecApprovalRequestEvent;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::Submission;
use codex_protocol::protocol::TurnCompleteEvent;
use codex_protocol::user_input::UserInput;
use rmcp::model::CallToolResult;
use rmcp::model::ContentBlock;
use rmcp::model::RequestId;
use serde_json::json;
use uuid::Uuid;

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
    let content = vec![ContentBlock::text(content_text.clone())];
    let structured_content = json!({
        "threadId": thread_id,
        "content": content_text,
    });
    let mut result = CallToolResult::success(content);
    result.is_error = is_error;
    result.structured_content = Some(structured_content);
    result
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
    active_turns: Arc<ActiveTurnRegistry>,
) {
    let NewThread {
        thread_id,
        thread,
        session_configured,
    } = match thread_manager
        .start_thread(StartThreadOptions::new(config.clone()))
        .await
    {
        Ok(res) => res,
        Err(e) => {
            let result = CallToolResult::error(vec![ContentBlock::text(format!(
                "Failed to start Codex session: {e}"
            ))]);
            outgoing.send_response(id.clone(), result);
            return;
        }
    };

    let session_configured_event = Event {
        // Use a fake id value for now.
        id: "".to_string(),
        msg: EventMsg::SessionConfigured(session_configured.clone()),
    };
    outgoing.send_event_as_notification(
        &session_configured_event,
        Some(OutgoingNotificationMeta {
            request_id: Some(id.clone()),
            thread_id: Some(thread_id),
        }),
    );

    // Preserve the legacy event ID for initial `codex` calls. Each call starts
    // a new thread, so the thread and turn pair remains unique.
    let turn_id = id.to_string();
    active_turns.register(id.clone(), thread_id, turn_id.clone());
    let submission = Submission {
        id: turn_id,
        op: Op::UserInput {
            items: vec![UserInput::Text {
                text: initial_prompt.clone(),
                // MCP tool prompts are plain text with no UI element ranges.
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: Default::default(),
        },
        client_user_message_id: None,
        trace: None,
        parent_turn_id: None,
    };

    if let Err(e) = thread.submit_with_id(submission).await {
        tracing::error!("Failed to submit initial prompt: {e}");
        let result = create_call_tool_result_with_thread_id(
            thread_id,
            format!("Failed to submit initial prompt: {e}"),
            Some(true),
        );
        active_turns.finish(&id, || outgoing.send_response(id.clone(), result));
        return;
    }

    run_codex_tool_session_inner(thread_id, thread, outgoing, id, active_turns).await;
}

pub async fn run_codex_tool_session_reply(
    thread_id: ThreadId,
    thread: Arc<CodexThread>,
    outgoing: Arc<OutgoingMessageSender>,
    request_id: RequestId,
    prompt: String,
    active_turns: Arc<ActiveTurnRegistry>,
) {
    // Replies share a thread, so use Core's UUIDv7 submission ID convention
    // instead of a reusable MCP request ID.
    let turn_id = Uuid::now_v7().to_string();
    active_turns.register(request_id.clone(), thread_id, turn_id.clone());
    if let Err(e) = thread
        .submit_with_id(Submission {
            id: turn_id,
            op: Op::UserInput {
                items: vec![UserInput::Text {
                    text: prompt,
                    // MCP tool prompts are plain text with no UI element ranges.
                    text_elements: Vec::new(),
                }],
                final_output_json_schema: None,
                responsesapi_client_metadata: None,
                additional_context: Default::default(),
                thread_settings: Default::default(),
            },
            client_user_message_id: None,
            trace: None,
            parent_turn_id: None,
        })
        .await
    {
        tracing::error!("Failed to submit user input: {e}");
        let result = create_call_tool_result_with_thread_id(
            thread_id,
            format!("Failed to submit user input: {e}"),
            Some(true),
        );
        active_turns.finish(&request_id, || {
            outgoing.send_response(request_id.clone(), result);
        });
        return;
    }

    run_codex_tool_session_inner(thread_id, thread, outgoing, request_id, active_turns).await;
}

async fn run_codex_tool_session_inner(
    thread_id: ThreadId,
    thread: Arc<CodexThread>,
    outgoing: Arc<OutgoingMessageSender>,
    request_id: RequestId,
    active_turns: Arc<ActiveTurnRegistry>,
) {
    let request_id_str = request_id.to_string();

    // Stream events until the task needs to pause for user interaction or
    // completes.
    loop {
        match thread.next_event().await {
            Ok(event) => {
                outgoing.send_event_as_notification(
                    &event,
                    Some(OutgoingNotificationMeta {
                        request_id: Some(request_id.clone()),
                        thread_id: Some(thread_id),
                    }),
                );

                match event.msg {
                    EventMsg::ExecApprovalRequest(ev) => {
                        let approval_id = ev.effective_approval_id();
                        let ExecApprovalRequestEvent {
                            turn_id: _,
                            environment_id: _,
                            started_at_ms: _,
                            command,
                            cwd,
                            call_id,
                            plugin_id: _,
                            script_path: _,
                            approval_id: _,
                            reason: _,
                            proposed_execpolicy_amendment: _,
                            proposed_network_policy_amendments: _,
                            parsed_cmd,
                            network_approval_context: _,
                            additional_permissions: _,
                            available_decisions: _,
                        } = ev;
                        handle_exec_approval_request(
                            command,
                            cwd.to_path_buf(),
                            outgoing.clone(),
                            thread.clone(),
                            request_id.clone(),
                            request_id_str.clone(),
                            event.id.clone(),
                            call_id,
                            approval_id,
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
                        active_turns.finish(&request_id, || {
                            outgoing.send_response(request_id.clone(), result);
                        });
                        break;
                    }
                    EventMsg::Warning(_)
                    | EventMsg::GuardianWarning(_)
                    | EventMsg::ModelVerification(_)
                    | EventMsg::SafetyBuffering(_)
                    | EventMsg::TurnModerationMetadata(_) => {
                        continue;
                    }
                    EventMsg::GuardianAssessment(_) => {
                        continue;
                    }
                    EventMsg::ElicitationRequest(_) => {
                        // TODO: forward elicitation requests to the client?
                        continue;
                    }
                    EventMsg::ApplyPatchApprovalRequest(ApplyPatchApprovalRequestEvent {
                        call_id,
                        turn_id: _,
                        started_at_ms: _,
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
                    EventMsg::TurnComplete(TurnCompleteEvent {
                        last_agent_message, ..
                    }) => {
                        let text = match last_agent_message {
                            Some(msg) => msg,
                            None => "".to_string(),
                        };
                        let result = create_call_tool_result_with_thread_id(
                            thread_id, text, /*is_error*/ None,
                        );
                        active_turns.finish(&request_id, || {
                            outgoing.send_response(request_id.clone(), result);
                        });
                        break;
                    }
                    EventMsg::SessionConfigured(_) => {
                        tracing::error!("unexpected SessionConfigured event");
                    }
                    EventMsg::ThreadGoalUpdated(_) | EventMsg::ThreadQueueChanged(_) => {
                        // Ignore thread-scoped metadata updates in MCP tool runner.
                    }
                    EventMsg::McpStartupUpdate(_) | EventMsg::McpStartupComplete(_) => {
                        // Ignored in MCP tool runner.
                    }
                    EventMsg::AgentMessage(AgentMessageEvent { .. }) => {
                        // TODO: think how we want to support this in the MCP
                    }
                    EventMsg::AgentReasoningRawContent(_)
                    | EventMsg::TurnStarted(_)
                    | EventMsg::ThreadSettingsApplied(_)
                    | EventMsg::EnvironmentConnected(_)
                    | EventMsg::EnvironmentDisconnected(_)
                    | EventMsg::TokenCount(_)
                    | EventMsg::AgentReasoning(_)
                    | EventMsg::AgentReasoningSectionBreak(_)
                    | EventMsg::McpToolCallBegin(_)
                    | EventMsg::McpToolCallEnd(_)
                    | EventMsg::RealtimeConversationListVoicesResponse(_)
                    | EventMsg::ExecCommandBegin(_)
                    | EventMsg::TerminalInteraction(_)
                    | EventMsg::ExecCommandOutputDelta(_)
                    | EventMsg::ExecCommandEnd(_)
                    | EventMsg::StreamError(_)
                    | EventMsg::PatchApplyBegin(_)
                    | EventMsg::PatchApplyUpdated(_)
                    | EventMsg::PatchApplyEnd(_)
                    | EventMsg::TurnDiff(_)
                    | EventMsg::WebSearchBegin(_)
                    | EventMsg::WebSearchEnd(_)
                    | EventMsg::PlanUpdate(_)
                    | EventMsg::TurnAborted(_)
                    | EventMsg::UserMessage(_)
                    | EventMsg::ShutdownComplete
                    | EventMsg::ImageGenerationBegin(_)
                    | EventMsg::ImageGenerationEnd(_)
                    | EventMsg::ViewImageToolCall(_)
                    | EventMsg::RawResponseItem(_)
                    | EventMsg::RawResponseCompleted(_)
                    | EventMsg::EnteredReviewMode(_)
                    | EventMsg::ItemStarted(_)
                    | EventMsg::ItemCompleted(_)
                    | EventMsg::HookStarted(_)
                    | EventMsg::HookCompleted(_)
                    | EventMsg::AgentMessageContentDelta(_)
                    | EventMsg::ReasoningContentDelta(_)
                    | EventMsg::ReasoningRawContentDelta(_)
                    | EventMsg::ExitedReviewMode(_)
                    | EventMsg::RequestUserInput(_)
                    | EventMsg::RequestPermissions(_)
                    | EventMsg::DynamicToolCallRequest(_)
                    | EventMsg::DynamicToolCallResponse(_)
                    | EventMsg::ContextCompacted(_)
                    | EventMsg::ModelReroute(_)
                    | EventMsg::ThreadRolledBack(_)
                    | EventMsg::CollabAgentSpawnBegin(_)
                    | EventMsg::CollabAgentSpawnEnd(_)
                    | EventMsg::CollabAgentInteractionBegin(_)
                    | EventMsg::CollabAgentInteractionEnd(_)
                    | EventMsg::CollabWaitingBegin(_)
                    | EventMsg::CollabWaitingEnd(_)
                    | EventMsg::CollabCloseBegin(_)
                    | EventMsg::CollabCloseEnd(_)
                    | EventMsg::CollabResumeBegin(_)
                    | EventMsg::CollabResumeEnd(_)
                    | EventMsg::SubAgentActivity(_)
                    | EventMsg::RealtimeConversationStarted(_)
                    | EventMsg::RealtimeConversationSdp(_)
                    | EventMsg::RealtimeConversationRealtime(_)
                    | EventMsg::RealtimeConversationClosed(_)
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
                active_turns.finish(&request_id, || {
                    outgoing.send_response(request_id.clone(), result);
                });
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
        let result = create_call_tool_result_with_thread_id(
            thread_id,
            "done".to_string(),
            /*is_error*/ None,
        );
        assert_eq!(
            result.structured_content,
            Some(json!({
                "threadId": thread_id,
                "content": "done",
            }))
        );
    }
}
