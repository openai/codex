use std::collections::HashMap;
use std::future::Future;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::anyhow;
use codex_analytics::GuardianReviewSessionKind;
use codex_protocol::config_types::Personality;
use codex_protocol::config_types::ReasoningSummary as ReasoningSummaryConfig;
use codex_protocol::models::DeveloperInstructions;
use codex_protocol::models::ResponseItem;
use codex_protocol::openai_models::ReasoningEffort as ReasoningEffortConfig;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::InitialHistory;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::protocol::SubAgentSource;
use codex_protocol::protocol::TokenUsage;
use serde_json::Value;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
use tracing::warn;

use crate::codex_delegate::run_codex_thread_interactive;
use crate::config::Config;
use crate::config::Constrained;
use crate::config::ManagedFeatures;
use crate::config::NetworkProxySpec;
use crate::config::Permissions;
use crate::rollout::recorder::RolloutRecorder;
use crate::session::Codex;
use crate::session::session::Session;
use crate::session::turn_context::TurnContext;
use codex_config::types::McpServerConfig;
use codex_features::Feature;
use codex_model_provider_info::ModelProviderInfo;
use codex_utils_absolute_path::AbsolutePathBuf;

use super::GUARDIAN_REVIEW_TIMEOUT;
use super::GUARDIAN_REVIEWER_NAME;
use super::GuardianApprovalRequest;
use super::prompt::GuardianPromptMode;
use super::prompt::GuardianTranscriptCursor;
use super::prompt::build_guardian_prompt_items;
use super::prompt::guardian_policy_prompt;
use super::prompt::guardian_policy_prompt_with_config;

const GUARDIAN_INTERRUPT_DRAIN_TIMEOUT: Duration = Duration::from_secs(5);
const GUARDIAN_FOLLOWUP_REVIEW_REMINDER: &str = concat!(
    "Use prior reviews as context, not binding precedent. ",
    "Follow the Workspace Policy. ",
    "If the user explicitly approves a previously rejected action after being informed of the ",
    "concrete risks, set outcome to \"allow\" unless the policy explicitly disallows user ",
    "overwrites in such cases."
);

#[derive(Debug)]
pub(crate) enum GuardianReviewSessionOutcome {
    Completed(anyhow::Result<Option<String>>),
    PromptBuildFailed(anyhow::Error),
    TimedOut,
    Aborted,
}

#[derive(Debug, Clone)]
pub(crate) struct GuardianReviewSessionMetadata {
    pub(crate) guardian_thread_id: String,
    pub(crate) guardian_session_kind: GuardianReviewSessionKind,
    pub(crate) guardian_model: String,
    pub(crate) guardian_reasoning_effort: Option<String>,
    pub(crate) had_prior_review_context: bool,
    pub(crate) reviewed_action_truncated: bool,
    pub(crate) token_usage: Option<TokenUsage>,
}

pub(crate) struct GuardianReviewSessionParams {
    pub(crate) parent_session: Arc<Session>,
    pub(crate) parent_turn: Arc<TurnContext>,
    pub(crate) spawn_config: Config,
    pub(crate) request: GuardianApprovalRequest,
    pub(crate) retry_reason: Option<String>,
    pub(crate) schema: Value,
    pub(crate) model: String,
    pub(crate) reasoning_effort: Option<ReasoningEffortConfig>,
    pub(crate) reasoning_summary: ReasoningSummaryConfig,
    pub(crate) personality: Option<Personality>,
    pub(crate) external_cancel: Option<CancellationToken>,
}

#[derive(Default)]
pub(crate) struct GuardianReviewSessionManager {
    state: Arc<Mutex<GuardianReviewSessionState>>,
}

#[derive(Default)]
struct GuardianReviewSessionState {
    trunk: Option<Arc<GuardianReviewSession>>,
    ephemeral_reviews: Vec<Arc<GuardianReviewSession>>,
}

struct GuardianReviewSession {
    codex: Codex,
    cancel_token: CancellationToken,
    reuse_key: GuardianReviewSessionReuseKey,
    review_lock: Mutex<()>,
    state: Mutex<GuardianReviewState>,
}

struct GuardianReviewState {
    prior_review_count: usize,
    last_reviewed_transcript_cursor: Option<GuardianTranscriptCursor>,
    last_committed_fork_snapshot: Option<GuardianReviewForkSnapshot>,
}

fn had_prior_review_context(prompt_mode: &GuardianPromptMode) -> bool {
    matches!(prompt_mode, GuardianPromptMode::Delta { .. })
}

fn token_usage_delta(start: &TokenUsage, end: &TokenUsage) -> TokenUsage {
    TokenUsage {
        input_tokens: (end.input_tokens - start.input_tokens).max(0),
        cached_input_tokens: (end.cached_input_tokens - start.cached_input_tokens).max(0),
        output_tokens: (end.output_tokens - start.output_tokens).max(0),
        reasoning_output_tokens: (end.reasoning_output_tokens - start.reasoning_output_tokens)
            .max(0),
        total_tokens: (end.total_tokens - start.total_tokens).max(0),
    }
}

async fn effective_guardian_reasoning_effort(
    params: &GuardianReviewSessionParams,
) -> Option<ReasoningEffortConfig> {
    let model_info = params
        .parent_session
        .services
        .models_manager
        .get_model_info(
            params.model.as_str(),
            &params.spawn_config.to_models_manager_config(),
        )
        .await;
    if model_info.supports_reasoning_summaries {
        params
            .reasoning_effort
            .or(model_info.default_reasoning_level)
    } else {
        None
    }
}

struct EphemeralReviewCleanup {
    state: Arc<Mutex<GuardianReviewSessionState>>,
    review_session: Option<Arc<GuardianReviewSession>>,
}

#[derive(Clone)]
struct GuardianReviewForkSnapshot {
    initial_history: InitialHistory,
    prior_review_count: usize,
    last_reviewed_transcript_cursor: Option<GuardianTranscriptCursor>,
}

#[derive(Debug, Clone, PartialEq)]
struct GuardianReviewSessionReuseKey {
    // Only include settings that affect spawned-session behavior so reuse
    // invalidation remains explicit and does not depend on unrelated config
    // bookkeeping.
    model: Option<String>,
    model_provider_id: String,
    model_provider: ModelProviderInfo,
    model_context_window: Option<i64>,
    model_auto_compact_token_limit: Option<i64>,
    model_reasoning_effort: Option<ReasoningEffortConfig>,
    model_reasoning_summary: Option<ReasoningSummaryConfig>,
    permissions: Permissions,
    developer_instructions: Option<String>,
    base_instructions: Option<String>,
    user_instructions: Option<String>,
    compact_prompt: Option<String>,
    cwd: AbsolutePathBuf,
    mcp_servers: Constrained<HashMap<String, McpServerConfig>>,
    codex_linux_sandbox_exe: Option<PathBuf>,
    main_execve_wrapper_exe: Option<PathBuf>,
    js_repl_node_path: Option<PathBuf>,
    js_repl_node_module_dirs: Vec<PathBuf>,
    zsh_path: Option<PathBuf>,
    features: ManagedFeatures,
    include_apply_patch_tool: bool,
    use_experimental_unified_exec_tool: bool,
}

impl GuardianReviewSessionReuseKey {
    fn from_spawn_config(spawn_config: &Config) -> Self {
        Self {
            model: spawn_config.model.clone(),
            model_provider_id: spawn_config.model_provider_id.clone(),
            model_provider: spawn_config.model_provider.clone(),
            model_context_window: spawn_config.model_context_window,
            model_auto_compact_token_limit: spawn_config.model_auto_compact_token_limit,
            model_reasoning_effort: spawn_config.model_reasoning_effort,
            model_reasoning_summary: spawn_config.model_reasoning_summary,
            permissions: spawn_config.permissions.clone(),
            developer_instructions: spawn_config.developer_instructions.clone(),
            base_instructions: spawn_config.base_instructions.clone(),
            user_instructions: spawn_config.user_instructions.clone(),
            compact_prompt: spawn_config.compact_prompt.clone(),
            cwd: spawn_config.cwd.clone(),
            mcp_servers: spawn_config.mcp_servers.clone(),
            codex_linux_sandbox_exe: spawn_config.codex_linux_sandbox_exe.clone(),
            main_execve_wrapper_exe: spawn_config.main_execve_wrapper_exe.clone(),
            js_repl_node_path: spawn_config.js_repl_node_path.clone(),
            js_repl_node_module_dirs: spawn_config.js_repl_node_module_dirs.clone(),
            zsh_path: spawn_config.zsh_path.clone(),
            features: spawn_config.features.clone(),
            include_apply_patch_tool: spawn_config.include_apply_patch_tool,
            use_experimental_unified_exec_tool: spawn_config.use_experimental_unified_exec_tool,
        }
    }
}

impl GuardianReviewSession {
    async fn shutdown(&self) {
        self.cancel_token.cancel();
        let _ = self.codex.shutdown_and_wait().await;
    }

    fn shutdown_in_background(self: &Arc<Self>) {
        let review_session = Arc::clone(self);
        drop(tokio::spawn(async move {
            review_session.shutdown().await;
        }));
    }

    async fn fork_snapshot(&self) -> Option<GuardianReviewForkSnapshot> {
        self.state.lock().await.last_committed_fork_snapshot.clone()
    }

    async fn refresh_last_committed_fork_snapshot(&self) {
        match load_rollout_items_for_fork(&self.codex.session).await {
            Ok(Some(items)) if !items.is_empty() => {
                let mut state = self.state.lock().await;
                let prior_review_count = state.prior_review_count;
                let last_reviewed_transcript_cursor = state.last_reviewed_transcript_cursor;
                state.last_committed_fork_snapshot = Some(GuardianReviewForkSnapshot {
                    initial_history: InitialHistory::Forked(items),
                    prior_review_count,
                    last_reviewed_transcript_cursor,
                });
            }
            Ok(Some(_)) => {}
            Ok(None) => {}
            Err(err) => {
                warn!("failed to refresh guardian trunk rollout snapshot: {err}");
            }
        }
    }
}

impl EphemeralReviewCleanup {
    fn new(
        state: Arc<Mutex<GuardianReviewSessionState>>,
        review_session: Arc<GuardianReviewSession>,
    ) -> Self {
        Self {
            state,
            review_session: Some(review_session),
        }
    }

    fn disarm(&mut self) {
        self.review_session = None;
    }
}

impl Drop for EphemeralReviewCleanup {
    fn drop(&mut self) {
        let Some(review_session) = self.review_session.take() else {
            return;
        };
        let state = Arc::clone(&self.state);
        drop(tokio::spawn(async move {
            let review_session = {
                let mut state = state.lock().await;
                state
                    .ephemeral_reviews
                    .iter()
                    .position(|active_review| Arc::ptr_eq(active_review, &review_session))
                    .map(|index| state.ephemeral_reviews.swap_remove(index))
            };
            if let Some(review_session) = review_session {
                review_session.shutdown().await;
            }
        }));
    }
}

impl GuardianReviewSessionManager {
    pub(crate) async fn shutdown(&self) {
        let (review_session, ephemeral_reviews) = {
            let mut state = self.state.lock().await;
            (
                state.trunk.take(),
                std::mem::take(&mut state.ephemeral_reviews),
            )
        };
        if let Some(review_session) = review_session {
            review_session.shutdown().await;
        }
        for review_session in ephemeral_reviews {
            review_session.shutdown().await;
        }
    }

    pub(crate) async fn run_review(
        &self,
        params: GuardianReviewSessionParams,
    ) -> (
        GuardianReviewSessionOutcome,
        Option<GuardianReviewSessionMetadata>,
    ) {
        let deadline = tokio::time::Instant::now() + GUARDIAN_REVIEW_TIMEOUT;
        let next_reuse_key = GuardianReviewSessionReuseKey::from_spawn_config(&params.spawn_config);
        let mut stale_trunk_to_shutdown = None;
        let mut spawned_trunk = false;
        let trunk_candidate = match run_before_review_deadline(
            deadline,
            params.external_cancel.as_ref(),
            self.state.lock(),
        )
        .await
        {
            Ok(mut state) => {
                if let Some(trunk) = state.trunk.as_ref()
                    && trunk.reuse_key != next_reuse_key
                    && trunk.review_lock.try_lock().is_ok()
                {
                    stale_trunk_to_shutdown = state.trunk.take();
                }

                if state.trunk.is_none() {
                    let spawn_cancel_token = CancellationToken::new();
                    let review_session = match run_before_review_deadline_with_cancel(
                        deadline,
                        params.external_cancel.as_ref(),
                        &spawn_cancel_token,
                        Box::pin(spawn_guardian_review_session(
                            &params,
                            params.spawn_config.clone(),
                            next_reuse_key.clone(),
                            spawn_cancel_token.clone(),
                            /*fork_snapshot*/ None,
                        )),
                    )
                    .await
                    {
                        Ok(Ok(review_session)) => Arc::new(review_session),
                        Ok(Err(err)) => {
                            return (GuardianReviewSessionOutcome::PromptBuildFailed(err), None);
                        }
                        Err(outcome) => return (outcome, None),
                    };
                    state.trunk = Some(Arc::clone(&review_session));
                    spawned_trunk = true;
                }

                state.trunk.as_ref().cloned()
            }
            Err(outcome) => return (outcome, None),
        };

        if let Some(review_session) = stale_trunk_to_shutdown {
            review_session.shutdown_in_background();
        }

        let Some(trunk) = trunk_candidate else {
            return (
                GuardianReviewSessionOutcome::Completed(Err(anyhow!(
                    "guardian review session was not available after spawn"
                ))),
                None,
            );
        };

        if trunk.reuse_key != next_reuse_key {
            return Box::pin(self.run_ephemeral_review(
                params,
                next_reuse_key,
                deadline,
                /*fork_snapshot*/ None,
            ))
            .await;
        }

        let trunk_guard = match trunk.review_lock.try_lock() {
            Ok(trunk_guard) => trunk_guard,
            Err(_) => {
                return Box::pin(self.run_ephemeral_review(
                    params,
                    next_reuse_key,
                    deadline,
                    trunk.fork_snapshot().await,
                ))
                .await;
            }
        };

        let guardian_session_kind = if spawned_trunk {
            GuardianReviewSessionKind::TrunkNew
        } else {
            GuardianReviewSessionKind::TrunkReused
        };
        let (outcome, keep_review_session, metadata) = Box::pin(run_review_on_session(
            trunk.as_ref(),
            &params,
            guardian_session_kind,
            deadline,
        ))
        .await;
        if keep_review_session && matches!(outcome, GuardianReviewSessionOutcome::Completed(_)) {
            trunk.refresh_last_committed_fork_snapshot().await;
        }
        drop(trunk_guard);

        if keep_review_session {
            (outcome, Some(metadata))
        } else {
            if let Some(review_session) = self.remove_trunk_if_current(&trunk).await {
                review_session.shutdown_in_background();
            }
            (outcome, Some(metadata))
        }
    }

    #[cfg(test)]
    pub(crate) async fn cache_for_test(&self, codex: Codex) {
        let reuse_key = GuardianReviewSessionReuseKey::from_spawn_config(
            codex.session.get_config().await.as_ref(),
        );
        self.state.lock().await.trunk = Some(Arc::new(GuardianReviewSession {
            reuse_key,
            codex,
            cancel_token: CancellationToken::new(),
            review_lock: Mutex::new(()),
            state: Mutex::new(GuardianReviewState {
                prior_review_count: 0,
                last_reviewed_transcript_cursor: None,
                last_committed_fork_snapshot: None,
            }),
        }));
    }

    #[cfg(test)]
    pub(crate) async fn register_ephemeral_for_test(&self, codex: Codex) {
        let reuse_key = GuardianReviewSessionReuseKey::from_spawn_config(
            codex.session.get_config().await.as_ref(),
        );
        self.state
            .lock()
            .await
            .ephemeral_reviews
            .push(Arc::new(GuardianReviewSession {
                reuse_key,
                codex,
                cancel_token: CancellationToken::new(),
                review_lock: Mutex::new(()),
                state: Mutex::new(GuardianReviewState {
                    prior_review_count: 0,
                    last_reviewed_transcript_cursor: None,
                    last_committed_fork_snapshot: None,
                }),
            }));
    }

    #[cfg(test)]
    pub(crate) async fn committed_fork_rollout_items_for_test(&self) -> Option<Vec<RolloutItem>> {
        let trunk = self.state.lock().await.trunk.clone()?;
        let state = trunk.state.lock().await;
        let snapshot = state.last_committed_fork_snapshot.as_ref()?;
        match &snapshot.initial_history {
            InitialHistory::Forked(items) => Some(items.clone()),
            InitialHistory::New | InitialHistory::Cleared | InitialHistory::Resumed(_) => None,
        }
    }

    async fn remove_trunk_if_current(
        &self,
        trunk: &Arc<GuardianReviewSession>,
    ) -> Option<Arc<GuardianReviewSession>> {
        let mut state = self.state.lock().await;
        if state
            .trunk
            .as_ref()
            .is_some_and(|current| Arc::ptr_eq(current, trunk))
        {
            state.trunk.take()
        } else {
            None
        }
    }

    async fn register_active_ephemeral(&self, review_session: Arc<GuardianReviewSession>) {
        self.state
            .lock()
            .await
            .ephemeral_reviews
            .push(review_session);
    }

    async fn take_active_ephemeral(
        &self,
        review_session: &Arc<GuardianReviewSession>,
    ) -> Option<Arc<GuardianReviewSession>> {
        let mut state = self.state.lock().await;
        let ephemeral_review_index = state
            .ephemeral_reviews
            .iter()
            .position(|active_review| Arc::ptr_eq(active_review, review_session))?;
        Some(state.ephemeral_reviews.swap_remove(ephemeral_review_index))
    }

    async fn run_ephemeral_review(
        &self,
        params: GuardianReviewSessionParams,
        reuse_key: GuardianReviewSessionReuseKey,
        deadline: tokio::time::Instant,
        fork_snapshot: Option<GuardianReviewForkSnapshot>,
    ) -> (
        GuardianReviewSessionOutcome,
        Option<GuardianReviewSessionMetadata>,
    ) {
        let spawn_cancel_token = CancellationToken::new();
        let mut fork_config = params.spawn_config.clone();
        fork_config.ephemeral = true;
        let review_session = match run_before_review_deadline_with_cancel(
            deadline,
            params.external_cancel.as_ref(),
            &spawn_cancel_token,
            Box::pin(spawn_guardian_review_session(
                &params,
                fork_config,
                reuse_key,
                spawn_cancel_token.clone(),
                fork_snapshot,
            )),
        )
        .await
        {
            Ok(Ok(review_session)) => Arc::new(review_session),
            Ok(Err(err)) => return (GuardianReviewSessionOutcome::PromptBuildFailed(err), None),
            Err(outcome) => return (outcome, None),
        };
        self.register_active_ephemeral(Arc::clone(&review_session))
            .await;
        let mut cleanup =
            EphemeralReviewCleanup::new(Arc::clone(&self.state), Arc::clone(&review_session));

        let (outcome, _, metadata) = Box::pin(run_review_on_session(
            review_session.as_ref(),
            &params,
            GuardianReviewSessionKind::EphemeralForked,
            deadline,
        ))
        .await;
        if let Some(review_session) = self.take_active_ephemeral(&review_session).await {
            cleanup.disarm();
            review_session.shutdown_in_background();
        }
        (outcome, Some(metadata))
    }
}

async fn spawn_guardian_review_session(
    params: &GuardianReviewSessionParams,
    spawn_config: Config,
    reuse_key: GuardianReviewSessionReuseKey,
    cancel_token: CancellationToken,
    fork_snapshot: Option<GuardianReviewForkSnapshot>,
) -> anyhow::Result<GuardianReviewSession> {
    let (initial_history, prior_review_count, initial_transcript_cursor) = match fork_snapshot {
        Some(fork_snapshot) => (
            Some(fork_snapshot.initial_history),
            fork_snapshot.prior_review_count,
            fork_snapshot.last_reviewed_transcript_cursor,
        ),
        None => (None, 0, None),
    };
    let codex = Box::pin(run_codex_thread_interactive(
        spawn_config,
        params.parent_session.services.auth_manager.clone(),
        params.parent_session.services.models_manager.clone(),
        Arc::clone(&params.parent_session),
        Arc::clone(&params.parent_turn),
        cancel_token.clone(),
        SubAgentSource::Other(GUARDIAN_REVIEWER_NAME.to_string()),
        initial_history,
    ))
    .await?;

    Ok(GuardianReviewSession {
        codex,
        cancel_token,
        reuse_key,
        review_lock: Mutex::new(()),
        state: Mutex::new(GuardianReviewState {
            prior_review_count,
            last_reviewed_transcript_cursor: initial_transcript_cursor,
            last_committed_fork_snapshot: None,
        }),
    })
}

async fn run_review_on_session(
    review_session: &GuardianReviewSession,
    params: &GuardianReviewSessionParams,
    guardian_session_kind: GuardianReviewSessionKind,
    deadline: tokio::time::Instant,
) -> (
    GuardianReviewSessionOutcome,
    bool,
    GuardianReviewSessionMetadata,
) {
    let (send_followup_reminder, prompt_mode) = {
        let state = review_session.state.lock().await;

        let send_followup_reminder = state.prior_review_count == 1;
        let prompt_mode = if state.prior_review_count == 0 {
            GuardianPromptMode::Full
        } else if let Some(cursor) = state.last_reviewed_transcript_cursor {
            GuardianPromptMode::Delta { cursor }
        } else {
            GuardianPromptMode::Full
        };

        (send_followup_reminder, prompt_mode)
    };
    let mut guardian_metadata = GuardianReviewSessionMetadata {
        guardian_thread_id: review_session.codex.session.conversation_id.to_string(),
        guardian_session_kind,
        guardian_model: params.model.clone(),
        guardian_reasoning_effort: effective_guardian_reasoning_effort(params)
            .await
            .map(|effort| effort.to_string()),
        had_prior_review_context: had_prior_review_context(&prompt_mode),
        reviewed_action_truncated: false,
        token_usage: None,
    };
    if send_followup_reminder {
        append_guardian_followup_reminder(review_session).await;
    }

    let submit_result = run_before_review_deadline(
        deadline,
        params.external_cancel.as_ref(),
        Box::pin(async {
            params
                .parent_session
                .services
                .network_approval
                .sync_session_approved_hosts_to(
                    &review_session.codex.session.services.network_approval,
                )
                .await;

            let prompt_items = build_guardian_prompt_items(
                params.parent_session.as_ref(),
                params.retry_reason.clone(),
                params.request.clone(),
                prompt_mode,
            )
            .await?;
            let reviewed_action_truncated = prompt_items.reviewed_action_truncated;
            // A fresh Guardian review session may not have token info yet. Treat that
            // as a zero baseline so the first review can still report usage.
            let token_usage_at_review_start = review_session
                .codex
                .session
                .total_token_usage()
                .await
                .unwrap_or_default();

            review_session
                .codex
                .submit(Op::UserTurn {
                    items: prompt_items.items,
                    cwd: params.parent_turn.cwd.to_path_buf(),
                    approval_policy: AskForApproval::Never,
                    approvals_reviewer: None,
                    sandbox_policy: SandboxPolicy::new_read_only_policy(),
                    model: params.model.clone(),
                    effort: params.reasoning_effort,
                    summary: Some(params.reasoning_summary),
                    service_tier: None,
                    final_output_json_schema: Some(params.schema.clone()),
                    collaboration_mode: None,
                    personality: params.personality,
                })
                .await?;

            Ok::<(GuardianTranscriptCursor, bool, TokenUsage), anyhow::Error>((
                prompt_items.transcript_cursor,
                reviewed_action_truncated,
                token_usage_at_review_start,
            ))
        }),
    )
    .await;
    let submit_result = match submit_result {
        Ok(submit_result) => submit_result,
        Err(outcome) => return (outcome, false, guardian_metadata),
    };
    let (transcript_cursor, reviewed_action_truncated, token_usage_at_review_start) =
        match submit_result {
            Ok(submit_result) => submit_result,
            Err(err) => {
                return (
                    GuardianReviewSessionOutcome::PromptBuildFailed(err),
                    false,
                    guardian_metadata,
                );
            }
        };
    guardian_metadata.reviewed_action_truncated = reviewed_action_truncated;

    let outcome =
        wait_for_guardian_review(review_session, deadline, params.external_cancel.as_ref()).await;
    if matches!(outcome.0, GuardianReviewSessionOutcome::Completed(_)) {
        if outcome.2
            && let Some(total_token_usage) = review_session.codex.session.total_token_usage().await
        {
            guardian_metadata.token_usage = Some(token_usage_delta(
                &token_usage_at_review_start,
                &total_token_usage,
            ));
        }
        let mut state = review_session.state.lock().await;
        state.prior_review_count = state.prior_review_count.saturating_add(1);
        state.last_reviewed_transcript_cursor = Some(transcript_cursor);
    }
    (outcome.0, outcome.1, guardian_metadata)
}

async fn append_guardian_followup_reminder(review_session: &GuardianReviewSession) {
    let turn_context = review_session.codex.session.new_default_turn().await;
    let reminder: ResponseItem =
        DeveloperInstructions::new(GUARDIAN_FOLLOWUP_REVIEW_REMINDER).into();
    review_session
        .codex
        .session
        .record_conversation_items(turn_context.as_ref(), std::slice::from_ref(&reminder))
        .await;
}

async fn load_rollout_items_for_fork(
    session: &Session,
) -> anyhow::Result<Option<Vec<RolloutItem>>> {
    session.flush_rollout().await?;
    let Some(rollout_path) = session.current_rollout_path().await else {
        return Ok(None);
    };
    let history = RolloutRecorder::get_rollout_history(rollout_path.as_path()).await?;
    Ok(Some(history.get_rollout_items()))
}

async fn wait_for_guardian_review(
    review_session: &GuardianReviewSession,
    deadline: tokio::time::Instant,
    external_cancel: Option<&CancellationToken>,
) -> (GuardianReviewSessionOutcome, bool, bool) {
    let timeout = tokio::time::sleep_until(deadline);
    tokio::pin!(timeout);
    let mut last_error_message: Option<String> = None;

    loop {
        tokio::select! {
            _ = &mut timeout => {
                let keep_review_session = interrupt_and_drain_turn(&review_session.codex).await.is_ok();
                return (GuardianReviewSessionOutcome::TimedOut, keep_review_session, false);
            }
            _ = async {
                if let Some(cancel_token) = external_cancel {
                    cancel_token.cancelled().await;
                } else {
                    std::future::pending::<()>().await;
                }
            } => {
                let keep_review_session = interrupt_and_drain_turn(&review_session.codex).await.is_ok();
                return (GuardianReviewSessionOutcome::Aborted, keep_review_session, false);
            }
            event = review_session.codex.next_event() => {
                match event {
                    Ok(event) => match event.msg {
                        EventMsg::TurnComplete(turn_complete) => {
                            if turn_complete.last_agent_message.is_none()
                                && let Some(error_message) = last_error_message
                            {
                                return (
                                    GuardianReviewSessionOutcome::Completed(Err(anyhow!(error_message))),
                                    true,
                                    true,
                                );
                            }
                            return (
                                GuardianReviewSessionOutcome::Completed(Ok(turn_complete.last_agent_message)),
                                true,
                                true,
                            );
                        }
                        EventMsg::Error(error) => {
                            last_error_message = Some(error.message);
                        }
                        EventMsg::TurnAborted(_) => {
                            return (GuardianReviewSessionOutcome::Aborted, true, false);
                        }
                        _ => {}
                    },
                    Err(err) => {
                        return (
                            GuardianReviewSessionOutcome::Completed(Err(err.into())),
                            false,
                            false,
                        );
                    }
                }
            }
        }
    }
}

pub(crate) fn build_guardian_review_session_config(
    parent_config: &Config,
    live_network_config: Option<codex_network_proxy::NetworkProxyConfig>,
    active_model: &str,
    reasoning_effort: Option<codex_protocol::openai_models::ReasoningEffort>,
) -> anyhow::Result<Config> {
    let mut guardian_config = parent_config.clone();
    guardian_config.model = Some(active_model.to_string());
    guardian_config.model_reasoning_effort = reasoning_effort;
    guardian_config.developer_instructions = Some(
        parent_config
            .guardian_policy_config
            .as_deref()
            .map(guardian_policy_prompt_with_config)
            .unwrap_or_else(guardian_policy_prompt),
    );
    guardian_config.permissions.approval_policy = Constrained::allow_only(AskForApproval::Never);
    guardian_config.permissions.sandbox_policy =
        Constrained::allow_only(SandboxPolicy::new_read_only_policy());
    if let Some(live_network_config) = live_network_config
        && guardian_config.permissions.network.is_some()
    {
        let network_constraints = guardian_config
            .config_layer_stack
            .requirements()
            .network
            .as_ref()
            .map(|network| network.value.clone());
        guardian_config.permissions.network = Some(NetworkProxySpec::from_config_and_constraints(
            live_network_config,
            network_constraints,
            &SandboxPolicy::new_read_only_policy(),
        )?);
    }
    for feature in [
        Feature::SpawnCsv,
        Feature::Collab,
        Feature::CodexHooks,
        Feature::WebSearchRequest,
        Feature::WebSearchCached,
    ] {
        guardian_config.features.disable(feature).map_err(|err| {
            anyhow::anyhow!(
                "guardian review session could not disable `features.{}`: {err}",
                feature.key()
            )
        })?;
        if guardian_config.features.enabled(feature) {
            anyhow::bail!(
                "guardian review session requires `features.{}` to be disabled",
                feature.key()
            );
        }
    }
    Ok(guardian_config)
}

async fn run_before_review_deadline<T>(
    deadline: tokio::time::Instant,
    external_cancel: Option<&CancellationToken>,
    future: impl Future<Output = T>,
) -> Result<T, GuardianReviewSessionOutcome> {
    tokio::select! {
        _ = tokio::time::sleep_until(deadline) => Err(GuardianReviewSessionOutcome::TimedOut),
        result = future => Ok(result),
        _ = async {
            if let Some(cancel_token) = external_cancel {
                cancel_token.cancelled().await;
            } else {
                std::future::pending::<()>().await;
            }
        } => Err(GuardianReviewSessionOutcome::Aborted),
    }
}

async fn run_before_review_deadline_with_cancel<T>(
    deadline: tokio::time::Instant,
    external_cancel: Option<&CancellationToken>,
    cancel_token: &CancellationToken,
    future: impl Future<Output = T>,
) -> Result<T, GuardianReviewSessionOutcome> {
    let result = run_before_review_deadline(deadline, external_cancel, future).await;
    if result.is_err() {
        cancel_token.cancel();
    }
    result
}

async fn interrupt_and_drain_turn(codex: &Codex) -> anyhow::Result<()> {
    let _ = codex.submit(Op::Interrupt).await;

    tokio::time::timeout(GUARDIAN_INTERRUPT_DRAIN_TIMEOUT, async {
        loop {
            let event = codex.next_event().await?;
            if matches!(
                event.msg,
                EventMsg::TurnAborted(_) | EventMsg::TurnComplete(_)
            ) {
                return Ok::<(), anyhow::Error>(());
            }
        }
    })
    .await
    .map_err(|_| anyhow!("timed out draining guardian review session after interrupt"))??;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn guardian_review_session_config_change_invalidates_cached_session() {
        let parent_config = crate::config::test_config().await;
        let cached_spawn_config = build_guardian_review_session_config(
            &parent_config,
            /*live_network_config*/ None,
            "active-model",
            /*reasoning_effort*/ None,
        )
        .expect("cached guardian config");
        let cached_reuse_key =
            GuardianReviewSessionReuseKey::from_spawn_config(&cached_spawn_config);

        let mut changed_parent_config = parent_config;
        changed_parent_config.model_provider.base_url =
            Some("https://guardian.example.invalid/v1".to_string());
        let next_spawn_config = build_guardian_review_session_config(
            &changed_parent_config,
            /*live_network_config*/ None,
            "active-model",
            /*reasoning_effort*/ None,
        )
        .expect("next guardian config");
        let next_reuse_key = GuardianReviewSessionReuseKey::from_spawn_config(&next_spawn_config);

        assert_ne!(cached_reuse_key, next_reuse_key);
        assert_eq!(
            cached_reuse_key,
            GuardianReviewSessionReuseKey::from_spawn_config(&cached_spawn_config)
        );
    }

    #[tokio::test]
    async fn guardian_review_session_config_disables_hooks() {
        let mut parent_config = crate::config::test_config().await;
        parent_config
            .features
            .enable(Feature::CodexHooks)
            .expect("enable hooks on parent config");

        let guardian_config = build_guardian_review_session_config(
            &parent_config,
            /*live_network_config*/ None,
            "active-model",
            /*reasoning_effort*/ None,
        )
        .expect("guardian config");

        assert!(!guardian_config.features.enabled(Feature::CodexHooks));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn run_before_review_deadline_times_out_before_future_completes() {
        let outcome = run_before_review_deadline(
            tokio::time::Instant::now() + Duration::from_millis(10),
            /*external_cancel*/ None,
            async {
                tokio::time::sleep(Duration::from_millis(50)).await;
            },
        )
        .await;

        assert!(matches!(
            outcome,
            Err(GuardianReviewSessionOutcome::TimedOut)
        ));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn run_before_review_deadline_aborts_when_cancelled() {
        let cancel_token = CancellationToken::new();
        let canceller = cancel_token.clone();
        drop(tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(10)).await;
            canceller.cancel();
        }));

        let outcome = run_before_review_deadline(
            tokio::time::Instant::now() + Duration::from_secs(1),
            Some(&cancel_token),
            std::future::pending::<()>(),
        )
        .await;

        assert!(matches!(
            outcome,
            Err(GuardianReviewSessionOutcome::Aborted)
        ));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn run_before_review_deadline_with_cancel_cancels_token_on_timeout() {
        let cancel_token = CancellationToken::new();

        let outcome = run_before_review_deadline_with_cancel(
            tokio::time::Instant::now() + Duration::from_millis(10),
            /*external_cancel*/ None,
            &cancel_token,
            async {
                tokio::time::sleep(Duration::from_millis(50)).await;
            },
        )
        .await;

        assert!(matches!(
            outcome,
            Err(GuardianReviewSessionOutcome::TimedOut)
        ));
        assert!(cancel_token.is_cancelled());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn run_before_review_deadline_with_cancel_cancels_token_on_abort() {
        let external_cancel = CancellationToken::new();
        let external_canceller = external_cancel.clone();
        let cancel_token = CancellationToken::new();
        drop(tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(10)).await;
            external_canceller.cancel();
        }));

        let outcome = run_before_review_deadline_with_cancel(
            tokio::time::Instant::now() + Duration::from_secs(1),
            Some(&external_cancel),
            &cancel_token,
            std::future::pending::<()>(),
        )
        .await;

        assert!(matches!(
            outcome,
            Err(GuardianReviewSessionOutcome::Aborted)
        ));
        assert!(cancel_token.is_cancelled());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn run_before_review_deadline_with_cancel_preserves_token_on_success() {
        let cancel_token = CancellationToken::new();

        let outcome = run_before_review_deadline_with_cancel(
            tokio::time::Instant::now() + Duration::from_secs(1),
            /*external_cancel*/ None,
            &cancel_token,
            async { 42usize },
        )
        .await;

        assert_eq!(outcome.unwrap(), 42);
        assert!(!cancel_token.is_cancelled());
    }

    #[test]
    fn had_prior_review_context_tracks_prompt_mode() {
        assert!(!had_prior_review_context(&GuardianPromptMode::Full));
        assert!(had_prior_review_context(&GuardianPromptMode::Delta {
            cursor: GuardianTranscriptCursor {
                parent_history_version: 7,
                transcript_entry_count: 42,
            }
        }));
    }

    #[test]
    fn token_usage_delta_never_reports_negative_usage() {
        let start = TokenUsage {
            input_tokens: 10,
            cached_input_tokens: 8,
            output_tokens: 6,
            reasoning_output_tokens: 4,
            total_tokens: 28,
        };
        let end = TokenUsage {
            input_tokens: 15,
            cached_input_tokens: 7,
            output_tokens: 10,
            reasoning_output_tokens: 2,
            total_tokens: 34,
        };

        assert_eq!(
            token_usage_delta(&start, &end),
            TokenUsage {
                input_tokens: 5,
                cached_input_tokens: 0,
                output_tokens: 4,
                reasoning_output_tokens: 0,
                total_tokens: 6,
            }
        );
    }
}
