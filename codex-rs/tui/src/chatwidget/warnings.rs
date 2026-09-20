//! Deduplicate incoming diagnostics and project them into the warning footer and viewer.

use crate::history_cell::HistoryCell;
use crate::tui::TuiEvent;
use std::collections::HashSet;
use std::sync::Arc;

const FALLBACK_MODEL_METADATA_WARNING_PREFIX: &str = "Model metadata for `";
const FALLBACK_MODEL_METADATA_WARNING_SUFFIX: &str =
    "` not found. Defaulting to fallback metadata; this can degrade performance and cause issues.";

#[derive(Default)]
pub(super) struct WarningDisplayState {
    pub(super) count: usize,
    /// Completed resume history does not end startup; active work does.
    pub(super) startup_complete: bool,
    /// Initialization config warnings may also arrive as ordinary thread warnings.
    pub(super) startup_config_warnings: HashSet<String>,
    fallback_model_metadata_slugs: HashSet<String>,
}

impl WarningDisplayState {
    pub(super) fn should_display(&mut self, message: &str) -> bool {
        !self.startup_config_warnings.contains(message)
            && fallback_model_metadata_warning_slug(message)
                .is_none_or(|slug| self.fallback_model_metadata_slugs.insert(slug.to_string()))
    }
}

fn fallback_model_metadata_warning_slug(message: &str) -> Option<&str> {
    message
        .strip_prefix(FALLBACK_MODEL_METADATA_WARNING_PREFIX)?
        .strip_suffix(FALLBACK_MODEL_METADATA_WARNING_SUFFIX)
}

impl super::ChatWidget {
    pub(crate) fn open_warnings(&mut self, cells: &[Arc<dyn HistoryCell>]) {
        self.bottom_pane
            .show_warnings(crate::history_cell::warning_entries(cells));
    }

    pub(crate) fn sync_warnings(&mut self, cells: &[Arc<dyn HistoryCell>]) {
        self.warning_display_state.count = crate::history_cell::warning_count(cells);
    }

    pub(crate) fn handle_warning_event(
        &mut self,
        event: &TuiEvent,
        cells: &[Arc<dyn HistoryCell>],
    ) -> bool {
        if let TuiEvent::Key(key) = event
            && self.bottom_pane.warnings_active()
            && matches!(
                key.kind,
                crossterm::event::KeyEventKind::Press | crossterm::event::KeyEventKind::Repeat
            )
        {
            // Chord routing may consume this key before it reaches the warning panel.
            self.bottom_pane
                .record_composer_activity_at(std::time::Instant::now());
        }
        if let TuiEvent::Mouse(mouse) = event
            && mouse.kind
                == crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left)
            && self
                .bottom_pane
                .warning_notice_contains(ratatui::layout::Position::new(mouse.column, mouse.row))
        {
            self.open_warnings(cells);
            return true;
        }
        false
    }
}
