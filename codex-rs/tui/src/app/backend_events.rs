//! Checks incoming event ownership before consulting native-thread caches or UI state.

use super::App;
use crate::app_server_session::AppServerSession;
use crate::app_server_session::AppServerSessionEvent;

impl App {
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
