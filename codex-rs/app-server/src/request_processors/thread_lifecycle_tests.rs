use super::*;
use pretty_assertions::assert_eq;

#[test]
fn reactivation_restarts_unloading_delay() {
    let delay = Duration::from_secs(60);
    let expired_at = Instant::now() - delay - Duration::from_secs(1);
    let (_has_retaining_subscribers_tx, has_retaining_subscribers_rx) = watch::channel(false);
    let (thread_status_tx, thread_status_rx) = watch::channel(ThreadStatus::Idle);
    let mut unloading_state = UnloadingState {
        delay,
        has_retaining_subscribers_rx,
        has_retaining_subscribers: (false, expired_at),
        thread_status_rx,
        is_active: (false, expired_at),
    };

    assert!(unloading_state.should_unload_now());

    thread_status_tx
        .send(ThreadStatus::Active {
            active_flags: Vec::new(),
        })
        .expect("thread status watcher should remain open");
    unloading_state.sync_receiver_values();
    assert_eq!(unloading_state.unloading_target(), None);

    thread_status_tx
        .send(ThreadStatus::Idle)
        .expect("thread status watcher should remain open");
    unloading_state.sync_receiver_values();
    assert!(!unloading_state.should_unload_now());
    assert!(
        unloading_state
            .unloading_target()
            .is_some_and(|target| target > Instant::now())
    );
}

#[tokio::test]
async fn coalesced_changes_win_over_expired_unloading_deadline() {
    let delay = Duration::from_secs(60);
    let expired_at = Instant::now() - delay - Duration::from_secs(1);
    let (has_retaining_subscribers_tx, has_retaining_subscribers_rx) = watch::channel(false);
    let (thread_status_tx, thread_status_rx) = watch::channel(ThreadStatus::Idle);
    let mut unloading_state = UnloadingState {
        delay,
        has_retaining_subscribers_rx,
        has_retaining_subscribers: (false, expired_at),
        thread_status_rx,
        is_active: (false, expired_at),
    };

    has_retaining_subscribers_tx
        .send(true)
        .expect("subscriber watcher should remain open");
    has_retaining_subscribers_tx
        .send(false)
        .expect("subscriber watcher should remain open");
    assert!(
        tokio::time::timeout(
            Duration::from_millis(20),
            unloading_state.wait_for_unloading_trigger(),
        )
        .await
        .is_err()
    );

    unloading_state.has_retaining_subscribers = (false, expired_at);
    unloading_state.is_active = (false, expired_at);
    thread_status_tx
        .send(ThreadStatus::Active {
            active_flags: Vec::new(),
        })
        .expect("thread status watcher should remain open");
    thread_status_tx
        .send(ThreadStatus::Idle)
        .expect("thread status watcher should remain open");
    assert!(
        tokio::time::timeout(
            Duration::from_millis(20),
            unloading_state.wait_for_unloading_trigger(),
        )
        .await
        .is_err()
    );
}
