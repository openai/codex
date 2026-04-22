//! Connects terminal resize events to source-backed transcript scrollback rebuilds.
//!
//! The app stores conversation history as `HistoryCell`s, but it also writes finalized history into
//! terminal scrollback for the normal chat view. When the terminal width changes, this module uses
//! the stored cells as source, clears the Codex-owned terminal history, and re-emits the transcript
//! for the new terminal size.
//!
//! Streaming output is the fragile part of this lifecycle. Active streams first appear as transient
//! stream cells, then consolidate into source-backed finalized cells. Resize work that happens
//! before consolidation is marked as stream-time work so consolidation can force one final rebuild
//! from the finalized source.

use std::collections::VecDeque;
use std::time::Duration;
use std::time::Instant;

use codex_features::Feature;
use color_eyre::eyre::Result;
use ratatui::text::Line;

use super::App;
use super::trailing_run_start;
use crate::history_cell;
use crate::history_cell::HistoryCell;
use crate::transcript_reflow::ResizeReflowDisableReason;
use crate::transcript_reflow::TRANSCRIPT_REFLOW_DEBOUNCE;
use crate::transcript_reflow::TranscriptReflowKind;
use crate::tui;

#[derive(Debug, Clone, Copy)]
pub(super) struct ResizeReflowRunStats {
    kind: TranscriptReflowKind,
    width: u16,
    cell_count: usize,
    rendered_line_count: usize,
    row_cap_limited: bool,
}

struct ReflowCellDisplay {
    lines: Vec<Line<'static>>,
    is_stream_continuation: bool,
}

pub(super) struct ReflowRenderResult {
    pub(super) lines: Vec<Line<'static>>,
    pub(super) rendered_cell_count: usize,
    pub(super) row_cap_limited: bool,
}

impl App {
    pub(super) fn reset_history_emission_state(&mut self) {
        self.has_emitted_history_lines = false;
        self.deferred_history_lines.clear();
    }

    fn display_lines_for_history_insert(
        &mut self,
        cell: &dyn HistoryCell,
        width: u16,
    ) -> Vec<Line<'static>> {
        let mut display = cell.display_lines(width);
        if !display.is_empty() && !cell.is_stream_continuation() {
            if self.has_emitted_history_lines {
                display.insert(0, Line::from(""));
            } else {
                self.has_emitted_history_lines = true;
            }
        }
        display
    }

    pub(super) fn insert_history_cell_lines(
        &mut self,
        tui: &mut tui::Tui,
        cell: &dyn HistoryCell,
        width: u16,
    ) {
        let display = self.display_lines_for_history_insert(cell, width);
        if display.is_empty() {
            return;
        }
        if self.overlay.is_some() {
            self.deferred_history_lines.extend(display);
        } else {
            tui.insert_history_lines(display);
        }
    }

    pub(super) fn terminal_resize_reflow_enabled(&self) -> bool {
        self.config.features.enabled(Feature::TerminalResizeReflow)
    }

    pub(super) fn terminal_resize_reflow_active(&self) -> bool {
        self.terminal_resize_reflow_enabled() && !self.transcript_reflow.is_runtime_disabled()
    }

    fn schedule_resize_reflow(
        &mut self,
        kind: TranscriptReflowKind,
        target_width: Option<u16>,
    ) -> bool {
        debug_assert!(self.terminal_resize_reflow_active());
        self.transcript_reflow
            .schedule_debounced(kind, target_width)
    }

    fn resize_reflow_slow_threshold(&self) -> Option<Duration> {
        self.config.terminal_resize_reflow.slow_threshold
    }

    fn resize_reflow_max_rows(&self) -> usize {
        self.config.terminal_resize_reflow.max_rows
    }

    pub(super) fn maybe_disable_slow_resize_reflow_flush(
        &mut self,
        stats: tui::ResizeReflowDrawStats,
    ) {
        let Some(threshold) = self.resize_reflow_slow_threshold() else {
            return;
        };
        if !stats.flushed_reflow_history
            || !self.transcript_reflow.record_elapsed(
                ResizeReflowDisableReason::FlushSlow,
                stats.reflow_flush_elapsed,
                threshold,
            )
        {
            return;
        }

        tracing::warn!(
            elapsed_ms = stats.reflow_flush_elapsed.as_millis(),
            threshold_ms = threshold.as_millis(),
            reason = ?self.transcript_reflow.runtime_disabled_reason(),
            "terminal resize reflow disabled for current transcript after slow terminal flush"
        );
        self.maybe_show_slow_resize_reflow_disabled_warning(stats.reflow_flush_elapsed, threshold);
    }

    fn maybe_disable_slow_resize_reflow_render(
        &mut self,
        elapsed: Duration,
        stats: ResizeReflowRunStats,
    ) {
        let Some(threshold) = self.resize_reflow_slow_threshold() else {
            return;
        };
        if !self.transcript_reflow.record_elapsed(
            ResizeReflowDisableReason::RenderSlow,
            elapsed,
            threshold,
        ) {
            return;
        }

        tracing::warn!(
            elapsed_ms = elapsed.as_millis(),
            threshold_ms = threshold.as_millis(),
            kind = ?stats.kind,
            width = stats.width,
            cell_count = stats.cell_count,
            rendered_line_count = stats.rendered_line_count,
            reason = ?self.transcript_reflow.runtime_disabled_reason(),
            "terminal resize reflow disabled for current transcript after slow render"
        );
        self.maybe_show_slow_resize_reflow_disabled_warning(elapsed, threshold);
    }

    fn maybe_note_row_cap_limited_reflow(&mut self, stats: ResizeReflowRunStats) {
        if !stats.row_cap_limited {
            return;
        }

        let max_rows = self.resize_reflow_max_rows();
        tracing::warn!(
            max_rows,
            kind = ?stats.kind,
            width = stats.width,
            cell_count = stats.cell_count,
            rendered_line_count = stats.rendered_line_count,
            "terminal resize reflow limited scrollback with rendered row cap"
        );
        self.maybe_show_row_cap_resize_reflow_trimmed_warning(stats.rendered_line_count);
    }

    fn maybe_show_slow_resize_reflow_disabled_warning(
        &mut self,
        elapsed: Duration,
        threshold: Duration,
    ) {
        if self.transcript_reflow.take_runtime_disable_warning_needed() {
            self.chat_widget.add_info_message(
                format!(
                    "Terminal resize reflow paused for this session because resizing took {}ms, above the {}ms limit. Future resizes will use legacy terminal behavior until the transcript is cleared.",
                    elapsed.as_millis(),
                    threshold.as_millis()
                ),
                /*hint*/ None,
            );
        }
    }

    fn maybe_show_row_cap_resize_reflow_trimmed_warning(&mut self, kept_line_count: usize) {
        if self.transcript_reflow.take_row_cap_trim_warning_needed() {
            self.chat_widget.add_info_message(
                format!(
                    "Terminal resize reflow limited scrollback to the most recent {kept_line_count} rows before rendering because the row cap is {}.",
                    self.resize_reflow_max_rows(),
                ),
                /*hint*/ None,
            );
        }
    }

    fn show_resize_reflow_timing_debug_message(
        &mut self,
        elapsed: Duration,
        stats: ResizeReflowRunStats,
    ) {
        let kind = match stats.kind {
            TranscriptReflowKind::VisibleRows => "visible-row",
            TranscriptReflowKind::Full => "full",
        };
        self.chat_widget.add_info_message(
            format!(
                "Terminal resize reflow {kind} pass took {}ms ({} rows from {} cells).",
                elapsed.as_millis(),
                stats.rendered_line_count,
                stats.cell_count
            ),
            /*hint*/ None,
        );
    }

    /// Finish stream consolidation by repairing any resize work that happened during streaming.
    ///
    /// This is called after agent-message stream cells have either been replaced by an
    /// `AgentMarkdownCell` or found to need no replacement. If a resize happened while the stream
    /// was active or while its transient cells were still present, this method runs an immediate
    /// source-backed reflow so terminal scrollback reflects the finalized cell instead of the
    /// transient stream rows.
    pub(super) fn maybe_finish_stream_reflow(&mut self, tui: &mut tui::Tui) -> Result<()> {
        if !self.terminal_resize_reflow_enabled() {
            self.transcript_reflow.clear();
            return Ok(());
        }
        if !self.terminal_resize_reflow_active() {
            return Ok(());
        }

        if self.transcript_reflow.take_stream_finish_reflow_needed() {
            self.schedule_immediate_resize_reflow(tui);
            self.maybe_run_resize_reflow(tui)?;
        } else if self.transcript_reflow.pending_is_due(Instant::now()) {
            tui.frame_requester().schedule_frame();
        }
        Ok(())
    }

    fn schedule_immediate_resize_reflow(&mut self, tui: &mut tui::Tui) {
        if !self.terminal_resize_reflow_enabled() {
            self.transcript_reflow.clear();
            return;
        }
        if !self.terminal_resize_reflow_active() {
            return;
        }
        self.transcript_reflow
            .schedule_immediate(TranscriptReflowKind::Full);
        tui.frame_requester().schedule_frame();
    }

    /// Force stream-finalized output through the resize reflow path.
    ///
    /// Proposed plan consolidation uses this stricter path because a completed plan is inserted or
    /// replaced as one styled source-backed cell. If this reflow is skipped after a stream-time
    /// resize, the visible scrollback can keep the pre-consolidation wrapping.
    pub(super) fn finish_required_stream_reflow(&mut self, tui: &mut tui::Tui) -> Result<()> {
        if !self.terminal_resize_reflow_enabled() {
            self.transcript_reflow.clear();
            return Ok(());
        }
        if !self.terminal_resize_reflow_active() {
            return Ok(());
        }
        self.schedule_immediate_resize_reflow(tui);
        self.maybe_run_resize_reflow(tui)?;
        if !self.transcript_reflow.has_pending_reflow() {
            self.transcript_reflow.clear_stream_flags();
        }
        Ok(())
    }

    /// Record terminal size changes and schedule any resize-sensitive transcript work.
    ///
    /// Width changes need a full rebuild because transcript wrapping changes. Height growth only
    /// needs a visible-row repaint: a tmux split can remove rows from the visible pane, and closing
    /// that split can expose blank or shifted rows even when the inline viewport's logical position
    /// did not move. The first observed width initializes resize tracking without scheduling a
    /// rebuild, because there is no previously emitted width to repair yet.
    pub(super) fn handle_draw_size_change(
        &mut self,
        size: ratatui::layout::Size,
        last_known_screen_size: ratatui::layout::Size,
        frame_requester: &tui::FrameRequester,
    ) -> bool {
        let width = self.transcript_reflow.note_width(size.width);
        let full_reflow_needed = self
            .transcript_reflow
            .full_reflow_needed_for_width(size.width);
        let height_growth_exposes_rows = size.height > last_known_screen_size.height;
        let should_rebuild_transcript = full_reflow_needed || height_growth_exposes_rows;
        if width.changed || width.initialized {
            self.chat_widget.on_terminal_resize(size.width);
        }
        if should_rebuild_transcript {
            if self.terminal_resize_reflow_active() {
                let reflow_kind = if full_reflow_needed {
                    TranscriptReflowKind::Full
                } else {
                    TranscriptReflowKind::VisibleRows
                };
                if matches!(reflow_kind, TranscriptReflowKind::Full)
                    && self.should_mark_reflow_as_stream_time()
                {
                    self.transcript_reflow.mark_resize_requested_during_stream();
                }
                let target_width = match reflow_kind {
                    TranscriptReflowKind::Full => Some(size.width),
                    TranscriptReflowKind::VisibleRows => None,
                };
                if self.schedule_resize_reflow(reflow_kind, target_width) {
                    frame_requester.schedule_frame();
                } else {
                    frame_requester.schedule_frame_in(TRANSCRIPT_REFLOW_DEBOUNCE);
                }
            } else if !self.terminal_resize_reflow_enabled() && width.changed {
                self.transcript_reflow.clear();
            }
        }
        if size != last_known_screen_size {
            self.refresh_status_line();
        }
        if self.terminal_resize_reflow_active() {
            self.maybe_clear_resize_reflow_without_terminal();
        }
        should_rebuild_transcript
    }

    fn maybe_clear_resize_reflow_without_terminal(&mut self) {
        if !self.terminal_resize_reflow_enabled() {
            self.transcript_reflow.clear();
            return;
        }
        if !self.terminal_resize_reflow_active() {
            return;
        }
        let Some(deadline) = self.transcript_reflow.pending_until() else {
            return;
        };
        if Instant::now() < deadline || self.overlay.is_some() || !self.transcript_cells.is_empty()
        {
            return;
        }

        self.transcript_reflow.clear_pending_reflow();
        self.reset_history_emission_state();
    }

    pub(super) fn handle_draw_pre_render(&mut self, tui: &mut tui::Tui) -> Result<()> {
        let size = tui.terminal.size()?;
        let should_rebuild_transcript = self.handle_draw_size_change(
            size,
            tui.terminal.last_known_screen_size,
            &tui.frame_requester(),
        );
        if should_rebuild_transcript && self.terminal_resize_reflow_active() {
            // Resize-sensitive history inserts queued before this frame may be wrapped for the old
            // viewport or targeted at rows no longer visible. Drop them and let resize reflow
            // rebuild from transcript cells.
            tui.clear_pending_history_lines();
        }
        self.maybe_run_resize_reflow(tui)?;
        Ok(())
    }

    /// Run a pending transcript reflow when its debounce deadline has arrived.
    ///
    /// Reflow is deferred while an overlay is active because the overlay owns the current draw
    /// surface. Callers must keep using `HistoryCell` source as the rebuild input; attempting to
    /// reuse terminal-wrapped output here would preserve exactly the stale wrapping this feature is
    /// meant to remove.
    pub(super) fn maybe_run_resize_reflow(&mut self, tui: &mut tui::Tui) -> Result<()> {
        if !self.terminal_resize_reflow_enabled() {
            self.transcript_reflow.clear();
            return Ok(());
        }
        if !self.terminal_resize_reflow_active() {
            return Ok(());
        }
        let Some(deadline) = self.transcript_reflow.pending_until() else {
            return Ok(());
        };
        let now = Instant::now();
        if now < deadline {
            // Later resize events push the reflow deadline out, while the frame scheduler coalesces
            // delayed draws to the earliest requested instant. If an early draw arrives before the
            // latest quiet-period deadline, re-arm the draw so the pending reflow cannot get stuck
            // until the next keypress.
            tui.frame_requester().schedule_frame_in(deadline - now);
            return Ok(());
        }
        if self.overlay.is_some() {
            return Ok(());
        }

        let reflow_kind = self
            .transcript_reflow
            .pending_kind()
            .unwrap_or(TranscriptReflowKind::Full);
        self.transcript_reflow.clear_pending_reflow();

        // Track that a reflow happened during an active stream or while trailing
        // unconsolidated AgentMessageCells are still pending consolidation so
        // ConsolidateAgentMessage can schedule a follow-up reflow.
        let reflow_ran_during_stream =
            !self.transcript_cells.is_empty() && self.should_mark_reflow_as_stream_time();

        let started = Instant::now();
        let stats = match reflow_kind {
            TranscriptReflowKind::Full => self.reflow_transcript_now(tui, reflow_kind)?,
            TranscriptReflowKind::VisibleRows => {
                self.repaint_visible_transcript_rows(tui, reflow_kind)?
            }
        };
        let elapsed = started.elapsed();
        self.show_resize_reflow_timing_debug_message(elapsed, stats);
        self.maybe_disable_slow_resize_reflow_render(elapsed, stats);
        self.transcript_reflow.mark_reflowed_width(stats.width);

        if reflow_ran_during_stream && !self.transcript_reflow.is_runtime_disabled() {
            self.transcript_reflow.mark_ran_during_stream();
        }
        if !self.transcript_reflow.is_runtime_disabled() {
            // Some terminals settle their final reported width after the repaint that handled the
            // last resize event. Request one cheap follow-up draw so `handle_draw_pre_render` can
            // sample that width and schedule a final reflow if needed.
            tui.frame_requester()
                .schedule_frame_in(TRANSCRIPT_REFLOW_DEBOUNCE);
        }

        Ok(())
    }

    fn reflow_transcript_now(
        &mut self,
        tui: &mut tui::Tui,
        kind: TranscriptReflowKind,
    ) -> Result<ResizeReflowRunStats> {
        let width = tui.terminal.size()?.width;
        let cell_count = self.transcript_cells.len();
        if self.transcript_cells.is_empty() {
            // Drop any queued pre-resize/pre-consolidation inserts before rebuilding from cells.
            tui.clear_pending_history_lines();
            self.reset_history_emission_state();
            return Ok(ResizeReflowRunStats {
                kind,
                width,
                cell_count,
                rendered_line_count: 0,
                row_cap_limited: false,
            });
        }

        let reflow_result = self.render_transcript_lines_for_reflow(width);
        let reflowed_lines = reflow_result.lines;
        let stats = ResizeReflowRunStats {
            kind,
            width,
            cell_count: reflow_result.rendered_cell_count,
            rendered_line_count: reflowed_lines.len(),
            row_cap_limited: reflow_result.row_cap_limited,
        };
        self.maybe_note_row_cap_limited_reflow(stats);

        // Drop any queued pre-resize/pre-consolidation inserts before rebuilding from cells.
        tui.clear_pending_history_lines();
        if tui.is_alt_screen_active() {
            tui.terminal.clear_visible_screen()?;
        } else {
            tui.terminal.clear_scrollback_and_visible_screen_ansi()?;
        }

        self.deferred_history_lines.clear();
        if !reflowed_lines.is_empty() {
            tui.insert_reflowed_history_lines(reflowed_lines);
        }

        Ok(stats)
    }

    fn repaint_visible_transcript_rows(
        &mut self,
        tui: &mut tui::Tui,
        kind: TranscriptReflowKind,
    ) -> Result<ResizeReflowRunStats> {
        let width = tui.terminal.size()?.width;
        let cell_count = self.transcript_cells.len();
        if self.transcript_cells.is_empty() {
            tui.clear_pending_history_lines();
            self.reset_history_emission_state();
            return Ok(ResizeReflowRunStats {
                kind,
                width,
                cell_count,
                rendered_line_count: 0,
                row_cap_limited: false,
            });
        }

        let reflow_result = self.render_transcript_lines_for_reflow(width);
        let reflowed_lines = reflow_result.lines;
        let stats = ResizeReflowRunStats {
            kind,
            width,
            cell_count: reflow_result.rendered_cell_count,
            rendered_line_count: reflowed_lines.len(),
            row_cap_limited: reflow_result.row_cap_limited,
        };
        self.maybe_note_row_cap_limited_reflow(stats);

        tui.clear_pending_history_lines();
        tui.terminal.clear_visible_screen()?;
        self.deferred_history_lines.clear();
        if !reflowed_lines.is_empty() {
            tui.insert_reflowed_history_lines(reflowed_lines);
        }

        Ok(stats)
    }

    pub(super) fn render_transcript_lines_for_reflow(&mut self, width: u16) -> ReflowRenderResult {
        let row_cap = (self.resize_reflow_max_rows() > 0).then_some(self.resize_reflow_max_rows());
        let mut cell_displays = VecDeque::new();
        let mut rendered_rows = 0usize;
        let mut start = self.transcript_cells.len();

        while start > 0 {
            start -= 1;
            let cell = self.transcript_cells[start].clone();
            let lines = cell.display_lines(width);
            rendered_rows += lines.len();
            cell_displays.push_front(ReflowCellDisplay {
                lines,
                is_stream_continuation: cell.is_stream_continuation(),
            });

            if row_cap.is_some_and(|max_rows| rendered_rows > max_rows) {
                break;
            }
        }

        while start > 0
            && cell_displays
                .front()
                .is_some_and(|display| display.is_stream_continuation)
        {
            start -= 1;
            let cell = self.transcript_cells[start].clone();
            cell_displays.push_front(ReflowCellDisplay {
                lines: cell.display_lines(width),
                is_stream_continuation: cell.is_stream_continuation(),
            });
        }

        let rendered_cell_count = cell_displays.len();
        let mut has_emitted_history_lines = false;
        let mut reflowed_lines = Vec::new();
        for display in cell_displays {
            if !display.lines.is_empty() && !display.is_stream_continuation {
                if has_emitted_history_lines {
                    reflowed_lines.push(Line::from(""));
                } else {
                    has_emitted_history_lines = true;
                }
            }
            reflowed_lines.extend(display.lines);
        }
        let row_cap_limited =
            row_cap.is_some_and(|max_rows| start > 0 || reflowed_lines.len() > max_rows);
        if let Some(max_rows) = row_cap
            && reflowed_lines.len() > max_rows
        {
            let trimmed_line_count = reflowed_lines.len() - max_rows;
            reflowed_lines = reflowed_lines.split_off(trimmed_line_count);
        }
        self.has_emitted_history_lines = !reflowed_lines.is_empty();

        ReflowRenderResult {
            lines: reflowed_lines,
            rendered_cell_count,
            row_cap_limited,
        }
    }

    /// Return whether current transcript state should be treated as stream-time resize state.
    ///
    /// The active stream controllers cover normal streaming. The trailing-cell checks cover the
    /// narrow window after a controller has stopped but before the app has processed the
    /// consolidation event that replaces transient stream cells with source-backed cells.
    pub(super) fn should_mark_reflow_as_stream_time(&self) -> bool {
        self.chat_widget.has_active_agent_stream()
            || self.chat_widget.has_active_plan_stream()
            || trailing_run_start::<history_cell::AgentMessageCell>(&self.transcript_cells)
                < self.transcript_cells.len()
            || trailing_run_start::<history_cell::ProposedPlanStreamCell>(&self.transcript_cells)
                < self.transcript_cells.len()
    }
}
