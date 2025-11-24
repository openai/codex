#![warn(rust_2018_idioms)]
#![cfg(not(target_os = "wasi"))] // Wasi doesn't support threads

use std::rc::Rc;
use std::sync::Arc;
use tokio::sync::Barrier;
use tokio_util::task;

/// Simple test of running a !Send future via spawn_pinned
#[tokio::test]
async fn can_spawn_not_send_future() {
    let pool = task::LocalPoolHandle::new(1);

    let output = pool
        .spawn_pinned(|| {
            // Rc is !Send + !Sync
            let local_data = Rc::new("test");

            // This future holds an Rc, so it is !Send
            async move { local_data.to_string() }
        })
        .await
        .unwrap();

    assert_eq!(output, "test");
}

/// Dropping the join handle still lets the task execute
#[test]
fn can_drop_future_and_still_get_output() {
    let pool = task::LocalPoolHandle::new(1);
    let (sender, receiver) = std::sync::mpsc::channel();

    pool.spawn_pinned(move || {
        // Rc is !Send + !Sync
        let local_data = Rc::new("test");

        // This future holds an Rc, so it is !Send
        async move {
            let _ = sender.send(local_data.to_string());
        }
    });

    assert_eq!(receiver.recv(), Ok("test".to_string()));
}

#[test]
#[should_panic(expected = "assertion failed: pool_size > 0")]
fn cannot_create_zero_sized_pool() {
    let _pool = task::LocalPoolHandle::new(0);
}

/// We should be able to spawn multiple futures onto the pool at the same time.
#[tokio::test]
async fn can_spawn_multiple_futures() {
    let pool = task::LocalPoolHandle::new(2);

    let join_handle1 = pool.spawn_pinned(|| {
        let local_data = Rc::new("test1");
        async move { local_data.to_string() }
    });
    let join_handle2 = pool.spawn_pinned(|| {
        let local_data = Rc::new("test2");
        async move { local_data.to_string() }
    });

    assert_eq!(join_handle1.await.unwrap(), "test1");
    assert_eq!(join_handle2.await.unwrap(), "test2");
}

/// A panic in the spawned task causes the join handle to return an error.
/// But, you can continue to spawn tasks.
#[tokio::test]
#[cfg(panic = "unwind")]
async fn task_panic_propagates() {
    let pool = task::LocalPoolHandle::new(1);

    let join_handle = pool.spawn_pinned(|| async {
        panic!("Test panic");
    });

    let result = join_handle.await;
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.is_panic());
    let panic_str = error.into_panic().downcast::<&'static str>().unwrap();
    assert_eq!(*panic_str, "Test panic");

    // Trying again with a "safe" task still works
    let join_handle = pool.spawn_pinned(|| async { "test" });
    let result = join_handle.await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "test");
}

/// A panic during task creation causes the join handle to return an error.
/// But, you can continue to spawn tasks.
#[tokio::test]
#[cfg(panic = "unwind")]
async fn callback_panic_does_not_kill_worker() {
    let pool = task::LocalPoolHandle::new(1);

    let join_handle = pool.spawn_pinned(|| {
        panic!("Test panic");
        #[allow(unreachable_code)]
        async {}
    });

    let result = join_handle.await;
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.is_panic());
    let panic_str = error.into_panic().downcast::<&'static str>().unwrap();
    assert_eq!(*panic_str, "Test panic");

    // Trying again with a "safe" callback works
    let join_handle = pool.spawn_pinned(|| async { "test" });
    let result = join_handle.await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "test");
}

/// Canceling the task via the returned join handle cancels the spawned task
/// (which has a different, internal join handle).
#[tokio::test]
async fn task_cancellation_propagates() {
    let pool = task::LocalPoolHandle::new(1);
    let notify_dropped = Arc::new(());
    let weak_notify_dropped = Arc::downgrade(&notify_dropped);

    let (start_sender, start_receiver) = tokio::sync::oneshot::channel();
    let (drop_sender, drop_receiver) = tokio::sync::oneshot::channel::<()>();
    let join_handle = pool.spawn_pinned(|| async move {
        let _drop_sender = drop_sender;
        // Move the Arc into the task
        let _notify_dropped = notify_dropped;
        let _ = start_sender.send(());

        // Keep the task running until it gets aborted
        futures::future::pending::<()>().await;
    });

    // Wait for the task to start
    let _ = start_receiver.await;

    join_handle.abort();

    // Wait for the inner task to abort, dropping the sender.
    // The top level join handle aborts quicker than the inner task (the abort
    // needs to propagate and get processed on the worker thread), so we can't
    // just await the top level join handle.
    let _ = drop_receiver.await;

    // Check that the Arc has been dropped. This verifies that the inner task
    // was canceled as well.
    assert!(weak_notify_dropped.upgrade().is_none());
}

/// Tasks should be given to the least burdened worker. When spawning two tasks
/// on a pool with two empty workers the tasks should be spawned on separate
/// workers.
#[tokio::test]
async fn tasks_are_balanced() {
    let pool = task::LocalPoolHandle::new(2);

    // Spawn a task so one thread has a task count of 1
    let (start_sender1, start_receiver1) = tokio::sync::oneshot::channel();
    let (end_sender1, end_receiver1) = tokio::sync::oneshot::channel();
    let join_handle1 = pool.spawn_pinned(|| async move {
        let _ = start_sender1.send(());
        let _ = end_receiver1.await;
        std::thread::current().id()
    });

    // Wait for the first task to start up
    let _ = start_receiver1.await;

    // This task should be spawned on the other thread
    let (start_sender2, start_receiver2) = tokio::sync::oneshot::channel();
    let join_handle2 = pool.spawn_pinned(|| async move {
        let _ = start_sender2.send(());
        std::thread::current().id()
    });

    // Wait for the second task to start up
    let _ = start_receiver2.await;

    // Allow the first task to end
    let _ = end_sender1.send(());

    let thread_id1 = join_handle1.await.unwrap();
    let thread_id2 = join_handle2.await.unwrap();

    // Since the first task was active when the second task spawned, they should
    // be on separate workers/threads.
    assert_ne!(thread_id1, thread_id2);
}

#[tokio::test]
async fn spawn_by_idx() {
    let pool = task::LocalPoolHandle::new(3);
    let barrier = Arc::new(Barrier::new(4));
    let barrier1 = barrier.clone();
    let barrier2 = barrier.clone();
    let barrier3 = barrier.clone();

    let handle1 = pool.spawn_pinned_by_idx(
        || async move {
            barrier1.wait().await;
            std::thread::current().id()
        },
        0,
    );
    pool.spawn_pinned_by_idx(
        || async move {
            barrier2.wait().await;
            std::thread::current().id()
        },
        0,
    );
    let handle2 = pool.spawn_pinned_by_idx(
        || async move {
            barrier3.wait().await;
            std::thread::current().id()
        },
        1,
    );

    let loads = pool.get_task_loads_for_each_worker();
    barrier.wait().await;
    assert_eq!(loads[0], 2);
    assert_eq!(loads[1], 1);
    assert_eq!(loads[2], 0);

    let thread_id1 = handle1.await.unwrap();
    let thread_id2 = handle2.await.unwrap();

    assert_ne!(thread_id1, thread_id2);
}
