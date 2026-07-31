use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::MutexGuard;
use std::sync::PoisonError;

use codex_protocol::ThreadId;
use rmcp::model::RequestId;

/// Tracks active MCP requests by both request ID and Codex turn.
///
/// Warning and response enqueue callbacks run while holding the same lock. This
/// makes route removal and response enqueue atomic with respect to warning
/// enqueue, so a warning is either queued before its tool response or dropped.
#[derive(Default)]
pub(crate) struct ActiveTurnRegistry {
    state: Mutex<ActiveTurnState>,
}

#[derive(Default)]
struct ActiveTurnState {
    by_request: HashMap<RequestId, ActiveTurn>,
    by_thread_and_turn: HashMap<(ThreadId, String), RequestId>,
}

struct ActiveTurn {
    thread_id: ThreadId,
    turn_id: String,
}

impl ActiveTurnRegistry {
    pub(crate) fn register(&self, request_id: RequestId, thread_id: ThreadId, turn_id: String) {
        let active_turn = ActiveTurn {
            thread_id,
            turn_id: turn_id.clone(),
        };
        let mut state = self.lock();
        if let Some(previous_turn) = state.by_request.insert(request_id.clone(), active_turn) {
            state
                .by_thread_and_turn
                .remove(&(previous_turn.thread_id, previous_turn.turn_id));
        }
        state
            .by_thread_and_turn
            .insert((thread_id, turn_id), request_id);
    }

    pub(crate) fn thread_id(&self, request_id: &RequestId) -> Option<ThreadId> {
        self.lock()
            .by_request
            .get(request_id)
            .map(|active_turn| active_turn.thread_id)
    }

    pub(crate) fn with_request_id(
        &self,
        thread_id: ThreadId,
        turn_id: &str,
        f: impl FnOnce(&RequestId),
    ) -> bool {
        let state = self.lock();
        let Some(request_id) = state
            .by_thread_and_turn
            .get(&(thread_id, turn_id.to_string()))
        else {
            return false;
        };
        f(request_id);
        true
    }

    pub(crate) fn unregister(&self, request_id: &RequestId) {
        let mut state = self.lock();
        Self::remove(&mut state, request_id);
    }

    pub(crate) fn finish(&self, request_id: &RequestId, finish: impl FnOnce()) {
        let mut state = self.lock();
        Self::remove(&mut state, request_id);
        finish();
    }

    fn remove(state: &mut ActiveTurnState, request_id: &RequestId) {
        if let Some(active_turn) = state.by_request.remove(request_id) {
            state
                .by_thread_and_turn
                .remove(&(active_turn.thread_id, active_turn.turn_id));
        }
    }

    fn lock(&self) -> MutexGuard<'_, ActiveTurnState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[cfg(test)]
#[path = "active_turn_registry_tests.rs"]
mod tests;
