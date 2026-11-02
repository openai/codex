use super::session::Session;
use super::turn_context::TurnContext;
use crate::agent::host::AgentRuntimeServices;
use crate::agent::host::AgentUsageReport;
use crate::agent::host::UsageAccountingDecision;
use crate::context::ContextualUserFragment;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result as CodexResult;
use codex_protocol::protocol::TokenUsage;

pub(super) async fn maybe_record_reminder(
    sess: &Session,
    turn_context: &TurnContext,
    window_id: &str,
) {
    // Host runtimes own budget policy, including whether accounting is enabled.
    if matches!(&sess.agent_runtime, AgentRuntimeServices::Host(_)) {
        return;
    }
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
    pub(crate) async fn record_rollout_budget_usage(
        &self,
        turn_id: &str,
        usage: &TokenUsage,
    ) -> CodexResult<()> {
        match &self.agent_runtime {
            AgentRuntimeServices::Controller => {
                self.services
                    .agent_control
                    .record_usage(usage.clone())
                    .await
            }
            AgentRuntimeServices::Host(services) => {
                let Some(accounting) = &services.usage_accounting else {
                    return Ok(());
                };
                let decision = accounting
                    .report_usage(AgentUsageReport {
                        thread_id: self.thread_id(),
                        turn_id: turn_id.to_owned(),
                        usage: usage.clone(),
                    })
                    .await
                    .map_err(|error| {
                        CodexErr::Fatal(format!("Host usage accounting failed: {error}"))
                    })?;
                match decision {
                    UsageAccountingDecision::Continue => Ok(()),
                    UsageAccountingDecision::Exhausted => Err(CodexErr::SessionBudgetExceeded),
                }
            }
        }
    }
}
