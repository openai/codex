//! Lifecycle coordination shared by the Parent and Side conversations.
//!
//! This module owns multi-pane shutdown and the special transition that keeps a Side conversation
//! usable after its Parent closes.

use super::*;

impl App {
    async fn shutdown_thread(&mut self, app_server: &mut AppServerSession, thread_id: ThreadId) {
        if let Err(err) = app_server.thread_unsubscribe(thread_id).await {
            tracing::warn!("failed to unsubscribe thread {thread_id}: {err}");
        }
        self.abort_thread_event_listener(thread_id);
    }

    pub(super) async fn shutdown_current_thread(&mut self, app_server: &mut AppServerSession) {
        if let Some(thread_id) = self.chat_widget.thread_id() {
            self.backtrack.pending_rollback = None;
            self.shutdown_thread(app_server, thread_id).await;
        }
    }

    pub(super) async fn shutdown_installed_threads(
        &mut self,
        app_server: &mut AppServerSession,
        thread_ids: Vec<ThreadId>,
        per_thread_timeout: Duration,
    ) {
        self.backtrack.pending_rollback = None;
        for thread_id in &thread_ids {
            self.abort_thread_event_listener(*thread_id);
        }
        for thread_id in thread_ids {
            match tokio::time::timeout(per_thread_timeout, app_server.thread_unsubscribe(thread_id))
                .await
            {
                Ok(Ok(_)) => {}
                Ok(Err(err)) => {
                    tracing::warn!("failed to unsubscribe thread {thread_id}: {err}");
                }
                Err(_) => {
                    tracing::warn!("timed out waiting to unsubscribe thread {thread_id}");
                }
            }
        }
    }

    pub(super) async fn handle_closed_parent_pane(
        &mut self,
        tui: &mut tui::Tui,
        parent_thread_id: ThreadId,
    ) {
        if self.pending_shutdown_exit_thread_id == Some(parent_thread_id) {
            self.pending_shutdown_exit_thread_id = None;
        }
        self.mark_agent_picker_thread_closed(parent_thread_id);
        self.retire_thread(parent_thread_id);
        self.pending_app_server_requests
            .discard_thread(parent_thread_id);
        if let Some(parent) = self.chat_widget.by_slot_mut(PaneSlot::Parent) {
            parent.mark_thread_closed();
        }
        self.clear_pane_thread(PaneSlot::Parent).await;
        debug_assert_eq!(
            self.chat_widget
                .by_slot(PaneSlot::Parent)
                .and_then(|pane| pane.active_thread_id),
            None
        );
        if let Some(side) = self.chat_widget.by_slot_mut(PaneSlot::Side) {
            side.add_info_message(
                "Parent conversation closed. This side conversation remains available.".to_string(),
                /*hint*/ None,
            );
        }
        self.focus_installed_conversation_pane(PaneSlot::Side);
        tui.frame_requester().schedule_frame();
    }
}
