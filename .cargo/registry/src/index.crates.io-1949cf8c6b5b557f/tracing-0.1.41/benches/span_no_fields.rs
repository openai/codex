use criterion::{criterion_group, criterion_main, Criterion};
use tracing::{span, Level};

mod shared;

fn bench(c: &mut Criterion) {
    shared::for_all_recording(&mut c.benchmark_group("span_no_fields"), |b| {
        b.iter(|| span!(Level::TRACE, "span"))
    });
}

criterion_group!(benches, bench);
criterion_main!(benches);
