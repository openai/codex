use std::sync::atomic::Ordering::SeqCst;
use std::sync::atomic::{AtomicBool, AtomicUsize};
use std::sync::{Arc, Mutex};

use crossbeam_deque::Steal::{Empty, Success};
use crossbeam_deque::Worker;
use crossbeam_utils::thread::scope;
use rand::Rng;

#[test]
fn smoke() {
    let w = Worker::new_fifo();
    let s = w.stealer();
    assert_eq!(w.pop(), None);
    assert_eq!(s.steal(), Empty);

    w.push(1);
    assert_eq!(w.pop(), Some(1));
    assert_eq!(w.pop(), None);
    assert_eq!(s.steal(), Empty);

    w.push(2);
    assert_eq!(s.steal(), Success(2));
    assert_eq!(s.steal(), Empty);
    assert_eq!(w.pop(), None);

    w.push(3);
    w.push(4);
    w.push(5);
    assert_eq!(s.steal(), Success(3));
    assert_eq!(s.steal(), Success(4));
    assert_eq!(s.steal(), Success(5));
    assert_eq!(s.steal(), Empty);

    w.push(6);
    w.push(7);
    w.push(8);
    w.push(9);
    assert_eq!(w.pop(), Some(6));
    assert_eq!(s.steal(), Success(7));
    assert_eq!(w.pop(), Some(8));
    assert_eq!(w.pop(), Some(9));
    assert_eq!(w.pop(), None);
}

#[test]
fn is_empty() {
    let w = Worker::new_fifo();
    let s = w.stealer();

    assert!(w.is_empty());
    w.push(1);
    assert!(!w.is_empty());
    w.push(2);
    assert!(!w.is_empty());
    let _ = w.pop();
    assert!(!w.is_empty());
    let _ = w.pop();
    assert!(w.is_empty());

    assert!(s.is_empty());
    w.push(1);
    assert!(!s.is_empty());
    w.push(2);
    assert!(!s.is_empty());
    let _ = s.steal();
    assert!(!s.is_empty());
    let _ = s.steal();
    assert!(s.is_empty());
}

#[test]
fn spsc() {
    #[cfg(miri)]
    const STEPS: usize = 500;
    #[cfg(not(miri))]
    const STEPS: usize = 50_000;

    let w = Worker::new_fifo();
    let s = w.stealer();

    scope(|scope| {
        scope.spawn(|_| {
            for i in 0..STEPS {
                loop {
                    if let Success(v) = s.steal() {
                        assert_eq!(i, v);
                        break;
                    }
                }
            }

            assert_eq!(s.steal(), Empty);
        });

        for i in 0..STEPS {
            w.push(i);
        }
    })
    .unwrap();
}

#[test]
fn stampede() {
    const THREADS: usize = 8;
    #[cfg(miri)]
    const COUNT: usize = 500;
    #[cfg(not(miri))]
    const COUNT: usize = 50_000;

    let w = Worker::new_fifo();

    for i in 0..COUNT {
        w.push(Box::new(i + 1));
    }
    let remaining = Arc::new(AtomicUsize::new(COUNT));

    scope(|scope| {
        for _ in 0..THREADS {
            let s = w.stealer();
            let remaining = remaining.clone();

            scope.spawn(move |_| {
                let mut last = 0;
                while remaining.load(SeqCst) > 0 {
                    if let Success(x) = s.steal() {
                        assert!(last < *x);
                        last = *x;
                        remaining.fetch_sub(1, SeqCst);
                    }
                }
            });
        }

        let mut last = 0;
        while remaining.load(SeqCst) > 0 {
            if let Some(x) = w.pop() {
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

    let w = Worker::new_fifo();
    let done = Arc::new(AtomicBool::new(false));
    let hits = Arc::new(AtomicUsize::new(0));

    scope(|scope| {
        for _ in 0..THREADS {
            let s = w.stealer();
            let done = done.clone();
            let hits = hits.clone();

            scope.spawn(move |_| {
                let w2 = Worker::new_fifo();

                while !done.load(SeqCst) {
                    if let Success(_) = s.steal() {
                        hits.fetch_add(1, SeqCst);
                    }

                    let _ = s.steal_batch(&w2);

                    if let Success(_) = s.steal_batch_and_pop(&w2) {
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
                while w.pop().is_some() {
                    hits.fetch_add(1, SeqCst);
                }
            } else {
                w.push(expected);
                expected += 1;
            }
        }

        while hits.load(SeqCst) < COUNT {
            while w.pop().is_some() {
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

    let w = Worker::new_fifo();
    let done = Arc::new(AtomicBool::new(false));
    let mut all_hits = Vec::new();

    scope(|scope| {
        for _ in 0..THREADS {
            let s = w.stealer();
            let done = done.clone();
            let hits = Arc::new(AtomicUsize::new(0));
            all_hits.push(hits.clone());

            scope.spawn(move |_| {
                let w2 = Worker::new_fifo();

                while !done.load(SeqCst) {
                    if let Success(_) = s.steal() {
                        hits.fetch_add(1, SeqCst);
                    }

                    let _ = s.steal_batch(&w2);

                    if let Success(_) = s.steal_batch_and_pop(&w2) {
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
                    while w.pop().is_some() {
                        my_hits += 1;
                    }
                } else {
                    w.push(i);
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

    let w = Worker::new_fifo();
    let dropped = Arc::new(Mutex::new(Vec::new()));
    let remaining = Arc::new(AtomicUsize::new(COUNT));

    for i in 0..COUNT {
        w.push(Elem(i, dropped.clone()));
    }

    scope(|scope| {
        for _ in 0..THREADS {
            let remaining = remaining.clone();
            let s = w.stealer();

            scope.spawn(move |_| {
                let w2 = Worker::new_fifo();
                let mut cnt = 0;

                while cnt < STEPS {
                    if let Success(_) = s.steal() {
                        cnt += 1;
                        remaining.fetch_sub(1, SeqCst);
                    }

                    let _ = s.steal_batch(&w2);

                    if let Success(_) = s.steal_batch_and_pop(&w2) {
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
            if w.pop().is_some() {
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

    drop(w);

    {
        let mut v = dropped.lock().unwrap();
        assert_eq!(v.len(), rem);
        v.sort_unstable();
        for pair in v.windows(2) {
            assert_eq!(pair[0] + 1, pair[1]);
        }
    }
}
