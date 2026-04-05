use super::watchdog::RemovedWatchdog;
use super::watchdog::WatchdogManager;
use super::watchdog::WatchdogRegistration;
use crate::agent::AgentStatus;
use crate::agent::registry::AgentMetadata;
use crate::agent::registry::AgentRegistry;
use crate::agent::role::DEFAULT_ROLE_NAME;
use crate::agent::role::resolve_role_config;
use crate::agent::status::is_final;
use crate::codex::emit_subagent_session_started;
use crate::codex_thread::ThreadConfigSnapshot;
use crate::find_archived_thread_path_by_id_str;
use crate::find_thread_path_by_id_str;
use crate::rollout::RolloutRecorder;
use crate::session_prefix::format_subagent_context_line;
use crate::session_prefix::format_subagent_notification_message;
use crate::shell_snapshot::ShellSnapshot;
use crate::thread_manager::ThreadManagerState;
use crate::thread_rollout_truncation::truncate_rollout_to_last_n_fork_turns;
use codex_features::Feature;
use codex_protocol::AgentPath;
use codex_protocol::ThreadId;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result as CodexResult;
use codex_protocol::models::ContentItem;
use codex_protocol::models::FunctionCallOutputBody;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::MessagePhase;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::AGENT_INBOX_KIND;
use codex_protocol::protocol::AgentInboxPayload;
use codex_protocol::protocol::InitialHistory;
use codex_protocol::protocol::InterAgentCommunication;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use codex_protocol::protocol::TokenUsage;
use codex_protocol::user_input::UserInput;
use codex_rollout::state_db;
use codex_state::DirectionalThreadSpawnEdgeStatus;
use serde::Serialize;
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::Weak;
use tokio::sync::Mutex;
use tokio::sync::watch;
use tracing::warn;
use uuid::Uuid;

const AGENT_NAMES: &str = include_str!("agent_names.txt");
const ROOT_LAST_TASK_MESSAGE: &str = "Main thread";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SpawnAgentForkMode {
    FullHistory,
    LastNTurns(usize),
}

#[derive(Clone, Debug, Default)]
pub(crate) struct SpawnAgentOptions {
    pub(crate) fork_parent_spawn_call_id: Option<String>,
    pub(crate) fork_mode: Option<SpawnAgentForkMode>,
}

#[derive(Clone, Debug)]
pub(crate) struct LiveAgent {
    pub(crate) thread_id: ThreadId,
    pub(crate) metadata: AgentMetadata,
    pub(crate) status: AgentStatus,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub(crate) struct ListedAgent {
    pub(crate) agent_name: String,
    pub(crate) agent_status: AgentStatus,
    pub(crate) last_task_message: Option<String>,
}

fn default_agent_nickname_list() -> Vec<&'static str> {
    AGENT_NAMES
        .lines()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .collect()
}

fn agent_nickname_candidates(
    config: &crate::config::Config,
    role_name: Option<&str>,
) -> Vec<String> {
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

fn keep_forked_rollout_item(item: &RolloutItem) -> bool {
    match item {
        RolloutItem::ResponseItem(ResponseItem::Message { role, phase, .. }) => match role.as_str()
        {
            "system" | "developer" | "user" => true,
            "assistant" => *phase == Some(MessagePhase::FinalAnswer),
            _ => false,
        },
        RolloutItem::ResponseItem(
            ResponseItem::Reasoning { .. }
            | ResponseItem::LocalShellCall { .. }
            | ResponseItem::FunctionCall { .. }
            | ResponseItem::ToolSearchCall { .. }
            | ResponseItem::FunctionCallOutput { .. }
            | ResponseItem::CustomToolCall { .. }
            | ResponseItem::CustomToolCallOutput { .. }
            | ResponseItem::ToolSearchOutput { .. }
            | ResponseItem::WebSearchCall { .. }
            | ResponseItem::ImageGenerationCall { .. }
            | ResponseItem::GhostSnapshot { .. }
            | ResponseItem::Compaction { .. }
            | ResponseItem::Other,
        ) => false,
        RolloutItem::Compacted(_)
        | RolloutItem::EventMsg(_)
        | RolloutItem::SessionMeta(_)
        | RolloutItem::TurnContext(_) => true,
    }
}

/// Control-plane handle for multi-agent operations.
/// `AgentControl` is held by each session (via `SessionServices`). It provides capability to
/// spawn new agents and the inter-agent communication layer.
/// An `AgentControl` instance is intended to be created at most once per root thread/session
/// tree. That same `AgentControl` is then shared with every sub-agent spawned from that root,
/// which keeps the registry scoped to that root thread rather than the entire `ThreadManager`.
#[derive(Clone)]
pub(crate) struct AgentControl {
    /// Weak handle back to the global thread registry/state.
    /// This is `Weak` to avoid reference cycles and shadow persistence of the form
    /// `ThreadManagerState -> CodexThread -> Session -> SessionServices -> ThreadManagerState`.
    manager: Weak<ThreadManagerState>,
    state: Arc<AgentRegistry>,
    watchdogs: Arc<WatchdogManager>,
    watchdog_compactions_in_progress: Arc<Mutex<HashSet<ThreadId>>>,
}

#[derive(Debug, Clone)]
pub(crate) struct AgentListing {
    pub(crate) thread_id: ThreadId,
    pub(crate) parent_thread_id: Option<ThreadId>,
    pub(crate) status: AgentStatus,
    pub(crate) depth: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum WatchdogParentCompactionResult {
    NotWatchdogHelper,
    ParentBusy {
        parent_thread_id: ThreadId,
    },
    AlreadyInProgress {
        parent_thread_id: ThreadId,
    },
    Submitted {
        parent_thread_id: ThreadId,
        submission_id: String,
    },
}

impl Default for AgentControl {
    fn default() -> Self {
        let manager = Weak::new();
        let state = Arc::new(AgentRegistry::default());
        let watchdogs = WatchdogManager::new(manager.clone(), Arc::clone(&state));
        Self::from_parts(manager, state, watchdogs)
    }
}

impl AgentControl {
    /// Construct a new `AgentControl` that can spawn/message agents via the given manager state.
    pub(crate) fn new(manager: Weak<ThreadManagerState>) -> Self {
        let state = Arc::new(AgentRegistry::default());
        let watchdogs = WatchdogManager::new(manager.clone(), Arc::clone(&state));
        watchdogs.start();
        Self::from_parts(manager, state, watchdogs)
    }

    pub(crate) fn from_parts(
        manager: Weak<ThreadManagerState>,
        state: Arc<AgentRegistry>,
        watchdogs: Arc<WatchdogManager>,
    ) -> Self {
        Self {
            manager,
            state,
            watchdogs,
            watchdog_compactions_in_progress: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    /// Spawn a new agent thread and submit the initial prompt.
    pub(crate) async fn spawn_agent(
        &self,
        config: crate::config::Config,
        initial_operation: Op,
        session_source: Option<SessionSource>,
    ) -> CodexResult<ThreadId> {
        Ok(self
            .spawn_agent_internal(
                config,
                initial_operation,
                session_source,
                SpawnAgentOptions::default(),
            )
            .await?
            .thread_id)
    }

    /// Spawn an agent thread with some metadata.
    pub(crate) async fn spawn_agent_with_metadata(
        &self,
        config: crate::config::Config,
        initial_operation: Op,
        session_source: Option<SessionSource>,
        options: SpawnAgentOptions, // TODO(jif) drop with new fork.
    ) -> CodexResult<LiveAgent> {
        self.spawn_agent_internal(config, initial_operation, session_source, options)
            .await
    }

    async fn spawn_agent_internal(
        &self,
        config: crate::config::Config,
        initial_operation: Op,
        session_source: Option<SessionSource>,
        options: SpawnAgentOptions,
    ) -> CodexResult<LiveAgent> {
        let state = self.upgrade()?;
        let mut reservation = self
            .reserve_spawn_slot_with_reconcile(&state, config.agent_max_threads)
            .await?;
        let inherited_shell_snapshot = self
            .inherited_shell_snapshot_for_source(&state, session_source.as_ref())
            .await;
        let inherited_exec_policy = self
            .inherited_exec_policy_for_source(&state, session_source.as_ref(), &config)
            .await;
        let (session_source, mut agent_metadata) = match session_source {
            Some(SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                parent_thread_id,
                depth,
                agent_path,
                agent_role,
                ..
            })) => {
                let (session_source, agent_metadata) = self.prepare_thread_spawn(
                    &mut reservation,
                    &config,
                    parent_thread_id,
                    depth,
                    agent_path,
                    agent_role,
                    /*preferred_agent_nickname*/ None,
                )?;
                (Some(session_source), agent_metadata)
            }
            other => (other, AgentMetadata::default()),
        };
        let notification_source = session_source.clone();

        // The same `AgentControl` is sent to spawn the thread.
        let new_thread = match (session_source, options.fork_mode.as_ref()) {
            (Some(session_source), Some(_)) => {
                self.spawn_forked_thread(
                    &state,
                    config,
                    session_source,
                    &options,
                    inherited_shell_snapshot,
                    inherited_exec_policy,
                )
                .await?
            }
            (Some(session_source), None) => {
                state
                    .spawn_new_thread_with_source(
                        config,
                        self.clone(),
                        session_source,
                        /*persist_extended_history*/ false,
                        /*metrics_service_name*/ None,
                        inherited_shell_snapshot,
                        inherited_exec_policy,
                    )
                    .await?
            }
            (None, _) => state.spawn_new_thread(config, self.clone()).await?,
        };
        agent_metadata.agent_id = Some(new_thread.thread_id);
        reservation.commit(agent_metadata.clone());

        if let Some(SessionSource::SubAgent(
            subagent_source @ SubAgentSource::ThreadSpawn {
                parent_thread_id, ..
            },
        )) = notification_source.as_ref()
            && new_thread.thread.enabled(Feature::GeneralAnalytics)
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
                    crate::codex::AppServerClientMetadata {
                        client_name: None,
                        client_version: None,
                    }
                }
            };
            let thread_config = new_thread.thread.codex.thread_config_snapshot().await;
            emit_subagent_session_started(
                &new_thread
                    .thread
                    .codex
                    .session
                    .services
                    .analytics_events_client,
                client_metadata,
                new_thread.thread_id,
                thread_config,
                subagent_source.clone(),
            );
        }

        // Notify a new thread has been created. This notification will be processed by clients
        // to subscribe or drain this newly created thread.
        // TODO(jif) add helper for drain
        state.notify_thread_created(new_thread.thread_id);

        self.persist_thread_spawn_edge_for_source(
            new_thread.thread.as_ref(),
            new_thread.thread_id,
            notification_source.as_ref(),
        )
        .await;

        self.send_input(new_thread.thread_id, initial_operation)
            .await?;
        if !new_thread.thread.enabled(Feature::MultiAgentV2) {
            let child_reference = agent_metadata
                .agent_path
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_else(|| new_thread.thread_id.to_string());
            self.maybe_start_completion_watcher(
                new_thread.thread_id,
                notification_source,
                child_reference,
                agent_metadata.agent_path.clone(),
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
        config: crate::config::Config,
        session_source: SessionSource,
        options: &SpawnAgentOptions,
        inherited_shell_snapshot: Option<Arc<ShellSnapshot>>,
        inherited_exec_policy: Option<Arc<crate::exec_policy::ExecPolicyManager>>,
    ) -> CodexResult<crate::thread_manager::NewThread> {
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
            // `record_conversation_items` only queues rollout writes asynchronously.
            // Flush/materialize the live parent before snapshotting JSONL for a fork.
            parent_thread
                .codex
                .session
                .ensure_rollout_materialized()
                .await;
            parent_thread.codex.session.flush_rollout().await;
        }

        let rollout_path = parent_thread
            .as_ref()
            .and_then(|parent_thread| parent_thread.rollout_path())
            .or(find_thread_path_by_id_str(
                config.codex_home.as_path(),
                &parent_thread_id.to_string(),
            )
            .await?)
            .ok_or_else(|| {
                CodexErr::Fatal(format!(
                    "parent thread rollout unavailable for fork: {parent_thread_id}"
                ))
            })?;

        let mut forked_rollout_items = RolloutRecorder::get_rollout_history(&rollout_path)
            .await?
            .get_rollout_items();
        if let SpawnAgentForkMode::LastNTurns(last_n_turns) = fork_mode {
            forked_rollout_items =
                truncate_rollout_to_last_n_fork_turns(&forked_rollout_items, *last_n_turns);
        }
        forked_rollout_items.retain(keep_forked_rollout_item);

        state
            .fork_thread_with_source(
                config,
                InitialHistory::Forked(forked_rollout_items),
                self.clone(),
                session_source,
                /*persist_extended_history*/ false,
                inherited_shell_snapshot,
                inherited_exec_policy,
            )
            .await
    }

    pub(crate) async fn spawn_agent_handle(
        &self,
        config: crate::config::Config,
        session_source: Option<SessionSource>,
    ) -> CodexResult<ThreadId> {
        let state = self.upgrade()?;
        let reservation = self
            .reserve_spawn_slot_with_reconcile(&state, config.agent_max_threads)
            .await?;
        let inherited_shell_snapshot = self
            .inherited_shell_snapshot_for_source(&state, session_source.as_ref())
            .await;
        let inherited_exec_policy = self
            .inherited_exec_policy_for_source(&state, session_source.as_ref(), &config)
            .await;

        let new_thread = match session_source {
            Some(session_source) => {
                state
                    .spawn_new_thread_with_source(
                        config,
                        self.clone(),
                        session_source,
                        false,
                        None,
                        inherited_shell_snapshot,
                        inherited_exec_policy,
                    )
                    .await?
            }
            None => state.spawn_new_thread(config, self.clone()).await?,
        };
        let agent_metadata = AgentMetadata {
            agent_id: Some(new_thread.thread_id),
            ..AgentMetadata::default()
        };
        reservation.commit(agent_metadata);
        state.notify_thread_created(new_thread.thread_id);
        Ok(new_thread.thread_id)
    }

    pub(crate) async fn fork_agent(
        &self,
        config: crate::config::Config,
        items: Vec<UserInput>,
        parent_thread_id: ThreadId,
        _nth_user_message: usize,
        session_source: SessionSource,
    ) -> CodexResult<ThreadId> {
        let state = self.upgrade()?;
        let reservation = self
            .reserve_spawn_slot_with_reconcile(&state, config.agent_max_threads)
            .await?;
        let inherited_shell_snapshot = self
            .inherited_shell_snapshot_for_source(&state, Some(&session_source))
            .await;
        let inherited_exec_policy = self
            .inherited_exec_policy_for_source(&state, Some(&session_source), &config)
            .await;

        let parent_thread = state.get_thread(parent_thread_id).await.ok();
        if let Some(parent_thread) = parent_thread.as_ref() {
            parent_thread
                .codex
                .session
                .ensure_rollout_materialized()
                .await;
            parent_thread.codex.session.flush_rollout().await;
        }
        let rollout_path = parent_thread
            .as_ref()
            .and_then(|thread| thread.rollout_path())
            .or(find_thread_path_by_id_str(
                config.codex_home.as_path(),
                &parent_thread_id.to_string(),
            )
            .await?)
            .ok_or_else(|| {
                CodexErr::UnsupportedOperation(format!(
                    "rollout history unavailable for thread {parent_thread_id}"
                ))
            })?;
        // Watchdog helpers must start as distinct child threads. Reusing the resume loader here
        // preserves the parent conversation id and can cause the owner to resume itself.
        let initial_history = RolloutRecorder::get_fork_history(&rollout_path).await?;

        let new_thread = state
            .fork_thread_with_source(
                config,
                initial_history,
                self.clone(),
                session_source,
                false,
                inherited_shell_snapshot,
                inherited_exec_policy,
            )
            .await?;
        let agent_metadata = AgentMetadata {
            agent_id: Some(new_thread.thread_id),
            ..AgentMetadata::default()
        };
        reservation.commit(agent_metadata);
        state.notify_thread_created(new_thread.thread_id);
        self.send_input(new_thread.thread_id, items.into()).await?;
        Ok(new_thread.thread_id)
    }

    /// Resume an existing agent thread from a recorded rollout file.
    pub(crate) async fn resume_agent_from_rollout(
        &self,
        config: crate::config::Config,
        thread_id: ThreadId,
        session_source: SessionSource,
    ) -> CodexResult<ThreadId> {
        let root_depth = thread_spawn_depth(&session_source).unwrap_or(0);
        let resumed_thread_id = self
            .resume_single_agent_from_rollout(config.clone(), thread_id, session_source)
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
                    match self
                        .resume_single_agent_from_rollout(
                            config.clone(),
                            child_thread_id,
                            child_session_source,
                        )
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

    async fn resume_single_agent_from_rollout(
        &self,
        config: crate::config::Config,
        thread_id: ThreadId,
        session_source: SessionSource,
    ) -> CodexResult<ThreadId> {
        let state = self.upgrade()?;
        let mut reservation = self.state.reserve_spawn_slot(config.agent_max_threads)?;
        let (session_source, agent_metadata) = match session_source {
            SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                parent_thread_id,
                depth,
                agent_path,
                agent_role: _,
                agent_nickname: _,
            }) => {
                let (resumed_agent_nickname, resumed_agent_role) =
                    if let Some(state_db_ctx) = state_db::get_state_db(&config).await {
                        match state_db_ctx.get_thread(thread_id).await {
                            Ok(Some(metadata)) => (metadata.agent_nickname, metadata.agent_role),
                            Ok(None) | Err(_) => (None, None),
                        }
                    } else {
                        (None, None)
                    };
                self.prepare_thread_spawn(
                    &mut reservation,
                    &config,
                    parent_thread_id,
                    depth,
                    agent_path,
                    resumed_agent_role,
                    resumed_agent_nickname,
                )?
            }
            other => (other, AgentMetadata::default()),
        };
        let notification_source = session_source.clone();
        let inherited_shell_snapshot = self
            .inherited_shell_snapshot_for_source(&state, Some(&session_source))
            .await;
        let inherited_exec_policy = self
            .inherited_exec_policy_for_source(&state, Some(&session_source), &config)
            .await;
        let rollout_path =
            match find_thread_path_by_id_str(config.codex_home.as_path(), &thread_id.to_string())
                .await?
            {
                Some(rollout_path) => rollout_path,
                None => find_archived_thread_path_by_id_str(
                    config.codex_home.as_path(),
                    &thread_id.to_string(),
                )
                .await?
                .ok_or_else(|| CodexErr::ThreadNotFound(thread_id))?,
            };

        let resumed_thread = state
            .resume_thread_from_rollout_with_source(
                config,
                rollout_path,
                self.clone(),
                session_source,
                inherited_shell_snapshot,
                inherited_exec_policy,
            )
            .await?;
        let mut agent_metadata = agent_metadata;
        agent_metadata.agent_id = Some(resumed_thread.thread_id);
        reservation.commit(agent_metadata.clone());
        // Resumed threads are re-registered in-memory and need the same listener
        // attachment path as freshly spawned threads.
        state.notify_thread_created(resumed_thread.thread_id);
        if !resumed_thread.thread.enabled(Feature::MultiAgentV2) {
            let child_reference = agent_metadata
                .agent_path
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_else(|| resumed_thread.thread_id.to_string());
            self.maybe_start_completion_watcher(
                resumed_thread.thread_id,
                Some(notification_source.clone()),
                child_reference,
                agent_metadata.agent_path.clone(),
            );
        }
        self.persist_thread_spawn_edge_for_source(
            resumed_thread.thread.as_ref(),
            resumed_thread.thread_id,
            Some(&notification_source),
        )
        .await;

        Ok(resumed_thread.thread_id)
    }

    /// Send rich user input items to an existing agent thread.
    pub(crate) async fn send_input(
        &self,
        agent_id: ThreadId,
        initial_operation: Op,
    ) -> CodexResult<String> {
        let last_task_message = render_input_preview(&initial_operation);
        let state = self.upgrade()?;
        let result = self
            .handle_thread_request_result(
                agent_id,
                &state,
                state.send_op(agent_id, initial_operation).await,
            )
            .await;
        if result.is_ok() {
            self.state
                .update_last_task_message(agent_id, last_task_message);
        }
        result
    }

    /// Append a prebuilt message to an existing agent thread outside the normal user-input path.
    #[cfg(test)]
    pub(crate) async fn append_message(
        &self,
        agent_id: ThreadId,
        message: ResponseItem,
    ) -> CodexResult<String> {
        let state = self.upgrade()?;
        self.handle_thread_request_result(
            agent_id,
            &state,
            state.append_message(agent_id, message).await,
        )
        .await
    }

    pub(crate) async fn send_inter_agent_communication(
        &self,
        agent_id: ThreadId,
        communication: InterAgentCommunication,
    ) -> CodexResult<String> {
        let last_task_message = communication.content.clone();
        let state = self.upgrade()?;
        let result = self
            .handle_thread_request_result(
                agent_id,
                &state,
                state
                    .send_op(agent_id, Op::InterAgentCommunication { communication })
                    .await,
            )
            .await;
        if result.is_ok() {
            self.state
                .update_last_task_message(agent_id, last_task_message);
        }
        result
    }

    pub(crate) async fn send_prompt(
        &self,
        agent_id: ThreadId,
        prompt: String,
    ) -> CodexResult<String> {
        self.send_input(
            agent_id,
            vec![UserInput::Text {
                text: prompt,
                text_elements: Vec::new(),
            }]
            .into(),
        )
        .await
    }

    pub(crate) async fn send_agent_message(
        &self,
        agent_id: ThreadId,
        sender_thread_id: ThreadId,
        message: String,
    ) -> CodexResult<String> {
        let state = self.upgrade()?;
        let thread = state.get_thread(agent_id).await?;
        let snapshot = thread.config_snapshot().await;
        if matches!(snapshot.session_source, SessionSource::SubAgent(_))
            || !snapshot.agent_use_function_call_inbox
        {
            return self.send_prompt(agent_id, message).await;
        }

        let result =
            inject_agent_message(&state, &thread, agent_id, sender_thread_id, message).await;
        if matches!(result, Err(CodexErr::InternalAgentDied)) {
            let _ = state.remove_thread(&agent_id).await;
            self.state.release_spawned_thread(agent_id);
        }
        result
    }

    /// Deliver watchdog wake-up input to an owner thread.
    ///
    /// This intentionally bypasses `agent_use_function_call_inbox` for non-subagent owners.
    /// Every watchdog check-in must wake the owner exactly once, and the injected inbox path
    /// reliably starts or resumes the owner's next turn while preserving helper identity.
    pub(crate) async fn send_watchdog_wakeup(
        &self,
        agent_id: ThreadId,
        sender_thread_id: ThreadId,
        message: String,
    ) -> CodexResult<String> {
        let state = self.upgrade()?;
        let thread = state.get_thread(agent_id).await?;
        let snapshot = thread.config_snapshot().await;
        let result = if matches!(snapshot.session_source, SessionSource::SubAgent(_)) {
            self.send_prompt(agent_id, message).await
        } else {
            inject_agent_message(&state, &thread, agent_id, sender_thread_id, message).await
        };
        if matches!(result, Err(CodexErr::InternalAgentDied)) {
            let _ = state.remove_thread(&agent_id).await;
            self.state.release_spawned_thread(agent_id);
        }
        result
    }

    pub(crate) async fn send_agent_message_or_input(
        &self,
        agent_id: ThreadId,
        sender_thread_id: ThreadId,
        message: Option<String>,
        items: Option<Vec<UserInput>>,
    ) -> CodexResult<String> {
        match (message, items) {
            (Some(message), None) => {
                self.send_agent_message(agent_id, sender_thread_id, message)
                    .await
            }
            (None, Some(items)) => self.send_input(agent_id, items.into()).await,
            _ => Err(CodexErr::UnsupportedOperation(
                "invalid agent input".to_string(),
            )),
        }
    }

    /// Interrupt the current task for an existing agent thread.
    pub(crate) async fn interrupt_agent(&self, agent_id: ThreadId) -> CodexResult<String> {
        let state = self.upgrade()?;
        state.send_op(agent_id, Op::Interrupt).await
    }

    async fn handle_thread_request_result(
        &self,
        agent_id: ThreadId,
        state: &Arc<ThreadManagerState>,
        result: CodexResult<String>,
    ) -> CodexResult<String> {
        if matches!(result, Err(CodexErr::InternalAgentDied)) {
            let _ = state.remove_thread(&agent_id).await;
            self.state.release_spawned_thread(agent_id);
        }
        result
    }

    /// Submit a shutdown request for a live agent without marking it explicitly closed in
    /// persisted spawn-edge state.
    pub(crate) async fn shutdown_live_agent(&self, agent_id: ThreadId) -> CodexResult<String> {
        let state = self.upgrade()?;
        if let Some(removed_watchdog) = self.watchdogs.unregister(agent_id).await
            && let Some(helper_id) = removed_watchdog.active_helper_id
        {
            let _ = state.send_op(helper_id, Op::Shutdown {}).await;
            let _ = state.remove_thread(&helper_id).await;
            self.state.release_spawned_thread(helper_id);
        }
        let result = if let Ok(thread) = state.get_thread(agent_id).await {
            thread.codex.session.ensure_rollout_materialized().await;
            thread.codex.session.flush_rollout().await;
            if matches!(thread.agent_status().await, AgentStatus::Shutdown) {
                Ok(String::new())
            } else {
                state.send_op(agent_id, Op::Shutdown {}).await
            }
        } else {
            state.send_op(agent_id, Op::Shutdown {}).await
        };
        let _ = state.remove_thread(&agent_id).await;
        self.state.release_spawned_thread(agent_id);
        result
    }

    pub(crate) async fn shutdown_agent(&self, agent_id: ThreadId) -> CodexResult<String> {
        self.shutdown_live_agent(agent_id).await
    }

    /// Mark `agent_id` as explicitly closed in persisted spawn-edge state, then shut down the
    /// agent and any live descendants reached from the in-memory tree.
    pub(crate) async fn close_agent(&self, agent_id: ThreadId) -> CodexResult<String> {
        let state = self.upgrade()?;
        if let Ok(thread) = state.get_thread(agent_id).await
            && let Some(state_db_ctx) = thread.state_db()
            && let Err(err) = state_db_ctx
                .set_thread_spawn_edge_status(agent_id, DirectionalThreadSpawnEdgeStatus::Closed)
                .await
        {
            warn!("failed to persist thread-spawn edge status for {agent_id}: {err}");
        }
        self.shutdown_agent_tree(agent_id).await
    }

    /// Shut down `agent_id` and any live descendants reachable from the in-memory spawn tree.
    async fn shutdown_agent_tree(&self, agent_id: ThreadId) -> CodexResult<String> {
        let descendant_ids = self.live_thread_spawn_descendants(agent_id).await?;
        let result = self.shutdown_live_agent(agent_id).await;
        for descendant_id in descendant_ids {
            if let Some(removed_watchdog) = self.watchdogs.unregister(descendant_id).await
                && let Some(helper_id) = removed_watchdog.active_helper_id
            {
                let _ = self.shutdown_live_agent(helper_id).await;
            }
            match self.shutdown_live_agent(descendant_id).await {
                Ok(_) | Err(CodexErr::ThreadNotFound(_)) | Err(CodexErr::InternalAgentDied) => {}
                Err(err) => return Err(err),
            }
        }
        result
    }

    /// Fetch the last known status for `agent_id`, returning `NotFound` when unavailable.
    pub(crate) async fn get_status(&self, agent_id: ThreadId) -> AgentStatus {
        let Ok(state) = self.upgrade() else {
            // No agent available if upgrade fails.
            return AgentStatus::NotFound;
        };
        let Ok(thread) = state.get_thread(agent_id).await else {
            return AgentStatus::NotFound;
        };
        thread.agent_status().await
    }

    pub(crate) fn register_session_root(
        &self,
        current_thread_id: ThreadId,
        current_session_source: &SessionSource,
    ) {
        if thread_spawn_parent_thread_id(current_session_source).is_none() {
            self.state.register_root_thread(current_thread_id);
        }
    }

    pub(crate) fn get_agent_metadata(&self, agent_id: ThreadId) -> Option<AgentMetadata> {
        self.state.agent_metadata_for_thread(agent_id)
    }

    pub(crate) async fn get_agent_config_snapshot(
        &self,
        agent_id: ThreadId,
    ) -> Option<ThreadConfigSnapshot> {
        let Ok(state) = self.upgrade() else {
            return None;
        };
        let Ok(thread) = state.get_thread(agent_id).await else {
            return None;
        };
        Some(thread.config_snapshot().await)
    }

    pub(crate) async fn resolve_agent_reference(
        &self,
        _current_thread_id: ThreadId,
        current_session_source: &SessionSource,
        agent_reference: &str,
    ) -> CodexResult<ThreadId> {
        let current_agent_path = current_session_source
            .get_agent_path()
            .unwrap_or_else(AgentPath::root);
        let agent_path = current_agent_path
            .resolve(agent_reference)
            .map_err(CodexErr::UnsupportedOperation)?;
        if let Some(thread_id) = self.state.agent_id_for_path(&agent_path) {
            return Ok(thread_id);
        }
        Err(CodexErr::UnsupportedOperation(format!(
            "live agent path `{}` not found",
            agent_path.as_str()
        )))
    }

    /// Subscribe to status updates for `agent_id`, yielding the latest value and changes.
    pub(crate) async fn subscribe_status(
        &self,
        agent_id: ThreadId,
    ) -> CodexResult<watch::Receiver<AgentStatus>> {
        let state = self.upgrade()?;
        let thread = state.get_thread(agent_id).await?;
        Ok(thread.subscribe_status())
    }

    pub(crate) async fn get_total_token_usage(&self, agent_id: ThreadId) -> Option<TokenUsage> {
        let Ok(state) = self.upgrade() else {
            return None;
        };
        let Ok(thread) = state.get_thread(agent_id).await else {
            return None;
        };
        thread.total_token_usage().await
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

        let mut live_agents = self.state.live_agents();
        live_agents.sort_by(|left, right| {
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
        let mut agents = Vec::with_capacity(live_agents.len().saturating_add(1));
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

        for metadata in live_agents {
            let Some(thread_id) = metadata.agent_id else {
                continue;
            };
            if resolved_prefix
                .as_ref()
                .is_some_and(|prefix| !agent_matches_prefix(metadata.agent_path.as_ref(), prefix))
            {
                continue;
            }

            let Ok(thread) = state.get_thread(thread_id).await else {
                continue;
            };
            let agent_name = metadata
                .agent_path
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_else(|| thread_id.to_string());
            let last_task_message = metadata.last_task_message.clone();
            agents.push(ListedAgent {
                agent_name,
                agent_status: thread.agent_status().await,
                last_task_message,
            });
        }

        Ok(agents)
    }

    /// Starts a detached watcher for sub-agents spawned from another thread.
    ///
    /// This is only enabled for `SubAgentSource::ThreadSpawn`, where a parent thread exists and
    /// can receive completion notifications.
    fn maybe_start_completion_watcher(
        &self,
        child_thread_id: ThreadId,
        session_source: Option<SessionSource>,
        child_reference: String,
        child_agent_path: Option<AgentPath>,
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

            if let Some(owner_thread_id) = control
                .watchdogs
                .owner_for_active_helper(child_thread_id)
                .await
            {
                let helper_sent_input = match control.upgrade() {
                    Ok(state) => state
                        .get_thread(child_thread_id)
                        .await
                        .ok()
                        .map(|thread| thread.last_completed_turn_used_agent_send_input())
                        .unwrap_or(false),
                    Err(_) => false,
                };
                if !helper_sent_input {
                    let fallback_message = match &status {
                        AgentStatus::Completed(Some(message)) if !message.trim().is_empty() => {
                            Some(message.clone())
                        }
                        AgentStatus::Completed(_) => Some(
                            "Watchdog check-in completed without calling send_input or returning a final message."
                                .to_string(),
                        ),
                        AgentStatus::Errored(message) if !message.trim().is_empty() => {
                            Some(message.clone())
                        }
                        _ => None,
                    };
                    if let Some(message) = fallback_message {
                        let _ = control
                            .send_watchdog_wakeup(owner_thread_id, child_thread_id, message)
                            .await;
                    }
                }
                return;
            }

            let Ok(state) = control.upgrade() else {
                return;
            };
            let child_thread = state.get_thread(child_thread_id).await.ok();
            let message = format_subagent_notification_message(child_reference.as_str(), &status);
            if child_agent_path.is_some()
                && child_thread
                    .as_ref()
                    .map(|thread| thread.enabled(Feature::MultiAgentV2))
                    .unwrap_or(true)
            {
                let Some(child_agent_path) = child_agent_path.clone() else {
                    return;
                };
                let Some(parent_agent_path) = child_agent_path
                    .as_str()
                    .rsplit_once('/')
                    .and_then(|(parent, _)| AgentPath::try_from(parent).ok())
                else {
                    return;
                };
                let communication = InterAgentCommunication::new(
                    child_agent_path,
                    parent_agent_path,
                    Vec::new(),
                    message,
                    /*trigger_turn*/ false,
                );
                let _ = control
                    .send_inter_agent_communication(parent_thread_id, communication)
                    .await;
                return;
            }
            let Ok(parent_thread) = state.get_thread(parent_thread_id).await else {
                return;
            };
            parent_thread
                .inject_user_message_without_turn(message)
                .await;
        });
    }

    pub(crate) async fn watchdog_targets(&self, agent_ids: &[ThreadId]) -> HashSet<ThreadId> {
        self.watchdogs.registered_targets(agent_ids).await
    }

    pub(crate) async fn register_watchdog(
        &self,
        registration: WatchdogRegistration,
    ) -> CodexResult<Vec<RemovedWatchdog>> {
        self.watchdogs.register(registration).await
    }

    pub(crate) async fn unregister_watchdog(
        &self,
        target_thread_id: ThreadId,
    ) -> Option<RemovedWatchdog> {
        self.watchdogs.unregister(target_thread_id).await
    }

    pub(crate) async fn unregister_watchdogs_for_owner(
        &self,
        owner_thread_id: ThreadId,
    ) -> Vec<RemovedWatchdog> {
        self.watchdogs.take_for_owner(owner_thread_id).await
    }

    pub(crate) async fn compact_parent_for_watchdog_helper(
        &self,
        helper_thread_id: ThreadId,
    ) -> CodexResult<WatchdogParentCompactionResult> {
        let Some(parent_thread_id) = self
            .watchdogs
            .owner_for_active_helper(helper_thread_id)
            .await
        else {
            return Ok(WatchdogParentCompactionResult::NotWatchdogHelper);
        };
        let state = self.upgrade()?;
        let parent_thread = state.get_thread(parent_thread_id).await?;
        let parent_has_active_turn = parent_thread.has_active_turn().await;

        {
            let mut compacting = self.watchdog_compactions_in_progress.lock().await;
            if compacting.contains(&parent_thread_id) {
                return Ok(WatchdogParentCompactionResult::AlreadyInProgress { parent_thread_id });
            }
            if parent_has_active_turn {
                return Ok(WatchdogParentCompactionResult::ParentBusy { parent_thread_id });
            }
            compacting.insert(parent_thread_id);
        }

        match state.send_op(parent_thread_id, Op::Compact).await {
            Ok(submission_id) => Ok(WatchdogParentCompactionResult::Submitted {
                parent_thread_id,
                submission_id,
            }),
            Err(err) => {
                let mut compacting = self.watchdog_compactions_in_progress.lock().await;
                compacting.remove(&parent_thread_id);
                Err(err)
            }
        }
    }

    pub(crate) async fn finish_watchdog_parent_compaction(&self, parent_thread_id: ThreadId) {
        let mut compacting = self.watchdog_compactions_in_progress.lock().await;
        compacting.remove(&parent_thread_id);
    }

    #[cfg(test)]
    pub(crate) async fn run_watchdogs_once_for_tests(&self) {
        self.watchdogs.run_once().await;
    }

    #[cfg(test)]
    pub(crate) async fn force_watchdog_due_for_tests(&self, target_thread_id: ThreadId) {
        self.watchdogs.force_due_for_tests(target_thread_id).await;
    }

    #[cfg(test)]
    pub(crate) async fn set_watchdog_active_helper_for_tests(
        &self,
        target_thread_id: ThreadId,
        helper_thread_id: ThreadId,
    ) {
        self.watchdogs
            .set_active_helper_for_tests(target_thread_id, helper_thread_id)
            .await;
    }

    pub(crate) async fn watchdog_owner_for_active_helper(
        &self,
        helper_thread_id: ThreadId,
    ) -> Option<ThreadId> {
        self.watchdogs
            .owner_for_active_helper(helper_thread_id)
            .await
    }

    pub(crate) async fn list_agents_legacy(
        &self,
        owner_thread_id: ThreadId,
        recursive: bool,
        all: bool,
    ) -> CodexResult<Vec<AgentListing>> {
        let state = self.upgrade()?;
        let thread_ids = state.list_thread_ids().await;

        let mut parent_by_thread = HashMap::with_capacity(thread_ids.len());
        let mut status_by_thread = HashMap::with_capacity(thread_ids.len());
        let mut depth_by_thread = HashMap::with_capacity(thread_ids.len());

        for thread_id in &thread_ids {
            let Ok(thread) = state.get_thread(*thread_id).await else {
                continue;
            };
            let snapshot = thread.config_snapshot().await;
            let (parent_thread_id, depth) = match snapshot.session_source {
                SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                    parent_thread_id,
                    depth,
                    ..
                }) => (
                    Some(parent_thread_id),
                    usize::try_from(depth).unwrap_or_default(),
                ),
                _ => (None, 0),
            };
            parent_by_thread.insert(*thread_id, parent_thread_id);
            status_by_thread.insert(*thread_id, thread.agent_status().await);
            depth_by_thread.insert(*thread_id, depth);
        }

        let mut children_by_parent: HashMap<ThreadId, Vec<ThreadId>> = HashMap::new();
        for (thread_id, parent_thread_id) in &parent_by_thread {
            if let Some(parent_thread_id) = parent_thread_id {
                children_by_parent
                    .entry(*parent_thread_id)
                    .or_default()
                    .push(*thread_id);
            }
        }
        for children in children_by_parent.values_mut() {
            children.sort_by_key(ToString::to_string);
        }

        let mut listings = Vec::new();
        if all {
            let mut all_thread_ids = thread_ids.into_iter().collect::<HashSet<_>>();
            all_thread_ids.extend(self.state.tracked_thread_ids());
            let mut all_thread_ids = all_thread_ids.into_iter().collect::<Vec<_>>();
            all_thread_ids.sort_by_key(ToString::to_string);
            for thread_id in all_thread_ids {
                listings.push(AgentListing {
                    thread_id,
                    parent_thread_id: parent_by_thread.get(&thread_id).copied().flatten(),
                    status: status_by_thread
                        .get(&thread_id)
                        .cloned()
                        .unwrap_or(AgentStatus::NotFound),
                    depth: depth_by_thread.get(&thread_id).copied().unwrap_or_default(),
                });
            }
            return Ok(listings);
        }

        let mut queue = VecDeque::new();
        if let Some(children) = children_by_parent.get(&owner_thread_id) {
            for child in children {
                queue.push_back((*child, 1));
            }
        }

        while let Some((thread_id, depth)) = queue.pop_front() {
            listings.push(AgentListing {
                thread_id,
                parent_thread_id: parent_by_thread.get(&thread_id).copied().flatten(),
                status: status_by_thread
                    .get(&thread_id)
                    .cloned()
                    .unwrap_or(AgentStatus::NotFound),
                depth,
            });

            if recursive && let Some(children) = children_by_parent.get(&thread_id) {
                for child in children {
                    queue.push_back((*child, depth + 1));
                }
            }
        }

        Ok(listings)
    }

    #[allow(clippy::too_many_arguments)]
    fn prepare_thread_spawn(
        &self,
        reservation: &mut crate::agent::registry::SpawnReservation,
        config: &crate::config::Config,
        parent_thread_id: ThreadId,
        depth: i32,
        agent_path: Option<AgentPath>,
        agent_role: Option<String>,
        preferred_agent_nickname: Option<String>,
    ) -> CodexResult<(SessionSource, AgentMetadata)> {
        if depth == 1 {
            self.state.register_root_thread(parent_thread_id);
        }
        if let Some(agent_path) = agent_path.as_ref() {
            reservation.reserve_agent_path(agent_path)?;
        }
        let candidate_names = agent_nickname_candidates(config, agent_role.as_deref());
        let candidate_name_refs: Vec<&str> = candidate_names.iter().map(String::as_str).collect();
        let agent_nickname = Some(reservation.reserve_agent_nickname_with_preference(
            &candidate_name_refs,
            preferred_agent_nickname.as_deref(),
        )?);
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

    fn upgrade(&self) -> CodexResult<Arc<ThreadManagerState>> {
        self.manager
            .upgrade()
            .ok_or_else(|| CodexErr::UnsupportedOperation("thread manager dropped".to_string()))
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

    async fn reserve_spawn_slot_with_reconcile(
        &self,
        state: &ThreadManagerState,
        max_threads: Option<usize>,
    ) -> CodexResult<crate::agent::registry::SpawnReservation> {
        self.reconcile_stale_guard_slots(state).await;
        match self.state.reserve_spawn_slot(max_threads) {
            Ok(reservation) => Ok(reservation),
            Err(CodexErr::AgentLimitReached { .. }) => {
                self.reconcile_stale_guard_slots(state).await;
                self.state.reserve_spawn_slot(max_threads)
            }
            Err(err) => Err(err),
        }
    }

    async fn reconcile_stale_guard_slots(&self, state: &ThreadManagerState) {
        let live_thread_ids: HashSet<ThreadId> =
            state.list_thread_ids().await.into_iter().collect();
        for tracked_thread_id in self.state.tracked_thread_ids() {
            if !live_thread_ids.contains(&tracked_thread_id) {
                self.state.release_spawned_thread(tracked_thread_id);
            }
        }
    }

    async fn inherited_exec_policy_for_source(
        &self,
        state: &Arc<ThreadManagerState>,
        session_source: Option<&SessionSource>,
        child_config: &crate::config::Config,
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

        for thread_id in state.list_thread_ids().await {
            let Ok(thread) = state.get_thread(thread_id).await else {
                continue;
            };
            let snapshot = thread.config_snapshot().await;
            let Some(parent_thread_id) = thread_spawn_parent_thread_id(&snapshot.session_source)
            else {
                continue;
            };
            children_by_parent
                .entry(parent_thread_id)
                .or_default()
                .push((
                    thread_id,
                    self.state
                        .agent_metadata_for_thread(thread_id)
                        .unwrap_or(AgentMetadata {
                            agent_id: Some(thread_id),
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

    async fn persist_thread_spawn_edge_for_source(
        &self,
        thread: &crate::CodexThread,
        child_thread_id: ThreadId,
        session_source: Option<&SessionSource>,
    ) {
        let Some(parent_thread_id) = session_source.and_then(thread_spawn_parent_thread_id) else {
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

fn thread_spawn_parent_thread_id(session_source: &SessionSource) -> Option<ThreadId> {
    match session_source {
        SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
            parent_thread_id, ..
        }) => Some(*parent_thread_id),
        _ => None,
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

pub(crate) fn render_input_preview(initial_operation: &Op) -> String {
    match initial_operation {
        Op::UserInput { items, .. } => items
            .iter()
            .map(|item| match item {
                UserInput::Text { text, .. } => text.clone(),
                UserInput::Image { .. } => "[image]".to_string(),
                UserInput::LocalImage { path } => format!("[local_image:{}]", path.display()),
                UserInput::Skill { name, path } => format!("[skill:${name}]({})", path.display()),
                UserInput::Mention { name, path } => format!("[mention:${name}]({path})"),
                _ => "[input]".to_string(),
            })
            .collect::<Vec<_>>()
            .join("\n"),
        Op::InterAgentCommunication { communication } => communication.content.clone(),
        _ => String::new(),
    }
}

fn thread_spawn_depth(session_source: &SessionSource) -> Option<i32> {
    match session_source {
        SessionSource::SubAgent(SubAgentSource::ThreadSpawn { depth, .. }) => Some(*depth),
        _ => None,
    }
}

fn build_agent_inbox_items(
    sender_thread_id: ThreadId,
    message: String,
    prepend_turn_start_user_message: bool,
) -> CodexResult<Vec<ResponseInputItem>> {
    let mut items = Vec::new();
    if prepend_turn_start_user_message {
        items.push(ResponseInputItem::Message {
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: String::new(),
            }],
        });
    }

    let call_id = format!("agent_inbox_{}", Uuid::new_v4());
    let output = serde_json::to_string(&AgentInboxPayload::new(sender_thread_id, message))
        .map_err(|err| {
            CodexErr::UnsupportedOperation(format!(
                "failed to serialize agent inbox payload: {err}"
            ))
        })?;

    items.extend([
        ResponseInputItem::FunctionCall {
            name: AGENT_INBOX_KIND.to_string(),
            arguments: "{}".to_string(),
            call_id: call_id.clone(),
        },
        ResponseInputItem::FunctionCallOutput {
            call_id,
            output: FunctionCallOutputPayload {
                body: FunctionCallOutputBody::Text(output),
                ..Default::default()
            },
        },
    ]);

    Ok(items)
}

async fn inject_agent_message(
    state: &ThreadManagerState,
    thread: &Arc<crate::CodexThread>,
    agent_id: ThreadId,
    sender_thread_id: ThreadId,
    message: String,
) -> CodexResult<String> {
    let prepend_turn_start_user_message = !thread.codex.session.active_turn.lock().await.is_some();
    state
        .send_op(
            agent_id,
            Op::InjectResponseItems {
                items: build_agent_inbox_items(
                    sender_thread_id,
                    message,
                    prepend_turn_start_user_message,
                )?,
            },
        )
        .await
}

#[cfg(test)]
#[path = "control_tests.rs"]
mod tests;
// Keep inbox coverage in `control_tests.rs`. The large inline test module below is a stale
// replay artifact from older pre-refactor rebases and no longer matches current core test APIs.
#[cfg(any())]
mod inbox_tests {
    use super::*;
    use crate::CodexAuth;
    use crate::CodexThread;
    use crate::ThreadManager;
    use crate::agent::agent_status_from_event;
    use crate::config::AgentRoleConfig;
    use crate::config::Config;
    use crate::config::ConfigBuilder;
    use crate::config_loader::LoaderOverrides;
    use crate::contextual_user_message::SUBAGENT_NOTIFICATION_OPEN_TAG;
    use crate::features::Feature;
    use assert_matches::assert_matches;
    use codex_protocol::config_types::ModeKind;
    use codex_protocol::models::ContentItem;
    use codex_protocol::models::ResponseInputItem;
    use codex_protocol::models::ResponseItem;
    use codex_protocol::protocol::ErrorEvent;
    use codex_protocol::protocol::EventMsg;
    use codex_protocol::protocol::SessionSource;
    use codex_protocol::protocol::SubAgentSource;
    use codex_protocol::protocol::TurnAbortReason;
    use codex_protocol::protocol::TurnAbortedEvent;
    use codex_protocol::protocol::TurnCompleteEvent;
    use codex_protocol::protocol::TurnStartedEvent;
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;
    use tokio::time::Duration;
    use tokio::time::sleep;
    use tokio::time::timeout;
    use toml::Value as TomlValue;

    async fn test_config_with_cli_overrides(
        cli_overrides: Vec<(String, TomlValue)>,
    ) -> (TempDir, Config) {
        let home = TempDir::new().expect("create temp dir");
        let config = ConfigBuilder::default()
            .codex_home(home.path().to_path_buf())
            .cli_overrides(cli_overrides)
            .loader_overrides(LoaderOverrides {
                #[cfg(target_os = "macos")]
                managed_preferences_base64: Some(String::new()),
                macos_managed_config_requirements_base64: Some(String::new()),
                ..LoaderOverrides::default()
            })
            .build()
            .await
            .expect("load default test config");
        (home, config)
    }

    async fn test_config() -> (TempDir, Config) {
        test_config_with_cli_overrides(Vec::new()).await
    }

    fn text_input(text: &str) -> Vec<UserInput> {
        vec![UserInput::Text {
            text: text.to_string(),
            text_elements: Vec::new(),
        }]
    }

    struct AgentControlHarness {
        _home: TempDir,
        config: Config,
        manager: ThreadManager,
        control: AgentControl,
    }

    impl AgentControlHarness {
        async fn new() -> Self {
            let (home, config) = test_config().await;
            let manager = ThreadManager::with_models_provider_and_home_for_tests(
                CodexAuth::from_api_key("dummy"),
                config.model_provider.clone(),
                config.codex_home.clone(),
            );
            let control = manager.agent_control();
            Self {
                _home: home,
                config,
                manager,
                control,
            }
        }

        async fn start_thread(&self) -> (ThreadId, Arc<CodexThread>) {
            let new_thread = self
                .manager
                .start_thread(self.config.clone())
                .await
                .expect("start thread");
            (new_thread.thread_id, new_thread.thread)
        }
    }

    #[test]
    fn build_agent_inbox_items_emits_function_call_and_output() {
        let sender_thread_id = ThreadId::new();
        let items = build_agent_inbox_items(sender_thread_id, "watchdog update".to_string(), false)
            .expect("tool role should build inbox items");

        assert_eq!(items.len(), 2);

        let call_id = match &items[0] {
            ResponseInputItem::FunctionCall {
                name,
                arguments,
                call_id,
            } => {
                assert_eq!(name, AGENT_INBOX_KIND);
                assert_eq!(arguments, "{}");
                call_id.clone()
            }
            other => panic!("expected function call item, got {other:?}"),
        };

        match &items[1] {
            ResponseInputItem::FunctionCallOutput {
                call_id: output_call_id,
                output,
            } => {
                assert_eq!(output_call_id, &call_id);
                let output_text = output
                    .body
                    .to_text()
                    .expect("payload should convert to text");
                let payload: AgentInboxPayload =
                    serde_json::from_str(&output_text).expect("payload should be valid json");
                assert!(payload.injected);
                assert_eq!(payload.kind, AGENT_INBOX_KIND);
                assert_eq!(payload.sender_thread_id, sender_thread_id);
                assert_eq!(payload.message, "watchdog update");
            }
            other => panic!("expected function call output item, got {other:?}"),
        }
    }

    #[test]
    fn build_agent_inbox_items_prepends_empty_user_message_when_requested() {
        let sender_thread_id = ThreadId::new();
        let items = build_agent_inbox_items(sender_thread_id, "watchdog update".to_string(), true)
            .expect("tool role should build inbox items");

        assert_eq!(items.len(), 3);
        assert_eq!(
            items[0],
            ResponseInputItem::Message {
                role: "user".to_string(),
                content: vec![ContentItem::InputText {
                    text: String::new(),
                }],
            }
        );
        assert_matches!(&items[1], ResponseInputItem::FunctionCall { .. });
        assert_matches!(&items[2], ResponseInputItem::FunctionCallOutput { .. });
    }

    #[tokio::test]
    async fn send_agent_message_to_root_thread_defaults_to_user_input() {
        let harness = AgentControlHarness::new().await;
        let (receiver_thread_id, _thread) = harness.start_thread().await;
        let sender_thread_id = ThreadId::new();

        let submission_id = harness
            .control
            .send_agent_message(
                receiver_thread_id,
                sender_thread_id,
                "watchdog update".to_string(),
            )
            .await
            .expect("send_agent_message should succeed");
        assert!(!submission_id.is_empty());

        let expected = (
            receiver_thread_id,
            Op::UserInput {
                items: vec![UserInput::Text {
                    text: "watchdog update".to_string(),
                    text_elements: Vec::new(),
                }],
                final_output_json_schema: None,
            },
        );
        let captured = harness
            .manager
            .captured_ops()
            .into_iter()
            .find(|entry| *entry == expected);

        assert_eq!(captured, Some(expected));
    }

    #[tokio::test]
    async fn send_agent_message_to_root_thread_injects_response_items_when_enabled() {
        let mut harness = AgentControlHarness::new().await;
        harness.config.agent_use_function_call_inbox = true;
        let (receiver_thread_id, _thread) = harness.start_thread().await;
        let sender_thread_id = ThreadId::new();

        let submission_id = harness
            .control
            .send_agent_message(
                receiver_thread_id,
                sender_thread_id,
                "watchdog update".to_string(),
            )
            .await
            .expect("send_agent_message should succeed");
        assert!(!submission_id.is_empty());

        let captured = harness
            .manager
            .captured_ops()
            .into_iter()
            .find(|(thread_id, op)| {
                *thread_id == receiver_thread_id && matches!(op, Op::InjectResponseItems { .. })
            })
            .expect("expected injected agent inbox op");

        let Op::InjectResponseItems { items } = captured.1 else {
            unreachable!("matched above");
        };
        assert_eq!(items.len(), 3);
        match &items[0] {
            ResponseInputItem::Message { role, content } => {
                assert_eq!(role, "user");
                assert_eq!(
                    content,
                    &vec![ContentItem::InputText {
                        text: String::new(),
                    }]
                );
            }
            other => panic!("expected prepended user message, got {other:?}"),
        }
        match &items[1] {
            ResponseInputItem::FunctionCall {
                name, arguments, ..
            } => {
                assert_eq!(name, AGENT_INBOX_KIND);
                assert_eq!(arguments, "{}");
            }
            other => panic!("expected function call item, got {other:?}"),
        }
        match &items[2] {
            ResponseInputItem::FunctionCallOutput { output, .. } => {
                let output_text = output
                    .body
                    .to_text()
                    .expect("payload should convert to text");
                let payload: AgentInboxPayload =
                    serde_json::from_str(&output_text).expect("payload should be valid json");
                assert_eq!(payload.sender_thread_id, sender_thread_id);
                assert_eq!(payload.message, "watchdog update");
            }
            other => panic!("expected function call output item, got {other:?}"),
        }
    }

    fn has_subagent_notification(history_items: &[ResponseItem]) -> bool {
        history_items.iter().any(|item| {
            let ResponseItem::Message { role, content, .. } = item else {
                return false;
            };
            if role != "user" {
                return false;
            }
            content.iter().any(|content_item| match content_item {
                ContentItem::InputText { text } | ContentItem::OutputText { text } => {
                    text.contains(SUBAGENT_NOTIFICATION_OPEN_TAG)
                }
                ContentItem::InputImage { .. } => false,
            })
        })
    }

    /// Returns true when any message item contains `needle` in a text span.
    fn history_contains_text(history_items: &[ResponseItem], needle: &str) -> bool {
        history_items.iter().any(|item| {
            let ResponseItem::Message { content, .. } = item else {
                return false;
            };
            content.iter().any(|content_item| match content_item {
                ContentItem::InputText { text } | ContentItem::OutputText { text } => {
                    text.contains(needle)
                }
                ContentItem::InputImage { .. } => false,
            })
        })
    }

    async fn wait_for_subagent_notification(parent_thread: &Arc<CodexThread>) -> bool {
        let wait = async {
            loop {
                let history_items = parent_thread
                    .codex
                    .session
                    .clone_history()
                    .await
                    .raw_items()
                    .to_vec();
                if has_subagent_notification(&history_items) {
                    return true;
                }
                sleep(Duration::from_millis(25)).await;
            }
        };
        timeout(Duration::from_secs(5), wait).await.is_ok()
    }

    #[tokio::test]
    async fn send_input_errors_when_manager_dropped() {
        let control = AgentControl::default();
        let err = control
            .send_input(
                ThreadId::new(),
                vec![UserInput::Text {
                    text: "hello".to_string(),
                    text_elements: Vec::new(),
                }],
            )
            .await
            .expect_err("send_input should fail without a manager");
        assert_eq!(
            err.to_string(),
            "unsupported operation: thread manager dropped"
        );
    }

    #[tokio::test]
    async fn get_status_returns_not_found_without_manager() {
        let control = AgentControl::default();
        let got = control.get_status(ThreadId::new()).await;
        assert_eq!(got, AgentStatus::NotFound);
    }

    #[tokio::test]
    async fn on_event_updates_status_from_task_started() {
        let status = agent_status_from_event(&EventMsg::TurnStarted(TurnStartedEvent {
            turn_id: "turn-1".to_string(),
            model_context_window: None,
            collaboration_mode_kind: ModeKind::Default,
        }));
        assert_eq!(status, Some(AgentStatus::Running));
    }

    #[tokio::test]
    async fn on_event_updates_status_from_task_complete() {
        let status = agent_status_from_event(&EventMsg::TurnComplete(TurnCompleteEvent {
            turn_id: "turn-1".to_string(),
            last_agent_message: Some("done".to_string()),
        }));
        let expected = AgentStatus::Completed(Some("done".to_string()));
        assert_eq!(status, Some(expected));
    }

    #[tokio::test]
    async fn on_event_updates_status_from_error() {
        let status = agent_status_from_event(&EventMsg::Error(ErrorEvent {
            message: "boom".to_string(),
            codex_error_info: None,
        }));

        let expected = AgentStatus::Errored("boom".to_string());
        assert_eq!(status, Some(expected));
    }

    #[tokio::test]
    async fn on_event_updates_status_from_turn_aborted() {
        let status = agent_status_from_event(&EventMsg::TurnAborted(TurnAbortedEvent {
            turn_id: Some("turn-1".to_string()),
            reason: TurnAbortReason::Interrupted,
        }));

        let expected = AgentStatus::Errored("Interrupted".to_string());
        assert_eq!(status, Some(expected));
    }

    #[tokio::test]
    async fn on_event_updates_status_from_shutdown_complete() {
        let status = agent_status_from_event(&EventMsg::ShutdownComplete);
        assert_eq!(status, Some(AgentStatus::Shutdown));
    }

    #[tokio::test]
    async fn spawn_agent_errors_when_manager_dropped() {
        let control = AgentControl::default();
        let (_home, config) = test_config().await;
        let err = control
            .spawn_agent(config, text_input("hello"), None)
            .await
            .expect_err("spawn_agent should fail without a manager");
        assert_eq!(
            err.to_string(),
            "unsupported operation: thread manager dropped"
        );
    }

    #[tokio::test]
    async fn resume_agent_errors_when_manager_dropped() {
        let control = AgentControl::default();
        let (_home, config) = test_config().await;
        let err = control
            .resume_agent_from_rollout(config, ThreadId::new(), SessionSource::Exec)
            .await
            .expect_err("resume_agent should fail without a manager");
        assert_eq!(
            err.to_string(),
            "unsupported operation: thread manager dropped"
        );
    }

    #[tokio::test]
    async fn send_input_errors_when_thread_missing() {
        let harness = AgentControlHarness::new().await;
        let thread_id = ThreadId::new();
        let err = harness
            .control
            .send_input(
                thread_id,
                vec![UserInput::Text {
                    text: "hello".to_string(),
                    text_elements: Vec::new(),
                }],
            )
            .await
            .expect_err("send_input should fail for missing thread");
        assert_matches!(err, CodexErr::ThreadNotFound(id) if id == thread_id);
    }

    #[tokio::test]
    async fn get_status_returns_not_found_for_missing_thread() {
        let harness = AgentControlHarness::new().await;
        let status = harness.control.get_status(ThreadId::new()).await;
        assert_eq!(status, AgentStatus::NotFound);
    }

    #[tokio::test]
    async fn get_status_returns_pending_init_for_new_thread() {
        let harness = AgentControlHarness::new().await;
        let (thread_id, _) = harness.start_thread().await;
        let status = harness.control.get_status(thread_id).await;
        assert_eq!(status, AgentStatus::PendingInit);
    }

    #[tokio::test]
    async fn subscribe_status_errors_for_missing_thread() {
        let harness = AgentControlHarness::new().await;
        let thread_id = ThreadId::new();
        let err = harness
            .control
            .subscribe_status(thread_id)
            .await
            .expect_err("subscribe_status should fail for missing thread");
        assert_matches!(err, CodexErr::ThreadNotFound(id) if id == thread_id);
    }

    #[tokio::test]
    async fn subscribe_status_updates_on_shutdown() {
        let harness = AgentControlHarness::new().await;
        let (thread_id, thread) = harness.start_thread().await;
        let mut status_rx = harness
            .control
            .subscribe_status(thread_id)
            .await
            .expect("subscribe_status should succeed");
        assert_eq!(status_rx.borrow().clone(), AgentStatus::PendingInit);

        let _ = thread
            .submit(Op::Shutdown {})
            .await
            .expect("shutdown should submit");

        let _ = status_rx.changed().await;
        assert_eq!(status_rx.borrow().clone(), AgentStatus::Shutdown);
    }

    #[tokio::test]
    async fn send_input_submits_user_message() {
        let harness = AgentControlHarness::new().await;
        let (thread_id, _thread) = harness.start_thread().await;

        let submission_id = harness
            .control
            .send_input(
                thread_id,
                vec![UserInput::Text {
                    text: "hello from tests".to_string(),
                    text_elements: Vec::new(),
                }],
            )
            .await
            .expect("send_input should succeed");
        assert!(!submission_id.is_empty());
        let expected = (
            thread_id,
            Op::UserInput {
                items: vec![UserInput::Text {
                    text: "hello from tests".to_string(),
                    text_elements: Vec::new(),
                }],
                final_output_json_schema: None,
            },
        );
        let captured = harness
            .manager
            .captured_ops()
            .into_iter()
            .find(|entry| *entry == expected);
        assert_eq!(captured, Some(expected));
    }

    #[tokio::test]
    async fn spawn_agent_creates_thread_and_sends_prompt() {
        let harness = AgentControlHarness::new().await;
        let thread_id = harness
            .control
            .spawn_agent(harness.config.clone(), text_input("spawned"), None)
            .await
            .expect("spawn_agent should succeed");
        let _thread = harness
            .manager
            .get_thread(thread_id)
            .await
            .expect("thread should be registered");
        let expected = (
            thread_id,
            Op::UserInput {
                items: vec![UserInput::Text {
                    text: "spawned".to_string(),
                    text_elements: Vec::new(),
                }],
                final_output_json_schema: None,
            },
        );
        let captured = harness
            .manager
            .captured_ops()
            .into_iter()
            .find(|entry| *entry == expected);
        assert_eq!(captured, Some(expected));
    }

    #[tokio::test]
    async fn spawn_agent_can_fork_parent_thread_history() {
        let harness = AgentControlHarness::new().await;
        let (parent_thread_id, parent_thread) = harness.start_thread().await;
        parent_thread
            .inject_user_message_without_turn("parent seed context".to_string())
            .await;
        let turn_context = parent_thread.codex.session.new_default_turn().await;
        let parent_spawn_call_id = "spawn-call-history".to_string();
        let parent_spawn_call = ResponseItem::FunctionCall {
            id: None,
            name: "spawn_agent".to_string(),
            namespace: None,
            arguments: "{}".to_string(),
            call_id: parent_spawn_call_id.clone(),
        };
        parent_thread
            .codex
            .session
            .record_conversation_items(turn_context.as_ref(), &[parent_spawn_call])
            .await;
        parent_thread
            .codex
            .session
            .ensure_rollout_materialized()
            .await;
        parent_thread.codex.session.flush_rollout().await;

        let child_thread_id = harness
            .control
            .spawn_agent_with_options(
                harness.config.clone(),
                text_input("child task"),
                Some(SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                    parent_thread_id,
                    depth: 1,
                    agent_nickname: None,
                    agent_role: None,
                })),
                SpawnAgentOptions {
                    fork_parent_spawn_call_id: Some(parent_spawn_call_id),
                },
            )
            .await
            .expect("forked spawn should succeed");

        let child_thread = harness
            .manager
            .get_thread(child_thread_id)
            .await
            .expect("child thread should be registered");
        assert_ne!(child_thread_id, parent_thread_id);
        let history = child_thread.codex.session.clone_history().await;
        assert!(history_contains_text(
            history.raw_items(),
            "parent seed context"
        ));

        let expected = (
            child_thread_id,
            Op::UserInput {
                items: vec![UserInput::Text {
                    text: "child task".to_string(),
                    text_elements: Vec::new(),
                }],
                final_output_json_schema: None,
            },
        );
        let captured = harness
            .manager
            .captured_ops()
            .into_iter()
            .find(|entry| *entry == expected);
        assert_eq!(captured, Some(expected));

        let _ = harness
            .control
            .shutdown_agent(child_thread_id)
            .await
            .expect("child shutdown should submit");
        let _ = parent_thread
            .submit(Op::Shutdown {})
            .await
            .expect("parent shutdown should submit");
    }

    #[tokio::test]
    async fn spawn_agent_fork_injects_output_for_parent_spawn_call() {
        let harness = AgentControlHarness::new().await;
        let (parent_thread_id, parent_thread) = harness.start_thread().await;
        let turn_context = parent_thread.codex.session.new_default_turn().await;
        let parent_spawn_call_id = "spawn-call-1".to_string();
        let parent_spawn_call = ResponseItem::FunctionCall {
            id: None,
            name: "spawn_agent".to_string(),
            namespace: None,
            arguments: "{}".to_string(),
            call_id: parent_spawn_call_id.clone(),
        };
        parent_thread
            .codex
            .session
            .record_conversation_items(turn_context.as_ref(), &[parent_spawn_call])
            .await;
        parent_thread
            .codex
            .session
            .ensure_rollout_materialized()
            .await;
        parent_thread.codex.session.flush_rollout().await;

        let child_thread_id = harness
            .control
            .spawn_agent_with_options(
                harness.config.clone(),
                text_input("child task"),
                Some(SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                    parent_thread_id,
                    depth: 1,
                    agent_nickname: None,
                    agent_role: None,
                })),
                SpawnAgentOptions {
                    fork_parent_spawn_call_id: Some(parent_spawn_call_id.clone()),
                },
            )
            .await
            .expect("forked spawn should succeed");

        let child_thread = harness
            .manager
            .get_thread(child_thread_id)
            .await
            .expect("child thread should be registered");
        let history = child_thread.codex.session.clone_history().await;
        let injected_output = history.raw_items().iter().find_map(|item| match item {
            ResponseItem::FunctionCallOutput { call_id, output }
                if call_id == &parent_spawn_call_id =>
            {
                Some(output)
            }
            _ => None,
        });
        let injected_output =
            injected_output.expect("forked child should contain synthetic tool output");
        assert_eq!(
            injected_output.text_content(),
            Some(FORKED_SPAWN_AGENT_OUTPUT_MESSAGE)
        );
        assert_eq!(injected_output.success, Some(true));

        let _ = harness
            .control
            .shutdown_agent(child_thread_id)
            .await
            .expect("child shutdown should submit");
        let _ = parent_thread
            .submit(Op::Shutdown {})
            .await
            .expect("parent shutdown should submit");
    }

    #[tokio::test]
    async fn spawn_agent_fork_flushes_parent_rollout_before_loading_history() {
        let harness = AgentControlHarness::new().await;
        let (parent_thread_id, parent_thread) = harness.start_thread().await;
        let turn_context = parent_thread.codex.session.new_default_turn().await;
        let parent_spawn_call_id = "spawn-call-unflushed".to_string();
        let parent_spawn_call = ResponseItem::FunctionCall {
            id: None,
            name: "spawn_agent".to_string(),
            namespace: None,
            arguments: "{}".to_string(),
            call_id: parent_spawn_call_id.clone(),
        };
        parent_thread
            .codex
            .session
            .record_conversation_items(turn_context.as_ref(), &[parent_spawn_call])
            .await;

        let child_thread_id = harness
            .control
            .spawn_agent_with_options(
                harness.config.clone(),
                text_input("child task"),
                Some(SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                    parent_thread_id,
                    depth: 1,
                    agent_nickname: None,
                    agent_role: None,
                })),
                SpawnAgentOptions {
                    fork_parent_spawn_call_id: Some(parent_spawn_call_id.clone()),
                },
            )
            .await
            .expect("forked spawn should flush parent rollout before loading history");

        let child_thread = harness
            .manager
            .get_thread(child_thread_id)
            .await
            .expect("child thread should be registered");
        let history = child_thread.codex.session.clone_history().await;

        let mut parent_call_index = None;
        let mut injected_output_index = None;
        for (idx, item) in history.raw_items().iter().enumerate() {
            match item {
                ResponseItem::FunctionCall { call_id, .. } if call_id == &parent_spawn_call_id => {
                    parent_call_index = Some(idx);
                }
                ResponseItem::FunctionCallOutput { call_id, .. }
                    if call_id == &parent_spawn_call_id =>
                {
                    injected_output_index = Some(idx);
                }
                _ => {}
            }
        }

        let parent_call_index =
            parent_call_index.expect("forked child should include the parent spawn_agent call");
        let injected_output_index = injected_output_index
            .expect("forked child should include synthetic output for the parent spawn_agent call");
        assert!(parent_call_index < injected_output_index);

        let _ = harness
            .control
            .shutdown_agent(child_thread_id)
            .await
            .expect("child shutdown should submit");
        let _ = parent_thread
            .submit(Op::Shutdown {})
            .await
            .expect("parent shutdown should submit");
    }

    #[tokio::test]
    async fn spawn_agent_respects_max_threads_limit() {
        let max_threads = 1usize;
        let (_home, config) = test_config_with_cli_overrides(vec![(
            "agents.max_threads".to_string(),
            TomlValue::Integer(max_threads as i64),
        )])
        .await;
        let manager = ThreadManager::with_models_provider_and_home_for_tests(
            CodexAuth::from_api_key("dummy"),
            config.model_provider.clone(),
            config.codex_home.clone(),
        );
        let control = manager.agent_control();

        let _ = manager
            .start_thread(config.clone())
            .await
            .expect("start thread");

        let first_agent_id = control
            .spawn_agent(config.clone(), text_input("hello"), None)
            .await
            .expect("spawn_agent should succeed");

        let err = control
            .spawn_agent(config, text_input("hello again"), None)
            .await
            .expect_err("spawn_agent should respect max threads");
        let CodexErr::AgentLimitReached {
            max_threads: seen_max_threads,
        } = err
        else {
            panic!("expected CodexErr::AgentLimitReached");
        };
        assert_eq!(seen_max_threads, max_threads);

        let _ = control
            .shutdown_agent(first_agent_id)
            .await
            .expect("shutdown agent");
    }

    #[tokio::test]
    async fn spawn_agent_releases_slot_after_shutdown() {
        let max_threads = 1usize;
        let (_home, config) = test_config_with_cli_overrides(vec![(
            "agents.max_threads".to_string(),
            TomlValue::Integer(max_threads as i64),
        )])
        .await;
        let manager = ThreadManager::with_models_provider_and_home_for_tests(
            CodexAuth::from_api_key("dummy"),
            config.model_provider.clone(),
            config.codex_home.clone(),
        );
        let control = manager.agent_control();

        let first_agent_id = control
            .spawn_agent(config.clone(), text_input("hello"), None)
            .await
            .expect("spawn_agent should succeed");
        let _ = control
            .shutdown_agent(first_agent_id)
            .await
            .expect("shutdown agent");

        let second_agent_id = control
            .spawn_agent(config.clone(), text_input("hello again"), None)
            .await
            .expect("spawn_agent should succeed after shutdown");
        let _ = control
            .shutdown_agent(second_agent_id)
            .await
            .expect("shutdown agent");
    }

    #[tokio::test]
    async fn spawn_agent_limit_shared_across_clones() {
        let max_threads = 1usize;
        let (_home, config) = test_config_with_cli_overrides(vec![(
            "agents.max_threads".to_string(),
            TomlValue::Integer(max_threads as i64),
        )])
        .await;
        let manager = ThreadManager::with_models_provider_and_home_for_tests(
            CodexAuth::from_api_key("dummy"),
            config.model_provider.clone(),
            config.codex_home.clone(),
        );
        let control = manager.agent_control();
        let cloned = control.clone();

        let first_agent_id = cloned
            .spawn_agent(config.clone(), text_input("hello"), None)
            .await
            .expect("spawn_agent should succeed");

        let err = control
            .spawn_agent(config, text_input("hello again"), None)
            .await
            .expect_err("spawn_agent should respect shared guard");
        let CodexErr::AgentLimitReached { max_threads } = err else {
            panic!("expected CodexErr::AgentLimitReached");
        };
        assert_eq!(max_threads, 1);

        let _ = control
            .shutdown_agent(first_agent_id)
            .await
            .expect("shutdown agent");
    }

    #[tokio::test]
    async fn resume_agent_respects_max_threads_limit() {
        let max_threads = 1usize;
        let (_home, config) = test_config_with_cli_overrides(vec![(
            "agents.max_threads".to_string(),
            TomlValue::Integer(max_threads as i64),
        )])
        .await;
        let manager = ThreadManager::with_models_provider_and_home_for_tests(
            CodexAuth::from_api_key("dummy"),
            config.model_provider.clone(),
            config.codex_home.clone(),
        );
        let control = manager.agent_control();

        let resumable_id = control
            .spawn_agent(config.clone(), text_input("hello"), None)
            .await
            .expect("spawn_agent should succeed");
        let _ = control
            .shutdown_agent(resumable_id)
            .await
            .expect("shutdown resumable thread");

        let active_id = control
            .spawn_agent(config.clone(), text_input("occupy"), None)
            .await
            .expect("spawn_agent should succeed for active slot");

        let err = control
            .resume_agent_from_rollout(config, resumable_id, SessionSource::Exec)
            .await
            .expect_err("resume should respect max threads");
        let CodexErr::AgentLimitReached {
            max_threads: seen_max_threads,
        } = err
        else {
            panic!("expected CodexErr::AgentLimitReached");
        };
        assert_eq!(seen_max_threads, max_threads);

        let _ = control
            .shutdown_agent(active_id)
            .await
            .expect("shutdown active thread");
    }

    #[tokio::test]
    async fn resume_agent_releases_slot_after_resume_failure() {
        let max_threads = 1usize;
        let (_home, config) = test_config_with_cli_overrides(vec![(
            "agents.max_threads".to_string(),
            TomlValue::Integer(max_threads as i64),
        )])
        .await;
        let manager = ThreadManager::with_models_provider_and_home_for_tests(
            CodexAuth::from_api_key("dummy"),
            config.model_provider.clone(),
            config.codex_home.clone(),
        );
        let control = manager.agent_control();

        let _ = control
            .resume_agent_from_rollout(config.clone(), ThreadId::new(), SessionSource::Exec)
            .await
            .expect_err("resume should fail for missing rollout path");

        let resumed_id = control
            .spawn_agent(config, text_input("hello"), None)
            .await
            .expect("spawn should succeed after failed resume");
        let _ = control
            .shutdown_agent(resumed_id)
            .await
            .expect("shutdown resumed thread");
    }

    #[tokio::test]
    async fn spawn_child_completion_notifies_parent_history() {
        let harness = AgentControlHarness::new().await;
        let (parent_thread_id, parent_thread) = harness.start_thread().await;

        let child_thread_id = harness
            .control
            .spawn_agent(
                harness.config.clone(),
                text_input("hello child"),
                Some(SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                    parent_thread_id,
                    depth: 1,
                    agent_nickname: None,
                    agent_role: Some("explorer".to_string()),
                })),
            )
            .await
            .expect("child spawn should succeed");

        let child_thread = harness
            .manager
            .get_thread(child_thread_id)
            .await
            .expect("child thread should exist");
        let mut status_rx = harness
            .control
            .subscribe_status(child_thread_id)
            .await
            .expect("status subscription should succeed");
        if matches!(status_rx.borrow().clone(), AgentStatus::PendingInit) {
            timeout(Duration::from_secs(5), async {
                loop {
                    status_rx
                        .changed()
                        .await
                        .expect("child status should advance past pending init");
                    if !matches!(status_rx.borrow().clone(), AgentStatus::PendingInit) {
                        break;
                    }
                }
            })
            .await
            .expect("child should initialize before shutdown");
        }
        let _ = child_thread
            .submit(Op::Shutdown {})
            .await
            .expect("child shutdown should submit");

        assert_eq!(wait_for_subagent_notification(&parent_thread).await, true);
    }

    #[tokio::test]
    async fn completion_watcher_notifies_parent_when_child_is_missing() {
        let harness = AgentControlHarness::new().await;
        let (parent_thread_id, parent_thread) = harness.start_thread().await;
        let child_thread_id = ThreadId::new();

        harness.control.maybe_start_completion_watcher(
            child_thread_id,
            Some(SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                parent_thread_id,
                depth: 1,
                agent_nickname: None,
                agent_role: Some("explorer".to_string()),
            })),
        );

        assert_eq!(wait_for_subagent_notification(&parent_thread).await, true);

        let history_items = parent_thread
            .codex
            .session
            .clone_history()
            .await
            .raw_items()
            .to_vec();
        assert_eq!(
            history_contains_text(
                &history_items,
                &format!("\"agent_id\":\"{child_thread_id}\"")
            ),
            true
        );
        assert_eq!(
            history_contains_text(&history_items, "\"status\":\"not_found\""),
            true
        );
    }

    #[tokio::test]
    async fn spawn_thread_subagent_gets_random_nickname_in_session_source() {
        let harness = AgentControlHarness::new().await;
        let (parent_thread_id, _parent_thread) = harness.start_thread().await;

        let child_thread_id = harness
            .control
            .spawn_agent(
                harness.config.clone(),
                text_input("hello child"),
                Some(SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                    parent_thread_id,
                    depth: 1,
                    agent_nickname: None,
                    agent_role: Some("explorer".to_string()),
                })),
            )
            .await
            .expect("child spawn should succeed");

        let child_thread = harness
            .manager
            .get_thread(child_thread_id)
            .await
            .expect("child thread should be registered");
        let snapshot = child_thread.config_snapshot().await;

        let SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
            parent_thread_id: seen_parent_thread_id,
            depth,
            agent_nickname,
            agent_role,
        }) = snapshot.session_source
        else {
            panic!("expected thread-spawn sub-agent source");
        };
        assert_eq!(seen_parent_thread_id, parent_thread_id);
        assert_eq!(depth, 1);
        assert!(agent_nickname.is_some());
        assert_eq!(agent_role, Some("explorer".to_string()));
    }

    #[tokio::test]
    async fn spawn_thread_subagent_uses_role_specific_nickname_candidates() {
        let mut harness = AgentControlHarness::new().await;
        harness.config.agent_roles.insert(
            "researcher".to_string(),
            AgentRoleConfig {
                description: Some("Research role".to_string()),
                config_file: None,
                nickname_candidates: Some(vec!["Atlas".to_string()]),
            },
        );
        let (parent_thread_id, _parent_thread) = harness.start_thread().await;

        let child_thread_id = harness
            .control
            .spawn_agent(
                harness.config.clone(),
                text_input("hello child"),
                Some(SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                    parent_thread_id,
                    depth: 1,
                    agent_nickname: None,
                    agent_role: Some("researcher".to_string()),
                })),
            )
            .await
            .expect("child spawn should succeed");

        let child_thread = harness
            .manager
            .get_thread(child_thread_id)
            .await
            .expect("child thread should be registered");
        let snapshot = child_thread.config_snapshot().await;

        let SessionSource::SubAgent(SubAgentSource::ThreadSpawn { agent_nickname, .. }) =
            snapshot.session_source
        else {
            panic!("expected thread-spawn sub-agent source");
        };
        assert_eq!(agent_nickname, Some("Atlas".to_string()));
    }

    #[tokio::test]
    async fn resume_thread_subagent_restores_stored_nickname_and_role() {
        let (home, mut config) = test_config().await;
        config
            .features
            .enable(Feature::Sqlite)
            .expect("test config should allow sqlite");
        let manager = ThreadManager::with_models_provider_and_home_for_tests(
            CodexAuth::from_api_key("dummy"),
            config.model_provider.clone(),
            config.codex_home.clone(),
        );
        let control = manager.agent_control();
        let harness = AgentControlHarness {
            _home: home,
            config,
            manager,
            control,
        };
        let (parent_thread_id, _parent_thread) = harness.start_thread().await;

        let child_thread_id = harness
            .control
            .spawn_agent(
                harness.config.clone(),
                text_input("hello child"),
                Some(SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                    parent_thread_id,
                    depth: 1,
                    agent_nickname: None,
                    agent_role: Some("explorer".to_string()),
                })),
            )
            .await
            .expect("child spawn should succeed");

        let child_thread = harness
            .manager
            .get_thread(child_thread_id)
            .await
            .expect("child thread should exist");
        let mut status_rx = harness
            .control
            .subscribe_status(child_thread_id)
            .await
            .expect("status subscription should succeed");
        if matches!(status_rx.borrow().clone(), AgentStatus::PendingInit) {
            timeout(Duration::from_secs(5), async {
                loop {
                    status_rx
                        .changed()
                        .await
                        .expect("child status should advance past pending init");
                    if !matches!(status_rx.borrow().clone(), AgentStatus::PendingInit) {
                        break;
                    }
                }
            })
            .await
            .expect("child should initialize before shutdown");
        }
        let original_snapshot = child_thread.config_snapshot().await;
        let original_nickname = original_snapshot
            .session_source
            .get_nickname()
            .expect("spawned sub-agent should have a nickname");
        let state_db = child_thread
            .state_db()
            .expect("sqlite state db should be available for nickname resume test");
        timeout(Duration::from_secs(5), async {
            loop {
                if let Ok(Some(metadata)) = state_db.get_thread(child_thread_id).await
                    && metadata.agent_nickname.is_some()
                    && metadata.agent_role.as_deref() == Some("explorer")
                {
                    break;
                }
                sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("child thread metadata should be persisted to sqlite before shutdown");

        let _ = harness
            .control
            .shutdown_agent(child_thread_id)
            .await
            .expect("child shutdown should submit");

        let resumed_thread_id = harness
            .control
            .resume_agent_from_rollout(
                harness.config.clone(),
                child_thread_id,
                SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                    parent_thread_id,
                    depth: 1,
                    agent_nickname: None,
                    agent_role: None,
                }),
            )
            .await
            .expect("resume should succeed");
        assert_eq!(resumed_thread_id, child_thread_id);

        let resumed_snapshot = harness
            .manager
            .get_thread(resumed_thread_id)
            .await
            .expect("resumed child thread should exist")
            .config_snapshot()
            .await;
        let SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
            parent_thread_id: resumed_parent_thread_id,
            depth: resumed_depth,
            agent_nickname: resumed_nickname,
            agent_role: resumed_role,
        }) = resumed_snapshot.session_source
        else {
            panic!("expected thread-spawn sub-agent source");
        };
        assert_eq!(resumed_parent_thread_id, parent_thread_id);
        assert_eq!(resumed_depth, 1);
        assert_eq!(resumed_nickname, Some(original_nickname));
        assert_eq!(resumed_role, Some("explorer".to_string()));

        let _ = harness
            .control
            .shutdown_agent(resumed_thread_id)
            .await
            .expect("resumed child shutdown should submit");
    }
}
