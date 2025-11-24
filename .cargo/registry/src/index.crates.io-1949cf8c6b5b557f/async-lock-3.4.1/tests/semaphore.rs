mod common;

use std::future::Future;
use std::mem::forget;
use std::pin::Pin;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    mpsc, Arc,
};
use std::task::Context;
use std::task::Poll;
use std::thread;

use common::check_yields_when_contended;

use async_lock::Semaphore;
use futures_lite::{future, pin};

#[test]
fn try_acquire() {
    let s = Semaphore::new(2);
    let g1 = s.try_acquire().unwrap();
    let _g2 = s.try_acquire().unwrap();

    assert!(s.try_acquire().is_none());
    drop(g1);
    assert!(s.try_acquire().is_some());
}

#[test]
fn stress() {
    const COUNT: usize = if cfg!(miri) { 500 } else { 10_000 };

    let s = Arc::new(Semaphore::new(5));
    let (tx, rx) = mpsc::channel::<()>();

    for _ in 0..50 {
        let s = s.clone();
        let tx = tx.clone();

        thread::spawn(move || {
            future::block_on(async {
                for _ in 0..COUNT {
                    s.acquire().await;
                }
                drop(tx);
            })
        });
    }

    drop(tx);
    let _ = rx.recv();

    let _g1 = s.try_acquire().unwrap();
    let g2 = s.try_acquire().unwrap();
    let _g3 = s.try_acquire().unwrap();
    let _g4 = s.try_acquire().unwrap();
    let _g5 = s.try_acquire().unwrap();

    assert!(s.try_acquire().is_none());
    drop(g2);
    assert!(s.try_acquire().is_some());
}

#[test]
fn as_mutex() {
    let s = Arc::new(Semaphore::new(1));
    let s2 = s.clone();
    let _t = thread::spawn(move || {
        future::block_on(async {
            let _g = s2.acquire().await;
        });
    });
    future::block_on(async {
        let _g = s.acquire().await;
    });
}

#[test]
fn multi_resource() {
    let s = Arc::new(Semaphore::new(2));
    let s2 = s.clone();
    let (tx1, rx1) = mpsc::channel();
    let (tx2, rx2) = mpsc::channel();
    let _t = thread::spawn(move || {
        future::block_on(async {
            let _g = s2.acquire().await;
            let _ = rx2.recv();
            tx1.send(()).unwrap();
        });
    });
    future::block_on(async {
        let _g = s.acquire().await;
        tx2.send(()).unwrap();
        rx1.recv().unwrap();
    });
}

#[test]
fn lifetime() {
    // Show that the future keeps the semaphore alive.
    let _fut = {
        let mutex = Arc::new(Semaphore::new(2));
        mutex.acquire_arc()
    };
}

#[test]
fn yields_when_contended() {
    let s = Semaphore::new(1);
    check_yields_when_contended(s.try_acquire().unwrap(), s.acquire());

    let s = Arc::new(s);
    check_yields_when_contended(s.try_acquire_arc().unwrap(), s.acquire_arc());
}

#[cfg(all(feature = "std", not(target_family = "wasm")))]
#[test]
fn smoke_blocking() {
    let s = Semaphore::new(2);
    let g1 = s.acquire_blocking();
    let _g2 = s.acquire_blocking();
    assert!(s.try_acquire().is_none());
    drop(g1);
    assert!(s.try_acquire().is_some());
}

#[cfg(all(feature = "std", not(target_family = "wasm")))]
#[test]
fn smoke_arc_blocking() {
    let s = Arc::new(Semaphore::new(2));
    let g1 = s.acquire_arc_blocking();
    let _g2 = s.acquire_arc_blocking();
    assert!(s.try_acquire().is_none());
    drop(g1);
    assert!(s.try_acquire().is_some());
}

#[test]
fn add_permits() {
    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    let s = Arc::new(Semaphore::new(0));
    let (tx, rx) = mpsc::channel::<()>();

    for _ in 0..50 {
        let s = s.clone();
        let tx = tx.clone();

        thread::spawn(move || {
            future::block_on(async {
                let perm = s.acquire().await;
                forget(perm);
                COUNTER.fetch_add(1, Ordering::Relaxed);
                drop(tx);
            })
        });
    }

    assert_eq!(COUNTER.load(Ordering::Relaxed), 0);

    s.add_permits(50);

    drop(tx);
    let _ = rx.recv();

    assert_eq!(COUNTER.load(Ordering::Relaxed), 50);
}

#[test]
fn add_permits_2() {
    future::block_on(AddPermitsTest);
}

struct AddPermitsTest;

impl Future for AddPermitsTest {
    type Output = ();
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        let s = Semaphore::new(0);
        let acq = s.acquire();
        pin!(acq);
        let acq_2 = s.acquire();
        pin!(acq_2);
        assert!(acq.as_mut().poll(cx).is_pending());
        assert!(acq_2.as_mut().poll(cx).is_pending());
        s.add_permits(1);
        let g = acq.poll(cx);
        assert!(g.is_ready());
        assert!(acq_2.poll(cx).is_pending());

        Poll::Ready(())
    }
}
