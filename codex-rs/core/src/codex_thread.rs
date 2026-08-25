use crate::agent::AgentStatus;
use crate::config::ConstraintResult;
use crate::context::ContextualUserFragment;
use crate::elicitation::ElicitationRegistration;
use crate::session::SessionIo;
use crate::session::SessionSettingsUpdate;
use crate::session::new_submission_id;
use crate::session::session::Session;
use codex_diagnostics::Gauge;
use codex_diagnostics::GaugeGuard;
use codex_exec_server::SelectedCapabilityRootsStatus;
use codex_extension_api::ConversationHistorySnapshot;
use codex_extension_api::ThreadIdleCause;
use codex_features::Feature;
use codex_history::RolloutItem;
use codex_otel::SessionTelemetry;
use codex_otel::current_span_w3c_trace_context;
use codex_protocol::ThreadId;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::config_types::CollaborationMode;
use codex_protocol::config_types::Personality;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result as CodexResult;
use codex_protocol::mcp::CallToolResult;
use codex_protocol::mcp::ClientMcpExtensions;
use codex_protocol::models::ActivePermissionProfile;
use codex_protocol::models::PermissionProfile;
use codex_protocol::models::ResponseItem;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EnvironmentConfig;
use codex_protocol::protocol::EnvironmentConfigState;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::MultiAgentVersion;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::protocol::SessionConfiguredEvent;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::Submission;
use codex_protocol::protocol::ThreadHistoryMode;
use codex_protocol::protocol::ThreadMemoryMode;
use codex_protocol::protocol::ThreadSettingsSnapshot;
use codex_protocol::protocol::ThreadSource;
use codex_protocol::protocol::TokenUsageInfo;
use codex_protocol::protocol::TurnEnvironmentSelection;
use codex_protocol::protocol::TurnEnvironmentSelections;
use codex_protocol::protocol::W3cTraceContext;
use codex_protocol::turn_input::RecoverTurnRequest;
use codex_protocol::turn_input::StartIfIdleSubmission;
use codex_protocol::turn_input::SteerSubmission;
use codex_protocol::turn_input::SuspendTurnOutcome;
use codex_protocol::turn_input::TurnInputMode;
use codex_protocol::turn_input::TurnInputRequest;
use codex_protocol::turn_input::TurnInputSubmission;
use codex_thread_store::ExtraConfig; // copybara:strip-for-public
use codex_thread_store::PersistContext;
use codex_thread_store::StoredThread;
use codex_thread_store::StoredThreadHistory;
use codex_thread_store::ThreadMetadataPatch;
use codex_thread_store::ThreadStoreError;
use codex_thread_store::ThreadStoreResult;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::LegacyAppPathString;
use codex_utils_path_uri::PathUri;
use rmcp::model::ReadResourceRequestParams;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::oneshot;
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

use codex_rollout::state_db::StateDbHandle;

static LIVE_THREADS: Gauge = Gauge::new("core.threads.live");

#[derive(Clone, Debug)]
pub struct ThreadConfigSnapshot {
    pub model: String,
    pub model_provider_id: String,
    pub service_tier: Option<String>,
    pub approval_policy: AskForApproval,
    pub approvals_reviewer: ApprovalsReviewer,
    pub permission_profile: PermissionProfile,
    pub active_permission_profile: Option<ActivePermissionProfile>,
    pub environments: TurnEnvironmentSelections,
    pub workspace_roots: Vec<AbsolutePathBuf>,
    pub profile_workspace_roots: Vec<AbsolutePathBuf>,
    pub ephemeral: bool,
    pub extra_config: Option<ExtraConfig>, // copybara:strip-for-public
    pub reasoning_effort: Option<ReasoningEffort>,
    pub reasoning_summary: Option<ReasoningSummary>,
    pub personality: Option<Personality>,
    pub collaboration_mode: CollaborationMode,
    pub session_source: SessionSource,
    pub history_mode: ThreadHistoryMode,
    pub forked_from_thread_id: Option<ThreadId>,
    pub parent_thread_id: Option<ThreadId>,
    pub thread_source: Option<ThreadSource>,
    pub originator: String,
}

impl ThreadConfigSnapshot {
    pub fn cwd(&self) -> &AbsolutePathBuf {
        &self.environments.legacy_fallback_cwd
    }

    pub fn environment_selections(&self) -> &[TurnEnvironmentSelection] {
        &self.environments.environments
    }

    /// Whether the primary environment has resolved configuration, if one is selected.
    pub fn is_primary_environment_configured(&self) -> bool {
        self.environment_selections()
            .first()
            .is_none_or(|selection| {
                matches!(
                    selection.config,
                    EnvironmentConfigState::FromThread | EnvironmentConfigState::Ready(_)
                )
            })
    }

    pub fn sandbox_policy(&self) -> SandboxPolicy {
        codex_sandboxing::compatibility_sandbox_policy_for_permission_profile(
            &self.permission_profile,
            self.cwd().as_path(),
        )
    }
}

/// Thread settings overrides that app-server validates before starting a turn.
#[derive(Clone, Default)]
pub struct CodexThreadSettingsOverrides {
    pub environments: Option<TurnEnvironmentSelections>,
    pub profile_workspace_roots: Option<Vec<AbsolutePathBuf>>,
    pub approval_policy: Option<AskForApproval>,
    pub approvals_reviewer: Option<ApprovalsReviewer>,
    pub sandbox_policy: Option<SandboxPolicy>,
    pub permission_profile: Option<PermissionProfile>,
    pub active_permission_profile: Option<ActivePermissionProfile>,
    pub windows_sandbox_level: Option<WindowsSandboxLevel>,
    pub model: Option<String>,
    pub effort: Option<Option<ReasoningEffort>>,
    pub summary: Option<ReasoningSummary>,
    pub service_tier: Option<Option<String>>,
    pub collaboration_mode: Option<CollaborationMode>,
    pub personality: Option<Personality>,
}

/// One root conversation message exposed only to a worker's Guardian reviewers.
#[derive(Debug, Eq, PartialEq)]
pub enum GuardianRootMessage {
    /// Genuine root-user input that can establish or revoke authorization.
    User(String),
    /// Root assistant final output that provides untrusted conversational context.
    Assistant(String),
    /// Bounded, already role-labeled genuine user answers and their assistant questions.
    UserInput(String),
}

impl GuardianRootMessage {
    /// Renders every line with its original role so message content cannot impersonate another role.
    pub fn render(self) -> String {
        let (role, text) = match self {
            Self::User(text) => ("user", text),
            Self::Assistant(text) => ("assistant", text),
            Self::UserInput(fragment) => return fragment,
        };
        text.lines()
            .map(|line| format!("{role}: {line}\n"))
            .collect()
    }
}

/// Authorization state that changes on history rewrites or genuine user input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GuardianAuthorizationVersion {
    /// Conversation-history rewrite generation.
    pub history_version: u64,
    /// Number of genuine user messages in the conversation snapshot.
    pub user_message_count: usize,
    /// Number of successful, host-produced answers to genuine user-input requests.
    pub user_input_response_count: usize,
}

impl GuardianAuthorizationVersion {
    /// Captures history replacement and genuine user input from the same snapshot.
    pub fn from_history(history: &dyn ConversationHistorySnapshot) -> Self {
        Self {
            history_version: history.history_version(),
            user_message_count: history
                .items()
                .filter(|item| item.is_user_message())
                .count(),
            user_input_response_count: 0,
        }
    }
}

/// Bounded root conversation and authorization state from one history snapshot.
#[derive(Debug, Eq, PartialEq)]
pub struct GuardianRootSnapshot {
    pub authorization_version: GuardianAuthorizationVersion,
    pub messages: Vec<GuardianRootMessage>,
}

pub struct CodexThread {
    pub(crate) session: Arc<Session>,
    pub(crate) io: SessionIo,
    pub(crate) session_source: SessionSource,
    session_configured: SessionConfiguredEvent,
    rollout_path: Option<PathBuf>,
    out_of_band_elicitations: Mutex<OutOfBandElicitations>,
    _diagnostics_guard: GaugeGuard,
}

#[derive(Default)]
struct OutOfBandElicitations {
    count: i64,
    registration: Option<ElicitationRegistration>,
}

#[derive(Debug, Eq, PartialEq)]
pub struct BackgroundTerminalInfo {
    pub item_id: String,
    pub process_id: String,
    pub command: String,
    pub cwd: PathUri,
}

/// Conduit for the bidirectional stream of messages that compose a thread
/// (formerly called a conversation) in Codex.
impl CodexThread {
    pub(crate) fn new(
        session: Arc<Session>,
        io: SessionIo,
        session_configured: SessionConfiguredEvent,
        rollout_path: Option<PathBuf>,
        session_source: SessionSource,
    ) -> Self {
        Self {
            session,
            io,
            session_source,
            session_configured,
            rollout_path,
            out_of_band_elicitations: Mutex::new(OutOfBandElicitations::default()),
            _diagnostics_guard: LIVE_THREADS.track(),
        }
    }

    pub async fn submit(&self, op: Op) -> CodexResult<String> {
        self.io.submit(op).await
    }

    /// Returns the session telemetry handle for thread-scoped production instrumentation.
    pub fn session_telemetry(&self) -> SessionTelemetry {
        self.session.services.session_telemetry.clone()
    }

    /// Returns extension-owned data attached to this thread runtime.
    pub fn thread_extension_data(&self) -> &codex_extension_api::ExtensionData {
        &self.session.services.thread_extension_data
    }

    pub async fn shutdown_and_wait(&self) -> CodexResult<()> {
        self.io.shutdown_and_wait().await
    }

    /// Wait until the underlying session loop has terminated.
    pub async fn wait_until_terminated(&self) {
        self.io.session_loop_termination.clone().await;
    }

    pub(crate) async fn emit_thread_ready_lifecycle(&self) {
        let config = self.config().await;
        for contributor in self
            .session
            .services
            .extensions
            .thread_lifecycle_contributors()
        {
            contributor
                .on_thread_ready(codex_extension_api::ThreadReadyInput {
                    config: config.as_ref(),
                    session_source: &self.session_source,
                    session_store: &self.session.services.session_extension_data,
                    thread_store: &self.session.services.thread_extension_data,
                })
                .await;
        }
    }

    pub(crate) async fn emit_thread_resume_lifecycle(&self) {
        for contributor in self
            .session
            .services
            .extensions
            .thread_lifecycle_contributors()
        {
            contributor
                .on_thread_resume(codex_extension_api::ThreadResumeInput {
                    session_store: &self.session.services.session_extension_data,
                    thread_store: &self.session.services.thread_extension_data,
                })
                .await;
        }
    }

    pub async fn emit_thread_idle_lifecycle_if_idle(&self, cause: ThreadIdleCause) {
        self.session.emit_thread_idle_lifecycle_if_idle(cause).await;
    }

    #[doc(hidden)]
    pub async fn ensure_rollout_materialized(&self) {
        self.session
            .ensure_rollout_materialized(PersistContext::Standard)
            .await;
    }

    #[doc(hidden)]
    pub async fn flush_rollout(&self) -> std::io::Result<()> {
        self.session.flush_rollout().await
    }

    pub async fn submit_with_trace(
        &self,
        op: Op,
        trace: Option<W3cTraceContext>,
    ) -> CodexResult<String> {
        self.io
            .submit_with_trace(
                op, trace, /*parent_turn_id*/ None, /*root_turn_id*/ None,
            )
            .await
    }

    /// Submits turn input without requiring the caller to inspect thread state.
    ///
    /// The result describes whether Core started a turn, steered an active
    /// turn, or declined it without recording or enqueueing the input. Only
    /// user input is accepted.
    pub async fn start_or_steer_turn(
        &self,
        request: TurnInputRequest,
    ) -> CodexResult<TurnInputSubmission> {
        self.submit_turn_input_with_mode(request, TurnInputMode::StartOrSteer)
            .await
    }

    /// Starts a regular turn only when the thread is idle.
    ///
    /// Core declines the input without recording or enqueueing it when idle
    /// work cannot start.
    pub async fn start_turn_if_idle(
        &self,
        request: TurnInputRequest,
    ) -> CodexResult<StartIfIdleSubmission> {
        match self
            .submit_turn_input_with_mode(request, TurnInputMode::StartIfIdle)
            .await?
        {
            TurnInputSubmission::Started { turn_id } => {
                Ok(StartIfIdleSubmission::Started { turn_id })
            }
            TurnInputSubmission::NotSubmitted { reason } => {
                Ok(StartIfIdleSubmission::NotSubmitted { reason })
            }
            TurnInputSubmission::Steered { .. } => {
                unreachable!("start-if-idle submission cannot steer")
            }
        }
    }

    /// Resumes an interrupted regular turn only when the thread is idle.
    ///
    /// Recovery starts no new user input and preserves the turn ID that was
    /// already recorded for the interrupted turn.
    pub async fn recover_turn_if_idle(
        &self,
        request: RecoverTurnRequest,
    ) -> CodexResult<StartIfIdleSubmission> {
        self.session
            .services
            .agent_control
            .ensure_execution_capacity_for_turn_start(self)
            .await?;
        let RecoverTurnRequest {
            turn_id,
            thread_settings,
            trace,
        } = request;
        match self
            .io
            .submit_recover_turn(thread_settings, trace, turn_id)
            .await?
        {
            TurnInputSubmission::Started { turn_id } => {
                Ok(StartIfIdleSubmission::Started { turn_id })
            }
            TurnInputSubmission::NotSubmitted { reason } => {
                Ok(StartIfIdleSubmission::NotSubmitted { reason })
            }
            TurnInputSubmission::Steered { .. } => {
                unreachable!("recovered turn submission cannot steer")
            }
        }
    }

    /// Stops the active unfinished root turn without recording TurnAborted or
    /// TurnComplete, so another worker can recover its original turn ID.
    ///
    /// Suspension is refused while a currently loaded descendant exists. Past
    /// descendants do not prevent recovery, and concurrent descendant admission
    /// is not sealed. Queued user input and outstanding approval, elicitation,
    /// or server-request waiters remain best effort and may be discarded.
    ///
    /// The session processes an accepted request even if its caller disconnects.
    /// Callers must not transfer ownership until suspension succeeds, which
    /// requires stopping execution, flushing history, and closing its writer.
    pub async fn suspend_turn_and_shutdown(&self) -> CodexResult<SuspendTurnOutcome> {
        if self.session_source.is_non_root_agent() {
            return Err(CodexErr::UnsupportedOperation(
                "turn suspension requires the owning root thread".to_string(),
            ));
        }

        // The session owns accepted suspension, so dropping this caller cannot interrupt
        // cancellation, persistence, or writer shutdown halfway through a handoff.
        let (reply, result) = oneshot::channel();
        self.io
            .tx_sub
            .send(Submission {
                id: new_submission_id(),
                op: Op::SuspendTurnAndShutdown { reply },
                trace: current_span_w3c_trace_context(),
                parent_turn_id: None,
                root_turn_id: None,
            })
            .await
            .map_err(|_| CodexErr::Fatal("thread session has stopped".to_string()))?;
        let outcome = result
            .await
            .map_err(|_| CodexErr::Fatal("thread suspension reply was lost".to_string()))??;
        if matches!(&outcome, SuspendTurnOutcome::Suspended { .. }) {
            self.io.session_loop_termination.clone().await;
        }
        Ok(outcome)
    }

    /// Steers only if `expected_turn_id` is still the active regular turn.
    pub async fn steer_turn(
        &self,
        request: TurnInputRequest,
        expected_turn_id: String,
    ) -> CodexResult<SteerSubmission> {
        match self
            .submit_turn_input_with_mode(request, TurnInputMode::Steer { expected_turn_id })
            .await?
        {
            TurnInputSubmission::Steered { turn_id } => Ok(SteerSubmission::Steered { turn_id }),
            TurnInputSubmission::NotSubmitted { reason } => {
                Ok(SteerSubmission::NotSubmitted { reason })
            }
            TurnInputSubmission::Started { .. } => {
                unreachable!("steer-only submission cannot start a turn")
            }
        }
    }

    async fn submit_turn_input_with_mode(
        &self,
        request: TurnInputRequest,
        mode: TurnInputMode,
    ) -> CodexResult<TurnInputSubmission> {
        if !matches!(mode, TurnInputMode::Steer { .. }) {
            self.session
                .services
                .agent_control
                .ensure_execution_capacity_for_turn_start(self)
                .await?;
        }
        self.io.submit_turn_input(request, mode).await
    }

    /// Persist whether this thread is eligible for future memory generation.
    pub async fn set_thread_memory_mode(&self, mode: ThreadMemoryMode) -> anyhow::Result<()> {
        self.session.set_thread_memory_mode(mode).await
    }

    /// Injects model-visible items into the currently active turn.
    ///
    /// This is the thread-level bridge to `Session::inject_if_running` for
    /// callers that only hold a `CodexThread`.
    /// It returns the unchanged items when this thread has no active turn.
    pub async fn inject_if_running(
        &self,
        items: Vec<ResponseItem>,
    ) -> Result<(), Vec<ResponseItem>> {
        self.session.inject_if_running(items).await
    }

    pub async fn set_app_server_client_info(
        &self,
        app_server_client_name: Option<String>,
        app_server_client_version: Option<String>,
        mcp_elicitations_auto_deny: bool,
    ) -> ConstraintResult<()> {
        self.session
            .set_app_server_client_info(
                app_server_client_name,
                app_server_client_version,
                mcp_elicitations_auto_deny,
            )
            .await
    }

    /// Preview persistent thread settings overrides without committing them.
    pub async fn preview_thread_settings_overrides(
        &self,
        overrides: CodexThreadSettingsOverrides,
    ) -> ConstraintResult<ThreadConfigSnapshot> {
        let updates = self.thread_settings_update(overrides).await;
        self.session.preview_settings(&updates).await
    }

    /// Restores thread-owned mutable settings captured from another loaded runtime.
    ///
    /// Runtime replacement uses this after resume so clients keep their current thread settings
    /// rather than reverting to the original layer-backed config.
    pub async fn restore_thread_settings(
        &self,
        settings: CodexThreadSettingsOverrides,
    ) -> ConstraintResult<()> {
        let updates = self.thread_settings_update(settings).await;
        self.session.update_settings(updates).await
    }

    async fn thread_settings_update(
        &self,
        overrides: CodexThreadSettingsOverrides,
    ) -> SessionSettingsUpdate {
        let CodexThreadSettingsOverrides {
            environments,
            profile_workspace_roots,
            approval_policy,
            approvals_reviewer,
            sandbox_policy,
            permission_profile,
            active_permission_profile,
            windows_sandbox_level,
            model,
            effort,
            summary,
            service_tier,
            collaboration_mode,
            personality,
        } = overrides;
        let collaboration_mode = if let Some(collaboration_mode) = collaboration_mode {
            collaboration_mode
        } else {
            self.session
                .collaboration_mode()
                .await
                .with_updates(model, effort, /*developer_instructions*/ None)
        };

        SessionSettingsUpdate {
            environments,
            profile_workspace_roots,
            approval_policy,
            approvals_reviewer,
            sandbox_policy,
            permission_profile,
            active_permission_profile,
            windows_sandbox_level,
            collaboration_mode: Some(collaboration_mode),
            reasoning_summary: summary,
            service_tier,
            personality,
            ..Default::default()
        }
    }

    pub async fn next_event(&self) -> CodexResult<Event> {
        self.io.next_event().await
    }

    pub async fn agent_status(&self) -> AgentStatus {
        self.io.agent_status().await
    }

    pub async fn list_background_terminals(&self) -> Vec<BackgroundTerminalInfo> {
        self.session.list_background_terminals().await
    }

    pub async fn terminate_background_terminal(&self, process_id: i32) -> bool {
        self.session.terminate_background_terminal(process_id).await
    }

    pub(crate) fn subscribe_status(&self) -> watch::Receiver<AgentStatus> {
        self.io.agent_status.clone()
    }

    /// Returns the complete token usage snapshot currently cached for this thread.
    ///
    /// This accessor is intentionally narrower than direct session access: it lets
    /// app-server lifecycle paths replay restored usage after resume or fork without
    /// exposing broader session mutation authority. A caller that only reads
    /// `total_token_usage` would drop last-turn usage and make the v2
    /// `thread/tokenUsage/updated` payload incomplete.
    pub async fn token_usage_info(&self) -> Option<TokenUsageInfo> {
        self.session.token_usage_info().await
    }

    /// Records a context fragment without creating a new user turn boundary.
    pub(crate) async fn inject_fragment_without_turn(&self, fragment: impl ContextualUserFragment) {
        let item = ContextualUserFragment::into(fragment);
        self.session
            .inject_no_new_turn(vec![item], /*current_turn_context*/ None)
            .await;
    }

    /// Record raw Responses API items without starting a new turn.
    pub async fn inject_response_items(&self, items: Vec<ResponseItem>) -> CodexResult<()> {
        self.inject_response_items_for_turn(items).await?;
        self.session.flush_rollout().await?;
        Ok(())
    }

    /// Record raw Responses API items immediately before admitting a user turn.
    ///
    /// The caller must submit the associated user input while retaining its
    /// thread-operation lock. The subsequent turn persistence includes both
    /// these items and the user input, without an independent rollout flush.
    pub async fn inject_response_items_for_turn(
        &self,
        items: Vec<ResponseItem>,
    ) -> CodexResult<()> {
        if items.is_empty() {
            return Err(CodexErr::InvalidRequest(
                "items must not be empty".to_string(),
            ));
        }

        let turn_context = self.session.new_default_turn().await;
        if self.session.reference_context_item().await.is_none() {
            // This history-only API runs without run_turn, so it owns its initial step.
            let step_context = self
                .session
                .capture_step_context(Arc::clone(&turn_context), &CancellationToken::new())
                .await?;
            self.session
                .record_context_updates_and_set_reference_context_item(step_context.as_ref())
                .await?;
        }
        self.session
            .inject_client_response_items(items, turn_context.as_ref())
            .await;
        Ok(())
    }

    pub fn rollout_path(&self) -> Option<PathBuf> {
        self.rollout_path.clone()
    }

    pub fn session_configured(&self) -> SessionConfiguredEvent {
        self.session_configured.clone()
    }

    pub(crate) fn is_running(&self) -> bool {
        !self.io.tx_sub.is_closed()
    }

    pub async fn guardian_trunk_rollout_path(&self) -> Option<PathBuf> {
        self.session
            .guardian_review_session
            .trunk_rollout_path()
            .await
    }

    pub async fn load_history(
        &self,
        include_archived: bool,
    ) -> ThreadStoreResult<StoredThreadHistory> {
        let live_thread = self
            .session
            .live_thread_for_persistence("load history")
            .map_err(|err| ThreadStoreError::Internal {
                message: err.to_string(),
            })?;
        live_thread.load_history(include_archived).await
    }

    pub async fn read_thread(
        &self,
        include_archived: bool,
        include_history: bool,
    ) -> ThreadStoreResult<StoredThread> {
        let live_thread = self
            .session
            .live_thread_for_persistence("read thread")
            .map_err(|err| ThreadStoreError::Internal {
                message: err.to_string(),
            })?;
        live_thread
            .read_thread(include_archived, include_history)
            .await
    }

    pub async fn update_thread_metadata(
        &self,
        patch: ThreadMetadataPatch,
        include_archived: bool,
    ) -> ThreadStoreResult<StoredThread> {
        let live_thread = self
            .session
            .live_thread_for_persistence("update thread metadata")
            .map_err(|err| ThreadStoreError::Internal {
                message: err.to_string(),
            })?;
        live_thread.update_metadata(patch, include_archived).await
    }

    /// Appends rollout items through the live thread so derived metadata stays in sync.
    pub async fn append_rollout_items(&self, items: &[RolloutItem]) -> ThreadStoreResult<()> {
        let live_thread = self
            .session
            .live_thread_for_persistence("append rollout items")
            .map_err(|err| ThreadStoreError::Internal {
                message: err.to_string(),
            })?;
        live_thread.append_items(items).await
    }

    pub fn state_db(&self) -> Option<StateDbHandle> {
        self.session.state_db()
    }

    pub async fn config_snapshot(&self) -> ThreadConfigSnapshot {
        self.session.thread_config_snapshot().await
    }

    /// Returns thread-owned settings suitable for rollout persistence and resume.
    pub async fn thread_settings_snapshot(&self) -> ThreadSettingsSnapshot {
        self.session.thread_settings_snapshot().await
    }

    /// Captures thread-owned settings and environment selections for runtime restoration.
    pub async fn restorable_thread_settings(&self) -> CodexThreadSettingsOverrides {
        self.session.restorable_thread_settings().await
    }

    /// Returns the MCP extensions declared by the client that created this runtime.
    pub fn client_mcp_extensions(&self) -> ClientMcpExtensions {
        self.session.services.client_mcp_extensions.clone()
    }

    /// Returns the files that supplied the thread's loaded model instructions.
    pub async fn instruction_sources(&self) -> Vec<PathUri> {
        self.session.instruction_sources().await
    }

    /// Returns loaded instruction sources rendered as legacy app-server path strings.
    pub async fn legacy_instruction_sources(&self) -> Vec<LegacyAppPathString> {
        self.instruction_sources()
            .await
            .into_iter()
            .map(Into::into)
            .collect()
    }

    pub async fn config(&self) -> Arc<crate::config::Config> {
        self.session.get_config().await
    }

    /// Observes this thread's published MCP connections that match the requested config.
    pub async fn mcp_connection_statuses(
        &self,
        config: &codex_mcp::McpConfig,
    ) -> std::collections::HashMap<String, codex_protocol::mcp::McpServerConnectionStatus> {
        self.session
            .services
            .mcp_runtime
            .connection_statuses(config)
            .await
    }

    /// Resolves MCP configuration and environment bindings from the same config snapshot.
    pub async fn runtime_mcp_config_and_context(
        &self,
        config: &crate::config::Config,
    ) -> (codex_mcp::McpConfig, codex_mcp::McpRuntimeContext) {
        self.session.runtime_mcp_config_and_context(config).await
    }

    /// Captures the exact MCP config and environment bindings for the current thread state.
    pub async fn current_mcp_config_and_runtime_context(
        &self,
    ) -> (Arc<codex_mcp::McpConfig>, codex_mcp::McpRuntimeContext) {
        let config = self.session.get_config().await;
        let (mcp_config, runtime_context) = self.runtime_mcp_config_and_context(&config).await;
        (Arc::new(mcp_config), runtime_context)
    }

    pub fn multi_agent_version(&self) -> Option<MultiAgentVersion> {
        self.session.multi_agent_version()
    }

    /// Returns bounded root conversation evidence and its authorization version atomically.
    pub async fn guardian_root_snapshot(&self) -> Option<GuardianRootSnapshot> {
        self.session
            .services
            .agent_control
            .root_user_authorization(self.session.thread_id)
            .await
    }

    /// Refresh the thread's layer-backed user config state from a caller-supplied
    /// config snapshot. Thread-scoped layers and session-static settings remain
    /// unchanged.
    pub async fn refresh_runtime_config(&self, next_config: crate::config::Config) {
        self.session.refresh_runtime_config(next_config).await;
    }

    /// Refresh MCP configuration and managed requirements without reloading unrelated settings.
    pub async fn refresh_mcp_config(&self, next_config: crate::config::Config) {
        self.session.refresh_mcp_config(next_config).await;
    }

    pub async fn environment_selections(&self) -> Vec<TurnEnvironmentSelection> {
        self.session.services.turn_environments.selections()
    }

    /// Installs resolved environment configuration and capability roots on this thread.
    pub async fn environment_ready(
        &self,
        selection: &TurnEnvironmentSelection,
        config: EnvironmentConfig,
    ) -> CodexResult<()> {
        self.session.environment_ready(selection, config).await
    }

    /// Fails this thread's pending environment without affecting other attached threads.
    pub async fn environment_failed(
        &self,
        selection: &TurnEnvironmentSelection,
        error: String,
    ) -> CodexResult<()> {
        self.session.environment_failed(selection, error).await
    }

    /// Passively inspects the selected capability roots whose environments are ready now.
    pub fn inspect_selected_capability_roots(&self) -> SelectedCapabilityRootsStatus {
        self.session.inspect_selected_capability_roots()
    }

    pub async fn read_mcp_resource(
        &self,
        server: &str,
        params: ReadResourceRequestParams,
    ) -> anyhow::Result<serde_json::Value> {
        self.session.refresh_mcp_if_dirty().await;
        let result = self
            .session
            .services
            .mcp_runtime
            .latest_read_resource(server, params)
            .await?;

        Ok(serde_json::to_value(result)?)
    }

    /// Reads an app resource using the current authority of its originating tool call.
    pub async fn read_mcp_resource_for_call(
        &self,
        call_id: &str,
        uri: &str,
    ) -> anyhow::Result<serde_json::Value> {
        self.session.refresh_mcp_if_dirty().await;
        let result = self
            .session
            .services
            .mcp_runtime
            .read_resource_for_call(self.session.thread_id, call_id, uri)
            .await?;

        Ok(serde_json::to_value(result)?)
    }

    pub async fn start_mcp_event_stream(
        &self,
        name: &str,
        arguments: serde_json::Value,
        meta: Option<serde_json::Value>,
    ) -> anyhow::Result<codex_mcp::McpEventStream> {
        let meta = match meta.as_ref() {
            Some(serde_json::Value::Object(meta)) => Some(meta),
            Some(other) => {
                anyhow::bail!("MCP event request _meta must be a JSON object, got {other}")
            }
            None => None,
        };
        let _ = self.session.services.auth_manager.auth().await;
        self.session.refresh_mcp_if_dirty().await;
        codex_mcp::McpResourceClient::new(Arc::clone(&self.session.services.mcp_runtime))
            .open_event_stream(name, &arguments, meta)
            .await
    }

    pub async fn call_mcp_tool(
        &self,
        server: &str,
        tool: &str,
        arguments: Option<serde_json::Value>,
        meta: Option<serde_json::Value>,
    ) -> anyhow::Result<CallToolResult> {
        self.session.refresh_mcp_if_dirty().await;
        self.session
            .services
            .mcp_runtime
            .latest_call_tool(
                server, tool, /*environment_id*/ None, arguments, meta,
                /*requested_timeout*/ None, /*wait_for_server*/ true,
            )
            .await
    }

    pub fn enabled(&self, feature: Feature) -> bool {
        self.session.enabled(feature)
    }

    pub async fn increment_out_of_band_elicitation_count(&self) -> CodexResult<i64> {
        let mut elicitations = self.out_of_band_elicitations.lock().await;
        let incremented = elicitations.count.checked_add(1).ok_or_else(|| {
            CodexErr::Fatal("out-of-band elicitation count overflowed".to_string())
        })?;
        if elicitations.count == 0 {
            elicitations.registration = Some(self.session.services.elicitations.register());
        }
        elicitations.count = incremented;
        Ok(incremented)
    }

    pub async fn decrement_out_of_band_elicitation_count(&self) -> CodexResult<i64> {
        let mut elicitations = self.out_of_band_elicitations.lock().await;
        if elicitations.count == 0 {
            return Err(CodexErr::InvalidRequest(
                "out-of-band elicitation count is already zero".to_string(),
            ));
        }

        elicitations.count -= 1;
        if elicitations.count == 0 {
            elicitations.registration = None;
        }
        Ok(elicitations.count)
    }
}
