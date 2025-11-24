use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering::{AcqRel, Acquire, Relaxed};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use crossbeam_epoch::{self as epoch, Atomic, Collector, LocalHandle, Owned, Shared};
use rand::Rng;

fn worker(a: Arc<Atomic<AtomicUsize>>, handle: LocalHandle) -> usize {
    let mut rng = rand::thread_rng();
    let mut sum = 0;

    if rng.gen() {
        thread::sleep(Duration::from_millis(1));
    }
    let timeout = Duration::from_millis(rng.gen_range(0..10));
    let now = Instant::now();

    while now.elapsed() < timeout {
        for _ in 0..100 {
            let guard = &handle.pin();
            guard.flush();

            let val = if rng.gen() {
                let p = a.swap(Owned::new(AtomicUsize::new(sum)), AcqRel, guard);
                unsafe {
                    guard.defer_destroy(p);
                    guard.flush();
                    p.deref().load(Relaxed)
                }
            } else {
                let p = a.load(Acquire, guard);
                unsafe { p.deref().fetch_add(sum, Relaxed) }
            };

            sum = sum.wrapping_add(val);
        }
    }

    sum
}

fn main() {
    for _ in 0..100 {
        let collector = Collector::new();
        let a = Arc::new(Atomic::new(AtomicUsize::new(777)));

        let threads = (0..16)
            .map(|_| {
                let a = a.clone();
                let c = collector.clone();
                thread::spawn(move || worker(a, c.register()))
            })
            .collect::<Vec<_>>();

        for t in threads {
            t.join().unwrap();
        }

        unsafe {
            a.swap(Shared::null(), AcqRel, epoch::unprotected())
                .into_owned();
        }
    }
}
