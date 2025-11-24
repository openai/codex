use std::sync::Arc;
use std::thread;

use async_lock::Barrier;
use futures_lite::future;

#[test]
#[cfg_attr(miri, ignore)]
fn smoke() {
    future::block_on(async move {
        const N: usize = 10;

        let barrier = Arc::new(Barrier::new(N));

        for _ in 0..10 {
            let (tx, rx) = flume::unbounded();

            for _ in 0..N - 1 {
                let c = barrier.clone();
                let tx = tx.clone();

                thread::spawn(move || {
                    future::block_on(async move {
                        let res = c.wait().await;
                        tx.send_async(res.is_leader()).await.unwrap();
                    })
                });
            }

            // At this point, all spawned threads should be blocked,
            // so we shouldn't get anything from the channel.
            let res = rx.try_recv();
            assert!(res.is_err());

            let mut leader_found = barrier.wait().await.is_leader();

            // Now, the barrier is cleared and we should get data.
            for _ in 0..N - 1 {
                if rx.recv_async().await.unwrap() {
                    assert!(!leader_found);
                    leader_found = true;
                }
            }
            assert!(leader_found);
        }
    });
}

#[cfg(all(feature = "std", not(target_family = "wasm")))]
#[test]
#[cfg_attr(miri, ignore)]
fn smoke_blocking() {
    future::block_on(async move {
        const N: usize = 10;

        let barrier = Arc::new(Barrier::new(N));

        for _ in 0..10 {
            let (tx, rx) = flume::unbounded();

            for _ in 0..N - 1 {
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
                if rx.recv_async().await.unwrap() {
                    assert!(!leader_found);
                    leader_found = true;
                }
            }
            assert!(leader_found);
        }
    });
}
