//! Streaming state for newline-gated assistant output.
//!
//! The streaming pipeline is split into:
//!
//! - [`crate::markdown_stream::MarkdownStreamCollector`]: accumulates raw deltas and commits
//!   complete rendered lines using the most recent width.
//! - [`StreamState`]: a small queue that supports "commit tick" animation by releasing at most one
//!   line per tick.
//! - [`controller::StreamController`]: orchestration (header emission, finalize/drain semantics,
//!   and converting queued lines into `HistoryCell`s).
//!
//! Unlike `tui2`, this pipeline stores fully rendered lines, so the width passed to the collector
//! at stream start controls how wrapping is applied for the lifetime of the stream.

use std::collections::VecDeque;

use ratatui::text::Line;

use crate::markdown_stream::MarkdownStreamCollector;
pub(crate) mod controller;

/// Per-stream queueing state for newline-gated assistant output.
///
/// The collector buffers incoming deltas and commits completed lines, while the queue releases
/// those lines one at a time to drive the streaming animation. The queue is FIFO so commit ticks
/// preserve source order.
pub(crate) struct StreamState {
    /// Accumulates deltas and produces committed, width-aware lines.
    pub(crate) collector: MarkdownStreamCollector,
    /// Buffered lines waiting to be emitted on commit ticks.
    queued_lines: VecDeque<Line<'static>>,
    /// Tracks whether any non-empty delta has been received for the stream.
    ///
    /// This allows callers to detect "empty" streams that never yielded output.
    pub(crate) has_seen_delta: bool,
}

impl StreamState {
    /// Create a fresh streaming state for one assistant message.
    pub(crate) fn new(width: Option<usize>) -> Self {
        Self {
            collector: MarkdownStreamCollector::new(width),
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

    /// Pop at most one queued line (for commit-tick animation).
    pub(crate) fn step(&mut self) -> Vec<Line<'static>> {
        self.queued_lines.pop_front().into_iter().collect()
    }

    /// Drain all queued lines (used on finalize).
    pub(crate) fn drain_all(&mut self) -> Vec<Line<'static>> {
        self.queued_lines.drain(..).collect()
    }

    /// True when there is no queued output waiting to be emitted by commit ticks.
    pub(crate) fn is_idle(&self) -> bool {
        self.queued_lines.is_empty()
    }

    /// Enqueue newly committed lines.
    pub(crate) fn enqueue(&mut self, lines: Vec<Line<'static>>) {
        self.queued_lines.extend(lines);
    }
}
