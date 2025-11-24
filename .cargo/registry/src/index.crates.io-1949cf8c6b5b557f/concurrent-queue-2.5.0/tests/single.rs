#![allow(clippy::bool_assert_comparison)]

use concurrent_queue::{ConcurrentQueue, ForcePushError, PopError, PushError};

#[cfg(not(target_family = "wasm"))]
use easy_parallel::Parallel;
#[cfg(not(target_family = "wasm"))]
use std::sync::atomic::{AtomicUsize, Ordering};

#[cfg(target_family = "wasm")]
use wasm_bindgen_test::wasm_bindgen_test as test;

#[test]
fn smoke() {
    let q = ConcurrentQueue::bounded(1);

    q.push(7).unwrap();
    assert_eq!(q.pop(), Ok(7));

    q.push(8).unwrap();
    assert_eq!(q.pop(), Ok(8));
    assert!(q.pop().is_err());
}

#[test]
fn capacity() {
    let q = ConcurrentQueue::<i32>::bounded(1);
    assert_eq!(q.capacity(), Some(1));
}

#[test]
fn len_empty_full() {
    let q = ConcurrentQueue::bounded(1);

    assert_eq!(q.len(), 0);
    assert_eq!(q.is_empty(), true);
    assert_eq!(q.is_full(), false);

    q.push(()).unwrap();

    assert_eq!(q.len(), 1);
    assert_eq!(q.is_empty(), false);
    assert_eq!(q.is_full(), true);

    q.pop().unwrap();

    assert_eq!(q.len(), 0);
    assert_eq!(q.is_empty(), true);
    assert_eq!(q.is_full(), false);
}

#[test]
fn close() {
    let q = ConcurrentQueue::<i32>::bounded(1);
    assert_eq!(q.push(10), Ok(()));

    assert!(!q.is_closed());
    assert!(q.close());

    assert!(q.is_closed());
    assert!(!q.close());

    assert_eq!(q.push(20), Err(PushError::Closed(20)));
    assert_eq!(q.pop(), Ok(10));
    assert_eq!(q.pop(), Err(PopError::Closed));
}

#[test]
fn force_push() {
    let q = ConcurrentQueue::<i32>::bounded(1);
    assert_eq!(q.force_push(10), Ok(None));

    assert!(!q.is_closed());
    assert_eq!(q.force_push(20), Ok(Some(10)));
    assert_eq!(q.force_push(30), Ok(Some(20)));

    assert!(q.close());
    assert_eq!(q.force_push(40), Err(ForcePushError(40)));
    assert_eq!(q.pop(), Ok(30));
    assert_eq!(q.pop(), Err(PopError::Closed));
}

#[cfg(not(target_family = "wasm"))]
#[test]
fn spsc() {
    const COUNT: usize = if cfg!(miri) { 100 } else { 100_000 };

    let q = ConcurrentQueue::bounded(1);

    Parallel::new()
        .add(|| {
            for i in 0..COUNT {
                loop {
                    if let Ok(x) = q.pop() {
                        assert_eq!(x, i);
                        break;
                    }
                }
            }
            assert!(q.pop().is_err());
        })
        .add(|| {
            for i in 0..COUNT {
                while q.push(i).is_err() {}
            }
        })
        .run();
}

#[cfg(not(target_family = "wasm"))]
#[test]
fn mpmc() {
    const COUNT: usize = if cfg!(miri) { 100 } else { 25_000 };
    const THREADS: usize = 1;

    let q = ConcurrentQueue::<usize>::bounded(THREADS);
    let v = (0..COUNT).map(|_| AtomicUsize::new(0)).collect::<Vec<_>>();

    Parallel::new()
        .each(0..THREADS, |_| {
            for _ in 0..COUNT {
                let n = loop {
                    if let Ok(x) = q.pop() {
                        break x;
                    }
                };
                v[n].fetch_add(1, Ordering::SeqCst);
            }
        })
        .each(0..THREADS, |_| {
            for i in 0..COUNT {
                while q.push(i).is_err() {}
            }
        })
        .run();

    for c in v {
        assert_eq!(c.load(Ordering::SeqCst), THREADS);
    }
}

#[cfg(not(target_family = "wasm"))]
#[test]
fn drops() {
    const RUNS: usize = if cfg!(miri) { 20 } else { 100 };
    const STEPS: usize = if cfg!(miri) { 100 } else { 10_000 };

    static DROPS: AtomicUsize = AtomicUsize::new(0);

    #[derive(Debug, PartialEq)]
    struct DropCounter;

    impl Drop for DropCounter {
        fn drop(&mut self) {
            DROPS.fetch_add(1, Ordering::SeqCst);
        }
    }

    for _ in 0..RUNS {
        let steps = fastrand::usize(..STEPS);
        let additional = fastrand::usize(0..=1);

        DROPS.store(0, Ordering::SeqCst);
        let q = ConcurrentQueue::bounded(1);

        Parallel::new()
            .add(|| {
                for _ in 0..steps {
                    while q.pop().is_err() {}
                }
            })
            .add(|| {
                for _ in 0..steps {
                    while q.push(DropCounter).is_err() {
                        DROPS.fetch_sub(1, Ordering::SeqCst);
                    }
                }
            })
            .run();

        for _ in 0..additional {
            q.push(DropCounter).unwrap();
        }

        assert_eq!(DROPS.load(Ordering::SeqCst), steps);
        drop(q);
        assert_eq!(DROPS.load(Ordering::SeqCst), steps + additional);
    }
}

#[cfg(not(target_family = "wasm"))]
#[test]
fn linearizable() {
    const COUNT: usize = if cfg!(miri) { 500 } else { 25_000 };
    const THREADS: usize = 4;

    let q = ConcurrentQueue::bounded(1);

    Parallel::new()
        .each(0..THREADS / 2, |_| {
            for _ in 0..COUNT {
                while q.push(0).is_err() {}
                q.pop().unwrap();
            }
        })
        .each(0..THREADS / 2, |_| {
            for _ in 0..COUNT {
                if q.force_push(0).unwrap().is_none() {
                    q.pop().unwrap();
                }
            }
        })
        .run();
}

#[cfg(not(target_family = "wasm"))]
#[test]
fn spsc_ring_buffer() {
    const COUNT: usize = if cfg!(miri) { 200 } else { 100_000 };

    let t = AtomicUsize::new(1);
    let q = ConcurrentQueue::<usize>::bounded(1);
    let v = (0..COUNT).map(|_| AtomicUsize::new(0)).collect::<Vec<_>>();

    Parallel::new()
        .add(|| loop {
            match t.load(Ordering::SeqCst) {
                0 if q.is_empty() => break,

                _ => {
                    while let Ok(n) = q.pop() {
                        v[n].fetch_add(1, Ordering::SeqCst);
                    }
                }
            }
        })
        .add(|| {
            for i in 0..COUNT {
                if let Ok(Some(n)) = q.force_push(i) {
                    v[n].fetch_add(1, Ordering::SeqCst);
                }
            }

            t.fetch_sub(1, Ordering::SeqCst);
        })
        .run();

    for c in v {
        assert_eq!(c.load(Ordering::SeqCst), 1);
    }
}

#[cfg(not(target_family = "wasm"))]
#[test]
fn mpmc_ring_buffer() {
    const COUNT: usize = if cfg!(miri) { 100 } else { 25_000 };
    const THREADS: usize = 4;

    let t = AtomicUsize::new(THREADS);
    let q = ConcurrentQueue::<usize>::bounded(1);
    let v = (0..COUNT).map(|_| AtomicUsize::new(0)).collect::<Vec<_>>();

    Parallel::new()
        .each(0..THREADS, |_| loop {
            match t.load(Ordering::SeqCst) {
                0 if q.is_empty() => break,

                _ => {
                    while let Ok(n) = q.pop() {
                        v[n].fetch_add(1, Ordering::SeqCst);
                    }
                }
            }
        })
        .each(0..THREADS, |_| {
            for i in 0..COUNT {
                if let Ok(Some(n)) = q.force_push(i) {
                    v[n].fetch_add(1, Ordering::SeqCst);
                }
            }

            t.fetch_sub(1, Ordering::SeqCst);
        })
        .run();

    for c in v {
        assert_eq!(c.load(Ordering::SeqCst), THREADS);
    }
}
