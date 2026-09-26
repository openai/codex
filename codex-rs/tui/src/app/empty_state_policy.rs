//! Allow fresh-thread decoration only on the ordinary compact conversation surface.
//! View changes hide it temporarily; non-startup content dismisses it until a new thread.

use super::*;
use crate::empty_state_animation::ComposerState;
use crate::empty_state_animation::is_startup_cell;
use crate::history_cell::HistoryRenderMode;

impl App {
    pub(super) fn first_screen_composer(&self) -> Option<ComposerState> {
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
        match self.chat_widget.history_render_mode() {
            HistoryRenderMode::Raw => None,
            HistoryRenderMode::Rich => match (
                self.overlay.as_ref(),
                view.is_detailed(),
                view.is_search_active(),
                view.is_activity_focused(),
                view.has_selection_range(),
            ) {
                // A refocus click without selected text remains an ordinary conversation.
                (None, false, false, false, false)
                    if (view.is_following() || view.has_pending_latest_selection())
                        && self.local_settings.tui.effects.welcome
                        && self
                            .chat_widget
                            .empty_state_animation
                            .borrow()
                            .is_eligible() =>
                {
                    composer
                }
                _ => None,
            },
        }
    }
}
