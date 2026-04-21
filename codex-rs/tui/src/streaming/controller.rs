//! Streams markdown deltas while retaining source for later transcript reflow.
//!
//! Streaming has two outputs with different lifetimes. The live viewport needs incremental
//! `HistoryCell`s so the user sees progress, while finalized transcript history needs raw markdown
//! source so it can be rendered again after a terminal resize. These controllers keep those outputs
//! tied together: newline-complete source is rendered into queued live cells, and finalization
//! returns the accumulated source to the app for consolidation.
//!
//! Width changes are handled by re-rendering from source and rebuilding only the not-yet-emitted
//! queue. Already emitted rows stay emitted until the app-level transcript reflow rebuilds the full
//! scrollback from finalized cells.

use crate::history_cell::HistoryCell;
use crate::history_cell::{self};
use crate::markdown::append_markdown;
use crate::render::line_utils::prefix_lines;
use crate::style::proposed_plan_style;
use ratatui::prelude::Stylize;
use ratatui::text::Line;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use std::time::Instant;

use super::StreamState;

struct StreamCore {
    state: StreamState,
    width: Option<usize>,
    raw_source: String,
    rendered_lines: Vec<Line<'static>>,
    enqueued_len: usize,
    emitted_len: usize,
    cwd: PathBuf,
}

impl StreamCore {
    fn new(width: Option<usize>, cwd: &Path) -> Self {
        Self {
            state: StreamState::new(width, cwd),
            width,
            raw_source: String::with_capacity(1024),
            rendered_lines: Vec::with_capacity(64),
            enqueued_len: 0,
            emitted_len: 0,
            cwd: cwd.to_path_buf(),
        }
    }

    fn push_delta(&mut self, delta: &str) -> bool {
        if !delta.is_empty() {
            self.state.has_seen_delta = true;
        }
        self.state.collector.push_delta(delta);

        if delta.contains('\n')
            && let Some(committed_source) = self.state.collector.commit_complete_source()
        {
            self.raw_source.push_str(&committed_source);
            self.recompute_render();
            return self.sync_queue_to_render();
        }

        false
    }

    fn finalize_remaining(&mut self) -> Vec<Line<'static>> {
        let remainder_source = self.state.collector.finalize_and_drain_source();
        if !remainder_source.is_empty() {
            self.raw_source.push_str(&remainder_source);
        }

        let mut rendered = Vec::new();
        append_markdown(
            &self.raw_source,
            self.width,
            Some(self.cwd.as_path()),
            &mut rendered,
        );
        if self.emitted_len >= rendered.len() {
            Vec::new()
        } else {
            rendered[self.emitted_len..].to_vec()
        }
    }

    fn tick(&mut self) -> Vec<Line<'static>> {
        let step = self.state.step();
        self.emitted_len += step.len();
        step
    }

    fn tick_batch(&mut self, max_lines: usize) -> Vec<Line<'static>> {
        if max_lines == 0 {
            return Vec::new();
        }
        let step = self.state.drain_n(max_lines);
        self.emitted_len += step.len();
        step
    }

    fn queued_lines(&self) -> usize {
        self.state.queued_len()
    }

    fn oldest_queued_age(&self, now: Instant) -> Option<Duration> {
        self.state.oldest_queued_age(now)
    }

    fn is_idle(&self) -> bool {
        self.state.is_idle()
    }

    fn set_width(&mut self, width: Option<usize>) {
        if self.width == width {
            return;
        }

        let had_pending_queue = self.state.queued_len() > 0;
        self.width = width;
        self.state.collector.set_width(width);
        if self.raw_source.is_empty() {
            return;
        }

        self.recompute_render();
        self.emitted_len = self.emitted_len.min(self.rendered_lines.len());
        if had_pending_queue
            && self.emitted_len == self.rendered_lines.len()
            && self.emitted_len > 0
        {
            // If wrapped remainder compresses into fewer lines at the new width,
            // keep at least one line un-emitted so pre-resize pending content is
            // not skipped permanently.
            self.emitted_len -= 1;
        }

        self.state.clear_queue();
        if self.emitted_len > 0 && !had_pending_queue {
            self.enqueued_len = self.rendered_lines.len();
            return;
        }
        self.rebuild_queue_from_render();
    }

    fn clear_queue(&mut self) {
        self.state.clear_queue();
        self.enqueued_len = self.emitted_len;
    }

    fn reset(&mut self) {
        self.state.clear();
        self.raw_source.clear();
        self.rendered_lines.clear();
        self.enqueued_len = 0;
        self.emitted_len = 0;
    }

    fn recompute_render(&mut self) {
        self.rendered_lines.clear();
        append_markdown(
            &self.raw_source,
            self.width,
            Some(self.cwd.as_path()),
            &mut self.rendered_lines,
        );
    }

    fn sync_queue_to_render(&mut self) -> bool {
        let target_len = self.rendered_lines.len().max(self.emitted_len);
        if target_len < self.enqueued_len {
            self.rebuild_queue_from_render();
            return self.state.queued_len() > 0;
        }

        if target_len == self.enqueued_len {
            return false;
        }

        self.state
            .enqueue(self.rendered_lines[self.enqueued_len..target_len].to_vec());
        self.enqueued_len = target_len;
        true
    }

    fn rebuild_queue_from_render(&mut self) {
        self.state.clear_queue();
        let target_len = self.rendered_lines.len().max(self.emitted_len);
        if self.emitted_len < target_len {
            self.state
                .enqueue(self.rendered_lines[self.emitted_len..target_len].to_vec());
        }
        self.enqueued_len = target_len;
    }
}

/// Controls newline-gated streaming for assistant messages.
///
/// The controller emits transient `AgentMessageCell`s for live display and returns raw markdown
/// source on `finalize` so the app can replace those transient cells with a source-backed
/// `AgentMarkdownCell`. Callers should use `set_width` on terminal resize; rebuilding the queue
/// from already emitted cells would duplicate output instead of preserving the stream position.
pub(crate) struct StreamController {
    core: StreamCore,
    header_emitted: bool,
}

impl StreamController {
    /// Create a stream controller that renders markdown relative to the given width and cwd.
    ///
    /// `width` is the content width available to markdown rendering, not necessarily the full
    /// terminal width. Passing a stale width after resize will keep queued live output wrapped for
    /// the old viewport until app-level reflow repairs the finalized transcript.
    pub(crate) fn new(width: Option<usize>, cwd: &Path) -> Self {
        Self {
            core: StreamCore::new(width, cwd),
            header_emitted: false,
        }
    }

    /// Push a raw model delta and return whether it produced queued complete lines.
    ///
    /// Deltas are committed only through newline boundaries. A `false` return can still mean source
    /// was buffered; it only means no newly renderable complete line is ready for live emission.
    pub(crate) fn push(&mut self, delta: &str) -> bool {
        self.core.push_delta(delta)
    }

    /// Finish the stream and return the final transient cell plus accumulated markdown source.
    ///
    /// The source is `None` only when the stream never accumulated content. Callers that discard the
    /// returned source cannot later consolidate the transcript into a width-sensitive finalized
    /// cell.
    pub(crate) fn finalize(&mut self) -> (Option<Box<dyn HistoryCell>>, Option<String>) {
        let remaining = self.core.finalize_remaining();
        if self.core.raw_source.is_empty() {
            self.core.reset();
            return (None, None);
        }

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

    pub(crate) fn queued_lines(&self) -> usize {
        self.core.queued_lines()
    }

    pub(crate) fn oldest_queued_age(&self, now: Instant) -> Option<Duration> {
        self.core.oldest_queued_age(now)
    }

    pub(crate) fn clear_queue(&mut self) {
        self.core.clear_queue();
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

/// Controls newline-gated streaming for proposed plan markdown.
///
/// This follows the same source-retention contract as `StreamController`, but wraps emitted lines
/// in the proposed-plan header, padding, and style. Finalization must return source for
/// `ProposedPlanCell`; otherwise a resized finalized plan would keep the transient stream shape.
pub(crate) struct PlanStreamController {
    core: StreamCore,
    header_emitted: bool,
    top_padding_emitted: bool,
}

impl PlanStreamController {
    /// Create a proposed-plan stream controller that renders markdown relative to the given cwd.
    ///
    /// The width has the same meaning as in `StreamController`: it is the markdown body width, and
    /// callers must update it when the terminal width changes.
    pub(crate) fn new(width: Option<usize>, cwd: &Path) -> Self {
        Self {
            core: StreamCore::new(width, cwd),
            header_emitted: false,
            top_padding_emitted: false,
        }
    }

    /// Push a raw proposed-plan delta and return whether it produced queued complete lines.
    ///
    /// Source may be buffered even when this returns `false`; callers should continue ticking only
    /// when queued lines exist.
    pub(crate) fn push(&mut self, delta: &str) -> bool {
        self.core.push_delta(delta)
    }

    /// Finish the plan stream and return the final transient cell plus accumulated markdown source.
    ///
    /// The returned source is consumed by app-level consolidation to create the source-backed
    /// `ProposedPlanCell` used for later resize reflow.
    pub(crate) fn finalize(&mut self) -> (Option<Box<dyn HistoryCell>>, Option<String>) {
        let remaining = self.core.finalize_remaining();
        if self.core.raw_source.is_empty() {
            self.core.reset();
            return (None, None);
        }

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

    pub(crate) fn queued_lines(&self) -> usize {
        self.core.queued_lines()
    }

    pub(crate) fn oldest_queued_age(&self, now: Instant) -> Option<Duration> {
        self.core.oldest_queued_age(now)
    }

    pub(crate) fn clear_queue(&mut self) {
        self.core.clear_queue();
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

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn test_cwd() -> PathBuf {
        std::env::temp_dir()
    }

    fn stream_controller(width: Option<usize>) -> StreamController {
        StreamController::new(width, &test_cwd())
    }

    fn plan_stream_controller(width: Option<usize>) -> PlanStreamController {
        PlanStreamController::new(width, &test_cwd())
    }

    fn lines_to_plain_strings(lines: &[Line<'_>]) -> Vec<String> {
        lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.clone())
                    .collect::<String>()
            })
            .collect()
    }

    fn collect_streamed_lines(deltas: &[&str], width: Option<usize>) -> Vec<String> {
        let mut ctrl = stream_controller(width);
        let mut lines = Vec::new();
        for delta in deltas {
            ctrl.push(delta);
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
            .map(|line| line.chars().skip(2).collect::<String>())
            .collect()
    }

    fn collect_plan_streamed_lines(deltas: &[&str], width: Option<usize>) -> Vec<String> {
        let mut ctrl = plan_stream_controller(width);
        let mut lines = Vec::new();
        for delta in deltas {
            ctrl.push(delta);
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
        let mut ctrl = stream_controller(Some(120));
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
        let mut ctrl = stream_controller(Some(80));
        assert!(ctrl.push("line one\n"));
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
    fn controller_finalize_returns_raw_source_for_consolidation() {
        let mut ctrl = stream_controller(Some(80));
        assert!(ctrl.push("hello\n"));
        let (_cell, source) = ctrl.finalize();
        assert_eq!(source, Some("hello\n".to_string()));
    }

    #[test]
    fn plan_controller_finalize_returns_raw_source_for_consolidation() {
        let mut ctrl = plan_stream_controller(Some(80));
        assert!(ctrl.push("- step\n"));
        let (_cell, source) = ctrl.finalize();
        assert_eq!(source, Some("- step\n".to_string()));
    }

    #[test]
    fn simple_lines_stream_in_order() {
        let actual = collect_streamed_lines(&["hello\n", "world\n"], Some(80));
        assert_eq!(actual, vec!["hello".to_string(), "world".to_string()]);
    }

    #[test]
    fn plan_lines_stream_in_order() {
        let actual = collect_plan_streamed_lines(&["- one\n", "- two\n"], Some(80));
        assert!(
            actual.iter().any(|line| line.contains("Proposed Plan")),
            "expected plan header in streamed plan: {actual:?}",
        );
        assert!(
            actual.iter().any(|line| line.contains("one")),
            "expected plan body in streamed plan: {actual:?}",
        );
    }
}
