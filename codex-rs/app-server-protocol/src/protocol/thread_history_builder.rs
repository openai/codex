use crate::protocol::thread_history::ThreadHistoryState;
use crate::protocol::v2::Turn;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::RolloutItem;

/// Convert persisted [`RolloutItem`] entries into a sequence of [`Turn`] values.
///
/// When available, this uses `TurnContext.turn_id` as the canonical turn id so
/// resumed/rebuilt thread history preserves the original turn identifiers.
pub fn build_turns_from_rollout_items(items: &[RolloutItem]) -> Vec<Turn> {
    let mut state = ThreadHistoryState::new();
    state.handle_rollout_items(items);
    state.finish()
}

/// Full-history compatibility adapter over the event-to-turn reducer.
pub struct ThreadHistoryBuilder {
    state: ThreadHistoryState,
}

impl Default for ThreadHistoryBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ThreadHistoryBuilder {
    pub fn new() -> Self {
        Self {
            state: ThreadHistoryState::new(),
        }
    }

    pub fn reset(&mut self) {
        self.state = ThreadHistoryState::new();
    }

    pub fn finish(self) -> Vec<Turn> {
        self.state.finish()
    }

    pub fn active_turn_snapshot(&self) -> Option<Turn> {
        self.state.active_turn_snapshot()
    }

    pub fn active_turn_position(&self) -> Option<usize> {
        self.state.active_turn_position()
    }

    pub fn has_active_turn(&self) -> bool {
        self.state.has_active_turn()
    }

    pub fn active_turn_id_if_explicit(&self) -> Option<String> {
        self.state.active_turn_id_if_explicit()
    }

    pub fn active_turn_start_index(&self) -> Option<usize> {
        self.state.active_turn_start_index()
    }

    pub fn handle_event(&mut self, event: &EventMsg) {
        self.state.handle_event(event);
    }

    pub fn handle_rollout_item(&mut self, item: &RolloutItem) {
        self.state.handle_rollout_item(item);
    }
}
