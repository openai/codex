use super::session::Session;
use super::step_context::StepContext;
use crate::context::ContextualUserFragment;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result as CodexResult;
use codex_protocol::protocol::TokenUsage;

pub(super) async fn maybe_record_reminder(
    sess: &Session,
    step_context: &StepContext,
    window_id: &str,
) {
    let turn_context = step_context.turn.as_ref();
    let budget = sess.services.agent_control.rollout_budget();
    let Some(reminder) = budget.pending_reminder(sess.thread_id(), window_id) else {
        return;
    };
    let response_item = ContextualUserFragment::into(crate::context::RolloutBudgetContext {
        remaining_tokens: reminder.remaining_tokens,
    });
    sess.record_conversation_items(step_context, std::slice::from_ref(&response_item))
        .await;
    budget.mark_reminder_delivered(sess.thread_id(), window_id, reminder);
}

impl Session {
    pub(crate) fn record_rollout_budget_usage(&self, usage: &TokenUsage) -> CodexResult<()> {
        if self
            .services
            .agent_control
            .rollout_budget()
            .record_usage(usage)
        {
            return Err(CodexErr::SessionBudgetExceeded);
        }
        Ok(())
    }
}
