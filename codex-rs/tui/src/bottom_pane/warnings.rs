//! Warning-panel ownership and focus routing; the ordinary composer is never replaced.

use super::*;
use crate::history_cell::WarningEntry;
use std::sync::Arc;

impl BottomPane {
    pub(crate) fn show_warnings(
        &mut self,
        entries: Vec<WarningEntry>,
        transcript: Arc<()>,
        daemon_restart_hint_at: Option<usize>,
    ) {
        let mut view = warnings_view::WarningsView::new(
            entries,
            transcript,
            self.keymap.clone(),
            self.app_event_tx.clone(),
        );
        view.daemon_restart_hint_at = daemon_restart_hint_at;
        self.warnings_view = Some(view);
        self.record_composer_activity_at(Instant::now());
        self.request_redraw();
    }

    pub(crate) fn active_warning(&self) -> Option<&warnings_view::WarningsView> {
        self.warnings_view.as_ref().filter(|_| {
            self.view_stack.is_empty() && !self.questions.as_ref().is_some_and(|q| q.expanded)
        })
    }

    pub(crate) fn warnings_active(&self) -> bool {
        self.active_warning().is_some()
    }

    pub(crate) fn warning_notice_contains(&self, position: ratatui::layout::Position) -> bool {
        self.no_modal_or_popup_active() && self.composer.warning_notice_contains(position)
    }
}
