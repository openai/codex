#![warn(rust_2018_idioms)]
#![cfg(all(
    feature = "full",
    tokio_unstable,
    not(target_os = "wasi"),
    target_has_atomic = "64"
))]

use std::future::Future;
use std::sync::{mpsc, Arc, Mutex};
use std::task::Poll;
use std::thread;
use tokio::macros::support::poll_fn;

use tokio::runtime::{HistogramConfiguration, HistogramScale, LogHistogram, Runtime};
use tokio::task::consume_budget;
use tokio::time::{self, Duration};

#[test]
fn num_workers() {
    let rt = current_thread();
    assert_eq!(1, rt.metrics().num_workers());

    let rt = threaded();
    assert_eq!(2, rt.metrics().num_workers());
}

#[test]
fn num_blocking_threads() {
    let rt = current_thread();
    assert_eq!(0, rt.metrics().num_blocking_threads());
    let _ = rt.block_on(rt.spawn_blocking(move || {}));
    assert_eq!(1, rt.metrics().num_blocking_threads());

    let rt = threaded();
    assert_eq!(0, rt.metrics().num_blocking_threads());
    let _ = rt.block_on(rt.spawn_blocking(move || {}));
    assert_eq!(1, rt.metrics().num_blocking_threads());
}

#[test]
fn num_idle_blocking_threads() {
    let rt = current_thread();
    assert_eq!(0, rt.metrics().num_idle_blocking_threads());
    let _ = rt.block_on(rt.spawn_blocking(move || {}));
    rt.block_on(async {
        time::sleep(Duration::from_millis(5)).await;
    });

    // We need to wait until the blocking thread has become idle. Usually 5ms is
    // enough for this to happen, but not always. When it isn't enough, sleep
    // for another second. We don't always wait for a whole second since we want
    // the test suite to finish quickly.
    //
    // Note that the timeout for idle threads to be killed is 10 seconds.
    if 0 == rt.metrics().num_idle_blocking_threads() {
        rt.block_on(async {
            time::sleep(Duration::from_secs(1)).await;
        });
    }

    assert_eq!(1, rt.metrics().num_idle_blocking_threads());
}

#[test]
fn blocking_queue_depth() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .max_blocking_threads(1)
        .build()
        .unwrap();

    assert_eq!(0, rt.metrics().blocking_queue_depth());

    let ready = Arc::new(Mutex::new(()));
    let guard = ready.lock().unwrap();

    let ready_cloned = ready.clone();
    let wait_until_ready = move || {
        let _unused = ready_cloned.lock().unwrap();
    };

    let h1 = rt.spawn_blocking(wait_until_ready.clone());
    let h2 = rt.spawn_blocking(wait_until_ready);
    assert!(rt.metrics().blocking_queue_depth() > 0);

    drop(guard);

    let _ = rt.block_on(h1);
    let _ = rt.block_on(h2);

    assert_eq!(0, rt.metrics().blocking_queue_depth());
}

#[test]
fn spawned_tasks_count() {
    let rt = current_thread();
    let metrics = rt.metrics();
    assert_eq!(0, metrics.spawned_tasks_count());

    rt.block_on(rt.spawn(async move {
        assert_eq!(1, metrics.spawned_tasks_count());
    }))
    .unwrap();

    assert_eq!(1, rt.metrics().spawned_tasks_count());

    let rt = threaded();
    let metrics = rt.metrics();
    assert_eq!(0, metrics.spawned_tasks_count());

    rt.block_on(rt.spawn(async move {
        assert_eq!(1, metrics.spawned_tasks_count());
    }))
    .unwrap();

    assert_eq!(1, rt.metrics().spawned_tasks_count());
}

#[test]
fn remote_schedule_count() {
    use std::thread;

    let rt = current_thread();
    let handle = rt.handle().clone();
    let task = thread::spawn(move || {
        handle.spawn(async {
            // DO nothing
        })
    })
    .join()
    .unwrap();

    rt.block_on(task).unwrap();

    assert_eq!(1, rt.metrics().remote_schedule_count());

    let rt = threaded();
    let handle = rt.handle().clone();
    let task = thread::spawn(move || {
        handle.spawn(async {
            // DO nothing
        })
    })
    .join()
    .unwrap();

    rt.block_on(task).unwrap();

    assert_eq!(1, rt.metrics().remote_schedule_count());
}

#[test]
fn worker_thread_id_current_thread() {
    let rt = current_thread();
    let metrics = rt.metrics();

    // Check that runtime is on this thread.
    rt.block_on(async {});
    assert_eq!(Some(thread::current().id()), metrics.worker_thread_id(0));

    // Move runtime to another thread.
    let thread_id = std::thread::scope(|scope| {
        let join_handle = scope.spawn(|| {
            rt.block_on(async {});
        });
        join_handle.thread().id()
    });
    assert_eq!(Some(thread_id), metrics.worker_thread_id(0));

    // Move runtime back to this thread.
    rt.block_on(async {});
    assert_eq!(Some(thread::current().id()), metrics.worker_thread_id(0));
}

#[test]
fn worker_thread_id_threaded() {
    let rt = threaded();
    let metrics = rt.metrics();

    rt.block_on(rt.spawn(async move {
        // Check that we are running on a worker thread and determine
        // the index of our worker.
        let thread_id = std::thread::current().id();
        let this_worker = (0..2)
            .position(|w| metrics.worker_thread_id(w) == Some(thread_id))
            .expect("task not running on any worker thread");

        // Force worker to another thread.
        let moved_thread_id = tokio::task::block_in_place(|| {
            assert_eq!(thread_id, std::thread::current().id());

            // Wait for worker to move to another thread.
            for _ in 0..100 {
                let new_id = metrics.worker_thread_id(this_worker).unwrap();
                if thread_id != new_id {
                    return new_id;
                }
                std::thread::sleep(Duration::from_millis(100));
            }

            panic!("worker did not move to new thread");
        });

        // After blocking task worker either stays on new thread or
        // is moved back to current thread.
        assert!(
            metrics.worker_thread_id(this_worker) == Some(moved_thread_id)
                || metrics.worker_thread_id(this_worker) == Some(thread_id)
        );
    }))
    .unwrap()
}

#[test]
fn worker_noop_count() {
    // There isn't really a great way to generate no-op parks as they happen as
    // false-positive events under concurrency.

    let rt = current_thread();
    let metrics = rt.metrics();
    rt.block_on(async {
        time::sleep(Duration::from_millis(1)).await;
    });
    drop(rt);
    assert!(0 < metrics.worker_noop_count(0));

    let rt = threaded();
    let metrics = rt.metrics();
    rt.block_on(async {
        time::sleep(Duration::from_millis(1)).await;
    });
    drop(rt);
    assert!(0 < metrics.worker_noop_count(0));
    assert!(0 < metrics.worker_noop_count(1));
}

#[test]
fn worker_steal_count() {
    // This metric only applies to the multi-threaded runtime.
    for _ in 0..10 {
        let rt = threaded_no_lifo();
        let metrics = rt.metrics();

        let successfully_spawned_stealable_task = rt.block_on(async {
            // The call to `try_spawn_stealable_task` may time out, which means
            // that the sending task couldn't be scheduled due to a deadlock in
            // the runtime.
            // This is expected behaviour, we just retry until we succeed or
            // exhaust all tries, the latter causing this test to fail.
            try_spawn_stealable_task().await.is_ok()
        });

        drop(rt);

        if successfully_spawned_stealable_task {
            let n: u64 = (0..metrics.num_workers())
                .map(|i| metrics.worker_steal_count(i))
                .sum();

            assert_eq!(1, n);
            return;
        }
    }

    panic!("exhausted every try to schedule the stealable task");
}

#[test]
fn worker_poll_count_and_time() {
    const N: u64 = 5;

    async fn task() {
        // Sync sleep
        std::thread::sleep(std::time::Duration::from_micros(10));
    }

    let rt = current_thread();
    let metrics = rt.metrics();
    rt.block_on(async {
        for _ in 0..N {
            tokio::spawn(task()).await.unwrap();
        }
    });
    drop(rt);
    assert_eq!(N, metrics.worker_poll_count(0));
    // Not currently supported for current-thread runtime
    assert_eq!(Duration::default(), metrics.worker_mean_poll_time(0));

    // Does not populate the histogram
    assert!(!metrics.poll_time_histogram_enabled());
    for i in 0..10 {
        assert_eq!(0, metrics.poll_time_histogram_bucket_count(0, i));
    }

    let rt = threaded();
    let metrics = rt.metrics();
    rt.block_on(async {
        for _ in 0..N {
            tokio::spawn(task()).await.unwrap();
        }
    });
    drop(rt);
    // Account for the `block_on` task
    let n = (0..metrics.num_workers())
        .map(|i| metrics.worker_poll_count(i))
        .sum();

    assert_eq!(N, n);

    let n: Duration = (0..metrics.num_workers())
        .map(|i| metrics.worker_mean_poll_time(i))
        .sum();

    assert!(n > Duration::default());

    // Does not populate the histogram
    assert!(!metrics.poll_time_histogram_enabled());
    for n in 0..metrics.num_workers() {
        for i in 0..10 {
            assert_eq!(0, metrics.poll_time_histogram_bucket_count(n, i));
        }
    }
}

#[test]
fn log_histogram() {
    const N: u64 = 50;
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .enable_metrics_poll_time_histogram()
        .metrics_poll_time_histogram_configuration(HistogramConfiguration::log(
            LogHistogram::builder()
                .max_value(Duration::from_secs(60))
                .min_value(Duration::from_nanos(100))
                .max_error(0.25),
        ))
        .build()
        .unwrap();
    let metrics = rt.metrics();
    let num_buckets = rt.metrics().poll_time_histogram_num_buckets();
    assert_eq!(num_buckets, 119);
    rt.block_on(async {
        for _ in 0..N {
            tokio::spawn(async {}).await.unwrap();
        }
    });
    drop(rt);
    assert_eq!(
        metrics.poll_time_histogram_bucket_range(0),
        Duration::from_nanos(0)..Duration::from_nanos(96)
    );
    assert_eq!(
        metrics.poll_time_histogram_bucket_range(1),
        Duration::from_nanos(96)..Duration::from_nanos(96 + 2_u64.pow(4))
    );
    assert_eq!(
        metrics.poll_time_histogram_bucket_range(118).end,
        Duration::from_nanos(u64::MAX)
    );
    let n = (0..metrics.num_workers())
        .flat_map(|i| (0..num_buckets).map(move |j| (i, j)))
        .map(|(worker, bucket)| metrics.poll_time_histogram_bucket_count(worker, bucket))
        .sum();
    assert_eq!(N, n);
}

#[test]
fn minimal_log_histogram() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .enable_metrics_poll_time_histogram()
        .metrics_poll_time_histogram_configuration(HistogramConfiguration::log(
            LogHistogram::builder()
                .max_value(Duration::from_millis(4))
                .min_value(Duration::from_micros(20))
                .precision_exact(0),
        ))
        .build()
        .unwrap();
    let metrics = rt.metrics();
    let num_buckets = rt.metrics().poll_time_histogram_num_buckets();
    for b in 1..num_buckets - 1 {
        let range = metrics.poll_time_histogram_bucket_range(b);
        let size = range.end - range.start;
        // Assert the buckets continue doubling in size
        assert_eq!(
            size,
            Duration::from_nanos((1 << (b - 1)) * 16384),
            "incorrect range for {b}"
        );
    }
    assert_eq!(num_buckets, 10);
}

#[test]
#[allow(deprecated)]
fn legacy_log_histogram() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .enable_metrics_poll_time_histogram()
        .metrics_poll_count_histogram_scale(HistogramScale::Log)
        .metrics_poll_count_histogram_resolution(Duration::from_micros(50))
        .metrics_poll_count_histogram_buckets(20)
        .build()
        .unwrap();
    let num_buckets = rt.metrics().poll_time_histogram_num_buckets();
    assert_eq!(num_buckets, 20);
}

#[test]
fn log_histogram_default_configuration() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .enable_metrics_poll_time_histogram()
        .metrics_poll_time_histogram_configuration(HistogramConfiguration::log(
            LogHistogram::default(),
        ))
        .build()
        .unwrap();
    let num_buckets = rt.metrics().poll_time_histogram_num_buckets();
    assert_eq!(num_buckets, 119);
}

#[test]
fn worker_poll_count_histogram() {
    const N: u64 = 5;

    let rts = [
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .enable_metrics_poll_time_histogram()
            .metrics_poll_time_histogram_configuration(HistogramConfiguration::linear(
                Duration::from_millis(50),
                3,
            ))
            .build()
            .unwrap(),
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .enable_metrics_poll_time_histogram()
            .metrics_poll_time_histogram_configuration(HistogramConfiguration::linear(
                Duration::from_millis(50),
                3,
            ))
            .build()
            .unwrap(),
    ];

    for rt in rts {
        let metrics = rt.metrics();
        rt.block_on(async {
            for _ in 0..N {
                tokio::spawn(async {}).await.unwrap();
            }
        });
        drop(rt);

        let num_workers = metrics.num_workers();
        let num_buckets = metrics.poll_time_histogram_num_buckets();

        assert!(metrics.poll_time_histogram_enabled());
        assert_eq!(num_buckets, 3);

        let n = (0..num_workers)
            .flat_map(|i| (0..num_buckets).map(move |j| (i, j)))
            .map(|(worker, bucket)| metrics.poll_time_histogram_bucket_count(worker, bucket))
            .sum();
        assert_eq!(N, n);
    }
}

#[test]
fn worker_poll_count_histogram_range() {
    let max = Duration::from_nanos(u64::MAX);

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .enable_metrics_poll_time_histogram()
        .metrics_poll_time_histogram_configuration(HistogramConfiguration::linear(us(50), 3))
        .build()
        .unwrap();
    let metrics = rt.metrics();

    assert_eq!(metrics.poll_time_histogram_bucket_range(0), us(0)..us(50));
    assert_eq!(metrics.poll_time_histogram_bucket_range(1), us(50)..us(100));
    assert_eq!(metrics.poll_time_histogram_bucket_range(2), us(100)..max);

    // ensure the old methods work too
    #[allow(deprecated)]
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .enable_metrics_poll_time_histogram()
        .metrics_poll_count_histogram_scale(tokio::runtime::HistogramScale::Log)
        .metrics_poll_count_histogram_buckets(3)
        .metrics_poll_count_histogram_resolution(us(50))
        .build()
        .unwrap();
    let metrics = rt.metrics();

    let a = Duration::from_nanos(50000_u64.next_power_of_two());
    let b = a * 2;

    assert_eq!(metrics.poll_time_histogram_bucket_range(0), us(0)..a);
    assert_eq!(metrics.poll_time_histogram_bucket_range(1), a..b);
    assert_eq!(metrics.poll_time_histogram_bucket_range(2), b..max);
}

#[test]
fn worker_poll_count_histogram_disabled_without_explicit_enable() {
    let rts = [
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .metrics_poll_time_histogram_configuration(HistogramConfiguration::linear(
                Duration::from_millis(50),
                3,
            ))
            .build()
            .unwrap(),
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .metrics_poll_time_histogram_configuration(HistogramConfiguration::linear(
                Duration::from_millis(50),
                3,
            ))
            .build()
            .unwrap(),
    ];

    for rt in rts {
        let metrics = rt.metrics();
        assert!(!metrics.poll_time_histogram_enabled());
    }
}

#[test]
fn worker_local_schedule_count() {
    let rt = current_thread();
    let metrics = rt.metrics();
    rt.block_on(async {
        tokio::spawn(async {}).await.unwrap();
    });
    drop(rt);

    assert_eq!(1, metrics.worker_local_schedule_count(0));
    assert_eq!(0, metrics.remote_schedule_count());

    let rt = threaded();
    let metrics = rt.metrics();
    rt.block_on(async {
        // Move to the runtime
        tokio::spawn(async {
            tokio::spawn(async {}).await.unwrap();
        })
        .await
        .unwrap();
    });
    drop(rt);

    let n: u64 = (0..metrics.num_workers())
        .map(|i| metrics.worker_local_schedule_count(i))
        .sum();

    assert_eq!(2, n);
    assert_eq!(1, metrics.remote_schedule_count());
}

#[test]
fn worker_overflow_count() {
    // Only applies to the threaded worker
    let rt = threaded();
    let metrics = rt.metrics();
    rt.block_on(async {
        // Move to the runtime
        tokio::spawn(async {
            let (tx1, rx1) = std::sync::mpsc::channel();
            let (tx2, rx2) = std::sync::mpsc::channel();

            // First, we need to block the other worker until all tasks have
            // been spawned.
            //
            // We spawn from outside the runtime to ensure that the other worker
            // will pick it up:
            // <https://github.com/tokio-rs/tokio/issues/4730>
            tokio::task::spawn_blocking(|| {
                tokio::spawn(async move {
                    tx1.send(()).unwrap();
                    rx2.recv().unwrap();
                });
            });

            rx1.recv().unwrap();

            // Spawn many tasks
            for _ in 0..300 {
                tokio::spawn(async {});
            }

            tx2.send(()).unwrap();
        })
        .await
        .unwrap();
    });
    drop(rt);

    let n: u64 = (0..metrics.num_workers())
        .map(|i| metrics.worker_overflow_count(i))
        .sum();

    assert_eq!(1, n);
}

#[test]
fn worker_local_queue_depth() {
    const N: usize = 100;

    let rt = current_thread();
    let metrics = rt.metrics();
    rt.block_on(async {
        for _ in 0..N {
            tokio::spawn(async {});
        }

        assert_eq!(N, metrics.worker_local_queue_depth(0));
    });

    let rt = threaded();
    let metrics = rt.metrics();
    rt.block_on(async move {
        // Move to the runtime
        tokio::spawn(async move {
            let (tx1, rx1) = std::sync::mpsc::channel();
            let (tx2, rx2) = std::sync::mpsc::channel();

            // First, we need to block the other worker until all tasks have
            // been spawned.
            tokio::spawn(async move {
                tx1.send(()).unwrap();
                rx2.recv().unwrap();
            });

            // Bump the next-run spawn
            tokio::spawn(async {});

            rx1.recv().unwrap();

            // Spawn some tasks
            for _ in 0..100 {
                tokio::spawn(async {});
            }

            let n: usize = (0..metrics.num_workers())
                .map(|i| metrics.worker_local_queue_depth(i))
                .sum();

            assert_eq!(n, N);

            tx2.send(()).unwrap();
        })
        .await
        .unwrap();
    });
}

#[test]
fn budget_exhaustion_yield() {
    let rt = current_thread();
    let metrics = rt.metrics();

    assert_eq!(0, metrics.budget_forced_yield_count());

    let mut did_yield = false;

    // block on a task which consumes budget until it yields
    rt.block_on(poll_fn(|cx| loop {
        if did_yield {
            return Poll::Ready(());
        }

        let fut = consume_budget();
        tokio::pin!(fut);

        if fut.poll(cx).is_pending() {
            did_yield = true;
            return Poll::Pending;
        }
    }));

    assert_eq!(1, rt.metrics().budget_forced_yield_count());
}

#[test]
fn budget_exhaustion_yield_with_joins() {
    let rt = current_thread();
    let metrics = rt.metrics();

    assert_eq!(0, metrics.budget_forced_yield_count());

    let mut did_yield_1 = false;
    let mut did_yield_2 = false;

    // block on a task which consumes budget until it yields
    rt.block_on(async {
        tokio::join!(
            poll_fn(|cx| loop {
                if did_yield_1 {
                    return Poll::Ready(());
                }

                let fut = consume_budget();
                tokio::pin!(fut);

                if fut.poll(cx).is_pending() {
                    did_yield_1 = true;
                    return Poll::Pending;
                }
            }),
            poll_fn(|cx| loop {
                if did_yield_2 {
                    return Poll::Ready(());
                }

                let fut = consume_budget();
                tokio::pin!(fut);

                if fut.poll(cx).is_pending() {
                    did_yield_2 = true;
                    return Poll::Pending;
                }
            })
        )
    });

    assert_eq!(1, rt.metrics().budget_forced_yield_count());
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn io_driver_fd_count() {
    let rt = current_thread();
    let metrics = rt.metrics();

    assert_eq!(metrics.io_driver_fd_registered_count(), 0);

    let stream = tokio::net::TcpStream::connect("google.com:80");
    let stream = rt.block_on(async move { stream.await.unwrap() });

    assert_eq!(metrics.io_driver_fd_registered_count(), 1);
    assert_eq!(metrics.io_driver_fd_deregistered_count(), 0);

    drop(stream);

    assert_eq!(metrics.io_driver_fd_deregistered_count(), 1);
    assert_eq!(metrics.io_driver_fd_registered_count(), 1);
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn io_driver_ready_count() {
    let rt = current_thread();
    let metrics = rt.metrics();

    let stream = tokio::net::TcpStream::connect("google.com:80");
    let _stream = rt.block_on(async move { stream.await.unwrap() });

    assert_eq!(metrics.io_driver_ready_count(), 1);
}

async fn try_spawn_stealable_task() -> Result<(), mpsc::RecvTimeoutError> {
    // We use a blocking channel to synchronize the tasks.
    let (tx, rx) = mpsc::channel();

    // Make sure we are in the context of the runtime.
    tokio::spawn(async move {
        // Spawn the task that sends to the channel.
        //
        // Note that the runtime needs to have the lifo slot disabled to make
        // this task stealable.
        tokio::spawn(async move {
            tx.send(()).unwrap();
        });

        // Blocking receive on the channel, timing out if the sending task
        // wasn't scheduled in time.
        rx.recv_timeout(Duration::from_secs(1))
    })
    .await
    .unwrap()?;

    Ok(())
}

fn current_thread() -> Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
}

fn threaded() -> Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap()
}

fn threaded_no_lifo() -> Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .disable_lifo_slot()
        .enable_all()
        .build()
        .unwrap()
}

fn us(n: u64) -> Duration {
    Duration::from_micros(n)
}
