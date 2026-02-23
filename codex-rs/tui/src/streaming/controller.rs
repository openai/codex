//! Two-region streaming controllers for agent messages and proposed plans.
//!
//! Each stream partitions rendered markdown into a *stable region* (committed
//! to scrollback via the animation queue in `StreamState`) and a *tail region*
//! (mutable, displayed in the active-cell slot as `StreamingAgentTailCell`).
//!
//! `StreamCore` owns the shared bookkeeping: source accumulation, re-rendering,
//! stable/tail partitioning, commit-animation queue management, and terminal
//! resize handling.  `StreamController` and `PlanStreamController` are thin
//! wrappers that add only their `emit()` styling and finalize return types.
//!
//! ## Table holdback
//!
//! Table rendering is inherently non-incremental: adding a new row can change
//! every column's width and reshape all prior rows.  The holdback mechanism
//! (`table_holdback_state`) detects pipe-table patterns (header + delimiter
//! pair) in the accumulated source and keeps content from the table header
//! onward as mutable tail until the stream finalizes. Holdback is enabled for
//! agent streams and disabled for proposed-plan streams so plan output
//! continues to stream incrementally. Lines in `Outside` and `Markdown` fence
//! contexts are scanned; lines inside non-markdown fences are skipped.
//!
//! ## Resize handling
//!
//! On terminal width change, `StreamCore::set_width` re-renders at the new
//! width and rebuilds the queued stable region from the current emitted line
//! count. This intentionally avoids byte-level remap complexity while the
//! stream is active; finalized content is canonicalized by transcript
//! consolidation into source-backed markdown cells.
//!
//! ## Invariants
//!
//! - `emitted_stable_len <= enqueued_stable_len <= rendered_lines.len()`.
//! - `raw_source` is append-only until `reset()`; never modified mid-stream.
//! - Tail starts exactly at `enqueued_stable_len`.
//! - During confirmed table streaming, only lines from the table header onward
//!   are forced into tail; pre-table lines may remain stable.

use crate::history_cell::HistoryCell;
use crate::history_cell::{self};
use crate::markdown::append_markdown_agent;
use crate::render::line_utils::prefix_lines;
use crate::style::proposed_plan_style;
use ratatui::prelude::Stylize;
use ratatui::text::Line;
use std::time::Duration;
use std::time::Instant;

use crate::table_detect::FenceKind;
use crate::table_detect::FenceTracker;
use crate::table_detect::is_table_delimiter_line;
use crate::table_detect::is_table_header_line;
use crate::table_detect::parse_table_segments;
use crate::table_detect::strip_blockquote_prefix;

use super::StreamState;

// ---------------------------------------------------------------------------
// StreamCore — shared bookkeeping for both stream controllers
// ---------------------------------------------------------------------------

/// Shared state and logic for the two-region streaming model.
///
/// Both [`StreamController`] (agent messages) and [`PlanStreamController`]
/// (proposed plans) delegate their core bookkeeping here: source
/// accumulation, re-rendering, stable/tail partitioning, commit-animation
/// queue management, and terminal resize handling.
///
/// The wrapping controllers add only their own `emit()` styling and
/// finalize return types.
struct StreamCore {
    state: StreamState,
    /// Whether table holdback is active for this stream type.
    table_holdback_mode: TableHoldbackMode,
    /// Current rendering width (columns available for markdown content).
    width: Option<usize>,
    /// Accumulated raw markdown source for the current stream.
    raw_source: String,
    /// Full re-render of `raw_source` at `width`. Rebuilt on every committed delta.
    rendered_lines: Vec<Line<'static>>,
    /// Lines enqueued into the commit-animation queue.
    enqueued_stable_len: usize,
    /// Lines actually emitted to scrollback.
    emitted_stable_len: usize,
    /// Cached rendered line count for prefix-before-table keyed by source start and width.
    stable_prefix_len_cache: Option<StablePrefixLenCache>,
    /// Incremental holdback scanner state for append-only source updates.
    holdback_scanner: TableHoldbackScanner,
}

struct StablePrefixLenCache {
    source_start: usize,
    width: Option<usize>,
    stable_prefix_len: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TableHoldbackMode {
    Enabled,
    Disabled,
}

impl StreamCore {
    fn new(width: Option<usize>, table_holdback_mode: TableHoldbackMode) -> Self {
        Self {
            state: StreamState::new(width),
            table_holdback_mode,
            width,
            raw_source: String::with_capacity(1024),
            rendered_lines: Vec::with_capacity(64),
            enqueued_stable_len: 0,
            emitted_stable_len: 0,
            stable_prefix_len_cache: None,
            holdback_scanner: TableHoldbackScanner::new(),
        }
    }

    /// Push a delta; if it contains a newline, commit completed lines and enqueue newly-stable
    /// lines. Returns `true` if new lines were enqueued.
    fn push_delta(&mut self, delta: &str) -> bool {
        if !delta.is_empty() {
            self.state.has_seen_delta = true;
        }
        self.state.collector.push_delta(delta);

        let mut enqueued = false;
        if delta.contains('\n')
            && let Some(committed_source) = self.state.collector.commit_complete_source()
        {
            self.raw_source.push_str(&committed_source);
            if self.table_holdback_mode == TableHoldbackMode::Enabled {
                self.holdback_scanner.push_source_chunk(&committed_source);
            }
            self.recompute_streaming_render();
            enqueued = self.sync_stable_queue();
        }
        enqueued
    }

    /// Drain the collector, re-render, and return lines not yet emitted.
    /// Does NOT reset state - the caller must call `reset()` afterward.
    fn finalize_remaining(&mut self) -> Vec<Line<'static>> {
        let remainder_source = self.state.collector.finalize_and_drain_source();
        if !remainder_source.is_empty() {
            self.raw_source.push_str(&remainder_source);
            if self.table_holdback_mode == TableHoldbackMode::Enabled {
                self.holdback_scanner.push_source_chunk(&remainder_source);
            }
        }
        let mut rendered = Vec::new();
        append_markdown_agent(&self.raw_source, self.width, &mut rendered);
        if self.emitted_stable_len >= rendered.len() {
            Vec::new()
        } else {
            rendered[self.emitted_stable_len..].to_vec()
        }
    }

    /// Step animation: dequeue one line, update the emitted count.
    fn tick(&mut self) -> Vec<Line<'static>> {
        let step = self.state.step();
        self.emitted_stable_len += step.len();
        step
    }

    /// Batch drain: dequeue up to `max_lines`, update the emitted count.
    fn tick_batch(&mut self, max_lines: usize) -> Vec<Line<'static>> {
        if max_lines == 0 {
            return Vec::new();
        }
        let step = self.state.drain_n(max_lines);
        if step.is_empty() {
            return step;
        }
        self.emitted_stable_len += step.len();
        step
    }

    // Trivial StreamCore accessors inlined — called on every animation tick
    // and render frame during active streaming.

    #[inline]
    fn is_idle(&self) -> bool {
        self.state.is_idle()
    }

    #[inline]
    fn queued_lines(&self) -> usize {
        self.state.queued_len()
    }

    #[inline]
    fn oldest_queued_age(&self, now: Instant) -> Option<Duration> {
        self.state.oldest_queued_age(now)
    }

    /// Lines that belong to the mutable tail, not yet queued for stable commit.
    ///
    /// The tail starts at `enqueued_stable_len` -- everything from that offset to
    /// the end of `rendered_lines` is displayed live in the active-cell slot.
    #[inline]
    fn current_tail_lines(&self) -> Vec<Line<'static>> {
        let start = self.enqueued_stable_len.min(self.rendered_lines.len());
        self.rendered_lines[start..].to_vec()
    }

    #[inline]
    fn has_tail(&self) -> bool {
        self.enqueued_stable_len < self.rendered_lines.len()
    }

    /// Update rendering width and rebuild queued stable lines for the new layout.
    ///
    /// Re-renders once at the new width and rebuilds queue state from the
    /// current emitted line count.
    fn set_width(&mut self, width: Option<usize>) {
        if self.width == width {
            return;
        }
        let had_pending_queue = self.state.queued_len() > 0;
        let had_live_tail = self.has_tail();
        self.width = width;
        self.state.collector.set_width(width);
        if self.raw_source.is_empty() {
            return;
        }

        self.recompute_streaming_render();
        self.emitted_stable_len = self.emitted_stable_len.min(self.rendered_lines.len());
        if had_pending_queue
            && self.emitted_stable_len == self.rendered_lines.len()
            && self.emitted_stable_len > 0
        {
            // If wrapped remainder compresses into fewer lines at the new width,
            // keep at least one line un-emitted so pre-resize pending content is
            // not skipped permanently.
            self.emitted_stable_len -= 1;
        }
        self.state.clear_queue();
        if self.emitted_stable_len > 0 && !had_pending_queue && !had_live_tail {
            // Avoid replaying already-emitted content after resize when no
            // stable lines were waiting in the queue and there was no mutable
            // tail to preserve.
            self.enqueued_stable_len = self.rendered_lines.len();
            return;
        }
        self.rebuild_stable_queue_from_render();
    }

    /// Clear all accumulated state for current stream.
    fn reset(&mut self) {
        self.state.clear();
        self.raw_source.clear();
        self.rendered_lines.clear();
        self.enqueued_stable_len = 0;
        self.emitted_stable_len = 0;
        self.stable_prefix_len_cache = None;
        self.holdback_scanner.reset();
    }

    /// Re-render the full `raw_source` at current `width`.
    fn recompute_streaming_render(&mut self) {
        self.rendered_lines.clear();
        append_markdown_agent(&self.raw_source, self.width, &mut self.rendered_lines);
    }

    /// Compute how many rendered lines should be in the stable region.
    fn compute_target_stable_len(&mut self) -> usize {
        let tail_budget = self.active_tail_budget_lines();
        self.rendered_lines
            .len()
            .saturating_sub(tail_budget)
            .max(self.emitted_stable_len)
    }

    /// Advance `enqueued_stable_len` toward the target stable boundary and enqueue any
    /// newly-stable lines. Returns `true` if new lines were enqueued.
    fn sync_stable_queue(&mut self) -> bool {
        let target_stable_len = self.compute_target_stable_len();

        // A structural rewrite moved the stable boundary backward into enqueue-but-unemitted
        // lines. Rebuild queue from the latest snapshot.
        if target_stable_len < self.enqueued_stable_len {
            self.state.clear_queue();
            if self.emitted_stable_len < target_stable_len {
                self.state.enqueue(
                    self.rendered_lines[self.emitted_stable_len..target_stable_len].to_vec(),
                );
            }
            self.enqueued_stable_len = target_stable_len;
            return self.state.queued_len() > 0;
        }

        if target_stable_len == self.enqueued_stable_len {
            return false;
        }

        self.state
            .enqueue(self.rendered_lines[self.enqueued_stable_len..target_stable_len].to_vec());
        self.enqueued_stable_len = target_stable_len;
        true
    }

    /// Rebuild the stable queue from the current render snapshot. Used after `set_width()` where
    /// the old queue is stale.
    fn rebuild_stable_queue_from_render(&mut self) {
        let target_stable_len = self.compute_target_stable_len();
        self.state.clear_queue();
        if self.emitted_stable_len < target_stable_len {
            self.state
                .enqueue(self.rendered_lines[self.emitted_stable_len..target_stable_len].to_vec());
        }
        self.enqueued_stable_len = target_stable_len;
    }

    /// How many rendered lines to withhold as mutable tail.
    ///
    /// When a table is detected (`Confirmed` or `PendingHeader`), the entire
    /// table region is held as tail because adding a row can reshape table
    /// column widths. For `PendingHeader`, only content from the speculative
    /// header line onward is kept mutable so earlier prose can continue
    /// streaming. When no table is detected, everything flows directly to
    /// stable. This is the core decision point for the holdback mechanism.
    fn active_tail_budget_lines(&mut self) -> usize {
        if self.table_holdback_mode == TableHoldbackMode::Disabled {
            return 0;
        }
        let scan_start = Instant::now();
        let holdback_state = self.holdback_scanner.state();
        let tail_budget = match holdback_state {
            TableHoldbackState::Confirmed { table_start: start }
            | TableHoldbackState::PendingHeader {
                header_start: start,
            } => self.tail_budget_from_source_start(start),
            TableHoldbackState::None => 0,
        };
        tracing::trace!(
            state = ?holdback_state,
            tail_budget,
            elapsed_us = scan_start.elapsed().as_micros(),
            "table holdback decision",
        );
        tail_budget
    }

    fn tail_budget_from_source_start(&mut self, source_start: usize) -> usize {
        if source_start == 0 {
            return self.rendered_lines.len();
        }
        let source_start = source_start.min(self.raw_source.len());
        let stable_prefix_len = self.stable_prefix_len_for_source_start(source_start);
        self.rendered_lines.len().saturating_sub(stable_prefix_len)
    }

    fn stable_prefix_len_for_source_start(&mut self, source_start: usize) -> usize {
        if let Some(cache) = &self.stable_prefix_len_cache
            && cache.source_start == source_start
            && cache.width == self.width
        {
            tracing::trace!(
                source_start,
                width = ?self.width,
                stable_prefix_len = cache.stable_prefix_len,
                "table holdback stable-prefix cache hit",
            );
            return cache.stable_prefix_len;
        }

        let render_start = Instant::now();
        let mut stable_prefix_render = Vec::new();
        append_markdown_agent(
            &self.raw_source[..source_start.min(self.raw_source.len())],
            self.width,
            &mut stable_prefix_render,
        );
        let stable_prefix_len = stable_prefix_render.len();
        tracing::trace!(
            source_start,
            width = ?self.width,
            stable_prefix_len,
            elapsed_us = render_start.elapsed().as_micros(),
            "table holdback stable-prefix render",
        );
        self.stable_prefix_len_cache = Some(StablePrefixLenCache {
            source_start,
            width: self.width,
            stable_prefix_len,
        });
        stable_prefix_len
    }
}

/// Controller for streaming agent message content with table-aware holdback.
///
/// Wraps [`StreamCore`] and adds `AgentMessageCell` emission styling.
pub(crate) struct StreamController {
    core: StreamCore,
    header_emitted: bool,
}

impl StreamController {
    pub(crate) fn new(width: Option<usize>) -> Self {
        Self {
            core: StreamCore::new(width, TableHoldbackMode::Enabled),
            header_emitted: false,
        }
    }

    pub(crate) fn push(&mut self, delta: &str) -> bool {
        self.core.push_delta(delta)
    }

    /// Finalize the active stream. Returns the final cell (if any remaining lines) and the raw
    /// markdown source for consolidation.
    pub(crate) fn finalize(&mut self) -> (Option<Box<dyn HistoryCell>>, Option<String>) {
        let remaining = self.core.finalize_remaining();
        if self.core.raw_source.is_empty() {
            self.core.reset();
            return (None, None);
        }

        // Move ownership — source is consumed before reset() clears it.
        let source = std::mem::take(&mut self.core.raw_source);
        let out = self.emit(remaining);
        self.core.reset();
        (out, Some(source))
    }

    pub(crate) fn on_commit_tick(&mut self) -> (Option<Box<dyn HistoryCell>>, bool) {
        let step = self.core.tick();
        (self.emit(step), self.core.is_idle())
    }

    pub(crate) fn on_commit_tick_batch(
        &mut self,
        max_lines: usize,
    ) -> (Option<Box<dyn HistoryCell>>, bool) {
        let step = self.core.tick_batch(max_lines);
        (self.emit(step), self.core.is_idle())
    }

    // Thin StreamController accessors inlined — one-liner delegates called
    // on every render frame and animation tick.

    #[inline]
    pub(crate) fn queued_lines(&self) -> usize {
        self.core.queued_lines()
    }

    pub(crate) fn oldest_queued_age(&self, now: Instant) -> Option<Duration> {
        self.core.oldest_queued_age(now)
    }

    #[inline]
    pub(crate) fn current_tail_lines(&self) -> Vec<Line<'static>> {
        self.core.current_tail_lines()
    }

    #[inline]
    pub(crate) fn tail_starts_stream(&self) -> bool {
        !self.header_emitted && self.core.enqueued_stable_len == 0
    }

    #[inline]
    pub(crate) fn has_live_tail(&self) -> bool {
        self.core.has_tail()
    }

    pub(crate) fn clear_queue(&mut self) {
        self.core.state.clear_queue();
        self.core.enqueued_stable_len = self.core.emitted_stable_len;
    }

    pub(crate) fn set_width(&mut self, width: Option<usize>) {
        self.core.set_width(width);
    }

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
// ---------------------------------------------------------------------------
// PlanStreamController — proposed plan streams
// ---------------------------------------------------------------------------

/// Controller that streams proposed plan markdown into a styled plan block.
///
/// Wraps [`StreamCore`] and adds plan-specific header, indentation, and
/// background styling.
pub(crate) struct PlanStreamController {
    core: StreamCore,
    header_emitted: bool,
    top_padding_emitted: bool,
}

impl PlanStreamController {
    pub(crate) fn new(width: Option<usize>) -> Self {
        Self {
            core: StreamCore::new(width, TableHoldbackMode::Disabled),
            header_emitted: false,
            top_padding_emitted: false,
        }
    }

    pub(crate) fn push(&mut self, delta: &str) -> bool {
        self.core.push_delta(delta)
    }

    /// Finalize the active stream. Returns the final cell (if any remaining
    /// lines) plus raw markdown source for consolidation.
    pub(crate) fn finalize(&mut self) -> (Option<Box<dyn HistoryCell>>, Option<String>) {
        let remaining = self.core.finalize_remaining();
        if self.core.raw_source.is_empty() {
            self.core.reset();
            return (None, None);
        }

        // Move ownership — source is consumed before reset() clears it.
        let source = std::mem::take(&mut self.core.raw_source);
        let out = self.emit(remaining, true);
        self.core.reset();
        (out, Some(source))
    }

    pub(crate) fn on_commit_tick(&mut self) -> (Option<Box<dyn HistoryCell>>, bool) {
        let step = self.core.tick();
        (self.emit(step, false), self.core.is_idle())
    }

    pub(crate) fn on_commit_tick_batch(
        &mut self,
        max_lines: usize,
    ) -> (Option<Box<dyn HistoryCell>>, bool) {
        let step = self.core.tick_batch(max_lines);
        (self.emit(step, false), self.core.is_idle())
    }

    #[inline]
    pub(crate) fn queued_lines(&self) -> usize {
        self.core.queued_lines()
    }

    #[inline]
    pub(crate) fn has_live_tail(&self) -> bool {
        self.core.has_tail()
    }

    pub(crate) fn oldest_queued_age(&self, now: Instant) -> Option<Duration> {
        self.core.oldest_queued_age(now)
    }

    pub(crate) fn clear_queue(&mut self) {
        self.core.state.clear_queue();
        self.core.enqueued_stable_len = self.core.emitted_stable_len;
    }

    pub(crate) fn set_width(&mut self, width: Option<usize>) {
        self.core.set_width(width);
    }

    fn emit(
        &mut self,
        lines: Vec<Line<'static>>,
        include_bottom_padding: bool,
    ) -> Option<Box<dyn HistoryCell>> {
        if lines.is_empty() && !include_bottom_padding {
            return None;
        }

        let mut out_lines: Vec<Line<'static>> = Vec::with_capacity(4);
        let is_stream_continuation = self.header_emitted;
        if !self.header_emitted {
            out_lines.push(vec!["• ".dim(), "Proposed Plan".bold()].into());
            out_lines.push(Line::from(" "));
            self.header_emitted = true;
        }

        let mut plan_lines: Vec<Line<'static>> = Vec::with_capacity(4);
        if !self.top_padding_emitted {
            plan_lines.push(Line::from(" "));
            self.top_padding_emitted = true;
        }
        plan_lines.extend(lines);
        if include_bottom_padding {
            plan_lines.push(Line::from(" "));
        }

        let plan_style = proposed_plan_style();
        let plan_lines = prefix_lines(plan_lines, "  ".into(), "  ".into())
            .into_iter()
            .map(|line| line.style(plan_style))
            .collect::<Vec<_>>();
        out_lines.extend(plan_lines);

        Some(Box::new(history_cell::new_proposed_plan_stream(
            out_lines,
            is_stream_continuation,
        )))
    }
}

// ---------------------------------------------------------------------------
// Table holdback infrastructure
// ---------------------------------------------------------------------------

/// A source line annotated with whether it falls inside a fenced code block.
#[cfg(test)]
struct ParsedLine<'a> {
    text: &'a str,
    fence_context: FenceKind,
    source_start: usize,
}

/// Parse source into lines tagged with fenced-code context for table scanning.
///
/// Uses [`FenceTracker`] from `table_detect` for fence state transitions.
#[cfg(test)]
fn parse_lines_with_fence_state(source: &str) -> Vec<ParsedLine<'_>> {
    let mut tracker = FenceTracker::new();
    let mut lines = Vec::new();
    let mut source_start = 0usize;

    for raw_line in source.split('\n') {
        lines.push(ParsedLine {
            text: raw_line,
            fence_context: tracker.kind(),
            source_start,
        });

        tracker.advance(raw_line);
        source_start = source_start
            .saturating_add(raw_line.len())
            .saturating_add(1);
    }

    lines
}

/// Strip blockquote prefixes and return the trimmed text if it contains
/// pipe-table segments, or `None` otherwise.
fn table_candidate_text(line: &str) -> Option<&str> {
    let stripped = strip_blockquote_prefix(line).trim();
    parse_table_segments(stripped).map(|_| stripped)
}

/// Result of scanning accumulated raw source for pipe-table patterns.
///
/// `StreamCore::active_tail_budget_lines` uses this to decide whether to
/// commit rendered lines to the stable queue or withhold them as mutable tail.
/// The incremental `TableHoldbackScanner` maintains state across deltas;
/// the stateless `table_holdback_state` function is used only in tests.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TableHoldbackState {
    /// No table detected -- all rendered lines can flow into the stable queue.
    None,
    /// The last non-blank line looks like a table header row but no delimiter
    /// row has followed yet. Hold back in case the next delta is a delimiter.
    PendingHeader { header_start: usize },
    /// A header + delimiter pair was found -- the source contains a confirmed
    /// table. Content from the table header onward stays mutable.
    Confirmed { table_start: usize },
}

#[derive(Clone, Copy)]
struct PreviousLineState {
    source_start: usize,
    fence_kind: FenceKind,
    is_header: bool,
}

/// Incremental scanner for table holdback state on append-only source streams.
struct TableHoldbackScanner {
    source_offset: usize,
    fence_tracker: FenceTracker,
    previous_line: Option<PreviousLineState>,
    pending_header_start: Option<usize>,
    confirmed_table_start: Option<usize>,
}

impl TableHoldbackScanner {
    fn new() -> Self {
        Self {
            source_offset: 0,
            fence_tracker: FenceTracker::new(),
            previous_line: None,
            pending_header_start: None,
            confirmed_table_start: None,
        }
    }

    fn reset(&mut self) {
        *self = Self::new();
    }

    fn state(&self) -> TableHoldbackState {
        if let Some(table_start) = self.confirmed_table_start {
            TableHoldbackState::Confirmed { table_start }
        } else if let Some(header_start) = self.pending_header_start {
            TableHoldbackState::PendingHeader { header_start }
        } else {
            TableHoldbackState::None
        }
    }

    fn push_source_chunk(&mut self, source_chunk: &str) {
        if source_chunk.is_empty() {
            return;
        }

        let scan_start = Instant::now();
        let mut lines = 0usize;
        for source_line in source_chunk.split_inclusive('\n') {
            lines += 1;
            self.push_line(source_line);
        }
        tracing::trace!(
            bytes = source_chunk.len(),
            lines,
            state = ?self.state(),
            elapsed_us = scan_start.elapsed().as_micros(),
            "table holdback incremental scan",
        );
    }

    fn push_line(&mut self, source_line: &str) {
        let line = source_line.strip_suffix('\n').unwrap_or(source_line);
        let source_start = self.source_offset;
        let fence_kind = self.fence_tracker.kind();

        let candidate_text = if fence_kind == FenceKind::Other {
            None
        } else {
            table_candidate_text(line)
        };
        let is_header = candidate_text.is_some_and(is_table_header_line);
        let is_delimiter = candidate_text.is_some_and(is_table_delimiter_line);

        if self.confirmed_table_start.is_none()
            && let Some(previous_line) = self.previous_line
            && previous_line.fence_kind != FenceKind::Other
            && fence_kind != FenceKind::Other
            && previous_line.is_header
            && is_delimiter
        {
            self.confirmed_table_start = Some(previous_line.source_start);
            self.pending_header_start = None;
        }

        if self.confirmed_table_start.is_none() && !line.trim().is_empty() {
            if fence_kind != FenceKind::Other && is_header {
                self.pending_header_start = Some(source_start);
            } else {
                self.pending_header_start = None;
            }
        }

        self.previous_line = Some(PreviousLineState {
            source_start,
            fence_kind,
            is_header,
        });

        self.fence_tracker.advance(line);
        self.source_offset = self.source_offset.saturating_add(source_line.len());
    }
}

/// Scan `source` for pipe-table patterns outside of non-markdown fenced code
/// blocks.
///
/// Walks consecutive line pairs looking for a header + delimiter match. Lines
/// inside `FenceKind::Other` fences are skipped. The scan also peels
/// blockquote prefixes (`>`) before checking for table syntax, so tables nested
/// inside blockquotes are detected.
///
/// If no confirmed table is found, checks whether the last non-blank line looks
/// like a table header (speculative `PendingHeader`) to avoid premature commit
/// of a potential table header before the delimiter arrives.
#[cfg(test)]
fn table_holdback_state(source: &str) -> TableHoldbackState {
    let lines = parse_lines_with_fence_state(source);
    for pair in lines.windows(2) {
        let [header_line, delimiter_line] = pair else {
            continue;
        };
        if header_line.fence_context == FenceKind::Other
            || delimiter_line.fence_context == FenceKind::Other
        {
            continue;
        }

        let Some(header_text) = table_candidate_text(header_line.text) else {
            continue;
        };
        let Some(delimiter_text) = table_candidate_text(delimiter_line.text) else {
            continue;
        };

        if is_table_header_line(header_text) && is_table_delimiter_line(delimiter_text) {
            return TableHoldbackState::Confirmed {
                table_start: header_line.source_start,
            };
        }
    }

    let pending_header = lines.iter().rev().find(|line| !line.text.trim().is_empty());
    if let Some(line) = pending_header
        && line.fence_context != FenceKind::Other
        && table_candidate_text(line.text).is_some_and(is_table_header_line)
    {
        return TableHoldbackState::PendingHeader {
            header_start: line.source_start,
        };
    }
    TableHoldbackState::None
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

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

    fn collect_streamed_lines(deltas: &[&str], width: Option<usize>) -> Vec<String> {
        let mut ctrl = StreamController::new(width);
        let mut lines = Vec::new();
        for d in deltas {
            ctrl.push(d);
            while let (Some(cell), idle) = ctrl.on_commit_tick() {
                lines.extend(cell.transcript_lines(u16::MAX));
                if idle {
                    break;
                }
            }
        }
        if let (Some(cell), _source) = ctrl.finalize() {
            lines.extend(cell.transcript_lines(u16::MAX));
        }
        lines_to_plain_strings(&lines)
            .into_iter()
            .map(|s| s.chars().skip(2).collect::<String>())
            .collect()
    }

    fn collect_plan_streamed_lines(deltas: &[&str], width: Option<usize>) -> Vec<String> {
        let mut ctrl = PlanStreamController::new(width);
        let mut lines = Vec::new();
        for d in deltas {
            ctrl.push(d);
            while let (Some(cell), idle) = ctrl.on_commit_tick() {
                lines.extend(cell.transcript_lines(u16::MAX));
                if idle {
                    break;
                }
            }
        }
        if let (Some(cell), _source) = ctrl.finalize() {
            lines.extend(cell.transcript_lines(u16::MAX));
        }
        lines_to_plain_strings(&lines)
    }

    #[test]
    fn controller_set_width_rebuilds_queued_lines() {
        let mut ctrl = StreamController::new(Some(120));
        let delta = "This is a long line that should wrap into multiple rows when resized.\n";
        assert!(ctrl.push(delta));
        assert_eq!(ctrl.queued_lines(), 1);

        ctrl.set_width(Some(24));
        let (cell, idle) = ctrl.on_commit_tick_batch(usize::MAX);
        let rendered = lines_to_plain_strings(
            &cell
                .expect("expected resized queued lines")
                .transcript_lines(u16::MAX),
        );

        assert!(idle);
        assert!(
            rendered.len() > 1,
            "expected resized content to occupy multiple lines, got {rendered:?}",
        );
    }

    #[test]
    fn controller_set_width_no_duplicate_after_emit() {
        let mut ctrl = StreamController::new(Some(120));
        let line =
            "This is a long line that definitely wraps when the terminal shrinks to 24 columns.\n";
        ctrl.push(line);
        let (cell, _) = ctrl.on_commit_tick_batch(usize::MAX);
        assert!(cell.is_some(), "expected emitted cell");
        assert_eq!(ctrl.queued_lines(), 0);

        ctrl.set_width(Some(24));

        assert_eq!(
            ctrl.queued_lines(),
            0,
            "already-emitted content must not be re-queued after resize",
        );
    }

    #[test]
    fn controller_tick_batch_zero_is_noop() {
        let mut ctrl = StreamController::new(Some(80));
        assert!(ctrl.push("line one\n"));
        assert_eq!(ctrl.queued_lines(), 1);

        let (cell, idle) = ctrl.on_commit_tick_batch(0);
        assert!(cell.is_none(), "batch size 0 should not emit lines");
        assert!(!idle, "batch size 0 should not drain queued lines");
        assert_eq!(
            ctrl.queued_lines(),
            1,
            "queue depth should remain unchanged"
        );
    }

    #[test]
    fn controller_has_live_tail_reflects_tail_presence() {
        let mut ctrl = StreamController::new(Some(80));
        assert!(!ctrl.has_live_tail());

        ctrl.core.rendered_lines = vec![Line::from("tail line")];
        ctrl.core.enqueued_stable_len = 0;
        assert!(ctrl.has_live_tail());

        ctrl.core.enqueued_stable_len = 1;
        assert!(!ctrl.has_live_tail());
    }

    #[test]
    fn plan_controller_has_live_tail_reflects_tail_presence() {
        let mut ctrl = PlanStreamController::new(Some(80));
        assert!(!ctrl.has_live_tail());

        ctrl.core.rendered_lines = vec![Line::from("tail line")];
        ctrl.core.enqueued_stable_len = 0;
        assert!(ctrl.has_live_tail());

        ctrl.core.enqueued_stable_len = 1;
        assert!(!ctrl.has_live_tail());
    }

    #[test]
    fn controller_set_width_partial_drain_no_lost_lines() {
        let mut ctrl = StreamController::new(Some(40));
        ctrl.push("AAAA BBBB CCCC DDDD EEEE FFFF GGGG HHHH IIII JJJJ\n");
        ctrl.push("second line\n");

        let (cell, idle) = ctrl.on_commit_tick();
        assert!(cell.is_some(), "expected 1 emitted line");
        assert!(!idle, "queue should still have lines");
        let remaining_before = ctrl.queued_lines();
        assert!(remaining_before > 0, "should have queued lines left");

        ctrl.set_width(Some(20));

        let (cell, source) = ctrl.finalize();
        let final_lines = cell
            .map(|c| lines_to_plain_strings(&c.transcript_lines(u16::MAX)))
            .unwrap_or_default();

        assert!(
            final_lines.iter().any(|l| l.contains("second line")),
            "un-emitted 'second line' was lost after resize; got: {final_lines:?}",
        );
        assert!(source.is_some(), "expected source from finalize");
    }

    #[test]
    fn controller_set_width_partial_drain_keeps_pending_queue() {
        let mut ctrl = StreamController::new(Some(40));
        ctrl.push("AAAA BBBB CCCC DDDD EEEE FFFF GGGG HHHH IIII JJJJ\n");
        ctrl.push("second line\n");

        let (cell, idle) = ctrl.on_commit_tick();
        assert!(cell.is_some(), "expected 1 emitted line");
        assert!(!idle, "queue should still have lines");
        assert!(ctrl.queued_lines() > 0, "expected pending queued lines");

        ctrl.set_width(Some(20));

        assert!(
            ctrl.queued_lines() > 0,
            "resize must preserve pending queued lines"
        );

        let mut drained = Vec::new();
        for _ in 0..64 {
            let (cell, is_idle) = ctrl.on_commit_tick();
            if let Some(cell) = cell {
                drained.extend(lines_to_plain_strings(&cell.transcript_lines(u16::MAX)));
            }
            if is_idle {
                break;
            }
        }

        assert!(
            drained.iter().any(|l| l.contains("second line")),
            "pending lines should continue draining after resize; got {drained:?}",
        );
    }

    #[test]
    fn controller_set_width_preserves_in_flight_tail() {
        let mut ctrl = StreamController::new(Some(80));
        ctrl.push("tail without newline");
        ctrl.set_width(Some(24));

        let (cell, _source) = ctrl.finalize();
        let rendered = lines_to_plain_strings(
            &cell
                .expect("expected finalized tail")
                .transcript_lines(u16::MAX),
        );

        assert_eq!(rendered, vec!["• tail without newline".to_string()]);
    }

    #[test]
    fn controller_set_width_preserves_table_tail_when_queue_is_empty() {
        let mut ctrl = StreamController::new(Some(80));
        ctrl.push("intro line\n");

        let (_cell, idle) = ctrl.on_commit_tick();
        assert!(idle, "intro line should fully drain");
        assert_eq!(ctrl.queued_lines(), 0, "expected empty queue before table");

        ctrl.push("| A | B |\n");
        assert_eq!(
            ctrl.queued_lines(),
            0,
            "pending table header should remain mutable tail, not queued",
        );
        assert!(ctrl.has_live_tail(), "expected live tail before resize");

        ctrl.set_width(Some(24));

        let tail_after = lines_to_plain_strings(&ctrl.current_tail_lines());
        assert!(
            !tail_after.is_empty(),
            "resize must keep mutable tail when queue is empty",
        );
        let joined = tail_after.join(" ");
        assert!(
            joined.contains('A') && joined.contains('B'),
            "expected table header content to remain in tail after resize: {tail_after:?}",
        );
    }

    #[test]
    fn plan_controller_set_width_preserves_in_flight_tail() {
        let mut ctrl = PlanStreamController::new(Some(80));
        ctrl.push("1. Item without newline");
        ctrl.set_width(Some(24));

        let rendered = lines_to_plain_strings(
            &(ctrl
                .finalize()
                .0
                .expect("expected finalized tail")
                .transcript_lines(u16::MAX)),
        );

        assert!(
            rendered
                .iter()
                .any(|line| line.contains("Item without newline")),
            "expected finalized plan content after resize, got {rendered:?}",
        );
    }

    #[test]
    fn plan_controller_streams_table_header_incrementally() {
        let mut ctrl = PlanStreamController::new(Some(80));
        assert!(ctrl.push("Intro\n"));
        let (_cell, idle) = ctrl.on_commit_tick_batch(usize::MAX);
        assert!(idle, "intro line should fully drain");

        assert!(
            ctrl.push("| Step | Owner |\n"),
            "table header should enqueue incrementally for plan streams",
        );
    }

    #[test]
    fn controller_loose_vs_tight_with_commit_ticks_matches_full() {
        let mut ctrl = StreamController::new(None);
        let mut lines = Vec::new();

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

        for d in deltas.iter() {
            ctrl.push(d);
            while let (Some(cell), idle) = ctrl.on_commit_tick() {
                lines.extend(cell.transcript_lines(u16::MAX));
                if idle {
                    break;
                }
            }
        }
        if let (Some(cell), _source) = ctrl.finalize() {
            lines.extend(cell.transcript_lines(u16::MAX));
        }

        let streamed: Vec<_> = lines_to_plain_strings(&lines)
            .into_iter()
            .map(|s| s.chars().skip(2).collect::<String>())
            .collect();

        let source: String = deltas.iter().copied().collect();
        let mut rendered: Vec<ratatui::text::Line<'static>> = Vec::new();
        crate::markdown::append_markdown_agent(&source, None, &mut rendered);
        let rendered_strs = lines_to_plain_strings(&rendered);

        assert_eq!(streamed, rendered_strs);

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

    #[test]
    fn controller_streamed_table_matches_full_render_widths() {
        let deltas = vec![
            "| Key | Description |\n",
            "| --- | --- |\n",
            "| -v | Enable very verbose logging output for debugging |\n",
            "\n",
        ];

        let streamed = collect_streamed_lines(&deltas, Some(80));

        let source: String = deltas.iter().copied().collect();
        let mut rendered = Vec::new();
        crate::markdown::append_markdown_agent(&source, Some(80), &mut rendered);
        let expected = lines_to_plain_strings(&rendered);

        assert_eq!(streamed, expected);
    }

    #[test]
    fn controller_holds_blockquoted_table_tail_until_stable() {
        let deltas = vec![
            "> | A | B |\n",
            "> | --- | --- |\n",
            "> | longvalue | ok |\n",
            "\n",
        ];

        let streamed = collect_streamed_lines(&deltas, Some(80));

        let source: String = deltas.iter().copied().collect();
        let mut rendered = Vec::new();
        crate::markdown::append_markdown_agent(&source, Some(80), &mut rendered);
        let expected = lines_to_plain_strings(&rendered);

        assert_eq!(streamed, expected);
    }

    #[test]
    fn controller_keeps_pre_table_lines_queued_when_table_is_confirmed() {
        let mut ctrl = StreamController::new(Some(80));

        ctrl.push("Intro line before table.\n");
        assert_eq!(ctrl.queued_lines(), 1);

        ctrl.push("| Key | Value |\n");
        ctrl.push("| --- | --- |\n");
        assert_eq!(
            ctrl.queued_lines(),
            1,
            "pre-table line should remain queued after table confirmation",
        );

        let (cell, idle) = ctrl.on_commit_tick();
        let committed = cell
            .map(|cell| lines_to_plain_strings(&cell.transcript_lines(u16::MAX)))
            .unwrap_or_default();
        assert!(
            committed
                .iter()
                .any(|line| line.contains("Intro line before table.")),
            "expected pre-table line to commit independently: {committed:?}",
        );
        assert!(idle, "only pre-table content should have been queued");
    }

    #[test]
    fn controller_set_width_during_confirmed_table_stream_matches_finalize_render() {
        let mut ctrl = StreamController::new(Some(120));
        let deltas = [
            "| Key | Description |\n",
            "| --- | --- |\n",
            "| one | value that should wrap after resize |\n",
        ];
        for delta in deltas {
            ctrl.push(delta);
        }
        assert_eq!(
            ctrl.queued_lines(),
            0,
            "confirmed table should remain mutable"
        );

        ctrl.set_width(Some(32));

        let (cell, source) = ctrl.finalize();
        let source = source.expect("expected finalized source");
        let streamed = lines_to_plain_strings(
            &cell
                .expect("expected finalized table")
                .transcript_lines(u16::MAX),
        )
        .into_iter()
        .map(|line| line.chars().skip(2).collect::<String>())
        .collect::<Vec<_>>();

        let mut rendered = Vec::new();
        crate::markdown::append_markdown_agent(&source, Some(32), &mut rendered);
        let expected = lines_to_plain_strings(&rendered);
        assert_eq!(streamed, expected);
    }

    #[test]
    fn controller_does_not_hold_back_pipe_prose_without_table_delimiter() {
        let mut ctrl = StreamController::new(Some(80));

        ctrl.push("status | owner | note\n");
        let (_first_commit, first_idle) = ctrl.on_commit_tick();
        assert!(first_idle);

        ctrl.push("next line\n");
        let (second_commit, _second_idle) = ctrl.on_commit_tick();
        assert!(
            second_commit.is_some(),
            "expected prose lines to be released once no table delimiter follows"
        );
    }

    #[test]
    fn controller_does_not_stall_repeated_pipe_prose_paragraphs() {
        let mut ctrl = StreamController::new(Some(80));

        ctrl.push("alpha | beta\n\n");
        let (_first_commit, first_idle) = ctrl.on_commit_tick();
        assert!(first_idle);

        ctrl.push("gamma | delta\n\n");
        let (second_commit, _second_idle) = ctrl.on_commit_tick();
        let second_lines = second_commit
            .map(|cell| lines_to_plain_strings(&cell.transcript_lines(u16::MAX)))
            .unwrap_or_default();

        assert!(
            second_lines
                .iter()
                .any(|line| line.contains("alpha | beta")),
            "expected the first pipe-prose paragraph to stream before finalize; got {second_lines:?}",
        );
    }

    #[test]
    fn controller_handles_table_immediately_after_heading() {
        let deltas = vec![
            "### 1) Basic table\n",
            "| Name | Role | Status |\n",
            "|---|---|---|\n",
            "| Alice | Admin | Active |\n",
            "| Bob | Editor | Pending |\n",
            "\n",
        ];

        let streamed = collect_streamed_lines(&deltas, Some(100));

        let source: String = deltas.iter().copied().collect();
        let mut rendered = Vec::new();
        crate::markdown::append_markdown_agent(&source, Some(100), &mut rendered);
        let expected = lines_to_plain_strings(&rendered);

        assert_eq!(streamed, expected);
    }

    #[test]
    fn controller_renders_unicode_for_multi_table_response_shape() {
        let source = "Absolutely. Here are several different Markdown table patterns you can use for rendering tests.\n\n| Name  | Role      |
  Location |\n|-------|-----------|----------|\n| Ava   | Engineer  | NYC      |\n| Malik | Designer  | Berlin   |\n| Priya | PM        | Remote
  |\n\n| Item        | Qty | Price | In Stock |\n|:------------|----:|------:|:--------:|\n| Keyboard    |   2 | 49.99 |    Yes   |\n| Mouse       |  10
   | 19.50 |    Yes   |\n| Monitor     |   1 | 219.0 |    No    |\n\n| Field         | Example                         | Notes
  |\n|---------------|----------------------------------|--------------------------|\n| Escaped pipe  | `foo \\| bar`                    | Should stay
  in one cell  |\n| Inline code   | `let x = value;`                | Monospace inline content |\n| Link          | [OpenAI](https://openai.com)    |
  Standard markdown link   |\n";

        let chunked = source
            .split_inclusive('\n')
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        let deltas = chunked.iter().map(String::as_str).collect::<Vec<_>>();
        let streamed = collect_streamed_lines(&deltas, Some(120));
        assert!(
            streamed.iter().any(|line| line.contains('┌')),
            "expected unicode table border in streamed output: {streamed:?}"
        );
    }

    #[test]
    fn controller_renders_unicode_for_no_outer_pipes_table_shape() {
        let source = "### 1) Basic\n\n| Name | Role | Active |\n|---|---|---|\n| Alice | Engineer | Yes |\n| Bob | Designer | No |\n\n### 2) No outer
  pipes\n\nCol A | Col B | Col C\n--- | --- | ---\nx | y | z\n10 | 20 | 30\n\n### 3) Another table\n\n| Key | Value |\n|---|---|\n| a | b |\n";

        let chunked = source
            .split_inclusive('\n')
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        let deltas = chunked.iter().map(String::as_str).collect::<Vec<_>>();
        let streamed = collect_streamed_lines(&deltas, Some(100));

        let mut rendered = Vec::new();
        crate::markdown::append_markdown_agent(source, Some(100), &mut rendered);
        let expected = lines_to_plain_strings(&rendered);

        assert_eq!(streamed, expected);
        let has_raw_no_outer_header = streamed
            .iter()
            .any(|line| line.trim() == "Col A | Col B | Col C");
        assert!(
            !has_raw_no_outer_header,
            "no-outer-pipes header should not remain raw in final streamed output: {streamed:?}"
        );
    }

    #[test]
    fn controller_stabilizes_first_no_outer_pipes_table_in_response() {
        let deltas = vec![
            "### No outer pipes first\n\n",
            "Col A | Col B | Col C\n",
            "--- | --- | ---\n",
            "x | y | z\n",
            "10 | 20 | 30\n",
            "\n",
            "After table paragraph.\n",
        ];
        let streamed = collect_streamed_lines(&deltas, Some(100));

        let source: String = deltas.iter().copied().collect();
        let mut rendered = Vec::new();
        crate::markdown::append_markdown_agent(&source, Some(100), &mut rendered);
        let expected = lines_to_plain_strings(&rendered);

        assert_eq!(streamed, expected);
        assert!(
            streamed.iter().any(|line| line.contains('┌')),
            "expected unicode table border for no-outer-pipes streaming: {streamed:?}"
        );
        assert!(
            !streamed
                .iter()
                .any(|line| line.trim() == "Col A | Col B | Col C"),
            "did not expect raw no-outer-pipes header in final streamed output: {streamed:?}"
        );
    }

    #[test]
    fn controller_stabilizes_two_column_no_outer_table_in_response() {
        let deltas = vec![
            "A | B\n",
            "--- | ---\n",
            "left | right\n",
            "\n",
            "After table paragraph.\n",
        ];
        let streamed = collect_streamed_lines(&deltas, Some(80));

        let source: String = deltas.iter().copied().collect();
        let mut rendered = Vec::new();
        crate::markdown::append_markdown_agent(&source, Some(80), &mut rendered);
        let expected = lines_to_plain_strings(&rendered);

        assert_eq!(streamed, expected);
        assert!(
            streamed.iter().any(|line| line.contains('┌')),
            "expected unicode table border for two-column no-outer table: {streamed:?}"
        );
        assert!(
            !streamed.iter().any(|line| line.trim() == "A | B"),
            "did not expect raw two-column no-outer header in final streamed output: {streamed:?}"
        );
    }

    #[test]
    fn controller_converts_no_outer_table_between_preboxed_sections() {
        let source = "  ┌───────┬──────────┬────────┐\n  │ Name  │ Role     │ Active │\n  ├───────┼──────────┼────────┤\n  │ Alice │ Engineer │ Yes    │\n  │ Bob   │ Designer │ No     │\n  │ Cara  │ PM       │ Yes    │\n  └───────┴──────────┴────────┘\n\n  ### 3) No outer pipes\n\n  Col A | Col B | Col C\n  --- | --- | ---\n  x | y | z\n  10 | 20 | 30\n\n  ┌─────────────────┬────────┬────────────────────────┐\n  │ Example         │ Output │ Notes                  │\n  ├─────────────────┼────────┼────────────────────────┤\n  │ a | b           │ `a     │ b`                     │\n  │ npm run test    │ ok     │ Inline code formatting │\n  │ SELECT * FROM t │ 3 rows │ SQL snippet            │\n  └─────────────────┴────────┴────────────────────────┘\n";

        let deltas = source
            .split_inclusive('\n')
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        let streamed = collect_streamed_lines(
            &deltas.iter().map(String::as_str).collect::<Vec<_>>(),
            Some(100),
        );

        let has_raw_no_outer_header = streamed
            .iter()
            .any(|line| line.trim() == "Col A | Col B | Col C");
        assert!(
            !has_raw_no_outer_header,
            "no-outer table header remained raw in streamed output: {streamed:?}"
        );
        assert!(
            streamed
                .iter()
                .any(|line| line.contains("┌───────┬───────┬───────┐")),
            "expected converted no-outer table border in streamed output: {streamed:?}"
        );
    }

    #[test]
    fn controller_keeps_markdown_fenced_tables_mutable_until_finalize() {
        let source = "```md\n| A | B |\n|---|---|\n| 1 | 2 |\n```\n";
        let deltas = vec![
            "```md\n",
            "| A | B |\n",
            "|---|---|\n",
            "| 1 | 2 |\n",
            "```\n",
        ];
        let streamed = collect_streamed_lines(&deltas, Some(80));

        let mut rendered = Vec::new();
        crate::markdown::append_markdown_agent(source, Some(80), &mut rendered);
        let expected = lines_to_plain_strings(&rendered);

        assert_eq!(streamed, expected);
        assert!(
            streamed.iter().any(|line| line.contains('┌')),
            "expected unicode table border in streamed output: {streamed:?}"
        );
        assert!(
            !streamed.iter().any(|line| line.trim() == "| A | B |"),
            "did not expect raw table header line after finalize: {streamed:?}"
        );
    }

    #[test]
    fn controller_keeps_markdown_fenced_no_outer_tables_mutable_until_finalize() {
        let source =
            "```md\nCol A | Col B | Col C\n--- | --- | ---\nx | y | z\n10 | 20 | 30\n```\n";
        let deltas = vec![
            "```md\n",
            "Col A | Col B | Col C\n",
            "--- | --- | ---\n",
            "x | y | z\n",
            "10 | 20 | 30\n",
            "```\n",
        ];
        let streamed = collect_streamed_lines(&deltas, Some(100));

        let mut rendered = Vec::new();
        crate::markdown::append_markdown_agent(source, Some(100), &mut rendered);
        let expected = lines_to_plain_strings(&rendered);

        assert_eq!(streamed, expected);
        assert!(
            streamed.iter().any(|line| line.contains('┌')),
            "expected unicode table border in streamed output: {streamed:?}"
        );
        assert!(
            !streamed
                .iter()
                .any(|line| line.trim() == "Col A | Col B | Col C"),
            "did not expect raw no-outer-pipes header line after finalize: {streamed:?}"
        );
    }

    #[test]
    fn controller_live_view_matches_render_during_interleaved_table_streaming() {
        let source = "Project updates are easier to scan when narrative and structured data alternate.\n\n| Focus Area | Owner | Priority | Status |\n|---|---|---|---|\n| Authentication cleanup | Maya | High | 80% |\n| CLI error messages | Jordan | Medium | 55% |\n| Docs refresh | Lee | Low | 30% |\n\nThe first checkpoint shows progress, but we still have open risks.\n\n| Task | Command / Artifact | Due | State |\n|---|---|---|---|\n| Run unit tests | `cargo test -p codex-core` | Today | ✅ |\n| Snapshot review | `cargo insta pending-snapshots -p codex-tui` | Today | ⏳ |\n| Changelog draft | Release template (https://replacechangelog.com/) | Tomorrow | 📝 |\n\nFinal sign-off criteria are summarized below.\n";
        let width = Some(72usize);
        let mut ctrl = StreamController::new(width);
        let mut emitted_lines: Vec<Line<'static>> = Vec::new();

        for delta in source.split_inclusive('\n') {
            ctrl.push(delta);
            loop {
                let (cell, idle) = ctrl.on_commit_tick();
                if let Some(cell) = cell {
                    emitted_lines.extend(cell.transcript_lines(u16::MAX).into_iter().map(|line| {
                        let plain: String = line
                            .spans
                            .iter()
                            .map(|s| s.content.clone())
                            .collect::<Vec<_>>()
                            .join("");
                        Line::from(plain.chars().skip(2).collect::<String>())
                    }));
                }
                if idle {
                    break;
                }
            }

            let mut visible = emitted_lines.clone();
            visible.extend(ctrl.current_tail_lines());
            let visible_plain = lines_to_plain_strings(&visible);

            let mut expected = Vec::new();
            crate::markdown::append_markdown_agent(&ctrl.core.raw_source, width, &mut expected);
            let expected_plain = lines_to_plain_strings(&expected);

            assert_eq!(
                visible_plain, expected_plain,
                "live view diverged after delta: {delta:?}"
            );
        }
    }

    #[test]
    fn controller_keeps_non_markdown_fenced_tables_as_code() {
        let source = "```sh\n| A | B |\n|---|---|\n| 1 | 2 |\n```\n";
        let deltas = vec![
            "```sh\n",
            "| A | B |\n",
            "|---|---|\n",
            "| 1 | 2 |\n",
            "```\n",
        ];
        let streamed = collect_streamed_lines(&deltas, Some(80));

        let mut rendered = Vec::new();
        crate::markdown::append_markdown_agent(source, Some(80), &mut rendered);
        let expected = lines_to_plain_strings(&rendered);

        assert_eq!(streamed, expected);
        assert!(
            streamed.iter().any(|line| line.trim() == "| A | B |"),
            "expected code-fenced pipe line to remain raw: {streamed:?}"
        );
        assert!(
            !streamed.iter().any(|line| line.contains('┌')),
            "did not expect unicode table border for non-markdown fence: {streamed:?}"
        );
    }

    #[test]
    fn plan_controller_streamed_table_emits_incremental_rows_before_finalize() {
        let deltas = vec![
            "## Build plan\n\n",
            "| Step | Owner |\n",
            "|---|---|\n",
            "| Write tests | Agent |\n",
            "| Verify output | User |\n",
            "\n",
        ];
        let streamed = collect_plan_streamed_lines(&deltas, Some(80));

        let source: String = deltas.iter().copied().collect();
        let baseline = collect_plan_streamed_lines(&[source.as_str()], Some(80));

        assert_ne!(streamed, baseline);
        assert!(
            streamed
                .iter()
                .any(|line| line.contains('│') || line.contains('└') || line.contains('┌')),
            "expected unicode table box drawing chars in plan streamed output: {streamed:?}"
        );
        assert!(
            streamed
                .iter()
                .any(|line| line.trim() == "| Step | Owner |"),
            "expected incremental raw table header line in plan output: {streamed:?}"
        );
    }

    #[test]
    fn plan_controller_streamed_markdown_fenced_table_emits_incremental_rows_before_finalize() {
        let deltas = vec![
            "## Build plan\n\n",
            "```md\n",
            "| Step | Owner |\n",
            "|---|---|\n",
            "| Write tests | Agent |\n",
            "| Verify output | User |\n",
            "```\n",
            "\n",
        ];
        let streamed = collect_plan_streamed_lines(&deltas, Some(80));

        let source: String = deltas.iter().copied().collect();
        let baseline = collect_plan_streamed_lines(&[source.as_str()], Some(80));

        assert_ne!(streamed, baseline);
        assert!(
            streamed
                .iter()
                .any(|line| line.contains('│') || line.contains('└') || line.contains('┌')),
            "expected unicode table box drawing chars in fenced plan output: {streamed:?}"
        );
        assert!(
            streamed
                .iter()
                .any(|line| line.trim() == "| Step | Owner |"),
            "expected incremental raw table header line in fenced plan output: {streamed:?}"
        );
    }

    #[test]
    fn table_holdback_state_detects_header_plus_delimiter() {
        let source = "| Key | Description |\n| --- | --- |\n";
        assert!(matches!(
            table_holdback_state(source),
            TableHoldbackState::Confirmed { .. }
        ));
    }

    #[test]
    fn table_holdback_state_detects_single_column_header_plus_delimiter() {
        let source = "| Only |\n| --- |\n";
        assert!(matches!(
            table_holdback_state(source),
            TableHoldbackState::Confirmed { .. }
        ));
    }

    #[test]
    fn table_holdback_state_ignores_table_like_lines_inside_unclosed_long_fence() {
        let source = "````sh\n```cmd\n| Key | Description |\n| --- | --- |\n````\n";
        assert!(
            matches!(table_holdback_state(source), TableHoldbackState::None),
            "table holdback should ignore pipe lines inside an open non-markdown fence",
        );
    }

    #[test]
    fn table_holdback_state_treats_indented_fence_text_as_plain_content() {
        let source = "    ```sh\n| Key | Description |\n| --- | --- |\n";
        assert!(
            matches!(
                table_holdback_state(source),
                TableHoldbackState::Confirmed { .. }
            ),
            "indented fence-like text should not open a fence and should not block table detection",
        );
    }

    #[test]
    fn table_holdback_state_ignores_table_like_lines_inside_blockquoted_other_fence() {
        let source = "> ```sh\n> | Key | Value |\n> | --- | --- |\n> ```\n";
        assert!(
            matches!(table_holdback_state(source), TableHoldbackState::None),
            "table holdback should ignore pipe lines inside non-markdown blockquoted fences",
        );
    }

    #[test]
    fn incremental_holdback_matches_stateless_scan_per_chunk() {
        let chunks = [
            "status | owner\n",
            "\n",
            "> ```sh\n",
            "> | A | B |\n",
            "> | --- | --- |\n",
            "> ```\n",
            "> | Key | Value |\n",
            "> | --- | --- |\n",
        ];

        let mut scanner = TableHoldbackScanner::new();
        let mut source = String::new();
        for chunk in chunks {
            source.push_str(chunk);
            scanner.push_source_chunk(chunk);
            assert_eq!(
                scanner.state(),
                table_holdback_state(&source),
                "scanner mismatch after chunk: {chunk:?}\nsource:\n{source}",
            );
        }
    }

    #[test]
    fn incremental_holdback_detects_header_delimiter_across_chunk_boundary() {
        let mut scanner = TableHoldbackScanner::new();
        scanner.push_source_chunk("| A | B |\n");
        assert_eq!(
            scanner.state(),
            TableHoldbackState::PendingHeader { header_start: 0 }
        );
        scanner.push_source_chunk("| --- | --- |\n");
        assert_eq!(
            scanner.state(),
            TableHoldbackState::Confirmed { table_start: 0 }
        );
    }

    #[test]
    fn controller_set_width_after_first_line_emit_does_not_requeue_first_line() {
        let mut ctrl = StreamController::new(Some(120));
        ctrl.push(
            "FIRSTTOKEN contains enough words to wrap once the width is reduced dramatically.\n",
        );
        ctrl.push("second line remains pending\n");

        let (first_emit, _) = ctrl.on_commit_tick();
        assert!(first_emit.is_some(), "expected first line emission");

        ctrl.set_width(Some(20));

        let (cell, _source) = ctrl.finalize();
        let remaining = cell
            .map(|cell| lines_to_plain_strings(&cell.transcript_lines(u16::MAX)))
            .unwrap_or_default()
            .into_iter()
            .map(|line| line.chars().skip(2).collect::<String>())
            .collect::<Vec<_>>();
        assert!(
            !remaining.iter().any(|line| line.contains("FIRSTTOKEN")),
            "first line should not be re-queued after resize: {remaining:?}",
        );
        assert!(
            remaining.iter().any(|line| line.contains("second line")),
            "expected pending second line after resize: {remaining:?}",
        );
    }

    #[test]
    fn controller_set_width_partial_wrapped_emit_preserves_remaining_content() {
        let mut ctrl = StreamController::new(Some(20));
        ctrl.push("The quick brown fox jumps over the lazy dog near the riverbank.\n");
        ctrl.push("tail line\n");

        let (first_emit, idle) = ctrl.on_commit_tick();
        assert!(first_emit.is_some(), "expected first wrapped line emission");
        assert!(!idle, "expected remaining queued content after one tick");
        assert!(
            ctrl.queued_lines() > 0,
            "expected non-empty queue before resize"
        );

        ctrl.set_width(Some(120));

        let (cell, _source) = ctrl.finalize();
        let remaining = cell
            .map(|c| lines_to_plain_strings(&c.transcript_lines(u16::MAX)))
            .unwrap_or_default()
            .into_iter()
            .map(|line| line.chars().skip(2).collect::<String>())
            .collect::<Vec<_>>();
        assert!(
            remaining.iter().any(|line| line.contains("tail line")),
            "un-emitted content should remain after resize remap: {remaining:?}",
        );
    }

    #[test]
    fn controller_set_width_partial_wrapped_emit_keeps_wrapped_remainder() {
        let mut ctrl = StreamController::new(Some(18));
        ctrl.push("alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu\n");

        let (first_emit, idle) = ctrl.on_commit_tick();
        assert!(first_emit.is_some(), "expected first wrapped line emission");
        assert!(!idle, "expected remaining wrapped content after one tick");
        assert!(
            ctrl.queued_lines() > 0,
            "expected queued wrapped remainder before resize"
        );

        ctrl.set_width(Some(80));

        let (cell, _source) = ctrl.finalize();
        let remaining = cell
            .map(|c| lines_to_plain_strings(&c.transcript_lines(u16::MAX)))
            .unwrap_or_default();
        let joined = remaining.join(" ");
        assert!(
            joined.contains("kappa") || joined.contains("lambda") || joined.contains("mu"),
            "wrapped remainder from partially emitted source line was lost after resize: {remaining:?}",
        );
    }
}
