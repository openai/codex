#![warn(rust_2018_idioms)]
// Too slow on miri.
#![cfg(all(feature = "full", not(target_os = "wasi"), not(miri)))]

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::runtime;
use tokio::sync::oneshot;
use tokio_test::{assert_err, assert_ok};

use std::future::{poll_fn, Future};
use std::pin::Pin;
use std::sync::atomic::Ordering::Relaxed;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::task::{Context, Poll, Waker};

macro_rules! cfg_metrics {
    ($($t:tt)*) => {
        #[cfg(all(tokio_unstable, target_has_atomic = "64"))]
        {
            $( $t )*
        }
    }
}

#[test]
fn single_thread() {
    // No panic when starting a runtime w/ a single thread
    let _ = runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(1)
        .build()
        .unwrap();
}

#[test]
fn many_oneshot_futures() {
    // used for notifying the main thread
    const NUM: usize = 1_000;

    for _ in 0..5 {
        let (tx, rx) = mpsc::channel();

        let rt = rt();
        let cnt = Arc::new(AtomicUsize::new(0));

        for _ in 0..NUM {
            let cnt = cnt.clone();
            let tx = tx.clone();

            rt.spawn(async move {
                let num = cnt.fetch_add(1, Relaxed) + 1;

                if num == NUM {
                    tx.send(()).unwrap();
                }
            });
        }

        rx.recv().unwrap();

        // Wait for the pool to shutdown
        drop(rt);
    }
}

#[test]
fn spawn_two() {
    let rt = rt();

    let out = rt.block_on(async {
        let (tx, rx) = oneshot::channel();

        tokio::spawn(async move {
            tokio::spawn(async move {
                tx.send("ZOMG").unwrap();
            });
        });

        assert_ok!(rx.await)
    });

    assert_eq!(out, "ZOMG");

    cfg_metrics! {
        let metrics = rt.metrics();
        drop(rt);
        assert_eq!(1, metrics.remote_schedule_count());

        let mut local = 0;
        for i in 0..metrics.num_workers() {
            local += metrics.worker_local_schedule_count(i);
        }

        assert_eq!(1, local);
    }
}

#[test]
fn many_multishot_futures() {
    const CHAIN: usize = 200;
    const CYCLES: usize = 5;
    const TRACKS: usize = 50;

    for _ in 0..50 {
        let rt = rt();
        let mut start_txs = Vec::with_capacity(TRACKS);
        let mut final_rxs = Vec::with_capacity(TRACKS);

        for _ in 0..TRACKS {
            let (start_tx, mut chain_rx) = tokio::sync::mpsc::channel(10);

            for _ in 0..CHAIN {
                let (next_tx, next_rx) = tokio::sync::mpsc::channel(10);

                // Forward all the messages
                rt.spawn(async move {
                    while let Some(v) = chain_rx.recv().await {
                        next_tx.send(v).await.unwrap();
                    }
                });

                chain_rx = next_rx;
            }

            // This final task cycles if needed
            let (final_tx, final_rx) = tokio::sync::mpsc::channel(10);
            let cycle_tx = start_tx.clone();
            let mut rem = CYCLES;

            rt.spawn(async move {
                for _ in 0..CYCLES {
                    let msg = chain_rx.recv().await.unwrap();

                    rem -= 1;

                    if rem == 0 {
                        final_tx.send(msg).await.unwrap();
                    } else {
                        cycle_tx.send(msg).await.unwrap();
                    }
                }
            });

            start_txs.push(start_tx);
            final_rxs.push(final_rx);
        }

        {
            rt.block_on(async move {
                for start_tx in start_txs {
                    start_tx.send("ping").await.unwrap();
                }

                for mut final_rx in final_rxs {
                    final_rx.recv().await.unwrap();
                }
            });
        }
    }
}

#[test]
fn lifo_slot_budget() {
    async fn my_fn() {
        spawn_another();
    }

    fn spawn_another() {
        tokio::spawn(my_fn());
    }

    let rt = runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(1)
        .build()
        .unwrap();

    let (send, recv) = oneshot::channel();

    rt.spawn(async move {
        tokio::spawn(my_fn());
        let _ = send.send(());
    });

    let _ = rt.block_on(recv);
}

#[test]
#[cfg_attr(miri, ignore)] // No `socket` in miri.
fn spawn_shutdown() {
    let rt = rt();
    let (tx, rx) = mpsc::channel();

    rt.block_on(async {
        tokio::spawn(client_server(tx.clone()));
    });

    // Use spawner
    rt.spawn(client_server(tx));

    assert_ok!(rx.recv());
    assert_ok!(rx.recv());

    drop(rt);
    assert_err!(rx.try_recv());
}

async fn client_server(tx: mpsc::Sender<()>) {
    let server = assert_ok!(TcpListener::bind("127.0.0.1:0").await);

    // Get the assigned address
    let addr = assert_ok!(server.local_addr());

    // Spawn the server
    tokio::spawn(async move {
        // Accept a socket
        let (mut socket, _) = server.accept().await.unwrap();

        // Write some data
        socket.write_all(b"hello").await.unwrap();
    });

    let mut client = TcpStream::connect(&addr).await.unwrap();

    let mut buf = vec![];
    client.read_to_end(&mut buf).await.unwrap();

    assert_eq!(buf, b"hello");
    tx.send(()).unwrap();
}

#[test]
fn drop_threadpool_drops_futures() {
    for _ in 0..1_000 {
        let num_inc = Arc::new(AtomicUsize::new(0));
        let num_dec = Arc::new(AtomicUsize::new(0));
        let num_drop = Arc::new(AtomicUsize::new(0));

        struct Never(Arc<AtomicUsize>);

        impl Future for Never {
            type Output = ();

            fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<()> {
                Poll::Pending
            }
        }

        impl Drop for Never {
            fn drop(&mut self) {
                self.0.fetch_add(1, Relaxed);
            }
        }

        let a = num_inc.clone();
        let b = num_dec.clone();

        let rt = runtime::Builder::new_multi_thread()
            .enable_all()
            .on_thread_start(move || {
                a.fetch_add(1, Relaxed);
            })
            .on_thread_stop(move || {
                b.fetch_add(1, Relaxed);
            })
            .build()
            .unwrap();

        rt.spawn(Never(num_drop.clone()));

        // Wait for the pool to shutdown
        drop(rt);

        // Assert that only a single thread was spawned.
        let a = num_inc.load(Relaxed);
        assert!(a >= 1);

        // Assert that all threads shutdown
        let b = num_dec.load(Relaxed);
        assert_eq!(a, b);

        // Assert that the future was dropped
        let c = num_drop.load(Relaxed);
        assert_eq!(c, 1);
    }
}

#[test]
fn start_stop_callbacks_called() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    let after_start = Arc::new(AtomicUsize::new(0));
    let before_stop = Arc::new(AtomicUsize::new(0));

    let after_inner = after_start.clone();
    let before_inner = before_stop.clone();
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .on_thread_start(move || {
            after_inner.clone().fetch_add(1, Ordering::Relaxed);
        })
        .on_thread_stop(move || {
            before_inner.clone().fetch_add(1, Ordering::Relaxed);
        })
        .build()
        .unwrap();

    let (tx, rx) = oneshot::channel();

    rt.spawn(async move {
        assert_ok!(tx.send(()));
    });

    assert_ok!(rt.block_on(rx));

    drop(rt);

    assert!(after_start.load(Ordering::Relaxed) > 0);
    assert!(before_stop.load(Ordering::Relaxed) > 0);
}

#[test]
// too slow on miri
#[cfg_attr(miri, ignore)]
fn blocking() {
    // used for notifying the main thread
    const NUM: usize = 1_000;

    for _ in 0..10 {
        let (tx, rx) = mpsc::channel();

        let rt = rt();
        let cnt = Arc::new(AtomicUsize::new(0));

        // there are four workers in the pool
        // so, if we run 4 blocking tasks, we know that handoff must have happened
        let block = Arc::new(std::sync::Barrier::new(5));
        for _ in 0..4 {
            let block = block.clone();
            rt.spawn(async move {
                tokio::task::block_in_place(move || {
                    block.wait();
                    block.wait();
                })
            });
        }
        block.wait();

        for _ in 0..NUM {
            let cnt = cnt.clone();
            let tx = tx.clone();

            rt.spawn(async move {
                let num = cnt.fetch_add(1, Relaxed) + 1;

                if num == NUM {
                    tx.send(()).unwrap();
                }
            });
        }

        rx.recv().unwrap();

        // Wait for the pool to shutdown
        block.wait();
    }
}

#[test]
fn multi_threadpool() {
    use tokio::sync::oneshot;

    let rt1 = rt();
    let rt2 = rt();

    let (tx, rx) = oneshot::channel();
    let (done_tx, done_rx) = mpsc::channel();

    rt2.spawn(async move {
        rx.await.unwrap();
        done_tx.send(()).unwrap();
    });

    rt1.spawn(async move {
        tx.send(()).unwrap();
    });

    done_rx.recv().unwrap();
}

// When `block_in_place` returns, it attempts to reclaim the yielded runtime
// worker. In this case, the remainder of the task is on the runtime worker and
// must take part in the cooperative task budgeting system.
//
// The test ensures that, when this happens, attempting to consume from a
// channel yields occasionally even if there are values ready to receive.
#[test]
fn coop_and_block_in_place() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        // Setting max threads to 1 prevents another thread from claiming the
        // runtime worker yielded as part of `block_in_place` and guarantees the
        // same thread will reclaim the worker at the end of the
        // `block_in_place` call.
        .max_blocking_threads(1)
        .build()
        .unwrap();

    rt.block_on(async move {
        let (tx, mut rx) = tokio::sync::mpsc::channel(1024);

        // Fill the channel
        for _ in 0..1024 {
            tx.send(()).await.unwrap();
        }

        drop(tx);

        tokio::spawn(async move {
            // Block in place without doing anything
            tokio::task::block_in_place(|| {});

            // Receive all the values, this should trigger a `Pending` as the
            // coop limit will be reached.
            poll_fn(|cx| {
                while let Poll::Ready(v) = {
                    tokio::pin! {
                        let fut = rx.recv();
                    }

                    Pin::new(&mut fut).poll(cx)
                } {
                    if v.is_none() {
                        panic!("did not yield");
                    }
                }

                Poll::Ready(())
            })
            .await
        })
        .await
        .unwrap();
    });
}

#[test]
fn yield_after_block_in_place() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .build()
        .unwrap();

    rt.block_on(async {
        tokio::spawn(async move {
            // Block in place then enter a new runtime
            tokio::task::block_in_place(|| {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .build()
                    .unwrap();

                rt.block_on(async {});
            });

            // Yield, then complete
            tokio::task::yield_now().await;
        })
        .await
        .unwrap()
    });
}

// Testing this does not panic
#[test]
fn max_blocking_threads() {
    let _rt = tokio::runtime::Builder::new_multi_thread()
        .max_blocking_threads(1)
        .build()
        .unwrap();
}

#[test]
#[should_panic]
fn max_blocking_threads_set_to_zero() {
    let _rt = tokio::runtime::Builder::new_multi_thread()
        .max_blocking_threads(0)
        .build()
        .unwrap();
}

/// Regression test for #6445.
///
/// After #6445, setting `global_queue_interval` to 1 is now technically valid.
/// This test confirms that there is no regression in `multi_thread_runtime`
/// when global_queue_interval is set to 1.
#[test]
fn global_queue_interval_set_to_one() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .global_queue_interval(1)
        .build()
        .unwrap();

    // Perform a simple work.
    let cnt = Arc::new(AtomicUsize::new(0));
    rt.block_on(async {
        let mut set = tokio::task::JoinSet::new();
        for _ in 0..10 {
            let cnt = cnt.clone();
            set.spawn(async move { cnt.fetch_add(1, Ordering::Relaxed) });
        }

        while let Some(res) = set.join_next().await {
            res.unwrap();
        }
    });
    assert_eq!(cnt.load(Relaxed), 10);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn hang_on_shutdown() {
    let (sync_tx, sync_rx) = std::sync::mpsc::channel::<()>();
    tokio::spawn(async move {
        tokio::task::block_in_place(|| sync_rx.recv().ok());
    });

    tokio::spawn(async {
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        drop(sync_tx);
    });
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
}

/// Demonstrates tokio-rs/tokio#3869
#[test]
fn wake_during_shutdown() {
    struct Shared {
        waker: Option<Waker>,
    }

    struct MyFuture {
        shared: Arc<Mutex<Shared>>,
        put_waker: bool,
    }

    impl MyFuture {
        fn new() -> (Self, Self) {
            let shared = Arc::new(Mutex::new(Shared { waker: None }));
            let f1 = MyFuture {
                shared: shared.clone(),
                put_waker: true,
            };
            let f2 = MyFuture {
                shared,
                put_waker: false,
            };
            (f1, f2)
        }
    }

    impl Future for MyFuture {
        type Output = ();

        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
            let me = Pin::into_inner(self);
            let mut lock = me.shared.lock().unwrap();
            if me.put_waker {
                lock.waker = Some(cx.waker().clone());
            }
            Poll::Pending
        }
    }

    impl Drop for MyFuture {
        fn drop(&mut self) {
            let mut lock = self.shared.lock().unwrap();
            if !self.put_waker {
                lock.waker.take().unwrap().wake();
            }
            drop(lock);
        }
    }

    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .unwrap();

    let (f1, f2) = MyFuture::new();

    rt.spawn(f1);
    rt.spawn(f2);

    rt.block_on(async { tokio::time::sleep(tokio::time::Duration::from_millis(20)).await });
}

#[should_panic]
#[tokio::test]
async fn test_block_in_place1() {
    tokio::task::block_in_place(|| {});
}

#[tokio::test(flavor = "multi_thread")]
async fn test_block_in_place2() {
    tokio::task::block_in_place(|| {});
}

#[should_panic]
#[tokio::main(flavor = "current_thread")]
#[test]
async fn test_block_in_place3() {
    tokio::task::block_in_place(|| {});
}

#[tokio::main]
#[test]
async fn test_block_in_place4() {
    tokio::task::block_in_place(|| {});
}

// Repro for tokio-rs/tokio#5239
#[test]
fn test_nested_block_in_place_with_block_on_between() {
    let rt = runtime::Builder::new_multi_thread()
        .worker_threads(1)
        // Needs to be more than 0
        .max_blocking_threads(1)
        .build()
        .unwrap();

    // Triggered by a race condition, so run a few times to make sure it is OK.
    for _ in 0..100 {
        let h = rt.handle().clone();

        rt.block_on(async move {
            tokio::spawn(async move {
                tokio::task::block_in_place(|| {
                    h.block_on(async {
                        tokio::task::block_in_place(|| {});
                    });
                })
            })
            .await
            .unwrap()
        });
    }
}

#[test]
fn yield_now_in_block_in_place() {
    let rt = runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .build()
        .unwrap();

    rt.block_on(async {
        tokio::spawn(async {
            tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(tokio::task::yield_now());
            })
        })
        .await
        .unwrap()
    })
}

#[test]
fn mutex_in_block_in_place() {
    const BUDGET: usize = 128;

    let rt = runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .build()
        .unwrap();

    rt.block_on(async {
        let lock = tokio::sync::Mutex::new(0);

        tokio::spawn(async move {
            tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async move {
                    for i in 0..(BUDGET + 1) {
                        let mut guard = lock.lock().await;
                        *guard = i;
                    }
                });
            })
        })
        .await
        .unwrap();
    })
}

// Testing the tuning logic is tricky as it is inherently timing based, and more
// of a heuristic than an exact behavior. This test checks that the interval
// changes over time based on load factors. There are no assertions, completion
// is sufficient. If there is a regression, this test will hang. In theory, we
// could add limits, but that would be likely to fail on CI.
#[test]
#[cfg(not(tokio_no_tuning_tests))]
fn test_tuning() {
    use std::sync::atomic::AtomicBool;
    use std::time::Duration;

    let rt = runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .build()
        .unwrap();

    fn iter(flag: Arc<AtomicBool>, counter: Arc<AtomicUsize>, stall: bool) {
        if flag.load(Relaxed) {
            if stall {
                std::thread::sleep(Duration::from_micros(5));
            }

            counter.fetch_add(1, Relaxed);
            tokio::spawn(async move { iter(flag, counter, stall) });
        }
    }

    let flag = Arc::new(AtomicBool::new(true));
    let counter = Arc::new(AtomicUsize::new(61));
    let interval = Arc::new(AtomicUsize::new(61));

    {
        let flag = flag.clone();
        let counter = counter.clone();
        rt.spawn(async move { iter(flag, counter, true) });
    }

    // Now, hammer the injection queue until the interval drops.
    let mut n = 0;
    loop {
        let curr = interval.load(Relaxed);

        if curr <= 8 {
            n += 1;
        } else {
            n = 0;
        }

        // Make sure we get a few good rounds. Jitter in the tuning could result
        // in one "good" value without being representative of reaching a good
        // state.
        if n == 3 {
            break;
        }

        if Arc::strong_count(&interval) < 5_000 {
            let counter = counter.clone();
            let interval = interval.clone();

            rt.spawn(async move {
                let prev = counter.swap(0, Relaxed);
                interval.store(prev, Relaxed);
            });

            std::thread::yield_now();
        }
    }

    flag.store(false, Relaxed);

    let w = Arc::downgrade(&interval);
    drop(interval);

    while w.strong_count() > 0 {
        std::thread::sleep(Duration::from_micros(500));
    }

    // Now, run it again with a faster task
    let flag = Arc::new(AtomicBool::new(true));
    // Set it high, we know it shouldn't ever really be this high
    let counter = Arc::new(AtomicUsize::new(10_000));
    let interval = Arc::new(AtomicUsize::new(10_000));

    {
        let flag = flag.clone();
        let counter = counter.clone();
        rt.spawn(async move { iter(flag, counter, false) });
    }

    // Now, hammer the injection queue until the interval reaches the expected range.
    let mut n = 0;
    loop {
        let curr = interval.load(Relaxed);

        if curr <= 1_000 && curr > 32 {
            n += 1;
        } else {
            n = 0;
        }

        if n == 3 {
            break;
        }

        if Arc::strong_count(&interval) <= 5_000 {
            let counter = counter.clone();
            let interval = interval.clone();

            rt.spawn(async move {
                let prev = counter.swap(0, Relaxed);
                interval.store(prev, Relaxed);
            });
        }

        std::thread::yield_now();
    }

    flag.store(false, Relaxed);
}

fn rt() -> runtime::Runtime {
    runtime::Runtime::new().unwrap()
}

#[cfg(tokio_unstable)]
mod unstable {
    use super::*;

    #[test]
    fn test_disable_lifo_slot() {
        use std::sync::mpsc::{channel, RecvTimeoutError};

        let rt = runtime::Builder::new_multi_thread()
            .disable_lifo_slot()
            .worker_threads(2)
            .build()
            .unwrap();

        // Spawn a background thread to poke the runtime periodically.
        //
        // This is necessary because we may end up triggering the issue in:
        // <https://github.com/tokio-rs/tokio/issues/4730>
        //
        // Spawning a task will wake up the second worker, which will then steal
        // the task. However, the steal will fail if the task is in the LIFO
        // slot, because the LIFO slot cannot be stolen.
        //
        // Note that this only happens rarely. Most of the time, this thread is
        // not necessary.
        let (kill_bg_thread, recv) = channel::<()>();
        let handle = rt.handle().clone();
        let bg_thread = std::thread::spawn(move || {
            let one_sec = std::time::Duration::from_secs(1);
            while recv.recv_timeout(one_sec) == Err(RecvTimeoutError::Timeout) {
                handle.spawn(async {});
            }
        });

        rt.block_on(async {
            tokio::spawn(async {
                // Spawn another task and block the thread until completion. If the LIFO slot
                // is used then the test doesn't complete.
                futures::executor::block_on(tokio::spawn(async {})).unwrap();
            })
            .await
            .unwrap();
        });

        drop(kill_bg_thread);
        bg_thread.join().unwrap();
    }

    #[test]
    fn runtime_id_is_same() {
        let rt = rt();

        let handle1 = rt.handle();
        let handle2 = rt.handle();

        assert_eq!(handle1.id(), handle2.id());
    }

    #[test]
    fn runtime_ids_different() {
        let rt1 = rt();
        let rt2 = rt();

        assert_ne!(rt1.handle().id(), rt2.handle().id());
    }
}
