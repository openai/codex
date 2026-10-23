//! Checks overall review budgets without sleeping in wall-clock time.

use super::*;

#[tokio::test(start_paused = true)]
async fn setup_uses_remaining_budget() {
    let deadline = ReviewDeadline::from_timeout(Some(Duration::from_secs(10)));
    run_before_review_deadline(deadline, /*external_cancel*/ None, async {
        tokio::time::sleep(Duration::from_secs(6)).await;
    })
    .await
    .expect("first stage fits");
    let result = run_before_review_deadline(deadline, /*external_cancel*/ None, async {
        tokio::time::sleep(Duration::from_secs(6)).await;
    })
    .await;
    assert!(matches!(
        result,
        Err(GuardianReviewSessionOutcome::TimedOut)
    ));
}

#[tokio::test(start_paused = true)]
async fn unlimited_setup_can_be_cancelled() {
    let cancel = CancellationToken::new();
    let (result, ()) = tokio::join!(
        run_before_review_deadline(
            ReviewDeadline::Unlimited,
            Some(&cancel),
            std::future::pending::<()>()
        ),
        async {
            tokio::time::sleep(Duration::from_secs(120)).await;
            cancel.cancel();
        },
    );
    assert!(matches!(result, Err(GuardianReviewSessionOutcome::Aborted)));
}
