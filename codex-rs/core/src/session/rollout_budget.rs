use super::session::Session;
use super::turn_context::TurnContext;
use crate::agent::host::AgentRuntimeServices;
use crate::agent::host::AgentUsageReport;
use crate::agent::host::UsageAccountingDecision;
use crate::client_common::SamplingUsage;
use crate::context::ContextualUserFragment;
use codex_protocol::error::CodexErr;
use codex_protocol::error::CodexErrorDetails;
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
    let response_item = sess.budget_reminder_item(&reminder);
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
    pub(crate) fn budget_reminder_item(
        &self,
        reminder: &crate::RolloutBudgetReminder,
    ) -> codex_protocol::models::ResponseItem {
        ContextualUserFragment::into(crate::context::RolloutBudgetContext {
            remaining_units: reminder.remaining_units,
        })
    }

    pub(crate) fn native_rollout_budget(&self) -> Option<std::sync::Arc<crate::RolloutBudget>> {
        match &self.agent_runtime {
            AgentRuntimeServices::Controller => self.services.agent_control.native_rollout_budget(),
            AgentRuntimeServices::Host(_) => None,
        }
    }

    /// All threads consult the same work gate, even when a host owns one thread's bill.
    pub(crate) fn work_budget(&self) -> &crate::RolloutBudget {
        &self.services.local_agent_runtime.rollout_budget
    }

    pub(crate) fn check_rollout_budget(&self) -> CodexResult<()> {
        self.services.budget_policy.check_start()
    }

    /// Return the diagnostic for a refused tool or propagate an accounting error.
    pub(crate) fn tool_start_denial(&self) -> CodexResult<Option<String>> {
        match self.check_rollout_budget() {
            Ok(()) => Ok(None),
            Err(error) => match error.details() {
                CodexErrorDetails::SessionBudgetExceeded => {
                    Ok(Some("rollout budget exhausted".into()))
                }
                _ => Err(error),
            },
        }
    }

    pub(crate) async fn record_rollout_budget_usage(
        &self,
        turn_id: &str,
        usage: &SamplingUsage,
    ) -> CodexResult<()> {
        if !usage.charge.posted {
            self.record_numeric_usage(turn_id, usage.provider.as_ref(), usage.charge.hooks.charge)
                .await?;
        }
        if let Some(message) = &usage.charge.hooks.invalid_price {
            // A bad price does not erase successful siblings' contributions.
            return Err(CodexErr::UsageAccountingFailed(format!(
                "Invalid post-hook budget units: {message}"
            )));
        }
        Ok(())
    }

    async fn record_numeric_usage(
        &self,
        turn_id: &str,
        usage: Option<&TokenUsage>,
        hook_charge: codex_hooks::HookCharge,
    ) -> CodexResult<()> {
        if usage.is_none() && hook_charge == codex_hooks::HookCharge::default() {
            return Ok(());
        }
        let stop_new_work = match &self.agent_runtime {
            AgentRuntimeServices::Controller => match self
                .services
                .agent_control
                .record_usage(usage.cloned(), hook_charge)
                .await
            {
                Ok(()) => false,
                // A legacy controller can report a posted bill and a stop through this error.
                // A native account has already chosen its own deadline action.
                Err(error)
                    if matches!(error.details(), CodexErrorDetails::SessionBudgetExceeded) =>
                {
                    self.native_rollout_budget().is_none()
                }
                Err(error) => return Err(CodexErr::UsageAccountingFailed(error.to_string())),
            },
            AgentRuntimeServices::Host(services) => {
                let Some(accounting) = &services.usage_accounting else {
                    return Ok(());
                };
                let decision = accounting
                    .report_usage(AgentUsageReport {
                        thread_id: self.thread_id(),
                        turn_id: turn_id.to_owned(),
                        usage: usage.cloned(),
                        additional_budget_units: hook_charge.units,
                        additional_budget_units_overflowed: hook_charge.overflowed,
                    })
                    .await
                    .map_err(|error| {
                        CodexErr::UsageAccountingFailed(format!(
                            "Host usage accounting failed: {error}"
                        ))
                    })?;
                decision == UsageAccountingDecision::Exhausted
            }
        };
        if stop_new_work {
            self.work_budget().stop_new_work();
        }
        Ok(())
    }
}
