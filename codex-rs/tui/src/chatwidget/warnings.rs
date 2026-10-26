//! Deduplicate incoming diagnostics and project them into the warning footer and viewer.

use crate::history_cell::HistoryCell;
use crate::history_cell::WarningEntry;
use crate::history_cell::WarningId;
use crate::transcript_view::TextLayout;
use crate::tui::TuiEvent;
use ratatui::layout::Rect;
use std::collections::BTreeMap;
use std::collections::HashSet;
use std::sync::Arc;

const FALLBACK_MODEL_METADATA_WARNING_PREFIX: &str = "Model metadata for `";
const FALLBACK_MODEL_METADATA_WARNING_SUFFIX: &str =
    "` not found. Defaulting to fallback metadata; this can degrade performance and cause issues.";

#[derive(Default)]
pub(crate) struct WarningDisplayState {
    pub(super) count: usize,
    /// Completed resume history does not end startup; active work does.
    pub(super) startup_complete: bool,
    /// Initialization config warnings may also arrive as ordinary thread warnings.
    pub(super) startup_config_warnings: HashSet<String>,
    fallback_model_metadata_slugs: HashSet<String>,
    /// Only hide an unchanged diagnostic: an identity can acquire more details later.
    pub(crate) dismissed: BTreeMap<WarningId, String>,
    /// Identity of the current transcript; queued decisions must not survive its reset.
    pub(crate) transcript: Arc<()>,
    /// Warning-bearing history cells are immutable. Avoid rebuilding diagnostic text until
    /// the transcript's cell identities or the user's dismissal decisions change.
    pub(crate) synced_cells: Option<Vec<Arc<dyn HistoryCell>>>,
}

impl WarningDisplayState {
    fn visible_entries(&self, cells: &[Arc<dyn HistoryCell>]) -> Vec<WarningEntry> {
        crate::history_cell::warning_entries(cells)
            .into_iter()
            .filter(|entry| self.dismissed.get(&entry.id) != Some(&entry.details))
            .collect()
    }

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
    pub(crate) fn take_warning_body(&self) -> Option<(Rect, TextLayout, usize)> {
        let view = self.bottom_pane.active_warning()?;
        let (layout, offset) = view.body_layout.take()?;
        Some((view.body_area.get(), layout, offset))
    }

    pub(crate) fn record_warning_activity(&mut self) {
        self.bottom_pane
            .record_composer_activity_at(std::time::Instant::now());
    }

    pub(crate) fn open_warnings(&mut self, cells: &[Arc<dyn HistoryCell>]) {
        *self.rendered_selection.borrow_mut() = Default::default();
        self.empty_state_animation.borrow_mut().pause_clock();
        let entries = self.warning_display_state.visible_entries(cells);
        let daemon_restart_hint_at = self
            .remote_connection
            .as_ref()
            .filter(|connection| connection.is_managed_daemon)
            .and_then(|_| {
                entries.iter().position(|entry| match &entry.id {
                    WarningId::Message(message) => self
                        .warning_display_state
                        .startup_config_warnings
                        .contains(message),
                    WarningId::McpServer(_) => false,
                })
            });
        self.bottom_pane.show_warnings(
            entries,
            Arc::clone(&self.warning_display_state.transcript),
            daemon_restart_hint_at,
        );
    }

    pub(crate) fn sync_warnings(&mut self, cells: &[Arc<dyn HistoryCell>]) {
        let state = &mut self.warning_display_state;
        if state.synced_cells.as_ref().is_some_and(|synced| {
            synced.len() == cells.len()
                && synced
                    .iter()
                    .zip(cells)
                    .all(|(previous, current)| Arc::ptr_eq(previous, current))
        }) {
            return;
        }
        state.count = if state.dismissed.is_empty() {
            crate::history_cell::warning_count(cells)
        } else {
            state.visible_entries(cells).len()
        };
        state.synced_cells = Some(cells.to_vec());
    }

    pub(crate) fn handle_warning_event(
        &mut self,
        event: &TuiEvent,
        cells: &[Arc<dyn HistoryCell>],
    ) -> bool {
        if let TuiEvent::Key(key) = event
            && self.bottom_pane.suppress_warning_keep_repeat
        {
            if key.kind == crossterm::event::KeyEventKind::Repeat
                && crate::key_hint::plain(crossterm::event::KeyCode::Char('k')).is_press(*key)
            {
                return true;
            }
            self.bottom_pane.suppress_warning_keep_repeat = false;
        }
        if let TuiEvent::Key(key) = event
            && self.bottom_pane.warnings_active()
            && matches!(
                key.kind,
                crossterm::event::KeyEventKind::Press | crossterm::event::KeyEventKind::Repeat
            )
        {
            // Chord routing may consume this key before it reaches the warning panel.
            self.record_warning_activity();
        }
        if let TuiEvent::Mouse(mouse) = event
            && self
                .bottom_pane
                .active_warning()
                .is_some_and(|view| view.scroll_mouse(*mouse))
        {
            self.record_warning_activity();
            self.request_redraw();
            return true;
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
