use super::*;
use codex_protocol::approvals::ExecPolicyAmendment;

fn pending_approval(accepted_decisions: Vec<ReviewDecision>) -> PendingApproval {
    let (tx_approve, _rx_approve) = oneshot::channel();
    PendingApproval {
        tx_approve,
        accepted_decisions,
    }
}

#[test]
fn one_shot_pending_approval_rejects_unoffered_persistent_decisions() {
    let pending = pending_approval(vec![ReviewDecision::Approved, ReviewDecision::Abort]);
    let amendment = ReviewDecision::ApprovedExecpolicyAmendment {
        proposed_execpolicy_amendment: ExecPolicyAmendment::new(vec!["echo".to_string()]),
    };

    assert!(pending.accepts(&ReviewDecision::Approved));
    assert!(pending.accepts(&ReviewDecision::Denied));
    assert!(!pending.accepts(&ReviewDecision::ApprovedForSession));
    assert!(!pending.accepts(&amendment));
}

#[test]
fn cacheable_pending_approval_keeps_session_decision_compatibility() {
    let pending = pending_approval(vec![
        ReviewDecision::Approved,
        ReviewDecision::ApprovedForSession,
        ReviewDecision::Abort,
    ]);

    assert!(pending.accepts(&ReviewDecision::ApprovedForSession));
}

#[test]
fn pending_approval_requires_the_exact_offered_amendment() {
    let offered = ReviewDecision::ApprovedExecpolicyAmendment {
        proposed_execpolicy_amendment: ExecPolicyAmendment::new(vec!["echo".to_string()]),
    };
    let substituted = ReviewDecision::ApprovedExecpolicyAmendment {
        proposed_execpolicy_amendment: ExecPolicyAmendment::new(vec!["rm".to_string()]),
    };
    let pending = pending_approval(vec![
        ReviewDecision::Approved,
        offered.clone(),
        ReviewDecision::Abort,
    ]);

    assert!(pending.accepts(&offered));
    assert!(!pending.accepts(&substituted));
    assert!(!pending.accepts(&ReviewDecision::ApprovedForSession));
}

#[test]
fn pending_approvals_with_the_same_id_are_isolated_by_request_kind() {
    let (tx_patch, _rx_patch) = oneshot::channel();
    let (tx_exec, _rx_exec) = oneshot::channel();
    let mut turn_state = TurnState::default();
    turn_state.insert_pending_approval(
        "approval".to_string(),
        tx_patch,
        PendingApprovalKind::Patch,
        vec![ReviewDecision::Approved],
    );
    turn_state.insert_pending_approval(
        "approval".to_string(),
        tx_exec,
        PendingApprovalKind::Exec,
        vec![ReviewDecision::Approved],
    );

    assert!(
        turn_state
            .remove_pending_approval("approval", PendingApprovalKind::Exec)
            .is_some()
    );
    assert!(
        turn_state
            .remove_pending_approval("approval", PendingApprovalKind::Patch)
            .is_some()
    );
}

#[tokio::test]
async fn consumed_approval_id_cannot_resolve_a_later_generation() {
    for stale_decision in [
        ReviewDecision::Approved,
        ReviewDecision::Abort,
        ReviewDecision::Denied,
    ] {
        let mut turn_state = TurnState::default();
        let (initial_tx, initial_rx) = oneshot::channel();
        turn_state.insert_pending_approval(
            "call-id".to_string(),
            initial_tx,
            PendingApprovalKind::Exec,
            vec![ReviewDecision::Approved],
        );
        turn_state
            .remove_pending_approval("call-id", PendingApprovalKind::Exec)
            .expect("initial waiter")
            .send(ReviewDecision::Approved);
        assert_eq!(initial_rx.await, Ok(ReviewDecision::Approved));

        let (retry_tx, retry_rx) = oneshot::channel();
        turn_state.insert_pending_approval(
            "retry-id".to_string(),
            retry_tx,
            PendingApprovalKind::Exec,
            vec![ReviewDecision::Approved],
        );

        assert!(
            turn_state
                .remove_pending_approval("call-id", PendingApprovalKind::Exec)
                .is_none(),
            "stale {stale_decision:?} must not consume the retry waiter"
        );
        turn_state
            .remove_pending_approval("retry-id", PendingApprovalKind::Exec)
            .expect("retry waiter remains pending")
            .send(ReviewDecision::Approved);
        assert_eq!(retry_rx.await, Ok(ReviewDecision::Approved));
    }
}
