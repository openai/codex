use codex_protocol::protocol::GuardianAssessmentAction;
use codex_protocol::protocol::GuardianAssessmentEvent;
use codex_protocol::protocol::GuardianAssessmentStatus;
use pretty_assertions::assert_eq;
use std::sync::Arc;
use tokio::sync::oneshot;
use tokio::time::Instant;

use super::GuardianReviewDrainOutcome;
use super::GuardianReviewTaskOwner;

fn forced_abort_event() -> GuardianAssessmentEvent {
    GuardianAssessmentEvent {
        id: "review-1".to_string(),
        target_item_id: Some("item-1".to_string()),
        turn_id: "turn-1".to_string(),
        started_at_ms: 1,
        completed_at_ms: None,
        status: GuardianAssessmentStatus::Aborted,
        risk_level: None,
        user_authorization: None,
        rationale: None,
        decision_source: None,
        action: GuardianAssessmentAction::McpToolCall {
            server: "test-server".to_string(),
            tool_name: "test-tool".to_string(),
            connector_id: None,
            connector_name: None,
            tool_title: None,
        },
    }
}

#[tokio::test]
async fn dropping_review_handle_leaves_cleanup_to_owner() {
    let owner = Arc::new(GuardianReviewTaskOwner::default());
    let activity = owner.begin().expect("review should start");
    let cancellation_token = owner.cancellation_token();
    let (completed_tx, completed_rx) = oneshot::channel();
    let task = owner
        .spawn(&tokio::runtime::Handle::current(), &activity, async move {
            cancellation_token.cancelled().await;
            let _ = completed_tx.send(());
        })
        .expect("review task should start");

    drop(task);

    assert_eq!(
        owner.close().drain().await,
        GuardianReviewDrainOutcome::Drained
    );
    assert_eq!(completed_rx.await, Ok(()));
}

#[tokio::test]
async fn forced_drain_aborts_review_task() {
    let owner = Arc::new(GuardianReviewTaskOwner::default());
    let activity = owner.begin().expect("review should start");
    let forced_abort = forced_abort_event();
    activity.mark_started(forced_abort.clone());
    let (drop_tx, mut drop_rx) = oneshot::channel::<()>();
    let task = owner
        .spawn(&tokio::runtime::Handle::current(), &activity, async move {
            let _drop_tx = drop_tx;
            std::future::pending::<()>().await;
        })
        .expect("review task should start");
    drop(task);

    let mut drain = owner.close();
    drain.deadline = Instant::now();
    assert_eq!(
        drain.drain().await,
        GuardianReviewDrainOutcome::Forced(vec![forced_abort])
    );
    assert_eq!(
        drop_rx.try_recv(),
        Err(oneshot::error::TryRecvError::Closed)
    );
}

#[tokio::test]
async fn panicked_review_returns_fallback_after_draining() {
    let owner = Arc::new(GuardianReviewTaskOwner::default());
    let activity = owner.begin().expect("review should start");
    let forced_abort = forced_abort_event();
    activity.mark_started(forced_abort.clone());
    let task = owner
        .spawn(&tokio::runtime::Handle::current(), &activity, async move {
            panic!("guardian review panicked");
        })
        .expect("review task should start");
    drop(task);

    assert_eq!(
        owner.close().drain().await,
        GuardianReviewDrainOutcome::Forced(vec![forced_abort])
    );
}

#[tokio::test]
async fn fallback_requires_started_nonterminal_review() {
    let owner = Arc::new(GuardianReviewTaskOwner::default());
    let activity = owner.begin().expect("review should start");
    let task = owner
        .spawn(
            &tokio::runtime::Handle::current(),
            &activity,
            std::future::pending::<()>(),
        )
        .expect("review task should start");
    drop(task);
    let mut drain = owner.close();
    drain.deadline = Instant::now();
    assert_eq!(
        drain.drain().await,
        GuardianReviewDrainOutcome::Forced(Vec::new())
    );

    let owner = Arc::new(GuardianReviewTaskOwner::default());
    let activity = owner.begin().expect("review should start");
    activity.mark_started(forced_abort_event());
    activity.mark_terminal();
    let task = owner
        .spawn(
            &tokio::runtime::Handle::current(),
            &activity,
            std::future::pending::<()>(),
        )
        .expect("review task should start");
    drop(task);
    let mut drain = owner.close();
    drain.deadline = Instant::now();
    assert_eq!(
        drain.drain().await,
        GuardianReviewDrainOutcome::Forced(Vec::new())
    );
}

#[test]
fn commit_is_linearized_against_cancellation() {
    let committed_owner = Arc::new(GuardianReviewTaskOwner::default());
    let committed = committed_owner.begin().expect("review should start");
    assert!(committed.try_commit());
    let _drain = committed_owner.close();
    assert!(committed.try_commit());

    let interrupted_owner = Arc::new(GuardianReviewTaskOwner::default());
    let interrupted = interrupted_owner
        .begin()
        .expect("review should start before interruption");
    let _drain = interrupted_owner.close();
    assert!(!interrupted.try_commit());
    assert!(interrupted_owner.begin().is_none());

    let cancelled_owner = Arc::new(GuardianReviewTaskOwner::default());
    let cancelled = cancelled_owner.begin().expect("review should start");
    assert!(cancelled.cancel());
    assert!(!cancelled.try_commit());
}
