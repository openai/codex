use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use std::{
    sync::{Arc, Barrier, RwLock},
    thread,
    time::{Duration, Instant},
};

#[derive(Clone)]
struct MultithreadedBench<T> {
    start: Arc<Barrier>,
    end: Arc<Barrier>,
    slab: Arc<T>,
}

impl<T: Send + Sync + 'static> MultithreadedBench<T> {
    fn new(slab: Arc<T>) -> Self {
        Self {
            start: Arc::new(Barrier::new(5)),
            end: Arc::new(Barrier::new(5)),
            slab,
        }
    }

    fn thread(&self, f: impl FnOnce(&Barrier, &T) + Send + 'static) -> &Self {
        let start = self.start.clone();
        let end = self.end.clone();
        let slab = self.slab.clone();
        thread::spawn(move || {
            f(&start, &*slab);
            end.wait();
        });
        self
    }

    fn run(&self) -> Duration {
        self.start.wait();
        let t0 = Instant::now();
        self.end.wait();
        t0.elapsed()
    }
}

const N_INSERTIONS: &[usize] = &[100, 300, 500, 700, 1000, 3000, 5000];

fn insert_remove_local(c: &mut Criterion) {
    // the 10000-insertion benchmark takes the `slab` crate about an hour to
    // run; don't run this unless you're prepared for that...
    // const N_INSERTIONS: &'static [usize] = &[100, 500, 1000, 5000, 10000];
    let mut group = c.benchmark_group("insert_remove_local");
    let g = group.measurement_time(Duration::from_secs(15));

    for i in N_INSERTIONS {
        g.bench_with_input(BenchmarkId::new("sharded_slab", i), i, |b, &i| {
            b.iter_custom(|iters| {
                let mut total = Duration::from_secs(0);
                for _ in 0..iters {
                    let bench = MultithreadedBench::new(Arc::new(sharded_slab::Slab::new()));
                    let elapsed = bench
                        .thread(move |start, slab| {
                            start.wait();
                            let v: Vec<_> = (0..i).map(|i| slab.insert(i).unwrap()).collect();
                            for i in v {
                                slab.remove(i);
                            }
                        })
                        .thread(move |start, slab| {
                            start.wait();
                            let v: Vec<_> = (0..i).map(|i| slab.insert(i).unwrap()).collect();
                            for i in v {
                                slab.remove(i);
                            }
                        })
                        .thread(move |start, slab| {
                            start.wait();
                            let v: Vec<_> = (0..i).map(|i| slab.insert(i).unwrap()).collect();
                            for i in v {
                                slab.remove(i);
                            }
                        })
                        .thread(move |start, slab| {
                            start.wait();
                            let v: Vec<_> = (0..i).map(|i| slab.insert(i).unwrap()).collect();
                            for i in v {
                                slab.remove(i);
                            }
                        })
                        .run();
                    total += elapsed;
                }
                total
            })
        });
        g.bench_with_input(BenchmarkId::new("slab_biglock", i), i, |b, &i| {
            b.iter_custom(|iters| {
                let mut total = Duration::from_secs(0);
                let i = i;
                for _ in 0..iters {
                    let bench = MultithreadedBench::new(Arc::new(RwLock::new(slab::Slab::new())));
                    let elapsed = bench
                        .thread(move |start, slab| {
                            start.wait();
                            let v: Vec<_> =
                                (0..i).map(|i| slab.write().unwrap().insert(i)).collect();
                            for i in v {
                                slab.write().unwrap().remove(i);
                            }
                        })
                        .thread(move |start, slab| {
                            start.wait();
                            let v: Vec<_> =
                                (0..i).map(|i| slab.write().unwrap().insert(i)).collect();
                            for i in v {
                                slab.write().unwrap().remove(i);
                            }
                        })
                        .thread(move |start, slab| {
                            start.wait();
                            let v: Vec<_> =
                                (0..i).map(|i| slab.write().unwrap().insert(i)).collect();
                            for i in v {
                                slab.write().unwrap().remove(i);
                            }
                        })
                        .thread(move |start, slab| {
                            start.wait();
                            let v: Vec<_> =
                                (0..i).map(|i| slab.write().unwrap().insert(i)).collect();
                            for i in v {
                                slab.write().unwrap().remove(i);
                            }
                        })
                        .run();
                    total += elapsed;
                }
                total
            })
        });
    }
    group.finish();
}

fn insert_remove_single_thread(c: &mut Criterion) {
    // the 10000-insertion benchmark takes the `slab` crate about an hour to
    // run; don't run this unless you're prepared for that...
    // const N_INSERTIONS: &'static [usize] = &[100, 500, 1000, 5000, 10000];
    let mut group = c.benchmark_group("insert_remove_single_threaded");

    for i in N_INSERTIONS {
        group.bench_with_input(BenchmarkId::new("sharded_slab", i), i, |b, &i| {
            let slab = sharded_slab::Slab::new();
            b.iter(|| {
                let v: Vec<_> = (0..i).map(|i| slab.insert(i).unwrap()).collect();
                for i in v {
                    slab.remove(i);
                }
            });
        });
        group.bench_with_input(BenchmarkId::new("slab_no_lock", i), i, |b, &i| {
            let mut slab = slab::Slab::new();
            b.iter(|| {
                let v: Vec<_> = (0..i).map(|i| slab.insert(i)).collect();
                for i in v {
                    slab.remove(i);
                }
            });
        });
        group.bench_with_input(BenchmarkId::new("slab_uncontended", i), i, |b, &i| {
            let slab = RwLock::new(slab::Slab::new());
            b.iter(|| {
                let v: Vec<_> = (0..i).map(|i| slab.write().unwrap().insert(i)).collect();
                for i in v {
                    slab.write().unwrap().remove(i);
                }
            });
        });
    }
    group.finish();
}

criterion_group!(benches, insert_remove_local, insert_remove_single_thread);
criterion_main!(benches);
