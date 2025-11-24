use criterion::{criterion_group, criterion_main, Criterion};
use log::trace;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing_subscriber::{EnvFilter, FmtSubscriber};

// This creates a bunch of threads and makes sure they start executing
// a given callback almost exactly at the same time.
fn run_on_many_threads<F, R>(thread_count: usize, callback: F) -> Vec<R>
where
    F: Fn() -> R + 'static + Send + Clone,
    R: Send + 'static,
{
    let started_count = Arc::new(AtomicUsize::new(0));
    let barrier = Arc::new(AtomicBool::new(false));
    #[allow(clippy::needless_collect)]
    let threads: Vec<_> = (0..thread_count)
        .map(|_| {
            let started_count = started_count.clone();
            let barrier = barrier.clone();
            let callback = callback.clone();

            std::thread::spawn(move || {
                started_count.fetch_add(1, Ordering::SeqCst);
                while !barrier.load(Ordering::SeqCst) {
                    std::thread::yield_now();
                }

                callback()
            })
        })
        .collect();

    while started_count.load(Ordering::SeqCst) != thread_count {
        std::thread::yield_now();
    }
    barrier.store(true, Ordering::SeqCst);

    threads
        .into_iter()
        .map(|handle| handle.join())
        .collect::<Result<Vec<R>, _>>()
        .unwrap()
}

fn bench_logger(c: &mut Criterion) {
    let env_filter = EnvFilter::default()
        .add_directive("info".parse().unwrap())
        .add_directive("ws=off".parse().unwrap())
        .add_directive("yamux=off".parse().unwrap())
        .add_directive("regalloc=off".parse().unwrap())
        .add_directive("cranelift_codegen=off".parse().unwrap())
        .add_directive("cranelift_wasm=warn".parse().unwrap())
        .add_directive("hyper=warn".parse().unwrap())
        .add_directive("dummy=trace".parse().unwrap());

    let builder = tracing_log::LogTracer::builder().with_max_level(log::LevelFilter::Trace);

    #[cfg(feature = "interest-cache")]
    let builder = builder.with_interest_cache(tracing_log::InterestCacheConfig::default());

    builder.init().unwrap();

    let builder = FmtSubscriber::builder()
        .with_env_filter(env_filter)
        .with_filter_reloading();

    let subscriber = builder.finish();
    tracing::subscriber::set_global_default(subscriber).unwrap();

    const THREAD_COUNT: usize = 8;

    c.bench_function("log_from_multiple_threads", |b| {
        b.iter_custom(|count| {
            let durations = run_on_many_threads(THREAD_COUNT, move || {
                let start = Instant::now();
                for _ in 0..count {
                    trace!("A dummy log!");
                }
                start.elapsed()
            });

            let total_time: Duration = durations.into_iter().sum();
            Duration::from_nanos((total_time.as_nanos() / THREAD_COUNT as u128) as u64)
        })
    });
}

criterion_group!(benches, bench_logger);
criterion_main!(benches);
