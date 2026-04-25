use super::*;
use crate::config::ConstraintError;
use tokio::sync::Semaphore;

/// Context for an initialized model agent
///
/// A session has at most 1 running task at a time, and can be interrupted by user input.
pub(crate) struct Session {
    pub(crate) conversation_id: ThreadId,
    pub(super) tx_event: Sender<Event>,
    pub(super) agent_status: watch::Sender<AgentStatus>,
    pub(super) out_of_band_elicitation_paused: watch::Sender<bool>,
    pub(super) state: Mutex<SessionState>,
    /// Serializes rebuild/apply cycles for the running proxy; each cycle
    /// rebuilds from the current SessionState while holding this lock.
    pub(super) managed_network_proxy_refresh_lock: Semaphore,
    /// The set of enabled features should be invariant for the lifetime of the
    /// session.
    pub(super) features: ManagedFeatures,
    pub(super) pending_mcp_server_refresh_config: Mutex<Option<McpServerRefreshConfig>>,
    pub(crate) conversation: Arc<RealtimeConversationManager>,
    pub(crate) active_turn: Mutex<Option<ActiveTurn>>,
    pub(super) mailbox: Mailbox,
    pub(super) mailbox_rx: Mutex<MailboxReceiver>,
    pub(super) idle_pending_input: Mutex<Vec<ResponseInputItem>>, // TODO (jif) merge with mailbox!
    pub(crate) guardian_review_session: GuardianReviewSessionManager,
    pub(crate) services: SessionServices,
    pub(super) next_internal_sub_id: AtomicU64,
}

#[derive(Clone)]
pub(crate) struct SessionConfiguration {
    /// Provider identifier ("openai", "openrouter", ...).
    pub(super) provider: ModelProviderInfo,

    pub(super) collaboration_mode: CollaborationMode,
    pub(super) model_reasoning_summary: Option<ReasoningSummaryConfig>,
    pub(super) service_tier: Option<ServiceTier>,

    /// Developer instructions that supplement the base instructions.
    pub(super) developer_instructions: Option<String>,

    /// Model instructions that are appended to the base instructions.
    pub(super) user_instructions: Option<String>,

    /// Personality preference for the model.
    pub(super) personality: Option<Personality>,

    /// Base instructions for the session.
    pub(super) base_instructions: String,

    /// Compact prompt override.
    pub(super) compact_prompt: Option<String>,

    /// When to escalate for approval for execution
    pub(super) approval_policy: Constrained<AskForApproval>,
    pub(super) approvals_reviewer: ApprovalsReviewer,
    /// Canonical permission profile for the session.
    pub(super) permission_profile: Constrained<PermissionProfile>,
    pub(super) windows_sandbox_level: WindowsSandboxLevel,

    /// Absolute working directory that should be treated as the *root* of the
    /// session. All relative paths supplied by the model as well as the
    /// execution sandbox are resolved against this directory **instead** of
    /// the process-wide current working directory.
    pub(super) cwd: AbsolutePathBuf,
    /// Directory containing all Codex state for this session.
    pub(super) codex_home: AbsolutePathBuf,
    /// Optional user-facing name for the thread, updated during the session.
    pub(super) thread_name: Option<String>,
    /// Sticky environments for turns that do not provide a turn-local override.
    pub(super) environments: Vec<TurnEnvironmentSelection>,

    // TODO(pakrym): Remove config from here
    pub(super) original_config_do_not_use: Arc<Config>,
    /// Optional service name tag for session metrics.
    pub(super) metrics_service_name: Option<String>,
    pub(super) app_server_client_name: Option<String>,
    pub(super) app_server_client_version: Option<String>,
    /// Source of the session (cli, vscode, exec, mcp, ...)
    pub(super) session_source: SessionSource,
    pub(super) dynamic_tools: Vec<DynamicToolSpec>,
    pub(super) persist_extended_history: bool,
    pub(super) inherited_shell_snapshot: Option<Arc<ShellSnapshot>>,
    pub(super) user_shell_override: Option<shell::Shell>,
}

impl SessionConfiguration {
    pub(crate) fn codex_home(&self) -> &AbsolutePathBuf {
        &self.codex_home
    }

    pub(super) fn permission_profile(&self) -> PermissionProfile {
        self.permission_profile.get().clone()
    }

    pub(super) fn sandbox_policy(&self) -> SandboxPolicy {
        self.permission_profile()
            .to_legacy_sandbox_policy(&self.cwd)
            .unwrap_or_else(|_| {
                let file_system_sandbox_policy = self.file_system_sandbox_policy();
                codex_sandboxing::compatibility_sandbox_policy_for_permission_profile(
                    self.permission_profile.get(),
                    &file_system_sandbox_policy,
                    self.network_sandbox_policy(),
                    &self.cwd,
                )
            })
    }

    pub(super) fn file_system_sandbox_policy(&self) -> FileSystemSandboxPolicy {
        self.permission_profile.get().file_system_sandbox_policy()
    }

    pub(super) fn network_sandbox_policy(&self) -> NetworkSandboxPolicy {
        self.permission_profile.get().network_sandbox_policy()
    }

    pub(super) fn thread_config_snapshot(&self) -> ThreadConfigSnapshot {
        ThreadConfigSnapshot {
            model: self.collaboration_mode.model().to_string(),
            model_provider_id: self.original_config_do_not_use.model_provider_id.clone(),
            service_tier: self.service_tier,
            approval_policy: self.approval_policy.value(),
            approvals_reviewer: self.approvals_reviewer,
            sandbox_policy: self.sandbox_policy(),
            permission_profile: self.permission_profile(),
            cwd: self.cwd.clone(),
            ephemeral: self.original_config_do_not_use.ephemeral,
            reasoning_effort: self.collaboration_mode.reasoning_effort(),
            personality: self.personality,
            session_source: self.session_source.clone(),
        }
    }

    pub(crate) fn apply(&self, updates: &SessionSettingsUpdate) -> ConstraintResult<Self> {
        let mut next_configuration = self.clone();
        let current_sandbox_policy = self.sandbox_policy();
        let current_file_system_sandbox_policy = self.file_system_sandbox_policy();
        let current_network_sandbox_policy = self.network_sandbox_policy();
        let legacy_file_system_projection =
            FileSystemSandboxPolicy::from_legacy_sandbox_policy_preserving_deny_entries(
                &current_sandbox_policy,
                &self.cwd,
                &current_file_system_sandbox_policy,
            );
        let file_system_policy_matches_legacy = current_file_system_sandbox_policy
            .is_semantically_equivalent_to(&legacy_file_system_projection, &self.cwd);
        if let Some(collaboration_mode) = updates.collaboration_mode.clone() {
            next_configuration.collaboration_mode = collaboration_mode;
        }
        if let Some(summary) = updates.reasoning_summary {
            next_configuration.model_reasoning_summary = Some(summary);
        }
        if let Some(service_tier) = updates.service_tier {
            next_configuration.service_tier = service_tier;
        }
        if let Some(personality) = updates.personality {
            next_configuration.personality = Some(personality);
        }
        if let Some(approval_policy) = updates.approval_policy {
            next_configuration.approval_policy.set(approval_policy)?;
        }
        if let Some(approvals_reviewer) = updates.approvals_reviewer {
            next_configuration.approvals_reviewer = approvals_reviewer;
        }
        if let Some(windows_sandbox_level) = updates.windows_sandbox_level {
            next_configuration.windows_sandbox_level = windows_sandbox_level;
        }

        let absolute_cwd = updates
            .cwd
            .as_ref()
            .map(|cwd| {
                AbsolutePathBuf::relative_to_current_dir(normalize_for_native_workdir(
                    cwd.as_path(),
                ))
                .unwrap_or_else(|e| {
                    warn!("failed to normalize update cwd: {cwd:?}: {e}");
                    self.cwd.clone()
                })
            })
            .unwrap_or_else(|| self.cwd.clone());

        let cwd_changed = absolute_cwd.as_path() != self.cwd.as_path();
        next_configuration.cwd = absolute_cwd.clone();
        if cwd_changed
            && let Some(primary_environment) = next_configuration.environments.first_mut()
        {
            primary_environment.cwd = absolute_cwd;
        }

        if let Some(permission_profile) = updates.permission_profile.clone() {
            next_configuration.set_permission_profile_projection(
                permission_profile,
                Some(&current_file_system_sandbox_policy),
            )?;
        } else if let Some(sandbox_policy) = updates.sandbox_policy.clone() {
            let file_system_sandbox_policy =
                FileSystemSandboxPolicy::from_legacy_sandbox_policy_preserving_deny_entries(
                    &sandbox_policy,
                    &next_configuration.cwd,
                    &current_file_system_sandbox_policy,
                );
            let network_sandbox_policy = NetworkSandboxPolicy::from(&sandbox_policy);
            next_configuration.permission_profile.set(
                PermissionProfile::from_runtime_permissions_with_enforcement(
                    SandboxEnforcement::from_legacy_sandbox_policy(&sandbox_policy),
                    &file_system_sandbox_policy,
                    network_sandbox_policy,
                ),
            )?;
        } else if cwd_changed && file_system_policy_matches_legacy {
            // Preserve richer split policies across cwd-only updates; only
            // rederive when the session is already using the legacy bridge.
            let file_system_sandbox_policy =
                FileSystemSandboxPolicy::from_legacy_sandbox_policy_preserving_deny_entries(
                    &current_sandbox_policy,
                    &next_configuration.cwd,
                    &current_file_system_sandbox_policy,
                );
            next_configuration.permission_profile.set(
                PermissionProfile::from_runtime_permissions_with_enforcement(
                    SandboxEnforcement::from_legacy_sandbox_policy(&current_sandbox_policy),
                    &file_system_sandbox_policy,
                    current_network_sandbox_policy,
                ),
            )?;
        }
        if let Some(app_server_client_name) = updates.app_server_client_name.clone() {
            next_configuration.app_server_client_name = Some(app_server_client_name);
        }
        if let Some(app_server_client_version) = updates.app_server_client_version.clone() {
            next_configuration.app_server_client_version = Some(app_server_client_version);
        }
        Ok(next_configuration)
    }

    fn set_permission_profile_projection(
        &mut self,
        permission_profile: PermissionProfile,
        preserve_deny_reads_from: Option<&FileSystemSandboxPolicy>,
    ) -> ConstraintResult<()> {
        let enforcement = permission_profile.enforcement();
        let (mut file_system_sandbox_policy, network_sandbox_policy) =
            permission_profile.to_runtime_permissions();
        if let Some(existing_file_system_policy) = preserve_deny_reads_from {
            file_system_sandbox_policy
                .preserve_deny_read_restrictions_from(existing_file_system_policy);
        }
        let effective_permission_profile =
            PermissionProfile::from_runtime_permissions_with_enforcement(
                enforcement,
                &file_system_sandbox_policy,
                network_sandbox_policy,
            );
        effective_permission_profile
            .to_legacy_sandbox_policy(&self.cwd)
            .map_err(|err| ConstraintError::InvalidValue {
                field_name: "permission_profile",
                candidate: format!("{effective_permission_profile:?}"),
                allowed: format!(
                    "permission profiles that can be represented by the active sandbox constraints: {err}"
                ),
                requirement_source: codex_config::RequirementSource::Unknown,
            })?;
        self.permission_profile.set(effective_permission_profile)?;
        Ok(())
    }
}

#[derive(Default, Clone)]
pub(crate) struct SessionSettingsUpdate {
    pub(crate) cwd: Option<PathBuf>,
    pub(crate) approval_policy: Option<AskForApproval>,
    pub(crate) approvals_reviewer: Option<ApprovalsReviewer>,
    pub(crate) sandbox_policy: Option<SandboxPolicy>,
    pub(crate) permission_profile: Option<PermissionProfile>,
    pub(crate) windows_sandbox_level: Option<WindowsSandboxLevel>,
    pub(crate) collaboration_mode: Option<CollaborationMode>,
    pub(crate) reasoning_summary: Option<ReasoningSummaryConfig>,
    pub(crate) service_tier: Option<Option<ServiceTier>>,
    pub(crate) final_output_json_schema: Option<Option<Value>>,
    /// Turn-local environment override. `None` inherits the sticky thread
    /// environments stored on `SessionConfiguration`; `Some([])` explicitly
    /// disables environments for this turn.
    pub(crate) environments: Option<Vec<TurnEnvironmentSelection>>,
    pub(crate) personality: Option<Personality>,
    pub(crate) app_server_client_name: Option<String>,
    pub(crate) app_server_client_version: Option<String>,
}

pub(crate) struct AppServerClientMetadata {
    pub(crate) client_name: Option<String>,
    pub(crate) client_version: Option<String>,
}

impl Session {
    #[instrument(name = "session_init", level = "info", skip_all)]
    #[allow(clippy::too_many_arguments)]
    #[expect(
        clippy::await_holding_invalid_type,
        reason = "session initialization must serialize access through session-owned manager guards"
    )]
    pub(crate) async fn new(
        mut session_configuration: SessionConfiguration,
        config: Arc<Config>,
        auth_manager: Arc<AuthManager>,
        models_manager: SharedModelsManager,
        exec_policy: Arc<ExecPolicyManager>,
        tx_event: Sender<Event>,
        agent_status: watch::Sender<AgentStatus>,
        initial_history: InitialHistory,
        session_source: SessionSource,
        skills_manager: Arc<SkillsManager>,
        plugins_manager: Arc<PluginsManager>,
        mcp_manager: Arc<McpManager>,
        skills_watcher: Arc<SkillsWatcher>,
        agent_control: AgentControl,
        environment_manager: Arc<EnvironmentManager>,
        analytics_events_client: Option<AnalyticsEventsClient>,
        thread_store: Arc<dyn ThreadStore>,
        parent_rollout_thread_trace: ThreadTraceContext,
    ) -> anyhow::Result<Arc<Self>> {
        debug!(
            "Configuring session: model={}; provider={:?}",
            session_configuration.collaboration_mode.model(),
            session_configuration.provider
        );
        let forked_from_id = initial_history.forked_from_id();

        let event_persistence_mode = if session_configuration.persist_extended_history {
            ThreadEventPersistenceMode::Extended
        } else {
            ThreadEventPersistenceMode::Limited
        };
        let conversation_id = match &initial_history {
            InitialHistory::New | InitialHistory::Cleared | InitialHistory::Forked(_) => {
                ThreadId::default()
            }
            InitialHistory::Resumed(resumed_history) => resumed_history.conversation_id,
        };
        let window_generation = match &initial_history {
            InitialHistory::Resumed(resumed_history) => u64::try_from(
                resumed_history
                    .history
                    .iter()
                    .filter(|item| matches!(item, RolloutItem::Compacted(_)))
                    .count(),
            )
            .unwrap_or(u64::MAX),
            InitialHistory::New | InitialHistory::Cleared | InitialHistory::Forked(_) => 0,
        };
        // Kick off independent async setup tasks in parallel to reduce startup latency.
        //
        // - initialize thread persistence with new or resumed session info
        // - perform default shell discovery
        // - load history metadata (skipped for subagents)
        let thread_persistence_fut = async {
            if config.ephemeral {
                Ok::<_, anyhow::Error>(None)
            } else {
                let live_thread = match &initial_history {
                    InitialHistory::New | InitialHistory::Cleared | InitialHistory::Forked(_) => {
                        LiveThread::create(
                            Arc::clone(&thread_store),
                            CreateThreadParams {
                                thread_id: conversation_id,
                                forked_from_id,
                                source: session_source,
                                base_instructions: BaseInstructions {
                                    text: session_configuration.base_instructions.clone(),
                                },
                                dynamic_tools: session_configuration.dynamic_tools.clone(),
                                event_persistence_mode,
                            },
                        )
                        .await?
                    }
                    InitialHistory::Resumed(resumed_history) => {
                        LiveThread::resume(
                            Arc::clone(&thread_store),
                            ResumeThreadParams {
                                thread_id: resumed_history.conversation_id,
                                rollout_path: resumed_history.rollout_path.clone(),
                                history: Some(resumed_history.history.clone()),
                                include_archived: true,
                                event_persistence_mode,
                            },
                        )
                        .await?
                    }
                };
                Ok(Some(live_thread))
            }
        }
        .instrument(info_span!(
            "session_init.thread_persistence",
            otel.name = "session_init.thread_persistence",
            session_init.ephemeral = config.ephemeral,
        ));
        let state_db_fut = async {
            if config.ephemeral {
                None
            } else if let Some(local_store) =
                thread_store.as_any().downcast_ref::<LocalThreadStore>()
            {
                local_store.state_db().await
            } else {
                None
            }
        }
        .instrument(info_span!(
            "session_init.state_db",
            otel.name = "session_init.state_db",
            session_init.ephemeral = config.ephemeral,
        ));

        let is_subagent = matches!(
            session_configuration.session_source,
            SessionSource::SubAgent(_)
        );
        let history_meta_fut = async {
            if is_subagent {
                (0, 0)
            } else {
                crate::message_history::history_metadata(&config).await
            }
        }
        .instrument(info_span!(
            "session_init.history_metadata",
            otel.name = "session_init.history_metadata",
            session_init.is_subagent = is_subagent,
        ));
        let auth_manager_clone = Arc::clone(&auth_manager);
        let config_for_mcp = Arc::clone(&config);
        let mcp_manager_for_mcp = Arc::clone(&mcp_manager);
        let auth_and_mcp_fut = async move {
            let auth = auth_manager_clone.auth().await;
            let mcp_servers = mcp_manager_for_mcp
                .effective_servers(&config_for_mcp, auth.as_ref())
                .await;
            let auth_statuses = compute_auth_statuses(
                mcp_servers.iter(),
                config_for_mcp.mcp_oauth_credentials_store_mode,
                auth.as_ref(),
            )
            .await;
            (auth, mcp_servers, auth_statuses)
        }
        .instrument(info_span!(
            "session_init.auth_mcp",
            otel.name = "session_init.auth_mcp",
        ));

        // Join all independent futures.
        let (
            thread_persistence_result,
            state_db_ctx,
            (history_log_id, history_entry_count),
            (auth, mcp_servers, auth_statuses),
        ) = tokio::join!(
            thread_persistence_fut,
            state_db_fut,
            history_meta_fut,
            auth_and_mcp_fut
        );

        let mut live_thread_init =
            LiveThreadInitGuard::new(thread_persistence_result.map_err(|e| {
                error!("failed to initialize thread persistence: {e:#}");
                e
            })?);
        let session_result: anyhow::Result<Arc<Self>> = async {
            let rollout_path = if let Some(live_thread) = live_thread_init.as_ref() {
                live_thread.local_rollout_path().await?
            } else {
                None
            };
            let trace_agent_path = session_configuration
                .session_source
                .get_agent_path()
                .unwrap_or_else(codex_protocol::AgentPath::root);
            let trace_task_name =
                (!trace_agent_path.is_root()).then(|| trace_agent_path.name().to_string());
            let trace_metadata = ThreadStartedTraceMetadata {
                thread_id: conversation_id.to_string(),
                agent_path: trace_agent_path.to_string(),
                task_name: trace_task_name,
                nickname: session_configuration.session_source.get_nickname(),
                agent_role: session_configuration.session_source.get_agent_role(),
                session_source: session_configuration.session_source.clone(),
                cwd: session_configuration.cwd.to_path_buf(),
                rollout_path: rollout_path.clone(),
                model: session_configuration.collaboration_mode.model().to_string(),
                provider_name: config.model_provider_id.clone(),
                approval_policy: session_configuration.approval_policy.value().to_string(),
                sandbox_policy: format!("{:?}", session_configuration.sandbox_policy()),
            };
            let rollout_thread_trace = if matches!(
                session_configuration.session_source,
                SessionSource::SubAgent(SubAgentSource::ThreadSpawn { .. })
            ) {
                // Spawned child threads are part of their root rollout tree. If the
                // parent had no trace bundle, do not create an orphan child bundle
                // that looks like an independent rollout.
                parent_rollout_thread_trace.start_child_thread_trace_or_disabled(trace_metadata)
            } else {
                ThreadTraceContext::start_root_or_disabled(trace_metadata)
            };

            let mut post_session_configured_events = Vec::<Event>::new();

            for usage in config.features.legacy_feature_usages() {
                post_session_configured_events.push(Event {
                    id: INITIAL_SUBMIT_ID.to_owned(),
                    msg: EventMsg::DeprecationNotice(DeprecationNoticeEvent {
                        summary: usage.summary.clone(),
                        details: usage.details.clone(),
                    }),
                });
            }
            if crate::config::uses_deprecated_instructions_file(&config.config_layer_stack) {
                post_session_configured_events.push(Event {
                    id: INITIAL_SUBMIT_ID.to_owned(),
                    msg: EventMsg::DeprecationNotice(DeprecationNoticeEvent {
                        summary: "`experimental_instructions_file` is deprecated and ignored. Use `model_instructions_file` instead."
                            .to_string(),
                        details: Some(
                            "Move the setting to `model_instructions_file` in config.toml (or under a profile) to load instructions from a file."
                                .to_string(),
                        ),
                    }),
                });
            }
            for message in &config.startup_warnings {
                post_session_configured_events.push(Event {
                    id: "".to_owned(),
                    msg: EventMsg::Warning(WarningEvent {
                        message: message.clone(),
                    }),
                });
            }
            let config_path = config.codex_home.join(CONFIG_TOML_FILE);
            if let Some(event) = unstable_features_warning_event(
                config
                    .config_layer_stack
                    .effective_config()
                    .get("features")
                    .and_then(TomlValue::as_table),
                config.suppress_unstable_features_warning,
                &config.features,
                &config_path.display().to_string(),
            ) {
                post_session_configured_events.push(event);
            }
            if config.permissions.approval_policy.value() == AskForApproval::OnFailure {
                post_session_configured_events.push(Event {
                    id: "".to_owned(),
                    msg: EventMsg::Warning(WarningEvent {
                        message: "`on-failure` approval policy is deprecated and will be removed in a future release. Use `on-request` for interactive approvals or `never` for non-interactive runs.".to_string(),
                    }),
                });
            }

            let auth = auth.as_ref();
            let auth_mode = auth.map(CodexAuth::auth_mode).map(TelemetryAuthMode::from);
            let account_id = auth.and_then(CodexAuth::get_account_id);
            let account_email = auth.and_then(CodexAuth::get_account_email);
            let originator = originator().value;
            let terminal_type = user_agent();
            let session_model = session_configuration.collaboration_mode.model().to_string();
            let auth_env_telemetry = collect_auth_env_telemetry(
                &session_configuration.provider,
                auth_manager.codex_api_key_env_enabled(),
            );
            let mut session_telemetry = SessionTelemetry::new(
                conversation_id,
                session_model.as_str(),
                session_model.as_str(),
                account_id.clone(),
                account_email.clone(),
                auth_mode,
                originator.clone(),
                config.otel.log_user_prompt,
                terminal_type.clone(),
                session_configuration.session_source.clone(),
            )
            .with_auth_env(auth_env_telemetry.to_otel_metadata());
            if let Some(service_name) = session_configuration.metrics_service_name.as_deref() {
                session_telemetry = session_telemetry.with_metrics_service_name(service_name);
            }
            let network_proxy_audit_metadata = NetworkProxyAuditMetadata {
                conversation_id: Some(conversation_id.to_string()),
                app_version: Some(env!("CARGO_PKG_VERSION").to_string()),
                user_account_id: account_id,
                auth_mode: auth_mode.map(|mode| mode.to_string()),
                originator: Some(originator),
                user_email: account_email,
                terminal_type: Some(terminal_type),
                model: Some(session_model.clone()),
                slug: Some(session_model),
            };
            config.features.emit_metrics(&session_telemetry);
            session_telemetry.counter(
                THREAD_STARTED_METRIC,
                /*inc*/ 1,
                &[(
                    "is_git",
                    if get_git_repo_root(&session_configuration.cwd).is_some() {
                        "true"
                    } else {
                        "false"
                    },
                )],
            );

            session_telemetry.conversation_starts(
                config.model_provider.name.as_str(),
                session_configuration.collaboration_mode.reasoning_effort(),
                config
                    .model_reasoning_summary
                    .unwrap_or(ReasoningSummaryConfig::Auto),
                config.model_context_window,
                config.model_auto_compact_token_limit,
                config.permissions.approval_policy.value(),
                config.permissions.sandbox_policy.get().clone(),
                mcp_servers.keys().map(String::as_str).collect(),
                config.active_profile.clone(),
            );

            let use_zsh_fork_shell = config.features.enabled(Feature::ShellZshFork);
            let mut default_shell = if let Some(user_shell_override) =
                session_configuration.user_shell_override.clone()
            {
                user_shell_override
            } else if use_zsh_fork_shell {
                let zsh_path = config.zsh_path.as_ref().ok_or_else(|| {
                    anyhow::anyhow!(
                        "zsh fork feature enabled, but `zsh_path` is not configured; set `zsh_path` in config.toml"
                    )
                })?;
                let zsh_path = zsh_path.to_path_buf();
                shell::get_shell(shell::ShellType::Zsh, Some(&zsh_path)).ok_or_else(|| {
                    anyhow::anyhow!(
                        "zsh fork feature enabled, but zsh_path `{}` is not usable; set `zsh_path` to a valid zsh executable",
                        zsh_path.display()
                    )
                })?
            } else {
                shell::default_user_shell()
            };
            // Create the mutable state for the Session.
            let shell_snapshot_tx = if config.features.enabled(Feature::ShellSnapshot) {
                if let Some(snapshot) = session_configuration.inherited_shell_snapshot.clone() {
                    let (tx, rx) = watch::channel(Some(snapshot));
                    default_shell.shell_snapshot = rx;
                    tx
                } else {
                    ShellSnapshot::start_snapshotting(
                        config.codex_home.clone(),
                        conversation_id,
                        session_configuration.cwd.clone(),
                        &mut default_shell,
                        session_telemetry.clone(),
                    )
                }
            } else {
                let (tx, rx) = watch::channel(None);
                default_shell.shell_snapshot = rx;
                tx
            };
            let thread_name =
                thread_title_from_state_db(state_db_ctx.as_ref(), &config.codex_home, conversation_id)
                    .instrument(info_span!(
                        "session_init.thread_name_lookup",
                        otel.name = "session_init.thread_name_lookup",
                    ))
                    .await;
            session_configuration.thread_name = thread_name.clone();
            let state = SessionState::new(session_configuration.clone());
            let managed_network_requirements_configured = config
                .config_layer_stack
                .requirements_toml()
                .network
                .is_some();
            let managed_network_requirements_enabled = config.managed_network_requirements_enabled();
            let network_approval = Arc::new(NetworkApprovalService::default());
            // The managed proxy can call back into core for allowlist-miss decisions.
            let network_policy_decider_session = if managed_network_requirements_configured {
                config
                    .permissions
                    .network
                    .as_ref()
                    .map(|_| Arc::new(RwLock::new(std::sync::Weak::<Session>::new())))
            } else {
                None
            };
            let blocked_request_observer = if managed_network_requirements_configured {
                config
                    .permissions
                    .network
                    .as_ref()
                    .map(|_| build_blocked_request_observer(Arc::clone(&network_approval)))
            } else {
                None
            };
            let network_policy_decider =
                network_policy_decider_session
                    .as_ref()
                    .map(|network_policy_decider_session| {
                        build_network_policy_decider(
                            Arc::clone(&network_approval),
                            Arc::clone(network_policy_decider_session),
                        )
                    });
            let (network_proxy, session_network_proxy) =
                if let Some(spec) = config.permissions.network.as_ref() {
                    let current_exec_policy = exec_policy.current();
                    let (network_proxy, session_network_proxy) = Self::start_managed_network_proxy(
                        spec,
                        current_exec_policy.as_ref(),
                        config.permissions.sandbox_policy.get(),
                        network_policy_decider.as_ref().map(Arc::clone),
                        blocked_request_observer.as_ref().map(Arc::clone),
                        managed_network_requirements_configured,
                        network_proxy_audit_metadata,
                    )
                    .instrument(info_span!(
                        "session_init.network_proxy",
                        otel.name = "session_init.network_proxy",
                        session_init.managed_network_requirements_enabled =
                            managed_network_requirements_enabled,
                    ))
                    .await?;
                    (Some(network_proxy), Some(session_network_proxy))
                } else {
                    (None, None)
                };

            let mut hook_shell_argv =
                default_shell.derive_exec_args("", /*use_login_shell*/ false);
            let hook_shell_program = hook_shell_argv.remove(0);
            let _ = hook_shell_argv.pop();
            let hooks = Hooks::new(HooksConfig {
                legacy_notify_argv: config.notify.clone(),
                feature_enabled: config.features.enabled(Feature::CodexHooks),
                config_layer_stack: Some(config.config_layer_stack.clone()),
                shell_program: Some(hook_shell_program),
                shell_args: hook_shell_argv,
            });
            for warning in hooks.startup_warnings() {
                post_session_configured_events.push(Event {
                    id: INITIAL_SUBMIT_ID.to_owned(),
                    msg: EventMsg::Warning(WarningEvent {
                        message: warning.clone(),
                    }),
                });
            }

            let installation_id = resolve_installation_id(&config.codex_home).await?;
            let analytics_events_client = analytics_events_client.unwrap_or_else(|| {
                AnalyticsEventsClient::new(
                    Arc::clone(&auth_manager),
                    config.chatgpt_base_url.trim_end_matches('/').to_string(),
                    config.analytics_enabled,
                )
            });
            let services = SessionServices {
                // Initialize the MCP connection manager with an uninitialized
                // instance. It will be replaced with one created via
                // McpConnectionManager::new() once all its constructor args are
                // available. This also ensures `SessionConfigured` is emitted
                // before any MCP-related events. It is reasonable to consider
                // changing this to use Option or OnceCell, though the current
                // setup is straightforward enough and performs well.
                mcp_connection_manager: Arc::new(RwLock::new(McpConnectionManager::new_uninitialized(
                    &config.permissions.approval_policy,
                    &config.permissions.permission_profile,
                ))),
                mcp_startup_cancellation_token: Mutex::new(CancellationToken::new()),
                unified_exec_manager: UnifiedExecProcessManager::new(
                    config.background_terminal_max_timeout,
                ),
                shell_zsh_path: config.zsh_path.clone(),
                main_execve_wrapper_exe: config.main_execve_wrapper_exe.clone(),
                analytics_events_client,
                hooks,
                rollout_thread_trace,
                user_shell: Arc::new(default_shell),
                shell_snapshot_tx,
                show_raw_agent_reasoning: config.show_raw_agent_reasoning,
                exec_policy,
                auth_manager: Arc::clone(&auth_manager),
                session_telemetry,
                models_manager: Arc::clone(&models_manager),
                tool_approvals: Mutex::new(ApprovalStore::default()),
                guardian_rejections: Mutex::new(HashMap::new()),
                guardian_rejection_circuit_breaker: Mutex::new(Default::default()),
                runtime_handle: tokio::runtime::Handle::current(),
                skills_manager,
                plugins_manager: Arc::clone(&plugins_manager),
                mcp_manager: Arc::clone(&mcp_manager),
                skills_watcher,
                agent_control,
                network_proxy,
                network_approval: Arc::clone(&network_approval),
                state_db: state_db_ctx.clone(),
                live_thread: live_thread_init.as_ref().cloned(),
                thread_store: Arc::clone(&thread_store),
                model_client: ModelClient::new(
                    Some(Arc::clone(&auth_manager)),
                    conversation_id,
                    installation_id,
                    session_configuration.provider.clone(),
                    session_configuration.session_source.clone(),
                    config.model_verbosity,
                    config.features.enabled(Feature::EnableRequestCompression),
                    config.features.enabled(Feature::RuntimeMetrics),
                    Self::build_model_client_beta_features_header(config.as_ref()),
                ),
                code_mode_service: crate::tools::code_mode::CodeModeService::new(),
                environment_manager,
            };
            services
                .model_client
                .set_window_generation(window_generation);
            let (out_of_band_elicitation_paused, _out_of_band_elicitation_paused_rx) =
                watch::channel(false);

            let (mailbox, mailbox_rx) = Mailbox::new();
            let sess = Arc::new(Session {
                conversation_id,
                tx_event: tx_event.clone(),
                agent_status,
                out_of_band_elicitation_paused,
                state: Mutex::new(state),
                managed_network_proxy_refresh_lock: Semaphore::new(/*permits*/ 1),
                features: config.features.clone(),
                pending_mcp_server_refresh_config: Mutex::new(None),
                conversation: Arc::new(RealtimeConversationManager::new()),
                active_turn: Mutex::new(None),
                mailbox,
                mailbox_rx: Mutex::new(mailbox_rx),
                idle_pending_input: Mutex::new(Vec::new()),
                guardian_review_session: GuardianReviewSessionManager::default(),
                services,
                next_internal_sub_id: AtomicU64::new(0),
            });
            if let Some(network_policy_decider_session) = network_policy_decider_session {
                let mut guard = network_policy_decider_session.write().await;
                *guard = Arc::downgrade(&sess);
            }
            // Dispatch the SessionConfiguredEvent first and then report any errors.
            // If resuming, include converted initial messages in the payload so UIs can render them immediately.
            let initial_messages = initial_history.get_event_msgs();
            let session_sandbox_policy = session_configuration.sandbox_policy();
            let events = std::iter::once(Event {
                id: INITIAL_SUBMIT_ID.to_owned(),
                msg: EventMsg::SessionConfigured(SessionConfiguredEvent {
                    session_id: conversation_id,
                    forked_from_id,
                    thread_name: session_configuration.thread_name.clone(),
                    model: session_configuration.collaboration_mode.model().to_string(),
                    model_provider_id: config.model_provider_id.clone(),
                    service_tier: session_configuration.service_tier,
                    approval_policy: session_configuration.approval_policy.value(),
                    approvals_reviewer: session_configuration.approvals_reviewer,
                    sandbox_policy: session_sandbox_policy.clone(),
                    permission_profile: Some(session_configuration.permission_profile()),
                    cwd: session_configuration.cwd.clone(),
                    reasoning_effort: session_configuration.collaboration_mode.reasoning_effort(),
                    history_log_id,
                    history_entry_count,
                    initial_messages,
                    network_proxy: session_network_proxy.filter(|_| {
                        Self::managed_network_proxy_active_for_sandbox_policy(
                            &session_sandbox_policy,
                        )
                    }),
                    rollout_path,
                }),
            })
            .chain(post_session_configured_events.into_iter());
            for event in events {
                sess.send_event_raw(event).await;
            }

            // Start the watcher after SessionConfigured so it cannot emit earlier events.
            sess.start_skills_watcher_listener();
            let mut required_mcp_servers: Vec<String> = mcp_servers
                .iter()
                .filter(|(_, server)| server.enabled && server.required)
                .map(|(name, _)| name.clone())
                .collect();
            required_mcp_servers.sort();
            let enabled_mcp_server_count = mcp_servers.values().filter(|server| server.enabled).count();
            let required_mcp_server_count = required_mcp_servers.len();
            let tool_plugin_provenance = mcp_manager.tool_plugin_provenance(config.as_ref()).await;
            {
                let mut cancel_guard = sess.services.mcp_startup_cancellation_token.lock().await;
                cancel_guard.cancel();
                *cancel_guard = CancellationToken::new();
            }
            let (mcp_connection_manager, cancel_token) = McpConnectionManager::new(
                &mcp_servers,
                config.mcp_oauth_credentials_store_mode,
                auth_statuses.clone(),
                &session_configuration.approval_policy,
                INITIAL_SUBMIT_ID.to_owned(),
                tx_event.clone(),
                session_configuration.permission_profile(),
                McpRuntimeEnvironment::new(
                    sess.services
                        .environment_manager
                        .default_environment()
                        .unwrap_or_else(|| sess.services.environment_manager.local_environment()),
                    session_configuration.cwd.to_path_buf(),
                ),
                config.codex_home.to_path_buf(),
                codex_apps_tools_cache_key(auth),
                tool_plugin_provenance,
                auth,
            )
            .instrument(info_span!(
                "session_init.mcp_manager_init",
                otel.name = "session_init.mcp_manager_init",
                session_init.enabled_mcp_server_count = enabled_mcp_server_count,
                session_init.required_mcp_server_count = required_mcp_server_count,
            ))
            .await;
            {
                let mut manager_guard = sess.services.mcp_connection_manager.write().await;
                *manager_guard = mcp_connection_manager;
            }
            {
                let mut cancel_guard = sess.services.mcp_startup_cancellation_token.lock().await;
                if cancel_guard.is_cancelled() {
                    cancel_token.cancel();
                }
                *cancel_guard = cancel_token;
            }
            if !required_mcp_servers.is_empty() {
                let failures = sess
                    .services
                    .mcp_connection_manager
                    .read()
                    .await
                    .required_startup_failures(&required_mcp_servers)
                    .instrument(info_span!(
                        "session_init.required_mcp_wait",
                        otel.name = "session_init.required_mcp_wait",
                        session_init.required_mcp_server_count = required_mcp_server_count,
                    ))
                    .await;
                if !failures.is_empty() {
                    let details = failures
                        .iter()
                        .map(|failure| format!("{}: {}", failure.server, failure.error))
                        .collect::<Vec<_>>()
                        .join("; ");
                    anyhow::bail!("required MCP servers failed to initialize: {details}");
                }
            }
            sess.schedule_startup_prewarm(session_configuration.base_instructions.clone())
                .await;
            let session_start_source = match &initial_history {
                InitialHistory::Resumed(_) => codex_hooks::SessionStartSource::Resume,
                InitialHistory::New | InitialHistory::Forked(_) => {
                    codex_hooks::SessionStartSource::Startup
                }
                InitialHistory::Cleared => codex_hooks::SessionStartSource::Clear,
            };

            // record_initial_history can emit events. We record only after the SessionConfiguredEvent is emitted.
            sess.record_initial_history(initial_history).await;
            {
                let mut state = sess.state.lock().await;
                state.set_pending_session_start_source(Some(session_start_source));
            }

            memories::start_memories_startup_task(
                &sess,
                Arc::clone(&config),
                &session_configuration.session_source,
            );

            Ok(sess)
        }
        .await;
        match session_result {
            Ok(sess) => {
                live_thread_init.commit();
                Ok(sess)
            }
            Err(err) => {
                live_thread_init.discard().await;
                Err(err)
            }
        }
    }
}
