//! Ordered calls and transcript-only reasoning for adjacent activity groups.
//!
//! Tool cells own call matching and compact previews. Reasoning is attached after the calls
//! already started, preserving its position as they complete without breaking the compact display.

use super::HistoryCell;
use super::HistoryRenderMode;
use ratatui::text::Line;

#[derive(Debug)]
pub(crate) struct ActivityGroup<T> {
    pub(crate) calls: Vec<T>,
    reasoning: Vec<(usize, Box<dyn HistoryCell>)>,
}

impl<T> ActivityGroup<T> {
    pub(crate) fn new(calls: Vec<T>) -> Self {
        Self {
            calls,
            reasoning: Vec::new(),
        }
    }

    pub(crate) fn push_reasoning(&mut self, cell: Box<dyn HistoryCell>) {
        self.reasoning.push((self.calls.len(), cell));
    }

    pub(crate) fn transcript_lines(
        &self,
        width: u16,
        mode: HistoryRenderMode,
        mut render_call: impl FnMut(usize, &T, &mut Vec<Line<'static>>),
    ) -> Vec<Line<'static>> {
        let mut lines = Vec::new();
        let mut reasoning = self.reasoning.iter().peekable();
        for (i, call) in self.calls.iter().enumerate() {
            render_call(i, call, &mut lines);
            if mode == HistoryRenderMode::Rich {
                while let Some((_, cell)) =
                    reasoning.next_if(|(after_calls, _)| *after_calls == i + 1)
                {
                    let reasoning_lines = cell.transcript_lines(width);
                    if !reasoning_lines.is_empty() {
                        lines.push("".into());
                        lines.extend(reasoning_lines);
                    }
                }
            }
        }
        lines
    }
}
