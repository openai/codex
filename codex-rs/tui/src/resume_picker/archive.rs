use codex_protocol::ThreadId;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;

use super::LoadTrigger;
use super::PickerLoadRequest;
use super::PickerState;
use super::Row;
use super::SearchState;
use super::SessionPickerAction;
use super::SessionPickerLaunchContext;
use super::SessionSelection;
use super::SessionStatus;
use super::SessionTarget;
use crate::backend_id::BackendId;
use crate::keymap::KeymapContext;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) enum ArchiveState {
    #[default]
    Idle,
    Pending {
        backend_id: BackendId,
        thread_id: ThreadId,
    },
    Restoring {
        thread_id: ThreadId,
    },
}

impl PickerState {
    pub(super) fn archive_shortcut_available(&self) -> bool {
        if !matches!(self.action, SessionPickerAction::Resume)
            || self.status == SessionStatus::Archived
        {
            return false;
        }

        let archive_key = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL);
        self.keymap.list.action_for(archive_key).is_none()
            && !self.keymap.chords.bindings.iter().any(|binding| {
                binding.action.context == KeymapContext::List
                    && binding.chord.prefix.is_press(archive_key)
            })
    }

    pub(super) fn request_archive_for_selected_session(&mut self) {
        if !matches!(self.archive_state, ArchiveState::Idle) {
            return;
        }

        let Some(row) = self.filtered_rows.get(self.selected) else {
            return;
        };
        let Some(thread_id) = row.thread_id else {
            self.inline_error = Some(String::from("Selected session does not have a thread ID."));
            self.request_frame();
            return;
        };

        let backend_id = row.backend_id.unwrap_or(self.backend_id);
        let key = (backend_id, thread_id);
        if matches!(
            self.launch_context,
            SessionPickerLaunchContext::ExistingSession {
                current_thread_id: Some(current_thread_id)
            } if current_thread_id == key
        ) {
            self.inline_error = Some(String::from(
                "Use /archive to archive the current session and exit.",
            ));
            self.request_frame();
            return;
        }

        self.archive_state = ArchiveState::Pending {
            backend_id,
            thread_id,
        };
        self.request_frame();
        (self.picker_loader)(PickerLoadRequest::Archive {
            backend_id,
            thread_id,
        });
    }

    pub(super) fn handle_archive_result(
        &mut self,
        backend_id: BackendId,
        thread_id: ThreadId,
        result: std::io::Result<()>,
    ) {
        if self.archive_state
            != (ArchiveState::Pending {
                backend_id,
                thread_id,
            })
        {
            return;
        }
        let key = (backend_id, thread_id);
        let default_backend = self.backend_id;
        let is_archived = |row: &Row| {
            (row.backend_id.unwrap_or(default_backend), row.thread_id)
                == (backend_id, Some(thread_id))
        };
        self.chord_matcher.cancel();
        self.archive_state = ArchiveState::Idle;

        if let Err(error) = result {
            self.inline_error = Some(format!("Failed to archive session: {error}"));
            self.request_frame();
            return;
        }

        let selected_key = self
            .filtered_rows
            .get(self.selected)
            .filter(|row| !is_archived(row))
            .map(|row| (row.backend_id.unwrap_or(default_backend), row.seen_key()));

        self.all_rows.retain(|row| !is_archived(row));
        // Keep the seen-row key as a tombstone so an older page that is already
        // in flight cannot put the archived session back into the picker.
        self.transcript_previews.remove(&key);
        if self.expanded_thread_id == Some(key) {
            self.expanded_thread_id = None;
        }
        self.transcript_cells.remove(&key);
        if self.pending_transcript_open == Some(key) {
            self.pending_transcript_open = None;
            if let Some(cancellation) = self.pending_transcript_cancellation.take() {
                let _ = cancellation.send(());
            }
            self.transcript_loading_frame_shown = false;
        }
        self.inline_error = None;
        self.apply_filter();

        if let Some(selected_key) = selected_key
            && let Some(selected) = self.filtered_rows.iter().position(|row| {
                (row.backend_id.unwrap_or(default_backend), row.seen_key()) == selected_key
            })
        {
            self.selected = selected;
            self.ensure_selected_visible();
        }
        if self.filtered_rows.is_empty()
            && !self.query.is_empty()
            && self.pagination.next_cursor.is_some()
            && !self.pagination.reached_scan_cap
            && !self.search_state.is_active()
        {
            let token = self.allocate_search_token();
            self.search_state = SearchState::Active { token };
            self.load_more_if_needed(LoadTrigger::Search { token });
        }
    }

    pub(super) fn request_unarchive(&mut self, thread_id: ThreadId, backend_id: BackendId) {
        if !matches!(self.archive_state, ArchiveState::Idle) {
            return;
        }

        self.archive_state = ArchiveState::Restoring { thread_id };
        self.request_frame();
        (self.picker_loader)(PickerLoadRequest::Unarchive {
            backend_id,
            thread_id,
        });
    }

    pub(super) fn handle_unarchive_result(
        &mut self,
        thread_id: ThreadId,
        result: std::io::Result<SessionTarget>,
    ) -> Option<SessionSelection> {
        if self.archive_state != (ArchiveState::Restoring { thread_id }) {
            return None;
        }
        self.chord_matcher.cancel();
        self.archive_state = ArchiveState::Idle;

        match result {
            Ok(target) => Some(SessionSelection::Resume(target)),
            Err(error) => {
                self.inline_error = Some(format!("Failed to restore archived session: {error}"));
                self.request_frame();
                None
            }
        }
    }
}

#[cfg(test)]
#[path = "archive_tests.rs"]
mod tests;
