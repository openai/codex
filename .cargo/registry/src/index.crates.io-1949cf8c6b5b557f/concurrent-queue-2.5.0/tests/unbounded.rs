#![allow(clippy::bool_assert_comparison)]

use concurrent_queue::{ConcurrentQueue, PopError, PushError};

#[cfg(not(target_family = "wasm"))]
use easy_parallel::Parallel;
#[cfg(not(target_family = "wasm"))]
use std::sync::atomic::{AtomicUsize, Ordering};

#[cfg(target_family = "wasm")]
use wasm_bindgen_test::wasm_bindgen_test as test;

#[test]
fn smoke() {
    let q = ConcurrentQueue::unbounded();
    q.push(7).unwrap();
    assert_eq!(q.pop(), Ok(7));

    q.push(8).unwrap();
    assert_eq!(q.pop(), Ok(8));
    assert!(q.pop().is_err());
}

#[test]
fn len_empty_full() {
    let q = ConcurrentQueue::unbounded();

    assert_eq!(q.len(), 0);
    assert_eq!(q.is_empty(), true);

    q.push(()).unwrap();

    assert_eq!(q.len(), 1);
    assert_eq!(q.is_empty(), false);

    q.pop().unwrap();

    assert_eq!(q.len(), 0);
    assert_eq!(q.is_empty(), true);
}

#[test]
fn len() {
    let q = ConcurrentQueue::unbounded();

    assert_eq!(q.len(), 0);

    for i in 0..50 {
        q.push(i).unwrap();
        assert_eq!(q.len(), i + 1);
    }

    for i in 0..50 {
        q.pop().unwrap();
        assert_eq!(q.len(), 50 - i - 1);
    }

    assert_eq!(q.len(), 0);
}

#[test]
fn close() {
    let q = ConcurrentQueue::unbounded();
    assert_eq!(q.push(10), Ok(()));

    assert!(!q.is_closed());
    assert!(q.close());

    assert!(q.is_closed());
    assert!(!q.close());

    assert_eq!(q.push(20), Err(PushError::Closed(20)));
    assert_eq!(q.pop(), Ok(10));
    assert_eq!(q.pop(), Err(PopError::Closed));
}

#[cfg(not(target_family = "wasm"))]
#[test]
fn spsc() {
    const COUNT: usize = if cfg!(miri) { 100 } else { 100_000 };

    let q = ConcurrentQueue::unbounded();

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
                q.push(i).unwrap();
            }
        })
        .run();
}

#[cfg(not(target_family = "wasm"))]
#[test]
fn mpmc() {
    const COUNT: usize = if cfg!(miri) { 100 } else { 25_000 };
    const THREADS: usize = 4;

    let q = ConcurrentQueue::<usize>::unbounded();
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
                q.push(i).unwrap();
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
        let steps = fastrand::usize(0..STEPS);
        let additional = fastrand::usize(0..1000);

        DROPS.store(0, Ordering::SeqCst);
        let q = ConcurrentQueue::unbounded();

        Parallel::new()
            .add(|| {
                for _ in 0..steps {
                    while q.pop().is_err() {}
                }
            })
            .add(|| {
                for _ in 0..steps {
                    q.push(DropCounter).unwrap();
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
