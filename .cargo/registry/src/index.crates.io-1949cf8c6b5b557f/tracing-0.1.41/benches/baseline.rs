use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench(c: &mut Criterion) {
    use std::sync::atomic::{AtomicUsize, Ordering};

    let mut group = c.benchmark_group("comparison");
    group.bench_function("relaxed_load", |b| {
        let foo = AtomicUsize::new(1);
        b.iter(|| black_box(foo.load(Ordering::Relaxed)));
    });
    group.bench_function("acquire_load", |b| {
        let foo = AtomicUsize::new(1);
        b.iter(|| black_box(foo.load(Ordering::Acquire)))
    });
    group.bench_function("log", |b| {
        b.iter(|| {
            log::log!(log::Level::Info, "log");
        })
    });
    group.finish();
}

criterion_group!(benches, bench);
criterion_main!(benches);
