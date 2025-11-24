use std::sync::atomic::Ordering::SeqCst;
use std::sync::atomic::{AtomicBool, AtomicUsize};
use std::sync::{Arc, Mutex};

use crossbeam_deque::Steal::{Empty, Success};
use crossbeam_deque::{Injector, Worker};
use crossbeam_utils::thread::scope;
use rand::Rng;

#[test]
fn smoke() {
    let q = Injector::new();
    assert_eq!(q.steal(), Empty);

    q.push(1);
    q.push(2);
    assert_eq!(q.steal(), Success(1));
    assert_eq!(q.steal(), Success(2));
    assert_eq!(q.steal(), Empty);

    q.push(3);
    assert_eq!(q.steal(), Success(3));
    assert_eq!(q.steal(), Empty);
}

#[test]
fn is_empty() {
    let q = Injector::new();
    assert!(q.is_empty());

    q.push(1);
    assert!(!q.is_empty());
    q.push(2);
    assert!(!q.is_empty());

    let _ = q.steal();
    assert!(!q.is_empty());
    let _ = q.steal();
    assert!(q.is_empty());

    q.push(3);
    assert!(!q.is_empty());
    let _ = q.steal();
    assert!(q.is_empty());
}

#[test]
fn spsc() {
    #[cfg(miri)]
    const COUNT: usize = 500;
    #[cfg(not(miri))]
    const COUNT: usize = 100_000;

    let q = Injector::new();

    scope(|scope| {
        scope.spawn(|_| {
            for i in 0..COUNT {
                loop {
                    if let Success(v) = q.steal() {
                        assert_eq!(i, v);
                        break;
                    }
                    #[cfg(miri)]
                    std::hint::spin_loop();
                }
            }

            assert_eq!(q.steal(), Empty);
        });

        for i in 0..COUNT {
            q.push(i);
        }
    })
    .unwrap();
}

#[test]
fn mpmc() {
    #[cfg(miri)]
    const COUNT: usize = 500;
    #[cfg(not(miri))]
    const COUNT: usize = 25_000;
    const THREADS: usize = 4;

    let q = Injector::new();
    let v = (0..COUNT).map(|_| AtomicUsize::new(0)).collect::<Vec<_>>();

    scope(|scope| {
        for _ in 0..THREADS {
            scope.spawn(|_| {
                for i in 0..COUNT {
                    q.push(i);
                }
            });
        }

        for _ in 0..THREADS {
            scope.spawn(|_| {
                for _ in 0..COUNT {
                    loop {
                        if let Success(n) = q.steal() {
                            v[n].fetch_add(1, SeqCst);
                            break;
                        }
                        #[cfg(miri)]
                        std::hint::spin_loop();
                    }
                }
            });
        }
    })
    .unwrap();

    for c in v {
        assert_eq!(c.load(SeqCst), THREADS);
    }
}

#[test]
fn stampede() {
    const THREADS: usize = 8;
    #[cfg(miri)]
    const COUNT: usize = 500;
    #[cfg(not(miri))]
    const COUNT: usize = 50_000;

    let q = Injector::new();

    for i in 0..COUNT {
        q.push(Box::new(i + 1));
    }
    let remaining = Arc::new(AtomicUsize::new(COUNT));

    scope(|scope| {
        for _ in 0..THREADS {
            let remaining = remaining.clone();
            let q = &q;

            scope.spawn(move |_| {
                let mut last = 0;
                while remaining.load(SeqCst) > 0 {
                    if let Success(x) = q.steal() {
                        assert!(last < *x);
                        last = *x;
                        remaining.fetch_sub(1, SeqCst);
                    }
                }
            });
        }

        let mut last = 0;
        while remaining.load(SeqCst) > 0 {
            if let Success(x) = q.steal() {
                assert!(last < *x);
                last = *x;
                remaining.fetch_sub(1, SeqCst);
            }
        }
    })
    .unwrap();
}

#[test]
fn stress() {
    const THREADS: usize = 8;
    #[cfg(miri)]
    const COUNT: usize = 500;
    #[cfg(not(miri))]
    const COUNT: usize = 50_000;

    let q = Injector::new();
    let done = Arc::new(AtomicBool::new(false));
    let hits = Arc::new(AtomicUsize::new(0));

    scope(|scope| {
        for _ in 0..THREADS {
            let done = done.clone();
            let hits = hits.clone();
            let q = &q;

            scope.spawn(move |_| {
                let w2 = Worker::new_fifo();

                while !done.load(SeqCst) {
                    if let Success(_) = q.steal() {
                        hits.fetch_add(1, SeqCst);
                    }

                    let _ = q.steal_batch(&w2);

                    if let Success(_) = q.steal_batch_and_pop(&w2) {
                        hits.fetch_add(1, SeqCst);
                    }

                    while w2.pop().is_some() {
                        hits.fetch_add(1, SeqCst);
                    }
                }
            });
        }

        let mut rng = rand::thread_rng();
        let mut expected = 0;
        while expected < COUNT {
            if rng.gen_range(0..3) == 0 {
                while let Success(_) = q.steal() {
                    hits.fetch_add(1, SeqCst);
                }
            } else {
                q.push(expected);
                expected += 1;
            }
        }

        while hits.load(SeqCst) < COUNT {
            while let Success(_) = q.steal() {
                hits.fetch_add(1, SeqCst);
            }
        }
        done.store(true, SeqCst);
    })
    .unwrap();
}

#[cfg_attr(miri, ignore)] // Miri is too slow
#[test]
fn no_starvation() {
    const THREADS: usize = 8;
    const COUNT: usize = 50_000;

    let q = Injector::new();
    let done = Arc::new(AtomicBool::new(false));
    let mut all_hits = Vec::new();

    scope(|scope| {
        for _ in 0..THREADS {
            let done = done.clone();
            let hits = Arc::new(AtomicUsize::new(0));
            all_hits.push(hits.clone());
            let q = &q;

            scope.spawn(move |_| {
                let w2 = Worker::new_fifo();

                while !done.load(SeqCst) {
                    if let Success(_) = q.steal() {
                        hits.fetch_add(1, SeqCst);
                    }

                    let _ = q.steal_batch(&w2);

                    if let Success(_) = q.steal_batch_and_pop(&w2) {
                        hits.fetch_add(1, SeqCst);
                    }

                    while w2.pop().is_some() {
                        hits.fetch_add(1, SeqCst);
                    }
                }
            });
        }

        let mut rng = rand::thread_rng();
        let mut my_hits = 0;
        loop {
            for i in 0..rng.gen_range(0..COUNT) {
                if rng.gen_range(0..3) == 0 && my_hits == 0 {
                    while let Success(_) = q.steal() {
                        my_hits += 1;
                    }
                } else {
                    q.push(i);
                }
            }

            if my_hits > 0 && all_hits.iter().all(|h| h.load(SeqCst) > 0) {
                break;
            }
        }
        done.store(true, SeqCst);
    })
    .unwrap();
}

#[test]
fn destructors() {
    #[cfg(miri)]
    const THREADS: usize = 2;
    #[cfg(not(miri))]
    const THREADS: usize = 8;
    #[cfg(miri)]
    const COUNT: usize = 500;
    #[cfg(not(miri))]
    const COUNT: usize = 50_000;
    #[cfg(miri)]
    const STEPS: usize = 100;
    #[cfg(not(miri))]
    const STEPS: usize = 1000;

    struct Elem(usize, Arc<Mutex<Vec<usize>>>);

    impl Drop for Elem {
        fn drop(&mut self) {
            self.1.lock().unwrap().push(self.0);
        }
    }

    let q = Injector::new();
    let dropped = Arc::new(Mutex::new(Vec::new()));
    let remaining = Arc::new(AtomicUsize::new(COUNT));

    for i in 0..COUNT {
        q.push(Elem(i, dropped.clone()));
    }

    scope(|scope| {
        for _ in 0..THREADS {
            let remaining = remaining.clone();
            let q = &q;

            scope.spawn(move |_| {
                let w2 = Worker::new_fifo();
                let mut cnt = 0;

                while cnt < STEPS {
                    if let Success(_) = q.steal() {
                        cnt += 1;
                        remaining.fetch_sub(1, SeqCst);
                    }

                    let _ = q.steal_batch(&w2);

                    if let Success(_) = q.steal_batch_and_pop(&w2) {
                        cnt += 1;
                        remaining.fetch_sub(1, SeqCst);
                    }

                    while w2.pop().is_some() {
                        cnt += 1;
                        remaining.fetch_sub(1, SeqCst);
                    }
                }
            });
        }

        for _ in 0..STEPS {
            if let Success(_) = q.steal() {
                remaining.fetch_sub(1, SeqCst);
            }
        }
    })
    .unwrap();

    let rem = remaining.load(SeqCst);
    assert!(rem > 0);

    {
        let mut v = dropped.lock().unwrap();
        assert_eq!(v.len(), COUNT - rem);
        v.clear();
    }

    drop(q);

    {
        let mut v = dropped.lock().unwrap();
        assert_eq!(v.len(), rem);
        v.sort_unstable();
        for pair in v.windows(2) {
            assert_eq!(pair[0] + 1, pair[1]);
        }
    }
}

// If `Block` is created on the stack, the array of slots will multiply this `BigStruct` and
// probably overflow the thread stack. It's now directly created on the heap to avoid this.
#[test]
fn stack_overflow() {
    const N: usize = 32_768;
    struct BigStruct {
        _data: [u8; N],
    }

    let q = Injector::new();

    q.push(BigStruct { _data: [0u8; N] });

    while !matches!(q.steal(), Empty) {}
}
