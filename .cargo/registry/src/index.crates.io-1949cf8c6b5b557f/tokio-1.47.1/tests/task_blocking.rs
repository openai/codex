#![warn(rust_2018_idioms)]
#![cfg(all(feature = "full", not(target_os = "wasi"), not(miri)))] // Wasi doesn't support threads

use tokio::{runtime, task, time};
use tokio_test::assert_ok;

use std::thread;
use std::time::Duration;

mod support {
    pub(crate) mod mpsc_stream;
}

#[tokio::test]
async fn basic_blocking() {
    // Run a few times
    for _ in 0..100 {
        let out = assert_ok!(
            tokio::spawn(async {
                assert_ok!(
                    task::spawn_blocking(|| {
                        thread::sleep(Duration::from_millis(5));
                        "hello"
                    })
                    .await
                )
            })
            .await
        );

        assert_eq!(out, "hello");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn block_in_blocking() {
    // Run a few times
    for _ in 0..100 {
        let out = assert_ok!(
            tokio::spawn(async {
                assert_ok!(
                    task::spawn_blocking(|| {
                        task::block_in_place(|| {
                            thread::sleep(Duration::from_millis(5));
                        });
                        "hello"
                    })
                    .await
                )
            })
            .await
        );

        assert_eq!(out, "hello");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn block_in_block() {
    // Run a few times
    for _ in 0..100 {
        let out = assert_ok!(
            tokio::spawn(async {
                task::block_in_place(|| {
                    task::block_in_place(|| {
                        thread::sleep(Duration::from_millis(5));
                    });
                    "hello"
                })
            })
            .await
        );

        assert_eq!(out, "hello");
    }
}

#[tokio::test(flavor = "current_thread")]
#[should_panic]
async fn no_block_in_current_thread_scheduler() {
    task::block_in_place(|| {});
}

#[test]
fn yes_block_in_threaded_block_on() {
    let rt = runtime::Runtime::new().unwrap();
    rt.block_on(async {
        task::block_in_place(|| {});
    });
}

#[test]
#[should_panic]
fn no_block_in_current_thread_block_on() {
    let rt = runtime::Builder::new_current_thread().build().unwrap();
    rt.block_on(async {
        task::block_in_place(|| {});
    });
}

#[test]
fn can_enter_current_thread_rt_from_within_block_in_place() {
    let outer = tokio::runtime::Runtime::new().unwrap();

    outer.block_on(async {
        tokio::task::block_in_place(|| {
            let inner = tokio::runtime::Builder::new_current_thread()
                .build()
                .unwrap();

            inner.block_on(async {})
        })
    });
}

#[test]
#[cfg(panic = "unwind")]
fn useful_panic_message_when_dropping_rt_in_rt() {
    use std::panic::{catch_unwind, AssertUnwindSafe};

    let outer = tokio::runtime::Runtime::new().unwrap();

    let result = catch_unwind(AssertUnwindSafe(|| {
        outer.block_on(async {
            let _ = tokio::runtime::Builder::new_current_thread()
                .build()
                .unwrap();
        });
    }));

    assert!(result.is_err());
    let err = result.unwrap_err();
    let err: &'static str = err.downcast_ref::<&'static str>().unwrap();

    assert!(
        err.contains("Cannot drop a runtime"),
        "Wrong panic message: {err:?}"
    );
}

#[test]
fn can_shutdown_with_zero_timeout_in_runtime() {
    let outer = tokio::runtime::Runtime::new().unwrap();

    outer.block_on(async {
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        rt.shutdown_timeout(Duration::from_nanos(0));
    });
}

#[test]
fn can_shutdown_now_in_runtime() {
    let outer = tokio::runtime::Runtime::new().unwrap();

    outer.block_on(async {
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        rt.shutdown_background();
    });
}

#[test]
fn coop_disabled_in_block_in_place() {
    let outer = tokio::runtime::Builder::new_multi_thread()
        .enable_time()
        .build()
        .unwrap();

    let (tx, rx) = support::mpsc_stream::unbounded_channel_stream();

    for i in 0..200 {
        tx.send(i).unwrap();
    }
    drop(tx);

    outer.block_on(async move {
        let jh = tokio::spawn(async move {
            tokio::task::block_in_place(move || {
                futures::executor::block_on(async move {
                    use tokio_stream::StreamExt;
                    assert_eq!(rx.fold(0, |n, _| n + 1).await, 200);
                })
            })
        });

        tokio::time::timeout(Duration::from_secs(1), jh)
            .await
            .expect("timed out (probably hanging)")
            .unwrap()
    });
}

#[test]
fn coop_disabled_in_block_in_place_in_block_on() {
    let (done_tx, done_rx) = std::sync::mpsc::channel();
    let done = done_tx.clone();
    thread::spawn(move || {
        let outer = tokio::runtime::Runtime::new().unwrap();

        let (tx, rx) = support::mpsc_stream::unbounded_channel_stream();

        for i in 0..200 {
            tx.send(i).unwrap();
        }
        drop(tx);

        outer.block_on(async move {
            tokio::task::block_in_place(move || {
                futures::executor::block_on(async move {
                    use tokio_stream::StreamExt;
                    assert_eq!(rx.fold(0, |n, _| n + 1).await, 200);
                })
            })
        });

        let _ = done.send(Ok(()));
    });

    thread::spawn(move || {
        thread::sleep(Duration::from_secs(1));
        let _ = done_tx.send(Err("timed out (probably hanging)"));
    });

    done_rx.recv().unwrap().unwrap();
}

#[cfg(feature = "test-util")]
#[tokio::test(start_paused = true)]
async fn blocking_when_paused() {
    // Do not auto-advance time when we have started a blocking task that has
    // not yet finished.
    time::timeout(
        Duration::from_secs(3),
        task::spawn_blocking(|| thread::sleep(Duration::from_millis(1))),
    )
    .await
    .expect("timeout should not trigger")
    .expect("blocking task should finish");

    // Really: Do not auto-advance time, even if the timeout is short and the
    // blocking task runs for longer than that. It doesn't matter: Tokio time
    // is paused; system time is not.
    time::timeout(
        Duration::from_millis(1),
        task::spawn_blocking(|| thread::sleep(Duration::from_millis(50))),
    )
    .await
    .expect("timeout should not trigger")
    .expect("blocking task should finish");
}

#[cfg(feature = "test-util")]
#[tokio::test(start_paused = true)]
async fn blocking_task_wakes_paused_runtime() {
    let t0 = std::time::Instant::now();
    time::timeout(
        Duration::from_secs(15),
        task::spawn_blocking(|| thread::sleep(Duration::from_millis(1))),
    )
    .await
    .expect("timeout should not trigger")
    .expect("blocking task should finish");
    assert!(
        t0.elapsed() < Duration::from_secs(10),
        "completing a spawn_blocking should wake the scheduler if it's parked while time is paused"
    );
}

#[cfg(feature = "test-util")]
#[tokio::test(start_paused = true)]
async fn unawaited_blocking_task_wakes_paused_runtime() {
    let t0 = std::time::Instant::now();

    // When this task finishes, time should auto-advance, even though the
    // JoinHandle has not been awaited yet.
    let a = task::spawn_blocking(|| {
        thread::sleep(Duration::from_millis(1));
    });

    crate::time::sleep(Duration::from_secs(15)).await;
    a.await.expect("blocking task should finish");
    assert!(
        t0.elapsed() < Duration::from_secs(10),
        "completing a spawn_blocking should wake the scheduler if it's parked while time is paused"
    );
}

#[cfg(panic = "unwind")]
#[cfg(feature = "test-util")]
#[tokio::test(start_paused = true)]
async fn panicking_blocking_task_wakes_paused_runtime() {
    let t0 = std::time::Instant::now();
    let result = time::timeout(
        Duration::from_secs(15),
        task::spawn_blocking(|| {
            thread::sleep(Duration::from_millis(1));
            panic!("blocking task panicked");
        }),
    )
    .await
    .expect("timeout should not trigger");
    assert!(result.is_err(), "blocking task should have panicked");
    assert!(
        t0.elapsed() < Duration::from_secs(10),
        "completing a spawn_blocking should wake the scheduler if it's parked while time is paused"
    );
}
