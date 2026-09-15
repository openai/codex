//! Counts policy denials per turn; the host carries out any requested interruption.

const MAX_CONSECUTIVE_CYBER_GUARDIAN_DENIALS_PER_TURN: u32 = 1;
const MAX_CONSECUTIVE_GUARDIAN_DENIALS_PER_TURN: u32 = 3;
const MAX_RECENT_CYBER_AUTO_REVIEW_DENIALS_PER_TURN: u32 = 1;
const MAX_RECENT_AUTO_REVIEW_DENIALS_PER_TURN: u32 = 10;
pub(crate) const AUTO_REVIEW_DENIAL_WINDOW_SIZE: usize = 50;

#[derive(Debug, Default)]
pub(crate) struct GuardianRejectionCircuitBreaker {
    turns: std::collections::HashMap<String, GuardianRejectionCircuitBreakerTurn>,
}

#[derive(Debug, Default)]
struct GuardianRejectionCircuitBreakerTurn {
    consecutive_denials: u32,
    recent_denials: std::collections::VecDeque<bool>,
    interrupt_triggered: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GuardianRejectionCircuitBreakerPolicy {
    Standard,
    CyberModel,
}

impl From<&codex_protocol::openai_models::ModelInfo> for GuardianRejectionCircuitBreakerPolicy {
    fn from(model: &codex_protocol::openai_models::ModelInfo) -> Self {
        if model.model_specialty.as_deref()
            == Some(codex_protocol::openai_models::MODEL_SPECIALTY_CYBER)
        {
            Self::CyberModel
        } else {
            Self::Standard
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GuardianRejectionCircuitBreakerAction {
    Continue,
    InterruptTurn {
        consecutive_denials: u32,
        recent_denials: u32,
    },
}

impl GuardianRejectionCircuitBreaker {
    pub(crate) fn clear_turn(&mut self, turn_id: &str) {
        self.turns.remove(turn_id);
    }

    pub(crate) fn record_denial(
        &mut self,
        turn_id: &str,
        policy: GuardianRejectionCircuitBreakerPolicy,
    ) -> GuardianRejectionCircuitBreakerAction {
        let turn = self.turns.entry(turn_id.to_string()).or_default();
        turn.consecutive_denials = turn.consecutive_denials.saturating_add(1);
        Self::record_recent_review(turn, /*denied*/ true);
        let recent_denials = turn.recent_denials.iter().filter(|denied| **denied).count() as u32;
        let (max_consecutive_denials, max_recent_denials) = match policy {
            GuardianRejectionCircuitBreakerPolicy::Standard => (
                MAX_CONSECUTIVE_GUARDIAN_DENIALS_PER_TURN,
                MAX_RECENT_AUTO_REVIEW_DENIALS_PER_TURN,
            ),
            GuardianRejectionCircuitBreakerPolicy::CyberModel => (
                MAX_CONSECUTIVE_CYBER_GUARDIAN_DENIALS_PER_TURN,
                MAX_RECENT_CYBER_AUTO_REVIEW_DENIALS_PER_TURN,
            ),
        };
        if !turn.interrupt_triggered
            && (turn.consecutive_denials >= max_consecutive_denials
                || recent_denials >= max_recent_denials)
        {
            turn.interrupt_triggered = true;
            GuardianRejectionCircuitBreakerAction::InterruptTurn {
                consecutive_denials: turn.consecutive_denials,
                recent_denials,
            }
        } else {
            GuardianRejectionCircuitBreakerAction::Continue
        }
    }

    pub(crate) fn record_non_denial(&mut self, turn_id: &str) {
        let turn = self.turns.entry(turn_id.to_string()).or_default();
        turn.consecutive_denials = 0;
        Self::record_recent_review(turn, /*denied*/ false);
    }

    fn record_recent_review(turn: &mut GuardianRejectionCircuitBreakerTurn, denied: bool) {
        turn.recent_denials.push_back(denied);
        if turn.recent_denials.len() > AUTO_REVIEW_DENIAL_WINDOW_SIZE {
            turn.recent_denials.pop_front();
        }
    }
}

#[cfg(test)]
#[path = "circuit_breaker_tests.rs"]
mod tests;
