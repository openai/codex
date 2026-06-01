//! Transcript and active-cell bookkeeping for `ChatWidget`.

use super::HistoryCell;
use super::MAX_AGENT_COPY_HISTORY;

#[derive(Debug)]
pub(super) struct AgentTurnMarkdown {
    pub(super) user_turn_count: usize,
    pub(super) markdown: String,
}

#[derive(Default)]
pub(super) struct TranscriptState {
    pub(super) active_cell: Option<Box<dyn HistoryCell>>,
    /// Monotonic-ish counter used to invalidate transcript overlay caching.
    pub(super) active_cell_revision: u64,
    /// Raw markdown of the most recently completed agent response that
    /// survived any local thread rollback.
    pub(super) last_agent_markdown: Option<String>,
    /// Copyable agent responses keyed by the number of visible user turns at
    /// the time the response completed.
    pub(super) agent_turn_markdowns: Vec<AgentTurnMarkdown>,
    /// Number of user turns currently reflected in the visible transcript.
    pub(super) visible_user_turn_count: usize,
    /// True when rollback discarded the requested copy source because it was
    /// older than the retained copy history.
    pub(super) copy_history_evicted_by_rollback: bool,
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
        match self.agent_turn_markdowns.last_mut() {
            Some(entry) if entry.user_turn_count == self.visible_user_turn_count => {
                entry.markdown = markdown.clone();
            }
            _ => {
                self.agent_turn_markdowns.push(AgentTurnMarkdown {
                    user_turn_count: self.visible_user_turn_count,
                    markdown: markdown.clone(),
                });
                if self.agent_turn_markdowns.len() > MAX_AGENT_COPY_HISTORY {
                    self.agent_turn_markdowns.remove(0);
                }
            }
        }
        self.last_agent_markdown = Some(markdown);
        self.copy_history_evicted_by_rollback = false;
        self.saw_copy_source_this_turn = true;
    }

    pub(super) fn record_visible_user_turn(&mut self) {
        self.visible_user_turn_count = self.visible_user_turn_count.saturating_add(1);
    }

    pub(super) fn reset_copy_history(&mut self) {
        self.last_agent_markdown = None;
        self.agent_turn_markdowns.clear();
        self.visible_user_turn_count = 0;
        self.copy_history_evicted_by_rollback = false;
        self.saw_copy_source_this_turn = false;
    }

    pub(super) fn truncate_copy_history_to_user_turn_count(&mut self, user_turn_count: usize) {
        self.visible_user_turn_count = user_turn_count;
        let had_copy_history = !self.agent_turn_markdowns.is_empty();
        self.agent_turn_markdowns
            .retain(|entry| entry.user_turn_count <= user_turn_count);
        self.last_agent_markdown = self
            .agent_turn_markdowns
            .last()
            .map(|entry| entry.markdown.clone());
        self.copy_history_evicted_by_rollback =
            had_copy_history && self.last_agent_markdown.is_none();
        self.saw_copy_source_this_turn = false;
    }

    pub(super) fn reset_turn_flags(&mut self) {
        self.saw_copy_source_this_turn = false;
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

    #[test]
    fn copy_history_tracks_latest_visible_turn() {
        let mut state = TranscriptState::default();
        state.record_visible_user_turn();
        state.record_agent_markdown("first".to_string());
        state.record_visible_user_turn();
        state.record_agent_markdown("second".to_string());

        state.truncate_copy_history_to_user_turn_count(/*user_turn_count*/ 1);

        assert_eq!(state.last_agent_markdown.as_deref(), Some("first"));
        assert!(!state.copy_history_evicted_by_rollback);
    }
}
