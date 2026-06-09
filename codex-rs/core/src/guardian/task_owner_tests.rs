use pretty_assertions::assert_eq;
use std::sync::Arc;
use tokio::sync::oneshot;
use tokio::time::Instant;

use super::GuardianReviewDrainOutcome;
use super::GuardianReviewTaskOwner;

#[tokio::test]
async fn dropping_review_handle_leaves_cleanup_to_owner() {
    let owner = Arc::new(GuardianReviewTaskOwner::default());
    let cancellation_token = owner.cancellation_token();
    let (completed_tx, completed_rx) = oneshot::channel();
    let task = owner
        .spawn(&tokio::runtime::Handle::current(), async move {
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
    let (drop_tx, mut drop_rx) = oneshot::channel::<()>();
    let task = owner
        .spawn(&tokio::runtime::Handle::current(), async move {
            let _drop_tx = drop_tx;
            std::future::pending::<()>().await;
        })
        .expect("review task should start");
    drop(task);

    let mut drain = owner.close();
    drain.deadline = Instant::now();
    assert_eq!(drain.drain().await, GuardianReviewDrainOutcome::Forced);
    assert_eq!(
        drop_rx.try_recv(),
        Err(oneshot::error::TryRecvError::Closed)
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
