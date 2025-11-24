#![cfg(loom)]

use loom::sync::{mpsc, Arc};
use loom::thread;

use async_lock::Barrier;

#[ignore]
#[test]
fn barrier_smoke() {
    loom::model(|| {
        const N: usize = 10;

        let barrier = Arc::new(Barrier::new(N));

        for _ in 0..10 {
            let (tx, rx) = mpsc::channel();

            for _ in 0..loom::MAX_THREADS - 1 {
                let c = barrier.clone();
                let tx = tx.clone();

                thread::spawn(move || {
                    let res = c.wait_blocking();
                    tx.send(res.is_leader()).unwrap();
                });
            }

            // At this point, all spawned threads should be blocked,
            // so we shouldn't get anything from the channel.
            let res = rx.try_recv();
            assert!(res.is_err());

            let mut leader_found = barrier.wait_blocking().is_leader();

            // Now, the barrier is cleared and we should get data.
            for _ in 0..N - 1 {
                if rx.recv().unwrap() {
                    assert!(!leader_found);
                    leader_found = true;
                }
            }
            assert!(leader_found);
        }
    });
}
