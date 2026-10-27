//! Holds the current speculative request, polled at checkpoints or awaited when compaction is due.
//! Discarded requests still charge completed usage; session shutdown cancels remaining network work.
//! Snapshot capture and launch registration hold session state; prompt preparation does not.

use std::sync::Arc;

use crate::compact_remote_v2::BackgroundCompactionCandidate;
use crate::compact_remote_v2::capture_background_input;
use crate::compact_remote_v2::run_background_compaction;
use crate::context::world_state::WorldState;
use crate::session::context_window::ContextWindowTokenStatus;
use crate::session::session::Session;
use crate::session::step_context::StepContext;
use crate::session::turn_context::TurnContext;
use codex_analytics::CompactionPhase;
use codex_features::Feature;
use codex_model_provider::RemoteCompactionSupport;
use codex_otel::auth_storage::AuthStorageOriginator;
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;
use tracing::debug;

#[derive(PartialEq, Eq)]
struct CompactionAttempt {
    history_version: u64,
    model_slug: String,
    compaction_hash: Option<String>,
    auth_owner_generation: Option<u64>,
}

#[derive(Default)]
pub(crate) struct AsyncCompactionState {
    last_attempt: Option<CompactionAttempt>,
    // Keep prepared request state off the turn stack while checkpoints await adoption.
    result: Option<oneshot::Receiver<Box<BackgroundCompactionCandidate>>>,
    tasks: TaskTracker,
    shutdown_cancellation: CancellationToken,
}

pub(super) fn enabled(turn_context: &TurnContext) -> bool {
    turn_context
        .config
        .features
        .enabled(Feature::AsyncCompaction)
        && !turn_context.config.features.enabled(Feature::TokenBudget)
        && matches!(
            turn_context.provider.capabilities().remote_compaction,
            RemoteCompactionSupport::V2
        )
}

pub(super) async fn maybe_start(
    sess: &Arc<Session>,
    step_context: &Arc<StepContext>,
    world_state: &Arc<WorldState>,
    phase: CompactionPhase,
    turn_cancellation: &CancellationToken,
    token_status: &ContextWindowTokenStatus,
    (turn_state, auth_owner_generation): (Option<String>, Option<u64>),
) {
    let turn_context = &step_context.turn;
    // Let the foreground establish the backend's model pin before starting a parallel request.
    if matches!(phase, CompactionPhase::PreTurn) && turn_state.is_none() {
        return;
    }
    if !enabled(turn_context)
        || token_status.token_limit_reached
        || (matches!(phase, CompactionPhase::PostTurn)
            && token_status.turn_end_compaction_threshold_reached)
        || step_context
            .settings
            .model_info
            .resolved_context_window()
            .is_none_or(|limit| {
                i128::from(token_status.active_context_tokens) * 100
                    < i128::from(limit)
                        * i128::from(turn_context.config.async_compaction_threshold_percent)
            })
    {
        return;
    }

    let base_instructions = sess.get_prompt_base_instructions().await;
    let (prepared_input, tasks, shutdown_cancellation, launch, send) = {
        let mut state = sess.state.lock().await;
        let model_info = &step_context.settings.model_info;
        let attempt = CompactionAttempt {
            history_version: state.history.history_version(),
            model_slug: model_info.slug.clone(),
            compaction_hash: model_info.comp_hash.clone(),
            auth_owner_generation,
        };
        // Stop blocks new launches; already-running compaction remains session-owned.
        if turn_cancellation.is_cancelled()
            || state.shutting_down
            || state.async_compaction.last_attempt.as_ref() == Some(&attempt)
            || state.history.annotated_items().is_empty()
            || sess.services.model_client.auth_owner_generation() != auth_owner_generation
        {
            return;
        }

        // Capture the arguments now; the async preparation body runs after the lock is released.
        let prepared_input = capture_background_input(
            sess,
            step_context,
            state.history.clone(),
            base_instructions,
            turn_state,
            auth_owner_generation,
            sess.current_window_from_state(&state),
        );
        let tasks = state.async_compaction.tasks.clone();
        let shutdown_cancellation = state.async_compaction.shutdown_cancellation.clone();
        // Register before unlocking so shutdown cannot finish between capture and spawning.
        let launch = tasks.token();
        state.async_compaction.last_attempt = Some(attempt);
        let (send, result) = oneshot::channel();
        state.async_compaction.result = Some(result);
        (prepared_input, tasks, shutdown_cancellation, launch, send)
    };

    // Prepare outside the lock on the foreground task so its step cannot advance before tool
    // attribution is captured. Keep normalization before attribution capture.
    let prepared_input = prepared_input.await;
    let task_session = Arc::clone(sess);
    let task_step = Arc::clone(step_context);
    let cutoff_world_state = Arc::clone(world_state);
    let storage_originator = AuthStorageOriginator::from_client_name(&turn_context.originator);
    let task = async move {
        match run_background_compaction(
            &task_session,
            task_step,
            cutoff_world_state,
            phase,
            prepared_input,
            &shutdown_cancellation,
        )
        .await
        {
            Ok(candidate) => {
                let _ = send.send(Box::new(candidate));
            }
            Err(error) => {
                debug!(%error, "discarding unsuccessful background compaction");
            }
        }
    };
    tasks.spawn(storage_originator.scope(task));
    drop(launch);
}

pub(super) async fn take_ready(
    sess: &Session,
    turn_context: &TurnContext,
) -> Option<Box<BackgroundCompactionCandidate>> {
    if !enabled(turn_context) {
        return None;
    }
    let mut state = sess.state.lock().await;
    let mut result = state.async_compaction.result.take()?;
    let candidate = match result.try_recv() {
        Ok(candidate) => candidate,
        Err(oneshot::error::TryRecvError::Empty) => {
            state.async_compaction.result = Some(result);
            return None;
        }
        // try_recv consumes a closed receiver; awaiting it later would panic.
        Err(oneshot::error::TryRecvError::Closed) => return None,
    };
    (candidate.prefix.generation == state.history.history_version()).then_some(candidate)
}

pub(crate) async fn wait_for_pending(
    sess: &Session,
    step_context: &StepContext,
) -> Option<Box<BackgroundCompactionCandidate>> {
    if !enabled(&step_context.turn) {
        return None;
    }
    let result = {
        let mut state = sess.state.lock().await;
        let result = state.async_compaction.result.take()?;
        let model = &step_context.settings.model_info;
        let attempt = state.async_compaction.last_attempt.as_ref()?;
        if attempt.history_version != state.history.history_version()
            || attempt.model_slug != model.slug
            || attempt.compaction_hash != model.comp_hash
            || attempt.auth_owner_generation != sess.services.model_client.auth_owner_generation()
        {
            return None;
        }
        result
    };
    // Release session state while the request finishes and charges its usage.
    result.await.ok()
}

pub(crate) async fn shutdown_cancellation(sess: &Session) -> CancellationToken {
    sess.state
        .lock()
        .await
        .async_compaction
        .shutdown_cancellation
        .clone()
}

pub(super) async fn shutdown(sess: &Session) {
    let tasks = {
        let mut state = sess.state.lock().await;
        let async_compaction = &mut state.async_compaction;
        async_compaction.result = None;
        async_compaction.tasks.close();
        async_compaction.shutdown_cancellation.cancel();
        async_compaction.tasks.clone()
    };
    // Drain completed usage charging before shutdown finishes.
    tasks.wait().await;
}

/// Test barrier: callers must gate foreground work so it cannot launch another attempt.
pub(crate) async fn wait_for_background_compaction_for_test(sess: &Session) {
    let tasks = sess.state.lock().await.async_compaction.tasks.clone();
    // Closing only enables waiting; it neither cancels tasks nor prevents future launches.
    tasks.close();
    tasks.wait().await;
}

#[cfg(test)]
#[path = "async_compaction_tests.rs"]
mod tests;
