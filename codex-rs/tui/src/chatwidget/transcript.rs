//! Transcript and active-cell bookkeeping for `ChatWidget`.

use super::HistoryCell;

#[derive(Default)]
pub(super) struct TranscriptState {
    pub(super) active_cell: Option<Box<dyn HistoryCell>>,
    /// Monotonic-ish counter used to invalidate transcript overlay caching.
    pub(super) active_cell_revision: u64,
    /// Raw markdown of the most recently completed agent response.
    pub(super) last_agent_markdown: Option<String>,
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
    }

    pub(super) fn record_agent_markdown(&mut self, markdown: String) {
        self.last_agent_markdown = Some(markdown);
        self.saw_copy_source_this_turn = true;
    }

    pub(super) fn reset_copy_history(&mut self) {
        self.last_agent_markdown = None;
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
