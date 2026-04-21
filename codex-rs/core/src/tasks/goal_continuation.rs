//! Scheduling glue for active-goal continuation turns.

use std::sync::Arc;

use tracing::warn;

use super::RegularTask;
use crate::session::session::Session;
use crate::state::ActiveTurn;

pub(super) async fn maybe_start_turn(session: &Arc<Session>) {
    let Ok(_continuation_guard) = session.goal_runtime.continuation_lock.acquire().await else {
        warn!("goal continuation semaphore closed");
        return;
    };
    let Some(items) = session.goal_continuation_items_if_active().await else {
        return;
    };

    let turn_state = {
        let mut active_turn = session.active_turn.lock().await;
        if active_turn.is_some() {
            return;
        }
        let active_turn = active_turn.get_or_insert_with(ActiveTurn::default);
        Arc::clone(&active_turn.turn_state)
    };
    {
        let mut turn_state = turn_state.lock().await;
        for item in items {
            turn_state.push_pending_input(item);
        }
    }

    let turn_context = session
        .new_default_turn_with_sub_id(uuid::Uuid::new_v4().to_string())
        .await;
    session
        .maybe_emit_unknown_model_warning_for_turn(turn_context.as_ref())
        .await;
    session
        .mark_thread_goal_continuation_turn_started(turn_context.sub_id.clone())
        .await;
    session
        .start_task(turn_context, Vec::new(), RegularTask::new())
        .await;
}
