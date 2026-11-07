//! Mouse gestures flush pending typing before layout and hit testing. Left dragging selects
//! editable text using the textarea's last rendered viewport and hides completion suggestions.
//! Double/triple clicks select words/logical lines using the transcript's shared gesture rules.
//! Selection release follows the transcript/footer copy policy while keeping editable text selected.
//! Explicit copy preserves the draft and cursor, and clears after confirmed delivery.
//! Wheel browsing uses the current viewport without changing popup or selection ownership.

use super::*;
use crate::clipboard_copy::CopyStatus;
use crate::clipboard_copy::worker::CopyResult;
use crate::tui::SelectionCopy;
use crate::tui::TuiEvent;
use crossterm::event::MouseButton;
use crossterm::event::MouseEvent;
use crossterm::event::MouseEventKind;

/// The input that will receive clipboard text, independent of a search preview.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ClipboardPasteTarget {
    Draft(String, super::super::textarea::EditTarget),
    HistorySearch(String),
    VimSearch(String, super::super::textarea::EditTarget),
}

impl ChatComposer {
    pub(crate) fn clipboard_paste_target(
        &self,
        shortcut: Option<KeyCode>,
    ) -> Option<ClipboardPasteTarget> {
        let allowed = self.draft.input_enabled
            && !self.blocks_direct_input
            && (shortcut.is_some()
                || (self.history_search.is_none()
                    && self.draft.textarea.vim_query().is_none()
                    && self.draft.textarea.mouse_selection_range().is_none()));
        if !allowed {
            return None;
        }
        Some(if let Some(search) = &self.history_search {
            ClipboardPasteTarget::HistorySearch(search.query.clone())
        } else if let Some(search) = self.draft.textarea.vim_query() {
            ClipboardPasteTarget::VimSearch(
                search.editor.text().to_string(),
                search.editor.edit_target(),
            )
        } else {
            ClipboardPasteTarget::Draft(self.current_text(), self.draft.textarea.edit_target())
        })
    }

    pub(in crate::bottom_pane) fn finish_copy(
        &mut self,
        completion: &(u64, crate::clipboard_copy::worker::CopyResult),
        current: bool,
    ) -> Option<usize> {
        self.draft.textarea.finish_copy(completion, current)
    }

    pub(crate) fn copy_selection(
        &mut self,
        event: &TuiEvent,
        copy: impl FnOnce(&str) -> Result<CopyStatus, String>,
    ) -> Option<(usize, Result<CopyStatus, String>)> {
        let copy_requested = matches!(event, TuiEvent::Key(key) if crate::text_selection::is_copy_key(*key))
            || matches!(event, TuiEvent::Mouse(mouse)
                    if mouse.kind == MouseEventKind::Down(MouseButton::Right)
                        && self.draft.textarea.contains_mouse(*mouse));
        if !copy_requested
            || !self.draft.input_enabled
            || self.history_search.is_some()
            || self.draft.textarea.vim_query().is_some()
        {
            return None;
        }
        if let Some(pasted) = self.draft.paste_burst.flush_before_modified_input() {
            self.apply_paste(pasted);
        }
        self.draft.paste_burst.clear_window_after_non_char();
        let range = self.draft.textarea.mouse_selection_range()?;
        self.end_mouse_drag();
        let text = &self.draft.textarea.text()[range];
        let char_count = text.chars().count();
        let result = copy(text);
        if let Ok(CopyStatus::Pending(id)) = result {
            self.draft.textarea.defer_copy(id);
        }
        if result == Ok(CopyStatus::Confirmed) {
            self.draft.textarea.set_cursor(self.draft.textarea.cursor());
        }
        Some((char_count, result))
    }

    pub(crate) fn copy_selection_on_release(
        &mut self,
        event: MouseEvent,
        selection_copy: SelectionCopy,
        copy: impl FnOnce(&str, SelectionCopy) -> (CopyResult, Option<std::sync::Arc<()>>),
    ) -> Option<(usize, Option<CopyResult>)> {
        if event.kind != MouseEventKind::Up(MouseButton::Left) {
            return None;
        }
        let range = self.draft.textarea.mouse_selection_range()?;
        let text = &self.draft.textarea.text()[range];
        let char_count = text.chars().count();
        let (result, primary_owner) = copy(text, selection_copy);
        self.draft.textarea.retain_primary(primary_owner);
        if selection_copy.copies_clipboard() {
            return Some((char_count, Some(result)));
        }
        Some((char_count, None))
    }

    pub(crate) fn clear_mouse_selection(&mut self) {
        self.draft.textarea.set_cursor(self.draft.textarea.cursor());
    }

    pub(crate) fn end_mouse_drag(&mut self) {
        self.draft.textarea.end_mouse_drag();
    }

    pub(crate) fn cancel_primary(&mut self) {
        self.draft.textarea.cancel_primary();
    }

    pub(crate) fn prepare_mouse(&mut self, event: MouseEvent) -> bool {
        if !self.draft.input_enabled
            || self.blocks_direct_input
            || self.history_search.is_some()
            || self.draft.textarea.vim_query().is_some()
        {
            self.end_mouse_drag();
            return false;
        }
        if matches!(event.kind, MouseEventKind::Down(_)) {
            if let Some(pasted) = self.draft.paste_burst.flush_before_modified_input() {
                self.apply_paste(pasted);
            }
            self.draft.paste_burst.clear_window_after_non_char();
        }
        true
    }

    /// Dispatch after preparation and rendering have refreshed the viewport.
    pub(crate) fn handle_mouse(&mut self, event: MouseEvent) -> bool {
        if self
            .draft
            .textarea
            .scroll_mouse(event, &mut self.draft.textarea_state.borrow_mut())
        {
            return true;
        }
        let handled = self
            .draft
            .textarea
            .handle_mouse(event, *self.draft.textarea_state.borrow());
        if handled {
            self.draft.skill_completion_space = None;
            self.draft.textarea_state.borrow_mut().follow_cursor();
            self.attachments.clear_remote_image_selection();
            self.sync_popups();
        }
        handled
    }
}

#[cfg(test)]
#[path = "mouse_tests.rs"]
mod tests;
