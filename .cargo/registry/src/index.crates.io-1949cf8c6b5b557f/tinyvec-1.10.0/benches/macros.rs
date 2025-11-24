use criterion::{criterion_group, criterion_main, Criterion};

use tinyvec::tiny_vec;

fn bench_tinyvec_macro(c: &mut Criterion) {
  let mut g = c.benchmark_group("tinyvec_macro");

  g.bench_function("0 of 32", |b| {
    b.iter(|| tiny_vec!([u8; 32]));
  });

  g.bench_function("16 of 32", |b| {
    b.iter(|| {
      tiny_vec!([u8; 32]=>
        1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16,
      )
    });
  });

  g.bench_function("32 of 32", |b| {
    b.iter(|| {
      tiny_vec!([u8; 32]=>
        1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16,
        17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32,
      )
    });
  });

  g.bench_function("33 of 32", |b| {
    b.iter(|| {
      tiny_vec!([u8; 32]=>
        1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16,
        17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32,
        33,
      )
    });
  });

  g.bench_function("64 of 32", |b| {
    b.iter(|| {
      tiny_vec!([u8; 32]=>
        1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16,
        17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32,
        33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48,
        49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64,
      )
    });
  });
}

criterion_group!(benches, bench_tinyvec_macro);
criterion_main!(benches);
