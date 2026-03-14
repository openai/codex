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
use crate::app_server_session::AppServerSession;
use crate::app_server_session::app_server_rate_limit_snapshot_to_core;
use crate::app_server_session::status_account_display_from_auth_mode;
use codex_app_server_client::InProcessServerEvent;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::ServerNotification;
use codex_protocol::ThreadId;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::RealtimeConversationClosedEvent;
use codex_protocol::protocol::RealtimeConversationRealtimeEvent;
use codex_protocol::protocol::RealtimeConversationStartedEvent;
use codex_protocol::protocol::RealtimeEvent;
use serde_json::Value;

impl App {
    pub(super) async fn handle_app_server_event(
        &mut self,
        app_server_client: &AppServerSession,
        event: InProcessServerEvent,
    ) {
        match event {
            InProcessServerEvent::Lagged { skipped } => {
                tracing::warn!(
                    skipped,
                    "app-server event consumer lagged; dropping ignored events"
                );
            }
            InProcessServerEvent::ServerNotification(notification) => match notification {
                ServerNotification::ServerRequestResolved(notification) => {
                    self.pending_app_server_requests
                        .resolve_notification(&notification.request_id);
                }
                ServerNotification::CommandExecOutputDelta(notification) => {
                    if let Some((thread_id, event)) =
                        self.note_command_exec_output_delta(&notification)
                        && let Err(err) = self.enqueue_thread_event(thread_id, event).await
                    {
                        tracing::warn!(
                            "failed to enqueue app-server command exec output for {thread_id}: {err}"
                        );
                    }
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
                        ),
                    );
                }
                notification => {
                    if let Some((thread_id, event)) = server_notification_thread_event(notification)
                    {
                        if self.primary_thread_id.is_none()
                            || matches!(event.msg, EventMsg::SessionConfigured(_))
                                && self.primary_thread_id == Some(thread_id)
                        {
                            if let Err(err) = self.enqueue_primary_event(event).await {
                                tracing::warn!(
                                    "failed to enqueue primary app-server server notification: {err}"
                                );
                            }
                        } else if let Err(err) = self.enqueue_thread_event(thread_id, event).await {
                            tracing::warn!(
                                "failed to enqueue app-server server notification for {thread_id}: {err}"
                            );
                        }
                    }
                }
            },
            InProcessServerEvent::LegacyNotification(notification) => {
                if let Some((thread_id, event)) = legacy_thread_event(notification.params) {
                    self.pending_app_server_requests.note_legacy_event(&event);
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
            InProcessServerEvent::ServerRequest(request) => {
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

fn server_notification_thread_event(notification: ServerNotification) -> Option<(ThreadId, Event)> {
    match notification {
        ServerNotification::ThreadRealtimeStarted(notification) => Some((
            ThreadId::from_string(&notification.thread_id).ok()?,
            Event {
                id: String::new(),
                msg: EventMsg::RealtimeConversationStarted(RealtimeConversationStartedEvent {
                    session_id: notification.session_id,
                }),
            },
        )),
        ServerNotification::ThreadRealtimeItemAdded(notification) => Some((
            ThreadId::from_string(&notification.thread_id).ok()?,
            Event {
                id: String::new(),
                msg: EventMsg::RealtimeConversationRealtime(RealtimeConversationRealtimeEvent {
                    payload: RealtimeEvent::ConversationItemAdded(notification.item),
                }),
            },
        )),
        ServerNotification::ThreadRealtimeOutputAudioDelta(notification) => Some((
            ThreadId::from_string(&notification.thread_id).ok()?,
            Event {
                id: String::new(),
                msg: EventMsg::RealtimeConversationRealtime(RealtimeConversationRealtimeEvent {
                    payload: RealtimeEvent::AudioOut(notification.audio.into()),
                }),
            },
        )),
        ServerNotification::ThreadRealtimeError(notification) => Some((
            ThreadId::from_string(&notification.thread_id).ok()?,
            Event {
                id: String::new(),
                msg: EventMsg::RealtimeConversationRealtime(RealtimeConversationRealtimeEvent {
                    payload: RealtimeEvent::Error(notification.message),
                }),
            },
        )),
        ServerNotification::ThreadRealtimeClosed(notification) => Some((
            ThreadId::from_string(&notification.thread_id).ok()?,
            Event {
                id: String::new(),
                msg: EventMsg::RealtimeConversationClosed(RealtimeConversationClosedEvent {
                    reason: notification.reason,
                }),
            },
        )),
        _ => None,
    }
}
