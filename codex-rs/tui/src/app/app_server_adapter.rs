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

    fn legacy_notification_to_thread_event(
        notification: codex_app_server_protocol::JSONRPCNotification,
    ) -> Option<(ThreadId, Event)> {
        let params = notification.params?;
        let thread_id = params.get("conversationId")?.as_str()?;
        let thread_id = ThreadId::from_string(thread_id).ok()?;
        let event: Event = serde_json::from_value(params).ok()?;
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
}
