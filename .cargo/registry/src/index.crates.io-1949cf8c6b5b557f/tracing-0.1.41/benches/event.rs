use criterion::{criterion_group, criterion_main, Criterion};

mod shared;

fn bench(c: &mut Criterion) {
    shared::for_all_recording(&mut c.benchmark_group("event"), |b| {
        b.iter(|| tracing::info!("hello world!"))
    });
}

criterion_group!(benches, bench);
criterion_main!(benches);
