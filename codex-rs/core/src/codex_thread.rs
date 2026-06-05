use crate::agent::AgentStatus;
use crate::config::ConstraintResult;
use crate::goals::ExternalGoalSet;
use crate::goals::GoalRuntimeEvent;
use crate::session::Codex;
use crate::session::SessionSettingsUpdate;
use crate::session::SteerInputError;
use codex_features::Feature;
use codex_otel::SessionTelemetry;
use codex_protocol::ThreadId;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::config_types::CollaborationMode;
use codex_protocol::config_types::Personality;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result as CodexResult;
use codex_protocol::mcp::CallToolResult;
use codex_protocol::models::ActivePermissionProfile;
use codex_protocol::models::ContentItem;
use codex_protocol::models::PermissionProfile;
use codex_protocol::models::ResponseItem;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::protocol::AdditionalContextEntry;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::MultiAgentVersion;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::protocol::SessionConfiguredEvent;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::Submission;
use codex_protocol::protocol::ThreadMemoryMode;
use codex_protocol::protocol::ThreadSource;
use codex_protocol::protocol::TokenUsageInfo;
use codex_protocol::protocol::TurnEnvironmentSelection;
use codex_protocol::protocol::W3cTraceContext;
use codex_protocol::user_input::UserInput;
use codex_thread_store::StoredThread;
use codex_thread_store::StoredThreadHistory;
use codex_thread_store::ThreadMetadataPatch;
use codex_thread_store::ThreadStoreError;
use codex_thread_store::ThreadStoreResult;
use codex_utils_absolute_path::AbsolutePathBuf;
use rmcp::model::ReadResourceRequestParams;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::watch;

use codex_rollout::state_db::StateDbHandle;

#[derive(Clone, Debug)]
pub struct ThreadConfigSnapshot {
    pub model: String,
    pub model_provider_id: String,
    pub service_tier: Option<String>,
    pub approval_policy: AskForApproval,
    pub approvals_reviewer: ApprovalsReviewer,
    pub permission_profile: PermissionProfile,
    pub active_permission_profile: Option<ActivePermissionProfile>,
    pub cwd: AbsolutePathBuf,
    pub workspace_roots: Vec<AbsolutePathBuf>,
    pub profile_workspace_roots: Vec<AbsolutePathBuf>,
    pub ephemeral: bool,
    pub reasoning_effort: Option<ReasoningEffort>,
    pub reasoning_summary: Option<ReasoningSummary>,
    pub personality: Option<Personality>,
    pub collaboration_mode: CollaborationMode,
    pub session_source: SessionSource,
    pub forked_from_thread_id: Option<ThreadId>,
    pub parent_thread_id: Option<ThreadId>,
    pub thread_source: Option<ThreadSource>,
}

/// Explains why `CodexThread::try_start_turn_if_idle` rejected an automatic
/// idle turn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TryStartTurnIfIdleRejectionReason {
    /// User/client-triggered mailbox work is already queued and must take
    /// priority over extension-initiated idle work.
    PendingTriggerTurn,
    /// The thread is in Plan mode, where automatic idle work must not start a
    /// new model turn.
    PlanMode,
    /// Another turn or task is active, or the idle reservation was lost before
    /// the automatic turn could start.
    Busy,
}

/// Rejection returned when an extension asks to start automatic idle work but
/// the thread is not eligible to run it.
#[derive(Debug)]
pub struct TryStartTurnIfIdleError {
    reason: TryStartTurnIfIdleRejectionReason,
    input: Vec<ResponseItem>,
}

impl TryStartTurnIfIdleError {
    pub(crate) fn new(reason: TryStartTurnIfIdleRejectionReason, input: Vec<ResponseItem>) -> Self {
        Self { reason, input }
    }

    /// Returns the stable reason the automatic idle turn was rejected.
    pub fn reason(&self) -> TryStartTurnIfIdleRejectionReason {
        self.reason
    }

    /// Consumes the rejection and returns the original model-visible input
    /// unchanged, so callers can retry, drop, or log it explicitly.
    pub fn into_input(self) -> Vec<ResponseItem> {
        self.input
    }
}

impl ThreadConfigSnapshot {
    pub fn sandbox_policy(&self) -> SandboxPolicy {
        codex_sandboxing::compatibility_sandbox_policy_for_permission_profile(
            &self.permission_profile,
            self.cwd.as_path(),
        )
    }
}

/// Thread settings overrides that app-server validates before starting a turn.
#[derive(Clone, Default)]
pub struct CodexThreadSettingsOverrides {
    pub cwd: Option<PathBuf>,
    pub workspace_roots: Option<Vec<AbsolutePathBuf>>,
    pub profile_workspace_roots: Option<Vec<AbsolutePathBuf>>,
    pub approval_policy: Option<AskForApproval>,
    pub approvals_reviewer: Option<ApprovalsReviewer>,
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

pub struct CodexThread {
    pub(crate) codex: Codex,
    pub(crate) session_source: SessionSource,
    session_configured: SessionConfiguredEvent,
    rollout_path: Option<PathBuf>,
    out_of_band_elicitation_count: Mutex<u64>,
}

/// Conduit for the bidirectional stream of messages that compose a thread
/// (formerly called a conversation) in Codex.
impl CodexThread {
    pub(crate) fn new(
        codex: Codex,
        session_configured: SessionConfiguredEvent,
        rollout_path: Option<PathBuf>,
        session_source: SessionSource,
    ) -> Self {
        Self {
            codex,
            session_source,
            session_configured,
            rollout_path,
            out_of_band_elicitation_count: Mutex::new(0),
        }
    }

    pub async fn submit(&self, op: Op) -> CodexResult<String> {
        self.codex.submit(op).await
    }

    /// Returns the session telemetry handle for thread-scoped production instrumentation.
    pub fn session_telemetry(&self) -> SessionTelemetry {
        self.codex.session.services.session_telemetry.clone()
    }

    pub async fn shutdown_and_wait(&self) -> CodexResult<()> {
        self.codex.shutdown_and_wait().await
    }

    /// Wait until the underlying session loop has terminated.
    pub async fn wait_until_terminated(&self) {
        self.codex.session_loop_termination.clone().await;
    }

    pub(crate) async fn emit_thread_resume_lifecycle(&self) {
        for contributor in self
            .codex
            .session
            .services
            .extensions
            .thread_lifecycle_contributors()
        {
            contributor
                .on_thread_resume(codex_extension_api::ThreadResumeInput {
                    session_store: &self.codex.session.services.session_extension_data,
                    thread_store: &self.codex.session.services.thread_extension_data,
                })
                .await;
        }
    }

    pub async fn apply_goal_resume_runtime_effects(&self) -> anyhow::Result<()> {
        self.codex
            .session
            .goal_runtime_apply(GoalRuntimeEvent::ThreadResumed)
            .await
    }

    pub async fn continue_active_goal_if_idle(&self) -> anyhow::Result<()> {
        self.codex
            .session
            .goal_runtime_apply(GoalRuntimeEvent::MaybeContinueIfIdle)
            .await
    }

    pub async fn prepare_external_goal_mutation(&self) {
        if let Err(err) = self
            .codex
            .session
            .goal_runtime_apply(GoalRuntimeEvent::ExternalMutationStarting)
            .await
        {
            tracing::warn!("failed to prepare external goal mutation: {err}");
        }
    }

    pub async fn apply_external_goal_set(&self, external_set: ExternalGoalSet) {
        if let Err(err) = self
            .codex
            .session
            .goal_runtime_apply(GoalRuntimeEvent::ExternalSet { external_set })
            .await
        {
            tracing::warn!("failed to apply external goal status runtime effects: {err}");
        }
    }

    pub async fn apply_external_goal_clear(&self) {
        if let Err(err) = self
            .codex
            .session
            .goal_runtime_apply(GoalRuntimeEvent::ExternalClear)
            .await
        {
            tracing::warn!("failed to apply external goal clear runtime effects: {err}");
        }
    }

    #[doc(hidden)]
    pub async fn ensure_rollout_materialized(&self) {
        self.codex.session.ensure_rollout_materialized().await;
    }

    #[doc(hidden)]
    pub async fn flush_rollout(&self) -> std::io::Result<()> {
        self.codex.session.flush_rollout().await
    }

    pub async fn submit_with_trace(
        &self,
        op: Op,
        trace: Option<W3cTraceContext>,
    ) -> CodexResult<String> {
        self.codex.submit_with_trace(op, trace).await
    }

    pub async fn submit_user_input_with_client_user_message_id(
        &self,
        op: Op,
        trace: Option<W3cTraceContext>,
        client_user_message_id: Option<String>,
    ) -> CodexResult<String> {
        self.codex
            .submit_user_input_with_client_user_message_id(op, trace, client_user_message_id)
            .await
    }

    /// Persist whether this thread is eligible for future memory generation.
    pub async fn set_thread_memory_mode(&self, mode: ThreadMemoryMode) -> anyhow::Result<()> {
        self.codex.set_thread_memory_mode(mode).await
    }

    pub async fn steer_input(
        &self,
        input: Vec<UserInput>,
        additional_context: BTreeMap<String, AdditionalContextEntry>,
        expected_turn_id: Option<&str>,
        client_user_message_id: Option<String>,
        responsesapi_client_metadata: Option<HashMap<String, String>>,
    ) -> Result<String, SteerInputError> {
        self.codex
            .steer_input(
                input,
                additional_context,
                expected_turn_id,
                client_user_message_id,
                responsesapi_client_metadata,
            )
            .await
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
        self.codex.session.inject_if_running(items).await
    }

    /// Starts an automatic regular turn with model-visible items only when idle
    /// work is allowed for this thread.
    ///
    /// This is the required entry point for extensions that want to launch
    /// model-visible work from `ThreadLifecycleContributor::on_thread_idle`.
    /// The call succeeds only if no user/client-triggered turn is queued, no
    /// task is currently active, and the thread is not in Plan mode. Active
    /// Review tasks are rejected by the active-task check because Review turns
    /// are not steerable.
    ///
    /// On rejection, the returned error includes a stable reason and carries
    /// the original `items` unchanged so the caller can decide whether to drop
    /// them, retry later, or log why no automatic turn was started.
    pub async fn try_start_turn_if_idle(
        &self,
        items: Vec<ResponseItem>,
    ) -> Result<(), TryStartTurnIfIdleError> {
        self.codex.session.try_start_turn_if_idle(items).await
    }

    pub async fn set_app_server_client_info(
        &self,
        app_server_client_name: Option<String>,
        app_server_client_version: Option<String>,
        mcp_elicitations_auto_deny: bool,
    ) -> ConstraintResult<()> {
        self.codex
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
        self.codex.session.preview_settings(&updates).await
    }

    async fn thread_settings_update(
        &self,
        overrides: CodexThreadSettingsOverrides,
    ) -> SessionSettingsUpdate {
        let CodexThreadSettingsOverrides {
            cwd,
            workspace_roots,
            profile_workspace_roots,
            approval_policy,
            approvals_reviewer,
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
            self.codex
                .session
                .collaboration_mode()
                .await
                .with_updates(model, effort, /*developer_instructions*/ None)
        };

        SessionSettingsUpdate {
            cwd,
            workspace_roots,
            profile_workspace_roots,
            approval_policy,
            approvals_reviewer,
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

    /// Use sparingly: this is intended to be removed soon.
    pub async fn submit_with_id(&self, sub: Submission) -> CodexResult<()> {
        self.codex.submit_with_id(sub).await
    }

    pub async fn next_event(&self) -> CodexResult<Event> {
        self.codex.next_event().await
    }

    pub async fn agent_status(&self) -> AgentStatus {
        self.codex.agent_status().await
    }

    pub(crate) fn subscribe_status(&self) -> watch::Receiver<AgentStatus> {
        self.codex.agent_status.clone()
    }

    /// Returns the complete token usage snapshot currently cached for this thread.
    ///
    /// This accessor is intentionally narrower than direct session access: it lets
    /// app-server lifecycle paths replay restored usage after resume or fork without
    /// exposing broader session mutation authority. A caller that only reads
    /// `total_token_usage` would drop last-turn usage and make the v2
    /// `thread/tokenUsage/updated` payload incomplete.
    pub async fn token_usage_info(&self) -> Option<TokenUsageInfo> {
        self.codex.session.token_usage_info().await
    }

    /// Records a user-role session-prefix message without creating a new user turn boundary.
    pub(crate) async fn inject_user_message_without_turn(&self, message: String) {
        let item = ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText { text: message }],
            phase: None,
        };
        self.codex
            .session
            .inject_no_new_turn(vec![item], /*current_turn_context*/ None)
            .await;
    }

    /// Record raw Responses API items without starting a new turn.
    pub async fn inject_response_items(&self, items: Vec<ResponseItem>) -> CodexResult<()> {
        if items.is_empty() {
            return Err(CodexErr::InvalidRequest(
                "items must not be empty".to_string(),
            ));
        }

        let turn_context = self.codex.session.new_default_turn().await;
        if self.codex.session.reference_context_item().await.is_none() {
            self.codex
                .session
                .record_context_updates_and_set_reference_context_item(turn_context.as_ref())
                .await;
        }
        self.codex
            .session
            .inject_no_new_turn(items, Some(turn_context.as_ref()))
            .await;
        self.codex.session.flush_rollout().await?;
        Ok(())
    }

    pub fn rollout_path(&self) -> Option<PathBuf> {
        self.rollout_path.clone()
    }

    pub fn session_configured(&self) -> SessionConfiguredEvent {
        self.session_configured.clone()
    }

    pub(crate) fn is_running(&self) -> bool {
        !self.codex.tx_sub.is_closed()
    }

    pub async fn guardian_trunk_rollout_path(&self) -> Option<PathBuf> {
        self.codex
            .session
            .guardian_review_session
            .trunk_rollout_path()
            .await
    }

    pub async fn load_history(
        &self,
        include_archived: bool,
    ) -> ThreadStoreResult<StoredThreadHistory> {
        let live_thread = self
            .codex
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
            .codex
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
            .codex
            .session
            .live_thread_for_persistence("update thread metadata")
            .map_err(|err| ThreadStoreError::Internal {
                message: err.to_string(),
            })?;
        live_thread.update_metadata(patch, include_archived).await
    }

    pub fn state_db(&self) -> Option<StateDbHandle> {
        self.codex.state_db()
    }

    pub async fn config_snapshot(&self) -> ThreadConfigSnapshot {
        self.codex.thread_config_snapshot().await
    }

    /// Returns the files that supplied the thread's loaded model instructions.
    pub async fn instruction_sources(&self) -> Vec<AbsolutePathBuf> {
        self.codex.instruction_sources().await
    }

    pub async fn config(&self) -> Arc<crate::config::Config> {
        self.codex.session.get_config().await
    }

    pub fn multi_agent_version(&self) -> Option<MultiAgentVersion> {
        self.codex.session.multi_agent_version()
    }

    /// Refresh the thread's layer-backed user config state from a caller-supplied
    /// config snapshot. Thread-scoped layers and session-static settings remain
    /// unchanged.
    pub async fn refresh_runtime_config(&self, next_config: crate::config::Config) {
        self.codex.session.refresh_runtime_config(next_config).await;
    }

    pub async fn environment_selections(&self) -> Vec<TurnEnvironmentSelection> {
        self.codex.thread_environment_selections().await
    }

    pub async fn read_mcp_resource(
        &self,
        server: &str,
        uri: &str,
    ) -> anyhow::Result<serde_json::Value> {
        let result = self
            .codex
            .session
            .read_resource(server, ReadResourceRequestParams::new(uri))
            .await?;

        Ok(serde_json::to_value(result)?)
    }

    pub async fn call_mcp_tool(
        &self,
        server: &str,
        tool: &str,
        arguments: Option<serde_json::Value>,
        meta: Option<serde_json::Value>,
    ) -> anyhow::Result<CallToolResult> {
        self.codex
            .session
            .call_tool(server, tool, arguments, meta)
            .await
    }

    pub fn enabled(&self, feature: Feature) -> bool {
        self.codex.enabled(feature)
    }

    pub async fn increment_out_of_band_elicitation_count(&self) -> CodexResult<u64> {
        let mut guard = self.out_of_band_elicitation_count.lock().await;
        let was_zero = *guard == 0;
        *guard = guard.checked_add(1).ok_or_else(|| {
            CodexErr::Fatal("out-of-band elicitation count overflowed".to_string())
        })?;

        if was_zero {
            self.codex
                .session
                .set_out_of_band_elicitation_pause_state(/*paused*/ true);
        }

        Ok(*guard)
    }

    pub async fn decrement_out_of_band_elicitation_count(&self) -> CodexResult<u64> {
        let mut guard = self.out_of_band_elicitation_count.lock().await;
        if *guard == 0 {
            return Err(CodexErr::InvalidRequest(
                "out-of-band elicitation count is already zero".to_string(),
            ));
        }

        *guard -= 1;
        let now_zero = *guard == 0;
        if now_zero {
            self.codex
                .session
                .set_out_of_band_elicitation_pause_state(/*paused*/ false);
        }

        Ok(*guard)
    }
}
