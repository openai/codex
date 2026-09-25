//! Mouse gestures flush pending typing before layout and hit testing. Left dragging selects
//! editable text using the textarea's last rendered viewport and hides completion suggestions.
//! Double/triple clicks select words/logical lines using the transcript's shared gesture rules.
//! Copy preserves the draft and cursor; confirmed copies clear selection for every gesture.

use super::*;
use crate::clipboard_copy::CopyStatus;
use crate::tui::TuiEvent;
use crossterm::event::MouseButton;
use crossterm::event::MouseEvent;
use crossterm::event::MouseEventKind;

impl ChatComposer {
    pub(crate) fn can_paste_on_right_click(&self) -> bool {
        self.draft.input_enabled
            && !self.blocks_direct_input
            && self.history_search.is_none()
            && self.draft.textarea.vim_query().is_none()
            && self.draft.textarea.mouse_selection_range().is_none()
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

    pub(crate) fn end_mouse_drag(&mut self) {
        self.draft.textarea.end_mouse_drag();
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
        let handled = self
            .draft
            .textarea
            .handle_mouse(event, *self.draft.textarea_state.borrow());
        if handled {
            self.attachments.clear_remote_image_selection();
            self.sync_popups();
        }
        handled
    }
}

#[cfg(test)]
#[path = "mouse_tests.rs"]
mod tests;
