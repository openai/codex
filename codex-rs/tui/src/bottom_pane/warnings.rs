//! Warning-panel ownership and focus routing; the ordinary composer is never replaced.

use super::*;
use crate::history_cell::WarningEntry;
use std::sync::Arc;

impl BottomPane {
    pub(crate) fn show_warnings(&mut self, entries: Vec<WarningEntry>, transcript: Arc<()>) {
        self.warnings_view = Some(warnings_view::WarningsView::new(
            entries,
            transcript,
            self.keymap.clone(),
            self.app_event_tx.clone(),
        ));
        self.record_composer_activity_at(Instant::now());
        self.request_redraw();
    }

    pub(crate) fn warnings_active(&self) -> bool {
        self.warnings_view.is_some()
            && self.view_stack.is_empty()
            && !self
                .questions
                .as_ref()
                .is_some_and(|questions| questions.expanded)
    }

    pub(crate) fn warning_notice_contains(&self, position: ratatui::layout::Position) -> bool {
        self.no_modal_or_popup_active() && self.composer.warning_notice_contains(position)
    }
}
