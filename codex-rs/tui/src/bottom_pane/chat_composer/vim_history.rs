//! Bounded Vim undo/redo history for the complete composer-owned draft.
//!
//! A textarea does not own pending paste payloads, image attachments, or mention targets. Keeping
//! edit transactions here lets one snapshot restore those values together with visible text. Keys
//! and direct composer-owned changes share the same bounded transaction state.
//! Committed snapshots share a byte budget. One pending edit snapshot has its own matching cap,
//! so canceled commands cannot evict committed history.

use std::collections::VecDeque;

use super::super::textarea::VimPersistentState;
use super::ChatComposer;
use super::ComposerDraft;
use crate::key_hint::KeyBindingListExt;
use crossterm::event::KeyEvent;

const MAX_VIM_UNDO_STEPS: usize = 64;
const MAX_VIM_UNDO_BYTES: usize = 1024 * 1024;

#[derive(Debug, Default)]
pub(super) struct VimHistory {
    undo: VecDeque<ComposerDraft>,
    redo: VecDeque<ComposerDraft>,
    pending: Option<ComposerDraft>,
}

impl ComposerDraft {
    fn vim_history_bytes(&self) -> usize {
        let mut bytes = self.text.len()
            + self.text_elements.len()
                * std::mem::size_of::<codex_protocol::user_input::TextElement>();
        for path in &self.local_image_paths {
            bytes += path.as_os_str().len();
        }
        for url in &self.remote_image_urls {
            bytes += url.len();
        }
        for binding in &self.mention_bindings {
            bytes += binding.mention.len() + binding.path.len();
        }
        for (placeholder, pasted) in &self.pending_pastes {
            bytes += placeholder.len() + pasted.len();
        }
        bytes
    }
}

impl VimHistory {
    fn trim(&mut self) {
        while self.undo.len() > MAX_VIM_UNDO_STEPS
            || self
                .undo
                .iter()
                .chain(self.redo.iter())
                .map(ComposerDraft::vim_history_bytes)
                .sum::<usize>()
                > MAX_VIM_UNDO_BYTES
        {
            if self.undo.pop_front().is_none() {
                self.redo.pop_front();
            }
        }
    }
}

impl ChatComposer {
    pub(super) fn wants_vim_history_key(&self, event: KeyEvent) -> bool {
        self.draft.textarea.is_vim_normal_mode()
            && !self.draft.textarea.is_vim_operator_pending()
            && !self.popups.active()
            && (self.vim_normal_keymap.redo.is_pressed(event)
                || self.vim_normal_keymap.undo.is_pressed(event))
    }

    /// Consume undo and redo before normal-mode commands, even when their history is empty.
    pub(super) fn handle_vim_history_key(&mut self, event: KeyEvent) -> bool {
        if !self.wants_vim_history_key(event) {
            return false;
        }

        let redo = self.vim_normal_keymap.redo.is_pressed(event);
        let snapshot = if redo {
            self.vim_history.redo.pop_back()
        } else {
            self.vim_history.undo.pop_back()
        };
        if let Some(snapshot) = snapshot {
            let current = self.snapshot_draft();
            if redo {
                self.vim_history.undo.push_back(current);
            } else {
                self.vim_history.redo.push_back(current);
            }
            let history = std::mem::take(&mut self.vim_history);
            let mut vim_state = VimPersistentState::default();
            self.draft
                .textarea
                .swap_vim_persistent_state(&mut vim_state);
            self.restore_draft(snapshot);
            self.draft
                .textarea
                .swap_vim_persistent_state(&mut vim_state);
            self.vim_history = history;
            self.draft.textarea.enter_vim_normal_mode();
            self.vim_history.trim();
        }
        true
    }

    /// Snapshot only keys that can begin or complete a Vim edit transaction.
    pub(super) fn begin_vim_edit(&mut self, event: KeyEvent) {
        if !self.draft.textarea.is_vim_enabled()
            || self.vim_history.pending.is_some()
            || self.draft.textarea.is_vim_operator_pending()
        {
            return;
        }

        if self.draft.textarea.is_vim_normal_mode()
            && (self.vim_normal_keymap.move_left.is_pressed(event)
                || self.vim_normal_keymap.move_right.is_pressed(event)
                || self.vim_normal_keymap.move_up.is_pressed(event)
                || self.vim_normal_keymap.move_down.is_pressed(event)
                || self.vim_normal_keymap.move_word_forward.is_pressed(event)
                || self.vim_normal_keymap.move_word_backward.is_pressed(event)
                || self.vim_normal_keymap.move_word_end.is_pressed(event)
                || self.vim_normal_keymap.move_line_start.is_pressed(event)
                || self.vim_normal_keymap.move_line_end.is_pressed(event)
                || self.vim_normal_keymap.find_forward.is_pressed(event)
                || self.vim_normal_keymap.find_backward.is_pressed(event)
                || self.vim_normal_keymap.jump_top.is_pressed(event)
                || self.vim_normal_keymap.jump_bottom.is_pressed(event)
                || self.draft.textarea.wants_vim_search_key(event)
                || self.vim_normal_keymap.yank_line.is_pressed(event)
                || self.vim_normal_keymap.start_yank_operator.is_pressed(event)
                || self.vim_normal_keymap.cancel_operator.is_pressed(event))
        {
            return;
        }

        self.begin_vim_edit_transaction();
    }

    /// Start one standalone draft edit without splitting an active Vim command.
    pub(super) fn begin_direct_vim_edit(&mut self) -> bool {
        if !self.draft.textarea.is_vim_enabled()
            || self.draft.textarea.is_vim_operator_pending()
            || self.vim_history.pending.is_some()
            || self.history_search.is_some()
        {
            return false;
        }

        self.begin_vim_edit_transaction();
        if self.vim_history.pending.is_none() {
            return false;
        }
        let mut vim_state = VimPersistentState::default();
        self.draft
            .textarea
            .swap_vim_persistent_state(&mut vim_state);
        vim_state.commands.last_change.clear();
        self.draft
            .textarea
            .swap_vim_persistent_state(&mut vim_state);
        true
    }

    fn begin_vim_edit_transaction(&mut self) {
        let visible_bytes = self.draft.textarea.text().len();
        let paste_bytes = self
            .draft
            .pending_pastes
            .iter()
            .map(|(placeholder, pasted)| placeholder.len() + pasted.len())
            .sum::<usize>();
        if visible_bytes.saturating_add(paste_bytes) > MAX_VIM_UNDO_BYTES {
            self.vim_history = VimHistory::default();
            return;
        }

        let snapshot = self.snapshot_draft();
        if snapshot.vim_history_bytes() <= MAX_VIM_UNDO_BYTES {
            self.vim_history.pending = Some(snapshot);
            self.vim_history.trim();
        } else {
            self.vim_history = VimHistory::default();
        }
    }

    /// Commit a complete normal-mode command or one insert-mode session.
    pub(super) fn finish_vim_edit(&mut self) {
        if !self.draft.textarea.is_vim_normal_mode()
            || self.draft.textarea.is_vim_operator_pending()
        {
            return;
        }

        let Some(snapshot) = self.vim_history.pending.take() else {
            return;
        };
        if snapshot.text == self.current_text()
            && snapshot.text_elements == self.current_text_elements()
            && snapshot.pending_pastes == self.draft.pending_pastes
            && snapshot.local_image_paths == self.attachments.local_image_paths()
            && snapshot.remote_image_urls == self.attachments.remote_image_urls()
            && snapshot.mention_bindings == self.snapshot_mention_bindings()
        {
            return;
        }

        self.vim_history.redo.clear();
        self.vim_history.undo.push_back(snapshot);
        self.vim_history.trim();
    }
}

#[cfg(test)]
#[path = "vim_history_tests.rs"]
mod tests;
