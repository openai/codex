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
//!
//! The row cap is enforced while rendering from `HistoryCell` source, not after writing to the
//! terminal. Initial resume replay uses the same display-line buffering contract so large sessions
//! do not write more retained rows than resize replay would later be willing to rebuild.

use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Instant;

use codex_features::Feature;
use color_eyre::eyre::Result;
use ratatui::text::Line;

use super::App;
use super::InitialHistoryReplayBuffer;
use crate::history_cell;
use crate::history_cell::HistoryCell;
use crate::insert_history::HistoryLineWrapPolicy;
use crate::terminal_hyperlinks::HyperlinkLine;
use crate::transcript_reflow::TRANSCRIPT_REFLOW_DEBOUNCE;
use crate::tui;

struct ReflowCellDisplay {
    lines: Vec<HyperlinkLine>,
    is_stream_continuation: bool,
}

/// Rendered transcript lines ready to be replayed into terminal scrollback.
///
/// This is intentionally line-oriented rather than cell-oriented because the terminal only accepts
/// already-wrapped rows. Callers should keep treating `transcript_cells` as the source of truth; the
/// rows here are a transient render product for a single terminal width.
pub(super) struct ReflowRenderResult {
    pub(super) lines: Vec<HyperlinkLine>,
}

pub(super) fn trailing_run_start<T: 'static>(transcript_cells: &[Arc<dyn HistoryCell>]) -> usize {
    let end = transcript_cells.len();
    let mut start = end;

    while start > 0
        && transcript_cells[start - 1].is_stream_continuation()
        && transcript_cells[start - 1].as_any().is::<T>()
    {
        start -= 1;
    }

    if start > 0
        && transcript_cells[start - 1].as_any().is::<T>()
        && !transcript_cells[start - 1].is_stream_continuation()
    {
        start -= 1;
    }

    start
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
    ) -> Vec<HyperlinkLine> {
        let mut display =
            cell.display_hyperlink_lines_for_mode(width, self.chat_widget.history_render_mode());
        if !display.is_empty() && !cell.is_stream_continuation() {
            if self.has_emitted_history_lines {
                display.insert(/*index*/ 0, HyperlinkLine::new(Line::from("")));
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
            tui.insert_history_hyperlink_lines_with_wrap_policy(
                display,
                self.history_line_wrap_policy(),
            );
        }
    }

    pub(super) fn terminal_resize_reflow_enabled(&self) -> bool {
        self.config.features.enabled(Feature::TerminalResizeReflow)
    }

    /// Defer resume or thread-switch history writes until replay has rebuilt `transcript_cells`.
    pub(super) fn begin_history_replay(&mut self) {
        if self.terminal_resize_reflow_enabled() && self.overlay.is_none() {
            self.transcript_reflow.clear_pending_reflow();
            self.initial_history_replay_buffer = Some(InitialHistoryReplayBuffer {
                awaiting_session_header: true,
                ..Default::default()
            });
        }
    }

    /// Fold stream-consolidation repair into the source-backed render that ends history replay.
    pub(super) fn absorb_reflow_into_history_replay(&mut self) -> bool {
        if self.initial_history_replay_buffer.is_none() {
            return false;
        }

        self.transcript_reflow.clear_pending_reflow();
        self.transcript_reflow.clear_stream_flags();
        true
    }

    /// Render replayed history once from source after all replay events have been applied.
    pub(super) fn finish_history_replay(&mut self, tui: &mut tui::Tui) {
        let Some(replay) = self.initial_history_replay_buffer.take() else {
            return;
        };

        let terminal_width = tui.terminal.last_known_screen_size.width;
        let reflowed_lines = self.render_history_replay_lines(replay.source_start, terminal_width);
        if !reflowed_lines.is_empty() {
            tui.insert_history_hyperlink_lines_with_wrap_policy(
                reflowed_lines,
                self.history_line_wrap_policy(),
            );
        }
    }

    pub(super) fn render_history_replay_lines(
        &mut self,
        source_start: usize,
        terminal_width: u16,
    ) -> Vec<HyperlinkLine> {
        let width = self.chat_widget.history_wrap_width(terminal_width);
        self.render_transcript_lines_for_reflow_from(source_start, width)
            .lines
    }

    pub(crate) fn history_line_wrap_policy(&self) -> HistoryLineWrapPolicy {
        if self.chat_widget.raw_output_mode() {
            HistoryLineWrapPolicy::Terminal
        } else {
            HistoryLineWrapPolicy::PreWrap
        }
    }

    fn schedule_resize_reflow(&mut self, target_width: Option<u16>) -> bool {
        debug_assert!(self.terminal_resize_reflow_enabled());
        self.transcript_reflow.schedule_debounced(target_width)
    }

    fn resize_reflow_max_rows(&self) -> Option<usize> {
        crate::resize_reflow_cap::resize_reflow_max_rows(self.config.terminal_resize_reflow)
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
        if self.absorb_reflow_into_history_replay() {
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
        self.transcript_reflow.schedule_immediate();
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
        if self.absorb_reflow_into_history_replay() {
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
    /// Width changes need a rebuild because transcript wrapping changes. Height changes can expose,
    /// hide, or shift rows around the inline viewport, so they also rebuild from source-backed
    /// cells. The first observed width initializes resize tracking without scheduling a rebuild,
    /// because there is no previously emitted width to repair yet.
    pub(super) fn handle_draw_size_change(
        &mut self,
        size: ratatui::layout::Size,
        last_known_screen_size: ratatui::layout::Size,
        frame_requester: &tui::FrameRequester,
    ) -> bool {
        let width = self.transcript_reflow.note_width(size.width);
        let reflow_needed = self.transcript_reflow.reflow_needed_for_width(size.width);
        let height_changed = size.height != last_known_screen_size.height;
        let resize_requires_rebuild = reflow_needed || height_changed;
        let replay_in_progress = self.initial_history_replay_buffer.is_some();
        let should_rebuild_transcript = resize_requires_rebuild && !replay_in_progress;
        if width.changed || width.initialized {
            self.chat_widget.on_terminal_resize(size.width);
        }
        if should_rebuild_transcript {
            if self.terminal_resize_reflow_enabled() {
                if reflow_needed && self.should_mark_reflow_as_stream_time() {
                    self.transcript_reflow.mark_resize_requested_during_stream();
                }
                let target_width = reflow_needed.then_some(size.width);
                if self.schedule_resize_reflow(target_width) {
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
        if self.terminal_resize_reflow_enabled() {
            self.maybe_clear_resize_reflow_without_terminal();
        }
        should_rebuild_transcript
    }

    fn maybe_clear_resize_reflow_without_terminal(&mut self) {
        if !self.terminal_resize_reflow_enabled() {
            self.transcript_reflow.clear();
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
        if should_rebuild_transcript && self.terminal_resize_reflow_enabled() {
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

        self.transcript_reflow.clear_pending_reflow();

        // Track that a reflow happened during an active stream or while trailing
        // unconsolidated AgentMessageCells are still pending consolidation so
        // ConsolidateAgentMessage can schedule a follow-up reflow.
        let reflow_ran_during_stream =
            !self.transcript_cells.is_empty() && self.should_mark_reflow_as_stream_time();

        let width = self.reflow_transcript_now(tui)?;
        self.transcript_reflow.mark_reflowed_width(width);

        if reflow_ran_during_stream {
            self.transcript_reflow.mark_ran_during_stream();
        }
        // Some terminals settle their final reported width after the repaint that handled the
        // last resize event. Request one cheap follow-up draw so `handle_draw_pre_render` can
        // sample that width and schedule a final reflow if needed.
        tui.frame_requester()
            .schedule_frame_in(TRANSCRIPT_REFLOW_DEBOUNCE);

        Ok(())
    }

    pub(super) fn reflow_transcript_now(&mut self, tui: &mut tui::Tui) -> Result<u16> {
        let terminal_width = tui.terminal.size()?.width;
        let width = self.chat_widget.history_wrap_width(terminal_width);
        if self.transcript_cells.is_empty() {
            // Drop any queued pre-resize/pre-consolidation inserts before rebuilding from cells.
            tui.clear_pending_history_lines();
            self.reset_history_emission_state();
            return Ok(terminal_width);
        }

        let reflow_result = self.render_transcript_lines_for_reflow(width);
        let reflowed_lines = reflow_result.lines;

        // Drop any queued pre-resize/pre-consolidation inserts before rebuilding from cells.
        tui.clear_pending_history_lines();
        tui.request_terminal_replay_clear();

        self.deferred_history_lines.clear();
        if !reflowed_lines.is_empty() {
            tui.insert_history_hyperlink_lines_with_wrap_policy(
                reflowed_lines,
                self.history_line_wrap_policy(),
            );
        }

        Ok(terminal_width)
    }

    /// Rebuild scrollback after rollback removes transcript cells.
    ///
    /// Unlike resize reflow, rollback must clear the terminal even when no cells remain. Otherwise
    /// the cancelled user prompt stays visible in scrollback despite being removed from the source
    /// transcript.
    pub(super) fn rebuild_transcript_after_backtrack(&mut self, tui: &mut tui::Tui) -> Result<()> {
        let terminal_width = tui.terminal.size()?.width;
        let width = self.chat_widget.history_wrap_width(terminal_width);
        let reflowed_lines = if self.transcript_cells.is_empty() {
            self.reset_history_emission_state();
            Vec::new()
        } else {
            self.render_transcript_lines_for_reflow(width).lines
        };

        tui.clear_pending_history_lines();
        tui.request_terminal_replay_clear();

        self.deferred_history_lines.clear();
        if !reflowed_lines.is_empty() {
            tui.insert_history_hyperlink_lines_with_wrap_policy(
                reflowed_lines,
                self.history_line_wrap_policy(),
            );
        }

        Ok(())
    }

    /// Render transcript cells for the current resize rebuild.
    ///
    /// Rendering walks backward from the transcript tail so row-capped sessions avoid formatting the
    /// full backlog. If the retained suffix begins inside a stream-continuation run, the walk extends
    /// to include the run's first cell; otherwise separators would be inserted as if the continuation
    /// were a new top-level history item. The final row trim happens after separators are restored,
    /// so the returned rows obey the cap exactly.
    pub(super) fn render_transcript_lines_for_reflow(&mut self, width: u16) -> ReflowRenderResult {
        self.render_transcript_lines_for_reflow_from(/*source_start*/ 0, width)
    }

    fn render_transcript_lines_for_reflow_from(
        &mut self,
        source_start: usize,
        width: u16,
    ) -> ReflowRenderResult {
        let row_cap = self.resize_reflow_max_rows();
        let mut cell_displays = VecDeque::new();
        let mut rendered_rows = 0usize;
        let mut start = self.transcript_cells.len();

        while start > source_start {
            start -= 1;
            let cell = self.transcript_cells[start].clone();
            let lines = cell
                .display_hyperlink_lines_for_mode(width, self.chat_widget.history_render_mode());
            rendered_rows += lines.len();
            cell_displays.push_front(ReflowCellDisplay {
                lines,
                is_stream_continuation: cell.is_stream_continuation(),
            });

            if row_cap.is_some_and(|max_rows| rendered_rows > max_rows) {
                break;
            }
        }

        while start > source_start
            && cell_displays
                .front()
                .is_some_and(|display| display.is_stream_continuation)
        {
            start -= 1;
            let cell = self.transcript_cells[start].clone();
            cell_displays.push_front(ReflowCellDisplay {
                lines: cell.display_hyperlink_lines_for_mode(
                    width,
                    self.chat_widget.history_render_mode(),
                ),
                is_stream_continuation: cell.is_stream_continuation(),
            });
        }

        let mut has_emitted_history_lines = false;
        let mut reflowed_lines = Vec::new();
        for display in cell_displays {
            if !display.lines.is_empty() && !display.is_stream_continuation {
                if has_emitted_history_lines {
                    reflowed_lines.push(HyperlinkLine::new(Line::from("")));
                } else {
                    has_emitted_history_lines = true;
                }
            }
            reflowed_lines.extend(display.lines);
        }
        if let Some(max_rows) = row_cap
            && reflowed_lines.len() > max_rows
        {
            let trimmed_line_count = reflowed_lines.len() - max_rows;
            reflowed_lines = reflowed_lines.split_off(trimmed_line_count);
        }
        self.has_emitted_history_lines = !reflowed_lines.is_empty();

        ReflowRenderResult {
            lines: reflowed_lines,
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
