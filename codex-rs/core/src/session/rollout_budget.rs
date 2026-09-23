use super::session::Session;
use super::turn_context::TurnContext;
use crate::context::ContextualUserFragment;
use codex_protocol::error::Result as CodexResult;
use codex_protocol::protocol::TokenUsage;

pub(super) async fn maybe_record_reminder(
    sess: &Session,
    turn_context: &TurnContext,
    window_id: &str,
) {
    let agent_control = &sess.services.agent_control;
    let Some(reminder) = agent_control
        .pending_budget_reminder(sess.thread_id(), window_id)
        .await
    else {
        return;
    };
    let response_item = ContextualUserFragment::into(crate::context::RolloutBudgetContext {
        remaining_tokens: reminder.remaining_tokens,
    });
    sess.record_conversation_items(
        turn_context,
        turn_context.model_info(),
        std::slice::from_ref(&response_item),
    )
    .await;
    agent_control
        .mark_budget_reminder_delivered(sess.thread_id(), window_id, reminder)
        .await;
}

impl Session {
    pub(crate) async fn record_rollout_budget_usage(&self, usage: &TokenUsage) -> CodexResult<()> {
        self.services
            .agent_control
            .record_usage(usage.clone())
            .await
    }
}
