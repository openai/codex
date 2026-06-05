use super::*;

const AGENT_NAMES: &str = include_str!("../agent_names.txt");

struct SpawnAgentThreadInheritance {
    shell_snapshot: Option<Arc<ShellSnapshot>>,
    exec_policy: Option<Arc<crate::exec_policy::ExecPolicyManager>>,
}

fn default_agent_nickname_list() -> Vec<&'static str> {
    AGENT_NAMES
        .lines()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .collect()
}

pub(super) fn agent_nickname_candidates(config: &Config, role_name: Option<&str>) -> Vec<String> {
    let role_name = role_name.unwrap_or(DEFAULT_ROLE_NAME);
    if let Some(candidates) =
        resolve_role_config(config, role_name).and_then(|role| role.nickname_candidates.clone())
    {
        return candidates;
    }

    default_agent_nickname_list()
        .into_iter()
        .map(ToOwned::to_owned)
        .collect()
}

fn resume_session_source(
    requested_source: SessionSource,
    stored_source: SessionSource,
    multi_agent_version: MultiAgentVersion,
) -> SessionSource {
    match multi_agent_version {
        MultiAgentVersion::V2 => resume_session_source_v2(requested_source, stored_source),
        MultiAgentVersion::Disabled | MultiAgentVersion::V1 => match stored_source {
            source @ SessionSource::SubAgent(SubAgentSource::ThreadSpawn { .. }) => source,
            _ => requested_source,
        },
    }
}

fn resume_session_source_v2(
    requested_source: SessionSource,
    stored_source: SessionSource,
) -> SessionSource {
    let stored_thread_spawn = match stored_source {
        SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
            parent_thread_id,
            depth,
            agent_path,
            agent_role,
            ..
        }) => Some((parent_thread_id, depth, agent_path, agent_role)),
        _ => None,
    };

    match requested_source {
        SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
            parent_thread_id,
            depth,
            agent_path,
            agent_role,
            ..
        }) => {
            let stored_agent_path = stored_thread_spawn
                .as_ref()
                .and_then(|(_, _, agent_path, _)| agent_path.clone());
            let stored_agent_role = stored_thread_spawn
                .as_ref()
                .and_then(|(_, _, _, agent_role)| agent_role.clone());
            SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                parent_thread_id,
                depth,
                agent_path: agent_path.or(stored_agent_path),
                agent_nickname: None,
                agent_role: agent_role.or(stored_agent_role),
            })
        }
        other => match stored_thread_spawn {
            Some((parent_thread_id, depth, agent_path, agent_role)) => {
                SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                    parent_thread_id,
                    depth,
                    agent_path,
                    agent_nickname: None,
                    agent_role,
                })
            }
            None => other,
        },
    }
}

fn keep_forked_rollout_item(item: &RolloutItem, preserve_reference_context_item: bool) -> bool {
    match item {
        RolloutItem::ResponseItem(ResponseItem::Message { role, phase, .. }) => match role.as_str()
        {
            "system" | "developer" | "user" => true,
            "assistant" => *phase == Some(MessagePhase::FinalAnswer),
            _ => false,
        },
        RolloutItem::ResponseItem(
            ResponseItem::AgentMessage { .. }
            | ResponseItem::Reasoning { .. }
            | ResponseItem::LocalShellCall { .. }
            | ResponseItem::FunctionCall { .. }
            | ResponseItem::ToolSearchCall { .. }
            | ResponseItem::FunctionCallOutput { .. }
            | ResponseItem::CustomToolCall { .. }
            | ResponseItem::CustomToolCallOutput { .. }
            | ResponseItem::ToolSearchOutput { .. }
            | ResponseItem::WebSearchCall { .. }
            | ResponseItem::ImageGenerationCall { .. }
            | ResponseItem::Compaction { .. }
            | ResponseItem::CompactionTrigger
            | ResponseItem::ContextCompaction { .. }
            | ResponseItem::Other,
        ) => false,
        // Full-history forks preserve the cached prompt prefix and can keep diffing
        // from the parent's durable baseline. Truncated forks drop part of that prompt,
        // so they must rebuild context on their first child turn.
        RolloutItem::TurnContext(_) => preserve_reference_context_item,
        RolloutItem::Compacted(_) | RolloutItem::EventMsg(_) | RolloutItem::SessionMeta(_) => true,
    }
}

fn is_multi_agent_v2_usage_hint_message(item: &ResponseItem, usage_hint_texts: &[String]) -> bool {
    let ResponseItem::Message { role, content, .. } = item else {
        return false;
    };
    if role != "developer" {
        return false;
    }
    let [ContentItem::InputText { text }] = content.as_slice() else {
        return false;
    };

    usage_hint_texts
        .iter()
        .any(|usage_hint_text| usage_hint_text == text)
}

impl AgentControl {
    /// Spawn a new agent thread and submit the initial prompt.
    #[cfg(test)]
    pub(crate) async fn spawn_agent(
        &self,
        config: Config,
        initial_operation: Op,
        session_source: Option<SessionSource>,
    ) -> CodexResult<ThreadId> {
        let spawned_agent = Box::pin(self.spawn_agent_internal(
            config,
            initial_operation,
            session_source,
            SpawnAgentOptions::default(),
        ))
        .await?;
        Ok(spawned_agent.thread_id)
    }

    /// Spawn an agent thread with some metadata.
    pub(crate) async fn spawn_agent_with_metadata(
        &self,
        config: Config,
        initial_operation: Op,
        session_source: Option<SessionSource>,
        options: SpawnAgentOptions, // TODO(jif) drop with new fork.
    ) -> CodexResult<LiveAgent> {
        Box::pin(self.spawn_agent_internal(config, initial_operation, session_source, options))
            .await
    }

    async fn spawn_agent_internal(
        &self,
        config: Config,
        initial_operation: Op,
        session_source: Option<SessionSource>,
        options: SpawnAgentOptions,
    ) -> CodexResult<LiveAgent> {
        let state = self.upgrade()?;
        let multi_agent_version = state
            .effective_multi_agent_version_for_spawn(
                &InitialHistory::New,
                session_source.as_ref(),
                options.parent_thread_id,
                /*forked_from_thread_id*/ None,
                &config,
            )
            .await;
        let agent_max_threads = config.effective_agent_max_threads(multi_agent_version);
        let mut reservation = self.state.reserve_spawn_slot(agent_max_threads)?;
        let inheritance = SpawnAgentThreadInheritance {
            shell_snapshot: self
                .inherited_shell_snapshot_for_source(&state, session_source.as_ref())
                .await,
            exec_policy: self
                .inherited_exec_policy_for_source(&state, session_source.as_ref(), &config)
                .await,
        };
        let (session_source, mut agent_metadata) = match session_source {
            Some(SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                parent_thread_id,
                depth,
                agent_path,
                agent_role,
                ..
            })) => {
                if let Some(agent_path) = agent_path.as_ref() {
                    self.ensure_agent_path_available(&state, parent_thread_id, depth, agent_path)
                        .await?;
                }
                let (session_source, agent_metadata) = self.prepare_thread_spawn(
                    &mut reservation,
                    &config,
                    parent_thread_id,
                    depth,
                    agent_path,
                    agent_role,
                    /*preferred_agent_nickname*/ None,
                    multi_agent_version,
                )?;
                (Some(session_source), agent_metadata)
            }
            other => (other, AgentMetadata::default()),
        };
        let notification_source = session_source.clone();

        let new_thread = match (session_source, options.fork_mode.as_ref(), inheritance) {
            (Some(session_source), Some(_), inheritance) => {
                Box::pin(self.spawn_forked_thread(
                    &state,
                    config.clone(),
                    session_source,
                    &options,
                    inheritance,
                    multi_agent_version,
                ))
                .await?
            }
            (Some(session_source), None, inheritance) => {
                Box::pin(state.spawn_new_thread_with_source(
                    config.clone(),
                    self.clone(),
                    session_source,
                    options.parent_thread_id,
                    /*forked_from_thread_id*/ None,
                    /*thread_source*/ Some(ThreadSource::Subagent),
                    /*metrics_service_name*/ None,
                    inheritance.shell_snapshot,
                    inheritance.exec_policy,
                    options.environments.clone(),
                ))
                .await?
            }
            (None, _, _) => Box::pin(state.spawn_new_thread(config.clone(), self.clone())).await?,
        };
        agent_metadata.agent_id = Some(new_thread.thread_id);
        if multi_agent_version == MultiAgentVersion::V2 {
            let mut residency = self.residency.lock().await;
            if let Err(err) = async {
                if !residency.contains(new_thread.thread_id) {
                    self.make_resident_room(&state, &mut residency, &config)
                        .await?;
                }
                residency.touch(new_thread.thread_id);
                CodexResult::<()>::Ok(())
            }
            .await
            {
                if state
                    .send_op(new_thread.thread_id, Op::Shutdown {})
                    .await
                    .is_ok()
                {
                    new_thread.thread.wait_until_terminated().await;
                }
                let _ = state.remove_thread(&new_thread.thread_id).await;
                return Err(err);
            }
        }
        reservation.commit(agent_metadata.clone());

        if let Some(SessionSource::SubAgent(
            subagent_source @ SubAgentSource::ThreadSpawn {
                parent_thread_id, ..
            },
        )) = notification_source.as_ref()
        {
            let client_metadata = match state.get_thread(*parent_thread_id).await {
                Ok(parent_thread) => {
                    parent_thread
                        .codex
                        .session
                        .app_server_client_metadata()
                        .await
                }
                Err(error) => {
                    tracing::warn!(
                        error = %error,
                        parent_thread_id = %parent_thread_id,
                        "skipping subagent thread analytics: failed to load parent thread metadata"
                    );
                    crate::session::session::AppServerClientMetadata {
                        client_name: None,
                        client_version: None,
                    }
                }
            };
            let thread_config = new_thread.thread.codex.thread_config_snapshot().await;
            let parent_thread_id = thread_config.parent_thread_id;
            emit_subagent_session_started(
                &new_thread
                    .thread
                    .codex
                    .session
                    .services
                    .analytics_events_client,
                client_metadata,
                new_thread.thread.codex.session.session_id(),
                new_thread.thread_id,
                parent_thread_id,
                thread_config,
                subagent_source.clone(),
            );
        }

        state.notify_thread_created(new_thread.thread_id);
        self.persist_thread_spawn_edge_for_source(
            new_thread.thread.as_ref(),
            new_thread.thread_id,
            notification_source.as_ref(),
        )
        .await;

        self.send_input(new_thread.thread_id, initial_operation)
            .await?;
        if multi_agent_version != MultiAgentVersion::V2 {
            let child_reference = agent_metadata
                .agent_path
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_else(|| new_thread.thread_id.to_string());
            self.maybe_start_completion_watcher(
                new_thread.thread_id,
                notification_source,
                child_reference,
            );
        }

        Ok(LiveAgent {
            thread_id: new_thread.thread_id,
            metadata: agent_metadata,
            status: self.get_status(new_thread.thread_id).await,
        })
    }

    async fn spawn_forked_thread(
        &self,
        state: &Arc<ThreadManagerState>,
        config: Config,
        session_source: SessionSource,
        options: &SpawnAgentOptions,
        inheritance: SpawnAgentThreadInheritance,
        multi_agent_version: MultiAgentVersion,
    ) -> CodexResult<crate::thread_manager::NewThread> {
        let SpawnAgentThreadInheritance {
            shell_snapshot: inherited_shell_snapshot,
            exec_policy: inherited_exec_policy,
        } = inheritance;
        if options.fork_parent_spawn_call_id.is_none() {
            return Err(CodexErr::Fatal(
                "spawn_agent fork requires a parent spawn call id".to_string(),
            ));
        }
        let Some(fork_mode) = options.fork_mode.as_ref() else {
            return Err(CodexErr::Fatal(
                "spawn_agent fork requires a fork mode".to_string(),
            ));
        };
        let SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
            parent_thread_id, ..
        }) = &session_source
        else {
            return Err(CodexErr::Fatal(
                "spawn_agent fork requires a thread-spawn session source".to_string(),
            ));
        };

        let parent_thread_id = *parent_thread_id;
        let parent_thread = state.get_thread(parent_thread_id).await.ok();
        if let Some(parent_thread) = parent_thread.as_ref() {
            parent_thread.ensure_rollout_materialized().await;
            parent_thread.flush_rollout().await?;
        }

        let parent_history = state
            .read_stored_thread(ReadThreadParams {
                thread_id: parent_thread_id,
                include_archived: true,
                include_history: true,
            })
            .await?
            .history
            .ok_or_else(|| {
                CodexErr::Fatal(format!(
                    "parent thread history unavailable for fork: {parent_thread_id}"
                ))
            })?;

        let mut forked_rollout_items = parent_history.items;
        if let SpawnAgentForkMode::LastNTurns(last_n_turns) = fork_mode {
            forked_rollout_items =
                truncate_rollout_to_last_n_fork_turns(&forked_rollout_items, *last_n_turns);
        }
        let multi_agent_v2_usage_hint_texts_to_filter: Vec<String> =
            if let Some(parent_thread) = parent_thread.as_ref() {
                if multi_agent_version == MultiAgentVersion::V2 {
                    let parent_config = parent_thread.codex.session.get_config().await;
                    [
                        parent_config
                            .multi_agent_v2
                            .root_agent_usage_hint_text
                            .clone(),
                        parent_config
                            .multi_agent_v2
                            .subagent_usage_hint_text
                            .clone(),
                    ]
                    .into_iter()
                    .flatten()
                    .collect()
                } else {
                    Vec::new()
                }
            } else if multi_agent_version == MultiAgentVersion::V2 {
                [
                    config.multi_agent_v2.root_agent_usage_hint_text.clone(),
                    config.multi_agent_v2.subagent_usage_hint_text.clone(),
                ]
                .into_iter()
                .flatten()
                .collect()
            } else {
                Vec::new()
            };
        let preserve_reference_context_item = matches!(fork_mode, SpawnAgentForkMode::FullHistory);
        forked_rollout_items.retain(|item| {
            keep_forked_rollout_item(item, preserve_reference_context_item)
                && !matches!(
                    item,
                    RolloutItem::ResponseItem(response_item)
                        if is_multi_agent_v2_usage_hint_message(
                            response_item,
                            &multi_agent_v2_usage_hint_texts_to_filter,
                        )
                )
        });
        for item in &mut forked_rollout_items {
            if let RolloutItem::Compacted(compacted) = item
                && let Some(replacement_history) = compacted.replacement_history.as_mut()
            {
                replacement_history.retain(|response_item| {
                    !is_multi_agent_v2_usage_hint_message(
                        response_item,
                        &multi_agent_v2_usage_hint_texts_to_filter,
                    )
                });
            }
        }
        if preserve_reference_context_item
            && multi_agent_version == MultiAgentVersion::V2
            && config.multi_agent_v2.usage_hint_enabled
            && let Some(subagent_usage_hint_text) =
                config.multi_agent_v2.subagent_usage_hint_text.clone()
            && let Some(subagent_usage_hint_message) =
                crate::context_manager::updates::build_developer_update_item(vec![
                    subagent_usage_hint_text,
                ])
        {
            forked_rollout_items.push(RolloutItem::ResponseItem(subagent_usage_hint_message));
        }

        state
            .fork_thread_with_source(
                config.clone(),
                InitialHistory::Forked(forked_rollout_items),
                self.clone(),
                session_source,
                /*thread_source*/ Some(ThreadSource::Subagent),
                /*parent_thread_id*/ Some(parent_thread_id),
                /*forked_from_thread_id*/ Some(parent_thread_id),
                inherited_shell_snapshot,
                inherited_exec_policy,
                options.environments.clone(),
            )
            .await
    }

    /// Resume an existing agent thread from a recorded rollout file.
    pub(crate) async fn resume_agent_from_rollout(
        &self,
        config: Config,
        thread_id: ThreadId,
        session_source: SessionSource,
    ) -> CodexResult<ThreadId> {
        let root_depth = thread_spawn_depth(&session_source).unwrap_or(0);
        let resumed_thread_id = Box::pin(self.resume_single_agent_from_rollout(
            config.clone(),
            thread_id,
            session_source,
        ))
        .await?;
        let state = self.upgrade()?;
        let Ok(resumed_thread) = state.get_thread(resumed_thread_id).await else {
            return Ok(resumed_thread_id);
        };
        let Some(state_db_ctx) = resumed_thread.state_db() else {
            return Ok(resumed_thread_id);
        };

        let mut resume_queue = VecDeque::from([(thread_id, root_depth)]);
        while let Some((parent_thread_id, parent_depth)) = resume_queue.pop_front() {
            let child_ids = match state_db_ctx
                .list_thread_spawn_children_with_status(
                    parent_thread_id,
                    DirectionalThreadSpawnEdgeStatus::Open,
                )
                .await
            {
                Ok(child_ids) => child_ids,
                Err(err) => {
                    warn!(
                        "failed to load persisted thread-spawn children for {parent_thread_id}: {err}"
                    );
                    continue;
                }
            };

            for child_thread_id in child_ids {
                let child_depth = parent_depth + 1;
                let child_resumed = if state.get_thread(child_thread_id).await.is_ok() {
                    true
                } else {
                    let child_session_source =
                        SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                            parent_thread_id,
                            depth: child_depth,
                            agent_path: None,
                            agent_nickname: None,
                            agent_role: None,
                        });
                    match Box::pin(self.resume_single_agent_from_rollout(
                        config.clone(),
                        child_thread_id,
                        child_session_source,
                    ))
                    .await
                    {
                        Ok(_) => true,
                        Err(err) => {
                            warn!("failed to resume descendant thread {child_thread_id}: {err}");
                            false
                        }
                    }
                };
                if child_resumed {
                    resume_queue.push_back((child_thread_id, child_depth));
                }
            }
        }

        Ok(resumed_thread_id)
    }

    pub(super) fn resume_single_agent_from_rollout(
        &self,
        config: Config,
        thread_id: ThreadId,
        session_source: SessionSource,
    ) -> BoxFuture<'_, CodexResult<ThreadId>> {
        Box::pin(async move {
            let state = self.upgrade()?;
            let stored_thread = state
                .read_stored_thread(ReadThreadParams {
                    thread_id,
                    include_archived: true,
                    include_history: true,
                })
                .await?;
            let stored_session_source = stored_thread.source.clone();
            let history = stored_thread
                .history
                .ok_or_else(|| CodexErr::ThreadNotFound(thread_id))?
                .items;
            let initial_history = InitialHistory::Resumed(ResumedHistory {
                conversation_id: thread_id,
                history,
                rollout_path: stored_thread.rollout_path,
            });
            let parent_thread_id = stored_thread.parent_thread_id;
            let multi_agent_version = state
                .effective_multi_agent_version_for_spawn(
                    &initial_history,
                    Some(&session_source),
                    parent_thread_id,
                    /*forked_from_thread_id*/ None,
                    &config,
                )
                .await;
            let reservation =
                if multi_agent_version == MultiAgentVersion::V2 {
                    None
                } else {
                    Some(self.state.reserve_spawn_slot(
                        config.effective_agent_max_threads(multi_agent_version),
                    )?)
                };
            let session_source =
                resume_session_source(session_source, stored_session_source, multi_agent_version);
            let agent_metadata = agent_metadata_from_session_source(thread_id, &session_source);
            let notification_source = session_source.clone();
            let inherited_shell_snapshot = self
                .inherited_shell_snapshot_for_source(&state, Some(&session_source))
                .await;
            let inherited_exec_policy = self
                .inherited_exec_policy_for_source(&state, Some(&session_source), &config)
                .await;

            let resumed_thread = state
                .resume_thread_with_history_with_source(ResumeThreadWithHistoryOptions {
                    config: config.clone(),
                    initial_history,
                    agent_control: self.clone(),
                    session_source,
                    parent_thread_id,
                    inherited_shell_snapshot,
                    inherited_exec_policy,
                })
                .await?;
            let mut agent_metadata = agent_metadata;
            agent_metadata.agent_id = Some(resumed_thread.thread_id);
            if let Some(reservation) = reservation {
                reservation.commit(agent_metadata.clone());
            } else {
                self.state.register_agent(agent_metadata.clone());
            }
            state.notify_thread_created(resumed_thread.thread_id);
            if multi_agent_version != MultiAgentVersion::V2 {
                let child_reference = agent_metadata
                    .agent_path
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| resumed_thread.thread_id.to_string());
                self.maybe_start_completion_watcher(
                    resumed_thread.thread_id,
                    Some(notification_source.clone()),
                    child_reference,
                );
            }
            self.persist_thread_spawn_edge_for_source(
                resumed_thread.thread.as_ref(),
                resumed_thread.thread_id,
                Some(&notification_source),
            )
            .await;

            Ok(resumed_thread.thread_id)
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn prepare_thread_spawn(
        &self,
        reservation: &mut crate::agent::registry::SpawnReservation,
        config: &Config,
        parent_thread_id: ThreadId,
        depth: i32,
        agent_path: Option<AgentPath>,
        agent_role: Option<String>,
        preferred_agent_nickname: Option<String>,
        multi_agent_version: MultiAgentVersion,
    ) -> CodexResult<(SessionSource, AgentMetadata)> {
        if depth == 1 {
            self.state.register_root_thread(parent_thread_id);
        }
        if let Some(agent_path) = agent_path.as_ref() {
            reservation.reserve_agent_path(agent_path)?;
        }
        let agent_nickname = if multi_agent_version == MultiAgentVersion::V2 {
            None
        } else {
            let candidate_names = agent_nickname_candidates(config, agent_role.as_deref());
            let candidate_name_refs: Vec<&str> =
                candidate_names.iter().map(String::as_str).collect();
            Some(reservation.reserve_agent_nickname_with_preference(
                &candidate_name_refs,
                preferred_agent_nickname.as_deref(),
            )?)
        };
        let session_source = SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
            parent_thread_id,
            depth,
            agent_path: agent_path.clone(),
            agent_nickname: agent_nickname.clone(),
            agent_role: agent_role.clone(),
        });
        let agent_metadata = AgentMetadata {
            agent_id: None,
            agent_path,
            agent_nickname,
            agent_role,
            last_task_message: None,
        };
        Ok((session_source, agent_metadata))
    }

    async fn ensure_agent_path_available(
        &self,
        state: &Arc<ThreadManagerState>,
        parent_thread_id: ThreadId,
        depth: i32,
        agent_path: &AgentPath,
    ) -> CodexResult<()> {
        let root_thread_id = if depth == 1 {
            parent_thread_id
        } else {
            self.state
                .agent_id_for_path(&AgentPath::root())
                .ok_or_else(|| {
                    CodexErr::UnsupportedOperation("root agent is unavailable".to_string())
                })?
        };
        if let Some(state_db_ctx) = state.state_db()
            && state_db_ctx
                .find_open_thread_spawn_descendant_by_path(root_thread_id, agent_path.as_str())
                .await
                .map_err(|err| {
                    CodexErr::Fatal(format!("failed to check agent path `{agent_path}`: {err}"))
                })?
                .is_some()
        {
            return Err(CodexErr::UnsupportedOperation(format!(
                "agent path `{agent_path}` already exists"
            )));
        }
        Ok(())
    }

    async fn inherited_shell_snapshot_for_source(
        &self,
        state: &Arc<ThreadManagerState>,
        session_source: Option<&SessionSource>,
    ) -> Option<Arc<ShellSnapshot>> {
        let Some(SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
            parent_thread_id, ..
        })) = session_source
        else {
            return None;
        };

        let parent_thread = state.get_thread(*parent_thread_id).await.ok()?;
        parent_thread.codex.session.user_shell().shell_snapshot()
    }

    async fn inherited_exec_policy_for_source(
        &self,
        state: &Arc<ThreadManagerState>,
        session_source: Option<&SessionSource>,
        child_config: &Config,
    ) -> Option<Arc<crate::exec_policy::ExecPolicyManager>> {
        let Some(SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
            parent_thread_id, ..
        })) = session_source
        else {
            return None;
        };

        let parent_thread = state.get_thread(*parent_thread_id).await.ok()?;
        let parent_config = parent_thread.codex.session.get_config().await;
        if !crate::exec_policy::child_uses_parent_exec_policy(&parent_config, child_config) {
            return None;
        }

        Some(Arc::clone(
            &parent_thread.codex.session.services.exec_policy,
        ))
    }

    async fn persist_thread_spawn_edge_for_source(
        &self,
        thread: &crate::CodexThread,
        child_thread_id: ThreadId,
        session_source: Option<&SessionSource>,
    ) {
        let Some(parent_thread_id) = session_source.and_then(SessionSource::parent_thread_id)
        else {
            return;
        };
        let Some(state_db_ctx) = thread.state_db() else {
            return;
        };
        if let Err(err) = state_db_ctx
            .upsert_thread_spawn_edge(
                parent_thread_id,
                child_thread_id,
                DirectionalThreadSpawnEdgeStatus::Open,
            )
            .await
        {
            warn!("failed to persist thread-spawn edge: {err}");
        }
    }
}
