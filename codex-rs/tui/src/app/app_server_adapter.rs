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
use codex_app_server_client::InProcessAppServerClient;
use codex_app_server_client::InProcessServerEvent;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::ServerRequest;

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
                self.handle_ignored_app_server_notification(notification);
            }
            InProcessServerEvent::LegacyNotification(notification) => {
                self.handle_ignored_app_server_legacy_notification(notification);
            }
            InProcessServerEvent::ServerRequest(request) => {
                self.handle_app_server_request(app_server_client, request)
                    .await;
            }
        }
    }

    async fn handle_app_server_request(
        &mut self,
        app_server_client: &InProcessAppServerClient,
        request: ServerRequest,
    ) {
        let method = Self::server_request_method_name(&request);
        let request_id = request.id().clone();
        tracing::warn!(
            ?request_id,
            method,
            "rejecting app-server request while TUI still uses direct core APIs"
        );
        if let Err(err) = self
            .reject_app_server_request(
                app_server_client,
                request_id,
                &method,
                "TUI client does not yet handle this app-server server request".to_string(),
            )
            .await
        {
            tracing::warn!("{err}");
        }
    }

    async fn reject_app_server_request(
        &self,
        app_server_client: &InProcessAppServerClient,
        request_id: codex_app_server_protocol::RequestId,
        method: &str,
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
            .map_err(|err| format!("failed to reject `{method}` server request: {err}"))
    }

    fn handle_ignored_app_server_notification(
        &mut self,
        _notification: codex_app_server_protocol::ServerNotification,
    ) {
    }

    fn handle_ignored_app_server_legacy_notification(
        &mut self,
        _notification: codex_app_server_protocol::JSONRPCNotification,
    ) {
    }

    fn server_request_method_name(request: &ServerRequest) -> String {
        serde_json::to_value(request)
            .ok()
            .and_then(|value| {
                value
                    .get("method")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned)
            })
            .unwrap_or_else(|| "unknown".to_string())
    }
}
