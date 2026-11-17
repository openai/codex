//! Bounded composer undo/redo history for the complete composer-owned draft.
//!
//! A textarea does not own pending paste payloads, image attachments, or mention targets. Keeping
//! edit transactions here lets one snapshot restore those values together with visible text. Keys
//! and direct composer-owned changes share the same bounded transaction state.
//! Committed snapshots share a byte budget. One pending edit snapshot has its own matching cap,
//! so canceled commands cannot evict committed history.

use std::collections::VecDeque;
use std::time::Duration;
use std::time::Instant;

use super::super::textarea::VimPersistentState;
use super::ChatComposer;
use super::ComposerDraft;
use crate::key_hint::KeyBindingListExt;
use crossterm::event::KeyEvent;

const MAX_UNDO_STEPS: usize = 64;
const MAX_UNDO_BYTES: usize = 8 * 1024 * 1024;

enum EditDirection {
    Undo,
    Redo,
}

#[derive(Debug)]
enum TypingEdit {
    Start,
    ReplaceSelection(usize),
    Continue,
    BackspaceStart,
    BackspaceContinue(usize),
}

#[derive(Debug, Default)]
pub(super) struct EditHistory {
    undo: VecDeque<ComposerDraft>,
    redo: VecDeque<ComposerDraft>,
    pending: Option<ComposerDraft>,
    typing: Option<(Instant, Instant, usize, Option<usize>)>,
    typing_edit: Option<TypingEdit>,
    backspace: Option<(Instant, usize)>,
}

impl ComposerDraft {
    fn history_bytes(&self) -> usize {
        let mut bytes = self.text.len()
            + self.text_elements.len()
                * std::mem::size_of::<codex_protocol::user_input::TextElement>();
        bytes += self.mention_ranges.len() * std::mem::size_of::<std::ops::Range<usize>>();
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

impl EditHistory {
    fn step(
        &mut self,
        direction: EditDirection,
        current: impl FnOnce() -> ComposerDraft,
    ) -> Option<ComposerDraft> {
        self.typing = None;
        self.backspace = None;
        let (from, to) = match direction {
            EditDirection::Undo => (&mut self.undo, &mut self.redo),
            EditDirection::Redo => (&mut self.redo, &mut self.undo),
        };
        let snapshot = from.pop_back()?;
        let current = current();
        if current.history_bytes() <= MAX_UNDO_BYTES {
            to.push_back(current);
        } else {
            to.clear();
        }
        self.trim();
        Some(snapshot)
    }

    fn trim(&mut self) {
        while self.undo.len() > MAX_UNDO_STEPS
            || self
                .undo
                .iter()
                .chain(self.redo.iter())
                .map(ComposerDraft::history_bytes)
                .sum::<usize>()
                > MAX_UNDO_BYTES
        {
            if self.undo.pop_front().is_none() {
                self.redo.pop_front();
            }
        }
    }
}

impl ChatComposer {
    pub(in crate::bottom_pane) fn capture_startup_undo(&mut self) -> Option<ComposerDraft> {
        self.begin_edit_transaction();
        self.edit_history.pending.take()
    }

    /// Make a draft transferred from the provisional startup composer undoable.
    pub(in crate::bottom_pane) fn record_startup_undo(&mut self, destination: ComposerDraft) {
        if self.config.undo_enabled && !self.draft.textarea.is_vim_enabled() {
            self.edit_history.undo.push_back(destination);
            self.edit_history.redo.clear();
            self.edit_history.trim();
        }
    }

    pub(super) fn wants_edit_history_key(&self, event: KeyEvent) -> bool {
        if self.history_search.is_some() || self.draft.textarea.vim_query().is_some() {
            return false;
        }
        (self.config.undo_enabled
            && !self.draft.textarea.is_vim_enabled()
            && (self.redo_keys.is_pressed(event) || self.undo_keys.is_pressed(event)))
            || (self.draft.textarea.is_vim_normal_mode()
                && !self.draft.textarea.is_vim_operator_pending()
                && !self.popups.active()
                && (self.vim_normal_keymap.redo.is_pressed(event)
                    || self.vim_normal_keymap.undo.is_pressed(event)))
    }

    /// Consume undo and redo even when their history is empty.
    pub(super) fn handle_edit_history_key(&mut self, event: KeyEvent) -> bool {
        if !self.wants_edit_history_key(event) {
            return false;
        }
        let normal = self.draft.textarea.is_vim_normal_mode();
        if !normal {
            self.flush_pending_paste_burst();
            self.finish_edit();
            self.popups.active = super::ActivePopup::None;
        }
        let direction = if (normal && self.vim_normal_keymap.redo.is_pressed(event))
            || (!normal && self.redo_keys.is_pressed(event))
        {
            EditDirection::Redo
        } else {
            EditDirection::Undo
        };
        let mut history = std::mem::take(&mut self.edit_history);
        let snapshot = history.step(direction, || self.snapshot_draft());
        if let Some(snapshot) = snapshot {
            let mut vim_state = VimPersistentState::default();
            self.draft
                .textarea
                .swap_vim_persistent_state(&mut vim_state);
            self.restore_draft(snapshot);
            self.draft
                .textarea
                .swap_vim_persistent_state(&mut vim_state);
        }
        self.edit_history = history;
        if normal {
            self.draft.textarea.enter_vim_normal_mode();
        }
        true
    }

    /// Group contiguous ordinary typing and backward-character deletions separately.
    pub(super) fn begin_composer_key_edit(&mut self, event: KeyEvent) {
        if !self.config.undo_enabled
            || self.draft.textarea.is_vim_enabled()
            || self.history_search.is_some()
            || self.wants_edit_history_key(event)
        {
            return;
        }
        let selection = self.draft.textarea.mouse_selection_range();
        let typing = (crate::key_hint::is_plain_text_key_event(event)
            || matches!(event.code, crossterm::event::KeyCode::Char(_))
                && crate::key_hint::is_altgr(event.modifiers))
            && !self.editor_keymap.insert_newline.is_pressed(event)
            && !self.editor_keymap.delete_backward_word.is_pressed(event);
        if !typing
            && !matches!(
                event.code,
                crossterm::event::KeyCode::Enter | crossterm::event::KeyCode::Tab
            )
            && self.draft.paste_burst.is_active()
        {
            self.flush_pending_paste_burst();
            self.finish_edit();
        }
        if self.edit_history.pending.is_some() || self.edit_history.typing_edit.is_some() {
            return;
        }
        let previous_backspace = self.edit_history.backspace.take();
        if self.editor_keymap.delete_backward.is_pressed(event)
            && !self.editor_keymap.delete_backward_word.is_pressed(event)
            && selection.is_none()
            && !self.popups.active()
            && self.attachments.selected_remote_image_index.is_none()
        {
            self.edit_history.typing = None;
            if previous_backspace.is_some_and(|(last, cursor)| {
                last.elapsed() < Duration::from_millis(500) && cursor == self.current_cursor()
            }) {
                self.edit_history.typing_edit =
                    Some(TypingEdit::BackspaceContinue(self.current_cursor()));
            } else {
                self.edit_history.typing_edit = Some(TypingEdit::BackspaceStart);
                self.begin_edit_transaction();
            }
            return;
        }
        if self.editor_keymap.move_left.is_pressed(event)
            || self.editor_keymap.move_right.is_pressed(event)
            || self.editor_keymap.move_up.is_pressed(event)
            || self.editor_keymap.move_down.is_pressed(event)
            || self.editor_keymap.move_word_left.is_pressed(event)
            || self.editor_keymap.move_word_right.is_pressed(event)
            || self.editor_keymap.move_line_start.is_pressed(event)
            || self.editor_keymap.move_line_end.is_pressed(event)
        {
            self.edit_history.typing = None;
            return;
        }
        let now = Instant::now();
        if typing
            && selection.is_none()
            && !self.popups.active()
            && self
                .edit_history
                .typing
                .is_some_and(|(start, last, cursor, _)| {
                    now.duration_since(start) < Duration::from_secs(1)
                        && now.duration_since(last) < Duration::from_millis(500)
                        && cursor == self.current_cursor()
                })
        {
            self.edit_history.typing_edit = Some(TypingEdit::Continue);
            return;
        }
        self.edit_history.typing_edit = typing.then(|| {
            selection.map_or(TypingEdit::Start, |range| {
                TypingEdit::ReplaceSelection(range.start)
            })
        });
        self.begin_edit_transaction();
    }

    /// Once buffered keys are classified as a paste, separate them from preceding typing.
    pub(super) fn begin_detected_paste_edit(&mut self) {
        if self.config.undo_enabled && !self.draft.textarea.is_vim_enabled() {
            self.edit_history.typing_edit = None;
            self.edit_history.typing = None;
            self.begin_edit_transaction();
        }
    }

    /// Reuse the pre-selection draft if the retroactive prefix began with a replacement.
    pub(super) fn begin_retroactive_paste_edit(&mut self, start: usize) {
        let replacement = self.config.undo_enabled
            && !self.draft.textarea.is_vim_enabled()
            && self
                .edit_history
                .typing
                .is_some_and(|(_, _, _, selection)| selection == Some(start));
        if replacement && let Some(snapshot) = self.edit_history.undo.pop_back() {
            self.edit_history.pending = Some(snapshot);
            self.edit_history.typing_edit = None;
            self.edit_history.typing = None;
        } else {
            self.begin_detected_paste_edit();
        }
    }

    /// A separator can promote a held character or extend an already detected paste.
    pub(super) fn begin_promoted_paste_edit(&mut self) {
        if self.config.undo_enabled && !self.draft.textarea.is_vim_enabled() {
            self.edit_history.typing_edit = None;
            self.edit_history.typing = None;
            if self.edit_history.pending.is_none() {
                self.begin_edit_transaction();
            }
        }
    }

    /// Snapshot only keys that can begin or complete a Vim edit transaction.
    pub(super) fn begin_vim_edit(&mut self, event: KeyEvent) {
        if !self.draft.textarea.is_vim_enabled()
            || self.edit_history.pending.is_some()
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

        self.begin_edit_transaction();
    }

    /// Start one standalone draft edit without splitting an active Vim command.
    pub(super) fn begin_direct_edit(&mut self) -> bool {
        if (!self.config.undo_enabled && !self.draft.textarea.is_vim_enabled())
            || self.draft.textarea.is_vim_operator_pending()
            || self.edit_history.pending.is_some()
            || self.edit_history.typing_edit.is_some()
            || self.history_search.is_some()
        {
            return false;
        }

        self.edit_history.typing = None;
        self.edit_history.backspace = None;
        self.begin_edit_transaction();
        if self.edit_history.pending.is_none() {
            return false;
        }
        if self.draft.textarea.is_vim_enabled() {
            let mut vim_state = VimPersistentState::default();
            self.draft
                .textarea
                .swap_vim_persistent_state(&mut vim_state);
            vim_state.commands.last_change.clear();
            self.draft
                .textarea
                .swap_vim_persistent_state(&mut vim_state);
        }
        true
    }

    pub(super) fn begin_edit_transaction(&mut self) {
        let visible_bytes = self.draft.textarea.text().len();
        let paste_bytes = self
            .draft
            .pending_pastes
            .iter()
            .map(|(placeholder, pasted)| placeholder.len() + pasted.len())
            .sum::<usize>();
        if visible_bytes.saturating_add(paste_bytes) > MAX_UNDO_BYTES {
            self.edit_history = EditHistory::default();
            return;
        }

        let snapshot = self.snapshot_draft();
        if snapshot.history_bytes() <= MAX_UNDO_BYTES {
            self.edit_history.pending = Some(snapshot);
            self.edit_history.trim();
        } else {
            self.edit_history = EditHistory::default();
        }
    }

    /// Commit an ordinary edit or a complete Vim command or insert session.
    pub(in crate::bottom_pane) fn finish_edit(&mut self) {
        if self.draft.textarea.is_vim_enabled()
            && (!self.draft.textarea.is_vim_normal_mode()
                || self.draft.textarea.is_vim_operator_pending())
        {
            return;
        }

        if !self.draft.textarea.is_vim_enabled() && self.draft.paste_burst.is_active() {
            return;
        }
        let typing_edit = self.edit_history.typing_edit.take();
        if let Some(TypingEdit::BackspaceContinue(previous)) = typing_edit {
            let cursor = self.current_cursor();
            self.edit_history.backspace = (cursor < previous).then(|| (Instant::now(), cursor));
            return;
        }
        if matches!(typing_edit, Some(TypingEdit::Continue)) {
            let cursor = self.current_cursor();
            self.edit_history.typing =
                self.edit_history
                    .typing
                    .and_then(|(start, _, previous, selection)| {
                        (cursor > previous).then_some((start, Instant::now(), cursor, selection))
                    });
            return;
        }
        let Some(snapshot) = self.edit_history.pending.take() else {
            return;
        };
        if snapshot.text == self.current_text() {
            let mut current = self.snapshot_draft();
            current.cursor = snapshot.cursor;
            if snapshot == current {
                self.edit_history.typing = None;
                return;
            }
        }

        let now = Instant::now();
        self.edit_history.redo.clear();
        self.edit_history.undo.push_back(snapshot);
        self.edit_history.backspace = matches!(typing_edit, Some(TypingEdit::BackspaceStart))
            .then(|| (now, self.current_cursor()));
        self.edit_history.typing = match typing_edit {
            Some(TypingEdit::Start) => Some((now, now, self.current_cursor(), None)),
            Some(TypingEdit::ReplaceSelection(start)) => {
                Some((now, now, self.current_cursor(), Some(start)))
            }
            Some(
                TypingEdit::Continue
                | TypingEdit::BackspaceStart
                | TypingEdit::BackspaceContinue(_),
            )
            | None => None,
        };
        self.edit_history.trim();
    }
}

#[cfg(test)]
#[path = "edit_history_tests.rs"]
mod tests;
