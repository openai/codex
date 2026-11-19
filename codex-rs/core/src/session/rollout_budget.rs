use super::session::Session;
use super::turn_context::TurnContext;
use crate::agent::host::AgentRuntimeServices;
use crate::agent::host::AgentUsageReport;
use crate::agent::host::UsageAccountingDecision;
use crate::client_common::ChargePosting;
use crate::client_common::SamplingUsage;
use crate::context::ContextualUserFragment;
use codex_protocol::error::CodexErr;
use codex_protocol::error::CodexErrorDetails;
use codex_protocol::error::Result as CodexResult;

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

    pub(crate) fn inline_compaction_allowed(&self, _turn: &TurnContext) -> CodexResult<bool> {
        self.check_rollout_budget().map(|()| true)
    }

    /// A final-only root can decline an inline compaction before it sends anything.
    /// Do not show the client a live compaction item unless a checkpoint will install.
    pub(crate) fn defer_compaction_start(
        &self,
        turn: &TurnContext,
        phase: codex_analytics::CompactionPhase,
    ) -> bool {
        self.services
            .budget_policy
            .defer_compaction_start(&turn.session_source, phase)
    }

    pub(crate) fn finish_compaction(
        &self,
        turn: &TurnContext,
        phase: codex_analytics::CompactionPhase,
        result: CodexResult<bool>,
    ) -> CodexResult<bool> {
        let standalone = matches!(phase, codex_analytics::CompactionPhase::StandaloneTurn);
        match result {
            Ok(installed) => {
                if standalone {
                    self.check_rollout_budget()?;
                } else {
                    self.inline_compaction_allowed(turn)?;
                }
                Ok(installed)
            }
            Err(err) => Err(err),
        }
    }

    pub(crate) async fn record_rollout_budget_usage(
        &self,
        turn_id: &str,
        usage: &SamplingUsage,
    ) -> CodexResult<()> {
        let result = match usage.charge.posting {
            ChargePosting::Unposted => self.record_numeric_usage(turn_id, usage).await,
            ChargePosting::Posted => Ok(()),
            ChargePosting::PostedTurnExhausted => Err(CodexErr::SessionBudgetExceeded),
        };
        if let Err(error) = &result
            && !matches!(error.details(), CodexErrorDetails::SessionBudgetExceeded)
        {
            return result;
        }
        if let Some(message) = &usage.charge.hooks.invalid_price {
            // A bad price does not erase successful siblings' contributions.
            return Err(CodexErr::UsageAccountingFailed(format!(
                "Invalid post-hook budget units: {message}"
            )));
        }
        result
    }

    async fn record_numeric_usage(&self, turn_id: &str, report: &SamplingUsage) -> CodexResult<()> {
        let usage = report.provider.as_ref();
        let hook_units = match report.charge.hooks.charge {
            codex_hooks::HookCharge::Units(units) => units,
            codex_hooks::HookCharge::Overflow => {
                if let Some(account) = self.native_rollout_budget()
                    && account.config().is_some_and(|config| config.enforce_limit)
                {
                    account.stop_on_overflow();
                }
                return Err(CodexErr::UsageAccountingFailed(
                    "responseChargeAmount: total hook cost exceeds f64 range".into(),
                ));
            }
        };
        if usage.is_none() && hook_units == 0.0 {
            return Ok(());
        }
        match &self.agent_runtime {
            AgentRuntimeServices::Controller => match self
                .services
                .agent_control
                .record_usage(usage.cloned(), hook_units)
                .await
            {
                Ok(()) => Ok(()),
                // A legacy controller can report a posted bill and a stop through this error.
                // A native account has already chosen its own deadline action.
                Err(error)
                    if matches!(error.details(), CodexErrorDetails::SessionBudgetExceeded) =>
                {
                    if self.native_rollout_budget().is_none() {
                        self.work_budget().stop_new_work();
                    }
                    Ok(())
                }
                Err(error) => Err(CodexErr::UsageAccountingFailed(error.to_string())),
            },
            AgentRuntimeServices::Host(services) => {
                let Some(accounting) = &services.usage_accounting else {
                    return Ok(());
                };
                let usage_report = AgentUsageReport {
                    thread_id: self.thread_id(),
                    turn_id: turn_id.to_owned(),
                    usage: usage.cloned(),
                    additional_budget_units: hook_units,
                };
                let host_error = |error| {
                    CodexErr::UsageAccountingFailed(format!(
                        "Host usage accounting failed: {error}"
                    ))
                };
                let decision = accounting
                    .report_usage(usage_report)
                    .await
                    .map_err(host_error)?;
                if decision == UsageAccountingDecision::Exhausted {
                    self.work_budget().stop_new_work();
                }
                match decision {
                    UsageAccountingDecision::Continue | UsageAccountingDecision::Exhausted => {
                        Ok(())
                    }
                    UsageAccountingDecision::ExhaustedForTurn => {
                        Err(CodexErr::SessionBudgetExceeded)
                    }
                }
            }
        }
    }
}
