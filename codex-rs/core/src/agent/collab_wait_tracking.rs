use codex_protocol::ThreadId;

use crate::codex::Session;

pub(crate) async fn begin_collab_wait(session: &Session, turn_id: &str, agent_ids: &[ThreadId]) {
    let active = session.active_turn.lock().await;
    if let Some(active_turn) = active.as_ref() {
        let mut state = active_turn.turn_state.lock().await;
        state.begin_wait(turn_id, agent_ids);
    }
}

pub(crate) async fn end_collab_wait(session: &Session, turn_id: &str, agent_ids: &[ThreadId]) {
    let active = session.active_turn.lock().await;
    if let Some(active_turn) = active.as_ref() {
        let mut state = active_turn.turn_state.lock().await;
        state.end_wait(turn_id, agent_ids);
    }
}

pub(crate) async fn is_collab_wait_active(
    session: &Session,
    turn_id: &str,
    agent_id: ThreadId,
) -> bool {
    let active = session.active_turn.lock().await;
    if let Some(active_turn) = active.as_ref() {
        let state = active_turn.turn_state.lock().await;
        return state.is_waiting_on(turn_id, agent_id);
    }
    false
}
