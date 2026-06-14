use super::*;
use crate::SkillLoadOutcome;
use crate::agents_md::LoadedAgentsMd;
use crate::config::GhostSnapshotConfig;
use crate::environment_selection::ResolvedTurnEnvironments;
use codex_core_skills::HostLoadedSkills;
use codex_file_system::FileSystemSandboxContext;
use codex_model_provider::SharedModelProvider;
use codex_model_provider::create_model_provider;
use codex_protocol::SessionId;
use codex_protocol::ThreadId;
use codex_protocol::models::AdditionalPermissionProfile;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ToolMode;
use codex_protocol::protocol::MultiAgentVersion;
use codex_protocol::protocol::ThreadSource;
use codex_protocol::protocol::TurnEnvironmentSelection;
use codex_sandboxing::compatibility_sandbox_policy_for_permission_profile;
use codex_sandboxing::policy_transforms::effective_file_system_sandbox_policy;
use codex_sandboxing::policy_transforms::effective_network_sandbox_policy;
use codex_utils_path_uri::PathUri;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use tracing::instrument;

#[derive(Clone, Debug)]
pub(crate) struct TurnSkillsContext {
    pub(crate) outcome: Arc<SkillLoadOutcome>,
    pub(crate) implicit_invocation_seen_skills: Arc<Mutex<HashSet<String>>>,
}

impl TurnSkillsContext {
    pub(crate) fn new(outcome: Arc<SkillLoadOutcome>) -> Self {
        Self {
            outcome,
            implicit_invocation_seen_skills: Arc::new(Mutex::new(HashSet::new())),
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct TurnEnvironment {
    pub(crate) environment_id: String,
    pub(crate) environment: Arc<Environment>,
    // Keep both representations together while cwd consumers migrate to URI semantics. Keeping
    // them synchronized means neither representation can be exposed through a mutable reference;
    // updates must rebuild the validated pair through `TurnEnvironment::new`. Once
    // `TurnEnvironment::cwd` itself becomes a `PathUri`, convert only at native filesystem and
    // process-launch boundaries and remove this paired migration state.
    cwd: AbsolutePathBuf,
    cwd_uri: PathUri,
    pub(crate) shell: Option<shell::Shell>,
}

impl TurnEnvironment {
    pub(crate) fn new(
        environment_id: String,
        environment: Arc<Environment>,
        cwd: AbsolutePathBuf,
        shell: Option<shell::Shell>,
    ) -> Self {
        let cwd_uri = PathUri::from_abs_path(&cwd);
        Self {
            environment_id,
            environment,
            cwd,
            cwd_uri,
            shell,
        }
    }

    pub(crate) fn cwd(&self) -> &AbsolutePathBuf {
        &self.cwd
    }

    pub(crate) fn cwd_uri(&self) -> &PathUri {
        &self.cwd_uri
    }

    pub(crate) fn selection(&self) -> TurnEnvironmentSelection {
        TurnEnvironmentSelection {
            environment_id: self.environment_id.clone(),
            cwd: self.cwd_uri.clone(),
        }
    }
}

/// The context needed for a single turn of the thread.
#[derive(Debug)]
pub struct TurnContext {
    pub(crate) sub_id: String,
    pub(crate) trace_id: Option<String>,
    pub(crate) realtime_active: bool,
    pub config: Arc<Config>,
    pub(crate) auth_manager: Option<Arc<AuthManager>>,
    pub(crate) model_info: ModelInfo,
    pub(crate) comp_hash: Option<String>,
    pub(crate) tool_mode: ToolMode,
    pub(crate) session_telemetry: SessionTelemetry,
    pub(crate) provider: SharedModelProvider,
    pub(crate) reasoning_effort: Option<ReasoningEffortConfig>,
    pub(crate) reasoning_summary: ReasoningSummaryConfig,
    pub(crate) session_source: SessionSource,
    pub(crate) parent_thread_id: Option<ThreadId>,
    pub(crate) thread_source: Option<ThreadSource>,
    pub(crate) environments: ResolvedTurnEnvironments,
    /// The session's absolute working directory. All relative paths provided
    /// by the model as well as sandbox policies are resolved against this path
    /// instead of `std::env::current_dir()`.
    #[deprecated(note = "use the selected turn environment cwd instead")]
    pub(crate) cwd: AbsolutePathBuf,
    pub(crate) current_date: Option<String>,
    pub(crate) timezone: Option<String>,
    pub(crate) app_server_client_name: Option<String>,
    pub(crate) developer_instructions: Option<String>,
    pub(crate) compact_prompt: Option<String>,
    pub(crate) user_instructions: Option<String>,
    pub(crate) collaboration_mode: CollaborationMode,
    pub(crate) multi_agent_version: MultiAgentVersion,
    pub(crate) personality: Option<Personality>,
    pub(crate) approval_policy: Constrained<AskForApproval>,
    pub(crate) permission_profile: PermissionProfile,
    pub(crate) network: Option<NetworkProxy>,
    pub(crate) windows_sandbox_level: WindowsSandboxLevel,
    pub(crate) shell_environment_policy: ShellEnvironmentPolicy,
    pub(crate) available_models: Vec<ModelPreset>,
    pub(crate) unified_exec_shell_mode: UnifiedExecShellMode,
    pub features: ManagedFeatures,
    pub(crate) ghost_snapshot: GhostSnapshotConfig,
    pub(crate) final_output_json_schema: Option<Value>,
    pub(crate) codex_self_exe: Option<PathBuf>,
    pub(crate) codex_linux_sandbox_exe: Option<PathBuf>,
    pub(crate) truncation_policy: TruncationPolicy,
    pub(crate) dynamic_tools: Vec<DynamicToolSpec>,
    pub(crate) turn_metadata_state: Arc<TurnMetadataState>,
    pub(crate) extension_data: Arc<codex_extension_api::ExtensionData>,
    pub(crate) turn_skills: TurnSkillsContext,
    pub(crate) turn_timing_state: Arc<TurnTimingState>,
    pub(crate) server_model_warning_emitted: AtomicBool,
    pub(crate) model_verification_emitted: AtomicBool,
}

enum TurnMultiAgentRuntime {
    ResolveAndStore,
    Preview,
}

impl TurnContext {
    pub(crate) fn permission_profile(&self) -> PermissionProfile {
        self.permission_profile.clone()
    }

    pub(crate) fn file_system_sandbox_policy(&self) -> FileSystemSandboxPolicy {
        self.permission_profile.file_system_sandbox_policy()
    }

    pub(crate) fn network_sandbox_policy(&self) -> NetworkSandboxPolicy {
        self.permission_profile.network_sandbox_policy()
    }

    pub(crate) fn sandbox_policy(&self) -> SandboxPolicy {
        compatibility_sandbox_policy_for_permission_profile(
            &self.permission_profile,
            #[allow(deprecated)]
            &self.cwd,
        )
    }

    pub(crate) fn effective_reasoning_effort(&self) -> Option<ReasoningEffortConfig> {
        if self.model_info.supports_reasoning_summaries {
            self.reasoning_effort
                .clone()
                .or_else(|| self.model_info.default_reasoning_level.clone())
        } else {
            None
        }
    }

    pub(crate) fn effective_reasoning_effort_for_tracing(&self) -> String {
        self.effective_reasoning_effort()
            .map(|effort| effort.to_string())
            .unwrap_or_else(|| "default".to_string())
    }

    pub(crate) fn model_context_window(&self) -> Option<i64> {
        let effective_context_window_percent = self.model_info.effective_context_window_percent;
        self.model_info
            .resolved_context_window()
            .map(|context_window| {
                context_window.saturating_mul(effective_context_window_percent) / 100
            })
    }

    pub(crate) fn apps_enabled(&self) -> bool {
        let uses_codex_backend = self
            .auth_manager
            .as_deref()
            .is_some_and(AuthManager::current_auth_uses_codex_backend);
        self.features.apps_enabled_for_auth(uses_codex_backend)
    }

    pub(crate) fn tool_environment_mode(&self) -> ToolEnvironmentMode {
        ToolEnvironmentMode::from_count(self.environments.turn_environments.len())
    }

    pub(crate) async fn with_model(
        &self,
        model: String,
        models_manager: &SharedModelsManager,
    ) -> Self {
        let mut config = (*self.config).clone();
        config.model = Some(model.clone());
        let model_info = models_manager
            .get_model_info(model.as_str(), &config.to_models_manager_config())
            .await;
        let tool_mode = model_info.tool_mode.unwrap_or_else(|| {
            if config.features.enabled(Feature::CodeModeOnly) {
                ToolMode::CodeModeOnly
            } else if config.features.enabled(Feature::CodeMode) {
                ToolMode::CodeMode
            } else {
                ToolMode::Direct
            }
        });
        let truncation_policy = model_info.truncation_policy.into();
        let supported_reasoning_levels = model_info
            .supported_reasoning_levels
            .iter()
            .map(|preset| preset.effort.clone())
            .collect::<Vec<_>>();
        let reasoning_effort = if let Some(current_reasoning_effort) = self.reasoning_effort.clone()
        {
            if supported_reasoning_levels.contains(&current_reasoning_effort) {
                Some(current_reasoning_effort)
            } else {
                supported_reasoning_levels
                    .get(supported_reasoning_levels.len().saturating_sub(1) / 2)
                    .cloned()
                    .or_else(|| model_info.default_reasoning_level.clone())
            }
        } else {
            supported_reasoning_levels
                .get(supported_reasoning_levels.len().saturating_sub(1) / 2)
                .cloned()
                .or_else(|| model_info.default_reasoning_level.clone())
        };
        config.model_reasoning_effort = reasoning_effort.clone();

        let collaboration_mode = self.collaboration_mode.with_updates(
            Some(model.clone()),
            Some(reasoning_effort.clone()),
            /*developer_instructions*/ None,
        );
        let features = self.features.clone();
        let available_models = models_manager
            .list_models(RefreshStrategy::OnlineIfUncached)
            .await;

        Self {
            sub_id: self.sub_id.clone(),
            trace_id: self.trace_id.clone(),
            realtime_active: self.realtime_active,
            config: Arc::new(config),
            auth_manager: self.auth_manager.clone(),
            model_info: model_info.clone(),
            comp_hash: model_info.comp_hash.clone(),
            tool_mode,
            session_telemetry: self
                .session_telemetry
                .clone()
                .with_model(model.as_str(), model_info.slug.as_str()),
            provider: self.provider.clone(),
            reasoning_effort,
            reasoning_summary: self.reasoning_summary,
            session_source: self.session_source.clone(),
            parent_thread_id: self.parent_thread_id,
            thread_source: self.thread_source.clone(),
            environments: self.environments.clone(),
            #[allow(deprecated)]
            cwd: self.cwd.clone(),
            current_date: self.current_date.clone(),
            timezone: self.timezone.clone(),
            app_server_client_name: self.app_server_client_name.clone(),
            developer_instructions: self.developer_instructions.clone(),
            compact_prompt: self.compact_prompt.clone(),
            user_instructions: self.user_instructions.clone(),
            collaboration_mode,
            multi_agent_version: self.multi_agent_version,
            personality: self.personality,
            approval_policy: self.approval_policy.clone(),
            permission_profile: self.permission_profile.clone(),
            network: self.network.clone(),
            windows_sandbox_level: self.windows_sandbox_level,
            shell_environment_policy: self.shell_environment_policy.clone(),
            available_models,
            unified_exec_shell_mode: self.unified_exec_shell_mode.clone(),
            features,
            ghost_snapshot: self.ghost_snapshot.clone(),
            final_output_json_schema: self.final_output_json_schema.clone(),
            codex_self_exe: self.codex_self_exe.clone(),
            codex_linux_sandbox_exe: self.codex_linux_sandbox_exe.clone(),
            truncation_policy,
            dynamic_tools: self.dynamic_tools.clone(),
            turn_metadata_state: self.turn_metadata_state.clone(),
            extension_data: Arc::clone(&self.extension_data),
            turn_skills: self.turn_skills.clone(),
            turn_timing_state: Arc::clone(&self.turn_timing_state),
            server_model_warning_emitted: AtomicBool::new(
                self.server_model_warning_emitted.load(Ordering::Relaxed),
            ),
            model_verification_emitted: AtomicBool::new(
                self.model_verification_emitted.load(Ordering::Relaxed),
            ),
        }
    }

    #[deprecated(note = "resolve paths from the selected turn environment cwd instead")]
    pub(crate) fn resolve_path(&self, path: Option<String>) -> AbsolutePathBuf {
        #[allow(deprecated)]
        path.as_ref()
            .map_or_else(|| self.cwd.clone(), |path| self.cwd.join(path))
    }

    pub(crate) fn file_system_sandbox_context(
        &self,
        additional_permissions: Option<AdditionalPermissionProfile>,
        cwd: &PathUri,
    ) -> FileSystemSandboxContext<PathUri> {
        let (base_file_system_sandbox_policy, base_network_sandbox_policy) =
            self.permission_profile.to_runtime_permissions();
        let file_system_sandbox_policy = effective_file_system_sandbox_policy(
            &base_file_system_sandbox_policy,
            additional_permissions.as_ref(),
        );
        let network_sandbox_policy = effective_network_sandbox_policy(
            base_network_sandbox_policy,
            additional_permissions.as_ref(),
        );
        let permissions = PermissionProfile::from_runtime_permissions_with_enforcement(
            self.permission_profile.enforcement(),
            &file_system_sandbox_policy,
            network_sandbox_policy,
        );
        FileSystemSandboxContext::<AbsolutePathBuf> {
            permissions,
            cwd: Some(cwd.clone()),
            windows_sandbox_level: self.windows_sandbox_level,
            windows_sandbox_private_desktop: self
                .config
                .permissions
                .windows_sandbox_private_desktop,
            use_legacy_landlock: self.features.use_legacy_landlock(),
        }
        .into_path_uri()
    }

    fn non_legacy_file_system_sandbox_policy(&self) -> Option<FileSystemSandboxPolicy> {
        // Omit the derived split filesystem policy when it is equivalent to
        // the legacy sandbox policy. This keeps turn-context payloads stable
        // while both fields exist; once callers consume only the split policy,
        // this comparison and the legacy projection should go away.
        let legacy_file_system_sandbox_policy =
            FileSystemSandboxPolicy::from_legacy_sandbox_policy_for_cwd(
                &self.sandbox_policy(),
                #[allow(deprecated)]
                &self.cwd,
            );
        let file_system_sandbox_policy = self.file_system_sandbox_policy();
        (file_system_sandbox_policy != legacy_file_system_sandbox_policy)
            .then_some(file_system_sandbox_policy)
    }

    pub(crate) fn compact_prompt(&self) -> &str {
        self.compact_prompt
            .as_deref()
            .unwrap_or(compact::SUMMARIZATION_PROMPT)
    }

    pub(crate) fn to_turn_context_item(&self) -> TurnContextItem {
        let workspace_roots = self.config.effective_workspace_roots();
        TurnContextItem {
            turn_id: Some(self.sub_id.clone()),
            #[allow(deprecated)]
            cwd: self.cwd.to_path_buf(),
            workspace_roots: (!workspace_roots.is_empty()).then_some(workspace_roots),
            current_date: self.current_date.clone(),
            timezone: self.timezone.clone(),
            approval_policy: self.approval_policy.value(),
            sandbox_policy: self.sandbox_policy(),
            permission_profile: Some(self.permission_profile()),
            network: self.turn_context_network_item(),
            file_system_sandbox_policy: self.non_legacy_file_system_sandbox_policy(),
            model: self.model_info.slug.clone(),
            comp_hash: self.comp_hash.clone(),
            personality: self.personality,
            collaboration_mode: Some(self.collaboration_mode.clone()),
            multi_agent_version: Some(self.multi_agent_version),
            realtime_active: Some(self.realtime_active),
            effort: self.reasoning_effort.clone(),
            summary: ReasoningSummaryConfig::Auto,
        }
    }

    fn turn_context_network_item(&self) -> Option<TurnContextNetworkItem> {
        let network = self
            .config
            .config_layer_stack
            .requirements()
            .network
            .as_ref()?;
        Some(TurnContextNetworkItem {
            allowed_domains: network
                .domains
                .as_ref()
                .and_then(codex_config::NetworkDomainPermissionsToml::allowed_domains)
                .unwrap_or_default(),
            denied_domains: network
                .domains
                .as_ref()
                .and_then(codex_config::NetworkDomainPermissionsToml::denied_domains)
                .unwrap_or_default(),
        })
    }
}

fn local_time_context() -> (String, String) {
    match iana_time_zone::get_timezone() {
        Ok(timezone) => (Local::now().format("%Y-%m-%d").to_string(), timezone),
        Err(_) => (
            Utc::now().format("%Y-%m-%d").to_string(),
            "Etc/UTC".to_string(),
        ),
    }
}

impl Session {
    /// Don't expand the number of mutated arguments on config. We are in the process of getting rid of it.
    pub(crate) fn build_per_turn_config(
        session_configuration: &SessionConfiguration,
        cwd: AbsolutePathBuf,
    ) -> Config {
        // todo(aibrahim): store this state somewhere else so we don't need to mut config
        let config = session_configuration.original_config_do_not_use.clone();
        let mut per_turn_config = (*config).clone();
        per_turn_config.cwd = cwd;
        per_turn_config.workspace_roots = session_configuration.workspace_roots.clone();
        per_turn_config
            .permissions
            .set_workspace_roots(session_configuration.workspace_roots.clone());
        per_turn_config.model_reasoning_effort =
            session_configuration.collaboration_mode.reasoning_effort();
        per_turn_config.model_reasoning_summary = session_configuration.model_reasoning_summary;
        per_turn_config.service_tier = session_configuration.service_tier.clone();
        per_turn_config.personality = session_configuration.personality;
        per_turn_config.approvals_reviewer = session_configuration.approvals_reviewer;
        session_configuration
            .apply_permission_profile_to_permissions(&mut per_turn_config.permissions);
        let permission_profile = session_configuration.permission_profile();
        let resolved_web_search_mode =
            resolve_web_search_mode_for_turn(&per_turn_config.web_search_mode, &permission_profile);
        if let Err(err) = per_turn_config
            .web_search_mode
            .set(resolved_web_search_mode)
        {
            let fallback_value = per_turn_config.web_search_mode.value();
            tracing::warn!(
                error = %err,
                ?resolved_web_search_mode,
                ?fallback_value,
                "resolved web_search_mode is disallowed by requirements; keeping constrained value"
            );
        }
        per_turn_config.features = config.features.clone();
        per_turn_config
    }

    pub(crate) fn build_effective_session_config(
        session_configuration: &SessionConfiguration,
    ) -> Config {
        let mut config =
            Self::build_per_turn_config(session_configuration, session_configuration.cwd().clone());
        config.model = Some(session_configuration.collaboration_mode.model().to_string());
        config.permissions.approval_policy = session_configuration.approval_policy.clone();
        config.workspace_roots = session_configuration.workspace_roots.clone();
        config
            .permissions
            .set_workspace_roots(session_configuration.workspace_roots.clone());
        config
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn make_turn_context(
        thread_id: ThreadId,
        session_id: SessionId,
        auth_manager: Option<Arc<AuthManager>>,
        session_telemetry: &SessionTelemetry,
        provider: ModelProviderInfo,
        session_configuration: &SessionConfiguration,
        multi_agent_version: MultiAgentVersion,
        user_shell: &shell::Shell,
        shell_zsh_path: Option<&PathBuf>,
        main_execve_wrapper_exe: Option<&PathBuf>,
        per_turn_config: Config,
        model_info: ModelInfo,
        models_manager: &SharedModelsManager,
        network: Option<NetworkProxy>,
        environments: ResolvedTurnEnvironments,
        cwd: AbsolutePathBuf,
        sub_id: String,
        skills_outcome: Arc<SkillLoadOutcome>,
    ) -> TurnContext {
        let reasoning_effort = session_configuration.collaboration_mode.reasoning_effort();
        let reasoning_summary = session_configuration
            .model_reasoning_summary
            .unwrap_or(model_info.default_reasoning_summary);
        let session_telemetry = session_telemetry.clone().with_model(
            session_configuration.collaboration_mode.model(),
            model_info.slug.as_str(),
        );
        let session_source = session_configuration.session_source.clone();
        let auth_manager_for_context = auth_manager.clone();
        let provider_for_context = create_model_provider(provider, auth_manager);
        let session_telemetry_for_context = session_telemetry;
        let available_models = models_manager.try_list_models().unwrap_or_default();
        let unified_exec_shell_mode = UnifiedExecShellMode::for_session(
            codex_tools::unified_exec_feature_mode_for_features(per_turn_config.features.get()),
            crate::tools::tool_user_shell_type(user_shell),
            shell_zsh_path,
            main_execve_wrapper_exe,
        );

        let mut per_turn_config = per_turn_config;
        let tool_mode = model_info.tool_mode.unwrap_or_else(|| {
            if per_turn_config.features.enabled(Feature::CodeModeOnly) {
                ToolMode::CodeModeOnly
            } else if per_turn_config.features.enabled(Feature::CodeMode) {
                ToolMode::CodeMode
            } else {
                ToolMode::Direct
            }
        });
        per_turn_config.service_tier = get_service_tier(
            per_turn_config.service_tier,
            per_turn_config.features.enabled(Feature::FastMode),
            &model_info,
        );
        let per_turn_config = Arc::new(per_turn_config);
        let turn_metadata_state = Arc::new(TurnMetadataState::new(
            session_id.to_string(),
            thread_id.to_string(),
            session_configuration.forked_from_thread_id,
            session_configuration.parent_thread_id,
            &session_configuration.session_source,
            sub_id.clone(),
            cwd.clone(),
            &session_configuration.permission_profile(),
            session_configuration.windows_sandbox_level,
            network.is_some(),
        ));
        let (current_date, timezone) = local_time_context();
        let extension_data = Arc::new(codex_extension_api::ExtensionData::new(sub_id.clone()));
        extension_data.insert(HostLoadedSkills::new(Arc::clone(&skills_outcome)));
        TurnContext {
            sub_id,
            trace_id: current_span_trace_id(),
            realtime_active: false,
            config: per_turn_config.clone(),
            auth_manager: auth_manager_for_context,
            model_info: model_info.clone(),
            comp_hash: model_info.comp_hash.clone(),
            tool_mode,
            session_telemetry: session_telemetry_for_context,
            provider: provider_for_context,
            reasoning_effort,
            reasoning_summary,
            session_source,
            parent_thread_id: session_configuration.parent_thread_id,
            thread_source: session_configuration.thread_source.clone(),
            environments,
            #[allow(deprecated)]
            cwd,
            current_date: Some(current_date),
            timezone: Some(timezone),
            app_server_client_name: session_configuration.app_server_client_name.clone(),
            developer_instructions: session_configuration.developer_instructions.clone(),
            compact_prompt: session_configuration.compact_prompt.clone(),
            user_instructions: session_configuration
                .loaded_agents_md
                .as_ref()
                .map(LoadedAgentsMd::render),
            collaboration_mode: session_configuration.collaboration_mode.clone(),
            multi_agent_version,
            personality: session_configuration.personality,
            approval_policy: session_configuration.approval_policy.clone(),
            permission_profile: session_configuration.permission_profile(),
            network,
            windows_sandbox_level: session_configuration.windows_sandbox_level,
            shell_environment_policy: per_turn_config.permissions.shell_environment_policy.clone(),
            available_models,
            unified_exec_shell_mode,
            features: per_turn_config.features.clone(),
            ghost_snapshot: per_turn_config.ghost_snapshot.clone(),
            final_output_json_schema: None,
            codex_self_exe: per_turn_config.codex_self_exe.clone(),
            codex_linux_sandbox_exe: per_turn_config.codex_linux_sandbox_exe.clone(),
            truncation_policy: model_info.truncation_policy.into(),
            dynamic_tools: session_configuration.dynamic_tools.clone(),
            turn_metadata_state,
            extension_data,
            turn_skills: TurnSkillsContext::new(skills_outcome),
            turn_timing_state: Arc::new(TurnTimingState::default()),
            server_model_warning_emitted: AtomicBool::new(false),
            model_verification_emitted: AtomicBool::new(false),
        }
    }

    pub(crate) async fn new_turn_with_sub_id(
        &self,
        sub_id: String,
        updates: SessionSettingsUpdate,
    ) -> CodexResult<Arc<TurnContext>> {
        let notify_config_contributors = !self.services.extensions.config_contributors().is_empty();
        let update_result: CodexResult<_> = {
            let mut state = self.state.lock().await;
            match state.session_configuration.clone().apply(&updates) {
                Ok(next) => {
                    let previous_cwd = state.session_configuration.cwd().clone();
                    let previous_permission_profile =
                        state.session_configuration.permission_profile();
                    let next_permission_profile = next.permission_profile();
                    let permission_profile_changed =
                        previous_permission_profile != next_permission_profile;
                    let codex_home = next.codex_home.clone();
                    let session_source = next.session_source.clone();
                    let previous_config = notify_config_contributors.then(|| {
                        Self::build_effective_session_config(&state.session_configuration)
                    });
                    let new_config = notify_config_contributors
                        .then(|| Self::build_effective_session_config(&next));
                    state.session_configuration = next.clone();
                    Ok((
                        next,
                        permission_profile_changed,
                        previous_cwd,
                        codex_home,
                        session_source,
                        previous_config,
                        new_config,
                    ))
                }
                Err(err) => Err(CodexErr::InvalidRequest(err.to_string())),
            }
        };

        let (
            session_configuration,
            permission_profile_changed,
            previous_cwd,
            codex_home,
            session_source,
            previous_config,
            new_config,
        ) = match update_result {
            Ok(update) => update,
            Err(err) => {
                let message = err.to_string();
                self.send_event_raw(Event {
                    id: sub_id.clone(),
                    msg: EventMsg::Error(ErrorEvent {
                        message: message.clone(),
                        codex_error_info: Some(CodexErrorInfo::BadRequest),
                    }),
                })
                .await;
                return Err(CodexErr::InvalidRequest(message));
            }
        };

        self.emit_config_changed_contributors(previous_config.as_ref(), new_config.as_ref());
        self.maybe_refresh_shell_snapshot_for_cwd(
            &previous_cwd,
            session_configuration.cwd(),
            &codex_home,
            &session_source,
        );

        if permission_profile_changed {
            self.refresh_managed_network_proxy_for_current_permission_profile()
                .await;
        }

        Ok(self
            .new_turn_from_configuration(
                sub_id,
                session_configuration,
                updates.final_output_json_schema,
            )
            .await)
    }

    async fn new_turn_from_configuration(
        &self,
        sub_id: String,
        session_configuration: SessionConfiguration,
        final_output_json_schema: Option<Option<Value>>,
    ) -> Arc<TurnContext> {
        self.new_turn_context_from_configuration(
            sub_id,
            session_configuration,
            final_output_json_schema,
            TurnMultiAgentRuntime::ResolveAndStore,
        )
        .await
    }

    async fn new_startup_prewarm_turn_from_configuration(
        &self,
        sub_id: String,
        session_configuration: SessionConfiguration,
    ) -> Arc<TurnContext> {
        self.new_turn_context_from_configuration(
            sub_id,
            session_configuration,
            /*final_output_json_schema*/ None,
            TurnMultiAgentRuntime::Preview,
        )
        .await
    }

    #[instrument(name = "turn_context.build", level = "trace", skip_all)]
    async fn new_turn_context_from_configuration(
        &self,
        sub_id: String,
        session_configuration: SessionConfiguration,
        final_output_json_schema: Option<Option<Value>>,
        multi_agent_runtime: TurnMultiAgentRuntime,
    ) -> Arc<TurnContext> {
        let turn_environments = crate::environment_selection::resolve_environment_selections(
            self.services.environment_manager.as_ref(),
            session_configuration.environment_selections(),
        )
        .await
        .unwrap_or_else(|err| {
            warn!("failed to resolve turn environments: {err}");
            ResolvedTurnEnvironments::default()
        });
        let primary_turn_environment = turn_environments.primary().cloned();
        let cwd = primary_turn_environment
            .as_ref()
            .map(|turn_environment| turn_environment.cwd().clone())
            .unwrap_or_else(|| session_configuration.cwd().clone());
        let per_turn_config = Self::build_per_turn_config(&session_configuration, cwd.clone());
        {
            let mcp_connection_manager = self.services.mcp_connection_manager.load_full();
            mcp_connection_manager.set_approval_policy(&session_configuration.approval_policy);
            mcp_connection_manager
                .set_permission_profile(session_configuration.permission_profile());
        }

        let model_info = self
            .services
            .models_manager
            .get_model_info(
                session_configuration.collaboration_mode.model(),
                &per_turn_config.to_models_manager_config(),
            )
            .await;

        let multi_agent_version = match multi_agent_runtime {
            TurnMultiAgentRuntime::ResolveAndStore => {
                self.resolve_multi_agent_version_for_model(&model_info, &per_turn_config)
            }
            TurnMultiAgentRuntime::Preview => self
                .multi_agent_version()
                .or(model_info.multi_agent_version)
                .unwrap_or_else(|| per_turn_config.multi_agent_version_from_features()),
        };
        let plugin_outcome = self
            .services
            .plugins_manager
            .plugins_for_config(&per_turn_config.plugins_config_input())
            .await;
        let effective_skill_roots = plugin_outcome.effective_plugin_skill_roots();
        let skills_input = skills_load_input_from_config(&per_turn_config, effective_skill_roots);
        let fs = primary_turn_environment
            .map(|turn_environment| turn_environment.environment.get_filesystem());
        let skills_outcome = Arc::new(
            self.services
                .skills_manager
                .skills_for_config(&skills_input, fs)
                .await,
        );
        let mut turn_context: TurnContext = Self::make_turn_context(
            self.thread_id(),
            self.session_id(),
            Some(Arc::clone(&self.services.auth_manager)),
            &self.services.session_telemetry,
            session_configuration.provider.clone(),
            &session_configuration,
            multi_agent_version,
            self.services.user_shell.as_ref(),
            self.services.shell_zsh_path.as_ref(),
            self.services.main_execve_wrapper_exe.as_ref(),
            per_turn_config,
            model_info,
            &self.services.models_manager,
            self.services
                .network_proxy
                .load_full()
                .as_ref()
                .and_then(|started_proxy| {
                    Self::managed_network_proxy_active_for_permission_profile(
                        &session_configuration.permission_profile(),
                    )
                    .then(|| started_proxy.proxy())
                }),
            turn_environments,
            cwd,
            sub_id,
            skills_outcome,
        );
        turn_context.realtime_active = self.conversation.running_state().await.is_some();

        if let Some(final_schema) = final_output_json_schema {
            turn_context.final_output_json_schema = final_schema;
        }
        let turn_context = Arc::new(turn_context);
        if turn_context
            .environments
            .single_local_environment_cwd()
            .is_some()
        {
            turn_context.turn_metadata_state.spawn_git_enrichment_task();
        }
        turn_context
    }

    pub(crate) async fn maybe_emit_unknown_model_warning_for_turn(&self, tc: &TurnContext) {
        if tc.model_info.used_fallback_model_metadata {
            self.send_event(
                tc,
                EventMsg::Warning(WarningEvent {
                    message: format!(
                        "Model metadata for `{}` not found. Defaulting to fallback metadata; this can degrade performance and cause issues.",
                        tc.model_info.slug
                    ),
                }),
            )
            .await;
        }
    }

    pub(crate) async fn new_default_turn(&self) -> Arc<TurnContext> {
        self.new_default_turn_with_sub_id(self.next_internal_sub_id())
            .await
    }

    pub(crate) async fn new_default_turn_with_sub_id(&self, sub_id: String) -> Arc<TurnContext> {
        let session_configuration = self.default_turn_configuration().await;
        self.new_turn_from_configuration(
            sub_id,
            session_configuration,
            /*final_output_json_schema*/ None,
        )
        .await
    }

    pub(crate) async fn new_startup_prewarm_turn_with_sub_id(
        &self,
        sub_id: String,
    ) -> Arc<TurnContext> {
        let session_configuration = self.default_turn_configuration().await;
        self.new_startup_prewarm_turn_from_configuration(sub_id, session_configuration)
            .await
    }

    async fn default_turn_configuration(&self) -> SessionConfiguration {
        let state = self.state.lock().await;
        state.session_configuration.clone()
    }
}
