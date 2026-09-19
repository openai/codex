//! Render live conversation cells in compact or detailed form.
use super::*;
impl ChatWidget {
    pub(crate) fn active_cell_hyperlink_lines_with(
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
}
