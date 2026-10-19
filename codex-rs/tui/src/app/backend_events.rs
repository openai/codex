//! Checks incoming event ownership before consulting native-thread caches or UI state.
//! Connection-wide lifecycle and account events may only update the displayed conversation's UI.

use super::App;
use crate::app_server_session::AppServerSession;
use crate::app_server_session::AppServerSessionEvent;
use crate::backend_id::BackendId;
use codex_app_server_client::AppServerEvent;
use codex_app_server_protocol::ServerNotification;

impl App {
    pub(super) async fn handle_polled_backend_event(
        &mut self,
        app_server: &AppServerSession,
        displayed_owner: BackendId,
        event: AppServerSessionEvent,
    ) {
        if event.backend_id != displayed_owner
            && match &event.event {
                AppServerEvent::Disconnected { .. } | AppServerEvent::Lagged { .. } => true,
                AppServerEvent::ServerNotification(notification) => matches!(
                    notification.as_ref(),
                    ServerNotification::AccountUpdated(_)
                        | ServerNotification::AccountRateLimitsUpdated(_)
                ),
                AppServerEvent::ServerRequest(_) => false,
            }
        {
            tracing::debug!(
                "ignoring a background backend event that targets displayed conversation state"
            );
            return;
        }
        self.handle_backend_event(app_server, event).await;
    }

    pub(super) async fn handle_backend_event(
        &mut self,
        app_server: &AppServerSession,
        event: AppServerSessionEvent,
    ) {
        if event.backend_id != app_server.backend_id() {
            tracing::warn!("ignoring server event from a different backend");
            return;
        }
        self.handle_app_server_event(app_server, event.event).await;
    }
}
