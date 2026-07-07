use super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn resume_reservation_prevents_idle_unload_before_listener_commit() {
    let now = Instant::now();
    let (_has_subscribers_tx, has_subscribers_rx) = watch::channel(false);
    let (_thread_status_tx, thread_status_rx) = watch::channel(ThreadStatus::Idle);
    let mut unloading_state = UnloadingState {
        delay: Duration::ZERO,
        has_subscribers_rx,
        has_subscribers: (false, now),
        thread_status_rx,
        is_active: (false, now),
    };
    let pending = PendingThreadUnloads::default();
    let thread_state_manager = ThreadStateManager::new();
    let thread_id = ThreadId::new();
    let connection_id = ConnectionId(1);
    thread_state_manager
        .connection_initialized(connection_id, ConnectionCapabilities::default())
        .await;
    let permit = Arc::new(Semaphore::new(1));
    let resume_permit = permit
        .clone()
        .acquire_owned()
        .await
        .expect("permit should be open");
    let (resume_handled_tx, resume_outcome_rx) = thread_state_manager
        .start_connection_reservation(thread_id, connection_id)
        .await
        .expect("live connection should reserve the thread");
    drop(resume_permit);
    assert!(
        try_start_idle_thread_unload(
            &mut unloading_state,
            permit,
            &pending,
            &thread_state_manager,
            thread_id,
            std::future::ready(false),
            std::future::ready(()),
        )
        .await
        .is_none()
    );
    assert!(!pending.contains(thread_id));

    assert!(
        thread_state_manager
            .unsubscribe_connection_from_thread(thread_id, connection_id)
            .await
    );
    let (replacement_tx, replacement_outcome_rx) = thread_state_manager
        .start_connection_reservation(thread_id, connection_id)
        .await
        .expect("replacement reservation should succeed");
    resume_handled_tx
        .send(ConnectionReservationAction::Commit)
        .expect("reservation owner should still be running");
    assert_eq!(
        resume_outcome_rx
            .await
            .expect("reservation owner should report an outcome"),
        ConnectionReservationOutcome::Handled,
    );
    assert!(
        thread_state_manager
            .subscribed_connection_ids(thread_id)
            .await
            .is_empty()
    );
    assert!(
        thread_state_manager
            .has_connections_or_reservations(thread_id)
            .await
    );

    drop(replacement_tx);
    assert_eq!(
        replacement_outcome_rx
            .await
            .expect("reservation owner should report abandonment"),
        ConnectionReservationOutcome::Abandoned,
    );
    assert!(
        !thread_state_manager
            .has_connections_or_reservations(thread_id)
            .await
    );
    assert!(
        thread_state_manager
            .subscribed_connection_ids(thread_id)
            .await
            .is_empty()
    );
    pending.close_and_wait().await;
    thread_state_manager.clear_all_listeners().await;
}

#[tokio::test]
async fn pending_resume_prevents_disconnect_from_reporting_thread_stale() {
    let thread_state_manager = ThreadStateManager::new();
    let thread_id = ThreadId::new();
    let committed_connection_id = ConnectionId(1);
    let pending_connection_id = ConnectionId(2);
    for connection_id in [committed_connection_id, pending_connection_id] {
        thread_state_manager
            .connection_initialized(connection_id, ConnectionCapabilities::default())
            .await;
    }
    assert!(
        thread_state_manager
            .try_add_connection_to_thread(thread_id, committed_connection_id)
            .await
    );
    let (reservation_tx, reservation_outcome_rx) = thread_state_manager
        .start_connection_reservation(thread_id, pending_connection_id)
        .await
        .expect("live connection should reserve the thread");

    assert!(
        thread_state_manager
            .remove_connection(committed_connection_id)
            .await
            .is_empty(),
        "a pending reservation must keep the thread from being reported stale"
    );

    drop(reservation_tx);
    assert_eq!(
        reservation_outcome_rx
            .await
            .expect("reservation owner should report abandonment"),
        ConnectionReservationOutcome::Abandoned,
    );
    thread_state_manager.clear_all_listeners().await;
}
