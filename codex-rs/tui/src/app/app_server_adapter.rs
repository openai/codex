/*
This module holds the temporary adapter layer between the TUI and the app
server during the hybrid migration period.

For now, the TUI still owns its existing direct-core behavior, but startup
allocates a local in-process app server and drains its event stream. Keeping
the app-server-specific wiring here keeps that transitional logic out of the
main `app.rs` orchestration path.

As more TUI flows move onto the app-server surface directly, this adapter
should shrink and eventually disappear.
*/

use super::App;
use crate::app_command::AppCommand;
use crate::app_event::AppEvent;
use crate::app_server_session::AppServerSession;
use crate::app_server_session::app_server_rate_limit_snapshot_to_core;
use crate::app_server_session::status_account_display_from_auth_mode;
#[cfg(test)]
use crate::exec_command::split_command_string;
use codex_app_server_client::AppServerEvent;
use codex_app_server_protocol::AuthMode;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ServerRequest;
#[cfg(test)]
use codex_app_server_protocol::Thread;
#[cfg(test)]
use codex_app_server_protocol::ThreadItem;
#[cfg(test)]
use codex_app_server_protocol::Turn;
#[cfg(test)]
use codex_app_server_protocol::TurnStatus;
use codex_protocol::ThreadId;
#[cfg(test)]
use codex_protocol::config_types::ModeKind;
#[cfg(test)]
use codex_protocol::items::AgentMessageContent;
#[cfg(test)]
use codex_protocol::items::AgentMessageItem;
#[cfg(test)]
use codex_protocol::items::ContextCompactionItem;
#[cfg(test)]
use codex_protocol::items::ImageGenerationItem;
#[cfg(test)]
use codex_protocol::items::PlanItem;
#[cfg(test)]
use codex_protocol::items::ReasoningItem;
#[cfg(test)]
use codex_protocol::items::TurnItem;
#[cfg(test)]
use codex_protocol::items::UserMessageItem;
#[cfg(test)]
use codex_protocol::items::WebSearchItem;
#[cfg(test)]
use codex_protocol::protocol::AgentMessageDeltaEvent;
#[cfg(test)]
use codex_protocol::protocol::AgentReasoningDeltaEvent;
#[cfg(test)]
use codex_protocol::protocol::AgentReasoningRawContentDeltaEvent;
#[cfg(test)]
use codex_protocol::protocol::ErrorEvent;
#[cfg(test)]
use codex_protocol::protocol::Event;
#[cfg(test)]
use codex_protocol::protocol::EventMsg;
#[cfg(test)]
use codex_protocol::protocol::ExecCommandBeginEvent;
#[cfg(test)]
use codex_protocol::protocol::ExecCommandEndEvent;
#[cfg(test)]
use codex_protocol::protocol::ExecCommandOutputDeltaEvent;
#[cfg(test)]
use codex_protocol::protocol::ExecCommandStatus;
#[cfg(test)]
use codex_protocol::protocol::ExecOutputStream;
#[cfg(test)]
use codex_protocol::protocol::ItemCompletedEvent;
#[cfg(test)]
use codex_protocol::protocol::ItemStartedEvent;
#[cfg(test)]
use codex_protocol::protocol::PlanDeltaEvent;
#[cfg(test)]
use codex_protocol::protocol::RealtimeConversationClosedEvent;
#[cfg(test)]
use codex_protocol::protocol::RealtimeConversationRealtimeEvent;
#[cfg(test)]
use codex_protocol::protocol::RealtimeConversationStartedEvent;
#[cfg(test)]
use codex_protocol::protocol::RealtimeEvent;
#[cfg(test)]
use codex_protocol::protocol::ThreadNameUpdatedEvent;
#[cfg(test)]
use codex_protocol::protocol::TokenCountEvent;
#[cfg(test)]
use codex_protocol::protocol::TokenUsage;
#[cfg(test)]
use codex_protocol::protocol::TokenUsageInfo;
#[cfg(test)]
use codex_protocol::protocol::TurnAbortReason;
#[cfg(test)]
use codex_protocol::protocol::TurnAbortedEvent;
#[cfg(test)]
use codex_protocol::protocol::TurnCompleteEvent;
#[cfg(test)]
use codex_protocol::protocol::TurnStartedEvent;
#[cfg(test)]
use std::time::Duration;

impl App {
    fn refresh_mcp_startup_expected_servers_from_config(&mut self) {
        let enabled_config_mcp_servers: Vec<String> = self
            .chat_widget
            .config_ref()
            .mcp_servers
            .get()
            .iter()
            .filter_map(|(name, server)| server.enabled.then_some(name.clone()))
            .collect();
        self.chat_widget
            .set_mcp_startup_expected_servers(enabled_config_mcp_servers);
    }

    pub(super) async fn handle_app_server_event(
        &mut self,
        app_server_client: &AppServerSession,
        event: AppServerEvent,
    ) {
        match event {
            AppServerEvent::Lagged { skipped } => {
                tracing::warn!(
                    skipped,
                    "app-server event consumer lagged; dropping ignored events"
                );
                self.refresh_mcp_startup_expected_servers_from_config();
                self.chat_widget.finish_mcp_startup_after_lag();
            }
            AppServerEvent::ServerNotification(notification) => {
                self.handle_server_notification_event(app_server_client, notification)
                    .await;
            }
            AppServerEvent::ServerRequest(request) => {
                self.handle_server_request_event(app_server_client, request)
                    .await;
            }
            AppServerEvent::Disconnected { message } => {
                tracing::warn!("app-server event stream disconnected: {message}");
                self.chat_widget.add_error_message(message.clone());
                self.app_event_tx.send(AppEvent::FatalExitRequest(message));
            }
        }
    }

    async fn handle_server_notification_event(
        &mut self,
        app_server_client: &AppServerSession,
        notification: ServerNotification,
    ) {
        match &notification {
            ServerNotification::ServerRequestResolved(notification) => {
                if let Some(request) = self
                    .pending_app_server_requests
                    .resolve_notification(&notification.request_id)
                {
                    self.chat_widget.dismiss_app_server_request(&request);
                }
            }
            ServerNotification::McpServerStatusUpdated(_) => {
                self.refresh_mcp_startup_expected_servers_from_config();
            }
            ServerNotification::AccountRateLimitsUpdated(notification) => {
                self.chat_widget.on_rate_limit_snapshot(Some(
                    app_server_rate_limit_snapshot_to_core(notification.rate_limits.clone()),
                ));
                return;
            }
            ServerNotification::AccountUpdated(notification) => {
                self.chat_widget.update_account_state(
                    status_account_display_from_auth_mode(
                        notification.auth_mode,
                        notification.plan_type,
                    ),
                    notification.plan_type,
                    matches!(
                        notification.auth_mode,
                        Some(AuthMode::Chatgpt) | Some(AuthMode::ChatgptAuthTokens)
                    ),
                );
                return;
            }
            ServerNotification::ExternalAgentConfigImportCompleted(_) => {
                let cwd = self.chat_widget.config_ref().cwd.to_path_buf();
                if let Err(err) = self.refresh_in_memory_config_from_disk().await {
                    tracing::warn!(
                        error = %err,
                        "failed to refresh config after external agent config import"
                    );
                }
                self.chat_widget.refresh_plugin_mentions();
                self.chat_widget.submit_op(AppCommand::reload_user_config());
                self.fetch_plugins_list(app_server_client, cwd);
                return;
            }
            _ => {}
        }

        match server_notification_thread_target(&notification) {
            ServerNotificationThreadTarget::Thread(thread_id) => {
                let result = if self.primary_thread_id == Some(thread_id)
                    || self.primary_thread_id.is_none()
                {
                    self.enqueue_primary_thread_notification(notification).await
                } else {
                    self.enqueue_thread_notification(thread_id, notification)
                        .await
                };

                if let Err(err) = result {
                    tracing::warn!("failed to enqueue app-server notification: {err}");
                }
                return;
            }
            ServerNotificationThreadTarget::InvalidThreadId(thread_id) => {
                tracing::warn!(
                    thread_id,
                    "ignoring app-server notification with invalid thread_id"
                );
                return;
            }
            ServerNotificationThreadTarget::Global => {}
        }

        self.chat_widget
            .handle_server_notification(notification, /*replay_kind*/ None);
    }

    async fn handle_server_request_event(
        &mut self,
        app_server_client: &AppServerSession,
        request: ServerRequest,
    ) {
        if let Some(unsupported) = self
            .pending_app_server_requests
            .note_server_request(&request)
        {
            tracing::warn!(
                request_id = ?unsupported.request_id,
                message = unsupported.message,
                "rejecting unsupported app-server request"
            );
            self.chat_widget
                .add_error_message(unsupported.message.clone());
            if let Err(err) = self
                .reject_app_server_request(
                    app_server_client,
                    unsupported.request_id,
                    unsupported.message,
                )
                .await
            {
                tracing::warn!("{err}");
            }
            return;
        }

        let Some(thread_id) = server_request_thread_id(&request) else {
            tracing::warn!("ignoring threadless app-server request");
            return;
        };

        let result =
            if self.primary_thread_id == Some(thread_id) || self.primary_thread_id.is_none() {
                self.enqueue_primary_thread_request(request).await
            } else {
                self.enqueue_thread_request(thread_id, request).await
            };
        if let Err(err) = result {
            tracing::warn!("failed to enqueue app-server request: {err}");
        }
    }
    async fn reject_app_server_request(
        &self,
        app_server_client: &AppServerSession,
        request_id: codex_app_server_protocol::RequestId,
        reason: String,
    ) -> std::result::Result<(), String> {
        app_server_client
            .reject_server_request(
                request_id,
                JSONRPCErrorError {
                    code: -32000,
                    message: reason,
                    data: None,
                },
            )
            .await
            .map_err(|err| format!("failed to reject app-server request: {err}"))
    }
}

fn server_request_thread_id(request: &ServerRequest) -> Option<ThreadId> {
    match request {
        ServerRequest::CommandExecutionRequestApproval { params, .. } => {
            ThreadId::from_string(&params.thread_id).ok()
        }
        ServerRequest::FileChangeRequestApproval { params, .. } => {
            ThreadId::from_string(&params.thread_id).ok()
        }
        ServerRequest::ToolRequestUserInput { params, .. } => {
            ThreadId::from_string(&params.thread_id).ok()
        }
        ServerRequest::McpServerElicitationRequest { params, .. } => {
            ThreadId::from_string(&params.thread_id).ok()
        }
        ServerRequest::PermissionsRequestApproval { params, .. } => {
            ThreadId::from_string(&params.thread_id).ok()
        }
        ServerRequest::DynamicToolCall { params, .. } => {
            ThreadId::from_string(&params.thread_id).ok()
        }
        ServerRequest::ChatgptAuthTokensRefresh { .. }
        | ServerRequest::ApplyPatchApproval { .. }
        | ServerRequest::ExecCommandApproval { .. } => None,
    }
}

#[derive(Debug, PartialEq, Eq)]
enum ServerNotificationThreadTarget {
    Thread(ThreadId),
    InvalidThreadId(String),
    Global,
}

fn server_notification_thread_target(
    notification: &ServerNotification,
) -> ServerNotificationThreadTarget {
    let thread_id = match notification {
        ServerNotification::Error(notification) => Some(notification.thread_id.as_str()),
        ServerNotification::ThreadStarted(notification) => Some(notification.thread.id.as_str()),
        ServerNotification::ThreadStatusChanged(notification) => {
            Some(notification.thread_id.as_str())
        }
        ServerNotification::ThreadArchived(notification) => Some(notification.thread_id.as_str()),
        ServerNotification::ThreadUnarchived(notification) => Some(notification.thread_id.as_str()),
        ServerNotification::ThreadClosed(notification) => Some(notification.thread_id.as_str()),
        ServerNotification::ThreadNameUpdated(notification) => {
            Some(notification.thread_id.as_str())
        }
        ServerNotification::ThreadTokenUsageUpdated(notification) => {
            Some(notification.thread_id.as_str())
        }
        ServerNotification::ThreadGoalUpdated(notification) => {
            Some(notification.thread_id.as_str())
        }
        ServerNotification::ThreadGoalCleared(notification) => {
            Some(notification.thread_id.as_str())
        }
        ServerNotification::TurnStarted(notification) => Some(notification.thread_id.as_str()),
        ServerNotification::HookStarted(notification) => Some(notification.thread_id.as_str()),
        ServerNotification::TurnCompleted(notification) => Some(notification.thread_id.as_str()),
        ServerNotification::HookCompleted(notification) => Some(notification.thread_id.as_str()),
        ServerNotification::TurnDiffUpdated(notification) => Some(notification.thread_id.as_str()),
        ServerNotification::TurnPlanUpdated(notification) => Some(notification.thread_id.as_str()),
        ServerNotification::ItemStarted(notification) => Some(notification.thread_id.as_str()),
        ServerNotification::ItemGuardianApprovalReviewStarted(notification) => {
            Some(notification.thread_id.as_str())
        }
        ServerNotification::ItemGuardianApprovalReviewCompleted(notification) => {
            Some(notification.thread_id.as_str())
        }
        ServerNotification::ItemCompleted(notification) => Some(notification.thread_id.as_str()),
        ServerNotification::RawResponseItemCompleted(notification) => {
            Some(notification.thread_id.as_str())
        }
        ServerNotification::AgentMessageDelta(notification) => {
            Some(notification.thread_id.as_str())
        }
        ServerNotification::PlanDelta(notification) => Some(notification.thread_id.as_str()),
        ServerNotification::CommandExecutionOutputDelta(notification) => {
            Some(notification.thread_id.as_str())
        }
        ServerNotification::TerminalInteraction(notification) => {
            Some(notification.thread_id.as_str())
        }
        ServerNotification::FileChangeOutputDelta(notification) => {
            Some(notification.thread_id.as_str())
        }
        ServerNotification::FileChangePatchUpdated(notification) => {
            Some(notification.thread_id.as_str())
        }
        ServerNotification::ServerRequestResolved(notification) => {
            Some(notification.thread_id.as_str())
        }
        ServerNotification::McpToolCallProgress(notification) => {
            Some(notification.thread_id.as_str())
        }
        ServerNotification::ReasoningSummaryTextDelta(notification) => {
            Some(notification.thread_id.as_str())
        }
        ServerNotification::ReasoningSummaryPartAdded(notification) => {
            Some(notification.thread_id.as_str())
        }
        ServerNotification::ReasoningTextDelta(notification) => {
            Some(notification.thread_id.as_str())
        }
        ServerNotification::ContextCompacted(notification) => Some(notification.thread_id.as_str()),
        ServerNotification::ModelRerouted(notification) => Some(notification.thread_id.as_str()),
        ServerNotification::ModelVerification(notification) => {
            Some(notification.thread_id.as_str())
        }
        ServerNotification::ThreadRealtimeStarted(notification) => {
            Some(notification.thread_id.as_str())
        }
        ServerNotification::ThreadRealtimeItemAdded(notification) => {
            Some(notification.thread_id.as_str())
        }
        ServerNotification::ThreadRealtimeTranscriptDelta(notification) => {
            Some(notification.thread_id.as_str())
        }
        ServerNotification::ThreadRealtimeTranscriptDone(notification) => {
            Some(notification.thread_id.as_str())
        }
        ServerNotification::ThreadRealtimeOutputAudioDelta(notification) => {
            Some(notification.thread_id.as_str())
        }
        ServerNotification::ThreadRealtimeSdp(notification) => {
            Some(notification.thread_id.as_str())
        }
        ServerNotification::ThreadRealtimeError(notification) => {
            Some(notification.thread_id.as_str())
        }
        ServerNotification::ThreadRealtimeClosed(notification) => {
            Some(notification.thread_id.as_str())
        }
        ServerNotification::Warning(notification) => notification.thread_id.as_deref(),
        ServerNotification::GuardianWarning(notification) => Some(notification.thread_id.as_str()),
        ServerNotification::SkillsChanged(_)
        | ServerNotification::McpServerStatusUpdated(_)
        | ServerNotification::McpServerOauthLoginCompleted(_)
        | ServerNotification::AccountUpdated(_)
        | ServerNotification::AccountRateLimitsUpdated(_)
        | ServerNotification::AppListUpdated(_)
        | ServerNotification::RemoteControlStatusChanged(_)
        | ServerNotification::ExternalAgentConfigImportCompleted(_)
        | ServerNotification::DeprecationNotice(_)
        | ServerNotification::ConfigWarning(_)
        | ServerNotification::FuzzyFileSearchSessionUpdated(_)
        | ServerNotification::FuzzyFileSearchSessionCompleted(_)
        | ServerNotification::CommandExecOutputDelta(_)
        | ServerNotification::FsChanged(_)
        | ServerNotification::WindowsWorldWritableWarning(_)
        | ServerNotification::WindowsSandboxSetupCompleted(_)
        | ServerNotification::AccountLoginCompleted(_) => None,
    };

    match thread_id {
        Some(thread_id) => match ThreadId::from_string(thread_id) {
            Ok(thread_id) => ServerNotificationThreadTarget::Thread(thread_id),
            Err(_) => ServerNotificationThreadTarget::InvalidThreadId(thread_id.to_string()),
        },
        None => ServerNotificationThreadTarget::Global,
    }
}

#[cfg(test)]
/// Convert a `Thread` snapshot into a flat sequence of protocol `Event`s
/// suitable for replaying into the TUI event store.
///
/// Each turn is expanded into `TurnStarted`, zero or more `ItemCompleted`,
/// and a terminal event that matches the turn's `TurnStatus`. Returns an
/// empty vec (with a warning log) if the thread ID is not a valid UUID.
pub(super) fn thread_snapshot_events(
    thread: &Thread,
    show_raw_agent_reasoning: bool,
) -> Vec<Event> {
    let Ok(thread_id) = ThreadId::from_string(&thread.id) else {
        tracing::warn!(
            thread_id = %thread.id,
            "ignoring app-server thread snapshot with invalid thread id"
        );
        return Vec::new();
    };

    thread
        .turns
        .iter()
        .flat_map(|turn| turn_snapshot_events(thread_id, turn, show_raw_agent_reasoning))
        .collect()
}

#[cfg(test)]
fn server_notification_thread_events(
    notification: ServerNotification,
) -> Option<(ThreadId, Vec<Event>)> {
    match notification {
        ServerNotification::ThreadTokenUsageUpdated(notification) => Some((
            ThreadId::from_string(&notification.thread_id).ok()?,
            vec![Event {
                id: String::new(),
                msg: EventMsg::TokenCount(TokenCountEvent {
                    info: Some(TokenUsageInfo {
                        total_token_usage: token_usage_from_app_server(
                            notification.token_usage.total,
                        ),
                        last_token_usage: token_usage_from_app_server(
                            notification.token_usage.last,
                        ),
                        model_context_window: notification.token_usage.model_context_window,
                    }),
                    rate_limits: None,
                }),
            }],
        )),
        ServerNotification::Error(notification) => Some((
            ThreadId::from_string(&notification.thread_id).ok()?,
            vec![Event {
                id: String::new(),
                msg: EventMsg::Error(ErrorEvent {
                    message: notification.error.message,
                    codex_error_info: notification
                        .error
                        .codex_error_info
                        .and_then(app_server_codex_error_info_to_core),
                }),
            }],
        )),
        ServerNotification::ThreadNameUpdated(notification) => Some((
            ThreadId::from_string(&notification.thread_id).ok()?,
            vec![Event {
                id: String::new(),
                msg: EventMsg::ThreadNameUpdated(ThreadNameUpdatedEvent {
                    thread_id: ThreadId::from_string(&notification.thread_id).ok()?,
                    thread_name: notification.thread_name,
                }),
            }],
        )),
        ServerNotification::TurnStarted(notification) => Some((
            ThreadId::from_string(&notification.thread_id).ok()?,
            vec![Event {
                id: String::new(),
                msg: EventMsg::TurnStarted(TurnStartedEvent {
                    turn_id: notification.turn.id,
                    started_at: notification.turn.started_at,
                    model_context_window: None,
                    collaboration_mode_kind: ModeKind::default(),
                }),
            }],
        )),
        ServerNotification::TurnCompleted(notification) => {
            let thread_id = ThreadId::from_string(&notification.thread_id).ok()?;
            let mut events = Vec::new();
            append_terminal_turn_events(
                &mut events,
                &notification.turn,
                /*include_failed_error*/ false,
            );
            Some((thread_id, events))
        }
        ServerNotification::ItemStarted(notification) => Some((
            ThreadId::from_string(&notification.thread_id).ok()?,
            command_execution_started_event(&notification.turn_id, &notification.item).or_else(
                || {
                    Some(vec![Event {
                        id: String::new(),
                        msg: EventMsg::ItemStarted(ItemStartedEvent {
                            thread_id: ThreadId::from_string(&notification.thread_id).ok()?,
                            turn_id: notification.turn_id.clone(),
                            item: thread_item_to_core(&notification.item)?,
                        }),
                    }])
                },
            )?,
        )),
        ServerNotification::ItemCompleted(notification) => Some((
            ThreadId::from_string(&notification.thread_id).ok()?,
            command_execution_completed_event(&notification.turn_id, &notification.item).or_else(
                || {
                    Some(vec![Event {
                        id: String::new(),
                        msg: EventMsg::ItemCompleted(ItemCompletedEvent {
                            thread_id: ThreadId::from_string(&notification.thread_id).ok()?,
                            turn_id: notification.turn_id.clone(),
                            item: thread_item_to_core(&notification.item)?,
                        }),
                    }])
                },
            )?,
        )),
        ServerNotification::CommandExecutionOutputDelta(notification) => Some((
            ThreadId::from_string(&notification.thread_id).ok()?,
            vec![Event {
                id: String::new(),
                msg: EventMsg::ExecCommandOutputDelta(ExecCommandOutputDeltaEvent {
                    call_id: notification.item_id,
                    stream: ExecOutputStream::Stdout,
                    chunk: notification.delta.into_bytes(),
                }),
            }],
        )),
        ServerNotification::AgentMessageDelta(notification) => Some((
            ThreadId::from_string(&notification.thread_id).ok()?,
            vec![Event {
                id: String::new(),
                msg: EventMsg::AgentMessageDelta(AgentMessageDeltaEvent {
                    delta: notification.delta,
                }),
            }],
        )),
        ServerNotification::PlanDelta(notification) => Some((
            ThreadId::from_string(&notification.thread_id).ok()?,
            vec![Event {
                id: String::new(),
                msg: EventMsg::PlanDelta(PlanDeltaEvent {
                    thread_id: notification.thread_id,
                    turn_id: notification.turn_id,
                    item_id: notification.item_id,
                    delta: notification.delta,
                }),
            }],
        )),
        ServerNotification::ReasoningSummaryTextDelta(notification) => Some((
            ThreadId::from_string(&notification.thread_id).ok()?,
            vec![Event {
                id: String::new(),
                msg: EventMsg::AgentReasoningDelta(AgentReasoningDeltaEvent {
                    delta: notification.delta,
                }),
            }],
        )),
        ServerNotification::ReasoningTextDelta(notification) => Some((
            ThreadId::from_string(&notification.thread_id).ok()?,
            vec![Event {
                id: String::new(),
                msg: EventMsg::AgentReasoningRawContentDelta(AgentReasoningRawContentDeltaEvent {
                    delta: notification.delta,
                }),
            }],
        )),
        ServerNotification::ThreadRealtimeStarted(notification) => Some((
            ThreadId::from_string(&notification.thread_id).ok()?,
            vec![Event {
                id: String::new(),
                msg: EventMsg::RealtimeConversationStarted(RealtimeConversationStartedEvent {
                    session_id: notification.session_id,
                    version: notification.version,
                }),
            }],
        )),
        ServerNotification::ThreadRealtimeItemAdded(notification) => Some((
            ThreadId::from_string(&notification.thread_id).ok()?,
            vec![Event {
                id: String::new(),
                msg: EventMsg::RealtimeConversationRealtime(RealtimeConversationRealtimeEvent {
                    payload: RealtimeEvent::ConversationItemAdded(notification.item),
                }),
            }],
        )),
        ServerNotification::ThreadRealtimeOutputAudioDelta(notification) => Some((
            ThreadId::from_string(&notification.thread_id).ok()?,
            vec![Event {
                id: String::new(),
                msg: EventMsg::RealtimeConversationRealtime(RealtimeConversationRealtimeEvent {
                    payload: RealtimeEvent::AudioOut(notification.audio.into()),
                }),
            }],
        )),
        ServerNotification::ThreadRealtimeSdp(notification) => Some((
            ThreadId::from_string(&notification.thread_id).ok()?,
            vec![Event {
                id: String::new(),
                msg: EventMsg::RealtimeConversationSdp(
                    codex_protocol::protocol::RealtimeConversationSdpEvent {
                        sdp: notification.sdp,
                    },
                ),
            }],
        )),
        ServerNotification::ThreadRealtimeError(notification) => Some((
            ThreadId::from_string(&notification.thread_id).ok()?,
            vec![Event {
                id: String::new(),
                msg: EventMsg::RealtimeConversationRealtime(RealtimeConversationRealtimeEvent {
                    payload: RealtimeEvent::Error(notification.message),
                }),
            }],
        )),
        ServerNotification::ThreadRealtimeClosed(notification) => Some((
            ThreadId::from_string(&notification.thread_id).ok()?,
            vec![Event {
                id: String::new(),
                msg: EventMsg::RealtimeConversationClosed(RealtimeConversationClosedEvent {
                    reason: notification.reason,
                }),
            }],
        )),
        _ => None,
    }
}

#[cfg(test)]
fn token_usage_from_app_server(
    value: codex_app_server_protocol::TokenUsageBreakdown,
) -> TokenUsage {
    TokenUsage {
        input_tokens: value.input_tokens,
        cached_input_tokens: value.cached_input_tokens,
        output_tokens: value.output_tokens,
        reasoning_output_tokens: value.reasoning_output_tokens,
        total_tokens: value.total_tokens,
    }
}

/// Expand a single `Turn` into the event sequence the TUI would have
/// observed if it had been connected for the turn's entire lifetime.
///
/// Snapshot replay keeps committed-item semantics for user / plan /
/// agent-message items, while replaying the legacy events that still
/// drive rendering for reasoning, web-search, image-generation, and
/// context-compaction history cells.
#[cfg(test)]
fn turn_snapshot_events(
    thread_id: ThreadId,
    turn: &Turn,
    show_raw_agent_reasoning: bool,
) -> Vec<Event> {
    let mut events = vec![Event {
        id: String::new(),
        msg: EventMsg::TurnStarted(TurnStartedEvent {
            turn_id: turn.id.clone(),
            started_at: None,
            model_context_window: None,
            collaboration_mode_kind: ModeKind::default(),
        }),
    }];

    for item in &turn.items {
        if let Some(command_events) = command_execution_snapshot_events(&turn.id, item) {
            events.extend(command_events);
            continue;
        }

        let Some(item) = thread_item_to_core(item) else {
            continue;
        };
        match item {
            TurnItem::UserMessage(_) | TurnItem::Plan(_) | TurnItem::AgentMessage(_) => {
                events.push(Event {
                    id: String::new(),
                    msg: EventMsg::ItemCompleted(ItemCompletedEvent {
                        thread_id,
                        turn_id: turn.id.clone(),
                        item,
                    }),
                });
            }
            TurnItem::Reasoning(_)
            | TurnItem::WebSearch(_)
            | TurnItem::ImageGeneration(_)
            | TurnItem::ContextCompaction(_) => {
                events.extend(
                    item.as_legacy_events(show_raw_agent_reasoning)
                        .into_iter()
                        .map(|msg| Event {
                            id: String::new(),
                            msg,
                        }),
                );
            }
            TurnItem::HookPrompt(_) => {}
        }
    }

    append_terminal_turn_events(&mut events, turn, /*include_failed_error*/ true);

    events
}

/// Append the terminal event(s) for a turn based on its `TurnStatus`.
///
/// This function is shared between the live notification bridge
/// (`TurnCompleted` handling) and the snapshot replay path so that both
/// produce identical `EventMsg` sequences for the same turn status.
///
/// - `Completed` → `TurnComplete`
/// - `Interrupted` → `TurnAborted { reason: Interrupted }`
/// - `Failed` → `Error` (if present) then `TurnComplete`
/// - `InProgress` → no events (the turn is still running)
#[cfg(test)]
fn append_terminal_turn_events(events: &mut Vec<Event>, turn: &Turn, include_failed_error: bool) {
    match turn.status {
        TurnStatus::Completed => events.push(Event {
            id: String::new(),
            msg: EventMsg::TurnComplete(TurnCompleteEvent {
                turn_id: turn.id.clone(),
                last_agent_message: None,
                completed_at: turn.completed_at,
                duration_ms: turn.duration_ms,
                time_to_first_token_ms: None,
            }),
        }),
        TurnStatus::Interrupted => events.push(Event {
            id: String::new(),
            msg: EventMsg::TurnAborted(TurnAbortedEvent {
                turn_id: Some(turn.id.clone()),
                reason: TurnAbortReason::Interrupted,
                completed_at: turn.completed_at,
                duration_ms: turn.duration_ms,
            }),
        }),
        TurnStatus::Failed => {
            if include_failed_error && let Some(error) = &turn.error {
                events.push(Event {
                    id: String::new(),
                    msg: EventMsg::Error(ErrorEvent {
                        message: error.message.clone(),
                        codex_error_info: error
                            .codex_error_info
                            .clone()
                            .and_then(app_server_codex_error_info_to_core),
                    }),
                });
            }
            events.push(Event {
                id: String::new(),
                msg: EventMsg::TurnComplete(TurnCompleteEvent {
                    turn_id: turn.id.clone(),
                    last_agent_message: None,
                    completed_at: turn.completed_at,
                    duration_ms: turn.duration_ms,
                    time_to_first_token_ms: None,
                }),
            });
        }
        TurnStatus::InProgress => {
            // Preserve unfinished turns during snapshot replay without emitting completion events.
        }
    }
}

#[cfg(test)]
fn thread_item_to_core(item: &ThreadItem) -> Option<TurnItem> {
    match item {
        ThreadItem::UserMessage { id, content } => Some(TurnItem::UserMessage(UserMessageItem {
            id: id.clone(),
            content: content
                .iter()
                .cloned()
                .map(codex_app_server_protocol::UserInput::into_core)
                .collect(),
        })),
        ThreadItem::AgentMessage {
            id,
            text,
            phase,
            memory_citation,
        } => Some(TurnItem::AgentMessage(AgentMessageItem {
            id: id.clone(),
            content: vec![AgentMessageContent::Text { text: text.clone() }],
            phase: phase.clone(),
            memory_citation: memory_citation.clone().map(|citation| {
                codex_protocol::memory_citation::MemoryCitation {
                    entries: citation
                        .entries
                        .into_iter()
                        .map(
                            |entry| codex_protocol::memory_citation::MemoryCitationEntry {
                                path: entry.path,
                                line_start: entry.line_start,
                                line_end: entry.line_end,
                                note: entry.note,
                            },
                        )
                        .collect(),
                    rollout_ids: citation.thread_ids,
                }
            }),
        })),
        ThreadItem::Plan { id, text } => Some(TurnItem::Plan(PlanItem {
            id: id.clone(),
            text: text.clone(),
        })),
        ThreadItem::Reasoning {
            id,
            summary,
            content,
        } => Some(TurnItem::Reasoning(ReasoningItem {
            id: id.clone(),
            summary_text: summary.clone(),
            raw_content: content.clone(),
        })),
        ThreadItem::WebSearch { id, query, action } => Some(TurnItem::WebSearch(WebSearchItem {
            id: id.clone(),
            query: query.clone(),
            action: app_server_web_search_action_to_core(action.clone()?)?,
        })),
        ThreadItem::ImageGeneration {
            id,
            status,
            revised_prompt,
            result,
            saved_path,
        } => Some(TurnItem::ImageGeneration(ImageGenerationItem {
            id: id.clone(),
            status: status.clone(),
            revised_prompt: revised_prompt.clone(),
            result: result.clone(),
            saved_path: saved_path.clone(),
        })),
        ThreadItem::ContextCompaction { id } => {
            Some(TurnItem::ContextCompaction(ContextCompactionItem {
                id: id.clone(),
            }))
        }
        ThreadItem::CommandExecution { .. }
        | ThreadItem::FileChange { .. }
        | ThreadItem::McpToolCall { .. }
        | ThreadItem::DynamicToolCall { .. }
        | ThreadItem::CollabAgentToolCall { .. }
        | ThreadItem::HookPrompt { .. }
        | ThreadItem::ImageView { .. }
        | ThreadItem::EnteredReviewMode { .. }
        | ThreadItem::ExitedReviewMode { .. } => {
            tracing::debug!("ignoring unsupported app-server thread item in TUI adapter");
            None
        }
    }
}

#[cfg(test)]
fn command_execution_started_event(turn_id: &str, item: &ThreadItem) -> Option<Vec<Event>> {
    let ThreadItem::CommandExecution {
        id,
        command,
        cwd,
        process_id,
        source,
        command_actions,
        ..
    } = item
    else {
        return None;
    };

    Some(vec![Event {
        id: String::new(),
        msg: EventMsg::ExecCommandBegin(ExecCommandBeginEvent {
            call_id: id.clone(),
            process_id: process_id.clone(),
            turn_id: turn_id.to_string(),
            command: split_command_string(command),
            cwd: cwd.clone(),
            parsed_cmd: command_actions
                .iter()
                .cloned()
                .map(codex_app_server_protocol::CommandAction::into_core)
                .collect(),
            source: source.to_core(),
            interaction_input: None,
        }),
    }])
}

#[cfg(test)]
fn command_execution_completed_event(turn_id: &str, item: &ThreadItem) -> Option<Vec<Event>> {
    let ThreadItem::CommandExecution {
        id,
        command,
        cwd,
        process_id,
        source,
        status,
        command_actions,
        aggregated_output,
        exit_code,
        duration_ms,
    } = item
    else {
        return None;
    };

    if matches!(
        status,
        codex_app_server_protocol::CommandExecutionStatus::InProgress
    ) {
        return Some(Vec::new());
    }

    let status = match status {
        codex_app_server_protocol::CommandExecutionStatus::InProgress => return Some(Vec::new()),
        codex_app_server_protocol::CommandExecutionStatus::Completed => {
            ExecCommandStatus::Completed
        }
        codex_app_server_protocol::CommandExecutionStatus::Failed => ExecCommandStatus::Failed,
        codex_app_server_protocol::CommandExecutionStatus::Declined => ExecCommandStatus::Declined,
    };

    let duration = Duration::from_millis(
        duration_ms
            .and_then(|value| u64::try_from(value).ok())
            .unwrap_or_default(),
    );
    let aggregated_output = aggregated_output.clone().unwrap_or_default();

    Some(vec![Event {
        id: String::new(),
        msg: EventMsg::ExecCommandEnd(ExecCommandEndEvent {
            call_id: id.clone(),
            process_id: process_id.clone(),
            turn_id: turn_id.to_string(),
            command: split_command_string(command),
            cwd: cwd.clone(),
            parsed_cmd: command_actions
                .iter()
                .cloned()
                .map(codex_app_server_protocol::CommandAction::into_core)
                .collect(),
            source: source.to_core(),
            interaction_input: None,
            stdout: String::new(),
            stderr: String::new(),
            aggregated_output: aggregated_output.clone(),
            exit_code: exit_code.unwrap_or(-1),
            duration,
            formatted_output: aggregated_output,
            status,
        }),
    }])
}

#[cfg(test)]
fn command_execution_snapshot_events(turn_id: &str, item: &ThreadItem) -> Option<Vec<Event>> {
    let mut events = command_execution_started_event(turn_id, item)?;
    if let Some(end_events) = command_execution_completed_event(turn_id, item) {
        events.extend(end_events);
    }
    Some(events)
}

#[cfg(test)]
fn app_server_web_search_action_to_core(
    action: codex_app_server_protocol::WebSearchAction,
) -> Option<codex_protocol::models::WebSearchAction> {
    match action {
        codex_app_server_protocol::WebSearchAction::Search { query, queries } => {
            Some(codex_protocol::models::WebSearchAction::Search { query, queries })
        }
        codex_app_server_protocol::WebSearchAction::OpenPage { url } => {
            Some(codex_protocol::models::WebSearchAction::OpenPage { url })
        }
        codex_app_server_protocol::WebSearchAction::FindInPage { url, pattern } => {
            Some(codex_protocol::models::WebSearchAction::FindInPage { url, pattern })
        }
        codex_app_server_protocol::WebSearchAction::Other => {
            Some(codex_protocol::models::WebSearchAction::Other)
        }
    }
}

#[cfg(test)]
fn app_server_codex_error_info_to_core(
    value: codex_app_server_protocol::CodexErrorInfo,
) -> Option<codex_protocol::protocol::CodexErrorInfo> {
    serde_json::from_value(serde_json::to_value(value).ok()?).ok()
}

#[cfg(test)]
mod tests {
    use super::ServerNotificationThreadTarget;
    use super::command_execution_started_event;
    use super::server_notification_thread_events;
    use super::server_notification_thread_target;
    use super::thread_snapshot_events;
    use super::turn_snapshot_events;
    use codex_app_server_protocol::AgentMessageDeltaNotification;
    use codex_app_server_protocol::CodexErrorInfo;
    use codex_app_server_protocol::CommandAction;
    use codex_app_server_protocol::CommandExecutionOutputDeltaNotification;
    use codex_app_server_protocol::CommandExecutionSource;
    use codex_app_server_protocol::CommandExecutionStatus;
    use codex_app_server_protocol::GuardianWarningNotification;
    use codex_app_server_protocol::ItemCompletedNotification;
    use codex_app_server_protocol::ItemStartedNotification;
    use codex_app_server_protocol::ReasoningSummaryTextDeltaNotification;
    use codex_app_server_protocol::ServerNotification;
    use codex_app_server_protocol::Thread;
    use codex_app_server_protocol::ThreadItem;
    use codex_app_server_protocol::ThreadStatus;
    use codex_app_server_protocol::Turn;
    use codex_app_server_protocol::TurnCompletedNotification;
    use codex_app_server_protocol::TurnError;
    use codex_app_server_protocol::TurnStatus;
    use codex_app_server_protocol::WarningNotification;
    use codex_protocol::ThreadId;
    use codex_protocol::items::AgentMessageContent;
    use codex_protocol::items::AgentMessageItem;
    use codex_protocol::items::TurnItem;
    use codex_protocol::models::MessagePhase;
    use codex_protocol::protocol::EventMsg;
    use codex_protocol::protocol::ExecCommandSource;
    use codex_protocol::protocol::SessionSource;
    use codex_protocol::protocol::TurnAbortReason;
    use codex_protocol::protocol::TurnAbortedEvent;
    use codex_utils_absolute_path::test_support::PathBufExt;
    use codex_utils_absolute_path::test_support::test_path_buf;
    use pretty_assertions::assert_eq;

    #[test]
    fn bridges_completed_agent_messages_from_server_notifications() {
        let thread_id = "019cee8c-b993-7e33-88c0-014d4e62612d".to_string();
        let turn_id = "019cee8c-b9b4-7f10-a1b0-38caa876a012".to_string();
        let item_id = "msg_123".to_string();

        let (actual_thread_id, events) = server_notification_thread_events(
            ServerNotification::ItemCompleted(ItemCompletedNotification {
                item: ThreadItem::AgentMessage {
                    id: item_id,
                    text: "Hello from your coding assistant.".to_string(),
                    phase: Some(MessagePhase::FinalAnswer),
                    memory_citation: None,
                },
                thread_id: thread_id.clone(),
                turn_id: turn_id.clone(),
            }),
        )
        .expect("notification should bridge");

        assert_eq!(
            actual_thread_id,
            ThreadId::from_string(&thread_id).expect("valid thread id")
        );
        let [event] = events.as_slice() else {
            panic!("expected one bridged event");
        };
        assert_eq!(event.id, String::new());
        let EventMsg::ItemCompleted(completed) = &event.msg else {
            panic!("expected item completed event");
        };
        assert_eq!(
            completed.thread_id,
            ThreadId::from_string(&thread_id).expect("valid thread id")
        );
        assert_eq!(completed.turn_id, turn_id);
        match &completed.item {
            TurnItem::AgentMessage(AgentMessageItem {
                id, content, phase, ..
            }) => {
                assert_eq!(id, "msg_123");
                let [AgentMessageContent::Text { text }] = content.as_slice() else {
                    panic!("expected a single text content item");
                };
                assert_eq!(text, "Hello from your coding assistant.");
                assert_eq!(*phase, Some(MessagePhase::FinalAnswer));
            }
            _ => panic!("expected bridged agent message item"),
        }
    }

    #[test]
    fn bridges_turn_completion_from_server_notifications() {
        let thread_id = "019cee8c-b993-7e33-88c0-014d4e62612d".to_string();
        let turn_id = "019cee8c-b9b4-7f10-a1b0-38caa876a012".to_string();

        let (actual_thread_id, events) = server_notification_thread_events(
            ServerNotification::TurnCompleted(TurnCompletedNotification {
                thread_id: thread_id.clone(),
                turn: Turn {
                    id: turn_id.clone(),
                    items: Vec::new(),
                    status: TurnStatus::Completed,
                    error: None,
                    started_at: None,
                    completed_at: Some(0),
                    duration_ms: None,
                },
            }),
        )
        .expect("notification should bridge");

        assert_eq!(
            actual_thread_id,
            ThreadId::from_string(&thread_id).expect("valid thread id")
        );
        let [event] = events.as_slice() else {
            panic!("expected one bridged event");
        };
        assert_eq!(event.id, String::new());
        let EventMsg::TurnComplete(completed) = &event.msg else {
            panic!("expected turn complete event");
        };
        assert_eq!(completed.turn_id, turn_id);
        assert_eq!(completed.last_agent_message, None);
        assert_eq!(completed.completed_at, Some(0));
        assert_eq!(completed.duration_ms, None);
    }

    #[test]
    fn bridges_command_execution_notifications_into_legacy_exec_events() {
        let thread_id = "019cee8c-b993-7e33-88c0-014d4e62612d".to_string();
        let turn_id = "019cee8c-b9b4-7f10-a1b0-38caa876a012".to_string();
        let item = ThreadItem::CommandExecution {
            id: "cmd-1".to_string(),
            command: "printf 'hello world\\n'".to_string(),
            cwd: test_path_buf("/tmp").abs(),
            process_id: None,
            source: CommandExecutionSource::UserShell,
            status: CommandExecutionStatus::InProgress,
            command_actions: vec![CommandAction::Unknown {
                command: "printf hello world".to_string(),
            }],
            aggregated_output: None,
            exit_code: None,
            duration_ms: None,
        };

        let (_, started_events) = server_notification_thread_events(
            ServerNotification::ItemStarted(ItemStartedNotification {
                item,
                thread_id: thread_id.clone(),
                turn_id: turn_id.clone(),
            }),
        )
        .expect("command execution start should bridge");
        let [started] = started_events.as_slice() else {
            panic!("expected one started event");
        };
        let EventMsg::ExecCommandBegin(begin) = &started.msg else {
            panic!("expected exec begin event");
        };
        assert_eq!(begin.call_id, "cmd-1");
        assert_eq!(
            begin.command,
            vec!["printf".to_string(), "hello world\\n".to_string()]
        );
        assert_eq!(begin.cwd.as_path(), test_path_buf("/tmp").as_path());
        assert_eq!(begin.source, ExecCommandSource::UserShell);

        let (_, delta_events) =
            server_notification_thread_events(ServerNotification::CommandExecutionOutputDelta(
                CommandExecutionOutputDeltaNotification {
                    thread_id: thread_id.clone(),
                    turn_id: turn_id.clone(),
                    item_id: "cmd-1".to_string(),
                    delta: "hello world\n".to_string(),
                },
            ))
            .expect("command execution delta should bridge");
        let [delta] = delta_events.as_slice() else {
            panic!("expected one delta event");
        };
        let EventMsg::ExecCommandOutputDelta(delta) = &delta.msg else {
            panic!("expected exec output delta event");
        };
        assert_eq!(delta.call_id, "cmd-1");
        assert_eq!(delta.chunk, b"hello world\n");

        let completed_item = ThreadItem::CommandExecution {
            id: "cmd-1".to_string(),
            command: "printf 'hello world\\n'".to_string(),
            cwd: test_path_buf("/tmp").abs(),
            process_id: None,
            source: CommandExecutionSource::UserShell,
            status: CommandExecutionStatus::Completed,
            command_actions: vec![CommandAction::Unknown {
                command: "printf hello world".to_string(),
            }],
            aggregated_output: Some("hello world\n".to_string()),
            exit_code: Some(0),
            duration_ms: Some(5),
        };
        let (_, completed_events) = server_notification_thread_events(
            ServerNotification::ItemCompleted(ItemCompletedNotification {
                item: completed_item,
                thread_id,
                turn_id,
            }),
        )
        .expect("command execution completion should bridge");
        let [completed] = completed_events.as_slice() else {
            panic!("expected one completed event");
        };
        let EventMsg::ExecCommandEnd(end) = &completed.msg else {
            panic!("expected exec end event");
        };
        assert_eq!(end.call_id, "cmd-1");
        assert_eq!(end.exit_code, 0);
        assert_eq!(end.formatted_output, "hello world\n");
        assert_eq!(end.aggregated_output, "hello world\n");
        assert_eq!(end.source, ExecCommandSource::UserShell);
    }

    #[test]
    fn command_execution_snapshot_preserves_non_roundtrippable_command_strings() {
        let item = ThreadItem::CommandExecution {
            id: "cmd-1".to_string(),
            command: r#"C:\Program Files\Git\bin\bash.exe -lc "echo hi""#.to_string(),
            cwd: test_path_buf("/tmp").abs(),
            process_id: None,
            source: CommandExecutionSource::UserShell,
            status: CommandExecutionStatus::InProgress,
            command_actions: vec![],
            aggregated_output: None,
            exit_code: None,
            duration_ms: None,
        };

        let events =
            command_execution_started_event("turn-1", &item).expect("command execution start");
        let [started] = events.as_slice() else {
            panic!("expected one started event");
        };
        let EventMsg::ExecCommandBegin(begin) = &started.msg else {
            panic!("expected exec begin event");
        };
        assert_eq!(
            begin.command,
            vec![r#"C:\Program Files\Git\bin\bash.exe -lc "echo hi""#.to_string()]
        );
    }

    #[test]
    fn replays_command_execution_items_from_thread_snapshots() {
        let thread = Thread {
            id: "019cee8c-b993-7e33-88c0-014d4e62612d".to_string(),
            forked_from_id: None,
            preview: String::new(),
            ephemeral: false,
            model_provider: "openai".to_string(),
            created_at: 1,
            updated_at: 1,
            status: ThreadStatus::Idle,
            path: None,
            cwd: test_path_buf("/tmp").abs(),
            cli_version: "test".to_string(),
            source: SessionSource::Cli.into(),
            agent_nickname: None,
            agent_role: None,
            git_info: None,
            name: None,
            turns: vec![Turn {
                id: "turn-1".to_string(),
                items: vec![ThreadItem::CommandExecution {
                    id: "cmd-1".to_string(),
                    command: "printf 'hello world\\n'".to_string(),
                    cwd: test_path_buf("/tmp").abs(),
                    process_id: None,
                    source: CommandExecutionSource::UserShell,
                    status: CommandExecutionStatus::Completed,
                    command_actions: vec![CommandAction::Unknown {
                        command: "printf hello world".to_string(),
                    }],
                    aggregated_output: Some("hello world\n".to_string()),
                    exit_code: Some(0),
                    duration_ms: Some(5),
                }],
                status: TurnStatus::Completed,
                error: None,
                started_at: None,
                completed_at: None,
                duration_ms: None,
            }],
        };

        let events = thread_snapshot_events(&thread, /*show_raw_agent_reasoning*/ false);
        assert!(matches!(events[0].msg, EventMsg::TurnStarted(_)));
        let EventMsg::ExecCommandBegin(begin) = &events[1].msg else {
            panic!("expected exec begin event");
        };
        assert_eq!(begin.call_id, "cmd-1");
        assert_eq!(begin.source, ExecCommandSource::UserShell);
        let EventMsg::ExecCommandEnd(end) = &events[2].msg else {
            panic!("expected exec end event");
        };
        assert_eq!(end.call_id, "cmd-1");
        assert_eq!(end.formatted_output, "hello world\n");
        assert!(matches!(events[3].msg, EventMsg::TurnComplete(_)));
    }

    #[test]
    fn bridges_interrupted_turn_completion_from_server_notifications() {
        let thread_id = "019cee8c-b993-7e33-88c0-014d4e62612d".to_string();
        let turn_id = "019cee8c-b9b4-7f10-a1b0-38caa876a012".to_string();

        let (actual_thread_id, events) = server_notification_thread_events(
            ServerNotification::TurnCompleted(TurnCompletedNotification {
                thread_id: thread_id.clone(),
                turn: Turn {
                    id: turn_id.clone(),
                    items: Vec::new(),
                    status: TurnStatus::Interrupted,
                    error: None,
                    started_at: None,
                    completed_at: Some(0),
                    duration_ms: None,
                },
            }),
        )
        .expect("notification should bridge");

        assert_eq!(
            actual_thread_id,
            ThreadId::from_string(&thread_id).expect("valid thread id")
        );
        let [event] = events.as_slice() else {
            panic!("expected one bridged event");
        };
        let EventMsg::TurnAborted(aborted) = &event.msg else {
            panic!("expected turn aborted event");
        };
        assert_eq!(aborted.turn_id.as_deref(), Some(turn_id.as_str()));
        assert_eq!(aborted.reason, TurnAbortReason::Interrupted);
    }

    #[test]
    fn bridges_failed_turn_completion_from_server_notifications() {
        let thread_id = "019cee8c-b993-7e33-88c0-014d4e62612d".to_string();
        let turn_id = "019cee8c-b9b4-7f10-a1b0-38caa876a012".to_string();

        let (actual_thread_id, events) = server_notification_thread_events(
            ServerNotification::TurnCompleted(TurnCompletedNotification {
                thread_id: thread_id.clone(),
                turn: Turn {
                    id: turn_id.clone(),
                    items: Vec::new(),
                    status: TurnStatus::Failed,
                    error: Some(TurnError {
                        message: "request failed".to_string(),
                        codex_error_info: Some(CodexErrorInfo::Other),
                        additional_details: None,
                    }),
                    started_at: None,
                    completed_at: Some(0),
                    duration_ms: None,
                },
            }),
        )
        .expect("notification should bridge");

        assert_eq!(
            actual_thread_id,
            ThreadId::from_string(&thread_id).expect("valid thread id")
        );
        let [complete_event] = events.as_slice() else {
            panic!("expected turn completion only");
        };
        let EventMsg::TurnComplete(completed) = &complete_event.msg else {
            panic!("expected turn complete event");
        };
        assert_eq!(completed.turn_id, turn_id);
        assert_eq!(completed.last_agent_message, None);
    }

    #[test]
    fn bridges_text_deltas_from_server_notifications() {
        let thread_id = "019cee8c-b993-7e33-88c0-014d4e62612d".to_string();

        let (_, agent_events) = server_notification_thread_events(
            ServerNotification::AgentMessageDelta(AgentMessageDeltaNotification {
                thread_id: thread_id.clone(),
                turn_id: "turn".to_string(),
                item_id: "item".to_string(),
                delta: "Hello".to_string(),
            }),
        )
        .expect("notification should bridge");
        let [agent_event] = agent_events.as_slice() else {
            panic!("expected one bridged agent delta event");
        };
        assert_eq!(agent_event.id, String::new());
        let EventMsg::AgentMessageDelta(delta) = &agent_event.msg else {
            panic!("expected bridged agent message delta");
        };
        assert_eq!(delta.delta, "Hello");

        let (_, reasoning_events) = server_notification_thread_events(
            ServerNotification::ReasoningSummaryTextDelta(ReasoningSummaryTextDeltaNotification {
                thread_id,
                turn_id: "turn".to_string(),
                item_id: "item".to_string(),
                delta: "Thinking".to_string(),
                summary_index: 0,
            }),
        )
        .expect("notification should bridge");
        let [reasoning_event] = reasoning_events.as_slice() else {
            panic!("expected one bridged reasoning delta event");
        };
        assert_eq!(reasoning_event.id, String::new());
        let EventMsg::AgentReasoningDelta(delta) = &reasoning_event.msg else {
            panic!("expected bridged reasoning delta");
        };
        assert_eq!(delta.delta, "Thinking");
    }

    #[test]
    fn bridges_thread_snapshot_turns_for_resume_restore() {
        let thread_id = ThreadId::new();
        let events = thread_snapshot_events(
            &Thread {
                id: thread_id.to_string(),
                forked_from_id: None,
                preview: "hello".to_string(),
                ephemeral: false,
                model_provider: "openai".to_string(),
                created_at: 0,
                updated_at: 0,
                status: ThreadStatus::Idle,
                path: None,
                cwd: test_path_buf("/tmp/project").abs(),
                cli_version: "test".to_string(),
                source: SessionSource::Cli.into(),
                agent_nickname: None,
                agent_role: None,
                git_info: None,
                name: Some("restore".to_string()),
                turns: vec![
                    Turn {
                        id: "turn-complete".to_string(),
                        items: vec![
                            ThreadItem::UserMessage {
                                id: "user-1".to_string(),
                                content: vec![codex_app_server_protocol::UserInput::Text {
                                    text: "hello".to_string(),
                                    text_elements: Vec::new(),
                                }],
                            },
                            ThreadItem::AgentMessage {
                                id: "assistant-1".to_string(),
                                text: "hi".to_string(),
                                phase: Some(MessagePhase::FinalAnswer),
                                memory_citation: None,
                            },
                        ],
                        status: TurnStatus::Completed,
                        error: None,
                        started_at: None,
                        completed_at: None,
                        duration_ms: None,
                    },
                    Turn {
                        id: "turn-interrupted".to_string(),
                        items: Vec::new(),
                        status: TurnStatus::Interrupted,
                        error: None,
                        started_at: None,
                        completed_at: None,
                        duration_ms: None,
                    },
                    Turn {
                        id: "turn-failed".to_string(),
                        items: Vec::new(),
                        status: TurnStatus::Failed,
                        error: Some(TurnError {
                            message: "request failed".to_string(),
                            codex_error_info: Some(CodexErrorInfo::Other),
                            additional_details: None,
                        }),
                        started_at: None,
                        completed_at: None,
                        duration_ms: None,
                    },
                ],
            },
            /*show_raw_agent_reasoning*/ false,
        );

        assert_eq!(events.len(), 9);
        assert!(matches!(events[0].msg, EventMsg::TurnStarted(_)));
        assert!(matches!(events[1].msg, EventMsg::ItemCompleted(_)));
        assert!(matches!(events[2].msg, EventMsg::ItemCompleted(_)));
        assert!(matches!(events[3].msg, EventMsg::TurnComplete(_)));
        assert!(matches!(events[4].msg, EventMsg::TurnStarted(_)));
        let EventMsg::TurnAborted(TurnAbortedEvent {
            turn_id, reason, ..
        }) = &events[5].msg
        else {
            panic!("expected interrupted turn replay");
        };
        assert_eq!(turn_id.as_deref(), Some("turn-interrupted"));
        assert_eq!(*reason, TurnAbortReason::Interrupted);
        assert!(matches!(events[6].msg, EventMsg::TurnStarted(_)));
        let EventMsg::Error(error) = &events[7].msg else {
            panic!("expected failed turn error replay");
        };
        assert_eq!(error.message, "request failed");
        assert_eq!(
            error.codex_error_info,
            Some(codex_protocol::protocol::CodexErrorInfo::Other)
        );
        assert!(matches!(events[8].msg, EventMsg::TurnComplete(_)));
    }

    #[test]
    fn bridges_non_message_snapshot_items_via_legacy_events() {
        let events = turn_snapshot_events(
            ThreadId::new(),
            &Turn {
                id: "turn-complete".to_string(),
                items: vec![
                    ThreadItem::Reasoning {
                        id: "reasoning-1".to_string(),
                        summary: vec!["Need to inspect config".to_string()],
                        content: vec!["hidden chain".to_string()],
                    },
                    ThreadItem::WebSearch {
                        id: "search-1".to_string(),
                        query: "ratatui stylize".to_string(),
                        action: Some(codex_app_server_protocol::WebSearchAction::Other),
                    },
                    ThreadItem::ImageGeneration {
                        id: "image-1".to_string(),
                        status: "completed".to_string(),
                        revised_prompt: Some("diagram".to_string()),
                        result: "image.png".to_string(),
                        saved_path: None,
                    },
                    ThreadItem::ContextCompaction {
                        id: "compact-1".to_string(),
                    },
                ],
                status: TurnStatus::Completed,
                error: None,
                started_at: None,
                completed_at: None,
                duration_ms: None,
            },
            /*show_raw_agent_reasoning*/ false,
        );

        assert_eq!(events.len(), 6);
        assert!(matches!(events[0].msg, EventMsg::TurnStarted(_)));
        let EventMsg::AgentReasoning(reasoning) = &events[1].msg else {
            panic!("expected reasoning replay");
        };
        assert_eq!(reasoning.text, "Need to inspect config");
        let EventMsg::WebSearchEnd(web_search) = &events[2].msg else {
            panic!("expected web search replay");
        };
        assert_eq!(web_search.call_id, "search-1");
        assert_eq!(web_search.query, "ratatui stylize");
        assert_eq!(
            web_search.action,
            codex_protocol::models::WebSearchAction::Other
        );
        let EventMsg::ImageGenerationEnd(image_generation) = &events[3].msg else {
            panic!("expected image generation replay");
        };
        assert_eq!(image_generation.call_id, "image-1");
        assert_eq!(image_generation.status, "completed");
        assert_eq!(image_generation.revised_prompt.as_deref(), Some("diagram"));
        assert_eq!(image_generation.result, "image.png");
        assert!(matches!(events[4].msg, EventMsg::ContextCompacted(_)));
        assert!(matches!(events[5].msg, EventMsg::TurnComplete(_)));
    }

    #[test]
    fn bridges_raw_reasoning_snapshot_items_when_enabled() {
        let events = turn_snapshot_events(
            ThreadId::new(),
            &Turn {
                id: "turn-complete".to_string(),
                items: vec![ThreadItem::Reasoning {
                    id: "reasoning-1".to_string(),
                    summary: vec!["Need to inspect config".to_string()],
                    content: vec!["hidden chain".to_string()],
                }],
                status: TurnStatus::Completed,
                error: None,
                started_at: None,
                completed_at: None,
                duration_ms: None,
            },
            /*show_raw_agent_reasoning*/ true,
        );

        assert_eq!(events.len(), 4);
        assert!(matches!(events[0].msg, EventMsg::TurnStarted(_)));
        let EventMsg::AgentReasoning(reasoning) = &events[1].msg else {
            panic!("expected reasoning replay");
        };
        assert_eq!(reasoning.text, "Need to inspect config");
        let EventMsg::AgentReasoningRawContent(raw_reasoning) = &events[2].msg else {
            panic!("expected raw reasoning replay");
        };
        assert_eq!(raw_reasoning.text, "hidden chain");
        assert!(matches!(events[3].msg, EventMsg::TurnComplete(_)));
    }

    #[test]
    fn warning_notifications_route_to_threads_when_thread_id_is_present() {
        let thread_id = ThreadId::new();
        let notification = ServerNotification::Warning(WarningNotification {
            thread_id: Some(thread_id.to_string()),
            message: "warning".to_string(),
        });

        let target = server_notification_thread_target(&notification);

        assert_eq!(target, ServerNotificationThreadTarget::Thread(thread_id));
    }

    #[test]
    fn guardian_warning_notifications_route_to_threads() {
        let thread_id = ThreadId::new();
        let notification = ServerNotification::GuardianWarning(GuardianWarningNotification {
            thread_id: thread_id.to_string(),
            message: "warning".to_string(),
        });

        let target = server_notification_thread_target(&notification);

        assert_eq!(target, ServerNotificationThreadTarget::Thread(thread_id));
    }
}
