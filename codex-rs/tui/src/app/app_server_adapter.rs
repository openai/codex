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
            InProcessServerEvent::ServerNotification(_) => {}
            InProcessServerEvent::LegacyNotification(_) => {}
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
