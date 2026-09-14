use std::time::Duration;

use pretty_assertions::assert_eq;
use tokio_util::sync::CancellationToken;

use super::run_before_review_deadline_with_cancel;
use crate::GuardianReviewSessionOutcome;

#[tokio::test(flavor = "current_thread")]
async fn run_before_review_deadline_with_cancel_cancels_token_on_timeout() {
    let cancel_token = CancellationToken::new();

    let outcome = run_before_review_deadline_with_cancel(
        tokio::time::Instant::now() + Duration::from_millis(10),
        /*external_cancel*/ None,
        &cancel_token,
        async {
            tokio::time::sleep(Duration::from_millis(50)).await;
        },
    )
    .await;

    assert!(matches!(
        outcome,
        Err(GuardianReviewSessionOutcome::TimedOut)
    ));
    assert!(cancel_token.is_cancelled());
}

#[tokio::test(flavor = "current_thread")]
async fn run_before_review_deadline_with_cancel_cancels_token_on_abort() {
    let external_cancel = CancellationToken::new();
    let external_canceller = external_cancel.clone();
    let cancel_token = CancellationToken::new();
    drop(tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(10)).await;
        external_canceller.cancel();
    }));

    let outcome = run_before_review_deadline_with_cancel(
        tokio::time::Instant::now() + Duration::from_secs(1),
        Some(&external_cancel),
        &cancel_token,
        std::future::pending::<()>(),
    )
    .await;

    assert!(matches!(
        outcome,
        Err(GuardianReviewSessionOutcome::Aborted)
    ));
    assert!(cancel_token.is_cancelled());
}

#[tokio::test(flavor = "current_thread")]
async fn run_before_review_deadline_with_cancel_preserves_token_on_success() {
    let cancel_token = CancellationToken::new();

    let outcome = run_before_review_deadline_with_cancel(
        tokio::time::Instant::now() + Duration::from_secs(1),
        /*external_cancel*/ None,
        &cancel_token,
        async { 42usize },
    )
    .await;

    assert_eq!(outcome.unwrap(), 42);
    assert!(!cancel_token.is_cancelled());
}
