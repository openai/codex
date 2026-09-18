//! Keeps shared rollout-budget accounting and reminder state behind the controller.
//! Callers acknowledge a reminder only after it has been added to conversation history.

use super::LocalAgentControl;
use crate::rollout_budget::RolloutBudgetReminder;
use codex_protocol::ThreadId;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result as CodexResult;
use codex_protocol::protocol::TokenUsage;

impl LocalAgentControl {
    pub(crate) fn record_rollout_budget_usage(&self, usage: &TokenUsage) -> CodexResult<()> {
        if self.rollout_budget.record_usage(usage)? {
            return Err(CodexErr::SessionBudgetExceeded);
        }
        Ok(())
    }

    pub(crate) fn pending_budget_reminder(
        &self,
        thread_id: ThreadId,
        window_id: &str,
    ) -> Option<RolloutBudgetReminder> {
        self.rollout_budget.pending_reminder(thread_id, window_id)
    }

    pub(crate) fn mark_budget_reminder_delivered(
        &self,
        thread_id: ThreadId,
        window_id: &str,
        reminder: RolloutBudgetReminder,
    ) {
        self.rollout_budget
            .mark_reminder_delivered(thread_id, window_id, reminder);
    }
}
