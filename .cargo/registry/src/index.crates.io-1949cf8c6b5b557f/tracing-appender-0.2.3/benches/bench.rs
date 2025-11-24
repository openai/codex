use criterion::{criterion_group, criterion_main, Criterion};
use std::{
    thread::{self, JoinHandle},
    time::Instant,
};
use tracing::{event, Level};
use tracing_appender::non_blocking;
use tracing_subscriber::fmt::MakeWriter;

// a no-op writer is used in order to measure the overhead incurred by
// tracing-subscriber.
#[derive(Clone)]
struct NoOpWriter;

impl NoOpWriter {
    fn new() -> NoOpWriter {
        NoOpWriter
    }
}

impl<'a> MakeWriter<'a> for NoOpWriter {
    type Writer = NoOpWriter;

    fn make_writer(&self) -> Self::Writer {
        self.clone()
    }
}

impl std::io::Write for NoOpWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn synchronous_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("synchronous");
    group.bench_function("single_thread", |b| {
        let subscriber = tracing_subscriber::fmt().with_writer(NoOpWriter::new());
        tracing::subscriber::with_default(subscriber.finish(), || {
            b.iter(|| event!(Level::INFO, "event"))
        });
    });

    group.bench_function("multiple_writers", |b| {
        b.iter_custom(|iters| {
            let mut handles: Vec<JoinHandle<()>> = Vec::new();

            let start = Instant::now();

            let make_writer = NoOpWriter::new();
            let cloned_make_writer = make_writer.clone();

            handles.push(thread::spawn(move || {
                let subscriber = tracing_subscriber::fmt().with_writer(make_writer);
                tracing::subscriber::with_default(subscriber.finish(), || {
                    for _ in 0..iters {
                        event!(Level::INFO, "event");
                    }
                });
            }));

            handles.push(thread::spawn(move || {
                let subscriber = tracing_subscriber::fmt().with_writer(cloned_make_writer);
                tracing::subscriber::with_default(subscriber.finish(), || {
                    for _ in 0..iters {
                        event!(Level::INFO, "event");
                    }
                });
            }));

            for handle in handles {
                let _ = handle.join();
            }

            start.elapsed()
        });
    });
}

fn non_blocking_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("non_blocking");

    group.bench_function("single_thread", |b| {
        let (non_blocking, _guard) = non_blocking(NoOpWriter::new());
        let subscriber = tracing_subscriber::fmt().with_writer(non_blocking);

        tracing::subscriber::with_default(subscriber.finish(), || {
            b.iter(|| event!(Level::INFO, "event"))
        });
    });

    group.bench_function("multiple_writers", |b| {
        b.iter_custom(|iters| {
            let (non_blocking, _guard) = non_blocking(NoOpWriter::new());

            let mut handles: Vec<JoinHandle<()>> = Vec::new();

            let start = Instant::now();

            let cloned_make_writer = non_blocking.clone();

            handles.push(thread::spawn(move || {
                let subscriber = tracing_subscriber::fmt().with_writer(non_blocking);
                tracing::subscriber::with_default(subscriber.finish(), || {
                    for _ in 0..iters {
                        event!(Level::INFO, "event");
                    }
                });
            }));

            handles.push(thread::spawn(move || {
                let subscriber = tracing_subscriber::fmt().with_writer(cloned_make_writer);
                tracing::subscriber::with_default(subscriber.finish(), || {
                    for _ in 0..iters {
                        event!(Level::INFO, "event");
                    }
                });
            }));

            for handle in handles {
                let _ = handle.join();
            }

            start.elapsed()
        });
    });
}

criterion_group!(benches, synchronous_benchmark, non_blocking_benchmark);
criterion_main!(benches);
