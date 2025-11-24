//! Benchmark sqrt and cbrt

#![feature(test)]

extern crate test;

use num_integer::Integer;
use num_traits::checked_pow;
use num_traits::{AsPrimitive, PrimInt, WrappingAdd, WrappingMul};
use test::{black_box, Bencher};

trait BenchInteger: Integer + PrimInt + WrappingAdd + WrappingMul + 'static {}

impl<T> BenchInteger for T where T: Integer + PrimInt + WrappingAdd + WrappingMul + 'static {}

fn bench<T, F>(b: &mut Bencher, v: &[T], f: F, n: u32)
where
    T: BenchInteger,
    F: Fn(&T) -> T,
{
    // Pre-validate the results...
    for i in v {
        let rt = f(i);
        if *i >= T::zero() {
            let rt1 = rt + T::one();
            assert!(rt.pow(n) <= *i);
            if let Some(x) = checked_pow(rt1, n as usize) {
                assert!(*i < x);
            }
        } else {
            let rt1 = rt - T::one();
            assert!(rt < T::zero());
            assert!(*i <= rt.pow(n));
            if let Some(x) = checked_pow(rt1, n as usize) {
                assert!(x < *i);
            }
        };
    }

    // Now just run as fast as we can!
    b.iter(|| {
        for i in v {
            black_box(f(i));
        }
    });
}

// Simple PRNG so we don't have to worry about rand compatibility
fn lcg<T>(x: T) -> T
where
    u32: AsPrimitive<T>,
    T: BenchInteger,
{
    // LCG parameters from Numerical Recipes
    // (but we're applying it to arbitrary sizes)
    const LCG_A: u32 = 1664525;
    const LCG_C: u32 = 1013904223;
    x.wrapping_mul(&LCG_A.as_()).wrapping_add(&LCG_C.as_())
}

fn bench_rand<T, F>(b: &mut Bencher, f: F, n: u32)
where
    u32: AsPrimitive<T>,
    T: BenchInteger,
    F: Fn(&T) -> T,
{
    let mut x: T = 3u32.as_();
    let v: Vec<T> = (0..1000)
        .map(|_| {
            x = lcg(x);
            x
        })
        .collect();
    bench(b, &v, f, n);
}

fn bench_rand_pos<T, F>(b: &mut Bencher, f: F, n: u32)
where
    u32: AsPrimitive<T>,
    T: BenchInteger,
    F: Fn(&T) -> T,
{
    let mut x: T = 3u32.as_();
    let v: Vec<T> = (0..1000)
        .map(|_| {
            x = lcg(x);
            while x < T::zero() {
                x = lcg(x);
            }
            x
        })
        .collect();
    bench(b, &v, f, n);
}

fn bench_small<T, F>(b: &mut Bencher, f: F, n: u32)
where
    u32: AsPrimitive<T>,
    T: BenchInteger,
    F: Fn(&T) -> T,
{
    let v: Vec<T> = (0..1000).map(|i| i.as_()).collect();
    bench(b, &v, f, n);
}

fn bench_small_pos<T, F>(b: &mut Bencher, f: F, n: u32)
where
    u32: AsPrimitive<T>,
    T: BenchInteger,
    F: Fn(&T) -> T,
{
    let v: Vec<T> = (0..1000)
        .map(|i| i.as_().mod_floor(&T::max_value()))
        .collect();
    bench(b, &v, f, n);
}

macro_rules! bench_roots {
    ($($T:ident),*) => {$(
        mod $T {
            use test::Bencher;
            use num_integer::Roots;

            #[bench]
            fn sqrt_rand(b: &mut Bencher) {
                crate::bench_rand_pos(b, $T::sqrt, 2);
            }

            #[bench]
            fn sqrt_small(b: &mut Bencher) {
                crate::bench_small_pos(b, $T::sqrt, 2);
            }

            #[bench]
            fn cbrt_rand(b: &mut Bencher) {
                crate::bench_rand(b, $T::cbrt, 3);
            }

            #[bench]
            fn cbrt_small(b: &mut Bencher) {
                crate::bench_small(b, $T::cbrt, 3);
            }

            #[bench]
            fn fourth_root_rand(b: &mut Bencher) {
                crate::bench_rand_pos(b, |x: &$T| x.nth_root(4), 4);
            }

            #[bench]
            fn fourth_root_small(b: &mut Bencher) {
                crate::bench_small_pos(b, |x: &$T| x.nth_root(4), 4);
            }

            #[bench]
            fn fifth_root_rand(b: &mut Bencher) {
                crate::bench_rand(b, |x: &$T| x.nth_root(5), 5);
            }

            #[bench]
            fn fifth_root_small(b: &mut Bencher) {
                crate::bench_small(b, |x: &$T| x.nth_root(5), 5);
            }
        }
    )*}
}

bench_roots!(i8, i16, i32, i64, i128);
bench_roots!(u8, u16, u32, u64, u128);
