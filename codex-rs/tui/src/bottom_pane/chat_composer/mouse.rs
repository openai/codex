//! Mouse gestures flush pending typing before layout and hit testing. Left dragging selects
//! editable text using the textarea's last rendered viewport and hides completion suggestions.
//! Double/triple clicks select words/logical lines using the transcript's shared gesture rules.
//! Copy shortcuts preserve the draft and selection and use fullscreen copy feedback above the input.

use super::*;
use crossterm::event::MouseEvent;
use crossterm::event::MouseEventKind;

impl ChatComposer {
    pub(crate) fn selection_for_copy(&mut self, key: KeyEvent) -> Option<String> {
        if !crate::text_selection::is_copy_key(key)
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
        self.draft
            .textarea
            .mouse_selection_range()
            .map(|range| self.draft.textarea.text()[range].to_owned())
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
