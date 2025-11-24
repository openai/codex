use criterion::{black_box, criterion_group, criterion_main, Criterion};
use itertools::Itertools;

fn comb_replacement_n10_k5(c: &mut Criterion) {
    c.bench_function("comb replacement n10k5", move |b| {
        b.iter(|| {
            for i in (0..10).combinations_with_replacement(5) {
                black_box(i);
            }
        })
    });
}

fn comb_replacement_n5_k10(c: &mut Criterion) {
    c.bench_function("comb replacement n5 k10", move |b| {
        b.iter(|| {
            for i in (0..5).combinations_with_replacement(10) {
                black_box(i);
            }
        })
    });
}

fn comb_replacement_n10_k10(c: &mut Criterion) {
    c.bench_function("comb replacement n10 k10", move |b| {
        b.iter(|| {
            for i in (0..10).combinations_with_replacement(10) {
                black_box(i);
            }
        })
    });
}

criterion_group!(
    benches,
    comb_replacement_n10_k5,
    comb_replacement_n5_k10,
    comb_replacement_n10_k10,
);
criterion_main!(benches);
