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
async fn overlapping_multi_claim_extensions_release_before_retrying_without_partial_claims() {
    let pending = PendingThreadUnloads::default();
    let root_a = ThreadId::new();
    let root_b = ThreadId::new();
    let late_a = ThreadId::new();
    let late_b = ThreadId::new();

    let PendingThreadUnloadClaimResult::Claimed(claim_a) = pending.try_claim_many([root_a]) else {
        panic!("first root should be claimable");
    };
    let PendingThreadUnloadClaimResult::Claimed(claim_b) = pending.try_claim_many([root_b]) else {
        panic!("second root should be claimable");
    };
    let (conflicts_a_tx, conflicts_a_rx) = oneshot::channel();
    let (release_a_tx, release_a_rx) = oneshot::channel();
    let completion_a = claim_a.start_with(move |owner| async move {
        let PendingThreadUnloadExtendResult::Pending(conflicts) =
            owner.try_extend([root_b, late_a])
        else {
            panic!("first extension should conflict with the second owner");
        };
        assert!(conflicts_a_tx.send(conflicts).is_ok());
        let _ = release_a_rx.await;
    });
    let (conflicts_b_tx, conflicts_b_rx) = oneshot::channel();
    let (release_b_tx, release_b_rx) = oneshot::channel();
    let completion_b = claim_b.start_with(move |owner| async move {
        let PendingThreadUnloadExtendResult::Pending(conflicts) =
            owner.try_extend([root_a, late_b])
        else {
            panic!("second extension should conflict with the first owner");
        };
        assert!(conflicts_b_tx.send(conflicts).is_ok());
        let _ = release_b_rx.await;
    });
    let conflicts_a = conflicts_a_rx.await.expect("first extension result");
    let conflicts_b = conflicts_b_rx.await.expect("second extension result");
    assert_eq!(conflicts_a.completions.len(), 1);
    assert_eq!(conflicts_b.completions.len(), 1);
    assert!(!pending.contains(late_a));
    assert!(!pending.contains(late_b));
    let unclaimed = ThreadId::new();
    let PendingThreadUnloadClaimResult::Pending(all_conflicts) =
        pending.try_claim_many([root_a, root_b, unclaimed])
    else {
        panic!("an overlapping multi-claim should report every conflicting owner");
    };
    assert_eq!(all_conflicts.completions.len(), 2);
    assert!(!pending.contains(unclaimed));

    // Neither owner may wait while retaining its partial group: each one is the other's
    // conflict. Ending both tracked operations lets every conflict subscription complete.
    release_a_tx.send(()).expect("release first owner");
    release_b_tx.send(()).expect("release second owner");
    wait_for_thread_unload(completion_a).await;
    wait_for_thread_unload(completion_b).await;
    wait_for_thread_unloads(conflicts_a).await;
    wait_for_thread_unloads(conflicts_b).await;
    wait_for_thread_unloads(all_conflicts).await;

    let PendingThreadUnloadClaimResult::Claimed(full_claim) =
        pending.try_claim_many([root_a, root_b, late_a, late_b, late_a])
    else {
        panic!("the complete deduplicated set should be claimable after both owners release");
    };
    let same_owner_unclaimed = ThreadId::new();
    let PendingThreadUnloadClaimResult::Pending(same_owner_conflict) =
        pending.try_claim_many([root_a, late_a, same_owner_unclaimed])
    else {
        panic!("overlap with one owner should conflict");
    };
    assert_eq!(same_owner_conflict.completions.len(), 1);
    assert!(!pending.contains(same_owner_unclaimed));
    let full_completion = full_claim.completed.clone();
    drop(full_claim);
    wait_for_thread_unload(full_completion).await;
    wait_for_thread_unloads(same_owner_conflict).await;
    assert!(pending.is_empty());

    let stale_root = ThreadId::new();
    let closing_late = ThreadId::new();
    let stale_late = ThreadId::new();
    let PendingThreadUnloadClaimResult::Claimed(stale_claim) = pending.try_claim_many([stale_root])
    else {
        panic!("stale-handle root should be claimable");
    };
    let (extend_tx, extend_rx) = oneshot::channel();
    let (stale_tx, stale_rx) = oneshot::channel();
    let stale_completion = stale_claim.start_with(move |owner| async move {
        let _ = extend_rx.await;
        assert!(matches!(
            owner.try_extend([closing_late]),
            PendingThreadUnloadExtendResult::Extended
        ));
        assert!(stale_tx.send(owner).is_ok());
    });
    let pending_for_drain = pending.clone();
    let drain = tokio::spawn(async move { pending_for_drain.close_and_wait().await });
    while !pending.lock_registry().closing {
        tokio::task::yield_now().await;
    }
    extend_tx.send(()).expect("extend while coordinator closes");
    let stale_owner = stale_rx.await.expect("stale owner handle");
    wait_for_thread_unload(stale_completion).await;
    drain.await.expect("coordinator drain should complete");
    assert!(matches!(
        stale_owner.try_extend([stale_late]),
        PendingThreadUnloadExtendResult::Finished
    ));
    assert!(!pending.contains(stale_late));
    assert!(pending.is_empty());
}
