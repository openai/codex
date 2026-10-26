use super::residency::is_v2_resident_session_source;
use super::spawn_guard::PendingSpawn;
use super::spawn_telemetry::SpawnMeasurements;
use super::spawn_telemetry::record_spawn_success;
use super::*;
use crate::agent::child_config::build_agent_resume_config;
use crate::agent::history::load_agent_model_context;
use crate::agent::history::prepare_forked_history;
use crate::agent::role::apply_role_to_config;
use crate::agent::types::AgentMetadata;
use crate::agent::types::LiveAgent;
use crate::agent::types::SpawnAgentOptions;
use crate::agents_md_manager::SessionInstructions;
use crate::codex_thread::CodexThread;
use crate::codex_thread::ThreadConfigSnapshot;
use crate::config::PermissionProfileSnapshot;
use crate::thread_manager::StartThreadOptions;
use codex_features::Feature;
use codex_protocol::error::AgentErrorContext;
use codex_protocol::intersect_effective_permission_profiles;
use codex_protocol::protocol::EnvironmentConfigState;
use codex_thread_store::PersistContext;
use codex_utils_path_uri::PathUri;
use futures::StreamExt;
use futures::stream;
use std::time::Instant;

const AGENT_NAMES: &str = include_str!("../../../assets/agent/agent_names.txt");

/// Initial input delivered after a spawned agent acquires execution capacity.
///
/// V2 communication spawns keep the communication and its context paired so centralized
/// submission and lifecycle logging cannot receive one without the other. Other spawn sources
/// provide user input directly, making an uncontextualized inter-agent communication
/// unrepresentable.
#[allow(clippy::large_enum_variant)]
pub(super) enum SpawnInitialInput {
    UserInput(Vec<UserInput>),
    InterAgentCommunication(InterAgentCommunication, AgentCommunicationContext),
}

async fn effective_spawn_config(thread: &CodexThread) -> CodexResult<ThreadConfigSnapshot> {
    let mut config = thread.config_snapshot().await;
    let model_info = thread
        .thread_extension_data()
        .get::<codex_protocol::openai_models::ModelInfo>()
        .ok_or_else(|| CodexErr::Fatal("spawned thread is missing model metadata".to_string()))?;
    config.reasoning_effort = config
        .reasoning_effort
        .or_else(|| model_info.default_reasoning_level.clone())
        .map(|effort| model_info.resolve_reasoning_effort(effort));
    Ok(config)
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

impl LocalAgentControl {
    /// Restore persisted V2 agent identities without reopening their runtimes.
    pub(crate) async fn restore_v2_agent_metadata(
        &self,
        config: &Config,
        root_thread_id: ThreadId,
    ) {
        let registry = &self.runtime.registry;
        registry.register_root_thread(root_thread_id);

        let Ok(state) = self.runtime.upgrade() else {
            return;
        };
        let Some(agent_graph_store) = state.agent_graph_store() else {
            return;
        };
        let descendant_ids = match agent_graph_store
            .list_thread_spawn_descendants(
                root_thread_id,
                Some(codex_agent_graph_store::ThreadSpawnEdgeStatus::Open),
            )
            .await
        {
            Ok(descendant_ids) => descendant_ids,
            Err(err) => {
                warn!("failed to restore persisted V2 agent metadata for {root_thread_id}: {err}");
                return;
            }
        };

        // Overlap storage reads, but reserve paths and nicknames in graph order.
        let mut stored_threads = stream::iter(
            descendant_ids
                .into_iter()
                .filter(|thread_id| registry.agent_metadata_for_thread(*thread_id).is_none())
                .map(|thread_id| {
                    let state = &state;
                    async move {
                        let stored_thread = state
                            .read_stored_thread(ReadThreadParams {
                                thread_id,
                                include_archived: true,
                                include_history: false,
                            })
                            .await;
                        (thread_id, stored_thread)
                    }
                }),
        )
        .buffered(/*n*/ 8);

        while let Some((thread_id, stored_thread)) = stored_threads.next().await {
            if registry.agent_metadata_for_thread(thread_id).is_some() {
                continue;
            }
            let restore_result = stored_thread.and_then(|stored_thread| {
                let stored_agent_path = stored_thread
                    .agent_path
                    .as_deref()
                    .map(AgentPath::try_from)
                    .transpose()
                    .map_err(|err| {
                        CodexErr::InvalidRequest(format!("invalid stored agent path: {err}"))
                    })?;
                let mut reservation = registry.reserve_spawn_slot(/*max_threads*/ None)?;
                let mut metadata = self.prepare_agent_metadata(
                    &mut reservation,
                    config,
                    stored_agent_path.or_else(|| stored_thread.source.get_agent_path()),
                    stored_thread
                        .agent_role
                        .or_else(|| stored_thread.source.get_agent_role()),
                    stored_thread
                        .agent_nickname
                        .or_else(|| stored_thread.source.get_nickname()),
                )?;
                metadata.agent_id = Some(thread_id);
                reservation.commit(metadata);
                Ok(())
            });
            if let Err(err) = restore_result {
                warn!("failed to restore V2 agent metadata for {thread_id}: {err}");
            }
        }
    }

    /// Spawn a new agent thread and submit the initial prompt.
    #[cfg(test)]
    pub(crate) async fn spawn_agent(
        &self,
        config: Config,
        initial_input: Vec<UserInput>,
        session_source: Option<SessionSource>,
    ) -> CodexResult<ThreadId> {
        let (spawned_agent, _) = Box::pin(self.spawn_agent_internal(
            config,
            SpawnInitialInput::UserInput(initial_input),
            session_source,
            SpawnAgentOptions::default(),
        ))
        .await?;
        Ok(spawned_agent.thread_id)
    }

    fn validate_loaded_v2_child(
        &self,
        thread: &CodexThread,
        parent_thread_id: ThreadId,
    ) -> CodexResult<()> {
        if thread.is_running()
            && thread.multi_agent_version() == Some(MultiAgentVersion::V2)
            && thread.session_source.parent_thread_id() == Some(parent_thread_id)
            && Arc::ptr_eq(
                &self.runtime.registry,
                &thread.session.services.local_agent_runtime.registry,
            )
        {
            return Ok(());
        }
        Err(CodexErr::InvalidRequest(format!(
            "multi-agent v2 child {} is not owned by its loaded parent",
            thread.session.thread_id
        )))
    }

    /// A provided parent enables owner-validated reloads; `None` preserves sender-driven reloads.
    /// Keep the returned residency pin until the caller has submitted its input.
    pub(crate) async fn ensure_v2_agent_loaded(
        &self,
        mut config: Config,
        thread_id: ThreadId,
        parent: Option<Arc<CodexThread>>,
    ) -> CodexResult<Option<tokio::sync::OwnedRwLockReadGuard<()>>> {
        let membership = self.runtime.admit_start()?;
        let state = self.runtime.upgrade()?;
        let owner_thread_id = parent.as_ref().map(|parent| parent.session.thread_id);
        if let Some(parent) = &parent {
            let parent_thread_id = parent.session.thread_id;
            let registered_parent = state.get_thread(parent_thread_id).await.ok();
            if !registered_parent
                .as_ref()
                .is_some_and(|registered| Arc::ptr_eq(registered, parent))
                || !parent.is_running()
                || parent.multi_agent_version() != Some(MultiAgentVersion::V2)
                || !Arc::ptr_eq(
                    &self.runtime.registry,
                    &parent.session.services.local_agent_runtime.registry,
                )
            {
                return Err(CodexErr::InvalidRequest(format!(
                    "cannot resume multi-agent v2 child {thread_id}: parent ownership is unavailable; resume the parent first"
                )));
            }
        }
        if owner_thread_id.is_none()
            && let Ok(thread) = state.get_thread(thread_id).await
        {
            return self.runtime.pin_v2_residency(&state, &thread).await;
        }
        if self
            .runtime
            .registry
            .agent_metadata_for_thread(thread_id)
            .is_none()
        {
            return Err(CodexErr::ThreadNotFound(thread_id));
        }
        let evicted_environments = self.runtime.registry.evicted_environments(thread_id);
        let mut environment_selections = evicted_environments
            .as_ref()
            .map(|environments| environments.selections.clone());
        let evicted_environments = evicted_environments.map(|environments| environments.bindings);

        let stored_thread = state
            .read_stored_thread(ReadThreadParams {
                thread_id,
                include_archived: true,
                include_history: false,
            })
            .await?;
        let stored_model = stored_thread.model.clone();
        let stored_model_provider = stored_thread.model_provider.clone();
        let stored_reasoning_effort = stored_thread.reasoning_effort.clone();
        let stored_source = stored_thread.source.clone();
        let stored_parent_thread_id = stored_thread.parent_thread_id;
        let history = load_agent_model_context(&state, thread_id, stored_thread.history_mode)
            .await?
            .ok_or(CodexErr::ThreadNotFound(thread_id))?;
        let initial_history = InitialHistory::Resumed(ResumedHistory {
            history_revision: history.revision,
            conversation_id: thread_id,
            history: Arc::new(history.items),
            rollout_path: stored_thread.rollout_path,
        });
        if initial_history.get_multi_agent_version() != Some(MultiAgentVersion::V2) {
            return Err(CodexErr::ThreadNotFound(thread_id));
        }
        let (session_source, _) = initial_history
            .get_resumed_session_sources()
            .unwrap_or((stored_source, None));
        if let Some(parent_thread_id) = owner_thread_id {
            if session_source.parent_thread_id() != Some(parent_thread_id)
                || initial_history
                    .get_resumed_parent_thread_id()
                    .is_some_and(|recorded_parent| recorded_parent != parent_thread_id)
                || stored_parent_thread_id
                    .is_some_and(|recorded_parent| recorded_parent != parent_thread_id)
            {
                return Err(CodexErr::InvalidRequest(format!(
                    "cannot resume multi-agent v2 child {thread_id}: recorded parent ownership is inconsistent"
                )));
            }
            if let Ok(thread) = state.get_thread(thread_id).await {
                self.validate_loaded_v2_child(&thread, parent_thread_id)?;
                return self.runtime.pin_v2_residency(&state, &thread).await;
            }
        }
        let parent = if let Some(parent) = parent {
            let turn = parent
                .session
                .new_turn_with_default_settings(Uuid::now_v7().to_string(), Default::default())
                .await;
            config = build_agent_resume_config(&turn).map_err(|_| {
                CodexErr::InvalidRequest(format!(
                    "cannot resume multi-agent v2 child {thread_id} with the current parent settings"
                ))
            })?;
            let environments = parent.session.inherited_environment_snapshot().await;
            Some((parent, environments))
        } else {
            None
        };
        config.model_reasoning_effort = stored_reasoning_effort;
        if let Some(role_name) = session_source.get_agent_role() {
            let runtime_approval_policy = config.permissions.approval_policy.value();
            let runtime_approvals_reviewer = config.approvals_reviewer;
            let runtime_cwd = config.cwd.clone();
            let runtime_permission_profile = match config.permissions.active_permission_profile() {
                Some(active_permission_profile) => {
                    PermissionProfileSnapshot::active_with_profile_workspace_roots(
                        config.permissions.permission_profile().clone(),
                        active_permission_profile,
                        config.permissions.profile_workspace_roots().to_vec(),
                    )
                }
                None => PermissionProfileSnapshot::legacy(
                    config.permissions.permission_profile().clone(),
                ),
            };

            apply_role_to_config(&mut config, Some(&role_name))
                .await
                .map_err(CodexErr::InvalidRequest)?;
            config
                .permissions
                .approval_policy
                .set(runtime_approval_policy)
                .map_err(|err| {
                    CodexErr::InvalidRequest(format!("approval_policy is invalid: {err}"))
                })?;
            config.approvals_reviewer = runtime_approvals_reviewer;
            config.cwd = runtime_cwd;
            config
                .permissions
                .set_permission_profile_from_session_snapshot(runtime_permission_profile)
                .map_err(|err| {
                    CodexErr::InvalidRequest(format!("permission_profile is invalid: {err}"))
                })?;
        }
        config.service_tier = self.root_service_tier();
        if let Some(model) = stored_model {
            config.model = Some(model);
        }
        if config.model_provider_id != stored_model_provider {
            config.model_provider = config
                .model_providers
                .get(&stored_model_provider)
                .cloned()
                .ok_or_else(|| {
                    CodexErr::InvalidRequest(format!(
                        "Model provider `{stored_model_provider}` not found"
                    ))
                })?;
            config.model_provider_id = stored_model_provider;
        }
        let parent_thread_id = owner_thread_id
            .or_else(|| initial_history.get_resumed_parent_thread_id())
            .or(stored_parent_thread_id);
        let (inherited_environments, inherited_exec_policy, client_mcp_extensions) = if let Some(
            (parent, parent_environments),
        ) =
            parent.as_ref()
        {
            let parent_config = parent.session.get_config().await;
            if !crate::exec_policy::child_uses_parent_exec_policy(&parent_config, &config) {
                return Err(CodexErr::InvalidRequest(format!(
                    "cannot resume multi-agent v2 child {thread_id}: parent execution policy has changed; retry through the parent"
                )));
            }
            if let Some(selections) = environment_selections.as_mut() {
                let parent_selections = parent_environments.all_selections();
                let ready_parent = parent_environments.refresh_readiness();
                let ready_evicted = evicted_environments
                    .as_ref()
                    .map(TurnEnvironmentSnapshot::refresh_readiness);
                for selection in selections {
                    let environment_id = &selection.environment_id;
                    let invalid_environment = |reason: &str| {
                        CodexErr::InvalidRequest(format!(
                            "cannot resume multi-agent v2 child {thread_id}: cached environment {environment_id} {reason}"
                        ))
                    };
                    // A changed parent selection does not revoke the child's retained binding.
                    // If the parent still configures this workspace, keep enforcing its current
                    // roots and permissions rather than restoring superseded authority.
                    let owner_environments = if parent_selections.iter().any(|parent_selection| {
                        parent_selection.environment_id == selection.environment_id
                            && parent_selection.cwd == selection.cwd
                    }) {
                        &ready_parent
                    } else {
                        ready_evicted.as_ref().unwrap_or(&ready_parent)
                    };
                    let owner_environment = owner_environments
                        .turn_environments()
                        .find(|environment| environment.selection.has_same_workspace(selection))
                        .ok_or_else(|| {
                            invalid_environment("has no matching ready environment binding")
                        })?;
                    let owner_config = owner_environment.config();
                    let child_config = match &selection.config {
                        EnvironmentConfigState::FromThread => {
                            // Pin current owner authority instead of re-inferring child settings.
                            selection.config = EnvironmentConfigState::Ready(owner_config.clone());
                            continue;
                        }
                        EnvironmentConfigState::Ready(config) => config,
                        EnvironmentConfigState::Pending | EnvironmentConfigState::Failed(_) => {
                            return Err(invalid_environment("configuration is not ready"));
                        }
                    };
                    let mut bounded_config = child_config.clone();
                    bounded_config.permission_profile = owner_config.permission_profile.clone();
                    if bounded_config != *owner_config {
                        return Err(invalid_environment(
                            "configuration differs from the current parent",
                        ));
                    }
                    if child_config.permission_profile == owner_config.permission_profile {
                        continue;
                    }
                    if owner_environment.environment.is_remote() {
                        return Err(invalid_environment(
                            "permissions changed on a remote executor",
                        ));
                    }
                    let cwd = selection.cwd.to_abs_path().map_err(|_| {
                        invalid_environment("working directory is not a local absolute path")
                    })?;
                    let roots = owner_environment
                        .workspace_roots()
                        .iter()
                        .map(PathUri::to_abs_path)
                        .collect::<Result<Vec<_>, _>>()
                        .map_err(|_| {
                            invalid_environment("workspace roots are not local absolute paths")
                        })?;
                    let authority = owner_environment
                        .permission_profile()
                        .clone()
                        .materialize_project_roots_with_workspace_roots(&roots);
                    let requested = child_config
                        .permission_profile
                        .permission_profile()
                        .clone()
                        .materialize_project_roots_with_workspace_roots(&roots);
                    let permissions =
                        intersect_effective_permission_profiles(&authority, &requested, &cwd)
                            .map_err(|err| {
                                invalid_environment(&format!(
                                    "permissions cannot be intersected safely: {err}"
                                ))
                            })?;
                    bounded_config.permission_profile =
                        PermissionProfileSnapshot::legacy(permissions);
                    selection.config = EnvironmentConfigState::Ready(bounded_config);
                }
            }
            (
                Some(
                    evicted_environments
                        .as_ref()
                        .unwrap_or(parent_environments)
                        .clone(),
                ),
                Some(Arc::clone(&parent.session.services.exec_policy)),
                Some(parent.client_mcp_extensions()),
            )
        } else {
            (
                match &evicted_environments {
                    Some(environments) => Some(environments.clone()),
                    None => {
                        self.inherited_environments_for_source(&state, Some(&session_source))
                            .await
                    }
                },
                inherited_exec_policy_for_source(&state, Some(&session_source), &config).await,
                None,
            )
        };
        let inherited_parent = if let Some((parent, _)) = parent.as_ref() {
            Some(Arc::clone(parent))
        } else if let Some(parent_thread_id) = parent_thread_id {
            state.get_thread(parent_thread_id).await.ok()
        } else {
            None
        };
        let prompt_cache_key = inherited_parent.as_ref().map(|parent| {
            parent
                .session
                .services
                .model_client
                .prompt_cache_key_policy()
        });
        let inherited_instructions = if let Some(parent) = inherited_parent {
            Some(parent.session.inherited_instructions().await)
        } else {
            self.runtime
                .shared_thread_instructions_provider
                .get()
                .map(|provider| SessionInstructions {
                    thread_provider: Some(Arc::clone(provider)),
                    ..Default::default()
                })
        };
        let subagent_analytics =
            if let SessionSource::SubAgent(source @ SubAgentSource::ThreadSpawn { .. }) =
                &session_source
                && let Some(parent_thread_id) = parent_thread_id
                && let Ok(parent) = state.get_thread(parent_thread_id).await
            {
                Some((
                    parent.session.app_server_client_metadata().await,
                    source.clone(),
                ))
            } else {
                None
            };
        // Reserving a slot can evict an idle nested parent. Capture its instructions and
        // analytics metadata alongside its authority before the live parent can disappear.
        let residency_slot = self
            .reserve_v2_residency_slot(&state, &config, &membership, Some(thread_id))
            .await?;

        match state
            .resume_thread_with_history_with_source(ResumeThreadWithHistoryOptions {
                config,
                prompt_cache_key,
                initial_history,
                agent_control: self.clone(),
                session_source,
                parent_thread_id,
                environment_selections,
                inherited_environments,
                inherited_instructions,
                inherited_exec_policy,
                client_mcp_extensions,
            })
            .await
        {
            Ok(reloaded_thread) => {
                if let Some(parent_thread_id) = owner_thread_id {
                    self.validate_loaded_v2_child(&reloaded_thread.thread, parent_thread_id)?;
                }
                let runtime = self.runtime.clone();
                // Finish registration even if the sender is cancelled while acquiring its pin.
                tokio::spawn(async move {
                    let _membership = membership;
                    let residency_pin = runtime
                        .pin_v2_residency(&state, &reloaded_thread.thread)
                        .await?;
                    runtime.registry.clear_evicted_environments(thread_id);
                    residency_slot.commit(reloaded_thread.thread_id);
                    // Register before listeners can forward events from the resumed thread.
                    if let Some((client_metadata, source)) = subagent_analytics {
                        let thread_config = reloaded_thread.thread.config_snapshot().await;
                        emit_subagent_session_started(
                            &reloaded_thread
                                .thread
                                .session
                                .services
                                .analytics_events_client,
                            client_metadata,
                            reloaded_thread.thread.session.session_id(),
                            reloaded_thread.thread_id,
                            thread_config.parent_thread_id,
                            thread_config,
                            source,
                            /*resumed_created_at*/ Some(stored_thread.created_at),
                        );
                    }
                    state.notify_thread_created(reloaded_thread.thread_id);
                    Ok(residency_pin)
                })
                .await?
            }
            Err(err) => {
                if let Ok(thread) = state.get_thread(thread_id).await {
                    if let Some(parent_thread_id) = owner_thread_id {
                        self.validate_loaded_v2_child(&thread, parent_thread_id)?;
                    }
                    self.runtime.registry.clear_evicted_environments(thread_id);
                    drop(residency_slot);
                    return self.runtime.pin_v2_residency(&state, &thread).await;
                }
                Err(err)
            }
        }
    }

    pub(super) async fn spawn_agent_internal(
        &self,
        config: Config,
        initial_input: SpawnInitialInput,
        session_source: Option<SessionSource>,
        options: SpawnAgentOptions,
    ) -> CodexResult<(LiveAgent, ThreadConfigSnapshot)> {
        let membership = self.runtime.admit_start()?;
        let spawn_started_at = Instant::now();
        let state = self.runtime.upgrade()?;
        // Resolve the parent before inheriting settings or copying attachments.
        // Recovering it later from history only repairs the child's metadata.
        let parent_thread_id = session_source
            .as_ref()
            .and_then(SessionSource::parent_thread_id)
            .or(options.parent_thread_id);
        let multi_agent_version = state
            .effective_multi_agent_version_for_spawn(
                &InitialHistory::New,
                session_source.as_ref(),
                parent_thread_id,
                /*forked_from_thread_id*/ None,
                &config,
            )
            .await;
        let product_sku = config.apps_mcp_product_sku.clone();
        if let Some(session_source) = session_source.as_ref() {
            self.ensure_execution_capacity(multi_agent_version, session_source)?;
        }
        let agent_max_threads = config.effective_agent_max_threads(multi_agent_version);
        let spawn_uses_v2_residency = multi_agent_version == MultiAgentVersion::V2
            && session_source
                .as_ref()
                .is_some_and(is_v2_resident_session_source);
        let (residency_slot, residency_reservation) = if spawn_uses_v2_residency {
            let residency_reservation_started_at = Instant::now();
            let residency_slot = self
                .reserve_v2_residency_slot(
                    &state,
                    &config,
                    &membership,
                    /*protected_thread_id*/ None,
                )
                .await?;
            (
                Some(residency_slot),
                Some(residency_reservation_started_at.elapsed()),
            )
        } else {
            (None, None)
        };
        let reservation_max_threads = if spawn_uses_v2_residency {
            None
        } else {
            agent_max_threads
        };
        let mut reservation = self
            .runtime
            .registry
            .reserve_spawn_slot(reservation_max_threads)?;
        let inherited_environments = match &options.environments {
            Some(environments) => Some(environments.clone()),
            None => {
                self.inherited_environments_for_source(&state, session_source.as_ref())
                    .await
            }
        };
        // Omitted plugin choices follow the parent even when no history is copied.
        // Explicit choices, including an empty list, belong to the spawning turn.
        let mut disabled_plugin_ids = options.disabled_plugin_ids.clone();
        if disabled_plugin_ids.is_none()
            && let Some(parent_thread_id) = parent_thread_id
            && let Ok(parent_thread) = state.get_thread(parent_thread_id).await
        {
            disabled_plugin_ids = Some(
                parent_thread
                    .thread_settings_snapshot()
                    .await
                    .disabled_plugin_ids,
            );
        }
        let inherited_exec_policy =
            inherited_exec_policy_for_source(&state, session_source.as_ref(), &config).await;
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

        // Capture child defaults once. Copying conversation history must not change
        // the capabilities of the turn which requested the spawn.
        let mut start_options = StartThreadOptions {
            session_source: session_source.clone(),
            thread_source: session_source.as_ref().map(|_| ThreadSource::Subagent),
            environments: options
                .environments
                .as_ref()
                .map(TurnEnvironmentSnapshot::inheritable_requests),
            inherited_environments,
            disabled_plugin_ids,
            ..StartThreadOptions::new(config.clone())
        };
        let fork_context = match (session_source, options.fork_mode.as_ref()) {
            (Some(session_source), Some(_)) => {
                let fork_context_started_at = Instant::now();
                let (initial_history, history_mode) = prepare_forked_history(
                    &state,
                    &session_source,
                    &options,
                    &config,
                    multi_agent_version,
                )
                .await
                .map_err(|err| err.with_agent_context(AgentErrorContext::ForkHistory))?;
                start_options.initial_history = initial_history;
                start_options.history_mode = history_mode;
                Some(fork_context_started_at.elapsed())
            }
            (Some(_), None) => {
                if let Some(parent_thread_id) = parent_thread_id
                    && let Ok(parent_thread) = state.get_thread(parent_thread_id).await
                {
                    start_options.history_mode = matches!(
                        parent_thread.config_snapshot().await.history_mode,
                        ThreadHistoryMode::Paginated
                    )
                    .then_some(ThreadHistoryMode::Paginated);
                    if multi_agent_version == MultiAgentVersion::V2
                        && config.features.enabled(Feature::MultiAgentV2DynamicTools)
                    {
                        start_options.dynamic_tools = parent_thread.session.dynamic_tools().await;
                    }
                }
                None
            }
            (None, _) => None,
        };
        let child_create_started_at = Instant::now();
        let new_thread = state
            .spawn_child_thread(
                start_options,
                self.clone(),
                parent_thread_id,
                inherited_exec_policy,
            )
            .await
            .map_err(|err| err.with_agent_context(AgentErrorContext::ChildStartup))?;
        let child_create = child_create_started_at.elapsed();
        agent_metadata.agent_id = Some(new_thread.thread_id);
        let mut pending_spawn =
            PendingSpawn::new(Arc::clone(&state), new_thread.thread_id, membership);

        if let Some(SessionSource::SubAgent(
            subagent_source @ SubAgentSource::ThreadSpawn {
                parent_thread_id, ..
            },
        )) = notification_source.as_ref()
        {
            let client_metadata = match state.get_thread(*parent_thread_id).await {
                Ok(parent_thread) => parent_thread.session.app_server_client_metadata().await,
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
            let thread_config = new_thread.thread.config_snapshot().await;
            let parent_thread_id = thread_config.parent_thread_id;
            emit_subagent_session_started(
                &new_thread.thread.session.services.analytics_events_client,
                client_metadata,
                new_thread.thread.session.session_id(),
                new_thread.thread_id,
                parent_thread_id,
                thread_config,
                subagent_source.clone(),
                /*resumed_created_at*/ None,
            );
        }

        let control = self.clone();
        let child = Arc::clone(&new_thread.thread);
        let child_thread_id = new_thread.thread_id;
        let source = notification_source.clone();
        pending_spawn.set_edge_write(tokio::spawn(async move {
            control
                .persist_thread_spawn_edge_for_source(
                    child.as_ref(),
                    child_thread_id,
                    source.as_ref(),
                )
                .await;
        }));
        let durability_wait_started_at = Instant::now();
        if options.fork_mode.is_some() {
            tokio::join!(
                new_thread
                    .thread
                    .session
                    .ensure_rollout_materialized(PersistContext::Standard),
                pending_spawn.wait_for_edge(),
            );
        } else {
            pending_spawn.wait_for_edge().await;
        }
        let durability_wait = durability_wait_started_at.elapsed();

        // Capture startup settings before the child can publish metadata for a later turn.
        let spawn_config = if multi_agent_version == MultiAgentVersion::V2 {
            Some(effective_spawn_config(&new_thread.thread).await?)
        } else {
            None
        };

        let start_options = TurnStartOptions {
            parent_turn_id: options.parent_turn_id,
            turn_trigger: options.turn_trigger,
            root_turn_id: options.root_turn_id,
            cyber_access_program: options.cyber_access_program,
            ..Default::default()
        };
        let input_admission_started_at = Instant::now();
        match initial_input {
            SpawnInitialInput::UserInput(input) => {
                self.send_input(new_thread.thread_id, input, start_options)
                    .await
                    .map_err(|err| err.with_agent_context(AgentErrorContext::InputAdmission))?;
            }
            SpawnInitialInput::InterAgentCommunication(communication, context) => {
                self.send_inter_agent_communication_after_capacity_check(
                    new_thread.thread_id,
                    &state,
                    communication,
                    context,
                    start_options,
                )
                .await
                .map_err(|err| err.with_agent_context(AgentErrorContext::InputAdmission))?;
            }
        }
        let input_admission = input_admission_started_at.elapsed();
        // Keep provisional ownership through the final awaits. Commit and return to the
        // backend caller in one poll; later tool-result publication is outside this boundary.
        let status = self.get_status(new_thread.thread_id).await;
        let config = match spawn_config {
            Some(config) => config,
            None => new_thread.thread.config_snapshot().await,
        };
        reservation.commit(agent_metadata.clone());
        if let Some(residency_slot) = residency_slot {
            residency_slot.commit(new_thread.thread_id);
        }
        let _membership = pending_spawn.disarm();

        // Notify a new thread has been created. This notification will be processed by clients
        // to subscribe or drain this newly created thread.
        // TODO(jif) add helper for drain
        state.notify_thread_created(new_thread.thread_id);
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
                agent_metadata.agent_path.clone(),
            );
        }

        let agent = LiveAgent {
            thread_id: new_thread.thread_id,
            metadata: agent_metadata,
            status,
        };
        let session_telemetry = new_thread
            .thread
            .session_telemetry()
            .with_product_sku(product_sku.as_deref());
        record_spawn_success(
            &session_telemetry,
            options.fork_mode.as_ref(),
            multi_agent_version,
            SpawnMeasurements {
                history_mode: config.history_mode,
                residency_reservation,
                fork_context,
                child_create,
                durability_wait,
                input_admission,
                total: spawn_started_at.elapsed(),
            },
        );
        Ok((agent, config))
    }

    /// Resume an existing agent thread from a recorded rollout file.
    pub(crate) async fn resume_agent_from_rollout(
        &self,
        config: Config,
        thread_id: ThreadId,
        session_source: SessionSource,
    ) -> CodexResult<ThreadId> {
        let _membership = self.runtime.admit_start()?;
        let root_depth = thread_spawn_depth(&session_source).unwrap_or(0);
        let (resumed_thread_id, resumed_multi_agent_version) = Box::pin(
            self.resume_single_agent_from_rollout(config.clone(), thread_id, session_source),
        )
        .await?;
        let state = self.runtime.upgrade()?;
        if config.multi_agent_version_from_features() == MultiAgentVersion::V2
            || resumed_multi_agent_version == MultiAgentVersion::V2
        {
            return Ok(resumed_thread_id);
        }
        let Some(agent_graph_store) = state.agent_graph_store() else {
            return Ok(resumed_thread_id);
        };

        let mut resume_queue = VecDeque::from([(thread_id, root_depth)]);
        while let Some((parent_thread_id, parent_depth)) = resume_queue.pop_front() {
            let child_ids = match agent_graph_store
                .list_thread_spawn_children(
                    parent_thread_id,
                    Some(codex_agent_graph_store::ThreadSpawnEdgeStatus::Open),
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
                        Ok((_, _)) => true,
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
        config: Config,
        thread_id: ThreadId,
        session_source: SessionSource,
    ) -> CodexResult<(ThreadId, MultiAgentVersion)> {
        let state = self.runtime.upgrade()?;
        let stored_thread = state
            .read_stored_thread(ReadThreadParams {
                thread_id,
                include_archived: true,
                include_history: false,
            })
            .await?;
        let resumed_agent_path = stored_thread
            .agent_path
            .as_deref()
            .map(AgentPath::try_from)
            .transpose()
            .map_err(|err| CodexErr::InvalidRequest(format!("invalid stored agent path: {err}")))?;
        let resumed_agent_nickname = stored_thread.agent_nickname.clone();
        let resumed_agent_role = stored_thread.agent_role.clone();
        let history = load_agent_model_context(&state, thread_id, stored_thread.history_mode)
            .await?
            .ok_or(CodexErr::ThreadNotFound(thread_id))?;
        let initial_history = InitialHistory::Resumed(ResumedHistory {
            history_revision: history.revision,
            conversation_id: thread_id,
            history: Arc::new(history.items),
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
        let agent_max_threads = config.effective_agent_max_threads(multi_agent_version);
        let mut reservation = self
            .runtime
            .registry
            .reserve_spawn_slot(agent_max_threads)?;
        let (session_source, agent_metadata) = match session_source {
            SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                parent_thread_id,
                depth,
                agent_path,
                agent_role: _,
                agent_nickname: _,
            }) => self.prepare_thread_spawn(
                &mut reservation,
                &config,
                parent_thread_id,
                depth,
                agent_path.or(resumed_agent_path),
                resumed_agent_role,
                resumed_agent_nickname,
            )?,
            other => (other, AgentMetadata::default()),
        };
        let notification_source = session_source.clone();
        let inherited_environments = self
            .inherited_environments_for_source(&state, Some(&session_source))
            .await;
        let inherited_exec_policy =
            inherited_exec_policy_for_source(&state, Some(&session_source), &config).await;

        let resumed_thread = state
            .resume_thread_with_history_with_source(ResumeThreadWithHistoryOptions {
                config: config.clone(),
                prompt_cache_key: None,
                initial_history,
                agent_control: self.clone(),
                session_source,
                parent_thread_id,
                environment_selections: None,
                inherited_environments,
                inherited_instructions: None,
                inherited_exec_policy,
                client_mcp_extensions: None,
            })
            .await?;
        let mut agent_metadata = agent_metadata;
        agent_metadata.agent_id = Some(resumed_thread.thread_id);
        reservation.commit(agent_metadata.clone());
        // Resumed threads are re-registered in-memory and need the same listener
        // attachment path as freshly spawned threads.
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
                agent_metadata.agent_path.clone(),
            );
        }
        self.persist_thread_spawn_edge_for_source(
            resumed_thread.thread.as_ref(),
            resumed_thread.thread_id,
            Some(&notification_source),
        )
        .await;

        Ok((resumed_thread.thread_id, multi_agent_version))
    }
}
