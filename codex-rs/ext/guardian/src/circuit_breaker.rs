use std::collections::VecDeque;

pub const MAX_CONSECUTIVE_GUARDIAN_DENIALS_PER_TURN: u32 = 3;
pub const MAX_RECENT_AUTO_REVIEW_DENIALS_PER_TURN: u32 = 10;
pub const AUTO_REVIEW_DENIAL_WINDOW_SIZE: usize = 50;

/// Per-turn denial history used to stop repeated unsafe approval requests.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct GuardianRejectionCircuitBreaker {
    consecutive_denials: u32,
    recent_denials: VecDeque<bool>,
    interrupt_triggered: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuardianRejectionCircuitBreakerAction {
    Continue,
    InterruptTurn {
        consecutive_denials: u32,
        recent_denials: u32,
    },
}

impl GuardianRejectionCircuitBreaker {
    pub fn record_denial(&mut self) -> GuardianRejectionCircuitBreakerAction {
        self.consecutive_denials = self.consecutive_denials.saturating_add(1);
        self.record_recent_review(/*denied*/ true);
        let recent_denials = self.recent_denials.iter().filter(|denied| **denied).count() as u32;
        if !self.interrupt_triggered
            && (self.consecutive_denials >= MAX_CONSECUTIVE_GUARDIAN_DENIALS_PER_TURN
                || recent_denials >= MAX_RECENT_AUTO_REVIEW_DENIALS_PER_TURN)
        {
            self.interrupt_triggered = true;
            GuardianRejectionCircuitBreakerAction::InterruptTurn {
                consecutive_denials: self.consecutive_denials,
                recent_denials,
            }
        } else {
            GuardianRejectionCircuitBreakerAction::Continue
        }
    }

    pub fn record_non_denial(&mut self) {
        self.consecutive_denials = 0;
        self.record_recent_review(/*denied*/ false);
    }

    fn record_recent_review(&mut self, denied: bool) {
        self.recent_denials.push_back(denied);
        if self.recent_denials.len() > AUTO_REVIEW_DENIAL_WINDOW_SIZE {
            self.recent_denials.pop_front();
        }
    }
}

#[cfg(test)]
#[path = "circuit_breaker_tests.rs"]
mod tests;
