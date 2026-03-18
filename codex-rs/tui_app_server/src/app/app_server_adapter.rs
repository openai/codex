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
use crate::app_event::AppEvent;
use crate::app_server_session::AppServerSession;
use crate::app_server_session::app_server_rate_limit_snapshot_to_core;
use crate::app_server_session::status_account_display_from_auth_mode;
use crate::local_chatgpt_auth::load_local_chatgpt_auth;
use codex_app_server_client::AppServerEvent;
use codex_app_server_protocol::ChatgptAuthTokensRefreshParams;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::JSONRPCNotification;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ServerRequest;
use codex_app_server_protocol::Thread;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::Turn;
use codex_app_server_protocol::TurnStatus;
use codex_protocol::ThreadId;
use codex_protocol::config_types::ModeKind;
use codex_protocol::items::AgentMessageContent;
use codex_protocol::items::AgentMessageItem;
use codex_protocol::items::ContextCompactionItem;
use codex_protocol::items::ImageGenerationItem;
use codex_protocol::items::PlanItem;
use codex_protocol::items::ReasoningItem;
use codex_protocol::items::TurnItem;
use codex_protocol::items::UserMessageItem;
use codex_protocol::items::WebSearchItem;
use codex_protocol::protocol::AgentMessageDeltaEvent;
use codex_protocol::protocol::AgentReasoningDeltaEvent;
use codex_protocol::protocol::AgentReasoningRawContentDeltaEvent;
use codex_protocol::protocol::ErrorEvent;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ExecCommandBeginEvent;
use codex_protocol::protocol::ExecCommandEndEvent;
use codex_protocol::protocol::ExecCommandOutputDeltaEvent;
use codex_protocol::protocol::ExecCommandStatus;
use codex_protocol::protocol::ExecOutputStream;
use codex_protocol::protocol::ItemCompletedEvent;
use codex_protocol::protocol::ItemStartedEvent;
use codex_protocol::protocol::PlanDeltaEvent;
use codex_protocol::protocol::RealtimeConversationClosedEvent;
use codex_protocol::protocol::RealtimeConversationRealtimeEvent;
use codex_protocol::protocol::RealtimeConversationStartedEvent;
use codex_protocol::protocol::RealtimeEvent;
use codex_protocol::protocol::ThreadNameUpdatedEvent;
use codex_protocol::protocol::TokenCountEvent;
use codex_protocol::protocol::TokenUsage;
use codex_protocol::protocol::TokenUsageInfo;
use codex_protocol::protocol::TurnAbortReason;
use codex_protocol::protocol::TurnAbortedEvent;
use codex_protocol::protocol::TurnCompleteEvent;
use codex_protocol::protocol::TurnStartedEvent;
use serde_json::Value;
use std::time::Duration;

#[derive(Debug, PartialEq, Eq)]
enum LegacyThreadNotification {
    Warning(String),
    Rollback { num_turns: u32 },
}

impl App {
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
            }
            AppServerEvent::ServerNotification(notification) => match notification {
                ServerNotification::ServerRequestResolved(notification) => {
                    self.pending_app_server_requests
                        .resolve_notification(&notification.request_id);
                }
                ServerNotification::AccountRateLimitsUpdated(notification) => {
                    self.chat_widget.on_rate_limit_snapshot(Some(
                        app_server_rate_limit_snapshot_to_core(notification.rate_limits),
                    ));
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
                            Some(codex_app_server_protocol::AuthMode::Chatgpt)
                                | Some(codex_app_server_protocol::AuthMode::ChatgptAuthTokens)
                        ),
                    );
                }
                notification => {
                    if !app_server_client.is_remote()
                        && matches!(
                            notification,
                            ServerNotification::TurnCompleted(_)
                                | ServerNotification::ThreadRealtimeItemAdded(_)
                                | ServerNotification::ThreadRealtimeOutputAudioDelta(_)
                                | ServerNotification::ThreadRealtimeError(_)
                        )
                    {
                        return;
                    }
                    if let Some((thread_id, events)) =
                        server_notification_thread_events(notification)
                    {
                        for event in events {
                            if self.primary_thread_id.is_none()
                                || matches!(event.msg, EventMsg::SessionConfigured(_))
                                    && self.primary_thread_id == Some(thread_id)
                            {
                                if let Err(err) = self.enqueue_primary_event(event).await {
                                    tracing::warn!(
                                        "failed to enqueue primary app-server server notification: {err}"
                                    );
                                }
                            } else if let Err(err) =
                                self.enqueue_thread_event(thread_id, event).await
                            {
                                tracing::warn!(
                                    "failed to enqueue app-server server notification for {thread_id}: {err}"
                                );
                            }
                        }
                    }
                }
            },
            AppServerEvent::LegacyNotification(notification) => {
                if let Some((thread_id, legacy_notification)) =
                    legacy_thread_notification(notification.clone())
                {
                    let result = match legacy_notification {
                        LegacyThreadNotification::Warning(message) => {
                            if self.primary_thread_id == Some(thread_id)
                                || self.primary_thread_id.is_none()
                            {
                                self.enqueue_primary_thread_legacy_warning(message).await
                            } else {
                                self.enqueue_thread_legacy_warning(thread_id, message).await
                            }
                        }
                        LegacyThreadNotification::Rollback { num_turns } => {
                            if self.primary_thread_id == Some(thread_id)
                                || self.primary_thread_id.is_none()
                            {
                                self.enqueue_primary_thread_legacy_rollback(num_turns).await
                            } else {
                                self.enqueue_thread_legacy_rollback(thread_id, num_turns)
                                    .await
                            }
                        }
                    };
                    if let Err(err) = result {
                        tracing::warn!("failed to enqueue app-server legacy notification: {err}");
                    }
                } else if let Some((thread_id, event)) = legacy_thread_event(notification.params) {
                    self.pending_app_server_requests.note_legacy_event(&event);
                    if legacy_event_is_shadowed_by_server_notification(&event.msg) {
                        return;
                    }
                    if self.primary_thread_id.is_none()
                        || matches!(event.msg, EventMsg::SessionConfigured(_))
                            && self.primary_thread_id == Some(thread_id)
                    {
                        if let Err(err) = self.enqueue_primary_event(event).await {
                            tracing::warn!("failed to enqueue primary app-server event: {err}");
                        }
                    } else if let Err(err) = self.enqueue_thread_event(thread_id, event).await {
                        tracing::warn!(
                            "failed to enqueue app-server thread event for {thread_id}: {err}"
                        );
                    }
                }
            }
            AppServerEvent::ServerRequest(request) => {
                if let ServerRequest::ChatgptAuthTokensRefresh { request_id, params } = request {
                    self.handle_chatgpt_auth_tokens_refresh_request(
                        app_server_client,
                        request_id,
                        params,
                    )
                    .await;
                    return;
                }
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
                }
            }
            AppServerEvent::Disconnected { message } => {
                tracing::warn!("app-server event stream disconnected: {message}");
                self.chat_widget.add_error_message(message.clone());
                self.app_event_tx.send(AppEvent::FatalExitRequest(message));
            }
        }
    }

    async fn handle_chatgpt_auth_tokens_refresh_request(
        &mut self,
        app_server_client: &AppServerSession,
        request_id: codex_app_server_protocol::RequestId,
        params: ChatgptAuthTokensRefreshParams,
    ) {
        let config = self.config.clone();
        let result = tokio::task::spawn_blocking(move || {
            resolve_chatgpt_auth_tokens_refresh_response(
                &config.codex_home,
                config.cli_auth_credentials_store_mode,
                config.forced_chatgpt_workspace_id.as_deref(),
                &params,
            )
        })
        .await;

        match result {
            Ok(Ok(response)) => {
                let response = serde_json::to_value(response).map_err(|err| {
                    format!("failed to serialize chatgpt auth refresh response: {err}")
                });
                match response {
                    Ok(response) => {
                        if let Err(err) = app_server_client
                            .resolve_server_request(request_id, response)
                            .await
                        {
                            tracing::warn!("failed to resolve chatgpt auth refresh request: {err}");
                        }
                    }
                    Err(err) => {
                        self.chat_widget.add_error_message(err.clone());
                        if let Err(reject_err) = self
                            .reject_app_server_request(app_server_client, request_id, err)
                            .await
                        {
                            tracing::warn!("{reject_err}");
                        }
                    }
                }
            }
            Ok(Err(err)) => {
                self.chat_widget.add_error_message(err.clone());
                if let Err(reject_err) = self
                    .reject_app_server_request(app_server_client, request_id, err)
                    .await
                {
                    tracing::warn!("{reject_err}");
                }
            }
            Err(err) => {
                let message = format!("chatgpt auth refresh task failed: {err}");
                self.chat_widget.add_error_message(message.clone());
                if let Err(reject_err) = self
                    .reject_app_server_request(app_server_client, request_id, message)
                    .await
                {
                    tracing::warn!("{reject_err}");
                }
            }
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

fn resolve_chatgpt_auth_tokens_refresh_response(
    codex_home: &std::path::Path,
    auth_credentials_store_mode: codex_core::auth::AuthCredentialsStoreMode,
    forced_chatgpt_workspace_id: Option<&str>,
    params: &ChatgptAuthTokensRefreshParams,
) -> Result<codex_app_server_protocol::ChatgptAuthTokensRefreshResponse, String> {
    let auth = load_local_chatgpt_auth(
        codex_home,
        auth_credentials_store_mode,
        forced_chatgpt_workspace_id,
    )?;
    if let Some(previous_account_id) = params.previous_account_id.as_deref()
        && previous_account_id != auth.chatgpt_account_id
    {
        return Err(format!(
            "local ChatGPT auth refresh account mismatch: expected `{previous_account_id}`, got `{}`",
            auth.chatgpt_account_id
        ));
    }
    Ok(auth.to_refresh_response())
}

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

fn legacy_thread_event(params: Option<Value>) -> Option<(ThreadId, Event)> {
    let Value::Object(mut params) = params? else {
        return None;
    };
    let thread_id = params
        .remove("conversationId")
        .and_then(|value| serde_json::from_value::<String>(value).ok())
        .and_then(|value| ThreadId::from_string(&value).ok());
    let event = serde_json::from_value::<Event>(Value::Object(params)).ok()?;
    let thread_id = thread_id.or(match &event.msg {
        EventMsg::SessionConfigured(session) => Some(session.session_id),
        _ => None,
    })?;
    Some((thread_id, event))
}

fn legacy_thread_notification(
    notification: JSONRPCNotification,
) -> Option<(ThreadId, LegacyThreadNotification)> {
    let method = notification
        .method
        .strip_prefix("codex/event/")
        .unwrap_or(&notification.method);

    let Value::Object(mut params) = notification.params? else {
        return None;
    };
    let thread_id = params
        .remove("conversationId")
        .and_then(|value| serde_json::from_value::<String>(value).ok())
        .and_then(|value| ThreadId::from_string(&value).ok())?;
    let msg = params.get("msg").and_then(Value::as_object)?;

    match method {
        "warning" => {
            let message = msg
                .get("type")
                .and_then(Value::as_str)
                .zip(msg.get("message"))
                .and_then(|(kind, message)| (kind == "warning").then_some(message))
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)?;
            Some((thread_id, LegacyThreadNotification::Warning(message)))
        }
        "thread_rolled_back" => {
            let num_turns = msg
                .get("type")
                .and_then(Value::as_str)
                .zip(msg.get("num_turns"))
                .and_then(|(kind, num_turns)| (kind == "thread_rolled_back").then_some(num_turns))
                .and_then(Value::as_u64)
                .and_then(|num_turns| u32::try_from(num_turns).ok())?;
            Some((thread_id, LegacyThreadNotification::Rollback { num_turns }))
        }
        _ => None,
    }
}

fn legacy_event_is_shadowed_by_server_notification(msg: &EventMsg) -> bool {
    matches!(
        msg,
        EventMsg::TokenCount(_)
            | EventMsg::Error(_)
            | EventMsg::ThreadNameUpdated(_)
            | EventMsg::TurnStarted(_)
            | EventMsg::ItemStarted(_)
            | EventMsg::ItemCompleted(_)
            | EventMsg::ExecCommandBegin(_)
            | EventMsg::ExecCommandOutputDelta(_)
            | EventMsg::ExecCommandEnd(_)
            | EventMsg::AgentMessageDelta(_)
            | EventMsg::PlanDelta(_)
            | EventMsg::AgentReasoningDelta(_)
            | EventMsg::AgentReasoningRawContentDelta(_)
            | EventMsg::RealtimeConversationStarted(_)
            | EventMsg::RealtimeConversationClosed(_)
    )
}

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
                    chunk: notification.delta,
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
fn turn_snapshot_events(
    thread_id: ThreadId,
    turn: &Turn,
    show_raw_agent_reasoning: bool,
) -> Vec<Event> {
    let mut events = vec![Event {
        id: String::new(),
        msg: EventMsg::TurnStarted(TurnStartedEvent {
            turn_id: turn.id.clone(),
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
fn append_terminal_turn_events(events: &mut Vec<Event>, turn: &Turn, include_failed_error: bool) {
    match turn.status {
        TurnStatus::Completed => events.push(Event {
            id: String::new(),
            msg: EventMsg::TurnComplete(TurnCompleteEvent {
                turn_id: turn.id.clone(),
                last_agent_message: None,
            }),
        }),
        TurnStatus::Interrupted => events.push(Event {
            id: String::new(),
            msg: EventMsg::TurnAborted(TurnAbortedEvent {
                turn_id: Some(turn.id.clone()),
                reason: TurnAbortReason::Interrupted,
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
                }),
            });
        }
        TurnStatus::InProgress => {
            // Preserve unfinished turns during snapshot replay without emitting completion events.
        }
    }
}

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
        ThreadItem::AgentMessage { id, text, phase } => {
            Some(TurnItem::AgentMessage(AgentMessageItem {
                id: id.clone(),
                content: vec![AgentMessageContent::Text { text: text.clone() }],
                phase: phase.clone(),
            }))
        }
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
        } => Some(TurnItem::ImageGeneration(ImageGenerationItem {
            id: id.clone(),
            status: status.clone(),
            revised_prompt: revised_prompt.clone(),
            result: result.clone(),
            saved_path: None,
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
        | ThreadItem::ImageView { .. }
        | ThreadItem::EnteredReviewMode { .. }
        | ThreadItem::ExitedReviewMode { .. } => {
            tracing::debug!("ignoring unsupported app-server thread item in TUI adapter");
            None
        }
    }
}

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
        formatted_output,
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
            formatted_output: if formatted_output.is_empty() {
                aggregated_output
            } else {
                formatted_output.clone()
            },
            status,
        }),
    }])
}

fn command_execution_snapshot_events(turn_id: &str, item: &ThreadItem) -> Option<Vec<Event>> {
    let mut events = command_execution_started_event(turn_id, item)?;
    if let Some(end_events) = command_execution_completed_event(turn_id, item) {
        events.extend(end_events);
    }
    Some(events)
}

fn split_command_string(command: &str) -> Vec<String> {
    let Some(parts) = shlex::split(command) else {
        return vec![command.to_string()];
    };
    match shlex::try_join(parts.iter().map(String::as_str)) {
        Ok(round_trip)
            if round_trip == command
                || (!command.contains(":\\")
                    && shlex::split(&round_trip).as_ref() == Some(&parts)) =>
        {
            parts
        }
        _ => vec![command.to_string()],
    }
}

#[cfg(test)]
mod refresh_tests {
    use super::*;

    use base64::Engine;
    use chrono::Utc;
    use codex_app_server_protocol::AuthMode;
    use codex_core::auth::AuthCredentialsStoreMode;
    use codex_core::auth::AuthDotJson;
    use codex_core::auth::save_auth;
    use codex_core::token_data::TokenData;
    use pretty_assertions::assert_eq;
    use serde::Serialize;
    use serde_json::json;
    use tempfile::TempDir;

    fn fake_jwt(account_id: &str, plan_type: &str) -> String {
        #[derive(Serialize)]
        struct Header {
            alg: &'static str,
            typ: &'static str,
        }

        let header = Header {
            alg: "none",
            typ: "JWT",
        };
        let payload = json!({
            "email": "user@example.com",
            "https://api.openai.com/auth": {
                "chatgpt_account_id": account_id,
                "chatgpt_plan_type": plan_type,
            },
        });
        let encode = |bytes: &[u8]| base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes);
        let header_b64 = encode(&serde_json::to_vec(&header).expect("serialize header"));
        let payload_b64 = encode(&serde_json::to_vec(&payload).expect("serialize payload"));
        let signature_b64 = encode(b"sig");
        format!("{header_b64}.{payload_b64}.{signature_b64}")
    }

    fn write_chatgpt_auth(codex_home: &std::path::Path) {
        let id_token = fake_jwt("workspace-1", "business");
        let access_token = fake_jwt("workspace-1", "business");
        save_auth(
            codex_home,
            &AuthDotJson {
                auth_mode: Some(AuthMode::Chatgpt),
                openai_api_key: None,
                tokens: Some(TokenData {
                    id_token: codex_core::token_data::parse_chatgpt_jwt_claims(&id_token)
                        .expect("id token should parse"),
                    access_token,
                    refresh_token: "refresh-token".to_string(),
                    account_id: Some("workspace-1".to_string()),
                }),
                last_refresh: Some(Utc::now()),
            },
            AuthCredentialsStoreMode::File,
        )
        .expect("chatgpt auth should save");
    }

    #[test]
    fn refresh_request_uses_local_chatgpt_auth() {
        let codex_home = TempDir::new().expect("tempdir");
        write_chatgpt_auth(codex_home.path());

        let response = resolve_chatgpt_auth_tokens_refresh_response(
            codex_home.path(),
            AuthCredentialsStoreMode::File,
            Some("workspace-1"),
            &ChatgptAuthTokensRefreshParams {
                reason: codex_app_server_protocol::ChatgptAuthTokensRefreshReason::Unauthorized,
                previous_account_id: Some("workspace-1".to_string()),
            },
        )
        .expect("refresh response should resolve");

        assert_eq!(response.chatgpt_account_id, "workspace-1");
        assert_eq!(response.chatgpt_plan_type.as_deref(), Some("business"));
        assert!(!response.access_token.is_empty());
    }

    #[test]
    fn refresh_request_rejects_account_mismatch() {
        let codex_home = TempDir::new().expect("tempdir");
        write_chatgpt_auth(codex_home.path());

        let err = resolve_chatgpt_auth_tokens_refresh_response(
            codex_home.path(),
            AuthCredentialsStoreMode::File,
            Some("workspace-1"),
            &ChatgptAuthTokensRefreshParams {
                reason: codex_app_server_protocol::ChatgptAuthTokensRefreshReason::Unauthorized,
                previous_account_id: Some("workspace-2".to_string()),
            },
        )
        .expect_err("mismatched account should fail");

        assert_eq!(
            err,
            "local ChatGPT auth refresh account mismatch: expected `workspace-2`, got `workspace-1`"
        );
    }
}

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

fn app_server_codex_error_info_to_core(
    value: codex_app_server_protocol::CodexErrorInfo,
) -> Option<codex_protocol::protocol::CodexErrorInfo> {
    serde_json::from_value(serde_json::to_value(value).ok()?).ok()
}

#[cfg(test)]
mod tests {
    use super::LegacyThreadNotification;
    use super::command_execution_started_event;
    use super::legacy_thread_notification;
    use super::server_notification_thread_events;
    use super::thread_snapshot_events;
    use super::turn_snapshot_events;
    use codex_app_server_protocol::AgentMessageDeltaNotification;
    use codex_app_server_protocol::CodexErrorInfo;
    use codex_app_server_protocol::CommandAction;
    use codex_app_server_protocol::CommandExecutionOutputDeltaNotification;
    use codex_app_server_protocol::CommandExecutionSource;
    use codex_app_server_protocol::CommandExecutionStatus;
    use codex_app_server_protocol::ItemCompletedNotification;
    use codex_app_server_protocol::ItemStartedNotification;
    use codex_app_server_protocol::JSONRPCNotification;
    use codex_app_server_protocol::ReasoningSummaryTextDeltaNotification;
    use codex_app_server_protocol::ServerNotification;
    use codex_app_server_protocol::Thread;
    use codex_app_server_protocol::ThreadItem;
    use codex_app_server_protocol::ThreadStatus;
    use codex_app_server_protocol::Turn;
    use codex_app_server_protocol::TurnCompletedNotification;
    use codex_app_server_protocol::TurnError;
    use codex_app_server_protocol::TurnStatus;
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
    use pretty_assertions::assert_eq;
    use serde_json::json;
    use std::path::PathBuf;

    #[test]
    fn legacy_warning_notification_extracts_thread_id_and_message() {
        let thread_id = ThreadId::new();
        let warning = legacy_thread_notification(JSONRPCNotification {
            method: "codex/event/warning".to_string(),
            params: Some(json!({
                "conversationId": thread_id.to_string(),
                "id": "event-1",
                "msg": {
                    "type": "warning",
                    "message": "legacy warning message",
                },
            })),
        });

        assert_eq!(
            warning,
            Some((
                thread_id,
                LegacyThreadNotification::Warning("legacy warning message".to_string())
            ))
        );
    }

    #[test]
    fn legacy_thread_rollback_notification_extracts_thread_id_and_turn_count() {
        let thread_id = ThreadId::new();
        let rollback = legacy_thread_notification(JSONRPCNotification {
            method: "codex/event/thread_rolled_back".to_string(),
            params: Some(json!({
                "conversationId": thread_id.to_string(),
                "id": "event-1",
                "msg": {
                    "type": "thread_rolled_back",
                    "num_turns": 2,
                },
            })),
        });

        assert_eq!(
            rollback,
            Some((
                thread_id,
                LegacyThreadNotification::Rollback { num_turns: 2 }
            ))
        );
    }

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
            TurnItem::AgentMessage(AgentMessageItem { id, content, phase }) => {
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
    }

    #[test]
    fn bridges_command_execution_notifications_into_legacy_exec_events() {
        let thread_id = "019cee8c-b993-7e33-88c0-014d4e62612d".to_string();
        let turn_id = "019cee8c-b9b4-7f10-a1b0-38caa876a012".to_string();
        let item = ThreadItem::CommandExecution {
            id: "cmd-1".to_string(),
            command: "printf 'hello world\\n'".to_string(),
            cwd: PathBuf::from("/tmp"),
            process_id: None,
            source: CommandExecutionSource::UserShell,
            status: CommandExecutionStatus::InProgress,
            command_actions: vec![CommandAction::Unknown {
                command: "printf hello world".to_string(),
            }],
            aggregated_output: None,
            formatted_output: String::new(),
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
        assert_eq!(begin.cwd, PathBuf::from("/tmp"));
        assert_eq!(begin.source, ExecCommandSource::UserShell);

        let (_, delta_events) =
            server_notification_thread_events(ServerNotification::CommandExecutionOutputDelta(
                CommandExecutionOutputDeltaNotification {
                    thread_id: thread_id.clone(),
                    turn_id: turn_id.clone(),
                    item_id: "cmd-1".to_string(),
                    delta: b"hello world\n".to_vec(),
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
            cwd: PathBuf::from("/tmp"),
            process_id: None,
            source: CommandExecutionSource::UserShell,
            status: CommandExecutionStatus::Completed,
            command_actions: vec![CommandAction::Unknown {
                command: "printf hello world".to_string(),
            }],
            aggregated_output: Some("hello world\n".to_string()),
            formatted_output: "hello world\n".to_string(),
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
            cwd: PathBuf::from("C:\\repo"),
            process_id: None,
            source: CommandExecutionSource::UserShell,
            status: CommandExecutionStatus::InProgress,
            command_actions: vec![],
            aggregated_output: None,
            formatted_output: String::new(),
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
            preview: String::new(),
            ephemeral: false,
            model_provider: "openai".to_string(),
            created_at: 1,
            updated_at: 1,
            status: ThreadStatus::Idle,
            path: None,
            cwd: PathBuf::from("/tmp"),
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
                    cwd: PathBuf::from("/tmp"),
                    process_id: None,
                    source: CommandExecutionSource::UserShell,
                    status: CommandExecutionStatus::Completed,
                    command_actions: vec![CommandAction::Unknown {
                        command: "printf hello world".to_string(),
                    }],
                    aggregated_output: Some("hello world\n".to_string()),
                    formatted_output: "hello world\n".to_string(),
                    exit_code: Some(0),
                    duration_ms: Some(5),
                }],
                status: TurnStatus::Completed,
                error: None,
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
                preview: "hello".to_string(),
                ephemeral: false,
                model_provider: "openai".to_string(),
                created_at: 0,
                updated_at: 0,
                status: ThreadStatus::Idle,
                path: None,
                cwd: PathBuf::from("/tmp/project"),
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
                            },
                        ],
                        status: TurnStatus::Completed,
                        error: None,
                    },
                    Turn {
                        id: "turn-interrupted".to_string(),
                        items: Vec::new(),
                        status: TurnStatus::Interrupted,
                        error: None,
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
        let EventMsg::TurnAborted(TurnAbortedEvent { turn_id, reason }) = &events[5].msg else {
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
                    },
                    ThreadItem::ContextCompaction {
                        id: "compact-1".to_string(),
                    },
                ],
                status: TurnStatus::Completed,
                error: None,
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
}
