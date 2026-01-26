use std::sync::Arc;

use codex_protocol::ThreadId;

use crate::codex::Session;

pub(crate) async fn begin_collab_wait(session: &Session, turn_id: &str, agent_ids: &[ThreadId]) {
    let turn_state = {
        let active = session.active_turn.lock().await;
        active
            .as_ref()
            .map(|active_turn| Arc::clone(&active_turn.turn_state))
    };
    if let Some(turn_state) = turn_state {
        let mut state = turn_state.lock().await;
        state.begin_wait(turn_id, agent_ids);
    }
}

pub(crate) async fn end_collab_wait(session: &Session, turn_id: &str, agent_ids: &[ThreadId]) {
    let turn_state = {
        let active = session.active_turn.lock().await;
        active
            .as_ref()
            .map(|active_turn| Arc::clone(&active_turn.turn_state))
    };
    if let Some(turn_state) = turn_state {
        let mut state = turn_state.lock().await;
        state.end_wait(turn_id, agent_ids);
    }
}

pub(crate) async fn mark_collab_wait_collected(
    session: &Session,
    turn_id: &str,
    agent_ids: &[ThreadId],
) {
    let turn_state = {
        let active = session.active_turn.lock().await;
        active
            .as_ref()
            .map(|active_turn| Arc::clone(&active_turn.turn_state))
    };
    if let Some(turn_state) = turn_state {
        let mut state = turn_state.lock().await;
        state.mark_wait_collected(turn_id, agent_ids);
    }
}

pub(crate) async fn is_collab_wait_suppressed(
    session: &Session,
    turn_id: &str,
    agent_id: ThreadId,
) -> bool {
    let turn_state = {
        let active = session.active_turn.lock().await;
        active
            .as_ref()
            .map(|active_turn| Arc::clone(&active_turn.turn_state))
    };
    if let Some(turn_state) = turn_state {
        let state = turn_state.lock().await;
        state.is_waiting_on(turn_id, agent_id) || state.is_wait_collected(turn_id, agent_id)
    } else {
        false
    }
}
