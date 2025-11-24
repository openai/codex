#![warn(rust_2018_idioms)]
#![cfg(all(feature = "full", not(target_os = "wasi"), target_has_atomic = "64"))]

use std::sync::mpsc;
use std::time::Duration;
use tokio::runtime::Runtime;
use tokio::time;

#[test]
fn num_workers() {
    let rt = current_thread();
    assert_eq!(1, rt.metrics().num_workers());

    let rt = threaded();
    assert_eq!(2, rt.metrics().num_workers());
}

#[test]
fn num_alive_tasks() {
    let rt = current_thread();
    let metrics = rt.metrics();
    assert_eq!(0, metrics.num_alive_tasks());
    rt.block_on(rt.spawn(async move {
        assert_eq!(1, metrics.num_alive_tasks());
    }))
    .unwrap();

    assert_eq!(0, rt.metrics().num_alive_tasks());

    let rt = threaded();
    let metrics = rt.metrics();
    assert_eq!(0, metrics.num_alive_tasks());
    rt.block_on(rt.spawn(async move {
        assert_eq!(1, metrics.num_alive_tasks());
    }))
    .unwrap();

    // try for 10 seconds to see if this eventually succeeds.
    // wake_join() is called before the task is released, so in multithreaded
    // code, this means we sometimes exit the block_on before the counter decrements.
    for _ in 0..100 {
        if rt.metrics().num_alive_tasks() == 0 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    assert_eq!(0, rt.metrics().num_alive_tasks());
}

#[test]
fn global_queue_depth_current_thread() {
    use std::thread;

    let rt = current_thread();
    let handle = rt.handle().clone();
    let metrics = rt.metrics();

    thread::spawn(move || {
        handle.spawn(async {});
    })
    .join()
    .unwrap();

    assert_eq!(1, metrics.global_queue_depth());
}

#[test]
fn global_queue_depth_multi_thread() {
    for _ in 0..10 {
        let rt = threaded();
        let metrics = rt.metrics();

        if let Ok(_blocking_tasks) = try_block_threaded(&rt) {
            for i in 0..10 {
                assert_eq!(i, metrics.global_queue_depth());
                rt.spawn(async {});
            }

            return;
        }
    }

    panic!("exhausted every try to block the runtime");
}

#[test]
fn worker_total_busy_duration() {
    const N: usize = 5;

    let zero = Duration::from_millis(0);

    let rt = current_thread();
    let metrics = rt.metrics();

    rt.block_on(async {
        for _ in 0..N {
            tokio::spawn(async {
                tokio::task::yield_now().await;
            })
            .await
            .unwrap();
        }
    });

    drop(rt);

    assert!(zero < metrics.worker_total_busy_duration(0));

    let rt = threaded();
    let metrics = rt.metrics();

    rt.block_on(async {
        for _ in 0..N {
            tokio::spawn(async {
                tokio::task::yield_now().await;
            })
            .await
            .unwrap();
        }
    });

    drop(rt);

    for i in 0..metrics.num_workers() {
        assert!(zero < metrics.worker_total_busy_duration(i));
    }
}

#[test]
fn worker_park_count() {
    let rt = current_thread();
    let metrics = rt.metrics();
    rt.block_on(async {
        time::sleep(Duration::from_millis(1)).await;
    });
    drop(rt);
    assert!(1 <= metrics.worker_park_count(0));

    let rt = threaded();
    let metrics = rt.metrics();
    rt.block_on(async {
        time::sleep(Duration::from_millis(1)).await;
    });
    drop(rt);
    assert!(1 <= metrics.worker_park_count(0));
    assert!(1 <= metrics.worker_park_count(1));
}

#[test]
fn worker_park_unpark_count() {
    let rt = current_thread();
    let metrics = rt.metrics();
    rt.block_on(rt.spawn(async {})).unwrap();
    drop(rt);
    assert!(2 <= metrics.worker_park_unpark_count(0));

    let rt = threaded();
    let metrics = rt.metrics();

    // Wait for workers to be parked after runtime startup.
    for _ in 0..100 {
        if 1 <= metrics.worker_park_unpark_count(0) && 1 <= metrics.worker_park_unpark_count(1) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    assert_eq!(1, metrics.worker_park_unpark_count(0));
    assert_eq!(1, metrics.worker_park_unpark_count(1));

    // Spawn a task to unpark and then park a worker.
    rt.block_on(rt.spawn(async {})).unwrap();
    for _ in 0..100 {
        if 3 <= metrics.worker_park_unpark_count(0) || 3 <= metrics.worker_park_unpark_count(1) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    assert!(3 <= metrics.worker_park_unpark_count(0) || 3 <= metrics.worker_park_unpark_count(1));

    // Both threads unpark for runtime shutdown.
    drop(rt);
    assert_eq!(0, metrics.worker_park_unpark_count(0) % 2);
    assert_eq!(0, metrics.worker_park_unpark_count(1) % 2);
    assert!(4 <= metrics.worker_park_unpark_count(0) || 4 <= metrics.worker_park_unpark_count(1));
}

fn try_block_threaded(rt: &Runtime) -> Result<Vec<mpsc::Sender<()>>, mpsc::RecvTimeoutError> {
    let (tx, rx) = mpsc::channel();

    let blocking_tasks = (0..rt.metrics().num_workers())
        .map(|_| {
            let tx = tx.clone();
            let (task, barrier) = mpsc::channel();

            // Spawn a task per runtime worker to block it.
            rt.spawn(async move {
                tx.send(()).ok();
                barrier.recv().ok();
            });

            task
        })
        .collect();

    // Make sure the previously spawned tasks are blocking the runtime by
    // receiving a message from each blocking task.
    //
    // If this times out we were unsuccessful in blocking the runtime and hit
    // a deadlock instead (which might happen and is expected behaviour).
    for _ in 0..rt.metrics().num_workers() {
        rx.recv_timeout(Duration::from_secs(1))?;
    }

    // Return senders of the mpsc channels used for blocking the runtime as a
    // surrogate handle for the tasks. Sending a message or dropping the senders
    // will unblock the runtime.
    Ok(blocking_tasks)
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
