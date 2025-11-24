use crate::runtime::park;
use crate::runtime::tests::loom_oneshot as oneshot;
use crate::runtime::{self, Runtime};

#[test]
fn yield_calls_park_before_scheduling_again() {
    // Don't need to check all permutations
    let mut loom = loom::model::Builder::default();
    loom.max_permutations = Some(1);
    loom.check(|| {
        let rt = mk_runtime(2);
        let (tx, rx) = oneshot::channel::<()>();

        rt.spawn(async {
            let tid = loom::thread::current().id();
            let park_count = park::current_thread_park_count();

            crate::task::yield_now().await;

            if tid == loom::thread::current().id() {
                let new_park_count = park::current_thread_park_count();
                assert_eq!(park_count + 1, new_park_count);
            }

            tx.send(());
        });

        rx.recv();
    });
}

fn mk_runtime(num_threads: usize) -> Runtime {
    runtime::Builder::new_multi_thread()
        .worker_threads(num_threads)
        .build()
        .unwrap()
}
