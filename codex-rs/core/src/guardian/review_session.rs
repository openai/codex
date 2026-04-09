use std::future::Future;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Duration;

use anyhow::anyhow;
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
use codex_protocol::user_input::UserInput;
use serde_json::Value;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
use tracing::warn;

use crate::agent::SubAgentConfigBuilder;
use crate::agent::SubAgentPromptInheritance;
use crate::agent::SubAgentPromptSelection;
use crate::codex::Codex;
use crate::codex::Session;
use crate::codex::TurnContext;
use crate::codex_delegate::run_codex_thread_interactive;
use crate::config::Config;
use crate::config::Constrained;
use crate::config::NetworkProxySpec;
use crate::config::Permissions;
use crate::rollout::recorder::RolloutRecorder;
use crate::session_surface::SessionSurfacePolicy;
use codex_model_provider_info::ModelProviderInfo;

use super::GUARDIAN_REVIEW_TIMEOUT;
use super::GUARDIAN_REVIEWER_NAME;
use super::prompt::guardian_policy_prompt;
use super::prompt::guardian_policy_prompt_with_config;

const GUARDIAN_INTERRUPT_DRAIN_TIMEOUT: Duration = Duration::from_secs(5);
const GUARDIAN_BASE_INSTRUCTIONS: &str = concat!(
    "You are Codex Guardian, a focused approval-review agent. ",
    "Follow the guardian developer policy. ",
    "Return only the required final JSON."
);
const GUARDIAN_FOLLOWUP_REVIEW_REMINDER: &str = concat!(
    "Use prior reviews as context, not binding precedent. ",
    "Follow the Workspace Policy. ",
    "If the user explicitly approves a previously rejected action after being informed of the ",
    "concrete risks, set user_authorization to high and derive outcome from policy."
);

#[derive(Debug)]
pub(crate) enum GuardianReviewSessionOutcome {
    Completed(anyhow::Result<Option<String>>),
    TimedOut,
    Aborted,
}

pub(crate) struct GuardianReviewSessionParams {
    pub(crate) parent_session: Arc<Session>,
    pub(crate) parent_turn: Arc<TurnContext>,
    pub(crate) spawn_config: Config,
    pub(crate) prompt_items: Vec<UserInput>,
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
    has_prior_review: AtomicBool,
    review_lock: Mutex<()>,
    last_committed_rollout_items: Mutex<Option<Vec<RolloutItem>>>,
}

struct EphemeralReviewCleanup {
    state: Arc<Mutex<GuardianReviewSessionState>>,
    review_session: Option<Arc<GuardianReviewSession>>,
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
    codex_linux_sandbox_exe: Option<PathBuf>,
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
            codex_linux_sandbox_exe: spawn_config.codex_linux_sandbox_exe.clone(),
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

    async fn fork_initial_history(&self) -> Option<InitialHistory> {
        self.last_committed_rollout_items
            .lock()
            .await
            .clone()
            .filter(|items| !items.is_empty())
            .map(InitialHistory::Forked)
    }

    async fn refresh_last_committed_rollout_items(&self) {
        match load_rollout_items_for_fork(&self.codex.session).await {
            Ok(Some(items)) => {
                *self.last_committed_rollout_items.lock().await = Some(items);
            }
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
    ) -> GuardianReviewSessionOutcome {
        let deadline = tokio::time::Instant::now() + GUARDIAN_REVIEW_TIMEOUT;
        let next_reuse_key = GuardianReviewSessionReuseKey::from_spawn_config(&params.spawn_config);
        let mut stale_trunk_to_shutdown = None;
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
                            /*initial_history*/ None,
                        )),
                    )
                    .await
                    {
                        Ok(Ok(review_session)) => Arc::new(review_session),
                        Ok(Err(err)) => {
                            return GuardianReviewSessionOutcome::Completed(Err(err));
                        }
                        Err(outcome) => return outcome,
                    };
                    state.trunk = Some(Arc::clone(&review_session));
                }

                state.trunk.as_ref().cloned()
            }
            Err(outcome) => return outcome,
        };

        if let Some(review_session) = stale_trunk_to_shutdown {
            review_session.shutdown_in_background();
        }

        let Some(trunk) = trunk_candidate else {
            return GuardianReviewSessionOutcome::Completed(Err(anyhow!(
                "guardian review session was not available after spawn"
            )));
        };

        if trunk.reuse_key != next_reuse_key {
            return self
                .run_ephemeral_review(
                    params,
                    next_reuse_key,
                    deadline,
                    /*initial_history*/ None,
                )
                .await;
        }

        let trunk_guard = match trunk.review_lock.try_lock() {
            Ok(trunk_guard) => trunk_guard,
            Err(_) => {
                let initial_history = trunk.fork_initial_history().await;
                return self
                    .run_ephemeral_review(params, next_reuse_key, deadline, initial_history)
                    .await;
            }
        };

        let (outcome, keep_review_session) =
            run_review_on_session(trunk.as_ref(), &params, deadline).await;
        if keep_review_session && matches!(outcome, GuardianReviewSessionOutcome::Completed(_)) {
            trunk.refresh_last_committed_rollout_items().await;
        }
        drop(trunk_guard);

        if keep_review_session {
            outcome
        } else {
            if let Some(review_session) = self.remove_trunk_if_current(&trunk).await {
                review_session.shutdown_in_background();
            }
            outcome
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
            has_prior_review: AtomicBool::new(false),
            review_lock: Mutex::new(()),
            last_committed_rollout_items: Mutex::new(None),
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
                has_prior_review: AtomicBool::new(false),
                review_lock: Mutex::new(()),
                last_committed_rollout_items: Mutex::new(None),
            }));
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
        initial_history: Option<InitialHistory>,
    ) -> GuardianReviewSessionOutcome {
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
                initial_history,
            )),
        )
        .await
        {
            Ok(Ok(review_session)) => Arc::new(review_session),
            Ok(Err(err)) => return GuardianReviewSessionOutcome::Completed(Err(err)),
            Err(outcome) => return outcome,
        };
        self.register_active_ephemeral(Arc::clone(&review_session))
            .await;
        let mut cleanup =
            EphemeralReviewCleanup::new(Arc::clone(&self.state), Arc::clone(&review_session));

        let (outcome, _) = run_review_on_session(review_session.as_ref(), &params, deadline).await;
        if let Some(review_session) = self.take_active_ephemeral(&review_session).await {
            cleanup.disarm();
            review_session.shutdown_in_background();
        }
        outcome
    }
}

async fn spawn_guardian_review_session(
    params: &GuardianReviewSessionParams,
    spawn_config: Config,
    reuse_key: GuardianReviewSessionReuseKey,
    cancel_token: CancellationToken,
    initial_history: Option<InitialHistory>,
) -> anyhow::Result<GuardianReviewSession> {
    let has_prior_review = initial_history.is_some();
    let codex = run_codex_thread_interactive(
        spawn_config,
        SessionSurfacePolicy::guardian_review(),
        params.parent_session.services.auth_manager.clone(),
        params.parent_session.services.models_manager.clone(),
        Arc::clone(&params.parent_session),
        Arc::clone(&params.parent_turn),
        cancel_token.clone(),
        SubAgentSource::Other(GUARDIAN_REVIEWER_NAME.to_string()),
        initial_history,
    )
    .await?;

    Ok(GuardianReviewSession {
        codex,
        cancel_token,
        reuse_key,
        has_prior_review: AtomicBool::new(has_prior_review),
        review_lock: Mutex::new(()),
        last_committed_rollout_items: Mutex::new(None),
    })
}

async fn run_review_on_session(
    review_session: &GuardianReviewSession,
    params: &GuardianReviewSessionParams,
    deadline: tokio::time::Instant,
) -> (GuardianReviewSessionOutcome, bool) {
    if review_session.has_prior_review.load(Ordering::Relaxed) {
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

            review_session
                .codex
                .submit(Op::UserTurn {
                    items: params.prompt_items.clone(),
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
                .await
        }),
    )
    .await;
    let submit_result = match submit_result {
        Ok(submit_result) => submit_result,
        Err(outcome) => return (outcome, false),
    };
    if let Err(err) = submit_result {
        return (
            GuardianReviewSessionOutcome::Completed(Err(err.into())),
            false,
        );
    }

    let outcome =
        wait_for_guardian_review(review_session, deadline, params.external_cancel.as_ref()).await;
    if matches!(outcome.0, GuardianReviewSessionOutcome::Completed(_)) {
        review_session
            .has_prior_review
            .store(true, Ordering::Relaxed);
    }
    outcome
}

async fn append_guardian_followup_reminder(review_session: &GuardianReviewSession) {
    let turn_context = review_session.codex.session.new_default_turn().await;
    let reminder: ResponseItem =
        DeveloperInstructions::new(GUARDIAN_FOLLOWUP_REVIEW_REMINDER).into();
    review_session
        .codex
        .session
        .record_into_history(std::slice::from_ref(&reminder), turn_context.as_ref())
        .await;
}

async fn load_rollout_items_for_fork(
    session: &Session,
) -> anyhow::Result<Option<Vec<RolloutItem>>> {
    session.flush_rollout().await;
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
) -> (GuardianReviewSessionOutcome, bool) {
    let timeout = tokio::time::sleep_until(deadline);
    tokio::pin!(timeout);
    let mut last_error_message: Option<String> = None;

    loop {
        tokio::select! {
            _ = &mut timeout => {
                let keep_review_session = interrupt_and_drain_turn(&review_session.codex).await.is_ok();
                return (GuardianReviewSessionOutcome::TimedOut, keep_review_session);
            }
            _ = async {
                if let Some(cancel_token) = external_cancel {
                    cancel_token.cancelled().await;
                } else {
                    std::future::pending::<()>().await;
                }
            } => {
                let keep_review_session = interrupt_and_drain_turn(&review_session.codex).await.is_ok();
                return (GuardianReviewSessionOutcome::Aborted, keep_review_session);
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
                                );
                            }
                            return (
                                GuardianReviewSessionOutcome::Completed(Ok(turn_complete.last_agent_message)),
                                true,
                            );
                        }
                        EventMsg::Error(error) => {
                            last_error_message = Some(error.message);
                        }
                        EventMsg::TurnAborted(_) => {
                            return (GuardianReviewSessionOutcome::Aborted, true);
                        }
                        _ => {}
                    },
                    Err(err) => {
                        return (
                            GuardianReviewSessionOutcome::Completed(Err(err.into())),
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
    let mut guardian_config = SubAgentConfigBuilder::from_parent_config(parent_config)
        .prompt_inheritance(SubAgentPromptInheritance::Select(
            guardian_review_prompt_selection(),
        ))
        .build();
    guardian_config.model = Some(active_model.to_string());
    guardian_config.model_reasoning_effort = reasoning_effort;
    guardian_config.personality = None;
    guardian_config.base_instructions = Some(GUARDIAN_BASE_INSTRUCTIONS.to_string());
    guardian_config.user_instructions = None;
    guardian_config.developer_instructions = Some(
        parent_config
            .guardian_policy_config
            .as_deref()
            .map(guardian_policy_prompt_with_config)
            .unwrap_or_else(guardian_policy_prompt),
    );
    guardian_config.compact_prompt = None;
    guardian_config.include_permissions_instructions = false;
    guardian_config.include_apps_instructions = false;
    guardian_config.include_environment_context = true;
    guardian_config.include_apply_patch_tool = false;
    guardian_config.use_experimental_unified_exec_tool = false;
    guardian_config.permissions.allow_login_shell = false;
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
    Ok(guardian_config)
}

const fn guardian_review_prompt_selection() -> SubAgentPromptSelection {
    SubAgentPromptSelection {
        base_instructions: false,
        user_instructions: false,
        project_docs: true,
        developer_instructions: true,
        compact_prompt: false,
        permissions: false,
        apps: false,
        environment_context: true,
    }
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

    #[test]
    fn guardian_review_session_config_change_invalidates_cached_session() {
        let parent_config = crate::config::test_config();
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

    #[test]
    fn guardian_review_session_cwd_change_does_not_invalidate_cached_session() {
        let mut parent_config = crate::config::test_config();
        let cached_spawn_config = build_guardian_review_session_config(
            &parent_config,
            /*live_network_config*/ None,
            "active-model",
            /*reasoning_effort*/ None,
        )
        .expect("cached guardian config");
        let cached_reuse_key =
            GuardianReviewSessionReuseKey::from_spawn_config(&cached_spawn_config);

        parent_config.cwd =
            codex_utils_absolute_path::AbsolutePathBuf::from_absolute_path("/tmp/guardian-cwd")
                .expect("absolute cwd");
        let next_spawn_config = build_guardian_review_session_config(
            &parent_config,
            /*live_network_config*/ None,
            "active-model",
            /*reasoning_effort*/ None,
        )
        .expect("next guardian config");

        assert_eq!(
            cached_reuse_key,
            GuardianReviewSessionReuseKey::from_spawn_config(&next_spawn_config)
        );
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
}
