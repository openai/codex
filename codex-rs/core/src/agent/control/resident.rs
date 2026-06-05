use super::*;

const ROOT_LAST_TASK_MESSAGE: &str = "Main thread";

fn max_resident_subagents(config: &Config) -> usize {
    MIN_RESIDENT_SUBAGENTS.max(
        config
            .multi_agent_v2
            .max_concurrent_threads_per_session
            .saturating_add(4),
    )
}

impl AgentControl {
    pub(crate) async fn send_message_to_agent(
        &self,
        config: &Config,
        agent_id: ThreadId,
        communication: InterAgentCommunication,
    ) -> CodexResult<String> {
        let metadata = self.ensure_agent_known(agent_id).await?;
        let state = self.upgrade()?;
        if metadata.agent_path.as_ref().is_some_and(AgentPath::is_root) {
            state.get_thread(agent_id).await?;
            return self
                .send_inter_agent_communication(agent_id, communication)
                .await;
        }
        let mut residency = self.residency.lock().await;
        self.ensure_agent_loaded(&state, &mut residency, config, agent_id)
            .await?;
        self.send_inter_agent_communication(agent_id, communication)
            .await
    }

    pub(crate) async fn send_followup_to_agent(
        &self,
        config: &Config,
        agent_id: ThreadId,
        communication: InterAgentCommunication,
    ) -> CodexResult<String> {
        self.ensure_agent_known(agent_id).await?;
        let state = self.upgrade()?;
        let max_threads = config
            .effective_agent_max_threads(MultiAgentVersion::V2)
            .unwrap_or_default();
        let mut residency = self.residency.lock().await;
        let reservation = self.state.reserve_execution_slot(agent_id, max_threads)?;
        self.ensure_agent_loaded(&state, &mut residency, config, agent_id)
            .await?;
        let sub_id = self
            .send_inter_agent_communication(agent_id, communication)
            .await?;
        reservation.commit();
        drop(residency);
        let thread = state.get_thread(agent_id).await?;
        thread
            .codex
            .session
            .maybe_start_turn_for_pending_work_with_sub_id(sub_id.clone())
            .await;
        Ok(sub_id)
    }

    pub(crate) async fn release_execution_slot_if_idle(&self, session: &Session) {
        let Some(metadata) = self.state.agent_metadata_for_thread(session.thread_id) else {
            return;
        };
        if metadata.agent_path.as_ref().is_none_or(AgentPath::is_root) {
            return;
        }
        let _residency = self.residency.lock().await;
        if session.active_turn.lock().await.is_none()
            && !session.input_queue.has_trigger_turn_mailbox_items().await
        {
            self.state.release_execution_slot(session.thread_id);
        }
    }

    pub(crate) async fn ensure_agent_known(
        &self,
        agent_id: ThreadId,
    ) -> CodexResult<AgentMetadata> {
        if let Some(metadata) = self.state.agent_metadata_for_thread(agent_id) {
            return Ok(metadata);
        }

        let state = self.upgrade()?;
        let stored_thread = state
            .read_stored_thread(ReadThreadParams {
                thread_id: agent_id,
                include_archived: true,
                include_history: false,
            })
            .await?;
        if let Some(state_db_ctx) = state.state_db()
            && !state_db_ctx
                .is_thread_spawn_edge_open(agent_id)
                .await
                .map_err(|err| {
                    CodexErr::Fatal(format!(
                        "failed to read spawned agent state for {agent_id}: {err}"
                    ))
                })?
        {
            return Err(CodexErr::InternalAgentDied);
        }
        if !matches!(
            &stored_thread.source,
            SessionSource::SubAgent(SubAgentSource::ThreadSpawn { .. })
        ) {
            return Err(CodexErr::ThreadNotFound(agent_id));
        }
        let metadata = agent_metadata_from_session_source(agent_id, &stored_thread.source);
        self.state.register_agent(metadata.clone());
        Ok(metadata)
    }

    async fn ensure_agent_loaded(
        &self,
        state: &Arc<ThreadManagerState>,
        residency: &mut ResidentAgents,
        config: &Config,
        agent_id: ThreadId,
    ) -> CodexResult<()> {
        if state.get_thread(agent_id).await.is_ok() {
            if !residency.contains(agent_id) {
                self.make_resident_room(state, residency, config).await?;
            }
            residency.touch(agent_id);
            return Ok(());
        }

        self.make_resident_room(state, residency, config).await?;
        let stored_thread = state
            .read_stored_thread(ReadThreadParams {
                thread_id: agent_id,
                include_archived: true,
                include_history: false,
            })
            .await?;
        self.resume_single_agent_from_rollout(config.clone(), agent_id, stored_thread.source)
            .await?;
        residency.touch(agent_id);
        Ok(())
    }

    pub(super) async fn make_resident_room(
        &self,
        state: &Arc<ThreadManagerState>,
        residency: &mut ResidentAgents,
        config: &Config,
    ) -> CodexResult<()> {
        if residency.len() < max_resident_subagents(config) {
            return Ok(());
        }
        let mut eviction_candidate = None;
        for agent_id in residency.snapshot() {
            if self.state.is_execution_active(agent_id) {
                continue;
            }
            if let Ok(thread) = state.get_thread(agent_id).await
                && thread
                    .codex
                    .session
                    .input_queue
                    .has_pending_mailbox_items()
                    .await
            {
                continue;
            }
            eviction_candidate = Some(agent_id);
            break;
        }
        let Some(agent_id) = eviction_candidate else {
            return Err(CodexErr::Fatal(
                "resident agent cache is full with no idle agent available to unload".to_string(),
            ));
        };
        self.unload_resident_agent(state, residency, agent_id).await
    }

    async fn unload_resident_agent(
        &self,
        state: &Arc<ThreadManagerState>,
        residency: &mut ResidentAgents,
        agent_id: ThreadId,
    ) -> CodexResult<()> {
        let Ok(thread) = state.get_thread(agent_id).await else {
            residency.remove(agent_id);
            return Ok(());
        };
        thread.ensure_rollout_materialized().await;
        thread.flush_rollout().await?;
        if !matches!(thread.agent_status().await, AgentStatus::Shutdown) {
            state.send_op(agent_id, Op::Shutdown {}).await?;
        }
        thread.wait_until_terminated().await;
        let _ = state.remove_thread(&agent_id).await;
        residency.remove(agent_id);
        self.state.clear_last_task_message(agent_id);
        Ok(())
    }

    pub(crate) async fn list_agents(
        &self,
        current_session_source: &SessionSource,
        path_prefix: Option<&str>,
    ) -> CodexResult<Vec<ListedAgent>> {
        let state = self.upgrade()?;
        let resolved_prefix = path_prefix
            .map(|prefix| {
                current_session_source
                    .get_agent_path()
                    .unwrap_or_else(AgentPath::root)
                    .resolve(prefix)
                    .map_err(CodexErr::UnsupportedOperation)
            })
            .transpose()?;

        let resident_agent_ids = self.residency.lock().await.snapshot();
        let mut resident_agents = resident_agent_ids
            .into_iter()
            .map(|agent_id| {
                self.state
                    .agent_metadata_for_thread(agent_id)
                    .unwrap_or(AgentMetadata {
                        agent_id: Some(agent_id),
                        ..Default::default()
                    })
            })
            .collect::<Vec<_>>();
        resident_agents.sort_by(|left, right| {
            left.agent_path
                .as_deref()
                .unwrap_or_default()
                .cmp(right.agent_path.as_deref().unwrap_or_default())
                .then_with(|| {
                    left.agent_id
                        .map(|id| id.to_string())
                        .unwrap_or_default()
                        .cmp(&right.agent_id.map(|id| id.to_string()).unwrap_or_default())
                })
        });

        let root_path = AgentPath::root();
        let mut agents = Vec::with_capacity(resident_agents.len().saturating_add(1));
        if resolved_prefix
            .as_ref()
            .is_none_or(|prefix| agent_matches_prefix(Some(&root_path), prefix))
            && let Some(root_thread_id) = self.state.agent_id_for_path(&root_path)
            && let Ok(root_thread) = state.get_thread(root_thread_id).await
        {
            agents.push(ListedAgent {
                agent_name: root_path.to_string(),
                agent_status: root_thread.agent_status().await,
                last_task_message: Some(ROOT_LAST_TASK_MESSAGE.to_string()),
            });
        }

        for metadata in resident_agents {
            let Some(thread_id) = metadata.agent_id else {
                continue;
            };
            if resolved_prefix
                .as_ref()
                .is_some_and(|prefix| !agent_matches_prefix(metadata.agent_path.as_ref(), prefix))
            {
                continue;
            }

            let agent_name = metadata
                .agent_path
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_else(|| thread_id.to_string());
            let last_task_message = metadata.last_task_message.clone();
            let Ok(thread) = state.get_thread(thread_id).await else {
                continue;
            };
            agents.push(ListedAgent {
                agent_name,
                agent_status: thread.agent_status().await,
                last_task_message,
            });
        }

        Ok(agents)
    }
}

fn agent_matches_prefix(agent_path: Option<&AgentPath>, prefix: &AgentPath) -> bool {
    if prefix.is_root() {
        return true;
    }

    agent_path.is_some_and(|agent_path| {
        agent_path == prefix
            || agent_path
                .as_str()
                .strip_prefix(prefix.as_str())
                .is_some_and(|suffix| suffix.starts_with('/'))
    })
}
