use criterion::{black_box, criterion_group, criterion_main, Criterion};
use itertools::Itertools;

// Keep aggregate generated elements the same, regardless of powerset length.
const TOTAL_ELEMENTS: usize = 1 << 12;
const fn calc_iters(n: usize) -> usize {
    TOTAL_ELEMENTS / (1 << n)
}

fn powerset_n(c: &mut Criterion, n: usize) {
    let id = format!("powerset {}", n);
    c.bench_function(id.as_str(), move |b| {
        b.iter(|| {
            for _ in 0..calc_iters(n) {
                for elt in (0..n).powerset() {
                    black_box(elt);
                }
            }
        })
    });
}

fn powerset_0(c: &mut Criterion) { powerset_n(c, 0); }

fn powerset_1(c: &mut Criterion) { powerset_n(c, 1); }

fn powerset_2(c: &mut Criterion) { powerset_n(c, 2); }

fn powerset_4(c: &mut Criterion) { powerset_n(c, 4); }

fn powerset_8(c: &mut Criterion) { powerset_n(c, 8); }

fn powerset_12(c: &mut Criterion) { powerset_n(c, 12); }

criterion_group!(
    benches,
    powerset_0,
    powerset_1,
    powerset_2,
    powerset_4,
    powerset_8,
    powerset_12,
);
criterion_main!(benches);