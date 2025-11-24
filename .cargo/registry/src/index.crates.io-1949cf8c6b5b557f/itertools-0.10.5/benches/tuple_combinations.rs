use criterion::{black_box, criterion_group, criterion_main, Criterion};
use itertools::Itertools;

// approximate 100_000 iterations for each combination
const N1: usize = 100_000;
const N2: usize = 448;
const N3: usize = 86;
const N4: usize = 41;

fn tuple_comb_for1(c: &mut Criterion) {
    c.bench_function("tuple comb for1", move |b| {
        b.iter(|| {
            for i in 0..N1 {
                black_box(i);
            }
        })
    });
}

fn tuple_comb_for2(c: &mut Criterion) {
    c.bench_function("tuple comb for2", move |b| {
        b.iter(|| {
            for i in 0..N2 {
                for j in (i + 1)..N2 {
                    black_box(i + j);
                }
            }
        })
    });
}

fn tuple_comb_for3(c: &mut Criterion) {
    c.bench_function("tuple comb for3", move |b| {
        b.iter(|| {
            for i in 0..N3 {
                for j in (i + 1)..N3 {
                    for k in (j + 1)..N3 {
                        black_box(i + j + k);
                    }
                }
            }
        })
    });
}

fn tuple_comb_for4(c: &mut Criterion) {
    c.bench_function("tuple comb for4", move |b| {
        b.iter(|| {
            for i in 0..N4 {
                for j in (i + 1)..N4 {
                    for k in (j + 1)..N4 {
                        for l in (k + 1)..N4 {
                            black_box(i + j + k + l);
                        }
                    }
                }
            }
        })
    });
}

fn tuple_comb_c1(c: &mut Criterion) {
    c.bench_function("tuple comb c1", move |b| {
        b.iter(|| {
            for (i,) in (0..N1).tuple_combinations() {
                black_box(i);
            }
        })
    });
}

fn tuple_comb_c2(c: &mut Criterion) {
    c.bench_function("tuple comb c2", move |b| {
        b.iter(|| {
            for (i, j) in (0..N2).tuple_combinations() {
                black_box(i + j);
            }
        })
    });
}

fn tuple_comb_c3(c: &mut Criterion) {
    c.bench_function("tuple comb c3", move |b| {
        b.iter(|| {
            for (i, j, k) in (0..N3).tuple_combinations() {
                black_box(i + j + k);
            }
        })
    });
}

fn tuple_comb_c4(c: &mut Criterion) {
    c.bench_function("tuple comb c4", move |b| {
        b.iter(|| {
            for (i, j, k, l) in (0..N4).tuple_combinations() {
                black_box(i + j + k + l);
            }
        })
    });
}

criterion_group!(
    benches,
    tuple_comb_for1,
    tuple_comb_for2,
    tuple_comb_for3,
    tuple_comb_for4,
    tuple_comb_c1,
    tuple_comb_c2,
    tuple_comb_c3,
    tuple_comb_c4,
);
criterion_main!(benches);
