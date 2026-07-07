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

#[tokio::test]
async fn freeze_handoffs_are_gapless_serialized_and_cancellation_safe() {
    let pending = PendingThreadUnloads::default();
    let mut freeze = pending
        .acquire_freeze_guard()
        .await
        .expect("freeze guard should be available");
    let root = ThreadId::new();
    let free_sibling = ThreadId::new();
    let late_a = ThreadId::new();
    let late_a_alias = ThreadId::new();
    let late_b = ThreadId::new();

    let PendingThreadUnloadClaimResult::Claimed(predecessor_a) =
        pending.try_claim_many([late_a, late_a_alias])
    else {
        panic!("first predecessor should claim late descendants");
    };
    let (release_predecessor_a_tx, release_predecessor_a_rx) = oneshot::channel();
    let predecessor_a_completion = predecessor_a.start(async move {
        let _ = release_predecessor_a_rx.await;
    });
    let PendingThreadUnloadClaimResult::Claimed(predecessor_b) = pending.try_claim(late_b) else {
        panic!("second predecessor should claim its late descendant");
    };
    let (release_predecessor_b_tx, release_predecessor_b_rx) = oneshot::channel();
    let predecessor_b_completion = predecessor_b.start(async move {
        let _ = release_predecessor_b_rx.await;
    });
    let PendingThreadUnloadClaimResult::Claimed(successor) = pending.try_claim(root) else {
        panic!("successor should claim its root");
    };
    let (conflicts_tx, conflicts_rx) = oneshot::channel();
    let (retry_tx, retry_rx) = oneshot::channel();
    let (stable_tx, stable_rx) = oneshot::channel();
    let (release_successor_tx, release_successor_rx) = oneshot::channel();
    let successor_completion = successor.start_with(move |owner| async move {
        let PendingThreadUnloadExtendResult::Pending(conflicts) = owner.try_extend(
            &mut freeze,
            [root, free_sibling, late_a, late_a_alias, late_b],
        ) else {
            panic!("late descendants should conflict with their predecessor");
        };
        assert!(conflicts_tx.send(conflicts).is_ok());
        let _ = retry_rx.await;
        assert!(matches!(
            owner.try_extend(
                &mut freeze,
                [root, free_sibling, late_a, late_a_alias, late_b]
            ),
            PendingThreadUnloadExtendResult::Extended
        ));
        let _ = stable_tx.send(());
        let _ = release_successor_rx.await;
    });
    let conflicts = conflicts_rx.await.expect("extension conflicts");
    assert_eq!(conflicts.completions.len(), 2);
    assert!(pending.contains(free_sibling));

    let pending_for_freeze = pending.clone();
    let next_freeze = tokio::spawn(async move { pending_for_freeze.acquire_freeze_guard().await });
    tokio::task::yield_now().await;
    assert!(!next_freeze.is_finished());

    release_predecessor_a_tx
        .send(())
        .expect("release first predecessor");
    release_predecessor_b_tx
        .send(())
        .expect("release second predecessor");
    wait_for_thread_unload(predecessor_a_completion).await;
    wait_for_thread_unload(predecessor_b_completion).await;
    wait_for_thread_unloads(conflicts).await;
    let PendingThreadUnloadClaimResult::Pending(successor_conflict) =
        pending.try_claim_many([root, free_sibling, late_a, late_a_alias, late_b])
    else {
        panic!("every ID should transfer to the one successor owner");
    };
    assert_eq!(successor_conflict.completions.len(), 1);
    retry_tx.send(()).expect("retry extension after handoff");
    stable_rx.await.expect("successor should become stable");
    release_successor_tx.send(()).expect("release successor");
    wait_for_thread_unload(successor_completion).await;
    wait_for_thread_unloads(successor_conflict).await;
    let mut freeze = next_freeze
        .await
        .expect("freeze waiter should not fail")
        .expect("next freeze guard should become available");
    assert!(pending.is_empty());

    let cancelled_root = ThreadId::new();
    let cancelled_free = ThreadId::new();
    let contested = ThreadId::new();
    let PendingThreadUnloadClaimResult::Claimed(predecessor) = pending.try_claim(contested) else {
        panic!("cancellation predecessor should be claimable");
    };
    let (release_predecessor_tx, release_predecessor_rx) = oneshot::channel();
    let predecessor_completion = predecessor.start(async move {
        let _ = release_predecessor_rx.await;
    });
    let PendingThreadUnloadClaimResult::Claimed(cancelled) = pending.try_claim(cancelled_root)
    else {
        panic!("cancelled successor should claim its root");
    };
    let (cancelled_conflicts_tx, cancelled_conflicts_rx) = oneshot::channel();
    let (stale_owner_tx, stale_owner_rx) = oneshot::channel();
    let cancelled_completion = cancelled.start_with(move |owner| async move {
        let PendingThreadUnloadExtendResult::Pending(conflicts) =
            owner.try_extend(&mut freeze, [cancelled_root, cancelled_free, contested])
        else {
            panic!("cancelled successor should register a handoff");
        };
        assert!(cancelled_conflicts_tx.send(conflicts).is_ok());
        assert!(stale_owner_tx.send(owner).is_ok());
    });
    let cancelled_conflicts = cancelled_conflicts_rx
        .await
        .expect("cancelled extension conflicts");
    let stale_owner = stale_owner_rx.await.expect("cancelled owner handle");
    wait_for_thread_unload(cancelled_completion).await;
    assert!(!pending.contains(cancelled_root));
    assert!(!pending.contains(cancelled_free));
    assert!(pending.contains(contested));
    let mut stale_freeze = pending
        .acquire_freeze_guard()
        .await
        .expect("cancelled freeze guard should be released");
    assert!(matches!(
        stale_owner.try_extend(&mut stale_freeze, [cancelled_free]),
        PendingThreadUnloadExtendResult::Finished
    ));
    drop(stale_freeze);
    release_predecessor_tx
        .send(())
        .expect("release cancellation predecessor");
    wait_for_thread_unload(predecessor_completion).await;
    wait_for_thread_unloads(cancelled_conflicts).await;
    assert!(pending.is_empty());
}
