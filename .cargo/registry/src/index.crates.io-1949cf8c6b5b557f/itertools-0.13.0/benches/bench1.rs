use criterion::{black_box, criterion_group, criterion_main, BatchSize, Criterion};
use itertools::free::cloned;
use itertools::iproduct;
use itertools::Itertools;

use std::cmp;
use std::iter::repeat;
use std::ops::{Add, Range};

fn slice_iter(c: &mut Criterion) {
    let xs: Vec<_> = repeat(1i32).take(20).collect();

    c.bench_function("slice iter", move |b| {
        b.iter(|| {
            for elt in xs.iter() {
                black_box(elt);
            }
        })
    });
}

fn slice_iter_rev(c: &mut Criterion) {
    let xs: Vec<_> = repeat(1i32).take(20).collect();

    c.bench_function("slice iter rev", move |b| {
        b.iter(|| {
            for elt in xs.iter().rev() {
                black_box(elt);
            }
        })
    });
}

fn zip_default_zip(c: &mut Criterion) {
    let xs = vec![0; 1024];
    let ys = vec![0; 768];
    let xs = black_box(xs);
    let ys = black_box(ys);

    c.bench_function("zip default zip", move |b| {
        b.iter(|| {
            for (&x, &y) in xs.iter().zip(&ys) {
                black_box(x);
                black_box(y);
            }
        })
    });
}

fn zipdot_i32_default_zip(c: &mut Criterion) {
    let xs = vec![2; 1024];
    let ys = vec![2; 768];
    let xs = black_box(xs);
    let ys = black_box(ys);

    c.bench_function("zipdot i32 default zip", move |b| {
        b.iter(|| {
            let mut s = 0;
            for (&x, &y) in xs.iter().zip(&ys) {
                s += x * y;
            }
            s
        })
    });
}

fn zipdot_f32_default_zip(c: &mut Criterion) {
    let xs = vec![2f32; 1024];
    let ys = vec![2f32; 768];
    let xs = black_box(xs);
    let ys = black_box(ys);

    c.bench_function("zipdot f32 default zip", move |b| {
        b.iter(|| {
            let mut s = 0.;
            for (&x, &y) in xs.iter().zip(&ys) {
                s += x * y;
            }
            s
        })
    });
}

fn zip_default_zip3(c: &mut Criterion) {
    let xs = vec![0; 1024];
    let ys = vec![0; 768];
    let zs = vec![0; 766];
    let xs = black_box(xs);
    let ys = black_box(ys);
    let zs = black_box(zs);

    c.bench_function("zip default zip3", move |b| {
        b.iter(|| {
            for ((&x, &y), &z) in xs.iter().zip(&ys).zip(&zs) {
                black_box(x);
                black_box(y);
                black_box(z);
            }
        })
    });
}

fn zip_slices_ziptuple(c: &mut Criterion) {
    let xs = vec![0; 1024];
    let ys = vec![0; 768];

    c.bench_function("zip slices ziptuple", move |b| {
        b.iter(|| {
            let xs = black_box(&xs);
            let ys = black_box(&ys);
            for (&x, &y) in itertools::multizip((xs, ys)) {
                black_box(x);
                black_box(y);
            }
        })
    });
}

fn zip_checked_counted_loop(c: &mut Criterion) {
    let xs = vec![0; 1024];
    let ys = vec![0; 768];
    let xs = black_box(xs);
    let ys = black_box(ys);

    c.bench_function("zip checked counted loop", move |b| {
        b.iter(|| {
            // Must slice to equal lengths, and then bounds checks are eliminated!
            let len = cmp::min(xs.len(), ys.len());
            let xs = &xs[..len];
            let ys = &ys[..len];

            for i in 0..len {
                let x = xs[i];
                let y = ys[i];
                black_box(x);
                black_box(y);
            }
        })
    });
}

fn zipdot_i32_checked_counted_loop(c: &mut Criterion) {
    let xs = vec![2; 1024];
    let ys = vec![2; 768];
    let xs = black_box(xs);
    let ys = black_box(ys);

    c.bench_function("zipdot i32 checked counted loop", move |b| {
        b.iter(|| {
            // Must slice to equal lengths, and then bounds checks are eliminated!
            let len = cmp::min(xs.len(), ys.len());
            let xs = &xs[..len];
            let ys = &ys[..len];

            let mut s = 0i32;

            for i in 0..len {
                s += xs[i] * ys[i];
            }
            s
        })
    });
}

fn zipdot_f32_checked_counted_loop(c: &mut Criterion) {
    let xs = vec![2f32; 1024];
    let ys = vec![2f32; 768];
    let xs = black_box(xs);
    let ys = black_box(ys);

    c.bench_function("zipdot f32 checked counted loop", move |b| {
        b.iter(|| {
            // Must slice to equal lengths, and then bounds checks are eliminated!
            let len = cmp::min(xs.len(), ys.len());
            let xs = &xs[..len];
            let ys = &ys[..len];

            let mut s = 0.;

            for i in 0..len {
                s += xs[i] * ys[i];
            }
            s
        })
    });
}

fn zipdot_f32_checked_counted_unrolled_loop(c: &mut Criterion) {
    let xs = vec![2f32; 1024];
    let ys = vec![2f32; 768];
    let xs = black_box(xs);
    let ys = black_box(ys);

    c.bench_function("zipdot f32 checked counted unrolled loop", move |b| {
        b.iter(|| {
            // Must slice to equal lengths, and then bounds checks are eliminated!
            let len = cmp::min(xs.len(), ys.len());
            let mut xs = &xs[..len];
            let mut ys = &ys[..len];

            let mut s = 0.;
            let (mut p0, mut p1, mut p2, mut p3, mut p4, mut p5, mut p6, mut p7) =
                (0., 0., 0., 0., 0., 0., 0., 0.);

            // how to unroll and have bounds checks eliminated (by cristicbz)
            // split sum into eight parts to enable vectorization (by bluss)
            while xs.len() >= 8 {
                p0 += xs[0] * ys[0];
                p1 += xs[1] * ys[1];
                p2 += xs[2] * ys[2];
                p3 += xs[3] * ys[3];
                p4 += xs[4] * ys[4];
                p5 += xs[5] * ys[5];
                p6 += xs[6] * ys[6];
                p7 += xs[7] * ys[7];

                xs = &xs[8..];
                ys = &ys[8..];
            }
            s += p0 + p4;
            s += p1 + p5;
            s += p2 + p6;
            s += p3 + p7;

            for i in 0..xs.len() {
                s += xs[i] * ys[i];
            }
            s
        })
    });
}

fn zip_unchecked_counted_loop(c: &mut Criterion) {
    let xs = vec![0; 1024];
    let ys = vec![0; 768];
    let xs = black_box(xs);
    let ys = black_box(ys);

    c.bench_function("zip unchecked counted loop", move |b| {
        b.iter(|| {
            let len = cmp::min(xs.len(), ys.len());
            for i in 0..len {
                unsafe {
                    let x = *xs.get_unchecked(i);
                    let y = *ys.get_unchecked(i);
                    black_box(x);
                    black_box(y);
                }
            }
        })
    });
}

fn zipdot_i32_unchecked_counted_loop(c: &mut Criterion) {
    let xs = vec![2; 1024];
    let ys = vec![2; 768];
    let xs = black_box(xs);
    let ys = black_box(ys);

    c.bench_function("zipdot i32 unchecked counted loop", move |b| {
        b.iter(|| {
            let len = cmp::min(xs.len(), ys.len());
            let mut s = 0i32;
            for i in 0..len {
                unsafe {
                    let x = *xs.get_unchecked(i);
                    let y = *ys.get_unchecked(i);
                    s += x * y;
                }
            }
            s
        })
    });
}

fn zipdot_f32_unchecked_counted_loop(c: &mut Criterion) {
    let xs = vec![2.; 1024];
    let ys = vec![2.; 768];
    let xs = black_box(xs);
    let ys = black_box(ys);

    c.bench_function("zipdot f32 unchecked counted loop", move |b| {
        b.iter(|| {
            let len = cmp::min(xs.len(), ys.len());
            let mut s = 0f32;
            for i in 0..len {
                unsafe {
                    let x = *xs.get_unchecked(i);
                    let y = *ys.get_unchecked(i);
                    s += x * y;
                }
            }
            s
        })
    });
}

fn zip_unchecked_counted_loop3(c: &mut Criterion) {
    let xs = vec![0; 1024];
    let ys = vec![0; 768];
    let zs = vec![0; 766];
    let xs = black_box(xs);
    let ys = black_box(ys);
    let zs = black_box(zs);

    c.bench_function("zip unchecked counted loop3", move |b| {
        b.iter(|| {
            let len = cmp::min(xs.len(), cmp::min(ys.len(), zs.len()));
            for i in 0..len {
                unsafe {
                    let x = *xs.get_unchecked(i);
                    let y = *ys.get_unchecked(i);
                    let z = *zs.get_unchecked(i);
                    black_box(x);
                    black_box(y);
                    black_box(z);
                }
            }
        })
    });
}

fn chunk_by_lazy_1(c: &mut Criterion) {
    let mut data = vec![0; 1024];
    for (index, elt) in data.iter_mut().enumerate() {
        *elt = index / 10;
    }

    let data = black_box(data);

    c.bench_function("chunk by lazy 1", move |b| {
        b.iter(|| {
            for (_key, chunk) in &data.iter().chunk_by(|elt| **elt) {
                for elt in chunk {
                    black_box(elt);
                }
            }
        })
    });
}

fn chunk_by_lazy_2(c: &mut Criterion) {
    let mut data = vec![0; 1024];
    for (index, elt) in data.iter_mut().enumerate() {
        *elt = index / 2;
    }

    let data = black_box(data);

    c.bench_function("chunk by lazy 2", move |b| {
        b.iter(|| {
            for (_key, chunk) in &data.iter().chunk_by(|elt| **elt) {
                for elt in chunk {
                    black_box(elt);
                }
            }
        })
    });
}

fn slice_chunks(c: &mut Criterion) {
    let data = vec![0; 1024];

    let data = black_box(data);
    let sz = black_box(10);

    c.bench_function("slice chunks", move |b| {
        b.iter(|| {
            for chunk in data.chunks(sz) {
                for elt in chunk {
                    black_box(elt);
                }
            }
        })
    });
}

fn chunks_lazy_1(c: &mut Criterion) {
    let data = vec![0; 1024];

    let data = black_box(data);
    let sz = black_box(10);

    c.bench_function("chunks lazy 1", move |b| {
        b.iter(|| {
            for chunk in &data.iter().chunks(sz) {
                for elt in chunk {
                    black_box(elt);
                }
            }
        })
    });
}

fn equal(c: &mut Criterion) {
    let data = vec![7; 1024];
    let l = data.len();
    let alpha = black_box(&data[1..]);
    let beta = black_box(&data[..l - 1]);

    c.bench_function("equal", move |b| b.iter(|| itertools::equal(alpha, beta)));
}

fn merge_default(c: &mut Criterion) {
    let mut data1 = vec![0; 1024];
    let mut data2 = vec![0; 800];
    let mut x = 0;

    #[allow(clippy::explicit_counter_loop, clippy::unused_enumerate_index)]
    for (_, elt) in data1.iter_mut().enumerate() {
        *elt = x;
        x += 1;
    }

    let mut y = 0;
    for (i, elt) in data2.iter_mut().enumerate() {
        *elt += y;
        if i % 3 == 0 {
            y += 3;
        } else {
            y += 0;
        }
    }
    let data1 = black_box(data1);
    let data2 = black_box(data2);

    c.bench_function("merge default", move |b| {
        b.iter(|| data1.iter().merge(&data2).count())
    });
}

fn merge_by_cmp(c: &mut Criterion) {
    let mut data1 = vec![0; 1024];
    let mut data2 = vec![0; 800];
    let mut x = 0;

    #[allow(clippy::explicit_counter_loop, clippy::unused_enumerate_index)]
    for (_, elt) in data1.iter_mut().enumerate() {
        *elt = x;
        x += 1;
    }

    let mut y = 0;
    for (i, elt) in data2.iter_mut().enumerate() {
        *elt += y;
        if i % 3 == 0 {
            y += 3;
        } else {
            y += 0;
        }
    }
    let data1 = black_box(data1);
    let data2 = black_box(data2);

    c.bench_function("merge by cmp", move |b| {
        b.iter(|| data1.iter().merge_by(&data2, PartialOrd::le).count())
    });
}

fn merge_by_lt(c: &mut Criterion) {
    let mut data1 = vec![0; 1024];
    let mut data2 = vec![0; 800];
    let mut x = 0;

    #[allow(clippy::explicit_counter_loop, clippy::unused_enumerate_index)]
    for (_, elt) in data1.iter_mut().enumerate() {
        *elt = x;
        x += 1;
    }

    let mut y = 0;
    for (i, elt) in data2.iter_mut().enumerate() {
        *elt += y;
        if i % 3 == 0 {
            y += 3;
        } else {
            y += 0;
        }
    }
    let data1 = black_box(data1);
    let data2 = black_box(data2);

    c.bench_function("merge by lt", move |b| {
        b.iter(|| data1.iter().merge_by(&data2, |a, b| a <= b).count())
    });
}

fn kmerge_default(c: &mut Criterion) {
    let mut data1 = vec![0; 1024];
    let mut data2 = vec![0; 800];
    let mut x = 0;

    #[allow(clippy::explicit_counter_loop, clippy::unused_enumerate_index)]
    for (_, elt) in data1.iter_mut().enumerate() {
        *elt = x;
        x += 1;
    }

    let mut y = 0;
    for (i, elt) in data2.iter_mut().enumerate() {
        *elt += y;
        if i % 3 == 0 {
            y += 3;
        } else {
            y += 0;
        }
    }
    let data1 = black_box(data1);
    let data2 = black_box(data2);
    let its = &[data1.iter(), data2.iter()];

    c.bench_function("kmerge default", move |b| {
        b.iter(|| its.iter().cloned().kmerge().count())
    });
}

fn kmerge_tenway(c: &mut Criterion) {
    let mut data = vec![0; 10240];

    let mut state = 1729u16;
    fn rng(state: &mut u16) -> u16 {
        let new = state.wrapping_mul(31421).wrapping_add(6927);
        *state = new;
        new
    }

    for elt in &mut data {
        *elt = rng(&mut state);
    }

    let mut chunks = Vec::new();
    let mut rest = &mut data[..];
    while !rest.is_empty() {
        let chunk_len = 1 + rng(&mut state) % 512;
        let chunk_len = cmp::min(rest.len(), chunk_len as usize);
        let (fst, tail) = { rest }.split_at_mut(chunk_len);
        fst.sort();
        chunks.push(fst.iter().cloned());
        rest = tail;
    }

    // println!("Chunk lengths: {}", chunks.iter().format_with(", ", |elt, f| f(&elt.len())));

    c.bench_function("kmerge tenway", move |b| {
        b.iter(|| chunks.iter().cloned().kmerge().count())
    });
}

fn fast_integer_sum<I>(iter: I) -> I::Item
where
    I: IntoIterator,
    I::Item: Default + Add<Output = I::Item>,
{
    iter.into_iter().fold(<_>::default(), |x, y| x + y)
}

fn step_vec_2(c: &mut Criterion) {
    let v = vec![0; 1024];

    c.bench_function("step vec 2", move |b| {
        b.iter(|| fast_integer_sum(cloned(v.iter().step_by(2))))
    });
}

fn step_vec_10(c: &mut Criterion) {
    let v = vec![0; 1024];

    c.bench_function("step vec 10", move |b| {
        b.iter(|| fast_integer_sum(cloned(v.iter().step_by(10))))
    });
}

fn step_range_2(c: &mut Criterion) {
    let v = black_box(0..1024);

    c.bench_function("step range 2", move |b| {
        b.iter(|| fast_integer_sum(v.clone().step_by(2)))
    });
}

fn step_range_10(c: &mut Criterion) {
    let v = black_box(0..1024);

    c.bench_function("step range 10", move |b| {
        b.iter(|| fast_integer_sum(v.clone().step_by(10)))
    });
}

fn vec_iter_mut_partition(c: &mut Criterion) {
    let data = std::iter::repeat(-1024i32..1024)
        .take(256)
        .flatten()
        .collect_vec();
    c.bench_function("vec iter mut partition", move |b| {
        b.iter_batched(
            || data.clone(),
            |mut data| {
                black_box(itertools::partition(black_box(&mut data), |n| *n >= 0));
            },
            BatchSize::LargeInput,
        )
    });
}

fn cartesian_product_iterator(c: &mut Criterion) {
    let xs = vec![0; 16];

    c.bench_function("cartesian product iterator", move |b| {
        b.iter(|| {
            let mut sum = 0;
            for (&x, &y, &z) in iproduct!(&xs, &xs, &xs) {
                sum += x;
                sum += y;
                sum += z;
            }
            sum
        })
    });
}

fn multi_cartesian_product_iterator(c: &mut Criterion) {
    let xs = [vec![0; 16], vec![0; 16], vec![0; 16]];

    c.bench_function("multi cartesian product iterator", move |b| {
        b.iter(|| {
            let mut sum = 0;
            for x in xs.iter().multi_cartesian_product() {
                sum += x[0];
                sum += x[1];
                sum += x[2];
            }
            sum
        })
    });
}

fn cartesian_product_nested_for(c: &mut Criterion) {
    let xs = vec![0; 16];

    c.bench_function("cartesian product nested for", move |b| {
        b.iter(|| {
            let mut sum = 0;
            for &x in &xs {
                for &y in &xs {
                    for &z in &xs {
                        sum += x;
                        sum += y;
                        sum += z;
                    }
                }
            }
            sum
        })
    });
}

fn all_equal(c: &mut Criterion) {
    let mut xs = vec![0; 5_000_000];
    xs.extend(vec![1; 5_000_000]);

    c.bench_function("all equal", move |b| b.iter(|| xs.iter().all_equal()));
}

fn all_equal_for(c: &mut Criterion) {
    let mut xs = vec![0; 5_000_000];
    xs.extend(vec![1; 5_000_000]);

    c.bench_function("all equal for", move |b| {
        b.iter(|| {
            for &x in &xs {
                if x != xs[0] {
                    return false;
                }
            }
            true
        })
    });
}

fn all_equal_default(c: &mut Criterion) {
    let mut xs = vec![0; 5_000_000];
    xs.extend(vec![1; 5_000_000]);

    c.bench_function("all equal default", move |b| {
        b.iter(|| xs.iter().dedup().nth(1).is_none())
    });
}

const PERM_COUNT: usize = 6;

fn permutations_iter(c: &mut Criterion) {
    struct NewIterator(Range<usize>);

    impl Iterator for NewIterator {
        type Item = usize;

        fn next(&mut self) -> Option<Self::Item> {
            self.0.next()
        }
    }

    c.bench_function("permutations iter", move |b| {
        b.iter(
            || {
                for _ in NewIterator(0..PERM_COUNT).permutations(PERM_COUNT) {}
            },
        )
    });
}

fn permutations_range(c: &mut Criterion) {
    c.bench_function("permutations range", move |b| {
        b.iter(|| for _ in (0..PERM_COUNT).permutations(PERM_COUNT) {})
    });
}

fn permutations_slice(c: &mut Criterion) {
    let v = (0..PERM_COUNT).collect_vec();

    c.bench_function("permutations slice", move |b| {
        b.iter(|| for _ in v.as_slice().iter().permutations(PERM_COUNT) {})
    });
}

criterion_group!(
    benches,
    slice_iter,
    slice_iter_rev,
    zip_default_zip,
    zipdot_i32_default_zip,
    zipdot_f32_default_zip,
    zip_default_zip3,
    zip_slices_ziptuple,
    zip_checked_counted_loop,
    zipdot_i32_checked_counted_loop,
    zipdot_f32_checked_counted_loop,
    zipdot_f32_checked_counted_unrolled_loop,
    zip_unchecked_counted_loop,
    zipdot_i32_unchecked_counted_loop,
    zipdot_f32_unchecked_counted_loop,
    zip_unchecked_counted_loop3,
    chunk_by_lazy_1,
    chunk_by_lazy_2,
    slice_chunks,
    chunks_lazy_1,
    equal,
    merge_default,
    merge_by_cmp,
    merge_by_lt,
    kmerge_default,
    kmerge_tenway,
    step_vec_2,
    step_vec_10,
    step_range_2,
    step_range_10,
    vec_iter_mut_partition,
    cartesian_product_iterator,
    multi_cartesian_product_iterator,
    cartesian_product_nested_for,
    all_equal,
    all_equal_for,
    all_equal_default,
    permutations_iter,
    permutations_range,
    permutations_slice,
);
criterion_main!(benches);
