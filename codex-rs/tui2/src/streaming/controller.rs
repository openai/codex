//! Orchestrates streaming assistant output into immutable transcript cells.
//!
//! The UI receives assistant output as a sequence of deltas. TUI2 wants to:
//!
//! - render incrementally during streaming,
//! - keep the transcript model append-only (emit immutable history cells),
//! - avoid duplicating content or showing partial final lines, and
//! - preserve resize reflow by not baking width-derived wraps into stored cells.
//!
//! [`StreamController`] glues together:
//!
//! - newline-gated delta accumulation (`MarkdownStreamCollector`),
//! - commit-tick animation (`StreamState` queue), and
//! - history cell emission (`AgentMessageCell::new_logical`).
//!
//! Each emitted cell contains **logical markdown lines** plus wrap metadata. The cell wraps those
//! lines at render time using the current viewport width and returns soft-wrap joiners for
//! copy/paste fidelity.

use crate::history_cell::HistoryCell;
use crate::history_cell::{self};
use crate::markdown_render::MarkdownLogicalLine;

use super::StreamState;

/// Controller that manages newline-gated streaming, header emission, and
/// commit animation across streams.
pub(crate) struct StreamController {
    state: StreamState,
    finishing_after_drain: bool,
    header_emitted: bool,
}

impl StreamController {
    /// Create a new controller for one assistant message stream.
    pub(crate) fn new() -> Self {
        Self {
            state: StreamState::new(),
            finishing_after_drain: false,
            header_emitted: false,
        }
    }

    /// Push a streaming delta and enqueue newly completed logical lines.
    ///
    /// Returns `true` when at least one logical line was committed and should trigger commit-tick
    /// animation.
    pub(crate) fn push(&mut self, delta: &str) -> bool {
        let state = &mut self.state;
        if !delta.is_empty() {
            state.has_seen_delta = true;
        }
        state.collector.push_delta(delta);
        if delta.contains('\n') {
            let newly_completed = state.collector.commit_complete_lines();
            if !newly_completed.is_empty() {
                state.enqueue(newly_completed);
                return true;
            }
        }
        false
    }

    /// Finalize the active stream and emit any remaining logical lines.
    ///
    /// This forces the final "partial" line to be committed (if present) and resets the controller
    /// so it is ready for the next stream.
    pub(crate) fn finalize(&mut self) -> Option<Box<dyn HistoryCell>> {
        // Finalize collector first.
        let remaining = {
            let state = &mut self.state;
            state.collector.finalize_and_drain()
        };
        // Collect all output first to avoid emitting headers when there is no content.
        let mut out_lines = Vec::new();
        {
            let state = &mut self.state;
            if !remaining.is_empty() {
                state.enqueue(remaining);
            }
            let step = state.drain_all();
            out_lines.extend(step);
        }

        // Cleanup
        self.state.clear();
        self.finishing_after_drain = false;
        self.emit(out_lines)
    }

    /// Advance the commit-tick animation by at most one logical line.
    ///
    /// Returns `(cell, idle)` where:
    /// - `cell` is a new immutable history cell to append to the transcript (if any output is ready)
    /// - `idle` is `true` once the queue is fully drained.
    pub(crate) fn on_commit_tick(&mut self) -> (Option<Box<dyn HistoryCell>>, bool) {
        let step = self.state.step();
        (self.emit(step), self.state.is_idle())
    }

    fn emit(&mut self, lines: Vec<MarkdownLogicalLine>) -> Option<Box<dyn HistoryCell>> {
        if lines.is_empty() {
            return None;
        }
        Some(Box::new(history_cell::AgentMessageCell::new_logical(
            lines,
            {
                let header_emitted = self.header_emitted;
                self.header_emitted = true;
                !header_emitted
            },
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines_to_plain_strings(lines: &[ratatui::text::Line<'_>]) -> Vec<String> {
        lines
            .iter()
            .map(|l| {
                l.spans
                    .iter()
                    .map(|s| s.content.clone())
                    .collect::<Vec<_>>()
                    .join("")
            })
            .collect()
    }

    #[tokio::test]
    async fn controller_loose_vs_tight_with_commit_ticks_matches_full() {
        let mut ctrl = StreamController::new();
        let mut lines = Vec::new();

        // Exact deltas from the session log (section: Loose vs. tight list items)
        let deltas = vec![
            "\n\n",
            "Loose",
            " vs",
            ".",
            " tight",
            " list",
            " items",
            ":\n",
            "1",
            ".",
            " Tight",
            " item",
            "\n",
            "2",
            ".",
            " Another",
            " tight",
            " item",
            "\n\n",
            "1",
            ".",
            " Loose",
            " item",
            " with",
            " its",
            " own",
            " paragraph",
            ".\n\n",
            "  ",
            " This",
            " paragraph",
            " belongs",
            " to",
            " the",
            " same",
            " list",
            " item",
            ".\n\n",
            "2",
            ".",
            " Second",
            " loose",
            " item",
            " with",
            " a",
            " nested",
            " list",
            " after",
            " a",
            " blank",
            " line",
            ".\n\n",
            "  ",
            " -",
            " Nested",
            " bullet",
            " under",
            " a",
            " loose",
            " item",
            "\n",
            "  ",
            " -",
            " Another",
            " nested",
            " bullet",
            "\n\n",
        ];

        // Simulate streaming with a commit tick attempt after each delta.
        for d in deltas.iter() {
            ctrl.push(d);
            while let (Some(cell), idle) = ctrl.on_commit_tick() {
                lines.extend(cell.transcript_lines(u16::MAX));
                if idle {
                    break;
                }
            }
        }
        // Finalize and flush remaining lines now.
        if let Some(cell) = ctrl.finalize() {
            lines.extend(cell.transcript_lines(u16::MAX));
        }

        let streamed: Vec<_> = lines_to_plain_strings(&lines)
            .into_iter()
            // skip • and 2-space indentation
            .map(|s| s.chars().skip(2).collect::<String>())
            .collect();

        // Full render of the same source
        let source: String = deltas.iter().copied().collect();
        let mut rendered: Vec<ratatui::text::Line<'static>> = Vec::new();
        crate::markdown::append_markdown(&source, None, &mut rendered);
        let rendered_strs = lines_to_plain_strings(&rendered);

        assert_eq!(streamed, rendered_strs);

        // Also assert exact expected plain strings for clarity.
        let expected = vec![
            "Loose vs. tight list items:".to_string(),
            "".to_string(),
            "1. Tight item".to_string(),
            "2. Another tight item".to_string(),
            "3. Loose item with its own paragraph.".to_string(),
            "".to_string(),
            "   This paragraph belongs to the same list item.".to_string(),
            "4. Second loose item with a nested list after a blank line.".to_string(),
            "    - Nested bullet under a loose item".to_string(),
            "    - Another nested bullet".to_string(),
        ];
        assert_eq!(
            streamed, expected,
            "expected exact rendered lines for loose/tight section"
        );
    }
}
