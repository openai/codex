use criterion::{black_box, criterion_group, criterion_main, Criterion};

mod shared;

fn bench(c: &mut Criterion) {
    shared::for_all_dispatches(&mut c.benchmark_group("Dispatch::get_clone"), |b| {
        b.iter(|| {
            let current = tracing::dispatcher::get_default(|current| current.clone());
            black_box(current);
        })
    });
}

criterion_group!(benches, bench);
criterion_main!(benches);
