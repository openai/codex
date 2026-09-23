//! Implements shared controller operations using the existing local runtime helpers.
//! Runtime loading, message delivery and shared state remain in their existing modules.

use super::LocalAgentControl;
use super::spawn::SpawnInitialInput;
use crate::agent::api::AgentConfigUpdate;
use crate::agent::api::AgentControl;
use crate::agent::api::AgentInfo;
use crate::agent::api::AgentInput;
use crate::agent::api::AgentTarget;
use crate::agent::api::AgentTurnOutcome;
use crate::agent::api::DeliveryReceipt;
use crate::agent::api::SendRequest;
use crate::agent::api::SpawnRequest;
use crate::agent::types::AgentExecutionGuard;
use crate::agent::types::LiveAgent;
use crate::agent::types::MessageDeliveryMode;
use crate::agent_communication::AgentCommunicationContext;
use crate::agent_communication::AgentCommunicationKind;
use crate::codex_thread::GuardianRootSnapshot;
use crate::codex_thread::ThreadConfigSnapshot;
use crate::rollout_budget::RolloutBudgetReminder;
use codex_protocol::AgentPath;
use codex_protocol::SessionId;
use codex_protocol::ThreadId;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result;
use codex_protocol::protocol::MultiAgentVersion;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::TokenUsage;
use codex_rollout_trace::ThreadTraceContext;
use futures::future::BoxFuture;
use std::collections::HashSet;

impl AgentControl for LocalAgentControl {
    fn identity(&self) -> SessionId {
        self.session_id()
    }

    fn resolve<'a>(
        &'a self,
        caller: ThreadId,
        parent: Option<ThreadId>,
        source: &'a SessionSource,
        target: &'a str,
    ) -> BoxFuture<'a, Result<ThreadId>> {
        Box::pin(async move {
            self.runtime.register_session_root(caller, parent);
            if let Ok(thread_id) = ThreadId::from_string(target) {
                return Ok(thread_id);
            }
            self.runtime
                .resolve_agent_reference(caller, source, target)
                .await
        })
    }

    fn spawn(
        &self,
        request: SpawnRequest,
    ) -> BoxFuture<'_, Result<(LiveAgent, ThreadConfigSnapshot)>> {
        Box::pin(async move {
            let SpawnRequest {
                caller,
                config,
                input,
                source,
                options,
            } = request;
            let input = match input {
                AgentInput::UserInput(input) => SpawnInitialInput::UserInput(input),
                AgentInput::Message { message, mode } => {
                    if mode != MessageDeliveryMode::TriggerTurn {
                        return Err(CodexErr::InvalidRequest(
                            "spawn input must start the child turn".to_string(),
                        ));
                    }
                    let recipient = source.get_agent_path().ok_or_else(|| {
                        CodexErr::InvalidRequest(
                            "spawned agent is missing a canonical task name".to_string(),
                        )
                    })?;
                    let author = recipient
                        .as_str()
                        .rsplit_once('/')
                        .and_then(|(parent, _)| AgentPath::try_from(parent).ok())
                        .ok_or_else(|| {
                            CodexErr::InvalidRequest("spawn input needs a child path".to_string())
                        })?;
                    SpawnInitialInput::InterAgentCommunication(
                        message.into_communication(author, recipient, mode),
                        AgentCommunicationContext::new(AgentCommunicationKind::Spawn, caller),
                    )
                }
            };
            Box::pin(self.spawn_agent_internal(config, input, Some(source), options)).await
        })
    }

    fn send(&self, request: SendRequest) -> BoxFuture<'_, Result<DeliveryReceipt>> {
        Box::pin(async move {
            let SendRequest {
                caller,
                target,
                resume_config,
                input,
                mut start_options,
            } = request;
            let target = self.resolve_target(caller, &target)?;
            let (metadata, submission_id) = match input {
                AgentInput::UserInput(input) => {
                    let receiver = self.get_agent_metadata(target);
                    if receiver.is_some() {
                        self.ensure_v2_agent_loaded(resume_config, target, /*parent*/ None)
                            .await?;
                    }
                    let submission_id = self.send_input(target, input, start_options).await?;
                    (receiver.unwrap_or_default(), submission_id)
                }
                AgentInput::Message { message, mode } => {
                    let receiver = self.runtime.ensure_agent_known(target)?;
                    let author = self
                        .runtime
                        .ensure_agent_known(caller)?
                        .agent_path
                        .unwrap_or_else(AgentPath::root);
                    if mode == MessageDeliveryMode::TriggerTurn
                        && receiver.agent_path.as_ref().is_some_and(AgentPath::is_root)
                    {
                        return Err(CodexErr::UnsupportedOperation(
                            "Follow-up tasks can't target the root agent".to_string(),
                        ));
                    }
                    let receiver_path = receiver.agent_path.clone().ok_or_else(|| {
                        CodexErr::UnsupportedOperation(
                            "target agent is missing an agent_path".to_string(),
                        )
                    })?;
                    self.ensure_v2_agent_loaded(resume_config, target, /*parent*/ None)
                        .await?;
                    let communication = message.into_communication(author, receiver_path, mode);
                    let kind = match mode {
                        MessageDeliveryMode::QueueOnly => {
                            start_options.parent_turn_id = None;
                            AgentCommunicationKind::Message
                        }
                        MessageDeliveryMode::TriggerTurn => AgentCommunicationKind::Followup,
                    };
                    let submission_id = self
                        .send_inter_agent_communication(
                            target,
                            communication,
                            AgentCommunicationContext::new(kind, caller),
                            start_options,
                        )
                        .await?;
                    (receiver, submission_id)
                }
            };
            Ok(DeliveryReceipt {
                thread_id: target,
                metadata,
                submission_id,
            })
        })
    }

    fn ensure_child_loaded(&self, parent: ThreadId, child: ThreadId) -> BoxFuture<'_, Result<()>> {
        Box::pin(async move {
            let parent = self.runtime.upgrade()?.get_thread(parent).await?;
            let config = parent.session.get_config().await.as_ref().clone();
            self.ensure_v2_agent_loaded(config, child, Some(parent))
                .await
        })
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

    fn list<'a>(
        &'a self,
        caller: ThreadId,
        parent: Option<ThreadId>,
        source: &'a SessionSource,
        path_prefix: Option<&'a str>,
    ) -> BoxFuture<'a, Result<Vec<LiveAgent>>> {
        Box::pin(async move {
            self.runtime.register_session_root(caller, parent);
            self.list_agents(source, path_prefix).await
        })
    }

    fn child_agent_paths(&self, parent: ThreadId) -> BoxFuture<'_, Vec<AgentPath>> {
        Box::pin(async move {
            let Some(parent_path) = self
                .runtime
                .registry
                .agent_metadata_for_thread(parent)
                .and_then(|metadata| metadata.agent_path)
            else {
                return Vec::new();
            };
            let parent_prefix = format!("{parent_path}/");
            let mut agent_paths = self
                .runtime
                .registry
                .live_agents()
                .into_iter()
                .filter_map(|metadata| metadata.agent_path)
                .filter(|path| {
                    path.as_str()
                        .strip_prefix(&parent_prefix)
                        .is_some_and(|name| !name.contains('/'))
                })
                .collect::<Vec<_>>();
            let loaded_paths = self
                .runtime
                .open_thread_spawn_children(parent)
                .await
                .unwrap_or_default()
                .into_iter()
                .filter_map(|(_, metadata)| metadata.agent_path)
                .collect::<HashSet<_>>();
            agent_paths.sort();
            // Stable sorting preserves alphabetical order within each group.
            agent_paths.sort_by_key(|path| !loaded_paths.contains(path));
            agent_paths
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
        source: &SessionSource,
    ) -> Option<AgentExecutionGuard> {
        self.execution_guard(version, source)
    }

    fn record_usage(&self, usage: TokenUsage) -> BoxFuture<'_, Result<()>> {
        Box::pin(async move { self.record_rollout_budget_usage(&usage) })
    }

    fn turn_finished<'a>(
        &'a self,
        outcome: AgentTurnOutcome,
        trace: &'a ThreadTraceContext,
    ) -> BoxFuture<'a, ()> {
        Box::pin(self.notify_parent_of_terminal_turn(outcome, trace))
    }

    fn service_tier(&self) -> Option<String> {
        self.root_service_tier()
    }

    fn propagate_config_update(&self, update: AgentConfigUpdate) {
        match update {
            AgentConfigUpdate::ServiceTier(tier) => self.set_root_service_tier(tier),
        }
    }

    fn get_guardian_package(&self, agent: ThreadId) -> BoxFuture<'_, Option<GuardianRootSnapshot>> {
        Box::pin(self.root_user_authorization(agent))
    }

    fn pending_budget_reminder<'a>(
        &'a self,
        agent: ThreadId,
        window: &'a str,
    ) -> BoxFuture<'a, Option<RolloutBudgetReminder>> {
        Box::pin(async move { LocalAgentControl::pending_budget_reminder(self, agent, window) })
    }

    fn mark_budget_reminder_delivered<'a>(
        &'a self,
        agent: ThreadId,
        window: &'a str,
        reminder: RolloutBudgetReminder,
    ) -> BoxFuture<'a, ()> {
        Box::pin(async move {
            LocalAgentControl::mark_budget_reminder_delivered(self, agent, window, reminder);
        })
    }
}
