use crate::codex_message_processor::ApiVersion;
use crate::codex_message_processor::PendingInterrupts;
use crate::outgoing_message::OutgoingMessageSender;
use codex_app_server_protocol::AccountRateLimitsUpdatedNotification;
use codex_app_server_protocol::AgentMessageDeltaNotification;
use codex_app_server_protocol::ApplyPatchApprovalParams;
use codex_app_server_protocol::ApplyPatchApprovalResponse;
use codex_app_server_protocol::ExecCommandApprovalParams;
use codex_app_server_protocol::ExecCommandApprovalResponse;
use codex_app_server_protocol::InterruptConversationResponse;
use codex_app_server_protocol::ItemCompletedNotification;
use codex_app_server_protocol::ItemStartedNotification;
use codex_app_server_protocol::ReasoningSummaryPartAddedNotification;
use codex_app_server_protocol::ReasoningSummaryTextDeltaNotification;
use codex_app_server_protocol::ReasoningTextDeltaNotification;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ServerRequestPayload;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::TurnInterruptResponse;
use codex_core::CodexConversation;
use codex_core::protocol::ApplyPatchApprovalRequestEvent;
use codex_core::protocol::Event;
use codex_core::protocol::EventMsg;
use codex_core::protocol::ExecApprovalRequestEvent;
use codex_core::protocol::Op;
use codex_core::protocol::ReviewDecision;
use codex_protocol::ConversationId;
use std::sync::Arc;
use tokio::sync::oneshot;
use tracing::error;

type JsonRpcResult = serde_json::Value;

pub(crate) async fn apply_bespoke_event_handling(
    event: Event,
    conversation_id: ConversationId,
    conversation: Arc<CodexConversation>,
    outgoing: Arc<OutgoingMessageSender>,
    pending_interrupts: PendingInterrupts,
) {
    let Event { id: event_id, msg } = event;
    match msg {
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
            // TODO(mbolin): Enforce a timeout so this task does not live indefinitely?
            tokio::spawn(async move {
                on_patch_approval_response(event_id, rx, conversation).await;
            });
        }
        EventMsg::McpToolCallBegin(begin_event) => {
            let item = ThreadItem::McpToolCall {
                id: begin_event.call_id,
                server: begin_event.invocation.server,
                tool: begin_event.invocation.tool,
                status: codex_app_server_protocol::McpToolCallStatus::InProgress,
                arguments: begin_event
                    .invocation
                    .arguments
                    .unwrap_or(serde_json::Value::Null),
                result: None,
                error: None,
            };
            let notification = ItemStartedNotification { item };
            outgoing
                .send_server_notification(ServerNotification::ItemStarted(notification))
                .await;
        }
        EventMsg::McpToolCallEnd(end_event) => {
            let status = if end_event.is_success() {
                codex_app_server_protocol::McpToolCallStatus::Completed
            } else {
                codex_app_server_protocol::McpToolCallStatus::Failed
            };

            let (result, error) = match &end_event.result {
                Ok(value) => (
                    Some(codex_app_server_protocol::McpToolCallResult {
                        content: value.content.clone(),
                        structured_content: value
                            .structured_content
                            .clone()
                            .unwrap_or(serde_json::Value::Null),
                    }),
                    None,
                ),
                Err(message) => (
                    None,
                    Some(codex_app_server_protocol::McpToolCallError {
                        message: message.clone(),
                    }),
                ),
            };

            let item = ThreadItem::McpToolCall {
                id: end_event.call_id,
                server: end_event.invocation.server,
                tool: end_event.invocation.tool,
                status,
                arguments: end_event
                    .invocation
                    .arguments
                    .clone()
                    .unwrap_or(serde_json::Value::Null),
                result,
                error,
            };
            let notification = ItemCompletedNotification { item };
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
        EventMsg::ExecApprovalRequest(ExecApprovalRequestEvent {
            call_id,
            command,
            cwd,
            reason,
            risk,
            parsed_cmd,
        }) => {
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

            // TODO(mbolin): Enforce a timeout so this task does not live indefinitely?
            tokio::spawn(async move {
                on_exec_approval_response(event_id, rx, conversation).await;
            });
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
        }

        _ => {}
    }
}

async fn on_patch_approval_response(
    event_id: String,
    receiver: oneshot::Receiver<JsonRpcResult>,
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
    receiver: oneshot::Receiver<JsonRpcResult>,
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
