use criterion::{black_box, criterion_group, criterion_main, Criterion};

mod shared;

fn bench(c: &mut Criterion) {
    let mut group = c.benchmark_group("empty_span");
    shared::for_all_dispatches(&mut group, |b| {
        b.iter(|| {
            let span = tracing::span::Span::none();
            black_box(&span);
        })
    });
    group.bench_function("baseline_struct", |b| {
        b.iter(|| {
            let span = FakeEmptySpan::new();
            black_box(&span);
        })
    });
}

struct FakeEmptySpan {
    inner: Option<(usize, std::sync::Arc<()>)>,
    meta: Option<&'static ()>,
}

impl FakeEmptySpan {
    fn new() -> Self {
        Self {
            inner: None,
            meta: None,
        }
    }
}

impl Drop for FakeEmptySpan {
    fn drop(&mut self) {
        black_box(&self.inner);
        black_box(&self.meta);
    }
}

criterion_group!(benches, bench);
criterion_main!(benches);
