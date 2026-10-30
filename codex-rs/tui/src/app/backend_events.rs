//! Checks incoming event ownership before consulting native-thread caches or UI state.
//! Connection-wide lifecycle and account events may only update the displayed conversation's UI.

use super::App;
use super::app_server_event_targets::ServerNotificationThreadTarget;
use super::app_server_event_targets::server_notification_thread_target;
use crate::app_server_session::AppServerSession;
use crate::app_server_session::AppServerSessionEvent;
use crate::backend_id::BackendId;
use codex_app_server_client::AppServerEvent;

impl App {
    pub(super) async fn handle_polled_backend_event(
        &mut self,
        app_server: &AppServerSession,
        displayed_owner: BackendId,
        event: AppServerSessionEvent,
    ) {
        if displayed_owner != event.backend_id {
            // A parked voice session still needs its own notifications.
            if event.backend_id == app_server.backend_id()
                && let AppServerEvent::ServerNotification(notification) = &event.event
                && let ServerNotificationThreadTarget::Thread(thread_id) =
                    server_notification_thread_target(notification)
                && self.voice_owner_thread_id() == Some(thread_id)
            {
                self.deliver_background_voice_notification(thread_id, notification);
            }
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
