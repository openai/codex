use crate::codex_message_processor::ApiVersion;
use crate::codex_message_processor::PendingInterrupts;
use crate::codex_message_processor::TurnSummary;
use crate::codex_message_processor::TurnSummaryStore;
use crate::outgoing_message::OutgoingMessageSender;
use codex_app_server_protocol::AccountRateLimitsUpdatedNotification;
use codex_app_server_protocol::AgentMessageDeltaNotification;
use codex_app_server_protocol::ApplyPatchApprovalParams;
use codex_app_server_protocol::ApplyPatchApprovalResponse;
use codex_app_server_protocol::ApprovalDecision;
use codex_app_server_protocol::CommandAction as V2ParsedCommand;
use codex_app_server_protocol::CommandExecutionOutputDeltaNotification;
use codex_app_server_protocol::CommandExecutionRequestApprovalParams;
use codex_app_server_protocol::CommandExecutionRequestApprovalResponse;
use codex_app_server_protocol::CommandExecutionStatus;
use codex_app_server_protocol::ExecCommandApprovalParams;
use codex_app_server_protocol::ExecCommandApprovalResponse;
use codex_app_server_protocol::InterruptConversationResponse;
use codex_app_server_protocol::ItemCompletedNotification;
use codex_app_server_protocol::ItemStartedNotification;
use codex_app_server_protocol::McpToolCallError;
use codex_app_server_protocol::McpToolCallResult;
use codex_app_server_protocol::McpToolCallStatus;
use codex_app_server_protocol::ReasoningSummaryPartAddedNotification;
use codex_app_server_protocol::ReasoningSummaryTextDeltaNotification;
use codex_app_server_protocol::ReasoningTextDeltaNotification;
use codex_app_server_protocol::SandboxCommandAssessment as V2SandboxCommandAssessment;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ServerRequestPayload;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::Turn;
use codex_app_server_protocol::TurnCompletedNotification;
use codex_app_server_protocol::TurnError;
use codex_app_server_protocol::TurnInterruptResponse;
use codex_app_server_protocol::TurnStatus;
use codex_core::CodexConversation;
use codex_core::parse_command::shlex_join;
use codex_core::protocol::ApplyPatchApprovalRequestEvent;
use codex_core::protocol::Event;
use codex_core::protocol::EventMsg;
use codex_core::protocol::ExecApprovalRequestEvent;
use codex_core::protocol::ExecCommandEndEvent;
use codex_core::protocol::McpToolCallBeginEvent;
use codex_core::protocol::McpToolCallEndEvent;
use codex_core::protocol::Op;
use codex_core::protocol::ReviewDecision;
use codex_core::review_format::format_review_findings_block;
use codex_protocol::ConversationId;
use codex_protocol::protocol::ReviewOutputEvent;
use std::convert::TryFrom;
use std::sync::Arc;
use tokio::sync::oneshot;
use tracing::error;

type JsonValue = serde_json::Value;

pub(crate) async fn apply_bespoke_event_handling(
    event: Event,
    conversation_id: ConversationId,
    conversation: Arc<CodexConversation>,
    outgoing: Arc<OutgoingMessageSender>,
    pending_interrupts: PendingInterrupts,
    turn_summary_store: TurnSummaryStore,
    api_version: ApiVersion,
) {
    let Event { id: event_id, msg } = event;
    match msg {
        EventMsg::TaskComplete(_ev) => {
            handle_turn_complete(conversation_id, event_id, &outgoing, &turn_summary_store).await;
        }
        EventMsg::ApplyPatchApprovalRequest(ApplyPatchApprovalRequestEvent {
            call_id,
            changes,
            reason,
            grant_root,
        }) => {
            let params = ApplyPatchApprovalParams {
                conversation_id,
                call_id,
                file_changes: changes,
                reason,
                grant_root,
            };
            let rx = outgoing
                .send_request(ServerRequestPayload::ApplyPatchApproval(params))
                .await;
            tokio::spawn(async move {
                on_patch_approval_response(event_id, rx, conversation).await;
            });
        }
        EventMsg::ExecApprovalRequest(ExecApprovalRequestEvent {
            call_id,
            turn_id,
            command,
            cwd,
            reason,
            risk,
            parsed_cmd,
        }) => match api_version {
            ApiVersion::V1 => {
                let params = ExecCommandApprovalParams {
                    conversation_id,
                    call_id,
                    command,
                    cwd,
                    reason,
                    risk,
                    parsed_cmd,
                };
                let rx = outgoing
                    .send_request(ServerRequestPayload::ExecCommandApproval(params))
                    .await;
                tokio::spawn(async move {
                    on_exec_approval_response(event_id, rx, conversation).await;
                });
            }
            ApiVersion::V2 => {
                let params = CommandExecutionRequestApprovalParams {
                    thread_id: conversation_id.to_string(),
                    turn_id: turn_id.clone(),
                    // Until we migrate the core to be aware of a first class CommandExecutionItem
                    // and emit the corresponding EventMsg, we repurpose the call_id as the item_id.
                    item_id: call_id.clone(),
                    reason,
                    risk: risk.map(V2SandboxCommandAssessment::from),
                };
                let rx = outgoing
                    .send_request(ServerRequestPayload::CommandExecutionRequestApproval(
                        params,
                    ))
                    .await;
                tokio::spawn(async move {
                    on_command_execution_request_approval_response(event_id, rx, conversation)
                        .await;
                });
            }
        },
        // TODO(celia): properly construct McpToolCall TurnItem in core.
        EventMsg::McpToolCallBegin(begin_event) => {
            let notification = construct_mcp_tool_call_notification(begin_event).await;
            outgoing
                .send_server_notification(ServerNotification::ItemStarted(notification))
                .await;
        }
        EventMsg::McpToolCallEnd(end_event) => {
            let notification = construct_mcp_tool_call_end_notification(end_event).await;
            outgoing
                .send_server_notification(ServerNotification::ItemCompleted(notification))
                .await;
        }
        EventMsg::AgentMessageContentDelta(event) => {
            let notification = AgentMessageDeltaNotification {
                item_id: event.item_id,
                delta: event.delta,
            };
            outgoing
                .send_server_notification(ServerNotification::AgentMessageDelta(notification))
                .await;
        }
        EventMsg::ReasoningContentDelta(event) => {
            let notification = ReasoningSummaryTextDeltaNotification {
                item_id: event.item_id,
                delta: event.delta,
                summary_index: event.summary_index,
            };
            outgoing
                .send_server_notification(ServerNotification::ReasoningSummaryTextDelta(
                    notification,
                ))
                .await;
        }
        EventMsg::ReasoningRawContentDelta(event) => {
            let notification = ReasoningTextDeltaNotification {
                item_id: event.item_id,
                delta: event.delta,
                content_index: event.content_index,
            };
            outgoing
                .send_server_notification(ServerNotification::ReasoningTextDelta(notification))
                .await;
        }
        EventMsg::AgentReasoningSectionBreak(event) => {
            let notification = ReasoningSummaryPartAddedNotification {
                item_id: event.item_id,
                summary_index: event.summary_index,
            };
            outgoing
                .send_server_notification(ServerNotification::ReasoningSummaryPartAdded(
                    notification,
                ))
                .await;
        }
        EventMsg::TokenCount(token_count_event) => {
            if let Some(rate_limits) = token_count_event.rate_limits {
                outgoing
                    .send_server_notification(ServerNotification::AccountRateLimitsUpdated(
                        AccountRateLimitsUpdatedNotification {
                            rate_limits: rate_limits.into(),
                        },
                    ))
                    .await;
            }
        }
        EventMsg::Error(ev) => {
            handle_error(conversation_id, ev.message, &turn_summary_store).await;
        }
        EventMsg::EnteredReviewMode(review_request) => {
            let notification = ItemStartedNotification {
                item: ThreadItem::CodeReview {
                    id: event_id.clone(),
                    review: review_request.user_facing_hint,
                },
            };
            outgoing
                .send_server_notification(ServerNotification::ItemStarted(notification))
                .await;
        }
        EventMsg::ItemStarted(item_started_event) => {
            let item: ThreadItem = item_started_event.item.clone().into();
            let notification = ItemStartedNotification { item };
            outgoing
                .send_server_notification(ServerNotification::ItemStarted(notification))
                .await;
        }
        EventMsg::ItemCompleted(item_completed_event) => {
            let item: ThreadItem = item_completed_event.item.clone().into();
            let notification = ItemCompletedNotification { item };
            outgoing
                .send_server_notification(ServerNotification::ItemCompleted(notification))
                .await;
        }
        EventMsg::ExitedReviewMode(review_event) => {
            let review_text = match review_event.review_output {
                Some(output) => render_review_output_text(&output),
                None => REVIEW_FALLBACK_MESSAGE.to_string(),
            };
            let notification = ItemCompletedNotification {
                item: ThreadItem::CodeReview {
                    id: event_id,
                    review: review_text,
                },
            };
            outgoing
                .send_server_notification(ServerNotification::ItemCompleted(notification))
                .await;
        }
        EventMsg::ExecCommandBegin(exec_command_begin_event) => {
            let item = ThreadItem::CommandExecution {
                id: exec_command_begin_event.call_id.clone(),
                command: shlex_join(&exec_command_begin_event.command),
                cwd: exec_command_begin_event.cwd,
                status: CommandExecutionStatus::InProgress,
                command_actions: exec_command_begin_event
                    .parsed_cmd
                    .into_iter()
                    .map(V2ParsedCommand::from)
                    .collect(),
                aggregated_output: None,
                exit_code: None,
                duration_ms: None,
            };
            let notification = ItemStartedNotification { item };
            outgoing
                .send_server_notification(ServerNotification::ItemStarted(notification))
                .await;
        }
        EventMsg::ExecCommandOutputDelta(exec_command_output_delta_event) => {
            let notification = CommandExecutionOutputDeltaNotification {
                item_id: exec_command_output_delta_event.call_id.clone(),
                delta: String::from_utf8_lossy(&exec_command_output_delta_event.chunk).to_string(),
            };
            outgoing
                .send_server_notification(ServerNotification::CommandExecutionOutputDelta(
                    notification,
                ))
                .await;
        }
        EventMsg::ExecCommandEnd(exec_command_end_event) => {
            let ExecCommandEndEvent {
                call_id,
                command,
                cwd,
                parsed_cmd,
                aggregated_output,
                exit_code,
                duration,
                ..
            } = exec_command_end_event;

            let status = if exit_code == 0 {
                CommandExecutionStatus::Completed
            } else {
                CommandExecutionStatus::Failed
            };

            let aggregated_output = if aggregated_output.is_empty() {
                None
            } else {
                Some(aggregated_output)
            };

            let duration_ms = i64::try_from(duration.as_millis()).unwrap_or(i64::MAX);

            let item = ThreadItem::CommandExecution {
                id: call_id,
                command: shlex_join(&command),
                cwd,
                status,
                command_actions: parsed_cmd.into_iter().map(V2ParsedCommand::from).collect(),
                aggregated_output,
                exit_code: Some(exit_code),
                duration_ms: Some(duration_ms),
            };

            let notification = ItemCompletedNotification { item };
            outgoing
                .send_server_notification(ServerNotification::ItemCompleted(notification))
                .await;
        }
        // If this is a TurnAborted, reply to any pending interrupt requests.
        EventMsg::TurnAborted(turn_aborted_event) => {
            let pending = {
                let mut map = pending_interrupts.lock().await;
                map.remove(&conversation_id).unwrap_or_default()
            };
            if !pending.is_empty() {
                for (rid, ver) in pending {
                    match ver {
                        ApiVersion::V1 => {
                            let response = InterruptConversationResponse {
                                abort_reason: turn_aborted_event.reason.clone(),
                            };
                            outgoing.send_response(rid, response).await;
                        }
                        ApiVersion::V2 => {
                            let response = TurnInterruptResponse {};
                            outgoing.send_response(rid, response).await;
                        }
                    }
                }
            }

            handle_turn_interrupted(conversation_id, event_id, &outgoing, &turn_summary_store)
                .await;
        }

        _ => {}
    }
}

async fn emit_turn_completed_with_status(
    event_id: String,
    status: TurnStatus,
    outgoing: &OutgoingMessageSender,
) {
    let notification = TurnCompletedNotification {
        turn: Turn {
            id: event_id,
            items: vec![],
            status,
        },
    };
    outgoing
        .send_server_notification(ServerNotification::TurnCompleted(notification))
        .await;
}

async fn find_and_remove_turn_summary(
    conversation_id: ConversationId,
    turn_summary_store: &TurnSummaryStore,
) -> TurnSummary {
    let mut map = turn_summary_store.lock().await;
    map.remove(&conversation_id).unwrap_or_default()
}

async fn handle_turn_complete(
    conversation_id: ConversationId,
    event_id: String,
    outgoing: &OutgoingMessageSender,
    turn_summary_store: &TurnSummaryStore,
) {
    let turn_summary = find_and_remove_turn_summary(conversation_id, turn_summary_store).await;

    let status = if let Some(message) = turn_summary.last_error_message {
        TurnStatus::Failed {
            error: TurnError { message },
        }
    } else {
        TurnStatus::Completed
    };

    emit_turn_completed_with_status(event_id, status, outgoing).await;
}

async fn handle_turn_interrupted(
    conversation_id: ConversationId,
    event_id: String,
    outgoing: &OutgoingMessageSender,
    turn_summary_store: &TurnSummaryStore,
) {
    find_and_remove_turn_summary(conversation_id, turn_summary_store).await;

    emit_turn_completed_with_status(event_id, TurnStatus::Interrupted, outgoing).await;
}

async fn handle_error(
    conversation_id: ConversationId,
    message: String,
    turn_summary_store: &TurnSummaryStore,
) {
    let mut map = turn_summary_store.lock().await;
    map.entry(conversation_id).or_default().last_error_message = Some(message);
}

async fn on_patch_approval_response(
    event_id: String,
    receiver: oneshot::Receiver<JsonValue>,
    codex: Arc<CodexConversation>,
) {
    let response = receiver.await;
    let value = match response {
        Ok(value) => value,
        Err(err) => {
            error!("request failed: {err:?}");
            if let Err(submit_err) = codex
                .submit(Op::PatchApproval {
                    id: event_id.clone(),
                    decision: ReviewDecision::Denied,
                })
                .await
            {
                error!("failed to submit denied PatchApproval after request failure: {submit_err}");
            }
            return;
        }
    };

    let response =
        serde_json::from_value::<ApplyPatchApprovalResponse>(value).unwrap_or_else(|err| {
            error!("failed to deserialize ApplyPatchApprovalResponse: {err}");
            ApplyPatchApprovalResponse {
                decision: ReviewDecision::Denied,
            }
        });

    if let Err(err) = codex
        .submit(Op::PatchApproval {
            id: event_id,
            decision: response.decision,
        })
        .await
    {
        error!("failed to submit PatchApproval: {err}");
    }
}

async fn on_exec_approval_response(
    event_id: String,
    receiver: oneshot::Receiver<JsonValue>,
    conversation: Arc<CodexConversation>,
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
    let response =
        serde_json::from_value::<ExecCommandApprovalResponse>(value).unwrap_or_else(|err| {
            error!("failed to deserialize ExecCommandApprovalResponse: {err}");
            // If we cannot deserialize the response, we deny the request to be
            // conservative.
            ExecCommandApprovalResponse {
                decision: ReviewDecision::Denied,
            }
        });

    if let Err(err) = conversation
        .submit(Op::ExecApproval {
            id: event_id,
            decision: response.decision,
        })
        .await
    {
        error!("failed to submit ExecApproval: {err}");
    }
}

const REVIEW_FALLBACK_MESSAGE: &str = "Reviewer failed to output a response.";

fn render_review_output_text(output: &ReviewOutputEvent) -> String {
    let mut sections = Vec::new();
    let explanation = output.overall_explanation.trim();
    if !explanation.is_empty() {
        sections.push(explanation.to_string());
    }
    if !output.findings.is_empty() {
        let findings = format_review_findings_block(&output.findings, None);
        let trimmed = findings.trim();
        if !trimmed.is_empty() {
            sections.push(trimmed.to_string());
        }
    }
    if sections.is_empty() {
        REVIEW_FALLBACK_MESSAGE.to_string()
    } else {
        sections.join("\n\n")
    }
}

async fn on_command_execution_request_approval_response(
    event_id: String,
    receiver: oneshot::Receiver<JsonValue>,
    conversation: Arc<CodexConversation>,
) {
    let response = receiver.await;
    let value = match response {
        Ok(value) => value,
        Err(err) => {
            error!("request failed: {err:?}");
            return;
        }
    };

    let response = serde_json::from_value::<CommandExecutionRequestApprovalResponse>(value)
        .unwrap_or_else(|err| {
            error!("failed to deserialize CommandExecutionRequestApprovalResponse: {err}");
            CommandExecutionRequestApprovalResponse {
                decision: ApprovalDecision::Decline,
                accept_settings: None,
            }
        });

    let CommandExecutionRequestApprovalResponse {
        decision,
        accept_settings,
    } = response;

    let decision = match (decision, accept_settings) {
        (ApprovalDecision::Accept, Some(settings)) if settings.for_session => {
            ReviewDecision::ApprovedForSession
        }
        (ApprovalDecision::Accept, _) => ReviewDecision::Approved,
        (ApprovalDecision::Decline, _) => ReviewDecision::Denied,
        (ApprovalDecision::Cancel, _) => ReviewDecision::Abort,
    };
    if let Err(err) = conversation
        .submit(Op::ExecApproval {
            id: event_id,
            decision,
        })
        .await
    {
        error!("failed to submit ExecApproval: {err}");
    }
}

/// similar to handle_mcp_tool_call_begin in exec
async fn construct_mcp_tool_call_notification(
    begin_event: McpToolCallBeginEvent,
) -> ItemStartedNotification {
    let item = ThreadItem::McpToolCall {
        id: begin_event.call_id,
        server: begin_event.invocation.server,
        tool: begin_event.invocation.tool,
        status: McpToolCallStatus::InProgress,
        arguments: begin_event.invocation.arguments.unwrap_or(JsonValue::Null),
        result: None,
        error: None,
    };
    ItemStartedNotification { item }
}

/// simiilar to handle_mcp_tool_call_end in exec
async fn construct_mcp_tool_call_end_notification(
    end_event: McpToolCallEndEvent,
) -> ItemCompletedNotification {
    let status = if end_event.is_success() {
        McpToolCallStatus::Completed
    } else {
        McpToolCallStatus::Failed
    };

    let (result, error) = match &end_event.result {
        Ok(value) => (
            Some(McpToolCallResult {
                content: value.content.clone(),
                structured_content: value.structured_content.clone(),
            }),
            None,
        ),
        Err(message) => (
            None,
            Some(McpToolCallError {
                message: message.clone(),
            }),
        ),
    };

    let item = ThreadItem::McpToolCall {
        id: end_event.call_id,
        server: end_event.invocation.server,
        tool: end_event.invocation.tool,
        status,
        arguments: end_event.invocation.arguments.unwrap_or(JsonValue::Null),
        result,
        error,
    };
    ItemCompletedNotification { item }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CHANNEL_CAPACITY;
    use crate::outgoing_message::OutgoingMessage;
    use crate::outgoing_message::OutgoingMessageSender;
    use anyhow::Result;
    use anyhow::anyhow;
    use anyhow::bail;
    use codex_core::protocol::McpInvocation;
    use mcp_types::CallToolResult;
    use mcp_types::ContentBlock;
    use mcp_types::TextContent;
    use pretty_assertions::assert_eq;
    use serde_json::Value as JsonValue;
    use std::collections::HashMap;
    use std::time::Duration;
    use tokio::sync::Mutex;
    use tokio::sync::mpsc;

    fn new_turn_summary_store() -> TurnSummaryStore {
        Arc::new(Mutex::new(HashMap::new()))
    }

    #[tokio::test]
    async fn test_handle_error_records_message() -> Result<()> {
        let conversation_id = ConversationId::new();
        let turn_summary_store = new_turn_summary_store();

        handle_error(conversation_id, "boom".to_string(), &turn_summary_store).await;

        let turn_summary = find_and_remove_turn_summary(conversation_id, &turn_summary_store).await;
        assert_eq!(turn_summary.last_error_message, Some("boom".to_string()));
        Ok(())
    }

    #[tokio::test]
    async fn test_handle_turn_complete_emits_completed_without_error() -> Result<()> {
        let conversation_id = ConversationId::new();
        let event_id = "complete1".to_string();
        let (tx, mut rx) = mpsc::channel(CHANNEL_CAPACITY);
        let outgoing = Arc::new(OutgoingMessageSender::new(tx));
        let turn_summary_store = new_turn_summary_store();

        handle_turn_complete(
            conversation_id,
            event_id.clone(),
            &outgoing,
            &turn_summary_store,
        )
        .await;

        let msg = rx
            .recv()
            .await
            .ok_or_else(|| anyhow!("should send one notification"))?;
        match msg {
            OutgoingMessage::AppServerNotification(ServerNotification::TurnCompleted(n)) => {
                assert_eq!(n.turn.id, event_id);
                assert_eq!(n.turn.status, TurnStatus::Completed);
            }
            other => bail!("unexpected message: {other:?}"),
        }
        assert!(rx.try_recv().is_err(), "no extra messages expected");
        Ok(())
    }

    #[tokio::test]
    async fn test_handle_turn_interrupted_emits_interrupted_with_error() -> Result<()> {
        let conversation_id = ConversationId::new();
        let event_id = "interrupt1".to_string();
        let turn_summary_store = new_turn_summary_store();
        handle_error(conversation_id, "oops".to_string(), &turn_summary_store).await;
        let (tx, mut rx) = mpsc::channel(CHANNEL_CAPACITY);
        let outgoing = Arc::new(OutgoingMessageSender::new(tx));

        handle_turn_interrupted(
            conversation_id,
            event_id.clone(),
            &outgoing,
            &turn_summary_store,
        )
        .await;

        let msg = rx
            .recv()
            .await
            .ok_or_else(|| anyhow!("should send one notification"))?;
        match msg {
            OutgoingMessage::AppServerNotification(ServerNotification::TurnCompleted(n)) => {
                assert_eq!(n.turn.id, event_id);
                assert_eq!(n.turn.status, TurnStatus::Interrupted);
            }
            other => bail!("unexpected message: {other:?}"),
        }
        assert!(rx.try_recv().is_err(), "no extra messages expected");
        Ok(())
    }

    #[tokio::test]
    async fn test_handle_turn_complete_emits_failed_with_error() -> Result<()> {
        let conversation_id = ConversationId::new();
        let event_id = "complete_err1".to_string();
        let turn_summary_store = new_turn_summary_store();
        handle_error(conversation_id, "bad".to_string(), &turn_summary_store).await;
        let (tx, mut rx) = mpsc::channel(CHANNEL_CAPACITY);
        let outgoing = Arc::new(OutgoingMessageSender::new(tx));

        handle_turn_complete(
            conversation_id,
            event_id.clone(),
            &outgoing,
            &turn_summary_store,
        )
        .await;

        let msg = rx
            .recv()
            .await
            .ok_or_else(|| anyhow!("should send one notification"))?;
        match msg {
            OutgoingMessage::AppServerNotification(ServerNotification::TurnCompleted(n)) => {
                assert_eq!(n.turn.id, event_id);
                assert_eq!(
                    n.turn.status,
                    TurnStatus::Failed {
                        error: TurnError {
                            message: "bad".to_string(),
                        }
                    }
                );
            }
            other => bail!("unexpected message: {other:?}"),
        }
        assert!(rx.try_recv().is_err(), "no extra messages expected");
        Ok(())
    }

    #[tokio::test]
    async fn test_construct_mcp_tool_call_begin_notification_with_args() {
        let begin_event = McpToolCallBeginEvent {
            call_id: "call_123".to_string(),
            invocation: McpInvocation {
                server: "codex".to_string(),
                tool: "list_mcp_resources".to_string(),
                arguments: Some(serde_json::json!({"server": ""})),
            },
        };

        let notification = construct_mcp_tool_call_notification(begin_event.clone()).await;

        let expected = ItemStartedNotification {
            item: ThreadItem::McpToolCall {
                id: begin_event.call_id,
                server: begin_event.invocation.server,
                tool: begin_event.invocation.tool,
                status: McpToolCallStatus::InProgress,
                arguments: serde_json::json!({"server": ""}),
                result: None,
                error: None,
            },
        };

        assert_eq!(notification, expected);
    }

    #[tokio::test]
    async fn test_handle_turn_complete_emits_error_multiple_turns() -> Result<()> {
        // Conversation A will have two turns; Conversation B will have one turn.
        let conversation_a = ConversationId::new();
        let conversation_b = ConversationId::new();
        let turn_summary_store = new_turn_summary_store();

        let (tx, mut rx) = mpsc::channel(CHANNEL_CAPACITY);
        let outgoing = Arc::new(OutgoingMessageSender::new(tx));

        // Turn 1 on conversation A
        let a_turn1 = "a_turn1".to_string();
        handle_error(conversation_a, "a1".to_string(), &turn_summary_store).await;
        handle_turn_complete(
            conversation_a,
            a_turn1.clone(),
            &outgoing,
            &turn_summary_store,
        )
        .await;

        // Turn 1 on conversation B
        let b_turn1 = "b_turn1".to_string();
        handle_error(conversation_b, "b1".to_string(), &turn_summary_store).await;
        handle_turn_complete(
            conversation_b,
            b_turn1.clone(),
            &outgoing,
            &turn_summary_store,
        )
        .await;

        // Turn 2 on conversation A
        let a_turn2 = "a_turn2".to_string();
        handle_turn_complete(
            conversation_a,
            a_turn2.clone(),
            &outgoing,
            &turn_summary_store,
        )
        .await;

        // Verify: A turn 1
        let msg = rx
            .recv()
            .await
            .ok_or_else(|| anyhow!("should send first notification"))?;
        match msg {
            OutgoingMessage::AppServerNotification(ServerNotification::TurnCompleted(n)) => {
                assert_eq!(n.turn.id, a_turn1);
                assert_eq!(
                    n.turn.status,
                    TurnStatus::Failed {
                        error: TurnError {
                            message: "a1".to_string(),
                        }
                    }
                );
            }
            other => bail!("unexpected message: {other:?}"),
        }

        // Verify: B turn 1
        let msg = rx
            .recv()
            .await
            .ok_or_else(|| anyhow!("should send second notification"))?;
        match msg {
            OutgoingMessage::AppServerNotification(ServerNotification::TurnCompleted(n)) => {
                assert_eq!(n.turn.id, b_turn1);
                assert_eq!(
                    n.turn.status,
                    TurnStatus::Failed {
                        error: TurnError {
                            message: "b1".to_string(),
                        }
                    }
                );
            }
            other => bail!("unexpected message: {other:?}"),
        }

        // Verify: A turn 2
        let msg = rx
            .recv()
            .await
            .ok_or_else(|| anyhow!("should send third notification"))?;
        match msg {
            OutgoingMessage::AppServerNotification(ServerNotification::TurnCompleted(n)) => {
                assert_eq!(n.turn.id, a_turn2);
                assert_eq!(n.turn.status, TurnStatus::Completed);
            }
            other => bail!("unexpected message: {other:?}"),
        }

        assert!(rx.try_recv().is_err(), "no extra messages expected");
        Ok(())
    }

    #[tokio::test]
    async fn test_construct_mcp_tool_call_begin_notification_without_args() {
        let begin_event = McpToolCallBeginEvent {
            call_id: "call_456".to_string(),
            invocation: McpInvocation {
                server: "codex".to_string(),
                tool: "list_mcp_resources".to_string(),
                arguments: None,
            },
        };

        let notification = construct_mcp_tool_call_notification(begin_event.clone()).await;

        let expected = ItemStartedNotification {
            item: ThreadItem::McpToolCall {
                id: begin_event.call_id,
                server: begin_event.invocation.server,
                tool: begin_event.invocation.tool,
                status: McpToolCallStatus::InProgress,
                arguments: JsonValue::Null,
                result: None,
                error: None,
            },
        };

        assert_eq!(notification, expected);
    }

    #[tokio::test]
    async fn test_construct_mcp_tool_call_end_notification_success() {
        let content = vec![ContentBlock::TextContent(TextContent {
            annotations: None,
            text: "{\"resources\":[]}".to_string(),
            r#type: "text".to_string(),
        })];
        let result = CallToolResult {
            content: content.clone(),
            is_error: Some(false),
            structured_content: None,
        };

        let end_event = McpToolCallEndEvent {
            call_id: "call_789".to_string(),
            invocation: McpInvocation {
                server: "codex".to_string(),
                tool: "list_mcp_resources".to_string(),
                arguments: Some(serde_json::json!({"server": ""})),
            },
            duration: Duration::from_nanos(92708),
            result: Ok(result),
        };

        let notification = construct_mcp_tool_call_end_notification(end_event.clone()).await;

        let expected = ItemCompletedNotification {
            item: ThreadItem::McpToolCall {
                id: end_event.call_id,
                server: end_event.invocation.server,
                tool: end_event.invocation.tool,
                status: McpToolCallStatus::Completed,
                arguments: serde_json::json!({"server": ""}),
                result: Some(McpToolCallResult {
                    content,
                    structured_content: None,
                }),
                error: None,
            },
        };

        assert_eq!(notification, expected);
    }

    #[tokio::test]
    async fn test_construct_mcp_tool_call_end_notification_error() {
        let end_event = McpToolCallEndEvent {
            call_id: "call_err".to_string(),
            invocation: McpInvocation {
                server: "codex".to_string(),
                tool: "list_mcp_resources".to_string(),
                arguments: None,
            },
            duration: Duration::from_millis(1),
            result: Err("boom".to_string()),
        };

        let notification = construct_mcp_tool_call_end_notification(end_event.clone()).await;

        let expected = ItemCompletedNotification {
            item: ThreadItem::McpToolCall {
                id: end_event.call_id,
                server: end_event.invocation.server,
                tool: end_event.invocation.tool,
                status: McpToolCallStatus::Failed,
                arguments: JsonValue::Null,
                result: None,
                error: Some(McpToolCallError {
                    message: "boom".to_string(),
                }),
            },
        };

        assert_eq!(notification, expected);
    }
}
