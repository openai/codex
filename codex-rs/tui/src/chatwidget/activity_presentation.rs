//! Expose live activity identity and local detail rendering to the owned transcript.
//! Canonical cells retain all available data; presentation does not modify tool execution.

use super::*;
use crate::transcript_view::ActivityTranscriptLines;

impl ChatWidget {
    pub(crate) fn active_activity_cell(&self) -> Option<&dyn HistoryCell> {
        self.transcript.active_cell.as_deref()
    }

    /// Render only the chosen activity presentation, before any auxiliary live text.
    pub(crate) fn active_cell_owned_transcript_lines(
        &self,
        width: u16,
        expanded: bool,
    ) -> Option<ActivityTranscriptLines> {
        let active = self.transcript.active_cell.as_deref();
        let mode = self.history_render_mode();
        let has_activity = mode == HistoryRenderMode::Rich
            && active.is_some_and(|cell| !cell.activity_ids().is_empty());
        let activity = active.map_or_else(Vec::new, |cell| {
            if expanded && has_activity {
                cell.expanded_hyperlink_lines(width)
            } else {
                history_cell::fullscreen_session_lines(cell, width, /*detailed*/ false, mode)
            }
        });
        let disclosure = active
            .filter(|_| has_activity)
            .and_then(|cell| cell.activity_disclosure(width));
        let mut auxiliary = self.active_auxiliary_lines(width);
        if activity.is_empty() && auxiliary.is_empty() {
            return None;
        }
        if !activity.is_empty() && !auxiliary.is_empty() {
            auxiliary.insert(/*index*/ 0, HyperlinkLine::from(""));
        }
        Some(ActivityTranscriptLines {
            activity,
            auxiliary,
            disclosure,
        })
    }
    pub(crate) fn active_auxiliary_lines(&self, width: u16) -> Vec<HyperlinkLine> {
        let active = self.active_activity_cell();
        let mode = self.history_render_mode();
        self.active_cell_hyperlink_lines_with(width, |cell, width| {
            if active.is_some_and(|active| std::ptr::eq(active, cell)) {
                Vec::new()
            } else {
                history_cell::fullscreen_session_lines(cell, width, /*detailed*/ false, mode)
            }
        })
        .unwrap_or_default()
    }
}
