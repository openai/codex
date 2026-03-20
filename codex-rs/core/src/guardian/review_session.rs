use std::collections::HashMap;
use std::future::Future;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::anyhow;
use codex_protocol::config_types::ReasoningSummary as ReasoningSummaryConfig;
use codex_protocol::openai_models::ReasoningEffort as ReasoningEffortConfig;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::InitialHistory;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::SubAgentSource;
use codex_protocol::user_input::UserInput;
use serde_json::Value;
use tokio::sync::Mutex;
use tokio::sync::OwnedSemaphorePermit;
use tokio::sync::Semaphore;
use tokio_util::sync::CancellationToken;
use tracing::warn;

use crate::codex::Codex;
use crate::codex::Session;
use crate::codex::TurnContext;
use crate::codex_delegate::run_codex_thread_interactive;
use crate::config::Config;
use crate::config::Constrained;
use crate::config::ManagedFeatures;
use crate::config::NetworkProxySpec;
use crate::config::Permissions;
use crate::config::types::McpServerConfig;
use crate::features::Feature;
use crate::model_provider_info::ModelProviderInfo;
use crate::protocol::SandboxPolicy;
use crate::rollout::recorder::RolloutRecorder;

use super::GUARDIAN_REVIEW_TIMEOUT;
use super::GUARDIAN_REVIEWER_NAME;
use super::prompt::guardian_policy_prompt;

const GUARDIAN_INTERRUPT_DRAIN_TIMEOUT: Duration = Duration::from_secs(5);
pub(crate) const GUARDIAN_ACTIVE_FORK_CAP: usize = 10;

#[derive(Debug)]
pub(crate) enum GuardianReviewSessionOutcome {
    Completed(anyhow::Result<Option<String>>),
    TimedOut,
    Aborted,
}

struct GuardianReviewExecutionResult {
    outcome: GuardianReviewSessionOutcome,
    session_healthy: bool,
}

enum GuardianTrunkState {
    Ready(Arc<GuardianReviewSession>),
    NeedsSpawn {
        stale_trunk_to_shutdown: Option<Arc<GuardianReviewSession>>,
    },
    ShutdownStarted,
}

enum GuardianReviewLane<'a> {
    UseTrunk(tokio::sync::MutexGuard<'a, ()>),
    Fork {
        fork_permit: OwnedSemaphorePermit,
        initial_history: Option<InitialHistory>,
    },
    RetryWithFreshTrunk,
}

pub(crate) struct GuardianReviewSessionParams {
    pub(crate) parent_session: Arc<Session>,
    pub(crate) parent_turn: Arc<TurnContext>,
    pub(crate) spawn_config: Config,
    pub(crate) prompt_items: Vec<UserInput>,
    pub(crate) schema: Value,
    pub(crate) model: String,
    pub(crate) reasoning_effort: Option<ReasoningEffortConfig>,
    pub(crate) external_cancel: Option<CancellationToken>,
}

#[derive(Clone)]
pub(crate) struct GuardianReviewSessionManager {
    state: Arc<Mutex<GuardianReviewSessionState>>,
    spawn_lock: Arc<Mutex<()>>,
    fork_pool: GuardianForkPool,
}

#[derive(Default)]
struct GuardianReviewSessionState {
    trunk: Option<Arc<GuardianReviewSession>>,
    shutdown_started: bool,
}

#[derive(Clone)]
struct GuardianForkPool {
    active_forks: Arc<Mutex<Vec<Arc<GuardianReviewSession>>>>,
    capacity: Arc<Semaphore>,
}

struct GuardianReviewSession {
    codex: Codex,
    cancel_token: CancellationToken,
    reuse_key: GuardianReviewSessionReuseKey,
    review_lock: Mutex<()>,
    last_committed_rollout_items: Mutex<Option<Vec<RolloutItem>>>,
}

struct ForkReviewCleanup {
    fork_pool: GuardianForkPool,
    review_session: Option<Arc<GuardianReviewSession>>,
    fork_permit: Option<OwnedSemaphorePermit>,
}

pub(super) struct GuardianResolvedReviewConfig {
    pub(super) spawn_config: Config,
    pub(super) model: String,
    pub(super) reasoning_effort: Option<ReasoningEffortConfig>,
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
    permissions: Permissions,
    developer_instructions: Option<String>,
    base_instructions: Option<String>,
    user_instructions: Option<String>,
    compact_prompt: Option<String>,
    cwd: PathBuf,
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

impl GuardianForkPool {
    fn new(capacity: usize) -> Self {
        Self {
            active_forks: Arc::default(),
            capacity: Arc::new(Semaphore::new(capacity)),
        }
    }

    fn try_acquire_permit(&self) -> Option<OwnedSemaphorePermit> {
        self.capacity.clone().try_acquire_owned().ok()
    }

    async fn acquire_permit(&self) -> Result<OwnedSemaphorePermit, tokio::sync::AcquireError> {
        self.capacity.clone().acquire_owned().await
    }

    async fn register(&self, review_session: Arc<GuardianReviewSession>) {
        self.active_forks.lock().await.push(review_session);
    }

    async fn unregister(&self, review_session: &Arc<GuardianReviewSession>) {
        let mut active_forks = self.active_forks.lock().await;
        if let Some(index) = active_forks
            .iter()
            .position(|active_review| Arc::ptr_eq(active_review, review_session))
        {
            active_forks.swap_remove(index);
        }
    }

    async fn take_all(&self) -> Vec<Arc<GuardianReviewSession>> {
        std::mem::take(&mut *self.active_forks.lock().await)
    }
}

impl ForkReviewCleanup {
    fn new(
        fork_pool: GuardianForkPool,
        review_session: Arc<GuardianReviewSession>,
        fork_permit: OwnedSemaphorePermit,
    ) -> Self {
        Self {
            fork_pool,
            review_session: Some(review_session),
            fork_permit: Some(fork_permit),
        }
    }

    fn into_parts(mut self) -> Option<(Arc<GuardianReviewSession>, OwnedSemaphorePermit)> {
        match (self.review_session.take(), self.fork_permit.take()) {
            (Some(review_session), Some(fork_permit)) => Some((review_session, fork_permit)),
            (review_session, fork_permit) => {
                warn!("fork cleanup lost ownership before teardown");
                if let Some(review_session) = review_session {
                    review_session.shutdown_in_background();
                }
                drop(fork_permit);
                None
            }
        }
    }
}

impl Drop for ForkReviewCleanup {
    fn drop(&mut self) {
        let Some(review_session) = self.review_session.take() else {
            return;
        };
        let fork_pool = self.fork_pool.clone();
        let Some(fork_permit) = self.fork_permit.take() else {
            return;
        };
        drop(tokio::spawn(async move {
            fork_pool.unregister(&review_session).await;
            drop(fork_permit);
            review_session.shutdown().await;
        }));
    }
}

impl Default for GuardianReviewSessionManager {
    fn default() -> Self {
        Self {
            state: Arc::default(),
            spawn_lock: Arc::default(),
            fork_pool: GuardianForkPool::new(GUARDIAN_ACTIVE_FORK_CAP),
        }
    }
}

impl GuardianReviewSessionManager {
    #[cfg(test)]
    pub(crate) fn new_for_test(active_fork_cap: usize) -> Self {
        Self {
            state: Arc::default(),
            spawn_lock: Arc::default(),
            fork_pool: GuardianForkPool::new(active_fork_cap),
        }
    }

    pub(crate) async fn trunk_rollout_path(&self) -> Option<PathBuf> {
        let trunk = self.state.lock().await.trunk.as_ref().cloned();
        let trunk = trunk?;
        trunk.codex.session.flush_rollout().await;
        trunk.codex.session.current_rollout_path().await
    }

    pub(crate) fn spawn_initialize_trunk_if_needed(
        &self,
        parent_session: Arc<Session>,
        parent_turn: Arc<TurnContext>,
    ) {
        let manager = self.clone();
        drop(tokio::spawn(async move {
            manager
                .initialize_trunk_if_needed(parent_session, parent_turn)
                .await;
        }));
    }

    pub(crate) async fn initialize_trunk_if_needed(
        &self,
        parent_session: Arc<Session>,
        parent_turn: Arc<TurnContext>,
    ) {
        let resolved =
            match resolve_guardian_review_config(parent_session.as_ref(), parent_turn.as_ref())
                .await
            {
                Ok(resolved) => resolved,
                Err(err) => {
                    warn!("failed to resolve guardian review config: {err}");
                    return;
                }
            };
        let next_reuse_key =
            GuardianReviewSessionReuseKey::from_spawn_config(&resolved.spawn_config);
        let params = GuardianReviewSessionParams {
            parent_session,
            parent_turn: Arc::clone(&parent_turn),
            spawn_config: resolved.spawn_config,
            prompt_items: Vec::new(),
            schema: Value::Null,
            model: resolved.model,
            reasoning_effort: resolved.reasoning_effort,
            external_cancel: None,
        };
        match self.prepare_trunk(&next_reuse_key).await {
            GuardianTrunkState::Ready(_) | GuardianTrunkState::ShutdownStarted => return,
            GuardianTrunkState::NeedsSpawn {
                stale_trunk_to_shutdown,
            } => {
                if let Some(review_session) = stale_trunk_to_shutdown {
                    review_session.shutdown_in_background();
                }
            }
        }

        let _spawn_guard = self.spawn_lock.lock().await;
        match self.prepare_trunk(&next_reuse_key).await {
            GuardianTrunkState::Ready(_) | GuardianTrunkState::ShutdownStarted => return,
            GuardianTrunkState::NeedsSpawn {
                stale_trunk_to_shutdown,
            } => {
                if let Some(review_session) = stale_trunk_to_shutdown {
                    review_session.shutdown_in_background();
                }
            }
        }

        let spawn_cancel_token = CancellationToken::new();
        let review_session = match spawn_guardian_review_session(
            &params,
            params.spawn_config.clone(),
            next_reuse_key,
            spawn_cancel_token,
            /*initial_history*/ None,
        )
        .await
        {
            Ok(review_session) => Arc::new(review_session),
            Err(err) => {
                warn!("failed to eagerly initialize guardian review session: {err}");
                return;
            }
        };
        match self.install_spawned_trunk(review_session).await {
            GuardianTrunkState::Ready(_) | GuardianTrunkState::ShutdownStarted => {}
            GuardianTrunkState::NeedsSpawn { .. } => {
                warn!("guardian review session was not available after eager initialization");
            }
        }
    }

    pub(crate) async fn shutdown(&self) {
        let review_session = {
            let mut state = self.state.lock().await;
            state.shutdown_started = true;
            state.trunk.take()
        };
        let active_forks = self.fork_pool.take_all().await;
        if let Some(review_session) = review_session {
            review_session.shutdown().await;
        }
        for review_session in active_forks {
            review_session.shutdown().await;
        }
    }

    pub(crate) async fn run_review(
        &self,
        params: GuardianReviewSessionParams,
    ) -> GuardianReviewSessionOutcome {
        let deadline = tokio::time::Instant::now() + GUARDIAN_REVIEW_TIMEOUT;
        let next_reuse_key = GuardianReviewSessionReuseKey::from_spawn_config(&params.spawn_config);
        loop {
            let trunk = match self.prepare_trunk(&next_reuse_key).await {
                GuardianTrunkState::Ready(trunk) => trunk,
                GuardianTrunkState::ShutdownStarted => {
                    return GuardianReviewSessionOutcome::Aborted;
                }
                GuardianTrunkState::NeedsSpawn {
                    stale_trunk_to_shutdown,
                } => {
                    if let Some(review_session) = stale_trunk_to_shutdown {
                        review_session.shutdown_in_background();
                    }

                    let _spawn_guard = match run_before_review_deadline(
                        deadline,
                        params.external_cancel.as_ref(),
                        self.spawn_lock.lock(),
                    )
                    .await
                    {
                        Ok(spawn_guard) => spawn_guard,
                        Err(outcome) => return outcome,
                    };

                    match self.prepare_trunk(&next_reuse_key).await {
                        GuardianTrunkState::Ready(trunk) => trunk,
                        GuardianTrunkState::ShutdownStarted => {
                            return GuardianReviewSessionOutcome::Aborted;
                        }
                        GuardianTrunkState::NeedsSpawn {
                            stale_trunk_to_shutdown,
                        } => {
                            if let Some(review_session) = stale_trunk_to_shutdown {
                                review_session.shutdown_in_background();
                            }

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

                            match self.install_spawned_trunk(review_session).await {
                                GuardianTrunkState::Ready(trunk) => trunk,
                                GuardianTrunkState::ShutdownStarted => {
                                    return GuardianReviewSessionOutcome::Aborted;
                                }
                                GuardianTrunkState::NeedsSpawn { .. } => continue,
                            }
                        }
                    }
                }
            };
            let review_lane = match self
                .choose_review_lane(
                    &trunk,
                    &next_reuse_key,
                    deadline,
                    params.external_cancel.as_ref(),
                )
                .await
            {
                Ok(review_lane) => review_lane,
                Err(outcome) => return outcome,
            };

            let trunk_guard = match review_lane {
                GuardianReviewLane::UseTrunk(trunk_guard) => trunk_guard,
                GuardianReviewLane::Fork {
                    fork_permit,
                    initial_history,
                } => {
                    return self
                        .run_forked_review(
                            params,
                            next_reuse_key.clone(),
                            deadline,
                            initial_history,
                            fork_permit,
                        )
                        .await;
                }
                GuardianReviewLane::RetryWithFreshTrunk => continue,
            };

            let execution_result = run_review_on_session(trunk.as_ref(), &params, deadline).await;
            if execution_result.session_healthy
                && matches!(
                    execution_result.outcome,
                    GuardianReviewSessionOutcome::Completed(_)
                )
            {
                trunk.refresh_last_committed_rollout_items().await;
            }
            drop(trunk_guard);

            return if execution_result.session_healthy {
                execution_result.outcome
            } else {
                if let Some(review_session) = self.remove_trunk_if_current(&trunk).await {
                    review_session.shutdown_in_background();
                }
                execution_result.outcome
            };
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

    async fn prepare_trunk(
        &self,
        next_reuse_key: &GuardianReviewSessionReuseKey,
    ) -> GuardianTrunkState {
        let mut state = self.state.lock().await;
        if state.shutdown_started {
            return GuardianTrunkState::ShutdownStarted;
        }
        if let Some(trunk) = state.trunk.as_ref()
            && trunk.reuse_key != *next_reuse_key
            && trunk.review_lock.try_lock().is_ok()
        {
            return GuardianTrunkState::NeedsSpawn {
                stale_trunk_to_shutdown: state.trunk.take(),
            };
        }
        if let Some(trunk) = state.trunk.as_ref() {
            GuardianTrunkState::Ready(Arc::clone(trunk))
        } else {
            GuardianTrunkState::NeedsSpawn {
                stale_trunk_to_shutdown: None,
            }
        }
    }

    async fn install_spawned_trunk(
        &self,
        review_session: Arc<GuardianReviewSession>,
    ) -> GuardianTrunkState {
        let mut state = self.state.lock().await;
        if state.shutdown_started {
            drop(state);
            review_session.shutdown_in_background();
            return GuardianTrunkState::ShutdownStarted;
        }
        if let Some(trunk) = state.trunk.as_ref() {
            let trunk = Arc::clone(trunk);
            drop(state);
            review_session.shutdown_in_background();
            return GuardianTrunkState::Ready(trunk);
        }
        state.trunk = Some(Arc::clone(&review_session));
        GuardianTrunkState::Ready(review_session)
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
            last_committed_rollout_items: Mutex::new(None),
        }));
    }

    #[cfg(test)]
    pub(crate) async fn register_fork_for_test(&self, codex: Codex) {
        let reuse_key = GuardianReviewSessionReuseKey::from_spawn_config(
            codex.session.get_config().await.as_ref(),
        );
        self.fork_pool
            .register(Arc::new(GuardianReviewSession {
                reuse_key,
                codex,
                cancel_token: CancellationToken::new(),
                review_lock: Mutex::new(()),
                last_committed_rollout_items: Mutex::new(None),
            }))
            .await;
    }

    async fn choose_review_lane<'a>(
        &self,
        trunk: &'a Arc<GuardianReviewSession>,
        next_reuse_key: &GuardianReviewSessionReuseKey,
        deadline: tokio::time::Instant,
        external_cancel: Option<&CancellationToken>,
    ) -> Result<GuardianReviewLane<'a>, GuardianReviewSessionOutcome> {
        if trunk.reuse_key != *next_reuse_key {
            return match self
                .wait_for_stale_trunk_or_fork(trunk, deadline, external_cancel)
                .await?
            {
                Some(fork_permit) => Ok(GuardianReviewLane::Fork {
                    fork_permit,
                    initial_history: None,
                }),
                None => Ok(GuardianReviewLane::RetryWithFreshTrunk),
            };
        }

        if let Ok(trunk_guard) = trunk.review_lock.try_lock() {
            return Ok(GuardianReviewLane::UseTrunk(trunk_guard));
        }

        self.wait_for_reusable_trunk_or_fork(trunk, deadline, external_cancel)
            .await
    }

    async fn wait_for_stale_trunk_or_fork(
        &self,
        trunk: &Arc<GuardianReviewSession>,
        deadline: tokio::time::Instant,
        external_cancel: Option<&CancellationToken>,
    ) -> Result<Option<OwnedSemaphorePermit>, GuardianReviewSessionOutcome> {
        if let Some(fork_permit) = self.fork_pool.try_acquire_permit() {
            return Ok(Some(fork_permit));
        }

        let permit_result = run_before_review_deadline(deadline, external_cancel, async {
            tokio::select! {
                _ = trunk.review_lock.lock() => None,
                permit = self.fork_pool.acquire_permit() => Some(permit),
            }
        })
        .await?;
        match permit_result {
            Some(Ok(fork_permit)) => Ok(Some(fork_permit)),
            Some(Err(err)) => Err(GuardianReviewSessionOutcome::Completed(Err(anyhow!(
                "guardian fork pool unexpectedly closed: {err}"
            )))),
            None => Ok(None),
        }
    }

    async fn wait_for_reusable_trunk_or_fork<'a>(
        &self,
        trunk: &'a Arc<GuardianReviewSession>,
        deadline: tokio::time::Instant,
        external_cancel: Option<&CancellationToken>,
    ) -> Result<GuardianReviewLane<'a>, GuardianReviewSessionOutcome> {
        if let Some(fork_permit) = self.fork_pool.try_acquire_permit() {
            return Ok(GuardianReviewLane::Fork {
                initial_history: trunk.fork_initial_history().await,
                fork_permit,
            });
        }

        let review_lane = run_before_review_deadline(deadline, external_cancel, async {
            tokio::select! {
                guard = trunk.review_lock.lock() => GuardianReviewLane::UseTrunk(guard),
                permit = self.fork_pool.acquire_permit() => match permit {
                    Ok(fork_permit) => GuardianReviewLane::Fork {
                        initial_history: trunk.fork_initial_history().await,
                        fork_permit,
                    },
                    Err(_) => GuardianReviewLane::RetryWithFreshTrunk,
                },
            }
        })
        .await?;
        if matches!(review_lane, GuardianReviewLane::RetryWithFreshTrunk) {
            return Err(GuardianReviewSessionOutcome::Completed(Err(anyhow!(
                "guardian fork pool unexpectedly closed"
            ))));
        }
        Ok(review_lane)
    }

    async fn run_forked_review(
        &self,
        params: GuardianReviewSessionParams,
        reuse_key: GuardianReviewSessionReuseKey,
        deadline: tokio::time::Instant,
        initial_history: Option<InitialHistory>,
        fork_permit: OwnedSemaphorePermit,
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
        self.fork_pool.register(Arc::clone(&review_session)).await;
        let cleanup = ForkReviewCleanup::new(
            self.fork_pool.clone(),
            Arc::clone(&review_session),
            fork_permit,
        );

        let execution_result =
            run_review_on_session(review_session.as_ref(), &params, deadline).await;
        if let Some((review_session, fork_permit)) = cleanup.into_parts() {
            self.fork_pool.unregister(&review_session).await;
            drop(fork_permit);
            review_session.shutdown_in_background();
        }
        execution_result.outcome
    }
}

async fn spawn_guardian_review_session(
    params: &GuardianReviewSessionParams,
    spawn_config: Config,
    reuse_key: GuardianReviewSessionReuseKey,
    cancel_token: CancellationToken,
    initial_history: Option<InitialHistory>,
) -> anyhow::Result<GuardianReviewSession> {
    let codex = run_codex_thread_interactive(
        spawn_config,
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
        review_lock: Mutex::new(()),
        last_committed_rollout_items: Mutex::new(None),
    })
}

async fn run_review_on_session(
    review_session: &GuardianReviewSession,
    params: &GuardianReviewSessionParams,
    deadline: tokio::time::Instant,
) -> GuardianReviewExecutionResult {
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
                    cwd: params.parent_turn.cwd.clone(),
                    approval_policy: AskForApproval::Never,
                    sandbox_policy: SandboxPolicy::new_read_only_policy(),
                    model: params.model.clone(),
                    effort: params.reasoning_effort,
                    summary: Some(ReasoningSummaryConfig::None),
                    service_tier: None,
                    final_output_json_schema: Some(params.schema.clone()),
                    collaboration_mode: None,
                    personality: None,
                })
                .await
        }),
    )
    .await;
    let submit_result = match submit_result {
        Ok(submit_result) => submit_result,
        Err(outcome) => {
            return GuardianReviewExecutionResult {
                outcome,
                session_healthy: false,
            };
        }
    };
    if let Err(err) = submit_result {
        return GuardianReviewExecutionResult {
            outcome: GuardianReviewSessionOutcome::Completed(Err(err.into())),
            session_healthy: false,
        };
    }

    wait_for_guardian_review(review_session, deadline, params.external_cancel.as_ref()).await
}

pub(super) async fn resolve_guardian_review_config(
    session: &Session,
    turn: &TurnContext,
) -> anyhow::Result<GuardianResolvedReviewConfig> {
    let live_network_config = match session.services.network_proxy.as_ref() {
        Some(network_proxy) => Some(network_proxy.proxy().current_cfg().await?),
        None => None,
    };
    let available_models = session
        .services
        .models_manager
        .list_models(crate::models_manager::manager::RefreshStrategy::Offline)
        .await;
    let preferred_reasoning_effort = |supports_low: bool, fallback| {
        if supports_low {
            Some(codex_protocol::openai_models::ReasoningEffort::Low)
        } else {
            fallback
        }
    };
    let preferred_model = available_models
        .iter()
        .find(|preset| preset.model == super::GUARDIAN_PREFERRED_MODEL);
    let (guardian_model, guardian_reasoning_effort) = if let Some(preset) = preferred_model {
        let reasoning_effort = preferred_reasoning_effort(
            preset
                .supported_reasoning_efforts
                .iter()
                .any(|effort| effort.effort == codex_protocol::openai_models::ReasoningEffort::Low),
            Some(preset.default_reasoning_effort),
        );
        (
            super::GUARDIAN_PREFERRED_MODEL.to_string(),
            reasoning_effort,
        )
    } else {
        let reasoning_effort = preferred_reasoning_effort(
            turn.model_info
                .supported_reasoning_levels
                .iter()
                .any(|preset| preset.effort == codex_protocol::openai_models::ReasoningEffort::Low),
            turn.reasoning_effort
                .or(turn.model_info.default_reasoning_level),
        );
        (turn.model_info.slug.clone(), reasoning_effort)
    };
    let spawn_config = build_guardian_review_session_config(
        turn.config.as_ref(),
        live_network_config,
        guardian_model.as_str(),
        guardian_reasoning_effort,
    )?;

    Ok(GuardianResolvedReviewConfig {
        spawn_config,
        model: guardian_model,
        reasoning_effort: guardian_reasoning_effort,
    })
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
) -> GuardianReviewExecutionResult {
    let timeout = tokio::time::sleep_until(deadline);
    tokio::pin!(timeout);

    loop {
        tokio::select! {
            _ = &mut timeout => {
                let session_healthy = interrupt_and_drain_turn(&review_session.codex).await.is_ok();
                return GuardianReviewExecutionResult {
                    outcome: GuardianReviewSessionOutcome::TimedOut,
                    session_healthy,
                };
            }
            _ = async {
                if let Some(cancel_token) = external_cancel {
                    cancel_token.cancelled().await;
                } else {
                    std::future::pending::<()>().await;
                }
            } => {
                let session_healthy = interrupt_and_drain_turn(&review_session.codex).await.is_ok();
                return GuardianReviewExecutionResult {
                    outcome: GuardianReviewSessionOutcome::Aborted,
                    session_healthy,
                };
            }
            event = review_session.codex.next_event() => {
                match event {
                    Ok(event) => match event.msg {
                        EventMsg::TurnComplete(turn_complete) => {
                            return GuardianReviewExecutionResult {
                                outcome: GuardianReviewSessionOutcome::Completed(
                                    Ok(turn_complete.last_agent_message),
                                ),
                                session_healthy: true,
                            };
                        }
                        EventMsg::TurnAborted(_) => {
                            return GuardianReviewExecutionResult {
                                outcome: GuardianReviewSessionOutcome::Aborted,
                                session_healthy: true,
                            };
                        }
                        _ => {}
                    },
                    Err(err) => {
                        return GuardianReviewExecutionResult {
                            outcome: GuardianReviewSessionOutcome::Completed(Err(err.into())),
                            session_healthy: false,
                        };
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
    guardian_config.model_reasoning_summary = Some(ReasoningSummaryConfig::None);
    guardian_config.personality = None;
    guardian_config.developer_instructions = Some(
        parent_config
            .guardian_developer_instructions
            .clone()
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

    #[test]
    fn guardian_review_session_config_change_invalidates_cached_session() {
        let parent_config = crate::config::test_config();
        let cached_spawn_config =
            build_guardian_review_session_config(&parent_config, None, "active-model", None)
                .expect("cached guardian config");
        let cached_reuse_key =
            GuardianReviewSessionReuseKey::from_spawn_config(&cached_spawn_config);

        let mut changed_parent_config = parent_config;
        changed_parent_config.model_provider.base_url =
            Some("https://guardian.example.invalid/v1".to_string());
        let next_spawn_config = build_guardian_review_session_config(
            &changed_parent_config,
            None,
            "active-model",
            None,
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
    fn guardian_review_session_config_disables_reasoning_summary_and_personality() {
        let mut parent_config = crate::config::test_config();
        parent_config.model_reasoning_summary = Some(ReasoningSummaryConfig::Detailed);
        parent_config.personality = Some(codex_protocol::config_types::Personality::Pragmatic);

        let guardian_config =
            build_guardian_review_session_config(&parent_config, None, "active-model", None)
                .expect("guardian config");

        assert_eq!(
            guardian_config.model_reasoning_summary,
            Some(ReasoningSummaryConfig::None)
        );
        assert_eq!(guardian_config.personality, None);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn run_before_review_deadline_times_out_before_future_completes() {
        let outcome = run_before_review_deadline(
            tokio::time::Instant::now() + Duration::from_millis(10),
            None,
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
            None,
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
            None,
            &cancel_token,
            async { 42usize },
        )
        .await;

        assert_eq!(outcome.unwrap(), 42);
        assert!(!cancel_token.is_cancelled());
    }
}
