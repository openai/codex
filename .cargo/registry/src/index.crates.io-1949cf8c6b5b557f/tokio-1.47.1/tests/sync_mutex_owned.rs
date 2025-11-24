#![warn(rust_2018_idioms)]
#![cfg(feature = "sync")]

#[cfg(all(target_family = "wasm", not(target_os = "wasi")))]
use wasm_bindgen_test::wasm_bindgen_test as test;
#[cfg(all(target_family = "wasm", not(target_os = "wasi")))]
use wasm_bindgen_test::wasm_bindgen_test as maybe_tokio_test;

#[cfg(not(all(target_family = "wasm", not(target_os = "wasi"))))]
use tokio::test as maybe_tokio_test;

use tokio::sync::Mutex;
use tokio_test::task::spawn;
use tokio_test::{assert_pending, assert_ready};

use std::sync::Arc;

#[test]
fn straight_execution() {
    let l = Arc::new(Mutex::new(100));

    {
        let mut t = spawn(l.clone().lock_owned());
        let mut g = assert_ready!(t.poll());
        assert_eq!(&*g, &100);
        *g = 99;
    }
    {
        let mut t = spawn(l.clone().lock_owned());
        let mut g = assert_ready!(t.poll());
        assert_eq!(&*g, &99);
        *g = 98;
    }
    {
        let mut t = spawn(l.lock_owned());
        let g = assert_ready!(t.poll());
        assert_eq!(&*g, &98);
    }
}

#[test]
fn readiness() {
    let l = Arc::new(Mutex::new(100));
    let mut t1 = spawn(l.clone().lock_owned());
    let mut t2 = spawn(l.lock_owned());

    let g = assert_ready!(t1.poll());

    // We can't now acquire the lease since it's already held in g
    assert_pending!(t2.poll());

    // But once g unlocks, we can acquire it
    drop(g);
    assert!(t2.is_woken());
    assert_ready!(t2.poll());
}

/// Ensure a mutex is unlocked if a future holding the lock
/// is aborted prematurely.
#[tokio::test]
#[cfg(feature = "full")]
#[cfg_attr(miri, ignore)]
async fn aborted_future_1() {
    use std::time::Duration;
    use tokio::time::{interval, timeout};

    let m1: Arc<Mutex<usize>> = Arc::new(Mutex::new(0));
    {
        let m2 = m1.clone();
        // Try to lock mutex in a future that is aborted prematurely
        timeout(Duration::from_millis(1u64), async move {
            let iv = interval(Duration::from_millis(1000));
            tokio::pin!(iv);
            m2.lock_owned().await;
            iv.as_mut().tick().await;
            iv.as_mut().tick().await;
        })
        .await
        .unwrap_err();
    }
    // This should succeed as there is no lock left for the mutex.
    timeout(Duration::from_millis(1u64), async move {
        m1.lock_owned().await;
    })
    .await
    .expect("Mutex is locked");
}

/// This test is similar to `aborted_future_1` but this time the
/// aborted future is waiting for the lock.
#[tokio::test]
#[cfg(feature = "full")]
async fn aborted_future_2() {
    use std::time::Duration;
    use tokio::time::timeout;

    let m1: Arc<Mutex<usize>> = Arc::new(Mutex::new(0));
    {
        // Lock mutex
        let _lock = m1.clone().lock_owned().await;
        {
            let m2 = m1.clone();
            // Try to lock mutex in a future that is aborted prematurely
            timeout(Duration::from_millis(1u64), async move {
                m2.lock_owned().await;
            })
            .await
            .unwrap_err();
        }
    }
    // This should succeed as there is no lock left for the mutex.
    timeout(Duration::from_millis(1u64), async move {
        m1.lock_owned().await;
    })
    .await
    .expect("Mutex is locked");
}

#[test]
fn try_lock_owned() {
    let m: Arc<Mutex<usize>> = Arc::new(Mutex::new(0));
    {
        let g1 = m.clone().try_lock_owned();
        assert!(g1.is_ok());
        let g2 = m.clone().try_lock_owned();
        assert!(g2.is_err());
    }
    let g3 = m.try_lock_owned();
    assert!(g3.is_ok());
}

#[maybe_tokio_test]
async fn debug_format() {
    let s = "debug";
    let m = Arc::new(Mutex::new(s.to_string()));
    assert_eq!(format!("{s:?}"), format!("{:?}", m.lock_owned().await));
}
