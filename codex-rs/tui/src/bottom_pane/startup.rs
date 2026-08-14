//! Bottom-pane adapters for the provisional startup composer and its protected handoff.
//!
//! Startup accepts safe editor input while keeping submission, search, and other actions
//! unavailable. Once initialization finishes, these adapters preserve buffered paste text,
//! composer history and cursor state, and the approval-idle timing inherited by the real composer.

use std::time::Instant;

use crossterm::event::KeyEvent;

use super::BottomPane;
use super::ComposerDraftSnapshot;
use super::HistoryEntry;
use crate::key_hint::KeyBindingListExt;

impl BottomPane {
    /// Return whether a key would submit, queue, or search outside the startup draft.
    pub(crate) fn is_startup_composer_action(&self, key_event: KeyEvent) -> bool {
        let composer = &self.keymap.composer;
        [
            &composer.submit,
            &composer.queue,
            &composer.history_search_previous,
            &composer.history_search_next,
        ]
        .into_iter()
        .any(|bindings| bindings.is_pressed(key_event))
            || self.composer_is_empty() && composer.toggle_shortcuts.is_pressed(key_event)
    }

    /// Return whether a key is bound only to local startup-safe text editing.
    pub(crate) fn is_safe_startup_editor_key(&self, key_event: KeyEvent) -> bool {
        let editor = &self.keymap.editor;
        [
            &editor.insert_newline,
            &editor.move_left,
            &editor.move_right,
            &editor.move_up,
            &editor.move_down,
            &editor.move_word_left,
            &editor.move_word_right,
            &editor.move_line_start,
            &editor.move_line_end,
            &editor.delete_backward,
            &editor.delete_forward,
            &editor.delete_backward_word,
            &editor.delete_forward_word,
            &editor.kill_line_start,
            &editor.kill_whole_line,
            &editor.kill_line_end,
            &editor.yank,
        ]
        .into_iter()
        .any(|bindings| bindings.is_pressed(key_event))
    }

    /// Enable Vim without interrupting an in-progress startup draft.
    pub(crate) fn enable_vim_in_insert_mode(&mut self) {
        self.composer.enable_vim_in_insert_mode();
        self.request_redraw();
    }

    /// Apply paste-burst preferences without dropping input buffered before config loading.
    pub(crate) fn set_disable_paste_burst(&mut self, disabled: bool) {
        self.disable_paste_burst = disabled;
        self.composer.set_disable_paste_burst(disabled);
    }

    /// Capture draft content together with the activity timestamp used by delayed approvals.
    pub(crate) fn composer_draft_snapshot(&self) -> ComposerDraftSnapshot {
        let mut draft = self.composer.draft_snapshot();
        draft.last_composer_activity_at = self.last_composer_activity_at;
        draft
    }

    /// Restore the visible cursor before synchronizing composer-owned popups.
    pub(crate) fn set_composer_cursor(&mut self, cursor: usize) {
        self.composer.set_current_cursor(cursor);
        self.composer.sync_popups();
    }

    /// Preserve startup-local history and the newest protected-prompt activity timestamp.
    pub(crate) fn restore_startup_composer_state(
        &mut self,
        last_composer_activity_at: Option<Instant>,
        startup_local_history: Vec<HistoryEntry>,
    ) {
        self.composer
            .restore_startup_local_history(startup_local_history);
        if let Some(last_composer_activity_at) = last_composer_activity_at {
            let last_composer_activity_at = self
                .last_composer_activity_at
                .map_or(last_composer_activity_at, |current| {
                    current.max(last_composer_activity_at)
                });
            self.record_composer_activity_at(last_composer_activity_at);
        }
    }

    /// Return whether an approval is waiting to become visible after typing stops.
    pub(crate) fn has_pending_approval(&self) -> bool {
        !self.delayed_approval_requests.is_empty()
    }

    /// Preserve buffered composer input before its owner or input mode changes.
    pub(crate) fn flush_composer_paste_burst(&mut self) {
        if self.composer.is_in_paste_burst() {
            self.composer.set_disable_paste_burst(/*disabled*/ true);
            self.composer
                .set_disable_paste_burst(self.disable_paste_burst);
        }
    }
}
