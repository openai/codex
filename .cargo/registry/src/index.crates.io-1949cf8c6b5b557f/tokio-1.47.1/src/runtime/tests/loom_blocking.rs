use crate::runtime::{self, Runtime};

use std::sync::Arc;

#[test]
fn blocking_shutdown() {
    loom::model(|| {
        let v = Arc::new(());

        let rt = mk_runtime(1);
        {
            let _enter = rt.enter();
            for _ in 0..2 {
                let v = v.clone();
                crate::task::spawn_blocking(move || {
                    assert!(1 < Arc::strong_count(&v));
                });
            }
        }

        drop(rt);
        assert_eq!(1, Arc::strong_count(&v));
    });
}

#[test]
fn spawn_mandatory_blocking_should_always_run() {
    use crate::runtime::tests::loom_oneshot;
    loom::model(|| {
        let rt = runtime::Builder::new_current_thread().build().unwrap();

        let (tx, rx) = loom_oneshot::channel();
        let _enter = rt.enter();
        runtime::spawn_blocking(|| {});
        runtime::spawn_mandatory_blocking(move || {
            let _ = tx.send(());
        })
        .unwrap();

        drop(rt);

        // This call will deadlock if `spawn_mandatory_blocking` doesn't run.
        let () = rx.recv();
    });
}

#[test]
fn spawn_mandatory_blocking_should_run_even_when_shutting_down_from_other_thread() {
    use crate::runtime::tests::loom_oneshot;
    loom::model(|| {
        let rt = runtime::Builder::new_current_thread().build().unwrap();
        let handle = rt.handle().clone();

        // Drop the runtime in a different thread
        {
            loom::thread::spawn(move || {
                drop(rt);
            });
        }

        let _enter = handle.enter();
        let (tx, rx) = loom_oneshot::channel();
        let handle = runtime::spawn_mandatory_blocking(move || {
            let _ = tx.send(());
        });

        // handle.is_some() means that `spawn_mandatory_blocking`
        // promised us to run the blocking task
        if handle.is_some() {
            // This call will deadlock if `spawn_mandatory_blocking` doesn't run.
            let () = rx.recv();
        }
    });
}

#[test]
fn spawn_blocking_when_paused() {
    use std::time::Duration;
    loom::model(|| {
        let rt = crate::runtime::Builder::new_current_thread()
            .enable_time()
            .start_paused(true)
            .build()
            .unwrap();
        let handle = rt.handle();
        let _enter = handle.enter();
        let a = crate::task::spawn_blocking(|| {});
        let b = crate::task::spawn_blocking(|| {});
        rt.block_on(crate::time::timeout(Duration::from_millis(1), async move {
            a.await.expect("blocking task should finish");
            b.await.expect("blocking task should finish");
        }))
        .expect("timeout should not trigger");
    });
}

fn mk_runtime(num_threads: usize) -> Runtime {
    runtime::Builder::new_multi_thread()
        .worker_threads(num_threads)
        .build()
        .unwrap()
}
