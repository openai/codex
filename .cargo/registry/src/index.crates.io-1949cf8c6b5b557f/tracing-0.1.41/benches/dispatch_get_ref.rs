use criterion::{black_box, criterion_group, criterion_main, Criterion};

mod shared;

fn bench(c: &mut Criterion) {
    shared::for_all_dispatches(&mut c.benchmark_group("Dispatch::get_ref"), |b| {
        b.iter(|| {
            tracing::dispatcher::get_default(|current| {
                black_box(&current);
            })
        })
    });
}

criterion_group!(benches, bench);
criterion_main!(benches);
