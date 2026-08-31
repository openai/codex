use std::collections::HashMap;
use std::collections::HashSet;
use std::future::Future;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::anyhow;
use codex_analytics::GuardianReviewAnalyticsResult;
use codex_analytics::GuardianReviewSessionAnalyticsParams;
use codex_analytics::GuardianReviewSessionKind;
use codex_extension_api::Instructions;
use codex_history::InitialHistory;
use codex_history::RolloutItem;
use codex_protocol::ThreadId;
use codex_protocol::config_types::AutoCompactTokenLimitScope;
use codex_protocol::config_types::Personality;
use codex_protocol::config_types::ReasoningSummary as ReasoningSummaryConfig;
use codex_protocol::items::TurnItem;
use codex_protocol::mcp::is_node_repl_backed_server;
use codex_protocol::models::BaseInstructionsProvenance;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ImageDetail;
use codex_protocol::models::PermissionProfile;
use codex_protocol::models::PermissionProfileSnapshot;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::openai_models::InputModality;
use codex_protocol::openai_models::ModelMessages;
use codex_protocol::openai_models::ReasoningEffort as ReasoningEffortConfig;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::CodexErrorInfo;
use codex_protocol::protocol::EnvironmentConfigState;
use codex_protocol::protocol::ErrorEvent;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use codex_protocol::protocol::TokenUsage;
use futures::future::BoxFuture;
use serde_json::Value;
use tokio::sync::Mutex;
use tokio::sync::Semaphore;
use tokio_util::sync::CancellationToken;
use tracing::warn;

use crate::codex_delegate::run_codex_thread_interactive;
use crate::config::Config;
use crate::config::Constrained;
use crate::config::ManagedFeatures;
use crate::config::NetworkProxySpec;
use crate::config::Permissions;
use crate::context::ContextualUserFragment;
use crate::context::GuardianFollowupReviewReminder;
use crate::context::GuardianNodeReplPolicy;
use crate::environment_selection::TurnEnvironmentSnapshot;
use crate::image_preparation::ImagePreparationMode;
use crate::image_preparation::ImageResizeNoticeMode;
use crate::image_preparation::prepare_response_items;
use crate::image_preparation::unified_image_budget_enabled;
use crate::session::GitEnrichmentPolicy;
use crate::session::SessionIo;
use crate::session::session::Session;
use crate::session::turn_context::TurnContext;
use codex_config::types::McpServerConfig;
use codex_features::Feature;
use codex_model_provider_info::ModelProviderInfo;
use codex_protocol::turn_input::TurnInputMode;
use codex_protocol::turn_input::TurnInputRequest;
use codex_protocol::turn_input::TurnInputSubmission;
use codex_protocol::turn_input::TurnStartOptions;
use codex_protocol::user_input::UserInput;
use codex_thread_store::PersistContext;
use codex_tools::normalize_output_image_detail;
use codex_utils_path_uri::PathUri;

use super::ApprovalRequestReasons;
use super::GUARDIAN_REVIEWER_NAME;
use super::GuardianApprovalRequest;
use super::GuardianReviewContext;
#[cfg(test)]
use super::prompt::BUNDLED_GUARDIAN_POLICY;
use super::prompt::BUNDLED_GUARDIAN_POLICY_TEMPLATE;
use super::prompt::GUARDIAN_TRANSCRIPT_START;
use super::prompt::GuardianPromptMode;
use super::prompt::GuardianTranscriptCursor;
use super::prompt::build_guardian_prompt_items_with_parent_turn;
use super::prompt::guardian_policy_prompt_with_config_and_template;
use super::review::guardian_review_session_config;

const GUARDIAN_INTERRUPT_DRAIN_TIMEOUT: Duration = Duration::from_secs(5);
const GUARDIAN_MAX_IMAGE_ITEM_TOKENS: i64 = 10_000;
#[derive(Debug)]
pub(crate) enum GuardianReviewSessionOutcome {
    Completed(anyhow::Result<Option<String>>),
    PromptBuildFailed(anyhow::Error),
    SessionFailed {
        error: anyhow::Error,
        error_info: Option<CodexErrorInfo>,
    },
    TimedOut,
    Aborted,
}

pub(crate) struct GuardianReviewSessionParams {
    pub(crate) parent_session: Arc<Session>,
    pub(crate) parent_context: GuardianReviewContext,
    pub(crate) spawn_config: Config,
    pub(crate) node_repl_policy: GuardianNodeReplPolicy,
    pub(crate) request: GuardianApprovalRequest,
    pub(crate) reasons: ApprovalRequestReasons,
    pub(crate) schema: Value,
    pub(crate) model: String,
    pub(crate) reasoning_effort: Option<ReasoningEffortConfig>,
    pub(crate) guardian_default_review_model_id: String,
    pub(crate) guardian_catalog_contains_auto_review: bool,
    pub(crate) guardian_review_model_overridden: bool,
    pub(crate) guardian_review_model_override: Option<String>,
    pub(crate) reasoning_summary: ReasoningSummaryConfig,
    pub(crate) personality: Option<Personality>,
    pub(crate) external_cancel: Option<CancellationToken>,
    pub(crate) deadline: tokio::time::Instant,
}

#[derive(Default)]
pub(crate) struct GuardianReviewSessionManager {
    state: Arc<Mutex<GuardianReviewSessionState>>,
    cancellation_token: CancellationToken,
}

#[derive(Default)]
struct GuardianReviewSessionState {
    trunk: Option<Arc<GuardianReviewSession>>,
    ephemeral_reviews: Vec<Arc<GuardianReviewSession>>,
}

struct GuardianReviewSession {
    session: Arc<Session>,
    io: SessionIo,
    cancel_token: CancellationToken,
    reuse_key: GuardianReviewSessionReuseKey,
    review_lock: Semaphore,
    state: Mutex<GuardianReviewState>,
}

struct GuardianReviewState {
    prior_review_count: usize,
    last_reviewed_transcript_cursor: Option<GuardianTranscriptCursor>,
    last_admitted_node_repl_response_sequence: u64,
    pending_node_repl_evidence_admission: Option<PendingNodeReplEvidenceAdmission>,
    last_committed_fork_snapshot: Option<GuardianReviewForkSnapshot>,
}

struct PendingNodeReplEvidenceAdmission {
    turn_id: String,
    response_sequence: u64,
}

fn had_prior_review_context(prompt_mode: &GuardianPromptMode) -> bool {
    matches!(prompt_mode, GuardianPromptMode::Delta { .. })
}

fn token_usage_delta(start: &TokenUsage, end: &TokenUsage) -> TokenUsage {
    TokenUsage {
        input_tokens: (end.input_tokens - start.input_tokens).max(0),
        cached_input_tokens: (end.cached_input_tokens - start.cached_input_tokens).max(0),
        cache_write_input_tokens: (end.cache_write_input_tokens - start.cache_write_input_tokens)
            .max(0),
        output_tokens: (end.output_tokens - start.output_tokens).max(0),
        reasoning_output_tokens: (end.reasoning_output_tokens - start.reasoning_output_tokens)
            .max(0),
        total_tokens: (end.total_tokens - start.total_tokens).max(0),
        codex_rollout_budget_units: None,
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
    last_admitted_node_repl_response_sequence: u64,
}

#[derive(Debug, Clone, PartialEq)]
struct GuardianReviewSessionReuseKey {
    // Only include settings that affect spawned-session behavior and parent
    // history rewrites that invalidate existing reviewer context.
    parent_history_version: u64,
    node_repl_auto_review_required: bool,
    node_repl_policy: String,
    model: Option<String>,
    model_provider_id: String,
    model_provider: ModelProviderInfo,
    model_context_window: Option<i64>,
    model_auto_compact_token_limit: Option<i64>,
    model_auto_compact_token_limit_scope: AutoCompactTokenLimitScope,
    model_reasoning_effort: Option<ReasoningEffortConfig>,
    model_reasoning_summary: Option<ReasoningSummaryConfig>,
    permissions: Permissions,
    developer_instructions: Option<String>,
    base_instructions: Option<String>,
    user_instructions: Option<Instructions>,
    compact_prompt: Option<String>,
    cwd: PathUri,
    mcp_servers: Constrained<HashMap<String, McpServerConfig>>,
    codex_linux_sandbox_exe: Option<PathBuf>,
    main_execve_wrapper_exe: Option<PathBuf>,
    zsh_path: Option<PathBuf>,
    features: ManagedFeatures,
    environment_ids: Vec<String>,
}

impl GuardianReviewSessionReuseKey {
    fn from_spawn_config(
        spawn_config: &Config,
        user_instructions: Option<Instructions>,
        parent_history_version: u64,
    ) -> Self {
        Self {
            parent_history_version: if spawn_config
                .features
                .enabled(Feature::GuardianReuseParentCompaction)
            {
                parent_history_version
            } else {
                0
            },
            node_repl_auto_review_required: false,
            node_repl_policy: String::new(),
            model: spawn_config.model.clone(),
            model_provider_id: spawn_config.model_provider_id.clone(),
            model_provider: spawn_config.model_provider.clone(),
            model_context_window: spawn_config.model_context_window,
            model_auto_compact_token_limit: spawn_config.model_auto_compact_token_limit,
            model_auto_compact_token_limit_scope: spawn_config.model_auto_compact_token_limit_scope,
            model_reasoning_effort: spawn_config.model_reasoning_effort.clone(),
            model_reasoning_summary: spawn_config.model_reasoning_summary,
            permissions: spawn_config.permissions.clone(),
            developer_instructions: spawn_config.developer_instructions.clone(),
            base_instructions: spawn_config.base_instructions.clone(),
            user_instructions,
            compact_prompt: spawn_config.compact_prompt.clone(),
            cwd: PathUri::from_abs_path(&spawn_config.cwd),
            mcp_servers: spawn_config.mcp_servers.clone(),
            codex_linux_sandbox_exe: spawn_config.codex_linux_sandbox_exe.clone(),
            main_execve_wrapper_exe: spawn_config.main_execve_wrapper_exe.clone(),
            zsh_path: spawn_config.zsh_path.clone(),
            features: spawn_config.features.clone(),
            environment_ids: Vec::new(),
        }
    }

    fn with_environments(mut self, environments: &TurnEnvironmentSnapshot) -> Self {
        self.environment_ids = environments
            .captured_environments()
            .into_keys()
            .collect::<Vec<_>>();
        self.environment_ids.sort_unstable();
        self
    }

    fn with_node_repl_policy_eligibility(mut self, required: bool) -> Self {
        self.node_repl_auto_review_required = required;
        self
    }

    fn with_node_repl_policy(mut self, policy: &GuardianNodeReplPolicy) -> Self {
        self.node_repl_policy = policy.body();
        self
    }
}

fn encrypted_parent_compaction<'a, I>(items: I) -> Option<ResponseItem>
where
    I: IntoIterator<Item = &'a ResponseItem>,
    I::IntoIter: DoubleEndedIterator,
{
    let item = items.into_iter().rev().find(|item| {
        matches!(
            item,
            ResponseItem::Compaction { .. } | ResponseItem::ContextCompaction { .. }
        )
    })?;

    match item {
        ResponseItem::Compaction {
            id: Some(_),
            encrypted_content,
            ..
        } if !encrypted_content.is_empty() => Some(item.clone()),
        ResponseItem::ContextCompaction {
            id: Some(_),
            encrypted_content: Some(encrypted_content),
            ..
        } if !encrypted_content.is_empty() => Some(item.clone()),
        _ => None,
    }
}

pub(crate) fn prompt_cache_key_override_for_review_session(
    session_source: &SessionSource,
    parent_thread_id: Option<ThreadId>,
) -> Option<String> {
    let SessionSource::SubAgent(SubAgentSource::Other(name)) = session_source else {
        return None;
    };
    if name != GUARDIAN_REVIEWER_NAME {
        return None;
    }
    let parent_thread_id = parent_thread_id?;
    Some(format!("guardian:{parent_thread_id}"))
}

impl GuardianReviewSession {
    async fn shutdown(&self) {
        self.cancel_token.cancel();
        let _ = self.io.shutdown_and_wait().await;
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
        match load_rollout_items_for_fork(&self.session).await {
            Ok(Some(items)) if !items.is_empty() => {
                let mut state = self.state.lock().await;
                let prior_review_count = state.prior_review_count;
                let last_reviewed_transcript_cursor = state.last_reviewed_transcript_cursor;
                let last_admitted_node_repl_response_sequence =
                    state.last_admitted_node_repl_response_sequence;
                state.last_committed_fork_snapshot = Some(GuardianReviewForkSnapshot {
                    initial_history: InitialHistory::Forked(items),
                    prior_review_count,
                    last_reviewed_transcript_cursor,
                    last_admitted_node_repl_response_sequence,
                });
            }
            Ok(Some(_)) => {}
            Ok(None) => {}
            Err(err) => {
                warn!("failed to refresh guardian trunk rollout snapshot: {err}");
            }
        }
    }

    async fn admit_node_repl_evidence(&self, event: &Event) {
        let EventMsg::ItemCompleted(completed) = &event.msg else {
            return;
        };
        let TurnItem::UserMessage(_) = &completed.item else {
            return;
        };

        let mut state = self.state.lock().await;
        let Some(pending) = state.pending_node_repl_evidence_admission.as_ref() else {
            return;
        };
        if completed.thread_id == self.session.thread_id()
            && event.id == pending.turn_id
            && completed.turn_id == pending.turn_id
        {
            state.last_admitted_node_repl_response_sequence = state
                .last_admitted_node_repl_response_sequence
                .max(pending.response_sequence);
            state.pending_node_repl_evidence_admission = None;
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
    pub(crate) fn initialize(
        &self,
        parent_session: Arc<Session>,
        parent_turn: Arc<TurnContext>,
    ) -> BoxFuture<'_, anyhow::Result<()>> {
        // Boxing breaks the Session::new -> Guardian -> Session::new future recursion.
        Box::pin(async move {
            let session_config =
                guardian_review_session_config(&parent_session, &parent_turn).await?;
            let spawn_config = session_config.spawn_config;
            let parent_history = parent_session.clone_history().await;
            let parent_compaction = spawn_config
                .features
                .enabled(Feature::GuardianReuseParentCompaction)
                .then(|| encrypted_parent_compaction(parent_history.raw_items()))
                .flatten();
            let parent_context = GuardianReviewContext::from(parent_turn);
            let reuse_key = GuardianReviewSessionReuseKey::from_spawn_config(
                &spawn_config,
                parent_session.user_instructions().await,
                parent_history.history_version(),
            )
            .with_environments(parent_context.environments())
            .with_node_repl_policy_eligibility(
                parent_context
                    .turn()
                    .model_info()
                    .node_repl_auto_review_required,
            )
            .with_node_repl_policy(&session_config.node_repl_policy);
            let spawn_cancel_token = self.cancellation_token.child_token();
            let spawn_cancel_guard = spawn_cancel_token.clone().drop_guard();
            let review_session = spawn_guardian_review_session(
                &parent_session,
                &parent_context,
                spawn_config,
                reuse_key,
                spawn_cancel_token.clone(),
                parent_compaction,
                /*fork_snapshot*/ None,
            )
            .await?;
            // A first review or shutdown may win while eager initialization is in flight;
            // install only if neither has happened.
            let mut state = self.state.lock().await;
            if !spawn_cancel_token.is_cancelled() && state.trunk.is_none() {
                state.trunk = Some(Arc::new(review_session));
                drop(spawn_cancel_guard.disarm());
            }
            Ok(())
        })
    }

    pub(crate) async fn trunk_rollout_path(&self) -> Option<PathBuf> {
        let trunk = self.state.lock().await.trunk.clone()?;
        trunk
            .session
            .ensure_rollout_materialized(PersistContext::Standard)
            .await;
        match trunk.session.current_rollout_path().await {
            Ok(path) => path,
            Err(err) => {
                warn!("failed to resolve guardian trunk rollout path: {err}");
                None
            }
        }
    }

    pub(crate) async fn shutdown(&self) {
        self.cancellation_token.cancel();
        self.invalidate().await;
    }

    pub(crate) async fn invalidate(&self) {
        let (review_session, ephemeral_reviews) = {
            let mut state = self.state.lock().await;
            (
                state.trunk.take(),
                std::mem::take(&mut state.ephemeral_reviews),
            )
        };
        for review_session in review_session.into_iter().chain(ephemeral_reviews) {
            if self.cancellation_token.is_cancelled() {
                review_session.shutdown().await;
            } else {
                review_session.cancel_token.cancel();
                review_session.shutdown_in_background();
            }
        }
    }

    #[expect(
        clippy::await_holding_invalid_type,
        reason = "review session selection and trunk spawning must stay serialized"
    )]
    pub(super) async fn run_review(
        &self,
        params: GuardianReviewSessionParams,
    ) -> (GuardianReviewSessionOutcome, GuardianReviewAnalyticsResult) {
        let deadline = params.deadline;
        let parent_history = params.parent_session.clone_history().await;
        let parent_compaction = params
            .spawn_config
            .features
            .enabled(Feature::GuardianReuseParentCompaction)
            .then(|| encrypted_parent_compaction(parent_history.raw_items()))
            .flatten();
        let mut next_reuse_key = GuardianReviewSessionReuseKey::from_spawn_config(
            &params.spawn_config,
            params.parent_session.user_instructions().await,
            parent_history.history_version(),
        )
        .with_environments(params.parent_context.environments())
        .with_node_repl_policy_eligibility(
            params
                .parent_context
                .turn()
                .model_info()
                .node_repl_auto_review_required,
        )
        .with_node_repl_policy(&params.node_repl_policy);
        let mut spawned_trunk = false;
        let trunk_candidate = match run_before_review_deadline(
            deadline,
            params.external_cancel.as_ref(),
            self.state.lock(),
        )
        .await
        {
            Ok(mut state) => {
                if parent_compaction.is_none()
                    && let Some(trunk) = state.trunk.as_ref()
                {
                    // Without a decryptable summary, the existing reviewer may
                    // hold the only remaining authorization or restriction.
                    next_reuse_key.parent_history_version = trunk.reuse_key.parent_history_version;
                }
                if let Some(trunk) = state.trunk.as_ref()
                    && trunk.reuse_key != next_reuse_key
                    && trunk.review_lock.try_acquire().is_ok()
                    && let Some(stale_trunk) = state.trunk.take()
                {
                    stale_trunk.shutdown_in_background();
                }

                if state.trunk.is_none() {
                    let spawn_cancel_token = self.cancellation_token.child_token();
                    let review_session = match run_before_review_deadline_with_cancel(
                        deadline,
                        params.external_cancel.as_ref(),
                        &spawn_cancel_token,
                        Box::pin(spawn_guardian_review_session(
                            &params.parent_session,
                            &params.parent_context,
                            params.spawn_config.clone(),
                            next_reuse_key.clone(),
                            spawn_cancel_token.clone(),
                            parent_compaction.clone(),
                            /*fork_snapshot*/ None,
                        )),
                    )
                    .await
                    {
                        Ok(Ok(review_session)) => Arc::new(review_session),
                        Ok(Err(err)) => {
                            return (
                                GuardianReviewSessionOutcome::PromptBuildFailed(err),
                                GuardianReviewAnalyticsResult::without_session(),
                            );
                        }
                        Err(outcome) => {
                            return (outcome, GuardianReviewAnalyticsResult::without_session());
                        }
                    };
                    state.trunk = Some(Arc::clone(&review_session));
                    spawned_trunk = true;
                }

                state.trunk.as_ref().cloned()
            }
            Err(outcome) => {
                return (outcome, GuardianReviewAnalyticsResult::without_session());
            }
        };

        let Some(trunk) = trunk_candidate else {
            return (
                GuardianReviewSessionOutcome::Completed(Err(anyhow!(
                    "guardian review session was not available after spawn"
                ))),
                GuardianReviewAnalyticsResult::without_session(),
            );
        };

        if trunk.reuse_key != next_reuse_key {
            return Box::pin(self.run_ephemeral_review(
                params,
                next_reuse_key,
                deadline,
                parent_compaction,
                /*fork_snapshot*/ None,
            ))
            .await;
        }

        let trunk_guard = match trunk.review_lock.try_acquire() {
            Ok(trunk_guard) => trunk_guard,
            Err(_) => {
                return Box::pin(self.run_ephemeral_review(
                    params,
                    next_reuse_key,
                    deadline,
                    parent_compaction,
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
        let (outcome, keep_review_session, analytics_result) = Box::pin(run_review_on_session(
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
            (outcome, analytics_result)
        } else {
            if let Some(review_session) = self.remove_trunk_if_current(&trunk).await {
                review_session.shutdown_in_background();
            }
            (outcome, analytics_result)
        }
    }

    #[cfg(test)]
    pub(crate) async fn cache_for_test(&self, session: Arc<Session>, io: SessionIo) {
        let reuse_key = GuardianReviewSessionReuseKey::from_spawn_config(
            session.get_config().await.as_ref(),
            session.user_instructions().await,
            session.clone_history().await.history_version(),
        );
        self.state.lock().await.trunk = Some(Arc::new(GuardianReviewSession {
            reuse_key,
            session,
            io,
            cancel_token: CancellationToken::new(),
            review_lock: Semaphore::new(/*permits*/ 1),
            state: Mutex::new(GuardianReviewState {
                prior_review_count: 0,
                last_reviewed_transcript_cursor: None,
                last_admitted_node_repl_response_sequence: 0,
                pending_node_repl_evidence_admission: None,
                last_committed_fork_snapshot: None,
            }),
        }));
    }

    #[cfg(test)]
    pub(crate) async fn register_ephemeral_for_test(&self, session: Arc<Session>, io: SessionIo) {
        let reuse_key = GuardianReviewSessionReuseKey::from_spawn_config(
            session.get_config().await.as_ref(),
            session.user_instructions().await,
            session.clone_history().await.history_version(),
        );
        self.state
            .lock()
            .await
            .ephemeral_reviews
            .push(Arc::new(GuardianReviewSession {
                reuse_key,
                session,
                io,
                cancel_token: CancellationToken::new(),
                review_lock: Semaphore::new(/*permits*/ 1),
                state: Mutex::new(GuardianReviewState {
                    prior_review_count: 0,
                    last_reviewed_transcript_cursor: None,
                    last_admitted_node_repl_response_sequence: 0,
                    pending_node_repl_evidence_admission: None,
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

    #[cfg(test)]
    pub(crate) async fn send_trunk_event_raw_for_test(&self, event: Event) {
        let trunk = self
            .state
            .lock()
            .await
            .trunk
            .clone()
            .expect("guardian trunk should exist");
        trunk.session.send_event_raw(event).await;
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
        parent_compaction: Option<ResponseItem>,
        fork_snapshot: Option<GuardianReviewForkSnapshot>,
    ) -> (GuardianReviewSessionOutcome, GuardianReviewAnalyticsResult) {
        let spawn_cancel_token = self.cancellation_token.child_token();
        let mut fork_config = params.spawn_config.clone();
        fork_config.ephemeral = true;
        let review_session = match run_before_review_deadline_with_cancel(
            deadline,
            params.external_cancel.as_ref(),
            &spawn_cancel_token,
            Box::pin(spawn_guardian_review_session(
                &params.parent_session,
                &params.parent_context,
                fork_config,
                reuse_key,
                spawn_cancel_token.clone(),
                parent_compaction,
                fork_snapshot,
            )),
        )
        .await
        {
            Ok(Ok(review_session)) => Arc::new(review_session),
            Ok(Err(err)) => {
                return (
                    GuardianReviewSessionOutcome::PromptBuildFailed(err),
                    GuardianReviewAnalyticsResult::without_session(),
                );
            }
            Err(outcome) => {
                return (outcome, GuardianReviewAnalyticsResult::without_session());
            }
        };
        self.register_active_ephemeral(Arc::clone(&review_session))
            .await;
        let mut cleanup =
            EphemeralReviewCleanup::new(Arc::clone(&self.state), Arc::clone(&review_session));

        let (outcome, _, analytics_result) = Box::pin(run_review_on_session(
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
        (outcome, analytics_result)
    }
}

async fn spawn_guardian_review_session(
    parent_session: &Arc<Session>,
    parent_context: &GuardianReviewContext,
    spawn_config: Config,
    reuse_key: GuardianReviewSessionReuseKey,
    cancel_token: CancellationToken,
    parent_compaction: Option<ResponseItem>,
    fork_snapshot: Option<GuardianReviewForkSnapshot>,
) -> anyhow::Result<GuardianReviewSession> {
    let (
        initial_history,
        prior_review_count,
        initial_transcript_cursor,
        last_admitted_node_repl_response_sequence,
    ) = match fork_snapshot {
        Some(fork_snapshot) => (
            Some(fork_snapshot.initial_history),
            fork_snapshot.prior_review_count,
            fork_snapshot.last_reviewed_transcript_cursor,
            fork_snapshot.last_admitted_node_repl_response_sequence,
        ),
        None => (
            parent_compaction
                .map(|item| InitialHistory::Forked(vec![RolloutItem::ResponseItem(item.into())])),
            0,
            None,
            0,
        ),
    };
    let (session, io) = Box::pin(run_codex_thread_interactive(
        spawn_config,
        parent_session.services.auth_manager.clone(),
        parent_session.services.models_manager.clone(),
        Arc::clone(parent_session),
        Arc::clone(parent_context.turn()),
        parent_context.environments().clone(),
        cancel_token.clone(),
        SubAgentSource::Other(GUARDIAN_REVIEWER_NAME.to_string()),
        initial_history,
        GitEnrichmentPolicy::Skip,
        codex_sandboxing::WindowsSandboxProxySettingsMode::Preserve,
    ))
    .await?;

    Ok(GuardianReviewSession {
        session,
        io,
        cancel_token,
        reuse_key,
        review_lock: Semaphore::new(/*permits*/ 1),
        state: Mutex::new(GuardianReviewState {
            prior_review_count,
            last_reviewed_transcript_cursor: initial_transcript_cursor,
            last_admitted_node_repl_response_sequence,
            pending_node_repl_evidence_admission: None,
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
    GuardianReviewAnalyticsResult,
) {
    let model_info = params
        .parent_session
        .services
        .models_manager
        .get_model_info(
            params.model.as_str(),
            &params.spawn_config.to_models_manager_config(),
        )
        .await;
    let guardian_reasoning_effort = params
        .reasoning_effort
        .clone()
        .or_else(|| model_info.default_reasoning_level.clone());
    let (prior_review_count, had_prior_context) = {
        let state = review_session.state.lock().await;
        (
            state.prior_review_count,
            state.last_reviewed_transcript_cursor.is_some(),
        )
    };
    let mut analytics_result =
        GuardianReviewAnalyticsResult::from_session(GuardianReviewSessionAnalyticsParams {
            guardian_thread_id: review_session.session.thread_id().to_string(),
            guardian_session_kind,
            guardian_model: params.model.clone(),
            guardian_reasoning_effort: guardian_reasoning_effort.map(|effort| effort.to_string()),
            guardian_default_review_model_id: params.guardian_default_review_model_id.clone(),
            guardian_catalog_contains_auto_review: params.guardian_catalog_contains_auto_review,
            guardian_review_model_overridden: params.guardian_review_model_overridden,
            guardian_review_model_override: params.guardian_review_model_override.clone(),
            guardian_model_provider_id: params.spawn_config.model_provider_id.clone(),
            had_prior_review_context: had_prior_context,
        });
    if prior_review_count > 0 {
        ensure_guardian_followup_reminder(review_session).await;
    }

    match run_before_review_deadline(
        deadline,
        params.external_cancel.as_ref(),
        Box::pin(ensure_guardian_node_repl_policy(review_session, params)),
    )
    .await
    {
        Ok(Ok(())) => {}
        Ok(Err(error)) => {
            return (
                GuardianReviewSessionOutcome::SessionFailed {
                    error,
                    error_info: None,
                },
                false,
                analytics_result,
            );
        }
        Err(outcome) => return (outcome, false, analytics_result),
    }

    if params.spawn_config.features.enabled(Feature::TokenBudget)
        && crate::session::context_window::context_window_token_status_for_model(
            review_session.session.as_ref(),
            &params.spawn_config,
            params.parent_context.turn(),
            &model_info,
        )
        .await
        .token_limit_reached
    {
        let compact_submission = run_before_review_deadline(
            deadline,
            params.external_cancel.as_ref(),
            review_session.io.submit(Op::Compact),
        )
        .await;
        let compact_turn_id = match compact_submission {
            Ok(Ok(turn_id)) => turn_id,
            Ok(Err(error)) => {
                return (
                    GuardianReviewSessionOutcome::SessionFailed {
                        error: error.into(),
                        error_info: None,
                    },
                    false,
                    analytics_result,
                );
            }
            Err(outcome) => return (outcome, false, analytics_result),
        };
        let (outcome, keep_review_session, _) = wait_for_guardian_review(
            review_session,
            &compact_turn_id,
            deadline,
            params.external_cancel.as_ref(),
            &mut analytics_result,
        )
        .await;
        if !matches!(outcome, GuardianReviewSessionOutcome::Completed(Ok(_))) {
            return (outcome, keep_review_session, analytics_result);
        }

        if prior_review_count > 0 {
            ensure_guardian_followup_reminder(review_session).await;
        }
    }

    let reviewer_has_full_transcript = review_session
        .session
        .clone_history()
        .await
        .raw_items()
        .any(|item| {
            matches!(item, ResponseItem::Message { role, content, .. }
            if role == "user" && content.iter().any(|content| {
                matches!(content, ContentItem::InputText { text }
                    if text == GUARDIAN_TRANSCRIPT_START)
            }))
        });
    let (prompt_mode, last_admitted_node_repl_response_sequence) = {
        let mut state = review_session.state.lock().await;
        state.pending_node_repl_evidence_admission = None;
        if !reviewer_has_full_transcript {
            state.last_reviewed_transcript_cursor = None;
            state.last_admitted_node_repl_response_sequence = 0;
        }

        let prompt_mode = state
            .last_reviewed_transcript_cursor
            .map_or(GuardianPromptMode::Full, |cursor| {
                GuardianPromptMode::Delta { cursor }
            });
        (prompt_mode, state.last_admitted_node_repl_response_sequence)
    };
    analytics_result.had_prior_review_context = Some(had_prior_review_context(&prompt_mode));

    let prompt_items = run_before_review_deadline(
        deadline,
        params.external_cancel.as_ref(),
        Box::pin(async {
            params
                .parent_session
                .services
                .network_approval
                .sync_session_approved_hosts_to(&review_session.session.services.network_approval)
                .await;

            let mut prompt_items = build_guardian_prompt_items_with_parent_turn(
                params.parent_session.as_ref(),
                Some(&params.parent_context),
                params.reasons.clone(),
                params.request.clone(),
                prompt_mode,
                last_admitted_node_repl_response_sequence,
            )
            .await?;

            if prompt_items
                .items
                .iter()
                .any(|item| matches!(item, UserInput::Image { .. }))
            {
                let reviewer_history = review_session.session.clone_history().await;
                let reviewer_image_urls = reviewer_history
                    .raw_items()
                    .flat_map(|item| match item {
                        ResponseItem::Message { content, .. } => content.as_slice(),
                        _ => &[],
                    })
                    .filter_map(|item| match item {
                        ContentItem::InputImage { image_url, .. } => Some(image_url.as_str()),
                        _ => None,
                    })
                    .collect::<HashSet<_>>();
                let context_window = model_info.resolved_context_window().map(|supported| {
                    params
                        .spawn_config
                        .model_context_window
                        .unwrap_or(supported)
                        .min(supported)
                        .saturating_mul(model_info.effective_context_window_percent.clamp(0, 100))
                        / 100
                });
                let admit_images = if let Some(context_window) = context_window.filter(|limit| {
                    *limit > 0
                        && !model_info.used_fallback_model_metadata
                        && model_info.input_modalities.contains(&InputModality::Image)
                }) {
                    let features = &params.spawn_config.features;
                    let mode = if unified_image_budget_enabled(features, &model_info) {
                        ImagePreparationMode::UnifiedBudget
                    } else {
                        ImagePreparationMode::DetailBased
                    };
                    prompt_items.items.retain_mut(|item| {
                        let UserInput::Image { detail, .. } = item else {
                            return true;
                        };
                        *detail = match normalize_output_image_detail(&model_info, *detail) {
                            _ if mode == ImagePreparationMode::UnifiedBudget => {
                                Some(ImageDetail::Original)
                            }
                            Some(ImageDetail::Low) => Some(ImageDetail::High),
                            detail => detail,
                        };
                        let mut prepared = vec![ResponseInputItem::from(vec![item.clone()]).into()];
                        prepare_response_items(
                            &mut prepared,
                            mode,
                            ImageResizeNoticeMode::Disabled,
                        );
                        let Some(ResponseItem::Message { content, .. }) = prepared.first() else {
                            return false;
                        };
                        content.iter().any(|item| {
                            matches!(item, ContentItem::InputImage { image_url, .. }
                                if !reviewer_image_urls.contains(image_url.as_str()))
                        })
                    });
                    let prompt: ResponseItem =
                        ResponseInputItem::from(prompt_items.items.clone()).into();
                    let prompt_tokens = crate::context_manager::estimate_item_token_count(&prompt);
                    let base_instructions = review_session.session.get_base_instructions().await;
                    let history_tokens = reviewer_history
                        .estimate_token_count_with_base_instructions(&base_instructions)
                        .unwrap_or(i64::MAX)
                        .max(review_session.session.get_total_token_usage().await);
                    prompt_tokens <= GUARDIAN_MAX_IMAGE_ITEM_TOKENS
                        && prompt_tokens.saturating_add(history_tokens) <= context_window
                } else {
                    false
                };
                if !admit_images {
                    prompt_items
                        .items
                        .retain(|item| !matches!(item, UserInput::Image { .. }));
                }
            }

            Ok::<_, anyhow::Error>(prompt_items)
        }),
    )
    .await;
    let prompt_items = match prompt_items {
        Ok(prompt_items) => prompt_items,
        Err(outcome) => return (outcome, false, analytics_result),
    };
    let prompt_items = match prompt_items {
        Ok(prompt_items) => prompt_items,
        Err(err) => {
            return (
                GuardianReviewSessionOutcome::PromptBuildFailed(err),
                false,
                analytics_result,
            );
        }
    };
    let reviewed_action_truncated = prompt_items.reviewed_action_truncated;
    let transcript_cursor = prompt_items.transcript_cursor;
    let node_repl_evidence_admission = (prompt_items.node_repl_evidence_sequence
        > last_admitted_node_repl_response_sequence)
        .then_some(prompt_items.node_repl_evidence_sequence);
    let token_usage_at_review_start = review_session
        .session
        .total_token_usage()
        .await
        .unwrap_or_default();
    let guardian_permission_snapshot = params
        .spawn_config
        .permissions
        .permission_profile_state()
        .snapshot();
    // Guardian must receive read-only permissions for every inherited environment.
    let parent_turn_environments = params
        .parent_context
        .environments()
        .turn_environments()
        .map(|environment| {
            let mut selection = environment.selection();
            let mut config = environment.config().clone();
            config.permission_profile =
                PermissionProfileSnapshot::legacy(read_only_guardian_permission_profile(
                    config.permission_profile.permission_profile(),
                ));
            selection.config = EnvironmentConfigState::Ready(config);
            selection
        })
        .collect();
    // TODO(anp): Migrate guardian review thread settings to a PathUri fallback cwd so foreign
    // parent environments do not fall back to the host-native config cwd.
    let parent_turn_legacy_fallback_cwd = params
        .parent_context
        .environments()
        .primary()
        .and_then(|environment| environment.cwd().to_abs_path().ok())
        .unwrap_or_else(|| params.parent_context.turn().config.cwd.clone());

    let parent_turn = params.parent_context.turn();
    let submission = review_session.io.submit_turn_input(
        TurnInputRequest::user_input(prompt_items.items)
            .with_thread_settings(codex_protocol::protocol::ThreadSettingsOverrides {
                environments: Some(codex_protocol::protocol::TurnEnvironmentSelections::new(
                    parent_turn_legacy_fallback_cwd,
                    parent_turn_environments,
                )),
                approval_policy: Some(AskForApproval::Never),
                sandbox_policy: None,
                permission_profile: Some(guardian_permission_snapshot.permission_profile().clone()),
                summary: Some(params.reasoning_summary),
                personality: params.personality,
                collaboration_mode: Some(codex_protocol::config_types::CollaborationMode {
                    mode: codex_protocol::config_types::ModeKind::Default,
                    settings: codex_protocol::config_types::Settings {
                        model: params.model.clone(),
                        reasoning_effort: params.reasoning_effort.clone(),
                        developer_instructions: None,
                    },
                }),
                ..Default::default()
            })
            .on_start(TurnStartOptions {
                final_output_json_schema: Some(params.schema.clone()),
                service_tier: None,
                parent_turn_id: Some(parent_turn.sub_id.clone()),
                root_turn_id: parent_turn.turn_metadata_state.root_turn_id(),
                ..Default::default()
            }),
        TurnInputMode::StartIfIdle,
    );
    let submit_result = run_before_review_deadline(
        deadline,
        params.external_cancel.as_ref(),
        Box::pin(submission),
    )
    .await;
    let child_turn_id = match submit_result {
        Ok(Ok(TurnInputSubmission::Started { turn_id })) => turn_id,
        Ok(Ok(submission)) => {
            return (
                GuardianReviewSessionOutcome::SessionFailed {
                    error: anyhow!("guardian review input was not started: {submission:?}"),
                    error_info: None,
                },
                false,
                analytics_result,
            );
        }
        Ok(Err(err)) => {
            return (
                GuardianReviewSessionOutcome::SessionFailed {
                    error: err.into(),
                    error_info: None,
                },
                false,
                analytics_result,
            );
        }
        Err(outcome) => return (outcome, false, analytics_result),
    };
    if let Some(response_sequence) = node_repl_evidence_admission {
        let mut state = review_session.state.lock().await;
        state.pending_node_repl_evidence_admission = Some(PendingNodeReplEvidenceAdmission {
            turn_id: child_turn_id.clone(),
            response_sequence,
        });
    }
    analytics_result.reviewed_action_truncated = reviewed_action_truncated;

    let outcome = wait_for_guardian_review(
        review_session,
        child_turn_id.as_str(),
        deadline,
        params.external_cancel.as_ref(),
        &mut analytics_result,
    )
    .await;
    if matches!(outcome.0, GuardianReviewSessionOutcome::Completed(_)) {
        if outcome.2
            && let Some(total_token_usage) = review_session.session.total_token_usage().await
        {
            analytics_result.token_usage = Some(token_usage_delta(
                &token_usage_at_review_start,
                &total_token_usage,
            ));
        }
        let mut state = review_session.state.lock().await;
        state.prior_review_count = state.prior_review_count.saturating_add(1);
        state.last_reviewed_transcript_cursor = Some(transcript_cursor);
    }
    (outcome.0, outcome.1, analytics_result)
}

async fn ensure_guardian_followup_reminder(review_session: &GuardianReviewSession) {
    let followup_reminder = GuardianFollowupReviewReminder.body();
    let already_injected = review_session
        .session
        .clone_history()
        .await
        .raw_items()
        .any(|item| {
            matches!(item, ResponseItem::Message { role, content, .. }
            if role == "developer"
                && content.iter().any(|content| {
                    matches!(content, ContentItem::InputText { text }
                        if text == &followup_reminder)
                }))
        });
    if already_injected {
        return;
    }

    let reminder: ResponseItem = ContextualUserFragment::into(GuardianFollowupReviewReminder);
    review_session
        .session
        .inject_no_new_turn(vec![reminder], /*current_turn_context*/ None)
        .await;
}

async fn ensure_guardian_node_repl_policy(
    review_session: &GuardianReviewSession,
    params: &GuardianReviewSessionParams,
) -> anyhow::Result<()> {
    if !params
        .parent_context
        .turn()
        .model_info()
        .node_repl_auto_review_required
        || !matches!(
            &params.request,
            GuardianApprovalRequest::McpToolCall { server, tool_name, .. }
                if is_node_repl_backed_server(server) && tool_name == "js"
        )
    {
        return Ok(());
    }

    let policy = &params.node_repl_policy;
    let policy_body = policy.body();
    if policy_body.is_empty() {
        return Ok(());
    }
    let already_injected = review_session
        .session
        .clone_history()
        .await
        .raw_items()
        .any(|item| {
            matches!(item, ResponseItem::Message { role, content, .. }
            if role == "developer"
                && content.iter().any(|content| {
                    matches!(content, ContentItem::InputText { text } if text == &policy_body)
                }))
        });
    if already_injected {
        return Ok(());
    }

    let turn_context = review_session.session.new_default_turn().await;
    if review_session
        .session
        .reference_context_item()
        .await
        .is_none()
    {
        let initialize_context: BoxFuture<'_, anyhow::Result<()>> = Box::pin(async {
            let step_context = review_session
                .session
                .capture_step_context(Arc::clone(&turn_context), &review_session.cancel_token)
                .await?;
            review_session
                .session
                .record_context_updates_and_set_reference_context_item(step_context.as_ref())
                .await?;
            Ok(())
        });
        initialize_context.await?;
    }

    let item: ResponseItem = ContextualUserFragment::into(policy.clone());
    review_session
        .session
        .inject_client_response_items(vec![item], turn_context.as_ref())
        .await;

    Ok(())
}

async fn load_rollout_items_for_fork(
    session: &Session,
) -> anyhow::Result<Option<Vec<RolloutItem>>> {
    session
        .try_ensure_rollout_materialized(PersistContext::Standard)
        .await?;
    session.flush_rollout().await?;
    let live_thread = session.live_thread_for_persistence("guardian review fork")?;
    let history = live_thread.load_history(/*include_archived*/ true).await?;
    Ok(Some(history.items))
}

async fn wait_for_guardian_review(
    review_session: &GuardianReviewSession,
    expected_turn_id: &str,
    deadline: tokio::time::Instant,
    external_cancel: Option<&CancellationToken>,
    analytics_result: &mut GuardianReviewAnalyticsResult,
) -> (GuardianReviewSessionOutcome, bool, bool) {
    let timeout = tokio::time::sleep_until(deadline);
    tokio::pin!(timeout);
    let mut last_error: Option<ErrorEvent> = None;

    loop {
        tokio::select! {
            _ = &mut timeout => {
                let keep_review_session = interrupt_and_drain_turn(
                    review_session,
                    expected_turn_id,
                )
                .await
                .is_ok();
                return (GuardianReviewSessionOutcome::TimedOut, keep_review_session, false);
            }
            _ = async {
                if let Some(cancel_token) = external_cancel {
                    cancel_token.cancelled().await;
                } else {
                    std::future::pending::<()>().await;
                }
            } => {
                let keep_review_session = interrupt_and_drain_turn(
                    review_session,
                    expected_turn_id,
                )
                .await
                .is_ok();
                return (GuardianReviewSessionOutcome::Aborted, keep_review_session, false);
            }
            event = review_session.io.next_event() => {
                match event {
                    Ok(event) if !event_matches_turn(&event, expected_turn_id) => {}
                    Ok(event) if matches!(&event.msg, EventMsg::ItemCompleted(_)) => {
                        review_session.admit_node_repl_evidence(&event).await;
                    }
                    Ok(event) => match event.msg {
                        EventMsg::TurnComplete(turn_complete) => {
                            analytics_result.time_to_first_token_ms = turn_complete
                                .time_to_first_token_ms
                                .and_then(|ms| u64::try_from(ms).ok());
                            if turn_complete.last_agent_message.is_none()
                                && let Some(error) = last_error
                            {
                                return (
                                    GuardianReviewSessionOutcome::SessionFailed {
                                        error: anyhow!(error.message),
                                        error_info: error.codex_error_info,
                                    },
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
                            last_error = Some(error);
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

fn event_matches_turn(event: &Event, expected_turn_id: &str) -> bool {
    if event.id != expected_turn_id {
        return false;
    }

    match &event.msg {
        EventMsg::TurnComplete(turn_complete) => turn_complete.turn_id == expected_turn_id,
        EventMsg::TurnAborted(turn_aborted) => {
            turn_aborted.turn_id.as_deref() == Some(expected_turn_id)
        }
        _ => true,
    }
}

fn read_only_guardian_permission_profile(
    permission_profile: &PermissionProfile,
) -> PermissionProfile {
    permission_profile
        .intersect_with_read_only()
        .unwrap_or(PermissionProfile::External {
            network: codex_protocol::permissions::NetworkSandboxPolicy::Restricted,
        })
}

pub(crate) fn build_guardian_review_session_config(
    parent_config: &Config,
    live_network_config: Option<codex_network_proxy::NetworkProxyConfig>,
    active_model: &str,
    reasoning_effort: Option<codex_protocol::openai_models::ReasoningEffort>,
    model_messages: Option<&ModelMessages>,
) -> anyhow::Result<Config> {
    let mut guardian_config = parent_config.clone();
    guardian_config.model = Some(active_model.to_string());
    guardian_config.model_reasoning_effort = reasoning_effort;
    guardian_config.model_provider.request_max_retries = Some(1);
    guardian_config.model_provider.stream_max_retries = Some(1);
    guardian_config.include_skill_instructions = false;
    guardian_config.memories.use_memories = false;
    guardian_config.memories.dedicated_tools = false;
    let catalog_auto_review = model_messages.and_then(|messages| messages.auto_review.as_ref());
    let tenant_policy_config = parent_config.resolve_guardian_policy(model_messages);
    let policy_template = catalog_auto_review
        .and_then(|messages| messages.policy_template.as_deref())
        .unwrap_or(BUNDLED_GUARDIAN_POLICY_TEMPLATE);
    guardian_config.base_instructions = Some(guardian_policy_prompt_with_config_and_template(
        tenant_policy_config,
        policy_template,
    ));
    guardian_config.base_instructions_provenance = Some(BaseInstructionsProvenance::Custom);
    guardian_config.notify = None;
    guardian_config.developer_instructions = None;
    guardian_config.permissions.approval_policy = Constrained::allow_only(AskForApproval::Never);
    let guardian_permission_profile =
        read_only_guardian_permission_profile(parent_config.permissions.permission_profile());
    guardian_config
        .permissions
        .set_permission_profile(guardian_permission_profile)
        .map_err(|err| {
            anyhow::anyhow!("guardian review session could not set permission profile: {err}")
        })?;
    guardian_config.include_apps_instructions = false;
    guardian_config
        .mcp_servers
        .set(HashMap::new())
        .map_err(|err| {
            anyhow::anyhow!("guardian review session could not clear MCP servers: {err}")
        })?;
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
            guardian_config.permissions.permission_profile(),
        )?);
    }
    for feature in [
        Feature::Collab,
        Feature::MultiAgentV2,
        Feature::GuardianV2,
        Feature::CodexHooks,
        Feature::Apps,
        Feature::Plugins,
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
            warn!(
                "guardian review session could not disable `features.{}`; continuing with the feature enabled",
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

async fn interrupt_and_drain_turn(
    review_session: &GuardianReviewSession,
    expected_turn_id: &str,
) -> anyhow::Result<()> {
    let _ = review_session.io.submit(Op::Interrupt).await;

    tokio::time::timeout(GUARDIAN_INTERRUPT_DRAIN_TIMEOUT, async {
        loop {
            let event = review_session.io.next_event().await?;
            if !event_matches_turn(&event, expected_turn_id) {
                continue;
            }
            review_session.admit_node_repl_evidence(&event).await;
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
#[path = "review_session_tests.rs"]
mod tests;
