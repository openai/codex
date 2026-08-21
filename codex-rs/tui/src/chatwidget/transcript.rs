//! Transcript and active-cell bookkeeping for `ChatWidget`.

use super::HistoryCell;
use super::HistoryRenderMode;
use std::cell::Cell;

/// Identifies the render state that determines an active cell's viewport height.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ActiveCellLayoutCacheKey {
    pub(super) cell_identity: usize,
    pub(super) revision: u64,
    pub(super) width: u16,
    pub(super) render_mode: HistoryRenderMode,
    pub(super) syntax_theme_revision: u64,
}

/// Retains the active cell's semantic and actual wrapped heights independently.
///
/// History cells may override their desired height, so it cannot be substituted for the rendered
/// row count used to keep overflowing content anchored to the bottom of the viewport.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ActiveCellLayoutCache {
    pub(super) key: ActiveCellLayoutCacheKey,
    pub(super) desired_height: Option<u16>,
    pub(super) rendered_height: Option<usize>,
}

#[derive(Default)]
pub(super) struct TranscriptState {
    pub(super) active_cell: Option<Box<dyn HistoryCell>>,
    /// Monotonic-ish counter used to invalidate transcript overlay caching.
    pub(super) active_cell_revision: u64,
    /// One bounded entry shared by layout and paint across unchanged active-cell frames.
    pub(super) active_cell_layout: Cell<Option<ActiveCellLayoutCache>>,
    /// Markdown of the most recently completed agent response for whole-response copying.
    pub(super) last_agent_markdown: Option<String>,
    /// Original source of that response, before display sanitization, for exact block copying.
    pub(super) last_agent_source: Option<String>,
    pub(super) last_completed_agent_message: Option<(String, String)>,
    /// Raw markdown of the most recently completed proposed plan.
    pub(super) latest_proposed_plan_markdown: Option<String>,
    /// Whether this turn already produced a copyable response.
    pub(super) saw_copy_source_this_turn: bool,
    /// Whether the next streamed assistant content should be preceded by a final message separator.
    pub(super) needs_final_message_separator: bool,
    /// Whether the current turn performed "work" (exec commands, MCP tool calls, patch applications).
    pub(super) had_work_activity: bool,
    /// Whether the current turn emitted a plan update.
    pub(super) saw_plan_update_this_turn: bool,
    /// Whether the current turn emitted a proposed plan item that has not been superseded by a
    /// later steer.
    pub(super) saw_plan_item_this_turn: bool,
    /// Latest `update_plan` checklist task counts for terminal-title rendering.
    pub(super) last_plan_progress: Option<(usize, usize)>,
    /// Incremental buffer for streamed plan content.
    pub(super) plan_delta_buffer: String,
    /// True while a plan item is streaming.
    pub(super) plan_item_active: bool,
}

impl TranscriptState {
    pub(super) fn new(active_cell: Option<Box<dyn HistoryCell>>) -> Self {
        Self {
            active_cell,
            ..Self::default()
        }
    }

    pub(super) fn bump_active_cell_revision(&mut self) {
        // Wrapping avoids overflow; wraparound would require 2^64 bumps and at
        // worst causes a one-time cache-key collision.
        self.active_cell_revision = self.active_cell_revision.wrapping_add(1);
        self.active_cell_layout.set(None);
    }

    /// Remove the active cell and invalidate its layout before its address can be reused.
    pub(super) fn take_active_cell(&mut self) -> Option<Box<dyn HistoryCell>> {
        let active_cell = self.active_cell.take();
        if active_cell.is_some() {
            self.active_cell_layout.set(None);
        }
        active_cell
    }

    pub(super) fn record_agent_markdown(&mut self, markdown: String, source: String) {
        self.last_agent_markdown = Some(markdown);
        self.last_agent_source = Some(source);
        self.saw_copy_source_this_turn = true;
    }

    pub(super) fn reset_copy_history(&mut self) {
        self.last_agent_markdown = None;
        self.last_agent_source = None;
        self.saw_copy_source_this_turn = false;
    }

    pub(super) fn reset_turn_flags(&mut self) {
        self.saw_copy_source_this_turn = false;
        self.last_completed_agent_message = None;
        self.saw_plan_update_this_turn = false;
        self.saw_plan_item_this_turn = false;
        self.had_work_activity = false;
        self.latest_proposed_plan_markdown = None;
        self.plan_delta_buffer.clear();
        self.plan_item_active = false;
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn active_cell_revision_wraps() {
        let mut state = TranscriptState {
            active_cell_revision: u64::MAX,
            ..TranscriptState::default()
        };

        state.bump_active_cell_revision();

        assert_eq!(state.active_cell_revision, 0);
    }
}
