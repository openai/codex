//! Streaming primitives used by the TUI transcript pipeline.
//!
//! `StreamState` owns newline-gated markdown collection and a FIFO queue of committed render lines.
//! Higher-level modules build on top of this state:
//! - `controller` adapts queued lines into `HistoryCell` emission rules for message and plan streams.
//! - `chunking` computes adaptive drain plans from queue pressure.
//! - `commit_tick` binds policy decisions to concrete controller drains.
//!
//! The key invariant is queue ordering. All drains pop from the front, and enqueue records an
//! arrival timestamp so policy code can reason about oldest queued age without peeking into text.

use std::collections::VecDeque;
use std::time::Duration;
use std::time::Instant;

use ratatui::text::Line;

use crate::markdown_stream::MarkdownStreamCollector;
pub(crate) mod chunking;
pub(crate) mod commit_tick;
pub(crate) mod controller;

struct QueuedLine {
    line: Line<'static>,
    enqueued_at: Instant,
}

/// Holds in-flight markdown stream state and queued committed lines.
pub(crate) struct StreamState {
    pub(crate) collector: MarkdownStreamCollector,
    queued_lines: VecDeque<QueuedLine>,
    pub(crate) has_seen_delta: bool,
}

impl StreamState {
    /// Creates an empty stream state with an optional target wrap width.
    pub(crate) fn new(width: Option<usize>) -> Self {
        Self {
            collector: MarkdownStreamCollector::new(width),
            queued_lines: VecDeque::new(),
            has_seen_delta: false,
        }
    }
    /// Resets collector and queue state for the next stream lifecycle.
    pub(crate) fn clear(&mut self) {
        self.collector.clear();
        self.queued_lines.clear();
        self.has_seen_delta = false;
    }
    /// Drains one queued line from the front of the queue.
    pub(crate) fn step(&mut self) -> Vec<Line<'static>> {
        self.queued_lines
            .pop_front()
            .map(|queued| queued.line)
            .into_iter()
            .collect()
    }
    /// Drains up to `max_lines` queued lines from the front of the queue.
    ///
    /// Callers that pass very large values still get bounded behavior because this method clamps to
    /// the currently available queue length.
    pub(crate) fn drain_n(&mut self, max_lines: usize) -> Vec<Line<'static>> {
        let end = max_lines.min(self.queued_lines.len());
        self.queued_lines
            .drain(..end)
            .map(|queued| queued.line)
            .collect()
    }
    /// Drains all queued lines from the front of the queue.
    pub(crate) fn drain_all(&mut self) -> Vec<Line<'static>> {
        self.queued_lines
            .drain(..)
            .map(|queued| queued.line)
            .collect()
    }
    /// Returns whether no lines are queued for commit.
    pub(crate) fn is_idle(&self) -> bool {
        self.queued_lines.is_empty()
    }
    /// Returns the current queue depth.
    pub(crate) fn queued_len(&self) -> usize {
        self.queued_lines.len()
    }
    /// Returns the age of the oldest queued line.
    pub(crate) fn oldest_queued_age(&self, now: Instant) -> Option<Duration> {
        self.queued_lines
            .front()
            .map(|queued| now.saturating_duration_since(queued.enqueued_at))
    }
    /// Appends committed lines to the queue with a shared enqueue timestamp.
    pub(crate) fn enqueue(&mut self, lines: Vec<Line<'static>>) {
        let now = Instant::now();
        self.queued_lines
            .extend(lines.into_iter().map(|line| QueuedLine {
                line,
                enqueued_at: now,
            }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn drain_n_clamps_to_available_lines() {
        let mut state = StreamState::new(None);
        state.enqueue(vec![Line::from("one")]);

        let drained = state.drain_n(8);
        assert_eq!(drained, vec![Line::from("one")]);
        assert!(state.is_idle());
    }
}
