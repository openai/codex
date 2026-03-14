/*
Adapter layer between the TUI and the in-process app server.

Fresh sessions are now started via the app-server `thread/start` RPC, and their
runtime events arrive as `LegacyNotification`s on the app-server event stream.
This module converts those notifications into `AppEvent::ThreadEvent` so they
enter the same pipeline as events from direct-core threads.

Resume/fork sessions still use direct `CodexThread` access with a per-thread
listener task; those events bypass this module entirely.

As more TUI flows migrate to the app-server surface, the legacy-notification
bridge should shrink and eventually be replaced by typed server notifications.
*/

use crate::app_event::AppEvent;

use super::App;
use codex_app_server_client::InProcessAppServerClient;
use codex_app_server_client::InProcessServerEvent;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::ServerNotification;
use codex_protocol::ThreadId;
use codex_protocol::protocol::Event;

impl App {
    pub(super) async fn handle_app_server_event(
        &mut self,
        app_server_client: &InProcessAppServerClient,
        event: InProcessServerEvent,
    ) {
        match event {
            InProcessServerEvent::Lagged { skipped } => {
                tracing::warn!(
                    skipped,
                    "app-server event consumer lagged; dropping ignored events"
                );
            }
            InProcessServerEvent::ServerNotification(notification) => {
                if let ServerNotification::SkillsChanged(_) = notification {
                    self.app_event_tx.send(AppEvent::RefreshSkillsList);
                }
            }
            InProcessServerEvent::LegacyNotification(notification) => {
                if let Some((thread_id, event)) =
                    Self::legacy_notification_to_thread_event(notification)
                {
                    self.app_event_tx
                        .send(AppEvent::ThreadEvent { thread_id, event });
                }
            }
            InProcessServerEvent::ServerRequest(request) => {
                let request_id = request.id().clone();
                tracing::warn!(
                    ?request_id,
                    "rejecting app-server request while TUI still uses direct core APIs"
                );
                if let Err(err) = self
                    .reject_app_server_request(
                        app_server_client,
                        request_id,
                        "TUI client does not yet handle this app-server server request".to_string(),
                    )
                    .await
                {
                    tracing::warn!("{err}");
                }
            }
        }
    }

    /// Extract a `(ThreadId, Event)` pair from a legacy JSON-RPC notification.
    ///
    /// Legacy notifications embed the thread id as a `"conversationId"` string
    /// field alongside the serialized `Event` body. Returns `None` if the
    /// notification is missing the field, contains an unparsable thread id,
    /// or fails to deserialize into an `Event`.
    fn legacy_notification_to_thread_event(
        notification: codex_app_server_protocol::JSONRPCNotification,
    ) -> Option<(ThreadId, Event)> {
        let params = notification.params?;
        let thread_id = params.get("conversationId")?.as_str()?;
        let thread_id = match ThreadId::from_string(thread_id) {
            Ok(thread_id) => thread_id,
            Err(err) => {
                tracing::warn!(
                    method = notification.method,
                    conversation_id = thread_id,
                    %err,
                    "failed to parse legacy notification thread id"
                );
                return None;
            }
        };
        let event: Event = match serde_json::from_value(params) {
            Ok(event) => event,
            Err(err) => {
                tracing::warn!(
                    method = notification.method,
                    thread_id = %thread_id,
                    %err,
                    "failed to deserialize legacy notification into thread event"
                );
                return None;
            }
        };
        Some((thread_id, event))
    }

    async fn reject_app_server_request(
        &self,
        app_server_client: &InProcessAppServerClient,
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

#[cfg(test)]
mod tests {
    use super::*;
    use assert_matches::assert_matches;
    use codex_app_server_protocol::JSONRPCNotification;
    use codex_protocol::protocol::EventMsg;
    use serde_json::json;

    #[test]
    fn legacy_notification_to_thread_event_parses_valid_payload() {
        let thread_id = ThreadId::new();
        let event = Event {
            id: "ev-1".to_string(),
            msg: EventMsg::ShutdownComplete,
        };

        let mut params = serde_json::to_value(&event).expect("event should serialize");
        params
            .as_object_mut()
            .expect("event should serialize to object")
            .insert(
                "conversationId".to_string(),
                serde_json::Value::String(thread_id.to_string()),
            );

        let parsed = App::legacy_notification_to_thread_event(JSONRPCNotification {
            method: "codex/event/shutdownComplete".to_string(),
            params: Some(params),
        })
        .expect("notification should parse");

        let (parsed_thread_id, parsed_event) = parsed;
        assert_eq!(parsed_thread_id, thread_id);
        assert_eq!(parsed_event.id, event.id);
        assert_matches!(parsed_event.msg, EventMsg::ShutdownComplete);
    }

    #[test]
    fn legacy_notification_to_thread_event_returns_none_without_conversation_id() {
        let parsed = App::legacy_notification_to_thread_event(JSONRPCNotification {
            method: "codex/event/shutdownComplete".to_string(),
            params: Some(json!({
                "id": "ev-1",
                "msg": "shutdown_complete",
            })),
        });

        assert!(parsed.is_none());
    }

    #[test]
    fn legacy_notification_to_thread_event_returns_none_for_invalid_event_payload() {
        let thread_id = ThreadId::new();

        let parsed = App::legacy_notification_to_thread_event(JSONRPCNotification {
            method: "codex/event/shutdownComplete".to_string(),
            params: Some(json!({
                "conversationId": thread_id.to_string(),
            })),
        });

        assert!(parsed.is_none());
    }

    #[test]
    fn legacy_notification_to_thread_event_returns_none_for_invalid_thread_id() {
        let parsed = App::legacy_notification_to_thread_event(JSONRPCNotification {
            method: "codex/event/shutdownComplete".to_string(),
            params: Some(json!({
                "conversationId": "not-a-thread-id",
                "id": "ev-1",
                "msg": "shutdown_complete",
            })),
        });

        assert!(parsed.is_none());
    }
}
