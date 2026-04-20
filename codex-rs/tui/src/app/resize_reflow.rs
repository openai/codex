use std::time::Instant;

use codex_features::Feature;
use color_eyre::eyre::Result;
use ratatui::text::Line;

use super::App;
use super::trailing_run_start;
use crate::history_cell;
use crate::history_cell::HistoryCell;
use crate::transcript_reflow::TRANSCRIPT_REFLOW_DEBOUNCE;
use crate::tui;

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

    pub(super) fn append_history_cell_lines_for_insert(
        &mut self,
        lines: &mut Vec<Line<'static>>,
        cell: &dyn HistoryCell,
        width: u16,
    ) {
        lines.extend(self.display_lines_for_history_insert(cell, width));
    }

    pub(super) fn terminal_resize_reflow_enabled(&self) -> bool {
        self.config.features.enabled(Feature::TerminalResizeReflow)
    }

    fn schedule_resize_reflow(&mut self) -> bool {
        debug_assert!(self.terminal_resize_reflow_enabled());
        self.transcript_reflow.schedule_debounced()
    }

    /// After stream consolidation, schedule a follow-up reflow if one ran mid-stream.
    pub(super) fn maybe_finish_stream_reflow(&mut self, tui: &mut tui::Tui) {
        if !self.terminal_resize_reflow_enabled() {
            self.transcript_reflow.clear();
            return;
        }
        if self.transcript_reflow.take_ran_during_stream() {
            if self.schedule_resize_reflow() {
                tui.frame_requester().schedule_frame();
            } else {
                tui.frame_requester()
                    .schedule_frame_in(TRANSCRIPT_REFLOW_DEBOUNCE);
            }
        } else if self.transcript_reflow.pending_is_due(Instant::now()) {
            tui.frame_requester().schedule_frame();
        }
    }

    fn schedule_immediate_resize_reflow(&mut self, tui: &mut tui::Tui) {
        if !self.terminal_resize_reflow_enabled() {
            self.transcript_reflow.clear();
            return;
        }
        self.transcript_reflow.schedule_immediate();
        tui.frame_requester().schedule_frame();
    }

    pub(super) fn finish_required_stream_reflow(&mut self, tui: &mut tui::Tui) -> Result<()> {
        if !self.terminal_resize_reflow_enabled() {
            self.transcript_reflow.clear();
            return Ok(());
        }
        self.schedule_immediate_resize_reflow(tui);
        self.maybe_run_resize_reflow(tui)?;
        if !self.transcript_reflow.has_pending_reflow() {
            self.transcript_reflow.clear_ran_during_stream();
        }
        Ok(())
    }

    pub(super) fn handle_draw_size_change(
        &mut self,
        size: ratatui::layout::Size,
        last_known_screen_size: ratatui::layout::Size,
        frame_requester: &tui::FrameRequester,
    ) -> bool {
        let width = self.transcript_reflow.note_width(size.width);
        if width.changed {
            self.chat_widget.on_terminal_resize(size.width);
            if self.terminal_resize_reflow_enabled() {
                if self.schedule_resize_reflow() {
                    frame_requester.schedule_frame();
                } else {
                    frame_requester.schedule_frame_in(TRANSCRIPT_REFLOW_DEBOUNCE);
                }
            } else {
                self.transcript_reflow.clear();
            }
        } else if width.initialized {
            self.chat_widget.on_terminal_resize(size.width);
        }
        if size != last_known_screen_size {
            self.refresh_status_line();
        }
        if self.terminal_resize_reflow_enabled() {
            self.maybe_clear_resize_reflow_without_terminal();
        }
        width.changed
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
        let width_changed = self.handle_draw_size_change(
            size,
            tui.terminal.last_known_screen_size,
            &tui.frame_requester(),
        );
        if width_changed && self.terminal_resize_reflow_enabled() {
            // Width-sensitive history inserts queued before this frame may be wrapped for the old
            // viewport. Drop them and let resize reflow rebuild from transcript cells.
            tui.clear_pending_history_lines();
        }
        self.maybe_run_resize_reflow(tui)?;
        Ok(())
    }

    pub(super) fn maybe_run_resize_reflow(&mut self, tui: &mut tui::Tui) -> Result<()> {
        if !self.terminal_resize_reflow_enabled() {
            self.transcript_reflow.clear();
            return Ok(());
        }
        let Some(deadline) = self.transcript_reflow.pending_until() else {
            return Ok(());
        };
        if Instant::now() < deadline || self.overlay.is_some() {
            return Ok(());
        }

        self.transcript_reflow.clear_pending_reflow();

        // Track that a reflow happened during an active stream or while trailing
        // unconsolidated AgentMessageCells are still pending consolidation so
        // ConsolidateAgentMessage can schedule a follow-up reflow.
        let reflow_ran_during_stream =
            !self.transcript_cells.is_empty() && self.should_mark_reflow_as_stream_time();

        self.reflow_transcript_now(tui)?;

        if reflow_ran_during_stream {
            self.transcript_reflow.mark_ran_during_stream();
        }

        Ok(())
    }

    fn reflow_transcript_now(&mut self, tui: &mut tui::Tui) -> Result<()> {
        // Drop any queued pre-resize/pre-consolidation inserts before rebuilding from cells.
        tui.clear_pending_history_lines();
        if self.transcript_cells.is_empty() {
            self.reset_history_emission_state();
            return Ok(());
        }

        if tui.is_alt_screen_active() {
            tui.terminal.clear_visible_screen()?;
        } else {
            tui.terminal.clear_scrollback_and_visible_screen_ansi()?;
        }

        self.reset_history_emission_state();

        let width = tui.terminal.size()?.width;
        let mut reflowed_lines = Vec::new();
        // Iterate by index to avoid cloning the Vec and bumping Arc refcounts.
        for i in 0..self.transcript_cells.len() {
            let cell = self.transcript_cells[i].clone();
            self.append_history_cell_lines_for_insert(&mut reflowed_lines, cell.as_ref(), width);
        }
        if !reflowed_lines.is_empty() {
            tui.insert_history_lines(reflowed_lines);
        }

        Ok(())
    }

    pub(super) fn should_mark_reflow_as_stream_time(&self) -> bool {
        self.chat_widget.has_active_agent_stream()
            || self.chat_widget.has_active_plan_stream()
            || trailing_run_start::<history_cell::AgentMessageCell>(&self.transcript_cells)
                < self.transcript_cells.len()
            || trailing_run_start::<history_cell::ProposedPlanStreamCell>(&self.transcript_cells)
                < self.transcript_cells.len()
    }
}
