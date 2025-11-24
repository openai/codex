//! Benchmark comparing the current GCD implemtation against an older one.

#![feature(test)]

extern crate test;

use num_integer::Integer;
use num_traits::{AsPrimitive, Bounded, Signed};
use test::{black_box, Bencher};

trait GcdOld: Integer {
    fn gcd_old(&self, other: &Self) -> Self;
}

macro_rules! impl_gcd_old_for_isize {
    ($T:ty) => {
        impl GcdOld for $T {
            /// Calculates the Greatest Common Divisor (GCD) of the number and
            /// `other`. The result is always positive.
            #[inline]
            fn gcd_old(&self, other: &Self) -> Self {
                // Use Stein's algorithm
                let mut m = *self;
                let mut n = *other;
                if m == 0 || n == 0 {
                    return (m | n).abs();
                }

                // find common factors of 2
                let shift = (m | n).trailing_zeros();

                // The algorithm needs positive numbers, but the minimum value
                // can't be represented as a positive one.
                // It's also a power of two, so the gcd can be
                // calculated by bitshifting in that case

                // Assuming two's complement, the number created by the shift
                // is positive for all numbers except gcd = abs(min value)
                // The call to .abs() causes a panic in debug mode
                if m == Self::min_value() || n == Self::min_value() {
                    return (1 << shift).abs();
                }

                // guaranteed to be positive now, rest like unsigned algorithm
                m = m.abs();
                n = n.abs();

                // divide n and m by 2 until odd
                // m inside loop
                n >>= n.trailing_zeros();

                while m != 0 {
                    m >>= m.trailing_zeros();
                    if n > m {
                        std::mem::swap(&mut n, &mut m)
                    }
                    m -= n;
                }

                n << shift
            }
        }
    };
}

impl_gcd_old_for_isize!(i8);
impl_gcd_old_for_isize!(i16);
impl_gcd_old_for_isize!(i32);
impl_gcd_old_for_isize!(i64);
impl_gcd_old_for_isize!(isize);
impl_gcd_old_for_isize!(i128);

macro_rules! impl_gcd_old_for_usize {
    ($T:ty) => {
        impl GcdOld for $T {
            /// Calculates the Greatest Common Divisor (GCD) of the number and
            /// `other`. The result is always positive.
            #[inline]
            fn gcd_old(&self, other: &Self) -> Self {
                // Use Stein's algorithm
                let mut m = *self;
                let mut n = *other;
                if m == 0 || n == 0 {
                    return m | n;
                }

                // find common factors of 2
                let shift = (m | n).trailing_zeros();

                // divide n and m by 2 until odd
                // m inside loop
                n >>= n.trailing_zeros();

                while m != 0 {
                    m >>= m.trailing_zeros();
                    if n > m {
                        std::mem::swap(&mut n, &mut m)
                    }
                    m -= n;
                }

                n << shift
            }
        }
    };
}

impl_gcd_old_for_usize!(u8);
impl_gcd_old_for_usize!(u16);
impl_gcd_old_for_usize!(u32);
impl_gcd_old_for_usize!(u64);
impl_gcd_old_for_usize!(usize);
impl_gcd_old_for_usize!(u128);

/// Return an iterator that yields all Fibonacci numbers fitting into a u128.
fn fibonacci() -> impl Iterator<Item = u128> {
    (0..185).scan((0, 1), |&mut (ref mut a, ref mut b), _| {
        let tmp = *a;
        *a = *b;
        *b += tmp;
        Some(*b)
    })
}

fn run_bench<T: Integer + Bounded + Copy + 'static>(b: &mut Bencher, gcd: fn(&T, &T) -> T)
where
    T: AsPrimitive<u128>,
    u128: AsPrimitive<T>,
{
    let max_value: u128 = T::max_value().as_();
    let pairs: Vec<(T, T)> = fibonacci()
        .collect::<Vec<_>>()
        .windows(2)
        .filter(|&pair| pair[0] <= max_value && pair[1] <= max_value)
        .map(|pair| (pair[0].as_(), pair[1].as_()))
        .collect();
    b.iter(|| {
        for &(ref m, ref n) in &pairs {
            black_box(gcd(m, n));
        }
    });
}

macro_rules! bench_gcd {
    ($T:ident) => {
        mod $T {
            use crate::{run_bench, GcdOld};
            use num_integer::Integer;
            use test::Bencher;

            #[bench]
            fn bench_gcd(b: &mut Bencher) {
                run_bench(b, $T::gcd);
            }

            #[bench]
            fn bench_gcd_old(b: &mut Bencher) {
                run_bench(b, $T::gcd_old);
            }
        }
    };
}

bench_gcd!(u8);
bench_gcd!(u16);
bench_gcd!(u32);
bench_gcd!(u64);
bench_gcd!(u128);

bench_gcd!(i8);
bench_gcd!(i16);
bench_gcd!(i32);
bench_gcd!(i64);
bench_gcd!(i128);
