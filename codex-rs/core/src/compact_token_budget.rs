use std::sync::Arc;

use crate::compact::InitialContextInjection;
use crate::context::world_state::WorldState;
use crate::hook_runtime::PostCompactHookOutcome;
use crate::hook_runtime::PreCompactHookOutcome;
use crate::hook_runtime::run_post_compact_hooks;
use crate::hook_runtime::run_pre_compact_hooks;
use crate::session::session::Session;
use crate::session::step_context::StepContext;
use crate::session::turn_context::TurnContext;
use codex_analytics::CompactionTrigger;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result as CodexResult;
use codex_protocol::items::ContextCompactionItem;
use codex_protocol::items::TurnItem;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::TurnStartedEvent;

/// Runs token-budget manual compaction as a normal compaction lifecycle.
///
/// Token-budget compaction skips model/server summarization and installs a fresh context window
/// instead. It is still modeled as compaction so compact hooks and `ContextCompaction` turn items
/// observe the same lifecycle as local or remote compaction.
pub(crate) async fn run_manual_compact_task(
    sess: Arc<Session>,
    turn_context: Arc<TurnContext>,
) -> CodexResult<()> {
    let start_event = EventMsg::TurnStarted(TurnStartedEvent {
        turn_id: turn_context.sub_id.clone(),
        trace_id: turn_context.trace_id.clone(),
        started_at: turn_context.turn_timing_state.started_at_unix_secs().await,
        model_context_window: turn_context.model_context_window(),
        collaboration_mode_kind: turn_context.mode,
    });
    sess.send_event(&turn_context, start_event).await;

    // Manual compaction runs outside run_turn, so it captures its own current step.
    let step_context = sess.capture_step_context(Arc::clone(&turn_context)).await;
    let world_state = Arc::new(sess.build_world_state_for_step(&step_context).await);
    run_compact_task_inner(&sess, &turn_context, world_state, CompactionTrigger::Manual).await
}

/// Runs token-budget inline auto-compaction as a normal compaction lifecycle.
///
/// Token-budget compaction skips model/server summarization and installs a fresh context window
/// instead. It is still modeled as compaction so compact hooks and `ContextCompaction` turn items
/// observe the same lifecycle as local or remote compaction.
pub(crate) async fn run_inline_auto_compact_task(
    sess: Arc<Session>,
    step_context: Arc<StepContext>,
    initial_context_injection: InitialContextInjection,
) -> CodexResult<()> {
    let turn_context = &step_context.turn;
    let world_state = match initial_context_injection {
        InitialContextInjection::BeforeLastUserMessage(world_state) => world_state,
        InitialContextInjection::DoNotInject => {
            Arc::new(sess.build_world_state_for_step(&step_context).await)
        }
    };
    run_compact_task_inner(&sess, turn_context, world_state, CompactionTrigger::Auto).await
}

async fn run_compact_task_inner(
    sess: &Arc<Session>,
    turn_context: &Arc<TurnContext>,
    world_state: Arc<WorldState>,
    trigger: CompactionTrigger,
) -> CodexResult<()> {
    let pre_compact_outcome = run_pre_compact_hooks(sess, turn_context, trigger).await;
    match pre_compact_outcome {
        PreCompactHookOutcome::Continue => {}
        PreCompactHookOutcome::Stopped => return Err(CodexErr::TurnAborted),
    }

    let compaction_item = TurnItem::ContextCompaction(ContextCompactionItem::new());
    sess.emit_turn_item_started(turn_context, &compaction_item)
        .await;
    sess.start_new_context_window(turn_context.as_ref(), world_state)
        .await;
    sess.emit_turn_item_completed(turn_context, compaction_item)
        .await;

    let post_compact_outcome = run_post_compact_hooks(sess, turn_context, trigger).await;
    if let PostCompactHookOutcome::Stopped = post_compact_outcome {
        return Err(CodexErr::TurnAborted);
    }

    Ok(())
}
