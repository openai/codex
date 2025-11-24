use crate::runtime::park;
use crate::runtime::{self, Runtime};

#[test]
fn yield_calls_park_before_scheduling_again() {
    // Don't need to check all permutations
    let mut loom = loom::model::Builder::default();
    loom.max_permutations = Some(1);
    loom.check(|| {
        let rt = mk_runtime();

        let jh = rt.spawn(async {
            let tid = loom::thread::current().id();
            let park_count = park::current_thread_park_count();

            crate::task::yield_now().await;

            if tid == loom::thread::current().id() {
                let new_park_count = park::current_thread_park_count();
                assert_eq!(park_count + 1, new_park_count);
            }
        });

        rt.block_on(jh).unwrap();
    });
}

fn mk_runtime() -> Runtime {
    runtime::Builder::new_current_thread().build().unwrap()
}
