use criterion::{black_box, criterion_group, criterion_main, Criterion};
use tracing::{span, Level};

mod shared;

fn bench(c: &mut Criterion) {
    shared::for_all_recording(&mut c.benchmark_group("span_repeated"), |b| {
        let n = black_box(N_SPANS);
        b.iter(|| (0..n).fold(mk_span(0), |_, i| mk_span(i as u64)))
    });
}

#[inline]
fn mk_span(i: u64) -> tracing::Span {
    span!(Level::TRACE, "span", i = i)
}

const N_SPANS: usize = 100;
criterion_group!(benches, bench);
criterion_main!(benches);
