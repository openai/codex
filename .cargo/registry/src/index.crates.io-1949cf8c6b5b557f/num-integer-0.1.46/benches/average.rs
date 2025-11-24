//! Benchmark sqrt and cbrt

#![feature(test)]

extern crate test;

use num_integer::Integer;
use num_traits::{AsPrimitive, PrimInt, WrappingAdd, WrappingMul};
use std::cmp::{max, min};
use std::fmt::Debug;
use test::{black_box, Bencher};

// --- Utilities for RNG ----------------------------------------------------

trait BenchInteger: Integer + PrimInt + WrappingAdd + WrappingMul + 'static {}

impl<T> BenchInteger for T where T: Integer + PrimInt + WrappingAdd + WrappingMul + 'static {}

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

// --- Alt. Implementations -------------------------------------------------

trait NaiveAverage {
    fn naive_average_ceil(&self, other: &Self) -> Self;
    fn naive_average_floor(&self, other: &Self) -> Self;
}

trait UncheckedAverage {
    fn unchecked_average_ceil(&self, other: &Self) -> Self;
    fn unchecked_average_floor(&self, other: &Self) -> Self;
}

trait ModuloAverage {
    fn modulo_average_ceil(&self, other: &Self) -> Self;
    fn modulo_average_floor(&self, other: &Self) -> Self;
}

macro_rules! naive_average {
    ($T:ident) => {
        impl super::NaiveAverage for $T {
            fn naive_average_floor(&self, other: &$T) -> $T {
                match self.checked_add(*other) {
                    Some(z) => Integer::div_floor(&z, &2),
                    None => {
                        if self > other {
                            let diff = self - other;
                            other + Integer::div_floor(&diff, &2)
                        } else {
                            let diff = other - self;
                            self + Integer::div_floor(&diff, &2)
                        }
                    }
                }
            }
            fn naive_average_ceil(&self, other: &$T) -> $T {
                match self.checked_add(*other) {
                    Some(z) => Integer::div_ceil(&z, &2),
                    None => {
                        if self > other {
                            let diff = self - other;
                            self - Integer::div_floor(&diff, &2)
                        } else {
                            let diff = other - self;
                            other - Integer::div_floor(&diff, &2)
                        }
                    }
                }
            }
        }
    };
}

macro_rules! unchecked_average {
    ($T:ident) => {
        impl super::UncheckedAverage for $T {
            fn unchecked_average_floor(&self, other: &$T) -> $T {
                self.wrapping_add(*other) / 2
            }
            fn unchecked_average_ceil(&self, other: &$T) -> $T {
                (self.wrapping_add(*other) / 2).wrapping_add(1)
            }
        }
    };
}

macro_rules! modulo_average {
    ($T:ident) => {
        impl super::ModuloAverage for $T {
            fn modulo_average_ceil(&self, other: &$T) -> $T {
                let (q1, r1) = self.div_mod_floor(&2);
                let (q2, r2) = other.div_mod_floor(&2);
                q1 + q2 + (r1 | r2)
            }
            fn modulo_average_floor(&self, other: &$T) -> $T {
                let (q1, r1) = self.div_mod_floor(&2);
                let (q2, r2) = other.div_mod_floor(&2);
                q1 + q2 + (r1 * r2)
            }
        }
    };
}

// --- Bench functions ------------------------------------------------------

fn bench_unchecked<T, F>(b: &mut Bencher, v: &[(T, T)], f: F)
where
    T: Integer + Debug + Copy,
    F: Fn(&T, &T) -> T,
{
    b.iter(|| {
        for (x, y) in v {
            black_box(f(x, y));
        }
    });
}

fn bench_ceil<T, F>(b: &mut Bencher, v: &[(T, T)], f: F)
where
    T: Integer + Debug + Copy,
    F: Fn(&T, &T) -> T,
{
    for &(i, j) in v {
        let rt = f(&i, &j);
        let (a, b) = (min(i, j), max(i, j));
        // if both number are the same sign, check rt is in the middle
        if (a < T::zero()) == (b < T::zero()) {
            if (b - a).is_even() {
                assert_eq!(rt - a, b - rt);
            } else {
                assert_eq!(rt - a, b - rt + T::one());
            }
        // if both number have a different sign,
        } else if (a + b).is_even() {
            assert_eq!(rt, (a + b) / (T::one() + T::one()))
        } else {
            assert_eq!(rt, (a + b + T::one()) / (T::one() + T::one()))
        }
    }
    bench_unchecked(b, v, f);
}

fn bench_floor<T, F>(b: &mut Bencher, v: &[(T, T)], f: F)
where
    T: Integer + Debug + Copy,
    F: Fn(&T, &T) -> T,
{
    for &(i, j) in v {
        let rt = f(&i, &j);
        let (a, b) = (min(i, j), max(i, j));
        // if both number are the same sign, check rt is in the middle
        if (a < T::zero()) == (b < T::zero()) {
            if (b - a).is_even() {
                assert_eq!(rt - a, b - rt);
            } else {
                assert_eq!(rt - a + T::one(), b - rt);
            }
        // if both number have a different sign,
        } else if (a + b).is_even() {
            assert_eq!(rt, (a + b) / (T::one() + T::one()))
        } else {
            assert_eq!(rt, (a + b - T::one()) / (T::one() + T::one()))
        }
    }
    bench_unchecked(b, v, f);
}

// --- Bench implementation -------------------------------------------------

macro_rules! bench_average {
    ($($T:ident),*) => {$(
        mod $T {
            use test::Bencher;
            use num_integer::{Average, Integer};
            use super::{UncheckedAverage, NaiveAverage, ModuloAverage};
            use super::{bench_ceil, bench_floor, bench_unchecked};

            naive_average!($T);
            unchecked_average!($T);
            modulo_average!($T);

            const SIZE: $T = 30;

            fn overflowing() -> Vec<($T, $T)> {
                (($T::max_value()-SIZE)..$T::max_value())
                    .flat_map(|x| -> Vec<_> {
                        (($T::max_value()-100)..($T::max_value()-100+SIZE))
                            .map(|y| (x, y))
                            .collect()
                    })
                    .collect()
            }

            fn small() -> Vec<($T, $T)> {
                (0..SIZE)
                   .flat_map(|x| -> Vec<_> {(0..SIZE).map(|y| (x, y)).collect()})
                   .collect()
            }

            fn rand() -> Vec<($T, $T)> {
                small()
                    .into_iter()
                    .map(|(x, y)| (super::lcg(x), super::lcg(y)))
                    .collect()
            }

            mod ceil {

                use super::*;

                mod small {

                    use super::*;

                    #[bench]
                    fn optimized(b: &mut Bencher) {
                        let v = small();
                        bench_ceil(b, &v, |x: &$T, y: &$T| x.average_ceil(y));
                    }

                    #[bench]
                    fn naive(b: &mut Bencher) {
                        let v = small();
                        bench_ceil(b, &v, |x: &$T, y: &$T| x.naive_average_ceil(y));
                    }

                    #[bench]
                    fn unchecked(b: &mut Bencher) {
                        let v = small();
                        bench_unchecked(b, &v, |x: &$T, y: &$T| x.unchecked_average_ceil(y));
                    }

                    #[bench]
                    fn modulo(b: &mut Bencher) {
                        let v = small();
                        bench_ceil(b, &v, |x: &$T, y: &$T| x.modulo_average_ceil(y));
                    }
                }

                mod overflowing {

                    use super::*;

                    #[bench]
                    fn optimized(b: &mut Bencher) {
                        let v = overflowing();
                        bench_ceil(b, &v, |x: &$T, y: &$T| x.average_ceil(y));
                    }

                    #[bench]
                    fn naive(b: &mut Bencher) {
                        let v = overflowing();
                        bench_ceil(b, &v, |x: &$T, y: &$T| x.naive_average_ceil(y));
                    }

                    #[bench]
                    fn unchecked(b: &mut Bencher) {
                        let v = overflowing();
                        bench_unchecked(b, &v, |x: &$T, y: &$T| x.unchecked_average_ceil(y));
                    }

                    #[bench]
                    fn modulo(b: &mut Bencher) {
                        let v = overflowing();
                        bench_ceil(b, &v, |x: &$T, y: &$T| x.modulo_average_ceil(y));
                    }
                }

                mod rand {

                    use super::*;

                    #[bench]
                    fn optimized(b: &mut Bencher) {
                        let v = rand();
                        bench_ceil(b, &v, |x: &$T, y: &$T| x.average_ceil(y));
                    }

                    #[bench]
                    fn naive(b: &mut Bencher) {
                        let v = rand();
                        bench_ceil(b, &v, |x: &$T, y: &$T| x.naive_average_ceil(y));
                    }

                    #[bench]
                    fn unchecked(b: &mut Bencher) {
                        let v = rand();
                        bench_unchecked(b, &v, |x: &$T, y: &$T| x.unchecked_average_ceil(y));
                    }

                    #[bench]
                    fn modulo(b: &mut Bencher) {
                        let v = rand();
                        bench_ceil(b, &v, |x: &$T, y: &$T| x.modulo_average_ceil(y));
                    }
                }

            }

            mod floor {

                use super::*;

                mod small {

                    use super::*;

                    #[bench]
                    fn optimized(b: &mut Bencher) {
                        let v = small();
                        bench_floor(b, &v, |x: &$T, y: &$T| x.average_floor(y));
                    }

                    #[bench]
                    fn naive(b: &mut Bencher) {
                        let v = small();
                        bench_floor(b, &v, |x: &$T, y: &$T| x.naive_average_floor(y));
                    }

                    #[bench]
                    fn unchecked(b: &mut Bencher) {
                        let v = small();
                        bench_unchecked(b, &v, |x: &$T, y: &$T| x.unchecked_average_floor(y));
                    }

                    #[bench]
                    fn modulo(b: &mut Bencher) {
                        let v = small();
                        bench_floor(b, &v, |x: &$T, y: &$T| x.modulo_average_floor(y));
                    }
                }

                mod overflowing {

                    use super::*;

                    #[bench]
                    fn optimized(b: &mut Bencher) {
                        let v = overflowing();
                        bench_floor(b, &v, |x: &$T, y: &$T| x.average_floor(y));
                    }

                    #[bench]
                    fn naive(b: &mut Bencher) {
                        let v = overflowing();
                        bench_floor(b, &v, |x: &$T, y: &$T| x.naive_average_floor(y));
                    }

                    #[bench]
                    fn unchecked(b: &mut Bencher) {
                        let v = overflowing();
                        bench_unchecked(b, &v, |x: &$T, y: &$T| x.unchecked_average_floor(y));
                    }

                    #[bench]
                    fn modulo(b: &mut Bencher) {
                        let v = overflowing();
                        bench_floor(b, &v, |x: &$T, y: &$T| x.modulo_average_floor(y));
                    }
                }

                mod rand {

                    use super::*;

                    #[bench]
                    fn optimized(b: &mut Bencher) {
                        let v = rand();
                        bench_floor(b, &v, |x: &$T, y: &$T| x.average_floor(y));
                    }

                    #[bench]
                    fn naive(b: &mut Bencher) {
                        let v = rand();
                        bench_floor(b, &v, |x: &$T, y: &$T| x.naive_average_floor(y));
                    }

                    #[bench]
                    fn unchecked(b: &mut Bencher) {
                        let v = rand();
                        bench_unchecked(b, &v, |x: &$T, y: &$T| x.unchecked_average_floor(y));
                    }

                    #[bench]
                    fn modulo(b: &mut Bencher) {
                        let v = rand();
                        bench_floor(b, &v, |x: &$T, y: &$T| x.modulo_average_floor(y));
                    }
                }

            }

        }
    )*}
}

bench_average!(i8, i16, i32, i64, i128, isize);
bench_average!(u8, u16, u32, u64, u128, usize);
