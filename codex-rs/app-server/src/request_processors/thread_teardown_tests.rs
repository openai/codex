use super::*;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

#[tokio::test]
async fn teardown_is_singleflight_and_outlives_its_first_waiter() {
    let pending = PendingThreadUnloads::default();
    let provisional_id = ThreadId::new();
    let PendingThreadUnloadClaimResult::Claimed(provisional) = pending.try_claim(provisional_id)
    else {
        panic!("new thread id should be claimable");
    };
    let completion = provisional.completed.clone();
    drop(provisional);
    wait_for_thread_unload(completion).await;

    let thread_id = ThreadId::new();
    let calls = Arc::new(AtomicUsize::new(0));
    let (started_tx, started_rx) = oneshot::channel();
    let (release_tx, release_rx) = oneshot::channel();
    let first_calls = Arc::clone(&calls);
    let first_completed = match pending.try_start(thread_id, async move {
        first_calls.fetch_add(1, Ordering::SeqCst);
        let _ = started_tx.send(());
        let _ = release_rx.await;
    }) {
        PendingThreadUnloadStartResult::Started(completed) => completed,
        _ => panic!("first teardown should start"),
    };
    let first = tokio::spawn(wait_for_thread_unload(first_completed));
    started_rx.await.expect("teardown should start");
    let second_calls = Arc::clone(&calls);
    let second_completed = match pending.try_start(thread_id, async move {
        second_calls.fetch_add(1, Ordering::SeqCst);
    }) {
        PendingThreadUnloadStartResult::Pending(completed) => completed,
        _ => panic!("second teardown should join the first"),
    };
    let second = tokio::spawn(wait_for_thread_unload(second_completed));
    let pending_for_drain = pending.clone();
    let drain = tokio::spawn(async move { pending_for_drain.close_and_wait().await });
    tokio::task::yield_now().await;
    assert!(!drain.is_finished());

    first.abort();
    assert!(
        first
            .await
            .expect_err("first waiter should be cancelled")
            .is_cancelled()
    );
    release_tx
        .send(())
        .expect("teardown should still be running");
    second.await.expect("second waiter should complete");
    drain.await.expect("teardown drain should complete");
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(pending.is_empty());
}
