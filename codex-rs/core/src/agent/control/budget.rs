//! Keeps shared rollout-budget accounting and reminder state behind the controller.
//! Callers acknowledge a reminder only after it has been added to conversation history.

use super::LocalAgentControl;
use crate::rollout_budget::RolloutBudgetReminder;
use codex_protocol::ThreadId;
use codex_protocol::error::Result as CodexResult;
use codex_protocol::protocol::TokenUsage;

impl LocalAgentControl {
    pub(crate) fn record_rollout_budget_usage(
        &self,
        usage: Option<&TokenUsage>,
        hook_units: f64,
    ) -> CodexResult<()> {
        // The caller keeps a consumed response even when its bill stops new work.
        self.runtime.rollout_budget.record_usage(usage, hook_units)
    }

    pub(crate) fn pending_budget_reminder(
        &self,
        thread_id: ThreadId,
        window_id: &str,
    ) -> Option<RolloutBudgetReminder> {
        self.runtime
            .rollout_budget
            .pending_reminder(thread_id, window_id)
    }

    pub(crate) fn mark_budget_reminder_delivered(
        &self,
        thread_id: ThreadId,
        window_id: &str,
        reminder: RolloutBudgetReminder,
    ) {
        self.runtime
            .rollout_budget
            .mark_reminder_delivered(thread_id, window_id, reminder);
    }
}
