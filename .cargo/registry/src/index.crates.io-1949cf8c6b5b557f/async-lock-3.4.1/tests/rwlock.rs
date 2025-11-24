mod common;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use common::check_yields_when_contended;

#[cfg(not(target_family = "wasm"))]
use futures_lite::prelude::*;
#[cfg(not(target_family = "wasm"))]
use std::thread;

use futures_lite::future;

use async_lock::{
    RwLock, RwLockReadGuard, RwLockReadGuardArc, RwLockUpgradableReadGuard,
    RwLockUpgradableReadGuardArc,
};

#[cfg(target_family = "wasm")]
use wasm_bindgen_test::wasm_bindgen_test as test;

#[cfg(target_family = "wasm")]
wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

#[cfg(not(target_family = "wasm"))]
fn spawn<T: Send + 'static>(f: impl Future<Output = T> + Send + 'static) -> future::Boxed<T> {
    let (s, r) = flume::bounded(1);
    thread::spawn(move || {
        future::block_on(async {
            let _ = s.send_async(f.await).await;
        })
    });
    async move { r.recv_async().await.unwrap() }.boxed()
}

#[test]
fn smoke() {
    future::block_on(async {
        let lock = RwLock::new(());
        drop(lock.read().await);
        drop(lock.write().await);
        drop((lock.read().await, lock.read().await));
        drop(lock.write().await);
    });
}

#[cfg(all(feature = "std", not(target_family = "wasm")))]
#[test]
fn smoke_blocking() {
    let lock = RwLock::new(());
    drop(lock.read_blocking());
    drop(lock.write_blocking());
    drop((lock.read_blocking(), lock.read_blocking()));
    let read = lock.read_blocking();
    let upgradabe = lock.upgradable_read_blocking();
    drop(read);
    drop(RwLockUpgradableReadGuard::upgrade_blocking(upgradabe));
    drop(lock.write_blocking());
}

#[cfg(all(feature = "std", not(target_family = "wasm")))]
#[test]
fn smoke_arc_blocking() {
    let lock = Arc::new(RwLock::new(()));
    drop(lock.read_arc_blocking());
    drop(lock.write_arc_blocking());
    drop((lock.read_arc_blocking(), lock.read_arc_blocking()));
    let read = lock.read_arc_blocking();
    let upgradabe = lock.upgradable_read_arc_blocking();
    drop(read);
    drop(RwLockUpgradableReadGuardArc::upgrade_blocking(upgradabe));
    drop(lock.write_arc_blocking());
}

#[test]
fn try_write() {
    future::block_on(async {
        let lock = RwLock::new(0isize);
        let read_guard = lock.read().await;
        assert!(lock.try_write().is_none());
        drop(read_guard);
    });
}

#[test]
fn into_inner() {
    let lock = RwLock::new(10);
    assert_eq!(lock.into_inner(), 10);
}

#[test]
fn into_inner_and_drop() {
    struct Counter(Arc<AtomicUsize>);

    impl Drop for Counter {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    let cnt = Arc::new(AtomicUsize::new(0));
    let lock = RwLock::new(Counter(cnt.clone()));
    assert_eq!(cnt.load(Ordering::SeqCst), 0);

    {
        let _inner = lock.into_inner();
        assert_eq!(cnt.load(Ordering::SeqCst), 0);
    }

    assert_eq!(cnt.load(Ordering::SeqCst), 1);
}

#[test]
fn get_mut() {
    let mut lock = RwLock::new(10);
    *lock.get_mut() = 20;
    assert_eq!(lock.into_inner(), 20);
}

// Miri bug; this works when async is replaced with blocking
#[cfg(not(target_family = "wasm"))]
#[test]
#[cfg_attr(miri, ignore)]
fn contention() {
    const N: u32 = 10;
    const M: usize = if cfg!(miri) { 100 } else { 1000 };

    let (tx, rx) = flume::unbounded();
    let tx = Arc::new(tx);
    let rw = Arc::new(RwLock::new(()));

    // Spawn N tasks that randomly acquire the lock M times.
    for _ in 0..N {
        let tx = tx.clone();
        let rw = rw.clone();

        let _spawned = spawn(async move {
            for _ in 0..M {
                if fastrand::u32(..N) == 0 {
                    drop(rw.write().await);
                } else {
                    drop(rw.read().await);
                }
            }
            tx.send_async(()).await.unwrap();
        });
    }

    future::block_on(async move {
        for _ in 0..N {
            rx.recv_async().await.unwrap();
        }
    });
}

#[cfg(not(target_family = "wasm"))]
#[test]
#[cfg_attr(miri, ignore)]
fn contention_arc() {
    const N: u32 = 10;
    const M: usize = if cfg!(miri) { 100 } else { 1000 };

    let (tx, rx) = flume::unbounded();
    let tx = Arc::new(tx);
    let rw = Arc::new(RwLock::new(()));

    // Spawn N tasks that randomly acquire the lock M times.
    for _ in 0..N {
        let tx = tx.clone();
        let rw = rw.clone();

        let _spawned = spawn(async move {
            for _ in 0..M {
                if fastrand::u32(..N) == 0 {
                    drop(rw.write_arc().await);
                } else {
                    drop(rw.read_arc().await);
                }
            }
            tx.send_async(()).await.unwrap();
        });
    }

    future::block_on(async move {
        for _ in 0..N {
            rx.recv_async().await.unwrap();
        }
    });
}

#[cfg(not(target_family = "wasm"))]
#[test]
fn writer_and_readers() {
    let lock = Arc::new(RwLock::new(0i32));
    let (tx, rx) = flume::unbounded();

    // Spawn a writer task.
    let _spawned = spawn({
        let lock = lock.clone();
        async move {
            let mut lock = lock.write().await;
            for _ in 0..1000 {
                let tmp = *lock;
                *lock = -1;
                future::yield_now().await;
                *lock = tmp + 1;
            }
            tx.send_async(()).await.unwrap();
        }
    });

    // Readers try to catch the writer in the act.
    let mut readers = Vec::new();
    for _ in 0..5 {
        let lock = lock.clone();
        readers.push(spawn(async move {
            for _ in 0..1000 {
                let lock = lock.read().await;
                assert!(*lock >= 0);
            }
        }));
    }

    future::block_on(async move {
        // Wait for readers to pass their asserts.
        for r in readers {
            r.await;
        }

        // Wait for writer to finish.
        rx.recv_async().await.unwrap();
        let lock = lock.read().await;
        assert_eq!(*lock, 1000);
    });
}

#[cfg(not(target_family = "wasm"))]
#[test]
fn writer_and_readers_arc() {
    let lock = Arc::new(RwLock::new(0i32));
    let (tx, rx) = flume::unbounded();

    // Spawn a writer task.
    let _spawned = spawn({
        let lock = lock.clone();
        async move {
            let mut lock = lock.write_arc().await;
            for _ in 0..1000 {
                let tmp = *lock;
                *lock = -1;
                future::yield_now().await;
                *lock = tmp + 1;
            }
            tx.send_async(()).await.unwrap();
        }
    });

    // Readers try to catch the writer in the act.
    let mut readers = Vec::new();
    for _ in 0..5 {
        let lock = lock.clone();
        readers.push(spawn(async move {
            for _ in 0..1000 {
                let lock = lock.read_arc().await;
                assert!(*lock >= 0);
            }
        }));
    }

    future::block_on(async move {
        // Wait for readers to pass their asserts.
        for r in readers {
            r.await;
        }

        // Wait for writer to finish.
        rx.recv_async().await.unwrap();
        let lock = lock.read_arc().await;
        assert_eq!(*lock, 1000);
    });
}

#[test]
fn upgrade() {
    future::block_on(async {
        let lock: RwLock<i32> = RwLock::new(0);

        let read_guard = lock.read().await;
        let read_guard2 = lock.read().await;
        // Should be able to obtain an upgradable lock.
        let upgradable_guard = lock.upgradable_read().await;
        // Should be able to obtain a read lock when an upgradable lock is active.
        let read_guard3 = lock.read().await;
        assert_eq!(0, *read_guard3);
        drop(read_guard);
        drop(read_guard2);
        drop(read_guard3);

        // Writers should not pass.
        assert!(lock.try_write().is_none());

        let mut write_guard = RwLockUpgradableReadGuard::try_upgrade(upgradable_guard).expect(
            "should be able to upgrade an upgradable lock because there are no more readers",
        );
        *write_guard += 1;
        drop(write_guard);

        let read_guard = lock.read().await;
        assert_eq!(1, *read_guard)
    });
}

#[test]
fn upgrade_arc() {
    future::block_on(async {
        let lock: Arc<RwLock<i32>> = Arc::new(RwLock::new(0));

        let read_guard = lock.read_arc().await;
        let read_guard2 = lock.read_arc().await;
        // Should be able to obtain an upgradable lock.
        let upgradable_guard = lock.upgradable_read_arc().await;
        // Should be able to obtain a read lock when an upgradable lock is active.
        let read_guard3 = lock.read_arc().await;
        assert_eq!(0, *read_guard3);
        drop(read_guard);
        drop(read_guard2);
        drop(read_guard3);

        // Writers should not pass.
        assert!(lock.try_write().is_none());

        let mut write_guard = RwLockUpgradableReadGuardArc::try_upgrade(upgradable_guard).expect(
            "should be able to upgrade an upgradable lock because there are no more readers",
        );
        *write_guard += 1;
        drop(write_guard);

        let read_guard = lock.read_arc().await;
        assert_eq!(1, *read_guard)
    });
}

#[test]
fn not_upgrade() {
    future::block_on(async {
        let mutex: RwLock<i32> = RwLock::new(0);

        let read_guard = mutex.read().await;
        let read_guard2 = mutex.read().await;
        // Should be able to obtain an upgradable lock.
        let upgradable_guard = mutex.upgradable_read().await;
        // Should be able to obtain a shared lock when an upgradable lock is active.
        let read_guard3 = mutex.read().await;
        assert_eq!(0, *read_guard3);
        drop(read_guard);
        drop(read_guard2);
        drop(read_guard3);

        // Drop the upgradable lock.
        drop(upgradable_guard);

        assert_eq!(0, *(mutex.read().await));

        // Should be able to acquire a write lock because there are no more readers.
        let mut write_guard = mutex.write().await;
        *write_guard += 1;
        drop(write_guard);

        let read_guard = mutex.read().await;
        assert_eq!(1, *read_guard)
    });
}

#[test]
fn not_upgrade_arc() {
    future::block_on(async {
        let mutex: Arc<RwLock<i32>> = Arc::new(RwLock::new(0));

        let read_guard = mutex.read_arc().await;
        let read_guard2 = mutex.read_arc().await;
        // Should be able to obtain an upgradable lock.
        let upgradable_guard = mutex.upgradable_read_arc().await;
        // Should be able to obtain a shared lock when an upgradable lock is active.
        let read_guard3 = mutex.read_arc().await;
        assert_eq!(0, *read_guard3);
        drop(read_guard);
        drop(read_guard2);
        drop(read_guard3);

        // Drop the upgradable lock.
        drop(upgradable_guard);

        assert_eq!(0, *(mutex.read_arc().await));

        // Should be able to acquire a write lock because there are no more readers.
        let mut write_guard = mutex.write_arc().await;
        *write_guard += 1;
        drop(write_guard);

        let read_guard = mutex.read_arc().await;
        assert_eq!(1, *read_guard)
    });
}

#[test]
fn upgradable_with_concurrent_writer() {
    future::block_on(async {
        let lock: Arc<RwLock<i32>> = Arc::new(RwLock::new(0));
        let lock2 = lock.clone();

        let upgradable_guard = lock.upgradable_read().await;

        future::or(
            async move {
                let mut write_guard = lock2.write().await;
                *write_guard = 1;
            },
            async move {
                let mut write_guard = RwLockUpgradableReadGuard::upgrade(upgradable_guard).await;
                assert_eq!(*write_guard, 0);
                *write_guard = 2;
            },
        )
        .await;

        assert_eq!(2, *(lock.write().await));

        let read_guard = lock.read().await;
        assert_eq!(2, *read_guard);
    });
}

#[test]
fn upgradable_with_concurrent_writer_arc() {
    future::block_on(async {
        let lock: Arc<RwLock<i32>> = Arc::new(RwLock::new(0));
        let lock2 = lock.clone();

        let upgradable_guard = lock.upgradable_read_arc().await;

        future::or(
            async move {
                let mut write_guard = lock2.write_arc().await;
                *write_guard = 1;
            },
            async move {
                let mut write_guard = RwLockUpgradableReadGuardArc::upgrade(upgradable_guard).await;
                assert_eq!(*write_guard, 0);
                *write_guard = 2;
            },
        )
        .await;

        assert_eq!(2, *(lock.write_arc().await));

        let read_guard = lock.read_arc().await;
        assert_eq!(2, *read_guard);
    });
}

#[test]
fn yields_when_contended() {
    let rw = RwLock::new(());

    check_yields_when_contended(rw.try_write().unwrap(), rw.read());
    check_yields_when_contended(rw.try_write().unwrap(), rw.upgradable_read());
    check_yields_when_contended(rw.try_write().unwrap(), rw.write());

    check_yields_when_contended(rw.try_read().unwrap(), rw.write());

    check_yields_when_contended(rw.try_upgradable_read().unwrap(), rw.write());
    check_yields_when_contended(rw.try_upgradable_read().unwrap(), rw.upgradable_read());

    let upgradable = rw.try_upgradable_read().unwrap();
    check_yields_when_contended(
        rw.try_read().unwrap(),
        RwLockUpgradableReadGuard::upgrade(upgradable),
    );
}

#[test]
fn yields_when_contended_arc() {
    let rw = Arc::new(RwLock::new(()));

    check_yields_when_contended(rw.try_write_arc().unwrap(), rw.read_arc());
    check_yields_when_contended(rw.try_write_arc().unwrap(), rw.upgradable_read_arc());
    check_yields_when_contended(rw.try_write_arc().unwrap(), rw.write_arc());

    check_yields_when_contended(rw.try_read_arc().unwrap(), rw.write_arc());

    check_yields_when_contended(rw.try_upgradable_read_arc().unwrap(), rw.write_arc());
    check_yields_when_contended(
        rw.try_upgradable_read_arc().unwrap(),
        rw.upgradable_read_arc(),
    );

    let upgradable = rw.try_upgradable_read_arc().unwrap();
    check_yields_when_contended(
        rw.try_read_arc().unwrap(),
        RwLockUpgradableReadGuardArc::upgrade(upgradable),
    );
}

#[test]
fn cancellation() {
    future::block_on(async {
        let rw = RwLock::new(());

        drop(rw.read());

        drop(rw.upgradable_read());

        drop(rw.write());

        let read = rw.read().await;
        drop(read);

        let upgradable_read = rw.upgradable_read().await;
        drop(upgradable_read);

        let write = rw.write().await;
        drop(write);

        let upgradable_read = rw.upgradable_read().await;
        drop(RwLockUpgradableReadGuard::upgrade(upgradable_read));

        let upgradable_read = rw.upgradable_read().await;
        let write = RwLockUpgradableReadGuard::upgrade(upgradable_read).await;
        drop(write);
    });
}

#[test]
fn arc_rwlock_refcounts() {
    future::block_on(async {
        let rw = Arc::new(RwLock::new(()));
        assert_eq!(Arc::strong_count(&rw), 1);

        drop(rw.read_arc());
        assert_eq!(Arc::strong_count(&rw), 1);

        drop(rw.upgradable_read_arc());
        assert_eq!(Arc::strong_count(&rw), 1);

        drop(rw.write());
        assert_eq!(Arc::strong_count(&rw), 1);

        let read = rw.read_arc().await;
        assert_eq!(Arc::strong_count(&rw), 2);
        drop(read);
        assert_eq!(Arc::strong_count(&rw), 1);

        let upgradable_read = rw.upgradable_read_arc().await;
        assert_eq!(Arc::strong_count(&rw), 2);
        drop(upgradable_read);
        assert_eq!(Arc::strong_count(&rw), 1);

        let write = rw.write_arc().await;
        assert_eq!(Arc::strong_count(&rw), 2);
        drop(write);
        assert_eq!(Arc::strong_count(&rw), 1);

        let upgradable_read = rw.upgradable_read_arc().await;
        assert_eq!(Arc::strong_count(&rw), 2);
        drop(RwLockUpgradableReadGuardArc::upgrade(upgradable_read));
        assert_eq!(Arc::strong_count(&rw), 1);

        let upgradable_read = rw.upgradable_read_arc().await;
        assert_eq!(Arc::strong_count(&rw), 2);
        let write = RwLockUpgradableReadGuardArc::upgrade(upgradable_read).await;
        assert_eq!(Arc::strong_count(&rw), 2);
        drop(write);
        assert_eq!(Arc::strong_count(&rw), 1);
    });
}

// We are testing that this compiles.
fn _covariance_test<'g>(guard: RwLockReadGuard<'g, &'static ()>) {
    let _: RwLockReadGuard<'g, &'g ()> = guard;
}

// We are testing that this compiles.
fn _covariance_test_arc(
    guard: RwLockReadGuardArc<&'static ()>,
    mut _guard_2: RwLockReadGuardArc<&()>,
) {
    _guard_2 = guard;
}
