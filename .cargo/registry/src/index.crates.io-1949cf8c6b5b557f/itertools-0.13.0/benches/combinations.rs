use criterion::{black_box, criterion_group, criterion_main, Criterion};
use itertools::Itertools;

// approximate 100_000 iterations for each combination
const N1: usize = 100_000;
const N2: usize = 448;
const N3: usize = 86;
const N4: usize = 41;
const N14: usize = 21;

fn comb_for1(c: &mut Criterion) {
    c.bench_function("comb for1", move |b| {
        b.iter(|| {
            for i in 0..N1 {
                black_box(vec![i]);
            }
        })
    });
}

fn comb_for2(c: &mut Criterion) {
    c.bench_function("comb for2", move |b| {
        b.iter(|| {
            for i in 0..N2 {
                for j in (i + 1)..N2 {
                    black_box(vec![i, j]);
                }
            }
        })
    });
}

fn comb_for3(c: &mut Criterion) {
    c.bench_function("comb for3", move |b| {
        b.iter(|| {
            for i in 0..N3 {
                for j in (i + 1)..N3 {
                    for k in (j + 1)..N3 {
                        black_box(vec![i, j, k]);
                    }
                }
            }
        })
    });
}

fn comb_for4(c: &mut Criterion) {
    c.bench_function("comb for4", move |b| {
        b.iter(|| {
            for i in 0..N4 {
                for j in (i + 1)..N4 {
                    for k in (j + 1)..N4 {
                        for l in (k + 1)..N4 {
                            black_box(vec![i, j, k, l]);
                        }
                    }
                }
            }
        })
    });
}

fn comb_c1(c: &mut Criterion) {
    c.bench_function("comb c1", move |b| {
        b.iter(|| {
            for combo in (0..N1).combinations(1) {
                black_box(combo);
            }
        })
    });
}

fn comb_c2(c: &mut Criterion) {
    c.bench_function("comb c2", move |b| {
        b.iter(|| {
            for combo in (0..N2).combinations(2) {
                black_box(combo);
            }
        })
    });
}

fn comb_c3(c: &mut Criterion) {
    c.bench_function("comb c3", move |b| {
        b.iter(|| {
            for combo in (0..N3).combinations(3) {
                black_box(combo);
            }
        })
    });
}

fn comb_c4(c: &mut Criterion) {
    c.bench_function("comb c4", move |b| {
        b.iter(|| {
            for combo in (0..N4).combinations(4) {
                black_box(combo);
            }
        })
    });
}

fn comb_c14(c: &mut Criterion) {
    c.bench_function("comb c14", move |b| {
        b.iter(|| {
            for combo in (0..N14).combinations(14) {
                black_box(combo);
            }
        })
    });
}

criterion_group!(
    benches, comb_for1, comb_for2, comb_for3, comb_for4, comb_c1, comb_c2, comb_c3, comb_c4,
    comb_c14,
);
criterion_main!(benches);
