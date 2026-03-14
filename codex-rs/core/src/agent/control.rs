use super::watchdog::RemovedWatchdog;
use super::watchdog::WatchdogManager;
use super::watchdog::WatchdogRegistration;
use crate::agent::AgentStatus;
use crate::agent::guards::Guards;
use crate::agent::role::DEFAULT_ROLE_NAME;
use crate::agent::role::resolve_role_config;
use crate::agent::status::is_final;
use crate::config::Config;
use crate::error::CodexErr;
use crate::error::Result as CodexResult;
use crate::find_thread_path_by_id_str;
use crate::rollout::RolloutRecorder;
use crate::session_prefix::format_subagent_context_line;
use crate::session_prefix::format_subagent_notification_message;
use crate::shell_snapshot::ShellSnapshot;
use crate::state_db;
use crate::thread_manager::ThreadManagerState;
use codex_protocol::ThreadId;
use codex_protocol::models::ContentItem;
use codex_protocol::models::FunctionCallOutputBody;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::AGENT_INBOX_KIND;
use codex_protocol::protocol::AgentInboxPayload;
use codex_protocol::protocol::ForkReferenceItem;
use codex_protocol::protocol::InitialHistory;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use codex_protocol::protocol::TokenUsage;
use codex_protocol::user_input::UserInput;
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::Weak;
use tokio::sync::Mutex;
use tokio::sync::watch;
use uuid::Uuid;

const AGENT_NAMES: &str = include_str!("agent_names.txt");
const FORKED_SPAWN_AGENT_OUTPUT_MESSAGE: &str = "You are the newly spawned agent. The prior conversation history was forked from your parent agent. Treat the next user message as your new task, and use the forked history only as background context.";

#[derive(Clone, Debug, Default)]
pub(crate) struct SpawnAgentOptions {
    pub(crate) fork_parent_spawn_call_id: Option<String>,
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

/// Control-plane handle for multi-agent operations.
/// `AgentControl` is held by each session (via `SessionServices`). It provides capability to
/// spawn new agents and the inter-agent communication layer.
/// An `AgentControl` instance is shared per "user session" which means the same `AgentControl`
/// is used for every sub-agent spawned by Codex. By doing so, we make sure the guards are
/// scoped to a user session.
#[derive(Clone)]
pub(crate) struct AgentControl {
    /// Weak handle back to the global thread registry/state.
    /// This is `Weak` to avoid reference cycles and shadow persistence of the form
    /// `ThreadManagerState -> CodexThread -> Session -> SessionServices -> ThreadManagerState`.
    manager: Weak<ThreadManagerState>,
    guards: Arc<Guards>,
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
        let guards = Arc::new(Guards::default());
        let watchdogs = WatchdogManager::new(manager.clone(), Arc::clone(&guards));
        Self::from_parts(manager, guards, watchdogs)
    }
}

impl AgentControl {
    /// Construct a new `AgentControl` that can spawn/message agents via the given manager state.
    pub(crate) fn new(manager: Weak<ThreadManagerState>) -> Self {
        let guards = Arc::new(Guards::default());
        let watchdogs = WatchdogManager::new(manager.clone(), Arc::clone(&guards));
        watchdogs.start();
        Self::from_parts(manager, guards, watchdogs)
    }

    pub(crate) fn from_parts(
        manager: Weak<ThreadManagerState>,
        guards: Arc<Guards>,
        watchdogs: Arc<WatchdogManager>,
    ) -> Self {
        Self {
            manager,
            guards,
            watchdogs,
            watchdog_compactions_in_progress: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    /// Spawn a new agent thread and submit the initial prompt.
    pub(crate) async fn spawn_agent(
        &self,
        config: crate::config::Config,
        items: Vec<UserInput>,
        session_source: Option<SessionSource>,
    ) -> CodexResult<ThreadId> {
        self.spawn_agent_with_options(config, items, session_source, SpawnAgentOptions::default())
            .await
    }

    pub(crate) async fn spawn_agent_with_options(
        &self,
        config: Config,
        items: Vec<UserInput>,
        session_source: Option<SessionSource>,
        options: SpawnAgentOptions,
    ) -> CodexResult<ThreadId> {
        let state = self.upgrade()?;
        let mut reservation = self
            .reserve_spawn_slot_with_reconcile(&state, config.agent_max_threads)
            .await?;
        let inherited_shell_snapshot = self
            .inherited_shell_snapshot_for_source(&state, session_source.as_ref())
            .await;
        let session_source = match session_source {
            Some(SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                parent_thread_id,
                depth,
                agent_role,
                ..
            })) => {
                let candidate_names = agent_nickname_candidates(&config, agent_role.as_deref());
                let candidate_name_refs: Vec<&str> =
                    candidate_names.iter().map(String::as_str).collect();
                let agent_nickname = reservation.reserve_agent_nickname(&candidate_name_refs)?;
                Some(SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                    parent_thread_id,
                    depth,
                    agent_nickname: Some(agent_nickname),
                    agent_role,
                }))
            }
            other => other,
        };
        let notification_source = session_source.clone();

        // The same `AgentControl` is sent to spawn the thread.
        let new_thread = match session_source {
            Some(session_source) => {
                if let Some(call_id) = options.fork_parent_spawn_call_id.as_ref() {
                    let SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                        parent_thread_id,
                        ..
                    }) = session_source.clone()
                    else {
                        return Err(CodexErr::Fatal(
                            "spawn_agent fork requires a thread-spawn session source".to_string(),
                        ));
                    };
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
                    let mut forked_rollout_items = RolloutRecorder::get_fork_history(&rollout_path)
                        .await?
                        .get_rollout_items();
                    if forked_rollout_items
                        .iter()
                        .any(|item| matches!(item, RolloutItem::ForkReference(_)))
                    {
                        forked_rollout_items =
                            crate::rollout::truncation::materialize_rollout_items_for_replay(
                                config.codex_home.as_path(),
                                &forked_rollout_items,
                            )
                            .await;
                    }
                    forked_rollout_items.push(RolloutItem::ForkReference(ForkReferenceItem {
                        rollout_path: rollout_path.clone(),
                        nth_user_message: usize::MAX,
                    }));
                    let mut output = FunctionCallOutputPayload::from_text(
                        FORKED_SPAWN_AGENT_OUTPUT_MESSAGE.to_string(),
                    );
                    output.success = Some(true);
                    forked_rollout_items.push(RolloutItem::ResponseItem(
                        ResponseItem::FunctionCallOutput {
                            call_id: call_id.clone(),
                            output,
                        },
                    ));
                    let initial_history = InitialHistory::Forked(forked_rollout_items);
                    state
                        .fork_thread_with_source(
                            config,
                            initial_history,
                            self.clone(),
                            session_source,
                            false,
                            inherited_shell_snapshot,
                        )
                        .await?
                } else {
                    state
                        .spawn_new_thread_with_source(
                            config,
                            self.clone(),
                            session_source,
                            false,
                            None,
                            inherited_shell_snapshot,
                        )
                        .await?
                }
            }
            None => state.spawn_new_thread(config, self.clone()).await?,
        };
        reservation.commit(new_thread.thread_id);

        // Notify a new thread has been created. This notification will be processed by clients
        // to subscribe or drain this newly created thread.
        // TODO(jif) add helper for drain
        state.notify_thread_created(new_thread.thread_id);

        self.send_input(new_thread.thread_id, items).await?;
        self.maybe_start_completion_watcher(new_thread.thread_id, notification_source);

        Ok(new_thread.thread_id)
    }

    pub(crate) async fn spawn_agent_handle(
        &self,
        config: Config,
        session_source: Option<SessionSource>,
    ) -> CodexResult<ThreadId> {
        let state = self.upgrade()?;
        let reservation = self
            .reserve_spawn_slot_with_reconcile(&state, config.agent_max_threads)
            .await?;
        let inherited_shell_snapshot = self
            .inherited_shell_snapshot_for_source(&state, session_source.as_ref())
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
                    )
                    .await?
            }
            None => state.spawn_new_thread(config, self.clone()).await?,
        };
        reservation.commit(new_thread.thread_id);
        state.notify_thread_created(new_thread.thread_id);
        Ok(new_thread.thread_id)
    }

    pub(crate) async fn fork_agent(
        &self,
        config: Config,
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
        // Fork helpers must start as distinct child threads. Reusing the resume loader here
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
            )
            .await?;
        reservation.commit(new_thread.thread_id);
        state.notify_thread_created(new_thread.thread_id);
        self.send_input(new_thread.thread_id, items).await?;
        Ok(new_thread.thread_id)
    }

    /// Resume an existing agent thread from a recorded rollout file.
    pub(crate) async fn resume_agent_from_rollout(
        &self,
        config: Config,
        thread_id: ThreadId,
        session_source: SessionSource,
    ) -> CodexResult<ThreadId> {
        let state = self.upgrade()?;
        let mut reservation = self
            .reserve_spawn_slot_with_reconcile(&state, config.agent_max_threads)
            .await?;
        let session_source = match session_source {
            SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                parent_thread_id,
                depth,
                ..
            }) => {
                // Collab resume callers rebuild a placeholder ThreadSpawn source. Rehydrate the
                // stored nickname/role from sqlite when available; otherwise leave both unset.
                let (resumed_agent_nickname, resumed_agent_role) =
                    if let Some(state_db_ctx) = state_db::get_state_db(&config).await {
                        match state_db_ctx.get_thread(thread_id).await {
                            Ok(Some(metadata)) => (metadata.agent_nickname, metadata.agent_role),
                            Ok(None) | Err(_) => (None, None),
                        }
                    } else {
                        (None, None)
                    };
                let reserved_agent_nickname = resumed_agent_nickname
                    .as_deref()
                    .map(|agent_nickname| {
                        let candidate_names =
                            agent_nickname_candidates(&config, resumed_agent_role.as_deref());
                        let candidate_name_refs: Vec<&str> =
                            candidate_names.iter().map(String::as_str).collect();
                        reservation.reserve_agent_nickname_with_preference(
                            &candidate_name_refs,
                            Some(agent_nickname),
                        )
                    })
                    .transpose()?;
                SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                    parent_thread_id,
                    depth,
                    agent_nickname: reserved_agent_nickname,
                    agent_role: resumed_agent_role,
                })
            }
            other => other,
        };
        let notification_source = session_source.clone();
        let inherited_shell_snapshot = self
            .inherited_shell_snapshot_for_source(&state, Some(&session_source))
            .await;
        let rollout_path =
            find_thread_path_by_id_str(config.codex_home.as_path(), &thread_id.to_string())
                .await?
                .ok_or_else(|| CodexErr::ThreadNotFound(thread_id))?;

        let resumed_thread = state
            .resume_thread_from_rollout_with_source(
                config,
                rollout_path,
                self.clone(),
                session_source,
                inherited_shell_snapshot,
            )
            .await?;
        reservation.commit(resumed_thread.thread_id);
        // Resumed threads are re-registered in-memory and need the same listener
        // attachment path as freshly spawned threads.
        state.notify_thread_created(resumed_thread.thread_id);
        self.maybe_start_completion_watcher(resumed_thread.thread_id, Some(notification_source));

        Ok(resumed_thread.thread_id)
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
            }],
        )
        .await
    }

    /// Send rich user input items to an existing agent thread.
    pub(crate) async fn send_input(
        &self,
        agent_id: ThreadId,
        items: Vec<UserInput>,
    ) -> CodexResult<String> {
        let state = self.upgrade()?;
        let result = state
            .send_op(
                agent_id,
                Op::UserInput {
                    items,
                    final_output_json_schema: None,
                },
            )
            .await;
        if matches!(result, Err(CodexErr::InternalAgentDied)) {
            let _ = state.remove_thread(&agent_id).await;
            self.guards.release_spawned_thread(agent_id);
        }
        result
    }

    /// Deliver inbox input to an existing agent thread.
    ///
    /// Watchdog helpers rely on this as the mandatory fallback wake-up path when
    /// a check-in reaches a terminal state without explicitly calling
    /// `send_input`.
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
            self.guards.release_spawned_thread(agent_id);
        }
        result
    }

    /// Deliver watchdog wake-up input to an owner thread.
    ///
    /// This intentionally bypasses `agent_use_function_call_inbox` for
    /// non-subagent owners. Watchdog check-ins must wake the owner exactly
    /// once; the injected inbox path reliably starts or resumes the owner's
    /// next turn while preserving helper identity in history.
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
            self.guards.release_spawned_thread(agent_id);
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
            (None, Some(items)) => self.send_input(agent_id, items).await,
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

    /// Submit a shutdown request to an existing agent thread.
    pub(crate) async fn shutdown_agent(&self, agent_id: ThreadId) -> CodexResult<String> {
        let state = self.upgrade()?;
        let mut descendants = self.collect_descendants(&state, agent_id).await;
        descendants.reverse();
        for descendant_id in descendants {
            if let Some(removed_watchdog) = self.watchdogs.unregister(descendant_id).await
                && let Some(helper_id) = removed_watchdog.active_helper_id
            {
                let _ = state.send_op(helper_id, Op::Shutdown {}).await;
                let _ = state.remove_thread(&helper_id).await;
                self.guards.release_spawned_thread(helper_id);
            }
            let _ = state.send_op(descendant_id, Op::Shutdown {}).await;
            let _ = state.remove_thread(&descendant_id).await;
            self.guards.release_spawned_thread(descendant_id);
        }
        if let Some(removed_watchdog) = self.watchdogs.unregister(agent_id).await
            && let Some(helper_id) = removed_watchdog.active_helper_id
        {
            let _ = state.send_op(helper_id, Op::Shutdown {}).await;
            let _ = state.remove_thread(&helper_id).await;
            self.guards.release_spawned_thread(helper_id);
        }
        let result = state.send_op(agent_id, Op::Shutdown {}).await;
        let _ = state.remove_thread(&agent_id).await;
        self.guards.release_spawned_thread(agent_id);
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

    pub(crate) async fn get_agent_nickname_and_role(
        &self,
        agent_id: ThreadId,
    ) -> Option<(Option<String>, Option<String>)> {
        let Ok(state) = self.upgrade() else {
            return None;
        };
        let Ok(thread) = state.get_thread(agent_id).await else {
            return None;
        };
        let session_source = thread.config_snapshot().await.session_source;
        Some((
            session_source.get_nickname(),
            session_source.get_agent_role(),
        ))
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
        let Ok(state) = self.upgrade() else {
            return String::new();
        };

        let mut agents = Vec::new();
        for thread_id in state.list_thread_ids().await {
            let Ok(thread) = state.get_thread(thread_id).await else {
                continue;
            };
            let snapshot = thread.config_snapshot().await;
            let SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                parent_thread_id: agent_parent_thread_id,
                agent_nickname,
                ..
            }) = snapshot.session_source
            else {
                continue;
            };
            if agent_parent_thread_id != parent_thread_id {
                continue;
            }
            agents.push(format_subagent_context_line(
                &thread_id.to_string(),
                agent_nickname.as_deref(),
            ));
        }
        agents.sort();
        agents.join("\n")
    }

    /// Starts a detached watcher for sub-agents spawned from another thread.
    ///
    /// This is only enabled for `SubAgentSource::ThreadSpawn`, where a parent thread exists and
    /// can receive completion notifications.
    fn maybe_start_completion_watcher(
        &self,
        child_thread_id: ThreadId,
        session_source: Option<SessionSource>,
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
            let Ok(parent_thread) = state.get_thread(parent_thread_id).await else {
                return;
            };
            parent_thread
                .inject_user_message_without_turn(format_subagent_notification_message(
                    &child_thread_id.to_string(),
                    &status,
                ))
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

        {
            let mut compacting = self.watchdog_compactions_in_progress.lock().await;
            if compacting.contains(&parent_thread_id) {
                return Ok(WatchdogParentCompactionResult::AlreadyInProgress { parent_thread_id });
            }
            compacting.insert(parent_thread_id);
        }

        if parent_thread.has_active_turn().await {
            let mut compacting = self.watchdog_compactions_in_progress.lock().await;
            compacting.remove(&parent_thread_id);
            return Ok(WatchdogParentCompactionResult::ParentBusy { parent_thread_id });
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

    pub(crate) async fn list_agents(
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
            all_thread_ids.extend(self.guards.tracked_thread_ids());
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
    ) -> CodexResult<crate::agent::guards::SpawnReservation> {
        self.reconcile_stale_guard_slots(state).await;
        match self.guards.reserve_spawn_slot(max_threads) {
            Ok(reservation) => Ok(reservation),
            Err(CodexErr::AgentLimitReached { .. }) => {
                self.reconcile_stale_guard_slots(state).await;
                self.guards.reserve_spawn_slot(max_threads)
            }
            Err(err) => Err(err),
        }
    }

    async fn reconcile_stale_guard_slots(&self, state: &ThreadManagerState) {
        let live_thread_ids: HashSet<ThreadId> =
            state.list_thread_ids().await.into_iter().collect();
        for tracked_thread_id in self.guards.tracked_thread_ids() {
            if !live_thread_ids.contains(&tracked_thread_id) {
                self.guards.release_spawned_thread(tracked_thread_id);
            }
        }
    }

    async fn collect_descendants(
        &self,
        state: &ThreadManagerState,
        owner_thread_id: ThreadId,
    ) -> Vec<ThreadId> {
        let thread_ids = state.list_thread_ids().await;
        let mut children_by_parent: HashMap<ThreadId, Vec<ThreadId>> = HashMap::new();
        for thread_id in thread_ids {
            let Ok(thread) = state.get_thread(thread_id).await else {
                continue;
            };
            let snapshot = thread.config_snapshot().await;
            if let SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                parent_thread_id, ..
            }) = snapshot.session_source
            {
                children_by_parent
                    .entry(parent_thread_id)
                    .or_default()
                    .push(thread_id);
            }
        }

        for children in children_by_parent.values_mut() {
            children.sort_by_key(ToString::to_string);
        }

        let mut descendants = Vec::new();
        let mut queue = VecDeque::new();
        if let Some(children) = children_by_parent.get(&owner_thread_id) {
            for child in children {
                queue.push_back(*child);
            }
        }

        while let Some(thread_id) = queue.pop_front() {
            descendants.push(thread_id);
            if let Some(children) = children_by_parent.get(&thread_id) {
                for child in children {
                    queue.push_back(*child);
                }
            }
        }

        descendants
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
#[cfg(test)]
mod fork_reference_tests {
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
    use crate::rollout::recorder::RolloutRecorder;
    use assert_matches::assert_matches;
    use codex_protocol::config_types::ModeKind;
    use codex_protocol::models::ContentItem;
    use codex_protocol::models::ResponseInputItem;
    use codex_protocol::models::ResponseItem;
    use codex_protocol::protocol::AgentInboxPayload;
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

    fn thread_spawn_source(parent_thread_id: ThreadId) -> SessionSource {
        SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
            parent_thread_id,
            depth: 1,
            agent_nickname: None,
            agent_role: None,
        })
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

    #[tokio::test]
    async fn send_watchdog_wakeup_to_root_thread_injects_response_items_by_default() {
        let harness = AgentControlHarness::new().await;
        let (receiver_thread_id, _thread) = harness.start_thread().await;
        let sender_thread_id = ThreadId::new();

        let submission_id = harness
            .control
            .send_watchdog_wakeup(
                receiver_thread_id,
                sender_thread_id,
                "watchdog update".to_string(),
            )
            .await
            .expect("send_watchdog_wakeup should succeed");
        assert!(!submission_id.is_empty());

        let captured = harness
            .manager
            .captured_ops()
            .into_iter()
            .find(|(thread_id, op)| {
                *thread_id == receiver_thread_id && matches!(op, Op::InjectResponseItems { .. })
            })
            .expect("expected injected watchdog wake-up op");

        let Op::InjectResponseItems { items } = captured.1 else {
            unreachable!("matched above");
        };
        assert_eq!(items.len(), 3);
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
    async fn fork_agent_uses_distinct_thread_id_and_marks_parent() {
        let harness = AgentControlHarness::new().await;
        let (parent_thread_id, parent_thread) = harness.start_thread().await;
        let turn_context = parent_thread.codex.session.new_default_turn().await;
        parent_thread
            .codex
            .session
            .record_conversation_items(
                turn_context.as_ref(),
                &[ResponseItem::Message {
                    id: None,
                    role: "user".to_string(),
                    content: vec![ContentItem::InputText {
                        text: "parent seed context".to_string(),
                    }],
                    end_turn: None,
                    phase: None,
                }],
            )
            .await;
        parent_thread
            .codex
            .session
            .ensure_rollout_materialized()
            .await;
        parent_thread.codex.session.flush_rollout().await;

        let child_thread_id = harness
            .control
            .fork_agent(
                harness.config.clone(),
                text_input("watchdog helper"),
                parent_thread_id,
                usize::MAX,
                SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                    parent_thread_id,
                    depth: 1,
                    agent_nickname: None,
                    agent_role: None,
                }),
            )
            .await
            .expect("fork_agent should spawn a distinct child thread");

        assert_ne!(child_thread_id, parent_thread_id);

        let child_thread = harness
            .manager
            .get_thread(child_thread_id)
            .await
            .expect("child thread should be registered");
        let history = child_thread.codex.session.clone_history().await;
        assert!(history_contains_text(
            history.raw_items(),
            "parent seed context"
        ));

        let child_rollout_path = child_thread
            .rollout_path()
            .expect("forked child should have a rollout path");
        let child_history = RolloutRecorder::get_rollout_history(&child_rollout_path)
            .await
            .expect("child rollout should load");
        assert_eq!(child_history.forked_from_id(), Some(parent_thread_id));

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
                model: None,
                config_file: None,
                spawn_mode: None,
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

    #[tokio::test]
    async fn run_watchdogs_once_cleans_up_handle_and_helper_after_owner_shutdown() {
        let (home, config) = test_config_with_cli_overrides(vec![(
            "agents.max_threads".to_string(),
            TomlValue::Integer(2),
        )])
        .await;
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
        let (owner_thread_id, owner_thread) = harness.start_thread().await;
        let watchdog_handle_id = harness
            .control
            .spawn_agent_handle(
                harness.config.clone(),
                Some(thread_spawn_source(owner_thread_id)),
            )
            .await
            .expect("watchdog handle should spawn");
        let helper_thread_id = harness
            .control
            .spawn_agent_handle(
                harness.config.clone(),
                Some(thread_spawn_source(owner_thread_id)),
            )
            .await
            .expect("watchdog helper should spawn");
        let removed = harness
            .control
            .register_watchdog(WatchdogRegistration {
                owner_thread_id,
                target_thread_id: watchdog_handle_id,
                child_depth: 1,
                interval_s: 1,
                prompt: "check in".to_string(),
                config: harness.config.clone(),
            })
            .await
            .expect("watchdog registration should succeed");
        assert_eq!(removed, Vec::<RemovedWatchdog>::new());
        harness
            .control
            .set_watchdog_active_helper_for_tests(watchdog_handle_id, helper_thread_id)
            .await;
        assert_eq!(
            harness
                .control
                .watchdog_owner_for_active_helper(helper_thread_id)
                .await,
            Some(owner_thread_id)
        );
        let tracked_before = harness.control.guards.tracked_thread_ids();
        assert_eq!(tracked_before.contains(&watchdog_handle_id), true);
        assert_eq!(tracked_before.contains(&helper_thread_id), true);

        let mut owner_status_rx = harness
            .control
            .subscribe_status(owner_thread_id)
            .await
            .expect("owner status subscription should succeed");
        let _ = owner_thread
            .submit(Op::Shutdown {})
            .await
            .expect("owner shutdown should submit");
        timeout(Duration::from_secs(2), async {
            loop {
                if matches!(owner_status_rx.borrow().clone(), AgentStatus::Shutdown) {
                    break;
                }
                owner_status_rx
                    .changed()
                    .await
                    .expect("owner status should reach shutdown");
            }
        })
        .await
        .expect("owner should reach shutdown");

        harness.control.run_watchdogs_once_for_tests().await;

        assert_eq!(
            harness.control.get_status(watchdog_handle_id).await,
            AgentStatus::NotFound
        );
        assert_eq!(
            harness.control.get_status(helper_thread_id).await,
            AgentStatus::NotFound
        );
        assert_eq!(
            harness
                .control
                .watchdog_owner_for_active_helper(helper_thread_id)
                .await,
            None
        );
        let tracked_after = harness.control.guards.tracked_thread_ids();
        assert_eq!(tracked_after.contains(&watchdog_handle_id), false);
        assert_eq!(tracked_after.contains(&helper_thread_id), false);

        let replacement_thread_id = harness
            .control
            .spawn_agent_handle(harness.config.clone(), None)
            .await
            .expect("cleanup should release watchdog helper slots");
        let ops = harness.manager.captured_ops();
        assert_eq!(
            ops.iter()
                .any(|(thread_id, op)| *thread_id == watchdog_handle_id
                    && matches!(op, Op::Shutdown)),
            true
        );
        assert_eq!(
            ops.iter()
                .any(|(thread_id, op)| *thread_id == helper_thread_id && matches!(op, Op::Shutdown)),
            true
        );

        let _ = harness
            .control
            .shutdown_agent(replacement_thread_id)
            .await
            .expect("replacement thread shutdown should submit");
    }

    #[tokio::test]
    async fn run_watchdogs_once_wakes_owner_when_helper_exits_without_send_input() {
        let harness = AgentControlHarness::new().await;
        let (owner_thread_id, owner_thread) = harness.start_thread().await;
        let watchdog_handle_id = harness
            .control
            .spawn_agent_handle(
                harness.config.clone(),
                Some(thread_spawn_source(owner_thread_id)),
            )
            .await
            .expect("watchdog handle should spawn");
        let helper_thread_id = harness
            .control
            .spawn_agent_handle(
                harness.config.clone(),
                Some(thread_spawn_source(owner_thread_id)),
            )
            .await
            .expect("watchdog helper should spawn");
        let removed = harness
            .control
            .register_watchdog(WatchdogRegistration {
                owner_thread_id,
                target_thread_id: watchdog_handle_id,
                child_depth: 1,
                interval_s: 1,
                prompt: "check in".to_string(),
                config: harness.config.clone(),
            })
            .await
            .expect("watchdog registration should succeed");
        assert_eq!(removed, Vec::<RemovedWatchdog>::new());
        harness
            .control
            .set_watchdog_active_helper_for_tests(watchdog_handle_id, helper_thread_id)
            .await;
        harness
            .control
            .force_watchdog_due_for_tests(watchdog_handle_id)
            .await;

        let mut helper_status_rx = harness
            .control
            .subscribe_status(helper_thread_id)
            .await
            .expect("helper status subscription should succeed");
        let _ = harness
            .control
            .shutdown_agent(helper_thread_id)
            .await
            .expect("helper shutdown should submit");
        timeout(Duration::from_secs(2), async {
            loop {
                if matches!(helper_status_rx.borrow().clone(), AgentStatus::Shutdown) {
                    break;
                }
                helper_status_rx
                    .changed()
                    .await
                    .expect("helper status should reach shutdown");
            }
        })
        .await
        .expect("helper should reach shutdown");
        harness.control.run_watchdogs_once_for_tests().await;

        let payload = timeout(Duration::from_secs(2), async {
            loop {
                let history = owner_thread.codex.session.clone_history().await;
                if let Some(payload) = history.raw_items().iter().find_map(|item| match item {
                    ResponseItem::FunctionCallOutput { output, .. } => output
                        .text_content()
                        .and_then(|text| serde_json::from_str::<AgentInboxPayload>(text).ok()),
                    _ => None,
                }) {
                    break payload;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("owner should receive fallback agent inbox payload");
        assert_eq!(payload.sender_thread_id, helper_thread_id);
        assert!(
            payload.message.starts_with("Watchdog check-in "),
            "expected watchdog fallback prefix, got {:?}",
            payload.message
        );
        assert!(
            payload.message.ends_with("before calling send_input."),
            "expected watchdog fallback suffix, got {:?}",
            payload.message
        );
    }

    #[tokio::test]
    async fn compact_parent_for_watchdog_helper_blocks_duplicates_until_finish() {
        let harness = AgentControlHarness::new().await;
        let (owner_thread_id, _owner_thread) = harness.start_thread().await;
        let watchdog_handle_id = harness
            .control
            .spawn_agent_handle(
                harness.config.clone(),
                Some(thread_spawn_source(owner_thread_id)),
            )
            .await
            .expect("watchdog handle should spawn");
        let helper_thread_id = harness
            .control
            .spawn_agent_handle(
                harness.config.clone(),
                Some(thread_spawn_source(owner_thread_id)),
            )
            .await
            .expect("watchdog helper should spawn");
        let removed = harness
            .control
            .register_watchdog(WatchdogRegistration {
                owner_thread_id,
                target_thread_id: watchdog_handle_id,
                child_depth: 1,
                interval_s: 1,
                prompt: "compact if needed".to_string(),
                config: harness.config.clone(),
            })
            .await
            .expect("watchdog registration should succeed");
        assert_eq!(removed, Vec::<RemovedWatchdog>::new());
        harness
            .control
            .set_watchdog_active_helper_for_tests(watchdog_handle_id, helper_thread_id)
            .await;
        let result = harness
            .control
            .compact_parent_for_watchdog_helper(helper_thread_id)
            .await
            .expect("first compact request should submit");
        let submission_id = match result {
            WatchdogParentCompactionResult::Submitted {
                parent_thread_id,
                submission_id,
            } => {
                assert_eq!(parent_thread_id, owner_thread_id);
                submission_id
            }
            other => panic!("expected submitted compaction result, got {other:?}"),
        };
        assert_eq!(submission_id.is_empty(), false);
        assert_eq!(
            harness
                .manager
                .captured_ops()
                .iter()
                .filter(|(thread_id, op)| *thread_id == owner_thread_id && matches!(op, Op::Compact))
                .count(),
            1
        );

        let result = harness
            .control
            .compact_parent_for_watchdog_helper(helper_thread_id)
            .await
            .expect("duplicate compact should be blocked");
        assert_eq!(
            result,
            WatchdogParentCompactionResult::AlreadyInProgress {
                parent_thread_id: owner_thread_id,
            }
        );

        harness
            .control
            .finish_watchdog_parent_compaction(owner_thread_id)
            .await;

        let result = harness
            .control
            .compact_parent_for_watchdog_helper(helper_thread_id)
            .await
            .expect("completed compact should unblock later requests");
        let resubmitted_id = match result {
            WatchdogParentCompactionResult::Submitted {
                parent_thread_id,
                submission_id,
            } => {
                assert_eq!(parent_thread_id, owner_thread_id);
                submission_id
            }
            other => panic!("expected submitted compaction result after finish, got {other:?}"),
        };
        assert_eq!(resubmitted_id.is_empty(), false);
        assert_eq!(
            harness
                .manager
                .captured_ops()
                .iter()
                .filter(|(thread_id, op)| *thread_id == owner_thread_id && matches!(op, Op::Compact))
                .count(),
            2
        );

        let _ = harness
            .control
            .shutdown_agent(watchdog_handle_id)
            .await
            .expect("watchdog handle shutdown should submit");
        let _ = harness
            .control
            .shutdown_agent(owner_thread_id)
            .await
            .expect("owner shutdown should submit");
    }

    #[tokio::test]
    async fn compact_parent_for_watchdog_helper_releases_marker_when_parent_is_busy() {
        let harness = AgentControlHarness::new().await;
        let (owner_thread_id, owner_thread) = harness.start_thread().await;
        let watchdog_handle_id = harness
            .control
            .spawn_agent_handle(
                harness.config.clone(),
                Some(thread_spawn_source(owner_thread_id)),
            )
            .await
            .expect("watchdog handle should spawn");
        let helper_thread_id = harness
            .control
            .spawn_agent_handle(
                harness.config.clone(),
                Some(thread_spawn_source(owner_thread_id)),
            )
            .await
            .expect("watchdog helper should spawn");
        let removed = harness
            .control
            .register_watchdog(WatchdogRegistration {
                owner_thread_id,
                target_thread_id: watchdog_handle_id,
                child_depth: 1,
                interval_s: 1,
                prompt: "compact if needed".to_string(),
                config: harness.config.clone(),
            })
            .await
            .expect("watchdog registration should succeed");
        assert_eq!(removed, Vec::<RemovedWatchdog>::new());
        harness
            .control
            .set_watchdog_active_helper_for_tests(watchdog_handle_id, helper_thread_id)
            .await;

        owner_thread
            .codex
            .session
            .active_turn
            .lock()
            .await
            .replace(crate::state::ActiveTurn::default());

        let result = harness
            .control
            .compact_parent_for_watchdog_helper(helper_thread_id)
            .await
            .expect("busy parent should not error");
        assert_eq!(
            result,
            WatchdogParentCompactionResult::ParentBusy {
                parent_thread_id: owner_thread_id
            }
        );

        owner_thread.codex.session.active_turn.lock().await.take();

        let result = harness
            .control
            .compact_parent_for_watchdog_helper(helper_thread_id)
            .await
            .expect("busy result should not leave stale marker");
        match result {
            WatchdogParentCompactionResult::Submitted {
                parent_thread_id, ..
            } => {
                assert_eq!(parent_thread_id, owner_thread_id);
            }
            other => panic!(
                "expected submitted compaction result after busy parent clears, got {other:?}"
            ),
        }
    }
}
