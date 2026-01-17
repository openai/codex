//! Orchestrates streaming assistant output into immutable transcript cells.
//!
//! The UI receives assistant output as a sequence of deltas. This controller:
//!
//! - gates commits on newline boundaries so partial markdown isn't rendered,
//! - animates streaming by releasing one logical line per commit tick, and
//! - emits immutable history cells with a single header per stream.
//!
//! It relies on [`StreamState`] and [`MarkdownStreamCollector`] for buffering and rendering, but
//! it is responsible for deciding when to emit history cells and when to flush the stream. The
//! controller never mutates past cells; it only drains queued lines into new, immutable cells.
//!
//! [`MarkdownStreamCollector`]: crate::markdown_stream::MarkdownStreamCollector

use crate::history_cell::HistoryCell;
use crate::history_cell::{self};
use ratatui::text::Line;

use super::StreamState;

/// Drives the newline-gated streaming pipeline for one assistant message.
///
/// The controller owns the stream-local state, converts incoming deltas into queued lines, and
/// emits [`HistoryCell`] instances as those lines are committed. It is not responsible for timing
/// commit ticks; callers decide when to call [`Self::on_commit_tick`].
pub(crate) struct StreamController {
    /// Per-stream state for buffering deltas and queued, committed lines.
    state: StreamState,
    /// Placeholder for a two-phase drain lifecycle; currently always reset to `false`.
    ///
    /// This is kept to mirror upstream stream-control state even though the TUI
    /// currently drains in a single pass.
    finishing_after_drain: bool,
    /// Tracks whether the assistant header has been emitted for this stream.
    header_emitted: bool,
}

impl StreamController {
    /// Creates a controller scoped to a single assistant stream.
    ///
    /// The optional `width` is forwarded to the markdown stream collector so wrapping matches the
    /// current viewport at commit time.
    pub(crate) fn new(width: Option<usize>) -> Self {
        Self {
            state: StreamState::new(width),
            finishing_after_drain: false,
            header_emitted: false,
        }
    }

    /// Pushes a streaming delta and enqueues newly completed lines.
    ///
    /// Returns `true` when at least one line was committed, which callers can use to start or
    /// continue commit-tick animation.
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

    /// Finalizes the active stream and emits any remaining lines.
    ///
    /// This forces the collector to commit a trailing partial line (if present) and resets the
    /// controller for the next stream. Returns `None` if the stream produced no lines.
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

    /// Advances the commit-tick animation by at most one queued line.
    ///
    /// Returns `(cell, idle)` where `cell` is the next history cell to append (if any) and `idle`
    /// reports whether the queue is fully drained.
    pub(crate) fn on_commit_tick(&mut self) -> (Option<Box<dyn HistoryCell>>, bool) {
        let step = self.state.step();
        (self.emit(step), self.state.is_idle())
    }

    /// Wraps committed lines into a history cell, emitting a header only once per stream.
    fn emit(&mut self, lines: Vec<Line<'static>>) -> Option<Box<dyn HistoryCell>> {
        if lines.is_empty() {
            return None;
        }
        Some(Box::new(history_cell::AgentMessageCell::new(lines, {
            let header_emitted = self.header_emitted;
            self.header_emitted = true;
            !header_emitted
        })))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Convert ratatui lines into plain strings for snapshot-friendly comparisons.
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

    /// Confirms commit-tick streaming output matches a full markdown render.
    #[tokio::test]
    async fn controller_loose_vs_tight_with_commit_ticks_matches_full() {
        let mut ctrl = StreamController::new(None);
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
