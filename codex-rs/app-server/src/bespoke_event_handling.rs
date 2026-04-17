use crate::codex_message_processor::ApiVersion;
use crate::codex_message_processor::read_rollout_items_from_rollout;
use crate::codex_message_processor::read_summary_from_rollout;
use crate::codex_message_processor::summary_to_thread;
use crate::error_code::INTERNAL_ERROR_CODE;
use crate::error_code::INVALID_REQUEST_ERROR_CODE;
use crate::outgoing_message::ClientRequestResult;
use crate::outgoing_message::ThreadScopedOutgoingMessageSender;
use crate::server_request_error::is_turn_transition_server_request_error;
use crate::thread_state::ThreadState;
use crate::thread_state::TurnSummary;
use crate::thread_state::resolve_server_request_on_thread_listener;
use crate::thread_status::ThreadWatchActiveGuard;
use crate::thread_status::ThreadWatchManager;
use codex_analytics::AnalyticsEventsClient;
use codex_app_server_protocol::AccountRateLimitsUpdatedNotification;
use codex_app_server_protocol::AdditionalPermissionProfile as V2AdditionalPermissionProfile;
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
use codex_app_server_protocol::CommandExecutionSource;
use codex_app_server_protocol::CommandExecutionStatus;
use codex_app_server_protocol::ContextCompactedNotification;
use codex_app_server_protocol::DeprecationNoticeNotification;
use codex_app_server_protocol::DynamicToolCallOutputContentItem;
use codex_app_server_protocol::DynamicToolCallParams;
use codex_app_server_protocol::DynamicToolCallStatus;
use codex_app_server_protocol::ErrorNotification;
use codex_app_server_protocol::ExecCommandApprovalParams;
use codex_app_server_protocol::ExecCommandApprovalResponse;
use codex_app_server_protocol::ExecPolicyAmendment as V2ExecPolicyAmendment;
use codex_app_server_protocol::FileChangeApprovalDecision;
use codex_app_server_protocol::FileChangeOutputDeltaNotification;
use codex_app_server_protocol::FileChangeRequestApprovalParams;
use codex_app_server_protocol::FileChangeRequestApprovalResponse;
use codex_app_server_protocol::FileUpdateChange;
use codex_app_server_protocol::GrantedPermissionProfile as V2GrantedPermissionProfile;
use codex_app_server_protocol::HookCompletedNotification;
use codex_app_server_protocol::HookStartedNotification;
use codex_app_server_protocol::InterruptConversationResponse;
use codex_app_server_protocol::ItemCompletedNotification;
use codex_app_server_protocol::ItemStartedNotification;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::McpServerElicitationAction;
use codex_app_server_protocol::McpServerElicitationRequestParams;
use codex_app_server_protocol::McpServerElicitationRequestResponse;
use codex_app_server_protocol::McpServerStartupState;
use codex_app_server_protocol::McpServerStatusUpdatedNotification;
use codex_app_server_protocol::McpToolCallError;
use codex_app_server_protocol::McpToolCallResult;
use codex_app_server_protocol::McpToolCallStatus;
use codex_app_server_protocol::ModelReroutedNotification;
use codex_app_server_protocol::NetworkApprovalContext as V2NetworkApprovalContext;
use codex_app_server_protocol::NetworkPolicyAmendment as V2NetworkPolicyAmendment;
use codex_app_server_protocol::NetworkPolicyRuleAction as V2NetworkPolicyRuleAction;
use codex_app_server_protocol::PatchApplyStatus;
use codex_app_server_protocol::PermissionsRequestApprovalParams;
use codex_app_server_protocol::PermissionsRequestApprovalResponse;
use codex_app_server_protocol::PlanDeltaNotification;
use codex_app_server_protocol::RawResponseItemCompletedNotification;
use codex_app_server_protocol::ReasoningSummaryPartAddedNotification;
use codex_app_server_protocol::ReasoningSummaryTextDeltaNotification;
use codex_app_server_protocol::ReasoningTextDeltaNotification;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ServerRequestPayload;
use codex_app_server_protocol::SkillsChangedNotification;
use codex_app_server_protocol::TerminalInteractionNotification;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::ThreadNameUpdatedNotification;
use codex_app_server_protocol::ThreadRealtimeClosedNotification;
use codex_app_server_protocol::ThreadRealtimeErrorNotification;
use codex_app_server_protocol::ThreadRealtimeItemAddedNotification;
use codex_app_server_protocol::ThreadRealtimeOutputAudioDeltaNotification;
use codex_app_server_protocol::ThreadRealtimeSdpNotification;
use codex_app_server_protocol::ThreadRealtimeStartedNotification;
use codex_app_server_protocol::ThreadRealtimeTranscriptDeltaNotification;
use codex_app_server_protocol::ThreadRealtimeTranscriptDoneNotification;
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
use codex_app_server_protocol::TurnStartedNotification;
use codex_app_server_protocol::TurnStatus;
use codex_app_server_protocol::build_command_execution_end_item;
use codex_app_server_protocol::build_file_change_approval_request_item;
use codex_app_server_protocol::build_file_change_begin_item;
use codex_app_server_protocol::build_file_change_end_item;
use codex_app_server_protocol::build_item_from_guardian_event;
use codex_app_server_protocol::build_turns_from_rollout_items;
use codex_app_server_protocol::convert_patch_changes;
use codex_app_server_protocol::guardian_auto_approval_review_notification;
use codex_core::CodexThread;
use codex_core::ThreadManager;
use codex_core::find_thread_name_by_id;
use codex_core::review_format::format_review_findings_block;
use codex_core::review_prompts;
use codex_protocol::ThreadId;
use codex_protocol::dynamic_tools::DynamicToolCallOutputContentItem as CoreDynamicToolCallOutputContentItem;
use codex_protocol::dynamic_tools::DynamicToolResponse as CoreDynamicToolResponse;
use codex_protocol::items::parse_hook_prompt_message;
use codex_protocol::plan_tool::UpdatePlanArgs;
use codex_protocol::protocol::CodexErrorInfo as CoreCodexErrorInfo;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ExecApprovalRequestEvent;
use codex_protocol::protocol::McpToolCallBeginEvent;
use codex_protocol::protocol::McpToolCallEndEvent;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::RealtimeEvent;
use codex_protocol::protocol::ReviewDecision;
use codex_protocol::protocol::ReviewOutputEvent;
use codex_protocol::protocol::TokenCountEvent;
use codex_protocol::protocol::TurnAbortedEvent;
use codex_protocol::protocol::TurnCompleteEvent;
use codex_protocol::protocol::TurnDiffEvent;
use codex_protocol::request_permissions::PermissionGrantScope as CorePermissionGrantScope;
use codex_protocol::request_permissions::RequestPermissionProfile as CoreRequestPermissionProfile;
use codex_protocol::request_permissions::RequestPermissionsResponse as CoreRequestPermissionsResponse;
use codex_protocol::request_user_input::RequestUserInputAnswer as CoreRequestUserInputAnswer;
use codex_protocol::request_user_input::RequestUserInputResponse as CoreRequestUserInputResponse;
use codex_sandboxing::policy_transforms::intersect_permission_profiles;
use codex_shell_command::parse_command::shlex_join;
use codex_utils_absolute_path::AbsolutePathBuf;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::oneshot;
use tracing::error;
use tracing::warn;

type JsonValue = serde_json::Value;

enum CommandExecutionApprovalPresentation {
    Network(V2NetworkApprovalContext),
    Command(CommandExecutionCompletionItem),
}

#[derive(Debug, PartialEq)]
struct CommandExecutionCompletionItem {
    command: String,
    cwd: AbsolutePathBuf,
    command_actions: Vec<V2ParsedCommand>,
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn apply_bespoke_event_handling(
    event: Event,
    conversation_id: ThreadId,
    conversation: Arc<CodexThread>,
    thread_manager: Arc<ThreadManager>,
    analytics_events_client: Option<AnalyticsEventsClient>,
    outgoing: ThreadScopedOutgoingMessageSender,
    thread_state: Arc<tokio::sync::Mutex<ThreadState>>,
    thread_watch_manager: ThreadWatchManager,
    api_version: ApiVersion,
    fallback_model_provider: String,
    codex_home: &Path,
) {
    let Event {
        id: event_turn_id,
        msg,
    } = event;
    match msg {
        EventMsg::TurnStarted(payload) => {
            // While not technically necessary as it was already done on TurnComplete, be extra cautios and abort any pending server requests.
            outgoing.abort_pending_server_requests().await;
            thread_watch_manager
                .note_turn_started(&conversation_id.to_string())
                .await;
            if let ApiVersion::V2 = api_version {
                let turn = {
                    let state = thread_state.lock().await;
                    state.active_turn_snapshot().unwrap_or_else(|| Turn {
                        id: payload.turn_id.clone(),
                        items: Vec::new(),
                        error: None,
                        status: TurnStatus::InProgress,
                        started_at: payload.started_at,
                        completed_at: None,
                        duration_ms: None,
                    })
                };
                let notification = TurnStartedNotification {
                    thread_id: conversation_id.to_string(),
                    turn,
                };
                if let Some(analytics_events_client) = analytics_events_client.as_ref() {
                    analytics_events_client
                        .track_notification(ServerNotification::TurnStarted(notification.clone()));
                }
                outgoing
                    .send_server_notification(ServerNotification::TurnStarted(notification))
                    .await;
            }
        }
        EventMsg::TurnComplete(turn_complete_event) => {
            // All per-thread requests are bound to a turn, so abort them.
            outgoing.abort_pending_server_requests().await;
            let turn_failed = thread_state.lock().await.turn_summary.last_error.is_some();
            thread_watch_manager
                .note_turn_completed(&conversation_id.to_string(), turn_failed)
                .await;
            handle_turn_complete(
                conversation_id,
                event_turn_id,
                turn_complete_event,
                analytics_events_client.as_ref(),
                &outgoing,
                &thread_state,
            )
            .await;
        }
        EventMsg::SkillsUpdateAvailable => {
            if let ApiVersion::V2 = api_version {
                outgoing
                    .send_server_notification(ServerNotification::SkillsChanged(
                        SkillsChangedNotification {},
                    ))
                    .await;
            }
        }
        EventMsg::McpStartupUpdate(update) => {
            if let ApiVersion::V2 = api_version {
                let (status, error) = match update.status {
                    codex_protocol::protocol::McpStartupStatus::Starting => {
                        (McpServerStartupState::Starting, None)
                    }
                    codex_protocol::protocol::McpStartupStatus::Ready => {
                        (McpServerStartupState::Ready, None)
                    }
                    codex_protocol::protocol::McpStartupStatus::Failed { error } => {
                        (McpServerStartupState::Failed, Some(error))
                    }
                    codex_protocol::protocol::McpStartupStatus::Cancelled => {
                        (McpServerStartupState::Cancelled, None)
                    }
                };
                let notification = McpServerStatusUpdatedNotification {
                    name: update.server,
                    status,
                    error,
                };
                outgoing
                    .send_server_notification(ServerNotification::McpServerStatusUpdated(
                        notification,
                    ))
                    .await;
            }
        }
        EventMsg::Warning(_warning_event) => {}
        EventMsg::GuardianAssessment(assessment) => {
            if let ApiVersion::V2 = api_version {
                let pending_command_execution = match build_item_from_guardian_event(
                    &assessment,
                    CommandExecutionStatus::InProgress,
                ) {
                    Some(ThreadItem::CommandExecution {
                        id,
                        command,
                        cwd,
                        command_actions,
                        ..
                    }) => Some((
                        id,
                        CommandExecutionCompletionItem {
                            command,
                            cwd,
                            command_actions,
                        },
                    )),
                    Some(_) | None => None,
                };
                let assessment_turn_id = if assessment.turn_id.is_empty() {
                    event_turn_id.clone()
                } else {
                    assessment.turn_id.clone()
                };
                if assessment.status
                    == codex_protocol::protocol::GuardianAssessmentStatus::InProgress
                    && let Some((target_item_id, completion_item)) =
                        pending_command_execution.as_ref()
                {
                    start_command_execution_item(
                        &conversation_id,
                        assessment_turn_id.clone(),
                        target_item_id.clone(),
                        completion_item.command.clone(),
                        completion_item.cwd.clone(),
                        completion_item.command_actions.clone(),
                        CommandExecutionSource::Agent,
                        &outgoing,
                        &thread_state,
                    )
                    .await;
                }
                let notification = guardian_auto_approval_review_notification(
                    &conversation_id,
                    &event_turn_id,
                    &assessment,
                );
                outgoing.send_server_notification(notification).await;
                let completion_status = match assessment.status {
                    codex_protocol::protocol::GuardianAssessmentStatus::Denied
                    | codex_protocol::protocol::GuardianAssessmentStatus::Aborted => {
                        Some(CommandExecutionStatus::Declined)
                    }
                    codex_protocol::protocol::GuardianAssessmentStatus::TimedOut => {
                        Some(CommandExecutionStatus::Failed)
                    }
                    codex_protocol::protocol::GuardianAssessmentStatus::InProgress
                    | codex_protocol::protocol::GuardianAssessmentStatus::Approved => None,
                };
                if let Some(completion_status) = completion_status
                    && let Some((target_item_id, completion_item)) = pending_command_execution
                {
                    complete_command_execution_item(
                        &conversation_id,
                        assessment_turn_id,
                        target_item_id,
                        completion_item.command,
                        completion_item.cwd,
                        /*process_id*/ None,
                        CommandExecutionSource::Agent,
                        completion_item.command_actions,
                        completion_status,
                        &outgoing,
                        &thread_state,
                    )
                    .await;
                }
            }
        }
        EventMsg::ModelReroute(event) => {
            if let ApiVersion::V2 = api_version {
                let notification = ModelReroutedNotification {
                    thread_id: conversation_id.to_string(),
                    turn_id: event_turn_id.clone(),
                    from_model: event.from_model,
                    to_model: event.to_model,
                    reason: event.reason.into(),
                };
                outgoing
                    .send_server_notification(ServerNotification::ModelRerouted(notification))
                    .await;
            }
        }
        EventMsg::RealtimeConversationStarted(event) => {
            if let ApiVersion::V2 = api_version {
                let notification = ThreadRealtimeStartedNotification {
                    thread_id: conversation_id.to_string(),
                    session_id: event.session_id,
                    version: event.version,
                };
                outgoing
                    .send_server_notification(ServerNotification::ThreadRealtimeStarted(
                        notification,
                    ))
                    .await;
            }
        }
        EventMsg::RealtimeConversationSdp(event) => {
            if let ApiVersion::V2 = api_version {
                let notification = ThreadRealtimeSdpNotification {
                    thread_id: conversation_id.to_string(),
                    sdp: event.sdp,
                };
                outgoing
                    .send_server_notification(ServerNotification::ThreadRealtimeSdp(notification))
                    .await;
            }
        }
        EventMsg::RealtimeConversationRealtime(event) => {
            if let ApiVersion::V2 = api_version {
                match event.payload {
                    RealtimeEvent::SessionUpdated { .. } => {}
                    RealtimeEvent::InputAudioSpeechStarted(event) => {
                        let notification = ThreadRealtimeItemAddedNotification {
                            thread_id: conversation_id.to_string(),
                            item: serde_json::json!({
                                "type": "input_audio_buffer.speech_started",
                                "item_id": event.item_id,
                            }),
                        };
                        outgoing
                            .send_server_notification(ServerNotification::ThreadRealtimeItemAdded(
                                notification,
                            ))
                            .await;
                    }
                    RealtimeEvent::InputTranscriptDelta(event) => {
                        let notification = ThreadRealtimeTranscriptDeltaNotification {
                            thread_id: conversation_id.to_string(),
                            role: "user".to_string(),
                            delta: event.delta,
                        };
                        outgoing
                            .send_server_notification(
                                ServerNotification::ThreadRealtimeTranscriptDelta(notification),
                            )
                            .await;
                    }
                    RealtimeEvent::InputTranscriptDone(event) => {
                        let notification = ThreadRealtimeTranscriptDoneNotification {
                            thread_id: conversation_id.to_string(),
                            role: "user".to_string(),
                            text: event.text,
                        };
                        outgoing
                            .send_server_notification(
                                ServerNotification::ThreadRealtimeTranscriptDone(notification),
                            )
                            .await;
                    }
                    RealtimeEvent::OutputTranscriptDelta(event) => {
                        let notification = ThreadRealtimeTranscriptDeltaNotification {
                            thread_id: conversation_id.to_string(),
                            role: "assistant".to_string(),
                            delta: event.delta,
                        };
                        outgoing
                            .send_server_notification(
                                ServerNotification::ThreadRealtimeTranscriptDelta(notification),
                            )
                            .await;
                    }
                    RealtimeEvent::OutputTranscriptDone(event) => {
                        let notification = ThreadRealtimeTranscriptDoneNotification {
                            thread_id: conversation_id.to_string(),
                            role: "assistant".to_string(),
                            text: event.text,
                        };
                        outgoing
                            .send_server_notification(
                                ServerNotification::ThreadRealtimeTranscriptDone(notification),
                            )
                            .await;
                    }
                    RealtimeEvent::AudioOut(audio) => {
                        let notification = ThreadRealtimeOutputAudioDeltaNotification {
                            thread_id: conversation_id.to_string(),
                            audio: audio.into(),
                        };
                        outgoing
                            .send_server_notification(
                                ServerNotification::ThreadRealtimeOutputAudioDelta(notification),
                            )
                            .await;
                    }
                    RealtimeEvent::ResponseCreated(_) => {}
                    RealtimeEvent::ResponseCancelled(event) => {
                        let notification = ThreadRealtimeItemAddedNotification {
                            thread_id: conversation_id.to_string(),
                            item: serde_json::json!({
                                "type": "response.cancelled",
                                "response_id": event.response_id,
                            }),
                        };
                        outgoing
                            .send_server_notification(ServerNotification::ThreadRealtimeItemAdded(
                                notification,
                            ))
                            .await;
                    }
                    RealtimeEvent::ResponseDone(_) => {}
                    RealtimeEvent::ConversationItemAdded(item) => {
                        let notification = ThreadRealtimeItemAddedNotification {
                            thread_id: conversation_id.to_string(),
                            item,
                        };
                        outgoing
                            .send_server_notification(ServerNotification::ThreadRealtimeItemAdded(
                                notification,
                            ))
                            .await;
                    }
                    RealtimeEvent::ConversationItemDone { .. } => {}
                    RealtimeEvent::HandoffRequested(handoff) => {
                        let notification = ThreadRealtimeItemAddedNotification {
                            thread_id: conversation_id.to_string(),
                            item: serde_json::json!({
                                "type": "handoff_request",
                                "handoff_id": handoff.handoff_id,
                                "item_id": handoff.item_id,
                                "input_transcript": handoff.input_transcript,
                                "active_transcript": handoff.active_transcript,
                            }),
                        };
                        outgoing
                            .send_server_notification(ServerNotification::ThreadRealtimeItemAdded(
                                notification,
                            ))
                            .await;
                    }
                    RealtimeEvent::Error(message) => {
                        let notification = ThreadRealtimeErrorNotification {
                            thread_id: conversation_id.to_string(),
                            message,
                        };
                        outgoing
                            .send_server_notification(ServerNotification::ThreadRealtimeError(
                                notification,
                            ))
                            .await;
                    }
                }
            }
        }
        EventMsg::RealtimeConversationClosed(event) => {
            if let ApiVersion::V2 = api_version {
                let notification = ThreadRealtimeClosedNotification {
                    thread_id: conversation_id.to_string(),
                    reason: event.reason,
                };
                outgoing
                    .send_server_notification(ServerNotification::ThreadRealtimeClosed(
                        notification,
                    ))
                    .await;
            }
        }
        EventMsg::ApplyPatchApprovalRequest(event) => {
            let permission_guard = thread_watch_manager
                .note_permission_requested(&conversation_id.to_string())
                .await;
            match api_version {
                ApiVersion::V1 => {
                    let params = ApplyPatchApprovalParams {
                        conversation_id,
                        call_id: event.call_id.clone(),
                        file_changes: event.changes.clone(),
                        reason: event.reason.clone(),
                        grant_root: event.grant_root.clone(),
                    };
                    let (_pending_request_id, rx) = outgoing
                        .send_request(ServerRequestPayload::ApplyPatchApproval(params))
                        .await;
                    let call_id = event.call_id.clone();
                    tokio::spawn(async move {
                        let _permission_guard = permission_guard;
                        on_patch_approval_response(call_id, rx, conversation).await;
                    });
                }
                ApiVersion::V2 => {
                    // Until we migrate the core to be aware of a first class FileChangeItem
                    // and emit the corresponding EventMsg, we repurpose the call_id as the item_id.
                    let item_id = event.call_id.clone();
                    let patch_changes = convert_patch_changes(&event.changes);
                    let first_start = {
                        let mut state = thread_state.lock().await;
                        state
                            .turn_summary
                            .file_change_started
                            .insert(item_id.clone())
                    };
                    if first_start {
                        let item = build_file_change_approval_request_item(&event);
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
                        turn_id: event.turn_id.clone(),
                        item_id: item_id.clone(),
                        reason: event.reason.clone(),
                        grant_root: event.grant_root.clone(),
                    };
                    let (pending_request_id, rx) = outgoing
                        .send_request(ServerRequestPayload::FileChangeRequestApproval(params))
                        .await;
                    tokio::spawn(async move {
                        on_file_change_request_approval_response(
                            event_turn_id,
                            conversation_id,
                            item_id,
                            patch_changes,
                            pending_request_id,
                            rx,
                            conversation,
                            outgoing,
                            thread_state.clone(),
                            permission_guard,
                        )
                        .await;
                    });
                }
            }
        }
        EventMsg::ExecApprovalRequest(ev) => {
            let permission_guard = thread_watch_manager
                .note_permission_requested(&conversation_id.to_string())
                .await;
            let approval_id_for_op = ev.effective_approval_id();
            let available_decisions = ev
                .effective_available_decisions()
                .into_iter()
                .map(CommandExecutionApprovalDecision::from)
                .collect::<Vec<_>>();
            let ExecApprovalRequestEvent {
                call_id,
                approval_id,
                turn_id,
                command,
                cwd,
                reason,
                network_approval_context,
                proposed_execpolicy_amendment,
                proposed_network_policy_amendments,
                additional_permissions,
                parsed_cmd,
                ..
            } = ev;
            match api_version {
                ApiVersion::V1 => {
                    let params = ExecCommandApprovalParams {
                        conversation_id,
                        call_id: call_id.clone(),
                        approval_id,
                        command,
                        cwd: cwd.to_path_buf(),
                        reason,
                        parsed_cmd,
                    };
                    let (_pending_request_id, rx) = outgoing
                        .send_request(ServerRequestPayload::ExecCommandApproval(params))
                        .await;
                    tokio::spawn(async move {
                        let _permission_guard = permission_guard;
                        on_exec_approval_response(
                            approval_id_for_op,
                            event_turn_id,
                            rx,
                            conversation,
                        )
                        .await;
                    });
                }
                ApiVersion::V2 => {
                    let command_actions = parsed_cmd
                        .iter()
                        .cloned()
                        .map(|parsed| V2ParsedCommand::from_core_with_cwd(parsed, &cwd))
                        .collect::<Vec<_>>();
                    let presentation = if let Some(network_approval_context) =
                        network_approval_context.map(V2NetworkApprovalContext::from)
                    {
                        CommandExecutionApprovalPresentation::Network(network_approval_context)
                    } else {
                        let command_string = shlex_join(&command);
                        let completion_item = CommandExecutionCompletionItem {
                            command: command_string,
                            cwd: cwd.clone(),
                            command_actions: command_actions.clone(),
                        };
                        CommandExecutionApprovalPresentation::Command(completion_item)
                    };
                    let (network_approval_context, command, cwd, command_actions, completion_item) =
                        match presentation {
                            CommandExecutionApprovalPresentation::Network(
                                network_approval_context,
                            ) => (Some(network_approval_context), None, None, None, None),
                            CommandExecutionApprovalPresentation::Command(completion_item) => (
                                None,
                                Some(completion_item.command.clone()),
                                Some(completion_item.cwd.clone()),
                                Some(completion_item.command_actions.clone()),
                                Some(completion_item),
                            ),
                        };
                    if approval_id.is_none()
                        && let Some(completion_item) = completion_item.as_ref()
                    {
                        start_command_execution_item(
                            &conversation_id,
                            event_turn_id.clone(),
                            call_id.clone(),
                            completion_item.command.clone(),
                            completion_item.cwd.clone(),
                            completion_item.command_actions.clone(),
                            CommandExecutionSource::Agent,
                            &outgoing,
                            &thread_state,
                        )
                        .await;
                    }
                    let proposed_execpolicy_amendment_v2 =
                        proposed_execpolicy_amendment.map(V2ExecPolicyAmendment::from);
                    let proposed_network_policy_amendments_v2 = proposed_network_policy_amendments
                        .map(|amendments| {
                            amendments
                                .into_iter()
                                .map(V2NetworkPolicyAmendment::from)
                                .collect()
                        });
                    let additional_permissions =
                        additional_permissions.map(V2AdditionalPermissionProfile::from);

                    let params = CommandExecutionRequestApprovalParams {
                        thread_id: conversation_id.to_string(),
                        turn_id: turn_id.clone(),
                        item_id: call_id.clone(),
                        approval_id: approval_id.clone(),
                        reason,
                        network_approval_context,
                        command,
                        cwd,
                        command_actions,
                        additional_permissions,
                        proposed_execpolicy_amendment: proposed_execpolicy_amendment_v2,
                        proposed_network_policy_amendments: proposed_network_policy_amendments_v2,
                        available_decisions: Some(available_decisions),
                    };
                    let (pending_request_id, rx) = outgoing
                        .send_request(ServerRequestPayload::CommandExecutionRequestApproval(
                            params,
                        ))
                        .await;
                    tokio::spawn(async move {
                        on_command_execution_request_approval_response(
                            event_turn_id,
                            conversation_id,
                            approval_id,
                            call_id,
                            completion_item,
                            pending_request_id,
                            rx,
                            conversation,
                            outgoing,
                            thread_state.clone(),
                            permission_guard,
                        )
                        .await;
                    });
                }
            }
        }
        EventMsg::RequestUserInput(request) => {
            if matches!(api_version, ApiVersion::V2) {
                let user_input_guard = thread_watch_manager
                    .note_user_input_requested(&conversation_id.to_string())
                    .await;
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
                let (pending_request_id, rx) = outgoing
                    .send_request(ServerRequestPayload::ToolRequestUserInput(params))
                    .await;
                tokio::spawn(async move {
                    on_request_user_input_response(
                        event_turn_id,
                        pending_request_id,
                        rx,
                        conversation,
                        thread_state,
                        user_input_guard,
                    )
                    .await;
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
        EventMsg::ElicitationRequest(request) => {
            if matches!(api_version, ApiVersion::V2) {
                let permission_guard = thread_watch_manager
                    .note_permission_requested(&conversation_id.to_string())
                    .await;
                let turn_id = match request.turn_id.clone() {
                    Some(turn_id) => Some(turn_id),
                    None => {
                        let state = thread_state.lock().await;
                        state.active_turn_snapshot().map(|turn| turn.id)
                    }
                };
                let server_name = request.server_name.clone();
                let request_body = match request.request.try_into() {
                    Ok(request_body) => request_body,
                    Err(err) => {
                        error!(
                            error = %err,
                            server_name,
                            request_id = ?request.id,
                            "failed to parse typed MCP elicitation schema"
                        );
                        if let Err(err) = conversation
                            .submit(Op::ResolveElicitation {
                                server_name: request.server_name,
                                request_id: request.id,
                                decision: codex_protocol::approvals::ElicitationAction::Cancel,
                                content: None,
                                meta: None,
                            })
                            .await
                        {
                            error!("failed to submit ResolveElicitation: {err}");
                        }
                        return;
                    }
                };
                let params = McpServerElicitationRequestParams {
                    thread_id: conversation_id.to_string(),
                    turn_id,
                    server_name: request.server_name.clone(),
                    request: request_body,
                };
                let (pending_request_id, rx) = outgoing
                    .send_request(ServerRequestPayload::McpServerElicitationRequest(params))
                    .await;
                tokio::spawn(async move {
                    on_mcp_server_elicitation_response(
                        request.server_name,
                        request.id,
                        pending_request_id,
                        rx,
                        conversation,
                        thread_state,
                        permission_guard,
                    )
                    .await;
                });
            }
        }
        EventMsg::RequestPermissions(request) => {
            if matches!(api_version, ApiVersion::V2) {
                let permission_guard = thread_watch_manager
                    .note_permission_requested(&conversation_id.to_string())
                    .await;
                let requested_permissions = request.permissions.clone();
                let params = PermissionsRequestApprovalParams {
                    thread_id: conversation_id.to_string(),
                    turn_id: request.turn_id.clone(),
                    item_id: request.call_id.clone(),
                    reason: request.reason,
                    permissions: request.permissions.into(),
                };
                let (pending_request_id, rx) = outgoing
                    .send_request(ServerRequestPayload::PermissionsRequestApproval(params))
                    .await;
                tokio::spawn(async move {
                    on_request_permissions_response(
                        request.call_id,
                        requested_permissions,
                        pending_request_id,
                        rx,
                        conversation,
                        thread_state,
                        permission_guard,
                    )
                    .await;
                });
            } else {
                error!(
                    "request_permissions is only supported on api v2 (call_id: {})",
                    request.call_id
                );
                let empty = CoreRequestPermissionsResponse {
                    permissions: Default::default(),
                    scope: CorePermissionGrantScope::Turn,
                };
                if let Err(err) = conversation
                    .submit(Op::RequestPermissionsResponse {
                        id: request.call_id,
                        response: empty,
                    })
                    .await
                {
                    error!("failed to submit RequestPermissionsResponse: {err}");
                }
            }
        }
        EventMsg::DynamicToolCallRequest(request) => {
            if matches!(api_version, ApiVersion::V2) {
                let call_id = request.call_id;
                let turn_id = request.turn_id;
                let tool = request.tool;
                let arguments = request.arguments;
                let item = ThreadItem::DynamicToolCall {
                    id: call_id.clone(),
                    tool: tool.clone(),
                    arguments: arguments.clone(),
                    status: DynamicToolCallStatus::InProgress,
                    content_items: None,
                    success: None,
                    duration_ms: None,
                };
                let notification = ItemStartedNotification {
                    thread_id: conversation_id.to_string(),
                    turn_id: turn_id.clone(),
                    item,
                };
                outgoing
                    .send_server_notification(ServerNotification::ItemStarted(notification))
                    .await;
                let params = DynamicToolCallParams {
                    thread_id: conversation_id.to_string(),
                    turn_id: turn_id.clone(),
                    call_id: call_id.clone(),
                    tool: tool.clone(),
                    arguments: arguments.clone(),
                };
                let (_pending_request_id, rx) = outgoing
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
        EventMsg::DynamicToolCallResponse(response) => {
            if matches!(api_version, ApiVersion::V2) {
                let status = if response.success {
                    DynamicToolCallStatus::Completed
                } else {
                    DynamicToolCallStatus::Failed
                };
                let duration_ms = i64::try_from(response.duration.as_millis()).ok();
                let item = ThreadItem::DynamicToolCall {
                    id: response.call_id,
                    tool: response.tool,
                    arguments: response.arguments,
                    status,
                    content_items: Some(
                        response
                            .content_items
                            .into_iter()
                            .map(|item| match item {
                                CoreDynamicToolCallOutputContentItem::InputText { text } => {
                                    DynamicToolCallOutputContentItem::InputText { text }
                                }
                                CoreDynamicToolCallOutputContentItem::InputImage { image_url } => {
                                    DynamicToolCallOutputContentItem::InputImage { image_url }
                                }
                            })
                            .collect(),
                    ),
                    success: Some(response.success),
                    duration_ms,
                };
                let notification = ItemCompletedNotification {
                    thread_id: conversation_id.to_string(),
                    turn_id: response.turn_id,
                    item,
                };
                outgoing
                    .send_server_notification(ServerNotification::ItemCompleted(notification))
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
                model: Some(begin_event.model),
                reasoning_effort: Some(begin_event.reasoning_effort),
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
                model: Some(end_event.model),
                reasoning_effort: Some(end_event.reasoning_effort),
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
                model: None,
                reasoning_effort: None,
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
                model: None,
                reasoning_effort: None,
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
                model: None,
                reasoning_effort: None,
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
                model: None,
                reasoning_effort: None,
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
                model: None,
                reasoning_effort: None,
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
            if thread_manager
                .get_thread(end_event.receiver_thread_id)
                .await
                .is_err()
            {
                thread_watch_manager
                    .remove_thread(&end_event.receiver_thread_id.to_string())
                    .await;
            }
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
                model: None,
                reasoning_effort: None,
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
        EventMsg::CollabResumeBegin(begin_event) => {
            let item = collab_resume_begin_item(begin_event);
            let notification = ItemStartedNotification {
                thread_id: conversation_id.to_string(),
                turn_id: event_turn_id.clone(),
                item,
            };
            outgoing
                .send_server_notification(ServerNotification::ItemStarted(notification))
                .await;
        }
        EventMsg::CollabResumeEnd(end_event) => {
            let item = collab_resume_end_item(end_event);
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
        EventMsg::ContextCompacted(event) => {
            // Core still fans out this deprecated event for legacy clients;
            // v2 clients receive the canonical ContextCompaction item instead.
            if matches!(api_version, ApiVersion::V2) {
                return;
            }
            let notification = ContextCompactedNotification {
                thread_id: conversation_id.to_string(),
                turn_id: event_turn_id.clone(),
                kind: event.kind.map(Into::into),
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
            thread_watch_manager
                .note_system_error(&conversation_id.to_string())
                .await;

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
                    &thread_state,
                    &outgoing,
                )
                .await;
            };

            if !ev.affects_turn_status() {
                return;
            }

            let turn_error = TurnError {
                message: ev.message,
                codex_error_info: ev.codex_error_info.map(V2CodexErrorInfo::from),
                additional_details: None,
            };
            handle_error(conversation_id, turn_error.clone(), &thread_state).await;
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
                path: view_image_event.path.clone(),
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
        EventMsg::HookStarted(event) => {
            if let ApiVersion::V2 = api_version {
                let notification = HookStartedNotification {
                    thread_id: conversation_id.to_string(),
                    turn_id: event.turn_id,
                    run: event.run.into(),
                };
                outgoing
                    .send_server_notification(ServerNotification::HookStarted(notification))
                    .await;
            }
        }
        EventMsg::HookCompleted(event) => {
            if let ApiVersion::V2 = api_version {
                let notification = HookCompletedNotification {
                    thread_id: conversation_id.to_string(),
                    turn_id: event.turn_id,
                    run: event.run.into(),
                };
                outgoing
                    .send_server_notification(ServerNotification::HookCompleted(notification))
                    .await;
            }
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
            maybe_emit_hook_prompt_item_completed(
                api_version,
                conversation_id,
                &event_turn_id,
                &raw_response_item_event.item,
                &outgoing,
            )
            .await;
            maybe_emit_raw_response_item_completed(
                api_version,
                conversation_id,
                &event_turn_id,
                raw_response_item_event.item,
                &outgoing,
            )
            .await;
        }
        EventMsg::PatchApplyBegin(patch_begin_event) => {
            // Until we migrate the core to be aware of a first class FileChangeItem
            // and emit the corresponding EventMsg, we repurpose the call_id as the item_id.
            let item_id = patch_begin_event.call_id.clone();

            let first_start = {
                let mut state = thread_state.lock().await;
                state
                    .turn_summary
                    .file_change_started
                    .insert(item_id.clone())
            };
            if first_start {
                let item = build_file_change_begin_item(&patch_begin_event);
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
            complete_file_change_item(
                conversation_id,
                item_id,
                build_file_change_end_item(&patch_end_event),
                event_turn_id.clone(),
                &outgoing,
                &thread_state,
            )
            .await;
        }
        EventMsg::ExecCommandBegin(exec_command_begin_event) => {
            if matches!(api_version, ApiVersion::V2)
                && matches!(
                    exec_command_begin_event.source,
                    codex_protocol::protocol::ExecCommandSource::UnifiedExecInteraction
                )
            {
                // TerminalInteraction is the v2 surface for unified exec
                // stdin/poll events. Suppress the legacy CommandExecution
                // item so clients do not render the same wait twice.
                return;
            }
            let item_id = exec_command_begin_event.call_id.clone();
            let cwd = exec_command_begin_event.cwd.clone();
            let command_actions = exec_command_begin_event
                .parsed_cmd
                .into_iter()
                .map(|parsed| V2ParsedCommand::from_core_with_cwd(parsed, &cwd))
                .collect::<Vec<_>>();
            let command = shlex_join(&exec_command_begin_event.command);
            let process_id = exec_command_begin_event.process_id;
            let first_start = {
                let mut state = thread_state.lock().await;
                state
                    .turn_summary
                    .command_execution_started
                    .insert(item_id.clone())
            };
            if first_start {
                let item = ThreadItem::CommandExecution {
                    id: item_id,
                    command,
                    cwd,
                    process_id,
                    source: exec_command_begin_event.source.into(),
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
        }
        EventMsg::ExecCommandOutputDelta(exec_command_output_delta_event) => {
            let item_id = exec_command_output_delta_event.call_id.clone();
            // The underlying EventMsg::ExecCommandOutputDelta is used for shell, unified_exec,
            // and apply_patch tool calls. We represent apply_patch with the FileChange item, and
            // everything else with the CommandExecution item.
            //
            // We need to detect which item type it is so we can emit the right notification.
            // We already have state tracking FileChange items on item/started, so let's use that.
            let is_file_change = {
                let state = thread_state.lock().await;
                state.turn_summary.file_change_started.contains(&item_id)
            };
            if is_file_change {
                let delta =
                    String::from_utf8_lossy(&exec_command_output_delta_event.chunk).to_string();
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
                    delta: String::from_utf8_lossy(&exec_command_output_delta_event.chunk)
                        .to_string(),
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
            let call_id = exec_command_end_event.call_id.clone();
            {
                let mut state = thread_state.lock().await;
                state
                    .turn_summary
                    .command_execution_started
                    .remove(&call_id);
            }
            if matches!(api_version, ApiVersion::V2)
                && matches!(
                    exec_command_end_event.source,
                    codex_protocol::protocol::ExecCommandSource::UnifiedExecInteraction
                )
            {
                // The paired begin event is suppressed above; keep the
                // completion out of v2 as well so no orphan legacy item is
                // emitted for unified exec interactions.
                return;
            }

            let item = build_command_execution_end_item(&exec_command_end_event);

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
            // All per-thread requests are bound to a turn, so abort them.
            outgoing.abort_pending_server_requests().await;
            let pending = {
                let mut state = thread_state.lock().await;
                std::mem::take(&mut state.pending_interrupts)
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

            thread_watch_manager
                .note_turn_interrupted(&conversation_id.to_string())
                .await;
            handle_turn_interrupted(
                conversation_id,
                event_turn_id,
                turn_aborted_event,
                analytics_events_client.as_ref(),
                &outgoing,
                &thread_state,
            )
            .await;
        }
        EventMsg::ThreadRolledBack(_rollback_event) => {
            let pending = {
                let mut state = thread_state.lock().await;
                state.pending_rollbacks.take()
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
                        let fallback_cwd = conversation.config_snapshot().await.cwd;
                        let mut thread = summary_to_thread(summary, &fallback_cwd);
                        match read_rollout_items_from_rollout(rollout_path.as_path()).await {
                            Ok(items) => {
                                thread.turns = build_turns_from_rollout_items(&items);
                                thread.status = thread_watch_manager
                                    .loaded_status_for_thread(&thread.id)
                                    .await;
                                match find_thread_name_by_id(codex_home, &conversation_id).await {
                                    Ok(name) => {
                                        thread.name = name;
                                    }
                                    Err(err) => {
                                        warn!(
                                            "Failed to read thread name for {conversation_id}: {err}"
                                        );
                                    }
                                }
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
                    .send_global_server_notification(ServerNotification::ThreadNameUpdated(
                        notification,
                    ))
                    .await;
            }
        }
        EventMsg::TurnDiff(turn_diff_event) => {
            handle_turn_diff(
                conversation_id,
                &event_turn_id,
                turn_diff_event,
                api_version,
                &outgoing,
            )
            .await;
        }
        EventMsg::PlanUpdate(plan_update_event) => {
            handle_turn_plan_update(
                conversation_id,
                &event_turn_id,
                plan_update_event,
                api_version,
                &outgoing,
            )
            .await;
        }
        EventMsg::ShutdownComplete => {
            thread_watch_manager
                .note_thread_shutdown(&conversation_id.to_string())
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
    outgoing: &ThreadScopedOutgoingMessageSender,
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
    outgoing: &ThreadScopedOutgoingMessageSender,
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

struct TurnCompletionMetadata {
    status: TurnStatus,
    error: Option<TurnError>,
    started_at: Option<i64>,
    completed_at: Option<i64>,
    duration_ms: Option<i64>,
}

async fn emit_turn_completed_with_status(
    conversation_id: ThreadId,
    event_turn_id: String,
    turn_completion_metadata: TurnCompletionMetadata,
    analytics_events_client: Option<&AnalyticsEventsClient>,
    outgoing: &ThreadScopedOutgoingMessageSender,
) {
    let notification = TurnCompletedNotification {
        thread_id: conversation_id.to_string(),
        turn: Turn {
            id: event_turn_id,
            items: vec![],
            error: turn_completion_metadata.error,
            status: turn_completion_metadata.status,
            started_at: turn_completion_metadata.started_at,
            completed_at: turn_completion_metadata.completed_at,
            duration_ms: turn_completion_metadata.duration_ms,
        },
    };
    if let Some(analytics_events_client) = analytics_events_client {
        analytics_events_client
            .track_notification(ServerNotification::TurnCompleted(notification.clone()));
    }
    outgoing
        .send_server_notification(ServerNotification::TurnCompleted(notification))
        .await;
}

async fn complete_file_change_item(
    conversation_id: ThreadId,
    item_id: String,
    item: ThreadItem,
    turn_id: String,
    outgoing: &ThreadScopedOutgoingMessageSender,
    thread_state: &Arc<Mutex<ThreadState>>,
) {
    thread_state
        .lock()
        .await
        .turn_summary
        .file_change_started
        .remove(&item_id);

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
async fn start_command_execution_item(
    conversation_id: &ThreadId,
    turn_id: String,
    item_id: String,
    command: String,
    cwd: AbsolutePathBuf,
    command_actions: Vec<V2ParsedCommand>,
    source: CommandExecutionSource,
    outgoing: &ThreadScopedOutgoingMessageSender,
    thread_state: &Arc<Mutex<ThreadState>>,
) -> bool {
    let first_start = {
        let mut state = thread_state.lock().await;
        state
            .turn_summary
            .command_execution_started
            .insert(item_id.clone())
    };
    if first_start {
        let notification = ItemStartedNotification {
            thread_id: conversation_id.to_string(),
            turn_id,
            item: ThreadItem::CommandExecution {
                id: item_id,
                command,
                cwd,
                process_id: None,
                source,
                status: CommandExecutionStatus::InProgress,
                command_actions,
                aggregated_output: None,
                exit_code: None,
                duration_ms: None,
            },
        };
        outgoing
            .send_server_notification(ServerNotification::ItemStarted(notification))
            .await;
    }
    first_start
}

#[allow(clippy::too_many_arguments)]
async fn complete_command_execution_item(
    conversation_id: &ThreadId,
    turn_id: String,
    item_id: String,
    command: String,
    cwd: AbsolutePathBuf,
    process_id: Option<String>,
    source: CommandExecutionSource,
    command_actions: Vec<V2ParsedCommand>,
    status: CommandExecutionStatus,
    outgoing: &ThreadScopedOutgoingMessageSender,
    thread_state: &Arc<Mutex<ThreadState>>,
) {
    let should_emit = thread_state
        .lock()
        .await
        .turn_summary
        .command_execution_started
        .remove(&item_id);
    if !should_emit {
        return;
    }

    let item = ThreadItem::CommandExecution {
        id: item_id,
        command,
        cwd,
        process_id,
        source,
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
    outgoing: &ThreadScopedOutgoingMessageSender,
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

pub(crate) async fn maybe_emit_hook_prompt_item_completed(
    api_version: ApiVersion,
    conversation_id: ThreadId,
    turn_id: &str,
    item: &codex_protocol::models::ResponseItem,
    outgoing: &ThreadScopedOutgoingMessageSender,
) {
    let ApiVersion::V2 = api_version else {
        return;
    };

    let codex_protocol::models::ResponseItem::Message {
        role, content, id, ..
    } = item
    else {
        return;
    };

    if role != "user" {
        return;
    }

    let Some(hook_prompt) = parse_hook_prompt_message(id.as_ref(), content) else {
        return;
    };

    let notification = ItemCompletedNotification {
        thread_id: conversation_id.to_string(),
        turn_id: turn_id.to_string(),
        item: ThreadItem::HookPrompt {
            id: hook_prompt.id,
            fragments: hook_prompt
                .fragments
                .into_iter()
                .map(codex_app_server_protocol::HookPromptFragment::from)
                .collect(),
        },
    };
    outgoing
        .send_server_notification(ServerNotification::ItemCompleted(notification))
        .await;
}

async fn find_and_remove_turn_summary(
    _conversation_id: ThreadId,
    thread_state: &Arc<Mutex<ThreadState>>,
) -> TurnSummary {
    let mut state = thread_state.lock().await;
    std::mem::take(&mut state.turn_summary)
}

async fn handle_turn_complete(
    conversation_id: ThreadId,
    event_turn_id: String,
    turn_complete_event: TurnCompleteEvent,
    analytics_events_client: Option<&AnalyticsEventsClient>,
    outgoing: &ThreadScopedOutgoingMessageSender,
    thread_state: &Arc<Mutex<ThreadState>>,
) {
    let turn_summary = find_and_remove_turn_summary(conversation_id, thread_state).await;

    let (status, error) = match turn_summary.last_error {
        Some(error) => (TurnStatus::Failed, Some(error)),
        None => (TurnStatus::Completed, None),
    };

    emit_turn_completed_with_status(
        conversation_id,
        event_turn_id,
        TurnCompletionMetadata {
            status,
            error,
            started_at: turn_summary.started_at,
            completed_at: turn_complete_event.completed_at,
            duration_ms: turn_complete_event.duration_ms,
        },
        analytics_events_client,
        outgoing,
    )
    .await;
}

async fn handle_turn_interrupted(
    conversation_id: ThreadId,
    event_turn_id: String,
    turn_aborted_event: TurnAbortedEvent,
    analytics_events_client: Option<&AnalyticsEventsClient>,
    outgoing: &ThreadScopedOutgoingMessageSender,
    thread_state: &Arc<Mutex<ThreadState>>,
) {
    let turn_summary = find_and_remove_turn_summary(conversation_id, thread_state).await;

    emit_turn_completed_with_status(
        conversation_id,
        event_turn_id,
        TurnCompletionMetadata {
            status: TurnStatus::Interrupted,
            error: None,
            started_at: turn_summary.started_at,
            completed_at: turn_aborted_event.completed_at,
            duration_ms: turn_aborted_event.duration_ms,
        },
        analytics_events_client,
        outgoing,
    )
    .await;
}

async fn handle_thread_rollback_failed(
    _conversation_id: ThreadId,
    message: String,
    thread_state: &Arc<Mutex<ThreadState>>,
    outgoing: &ThreadScopedOutgoingMessageSender,
) {
    let pending_rollback = thread_state.lock().await.pending_rollbacks.take();

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
    outgoing: &ThreadScopedOutgoingMessageSender,
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
    _conversation_id: ThreadId,
    error: TurnError,
    thread_state: &Arc<Mutex<ThreadState>>,
) {
    let mut state = thread_state.lock().await;
    state.turn_summary.last_error = Some(error);
}

async fn on_patch_approval_response(
    call_id: String,
    receiver: oneshot::Receiver<ClientRequestResult>,
    codex: Arc<CodexThread>,
) {
    let response = receiver.await;
    let value = match response {
        Ok(Ok(value)) => value,
        Ok(Err(err)) if is_turn_transition_server_request_error(&err) => return,
        Ok(Err(err)) => {
            error!("request failed with client error: {err:?}");
            if let Err(submit_err) = codex
                .submit(Op::PatchApproval {
                    id: call_id.clone(),
                    decision: ReviewDecision::Denied,
                })
                .await
            {
                error!("failed to submit denied PatchApproval after request failure: {submit_err}");
            }
            return;
        }
        Err(err) => {
            error!("request failed: {err:?}");
            if let Err(submit_err) = codex
                .submit(Op::PatchApproval {
                    id: call_id.clone(),
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
            id: call_id,
            decision: response.decision,
        })
        .await
    {
        error!("failed to submit PatchApproval: {err}");
    }
}

async fn on_exec_approval_response(
    call_id: String,
    turn_id: String,
    receiver: oneshot::Receiver<ClientRequestResult>,
    conversation: Arc<CodexThread>,
) {
    let response = receiver.await;
    let value = match response {
        Ok(Ok(value)) => value,
        Ok(Err(err)) if is_turn_transition_server_request_error(&err) => return,
        Ok(Err(err)) => {
            error!("request failed with client error: {err:?}");
            return;
        }
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
            id: call_id,
            turn_id: Some(turn_id),
            decision: response.decision,
        })
        .await
    {
        error!("failed to submit ExecApproval: {err}");
    }
}

async fn on_request_user_input_response(
    event_turn_id: String,
    pending_request_id: RequestId,
    receiver: oneshot::Receiver<ClientRequestResult>,
    conversation: Arc<CodexThread>,
    thread_state: Arc<Mutex<ThreadState>>,
    user_input_guard: ThreadWatchActiveGuard,
) {
    let response = receiver.await;
    resolve_server_request_on_thread_listener(&thread_state, pending_request_id).await;
    drop(user_input_guard);
    let value = match response {
        Ok(Ok(value)) => value,
        Ok(Err(err)) if is_turn_transition_server_request_error(&err) => return,
        Ok(Err(err)) => {
            error!("request failed with client error: {err:?}");
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

async fn on_mcp_server_elicitation_response(
    server_name: String,
    request_id: codex_protocol::mcp::RequestId,
    pending_request_id: RequestId,
    receiver: oneshot::Receiver<ClientRequestResult>,
    conversation: Arc<CodexThread>,
    thread_state: Arc<Mutex<ThreadState>>,
    permission_guard: ThreadWatchActiveGuard,
) {
    let response = receiver.await;
    resolve_server_request_on_thread_listener(&thread_state, pending_request_id).await;
    drop(permission_guard);
    let response = mcp_server_elicitation_response_from_client_result(response);

    if let Err(err) = conversation
        .submit(Op::ResolveElicitation {
            server_name,
            request_id,
            decision: response.action.to_core(),
            content: response.content,
            meta: response.meta,
        })
        .await
    {
        error!("failed to submit ResolveElicitation: {err}");
    }
}

fn mcp_server_elicitation_response_from_client_result(
    response: std::result::Result<ClientRequestResult, oneshot::error::RecvError>,
) -> McpServerElicitationRequestResponse {
    match response {
        Ok(Ok(value)) => serde_json::from_value::<McpServerElicitationRequestResponse>(value)
            .unwrap_or_else(|err| {
                error!("failed to deserialize McpServerElicitationRequestResponse: {err}");
                McpServerElicitationRequestResponse {
                    action: McpServerElicitationAction::Decline,
                    content: None,
                    meta: None,
                }
            }),
        Ok(Err(err)) if is_turn_transition_server_request_error(&err) => {
            McpServerElicitationRequestResponse {
                action: McpServerElicitationAction::Cancel,
                content: None,
                meta: None,
            }
        }
        Ok(Err(err)) => {
            error!("request failed with client error: {err:?}");
            McpServerElicitationRequestResponse {
                action: McpServerElicitationAction::Decline,
                content: None,
                meta: None,
            }
        }
        Err(err) => {
            error!("request failed: {err:?}");
            McpServerElicitationRequestResponse {
                action: McpServerElicitationAction::Decline,
                content: None,
                meta: None,
            }
        }
    }
}

async fn on_request_permissions_response(
    call_id: String,
    requested_permissions: CoreRequestPermissionProfile,
    pending_request_id: RequestId,
    receiver: oneshot::Receiver<ClientRequestResult>,
    conversation: Arc<CodexThread>,
    thread_state: Arc<Mutex<ThreadState>>,
    request_permissions_guard: ThreadWatchActiveGuard,
) {
    let response = receiver.await;
    resolve_server_request_on_thread_listener(&thread_state, pending_request_id).await;
    drop(request_permissions_guard);
    let Some(response) =
        request_permissions_response_from_client_result(requested_permissions, response)
    else {
        return;
    };

    if let Err(err) = conversation
        .submit(Op::RequestPermissionsResponse {
            id: call_id,
            response,
        })
        .await
    {
        error!("failed to submit RequestPermissionsResponse: {err}");
    }
}

fn request_permissions_response_from_client_result(
    requested_permissions: CoreRequestPermissionProfile,
    response: std::result::Result<ClientRequestResult, oneshot::error::RecvError>,
) -> Option<CoreRequestPermissionsResponse> {
    let value = match response {
        Ok(Ok(value)) => value,
        Ok(Err(err)) if is_turn_transition_server_request_error(&err) => return None,
        Ok(Err(err)) => {
            error!("request failed with client error: {err:?}");
            return Some(CoreRequestPermissionsResponse {
                permissions: Default::default(),
                scope: CorePermissionGrantScope::Turn,
            });
        }
        Err(err) => {
            error!("request failed: {err:?}");
            return Some(CoreRequestPermissionsResponse {
                permissions: Default::default(),
                scope: CorePermissionGrantScope::Turn,
            });
        }
    };

    let response = serde_json::from_value::<PermissionsRequestApprovalResponse>(value)
        .unwrap_or_else(|err| {
            error!("failed to deserialize PermissionsRequestApprovalResponse: {err}");
            PermissionsRequestApprovalResponse {
                permissions: V2GrantedPermissionProfile::default(),
                scope: codex_app_server_protocol::PermissionGrantScope::Turn,
            }
        });
    Some(CoreRequestPermissionsResponse {
        permissions: intersect_permission_profiles(
            requested_permissions.into(),
            response.permissions.into(),
        )
        .into(),
        scope: response.scope.to_core(),
    })
}

const REVIEW_FALLBACK_MESSAGE: &str = "Reviewer failed to output a response.";

fn render_review_output_text(output: &ReviewOutputEvent) -> String {
    let mut sections = Vec::new();
    let explanation = output.overall_explanation.trim();
    if !explanation.is_empty() {
        sections.push(explanation.to_string());
    }
    if !output.findings.is_empty() {
        let findings = format_review_findings_block(&output.findings, /*selection*/ None);
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
    pending_request_id: RequestId,
    receiver: oneshot::Receiver<ClientRequestResult>,
    codex: Arc<CodexThread>,
    outgoing: ThreadScopedOutgoingMessageSender,
    thread_state: Arc<Mutex<ThreadState>>,
    permission_guard: ThreadWatchActiveGuard,
) {
    let response = receiver.await;
    resolve_server_request_on_thread_listener(&thread_state, pending_request_id).await;
    drop(permission_guard);
    let (decision, completion_status) = match response {
        Ok(Ok(value)) => {
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
        Ok(Err(err)) if is_turn_transition_server_request_error(&err) => return,
        Ok(Err(err)) => {
            error!("request failed with client error: {err:?}");
            (ReviewDecision::Denied, Some(PatchApplyStatus::Failed))
        }
        Err(err) => {
            error!("request failed: {err:?}");
            (ReviewDecision::Denied, Some(PatchApplyStatus::Failed))
        }
    };

    if let Some(status) = completion_status {
        complete_file_change_item(
            conversation_id,
            item_id.clone(),
            ThreadItem::FileChange {
                id: item_id.clone(),
                changes,
                status,
            },
            event_turn_id.clone(),
            &outgoing,
            &thread_state,
        )
        .await;
    }

    if let Err(err) = codex
        .submit(Op::PatchApproval {
            id: item_id,
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
    approval_id: Option<String>,
    item_id: String,
    completion_item: Option<CommandExecutionCompletionItem>,
    pending_request_id: RequestId,
    receiver: oneshot::Receiver<ClientRequestResult>,
    conversation: Arc<CodexThread>,
    outgoing: ThreadScopedOutgoingMessageSender,
    thread_state: Arc<Mutex<ThreadState>>,
    permission_guard: ThreadWatchActiveGuard,
) {
    let response = receiver.await;
    resolve_server_request_on_thread_listener(&thread_state, pending_request_id).await;
    drop(permission_guard);
    let (decision, completion_status) = match response {
        Ok(Ok(value)) => {
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
                CommandExecutionApprovalDecision::ApplyNetworkPolicyAmendment {
                    network_policy_amendment,
                } => {
                    let completion_status = match network_policy_amendment.action {
                        V2NetworkPolicyRuleAction::Allow => None,
                        V2NetworkPolicyRuleAction::Deny => Some(CommandExecutionStatus::Declined),
                    };
                    (
                        ReviewDecision::NetworkPolicyAmendment {
                            network_policy_amendment: network_policy_amendment.into_core(),
                        },
                        completion_status,
                    )
                }
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
        Ok(Err(err)) if is_turn_transition_server_request_error(&err) => return,
        Ok(Err(err)) => {
            error!("request failed with client error: {err:?}");
            (ReviewDecision::Denied, Some(CommandExecutionStatus::Failed))
        }
        Err(err) => {
            error!("request failed: {err:?}");
            (ReviewDecision::Denied, Some(CommandExecutionStatus::Failed))
        }
    };

    let suppress_subcommand_completion_item = {
        // For regular shell/unified_exec approvals, approval_id is null.
        // For zsh-fork subcommand approvals, approval_id is present and
        // item_id points to the parent command item.
        if approval_id.is_some() {
            let state = thread_state.lock().await;
            state
                .turn_summary
                .command_execution_started
                .contains(&item_id)
        } else {
            false
        }
    };

    if let Some(status) = completion_status
        && !suppress_subcommand_completion_item
        && let Some(completion_item) = completion_item
    {
        complete_command_execution_item(
            &conversation_id,
            event_turn_id.clone(),
            item_id.clone(),
            completion_item.command,
            completion_item.cwd,
            /*process_id*/ None,
            CommandExecutionSource::Agent,
            completion_item.command_actions,
            status,
            &outgoing,
            &thread_state,
        )
        .await;
    }

    if let Err(err) = conversation
        .submit(Op::ExecApproval {
            id: approval_id.unwrap_or_else(|| item_id.clone()),
            turn_id: Some(event_turn_id),
            decision,
        })
        .await
    {
        error!("failed to submit ExecApproval: {err}");
    }
}

fn collab_resume_begin_item(
    begin_event: codex_protocol::protocol::CollabResumeBeginEvent,
) -> ThreadItem {
    ThreadItem::CollabAgentToolCall {
        id: begin_event.call_id,
        tool: CollabAgentTool::ResumeAgent,
        status: V2CollabToolCallStatus::InProgress,
        sender_thread_id: begin_event.sender_thread_id.to_string(),
        receiver_thread_ids: vec![begin_event.receiver_thread_id.to_string()],
        prompt: None,
        model: None,
        reasoning_effort: None,
        agents_states: HashMap::new(),
    }
}

fn collab_resume_end_item(end_event: codex_protocol::protocol::CollabResumeEndEvent) -> ThreadItem {
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
    ThreadItem::CollabAgentToolCall {
        id: end_event.call_id,
        tool: CollabAgentTool::ResumeAgent,
        status,
        sender_thread_id: end_event.sender_thread_id.to_string(),
        receiver_thread_ids: vec![receiver_id],
        prompt: None,
        model: None,
        reasoning_effort: None,
        agents_states,
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
        mcp_app_resource_uri: begin_event.mcp_app_resource_uri,
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
            Some(Box::new(McpToolCallResult {
                content: value.content.clone(),
                structured_content: value.structured_content.clone(),
                meta: value.meta.clone(),
            })),
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
        mcp_app_resource_uri: end_event.mcp_app_resource_uri,
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
    use crate::outgoing_message::ConnectionId;
    use crate::outgoing_message::OutgoingEnvelope;
    use crate::outgoing_message::OutgoingMessage;
    use crate::outgoing_message::OutgoingMessageSender;
    use anyhow::Result;
    use anyhow::anyhow;
    use anyhow::bail;
    use codex_app_server_protocol::AutoReviewDecisionSource;
    use codex_app_server_protocol::GuardianApprovalReviewStatus;
    use codex_app_server_protocol::JSONRPCErrorError;
    use codex_app_server_protocol::TurnPlanStepStatus;
    use codex_login::AuthManager;
    use codex_login::CodexAuth;
    use codex_protocol::items::HookPromptFragment;
    use codex_protocol::items::build_hook_prompt_message;
    use codex_protocol::mcp::CallToolResult;
    use codex_protocol::models::FileSystemPermissions as CoreFileSystemPermissions;
    use codex_protocol::models::NetworkPermissions as CoreNetworkPermissions;
    use codex_protocol::plan_tool::PlanItemArg;
    use codex_protocol::plan_tool::StepStatus;
    use codex_protocol::protocol::CollabResumeBeginEvent;
    use codex_protocol::protocol::CollabResumeEndEvent;
    use codex_protocol::protocol::CreditsSnapshot;
    use codex_protocol::protocol::GuardianAssessmentEvent;
    use codex_protocol::protocol::GuardianAssessmentStatus;
    use codex_protocol::protocol::McpInvocation;
    use codex_protocol::protocol::RateLimitSnapshot;
    use codex_protocol::protocol::RateLimitWindow;
    use codex_protocol::protocol::TokenUsage;
    use codex_protocol::protocol::TokenUsageInfo;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use codex_utils_absolute_path::test_support::PathBufExt;
    use codex_utils_absolute_path::test_support::test_path_buf;
    use core_test_support::load_default_config_for_test;
    use pretty_assertions::assert_eq;
    use rmcp::model::Content;
    use serde_json::Value as JsonValue;
    use serde_json::json;
    use std::path::PathBuf;
    use std::time::Duration;
    use tempfile::TempDir;
    use tokio::sync::Mutex;
    use tokio::sync::mpsc;

    fn new_thread_state() -> Arc<Mutex<ThreadState>> {
        Arc::new(Mutex::new(ThreadState::default()))
    }

    const TEST_TURN_COMPLETED_AT: i64 = 1_716_000_456;
    const TEST_TURN_DURATION_MS: i64 = 1_234;

    async fn recv_broadcast_message(
        rx: &mut mpsc::Receiver<OutgoingEnvelope>,
    ) -> Result<OutgoingMessage> {
        let envelope = rx
            .recv()
            .await
            .ok_or_else(|| anyhow!("should send one message"))?;
        match envelope {
            OutgoingEnvelope::Broadcast { message } => Ok(message),
            OutgoingEnvelope::ToConnection { message, .. } => Ok(message),
        }
    }

    fn turn_complete_event(turn_id: &str) -> TurnCompleteEvent {
        TurnCompleteEvent {
            turn_id: turn_id.to_string(),
            last_agent_message: None,
            completed_at: Some(TEST_TURN_COMPLETED_AT),
            duration_ms: Some(TEST_TURN_DURATION_MS),
        }
    }

    fn turn_aborted_event(turn_id: &str) -> TurnAbortedEvent {
        TurnAbortedEvent {
            turn_id: Some(turn_id.to_string()),
            reason: codex_protocol::protocol::TurnAbortReason::Interrupted,
            completed_at: Some(TEST_TURN_COMPLETED_AT),
            duration_ms: Some(TEST_TURN_DURATION_MS),
        }
    }

    fn command_execution_completion_item(command: &str) -> CommandExecutionCompletionItem {
        CommandExecutionCompletionItem {
            command: command.to_string(),
            cwd: test_path_buf("/tmp").abs(),
            command_actions: vec![V2ParsedCommand::Unknown {
                command: command.to_string(),
            }],
        }
    }

    fn guardian_command_assessment(
        id: &str,
        turn_id: &str,
        status: GuardianAssessmentStatus,
    ) -> GuardianAssessmentEvent {
        let (risk_level, user_authorization, rationale) = match status {
            GuardianAssessmentStatus::InProgress => (None, None, None),
            GuardianAssessmentStatus::Approved => (
                Some(codex_protocol::protocol::GuardianRiskLevel::Low),
                Some(codex_protocol::protocol::GuardianUserAuthorization::High),
                Some("looks safe".to_string()),
            ),
            GuardianAssessmentStatus::Denied => (
                Some(codex_protocol::protocol::GuardianRiskLevel::High),
                Some(codex_protocol::protocol::GuardianUserAuthorization::Low),
                Some("too risky".to_string()),
            ),
            GuardianAssessmentStatus::TimedOut => {
                (None, None, Some("review timed out".to_string()))
            }
            GuardianAssessmentStatus::Aborted => (None, None, None),
        };
        GuardianAssessmentEvent {
            id: format!("review-{id}"),
            target_item_id: Some(id.to_string()),
            turn_id: turn_id.to_string(),
            status,
            risk_level,
            user_authorization,
            rationale,
            decision_source: if matches!(status, GuardianAssessmentStatus::InProgress) {
                None
            } else {
                Some(codex_protocol::protocol::GuardianAssessmentDecisionSource::Agent)
            },
            action: serde_json::from_value(json!({
                "type": "command",
                "source": "shell",
                "command": format!("rm -f /tmp/{id}.sqlite"),
                "cwd": test_path_buf("/tmp"),
            }))
            .expect("guardian action"),
        }
    }

    struct GuardianAssessmentTestContext {
        conversation_id: ThreadId,
        conversation: Arc<CodexThread>,
        thread_manager: Arc<ThreadManager>,
        outgoing: ThreadScopedOutgoingMessageSender,
        thread_state: Arc<Mutex<ThreadState>>,
        thread_watch_manager: ThreadWatchManager,
        analytics_events_client: AnalyticsEventsClient,
        codex_home: PathBuf,
    }

    impl GuardianAssessmentTestContext {
        async fn apply_guardian_assessment_event(&self, assessment: GuardianAssessmentEvent) {
            let event_turn_id = assessment.turn_id.clone();
            apply_bespoke_event_handling(
                Event {
                    id: event_turn_id,
                    msg: EventMsg::GuardianAssessment(assessment),
                },
                self.conversation_id,
                self.conversation.clone(),
                self.thread_manager.clone(),
                Some(self.analytics_events_client.clone()),
                self.outgoing.clone(),
                self.thread_state.clone(),
                self.thread_watch_manager.clone(),
                ApiVersion::V2,
                "test-provider".to_string(),
                &self.codex_home,
            )
            .await;
        }
    }

    #[test]
    fn guardian_assessment_started_uses_event_turn_id_fallback() {
        let conversation_id = ThreadId::new();
        let action = codex_protocol::protocol::GuardianAssessmentAction::Command {
            source: codex_protocol::protocol::GuardianCommandSource::Shell,
            command: "rm -rf /tmp/example.sqlite".to_string(),
            cwd: test_path_buf("/tmp").abs(),
        };
        let notification = guardian_auto_approval_review_notification(
            &conversation_id,
            "turn-from-event",
            &GuardianAssessmentEvent {
                id: "review-1".to_string(),
                target_item_id: Some("item-1".to_string()),
                turn_id: String::new(),
                status: codex_protocol::protocol::GuardianAssessmentStatus::InProgress,
                risk_level: None,
                user_authorization: None,
                rationale: None,
                decision_source: None,
                action: action.clone(),
            },
        );

        match notification {
            ServerNotification::ItemGuardianApprovalReviewStarted(payload) => {
                assert_eq!(payload.thread_id, conversation_id.to_string());
                assert_eq!(payload.turn_id, "turn-from-event");
                assert_eq!(payload.review_id, "review-1");
                assert_eq!(payload.target_item_id.as_deref(), Some("item-1"));
                assert_eq!(
                    payload.review.status,
                    GuardianApprovalReviewStatus::InProgress
                );
                assert_eq!(payload.review.risk_level, None);
                assert_eq!(payload.review.user_authorization, None);
                assert_eq!(payload.review.rationale, None);
                assert_eq!(payload.action, action.into());
            }
            other => panic!("unexpected notification: {other:?}"),
        }
    }

    #[test]
    fn guardian_assessment_completed_emits_review_payload() {
        let conversation_id = ThreadId::new();
        let action = codex_protocol::protocol::GuardianAssessmentAction::Command {
            source: codex_protocol::protocol::GuardianCommandSource::Shell,
            command: "rm -rf /tmp/example.sqlite".to_string(),
            cwd: test_path_buf("/tmp").abs(),
        };
        let notification = guardian_auto_approval_review_notification(
            &conversation_id,
            "turn-from-event",
            &GuardianAssessmentEvent {
                id: "review-2".to_string(),
                target_item_id: Some("item-2".to_string()),
                turn_id: "turn-from-assessment".to_string(),
                status: codex_protocol::protocol::GuardianAssessmentStatus::Denied,
                risk_level: Some(codex_protocol::protocol::GuardianRiskLevel::High),
                user_authorization: Some(codex_protocol::protocol::GuardianUserAuthorization::Low),
                rationale: Some("too risky".to_string()),
                decision_source: Some(
                    codex_protocol::protocol::GuardianAssessmentDecisionSource::Agent,
                ),
                action: action.clone(),
            },
        );

        match notification {
            ServerNotification::ItemGuardianApprovalReviewCompleted(payload) => {
                assert_eq!(payload.thread_id, conversation_id.to_string());
                assert_eq!(payload.turn_id, "turn-from-assessment");
                assert_eq!(payload.review_id, "review-2");
                assert_eq!(payload.target_item_id.as_deref(), Some("item-2"));
                assert_eq!(payload.decision_source, AutoReviewDecisionSource::Agent);
                assert_eq!(payload.review.status, GuardianApprovalReviewStatus::Denied);
                assert_eq!(
                    payload.review.risk_level,
                    Some(codex_app_server_protocol::GuardianRiskLevel::High)
                );
                assert_eq!(
                    payload.review.user_authorization,
                    Some(codex_app_server_protocol::GuardianUserAuthorization::Low)
                );
                assert_eq!(payload.review.rationale.as_deref(), Some("too risky"));
                assert_eq!(payload.action, action.into());
            }
            other => panic!("unexpected notification: {other:?}"),
        }
    }

    #[test]
    fn guardian_assessment_aborted_emits_completed_review_payload() {
        let conversation_id = ThreadId::new();
        let action = codex_protocol::protocol::GuardianAssessmentAction::NetworkAccess {
            target: "api.openai.com:443".to_string(),
            host: "api.openai.com".to_string(),
            protocol: codex_protocol::protocol::NetworkApprovalProtocol::Https,
            port: 443,
        };
        let notification = guardian_auto_approval_review_notification(
            &conversation_id,
            "turn-from-event",
            &GuardianAssessmentEvent {
                id: "review-3".to_string(),
                target_item_id: None,
                turn_id: "turn-from-assessment".to_string(),
                status: codex_protocol::protocol::GuardianAssessmentStatus::Aborted,
                risk_level: None,
                user_authorization: None,
                rationale: None,
                decision_source: Some(
                    codex_protocol::protocol::GuardianAssessmentDecisionSource::Agent,
                ),
                action: action.clone(),
            },
        );

        match notification {
            ServerNotification::ItemGuardianApprovalReviewCompleted(payload) => {
                assert_eq!(payload.thread_id, conversation_id.to_string());
                assert_eq!(payload.turn_id, "turn-from-assessment");
                assert_eq!(payload.review_id, "review-3");
                assert_eq!(payload.target_item_id, None);
                assert_eq!(payload.decision_source, AutoReviewDecisionSource::Agent);
                assert_eq!(payload.review.status, GuardianApprovalReviewStatus::Aborted);
                assert_eq!(payload.review.risk_level, None);
                assert_eq!(payload.review.user_authorization, None);
                assert_eq!(payload.review.rationale, None);
                assert_eq!(payload.action, action.into());
            }
            other => panic!("unexpected notification: {other:?}"),
        }
    }

    #[tokio::test]
    async fn command_execution_started_helper_emits_once() -> Result<()> {
        let conversation_id = ThreadId::new();
        let thread_state = new_thread_state();
        let (tx, mut rx) = mpsc::channel(CHANNEL_CAPACITY);
        let outgoing = Arc::new(OutgoingMessageSender::new(tx));
        let outgoing = ThreadScopedOutgoingMessageSender::new(
            outgoing,
            vec![ConnectionId(1)],
            ThreadId::new(),
        );
        let completion_item = command_execution_completion_item("printf hi");

        let first_start = start_command_execution_item(
            &conversation_id,
            "turn-1".to_string(),
            "cmd-1".to_string(),
            completion_item.command.clone(),
            completion_item.cwd.clone(),
            completion_item.command_actions.clone(),
            CommandExecutionSource::Agent,
            &outgoing,
            &thread_state,
        )
        .await;
        assert!(first_start);

        let msg = recv_broadcast_message(&mut rx).await?;
        match msg {
            OutgoingMessage::AppServerNotification(ServerNotification::ItemStarted(payload)) => {
                assert_eq!(payload.thread_id, conversation_id.to_string());
                assert_eq!(payload.turn_id, "turn-1");
                assert_eq!(
                    payload.item,
                    ThreadItem::CommandExecution {
                        id: "cmd-1".to_string(),
                        command: completion_item.command.clone(),
                        cwd: completion_item.cwd.clone(),
                        process_id: None,
                        source: CommandExecutionSource::Agent,
                        status: CommandExecutionStatus::InProgress,
                        command_actions: completion_item.command_actions.clone(),
                        aggregated_output: None,
                        exit_code: None,
                        duration_ms: None,
                    }
                );
            }
            other => bail!("unexpected message: {other:?}"),
        }

        let second_start = start_command_execution_item(
            &conversation_id,
            "turn-1".to_string(),
            "cmd-1".to_string(),
            completion_item.command.clone(),
            completion_item.cwd.clone(),
            completion_item.command_actions.clone(),
            CommandExecutionSource::Agent,
            &outgoing,
            &thread_state,
        )
        .await;
        assert!(!second_start);
        assert!(rx.try_recv().is_err(), "duplicate start should not emit");
        Ok(())
    }

    #[tokio::test]
    async fn complete_command_execution_item_emits_declined_once_for_pending_command() -> Result<()>
    {
        let conversation_id = ThreadId::new();
        let thread_state = new_thread_state();
        let (tx, mut rx) = mpsc::channel(CHANNEL_CAPACITY);
        let outgoing = Arc::new(OutgoingMessageSender::new(tx));
        let outgoing = ThreadScopedOutgoingMessageSender::new(
            outgoing,
            vec![ConnectionId(1)],
            ThreadId::new(),
        );
        let completion_item = command_execution_completion_item("printf hi");

        start_command_execution_item(
            &conversation_id,
            "turn-1".to_string(),
            "cmd-1".to_string(),
            completion_item.command.clone(),
            completion_item.cwd.clone(),
            completion_item.command_actions.clone(),
            CommandExecutionSource::Agent,
            &outgoing,
            &thread_state,
        )
        .await;
        let _started = recv_broadcast_message(&mut rx).await?;

        complete_command_execution_item(
            &conversation_id,
            "turn-1".to_string(),
            "cmd-1".to_string(),
            completion_item.command.clone(),
            completion_item.cwd.clone(),
            /*process_id*/ None,
            CommandExecutionSource::Agent,
            completion_item.command_actions.clone(),
            CommandExecutionStatus::Declined,
            &outgoing,
            &thread_state,
        )
        .await;

        let completed = recv_broadcast_message(&mut rx).await?;
        match completed {
            OutgoingMessage::AppServerNotification(ServerNotification::ItemCompleted(payload)) => {
                let ThreadItem::CommandExecution { id, status, .. } = payload.item else {
                    bail!("expected command execution completion");
                };
                assert_eq!(id, "cmd-1");
                assert_eq!(status, CommandExecutionStatus::Declined);
            }
            other => bail!("unexpected message: {other:?}"),
        }

        complete_command_execution_item(
            &conversation_id,
            "turn-1".to_string(),
            "cmd-1".to_string(),
            completion_item.command,
            completion_item.cwd,
            /*process_id*/ None,
            CommandExecutionSource::Agent,
            completion_item.command_actions,
            CommandExecutionStatus::Declined,
            &outgoing,
            &thread_state,
        )
        .await;
        assert!(
            rx.try_recv().is_err(),
            "completion should not emit after the pending item is cleared"
        );
        Ok(())
    }

    #[tokio::test]
    async fn guardian_command_execution_notifications_wrap_review_lifecycle() -> Result<()> {
        let codex_home = TempDir::new()?;
        let config = load_default_config_for_test(&codex_home).await;
        let thread_manager = Arc::new(
            codex_core::test_support::thread_manager_with_models_provider_and_home(
                CodexAuth::create_dummy_chatgpt_auth_for_testing(),
                config.model_provider.clone(),
                config.codex_home.to_path_buf(),
                Arc::new(codex_exec_server::EnvironmentManager::new(
                    /*exec_server_url*/ None,
                )),
            ),
        );
        let codex_core::NewThread {
            thread_id: conversation_id,
            thread: conversation,
            ..
        } = thread_manager.start_thread(config).await?;
        let thread_state = new_thread_state();
        let thread_watch_manager = ThreadWatchManager::new();
        let (tx, mut rx) = mpsc::channel(CHANNEL_CAPACITY);
        let outgoing = Arc::new(OutgoingMessageSender::new(tx));
        let outgoing = ThreadScopedOutgoingMessageSender::new(
            outgoing,
            vec![ConnectionId(1)],
            conversation_id,
        );
        let guardian_context = GuardianAssessmentTestContext {
            conversation_id,
            conversation: conversation.clone(),
            thread_manager: thread_manager.clone(),
            outgoing: outgoing.clone(),
            thread_state: thread_state.clone(),
            thread_watch_manager: thread_watch_manager.clone(),
            analytics_events_client: AnalyticsEventsClient::new(
                AuthManager::from_auth_for_testing(
                    CodexAuth::create_dummy_chatgpt_auth_for_testing(),
                ),
                "http://localhost".to_string(),
                Some(false),
            ),
            codex_home: codex_home.path().to_path_buf(),
        };

        guardian_context
            .apply_guardian_assessment_event(guardian_command_assessment(
                "cmd-guardian-approved",
                "turn-guardian-approved",
                GuardianAssessmentStatus::InProgress,
            ))
            .await;
        let first = recv_broadcast_message(&mut rx).await?;
        match first {
            OutgoingMessage::AppServerNotification(ServerNotification::ItemStarted(payload)) => {
                assert_eq!(payload.turn_id, "turn-guardian-approved");
                let ThreadItem::CommandExecution { id, status, .. } = payload.item else {
                    bail!("expected command execution item");
                };
                assert_eq!(id, "cmd-guardian-approved");
                assert_eq!(status, CommandExecutionStatus::InProgress);
            }
            other => bail!("unexpected message: {other:?}"),
        }
        let second = recv_broadcast_message(&mut rx).await?;
        match second {
            OutgoingMessage::AppServerNotification(
                ServerNotification::ItemGuardianApprovalReviewStarted(payload),
            ) => {
                assert_eq!(payload.review_id, "review-cmd-guardian-approved");
                assert_eq!(
                    payload.target_item_id.as_deref(),
                    Some("cmd-guardian-approved")
                );
                assert_eq!(
                    payload.review.status,
                    GuardianApprovalReviewStatus::InProgress
                );
            }
            other => bail!("unexpected message: {other:?}"),
        }

        guardian_context
            .apply_guardian_assessment_event(guardian_command_assessment(
                "cmd-guardian-approved",
                "turn-guardian-approved",
                GuardianAssessmentStatus::Approved,
            ))
            .await;
        let third = recv_broadcast_message(&mut rx).await?;
        match third {
            OutgoingMessage::AppServerNotification(
                ServerNotification::ItemGuardianApprovalReviewCompleted(payload),
            ) => {
                assert_eq!(payload.review_id, "review-cmd-guardian-approved");
                assert_eq!(
                    payload.target_item_id.as_deref(),
                    Some("cmd-guardian-approved")
                );
                assert_eq!(payload.decision_source, AutoReviewDecisionSource::Agent);
                assert_eq!(
                    payload.review.status,
                    GuardianApprovalReviewStatus::Approved
                );
            }
            other => bail!("unexpected message: {other:?}"),
        }
        assert!(
            rx.try_recv().is_err(),
            "approved review should not complete the command item"
        );

        guardian_context
            .apply_guardian_assessment_event(guardian_command_assessment(
                "cmd-guardian-denied",
                "turn-guardian-denied",
                GuardianAssessmentStatus::InProgress,
            ))
            .await;
        let fourth = recv_broadcast_message(&mut rx).await?;
        match fourth {
            OutgoingMessage::AppServerNotification(ServerNotification::ItemStarted(payload)) => {
                assert_eq!(payload.turn_id, "turn-guardian-denied");
                let ThreadItem::CommandExecution { id, status, .. } = payload.item else {
                    bail!("expected command execution item");
                };
                assert_eq!(id, "cmd-guardian-denied");
                assert_eq!(status, CommandExecutionStatus::InProgress);
            }
            other => bail!("unexpected message: {other:?}"),
        }
        let fifth = recv_broadcast_message(&mut rx).await?;
        match fifth {
            OutgoingMessage::AppServerNotification(
                ServerNotification::ItemGuardianApprovalReviewStarted(payload),
            ) => {
                assert_eq!(payload.review_id, "review-cmd-guardian-denied");
                assert_eq!(
                    payload.target_item_id.as_deref(),
                    Some("cmd-guardian-denied")
                );
                assert_eq!(
                    payload.review.status,
                    GuardianApprovalReviewStatus::InProgress
                );
            }
            other => bail!("unexpected message: {other:?}"),
        }

        guardian_context
            .apply_guardian_assessment_event(guardian_command_assessment(
                "cmd-guardian-denied",
                "turn-guardian-denied",
                GuardianAssessmentStatus::Denied,
            ))
            .await;
        let sixth = recv_broadcast_message(&mut rx).await?;
        match sixth {
            OutgoingMessage::AppServerNotification(
                ServerNotification::ItemGuardianApprovalReviewCompleted(payload),
            ) => {
                assert_eq!(payload.review_id, "review-cmd-guardian-denied");
                assert_eq!(
                    payload.target_item_id.as_deref(),
                    Some("cmd-guardian-denied")
                );
                assert_eq!(payload.decision_source, AutoReviewDecisionSource::Agent);
                assert_eq!(payload.review.status, GuardianApprovalReviewStatus::Denied);
            }
            other => bail!("unexpected message: {other:?}"),
        }
        let seventh = recv_broadcast_message(&mut rx).await?;
        match seventh {
            OutgoingMessage::AppServerNotification(ServerNotification::ItemCompleted(payload)) => {
                let ThreadItem::CommandExecution { id, status, .. } = payload.item else {
                    bail!("expected command execution completion");
                };
                assert_eq!(id, "cmd-guardian-denied");
                assert_eq!(status, CommandExecutionStatus::Declined);
            }
            other => bail!("unexpected message: {other:?}"),
        }

        let mut missing_target = guardian_command_assessment(
            "cmd-guardian-missing-target",
            "turn-guardian-missing-target",
            GuardianAssessmentStatus::InProgress,
        );
        missing_target.target_item_id = None;
        guardian_context
            .apply_guardian_assessment_event(missing_target)
            .await;
        let eighth = recv_broadcast_message(&mut rx).await?;
        match eighth {
            OutgoingMessage::AppServerNotification(
                ServerNotification::ItemGuardianApprovalReviewStarted(payload),
            ) => {
                assert_eq!(payload.review_id, "review-cmd-guardian-missing-target");
                assert_eq!(payload.target_item_id, None);
                assert_eq!(
                    payload.review.status,
                    GuardianApprovalReviewStatus::InProgress
                );
            }
            other => bail!("unexpected message: {other:?}"),
        }

        assert!(rx.try_recv().is_err(), "no extra messages expected");
        conversation.shutdown_and_wait().await?;
        Ok(())
    }

    #[test]
    fn file_change_accept_for_session_maps_to_approved_for_session() {
        let (decision, completion_status) =
            map_file_change_approval_decision(FileChangeApprovalDecision::AcceptForSession);
        assert_eq!(decision, ReviewDecision::ApprovedForSession);
        assert_eq!(completion_status, None);
    }

    #[test]
    fn mcp_server_elicitation_turn_transition_error_maps_to_cancel() {
        let error = JSONRPCErrorError {
            code: -1,
            message: "client request resolved because the turn state was changed".to_string(),
            data: Some(serde_json::json!({ "reason": "turnTransition" })),
        };

        let response = mcp_server_elicitation_response_from_client_result(Ok(Err(error)));

        assert_eq!(
            response,
            McpServerElicitationRequestResponse {
                action: McpServerElicitationAction::Cancel,
                content: None,
                meta: None,
            }
        );
    }

    #[test]
    fn request_permissions_turn_transition_error_is_ignored() {
        let error = JSONRPCErrorError {
            code: -1,
            message: "client request resolved because the turn state was changed".to_string(),
            data: Some(serde_json::json!({ "reason": "turnTransition" })),
        };

        let response = request_permissions_response_from_client_result(
            CoreRequestPermissionProfile::default(),
            Ok(Err(error)),
        );

        assert_eq!(response, None);
    }

    #[test]
    fn request_permissions_response_accepts_partial_network_and_file_system_grants() {
        let input_path = if cfg!(target_os = "windows") {
            r"C:\tmp\input"
        } else {
            "/tmp/input"
        };
        let output_path = if cfg!(target_os = "windows") {
            r"C:\tmp\output"
        } else {
            "/tmp/output"
        };
        let ignored_path = if cfg!(target_os = "windows") {
            r"C:\tmp\ignored"
        } else {
            "/tmp/ignored"
        };
        let absolute_path = |path: &str| {
            AbsolutePathBuf::try_from(std::path::PathBuf::from(path)).expect("absolute path")
        };
        let requested_permissions = CoreRequestPermissionProfile {
            network: Some(CoreNetworkPermissions {
                enabled: Some(true),
            }),
            file_system: Some(CoreFileSystemPermissions {
                read: Some(vec![absolute_path(input_path)]),
                write: Some(vec![absolute_path(output_path)]),
            }),
        };
        let cases = vec![
            (
                serde_json::json!({}),
                CoreRequestPermissionProfile::default(),
            ),
            (
                serde_json::json!({
                    "network": {
                        "enabled": true,
                    },
                }),
                CoreRequestPermissionProfile {
                    network: Some(CoreNetworkPermissions {
                        enabled: Some(true),
                    }),
                    ..CoreRequestPermissionProfile::default()
                },
            ),
            (
                serde_json::json!({
                    "fileSystem": {
                        "write": [output_path],
                    },
                }),
                CoreRequestPermissionProfile {
                    file_system: Some(CoreFileSystemPermissions {
                        read: None,
                        write: Some(vec![absolute_path(output_path)]),
                    }),
                    ..CoreRequestPermissionProfile::default()
                },
            ),
            (
                serde_json::json!({
                    "fileSystem": {
                        "read": [input_path],
                        "write": [output_path, ignored_path],
                    },
                    "macos": {
                        "calendar": true,
                    },
                }),
                CoreRequestPermissionProfile {
                    file_system: Some(CoreFileSystemPermissions {
                        read: Some(vec![absolute_path(input_path)]),
                        write: Some(vec![absolute_path(output_path)]),
                    }),
                    ..CoreRequestPermissionProfile::default()
                },
            ),
        ];

        for (granted_permissions, expected_permissions) in cases {
            let response = request_permissions_response_from_client_result(
                requested_permissions.clone(),
                Ok(Ok(serde_json::json!({
                    "permissions": granted_permissions,
                }))),
            )
            .expect("response should be accepted");

            assert_eq!(
                response,
                CoreRequestPermissionsResponse {
                    permissions: expected_permissions,
                    scope: CorePermissionGrantScope::Turn,
                }
            );
        }
    }

    #[test]
    fn request_permissions_response_preserves_session_scope() {
        let response = request_permissions_response_from_client_result(
            CoreRequestPermissionProfile::default(),
            Ok(Ok(serde_json::json!({
                "scope": "session",
                "permissions": {},
            }))),
        )
        .expect("response should be accepted");

        assert_eq!(
            response,
            CoreRequestPermissionsResponse {
                permissions: CoreRequestPermissionProfile::default(),
                scope: CorePermissionGrantScope::Session,
            }
        );
    }

    #[test]
    fn collab_resume_begin_maps_to_item_started_resume_agent() {
        let event = CollabResumeBeginEvent {
            call_id: "call-1".to_string(),
            sender_thread_id: ThreadId::new(),
            receiver_thread_id: ThreadId::new(),
            receiver_agent_nickname: None,
            receiver_agent_role: None,
        };

        let item = collab_resume_begin_item(event.clone());
        let expected = ThreadItem::CollabAgentToolCall {
            id: event.call_id,
            tool: CollabAgentTool::ResumeAgent,
            status: V2CollabToolCallStatus::InProgress,
            sender_thread_id: event.sender_thread_id.to_string(),
            receiver_thread_ids: vec![event.receiver_thread_id.to_string()],
            prompt: None,
            model: None,
            reasoning_effort: None,
            agents_states: HashMap::new(),
        };
        assert_eq!(item, expected);
    }

    #[test]
    fn collab_resume_end_maps_to_item_completed_resume_agent() {
        let event = CollabResumeEndEvent {
            call_id: "call-2".to_string(),
            sender_thread_id: ThreadId::new(),
            receiver_thread_id: ThreadId::new(),
            receiver_agent_nickname: None,
            receiver_agent_role: None,
            status: codex_protocol::protocol::AgentStatus::NotFound,
        };

        let item = collab_resume_end_item(event.clone());
        let receiver_id = event.receiver_thread_id.to_string();
        let expected = ThreadItem::CollabAgentToolCall {
            id: event.call_id,
            tool: CollabAgentTool::ResumeAgent,
            status: V2CollabToolCallStatus::Failed,
            sender_thread_id: event.sender_thread_id.to_string(),
            receiver_thread_ids: vec![receiver_id.clone()],
            prompt: None,
            model: None,
            reasoning_effort: None,
            agents_states: [(
                receiver_id,
                V2CollabAgentStatus::from(codex_protocol::protocol::AgentStatus::NotFound),
            )]
            .into_iter()
            .collect(),
        };
        assert_eq!(item, expected);
    }

    #[tokio::test]
    async fn test_handle_error_records_message() -> Result<()> {
        let conversation_id = ThreadId::new();
        let thread_state = new_thread_state();

        handle_error(
            conversation_id,
            TurnError {
                message: "boom".to_string(),
                codex_error_info: Some(V2CodexErrorInfo::InternalServerError),
                additional_details: None,
            },
            &thread_state,
        )
        .await;

        let turn_summary = find_and_remove_turn_summary(conversation_id, &thread_state).await;
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
        let outgoing = ThreadScopedOutgoingMessageSender::new(
            outgoing,
            vec![ConnectionId(1)],
            ThreadId::new(),
        );
        let thread_state = new_thread_state();
        {
            let mut state = thread_state.lock().await;
            state.track_current_turn_event(&EventMsg::TurnStarted(
                codex_protocol::protocol::TurnStartedEvent {
                    turn_id: event_turn_id.clone(),
                    started_at: Some(42),
                    model_context_window: None,
                    collaboration_mode_kind: Default::default(),
                },
            ));
            state.track_current_turn_event(&EventMsg::TurnComplete(turn_complete_event(
                &event_turn_id,
            )));
        }

        handle_turn_complete(
            conversation_id,
            event_turn_id.clone(),
            turn_complete_event(&event_turn_id),
            /*analytics_events_client*/ None,
            &outgoing,
            &thread_state,
        )
        .await;

        let msg = recv_broadcast_message(&mut rx).await?;
        match msg {
            OutgoingMessage::AppServerNotification(ServerNotification::TurnCompleted(n)) => {
                assert_eq!(n.turn.id, event_turn_id);
                assert_eq!(n.turn.status, TurnStatus::Completed);
                assert_eq!(n.turn.error, None);
                assert_eq!(n.turn.started_at, Some(42));
                assert_eq!(n.turn.completed_at, Some(TEST_TURN_COMPLETED_AT));
                assert_eq!(n.turn.duration_ms, Some(TEST_TURN_DURATION_MS));
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
        let thread_state = new_thread_state();
        handle_error(
            conversation_id,
            TurnError {
                message: "oops".to_string(),
                codex_error_info: None,
                additional_details: None,
            },
            &thread_state,
        )
        .await;
        let (tx, mut rx) = mpsc::channel(CHANNEL_CAPACITY);
        let outgoing = Arc::new(OutgoingMessageSender::new(tx));
        let outgoing = ThreadScopedOutgoingMessageSender::new(
            outgoing,
            vec![ConnectionId(1)],
            ThreadId::new(),
        );

        handle_turn_interrupted(
            conversation_id,
            event_turn_id.clone(),
            turn_aborted_event(&event_turn_id),
            /*analytics_events_client*/ None,
            &outgoing,
            &thread_state,
        )
        .await;

        let msg = recv_broadcast_message(&mut rx).await?;
        match msg {
            OutgoingMessage::AppServerNotification(ServerNotification::TurnCompleted(n)) => {
                assert_eq!(n.turn.id, event_turn_id);
                assert_eq!(n.turn.status, TurnStatus::Interrupted);
                assert_eq!(n.turn.error, None);
                assert_eq!(n.turn.completed_at, Some(TEST_TURN_COMPLETED_AT));
                assert_eq!(n.turn.duration_ms, Some(TEST_TURN_DURATION_MS));
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
        let thread_state = new_thread_state();
        handle_error(
            conversation_id,
            TurnError {
                message: "bad".to_string(),
                codex_error_info: Some(V2CodexErrorInfo::Other),
                additional_details: None,
            },
            &thread_state,
        )
        .await;
        let (tx, mut rx) = mpsc::channel(CHANNEL_CAPACITY);
        let outgoing = Arc::new(OutgoingMessageSender::new(tx));
        let outgoing = ThreadScopedOutgoingMessageSender::new(
            outgoing,
            vec![ConnectionId(1)],
            ThreadId::new(),
        );

        handle_turn_complete(
            conversation_id,
            event_turn_id.clone(),
            turn_complete_event(&event_turn_id),
            /*analytics_events_client*/ None,
            &outgoing,
            &thread_state,
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
                assert_eq!(n.turn.completed_at, Some(TEST_TURN_COMPLETED_AT));
                assert_eq!(n.turn.duration_ms, Some(TEST_TURN_DURATION_MS));
            }
            other => bail!("unexpected message: {other:?}"),
        }
        assert!(rx.try_recv().is_err(), "no extra messages expected");
        Ok(())
    }

    #[tokio::test]
    async fn test_handle_turn_plan_update_emits_notification_for_v2() -> Result<()> {
        let (tx, mut rx) = mpsc::channel(CHANNEL_CAPACITY);
        let outgoing = Arc::new(OutgoingMessageSender::new(tx));
        let outgoing = ThreadScopedOutgoingMessageSender::new(
            outgoing,
            vec![ConnectionId(1)],
            ThreadId::new(),
        );
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
        let outgoing = ThreadScopedOutgoingMessageSender::new(
            outgoing,
            vec![ConnectionId(1)],
            ThreadId::new(),
        );

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
            limit_id: Some("codex".to_string()),
            limit_name: None,
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
            rate_limit_reached_type: None,
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
                assert_eq!(payload.rate_limits.limit_id.as_deref(), Some("codex"));
                assert_eq!(payload.rate_limits.limit_name, None);
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
        let outgoing = ThreadScopedOutgoingMessageSender::new(
            outgoing,
            vec![ConnectionId(1)],
            ThreadId::new(),
        );

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
            mcp_app_resource_uri: Some("ui://widget/list-resources.html".to_string()),
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
                mcp_app_resource_uri: Some("ui://widget/list-resources.html".to_string()),
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
        let thread_state = new_thread_state();

        let (tx, mut rx) = mpsc::channel(CHANNEL_CAPACITY);
        let outgoing = Arc::new(OutgoingMessageSender::new(tx));
        let outgoing = ThreadScopedOutgoingMessageSender::new(
            outgoing,
            vec![ConnectionId(1)],
            ThreadId::new(),
        );

        // Turn 1 on conversation A
        let a_turn1 = "a_turn1".to_string();
        handle_error(
            conversation_a,
            TurnError {
                message: "a1".to_string(),
                codex_error_info: Some(V2CodexErrorInfo::BadRequest),
                additional_details: None,
            },
            &thread_state,
        )
        .await;
        handle_turn_complete(
            conversation_a,
            a_turn1.clone(),
            turn_complete_event(&a_turn1),
            /*analytics_events_client*/ None,
            &outgoing,
            &thread_state,
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
            &thread_state,
        )
        .await;
        handle_turn_complete(
            conversation_b,
            b_turn1.clone(),
            turn_complete_event(&b_turn1),
            /*analytics_events_client*/ None,
            &outgoing,
            &thread_state,
        )
        .await;

        // Turn 2 on conversation A
        let a_turn2 = "a_turn2".to_string();
        handle_turn_complete(
            conversation_a,
            a_turn2.clone(),
            turn_complete_event(&a_turn2),
            /*analytics_events_client*/ None,
            &outgoing,
            &thread_state,
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
            mcp_app_resource_uri: None,
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
                mcp_app_resource_uri: None,
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
            meta: Some(serde_json::json!({
                "ui/resourceUri": "ui://widget/list-resources.html"
            })),
        };

        let end_event = McpToolCallEndEvent {
            call_id: "call_789".to_string(),
            invocation: McpInvocation {
                server: "codex".to_string(),
                tool: "list_mcp_resources".to_string(),
                arguments: Some(serde_json::json!({"server": ""})),
            },
            mcp_app_resource_uri: Some("ui://widget/list-resources.html".to_string()),
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
                mcp_app_resource_uri: Some("ui://widget/list-resources.html".to_string()),
                result: Some(Box::new(McpToolCallResult {
                    content,
                    structured_content: None,
                    meta: Some(serde_json::json!({
                        "ui/resourceUri": "ui://widget/list-resources.html"
                    })),
                })),
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
            mcp_app_resource_uri: None,
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
                mcp_app_resource_uri: None,
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
        let outgoing = Arc::new(OutgoingMessageSender::new(tx));
        let outgoing = ThreadScopedOutgoingMessageSender::new(
            outgoing,
            vec![ConnectionId(1)],
            ThreadId::new(),
        );
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
        let outgoing = Arc::new(OutgoingMessageSender::new(tx));
        let outgoing = ThreadScopedOutgoingMessageSender::new(
            outgoing,
            vec![ConnectionId(1)],
            ThreadId::new(),
        );
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

    #[tokio::test]
    async fn test_hook_prompt_raw_response_emits_item_completed() -> Result<()> {
        let (tx, mut rx) = mpsc::channel(CHANNEL_CAPACITY);
        let outgoing = Arc::new(OutgoingMessageSender::new(tx));
        let conversation_id = ThreadId::new();
        let outgoing = ThreadScopedOutgoingMessageSender::new(
            outgoing,
            vec![ConnectionId(1)],
            conversation_id,
        );
        let item = build_hook_prompt_message(&[
            HookPromptFragment::from_single_hook("Retry with tests.", "hook-run-1"),
            HookPromptFragment::from_single_hook("Then summarize cleanly.", "hook-run-2"),
        ])
        .expect("hook prompt message");

        maybe_emit_hook_prompt_item_completed(
            ApiVersion::V2,
            conversation_id,
            "turn-1",
            &item,
            &outgoing,
        )
        .await;

        let msg = recv_broadcast_message(&mut rx).await?;
        match msg {
            OutgoingMessage::AppServerNotification(ServerNotification::ItemCompleted(
                notification,
            )) => {
                assert_eq!(notification.thread_id, conversation_id.to_string());
                assert_eq!(notification.turn_id, "turn-1");
                assert_eq!(
                    notification.item,
                    ThreadItem::HookPrompt {
                        id: notification.item.id().to_string(),
                        fragments: vec![
                            codex_app_server_protocol::HookPromptFragment {
                                text: "Retry with tests.".into(),
                                hook_run_id: "hook-run-1".into(),
                            },
                            codex_app_server_protocol::HookPromptFragment {
                                text: "Then summarize cleanly.".into(),
                                hook_run_id: "hook-run-2".into(),
                            },
                        ],
                    }
                );
            }
            other => bail!("unexpected message: {other:?}"),
        }
        assert!(rx.try_recv().is_err(), "no extra messages expected");
        Ok(())
    }
}
