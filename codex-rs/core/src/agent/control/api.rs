//! Adapts the existing local operations to the shared controller interface.
//! Runtime loading, message delivery and shared state remain in their existing modules.

use super::LocalAgentControl;
use crate::agent::api::AgentConfigUpdate;
use crate::agent::api::AgentControl;
use crate::agent::api::AgentInfo;
use crate::agent::api::AgentTarget;
use crate::agent::api::AgentTurnOutcome;
use crate::agent::api::DeliveryReceipt;
use crate::agent::api::SendRequest;
use crate::agent::api::SpawnRequest;
use crate::agent::api::StatusSubscription;
use crate::agent::types::AgentExecutionGuard;
use crate::agent::types::LiveAgent;
use crate::codex_thread::GuardianRootSnapshot;
use crate::codex_thread::ThreadConfigSnapshot;
use crate::config::Config;
use crate::rollout_budget::RolloutBudgetReminder;
use codex_protocol::SessionId;
use codex_protocol::ThreadId;
use codex_protocol::error::Result;
use codex_protocol::protocol::MultiAgentVersion;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::TokenUsage;
use codex_rollout_trace::ThreadTraceContext;
use futures::future::BoxFuture;

impl AgentControl for LocalAgentControl {
    fn identity(&self) -> SessionId {
        self.session_id()
    }

    fn spawn(
        &self,
        request: SpawnRequest,
    ) -> BoxFuture<'_, Result<(LiveAgent, ThreadConfigSnapshot)>> {
        Box::pin(LocalAgentControl::spawn(self, request))
    }

    fn resume(
        &self,
        caller: ThreadId,
        target: AgentTarget,
        config: Config,
        source: SessionSource,
    ) -> BoxFuture<'_, Result<(LiveAgent, ThreadConfigSnapshot)>> {
        Box::pin(async move {
            let target = self.resolve_target(caller, &target)?;
            self.resume_agent(config, target, source).await
        })
    }

    fn send(&self, request: SendRequest) -> BoxFuture<'_, Result<DeliveryReceipt>> {
        Box::pin(LocalAgentControl::send(self, request))
    }

    fn interrupt(
        &self,
        caller: ThreadId,
        target: AgentTarget,
        version: MultiAgentVersion,
    ) -> BoxFuture<'_, Result<AgentInfo>> {
        Box::pin(async move {
            let target = self.resolve_target(caller, &target)?;
            match version {
                MultiAgentVersion::Disabled | MultiAgentVersion::V1 => {
                    let snapshot = self.inspect_agent(target).await?;
                    self.interrupt_agent(target).await?;
                    Ok(snapshot)
                }
                MultiAgentVersion::V2 => self.interrupt_spawned_agent(caller, target).await,
            }
        })
    }

    fn close(&self, caller: ThreadId, target: AgentTarget) -> BoxFuture<'_, Result<AgentInfo>> {
        Box::pin(async move {
            let target = self.resolve_target(caller, &target)?;
            self.close_agent(target).await
        })
    }

    fn inspect(&self, caller: ThreadId, target: AgentTarget) -> BoxFuture<'_, Result<AgentInfo>> {
        Box::pin(LocalAgentControl::inspect(self, caller, target))
    }

    fn list<'a>(
        &'a self,
        source: &'a SessionSource,
        path_prefix: Option<&'a str>,
    ) -> BoxFuture<'a, Result<Vec<LiveAgent>>> {
        Box::pin(self.list_agents(source, path_prefix))
    }

    fn watch(
        &self,
        caller: ThreadId,
        target: AgentTarget,
    ) -> BoxFuture<'_, Result<StatusSubscription>> {
        Box::pin(async move {
            let target = self.resolve_target(caller, &target)?;
            self.subscribe_status(target).await
        })
    }

    fn check_turn_admission(
        &self,
        version: MultiAgentVersion,
        source: &SessionSource,
    ) -> Result<()> {
        self.ensure_execution_capacity(version, source)
    }

    fn admit_turn(
        &self,
        version: MultiAgentVersion,
        source: SessionSource,
    ) -> BoxFuture<'_, Result<Option<AgentExecutionGuard>>> {
        Box::pin(async move { Ok(self.execution_guard(version, &source)) })
    }

    fn record_usage(&self, usage: TokenUsage) -> BoxFuture<'_, Result<()>> {
        Box::pin(async move { self.record_rollout_budget_usage(&usage) })
    }

    fn turn_finished<'a>(
        &'a self,
        outcome: AgentTurnOutcome,
        trace: &'a ThreadTraceContext,
    ) -> BoxFuture<'a, Result<()>> {
        Box::pin(async move {
            self.notify_parent_of_terminal_turn(outcome, trace).await;
            Ok(())
        })
    }

    fn service_tier(&self) -> Option<String> {
        self.root_service_tier()
    }

    fn propagate_config_update(&self, update: AgentConfigUpdate) -> BoxFuture<'_, Result<()>> {
        Box::pin(async move {
            match update {
                AgentConfigUpdate::ServiceTier(tier) => self.set_root_service_tier(tier),
            }
            Ok(())
        })
    }

    fn get_guardian_package(
        &self,
        agent: ThreadId,
    ) -> BoxFuture<'_, Result<Option<GuardianRootSnapshot>>> {
        Box::pin(async move { Ok(self.root_user_authorization(agent).await) })
    }

    fn pending_budget_reminder<'a>(
        &'a self,
        agent: ThreadId,
        window: &'a str,
    ) -> BoxFuture<'a, Result<Option<RolloutBudgetReminder>>> {
        Box::pin(async move {
            Ok(LocalAgentControl::pending_budget_reminder(
                self, agent, window,
            ))
        })
    }

    fn mark_budget_reminder_delivered<'a>(
        &'a self,
        agent: ThreadId,
        window: &'a str,
        reminder: RolloutBudgetReminder,
    ) -> BoxFuture<'a, Result<()>> {
        Box::pin(async move {
            LocalAgentControl::mark_budget_reminder_delivered(self, agent, window, reminder);
            Ok(())
        })
    }
}
