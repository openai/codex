//! Transcript consolidation for finalized streaming agent messages.
//!
//! During streaming, the chat widget emits transient `AgentMessageCell`s so it
//! can animate stable lines into scrollback while keeping the active mutable
//! tail in the bottom pane. Once the answer finishes, the app replaces that
//! stream with a single source-backed `AgentMarkdownCell`, keeping interleaved
//! notices after the answer in their original order. This makes the
//! transcript the canonical owner of the raw markdown source used for future
//! resize re-renders. The retained view preserves its displayed revision before that
//! replacement, then repaints without replaying terminal scrollback.

use std::ops::Range;
use std::sync::Arc;

use color_eyre::eyre::Result;

use super::App;
use crate::app_event::ConsolidationScrollbackReflow;
use crate::history_cell;
use crate::history_cell::HistoryCell;
use crate::pager_overlay::Overlay;
use crate::tui;

impl App {
    pub(super) fn handle_consolidate_agent_message(
        &mut self,
        tui: &mut tui::Tui,
        source: history_cell::AgentMarkdownCell,
        emitted_cells: usize,
        scrollback_reflow: ConsolidationScrollbackReflow,
        deferred_history_cell: Option<Box<dyn HistoryCell>>,
    ) -> Result<()> {
        // Some finalize paths must preserve a last provisional stream cell long
        // enough for queue ordering, then fold it into the canonical
        // source-backed cell during consolidation.
        if let Some(cell) = deferred_history_cell {
            let cell: Arc<dyn HistoryCell> = cell.into();
            if let Some(Overlay::Transcript(t)) = &mut self.overlay {
                t.insert_cell(cell.clone());
            }
            if !tui.is_owned_screen() {
                self.native_history.defer(&cell);
            }
            self.transcript_cells.push(cell);
        }

        // A notice may follow the last fragment or separate two fragments. Bound
        // the walk by what this controller emitted so an empty stream never
        // claims a previous interrupted answer.
        let is_fragment =
            |cell: &Arc<dyn HistoryCell>| cell.as_any().is::<history_cell::AgentMessageCell>();
        let mut start = self.transcript_cells.len();
        let mut end = start;
        let mut fragments = 0;
        for (index, cell) in self.transcript_cells.iter().enumerate().rev() {
            if fragments == emitted_cells
                || cell.as_any().is::<history_cell::AgentMarkdownCell>()
                || cell.as_any().is::<history_cell::UserHistoryCell>()
                || cell.as_any().is::<history_cell::FinalMessageSeparator>()
            {
                break;
            }
            if is_fragment(cell) {
                if fragments == 0 {
                    end = index + 1;
                }
                start = index;
                fragments += 1;
                if !cell.is_stream_continuation() {
                    break;
                }
            }
        }

        if end - start > fragments {
            if tui.is_owned_screen() {
                self.transcript_view
                    .retain_before_regroup(&self.transcript_cells, start..end);
            }
            // Stable partition keeps notices out of the replacement range and
            // preserves their identities in the native output queue.
            self.transcript_cells[start..end].sort_by_key(|cell| !is_fragment(cell));
        }
        self.replace_agent_message(tui, start..end, fragments, source, scrollback_reflow)
    }

    pub(super) fn replace_agent_message(
        &mut self,
        tui: &mut tui::Tui,
        range: Range<usize>,
        fragments: usize,
        source: history_cell::AgentMarkdownCell,
        mut scrollback_reflow: ConsolidationScrollbackReflow,
    ) -> Result<()> {
        let start = range.start;
        let end = start + fragments;
        if start < end {
            tracing::debug!(
                "ConsolidateAgentMessage: replacing cells [{start}..{end}] with AgentMarkdownCell"
            );
            let consolidated: Arc<dyn HistoryCell> = Arc::new(source);
            self.native_history
                .consolidate(&self.transcript_cells[start..end], &consolidated);
            let notices = &self.transcript_cells[end..range.end];
            // Replaying native scrollback clears the terminal's history. Hidden
            // diagnostics and entirely queued answers never split the printed output.
            if !notices.is_empty()
                && !tui.is_owned_screen()
                && scrollback_reflow == ConsolidationScrollbackReflow::IfResizeReflowRan
                && !self.native_history.contains(&consolidated)
            {
                let width = self
                    .chat_widget
                    .history_wrap_width(tui.terminal.last_known_screen_size.width);
                let mode = self.chat_widget.history_render_mode();
                if notices.iter().any(|notice| {
                    !notice
                        .display_hyperlink_lines_for_mode(width, mode)
                        .is_empty()
                }) {
                    scrollback_reflow = ConsolidationScrollbackReflow::Required;
                }
            }
            if tui.is_owned_screen() {
                self.transcript_view.replace_group(
                    &self.transcript_cells,
                    start..end,
                    &consolidated,
                );
            } else {
                self.transcript_view.replace_range(
                    &self.transcript_cells,
                    start..end,
                    &consolidated,
                );
            }
            if let Some(Overlay::Transcript(t)) = &mut self.overlay {
                t.regroup_answer(
                    range.start,
                    &self.transcript_cells[range],
                    fragments,
                    consolidated.clone(),
                );
                tui.frame_requester().schedule_frame();
            }
            self.transcript_cells.splice(start..end, [consolidated]);

            self.finish_agent_message_consolidation(tui, scrollback_reflow)?;
        } else {
            tracing::debug!(
                "ConsolidateAgentMessage: no cells to consolidate(start={start}, end={end})",
            );
            self.maybe_finish_stream_reflow(tui)?;
        }

        Ok(())
    }

    fn finish_agent_message_consolidation(
        &mut self,
        tui: &mut tui::Tui,
        scrollback_reflow: ConsolidationScrollbackReflow,
    ) -> Result<()> {
        if tui.is_owned_screen() {
            self.transcript_reflow.clear_pending_reflow();
            self.transcript_reflow.clear_stream_flags();
            tui.frame_requester().schedule_frame();
            return Ok(());
        }
        match scrollback_reflow {
            ConsolidationScrollbackReflow::IfResizeReflowRan => {
                self.maybe_finish_stream_reflow(tui)?;
            }
            ConsolidationScrollbackReflow::Required => {
                self.finish_required_stream_reflow(tui)?;
            }
        }

        Ok(())
    }
}
