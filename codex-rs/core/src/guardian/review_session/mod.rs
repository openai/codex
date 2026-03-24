//! Guardian review-session orchestration.
//!
//! This module owns the long-lived guardian trunk session plus any temporary forked sessions used
//! for parallel approvals. Config resolution and child-session spawning live in `spawn`, while
//! per-review execution and deadline handling live in `execution`.

mod execution;
mod spawn;
#[cfg(test)]
mod tests;

use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use codex_protocol::openai_models::ReasoningEffort as ReasoningEffortConfig;
use codex_protocol::protocol::InitialHistory;
use codex_protocol::user_input::UserInput;
use serde_json::Value;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
use tracing::warn;

use crate::codex::Codex;
use crate::codex::Session;
use crate::codex::TurnContext;
use crate::config::Config;
use crate::thread_manager::ForkSnapshot;
use crate::thread_manager::snapshot_rollout_history;

use self::execution::run_before_review_deadline;
use self::execution::run_review_on_session;
use self::spawn::GuardianReviewSessionSpawnOutcome;
#[cfg(test)]
pub(crate) use self::spawn::build_guardian_review_session_config;
pub(super) use self::spawn::resolve_guardian_review_config;
use self::spawn::review_outcome_from_spawn_outcome;
use self::spawn::spawn_review_session_before_deadline;

use super::GUARDIAN_REVIEW_TIMEOUT;

const GUARDIAN_EAGER_INIT_SPAWN_TIMEOUT: Duration = Duration::from_secs(5);
const GUARDIAN_FOLLOWUP_REVIEW_REMINDER: &str = concat!(
    "Use prior reviews as context, not binding precedent. ",
    "Follow the Workspace Policy. ",
    "If the user explicitly approves a previously rejected action after being informed of the ",
    "concrete risks, treat the action as authorized and assign low/medium risk."
);
const GUARDIAN_INTERRUPT_DRAIN_TIMEOUT: Duration = Duration::from_secs(5);

/// Public outcome for one guardian review attempt.
///
/// `Completed` includes both successful JSON output and ordinary execution failures from the child
/// guardian session. `TimedOut` and `Aborted` are reserved for the outer review deadline and
/// cancellation policy.
#[derive(Debug)]
pub(crate) enum GuardianReviewSessionOutcome {
    Completed(anyhow::Result<Option<String>>),
    TimedOut,
    Aborted,
}

/// Internal execution result for one review turn on an already-spawned guardian session.
///
/// `session_healthy` answers a narrower question than the overall review outcome: can this same
/// guardian session safely stay cached for future reviews, or should the caller evict it?
struct GuardianReviewExecutionResult {
    outcome: GuardianReviewSessionOutcome,
    session_healthy: bool,
}

/// Result of inspecting the cached trunk for a specific review request.
///
/// `Ready` may still contain a trunk whose reuse key no longer matches the current request when
/// that older trunk is busy. Callers handle that case by forking instead of trying to replace a
/// live session out from under an in-flight review.
enum GuardianTrunkState {
    Ready(Arc<GuardianReviewSession>),
    NeedsSpawn,
    ShutdownStarted,
}

/// Inputs for submitting one review turn to a guardian sub-session.
///
/// This is intentionally review-specific. Eager trunk initialization resolves config directly and
/// does not fabricate an "empty review" just to reuse this type.
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

/// Owns the shared guardian trunk and any temporary forked review sessions.
///
/// The trunk is the reusable guardian sub-session for the current session. Eager init is
/// best-effort and only fills an empty trunk slot. Real reviews may replace a stale idle trunk or
/// fork when the trunk is busy.
#[derive(Clone)]
pub(crate) struct GuardianReviewSessionManager {
    /// Shared trunk plus currently active forked sessions.
    state: Arc<Mutex<GuardianReviewSessionState>>,
    /// Serializes real trunk creation so eager init and first-use review cannot spawn duplicates.
    spawn_lock: Arc<Mutex<()>>,
    /// Allows shutdown to stop detached eager-init work before it finishes config resolution/spawn.
    eager_init_cancel: CancellationToken,
}

impl Default for GuardianReviewSessionManager {
    fn default() -> Self {
        Self {
            state: Arc::new(Mutex::new(GuardianReviewSessionState::default())),
            spawn_lock: Arc::new(Mutex::new(())),
            eager_init_cancel: CancellationToken::new(),
        }
    }
}

#[derive(Default)]
struct GuardianReviewSessionState {
    /// Shared guardian session reused across sequential approvals.
    trunk: Option<Arc<GuardianReviewSession>>,
    /// Forked sessions used only while a parallel review is in flight.
    active_forks: Vec<Arc<GuardianReviewSession>>,
    /// Once set, no new trunk or fork should be registered.
    shutdown_started: bool,
}

/// Runtime state for one guardian sub-session.
///
/// The trunk persists across approvals, while forked sessions are short-lived and always shut down
/// after the review that spawned them. Parallel forks snapshot the trunk through the generic
/// `ForkSnapshot` path instead of maintaining a guardian-specific committed-history cache.
struct GuardianReviewSession {
    /// Child Codex session running the guardian prompt.
    codex: Codex,
    /// Session-scoped cancellation used during shutdown.
    cancel_token: CancellationToken,
    /// Effective guardian config used for this child session.
    ///
    /// The trunk remains reusable only while future reviews resolve to the same config.
    spawn_config: Config,
    /// Tracks whether this session has already completed at least one review turn.
    has_prior_review: AtomicBool,
    /// Prevents overlapping reviews on the same guardian session.
    review_lock: Mutex<()>,
}

impl GuardianReviewSession {
    fn new(
        codex: Codex,
        cancel_token: CancellationToken,
        spawn_config: Config,
        has_prior_review: bool,
    ) -> Self {
        Self {
            codex,
            cancel_token,
            spawn_config,
            has_prior_review: AtomicBool::new(has_prior_review),
            review_lock: Mutex::new(()),
        }
    }

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

    /// Snapshot the trunk as a forkable committed prefix.
    ///
    /// `usize::MAX` with `TruncateBeforeNthUserMessage` means "keep everything committed so far,
    /// but if the session is currently mid-turn, drop that unfinished turn suffix." That matches
    /// the guardian policy of ignoring the in-flight review and forking from the last stable trunk
    /// state without synthesizing an interrupt marker.
    async fn fork_initial_history(&self) -> Option<InitialHistory> {
        self.codex.session.ensure_rollout_materialized().await;
        self.codex.session.flush_rollout().await;
        let rollout_path = self.codex.session.current_rollout_path().await?;
        match snapshot_rollout_history(
            rollout_path.as_path(),
            ForkSnapshot::TruncateBeforeNthUserMessage(usize::MAX),
        )
        .await
        {
            Ok(InitialHistory::New) => None,
            Ok(initial_history) => Some(initial_history),
            Err(err) => {
                warn!("failed to snapshot guardian trunk for fork: {err}");
                None
            }
        }
    }
}

/// RAII cleanup for forked guardian sessions.
///
/// The normal path removes the fork from `active_forks` and disarms this guard explicitly. If the
/// future is dropped early, the guard cleans up the fork in the background so shutdown does not
/// leak a live sub-session.
struct ForkReviewCleanup {
    state: Arc<Mutex<GuardianReviewSessionState>>,
    review_session: Option<Arc<GuardianReviewSession>>,
}

impl ForkReviewCleanup {
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

impl Drop for ForkReviewCleanup {
    fn drop(&mut self) {
        let Some(review_session) = self.review_session.take() else {
            return;
        };
        let state = Arc::clone(&self.state);
        drop(tokio::spawn(async move {
            let review_session = {
                let mut state = state.lock().await;
                take_review_session(&mut state.active_forks, &review_session)
            };
            if let Some(review_session) = review_session {
                review_session.shutdown().await;
            }
        }));
    }
}

impl GuardianReviewSessionManager {
    /// Starts best-effort background trunk initialization for a turn that is about to launch a
    /// fresh task.
    pub(crate) fn spawn_eager_trunk_init_if_needed(
        &self,
        parent_session: Arc<Session>,
        parent_turn: Arc<TurnContext>,
    ) {
        // Detached on purpose: eager init is only latency optimization and should never delay the
        // parent turn's normal execution path.
        let manager = self.clone();
        let eager_init_cancel = self.eager_init_cancel.clone();
        drop(tokio::spawn(async move {
            manager
                .run_eager_trunk_init_if_needed(parent_session, parent_turn, eager_init_cancel)
                .await;
        }));
    }

    async fn run_eager_trunk_init_if_needed(
        &self,
        parent_session: Arc<Session>,
        parent_turn: Arc<TurnContext>,
        eager_init_cancel: CancellationToken,
    ) {
        if !self.should_spawn_trunk_eagerly().await {
            return;
        }

        // Resolve the effective guardian model/config only after confirming the slot is still
        // empty, so steady-state turns with a warm trunk do not pay repeated model/network reads.
        let resolved = tokio::select! {
            _ = eager_init_cancel.cancelled() => return,
            resolved = resolve_guardian_review_config(parent_session.as_ref(), parent_turn.as_ref()) => resolved,
        };
        let resolved = match resolved {
            Ok(resolved) => resolved,
            Err(err) => {
                warn!("failed to resolve guardian review config: {err}");
                return;
            }
        };
        self.maybe_spawn_trunk_eagerly(
            &parent_session,
            &parent_turn,
            resolved.spawn_config,
            &eager_init_cancel,
        )
        .await;
    }

    pub(crate) async fn shutdown(&self) {
        self.eager_init_cancel.cancel();
        let (review_session, active_forks) = {
            let mut state = self.state.lock().await;
            state.shutdown_started = true;
            (state.trunk.take(), std::mem::take(&mut state.active_forks))
        };
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
        let trunk = match self
            .get_or_spawn_trunk_for_review(&params, deadline, params.external_cancel.as_ref())
            .await
        {
            Ok(Some(trunk)) => trunk,
            Ok(None) => return GuardianReviewSessionOutcome::Aborted,
            Err(outcome) => return outcome,
        };

        // A stale-but-busy trunk stays in place so the in-flight review can finish. New work forks
        // instead of replacing the live session.
        if trunk.spawn_config != params.spawn_config {
            return self
                .run_forked_review(params, deadline, /*initial_history*/ None)
                .await;
        }

        let trunk_guard = match trunk.review_lock.try_lock() {
            Ok(trunk_guard) => trunk_guard,
            Err(_) => {
                let initial_history = trunk.fork_initial_history().await;
                return self
                    .run_forked_review(params, deadline, initial_history)
                    .await;
            }
        };

        let execution_result = run_review_on_session(trunk.as_ref(), &params, deadline).await;
        drop(trunk_guard);

        if execution_result.session_healthy {
            execution_result.outcome
        } else {
            // Submit/wait failures can leave the child session in an unknown state, so only keep
            // the trunk cached when execution explicitly says it is still safe to reuse.
            let review_session = self.remove_trunk_if_current(&trunk).await;
            if let Some(review_session) = review_session {
                review_session.shutdown_in_background();
            }
            execution_result.outcome
        }
    }

    /// Best-effort eager initialization for the shared trunk.
    ///
    /// This path is intentionally conservative: it only fills an empty trunk slot and gives up if
    /// another spawn is already in progress or if shutdown begins.
    async fn maybe_spawn_trunk_eagerly(
        &self,
        parent_session: &Arc<Session>,
        parent_turn: &Arc<TurnContext>,
        spawn_config: Config,
        eager_init_cancel: &CancellationToken,
    ) {
        let Ok(_spawn_guard) = self.spawn_lock.try_lock() else {
            return;
        };

        if !self.should_spawn_trunk_eagerly().await {
            return;
        }

        match self
            .spawn_and_install_trunk(
                tokio::time::Instant::now() + GUARDIAN_EAGER_INIT_SPAWN_TIMEOUT,
                Some(eager_init_cancel),
                parent_session,
                parent_turn,
                spawn_config,
            )
            .await
        {
            Ok(Some(_)) => {}
            Ok(None) => {
                warn!("guardian review session was not available after eager initialization");
            }
            Err(GuardianReviewSessionSpawnOutcome::Failed(err)) => {
                warn!("failed to eagerly initialize guardian review session: {err}");
            }
            Err(GuardianReviewSessionSpawnOutcome::TimedOut) => {
                warn!("timed out while eagerly initializing guardian review session");
            }
            Err(GuardianReviewSessionSpawnOutcome::Aborted) => {}
        }
    }

    /// Returns the shared guardian trunk to consider for this review, spawning one if needed.
    ///
    /// The returned trunk may still have a stale reuse key when an older trunk is busy; callers
    /// handle that by forking instead of trying to reuse it.
    async fn get_or_spawn_trunk_for_review(
        &self,
        params: &GuardianReviewSessionParams,
        deadline: tokio::time::Instant,
        external_cancel: Option<&CancellationToken>,
    ) -> Result<Option<Arc<GuardianReviewSession>>, GuardianReviewSessionOutcome> {
        match self.prepare_trunk(&params.spawn_config).await {
            GuardianTrunkState::Ready(trunk) => return Ok(Some(trunk)),
            GuardianTrunkState::ShutdownStarted => return Ok(None),
            GuardianTrunkState::NeedsSpawn => {}
        }

        let spawn_guard =
            match run_before_review_deadline(deadline, external_cancel, self.spawn_lock.lock())
                .await
            {
                Ok(spawn_guard) => spawn_guard,
                Err(outcome) => return Err(outcome),
            };

        // Another task may have finished spawning while we were waiting on `spawn_lock`.
        let trunk = match self.prepare_trunk(&params.spawn_config).await {
            GuardianTrunkState::Ready(trunk) => Some(trunk),
            GuardianTrunkState::ShutdownStarted => None,
            GuardianTrunkState::NeedsSpawn => {
                match self
                    .spawn_and_install_trunk(
                        deadline,
                        external_cancel,
                        &params.parent_session,
                        &params.parent_turn,
                        params.spawn_config.clone(),
                    )
                    .await
                {
                    Ok(trunk) => trunk,
                    Err(spawn_outcome) => {
                        drop(spawn_guard);
                        return Err(review_outcome_from_spawn_outcome(spawn_outcome));
                    }
                }
            }
        };
        drop(spawn_guard);
        Ok(trunk)
    }

    async fn spawn_and_install_trunk(
        &self,
        deadline: tokio::time::Instant,
        external_cancel: Option<&CancellationToken>,
        parent_session: &Arc<Session>,
        parent_turn: &Arc<TurnContext>,
        spawn_config: Config,
    ) -> Result<Option<Arc<GuardianReviewSession>>, GuardianReviewSessionSpawnOutcome> {
        // Spawn under the caller's deadline policy first, then register the result against shared
        // trunk state exactly once in `install_spawned_trunk`.
        let review_session = spawn_review_session_before_deadline(
            deadline,
            external_cancel,
            parent_session,
            parent_turn,
            spawn_config,
            /*initial_history*/ None,
        )
        .await?;
        Ok(self.install_spawned_trunk(review_session).await)
    }

    /// Inspects the cached trunk and eagerly evicts a stale idle trunk so the caller can spawn a
    /// replacement. Busy trunks are left in place.
    async fn prepare_trunk(&self, next_spawn_config: &Config) -> GuardianTrunkState {
        let (trunk_state, stale_trunk_to_shutdown) = {
            let mut state = self.state.lock().await;
            if state.shutdown_started {
                return GuardianTrunkState::ShutdownStarted;
            }
            if let Some(trunk) = state.trunk.as_ref()
                && trunk.spawn_config != *next_spawn_config
                && trunk.review_lock.try_lock().is_ok()
            {
                (GuardianTrunkState::NeedsSpawn, state.trunk.take())
            } else if let Some(trunk) = state.trunk.as_ref() {
                (GuardianTrunkState::Ready(Arc::clone(trunk)), None)
            } else {
                (GuardianTrunkState::NeedsSpawn, None)
            }
        };
        if let Some(review_session) = stale_trunk_to_shutdown {
            review_session.shutdown_in_background();
        }
        trunk_state
    }

    /// Returns true only when eager init should try to spawn a trunk now.
    ///
    /// Eager init is strictly best-effort: if any trunk already exists, or shutdown has started,
    /// the caller should do nothing and let the real review path handle trunk reuse/replacement.
    async fn should_spawn_trunk_eagerly(&self) -> bool {
        let state = self.state.lock().await;
        if state.shutdown_started {
            return false;
        }
        state.trunk.is_none()
    }

    async fn install_spawned_trunk(
        &self,
        review_session: Arc<GuardianReviewSession>,
    ) -> Option<Arc<GuardianReviewSession>> {
        let mut state = self.state.lock().await;
        if state.shutdown_started {
            drop(state);
            review_session.shutdown().await;
            return None;
        }
        if let Some(trunk) = state.trunk.as_ref() {
            // Another task installed the trunk while this spawn was in flight, so prefer the
            // already-cached trunk and retire the newly spawned duplicate in the background.
            let trunk = Arc::clone(trunk);
            drop(state);
            review_session.shutdown_in_background();
            return Some(trunk);
        }
        state.trunk = Some(Arc::clone(&review_session));
        Some(review_session)
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

    async fn register_active_fork(&self, review_session: Arc<GuardianReviewSession>) -> bool {
        let mut state = self.state.lock().await;
        if state.shutdown_started {
            return false;
        }
        state.active_forks.push(review_session);
        true
    }

    async fn take_active_fork(
        &self,
        review_session: &Arc<GuardianReviewSession>,
    ) -> Option<Arc<GuardianReviewSession>> {
        let mut state = self.state.lock().await;
        take_review_session(&mut state.active_forks, review_session)
    }

    #[cfg(test)]
    pub(crate) async fn cache_for_test(&self, codex: Codex) {
        let spawn_config = codex.session.get_config().await.as_ref().clone();
        self.state.lock().await.trunk = Some(Arc::new(GuardianReviewSession::new(
            codex,
            CancellationToken::new(),
            spawn_config,
            /*has_prior_review*/ false,
        )));
    }

    #[cfg(test)]
    pub(crate) async fn register_fork_for_test(&self, codex: Codex) {
        let spawn_config = codex.session.get_config().await.as_ref().clone();
        self.state
            .lock()
            .await
            .active_forks
            .push(Arc::new(GuardianReviewSession::new(
                codex,
                CancellationToken::new(),
                spawn_config,
                /*has_prior_review*/ false,
            )));
    }

    async fn run_forked_review(
        &self,
        params: GuardianReviewSessionParams,
        deadline: tokio::time::Instant,
        initial_history: Option<InitialHistory>,
    ) -> GuardianReviewSessionOutcome {
        // Forks never become the cached trunk. They exist only to let a parallel approval proceed
        // without waiting on a busy trunk session.
        let mut fork_config = params.spawn_config.clone();
        fork_config.ephemeral = true;
        let review_session = match spawn_review_session_before_deadline(
            deadline,
            params.external_cancel.as_ref(),
            &params.parent_session,
            &params.parent_turn,
            fork_config,
            initial_history,
        )
        .await
        {
            Ok(review_session) => review_session,
            Err(spawn_outcome) => return review_outcome_from_spawn_outcome(spawn_outcome),
        };
        if !self.register_active_fork(Arc::clone(&review_session)).await {
            review_session.shutdown_in_background();
            return GuardianReviewSessionOutcome::Aborted;
        }
        let mut cleanup =
            ForkReviewCleanup::new(Arc::clone(&self.state), Arc::clone(&review_session));

        let execution_result =
            run_review_on_session(review_session.as_ref(), &params, deadline).await;
        let review_session = self.take_active_fork(&review_session).await;
        if let Some(review_session) = review_session {
            cleanup.disarm();
            review_session.shutdown_in_background();
        }
        execution_result.outcome
    }
}

/// Removes a tracked fork session by pointer identity.
///
/// Both the normal fork cleanup path and the RAII drop path use this helper so the vector-removal
/// logic stays in one place.
fn take_review_session(
    active_sessions: &mut Vec<Arc<GuardianReviewSession>>,
    review_session: &Arc<GuardianReviewSession>,
) -> Option<Arc<GuardianReviewSession>> {
    let index = active_sessions
        .iter()
        .position(|active_review| Arc::ptr_eq(active_review, review_session))?;
    Some(active_sessions.swap_remove(index))
}
