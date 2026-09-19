//! Expose live activity identity and local detail rendering to the owned transcript.
//! Canonical cells retain all available data; presentation does not modify tool execution.

use super::*;
use crate::transcript_view::ActivityTranscriptLines;

impl ChatWidget {
    pub(super) fn active_cell_hyperlink_lines_with(
        &self,
        width: u16,
        render: impl Fn(&dyn HistoryCell, u16) -> Vec<HyperlinkLine>,
    ) -> Option<Vec<HyperlinkLine>> {
        let mut lines = Vec::new();
        if let Some(cell) = self.transcript.active_cell.as_ref() {
            lines.extend(render(cell.as_ref(), width));
        }
        for cell in self
            .realtime_conversation
            .pending_history_cells
            .iter()
            .chain(self.realtime_conversation.live_transcript_cells())
        {
            let realtime_lines = render(cell.as_ref(), width);
            if !realtime_lines.is_empty() && !lines.is_empty() {
                lines.push(HyperlinkLine::from(""));
            }
            lines.extend(realtime_lines);
        }
        if let Some(rate_limit_reset_hint) = self.pending_rate_limit_reset_hint() {
            let hint_lines = render(rate_limit_reset_hint, width);
            if !hint_lines.is_empty() && !lines.is_empty() {
                lines.push(HyperlinkLine::from(""));
            }
            lines.extend(hint_lines);
        }
        (!lines.is_empty()).then_some(lines)
    }

    pub(crate) fn active_activity_ids(&self) -> Vec<String> {
        self.transcript
            .active_cell
            .as_ref()
            .map_or_else(Vec::new, |cell| cell.activity_ids())
    }

    /// Render only the chosen activity presentation, before any auxiliary live text.
    pub(crate) fn active_cell_owned_transcript_lines(
        &self,
        width: u16,
        expanded: bool,
    ) -> Option<ActivityTranscriptLines> {
        let active = self.transcript.active_cell.as_deref();
        let mode = self.history_render_mode();
        let render_compact = |cell: &dyn HistoryCell, width| {
            if mode == HistoryRenderMode::Raw {
                cell.display_hyperlink_lines_for_mode(width, mode)
            } else {
                cell.compact_hyperlink_lines(width)
            }
        };
        let has_activity = mode == HistoryRenderMode::Rich
            && active.is_some_and(|cell| !cell.activity_ids().is_empty());
        let activity = active.map_or_else(Vec::new, |cell| {
            if expanded && has_activity {
                cell.expanded_hyperlink_lines(width)
            } else {
                render_compact(cell, width)
            }
        });
        let has_hidden_details =
            has_activity && active.is_some_and(|cell| cell.has_hidden_activity_details(width));
        let mut auxiliary = self
            .active_cell_hyperlink_lines_with(width, |cell, width| {
                if active.is_some_and(|active| std::ptr::eq(active, cell)) {
                    Vec::new()
                } else {
                    render_compact(cell, width)
                }
            })
            .unwrap_or_default();
        if activity.is_empty() && auxiliary.is_empty() {
            return None;
        }
        if !activity.is_empty() && !auxiliary.is_empty() {
            auxiliary.insert(/*index*/ 0, HyperlinkLine::from(""));
        }
        Some(ActivityTranscriptLines {
            activity,
            auxiliary,
            has_hidden_details,
        })
    }
}
