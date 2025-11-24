use std::{convert::TryInto, fmt::Display};

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};

use deadpool::managed::Metrics;
use tokio::task::JoinHandle;

//const ITERATIONS: usize = 1_048_576;
const ITERATIONS: usize = 1 << 15;

#[derive(Copy, Clone, Debug)]
struct Config {
    pool_size: usize,
    workers: usize,
}

impl Display for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "w{}s{}", self.workers, self.pool_size)
    }
}

impl Config {
    fn operations_per_worker(&self) -> usize {
        ITERATIONS / self.workers
    }
}

#[rustfmt::skip]
const CONFIGS: &[Config] = &[
    // 8 workers
    Config { workers:  8, pool_size:  2 },
    Config { workers:  8, pool_size:  4 },
    Config { workers:  8, pool_size:  8 },
    // 16 workers
    Config { workers: 16, pool_size:  4 },
    Config { workers: 16, pool_size:  8 },
    Config { workers: 16, pool_size: 16 },
    // 32 workers
    Config { workers: 32, pool_size:  8 },
    Config { workers: 32, pool_size: 16 },
    Config { workers: 32, pool_size: 32 },
];

struct Manager {}

impl deadpool::managed::Manager for Manager {
    type Type = ();
    type Error = ();
    async fn create(&self) -> Result<Self::Type, Self::Error> {
        Ok(())
    }
    async fn recycle(
        &self,
        _: &mut Self::Type,
        _: &Metrics,
    ) -> deadpool::managed::RecycleResult<Self::Error> {
        Ok(())
    }
}

type Pool = deadpool::managed::Pool<Manager>;

async fn bench_get(cfg: Config) {
    let pool = Pool::builder(Manager {})
        .max_size(cfg.pool_size)
        .build()
        .unwrap();
    let join_handles: Vec<JoinHandle<()>> = (0..cfg.workers)
        .map(|_| {
            let pool = pool.clone();
            tokio::spawn(async move {
                for _ in 0..cfg.operations_per_worker() {
                    let _ = pool.get().await;
                }
            })
        })
        .collect();
    for join_handle in join_handles {
        join_handle.await.unwrap();
    }
}

fn criterion_benchmark(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let mut group = c.benchmark_group("managed");
    group.throughput(criterion::Throughput::Elements(
        ITERATIONS.try_into().expect("Can't convert u64 to usize"),
    ));
    for &config in CONFIGS {
        group.bench_with_input(BenchmarkId::new("get", config), &config, |b, &cfg| {
            b.to_async(&runtime).iter(|| bench_get(cfg))
        });
    }
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
