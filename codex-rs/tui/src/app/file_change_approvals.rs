//! Recover file-change details for live and replayed approval requests.
//!
//! The approval RPC identifies an item but does not repeat its diff. Recover the
//! matching item before constructing the approval so its full-screen pager owns
//! the same changes that were already displayed in the transcript.

use super::*;

impl App {
    pub(super) async fn thread_file_change_changes(
        &self,
        thread_id: ThreadId,
        turn_id: &str,
        item_id: &str,
    ) -> Option<Vec<codex_app_server_protocol::FileUpdateChange>> {
        let channel = self.thread_event_channels.get(&thread_id)?;
        let store = channel.store.lock().await;
        store.file_change_changes(turn_id, item_id)
    }

    pub(super) async fn handle_thread_event_now_recovering_file_changes(
        &mut self,
        event: ThreadBufferedEvent,
    ) {
        let changes = match (&event, self.active_thread_id) {
            (ThreadBufferedEvent::Request(request), Some(thread_id)) => match request.as_ref() {
                ServerRequest::FileChangeRequestApproval { params, .. } => {
                    self.thread_file_change_changes(thread_id, &params.turn_id, &params.item_id)
                        .await
                }
                _ => None,
            },
            _ => None,
        };
        match (changes, event) {
            (Some(changes), ThreadBufferedEvent::Request(request)) => {
                if self
                    .pending_app_server_requests
                    .contains_server_request(request.as_ref())
                {
                    self.handle_file_change_request(*request, changes);
                }
            }
            (_, event) => self.handle_thread_event_now(event),
        }
    }

    pub(super) fn handle_file_change_request(
        &mut self,
        request: ServerRequest,
        changes: Vec<codex_app_server_protocol::FileUpdateChange>,
    ) {
        let ServerRequest::FileChangeRequestApproval { params, .. } = request else {
            return;
        };
        self.chat_widget.on_apply_patch_approval_request(
            params.item_id.clone(),
            crate::approval_events::ApplyPatchApprovalRequestEvent {
                call_id: params.item_id,
                turn_id: params.turn_id,
                changes: crate::app_server_approval_conversions::file_update_changes_to_display(
                    changes,
                ),
                reason: params.reason,
                grant_root: params.grant_root,
            },
        );
        if self.startup_protected_input_boundary && !self.chat_widget.has_active_view() {
            self.startup_pending_protected_request = true;
        }
    }
}
