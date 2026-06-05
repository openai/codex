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

    pub(crate) async fn list_live_agent_subtree_thread_ids(
        &self,
        agent_id: ThreadId,
    ) -> CodexResult<Vec<ThreadId>> {
        let mut thread_ids = vec![agent_id];
        thread_ids.extend(self.live_thread_spawn_descendants(agent_id).await?);
        Ok(thread_ids)
    }

    pub(crate) async fn format_environment_context_subagents(
        &self,
        parent_thread_id: ThreadId,
    ) -> String {
        let Ok(agents) = self.open_thread_spawn_children(parent_thread_id).await else {
            return String::new();
        };

        agents
            .into_iter()
            .map(|(thread_id, metadata)| {
                let reference = metadata
                    .agent_path
                    .as_ref()
                    .map(|agent_path| agent_path.name().to_string())
                    .unwrap_or_else(|| thread_id.to_string());
                format_subagent_context_line(reference.as_str(), metadata.agent_nickname.as_deref())
            })
            .collect::<Vec<_>>()
            .join("\n")
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

    async fn open_thread_spawn_children(
        &self,
        parent_thread_id: ThreadId,
    ) -> CodexResult<Vec<(ThreadId, AgentMetadata)>> {
        let mut children_by_parent = self.live_thread_spawn_children().await?;
        Ok(children_by_parent
            .remove(&parent_thread_id)
            .unwrap_or_default())
    }

    async fn live_thread_spawn_children(
        &self,
    ) -> CodexResult<HashMap<ThreadId, Vec<(ThreadId, AgentMetadata)>>> {
        let state = self.upgrade()?;
        let mut children_by_parent = HashMap::<ThreadId, Vec<(ThreadId, AgentMetadata)>>::new();

        for (parent_thread_id, child_thread_id) in state.list_live_thread_spawn_edges().await {
            children_by_parent
                .entry(parent_thread_id)
                .or_default()
                .push((
                    child_thread_id,
                    self.state
                        .agent_metadata_for_thread(child_thread_id)
                        .unwrap_or(AgentMetadata {
                            agent_id: Some(child_thread_id),
                            ..Default::default()
                        }),
                ));
        }

        for children in children_by_parent.values_mut() {
            children.sort_by(|left, right| {
                left.1
                    .agent_path
                    .as_deref()
                    .unwrap_or_default()
                    .cmp(right.1.agent_path.as_deref().unwrap_or_default())
                    .then_with(|| left.0.to_string().cmp(&right.0.to_string()))
            });
        }

        Ok(children_by_parent)
    }

    async fn live_thread_spawn_descendants(
        &self,
        root_thread_id: ThreadId,
    ) -> CodexResult<Vec<ThreadId>> {
        let mut children_by_parent = self.live_thread_spawn_children().await?;
        let mut descendants = Vec::new();
        let mut stack = children_by_parent
            .remove(&root_thread_id)
            .unwrap_or_default()
            .into_iter()
            .map(|(child_thread_id, _)| child_thread_id)
            .rev()
            .collect::<Vec<_>>();

        while let Some(thread_id) = stack.pop() {
            descendants.push(thread_id);
            if let Some(children) = children_by_parent.remove(&thread_id) {
                for (child_thread_id, _) in children.into_iter().rev() {
                    stack.push(child_thread_id);
                }
            }
        }

        Ok(descendants)
    }
}
