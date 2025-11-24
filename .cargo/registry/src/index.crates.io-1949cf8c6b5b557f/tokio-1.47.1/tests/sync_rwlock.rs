#![warn(rust_2018_idioms)]
#![cfg(feature = "sync")]

#[cfg(all(target_family = "wasm", not(target_os = "wasi")))]
use wasm_bindgen_test::wasm_bindgen_test as test;
#[cfg(all(target_family = "wasm", not(target_os = "wasi")))]
use wasm_bindgen_test::wasm_bindgen_test as maybe_tokio_test;

#[cfg(not(all(target_family = "wasm", not(target_os = "wasi"))))]
use tokio::test as maybe_tokio_test;

use std::task::Poll;

use futures::future::FutureExt;

use tokio::sync::{RwLock, RwLockWriteGuard};
use tokio_test::task::spawn;
use tokio_test::{assert_pending, assert_ready};

#[test]
fn into_inner() {
    let rwlock = RwLock::new(42);
    assert_eq!(rwlock.into_inner(), 42);
}

// multiple reads should be Ready
#[test]
fn read_shared() {
    let rwlock = RwLock::new(100);

    let mut t1 = spawn(rwlock.read());
    let _g1 = assert_ready!(t1.poll());
    let mut t2 = spawn(rwlock.read());
    let _g2 = assert_ready!(t2.poll());
}

// When there is an active shared owner, exclusive access should not be possible
#[test]
fn write_shared_pending() {
    let rwlock = RwLock::new(100);
    let mut t1 = spawn(rwlock.read());

    let _g1 = assert_ready!(t1.poll());
    let mut t2 = spawn(rwlock.write());
    assert_pending!(t2.poll());
}

// When there is an active exclusive owner, subsequent exclusive access should not be possible
#[test]
fn read_exclusive_pending() {
    let rwlock = RwLock::new(100);
    let mut t1 = spawn(rwlock.write());

    let _g1 = assert_ready!(t1.poll());
    let mut t2 = spawn(rwlock.read());
    assert_pending!(t2.poll());
}

// If the max shared access is reached and subsequent shared access is pending
// should be made available when one of the shared accesses is dropped
#[test]
fn exhaust_reading() {
    let rwlock = RwLock::with_max_readers(100, 1024);
    let mut reads = Vec::new();
    loop {
        let mut t = spawn(rwlock.read());
        match t.poll() {
            Poll::Ready(guard) => reads.push(guard),
            Poll::Pending => break,
        }
    }

    let mut t1 = spawn(rwlock.read());
    assert_pending!(t1.poll());
    let g2 = reads.pop().unwrap();
    drop(g2);
    assert!(t1.is_woken());
    let _g1 = assert_ready!(t1.poll());
}

// When there is an active exclusive owner, subsequent exclusive access should not be possible
#[test]
fn write_exclusive_pending() {
    let rwlock = RwLock::new(100);
    let mut t1 = spawn(rwlock.write());

    let _g1 = assert_ready!(t1.poll());
    let mut t2 = spawn(rwlock.write());
    assert_pending!(t2.poll());
}

// When there is an active shared owner, exclusive access should be possible after shared is dropped
#[test]
fn write_shared_drop() {
    let rwlock = RwLock::new(100);
    let mut t1 = spawn(rwlock.read());

    let g1 = assert_ready!(t1.poll());
    let mut t2 = spawn(rwlock.write());
    assert_pending!(t2.poll());
    drop(g1);
    assert!(t2.is_woken());
    let _g2 = assert_ready!(t2.poll());
}

// when there is an active shared owner, and exclusive access is triggered,
// subsequent shared access should not be possible as write gathers all the available semaphore permits
#[test]
fn write_read_shared_pending() {
    let rwlock = RwLock::new(100);
    let mut t1 = spawn(rwlock.read());
    let _g1 = assert_ready!(t1.poll());

    let mut t2 = spawn(rwlock.read());
    let _g2 = assert_ready!(t2.poll());

    let mut t3 = spawn(rwlock.write());
    assert_pending!(t3.poll());

    let mut t4 = spawn(rwlock.read());
    assert_pending!(t4.poll());
}

// when there is an active shared owner, and exclusive access is triggered,
// reading should be possible after pending exclusive access is dropped
#[test]
fn write_read_shared_drop_pending() {
    let rwlock = RwLock::new(100);
    let mut t1 = spawn(rwlock.read());
    let _g1 = assert_ready!(t1.poll());

    let mut t2 = spawn(rwlock.write());
    assert_pending!(t2.poll());

    let mut t3 = spawn(rwlock.read());
    assert_pending!(t3.poll());
    drop(t2);

    assert!(t3.is_woken());
    let _t3 = assert_ready!(t3.poll());
}

// Acquire an RwLock nonexclusively by a single task
#[maybe_tokio_test]
async fn read_uncontested() {
    let rwlock = RwLock::new(100);
    let result = *rwlock.read().await;

    assert_eq!(result, 100);
}

// Acquire an uncontested RwLock in exclusive mode
#[maybe_tokio_test]
async fn write_uncontested() {
    let rwlock = RwLock::new(100);
    let mut result = rwlock.write().await;
    *result += 50;
    assert_eq!(*result, 150);
}

// RwLocks should be acquired in the order that their Futures are waited upon.
#[maybe_tokio_test]
async fn write_order() {
    let rwlock = RwLock::<Vec<u32>>::new(vec![]);
    let fut2 = rwlock.write().map(|mut guard| guard.push(2));
    let fut1 = rwlock.write().map(|mut guard| guard.push(1));
    fut1.await;
    fut2.await;

    let g = rwlock.read().await;
    assert_eq!(*g, vec![1, 2]);
}

// A single RwLock is contested by tasks in multiple threads
#[cfg(all(feature = "full", not(target_os = "wasi")))] // Wasi doesn't support threads
#[cfg_attr(miri, ignore)] // Too slow on miri.
#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn multithreaded() {
    use futures::stream::{self, StreamExt};
    use std::sync::Arc;
    use tokio::sync::Barrier;

    let barrier = Arc::new(Barrier::new(5));
    let rwlock = Arc::new(RwLock::<u32>::new(0));
    let rwclone1 = rwlock.clone();
    let rwclone2 = rwlock.clone();
    let rwclone3 = rwlock.clone();
    let rwclone4 = rwlock.clone();

    let b1 = barrier.clone();
    tokio::spawn(async move {
        stream::iter(0..1000)
            .for_each(move |_| {
                let rwlock = rwclone1.clone();
                async move {
                    let mut guard = rwlock.write().await;
                    *guard += 2;
                }
            })
            .await;
        b1.wait().await;
    });

    let b2 = barrier.clone();
    tokio::spawn(async move {
        stream::iter(0..1000)
            .for_each(move |_| {
                let rwlock = rwclone2.clone();
                async move {
                    let mut guard = rwlock.write().await;
                    *guard += 3;
                }
            })
            .await;
        b2.wait().await;
    });

    let b3 = barrier.clone();
    tokio::spawn(async move {
        stream::iter(0..1000)
            .for_each(move |_| {
                let rwlock = rwclone3.clone();
                async move {
                    let mut guard = rwlock.write().await;
                    *guard += 5;
                }
            })
            .await;
        b3.wait().await;
    });

    let b4 = barrier.clone();
    tokio::spawn(async move {
        stream::iter(0..1000)
            .for_each(move |_| {
                let rwlock = rwclone4.clone();
                async move {
                    let mut guard = rwlock.write().await;
                    *guard += 7;
                }
            })
            .await;
        b4.wait().await;
    });

    barrier.wait().await;
    let g = rwlock.read().await;
    assert_eq!(*g, 17_000);
}

#[maybe_tokio_test]
async fn try_write() {
    let lock = RwLock::new(0);
    let read_guard = lock.read().await;
    assert!(lock.try_write().is_err());
    drop(read_guard);
    assert!(lock.try_write().is_ok());
}

#[test]
fn try_read_try_write() {
    let lock: RwLock<usize> = RwLock::new(15);

    {
        let rg1 = lock.try_read().unwrap();
        assert_eq!(*rg1, 15);

        assert!(lock.try_write().is_err());

        let rg2 = lock.try_read().unwrap();
        assert_eq!(*rg2, 15)
    }

    {
        let mut wg = lock.try_write().unwrap();
        *wg = 1515;

        assert!(lock.try_read().is_err())
    }

    assert_eq!(*lock.try_read().unwrap(), 1515);
}

#[maybe_tokio_test]
async fn downgrade_map() {
    let lock = RwLock::new(0);
    let write_guard = lock.write().await;
    let mut read_t = spawn(lock.read());

    // We can't create a read when a write exists
    assert_pending!(read_t.poll());

    // During the call to `f`, `read_t` doesn't have access yet.
    let read_guard1 = RwLockWriteGuard::downgrade_map(write_guard, |v| {
        assert_pending!(read_t.poll());
        v
    });

    // After the downgrade, `read_t` got the lock
    let read_guard2 = assert_ready!(read_t.poll());

    // Ensure they're equal, as we return the original value
    assert_eq!(&*read_guard1 as *const _, &*read_guard2 as *const _);
}

#[maybe_tokio_test]
async fn try_downgrade_map() {
    let lock = RwLock::new(0);
    let write_guard = lock.write().await;
    let mut read_t = spawn(lock.read());

    // We can't create a read when a write exists
    assert_pending!(read_t.poll());

    // During the call to `f`, `read_t` doesn't have access yet.
    let write_guard = RwLockWriteGuard::try_downgrade_map(write_guard, |_| {
        assert_pending!(read_t.poll());
        None::<&()>
    })
    .expect_err("downgrade didn't fail");

    // After `f` returns `None`, `read_t` doesn't have access
    assert_pending!(read_t.poll());

    // After `f` returns `Some`, `read_t` does have access
    let read_guard1 = RwLockWriteGuard::try_downgrade_map(write_guard, |v| Some(v))
        .expect("downgrade didn't succeed");
    let read_guard2 = assert_ready!(read_t.poll());

    // Ensure they're equal, as we return the original value
    assert_eq!(&*read_guard1 as *const _, &*read_guard2 as *const _);
}
