//! Streaming state for newline-gated assistant output.
//!
//! The streaming pipeline in `tui2` is split into:
//!
//! - [`crate::markdown_stream::MarkdownStreamCollector`]: accumulates raw deltas and commits
//!   completed *logical* markdown lines (width-agnostic).
//! - [`StreamState`]: a small queue that supports "commit tick" animation by releasing at most one
//!   logical line per tick.
//! - [`controller::StreamController`]: orchestration (header emission, finalize/drain semantics,
//!   and converting queued logical lines into `HistoryCell`s).
//!
//! Keeping the queued units as logical lines (not wrapped visual lines) is essential for resize
//! reflow: visual wrapping depends on the current viewport width and must be performed at render
//! time inside the relevant history cell.

use std::collections::VecDeque;

use crate::markdown_render::MarkdownLogicalLine;
use crate::markdown_stream::MarkdownStreamCollector;
pub(crate) mod controller;

pub(crate) struct StreamState {
    pub(crate) collector: MarkdownStreamCollector,
    queued_lines: VecDeque<MarkdownLogicalLine>,
    pub(crate) has_seen_delta: bool,
}

impl StreamState {
    /// Create a fresh streaming state for one assistant message.
    pub(crate) fn new() -> Self {
        Self {
            collector: MarkdownStreamCollector::new(),
            queued_lines: VecDeque::new(),
            has_seen_delta: false,
        }
    }
    /// Reset state for the next stream.
    pub(crate) fn clear(&mut self) {
        self.collector.clear();
        self.queued_lines.clear();
        self.has_seen_delta = false;
    }
    /// Pop at most one queued logical line (for commit-tick animation).
    pub(crate) fn step(&mut self) -> Vec<MarkdownLogicalLine> {
        self.queued_lines.pop_front().into_iter().collect()
    }
    /// Drain all queued logical lines (used on finalize).
    pub(crate) fn drain_all(&mut self) -> Vec<MarkdownLogicalLine> {
        self.queued_lines.drain(..).collect()
    }
    /// True when there is no queued output waiting to be emitted by commit ticks.
    pub(crate) fn is_idle(&self) -> bool {
        self.queued_lines.is_empty()
    }
    /// Enqueue newly committed logical lines.
    pub(crate) fn enqueue(&mut self, lines: Vec<MarkdownLogicalLine>) {
        self.queued_lines.extend(lines);
    }
}
