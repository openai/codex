use atomic_waker::AtomicWaker;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkGroup, Criterion};

use std::sync::Arc;
use std::task::{Wake, Waker};

enum Contention {
    Low,
    High,
}

impl Contention {
    fn bench<M>(
        self,
        group: &mut BenchmarkGroup<'_, M>,
        f: impl Fn(&AtomicWaker) + Send + Sync + Clone,
    ) where
        M: criterion::measurement::Measurement,
    {
        let waker = AtomicWaker::new();

        match self {
            Self::Low => {
                // Run the benchmark with low contention.
                group.bench_function("low contention", move |b| {
                    b.iter(|| {
                        for _ in 0..500 {
                            f(&waker);
                        }
                    })
                });
            }

            Self::High => {
                // Run the benchmark with high contention.
                group.bench_function("high contention", move |b| {
                    b.iter(|| {
                        use rayon::prelude::*;
                        (0..100_000).into_par_iter().for_each(|_| f(&waker));
                    })
                });
            }
        }
    }
}

fn run_lo_hi(c: &mut Criterion, name: &str, f: impl Fn(&AtomicWaker) + Send + Sync + Clone) {
    let mut group = c.benchmark_group(name);

    Contention::Low.bench(&mut group, f.clone());
    Contention::High.bench(&mut group, f);

    group.finish();
}

fn store_and_wake(c: &mut Criterion) {
    run_lo_hi(c, "store and wake", |waker| {
        let noop_waker = noop_waker();
        waker.register(&noop_waker);
        waker.wake();
    });

    run_lo_hi(c, "store and take", |waker| {
        let noop_waker = noop_waker();
        waker.register(&noop_waker);
        black_box(waker.take());
    });
}

fn wake_without_store(c: &mut Criterion) {
    run_lo_hi(c, "wake without store", |waker| {
        waker.wake();
    });

    run_lo_hi(c, "take without store", |waker| {
        black_box(waker.take());
    });
}

criterion_group!(benches, store_and_wake, wake_without_store);

criterion_main!(benches);

fn noop_waker() -> Waker {
    struct Noop;

    impl Wake for Noop {
        fn wake(self: Arc<Self>) {
            // Do nothing
        }

        fn wake_by_ref(self: &Arc<Self>) {
            // Do nothing
        }
    }

    Waker::from(Arc::new(Noop))
}
