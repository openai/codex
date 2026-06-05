use super::*;

impl AgentControl {
    /// Shut down a live agent and remove its logical identity from the registry.
    pub(crate) async fn shutdown_and_forget_agent(
        &self,
        agent_id: ThreadId,
    ) -> CodexResult<String> {
        let state = self.upgrade()?;
        let result = if let Ok(thread) = state.get_thread(agent_id).await {
            thread.codex.session.ensure_rollout_materialized().await;
            thread.codex.session.flush_rollout().await?;
            let result = if matches!(thread.agent_status().await, AgentStatus::Shutdown) {
                Ok(String::new())
            } else {
                state.send_op(agent_id, Op::Shutdown {}).await
            };
            thread.wait_until_terminated().await;
            result
        } else {
            state.send_op(agent_id, Op::Shutdown {}).await
        };
        let _ = state.remove_thread(&agent_id).await;
        self.residency.lock().await.remove(agent_id);
        self.state.release_spawned_thread(agent_id);
        result
    }

    /// Submit a shutdown request for a live agent without marking it explicitly closed in
    /// persisted spawn-edge state.
    pub(crate) async fn shutdown_live_agent(&self, agent_id: ThreadId) -> CodexResult<String> {
        self.shutdown_and_forget_agent(agent_id).await
    }

    /// Permanently close an agent and all open descendants.
    pub(crate) async fn close_agent(&self, agent_id: ThreadId) -> CodexResult<String> {
        self.close_agent_v1(agent_id).await
    }

    /// Permanently close a V1 agent and all open descendants.
    pub(crate) async fn close_agent_v1(&self, agent_id: ThreadId) -> CodexResult<String> {
        let state = self.upgrade()?;
        let known_agent = self.state.agent_metadata_for_thread(agent_id).is_some();
        let mut agent_ids = vec![agent_id];
        if let Some(state_db_ctx) = state.state_db() {
            let descendants = state_db_ctx
                .list_thread_spawn_descendants_with_status(
                    agent_id,
                    DirectionalThreadSpawnEdgeStatus::Open,
                )
                .await
                .map_err(|err| {
                    CodexErr::Fatal(format!(
                        "failed to list open descendants for {agent_id}: {err}"
                    ))
                })?;
            agent_ids.extend(descendants);
            for thread_id in &agent_ids {
                state_db_ctx
                    .set_thread_spawn_edge_status(
                        *thread_id,
                        DirectionalThreadSpawnEdgeStatus::Closed,
                    )
                    .await
                    .map_err(|err| {
                        CodexErr::Fatal(format!(
                            "failed to persist closed agent state for {thread_id}: {err}"
                        ))
                    })?;
            }
        } else {
            agent_ids.extend(self.live_thread_spawn_descendants(agent_id).await?);
        }
        let mut target_result = Ok(String::new());
        for thread_id in agent_ids {
            let result = self.shutdown_and_forget_agent(thread_id).await;
            if thread_id == agent_id {
                target_result = result;
            } else if let Err(err) = result
                && !matches!(
                    err,
                    CodexErr::ThreadNotFound(_) | CodexErr::InternalAgentDied
                )
            {
                return Err(err);
            }
        }
        match target_result {
            Err(CodexErr::ThreadNotFound(_)) | Err(CodexErr::InternalAgentDied) if known_agent => {
                Ok(String::new())
            }
            result => result,
        }
    }

    /// Shut down `agent_id` and any live descendants reachable from the in-memory spawn tree.
    pub(crate) async fn shutdown_agent_tree(&self, agent_id: ThreadId) -> CodexResult<String> {
        let descendant_ids = self.live_thread_spawn_descendants(agent_id).await?;
        let result = self.shutdown_and_forget_agent(agent_id).await;
        for descendant_id in descendant_ids {
            match self.shutdown_and_forget_agent(descendant_id).await {
                Ok(_) | Err(CodexErr::ThreadNotFound(_)) | Err(CodexErr::InternalAgentDied) => {}
                Err(err) => return Err(err),
            }
        }
        result
    }

    pub(super) fn maybe_start_completion_watcher(
        &self,
        child_thread_id: ThreadId,
        session_source: Option<SessionSource>,
        child_reference: String,
    ) {
        let Some(SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
            parent_thread_id, ..
        })) = session_source
        else {
            return;
        };
        let control = self.clone();
        tokio::spawn(async move {
            let status = match control.subscribe_status(child_thread_id).await {
                Ok(mut status_rx) => {
                    let mut status = status_rx.borrow().clone();
                    while !is_final(&status) {
                        if status_rx.changed().await.is_err() {
                            status = control.get_status(child_thread_id).await;
                            break;
                        }
                        status = status_rx.borrow().clone();
                    }
                    status
                }
                Err(_) => control.get_status(child_thread_id).await,
            };
            if !is_final(&status) {
                return;
            }

            let Ok(state) = control.upgrade() else {
                return;
            };
            let message = format_subagent_notification_message(child_reference.as_str(), &status);
            let Ok(parent_thread) = state.get_thread(parent_thread_id).await else {
                return;
            };
            parent_thread
                .inject_user_message_without_turn(message)
                .await;
        });
    }
}
