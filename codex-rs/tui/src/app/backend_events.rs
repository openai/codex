//! Checks incoming event ownership before consulting native-thread caches or UI state.
//! Connection-wide lifecycle and account events may only update the displayed conversation's UI.

use super::App;
use super::app_server_event_targets::ServerNotificationThreadTarget;
use super::app_server_event_targets::server_notification_thread_target;
use crate::app_server_session::AppServerSession;
use crate::app_server_session::AppServerSessionEvent;
use crate::backend_id::BackendId;
use crate::chatwidget::ThreadInputState;
use codex_app_server_client::AppServerEvent;
use codex_app_server_protocol::ServerNotification;
use codex_protocol::ThreadId;
use std::collections::HashMap;

impl App {
    pub(super) async fn handle_polled_backend_event(
        &mut self,
        app_server: &AppServerSession,
        displayed_owner: BackendId,
        parked_inputs: &mut HashMap<ThreadId, ThreadInputState>,
        event: AppServerSessionEvent,
    ) {
        if displayed_owner != event.backend_id {
            if event.backend_id != app_server.backend_id() {
                return;
            }
            let invalidates_discovery = match &event.event {
                AppServerEvent::Lagged { .. } | AppServerEvent::Disconnected { .. } => true,
                AppServerEvent::ServerNotification(notification) => matches!(
                    notification.as_ref(),
                    ServerNotification::AccountUpdated(_)
                        | ServerNotification::ThreadArchived(_)
                        | ServerNotification::ThreadDeleted(_)
                        | ServerNotification::ThreadUnarchived(_)
                ),
                _ => false,
            };
            if invalidates_discovery {
                self.agents_overview
                    .invalidate_discovery(event.backend_id, /*displayed*/ false);
            }
            if let AppServerEvent::ServerNotification(notification) = &event.event
                && let ServerNotificationThreadTarget::Thread(thread_id) =
                    server_notification_thread_target(notification)
            {
                if matches!(
                    notification.as_ref(),
                    ServerNotification::ThreadArchived(_)
                        | ServerNotification::ThreadDeleted(_)
                        | ServerNotification::ThreadUnarchived(_)
                ) && self.backend_registry.owner(thread_id) == Some(event.backend_id)
                {
                    // A parked removal must not consume the displayed backend's page budget.
                    let refill_count = self.agents_overview.refill_count;
                    self.track_agents_overview_notification(notification);
                    self.agents_overview.refill_count = refill_count;
                    self.repaint_agents_overview();
                }
                if let ServerNotification::ThreadSettingsUpdated(settings) = notification.as_ref()
                    && self.backend_registry.owner(thread_id) == Some(event.backend_id)
                    && self
                        .update_retained_blank_settings(thread_id, settings)
                        .await
                    && let Some(input) = parked_inputs.get_mut(&thread_id)
                {
                    input.pending_thread_settings = Some(settings.clone());
                }
                if self.voice_owner_thread_id() == Some(thread_id) {
                    self.deliver_background_voice_notification(thread_id, notification);
                }
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
