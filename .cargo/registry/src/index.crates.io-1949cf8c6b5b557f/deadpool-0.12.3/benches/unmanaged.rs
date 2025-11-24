use criterion::{criterion_group, criterion_main, Criterion};

use deadpool::unmanaged::Pool;

const ITERATIONS: usize = 1_000_000;

#[tokio::main]
async fn use_pool() {
    let pool = Pool::new(16);
    pool.add(()).await.unwrap();
    for _ in 0..ITERATIONS {
        let _ = pool.get().await.unwrap();
    }
}

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("use_pool", |b| b.iter(use_pool));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
