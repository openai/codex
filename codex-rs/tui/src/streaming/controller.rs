//! Two-region streaming controllers for agent messages and proposed plans.
//!
//! Each stream partitions rendered markdown into a *stable region* (committed
//! to scrollback via the animation queue in `StreamState`) and a *tail region*
//! (mutable, displayed in the active-cell slot as a transient stream-tail cell).
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
//! agent and proposed-plan streams. Lines in `Outside` and `Markdown` fence
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
use crate::history_cell::HistoryRenderMode;
use crate::history_cell::raw_lines_from_source;
use crate::history_cell::{self};
use crate::markdown::render_markdown_agent_with_links_and_cwd;
use crate::style::proposed_plan_style;
use crate::terminal_hyperlinks::HyperlinkLine;
use crate::terminal_hyperlinks::TerminalHyperlink;
use crate::terminal_hyperlinks::plain_hyperlink_lines;
use crate::terminal_hyperlinks::prefix_hyperlink_lines;
use ratatui::prelude::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use std::time::Instant;
use unicode_width::UnicodeWidthStr;

use super::StreamState;
use super::table_holdback::TableHoldbackScanner;
use super::table_holdback::TableHoldbackState;
#[cfg(test)]
use super::table_holdback::table_holdback_state;

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
    /// Current rendering width (columns available for markdown content).
    width: Option<usize>,
    /// Accumulated raw markdown source for the current stream.
    raw_source: String,
    /// Full re-render of `raw_source` at `width`. Rebuilt on every committed delta.
    rendered_lines: Vec<HyperlinkLine>,
    /// Lines enqueued into the commit-animation queue.
    enqueued_stable_len: usize,
    /// Lines actually emitted to scrollback.
    emitted_stable_len: usize,
    /// Session cwd used to keep local file-link display stable during stream re-renders.
    cwd: PathBuf,
    render_mode: HistoryRenderMode,
    /// Cached rendered line count for prefix-before-table keyed by source start and width.
    stable_prefix_len_cache: Option<StablePrefixLenCache>,
    /// Incremental holdback scanner state for append-only source updates.
    holdback_scanner: TableHoldbackScanner,
    /// Number of queued lines that preserve an old render's un-emitted suffix.
    synthetic_queue_remaining: usize,
    /// Rendered-line index to use once the synthetic suffix has drained.
    synthetic_queue_target_emitted_len: Option<usize>,
    /// Source-line prefix already visible when a synthetic queue is rebuilt.
    synthetic_queue_partial: Option<PartialSourceLine>,
    /// Number of source lines to keep mutable even when no structural holdback is active.
    ///
    /// CommonMark can reinterpret the newest block after the next line arrives. For example, a
    /// paragraph after a fenced code block inside a list can shift when the following blank/list
    /// marker is parsed. Holding a small source suffix keeps that boundary out of scrollback until
    /// it is stable.
    default_tail_lines: usize,
}

struct StablePrefixLenCache {
    /// Byte offset of the candidate table/header start in `raw_source`.
    source_start: usize,
    /// Width that produced `stable_prefix_len`.
    width: Option<usize>,
    /// Rendered line count for `raw_source[..source_start]` at `width`.
    ///
    /// The streaming controller uses this to avoid repeatedly re-rendering the
    /// same stable prefix while a live table tail is still mutating.
    stable_prefix_len: usize,
}

#[derive(Clone)]
struct PartialSourceLine {
    source_start: usize,
    source_end: usize,
    emitted_source_end: usize,
    emitted_prefix: Vec<HyperlinkLine>,
}

impl StreamCore {
    fn new(
        width: Option<usize>,
        cwd: &Path,
        render_mode: HistoryRenderMode,
        default_tail_lines: usize,
    ) -> Self {
        Self {
            state: StreamState::new(width, cwd),
            width,
            raw_source: String::with_capacity(1024),
            rendered_lines: Vec::with_capacity(64),
            enqueued_stable_len: 0,
            emitted_stable_len: 0,
            cwd: cwd.to_path_buf(),
            render_mode,
            stable_prefix_len_cache: None,
            holdback_scanner: TableHoldbackScanner::new(),
            synthetic_queue_remaining: 0,
            synthetic_queue_target_emitted_len: None,
            synthetic_queue_partial: None,
            default_tail_lines,
        }
    }

    /// Push a streaming delta and enqueue any newly-stable rendered lines.
    ///
    /// Only newline-terminated source is committed into `raw_source`. This is
    /// important for tables because an unterminated partial row must stay out
    /// of both the stable queue and the live tail until its structure is
    /// unambiguous; otherwise the user can briefly see malformed columns that
    /// immediately disappear on the next delta.
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
            self.holdback_scanner.push_source_chunk(&committed_source);
            self.recompute_streaming_render();
            enqueued = self.sync_stable_queue();
        }
        enqueued
    }

    /// Drain the collector, render the final source snapshot, and return lines not yet emitted.
    ///
    /// This intentionally re-renders from the full raw source instead of
    /// trying to stitch together queued stable lines and the current tail. The
    /// final render is the canonical transcript representation used for
    /// consolidation, so callers that skip `reset()` can accidentally replay a
    /// finished stream into the next answer.
    fn finalize_remaining(&mut self) -> Vec<HyperlinkLine> {
        let mut remaining = self.state.drain_n(/*max_lines*/ usize::MAX);
        self.account_emitted_step(&remaining);
        let remainder_source = self.state.collector.finalize_and_drain_source();
        if !remainder_source.is_empty() {
            self.raw_source.push_str(&remainder_source);
            self.holdback_scanner.push_source_chunk(&remainder_source);
        }
        let rendered = self.render_source(&self.raw_source);
        self.rendered_lines = rendered;
        if let Some(partial_line) = self.synthetic_queue_partial.take() {
            let source_line_start_len =
                self.rendered_len_for_source_boundary(partial_line.source_start);
            let source_line_end_len =
                self.rendered_len_for_source_boundary(partial_line.source_end);
            remaining.extend(trim_rendered_prefix(
                self.rendered_lines[source_line_start_len..source_line_end_len].to_vec(),
                &partial_line.emitted_prefix,
            ));
            if source_line_end_len < self.rendered_lines.len() {
                remaining.extend(self.rendered_lines[source_line_end_len..].to_vec());
            }
        } else if self.emitted_stable_len < self.rendered_lines.len() {
            remaining.extend(self.rendered_lines[self.emitted_stable_len..].to_vec());
        }
        remaining
    }

    /// Step animation: dequeue one line, update the emitted count.
    fn tick(&mut self) -> Vec<HyperlinkLine> {
        let step = self.state.step();
        self.account_emitted_step(&step);
        step
    }

    /// Batch drain: dequeue up to `max_lines`, update the emitted count.
    fn tick_batch(&mut self, max_lines: usize) -> Vec<HyperlinkLine> {
        if max_lines == 0 {
            return Vec::new();
        }
        let step = self.state.drain_n(max_lines);
        if step.is_empty() {
            return step;
        }
        self.account_emitted_step(&step);
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
    /// The tail starts at `enqueued_stable_len`, so this returns the portion
    /// of the current render snapshot that is still allowed to change without
    /// violating scrollback ordering. If callers were to derive the tail from
    /// `emitted_stable_len` instead, queued-but-not-yet-emitted lines could
    /// reappear in the active cell and duplicate content on screen.
    #[inline]
    fn current_tail_lines(&self) -> Vec<HyperlinkLine> {
        let start = self.enqueued_stable_len.min(self.rendered_lines.len());
        self.rendered_lines[start..].to_vec()
    }

    #[cfg(test)]
    #[inline]
    fn has_tail(&self) -> bool {
        self.enqueued_stable_len < self.rendered_lines.len()
    }

    /// Update rendering width and rebuild queued stable lines for the new layout.
    ///
    /// Re-renders once at the new width and rebuilds queue state from the
    /// current emitted line count.
    ///
    /// Resize is the point where source-backed rendering matters most:
    /// previously emitted prose must stay in scrollback order, while any live
    /// table tail is free to reshape at the new width. This method preserves
    /// that split without attempting byte-for-byte line remapping.
    fn set_width(&mut self, width: Option<usize>) {
        if self.width == width {
            return;
        }
        let had_pending_queue = self.state.queued_len() > 0;
        let had_structural_tail = self.requires_final_scrollback_reflow();
        let partial_line = if had_pending_queue {
            self.synthetic_queue_partial
                .clone()
                .or_else(|| self.partial_source_line_after_rendered_len(self.emitted_stable_len))
        } else {
            None
        };
        let emitted_source_boundary =
            self.source_boundary_after_rendered_len(self.emitted_stable_len);
        self.width = width;
        self.state.collector.set_width(width);
        if self.raw_source.is_empty() {
            return;
        }

        self.recompute_streaming_render();
        if let Some(partial_line) = partial_line {
            self.rebuild_stable_queue_preserving_partial_source_line(partial_line);
            return;
        }
        self.emitted_stable_len = self.rendered_len_for_source_boundary(emitted_source_boundary);
        let target_stable_len = self.compute_target_stable_len();
        if had_pending_queue
            && self.emitted_stable_len >= target_stable_len
            && target_stable_len > 0
        {
            self.emitted_stable_len = target_stable_len;
        }
        self.clear_stable_queue();
        if self.emitted_stable_len > 0 && !had_pending_queue && !had_structural_tail {
            // Avoid replaying already-emitted content after resize when no
            // stable lines were waiting in the queue and there was no mutable
            // structural tail to preserve. Use the unfloored stable boundary so
            // ordinary source-line tails are remapped to the new render suffix
            // and stay mutable even if the stable prefix compressed.
            let target_stable_len = self.stable_len_before_emitted_floor();
            self.emitted_stable_len = target_stable_len;
            self.enqueued_stable_len = target_stable_len;
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
        self.clear_synthetic_queue_accounting();
    }

    fn render_source(&self, source: &str) -> Vec<HyperlinkLine> {
        match self.render_mode {
            HistoryRenderMode::Rich => render_markdown_agent_with_links_and_cwd(
                source,
                self.width,
                Some(self.cwd.as_path()),
            ),
            HistoryRenderMode::Raw => plain_hyperlink_lines(raw_lines_from_source(source)),
        }
    }

    fn recompute_streaming_render(&mut self) {
        self.rendered_lines = self.render_source(&self.raw_source);
    }

    fn set_render_mode(&mut self, render_mode: HistoryRenderMode) -> bool {
        if self.render_mode == render_mode {
            return false;
        }

        let had_pending_queue = self.state.queued_len() > 0;
        let had_structural_tail = self.requires_final_scrollback_reflow();
        let partial_line = if had_pending_queue {
            self.synthetic_queue_partial
                .clone()
                .or_else(|| self.partial_source_line_after_rendered_len(self.emitted_stable_len))
        } else {
            None
        };
        let live_default_tail_source_start = if !had_pending_queue && !had_structural_tail {
            self.live_default_tail_source_start()
        } else {
            None
        };
        let emitted_source_boundary =
            self.source_boundary_after_rendered_len(self.emitted_stable_len);
        self.render_mode = render_mode;
        self.stable_prefix_len_cache = None;
        if self.raw_source.is_empty() {
            return false;
        }

        self.recompute_streaming_render();
        if let Some(partial_line) = partial_line {
            self.rebuild_stable_queue_preserving_partial_source_line(partial_line);
            return true;
        }
        self.emitted_stable_len = self.rendered_len_for_source_boundary(emitted_source_boundary);
        let target_stable_len = self.compute_target_stable_len();
        if had_pending_queue
            && self.emitted_stable_len >= target_stable_len
            && target_stable_len > 0
        {
            self.emitted_stable_len = target_stable_len;
        }
        self.clear_stable_queue();
        if self.emitted_stable_len > 0 && !had_pending_queue && !had_structural_tail {
            let target_stable_len = if let Some(source_start) = live_default_tail_source_start {
                self.stable_prefix_len_for_source_start(source_start)
            } else {
                self.compute_target_stable_len()
            };
            self.emitted_stable_len = target_stable_len;
            self.enqueued_stable_len = target_stable_len;
            return true;
        }
        self.rebuild_stable_queue_from_render();
        true
    }

    /// Compute how many rendered lines should be in the stable region.
    fn compute_target_stable_len(&mut self) -> usize {
        self.stable_len_before_emitted_floor()
            .max(self.emitted_stable_len)
    }

    fn stable_len_before_emitted_floor(&mut self) -> usize {
        let tail_budget = self.active_tail_budget_lines();
        self.rendered_lines.len().saturating_sub(tail_budget)
    }

    /// Advance `enqueued_stable_len` toward the target stable boundary and enqueue any
    /// newly-stable lines. Returns `true` if new lines were enqueued.
    fn sync_stable_queue(&mut self) -> bool {
        let target_stable_len = self.compute_target_stable_len();

        // A structural rewrite moved the stable boundary backward into enqueue-but-unemitted
        // lines. Rebuild queue from the latest snapshot.
        if target_stable_len < self.enqueued_stable_len {
            self.clear_stable_queue();
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

    /// Rebuild the stable queue from the current render snapshot.
    ///
    /// This is used after `set_width()`, where any queued lines were computed
    /// against the old width and can no longer be trusted to line up with the
    /// current render.
    fn rebuild_stable_queue_from_render(&mut self) {
        let target_stable_len = self.compute_target_stable_len();
        self.clear_stable_queue();
        if self.emitted_stable_len < target_stable_len {
            self.state
                .enqueue(self.rendered_lines[self.emitted_stable_len..target_stable_len].to_vec());
        }
        self.enqueued_stable_len = target_stable_len;
    }

    fn rebuild_stable_queue_preserving_partial_source_line(
        &mut self,
        partial_line: PartialSourceLine,
    ) {
        let source_line_start_len =
            self.rendered_len_for_source_boundary(partial_line.source_start);
        let source_line_end_len = self.rendered_len_for_source_boundary(partial_line.source_end);
        let target_stable_len = self.compute_target_stable_len();

        self.clear_stable_queue();
        self.emitted_stable_len = source_line_start_len;

        let mut queued = if self.render_mode == HistoryRenderMode::Raw {
            plain_hyperlink_lines(raw_lines_from_source(
                &self.raw_source[partial_line.emitted_source_end..partial_line.source_end],
            ))
        } else {
            trim_rendered_prefix(
                self.rendered_lines[source_line_start_len..source_line_end_len].to_vec(),
                &partial_line.emitted_prefix,
            )
        };
        let synthetic_len = queued.len();
        if source_line_end_len < target_stable_len {
            queued.extend(self.rendered_lines[source_line_end_len..target_stable_len].to_vec());
        }
        if !queued.is_empty() {
            self.state.enqueue(queued);
        }
        self.enqueued_stable_len = source_line_end_len.max(target_stable_len);
        if synthetic_len > 0 {
            self.synthetic_queue_remaining = synthetic_len;
            self.synthetic_queue_target_emitted_len = Some(source_line_end_len);
            self.synthetic_queue_partial = Some(partial_line);
        }
    }

    fn clear_stable_queue(&mut self) {
        self.state.clear_queue();
        self.clear_synthetic_queue_accounting();
    }

    fn clear_synthetic_queue_accounting(&mut self) {
        self.synthetic_queue_remaining = 0;
        self.synthetic_queue_target_emitted_len = None;
        self.synthetic_queue_partial = None;
    }

    fn clear_queue_for_interruption(&mut self) {
        self.state.clear_queue();
        if self.synthetic_queue_remaining == 0 {
            self.synthetic_queue_partial = None;
        }
        self.synthetic_queue_remaining = 0;
        self.synthetic_queue_target_emitted_len = None;
    }

    fn account_emitted_step(&mut self, step: &[HyperlinkLine]) {
        let mut ordinary_lines = step.len();
        if self.synthetic_queue_remaining > 0 {
            let synthetic_lines = ordinary_lines.min(self.synthetic_queue_remaining);
            let mut emitted_prefix = None;
            if let Some(partial_line) = &mut self.synthetic_queue_partial {
                partial_line
                    .emitted_prefix
                    .extend(step[..synthetic_lines].to_vec());
                emitted_prefix = Some((
                    partial_line.source_start,
                    partial_line.source_end,
                    partial_line.emitted_prefix.clone(),
                ));
            }
            if let Some((source_start, source_end, emitted_prefix)) = emitted_prefix {
                let emitted_source_end = self.source_boundary_after_rendered_prefix(
                    source_start,
                    source_end,
                    &emitted_prefix,
                );
                if let Some(partial_line) = &mut self.synthetic_queue_partial {
                    partial_line.emitted_source_end = emitted_source_end;
                }
            }
            ordinary_lines -= synthetic_lines;
            self.synthetic_queue_remaining -= synthetic_lines;
            if self.synthetic_queue_remaining == 0
                && let Some(target_len) = self.synthetic_queue_target_emitted_len.take()
            {
                self.emitted_stable_len = target_len;
                self.synthetic_queue_partial = None;
            }
        }
        self.emitted_stable_len += ordinary_lines;
    }

    /// How many rendered lines to withhold as mutable tail.
    ///
    /// When a table is detected (`Confirmed` or `PendingHeader`), the entire
    /// table region is held as tail because adding a row can reshape table
    /// column widths. For `PendingHeader`, only content from the speculative
    /// header line onward is kept mutable so earlier prose can continue
    /// streaming. When no table is detected, the configured default source
    /// suffix remains mutable. This is the core decision point for the holdback
    /// mechanism.
    fn active_tail_budget_lines(&mut self) -> usize {
        if self.render_mode == HistoryRenderMode::Raw {
            return 0;
        }
        let scan_start = Instant::now();
        let holdback_state = self.holdback_scanner.state();
        let tail_budget = match holdback_state {
            TableHoldbackState::Confirmed { table_start: start }
            | TableHoldbackState::PendingHeader {
                header_start: start,
            } => self.tail_budget_from_source_start(start),
            TableHoldbackState::None => self.default_tail_budget_lines(),
        };
        tracing::trace!(
            state = ?holdback_state,
            tail_budget,
            elapsed_us = scan_start.elapsed().as_micros(),
            "table holdback decision",
        );
        tail_budget
    }

    /// Convert a raw-source boundary into the number of rendered tail lines.
    ///
    /// The important contract here is that the holdback scanner reasons in
    /// byte offsets while the queue operates in rendered lines. This helper is
    /// the only place where those coordinate systems are bridged.
    fn tail_budget_from_source_start(&mut self, source_start: usize) -> usize {
        if source_start == 0 {
            return self.rendered_lines.len();
        }
        let source_start = source_start.min(self.raw_source.len());
        let stable_prefix_len = self.stable_prefix_len_for_source_start(source_start);
        self.rendered_lines.len().saturating_sub(stable_prefix_len)
    }

    /// Render the stable prefix before `source_start` and return its line count.
    ///
    /// This value is cached because dense table streams can call into this path
    /// for every committed line while the header/delimiter/body are still
    /// arriving incrementally.
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
        let stable_prefix_render =
            self.render_source(&self.raw_source[..source_start.min(self.raw_source.len())]);
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

    fn rendered_len_for_source_boundary(&mut self, source_boundary: usize) -> usize {
        if source_boundary == 0 {
            return 0;
        }
        if source_boundary >= self.raw_source.len() {
            return self.rendered_lines.len();
        }
        self.stable_prefix_len_for_source_start(source_boundary)
    }

    fn source_boundary_after_rendered_len(&self, rendered_len: usize) -> usize {
        if rendered_len == 0 || self.raw_source.is_empty() {
            return 0;
        }
        if rendered_len >= self.rendered_lines.len() {
            return self.raw_source.len();
        }

        let mut boundaries = self
            .raw_source
            .match_indices('\n')
            .map(|(idx, _)| idx + 1)
            .collect::<Vec<_>>();
        if boundaries.last().copied() != Some(self.raw_source.len()) {
            boundaries.push(self.raw_source.len());
        }

        for boundary in boundaries {
            if self.render_source(&self.raw_source[..boundary]).len() >= rendered_len {
                return boundary;
            }
        }
        self.raw_source.len()
    }

    fn partial_source_line_after_rendered_len(
        &self,
        rendered_len: usize,
    ) -> Option<PartialSourceLine> {
        if rendered_len == 0 || rendered_len >= self.rendered_lines.len() {
            return None;
        }

        let mut source_start = 0;
        let mut source_start_rendered_len = 0;
        for source_end in self.source_line_boundaries() {
            let source_end_rendered_len = self.render_source(&self.raw_source[..source_end]).len();
            if rendered_len < source_end_rendered_len {
                if rendered_len <= source_start_rendered_len {
                    return None;
                }
                let emitted_prefix =
                    self.rendered_lines[source_start_rendered_len..rendered_len].to_vec();
                let emitted_source_end = self.source_boundary_after_rendered_prefix(
                    source_start,
                    source_end,
                    &emitted_prefix,
                );
                return Some(PartialSourceLine {
                    source_start,
                    source_end,
                    emitted_source_end,
                    emitted_prefix,
                });
            }
            source_start = source_end;
            source_start_rendered_len = source_end_rendered_len;
        }
        None
    }

    fn source_line_boundaries(&self) -> Vec<usize> {
        let mut boundaries = self
            .raw_source
            .match_indices('\n')
            .map(|(idx, _)| idx + 1)
            .collect::<Vec<_>>();
        if boundaries.last().copied() != Some(self.raw_source.len()) {
            boundaries.push(self.raw_source.len());
        }
        boundaries
    }

    fn source_boundary_after_rendered_prefix(
        &self,
        source_start: usize,
        source_end: usize,
        emitted_prefix: &[HyperlinkLine],
    ) -> usize {
        if emitted_prefix.is_empty() {
            return source_start;
        }
        let emitted_text = emitted_prefix
            .iter()
            .map(hyperlink_line_text)
            .collect::<Vec<_>>();
        let emitted_visible_text = emitted_text.join(" ");
        if let Some(boundary) = visible_text_source_boundary(
            &self.raw_source[source_start..source_end],
            &emitted_visible_text,
        ) {
            return source_start + boundary;
        }

        let mut boundaries = Vec::new();
        boundaries.push(source_start);
        boundaries.extend(
            self.raw_source[source_start..source_end]
                .char_indices()
                .map(|(idx, ch)| source_start + idx + ch.len_utf8()),
        );

        for boundary in boundaries {
            let rendered = self.render_source(&self.raw_source[..boundary]);
            if rendered.len() < emitted_prefix.len() {
                continue;
            }
            let rendered_text = rendered[..emitted_prefix.len()]
                .iter()
                .map(hyperlink_line_text)
                .collect::<Vec<_>>();
            if rendered_text == emitted_text {
                return boundary;
            }
        }

        self.source_boundary_after_rendered_len(emitted_prefix.len())
            .min(source_end)
            .max(source_start)
    }

    fn default_tail_budget_lines(&mut self) -> usize {
        if self.default_tail_lines == 0 || self.rendered_lines.is_empty() {
            return 0;
        }
        let Some(source_start) = self.default_tail_source_start() else {
            return 0;
        };
        self.tail_budget_from_source_start(source_start)
    }

    fn live_default_tail_source_start(&self) -> Option<usize> {
        if self.default_tail_lines == 0 || self.enqueued_stable_len >= self.rendered_lines.len() {
            return None;
        }
        self.default_tail_source_start()
    }

    fn default_tail_source_start(&self) -> Option<usize> {
        tail_source_start(&self.raw_source, self.default_tail_lines)
    }

    fn requires_final_scrollback_reflow(&self) -> bool {
        self.render_mode != HistoryRenderMode::Raw
            && !matches!(self.holdback_scanner.state(), TableHoldbackState::None)
    }
}

fn visible_text_source_boundary(source: &str, visible_prefix: &str) -> Option<usize> {
    if visible_prefix.is_empty() {
        return Some(0);
    }

    let mut visible_chars = visible_prefix.chars().peekable();
    let mut iter = source.char_indices().peekable();
    while let Some((idx, ch)) = iter.next() {
        if visible_chars.peek().is_none() {
            return Some(idx);
        }

        if ch == '[' {
            let mut label_end = None;
            let mut label_chars = Vec::new();
            for (label_idx, label_ch) in iter.by_ref() {
                if label_ch == ']' {
                    label_end = Some(label_idx + label_ch.len_utf8());
                    break;
                }
                label_chars.push((label_idx, label_ch));
            }
            let label_end = label_end?;
            for (label_idx, label_ch) in label_chars {
                match visible_chars.peek().copied() {
                    Some(expected) if expected == label_ch => {
                        visible_chars.next();
                        if visible_chars.peek().is_none() {
                            return Some(
                                skip_link_destination(source, label_end)
                                    .unwrap_or(label_idx + label_ch.len_utf8()),
                            );
                        }
                    }
                    _ => return None,
                }
            }
            if let Some(destination_end) = skip_link_destination(source, label_end) {
                while let Some((next_idx, _)) = iter.peek().copied() {
                    if next_idx < destination_end {
                        iter.next();
                    } else {
                        break;
                    }
                }
            }
            continue;
        }

        if matches!(ch, '*' | '_' | '`') {
            continue;
        }

        match visible_chars.peek().copied() {
            Some(expected) if expected == ch => {
                visible_chars.next();
                if visible_chars.peek().is_none() {
                    return Some(idx + ch.len_utf8());
                }
            }
            _ => return None,
        }
    }

    visible_chars.peek().is_none().then_some(source.len())
}

fn skip_link_destination(source: &str, label_end: usize) -> Option<usize> {
    let after_label = source.get(label_end..)?;
    if !after_label.starts_with('(') {
        return None;
    }
    after_label
        .find(')')
        .map(|destination_end| label_end + destination_end + ')'.len_utf8())
}

fn trim_rendered_prefix(
    lines: Vec<HyperlinkLine>,
    emitted_prefix: &[HyperlinkLine],
) -> Vec<HyperlinkLine> {
    let prefix_text = emitted_prefix
        .iter()
        .map(hyperlink_line_text)
        .collect::<Vec<_>>()
        .join(" ");
    let mut prefix_chars = emitted_prefix
        .iter()
        .map(hyperlink_line_text)
        .map(|text| text.chars().count())
        .sum::<usize>();
    if prefix_chars == 0 {
        return lines;
    }

    let mut out = Vec::with_capacity(lines.len());
    for line in lines {
        let text = hyperlink_line_text(&line);
        let line_chars = text.chars().count();
        let chars_to_skip = if !prefix_text.is_empty() {
            text.find(&prefix_text)
                .map(|start| text[..start].chars().count() + prefix_text.chars().count())
        } else {
            None
        };
        if prefix_chars >= line_chars {
            prefix_chars -= line_chars;
            continue;
        }
        if prefix_chars > 0 {
            let chars_to_skip = chars_to_skip.unwrap_or(prefix_chars);
            prefix_chars = 0;
            if let Some(trimmed_line) = trim_hyperlink_line_prefix(line, chars_to_skip) {
                out.push(trimmed_line);
            }
        } else {
            out.push(line);
        }
    }
    out
}

fn trim_hyperlink_line_prefix(line: HyperlinkLine, chars_to_skip: usize) -> Option<HyperlinkLine> {
    let text = hyperlink_line_text(&line);
    let leading_whitespace = text
        .chars()
        .skip(chars_to_skip)
        .take_while(|ch| ch.is_whitespace())
        .count();
    let total_chars_to_skip = chars_to_skip + leading_whitespace;
    if total_chars_to_skip >= text.chars().count() {
        return None;
    }

    let skipped_text = text.chars().take(total_chars_to_skip).collect::<String>();
    let skipped_columns = skipped_text.width();
    let mut remaining_chars_to_skip = total_chars_to_skip;
    let mut spans = Vec::with_capacity(line.line.spans.len());
    for span in line.line.spans {
        let span_chars = span.content.chars().count();
        if remaining_chars_to_skip >= span_chars {
            remaining_chars_to_skip -= span_chars;
            continue;
        }
        if remaining_chars_to_skip > 0 {
            let suffix = span
                .content
                .chars()
                .skip(remaining_chars_to_skip)
                .collect::<String>();
            spans.push(Span {
                content: suffix.into(),
                style: span.style,
            });
            remaining_chars_to_skip = 0;
        } else {
            spans.push(span);
        }
    }

    let hyperlinks = line
        .hyperlinks
        .into_iter()
        .filter_map(|link| {
            if link.columns.end <= skipped_columns {
                None
            } else {
                Some(TerminalHyperlink {
                    columns: link.columns.start.saturating_sub(skipped_columns)
                        ..link.columns.end.saturating_sub(skipped_columns),
                    destination: link.destination,
                })
            }
        })
        .collect::<Vec<_>>();
    Some(HyperlinkLine {
        line: Line::from(spans).style(line.line.style),
        hyperlinks,
    })
}

fn hyperlink_line_text(line: &HyperlinkLine) -> String {
    line.line
        .spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect()
}

fn tail_source_start(source: &str, tail_lines: usize) -> Option<usize> {
    if source.is_empty() || tail_lines == 0 {
        return None;
    }

    let mut end = source.len();
    if source.as_bytes().last() == Some(&b'\n') {
        end = end.saturating_sub(1);
    }
    let content_end = end;

    let mut start = 0;
    for _ in 0..tail_lines {
        start = source[..end].rfind('\n').map(|idx| idx + 1).unwrap_or(0);
        end = start.saturating_sub(1);
    }
    if start == content_end && start > 0 {
        start = source[..end].rfind('\n').map(|idx| idx + 1).unwrap_or(0);
    }
    Some(start)
}

/// Controller for streaming agent message content with table-aware holdback.
///
/// Wraps [`StreamCore`] and adds `AgentMessageCell` emission styling.
pub(crate) struct StreamController {
    core: StreamCore,
    header_emitted: bool,
}

impl StreamController {
    /// Create a controller whose markdown renderer shortens local file links relative to `cwd`.
    ///
    /// `width` is the content width available to markdown rendering, not necessarily the full
    /// terminal width. Passing a stale width after resize will keep queued live output wrapped for
    /// the old viewport until app-level reflow repairs the finalized transcript.
    pub(crate) fn new(width: Option<usize>, cwd: &Path, render_mode: HistoryRenderMode) -> Self {
        Self {
            core: StreamCore::new(width, cwd, render_mode, /*default_tail_lines*/ 1),
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
    pub(crate) fn current_tail_lines(&self) -> Vec<HyperlinkLine> {
        self.core.current_tail_lines()
    }

    #[inline]
    pub(crate) fn tail_starts_stream(&self) -> bool {
        !self.header_emitted && self.core.enqueued_stable_len == 0
    }

    #[cfg(test)]
    #[inline]
    pub(crate) fn has_live_tail(&self) -> bool {
        self.core.has_tail()
    }

    pub(crate) fn requires_final_scrollback_reflow(&self) -> bool {
        self.core.requires_final_scrollback_reflow()
    }

    pub(crate) fn clear_queue(&mut self) {
        self.core.clear_queue_for_interruption();
        self.core.enqueued_stable_len = self.core.emitted_stable_len;
    }

    pub(crate) fn set_width(&mut self, width: Option<usize>) {
        self.core.set_width(width);
    }

    pub(crate) fn set_render_mode(&mut self, render_mode: HistoryRenderMode) -> bool {
        self.core.set_render_mode(render_mode)
    }

    fn emit(&mut self, lines: Vec<HyperlinkLine>) -> Option<Box<dyn HistoryCell>> {
        if lines.is_empty() {
            return None;
        }
        Some(Box::new(
            history_cell::AgentMessageCell::new_hyperlink_lines(lines, {
                let header_emitted = self.header_emitted;
                self.header_emitted = true;
                !header_emitted
            }),
        ))
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
    /// Create a plan-stream controller whose markdown renderer shortens local file links relative
    /// to `cwd`.
    ///
    /// The width has the same meaning as in `StreamController`: it is the markdown body width, and
    /// callers must update it when the terminal width changes.
    pub(crate) fn new(width: Option<usize>, cwd: &Path, render_mode: HistoryRenderMode) -> Self {
        Self {
            core: StreamCore::new(width, cwd, render_mode, /*default_tail_lines*/ 1),
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
        let out = self.emit(remaining, /*include_bottom_padding*/ true);
        self.core.reset();
        (out, Some(source))
    }

    pub(crate) fn on_commit_tick(&mut self) -> (Option<Box<dyn HistoryCell>>, bool) {
        let step = self.core.tick();
        (
            self.emit(step, /*include_bottom_padding*/ false),
            self.core.is_idle(),
        )
    }

    pub(crate) fn on_commit_tick_batch(
        &mut self,
        max_lines: usize,
    ) -> (Option<Box<dyn HistoryCell>>, bool) {
        let step = self.core.tick_batch(max_lines);
        (
            self.emit(step, /*include_bottom_padding*/ false),
            self.core.is_idle(),
        )
    }

    #[inline]
    pub(crate) fn queued_lines(&self) -> usize {
        self.core.queued_lines()
    }

    #[cfg(test)]
    #[inline]
    pub(crate) fn has_live_tail(&self) -> bool {
        self.core.has_tail()
    }

    pub(crate) fn requires_final_scrollback_reflow(&self) -> bool {
        self.core.requires_final_scrollback_reflow()
    }

    #[inline]
    pub(crate) fn current_tail_lines(&self) -> Vec<HyperlinkLine> {
        self.core.current_tail_lines()
    }

    #[inline]
    pub(crate) fn tail_starts_stream(&self) -> bool {
        !self.header_emitted && self.core.enqueued_stable_len == 0
    }

    pub(crate) fn current_tail_display_lines(&self) -> Vec<HyperlinkLine> {
        let lines = self.current_tail_lines();
        if lines.is_empty() {
            return Vec::new();
        }
        self.render_display_lines(lines, /*include_bottom_padding*/ false)
    }

    pub(crate) fn oldest_queued_age(&self, now: Instant) -> Option<Duration> {
        self.core.oldest_queued_age(now)
    }

    pub(crate) fn clear_queue(&mut self) {
        self.core.clear_queue_for_interruption();
        self.core.enqueued_stable_len = self.core.emitted_stable_len;
    }

    pub(crate) fn set_width(&mut self, width: Option<usize>) {
        self.core.set_width(width);
    }

    pub(crate) fn set_render_mode(&mut self, render_mode: HistoryRenderMode) -> bool {
        self.core.set_render_mode(render_mode)
    }

    fn emit(
        &mut self,
        lines: Vec<HyperlinkLine>,
        include_bottom_padding: bool,
    ) -> Option<Box<dyn HistoryCell>> {
        if lines.is_empty() && !include_bottom_padding {
            return None;
        }

        let is_stream_continuation = self.header_emitted;
        let out_lines = self.render_display_lines(lines, include_bottom_padding);
        self.header_emitted = true;
        self.top_padding_emitted = true;

        Some(Box::new(history_cell::new_proposed_plan_stream(
            out_lines,
            is_stream_continuation,
        )))
    }

    fn render_display_lines(
        &self,
        lines: Vec<HyperlinkLine>,
        include_bottom_padding: bool,
    ) -> Vec<HyperlinkLine> {
        let mut out_lines: Vec<HyperlinkLine> = Vec::with_capacity(/*capacity*/ 4);
        if !self.header_emitted {
            out_lines.push(HyperlinkLine::new(
                vec!["• ".dim(), "Proposed Plan".bold()].into(),
            ));
            out_lines.push(HyperlinkLine::new(Line::from(" ")));
        }

        let mut plan_lines: Vec<HyperlinkLine> = Vec::with_capacity(/*capacity*/ 4);
        if !self.top_padding_emitted {
            plan_lines.push(HyperlinkLine::new(Line::from(" ")));
        }
        plan_lines.extend(lines);
        if include_bottom_padding {
            plan_lines.push(HyperlinkLine::new(Line::from(" ")));
        }

        let plan_style = proposed_plan_style();
        let plan_lines = prefix_hyperlink_lines(plan_lines, "  ".into(), "  ".into())
            .into_iter()
            .map(|line| line.style(plan_style))
            .collect::<Vec<_>>();
        out_lines.extend(plan_lines);
        out_lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terminal_hyperlinks::visible_lines;
    use pretty_assertions::assert_eq;
    use std::path::PathBuf;

    fn test_cwd() -> PathBuf {
        // These tests only need a stable absolute cwd; using temp_dir() avoids baking Unix- or
        // Windows-specific root semantics into the fixtures.
        std::env::temp_dir()
    }

    fn stream_controller(width: Option<usize>) -> StreamController {
        StreamController::new(width, &test_cwd(), HistoryRenderMode::Rich)
    }

    fn plan_stream_controller(width: Option<usize>) -> PlanStreamController {
        PlanStreamController::new(width, &test_cwd(), HistoryRenderMode::Rich)
    }

    const HAPPY_PATH_DUPLICATE_REPRO_DELTAS: &[&str] = &[
        "**Happy Path**\n",
        "\n",
        "1. In the second terminal, make the smoke home non-writable:\n",
        "   ```fish\n",
        "   chmod a-w $CODEX_SMOKE_HOME\n",
        "   ```\n",
        "   Expected: the running TUI stays open.\n",
        "\n",
        "2. In the TUI, type `/model` and press Enter.\n",
    ];

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

    fn hyperlink_lines_to_plain_strings(lines: &[HyperlinkLine]) -> Vec<String> {
        lines_to_plain_strings(&visible_lines(lines.to_vec()))
    }

    fn collect_streamed_lines(deltas: &[&str], width: Option<usize>) -> Vec<String> {
        let mut ctrl = stream_controller(width);
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

    fn collect_visible_stream_snapshots(deltas: &[&str], width: Option<usize>) -> Vec<Vec<String>> {
        let mut ctrl = stream_controller(width);
        let mut committed_lines = Vec::new();
        let mut snapshots = Vec::new();
        for delta in deltas {
            ctrl.push(delta);
            while let (Some(cell), idle) = ctrl.on_commit_tick() {
                committed_lines.extend(cell.transcript_lines(u16::MAX));
                if idle {
                    break;
                }
            }

            let mut visible = lines_to_plain_strings(&committed_lines)
                .into_iter()
                .map(|s| s.chars().skip(2).collect::<String>())
                .collect::<Vec<_>>();
            visible.extend(hyperlink_lines_to_plain_strings(&ctrl.current_tail_lines()));
            snapshots.push(visible);
        }
        snapshots
    }

    fn collect_visible_plan_stream_snapshots(
        deltas: &[&str],
        width: Option<usize>,
    ) -> Vec<Vec<String>> {
        let mut ctrl = plan_stream_controller(width);
        let mut committed_lines = Vec::new();
        let mut snapshots = Vec::new();
        for delta in deltas {
            ctrl.push(delta);
            while let (Some(cell), idle) = ctrl.on_commit_tick() {
                committed_lines.extend(cell.transcript_lines(u16::MAX));
                if idle {
                    break;
                }
            }

            let mut visible = lines_to_plain_strings(&committed_lines);
            visible.extend(hyperlink_lines_to_plain_strings(
                &ctrl.current_tail_display_lines(),
            ));
            snapshots.push(visible);
        }
        snapshots
    }

    fn format_stream_snapshots(snapshots: &[Vec<String>]) -> String {
        snapshots
            .iter()
            .enumerate()
            .map(|(idx, lines)| format!("after delta {idx}:\n{}", lines.join("\n")))
            .collect::<Vec<_>>()
            .join("\n---\n")
    }

    fn collect_plan_streamed_lines(deltas: &[&str], width: Option<usize>) -> Vec<String> {
        let mut ctrl = plan_stream_controller(width);
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
        let mut ctrl = stream_controller(Some(120));
        let delta = "This is a long line that should wrap into multiple rows when resized.\n";
        assert!(!ctrl.push(delta));
        assert!(ctrl.push("tail line\n"));
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
        let mut ctrl = stream_controller(Some(120));
        let line =
            "This is a long line that definitely wraps when the terminal shrinks to 24 columns.\n";
        ctrl.push(line);
        ctrl.push("tail line\n");
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
        let mut ctrl = stream_controller(Some(80));
        assert!(!ctrl.push("line one\n"));
        assert!(ctrl.push("line two\n"));
        assert_eq!(ctrl.queued_lines(), 1);

        let (cell, idle) = ctrl.on_commit_tick_batch(/*max_lines*/ 0);
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
        let mut ctrl = stream_controller(Some(80));
        assert!(!ctrl.has_live_tail());

        ctrl.core.rendered_lines = vec![Line::from("tail line").into()];
        ctrl.core.enqueued_stable_len = 0;
        assert!(ctrl.has_live_tail());

        ctrl.core.enqueued_stable_len = 1;
        assert!(!ctrl.has_live_tail());
    }

    #[test]
    fn plan_controller_has_live_tail_reflects_tail_presence() {
        let mut ctrl = plan_stream_controller(Some(80));
        assert!(!ctrl.has_live_tail());

        ctrl.core.rendered_lines = vec![Line::from("tail line").into()];
        ctrl.core.enqueued_stable_len = 0;
        assert!(ctrl.has_live_tail());

        ctrl.core.enqueued_stable_len = 1;
        assert!(!ctrl.has_live_tail());
    }

    #[test]
    fn controller_live_tail_keeps_uncommitted_table_cell_newline_gated() {
        let mut ctrl = stream_controller(Some(80));
        ctrl.push("| A | B |\n");
        ctrl.push("| --- | --- |\n");
        ctrl.push("| partial");

        let tail = hyperlink_lines_to_plain_strings(&ctrl.current_tail_lines()).join("\n");
        assert!(
            !tail.contains("partial"),
            "expected live tail to remain newline-gated: {tail:?}",
        );
    }

    #[test]
    fn controller_live_tail_requires_table_holdback_state() {
        let mut ctrl = stream_controller(Some(80));
        ctrl.push("plain text without newline");

        assert!(
            ctrl.current_tail_lines().is_empty(),
            "expected no live tail outside table holdback state",
        );
        assert!(!ctrl.has_live_tail());
    }

    #[test]
    fn controller_does_not_duplicate_list_continuation_after_fenced_code() {
        let snapshots =
            collect_visible_stream_snapshots(HAPPY_PATH_DUPLICATE_REPRO_DELTAS, Some(/*width*/ 80));

        for lines in &snapshots {
            let expected_count = lines
                .iter()
                .filter(|line| line.trim() == "Expected: the running TUI stays open.")
                .count();
            assert!(
                expected_count <= 1,
                "expected list continuation to render at most once, got {lines:?}",
            );
        }
        let final_count = snapshots
            .last()
            .into_iter()
            .flatten()
            .filter(|line| line.trim() == "Expected: the running TUI stays open.")
            .count();
        assert_eq!(
            final_count, 1,
            "expected final visible snapshot to include the list continuation once"
        );
    }

    #[test]
    fn controller_streamed_list_continuation_snapshot() {
        let snapshots =
            collect_visible_stream_snapshots(HAPPY_PATH_DUPLICATE_REPRO_DELTAS, Some(/*width*/ 80));

        insta::assert_snapshot!(format_stream_snapshots(&snapshots));
    }

    #[test]
    fn plan_controller_does_not_duplicate_list_continuation_after_fenced_code() {
        let lines =
            collect_plan_streamed_lines(HAPPY_PATH_DUPLICATE_REPRO_DELTAS, Some(/*width*/ 80));

        let expected_count = lines
            .iter()
            .filter(|line| line.trim() == "Expected: the running TUI stays open.")
            .count();
        assert_eq!(
            expected_count, 1,
            "expected plan list continuation to render once, got {lines:?}",
        );
    }

    #[test]
    fn plan_controller_streamed_list_continuation_snapshot() {
        let snapshots = collect_visible_plan_stream_snapshots(
            HAPPY_PATH_DUPLICATE_REPRO_DELTAS,
            Some(/*width*/ 80),
        );

        insta::assert_snapshot!(format_stream_snapshots(&snapshots));
    }

    #[test]
    fn controller_live_tail_rerenders_table_tail_after_resize() {
        let mut ctrl = stream_controller(Some(96));
        ctrl.push("| # | Feature | Details | Link |\n");
        ctrl.push("| --- | --- | --- | --- |\n");
        ctrl.push(
            "| 1 | RESIZE_REPRO_SENTINEL | long wrapped content that should be reflowed | https://example.com/resize |\n",
        );

        for width in [48, 104, 56] {
            ctrl.set_width(Some(width));
            let tail = hyperlink_lines_to_plain_strings(&ctrl.current_tail_lines());

            let mut expected = Vec::new();
            crate::markdown::append_markdown_agent(
                &ctrl.core.raw_source,
                Some(width),
                &mut expected,
            );
            let expected = lines_to_plain_strings(&expected);

            assert_eq!(
                tail, expected,
                "expected live table tail to be rerendered at width {width}",
            );
        }
    }

    #[test]
    fn controller_set_width_partial_drain_no_lost_lines() {
        let mut ctrl = stream_controller(Some(40));
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
    fn controller_set_width_partial_drain_does_not_duplicate_emitted_prefix() {
        let mut ctrl = stream_controller(Some(40));
        ctrl.push("AAAA BBBB CCCC DDDD EEEE FFFF GGGG HHHH IIII JJJJ\n");
        ctrl.push("second line\n");

        let (cell, idle) = ctrl.on_commit_tick();
        let emitted_before_resize = cell
            .expect("expected 1 emitted line")
            .transcript_lines(u16::MAX);
        assert!(!idle, "queue should still have lines");
        assert!(ctrl.queued_lines() > 0, "expected pending queued lines");

        ctrl.set_width(Some(/*width*/ 120));

        assert!(
            ctrl.queued_lines() > 0,
            "un-emitted source-line suffix should remain queued after widening"
        );

        let (cell, _source) = ctrl.finalize();
        let final_lines = cell
            .map(|cell| lines_to_plain_strings(&cell.transcript_lines(u16::MAX)))
            .unwrap_or_default();
        let emitted_before_resize = lines_to_plain_strings(&emitted_before_resize).join("\n");
        let finalized_after_resize = final_lines.join("\n");
        assert!(
            !finalized_after_resize.contains("AAAA BBBB CCCC"),
            "finalize must not replay the already-visible line prefix; emitted before resize: {emitted_before_resize:?}, finalized after resize: {finalized_after_resize:?}",
        );
        assert!(
            finalized_after_resize.contains("IIII JJJJ"),
            "finalize must preserve the un-emitted source-line suffix; emitted before resize: {emitted_before_resize:?}, finalized after resize: {finalized_after_resize:?}",
        );
        assert!(
            final_lines.iter().any(|line| line.contains("second line")),
            "ordinary mutable tail should flush on finalize; got {final_lines:?}",
        );
    }

    #[test]
    fn controller_set_width_preserves_in_flight_tail() {
        let mut ctrl = stream_controller(Some(80));
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
    fn controller_set_width_preserves_default_tail_after_stable_prefix_compresses() {
        let mut ctrl = stream_controller(Some(/*width*/ 24));
        ctrl.push("AAAA BBBB CCCC DDDD EEEE FFFF GGGG HHHH IIII JJJJ\n");
        ctrl.push("tail stays visible\n");

        let (_cell, idle) = ctrl.on_commit_tick_batch(usize::MAX);
        assert!(idle, "stable prefix should fully drain");
        assert_eq!(ctrl.queued_lines(), 0, "expected empty queue before resize");
        assert!(
            ctrl.has_live_tail(),
            "expected short second line to be held as the default tail",
        );

        ctrl.set_width(Some(/*width*/ 120));

        let tail_after_resize = hyperlink_lines_to_plain_strings(&ctrl.current_tail_lines());
        assert_eq!(
            tail_after_resize,
            vec!["tail stays visible".to_string()],
            "wider resize must not mark the default tail as already emitted",
        );

        let (cell, _source) = ctrl.finalize();
        let finalized = cell
            .map(|cell| lines_to_plain_strings(&cell.transcript_lines(u16::MAX)))
            .unwrap_or_default();
        assert_eq!(
            finalized,
            vec!["  tail stays visible".to_string()],
            "finalize should still flush the preserved default tail",
        );
    }

    #[test]
    fn controller_set_render_mode_does_not_replay_drained_raw_output() {
        let mut ctrl =
            StreamController::new(Some(/*width*/ 120), &test_cwd(), HistoryRenderMode::Raw);
        ctrl.push("AAAA BBBB CCCC DDDD EEEE FFFF GGGG HHHH IIII JJJJ\n");
        ctrl.push("tail already emitted\n");

        let (_cell, idle) = ctrl.on_commit_tick_batch(usize::MAX);
        assert!(idle, "raw stable output should fully drain");
        assert_eq!(ctrl.queued_lines(), 0, "expected empty queue before toggle");

        ctrl.set_render_mode(HistoryRenderMode::Rich);

        let (cell, _source) = ctrl.finalize();
        assert!(
            cell.is_none(),
            "already-drained raw output must not be replayed after switching to rich mode",
        );
    }

    #[test]
    fn controller_set_render_mode_preserves_held_default_tail_when_switching_to_raw() {
        let mut ctrl = stream_controller(Some(/*width*/ 120));
        ctrl.push("prefix already emitted\n");
        ctrl.push("tail stays visible\n");

        let (_cell, idle) = ctrl.on_commit_tick_batch(usize::MAX);
        assert!(idle, "stable prefix should fully drain");
        assert_eq!(ctrl.queued_lines(), 0, "expected empty queue before toggle");
        assert!(
            ctrl.has_live_tail(),
            "expected second line to be held as the default tail",
        );

        ctrl.set_render_mode(HistoryRenderMode::Raw);

        let tail_after_toggle = hyperlink_lines_to_plain_strings(&ctrl.current_tail_lines());
        assert_eq!(
            tail_after_toggle,
            vec!["tail stays visible".to_string()],
            "raw toggle must not mark the held default tail as already emitted",
        );

        assert!(
            ctrl.push("new raw line\n"),
            "next raw line should make the preserved tail stable",
        );
        let (cell, idle) = ctrl.on_commit_tick_batch(usize::MAX);
        assert!(idle, "raw stable output should fully drain");
        let emitted = cell
            .map(|cell| lines_to_plain_strings(&cell.transcript_lines(u16::MAX)))
            .unwrap_or_default();
        assert_eq!(
            emitted,
            vec![
                "  tail stays visible".to_string(),
                "  new raw line".to_string(),
            ],
            "held tail should drain before newer raw output",
        );
    }

    #[test]
    fn controller_set_width_preserves_table_tail_when_queue_is_empty() {
        let mut ctrl = stream_controller(Some(80));
        ctrl.push("intro line\n");

        let (_cell, idle) = ctrl.on_commit_tick();
        assert!(idle, "intro line should fully drain");
        assert_eq!(ctrl.queued_lines(), 0, "expected empty queue before table");

        ctrl.push("| A | B |\n");
        let (_cell, idle) = ctrl.on_commit_tick();
        assert!(idle, "pre-table intro should drain before resize");
        assert_eq!(
            ctrl.queued_lines(),
            0,
            "pending table header should remain mutable tail, not queued",
        );
        assert!(ctrl.has_live_tail(), "expected live tail before resize");

        ctrl.set_width(Some(24));

        let tail_after = hyperlink_lines_to_plain_strings(&ctrl.current_tail_lines());
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
        let mut ctrl = plan_stream_controller(Some(80));
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
    fn plan_controller_holds_table_header_as_live_tail() {
        let mut ctrl = plan_stream_controller(Some(80));
        assert!(!ctrl.push("Intro\n"));
        assert!(
            ctrl.has_live_tail(),
            "expected plan intro to start as mutable tail",
        );

        assert!(ctrl.push("| Step | Owner |\n"));
        let (_cell, idle) = ctrl.on_commit_tick_batch(/*max_lines*/ usize::MAX);
        assert!(idle, "intro line should fully drain");

        assert!(
            ctrl.has_live_tail(),
            "expected plan table header to be held"
        );
    }

    #[test]
    fn controller_loose_vs_tight_with_commit_ticks_matches_full() {
        let mut ctrl = stream_controller(/*width*/ None);
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
        crate::markdown::append_markdown_agent(&source, /*width*/ None, &mut rendered);
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
            "".to_string(),
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
        crate::markdown::append_markdown_agent(&source, /*width*/ Some(80), &mut rendered);
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
        crate::markdown::append_markdown_agent(&source, /*width*/ Some(80), &mut rendered);
        let expected = lines_to_plain_strings(&rendered);

        assert_eq!(streamed, expected);
    }

    #[test]
    fn controller_keeps_pre_table_lines_queued_when_table_is_confirmed() {
        let mut ctrl = stream_controller(Some(80));

        ctrl.push("Intro line before table.\n");
        assert_eq!(ctrl.queued_lines(), 0);

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
        let mut ctrl = stream_controller(Some(120));
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
        crate::markdown::append_markdown_agent(&source, /*width*/ Some(32), &mut rendered);
        let expected = lines_to_plain_strings(&rendered);
        assert_eq!(streamed, expected);
    }

    #[test]
    fn controller_does_not_hold_back_pipe_prose_without_table_delimiter() {
        let mut ctrl = stream_controller(Some(80));

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
        let mut ctrl = stream_controller(Some(80));

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
        crate::markdown::append_markdown_agent(&source, /*width*/ Some(100), &mut rendered);
        let expected = lines_to_plain_strings(&rendered);

        assert_eq!(streamed, expected);
    }

    #[test]
    fn controller_renders_separators_for_multi_table_response_shape() {
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
            streamed.iter().any(|line| line.contains('━')),
            "expected table separator in streamed output: {streamed:?}"
        );
    }

    #[test]
    fn controller_renders_separators_for_no_outer_pipes_table_shape() {
        let source = "### 1) Basic\n\n| Name | Role | Active |\n|---|---|---|\n| Alice | Engineer | Yes |\n| Bob | Designer | No |\n\n### 2) No outer
  pipes\n\nCol A | Col B | Col C\n--- | --- | ---\nx | y | z\n10 | 20 | 30\n\n### 3) Another table\n\n| Key | Value |\n|---|---|\n| a | b |\n";

        let chunked = source
            .split_inclusive('\n')
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        let deltas = chunked.iter().map(String::as_str).collect::<Vec<_>>();
        let streamed = collect_streamed_lines(&deltas, Some(100));

        let mut rendered = Vec::new();
        crate::markdown::append_markdown_agent(source, /*width*/ Some(100), &mut rendered);
        let expected = lines_to_plain_strings(&rendered);

        assert_eq!(streamed, expected);
        let has_raw_no_outer_header = streamed
            .iter()
            .any(|line| line.trim() == "Col A | Col B | Col C");
        assert!(
            !has_raw_no_outer_header,
            "no-outer-pipes header should not remain raw in final streamed output: {streamed:?}"
        );
        assert!(
            streamed.iter().any(|line| line.contains('━')),
            "expected table separator in final streamed output: {streamed:?}"
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
        crate::markdown::append_markdown_agent(&source, /*width*/ Some(100), &mut rendered);
        let expected = lines_to_plain_strings(&rendered);

        assert_eq!(streamed, expected);
        assert!(
            streamed.iter().any(|line| line.contains('━')),
            "expected table separator for no-outer-pipes streaming: {streamed:?}"
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
        crate::markdown::append_markdown_agent(&source, /*width*/ Some(80), &mut rendered);
        let expected = lines_to_plain_strings(&rendered);

        assert_eq!(streamed, expected);
        assert!(
            streamed.iter().any(|line| line.contains('━')),
            "expected table separator for two-column no-outer table: {streamed:?}"
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
                .any(|line| line.contains(" Col A    Col B    Col C")),
            "expected converted no-outer table header in streamed output: {streamed:?}"
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
        crate::markdown::append_markdown_agent(source, /*width*/ Some(80), &mut rendered);
        let expected = lines_to_plain_strings(&rendered);

        assert_eq!(streamed, expected);
        assert!(
            streamed.iter().any(|line| line.contains('━')),
            "expected table separator in streamed output: {streamed:?}"
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
        crate::markdown::append_markdown_agent(source, /*width*/ Some(100), &mut rendered);
        let expected = lines_to_plain_strings(&rendered);

        assert_eq!(streamed, expected);
        assert!(
            streamed.iter().any(|line| line.contains('━')),
            "expected table separator in streamed output: {streamed:?}"
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
        let mut ctrl = stream_controller(width);
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
            visible.extend(visible_lines(ctrl.current_tail_lines()));
            let visible_plain = lines_to_plain_strings(&visible);

            let mut expected = Vec::new();
            crate::markdown::append_markdown_agent(
                &ctrl.core.raw_source,
                /*width*/ width,
                &mut expected,
            );
            let expected_plain = lines_to_plain_strings(&expected);

            assert_eq!(
                visible_plain, expected_plain,
                "live view diverged after delta: {delta:?}"
            );
        }
    }

    #[test]
    fn finalized_stream_table_preserves_semantic_url_fragments() {
        let destination = "https://example.com/a/very/long/path/to/a/table/artifact";
        let source = format!("| Item | URL |\n| --- | --- |\n| report | {destination} |\n");
        let mut ctrl = stream_controller(/*width*/ Some(32));
        ctrl.push(&source);

        let (cell, _) = ctrl.finalize();
        let lines = cell
            .expect("final stream table cell")
            .display_hyperlink_lines(/*width*/ 32);
        let linked_rows = lines
            .iter()
            .filter(|line| !line.hyperlinks.is_empty())
            .collect::<Vec<_>>();

        assert!(linked_rows.len() > 1);
        assert!(linked_rows.iter().all(|line| {
            line.hyperlinks
                .iter()
                .all(|link| link.destination == destination)
        }));
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
        crate::markdown::append_markdown_agent(source, /*width*/ Some(80), &mut rendered);
        let expected = lines_to_plain_strings(&rendered);

        assert_eq!(streamed, expected);
        assert!(
            streamed.iter().any(|line| line.trim() == "| A | B |"),
            "expected code-fenced pipe line to remain raw: {streamed:?}"
        );
        assert!(
            !streamed
                .iter()
                .any(|line| line.contains('━') || line.contains('─')),
            "did not expect a table separator for non-markdown fence: {streamed:?}"
        );
    }

    #[test]
    fn plan_controller_streamed_table_matches_final_render() {
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

        assert_eq!(streamed, baseline);
        assert!(
            streamed.iter().any(|line| line.contains('━')),
            "expected table separators in plan streamed output: {streamed:?}"
        );
        assert!(
            !streamed
                .iter()
                .any(|line| line.trim() == "| Step | Owner |"),
            "did not expect raw table header line in plan output: {streamed:?}"
        );
    }

    #[test]
    fn finalized_plan_stream_preserves_semantic_url_fragments() {
        let destination = "https://example.com/a/very/long/path/to/a/table/artifact";
        let source = format!("| Step | URL |\n| --- | --- |\n| Verify | {destination} |\n");
        let mut ctrl = PlanStreamController::new(
            /*width*/ Some(32),
            &test_cwd(),
            HistoryRenderMode::Rich,
        );
        ctrl.push(&source);

        let (cell, _) = ctrl.finalize();
        let lines = cell
            .expect("final plan stream table cell")
            .display_hyperlink_lines(/*width*/ 32);
        let linked_rows = lines
            .iter()
            .filter(|line| !line.hyperlinks.is_empty())
            .collect::<Vec<_>>();

        assert!(linked_rows.len() > 1);
        assert!(linked_rows.iter().all(|line| {
            line.hyperlinks
                .iter()
                .all(|link| link.destination == destination)
        }));
    }

    #[test]
    fn plan_controller_streamed_markdown_fenced_table_matches_final_render() {
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

        assert_eq!(streamed, baseline);
        assert!(
            streamed.iter().any(|line| line.contains('━')),
            "expected table separators in fenced plan output: {streamed:?}"
        );
        assert!(
            !streamed
                .iter()
                .any(|line| line.trim() == "| Step | Owner |"),
            "did not expect raw table header line in fenced plan output: {streamed:?}"
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
        let mut ctrl = stream_controller(Some(120));
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
        let mut ctrl = stream_controller(Some(20));
        ctrl.push("The quick brown fox jumps over the lazy dog near the riverbank.\n");
        ctrl.push("tail line\n");

        let (first_emit, idle) = ctrl.on_commit_tick();
        assert!(first_emit.is_some(), "expected first wrapped line emission");
        assert!(!idle, "expected remaining queued content after one tick");
        assert!(
            ctrl.queued_lines() > 0,
            "expected non-empty queue before resize"
        );

        ctrl.set_width(Some(/*width*/ 120));

        let (cell, _source) = ctrl.finalize();
        let remaining = cell
            .map(|c| lines_to_plain_strings(&c.transcript_lines(u16::MAX)))
            .unwrap_or_default()
            .into_iter()
            .map(|line| line.chars().skip(2).collect::<String>())
            .collect::<Vec<_>>();
        assert!(
            remaining.iter().any(|line| line.contains("riverbank"))
                && remaining.iter().any(|line| line.contains("tail line")),
            "un-emitted content should remain after resize remap: {remaining:?}",
        );
    }

    #[test]
    fn controller_set_width_partial_wrapped_emit_does_not_replay_prefix() {
        let mut ctrl = stream_controller(Some(/*width*/ 18));
        ctrl.push("alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu\n");
        ctrl.push("tail line\n");

        let (first_emit, idle) = ctrl.on_commit_tick();
        let first_emit = first_emit
            .expect("expected first wrapped line emission")
            .transcript_lines(u16::MAX);
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
        let first_emit = lines_to_plain_strings(&first_emit).join("\n");
        let joined = remaining.join("\n");
        assert!(
            !joined.contains("alpha beta"),
            "finalize must not replay the already-visible prefix; emitted before resize: {first_emit:?}, finalized after resize: {joined:?}",
        );
        assert!(
            joined.contains("lambda mu"),
            "finalize must preserve the un-emitted source-line suffix; emitted before resize: {first_emit:?}, finalized after resize: {joined:?}",
        );
        assert!(
            remaining.iter().any(|line| line.contains("tail line")),
            "ordinary mutable tail should flush on finalize; got {remaining:?}",
        );
    }

    #[test]
    fn controller_set_width_reuses_synthetic_partial_across_repeated_resize() {
        let mut ctrl = stream_controller(Some(/*width*/ 18));
        ctrl.push("alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu\n");
        ctrl.push("tail line\n");

        let (first_emit, idle) = ctrl.on_commit_tick();
        let first_emit = first_emit
            .expect("expected first wrapped line emission")
            .transcript_lines(u16::MAX);
        assert!(!idle, "expected remaining wrapped content after one tick");

        ctrl.set_width(Some(/*width*/ 80));
        assert!(
            ctrl.queued_lines() > 0,
            "expected synthetic queue after first resize"
        );
        ctrl.set_width(Some(/*width*/ 24));

        let (cell, _source) = ctrl.finalize();
        let remaining = cell
            .map(|c| lines_to_plain_strings(&c.transcript_lines(u16::MAX)))
            .unwrap_or_default();
        let first_emit = lines_to_plain_strings(&first_emit).join("\n");
        let joined = remaining.join("\n");
        assert!(
            !joined.contains("alpha beta"),
            "finalize must not replay the already-visible prefix after repeated resize; emitted before resize: {first_emit:?}, finalized after resize: {joined:?}",
        );
        assert!(
            joined.contains("lambda mu") && joined.contains("tail line"),
            "repeated resize must preserve un-emitted suffix and tail; emitted before resize: {first_emit:?}, finalized after resize: {joined:?}",
        );
    }

    #[test]
    fn controller_set_render_mode_partial_wrapped_emit_preserves_remaining_content() {
        let mut ctrl = stream_controller(Some(/*width*/ 18));
        ctrl.push("alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu\n");
        ctrl.push("tail line\n");

        let (first_emit, idle) = ctrl.on_commit_tick();
        let first_emit = first_emit
            .expect("expected first wrapped line emission")
            .transcript_lines(u16::MAX);
        assert!(!idle, "expected remaining wrapped content after one tick");
        assert!(
            ctrl.queued_lines() > 0,
            "expected queued wrapped remainder before raw toggle"
        );

        ctrl.set_render_mode(HistoryRenderMode::Raw);

        let (cell, _source) = ctrl.finalize();
        let remaining = cell
            .map(|c| lines_to_plain_strings(&c.transcript_lines(u16::MAX)))
            .unwrap_or_default();
        let first_emit = lines_to_plain_strings(&first_emit).join("\n");
        let joined = remaining.join("\n");
        assert!(
            !joined.contains("alpha beta"),
            "finalize must not replay the already-visible prefix after raw toggle; emitted before toggle: {first_emit:?}, finalized after toggle: {joined:?}",
        );
        assert!(
            joined.contains("lambda mu") && joined.contains("tail line"),
            "finalize must preserve un-emitted content after raw toggle; emitted before toggle: {first_emit:?}, finalized after toggle: {joined:?}",
        );
    }

    #[test]
    fn controller_set_render_mode_rerenders_partial_suffix_in_raw_mode() {
        let mut ctrl = stream_controller(Some(/*width*/ 18));
        ctrl.push("**alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu**\n");
        ctrl.push("tail line\n");

        let (first_emit, idle) = ctrl.on_commit_tick();
        let first_emit = first_emit
            .expect("expected first rich wrapped line emission")
            .transcript_lines(u16::MAX);
        assert!(!idle, "expected remaining rich content after one tick");

        ctrl.set_render_mode(HistoryRenderMode::Raw);

        let (cell, _source) = ctrl.finalize();
        let remaining = cell
            .map(|c| lines_to_plain_strings(&c.transcript_lines(u16::MAX)))
            .unwrap_or_default();
        let first_emit = lines_to_plain_strings(&first_emit).join("\n");
        let joined = remaining.join("\n");
        assert!(
            !joined.contains("alpha beta"),
            "finalize must not replay the already-visible rich prefix after raw toggle; emitted before toggle: {first_emit:?}, finalized after toggle: {joined:?}",
        );
        assert!(
            joined.contains("**") && joined.contains("tail line"),
            "pending suffix should be rendered from raw mode after toggle, not old rich rows; emitted before toggle: {first_emit:?}, finalized after toggle: {joined:?}",
        );
    }

    #[test]
    fn controller_set_render_mode_raw_suffix_starts_after_rich_link_prefix() {
        let mut ctrl = stream_controller(Some(/*width*/ 10));
        ctrl.push(
            "[alpha beta](https://example.com) gamma delta epsilon zeta eta theta iota kappa lambda mu\n",
        );
        ctrl.push("tail line\n");

        let (first_emit, idle) = ctrl.on_commit_tick();
        let first_emit = first_emit
            .expect("expected first rich wrapped line emission")
            .transcript_lines(u16::MAX);
        assert!(!idle, "expected remaining rich content after one tick");

        ctrl.set_render_mode(HistoryRenderMode::Raw);

        let (cell, _source) = ctrl.finalize();
        let remaining = cell
            .map(|c| lines_to_plain_strings(&c.transcript_lines(u16::MAX)))
            .unwrap_or_default();
        let first_emit = lines_to_plain_strings(&first_emit).join("\n");
        let joined = remaining.join("\n");
        assert!(
            !joined.contains("https://example.com") && !joined.contains("](https://"),
            "raw suffix must not start inside already-visible link markup; emitted before toggle: {first_emit:?}, finalized after toggle: {joined:?}",
        );
        assert!(
            joined.contains("gamma") && joined.contains("tail line"),
            "raw suffix should preserve un-emitted source after the rendered link label; emitted before toggle: {first_emit:?}, finalized after toggle: {joined:?}",
        );
    }

    #[test]
    fn controller_clear_queue_advances_synthetic_accounting_before_finalize() {
        let mut ctrl = stream_controller(Some(/*width*/ 18));
        ctrl.push("alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu\n");
        ctrl.push("tail line\n");

        let (first_emit, idle) = ctrl.on_commit_tick();
        let first_emit = first_emit
            .expect("expected first wrapped line emission")
            .transcript_lines(u16::MAX);
        assert!(!idle, "expected remaining wrapped content after one tick");

        ctrl.set_width(Some(/*width*/ 80));
        assert!(
            ctrl.queued_lines() > 0,
            "expected synthetic queue after resizing a partially emitted source line",
        );

        ctrl.clear_queue();
        let (cell, _source) = ctrl.finalize();
        let remaining = cell
            .map(|c| lines_to_plain_strings(&c.transcript_lines(u16::MAX)))
            .unwrap_or_default();
        let first_emit = lines_to_plain_strings(&first_emit).join("\n");
        let joined = remaining.join("\n");
        assert!(
            !joined.contains("alpha beta"),
            "finalize must not replay the already-visible prefix after clear_queue; emitted before clear: {first_emit:?}, finalized after clear: {joined:?}",
        );
        assert!(
            joined.contains("lambda mu"),
            "finalize must preserve the un-emitted source-line suffix after clear_queue; emitted before clear: {first_emit:?}, finalized after clear: {joined:?}",
        );
        assert!(
            remaining.iter().any(|line| line.contains("tail line")),
            "ordinary mutable tail should still flush on finalize after clear_queue; got {remaining:?}",
        );
    }

    #[test]
    fn trim_rendered_prefix_preserves_suffix_metadata() {
        let line = HyperlinkLine {
            line: vec!["alpha ".into(), "beta".bold(), " gamma".into()].into(),
            hyperlinks: vec![TerminalHyperlink {
                columns: 6..10,
                destination: "https://example.com".to_string(),
            }],
        };

        let trimmed = trim_rendered_prefix(vec![line], &[HyperlinkLine::from("alpha ")]);

        assert_eq!(
            hyperlink_lines_to_plain_strings(&trimmed),
            vec!["beta gamma"]
        );
        assert_eq!(trimmed[0].hyperlinks[0].columns, 0..4);
        assert_eq!(trimmed[0].hyperlinks[0].destination, "https://example.com");
        assert!(
            trimmed[0].line.spans[0]
                .style
                .add_modifier
                .contains(ratatui::style::Modifier::BOLD),
            "expected trimmed suffix to preserve span styling",
        );
    }
}
