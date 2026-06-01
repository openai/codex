//! Agent-turn lifecycle state for `ChatWidget`.

use std::collections::HashSet;
use std::time::Instant;

use codex_utils_sleep_inhibitor::SleepInhibitor;

#[derive(Debug)]
pub(super) struct TurnLifecycleState {
    pub(super) sleep_inhibitor: SleepInhibitor,
    /// Tracks whether codex-core currently considers an agent turn to be in progress.
    pub(super) agent_turn_running: bool,
    pub(super) last_turn_id: Option<String>,
    pub(super) budget_limited_turn_ids: HashSet<String>,
    pub(super) goal_status_active_turn_started_at: Option<Instant>,
}

impl TurnLifecycleState {
    pub(super) fn new(prevent_idle_sleep: bool) -> Self {
        Self {
            sleep_inhibitor: SleepInhibitor::new(prevent_idle_sleep),
            agent_turn_running: false,
            last_turn_id: None,
            budget_limited_turn_ids: HashSet::new(),
            goal_status_active_turn_started_at: None,
        }
    }

    pub(super) fn start(&mut self, now: Instant) {
        self.agent_turn_running = true;
        self.goal_status_active_turn_started_at = Some(now);
        self.sleep_inhibitor.set_turn_running(/*turn_running*/ true);
    }

    pub(super) fn finish(&mut self) {
        self.agent_turn_running = false;
        self.goal_status_active_turn_started_at = None;
        self.sleep_inhibitor
            .set_turn_running(/*turn_running*/ false);
    }

    pub(super) fn restore_running(&mut self, running: bool, now: Instant) {
        self.agent_turn_running = running;
        self.goal_status_active_turn_started_at = running.then_some(now);
        self.sleep_inhibitor.set_turn_running(running);
    }

    pub(super) fn reset_thread(&mut self) {
        self.finish();
        self.last_turn_id = None;
        self.budget_limited_turn_ids.clear();
    }

    pub(super) fn set_prevent_idle_sleep(&mut self, enabled: bool) {
        self.sleep_inhibitor = SleepInhibitor::new(enabled);
        self.sleep_inhibitor
            .set_turn_running(self.agent_turn_running);
    }

    pub(super) fn mark_budget_limited(&mut self, turn_id: String) {
        self.budget_limited_turn_ids.insert(turn_id);
    }

    pub(super) fn take_budget_limited(&mut self, turn_id: &str) -> bool {
        self.budget_limited_turn_ids.remove(turn_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_and_finish_update_running_state() {
        let mut state = TurnLifecycleState::new(/*prevent_idle_sleep*/ false);

        state.start(Instant::now());
        assert!(state.agent_turn_running);
        assert!(state.goal_status_active_turn_started_at.is_some());
        assert!(state.sleep_inhibitor.is_turn_running());

        state.finish();
        assert!(!state.agent_turn_running);
        assert!(state.goal_status_active_turn_started_at.is_none());
        assert!(!state.sleep_inhibitor.is_turn_running());
    }

    #[test]
    fn budget_limited_turn_ids_are_consumed() {
        let mut state = TurnLifecycleState::new(/*prevent_idle_sleep*/ false);

        state.mark_budget_limited("turn-1".to_string());

        assert!(state.take_budget_limited("turn-1"));
        assert!(!state.take_budget_limited("turn-1"));
    }
}
