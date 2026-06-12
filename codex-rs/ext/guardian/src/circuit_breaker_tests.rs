use std::collections::VecDeque;

use pretty_assertions::assert_eq;

use super::*;

#[test]
fn interrupts_once_after_three_consecutive_denials() {
    let mut circuit_breaker = GuardianRejectionCircuitBreaker::default();
    assert_eq!(
        circuit_breaker.record_denial(),
        GuardianRejectionCircuitBreakerAction::Continue
    );
    assert_eq!(
        circuit_breaker.record_denial(),
        GuardianRejectionCircuitBreakerAction::Continue
    );
    assert_eq!(
        circuit_breaker.record_denial(),
        GuardianRejectionCircuitBreakerAction::InterruptTurn {
            consecutive_denials: 3,
            recent_denials: 3,
        }
    );
    assert_eq!(
        circuit_breaker.record_denial(),
        GuardianRejectionCircuitBreakerAction::Continue
    );
    assert_eq!(
        circuit_breaker,
        GuardianRejectionCircuitBreaker {
            consecutive_denials: 4,
            recent_denials: VecDeque::from([true, true, true, true]),
            interrupt_triggered: true,
        }
    );
}

#[test]
fn non_denial_resets_consecutive_denials() {
    let mut circuit_breaker = GuardianRejectionCircuitBreaker::default();
    assert_eq!(
        circuit_breaker.record_denial(),
        GuardianRejectionCircuitBreakerAction::Continue
    );
    circuit_breaker.record_non_denial();
    assert_eq!(
        circuit_breaker.record_denial(),
        GuardianRejectionCircuitBreakerAction::Continue
    );
    assert_eq!(
        circuit_breaker.record_denial(),
        GuardianRejectionCircuitBreakerAction::Continue
    );
    assert_eq!(
        circuit_breaker.record_denial(),
        GuardianRejectionCircuitBreakerAction::InterruptTurn {
            consecutive_denials: 3,
            recent_denials: 4,
        }
    );
}

#[test]
fn interrupts_after_ten_recent_denials() {
    let mut circuit_breaker = GuardianRejectionCircuitBreaker::default();
    for _ in 0..9 {
        assert_eq!(
            circuit_breaker.record_denial(),
            GuardianRejectionCircuitBreakerAction::Continue
        );
        circuit_breaker.record_non_denial();
    }
    assert_eq!(
        circuit_breaker.record_denial(),
        GuardianRejectionCircuitBreakerAction::InterruptTurn {
            consecutive_denials: 1,
            recent_denials: 10,
        }
    );
}

#[test]
fn forgets_denials_outside_recent_review_window() {
    let mut circuit_breaker = GuardianRejectionCircuitBreaker::default();
    for _ in 0..9 {
        assert_eq!(
            circuit_breaker.record_denial(),
            GuardianRejectionCircuitBreakerAction::Continue
        );
        circuit_breaker.record_non_denial();
    }
    for _ in 0..(AUTO_REVIEW_DENIAL_WINDOW_SIZE - 18) {
        circuit_breaker.record_non_denial();
    }
    assert_eq!(
        circuit_breaker.record_denial(),
        GuardianRejectionCircuitBreakerAction::Continue
    );
}
