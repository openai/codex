//! Allow fresh-thread decoration only on the ordinary compact conversation surface.
//! View changes hide it temporarily; non-startup content dismisses it until a new thread.

use super::*;
use crate::empty_state_animation::Presentation;
use crate::empty_state_animation::is_startup_cell;
use crate::history_cell::HistoryRenderMode;
use crate::motion::MotionMode;

impl App {
    pub(super) fn empty_state_presentation(
        &self,
        motion: MotionMode,
        focused: bool,
    ) -> Presentation {
        if !self
            .transcript_cells
            .iter()
            .all(|cell| is_startup_cell(cell.as_ref()))
        {
            self.chat_widget
                .empty_state_animation
                .borrow_mut()
                .dismiss();
        }
        // Evaluate live content even while a different view temporarily owns the screen.
        let composer = self.chat_widget.empty_state_composer();
        let view = &self.transcript_view;
        let composer = match self.chat_widget.history_render_mode() {
            HistoryRenderMode::Raw => None,
            HistoryRenderMode::Rich => match (
                self.overlay.as_ref(),
                view.is_detailed(),
                view.is_search_active(),
                view.is_activity_focused(),
                view.has_selection_range(),
            ) {
                // A refocus click without selected text remains an ordinary conversation.
                (None, false, false, false, false) => composer,
                _ => None,
            },
        };
        Presentation::for_composer(composer, motion, focused)
    }
}
