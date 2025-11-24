use criterion::{black_box, BatchSize};

use thread_local::ThreadLocal;

fn main() {
    let mut c = criterion::Criterion::default().configure_from_args();

    c.bench_function("get", |b| {
        let local = ThreadLocal::new();
        local.get_or(|| Box::new(0));
        b.iter(|| {
            black_box(local.get());
        });
    });

    c.bench_function("insert", |b| {
        b.iter_batched_ref(
            ThreadLocal::new,
            |local| {
                black_box(local.get_or(|| 0));
            },
            BatchSize::SmallInput,
        )
    });
}
