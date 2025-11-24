#![warn(rust_2018_idioms)]
#![cfg(feature = "full")]

use futures::{
    future::{pending, ready},
    FutureExt,
};

use tokio::runtime;
use tokio::sync::{mpsc, oneshot};
use tokio::task::{self, LocalSet};
use tokio::time;

#[cfg(not(target_os = "wasi"))]
use std::cell::Cell;
use std::sync::atomic::AtomicBool;
#[cfg(not(target_os = "wasi"))]
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
#[cfg(not(target_os = "wasi"))]
use std::sync::atomic::Ordering::SeqCst;
use std::time::Duration;

#[tokio::test(flavor = "current_thread")]
async fn local_current_thread_scheduler() {
    LocalSet::new()
        .run_until(async {
            task::spawn_local(async {}).await.unwrap();
        })
        .await;
}

#[cfg(not(target_os = "wasi"))] // Wasi doesn't support threads
#[tokio::test(flavor = "multi_thread")]
async fn local_threadpool() {
    thread_local! {
        static ON_RT_THREAD: Cell<bool> = const { Cell::new(false) };
    }

    ON_RT_THREAD.with(|cell| cell.set(true));

    LocalSet::new()
        .run_until(async {
            assert!(ON_RT_THREAD.with(|cell| cell.get()));
            task::spawn_local(async {
                assert!(ON_RT_THREAD.with(|cell| cell.get()));
            })
            .await
            .unwrap();
        })
        .await;
}

#[cfg(not(target_os = "wasi"))] // Wasi doesn't support threads
#[tokio::test(flavor = "multi_thread")]
async fn localset_future_threadpool() {
    thread_local! {
        static ON_LOCAL_THREAD: Cell<bool> = const { Cell::new(false) };
    }

    ON_LOCAL_THREAD.with(|cell| cell.set(true));

    let local = LocalSet::new();
    local.spawn_local(async move {
        assert!(ON_LOCAL_THREAD.with(|cell| cell.get()));
    });
    local.await;
}

#[cfg(not(target_os = "wasi"))] // Wasi doesn't support threads
#[tokio::test(flavor = "multi_thread")]
async fn localset_future_timers() {
    static RAN1: AtomicBool = AtomicBool::new(false);
    static RAN2: AtomicBool = AtomicBool::new(false);

    let local = LocalSet::new();
    local.spawn_local(async move {
        time::sleep(Duration::from_millis(5)).await;
        RAN1.store(true, Ordering::SeqCst);
    });
    local.spawn_local(async move {
        time::sleep(Duration::from_millis(10)).await;
        RAN2.store(true, Ordering::SeqCst);
    });
    local.await;
    assert!(RAN1.load(Ordering::SeqCst));
    assert!(RAN2.load(Ordering::SeqCst));
}

#[tokio::test]
async fn localset_future_drives_all_local_futs() {
    static RAN1: AtomicBool = AtomicBool::new(false);
    static RAN2: AtomicBool = AtomicBool::new(false);
    static RAN3: AtomicBool = AtomicBool::new(false);

    let local = LocalSet::new();
    local.spawn_local(async move {
        task::spawn_local(async {
            task::yield_now().await;
            RAN3.store(true, Ordering::SeqCst);
        });
        task::yield_now().await;
        RAN1.store(true, Ordering::SeqCst);
    });
    local.spawn_local(async move {
        task::yield_now().await;
        RAN2.store(true, Ordering::SeqCst);
    });
    local.await;
    assert!(RAN1.load(Ordering::SeqCst));
    assert!(RAN2.load(Ordering::SeqCst));
    assert!(RAN3.load(Ordering::SeqCst));
}

#[cfg(not(target_os = "wasi"))] // Wasi doesn't support threads
#[tokio::test(flavor = "multi_thread")]
async fn local_threadpool_timer() {
    // This test ensures that runtime services like the timer are properly
    // set for the local task set.
    thread_local! {
        static ON_RT_THREAD: Cell<bool> = const { Cell::new(false) };
    }

    ON_RT_THREAD.with(|cell| cell.set(true));

    LocalSet::new()
        .run_until(async {
            assert!(ON_RT_THREAD.with(|cell| cell.get()));
            let join = task::spawn_local(async move {
                assert!(ON_RT_THREAD.with(|cell| cell.get()));
                time::sleep(Duration::from_millis(10)).await;
                assert!(ON_RT_THREAD.with(|cell| cell.get()));
            });
            join.await.unwrap();
        })
        .await;
}
#[test]
fn enter_guard_spawn() {
    let local = LocalSet::new();
    let _guard = local.enter();
    // Run the local task set.

    let join = task::spawn_local(async { true });
    let rt = runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    local.block_on(&rt, async move {
        assert!(join.await.unwrap());
    });
}

#[cfg(not(target_os = "wasi"))]
mod block_in_place_cases {
    use super::*;
    use std::future::Future;
    use std::pin::Pin;
    use std::task::{Context, Poll};

    struct BlockInPlaceOnDrop;
    impl Future for BlockInPlaceOnDrop {
        type Output = ();

        fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
            Poll::Pending
        }
    }
    impl Drop for BlockInPlaceOnDrop {
        fn drop(&mut self) {
            tokio::task::block_in_place(|| {});
        }
    }

    async fn complete(jh: tokio::task::JoinHandle<()>) {
        match jh.await {
            Ok(()) => {}
            Err(err) if err.is_panic() => std::panic::resume_unwind(err.into_panic()),
            Err(err) if err.is_cancelled() => panic!("task cancelled"),
            Err(err) => panic!("{err:?}"),
        }
    }

    #[test]
    #[should_panic = "can call blocking only when running on the multi-threaded runtime"]
    fn local_threadpool_blocking_in_place() {
        thread_local! {
            static ON_RT_THREAD: Cell<bool> = const { Cell::new(false) };
        }

        ON_RT_THREAD.with(|cell| cell.set(true));

        let rt = runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        LocalSet::new().block_on(&rt, async {
            assert!(ON_RT_THREAD.with(|cell| cell.get()));
            let join = task::spawn_local(async move {
                assert!(ON_RT_THREAD.with(|cell| cell.get()));
                task::block_in_place(|| {});
                assert!(ON_RT_THREAD.with(|cell| cell.get()));
            });
            complete(join).await;
        });
    }

    #[tokio::test(flavor = "multi_thread")]
    #[should_panic = "can call blocking only when running on the multi-threaded runtime"]
    async fn block_in_place_in_run_until_mt() {
        let local_set = LocalSet::new();
        local_set
            .run_until(async {
                tokio::task::block_in_place(|| {});
            })
            .await;
    }

    #[tokio::test(flavor = "multi_thread")]
    #[should_panic = "can call blocking only when running on the multi-threaded runtime"]
    async fn block_in_place_in_spawn_local_mt() {
        let local_set = LocalSet::new();
        let jh = local_set.spawn_local(async {
            tokio::task::block_in_place(|| {});
        });
        local_set.await;
        complete(jh).await;
    }

    #[tokio::test(flavor = "multi_thread")]
    #[should_panic = "can call blocking only when running on the multi-threaded runtime"]
    async fn block_in_place_in_spawn_local_drop_mt() {
        let local_set = LocalSet::new();
        let jh = local_set.spawn_local(BlockInPlaceOnDrop);
        local_set.run_until(tokio::task::yield_now()).await;
        drop(local_set);
        complete(jh).await;
    }

    #[tokio::test(flavor = "current_thread")]
    #[should_panic = "can call blocking only when running on the multi-threaded runtime"]
    async fn block_in_place_in_run_until_ct() {
        let local_set = LocalSet::new();
        local_set
            .run_until(async {
                tokio::task::block_in_place(|| {});
            })
            .await;
    }

    #[tokio::test(flavor = "current_thread")]
    #[should_panic = "can call blocking only when running on the multi-threaded runtime"]
    async fn block_in_place_in_spawn_local_ct() {
        let local_set = LocalSet::new();
        let jh = local_set.spawn_local(async {
            tokio::task::block_in_place(|| {});
        });
        local_set.await;
        complete(jh).await;
    }

    #[tokio::test(flavor = "current_thread")]
    #[should_panic = "can call blocking only when running on the multi-threaded runtime"]
    async fn block_in_place_in_spawn_local_drop_ct() {
        let local_set = LocalSet::new();
        let jh = local_set.spawn_local(BlockInPlaceOnDrop);
        local_set.run_until(tokio::task::yield_now()).await;
        drop(local_set);
        complete(jh).await;
    }
}

#[cfg(not(target_os = "wasi"))] // Wasi doesn't support threads
#[tokio::test(flavor = "multi_thread")]
async fn local_threadpool_blocking_run() {
    thread_local! {
        static ON_RT_THREAD: Cell<bool> = const { Cell::new(false) };
    }

    ON_RT_THREAD.with(|cell| cell.set(true));

    LocalSet::new()
        .run_until(async {
            assert!(ON_RT_THREAD.with(|cell| cell.get()));
            let join = task::spawn_local(async move {
                assert!(ON_RT_THREAD.with(|cell| cell.get()));
                task::spawn_blocking(|| {
                    assert!(
                        !ON_RT_THREAD.with(|cell| cell.get()),
                        "blocking must not run on the local task set's thread"
                    );
                })
                .await
                .unwrap();
                assert!(ON_RT_THREAD.with(|cell| cell.get()));
            });
            join.await.unwrap();
        })
        .await;
}

#[cfg(not(target_os = "wasi"))] // Wasi doesn't support threads
#[tokio::test(flavor = "multi_thread")]
async fn all_spawns_are_local() {
    use futures::future;
    thread_local! {
        static ON_RT_THREAD: Cell<bool> = const { Cell::new(false) };
    }

    ON_RT_THREAD.with(|cell| cell.set(true));

    LocalSet::new()
        .run_until(async {
            assert!(ON_RT_THREAD.with(|cell| cell.get()));
            let handles = (0..128)
                .map(|_| {
                    task::spawn_local(async {
                        assert!(ON_RT_THREAD.with(|cell| cell.get()));
                    })
                })
                .collect::<Vec<_>>();
            for joined in future::join_all(handles).await {
                joined.unwrap();
            }
        })
        .await;
}

#[cfg(not(target_os = "wasi"))] // Wasi doesn't support threads
#[tokio::test(flavor = "multi_thread")]
async fn nested_spawn_is_local() {
    thread_local! {
        static ON_RT_THREAD: Cell<bool> = const { Cell::new(false) };
    }

    ON_RT_THREAD.with(|cell| cell.set(true));

    LocalSet::new()
        .run_until(async {
            assert!(ON_RT_THREAD.with(|cell| cell.get()));
            task::spawn_local(async {
                assert!(ON_RT_THREAD.with(|cell| cell.get()));
                task::spawn_local(async {
                    assert!(ON_RT_THREAD.with(|cell| cell.get()));
                    task::spawn_local(async {
                        assert!(ON_RT_THREAD.with(|cell| cell.get()));
                        task::spawn_local(async {
                            assert!(ON_RT_THREAD.with(|cell| cell.get()));
                        })
                        .await
                        .unwrap();
                    })
                    .await
                    .unwrap();
                })
                .await
                .unwrap();
            })
            .await
            .unwrap();
        })
        .await;
}

#[cfg(not(target_os = "wasi"))] // Wasi doesn't support threads
#[test]
fn join_local_future_elsewhere() {
    thread_local! {
        static ON_RT_THREAD: Cell<bool> = const { Cell::new(false) };
    }

    ON_RT_THREAD.with(|cell| cell.set(true));

    let rt = runtime::Runtime::new().unwrap();
    let local = LocalSet::new();
    local.block_on(&rt, async move {
        let (tx, rx) = oneshot::channel();
        let join = task::spawn_local(async move {
            assert!(
                ON_RT_THREAD.with(|cell| cell.get()),
                "local task must run on local thread, no matter where it is awaited"
            );
            rx.await.unwrap();

            "hello world"
        });
        let join2 = task::spawn(async move {
            assert!(
                !ON_RT_THREAD.with(|cell| cell.get()),
                "spawned task should be on a worker"
            );

            tx.send(()).expect("task shouldn't have ended yet");

            join.await.expect("task should complete successfully");
        });
        join2.await.unwrap()
    });
}

// Tests for <https://github.com/tokio-rs/tokio/issues/4973>
#[cfg(not(target_os = "wasi"))] // Wasi doesn't support threads
#[tokio::test(flavor = "multi_thread")]
async fn localset_in_thread_local() {
    thread_local! {
        static LOCAL_SET: LocalSet = LocalSet::new();
    }

    // holds runtime thread until end of main fn.
    let (_tx, rx) = oneshot::channel::<()>();
    let handle = tokio::runtime::Handle::current();

    std::thread::spawn(move || {
        LOCAL_SET.with(|local_set| {
            handle.block_on(local_set.run_until(async move {
                let _ = rx.await;
            }))
        });
    });
}

#[test]
fn drop_cancels_tasks() {
    use std::rc::Rc;

    // This test reproduces issue #1842
    let rt = rt();
    let rc1 = Rc::new(());
    let rc2 = rc1.clone();

    let (started_tx, started_rx) = oneshot::channel();

    let local = LocalSet::new();
    local.spawn_local(async move {
        // Move this in
        let _rc2 = rc2;

        started_tx.send(()).unwrap();
        futures::future::pending::<()>().await;
    });

    local.block_on(&rt, async {
        started_rx.await.unwrap();
    });
    drop(local);
    drop(rt);

    assert_eq!(1, Rc::strong_count(&rc1));
}

/// Runs a test function in a separate thread, and panics if the test does not
/// complete within the specified timeout, or if the test function panics.
///
/// This is intended for running tests whose failure mode is a hang or infinite
/// loop that cannot be detected otherwise.
fn with_timeout(timeout: Duration, f: impl FnOnce() + Send + 'static) {
    use std::sync::mpsc::RecvTimeoutError;

    let (done_tx, done_rx) = std::sync::mpsc::channel();
    let thread = std::thread::spawn(move || {
        f();

        // Send a message on the channel so that the test thread can
        // determine if we have entered an infinite loop:
        done_tx.send(()).unwrap();
    });

    // Since the failure mode of this test is an infinite loop, rather than
    // something we can easily make assertions about, we'll run it in a
    // thread. When the test thread finishes, it will send a message on a
    // channel to this thread. We'll wait for that message with a fairly
    // generous timeout, and if we don't receive it, we assume the test
    // thread has hung.
    //
    // Note that it should definitely complete in under a minute, but just
    // in case CI is slow, we'll give it a long timeout.
    match done_rx.recv_timeout(timeout) {
        Err(RecvTimeoutError::Timeout) => panic!(
            "test did not complete within {timeout:?} seconds, \
             we have (probably) entered an infinite loop!",
        ),
        // Did the test thread panic? We'll find out for sure when we `join`
        // with it.
        Err(RecvTimeoutError::Disconnected) => {}
        // Test completed successfully!
        Ok(()) => {}
    }

    thread.join().expect("test thread should not panic!")
}

#[cfg_attr(
    target_os = "wasi",
    ignore = "`unwrap()` in `with_timeout()` panics on Wasi"
)]
#[test]
fn drop_cancels_remote_tasks() {
    // This test reproduces issue #1885.
    with_timeout(Duration::from_secs(60), || {
        let (tx, mut rx) = mpsc::channel::<()>(1024);

        let rt = rt();

        let local = LocalSet::new();
        local.spawn_local(async move { while rx.recv().await.is_some() {} });
        local.block_on(&rt, async {
            time::sleep(Duration::from_millis(1)).await;
        });

        drop(tx);

        // This enters an infinite loop if the remote notified tasks are not
        // properly cancelled.
        drop(local);
    });
}

#[cfg_attr(
    target_os = "wasi",
    ignore = "FIXME: `task::spawn_local().await.unwrap()` panics on Wasi"
)]
#[test]
fn local_tasks_wake_join_all() {
    // This test reproduces issue #2460.
    with_timeout(Duration::from_secs(60), || {
        use futures::future::join_all;
        use tokio::task::LocalSet;

        let rt = rt();
        let set = LocalSet::new();
        let mut handles = Vec::new();

        for _ in 1..=128 {
            handles.push(set.spawn_local(async move {
                tokio::task::spawn_local(async move {}).await.unwrap();
            }));
        }

        rt.block_on(set.run_until(join_all(handles)));
    });
}

#[cfg(not(target_os = "wasi"))] // Wasi doesn't support panic recovery
#[test]
fn local_tasks_are_polled_after_tick() {
    // This test depends on timing, so we run it up to five times.
    for _ in 0..4 {
        let res = std::panic::catch_unwind(local_tasks_are_polled_after_tick_inner);
        if res.is_ok() {
            // success
            return;
        }
    }

    // Test failed 4 times. Try one more time without catching panics. If it
    // fails again, the test fails.
    local_tasks_are_polled_after_tick_inner();
}

#[cfg(not(target_os = "wasi"))] // Wasi doesn't support panic recovery
#[tokio::main(flavor = "current_thread")]
async fn local_tasks_are_polled_after_tick_inner() {
    // Reproduces issues #1899 and #1900

    static RX1: AtomicUsize = AtomicUsize::new(0);
    static RX2: AtomicUsize = AtomicUsize::new(0);
    const EXPECTED: usize = 500;

    RX1.store(0, SeqCst);
    RX2.store(0, SeqCst);

    let (tx, mut rx) = mpsc::unbounded_channel();

    let local = LocalSet::new();

    local
        .run_until(async {
            let task2 = task::spawn(async move {
                // Wait a bit
                time::sleep(Duration::from_millis(10)).await;

                let mut oneshots = Vec::with_capacity(EXPECTED);

                // Send values
                for _ in 0..EXPECTED {
                    let (oneshot_tx, oneshot_rx) = oneshot::channel();
                    oneshots.push(oneshot_tx);
                    tx.send(oneshot_rx).unwrap();
                }

                time::sleep(Duration::from_millis(10)).await;

                for tx in oneshots.drain(..) {
                    tx.send(()).unwrap();
                }

                loop {
                    time::sleep(Duration::from_millis(20)).await;
                    let rx1 = RX1.load(SeqCst);
                    let rx2 = RX2.load(SeqCst);

                    if rx1 == EXPECTED && rx2 == EXPECTED {
                        break;
                    }
                }
            });

            while let Some(oneshot) = rx.recv().await {
                RX1.fetch_add(1, SeqCst);

                task::spawn_local(async move {
                    oneshot.await.unwrap();
                    RX2.fetch_add(1, SeqCst);
                });
            }

            task2.await.unwrap();
        })
        .await;
}

#[tokio::test]
async fn acquire_mutex_in_drop() {
    use futures::future::pending;

    let (tx1, rx1) = oneshot::channel();
    let (tx2, rx2) = oneshot::channel();
    let local = LocalSet::new();

    local.spawn_local(async move {
        let _ = rx2.await;
        unreachable!();
    });

    local.spawn_local(async move {
        let _ = rx1.await;
        tx2.send(()).unwrap();
        unreachable!();
    });

    // Spawn a task that will never notify
    local.spawn_local(async move {
        pending::<()>().await;
        tx1.send(()).unwrap();
    });

    // Tick the loop
    local
        .run_until(async {
            task::yield_now().await;
        })
        .await;

    // Drop the LocalSet
    drop(local);
}

#[tokio::test]
async fn spawn_wakes_localset() {
    let local = LocalSet::new();
    futures::select! {
        _ = local.run_until(pending::<()>()).fuse() => unreachable!(),
        ret = async { local.spawn_local(ready(())).await.unwrap()}.fuse() => ret
    }
}

/// Checks that the task wakes up with `enter`.
/// Reproduces <https://github.com/tokio-rs/tokio/issues/5020>.
#[tokio::test]
async fn sleep_with_local_enter_guard() {
    let local = LocalSet::new();
    let _guard = local.enter();

    let (tx, rx) = oneshot::channel();

    local
        .run_until(async move {
            tokio::task::spawn_local(async move {
                time::sleep(Duration::ZERO).await;

                tx.send(()).expect("failed to send");
            });
            assert_eq!(rx.await, Ok(()));
        })
        .await;
}

#[test]
fn store_local_set_in_thread_local_with_runtime() {
    use tokio::runtime::Runtime;

    thread_local! {
        static CURRENT: RtAndLocalSet = RtAndLocalSet::new();
    }

    struct RtAndLocalSet {
        rt: Runtime,
        local: LocalSet,
    }

    impl RtAndLocalSet {
        fn new() -> RtAndLocalSet {
            RtAndLocalSet {
                rt: tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap(),
                local: LocalSet::new(),
            }
        }

        async fn inner_method(&self) {
            self.local
                .run_until(async move {
                    tokio::task::spawn_local(async {});
                })
                .await
        }

        fn method(&self) {
            self.rt.block_on(self.inner_method());
        }
    }

    CURRENT.with(|f| {
        f.method();
    });
}

#[cfg(tokio_unstable)]
mod unstable {
    use tokio::runtime::UnhandledPanic;
    use tokio::task::LocalSet;

    #[tokio::test]
    #[should_panic(
        expected = "a spawned task panicked and the LocalSet is configured to shutdown on unhandled panic"
    )]
    async fn shutdown_on_panic() {
        LocalSet::new()
            .unhandled_panic(UnhandledPanic::ShutdownRuntime)
            .run_until(async {
                tokio::task::spawn_local(async {
                    panic!("boom");
                });

                futures::future::pending::<()>().await;
            })
            .await;
    }

    // This test compares that, when the task driving `run_until` has already
    // consumed budget, the `run_until` future has less budget than a "spawned"
    // task.
    //
    // "Budget" is a fuzzy metric as the Tokio runtime is able to change values
    // internally. This is why the test uses indirection to test this.
    #[tokio::test]
    async fn run_until_does_not_get_own_budget() {
        // Consume some budget
        tokio::task::consume_budget().await;

        LocalSet::new()
            .run_until(async {
                let spawned = tokio::spawn(async {
                    let mut spawned_n = 0;

                    {
                        let mut spawned = tokio_test::task::spawn(async {
                            loop {
                                spawned_n += 1;
                                tokio::task::consume_budget().await;
                            }
                        });
                        // Poll once
                        assert!(!spawned.poll().is_ready());
                    }

                    spawned_n
                });

                let mut run_until_n = 0;
                {
                    let mut run_until = tokio_test::task::spawn(async {
                        loop {
                            run_until_n += 1;
                            tokio::task::consume_budget().await;
                        }
                    });
                    // Poll once
                    assert!(!run_until.poll().is_ready());
                }

                let spawned_n = spawned.await.unwrap();
                assert_ne!(spawned_n, 0);
                assert_ne!(run_until_n, 0);
                assert!(spawned_n > run_until_n);
            })
            .await
    }
}

fn rt() -> runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
}
