#![allow(clippy::bool_assert_comparison, unused_imports)]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread::sleep;
use std::time::Duration;

use async_channel::{unbounded, RecvError, SendError, TryRecvError, TrySendError};
use easy_parallel::Parallel;
use futures_lite::{future, prelude::*};

#[cfg(target_family = "wasm")]
use wasm_bindgen_test::wasm_bindgen_test as test;

#[cfg(not(target_family = "wasm"))]
fn ms(ms: u64) -> Duration {
    Duration::from_millis(ms)
}

#[test]
fn smoke() {
    let (s, r) = unbounded();

    s.try_send(7).unwrap();
    assert_eq!(r.try_recv(), Ok(7));

    future::block_on(s.send(8)).unwrap();
    assert_eq!(future::block_on(r.recv()), Ok(8));
    assert_eq!(r.try_recv(), Err(TryRecvError::Empty));
}

#[cfg(all(feature = "std", not(target_family = "wasm")))]
#[test]
fn smoke_blocking() {
    let (s, r) = unbounded();

    s.send_blocking(7).unwrap();
    assert_eq!(r.try_recv(), Ok(7));

    s.send_blocking(8).unwrap();
    assert_eq!(future::block_on(r.recv()), Ok(8));

    future::block_on(s.send(9)).unwrap();
    assert_eq!(r.recv_blocking(), Ok(9));

    assert_eq!(r.try_recv(), Err(TryRecvError::Empty));
}

#[test]
fn capacity() {
    let (s, r) = unbounded::<()>();
    assert_eq!(s.capacity(), None);
    assert_eq!(r.capacity(), None);
}

#[test]
fn len_empty_full() {
    let (s, r) = unbounded();

    assert_eq!(s.len(), 0);
    assert_eq!(s.is_empty(), true);
    assert_eq!(s.is_full(), false);
    assert_eq!(r.len(), 0);
    assert_eq!(r.is_empty(), true);
    assert_eq!(r.is_full(), false);

    future::block_on(s.send(())).unwrap();

    assert_eq!(s.len(), 1);
    assert_eq!(s.is_empty(), false);
    assert_eq!(s.is_full(), false);
    assert_eq!(r.len(), 1);
    assert_eq!(r.is_empty(), false);
    assert_eq!(r.is_full(), false);

    future::block_on(r.recv()).unwrap();

    assert_eq!(s.len(), 0);
    assert_eq!(s.is_empty(), true);
    assert_eq!(s.is_full(), false);
    assert_eq!(r.len(), 0);
    assert_eq!(r.is_empty(), true);
    assert_eq!(r.is_full(), false);
}

#[cfg(not(target_family = "wasm"))]
#[test]
fn try_recv() {
    let (s, r) = unbounded();

    Parallel::new()
        .add(move || {
            assert_eq!(r.try_recv(), Err(TryRecvError::Empty));
            sleep(ms(1500));
            assert_eq!(r.try_recv(), Ok(7));
            sleep(ms(500));
            assert_eq!(r.try_recv(), Err(TryRecvError::Closed));
        })
        .add(move || {
            sleep(ms(1000));
            future::block_on(s.send(7)).unwrap();
        })
        .run();
}

#[cfg(not(target_family = "wasm"))]
#[test]
fn recv() {
    let (s, r) = unbounded();

    Parallel::new()
        .add(move || {
            assert_eq!(future::block_on(r.recv()), Ok(7));
            sleep(ms(1000));
            assert_eq!(future::block_on(r.recv()), Ok(8));
            sleep(ms(1000));
            assert_eq!(future::block_on(r.recv()), Ok(9));
            assert_eq!(future::block_on(r.recv()), Err(RecvError));
        })
        .add(move || {
            sleep(ms(1500));
            future::block_on(s.send(7)).unwrap();
            future::block_on(s.send(8)).unwrap();
            future::block_on(s.send(9)).unwrap();
        })
        .run();
}

#[test]
fn try_send() {
    let (s, r) = unbounded();
    for i in 0..1000 {
        assert_eq!(s.try_send(i), Ok(()));
    }

    drop(r);
    assert_eq!(s.try_send(777), Err(TrySendError::Closed(777)));
}

#[test]
fn send() {
    let (s, r) = unbounded();
    for i in 0..1000 {
        assert_eq!(future::block_on(s.send(i)), Ok(()));
    }

    drop(r);
    assert_eq!(future::block_on(s.send(777)), Err(SendError(777)));
}

#[test]
fn send_after_close() {
    let (s, r) = unbounded();

    future::block_on(s.send(1)).unwrap();
    future::block_on(s.send(2)).unwrap();
    future::block_on(s.send(3)).unwrap();

    drop(r);

    assert_eq!(future::block_on(s.send(4)), Err(SendError(4)));
    assert_eq!(s.try_send(5), Err(TrySendError::Closed(5)));
}

#[test]
fn recv_after_close() {
    let (s, r) = unbounded();

    future::block_on(s.send(1)).unwrap();
    future::block_on(s.send(2)).unwrap();
    future::block_on(s.send(3)).unwrap();

    drop(s);

    assert_eq!(future::block_on(r.recv()), Ok(1));
    assert_eq!(future::block_on(r.recv()), Ok(2));
    assert_eq!(future::block_on(r.recv()), Ok(3));
    assert_eq!(future::block_on(r.recv()), Err(RecvError));
}

#[test]
fn len() {
    let (s, r) = unbounded();

    assert_eq!(s.len(), 0);
    assert_eq!(r.len(), 0);

    for i in 0..50 {
        future::block_on(s.send(i)).unwrap();
        assert_eq!(s.len(), i + 1);
    }

    for i in 0..50 {
        future::block_on(r.recv()).unwrap();
        assert_eq!(r.len(), 50 - i - 1);
    }

    assert_eq!(s.len(), 0);
    assert_eq!(r.len(), 0);
}

#[test]
fn receiver_count() {
    let (s, r) = unbounded::<()>();
    let receiver_clones: Vec<_> = (0..20).map(|_| r.clone()).collect();

    assert_eq!(s.receiver_count(), 21);
    assert_eq!(r.receiver_count(), 21);

    drop(receiver_clones);

    assert_eq!(s.receiver_count(), 1);
    assert_eq!(r.receiver_count(), 1);
}

#[test]
fn sender_count() {
    let (s, r) = unbounded::<()>();
    let sender_clones: Vec<_> = (0..20).map(|_| s.clone()).collect();

    assert_eq!(s.sender_count(), 21);
    assert_eq!(r.sender_count(), 21);

    drop(sender_clones);

    assert_eq!(s.receiver_count(), 1);
    assert_eq!(r.receiver_count(), 1);
}

#[cfg(not(target_family = "wasm"))]
#[test]
fn close_wakes_receiver() {
    let (s, r) = unbounded::<()>();

    Parallel::new()
        .add(move || {
            assert_eq!(future::block_on(r.recv()), Err(RecvError));
        })
        .add(move || {
            sleep(ms(1000));
            drop(s);
        })
        .run();
}

#[cfg(not(target_family = "wasm"))]
#[test]
fn spsc() {
    const COUNT: usize = 100_000;

    let (s, r) = unbounded();

    Parallel::new()
        .add(move || {
            for i in 0..COUNT {
                assert_eq!(future::block_on(r.recv()), Ok(i));
            }
            assert_eq!(future::block_on(r.recv()), Err(RecvError));
        })
        .add(move || {
            for i in 0..COUNT {
                future::block_on(s.send(i)).unwrap();
            }
        })
        .run();
}

#[cfg(not(target_family = "wasm"))]
#[test]
fn mpmc() {
    const COUNT: usize = 25_000;
    const THREADS: usize = 4;

    let (s, r) = unbounded::<usize>();
    let v = (0..COUNT).map(|_| AtomicUsize::new(0)).collect::<Vec<_>>();

    Parallel::new()
        .each(0..THREADS, |_| {
            for _ in 0..COUNT {
                let n = future::block_on(r.recv()).unwrap();
                v[n].fetch_add(1, Ordering::SeqCst);
            }
        })
        .each(0..THREADS, |_| {
            for i in 0..COUNT {
                future::block_on(s.send(i)).unwrap();
            }
        })
        .run();

    assert_eq!(r.try_recv(), Err(TryRecvError::Empty));

    for c in v {
        assert_eq!(c.load(Ordering::SeqCst), THREADS);
    }
}

#[cfg(not(target_family = "wasm"))]
#[test]
fn mpmc_stream() {
    const COUNT: usize = 25_000;
    const THREADS: usize = 4;

    let (s, r) = unbounded::<usize>();
    let v = (0..COUNT).map(|_| AtomicUsize::new(0)).collect::<Vec<_>>();
    let v = &v;

    Parallel::new()
        .each(0..THREADS, {
            let r = r.clone();
            move |_| {
                futures_lite::pin!(r);
                for _ in 0..COUNT {
                    let n = future::block_on(r.next()).unwrap();
                    v[n].fetch_add(1, Ordering::SeqCst);
                }
            }
        })
        .each(0..THREADS, |_| {
            for i in 0..COUNT {
                future::block_on(s.send(i)).unwrap();
            }
        })
        .run();

    assert_eq!(r.try_recv(), Err(TryRecvError::Empty));

    for c in v {
        assert_eq!(c.load(Ordering::SeqCst), THREADS);
    }
}

#[cfg(all(feature = "std", not(target_family = "wasm")))]
#[test]
fn weak() {
    let (s, r) = unbounded::<usize>();

    // Create a weak sender/receiver pair.
    let (weak_s, weak_r) = (s.downgrade(), r.downgrade());

    // Upgrade and send.
    {
        let s = weak_s.upgrade().unwrap();
        s.send_blocking(3).unwrap();
        let r = weak_r.upgrade().unwrap();
        assert_eq!(r.recv_blocking(), Ok(3));
    }

    // Drop the original sender/receiver pair.
    drop((s, r));

    // Try to upgrade again.
    {
        assert!(weak_s.upgrade().is_none());
        assert!(weak_r.upgrade().is_none());
    }
}
