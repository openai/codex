use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::{io, time::Duration};

mod support;
use support::MultithreadedBench;

/// A fake writer that doesn't actually do anything.
///
/// We want to measure the subscriber's overhead, *not* the performance of
/// stdout/file writers. Using a no-op Write implementation lets us only measure
/// the subscriber's overhead.
struct NoWriter;

impl io::Write for NoWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl NoWriter {
    fn new() -> Self {
        Self
    }
}

fn bench_new_span(c: &mut Criterion) {
    bench_thrpt(c, "new_span", |group, i| {
        group.bench_with_input(BenchmarkId::new("single_thread", i), i, |b, &i| {
            tracing::dispatcher::with_default(&mk_dispatch(), || {
                b.iter(|| {
                    for n in 0..i {
                        let _span = tracing::info_span!("span", n);
                    }
                })
            });
        });
        group.bench_with_input(BenchmarkId::new("multithreaded", i), i, |b, &i| {
            b.iter_custom(|iters| {
                let mut total = Duration::from_secs(0);
                let dispatch = mk_dispatch();
                for _ in 0..iters {
                    let bench = MultithreadedBench::new(dispatch.clone());
                    let elapsed = bench
                        .thread(move || {
                            for n in 0..i {
                                let _span = tracing::info_span!("span", n);
                            }
                        })
                        .thread(move || {
                            for n in 0..i {
                                let _span = tracing::info_span!("span", n);
                            }
                        })
                        .thread(move || {
                            for n in 0..i {
                                let _span = tracing::info_span!("span", n);
                            }
                        })
                        .thread(move || {
                            for n in 0..i {
                                let _span = tracing::info_span!("span", n);
                            }
                        })
                        .run();
                    total += elapsed;
                }
                total
            })
        });
    });
}

type Group<'a> = criterion::BenchmarkGroup<'a, criterion::measurement::WallTime>;
fn bench_thrpt(c: &mut Criterion, name: &'static str, mut f: impl FnMut(&mut Group<'_>, &usize)) {
    const N_SPANS: &[usize] = &[1, 10, 50];

    let mut group = c.benchmark_group(name);
    for spans in N_SPANS {
        group.throughput(Throughput::Elements(*spans as u64));
        f(&mut group, spans);
    }
    group.finish();
}

fn mk_dispatch() -> tracing::Dispatch {
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_writer(NoWriter::new)
        .finish();
    tracing::Dispatch::new(subscriber)
}

fn bench_event(c: &mut Criterion) {
    bench_thrpt(c, "event", |group, i| {
        group.bench_with_input(BenchmarkId::new("root/single_threaded", i), i, |b, &i| {
            let dispatch = mk_dispatch();
            tracing::dispatcher::with_default(&dispatch, || {
                b.iter(|| {
                    for n in 0..i {
                        tracing::info!(n);
                    }
                })
            });
        });
        group.bench_with_input(BenchmarkId::new("root/multithreaded", i), i, |b, &i| {
            b.iter_custom(|iters| {
                let mut total = Duration::from_secs(0);
                let dispatch = mk_dispatch();
                for _ in 0..iters {
                    let bench = MultithreadedBench::new(dispatch.clone());
                    let elapsed = bench
                        .thread(move || {
                            for n in 0..i {
                                tracing::info!(n);
                            }
                        })
                        .thread(move || {
                            for n in 0..i {
                                tracing::info!(n);
                            }
                        })
                        .thread(move || {
                            for n in 0..i {
                                tracing::info!(n);
                            }
                        })
                        .thread(move || {
                            for n in 0..i {
                                tracing::info!(n);
                            }
                        })
                        .run();
                    total += elapsed;
                }
                total
            })
        });
        group.bench_with_input(
            BenchmarkId::new("unique_parent/single_threaded", i),
            i,
            |b, &i| {
                tracing::dispatcher::with_default(&mk_dispatch(), || {
                    let span = tracing::info_span!("unique_parent", foo = false);
                    let _guard = span.enter();
                    b.iter(|| {
                        for n in 0..i {
                            tracing::info!(n);
                        }
                    })
                });
            },
        );
        group.bench_with_input(
            BenchmarkId::new("unique_parent/multithreaded", i),
            i,
            |b, &i| {
                b.iter_custom(|iters| {
                    let mut total = Duration::from_secs(0);
                    let dispatch = mk_dispatch();
                    for _ in 0..iters {
                        let bench = MultithreadedBench::new(dispatch.clone());
                        let elapsed = bench
                            .thread_with_setup(move |start| {
                                let span = tracing::info_span!("unique_parent", foo = false);
                                let _guard = span.enter();
                                start.wait();
                                for n in 0..i {
                                    tracing::info!(n);
                                }
                            })
                            .thread_with_setup(move |start| {
                                let span = tracing::info_span!("unique_parent", foo = false);
                                let _guard = span.enter();
                                start.wait();
                                for n in 0..i {
                                    tracing::info!(n);
                                }
                            })
                            .thread_with_setup(move |start| {
                                let span = tracing::info_span!("unique_parent", foo = false);
                                let _guard = span.enter();
                                start.wait();
                                for n in 0..i {
                                    tracing::info!(n);
                                }
                            })
                            .thread_with_setup(move |start| {
                                let span = tracing::info_span!("unique_parent", foo = false);
                                let _guard = span.enter();
                                start.wait();
                                for n in 0..i {
                                    tracing::info!(n);
                                }
                            })
                            .run();
                        total += elapsed;
                    }
                    total
                })
            },
        );
        group.bench_with_input(
            BenchmarkId::new("shared_parent/multithreaded", i),
            i,
            |b, &i| {
                b.iter_custom(|iters| {
                    let dispatch = mk_dispatch();
                    let mut total = Duration::from_secs(0);
                    for _ in 0..iters {
                        let parent = tracing::dispatcher::with_default(&dispatch, || {
                            tracing::info_span!("shared_parent", foo = "hello world")
                        });
                        let bench = MultithreadedBench::new(dispatch.clone());
                        let parent2 = parent.clone();
                        bench.thread_with_setup(move |start| {
                            let _guard = parent2.enter();
                            start.wait();
                            for n in 0..i {
                                tracing::info!(n);
                            }
                        });
                        let parent2 = parent.clone();
                        bench.thread_with_setup(move |start| {
                            let _guard = parent2.enter();
                            start.wait();
                            for n in 0..i {
                                tracing::info!(n);
                            }
                        });
                        let parent2 = parent.clone();
                        bench.thread_with_setup(move |start| {
                            let _guard = parent2.enter();
                            start.wait();
                            for n in 0..i {
                                tracing::info!(n);
                            }
                        });
                        let parent2 = parent.clone();
                        bench.thread_with_setup(move |start| {
                            let _guard = parent2.enter();
                            start.wait();
                            for n in 0..i {
                                tracing::info!(n);
                            }
                        });
                        let elapsed = bench.run();
                        total += elapsed;
                    }
                    total
                })
            },
        );
        group.bench_with_input(
            BenchmarkId::new("multi-parent/multithreaded", i),
            i,
            |b, &i| {
                b.iter_custom(|iters| {
                    let dispatch = mk_dispatch();
                    let mut total = Duration::from_secs(0);
                    for _ in 0..iters {
                        let parent = tracing::dispatcher::with_default(&dispatch, || {
                            tracing::info_span!("multiparent", foo = "hello world")
                        });
                        let bench = MultithreadedBench::new(dispatch.clone());
                        let parent2 = parent.clone();
                        bench.thread_with_setup(move |start| {
                            let _guard = parent2.enter();
                            start.wait();
                            let mut span = tracing::info_span!("parent");
                            for n in 0..i {
                                let s = tracing::info_span!(parent: &span, "parent2", n, i);
                                s.in_scope(|| {
                                    tracing::info!(n);
                                });
                                span = s;
                            }
                        });
                        let parent2 = parent.clone();
                        bench.thread_with_setup(move |start| {
                            let _guard = parent2.enter();
                            start.wait();
                            let mut span = tracing::info_span!("parent");
                            for n in 0..i {
                                let s = tracing::info_span!(parent: &span, "parent2", n, i);
                                s.in_scope(|| {
                                    tracing::info!(n);
                                });
                                span = s;
                            }
                        });
                        let parent2 = parent.clone();
                        bench.thread_with_setup(move |start| {
                            let _guard = parent2.enter();
                            start.wait();
                            let mut span = tracing::info_span!("parent");
                            for n in 0..i {
                                let s = tracing::info_span!(parent: &span, "parent2", n, i);
                                s.in_scope(|| {
                                    tracing::info!(n);
                                });
                                span = s;
                            }
                        });
                        let parent2 = parent.clone();
                        bench.thread_with_setup(move |start| {
                            let _guard = parent2.enter();
                            start.wait();
                            let mut span = tracing::info_span!("parent");
                            for n in 0..i {
                                let s = tracing::info_span!(parent: &span, "parent2", n, i);
                                s.in_scope(|| {
                                    tracing::info!(n);
                                });
                                span = s;
                            }
                        });
                        let elapsed = bench.run();
                        total += elapsed;
                    }
                    total
                })
            },
        );
    });
}

criterion_group!(benches, bench_new_span, bench_event);
criterion_main!(benches);
