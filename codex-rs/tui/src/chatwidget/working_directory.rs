//! Working-directory slash commands for the current local session.

use super::*;
use crate::history_cell::McpInventoryLoadingCell;

impl ChatWidget {
    pub(crate) fn can_change_working_directory(&self, thread_id: ThreadId) -> bool {
        let active = &self.transcript.active_cell;
        self.thread_id == Some(thread_id)
            && !self.active_side_conversation
            && !self.blocks_direct_input
            && !self.config.ephemeral
            && self.unified_exec_processes.is_empty()
            && !self.is_user_turn_pending_or_running()
            && !self.input_queue.has_queued_follow_up_messages()
            && self.input_queue.pending_steers.is_empty()
            && active
                .as_ref()
                .is_none_or(|cell| !cell.as_any().is::<McpInventoryLoadingCell>())
    }

    pub(super) fn request_working_directory_change(&mut self, path: &str) {
        if !self.is_session_configured() {
            self.add_error_message(
                "The session must start before you can change its working directory.".to_string(),
            );
            return;
        }

        let Some(thread_id) = self.thread_id else {
            return;
        };
        if !self.can_change_working_directory(thread_id) {
            let message =
                "Changing directories requires an idle primary session without queued input.";
            self.app_event_tx.send(AppEvent::InsertHistoryCell(Box::new(
                history_cell::new_error_event(message.to_string()),
            )));
            return;
        }

        self.app_event_tx.send(AppEvent::ChangeWorkingDirectory {
            thread_id,
            requested_cwd: PathBuf::from(path),
        });
    }
}
