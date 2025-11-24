//! Benchmarks that compare TinyVec to SmallVec
//!
//! All the following commentary is based on the latest nightly at the time:
//! rustc 1.55.0 (c8dfcfe04 2021-09-06).
//!
//! Some of these benchmarks are just a few instructions, so we put our own for
//! loop inside the criterion::Bencher::iter call. This seems to improve the
//! stability of measurements, and it has the wonderful side effect of making
//! the emitted assembly easier to follow. Some of these benchmarks are totally
//! inlined so that there are no calls at all in the hot path, so finding
//! this for loop is an easy way to find your way around the emitted assembly.
//!
//! The clear method is cheaper to call for arrays of elements without a Drop
//! impl, so wherever possible we reuse a single object in the benchmark loop,
//! with a clear + black_box on each iteration in an attempt to not make that
//! visible to the optimizer.
//!
//! We always call black_box(&v), instead of v = black_box(v) because the latter
//! does a move of the inline array, which is linear in the size of the array
//! and thus varies based on the array type being benchmarked, and this move can
//! be more expensive than the function we're trying to benchmark.
//!
//! We also black_box the input to each method call. This has a significant
//! effect on the assembly emitted, for example if we do not black_box the range
//! we iterate over in the ::push benchmarks, the loop is unrolled. It's not
//! entirely clear if it's better to black_box the iterator that yields the
//! items being pushed, or to black_box at a deeper level: v.push(black_box(i))
//! for example. Anecdotally, it seems like the latter approach produces
//! unreasonably bad assembly.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use smallvec::SmallVec;
use std::iter::FromIterator;
use tinyvec::TinyVec;

const ITERS: usize = 10_000;

macro_rules! tinyvec_benches {
  ($c:expr, $type:ty ; $len:expr) => {{
    let mut g = $c.benchmark_group(concat!(
      "TinyVec_",
      stringify!($type),
      "_",
      stringify!($len)
    ));

    g.bench_function(
      concat!(
        "TinyVec<[",
        stringify!($type),
        "; ",
        stringify!($len),
        "]>::default"
      ),
      |b| {
        b.iter(|| {
          for _ in 0..ITERS {
            let v: TinyVec<[$type; $len]> = TinyVec::default();
            black_box(&v);
          }
        });
      },
    );

    g.bench_function(
      concat!(
        "TinyVec<[",
        stringify!($type),
        "; ",
        stringify!($len),
        "]>::clone"
      ),
      |b| {
        b.iter(|| {
          let outer: TinyVec<[$type; $len]> =
            black_box(TinyVec::from_iter(0..=($len as usize - 1) as _));
          for _ in 0..ITERS {
            let v = outer.clone();
            black_box(&v);
          }
        });
      },
    );

    g.bench_function(
      concat!(
        "TinyVec<[",
        stringify!($type),
        "; ",
        stringify!($len),
        "]>::clear"
      ),
      |b| {
        b.iter(|| {
          let mut v: TinyVec<[$type; $len]> = TinyVec::default();
          for _ in 0..ITERS {
            v.clear();
            black_box(&v);
          }
        });
      },
    );

    g.bench_function(
      concat!(
        "TinyVec<[",
        stringify!($type),
        "; ",
        stringify!($len),
        "]>::push"
      ),
      |b| {
        b.iter(|| {
          let mut v: TinyVec<[$type; $len]> = TinyVec::default();
          for _ in 0..ITERS {
            v.clear();
            black_box(&v);
            for i in black_box(0..=($len as usize - 1) as _) {
              v.push(i);
            }
            black_box(&v);
          }
        });
      },
    );

    g.bench_function(
      concat!(
        "TinyVec<[",
        stringify!($type),
        "; ",
        stringify!($len),
        "]>::from_iter"
      ),
      |b| {
        b.iter(|| {
          for _ in 0..ITERS {
            let v: TinyVec<[$type; $len]> =
              TinyVec::from_iter(black_box(0..=($len as usize - 1) as _));
            black_box(&v);
          }
        });
      },
    );

    g.bench_function(
      concat!(
        "TinyVec<[",
        stringify!($type),
        "; ",
        stringify!($len),
        "]>::from_slice"
      ),
      |b| {
        b.iter(|| {
          let data: &[$type] = &[0, 1, 2, 3, 4, 5, 6, 7];
          for _ in 0..ITERS {
            let v: TinyVec<[$type; $len]> = TinyVec::from(black_box(data));
            black_box(&v);
          }
        });
      },
    );

    g.bench_function(
      concat!(
        "TinyVec<[",
        stringify!($type),
        "; ",
        stringify!($len),
        "]>::extend"
      ),
      |b| {
        b.iter(|| {
          let mut v: TinyVec<[$type; $len]> = black_box(TinyVec::default());
          for _ in 0..ITERS {
            v.clear();
            black_box(&v);
            v.extend(black_box(0..=($len as usize - 1) as _));
            black_box(&v);
          }
        });
      },
    );

    g.bench_function(
      concat!(
        "TinyVec<[",
        stringify!($type),
        "; ",
        stringify!($len),
        "]>::extend_from_slice"
      ),
      |b| {
        b.iter(|| {
          let data: &[$type] = black_box(&[0, 1, 2, 3, 4, 5, 6, 7]);
          let mut v: TinyVec<[$type; $len]> = black_box(TinyVec::default());
          for _ in 0..ITERS {
            v.clear();
            black_box(&v);
            v.extend_from_slice(data);
            black_box(&v);
          }
        });
      },
    );

    g.bench_function(
      concat!(
        "TinyVec<[",
        stringify!($type),
        "; ",
        stringify!($len),
        "]>::insert"
      ),
      |b| {
        b.iter(|| {
          let mut v: TinyVec<[$type; $len]> = TinyVec::default();
          for _ in 0..ITERS {
            v.clear();
            black_box(&v);
            for i in black_box(0..=($len as usize - 1) as _) {
              v.insert(i as usize, i);
            }
            black_box(&v);
          }
        });
      },
    );

    g.bench_function(
      concat!(
        "TinyVec<[",
        stringify!($type),
        "; ",
        stringify!($len),
        "]>::remove"
      ),
      |b| {
        b.iter(|| {
          let outer: TinyVec<[$type; $len]> =
            black_box(TinyVec::from_iter(0..=($len as usize - 1) as _));
          for _ in 0..ITERS {
            let mut v = outer.clone();
            for i in black_box((0..=($len as usize - 1) as _).rev()) {
              v.remove(i);
            }
            black_box(&v);
          }
        });
      },
    );
  }};
}

fn tinyvec_benches(c: &mut Criterion) {
  tinyvec_benches!(c, u8; 8);
  tinyvec_benches!(c, u8; 16);
  tinyvec_benches!(c, u8; 32);
  tinyvec_benches!(c, u8; 64);
  tinyvec_benches!(c, u8; 128);
  tinyvec_benches!(c, u8; 256);
  tinyvec_benches!(c, u64; 2);
  tinyvec_benches!(c, u64; 4);
  tinyvec_benches!(c, u64; 8);
  tinyvec_benches!(c, u64; 16);
  tinyvec_benches!(c, u64; 32);
}

macro_rules! smallvec_benches {
  ($c:expr, $type:ty ; $len:expr) => {{
    let mut g = $c.benchmark_group(concat!(
      "SmallVec_",
      stringify!($type),
      "_",
      stringify!($len)
    ));

    g.bench_function(
      concat!(
        "SmallVec<[",
        stringify!($type),
        "; ",
        stringify!($len),
        "]>::default"
      ),
      |b| {
        b.iter(|| {
          for _ in 0..ITERS {
            let v: SmallVec<[$type; $len]> = SmallVec::default();
            black_box(&v);
          }
        });
      },
    );

    g.bench_function(
      concat!(
        "SmallVec<[",
        stringify!($type),
        "; ",
        stringify!($len),
        "]>::clone"
      ),
      |b| {
        b.iter(|| {
          let outer: SmallVec<[$type; $len]> =
            black_box(SmallVec::from_iter(0..=($len as usize - 1) as _));
          for _ in 0..ITERS {
            let v = outer.clone();
            black_box(&v);
          }
        });
      },
    );

    g.bench_function(
      concat!(
        "SmallVec<[",
        stringify!($type),
        "; ",
        stringify!($len),
        "]>::clear"
      ),
      |b| {
        b.iter(|| {
          let mut v: SmallVec<[$type; $len]> = SmallVec::default();
          for _ in 0..ITERS {
            v.clear();
            black_box(&v);
          }
        });
      },
    );

    g.bench_function(
      concat!(
        "SmallVec<[",
        stringify!($type),
        "; ",
        stringify!($len),
        "]>::push"
      ),
      |b| {
        b.iter(|| {
          let mut v: SmallVec<[$type; $len]> = SmallVec::default();
          for _ in 0..ITERS {
            v.clear();
            black_box(&v);
            for i in black_box(0..=($len as usize - 1) as _) {
              v.push(i);
            }
            black_box(&v);
          }
        });
      },
    );

    g.bench_function(
      concat!(
        "SmallVec<[",
        stringify!($type),
        "; ",
        stringify!($len),
        "]>::from_iter"
      ),
      |b| {
        b.iter(|| {
          for _ in 0..ITERS {
            let v: SmallVec<[$type; $len]> =
              SmallVec::from_iter(black_box(0..=($len as usize - 1) as _));
            black_box(&v);
          }
        });
      },
    );

    g.bench_function(
      concat!(
        "SmallVec<[",
        stringify!($type),
        "; ",
        stringify!($len),
        "]>::from_slice"
      ),
      |b| {
        b.iter(|| {
          let data: &[$type] = &[0, 1, 2, 3, 4, 5, 6, 7];
          for _ in 0..ITERS {
            let v: SmallVec<[$type; $len]> = SmallVec::from(black_box(data));
            black_box(&v);
          }
        });
      },
    );

    g.bench_function(
      concat!(
        "SmallVec<[",
        stringify!($type),
        "; ",
        stringify!($len),
        "]>::extend"
      ),
      |b| {
        b.iter(|| {
          let mut v: SmallVec<[$type; $len]> = black_box(SmallVec::default());
          for _ in 0..ITERS {
            v.clear();
            black_box(&v);
            v.extend(black_box(0..=($len as usize - 1) as _));
            black_box(&v);
          }
        });
      },
    );

    g.bench_function(
      concat!(
        "SmallVec<[",
        stringify!($type),
        "; ",
        stringify!($len),
        "]>::extend_from_slice"
      ),
      |b| {
        b.iter(|| {
          let data: &[$type] = black_box(&[0, 1, 2, 3, 4, 5, 6, 7]);
          let mut v: SmallVec<[$type; $len]> = black_box(SmallVec::default());
          for _ in 0..ITERS {
            v.clear();
            black_box(&v);
            v.extend_from_slice(data);
            black_box(&v);
          }
        });
      },
    );

    g.bench_function(
      concat!(
        "SmallVec<[",
        stringify!($type),
        "; ",
        stringify!($len),
        "]>::insert"
      ),
      |b| {
        b.iter(|| {
          let mut v: SmallVec<[$type; $len]> = SmallVec::default();
          for _ in 0..ITERS {
            v.clear();
            black_box(&v);
            for i in black_box(0..=($len as usize - 1) as _) {
              v.insert(i as usize, i);
            }
            black_box(&v);
          }
        });
      },
    );

    g.bench_function(
      concat!(
        "SmallVec<[",
        stringify!($type),
        "; ",
        stringify!($len),
        "]>::remove"
      ),
      |b| {
        b.iter(|| {
          let outer: SmallVec<[$type; $len]> =
            black_box(SmallVec::from_iter(0..=($len as usize - 1) as _));
          for _ in 0..ITERS {
            let mut v = outer.clone();
            for i in black_box((0..=($len as usize - 1) as _).rev()) {
              v.remove(i);
            }
            black_box(&v);
          }
        });
      },
    );
  }};
}

fn smallvec_benches(c: &mut Criterion) {
  smallvec_benches!(c, u8; 8);
  smallvec_benches!(c, u8; 16);
  smallvec_benches!(c, u8; 32);
  smallvec_benches!(c, u8; 64);
  smallvec_benches!(c, u8; 128);
  smallvec_benches!(c, u8; 256);
  smallvec_benches!(c, u64; 2);
  smallvec_benches!(c, u64; 4);
  smallvec_benches!(c, u64; 8);
  smallvec_benches!(c, u64; 16);
  smallvec_benches!(c, u64; 32);
}

criterion_group!(benches, tinyvec_benches, smallvec_benches);
criterion_main!(benches);
