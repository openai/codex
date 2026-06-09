use std::future;
use std::time::Duration;

use pretty_assertions::assert_eq;
use tokio_util::sync::CancellationToken;

use super::EagerCompactionRunOutcome;
use super::GuardianEagerCompaction;

#[tokio::test]
async fn cancellation_releases_eager_compaction_waiters() {
    let eager_compaction = GuardianEagerCompaction::default();
    let run = eager_compaction.begin().await.expect("start maintenance");
    let cancel_token = CancellationToken::new();
    cancel_token.cancel();

    let outcome = run
        .run_bounded(
            &cancel_token,
            Duration::from_secs(/*secs*/ 1),
            future::pending(),
        )
        .await;

    assert_eq!(outcome, EagerCompactionRunOutcome::Cancelled);
    tokio::time::timeout(Duration::from_secs(/*secs*/ 1), eager_compaction.wait())
        .await
        .expect("waiter should be released after cancellation");
}

#[tokio::test]
async fn total_timeout_releases_eager_compaction_waiters() {
    let eager_compaction = GuardianEagerCompaction::default();
    let run = eager_compaction.begin().await.expect("start maintenance");
    let cancel_token = CancellationToken::new();

    let outcome = run
        .run_bounded(
            &cancel_token,
            Duration::from_millis(/*millis*/ 10),
            future::pending(),
        )
        .await;

    assert_eq!(outcome, EagerCompactionRunOutcome::TimedOut);
    tokio::time::timeout(Duration::from_secs(/*secs*/ 1), eager_compaction.wait())
        .await
        .expect("waiter should be released after the total maintenance timeout");
}
