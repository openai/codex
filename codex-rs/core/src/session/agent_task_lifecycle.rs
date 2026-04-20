use crate::agent_identity::RegisteredAgentTask;
use crate::session::session::Session;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::SessionAgentTask;
use codex_protocol::protocol::SessionStateUpdate;
use tracing::debug;
use tracing::info;
use tracing::warn;

impl Session {
    pub(super) async fn maybe_prewarm_agent_task_registration(&self) {
        // Startup task registration is best-effort: regular turns already retry on demand, and
        // a prewarm failure should not shut down the session or block unrelated work.
        if let Err(error) = self.ensure_agent_task_registered().await {
            warn!(
                error = %error,
                "startup agent task prewarm failed; regular turns will retry registration"
            );
        }
    }

    fn latest_persisted_agent_task(
        rollout_items: &[RolloutItem],
    ) -> Option<Option<SessionAgentTask>> {
        rollout_items.iter().rev().find_map(|item| match item {
            RolloutItem::SessionState(update) => Some(update.agent_task.clone()),
            _ => None,
        })
    }

    pub(super) async fn restore_persisted_agent_task(&self, rollout_items: &[RolloutItem]) {
        let Some(agent_task_update) = Self::latest_persisted_agent_task(rollout_items) else {
            return;
        };

        match agent_task_update {
            Some(agent_task) => {
                let registered_task =
                    RegisteredAgentTask::from_session_agent_task(agent_task.clone());
                if self
                    .services
                    .agent_identity_manager
                    .task_matches_current_identity(&registered_task)
                    .await
                {
                    let mut state = self.state.lock().await;
                    state.set_agent_task(agent_task);
                } else {
                    debug!(
                        agent_runtime_id = %registered_task.agent_runtime_id,
                        task_id = %registered_task.task_id,
                        "discarding persisted agent task because it does not match the registered agent identity"
                    );
                    let mut state = self.state.lock().await;
                    state.clear_agent_task();
                }
            }
            None => {
                let mut state = self.state.lock().await;
                state.clear_agent_task();
            }
        }
    }

    async fn persist_agent_task_update(&self, agent_task: Option<&RegisteredAgentTask>) {
        self.persist_rollout_items(&[RolloutItem::SessionState(SessionStateUpdate {
            agent_task: agent_task.map(RegisteredAgentTask::to_session_agent_task),
        })])
        .await;
    }

    async fn clear_cached_agent_task(&self, agent_task: &RegisteredAgentTask) {
        let cleared = {
            let mut state = self.state.lock().await;
            if state.agent_task().as_ref() == Some(&agent_task.to_session_agent_task()) {
                state.clear_agent_task();
                true
            } else {
                false
            }
        };
        if cleared {
            self.persist_agent_task_update(/*agent_task*/ None).await;
        }
    }

    async fn cache_agent_task(&self, agent_task: RegisteredAgentTask) -> RegisteredAgentTask {
        let session_agent_task = agent_task.to_session_agent_task();
        let changed = {
            let mut state = self.state.lock().await;
            if state.agent_task().as_ref() == Some(&session_agent_task) {
                false
            } else {
                state.set_agent_task(session_agent_task);
                true
            }
        };
        if changed {
            self.persist_agent_task_update(Some(&agent_task)).await;
        }
        agent_task
    }

    pub(super) async fn cached_agent_task_for_current_identity(
        &self,
    ) -> Option<RegisteredAgentTask> {
        let agent_task = {
            let state = self.state.lock().await;
            state
                .agent_task()
                .map(RegisteredAgentTask::from_session_agent_task)
        }?;

        if self
            .services
            .agent_identity_manager
            .task_matches_current_identity(&agent_task)
            .await
        {
            debug!(
                agent_runtime_id = %agent_task.agent_runtime_id,
                task_id = %agent_task.task_id,
                "reusing cached agent task"
            );
            return Some(agent_task);
        }

        debug!(
            agent_runtime_id = %agent_task.agent_runtime_id,
            task_id = %agent_task.task_id,
            "discarding cached agent task because the registered agent identity changed"
        );
        self.clear_cached_agent_task(&agent_task).await;
        None
    }

    pub(super) async fn ensure_agent_task_registered(
        &self,
    ) -> anyhow::Result<Option<RegisteredAgentTask>> {
        if let Some(agent_task) = self.cached_agent_task_for_current_identity().await {
            return Ok(Some(agent_task));
        }

        let _guard = self.agent_task_registration_lock.lock().await;
        if let Some(agent_task) = self.cached_agent_task_for_current_identity().await {
            return Ok(Some(agent_task));
        }

        for _ in 0..2 {
            let Some(agent_task) = self.services.agent_identity_manager.register_task().await?
            else {
                return Ok(None);
            };

            if !self
                .services
                .agent_identity_manager
                .task_matches_current_identity(&agent_task)
                .await
            {
                debug!(
                    agent_runtime_id = %agent_task.agent_runtime_id,
                    task_id = %agent_task.task_id,
                    "discarding newly registered agent task because the registered agent identity changed"
                );
                continue;
            }

            let agent_task = self.cache_agent_task(agent_task).await;

            info!(
                thread_id = %self.conversation_id,
                agent_runtime_id = %agent_task.agent_runtime_id,
                task_id = %agent_task.task_id,
                "registered agent task for thread"
            );
            return Ok(Some(agent_task));
        }

        Ok(None)
    }
}
