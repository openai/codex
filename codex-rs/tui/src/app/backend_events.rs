//! Checks incoming event ownership before consulting native-thread caches or UI state.
//! Connection-wide lifecycle and account events may only update the displayed conversation's UI.

use super::App;
use super::agents_overview::AGENTS_OVERVIEW_VIEW_ID;
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
            if let AppServerEvent::CloudInventoryChanged(lifecycle) = &event.event {
                self.invalidate_cloud_inventory(app_server, lifecycle.as_deref());
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
                self.cancel_agents_overview_refresh(event.backend_id);
                self.reset_agents_overview_discovery(event.backend_id);
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
                    self.track_agents_overview_notification(notification);
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

    pub(super) fn invalidate_cloud_inventory(
        &mut self,
        app_server: &AppServerSession,
        lifecycle: Option<&ServerNotification>,
    ) {
        if app_server.thread_params_mode() != crate::app_server_session::ThreadParamsMode::Cloud {
            return;
        }
        let owner = app_server.backend_id();
        let status_changed = matches!(lifecycle, Some(ServerNotification::ThreadStatusChanged(_)));
        if let Some(
            notification @ (ServerNotification::ThreadArchived(_)
            | ServerNotification::ThreadUnarchived(_)
            | ServerNotification::ThreadStatusChanged(_)),
        ) = lifecycle
            && let ServerNotificationThreadTarget::Thread(id) =
                server_notification_thread_target(notification)
            && self.backend_registry.observe(owner, id)
        {
            self.track_agents_overview_notification(notification);
            self.repaint_agents_overview();
            if status_changed && matches!(self.agents_overview.threads.get(&id), Some(Some(_))) {
                return;
            }
        }
        if self.agents_overview.cloud_refresh_pending(owner) {
            self.agents_overview.cloud_inventory_dirty.insert(owner);
            return;
        }
        self.agents_overview.cloud_inventory_dirty.remove(&owner);
        self.agents_overview.refresh_recent_backends.insert(owner);
        if self.cloud_inventory_visible(owner) {
            self.start_agents_overview_refresh_for_backend(owner, app_server.request_handle());
        }
    }

    pub(super) fn cloud_inventory_visible(&self, owner: BackendId) -> bool {
        self.app_server_backend_id == Some(owner)
            && !self.reconnect.offline
            && self
                .chat_widget
                .selected_index_for_present_view(AGENTS_OVERVIEW_VIEW_ID)
                .is_some()
    }

    pub(super) fn poll_cloud_inventory(&mut self, app_server: &AppServerSession, attached: bool) {
        let owner = app_server.backend_id();
        if self.agents_overview.active_refresh.is_some() || !self.cloud_inventory_visible(owner) {
            return;
        }
        let overview = &mut self.agents_overview;
        if !overview.cloud_refresh_pending(owner)
            && !overview.cloud_inventory_dirty.remove(&owner)
            && !overview.refresh_recent_backends.contains(&owner)
            && overview.initialized.contains(&owner)
            && attached
        {
            return;
        }
        overview.refresh_recent_backends.insert(owner);
        self.start_agents_overview_refresh_for_backend(owner, app_server.request_handle());
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
