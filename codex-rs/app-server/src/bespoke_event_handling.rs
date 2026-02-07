use crate::codex_message_processor::ApiVersion;
use crate::codex_message_processor::PendingInterrupts;
use crate::codex_message_processor::PendingRollbacks;
use crate::codex_message_processor::TurnSummary;
use crate::codex_message_processor::TurnSummaryStore;
use crate::codex_message_processor::read_event_msgs_from_rollout;
use crate::codex_message_processor::read_summary_from_rollout;
use crate::codex_message_processor::summary_to_thread;
use crate::error_code::INTERNAL_ERROR_CODE;
use crate::error_code::INVALID_REQUEST_ERROR_CODE;
use crate::outgoing_message::OutgoingMessageSender;
use codex_app_server_protocol::AccountRateLimitsUpdatedNotification;
use codex_app_server_protocol::AgentMessageDeltaNotification;
use codex_app_server_protocol::ApplyPatchApprovalParams;
use codex_app_server_protocol::ApplyPatchApprovalResponse;
use codex_app_server_protocol::CodexErrorInfo as V2CodexErrorInfo;
use codex_app_server_protocol::CollabAgentState as V2CollabAgentStatus;
use codex_app_server_protocol::CollabAgentTool;
use codex_app_server_protocol::CollabAgentToolCallStatus as V2CollabToolCallStatus;
use codex_app_server_protocol::CommandAction as V2ParsedCommand;
use codex_app_server_protocol::CommandExecutionApprovalDecision;
use codex_app_server_protocol::CommandExecutionOutputDeltaNotification;
use codex_app_server_protocol::CommandExecutionRequestApprovalParams;
use codex_app_server_protocol::CommandExecutionRequestApprovalResponse;
use codex_app_server_protocol::CommandExecutionStatus;
use codex_app_server_protocol::ContextCompactedNotification;
use codex_app_server_protocol::DeprecationNoticeNotification;
use codex_app_server_protocol::DynamicToolCallParams;
use codex_app_server_protocol::ErrorNotification;
use codex_app_server_protocol::ExecCommandApprovalParams;
use codex_app_server_protocol::ExecCommandApprovalResponse;
use codex_app_server_protocol::ExecPolicyAmendment as V2ExecPolicyAmendment;
use codex_app_server_protocol::FileChangeApprovalDecision;
use codex_app_server_protocol::FileChangeOutputDeltaNotification;
use codex_app_server_protocol::FileChangeRequestApprovalParams;
use codex_app_server_protocol::FileChangeRequestApprovalResponse;
use codex_app_server_protocol::FileUpdateChange;
use codex_app_server_protocol::InterruptConversationResponse;
use codex_app_server_protocol::ItemCompletedNotification;
use codex_app_server_protocol::ItemStartedNotification;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::McpToolCallError;
use codex_app_server_protocol::McpToolCallResult;
use codex_app_server_protocol::McpToolCallStatus;
use codex_app_server_protocol::PatchApplyStatus;
use codex_app_server_protocol::PatchChangeKind as V2PatchChangeKind;
use codex_app_server_protocol::PlanDeltaNotification;
use codex_app_server_protocol::RawResponseItemCompletedNotification;
use codex_app_server_protocol::ReasoningSummaryPartAddedNotification;
use codex_app_server_protocol::ReasoningSummaryTextDeltaNotification;
use codex_app_server_protocol::ReasoningTextDeltaNotification;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ServerRequestPayload;
use codex_app_server_protocol::TerminalInteractionNotification;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::ThreadNameUpdatedNotification;
use codex_app_server_protocol::ThreadRollbackResponse;
use codex_app_server_protocol::ThreadTokenUsage;
use codex_app_server_protocol::ThreadTokenUsageUpdatedNotification;
use codex_app_server_protocol::ToolRequestUserInputOption;
use codex_app_server_protocol::ToolRequestUserInputParams;
use codex_app_server_protocol::ToolRequestUserInputQuestion;
use codex_app_server_protocol::ToolRequestUserInputResponse;
use codex_app_server_protocol::Turn;
use codex_app_server_protocol::TurnCompletedNotification;
use codex_app_server_protocol::TurnDiffUpdatedNotification;
use codex_app_server_protocol::TurnError;
use codex_app_server_protocol::TurnInterruptResponse;
use codex_app_server_protocol::TurnPlanStep;
use codex_app_server_protocol::TurnPlanUpdatedNotification;
use codex_app_server_protocol::TurnStatus;
use codex_app_server_protocol::build_turns_from_event_msgs;
use codex_core::CodexThread;
use codex_core::parse_command::shlex_join;
use codex_core::protocol::ApplyPatchApprovalRequestEvent;
use codex_core::protocol::CodexErrorInfo as CoreCodexErrorInfo;
use codex_core::protocol::Event;
use codex_core::protocol::EventMsg;
use codex_core::protocol::ExecApprovalRequestEvent;
use codex_core::protocol::ExecCommandEndEvent;
use codex_core::protocol::FileChange as CoreFileChange;
use codex_core::protocol::McpToolCallBeginEvent;
use codex_core::protocol::McpToolCallEndEvent;
use codex_core::protocol::Op;
use codex_core::protocol::ReviewDecision;
use codex_core::protocol::TokenCountEvent;
use codex_core::protocol::TurnDiffEvent;
use codex_core::review_format::format_review_findings_block;
use codex_core::review_prompts;
use codex_protocol::ThreadId;
use codex_protocol::dynamic_tools::DynamicToolCallOutputContentItem as CoreDynamicToolCallOutputContentItem;
use codex_protocol::dynamic_tools::DynamicToolResponse as CoreDynamicToolResponse;
use codex_protocol::plan_tool::UpdatePlanArgs;
use codex_protocol::protocol::ReviewOutputEvent;
use codex_protocol::request_user_input::RequestUserInputAnswer as CoreRequestUserInputAnswer;
use codex_protocol::request_user_input::RequestUserInputResponse as CoreRequestUserInputResponse;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::oneshot;
use tracing::error;

type JsonValue = serde_json::Value;

#[allow(clippy::too_many_arguments)]
pub(crate) async fn apply_bespoke_event_handling(
    event: Event,
    conversation_id: ThreadId,
    conversation: Arc<CodexThread>,
    outgoing: Arc<OutgoingMessageSender>,
    pending_interrupts: PendingInterrupts,
    pending_rollbacks: PendingRollbacks,
    turn_summary_store: TurnSummaryStore,
    api_version: ApiVersion,
    fallback_model_provider: String,
) {
    let Event {
        id: event_turn_id,
        msg,
    } = event;
    match msg {
        EventMsg::TurnStarted(_) => {}
        EventMsg::TurnComplete(_ev) => {
            handle_turn_complete(
                conversation_id,
                event_turn_id,
                &outgoing,
                &turn_summary_store,
            )
            .await;
        }
        EventMsg::ApplyPatchApprovalRequest(ApplyPatchApprovalRequestEvent {
            call_id,
            turn_id,
            changes,
            reason,
            grant_root,
        }) => match api_version {
            ApiVersion::V1 => {
                let params = ApplyPatchApprovalParams {
                    conversation_id,
                    call_id,
                    file_changes: changes.clone(),
                    reason,
                    grant_root,
                };
                let rx = outgoing
                    .send_request(ServerRequestPayload::ApplyPatchApproval(params))
                    .await;
                tokio::spawn(async move {
                    on_patch_approval_response(event_turn_id, rx, conversation).await;
                });
            }
            ApiVersion::V2 => {
                // Until we migrate the core to be aware of a first class FileChangeItem
                // and emit the corresponding EventMsg, we repurpose the call_id as the item_id.
                let item_id = call_id.clone();
                let patch_changes = convert_patch_changes(&changes);

                let first_start = {
                    let mut map = turn_summary_store.lock().await;
                    let summary = map.entry(conversation_id).or_default();
                    summary.file_change_started.insert(item_id.clone())
                };
                if first_start {
                    let item = ThreadItem::FileChange {
                        id: item_id.clone(),
                        changes: patch_changes.clone(),
                        status: PatchApplyStatus::InProgress,
                    };
                    let notification = ItemStartedNotification {
                        thread_id: conversation_id.to_string(),
                        turn_id: event_turn_id.clone(),
                        item,
                    };
                    outgoing
                        .send_server_notification(ServerNotification::ItemStarted(notification))
                        .await;
                }

                let params = FileChangeRequestApprovalParams {
                    thread_id: conversation_id.to_string(),
                    turn_id: turn_id.clone(),
                    item_id: item_id.clone(),
                    reason,
                    grant_root,
                };
                let rx = outgoing
                    .send_request(ServerRequestPayload::FileChangeRequestApproval(params))
                    .await;
                tokio::spawn(async move {
                    on_file_change_request_approval_response(
                        event_turn_id,
                        conversation_id,
                        item_id,
                        patch_changes,
                        rx,
                        conversation,
                        outgoing,
                        turn_summary_store,
                    )
                    .await;
                });
            }
        },
        EventMsg::ExecApprovalRequest(ExecApprovalRequestEvent {
            call_id,
            turn_id,
            command,
            cwd,
            reason,
            proposed_execpolicy_amendment,
            parsed_cmd,
        }) => match api_version {
            ApiVersion::V1 => {
                let params = ExecCommandApprovalParams {
                    conversation_id,
                    call_id,
                    command,
                    cwd,
                    reason,
                    parsed_cmd,
                };
                let rx = outgoing
                    .send_request(ServerRequestPayload::ExecCommandApproval(params))
                    .await;
                tokio::spawn(async move {
                    on_exec_approval_response(event_turn_id, rx, conversation).await;
                });
            }
            ApiVersion::V2 => {
                let item_id = call_id.clone();
                let command_actions = parsed_cmd
                    .iter()
                    .cloned()
                    .map(V2ParsedCommand::from)
                    .collect::<Vec<_>>();
                let command_string = shlex_join(&command);
                let proposed_execpolicy_amendment_v2 =
                    proposed_execpolicy_amendment.map(V2ExecPolicyAmendment::from);

                let params = CommandExecutionRequestApprovalParams {
                    thread_id: conversation_id.to_string(),
                    turn_id: turn_id.clone(),
                    // Until we migrate the core to be aware of a first class CommandExecutionItem
                    // and emit the corresponding EventMsg, we repurpose the call_id as the item_id.
                    item_id: item_id.clone(),
                    reason,
                    command: Some(command_string.clone()),
                    cwd: Some(cwd.clone()),
                    command_actions: Some(command_actions.clone()),
                    proposed_execpolicy_amendment: proposed_execpolicy_amendment_v2,
                };
                let rx = outgoing
                    .send_request(ServerRequestPayload::CommandExecutionRequestApproval(
                        params,
                    ))
                    .await;
                tokio::spawn(async move {
                    on_command_execution_request_approval_response(
                        event_turn_id,
                        conversation_id,
                        item_id,
                        command_string,
                        cwd,
                        command_actions,
                        rx,
                        conversation,
                        outgoing,
                    )
                    .await;
                });
            }
        },
        EventMsg::RequestUserInput(request) => {
            if matches!(api_version, ApiVersion::V2) {
                let questions = request
                    .questions
                    .into_iter()
                    .map(|question| ToolRequestUserInputQuestion {
                        id: question.id,
                        header: question.header,
                        question: question.question,
                        is_other: question.is_other,
                        is_secret: question.is_secret,
                        options: question.options.map(|options| {
                            options
                                .into_iter()
                                .map(|option| ToolRequestUserInputOption {
                                    label: option.label,
                                    description: option.description,
                                })
                                .collect()
                        }),
                    })
                    .collect();
                let params = ToolRequestUserInputParams {
                    thread_id: conversation_id.to_string(),
                    turn_id: request.turn_id,
                    item_id: request.call_id,
                    questions,
                };
                let rx = outgoing
                    .send_request(ServerRequestPayload::ToolRequestUserInput(params))
                    .await;
                tokio::spawn(async move {
                    on_request_user_input_response(event_turn_id, rx, conversation).await;
                });
            } else {
                error!(
                    "request_user_input is only supported on api v2 (call_id: {})",
                    request.call_id
                );
                let empty = CoreRequestUserInputResponse {
                    answers: HashMap::new(),
                };
                if let Err(err) = conversation
                    .submit(Op::UserInputAnswer {
                        id: event_turn_id,
                        response: empty,
                    })
                    .await
                {
                    error!("failed to submit UserInputAnswer: {err}");
                }
            }
        }
        EventMsg::DynamicToolCallRequest(request) => {
            if matches!(api_version, ApiVersion::V2) {
                let call_id = request.call_id;
                let params = DynamicToolCallParams {
                    thread_id: conversation_id.to_string(),
                    turn_id: request.turn_id,
                    call_id: call_id.clone(),
                    tool: request.tool,
                    arguments: request.arguments,
                };
                let rx = outgoing
                    .send_request(ServerRequestPayload::DynamicToolCall(params))
                    .await;
                tokio::spawn(async move {
                    crate::dynamic_tools::on_call_response(call_id, rx, conversation).await;
                });
            } else {
                error!(
                    "dynamic tool calls are only supported on api v2 (call_id: {})",
                    request.call_id
                );
                let call_id = request.call_id;
                let _ = conversation
                    .submit(Op::DynamicToolResponse {
                        id: call_id.clone(),
                        response: CoreDynamicToolResponse {
                            content_items: vec![CoreDynamicToolCallOutputContentItem::InputText {
                                text: "dynamic tool calls require api v2".to_string(),
                            }],
                            success: false,
                        },
                    })
                    .await;
            }
        }
        // TODO(celia): properly construct McpToolCall TurnItem in core.
        EventMsg::McpToolCallBegin(begin_event) => {
            let notification = construct_mcp_tool_call_notification(
                begin_event,
                conversation_id.to_string(),
                event_turn_id.clone(),
            )
            .await;
            outgoing
                .send_server_notification(ServerNotification::ItemStarted(notification))
                .await;
        }
        EventMsg::McpToolCallEnd(end_event) => {
            let notification = construct_mcp_tool_call_end_notification(
                end_event,
                conversation_id.to_string(),
                event_turn_id.clone(),
            )
            .await;
            outgoing
                .send_server_notification(ServerNotification::ItemCompleted(notification))
                .await;
        }
        EventMsg::CollabAgentSpawnBegin(begin_event) => {
            let item = ThreadItem::CollabAgentToolCall {
                id: begin_event.call_id,
                tool: CollabAgentTool::SpawnAgent,
                status: V2CollabToolCallStatus::InProgress,
                sender_thread_id: begin_event.sender_thread_id.to_string(),
                receiver_thread_ids: Vec::new(),
                prompt: Some(begin_event.prompt),
                agents_states: HashMap::new(),
            };
            let notification = ItemStartedNotification {
                thread_id: conversation_id.to_string(),
                turn_id: event_turn_id.clone(),
                item,
            };
            outgoing
                .send_server_notification(ServerNotification::ItemStarted(notification))
                .await;
        }
        EventMsg::CollabAgentSpawnEnd(end_event) => {
            let has_receiver = end_event.new_thread_id.is_some();
            let status = match &end_event.status {
                codex_protocol::protocol::AgentStatus::Errored(_)
                | codex_protocol::protocol::AgentStatus::NotFound => V2CollabToolCallStatus::Failed,
                _ if has_receiver => V2CollabToolCallStatus::Completed,
                _ => V2CollabToolCallStatus::Failed,
            };
            let (receiver_thread_ids, agents_states) = match end_event.new_thread_id {
                Some(id) => {
                    let receiver_id = id.to_string();
                    let received_status = V2CollabAgentStatus::from(end_event.status.clone());
                    (
                        vec![receiver_id.clone()],
                        [(receiver_id, received_status)].into_iter().collect(),
                    )
                }
                None => (Vec::new(), HashMap::new()),
            };
            let item = ThreadItem::CollabAgentToolCall {
                id: end_event.call_id,
                tool: CollabAgentTool::SpawnAgent,
                status,
                sender_thread_id: end_event.sender_thread_id.to_string(),
                receiver_thread_ids,
                prompt: Some(end_event.prompt),
                agents_states,
            };
            let notification = ItemCompletedNotification {
                thread_id: conversation_id.to_string(),
                turn_id: event_turn_id.clone(),
                item,
            };
            outgoing
                .send_server_notification(ServerNotification::ItemCompleted(notification))
                .await;
        }
        EventMsg::CollabAgentInteractionBegin(begin_event) => {
            let receiver_thread_ids = vec![begin_event.receiver_thread_id.to_string()];
            let item = ThreadItem::CollabAgentToolCall {
                id: begin_event.call_id,
                tool: CollabAgentTool::SendInput,
                status: V2CollabToolCallStatus::InProgress,
                sender_thread_id: begin_event.sender_thread_id.to_string(),
                receiver_thread_ids,
                prompt: Some(begin_event.prompt),
                agents_states: HashMap::new(),
            };
            let notification = ItemStartedNotification {
                thread_id: conversation_id.to_string(),
                turn_id: event_turn_id.clone(),
                item,
            };
            outgoing
                .send_server_notification(ServerNotification::ItemStarted(notification))
                .await;
        }
        EventMsg::CollabAgentInteractionEnd(end_event) => {
            let status = match &end_event.status {
                codex_protocol::protocol::AgentStatus::Errored(_)
                | codex_protocol::protocol::AgentStatus::NotFound => V2CollabToolCallStatus::Failed,
                _ => V2CollabToolCallStatus::Completed,
            };
            let receiver_id = end_event.receiver_thread_id.to_string();
            let received_status = V2CollabAgentStatus::from(end_event.status);
            let item = ThreadItem::CollabAgentToolCall {
                id: end_event.call_id,
                tool: CollabAgentTool::SendInput,
                status,
                sender_thread_id: end_event.sender_thread_id.to_string(),
                receiver_thread_ids: vec![receiver_id.clone()],
                prompt: Some(end_event.prompt),
                agents_states: [(receiver_id, received_status)].into_iter().collect(),
            };
            let notification = ItemCompletedNotification {
                thread_id: conversation_id.to_string(),
                turn_id: event_turn_id.clone(),
                item,
            };
            outgoing
                .send_server_notification(ServerNotification::ItemCompleted(notification))
                .await;
        }
        EventMsg::CollabWaitingBegin(begin_event) => {
            let receiver_thread_ids = begin_event
                .receiver_thread_ids
                .iter()
                .map(ToString::to_string)
                .collect();
            let item = ThreadItem::CollabAgentToolCall {
                id: begin_event.call_id,
                tool: CollabAgentTool::Wait,
                status: V2CollabToolCallStatus::InProgress,
                sender_thread_id: begin_event.sender_thread_id.to_string(),
                receiver_thread_ids,
                prompt: None,
                agents_states: HashMap::new(),
            };
            let notification = ItemStartedNotification {
                thread_id: conversation_id.to_string(),
                turn_id: event_turn_id.clone(),
                item,
            };
            outgoing
                .send_server_notification(ServerNotification::ItemStarted(notification))
                .await;
        }
        EventMsg::CollabWaitingEnd(end_event) => {
            let status = if end_event.statuses.values().any(|status| {
                matches!(
                    status,
                    codex_protocol::protocol::AgentStatus::Errored(_)
                        | codex_protocol::protocol::AgentStatus::NotFound
                )
            }) {
                V2CollabToolCallStatus::Failed
            } else {
                V2CollabToolCallStatus::Completed
            };
            let receiver_thread_ids = end_event.statuses.keys().map(ToString::to_string).collect();
            let agents_states = end_event
                .statuses
                .iter()
                .map(|(id, status)| (id.to_string(), V2CollabAgentStatus::from(status.clone())))
                .collect();
            let item = ThreadItem::CollabAgentToolCall {
                id: end_event.call_id,
                tool: CollabAgentTool::Wait,
                status,
                sender_thread_id: end_event.sender_thread_id.to_string(),
                receiver_thread_ids,
                prompt: None,
                agents_states,
            };
            let notification = ItemCompletedNotification {
                thread_id: conversation_id.to_string(),
                turn_id: event_turn_id.clone(),
                item,
            };
            outgoing
                .send_server_notification(ServerNotification::ItemCompleted(notification))
                .await;
        }
        EventMsg::CollabCloseBegin(begin_event) => {
            let item = ThreadItem::CollabAgentToolCall {
                id: begin_event.call_id,
                tool: CollabAgentTool::CloseAgent,
                status: V2CollabToolCallStatus::InProgress,
                sender_thread_id: begin_event.sender_thread_id.to_string(),
                receiver_thread_ids: vec![begin_event.receiver_thread_id.to_string()],
                prompt: None,
                agents_states: HashMap::new(),
            };
            let notification = ItemStartedNotification {
                thread_id: conversation_id.to_string(),
                turn_id: event_turn_id.clone(),
                item,
            };
            outgoing
                .send_server_notification(ServerNotification::ItemStarted(notification))
                .await;
        }
        EventMsg::CollabCloseEnd(end_event) => {
            let status = match &end_event.status {
                codex_protocol::protocol::AgentStatus::Errored(_)
                | codex_protocol::protocol::AgentStatus::NotFound => V2CollabToolCallStatus::Failed,
                _ => V2CollabToolCallStatus::Completed,
            };
            let receiver_id = end_event.receiver_thread_id.to_string();
            let agents_states = [(
                receiver_id.clone(),
                V2CollabAgentStatus::from(end_event.status),
            )]
            .into_iter()
            .collect();
            let item = ThreadItem::CollabAgentToolCall {
                id: end_event.call_id,
                tool: CollabAgentTool::CloseAgent,
                status,
                sender_thread_id: end_event.sender_thread_id.to_string(),
                receiver_thread_ids: vec![receiver_id],
                prompt: None,
                agents_states,
            };
            let notification = ItemCompletedNotification {
                thread_id: conversation_id.to_string(),
                turn_id: event_turn_id.clone(),
                item,
            };
            outgoing
                .send_server_notification(ServerNotification::ItemCompleted(notification))
                .await;
        }
        EventMsg::AgentMessageContentDelta(event) => {
            let codex_protocol::protocol::AgentMessageContentDeltaEvent { item_id, delta, .. } =
                event;
            let notification = AgentMessageDeltaNotification {
                thread_id: conversation_id.to_string(),
                turn_id: event_turn_id.clone(),
                item_id,
                delta,
            };
            outgoing
                .send_server_notification(ServerNotification::AgentMessageDelta(notification))
                .await;
        }
        EventMsg::PlanDelta(event) => {
            let notification = PlanDeltaNotification {
                thread_id: conversation_id.to_string(),
                turn_id: event_turn_id.clone(),
                item_id: event.item_id,
                delta: event.delta,
            };
            outgoing
                .send_server_notification(ServerNotification::PlanDelta(notification))
                .await;
        }
        EventMsg::ContextCompacted(..) => {
            let notification = ContextCompactedNotification {
                thread_id: conversation_id.to_string(),
                turn_id: event_turn_id.clone(),
            };
            outgoing
                .send_server_notification(ServerNotification::ContextCompacted(notification))
                .await;
        }
        EventMsg::DeprecationNotice(event) => {
            let notification = DeprecationNoticeNotification {
                summary: event.summary,
                details: event.details,
            };
            outgoing
                .send_server_notification(ServerNotification::DeprecationNotice(notification))
                .await;
        }
        EventMsg::ReasoningContentDelta(event) => {
            let notification = ReasoningSummaryTextDeltaNotification {
                thread_id: conversation_id.to_string(),
                turn_id: event_turn_id.clone(),
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
                thread_id: conversation_id.to_string(),
                turn_id: event_turn_id.clone(),
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
                thread_id: conversation_id.to_string(),
                turn_id: event_turn_id.clone(),
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
            handle_token_count_event(conversation_id, event_turn_id, token_count_event, &outgoing)
                .await;
        }
        EventMsg::Error(ev) => {
            let message = ev.message.clone();
            let codex_error_info = ev.codex_error_info.clone();

            // If this error belongs to an in-flight `thread/rollback` request, fail that request
            // (and clear pending state) so subsequent rollbacks are unblocked.
            //
            // Don't send a notification for this error.
            if matches!(
                codex_error_info,
                Some(CoreCodexErrorInfo::ThreadRollbackFailed)
            ) {
                return handle_thread_rollback_failed(
                    conversation_id,
                    message,
                    &pending_rollbacks,
                    &outgoing,
                )
                .await;
            };

            let turn_error = TurnError {
                message: ev.message,
                codex_error_info: ev.codex_error_info.map(V2CodexErrorInfo::from),
                additional_details: None,
            };
            handle_error(conversation_id, turn_error.clone(), &turn_summary_store).await;
            outgoing
                .send_server_notification(ServerNotification::Error(ErrorNotification {
                    error: turn_error.clone(),
                    will_retry: false,
                    thread_id: conversation_id.to_string(),
                    turn_id: event_turn_id.clone(),
                }))
                .await;
        }
        EventMsg::StreamError(ev) => {
            // We don't need to update the turn summary store for stream errors as they are intermediate error states for retries,
            // but we notify the client.
            let turn_error = TurnError {
                message: ev.message,
                codex_error_info: ev.codex_error_info.map(V2CodexErrorInfo::from),
                additional_details: ev.additional_details,
            };
            outgoing
                .send_server_notification(ServerNotification::Error(ErrorNotification {
                    error: turn_error,
                    will_retry: true,
                    thread_id: conversation_id.to_string(),
                    turn_id: event_turn_id.clone(),
                }))
                .await;
        }
        EventMsg::ViewImageToolCall(view_image_event) => {
            let item = ThreadItem::ImageView {
                id: view_image_event.call_id.clone(),
                path: view_image_event.path.to_string_lossy().into_owned(),
            };
            let started = ItemStartedNotification {
                thread_id: conversation_id.to_string(),
                turn_id: event_turn_id.clone(),
                item: item.clone(),
            };
            outgoing
                .send_server_notification(ServerNotification::ItemStarted(started))
                .await;
            let completed = ItemCompletedNotification {
                thread_id: conversation_id.to_string(),
                turn_id: event_turn_id.clone(),
                item,
            };
            outgoing
                .send_server_notification(ServerNotification::ItemCompleted(completed))
                .await;
        }
        EventMsg::EnteredReviewMode(review_request) => {
            let review = review_request
                .user_facing_hint
                .unwrap_or_else(|| review_prompts::user_facing_hint(&review_request.target));
            let item = ThreadItem::EnteredReviewMode {
                id: event_turn_id.clone(),
                review,
            };
            let started = ItemStartedNotification {
                thread_id: conversation_id.to_string(),
                turn_id: event_turn_id.clone(),
                item: item.clone(),
            };
            outgoing
                .send_server_notification(ServerNotification::ItemStarted(started))
                .await;
            let completed = ItemCompletedNotification {
                thread_id: conversation_id.to_string(),
                turn_id: event_turn_id.clone(),
                item,
            };
            outgoing
                .send_server_notification(ServerNotification::ItemCompleted(completed))
                .await;
        }
        EventMsg::ItemStarted(item_started_event) => {
            let item: ThreadItem = item_started_event.item.clone().into();
            let notification = ItemStartedNotification {
                thread_id: conversation_id.to_string(),
                turn_id: event_turn_id.clone(),
                item,
            };
            outgoing
                .send_server_notification(ServerNotification::ItemStarted(notification))
                .await;
        }
        EventMsg::ItemCompleted(item_completed_event) => {
            let item: ThreadItem = item_completed_event.item.clone().into();
            let notification = ItemCompletedNotification {
                thread_id: conversation_id.to_string(),
                turn_id: event_turn_id.clone(),
                item,
            };
            outgoing
                .send_server_notification(ServerNotification::ItemCompleted(notification))
                .await;
        }
        EventMsg::ExitedReviewMode(review_event) => {
            let review = match review_event.review_output {
                Some(output) => render_review_output_text(&output),
                None => REVIEW_FALLBACK_MESSAGE.to_string(),
            };
            let item = ThreadItem::ExitedReviewMode {
                id: event_turn_id.clone(),
                review,
            };
            let started = ItemStartedNotification {
                thread_id: conversation_id.to_string(),
                turn_id: event_turn_id.clone(),
                item: item.clone(),
            };
            outgoing
                .send_server_notification(ServerNotification::ItemStarted(started))
                .await;
            let completed = ItemCompletedNotification {
                thread_id: conversation_id.to_string(),
                turn_id: event_turn_id.clone(),
                item,
            };
            outgoing
                .send_server_notification(ServerNotification::ItemCompleted(completed))
                .await;
        }
        EventMsg::RawResponseItem(raw_response_item_event) => {
            maybe_emit_raw_response_item_completed(
                api_version,
                conversation_id,
                &event_turn_id,
                raw_response_item_event.item,
                outgoing.as_ref(),
            )
            .await;
        }
        EventMsg::PatchApplyBegin(patch_begin_event) => {
            // Until we migrate the core to be aware of a first class FileChangeItem
            // and emit the corresponding EventMsg, we repurpose the call_id as the item_id.
            let item_id = patch_begin_event.call_id.clone();

            let first_start = {
                let mut map = turn_summary_store.lock().await;
                let summary = map.entry(conversation_id).or_default();
                summary.file_change_started.insert(item_id.clone())
            };
            if first_start {
                let item = ThreadItem::FileChange {
                    id: item_id.clone(),
                    changes: convert_patch_changes(&patch_begin_event.changes),
                    status: PatchApplyStatus::InProgress,
                };
                let notification = ItemStartedNotification {
                    thread_id: conversation_id.to_string(),
                    turn_id: event_turn_id.clone(),
                    item,
                };
                outgoing
                    .send_server_notification(ServerNotification::ItemStarted(notification))
                    .await;
            }
        }
        EventMsg::PatchApplyEnd(patch_end_event) => {
            // Until we migrate the core to be aware of a first class FileChangeItem
            // and emit the corresponding EventMsg, we repurpose the call_id as the item_id.
            let item_id = patch_end_event.call_id.clone();

            let status = if patch_end_event.success {
                PatchApplyStatus::Completed
            } else {
                PatchApplyStatus::Failed
            };
            let changes = convert_patch_changes(&patch_end_event.changes);
            complete_file_change_item(
                conversation_id,
                item_id,
                changes,
                status,
                event_turn_id.clone(),
                outgoing.as_ref(),
                &turn_summary_store,
            )
            .await;
        }
        EventMsg::ExecCommandBegin(exec_command_begin_event) => {
            let item_id = exec_command_begin_event.call_id.clone();
            let command_actions = exec_command_begin_event
                .parsed_cmd
                .into_iter()
                .map(V2ParsedCommand::from)
                .collect::<Vec<_>>();
            let command = shlex_join(&exec_command_begin_event.command);
            let cwd = exec_command_begin_event.cwd;
            let process_id = exec_command_begin_event.process_id;

            let item = ThreadItem::CommandExecution {
                id: item_id,
                command,
                cwd,
                process_id,
                status: CommandExecutionStatus::InProgress,
                command_actions,
                aggregated_output: None,
                exit_code: None,
                duration_ms: None,
            };
            let notification = ItemStartedNotification {
                thread_id: conversation_id.to_string(),
                turn_id: event_turn_id.clone(),
                item,
            };
            outgoing
                .send_server_notification(ServerNotification::ItemStarted(notification))
                .await;
        }
        EventMsg::ExecCommandOutputDelta(exec_command_output_delta_event) => {
            let item_id = exec_command_output_delta_event.call_id.clone();
            let delta = String::from_utf8_lossy(&exec_command_output_delta_event.chunk).to_string();
            // The underlying EventMsg::ExecCommandOutputDelta is used for shell, unified_exec,
            // and apply_patch tool calls. We represent apply_patch with the FileChange item, and
            // everything else with the CommandExecution item.
            //
            // We need to detect which item type it is so we can emit the right notification.
            // We already have state tracking FileChange items on item/started, so let's use that.
            let is_file_change = {
                let map = turn_summary_store.lock().await;
                map.get(&conversation_id)
                    .is_some_and(|summary| summary.file_change_started.contains(&item_id))
            };
            if is_file_change {
                let notification = FileChangeOutputDeltaNotification {
                    thread_id: conversation_id.to_string(),
                    turn_id: event_turn_id.clone(),
                    item_id,
                    delta,
                };
                outgoing
                    .send_server_notification(ServerNotification::FileChangeOutputDelta(
                        notification,
                    ))
                    .await;
            } else {
                let notification = CommandExecutionOutputDeltaNotification {
                    thread_id: conversation_id.to_string(),
                    turn_id: event_turn_id.clone(),
                    item_id,
                    delta,
                };
                outgoing
                    .send_server_notification(ServerNotification::CommandExecutionOutputDelta(
                        notification,
                    ))
                    .await;
            }
        }
        EventMsg::TerminalInteraction(terminal_event) => {
            let item_id = terminal_event.call_id.clone();

            let notification = TerminalInteractionNotification {
                thread_id: conversation_id.to_string(),
                turn_id: event_turn_id.clone(),
                item_id,
                process_id: terminal_event.process_id,
                stdin: terminal_event.stdin,
            };
            outgoing
                .send_server_notification(ServerNotification::TerminalInteraction(notification))
                .await;
        }
        EventMsg::ExecCommandEnd(exec_command_end_event) => {
            let ExecCommandEndEvent {
                call_id,
                command,
                cwd,
                parsed_cmd,
                process_id,
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
            let command_actions = parsed_cmd
                .into_iter()
                .map(V2ParsedCommand::from)
                .collect::<Vec<_>>();

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
                process_id,
                status,
                command_actions,
                aggregated_output,
                exit_code: Some(exit_code),
                duration_ms: Some(duration_ms),
            };

            let notification = ItemCompletedNotification {
                thread_id: conversation_id.to_string(),
                turn_id: event_turn_id.clone(),
                item,
            };
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

            handle_turn_interrupted(
                conversation_id,
                event_turn_id,
                &outgoing,
                &turn_summary_store,
            )
            .await;
        }
        EventMsg::ThreadRolledBack(_rollback_event) => {
            let pending = {
                let mut map = pending_rollbacks.lock().await;
                map.remove(&conversation_id)
            };

            if let Some(request_id) = pending {
                let Some(rollout_path) = conversation.rollout_path() else {
                    let error = JSONRPCErrorError {
                        code: INVALID_REQUEST_ERROR_CODE,
                        message: "thread has no persisted rollout".to_string(),
                        data: None,
                    };
                    outgoing.send_error(request_id, error).await;
                    return;
                };
                let response = match read_summary_from_rollout(
                    rollout_path.as_path(),
                    fallback_model_provider.as_str(),
                )
                .await
                {
                    Ok(summary) => {
                        let mut thread = summary_to_thread(summary);
                        match read_event_msgs_from_rollout(rollout_path.as_path()).await {
                            Ok(events) => {
                                thread.turns = build_turns_from_event_msgs(&events);
                                ThreadRollbackResponse { thread }
                            }
                            Err(err) => {
                                let error = JSONRPCErrorError {
                                    code: INTERNAL_ERROR_CODE,
                                    message: format!(
                                        "failed to load rollout `{}`: {err}",
                                        rollout_path.display()
                                    ),
                                    data: None,
                                };
                                outgoing.send_error(request_id.clone(), error).await;
                                return;
                            }
                        }
                    }
                    Err(err) => {
                        let error = JSONRPCErrorError {
                            code: INTERNAL_ERROR_CODE,
                            message: format!(
                                "failed to load rollout `{}`: {err}",
                                rollout_path.display()
                            ),
                            data: None,
                        };
                        outgoing.send_error(request_id.clone(), error).await;
                        return;
                    }
                };

                outgoing.send_response(request_id, response).await;
            }
        }
        EventMsg::ThreadNameUpdated(thread_name_event) => {
            if let ApiVersion::V2 = api_version {
                let notification = ThreadNameUpdatedNotification {
                    thread_id: thread_name_event.thread_id.to_string(),
                    thread_name: thread_name_event.thread_name,
                };
                outgoing
                    .send_server_notification(ServerNotification::ThreadNameUpdated(notification))
                    .await;
            }
        }
        EventMsg::TurnDiff(turn_diff_event) => {
            handle_turn_diff(
                conversation_id,
                &event_turn_id,
                turn_diff_event,
                api_version,
                outgoing.as_ref(),
            )
            .await;
        }
        EventMsg::PlanUpdate(plan_update_event) => {
            handle_turn_plan_update(
                conversation_id,
                &event_turn_id,
                plan_update_event,
                api_version,
                outgoing.as_ref(),
            )
            .await;
        }

        _ => {}
    }
}

async fn handle_turn_diff(
    conversation_id: ThreadId,
    event_turn_id: &str,
    turn_diff_event: TurnDiffEvent,
    api_version: ApiVersion,
    outgoing: &OutgoingMessageSender,
) {
    if let ApiVersion::V2 = api_version {
        let notification = TurnDiffUpdatedNotification {
            thread_id: conversation_id.to_string(),
            turn_id: event_turn_id.to_string(),
            diff: turn_diff_event.unified_diff,
        };
        outgoing
            .send_server_notification(ServerNotification::TurnDiffUpdated(notification))
            .await;
    }
}

async fn handle_turn_plan_update(
    conversation_id: ThreadId,
    event_turn_id: &str,
    plan_update_event: UpdatePlanArgs,
    api_version: ApiVersion,
    outgoing: &OutgoingMessageSender,
) {
    // `update_plan` is a todo/checklist tool; it is not related to plan-mode updates
    if let ApiVersion::V2 = api_version {
        let notification = TurnPlanUpdatedNotification {
            thread_id: conversation_id.to_string(),
            turn_id: event_turn_id.to_string(),
            explanation: plan_update_event.explanation,
            plan: plan_update_event
                .plan
                .into_iter()
                .map(TurnPlanStep::from)
                .collect(),
        };
        outgoing
            .send_server_notification(ServerNotification::TurnPlanUpdated(notification))
            .await;
    }
}

async fn emit_turn_completed_with_status(
    conversation_id: ThreadId,
    event_turn_id: String,
    status: TurnStatus,
    error: Option<TurnError>,
    outgoing: &OutgoingMessageSender,
) {
    let notification = TurnCompletedNotification {
        thread_id: conversation_id.to_string(),
        turn: Turn {
            id: event_turn_id,
            items: vec![],
            error,
            status,
        },
    };
    outgoing
        .send_server_notification(ServerNotification::TurnCompleted(notification))
        .await;
}

async fn complete_file_change_item(
    conversation_id: ThreadId,
    item_id: String,
    changes: Vec<FileUpdateChange>,
    status: PatchApplyStatus,
    turn_id: String,
    outgoing: &OutgoingMessageSender,
    turn_summary_store: &TurnSummaryStore,
) {
    {
        let mut map = turn_summary_store.lock().await;
        if let Some(summary) = map.get_mut(&conversation_id) {
            summary.file_change_started.remove(&item_id);
        }
    }

    let item = ThreadItem::FileChange {
        id: item_id,
        changes,
        status,
    };
    let notification = ItemCompletedNotification {
        thread_id: conversation_id.to_string(),
        turn_id,
        item,
    };
    outgoing
        .send_server_notification(ServerNotification::ItemCompleted(notification))
        .await;
}

#[allow(clippy::too_many_arguments)]
async fn complete_command_execution_item(
    conversation_id: ThreadId,
    turn_id: String,
    item_id: String,
    command: String,
    cwd: PathBuf,
    process_id: Option<String>,
    command_actions: Vec<V2ParsedCommand>,
    status: CommandExecutionStatus,
    outgoing: &OutgoingMessageSender,
) {
    let item = ThreadItem::CommandExecution {
        id: item_id,
        command,
        cwd,
        process_id,
        status,
        command_actions,
        aggregated_output: None,
        exit_code: None,
        duration_ms: None,
    };
    let notification = ItemCompletedNotification {
        thread_id: conversation_id.to_string(),
        turn_id,
        item,
    };
    outgoing
        .send_server_notification(ServerNotification::ItemCompleted(notification))
        .await;
}

async fn maybe_emit_raw_response_item_completed(
    api_version: ApiVersion,
    conversation_id: ThreadId,
    turn_id: &str,
    item: codex_protocol::models::ResponseItem,
    outgoing: &OutgoingMessageSender,
) {
    let ApiVersion::V2 = api_version else {
        return;
    };

    let notification = RawResponseItemCompletedNotification {
        thread_id: conversation_id.to_string(),
        turn_id: turn_id.to_string(),
        item,
    };
    outgoing
        .send_server_notification(ServerNotification::RawResponseItemCompleted(notification))
        .await;
}

async fn find_and_remove_turn_summary(
    conversation_id: ThreadId,
    turn_summary_store: &TurnSummaryStore,
) -> TurnSummary {
    let mut map = turn_summary_store.lock().await;
    map.remove(&conversation_id).unwrap_or_default()
}

async fn handle_turn_complete(
    conversation_id: ThreadId,
    event_turn_id: String,
    outgoing: &OutgoingMessageSender,
    turn_summary_store: &TurnSummaryStore,
) {
    let turn_summary = find_and_remove_turn_summary(conversation_id, turn_summary_store).await;

    let (status, error) = match turn_summary.last_error {
        Some(error) => (TurnStatus::Failed, Some(error)),
        None => (TurnStatus::Completed, None),
    };

    emit_turn_completed_with_status(conversation_id, event_turn_id, status, error, outgoing).await;
}

async fn handle_turn_interrupted(
    conversation_id: ThreadId,
    event_turn_id: String,
    outgoing: &OutgoingMessageSender,
    turn_summary_store: &TurnSummaryStore,
) {
    find_and_remove_turn_summary(conversation_id, turn_summary_store).await;

    emit_turn_completed_with_status(
        conversation_id,
        event_turn_id,
        TurnStatus::Interrupted,
        None,
        outgoing,
    )
    .await;
}

async fn handle_thread_rollback_failed(
    conversation_id: ThreadId,
    message: String,
    pending_rollbacks: &PendingRollbacks,
    outgoing: &OutgoingMessageSender,
) {
    let pending_rollback = {
        let mut map = pending_rollbacks.lock().await;
        map.remove(&conversation_id)
    };

    if let Some(request_id) = pending_rollback {
        outgoing
            .send_error(
                request_id,
                JSONRPCErrorError {
                    code: INVALID_REQUEST_ERROR_CODE,
                    message: message.clone(),
                    data: None,
                },
            )
            .await;
    }
}

async fn handle_token_count_event(
    conversation_id: ThreadId,
    turn_id: String,
    token_count_event: TokenCountEvent,
    outgoing: &OutgoingMessageSender,
) {
    let TokenCountEvent { info, rate_limits } = token_count_event;
    if let Some(token_usage) = info.map(ThreadTokenUsage::from) {
        let notification = ThreadTokenUsageUpdatedNotification {
            thread_id: conversation_id.to_string(),
            turn_id,
            token_usage,
        };
        outgoing
            .send_server_notification(ServerNotification::ThreadTokenUsageUpdated(notification))
            .await;
    }
    if let Some(rate_limits) = rate_limits {
        outgoing
            .send_server_notification(ServerNotification::AccountRateLimitsUpdated(
                AccountRateLimitsUpdatedNotification {
                    rate_limits: rate_limits.into(),
                },
            ))
            .await;
    }
}

async fn handle_error(
    conversation_id: ThreadId,
    error: TurnError,
    turn_summary_store: &TurnSummaryStore,
) {
    let mut map = turn_summary_store.lock().await;
    map.entry(conversation_id).or_default().last_error = Some(error);
}

async fn on_patch_approval_response(
    event_turn_id: String,
    receiver: oneshot::Receiver<JsonValue>,
    codex: Arc<CodexThread>,
) {
    let response = receiver.await;
    let value = match response {
        Ok(value) => value,
        Err(err) => {
            error!("request failed: {err:?}");
            if let Err(submit_err) = codex
                .submit(Op::PatchApproval {
                    id: event_turn_id.clone(),
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
            id: event_turn_id,
            decision: response.decision,
        })
        .await
    {
        error!("failed to submit PatchApproval: {err}");
    }
}

async fn on_exec_approval_response(
    event_turn_id: String,
    receiver: oneshot::Receiver<JsonValue>,
    conversation: Arc<CodexThread>,
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
            id: event_turn_id,
            decision: response.decision,
        })
        .await
    {
        error!("failed to submit ExecApproval: {err}");
    }
}

async fn on_request_user_input_response(
    event_turn_id: String,
    receiver: oneshot::Receiver<JsonValue>,
    conversation: Arc<CodexThread>,
) {
    let response = receiver.await;
    let value = match response {
        Ok(value) => value,
        Err(err) => {
            error!("request failed: {err:?}");
            let empty = CoreRequestUserInputResponse {
                answers: HashMap::new(),
            };
            if let Err(err) = conversation
                .submit(Op::UserInputAnswer {
                    id: event_turn_id,
                    response: empty,
                })
                .await
            {
                error!("failed to submit UserInputAnswer: {err}");
            }
            return;
        }
    };

    let response =
        serde_json::from_value::<ToolRequestUserInputResponse>(value).unwrap_or_else(|err| {
            error!("failed to deserialize ToolRequestUserInputResponse: {err}");
            ToolRequestUserInputResponse {
                answers: HashMap::new(),
            }
        });
    let response = CoreRequestUserInputResponse {
        answers: response
            .answers
            .into_iter()
            .map(|(id, answer)| {
                (
                    id,
                    CoreRequestUserInputAnswer {
                        answers: answer.answers,
                    },
                )
            })
            .collect(),
    };

    if let Err(err) = conversation
        .submit(Op::UserInputAnswer {
            id: event_turn_id,
            response,
        })
        .await
    {
        error!("failed to submit UserInputAnswer: {err}");
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

fn convert_patch_changes(changes: &HashMap<PathBuf, CoreFileChange>) -> Vec<FileUpdateChange> {
    let mut converted: Vec<FileUpdateChange> = changes
        .iter()
        .map(|(path, change)| FileUpdateChange {
            path: path.to_string_lossy().into_owned(),
            kind: map_patch_change_kind(change),
            diff: format_file_change_diff(change),
        })
        .collect();
    converted.sort_by(|a, b| a.path.cmp(&b.path));
    converted
}

fn map_patch_change_kind(change: &CoreFileChange) -> V2PatchChangeKind {
    match change {
        CoreFileChange::Add { .. } => V2PatchChangeKind::Add,
        CoreFileChange::Delete { .. } => V2PatchChangeKind::Delete,
        CoreFileChange::Update { move_path, .. } => V2PatchChangeKind::Update {
            move_path: move_path.clone(),
        },
    }
}

fn format_file_change_diff(change: &CoreFileChange) -> String {
    match change {
        CoreFileChange::Add { content } => content.clone(),
        CoreFileChange::Delete { content } => content.clone(),
        CoreFileChange::Update {
            unified_diff,
            move_path,
        } => {
            if let Some(path) = move_path {
                format!("{unified_diff}\n\nMoved to: {}", path.display())
            } else {
                unified_diff.clone()
            }
        }
    }
}

fn map_file_change_approval_decision(
    decision: FileChangeApprovalDecision,
) -> (ReviewDecision, Option<PatchApplyStatus>) {
    match decision {
        FileChangeApprovalDecision::Accept => (ReviewDecision::Approved, None),
        FileChangeApprovalDecision::AcceptForSession => (ReviewDecision::ApprovedForSession, None),
        FileChangeApprovalDecision::Decline => {
            (ReviewDecision::Denied, Some(PatchApplyStatus::Declined))
        }
        FileChangeApprovalDecision::Cancel => {
            (ReviewDecision::Abort, Some(PatchApplyStatus::Declined))
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn on_file_change_request_approval_response(
    event_turn_id: String,
    conversation_id: ThreadId,
    item_id: String,
    changes: Vec<FileUpdateChange>,
    receiver: oneshot::Receiver<JsonValue>,
    codex: Arc<CodexThread>,
    outgoing: Arc<OutgoingMessageSender>,
    turn_summary_store: TurnSummaryStore,
) {
    let response = receiver.await;
    let (decision, completion_status) = match response {
        Ok(value) => {
            let response = serde_json::from_value::<FileChangeRequestApprovalResponse>(value)
                .unwrap_or_else(|err| {
                    error!("failed to deserialize FileChangeRequestApprovalResponse: {err}");
                    FileChangeRequestApprovalResponse {
                        decision: FileChangeApprovalDecision::Decline,
                    }
                });

            let (decision, completion_status) =
                map_file_change_approval_decision(response.decision);
            // Allow EventMsg::PatchApplyEnd to emit ItemCompleted for accepted patches.
            // Only short-circuit on declines/cancels/failures.
            (decision, completion_status)
        }
        Err(err) => {
            error!("request failed: {err:?}");
            (ReviewDecision::Denied, Some(PatchApplyStatus::Failed))
        }
    };

    if let Some(status) = completion_status {
        complete_file_change_item(
            conversation_id,
            item_id,
            changes,
            status,
            event_turn_id.clone(),
            outgoing.as_ref(),
            &turn_summary_store,
        )
        .await;
    }

    if let Err(err) = codex
        .submit(Op::PatchApproval {
            id: event_turn_id,
            decision,
        })
        .await
    {
        error!("failed to submit PatchApproval: {err}");
    }
}

#[allow(clippy::too_many_arguments)]
async fn on_command_execution_request_approval_response(
    event_turn_id: String,
    conversation_id: ThreadId,
    item_id: String,
    command: String,
    cwd: PathBuf,
    command_actions: Vec<V2ParsedCommand>,
    receiver: oneshot::Receiver<JsonValue>,
    conversation: Arc<CodexThread>,
    outgoing: Arc<OutgoingMessageSender>,
) {
    let response = receiver.await;
    let (decision, completion_status) = match response {
        Ok(value) => {
            let response = serde_json::from_value::<CommandExecutionRequestApprovalResponse>(value)
                .unwrap_or_else(|err| {
                    error!("failed to deserialize CommandExecutionRequestApprovalResponse: {err}");
                    CommandExecutionRequestApprovalResponse {
                        decision: CommandExecutionApprovalDecision::Decline,
                    }
                });

            let decision = response.decision;

            let (decision, completion_status) = match decision {
                CommandExecutionApprovalDecision::Accept => (ReviewDecision::Approved, None),
                CommandExecutionApprovalDecision::AcceptForSession => {
                    (ReviewDecision::ApprovedForSession, None)
                }
                CommandExecutionApprovalDecision::AcceptWithExecpolicyAmendment {
                    execpolicy_amendment,
                } => (
                    ReviewDecision::ApprovedExecpolicyAmendment {
                        proposed_execpolicy_amendment: execpolicy_amendment.into_core(),
                    },
                    None,
                ),
                CommandExecutionApprovalDecision::Decline => (
                    ReviewDecision::Denied,
                    Some(CommandExecutionStatus::Declined),
                ),
                CommandExecutionApprovalDecision::Cancel => (
                    ReviewDecision::Abort,
                    Some(CommandExecutionStatus::Declined),
                ),
            };
            (decision, completion_status)
        }
        Err(err) => {
            error!("request failed: {err:?}");
            (ReviewDecision::Denied, Some(CommandExecutionStatus::Failed))
        }
    };

    if let Some(status) = completion_status {
        complete_command_execution_item(
            conversation_id,
            event_turn_id.clone(),
            item_id.clone(),
            command.clone(),
            cwd.clone(),
            None,
            command_actions.clone(),
            status,
            outgoing.as_ref(),
        )
        .await;
    }

    if let Err(err) = conversation
        .submit(Op::ExecApproval {
            id: event_turn_id,
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
    thread_id: String,
    turn_id: String,
) -> ItemStartedNotification {
    let item = ThreadItem::McpToolCall {
        id: begin_event.call_id,
        server: begin_event.invocation.server,
        tool: begin_event.invocation.tool,
        status: McpToolCallStatus::InProgress,
        arguments: begin_event.invocation.arguments.unwrap_or(JsonValue::Null),
        result: None,
        error: None,
        duration_ms: None,
    };
    ItemStartedNotification {
        thread_id,
        turn_id,
        item,
    }
}

/// similar to handle_mcp_tool_call_end in exec
async fn construct_mcp_tool_call_end_notification(
    end_event: McpToolCallEndEvent,
    thread_id: String,
    turn_id: String,
) -> ItemCompletedNotification {
    let status = if end_event.is_success() {
        McpToolCallStatus::Completed
    } else {
        McpToolCallStatus::Failed
    };
    let duration_ms = i64::try_from(end_event.duration.as_millis()).ok();

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
        duration_ms,
    };
    ItemCompletedNotification {
        thread_id,
        turn_id,
        item,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CHANNEL_CAPACITY;
    use crate::outgoing_message::OutgoingEnvelope;
    use crate::outgoing_message::OutgoingMessage;
    use crate::outgoing_message::OutgoingMessageSender;
    use anyhow::Result;
    use anyhow::anyhow;
    use anyhow::bail;
    use codex_app_server_protocol::TurnPlanStepStatus;
    use codex_core::protocol::CreditsSnapshot;
    use codex_core::protocol::McpInvocation;
    use codex_core::protocol::RateLimitSnapshot;
    use codex_core::protocol::RateLimitWindow;
    use codex_core::protocol::TokenUsage;
    use codex_core::protocol::TokenUsageInfo;
    use codex_protocol::mcp::CallToolResult;
    use codex_protocol::plan_tool::PlanItemArg;
    use codex_protocol::plan_tool::StepStatus;
    use pretty_assertions::assert_eq;
    use rmcp::model::Content;
    use serde_json::Value as JsonValue;
    use std::collections::HashMap;
    use std::time::Duration;
    use tokio::sync::Mutex;
    use tokio::sync::mpsc;

    fn new_turn_summary_store() -> TurnSummaryStore {
        Arc::new(Mutex::new(HashMap::new()))
    }

    async fn recv_broadcast_message(
        rx: &mut mpsc::Receiver<OutgoingEnvelope>,
    ) -> Result<OutgoingMessage> {
        let envelope = rx
            .recv()
            .await
            .ok_or_else(|| anyhow!("should send one message"))?;
        match envelope {
            OutgoingEnvelope::Broadcast { message } => Ok(message),
            OutgoingEnvelope::ToConnection { connection_id, .. } => {
                bail!("unexpected targeted message for connection {connection_id:?}")
            }
        }
    }

    #[test]
    fn file_change_accept_for_session_maps_to_approved_for_session() {
        let (decision, completion_status) =
            map_file_change_approval_decision(FileChangeApprovalDecision::AcceptForSession);
        assert_eq!(decision, ReviewDecision::ApprovedForSession);
        assert_eq!(completion_status, None);
    }

    #[tokio::test]
    async fn test_handle_error_records_message() -> Result<()> {
        let conversation_id = ThreadId::new();
        let turn_summary_store = new_turn_summary_store();

        handle_error(
            conversation_id,
            TurnError {
                message: "boom".to_string(),
                codex_error_info: Some(V2CodexErrorInfo::InternalServerError),
                additional_details: None,
            },
            &turn_summary_store,
        )
        .await;

        let turn_summary = find_and_remove_turn_summary(conversation_id, &turn_summary_store).await;
        assert_eq!(
            turn_summary.last_error,
            Some(TurnError {
                message: "boom".to_string(),
                codex_error_info: Some(V2CodexErrorInfo::InternalServerError),
                additional_details: None,
            })
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_handle_turn_complete_emits_completed_without_error() -> Result<()> {
        let conversation_id = ThreadId::new();
        let event_turn_id = "complete1".to_string();
        let (tx, mut rx) = mpsc::channel(CHANNEL_CAPACITY);
        let outgoing = Arc::new(OutgoingMessageSender::new(tx));
        let turn_summary_store = new_turn_summary_store();

        handle_turn_complete(
            conversation_id,
            event_turn_id.clone(),
            &outgoing,
            &turn_summary_store,
        )
        .await;

        let msg = recv_broadcast_message(&mut rx).await?;
        match msg {
            OutgoingMessage::AppServerNotification(ServerNotification::TurnCompleted(n)) => {
                assert_eq!(n.turn.id, event_turn_id);
                assert_eq!(n.turn.status, TurnStatus::Completed);
                assert_eq!(n.turn.error, None);
            }
            other => bail!("unexpected message: {other:?}"),
        }
        assert!(rx.try_recv().is_err(), "no extra messages expected");
        Ok(())
    }

    #[tokio::test]
    async fn test_handle_turn_interrupted_emits_interrupted_with_error() -> Result<()> {
        let conversation_id = ThreadId::new();
        let event_turn_id = "interrupt1".to_string();
        let turn_summary_store = new_turn_summary_store();
        handle_error(
            conversation_id,
            TurnError {
                message: "oops".to_string(),
                codex_error_info: None,
                additional_details: None,
            },
            &turn_summary_store,
        )
        .await;
        let (tx, mut rx) = mpsc::channel(CHANNEL_CAPACITY);
        let outgoing = Arc::new(OutgoingMessageSender::new(tx));

        handle_turn_interrupted(
            conversation_id,
            event_turn_id.clone(),
            &outgoing,
            &turn_summary_store,
        )
        .await;

        let msg = recv_broadcast_message(&mut rx).await?;
        match msg {
            OutgoingMessage::AppServerNotification(ServerNotification::TurnCompleted(n)) => {
                assert_eq!(n.turn.id, event_turn_id);
                assert_eq!(n.turn.status, TurnStatus::Interrupted);
                assert_eq!(n.turn.error, None);
            }
            other => bail!("unexpected message: {other:?}"),
        }
        assert!(rx.try_recv().is_err(), "no extra messages expected");
        Ok(())
    }

    #[tokio::test]
    async fn test_handle_turn_complete_emits_failed_with_error() -> Result<()> {
        let conversation_id = ThreadId::new();
        let event_turn_id = "complete_err1".to_string();
        let turn_summary_store = new_turn_summary_store();
        handle_error(
            conversation_id,
            TurnError {
                message: "bad".to_string(),
                codex_error_info: Some(V2CodexErrorInfo::Other),
                additional_details: None,
            },
            &turn_summary_store,
        )
        .await;
        let (tx, mut rx) = mpsc::channel(CHANNEL_CAPACITY);
        let outgoing = Arc::new(OutgoingMessageSender::new(tx));

        handle_turn_complete(
            conversation_id,
            event_turn_id.clone(),
            &outgoing,
            &turn_summary_store,
        )
        .await;

        let msg = recv_broadcast_message(&mut rx).await?;
        match msg {
            OutgoingMessage::AppServerNotification(ServerNotification::TurnCompleted(n)) => {
                assert_eq!(n.turn.id, event_turn_id);
                assert_eq!(n.turn.status, TurnStatus::Failed);
                assert_eq!(
                    n.turn.error,
                    Some(TurnError {
                        message: "bad".to_string(),
                        codex_error_info: Some(V2CodexErrorInfo::Other),
                        additional_details: None,
                    })
                );
            }
            other => bail!("unexpected message: {other:?}"),
        }
        assert!(rx.try_recv().is_err(), "no extra messages expected");
        Ok(())
    }

    #[tokio::test]
    async fn test_handle_turn_plan_update_emits_notification_for_v2() -> Result<()> {
        let (tx, mut rx) = mpsc::channel(CHANNEL_CAPACITY);
        let outgoing = OutgoingMessageSender::new(tx);
        let update = UpdatePlanArgs {
            explanation: Some("need plan".to_string()),
            plan: vec![
                PlanItemArg {
                    step: "first".to_string(),
                    status: StepStatus::Pending,
                },
                PlanItemArg {
                    step: "second".to_string(),
                    status: StepStatus::Completed,
                },
            ],
        };

        let conversation_id = ThreadId::new();

        handle_turn_plan_update(
            conversation_id,
            "turn-123",
            update,
            ApiVersion::V2,
            &outgoing,
        )
        .await;

        let msg = recv_broadcast_message(&mut rx).await?;
        match msg {
            OutgoingMessage::AppServerNotification(ServerNotification::TurnPlanUpdated(n)) => {
                assert_eq!(n.thread_id, conversation_id.to_string());
                assert_eq!(n.turn_id, "turn-123");
                assert_eq!(n.explanation.as_deref(), Some("need plan"));
                assert_eq!(n.plan.len(), 2);
                assert_eq!(n.plan[0].step, "first");
                assert_eq!(n.plan[0].status, TurnPlanStepStatus::Pending);
                assert_eq!(n.plan[1].step, "second");
                assert_eq!(n.plan[1].status, TurnPlanStepStatus::Completed);
            }
            other => bail!("unexpected message: {other:?}"),
        }
        assert!(rx.try_recv().is_err(), "no extra messages expected");
        Ok(())
    }

    #[tokio::test]
    async fn test_handle_token_count_event_emits_usage_and_rate_limits() -> Result<()> {
        let conversation_id = ThreadId::new();
        let turn_id = "turn-123".to_string();
        let (tx, mut rx) = mpsc::channel(CHANNEL_CAPACITY);
        let outgoing = Arc::new(OutgoingMessageSender::new(tx));

        let info = TokenUsageInfo {
            total_token_usage: TokenUsage {
                input_tokens: 100,
                cached_input_tokens: 25,
                output_tokens: 50,
                reasoning_output_tokens: 9,
                total_tokens: 200,
            },
            last_token_usage: TokenUsage {
                input_tokens: 10,
                cached_input_tokens: 5,
                output_tokens: 7,
                reasoning_output_tokens: 1,
                total_tokens: 23,
            },
            model_context_window: Some(4096),
        };
        let rate_limits = RateLimitSnapshot {
            primary: Some(RateLimitWindow {
                used_percent: 42.5,
                window_minutes: Some(15),
                resets_at: Some(1700000000),
            }),
            secondary: None,
            credits: Some(CreditsSnapshot {
                has_credits: true,
                unlimited: false,
                balance: Some("5".to_string()),
            }),
            plan_type: None,
        };

        handle_token_count_event(
            conversation_id,
            turn_id.clone(),
            TokenCountEvent {
                info: Some(info),
                rate_limits: Some(rate_limits),
            },
            &outgoing,
        )
        .await;

        let first = recv_broadcast_message(&mut rx).await?;
        match first {
            OutgoingMessage::AppServerNotification(
                ServerNotification::ThreadTokenUsageUpdated(payload),
            ) => {
                assert_eq!(payload.thread_id, conversation_id.to_string());
                assert_eq!(payload.turn_id, turn_id);
                let usage = payload.token_usage;
                assert_eq!(usage.total.total_tokens, 200);
                assert_eq!(usage.total.cached_input_tokens, 25);
                assert_eq!(usage.last.output_tokens, 7);
                assert_eq!(usage.model_context_window, Some(4096));
            }
            other => bail!("unexpected notification: {other:?}"),
        }

        let second = recv_broadcast_message(&mut rx).await?;
        match second {
            OutgoingMessage::AppServerNotification(
                ServerNotification::AccountRateLimitsUpdated(payload),
            ) => {
                assert!(payload.rate_limits.primary.is_some());
                assert!(payload.rate_limits.credits.is_some());
            }
            other => bail!("unexpected notification: {other:?}"),
        }
        Ok(())
    }

    #[tokio::test]
    async fn test_handle_token_count_event_without_usage_info() -> Result<()> {
        let conversation_id = ThreadId::new();
        let turn_id = "turn-456".to_string();
        let (tx, mut rx) = mpsc::channel(CHANNEL_CAPACITY);
        let outgoing = Arc::new(OutgoingMessageSender::new(tx));

        handle_token_count_event(
            conversation_id,
            turn_id.clone(),
            TokenCountEvent {
                info: None,
                rate_limits: None,
            },
            &outgoing,
        )
        .await;

        assert!(
            rx.try_recv().is_err(),
            "no notifications should be emitted when token usage info is absent"
        );
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

        let thread_id = ThreadId::new().to_string();
        let turn_id = "turn_1".to_string();
        let notification = construct_mcp_tool_call_notification(
            begin_event.clone(),
            thread_id.clone(),
            turn_id.clone(),
        )
        .await;

        let expected = ItemStartedNotification {
            thread_id,
            turn_id,
            item: ThreadItem::McpToolCall {
                id: begin_event.call_id,
                server: begin_event.invocation.server,
                tool: begin_event.invocation.tool,
                status: McpToolCallStatus::InProgress,
                arguments: serde_json::json!({"server": ""}),
                result: None,
                error: None,
                duration_ms: None,
            },
        };

        assert_eq!(notification, expected);
    }

    #[tokio::test]
    async fn test_handle_turn_complete_emits_error_multiple_turns() -> Result<()> {
        // Conversation A will have two turns; Conversation B will have one turn.
        let conversation_a = ThreadId::new();
        let conversation_b = ThreadId::new();
        let turn_summary_store = new_turn_summary_store();

        let (tx, mut rx) = mpsc::channel(CHANNEL_CAPACITY);
        let outgoing = Arc::new(OutgoingMessageSender::new(tx));

        // Turn 1 on conversation A
        let a_turn1 = "a_turn1".to_string();
        handle_error(
            conversation_a,
            TurnError {
                message: "a1".to_string(),
                codex_error_info: Some(V2CodexErrorInfo::BadRequest),
                additional_details: None,
            },
            &turn_summary_store,
        )
        .await;
        handle_turn_complete(
            conversation_a,
            a_turn1.clone(),
            &outgoing,
            &turn_summary_store,
        )
        .await;

        // Turn 1 on conversation B
        let b_turn1 = "b_turn1".to_string();
        handle_error(
            conversation_b,
            TurnError {
                message: "b1".to_string(),
                codex_error_info: None,
                additional_details: None,
            },
            &turn_summary_store,
        )
        .await;
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
        let msg = recv_broadcast_message(&mut rx).await?;
        match msg {
            OutgoingMessage::AppServerNotification(ServerNotification::TurnCompleted(n)) => {
                assert_eq!(n.turn.id, a_turn1);
                assert_eq!(n.turn.status, TurnStatus::Failed);
                assert_eq!(
                    n.turn.error,
                    Some(TurnError {
                        message: "a1".to_string(),
                        codex_error_info: Some(V2CodexErrorInfo::BadRequest),
                        additional_details: None,
                    })
                );
            }
            other => bail!("unexpected message: {other:?}"),
        }

        // Verify: B turn 1
        let msg = recv_broadcast_message(&mut rx).await?;
        match msg {
            OutgoingMessage::AppServerNotification(ServerNotification::TurnCompleted(n)) => {
                assert_eq!(n.turn.id, b_turn1);
                assert_eq!(n.turn.status, TurnStatus::Failed);
                assert_eq!(
                    n.turn.error,
                    Some(TurnError {
                        message: "b1".to_string(),
                        codex_error_info: None,
                        additional_details: None,
                    })
                );
            }
            other => bail!("unexpected message: {other:?}"),
        }

        // Verify: A turn 2
        let msg = recv_broadcast_message(&mut rx).await?;
        match msg {
            OutgoingMessage::AppServerNotification(ServerNotification::TurnCompleted(n)) => {
                assert_eq!(n.turn.id, a_turn2);
                assert_eq!(n.turn.status, TurnStatus::Completed);
                assert_eq!(n.turn.error, None);
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

        let thread_id = ThreadId::new().to_string();
        let turn_id = "turn_2".to_string();
        let notification = construct_mcp_tool_call_notification(
            begin_event.clone(),
            thread_id.clone(),
            turn_id.clone(),
        )
        .await;

        let expected = ItemStartedNotification {
            thread_id,
            turn_id,
            item: ThreadItem::McpToolCall {
                id: begin_event.call_id,
                server: begin_event.invocation.server,
                tool: begin_event.invocation.tool,
                status: McpToolCallStatus::InProgress,
                arguments: JsonValue::Null,
                result: None,
                error: None,
                duration_ms: None,
            },
        };

        assert_eq!(notification, expected);
    }

    #[tokio::test]
    async fn test_construct_mcp_tool_call_end_notification_success() {
        let content = vec![
            serde_json::to_value(Content::text("{\"resources\":[]}"))
                .expect("content should serialize"),
        ];
        let result = CallToolResult {
            content: content.clone(),
            is_error: Some(false),
            structured_content: None,
            meta: None,
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

        let thread_id = ThreadId::new().to_string();
        let turn_id = "turn_3".to_string();
        let notification = construct_mcp_tool_call_end_notification(
            end_event.clone(),
            thread_id.clone(),
            turn_id.clone(),
        )
        .await;

        let expected = ItemCompletedNotification {
            thread_id,
            turn_id,
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
                duration_ms: Some(0),
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

        let thread_id = ThreadId::new().to_string();
        let turn_id = "turn_4".to_string();
        let notification = construct_mcp_tool_call_end_notification(
            end_event.clone(),
            thread_id.clone(),
            turn_id.clone(),
        )
        .await;

        let expected = ItemCompletedNotification {
            thread_id,
            turn_id,
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
                duration_ms: Some(1),
            },
        };

        assert_eq!(notification, expected);
    }

    #[tokio::test]
    async fn test_handle_turn_diff_emits_v2_notification() -> Result<()> {
        let (tx, mut rx) = mpsc::channel(CHANNEL_CAPACITY);
        let outgoing = OutgoingMessageSender::new(tx);
        let unified_diff = "--- a\n+++ b\n".to_string();
        let conversation_id = ThreadId::new();

        handle_turn_diff(
            conversation_id,
            "turn-1",
            TurnDiffEvent {
                unified_diff: unified_diff.clone(),
            },
            ApiVersion::V2,
            &outgoing,
        )
        .await;

        let msg = recv_broadcast_message(&mut rx).await?;
        match msg {
            OutgoingMessage::AppServerNotification(ServerNotification::TurnDiffUpdated(
                notification,
            )) => {
                assert_eq!(notification.thread_id, conversation_id.to_string());
                assert_eq!(notification.turn_id, "turn-1");
                assert_eq!(notification.diff, unified_diff);
            }
            other => bail!("unexpected message: {other:?}"),
        }
        assert!(rx.try_recv().is_err(), "no extra messages expected");
        Ok(())
    }

    #[tokio::test]
    async fn test_handle_turn_diff_is_noop_for_v1() -> Result<()> {
        let (tx, mut rx) = mpsc::channel(CHANNEL_CAPACITY);
        let outgoing = OutgoingMessageSender::new(tx);
        let conversation_id = ThreadId::new();

        handle_turn_diff(
            conversation_id,
            "turn-1",
            TurnDiffEvent {
                unified_diff: "diff".to_string(),
            },
            ApiVersion::V1,
            &outgoing,
        )
        .await;

        assert!(rx.try_recv().is_err(), "no messages expected");
        Ok(())
    }
}
