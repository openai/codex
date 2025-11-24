// Copyright 2013-2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Integer trait and functions.
//!
//! ## Compatibility
//!
//! The `num-integer` crate is tested for rustc 1.31 and greater.

#![doc(html_root_url = "https://docs.rs/num-integer/0.1")]
#![no_std]

use core::mem;
use core::ops::Add;

use num_traits::{Num, Signed, Zero};

mod roots;
pub use crate::roots::Roots;
pub use crate::roots::{cbrt, nth_root, sqrt};

mod average;
pub use crate::average::Average;
pub use crate::average::{average_ceil, average_floor};

pub trait Integer: Sized + Num + PartialOrd + Ord + Eq {
    /// Floored integer division.
    ///
    /// # Examples
    ///
    /// ~~~
    /// # use num_integer::Integer;
    /// assert!(( 8).div_floor(& 3) ==  2);
    /// assert!(( 8).div_floor(&-3) == -3);
    /// assert!((-8).div_floor(& 3) == -3);
    /// assert!((-8).div_floor(&-3) ==  2);
    ///
    /// assert!(( 1).div_floor(& 2) ==  0);
    /// assert!(( 1).div_floor(&-2) == -1);
    /// assert!((-1).div_floor(& 2) == -1);
    /// assert!((-1).div_floor(&-2) ==  0);
    /// ~~~
    fn div_floor(&self, other: &Self) -> Self;

    /// Floored integer modulo, satisfying:
    ///
    /// ~~~
    /// # use num_integer::Integer;
    /// # let n = 1; let d = 1;
    /// assert!(n.div_floor(&d) * d + n.mod_floor(&d) == n)
    /// ~~~
    ///
    /// # Examples
    ///
    /// ~~~
    /// # use num_integer::Integer;
    /// assert!(( 8).mod_floor(& 3) ==  2);
    /// assert!(( 8).mod_floor(&-3) == -1);
    /// assert!((-8).mod_floor(& 3) ==  1);
    /// assert!((-8).mod_floor(&-3) == -2);
    ///
    /// assert!(( 1).mod_floor(& 2) ==  1);
    /// assert!(( 1).mod_floor(&-2) == -1);
    /// assert!((-1).mod_floor(& 2) ==  1);
    /// assert!((-1).mod_floor(&-2) == -1);
    /// ~~~
    fn mod_floor(&self, other: &Self) -> Self;

    /// Ceiled integer division.
    ///
    /// # Examples
    ///
    /// ~~~
    /// # use num_integer::Integer;
    /// assert_eq!(( 8).div_ceil( &3),  3);
    /// assert_eq!(( 8).div_ceil(&-3), -2);
    /// assert_eq!((-8).div_ceil( &3), -2);
    /// assert_eq!((-8).div_ceil(&-3),  3);
    ///
    /// assert_eq!(( 1).div_ceil( &2), 1);
    /// assert_eq!(( 1).div_ceil(&-2), 0);
    /// assert_eq!((-1).div_ceil( &2), 0);
    /// assert_eq!((-1).div_ceil(&-2), 1);
    /// ~~~
    fn div_ceil(&self, other: &Self) -> Self {
        let (q, r) = self.div_mod_floor(other);
        if r.is_zero() {
            q
        } else {
            q + Self::one()
        }
    }

    /// Greatest Common Divisor (GCD).
    ///
    /// # Examples
    ///
    /// ~~~
    /// # use num_integer::Integer;
    /// assert_eq!(6.gcd(&8), 2);
    /// assert_eq!(7.gcd(&3), 1);
    /// ~~~
    fn gcd(&self, other: &Self) -> Self;

    /// Lowest Common Multiple (LCM).
    ///
    /// # Examples
    ///
    /// ~~~
    /// # use num_integer::Integer;
    /// assert_eq!(7.lcm(&3), 21);
    /// assert_eq!(2.lcm(&4), 4);
    /// assert_eq!(0.lcm(&0), 0);
    /// ~~~
    fn lcm(&self, other: &Self) -> Self;

    /// Greatest Common Divisor (GCD) and
    /// Lowest Common Multiple (LCM) together.
    ///
    /// Potentially more efficient than calling `gcd` and `lcm`
    /// individually for identical inputs.
    ///
    /// # Examples
    ///
    /// ~~~
    /// # use num_integer::Integer;
    /// assert_eq!(10.gcd_lcm(&4), (2, 20));
    /// assert_eq!(8.gcd_lcm(&9), (1, 72));
    /// ~~~
    #[inline]
    fn gcd_lcm(&self, other: &Self) -> (Self, Self) {
        (self.gcd(other), self.lcm(other))
    }

    /// Greatest common divisor and Bézout coefficients.
    ///
    /// # Examples
    ///
    /// ~~~
    /// # fn main() {
    /// # use num_integer::{ExtendedGcd, Integer};
    /// # use num_traits::NumAssign;
    /// fn check<A: Copy + Integer + NumAssign>(a: A, b: A) -> bool {
    ///     let ExtendedGcd { gcd, x, y, .. } = a.extended_gcd(&b);
    ///     gcd == x * a + y * b
    /// }
    /// assert!(check(10isize, 4isize));
    /// assert!(check(8isize,  9isize));
    /// # }
    /// ~~~
    #[inline]
    fn extended_gcd(&self, other: &Self) -> ExtendedGcd<Self>
    where
        Self: Clone,
    {
        let mut s = (Self::zero(), Self::one());
        let mut t = (Self::one(), Self::zero());
        let mut r = (other.clone(), self.clone());

        while !r.0.is_zero() {
            let q = r.1.clone() / r.0.clone();
            let f = |mut r: (Self, Self)| {
                mem::swap(&mut r.0, &mut r.1);
                r.0 = r.0 - q.clone() * r.1.clone();
                r
            };
            r = f(r);
            s = f(s);
            t = f(t);
        }

        if r.1 >= Self::zero() {
            ExtendedGcd {
                gcd: r.1,
                x: s.1,
                y: t.1,
            }
        } else {
            ExtendedGcd {
                gcd: Self::zero() - r.1,
                x: Self::zero() - s.1,
                y: Self::zero() - t.1,
            }
        }
    }

    /// Greatest common divisor, least common multiple, and Bézout coefficients.
    #[inline]
    fn extended_gcd_lcm(&self, other: &Self) -> (ExtendedGcd<Self>, Self)
    where
        Self: Clone + Signed,
    {
        (self.extended_gcd(other), self.lcm(other))
    }

    /// Deprecated, use `is_multiple_of` instead.
    #[deprecated(note = "Please use is_multiple_of instead")]
    #[inline]
    fn divides(&self, other: &Self) -> bool {
        self.is_multiple_of(other)
    }

    /// Returns `true` if `self` is a multiple of `other`.
    ///
    /// # Examples
    ///
    /// ~~~
    /// # use num_integer::Integer;
    /// assert_eq!(9.is_multiple_of(&3), true);
    /// assert_eq!(3.is_multiple_of(&9), false);
    /// ~~~
    fn is_multiple_of(&self, other: &Self) -> bool;

    /// Returns `true` if the number is even.
    ///
    /// # Examples
    ///
    /// ~~~
    /// # use num_integer::Integer;
    /// assert_eq!(3.is_even(), false);
    /// assert_eq!(4.is_even(), true);
    /// ~~~
    fn is_even(&self) -> bool;

    /// Returns `true` if the number is odd.
    ///
    /// # Examples
    ///
    /// ~~~
    /// # use num_integer::Integer;
    /// assert_eq!(3.is_odd(), true);
    /// assert_eq!(4.is_odd(), false);
    /// ~~~
    fn is_odd(&self) -> bool;

    /// Simultaneous truncated integer division and modulus.
    /// Returns `(quotient, remainder)`.
    ///
    /// # Examples
    ///
    /// ~~~
    /// # use num_integer::Integer;
    /// assert_eq!(( 8).div_rem( &3), ( 2,  2));
    /// assert_eq!(( 8).div_rem(&-3), (-2,  2));
    /// assert_eq!((-8).div_rem( &3), (-2, -2));
    /// assert_eq!((-8).div_rem(&-3), ( 2, -2));
    ///
    /// assert_eq!(( 1).div_rem( &2), ( 0,  1));
    /// assert_eq!(( 1).div_rem(&-2), ( 0,  1));
    /// assert_eq!((-1).div_rem( &2), ( 0, -1));
    /// assert_eq!((-1).div_rem(&-2), ( 0, -1));
    /// ~~~
    fn div_rem(&self, other: &Self) -> (Self, Self);

    /// Simultaneous floored integer division and modulus.
    /// Returns `(quotient, remainder)`.
    ///
    /// # Examples
    ///
    /// ~~~
    /// # use num_integer::Integer;
    /// assert_eq!(( 8).div_mod_floor( &3), ( 2,  2));
    /// assert_eq!(( 8).div_mod_floor(&-3), (-3, -1));
    /// assert_eq!((-8).div_mod_floor( &3), (-3,  1));
    /// assert_eq!((-8).div_mod_floor(&-3), ( 2, -2));
    ///
    /// assert_eq!(( 1).div_mod_floor( &2), ( 0,  1));
    /// assert_eq!(( 1).div_mod_floor(&-2), (-1, -1));
    /// assert_eq!((-1).div_mod_floor( &2), (-1,  1));
    /// assert_eq!((-1).div_mod_floor(&-2), ( 0, -1));
    /// ~~~
    fn div_mod_floor(&self, other: &Self) -> (Self, Self) {
        (self.div_floor(other), self.mod_floor(other))
    }

    /// Rounds up to nearest multiple of argument.
    ///
    /// # Notes
    ///
    /// For signed types, `a.next_multiple_of(b) = a.prev_multiple_of(b.neg())`.
    ///
    /// # Examples
    ///
    /// ~~~
    /// # use num_integer::Integer;
    /// assert_eq!(( 16).next_multiple_of(& 8),  16);
    /// assert_eq!(( 23).next_multiple_of(& 8),  24);
    /// assert_eq!(( 16).next_multiple_of(&-8),  16);
    /// assert_eq!(( 23).next_multiple_of(&-8),  16);
    /// assert_eq!((-16).next_multiple_of(& 8), -16);
    /// assert_eq!((-23).next_multiple_of(& 8), -16);
    /// assert_eq!((-16).next_multiple_of(&-8), -16);
    /// assert_eq!((-23).next_multiple_of(&-8), -24);
    /// ~~~
    #[inline]
    fn next_multiple_of(&self, other: &Self) -> Self
    where
        Self: Clone,
    {
        let m = self.mod_floor(other);
        self.clone()
            + if m.is_zero() {
                Self::zero()
            } else {
                other.clone() - m
            }
    }

    /// Rounds down to nearest multiple of argument.
    ///
    /// # Notes
    ///
    /// For signed types, `a.prev_multiple_of(b) = a.next_multiple_of(b.neg())`.
    ///
    /// # Examples
    ///
    /// ~~~
    /// # use num_integer::Integer;
    /// assert_eq!(( 16).prev_multiple_of(& 8),  16);
    /// assert_eq!(( 23).prev_multiple_of(& 8),  16);
    /// assert_eq!(( 16).prev_multiple_of(&-8),  16);
    /// assert_eq!(( 23).prev_multiple_of(&-8),  24);
    /// assert_eq!((-16).prev_multiple_of(& 8), -16);
    /// assert_eq!((-23).prev_multiple_of(& 8), -24);
    /// assert_eq!((-16).prev_multiple_of(&-8), -16);
    /// assert_eq!((-23).prev_multiple_of(&-8), -16);
    /// ~~~
    #[inline]
    fn prev_multiple_of(&self, other: &Self) -> Self
    where
        Self: Clone,
    {
        self.clone() - self.mod_floor(other)
    }

    /// Decrements self by one.
    ///
    /// # Examples
    ///
    /// ~~~
    /// # use num_integer::Integer;
    /// let mut x: i32 = 43;
    /// x.dec();
    /// assert_eq!(x, 42);
    /// ~~~
    fn dec(&mut self)
    where
        Self: Clone,
    {
        *self = self.clone() - Self::one()
    }

    /// Increments self by one.
    ///
    /// # Examples
    ///
    /// ~~~
    /// # use num_integer::Integer;
    /// let mut x: i32 = 41;
    /// x.inc();
    /// assert_eq!(x, 42);
    /// ~~~
    fn inc(&mut self)
    where
        Self: Clone,
    {
        *self = self.clone() + Self::one()
    }
}

/// Greatest common divisor and Bézout coefficients
///
/// ```no_build
/// let e = isize::extended_gcd(a, b);
/// assert_eq!(e.gcd, e.x*a + e.y*b);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExtendedGcd<A> {
    pub gcd: A,
    pub x: A,
    pub y: A,
}

/// Simultaneous integer division and modulus
#[inline]
pub fn div_rem<T: Integer>(x: T, y: T) -> (T, T) {
    x.div_rem(&y)
}
/// Floored integer division
#[inline]
pub fn div_floor<T: Integer>(x: T, y: T) -> T {
    x.div_floor(&y)
}
/// Floored integer modulus
#[inline]
pub fn mod_floor<T: Integer>(x: T, y: T) -> T {
    x.mod_floor(&y)
}
/// Simultaneous floored integer division and modulus
#[inline]
pub fn div_mod_floor<T: Integer>(x: T, y: T) -> (T, T) {
    x.div_mod_floor(&y)
}
/// Ceiled integer division
#[inline]
pub fn div_ceil<T: Integer>(x: T, y: T) -> T {
    x.div_ceil(&y)
}

/// Calculates the Greatest Common Divisor (GCD) of the number and `other`. The
/// result is always non-negative.
#[inline(always)]
pub fn gcd<T: Integer>(x: T, y: T) -> T {
    x.gcd(&y)
}
/// Calculates the Lowest Common Multiple (LCM) of the number and `other`.
#[inline(always)]
pub fn lcm<T: Integer>(x: T, y: T) -> T {
    x.lcm(&y)
}

/// Calculates the Greatest Common Divisor (GCD) and
/// Lowest Common Multiple (LCM) of the number and `other`.
#[inline(always)]
pub fn gcd_lcm<T: Integer>(x: T, y: T) -> (T, T) {
    x.gcd_lcm(&y)
}

macro_rules! impl_integer_for_isize {
    ($T:ty, $test_mod:ident) => {
        impl Integer for $T {
            /// Floored integer division
            #[inline]
            fn div_floor(&self, other: &Self) -> Self {
                // Algorithm from [Daan Leijen. _Division and Modulus for Computer Scientists_,
                // December 2001](http://research.microsoft.com/pubs/151917/divmodnote-letter.pdf)
                let (d, r) = self.div_rem(other);
                if (r > 0 && *other < 0) || (r < 0 && *other > 0) {
                    d - 1
                } else {
                    d
                }
            }

            /// Floored integer modulo
            #[inline]
            fn mod_floor(&self, other: &Self) -> Self {
                // Algorithm from [Daan Leijen. _Division and Modulus for Computer Scientists_,
                // December 2001](http://research.microsoft.com/pubs/151917/divmodnote-letter.pdf)
                let r = *self % *other;
                if (r > 0 && *other < 0) || (r < 0 && *other > 0) {
                    r + *other
                } else {
                    r
                }
            }

            /// Calculates `div_floor` and `mod_floor` simultaneously
            #[inline]
            fn div_mod_floor(&self, other: &Self) -> (Self, Self) {
                // Algorithm from [Daan Leijen. _Division and Modulus for Computer Scientists_,
                // December 2001](http://research.microsoft.com/pubs/151917/divmodnote-letter.pdf)
                let (d, r) = self.div_rem(other);
                if (r > 0 && *other < 0) || (r < 0 && *other > 0) {
                    (d - 1, r + *other)
                } else {
                    (d, r)
                }
            }

            #[inline]
            fn div_ceil(&self, other: &Self) -> Self {
                let (d, r) = self.div_rem(other);
                if (r > 0 && *other > 0) || (r < 0 && *other < 0) {
                    d + 1
                } else {
                    d
                }
            }

            /// Calculates the Greatest Common Divisor (GCD) of the number and
            /// `other`. The result is always non-negative.
            #[inline]
            fn gcd(&self, other: &Self) -> Self {
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
                m >>= m.trailing_zeros();
                n >>= n.trailing_zeros();

                while m != n {
                    if m > n {
                        m -= n;
                        m >>= m.trailing_zeros();
                    } else {
                        n -= m;
                        n >>= n.trailing_zeros();
                    }
                }
                m << shift
            }

            #[inline]
            fn extended_gcd_lcm(&self, other: &Self) -> (ExtendedGcd<Self>, Self) {
                let egcd = self.extended_gcd(other);
                // should not have to recalculate abs
                let lcm = if egcd.gcd.is_zero() {
                    Self::zero()
                } else {
                    (*self * (*other / egcd.gcd)).abs()
                };
                (egcd, lcm)
            }

            /// Calculates the Lowest Common Multiple (LCM) of the number and
            /// `other`.
            #[inline]
            fn lcm(&self, other: &Self) -> Self {
                self.gcd_lcm(other).1
            }

            /// Calculates the Greatest Common Divisor (GCD) and
            /// Lowest Common Multiple (LCM) of the number and `other`.
            #[inline]
            fn gcd_lcm(&self, other: &Self) -> (Self, Self) {
                if self.is_zero() && other.is_zero() {
                    return (Self::zero(), Self::zero());
                }
                let gcd = self.gcd(other);
                // should not have to recalculate abs
                let lcm = (*self * (*other / gcd)).abs();
                (gcd, lcm)
            }

            /// Returns `true` if the number is a multiple of `other`.
            #[inline]
            fn is_multiple_of(&self, other: &Self) -> bool {
                if other.is_zero() {
                    return self.is_zero();
                }
                *self % *other == 0
            }

            /// Returns `true` if the number is divisible by `2`
            #[inline]
            fn is_even(&self) -> bool {
                (*self) & 1 == 0
            }

            /// Returns `true` if the number is not divisible by `2`
            #[inline]
            fn is_odd(&self) -> bool {
                !self.is_even()
            }

            /// Simultaneous truncated integer division and modulus.
            #[inline]
            fn div_rem(&self, other: &Self) -> (Self, Self) {
                (*self / *other, *self % *other)
            }

            /// Rounds up to nearest multiple of argument.
            #[inline]
            fn next_multiple_of(&self, other: &Self) -> Self {
                // Avoid the overflow of `MIN % -1`
                if *other == -1 {
                    return *self;
                }

                let m = Integer::mod_floor(self, other);
                *self + if m == 0 { 0 } else { other - m }
            }

            /// Rounds down to nearest multiple of argument.
            #[inline]
            fn prev_multiple_of(&self, other: &Self) -> Self {
                // Avoid the overflow of `MIN % -1`
                if *other == -1 {
                    return *self;
                }

                *self - Integer::mod_floor(self, other)
            }
        }

        #[cfg(test)]
        mod $test_mod {
            use crate::Integer;
            use core::mem;

            /// Checks that the division rule holds for:
            ///
            /// - `n`: numerator (dividend)
            /// - `d`: denominator (divisor)
            /// - `qr`: quotient and remainder
            #[cfg(test)]
            fn test_division_rule((n, d): ($T, $T), (q, r): ($T, $T)) {
                assert_eq!(d * q + r, n);
            }

            #[test]
            fn test_div_rem() {
                fn test_nd_dr(nd: ($T, $T), qr: ($T, $T)) {
                    let (n, d) = nd;
                    let separate_div_rem = (n / d, n % d);
                    let combined_div_rem = n.div_rem(&d);

                    test_division_rule(nd, qr);

                    assert_eq!(separate_div_rem, qr);
                    assert_eq!(combined_div_rem, qr);
                }

                test_nd_dr((8, 3), (2, 2));
                test_nd_dr((8, -3), (-2, 2));
                test_nd_dr((-8, 3), (-2, -2));
                test_nd_dr((-8, -3), (2, -2));

                test_nd_dr((1, 2), (0, 1));
                test_nd_dr((1, -2), (0, 1));
                test_nd_dr((-1, 2), (0, -1));
                test_nd_dr((-1, -2), (0, -1));
            }

            #[test]
            fn test_div_mod_floor() {
                fn test_nd_dm(nd: ($T, $T), dm: ($T, $T)) {
                    let (n, d) = nd;
                    let separate_div_mod_floor =
                        (Integer::div_floor(&n, &d), Integer::mod_floor(&n, &d));
                    let combined_div_mod_floor = Integer::div_mod_floor(&n, &d);

                    test_division_rule(nd, dm);

                    assert_eq!(separate_div_mod_floor, dm);
                    assert_eq!(combined_div_mod_floor, dm);
                }

                test_nd_dm((8, 3), (2, 2));
                test_nd_dm((8, -3), (-3, -1));
                test_nd_dm((-8, 3), (-3, 1));
                test_nd_dm((-8, -3), (2, -2));

                test_nd_dm((1, 2), (0, 1));
                test_nd_dm((1, -2), (-1, -1));
                test_nd_dm((-1, 2), (-1, 1));
                test_nd_dm((-1, -2), (0, -1));
            }

            #[test]
            fn test_gcd() {
                assert_eq!((10 as $T).gcd(&2), 2 as $T);
                assert_eq!((10 as $T).gcd(&3), 1 as $T);
                assert_eq!((0 as $T).gcd(&3), 3 as $T);
                assert_eq!((3 as $T).gcd(&3), 3 as $T);
                assert_eq!((56 as $T).gcd(&42), 14 as $T);
                assert_eq!((3 as $T).gcd(&-3), 3 as $T);
                assert_eq!((-6 as $T).gcd(&3), 3 as $T);
                assert_eq!((-4 as $T).gcd(&-2), 2 as $T);
            }

            #[test]
            fn test_gcd_cmp_with_euclidean() {
                fn euclidean_gcd(mut m: $T, mut n: $T) -> $T {
                    while m != 0 {
                        mem::swap(&mut m, &mut n);
                        m %= n;
                    }

                    n.abs()
                }

                // gcd(-128, b) = 128 is not representable as positive value
                // for i8
                for i in -127..=127 {
                    for j in -127..=127 {
                        assert_eq!(euclidean_gcd(i, j), i.gcd(&j));
                    }
                }
            }

            #[test]
            fn test_gcd_min_val() {
                let min = <$T>::min_value();
                let max = <$T>::max_value();
                let max_pow2 = max / 2 + 1;
                assert_eq!(min.gcd(&max), 1 as $T);
                assert_eq!(max.gcd(&min), 1 as $T);
                assert_eq!(min.gcd(&max_pow2), max_pow2);
                assert_eq!(max_pow2.gcd(&min), max_pow2);
                assert_eq!(min.gcd(&42), 2 as $T);
                assert_eq!((42 as $T).gcd(&min), 2 as $T);
            }

            #[test]
            #[should_panic]
            fn test_gcd_min_val_min_val() {
                let min = <$T>::min_value();
                assert!(min.gcd(&min) >= 0);
            }

            #[test]
            #[should_panic]
            fn test_gcd_min_val_0() {
                let min = <$T>::min_value();
                assert!(min.gcd(&0) >= 0);
            }

            #[test]
            #[should_panic]
            fn test_gcd_0_min_val() {
                let min = <$T>::min_value();
                assert!((0 as $T).gcd(&min) >= 0);
            }

            #[test]
            fn test_lcm() {
                assert_eq!((1 as $T).lcm(&0), 0 as $T);
                assert_eq!((0 as $T).lcm(&1), 0 as $T);
                assert_eq!((1 as $T).lcm(&1), 1 as $T);
                assert_eq!((-1 as $T).lcm(&1), 1 as $T);
                assert_eq!((1 as $T).lcm(&-1), 1 as $T);
                assert_eq!((-1 as $T).lcm(&-1), 1 as $T);
                assert_eq!((8 as $T).lcm(&9), 72 as $T);
                assert_eq!((11 as $T).lcm(&5), 55 as $T);
            }

            #[test]
            fn test_gcd_lcm() {
                use core::iter::once;
                for i in once(0)
                    .chain((1..).take(127).flat_map(|a| once(a).chain(once(-a))))
                    .chain(once(-128))
                {
                    for j in once(0)
                        .chain((1..).take(127).flat_map(|a| once(a).chain(once(-a))))
                        .chain(once(-128))
                    {
                        assert_eq!(i.gcd_lcm(&j), (i.gcd(&j), i.lcm(&j)));
                    }
                }
            }

            #[test]
            fn test_extended_gcd_lcm() {
                use crate::ExtendedGcd;
                use core::fmt::Debug;
                use num_traits::NumAssign;

                fn check<A: Copy + Debug + Integer + NumAssign>(a: A, b: A) {
                    let ExtendedGcd { gcd, x, y, .. } = a.extended_gcd(&b);
                    assert_eq!(gcd, x * a + y * b);
                }

                use core::iter::once;
                for i in once(0)
                    .chain((1..).take(127).flat_map(|a| once(a).chain(once(-a))))
                    .chain(once(-128))
                {
                    for j in once(0)
                        .chain((1..).take(127).flat_map(|a| once(a).chain(once(-a))))
                        .chain(once(-128))
                    {
                        check(i, j);
                        let (ExtendedGcd { gcd, .. }, lcm) = i.extended_gcd_lcm(&j);
                        assert_eq!((gcd, lcm), (i.gcd(&j), i.lcm(&j)));
                    }
                }
            }

            #[test]
            fn test_even() {
                assert_eq!((-4 as $T).is_even(), true);
                assert_eq!((-3 as $T).is_even(), false);
                assert_eq!((-2 as $T).is_even(), true);
                assert_eq!((-1 as $T).is_even(), false);
                assert_eq!((0 as $T).is_even(), true);
                assert_eq!((1 as $T).is_even(), false);
                assert_eq!((2 as $T).is_even(), true);
                assert_eq!((3 as $T).is_even(), false);
                assert_eq!((4 as $T).is_even(), true);
            }

            #[test]
            fn test_odd() {
                assert_eq!((-4 as $T).is_odd(), false);
                assert_eq!((-3 as $T).is_odd(), true);
                assert_eq!((-2 as $T).is_odd(), false);
                assert_eq!((-1 as $T).is_odd(), true);
                assert_eq!((0 as $T).is_odd(), false);
                assert_eq!((1 as $T).is_odd(), true);
                assert_eq!((2 as $T).is_odd(), false);
                assert_eq!((3 as $T).is_odd(), true);
                assert_eq!((4 as $T).is_odd(), false);
            }

            #[test]
            fn test_multiple_of_one_limits() {
                for x in &[<$T>::min_value(), <$T>::max_value()] {
                    for one in &[1, -1] {
                        assert_eq!(Integer::next_multiple_of(x, one), *x);
                        assert_eq!(Integer::prev_multiple_of(x, one), *x);
                    }
                }
            }
        }
    };
}

impl_integer_for_isize!(i8, test_integer_i8);
impl_integer_for_isize!(i16, test_integer_i16);
impl_integer_for_isize!(i32, test_integer_i32);
impl_integer_for_isize!(i64, test_integer_i64);
impl_integer_for_isize!(i128, test_integer_i128);
impl_integer_for_isize!(isize, test_integer_isize);

macro_rules! impl_integer_for_usize {
    ($T:ty, $test_mod:ident) => {
        impl Integer for $T {
            /// Unsigned integer division. Returns the same result as `div` (`/`).
            #[inline]
            fn div_floor(&self, other: &Self) -> Self {
                *self / *other
            }

            /// Unsigned integer modulo operation. Returns the same result as `rem` (`%`).
            #[inline]
            fn mod_floor(&self, other: &Self) -> Self {
                *self % *other
            }

            #[inline]
            fn div_ceil(&self, other: &Self) -> Self {
                *self / *other + (0 != *self % *other) as Self
            }

            /// Calculates the Greatest Common Divisor (GCD) of the number and `other`
            #[inline]
            fn gcd(&self, other: &Self) -> Self {
                // Use Stein's algorithm
                let mut m = *self;
                let mut n = *other;
                if m == 0 || n == 0 {
                    return m | n;
                }

                // find common factors of 2
                let shift = (m | n).trailing_zeros();

                // divide n and m by 2 until odd
                m >>= m.trailing_zeros();
                n >>= n.trailing_zeros();

                while m != n {
                    if m > n {
                        m -= n;
                        m >>= m.trailing_zeros();
                    } else {
                        n -= m;
                        n >>= n.trailing_zeros();
                    }
                }
                m << shift
            }

            #[inline]
            fn extended_gcd_lcm(&self, other: &Self) -> (ExtendedGcd<Self>, Self) {
                let egcd = self.extended_gcd(other);
                // should not have to recalculate abs
                let lcm = if egcd.gcd.is_zero() {
                    Self::zero()
                } else {
                    *self * (*other / egcd.gcd)
                };
                (egcd, lcm)
            }

            /// Calculates the Lowest Common Multiple (LCM) of the number and `other`.
            #[inline]
            fn lcm(&self, other: &Self) -> Self {
                self.gcd_lcm(other).1
            }

            /// Calculates the Greatest Common Divisor (GCD) and
            /// Lowest Common Multiple (LCM) of the number and `other`.
            #[inline]
            fn gcd_lcm(&self, other: &Self) -> (Self, Self) {
                if self.is_zero() && other.is_zero() {
                    return (Self::zero(), Self::zero());
                }
                let gcd = self.gcd(other);
                let lcm = *self * (*other / gcd);
                (gcd, lcm)
            }

            /// Returns `true` if the number is a multiple of `other`.
            #[inline]
            fn is_multiple_of(&self, other: &Self) -> bool {
                if other.is_zero() {
                    return self.is_zero();
                }
                *self % *other == 0
            }

            /// Returns `true` if the number is divisible by `2`.
            #[inline]
            fn is_even(&self) -> bool {
                *self % 2 == 0
            }

            /// Returns `true` if the number is not divisible by `2`.
            #[inline]
            fn is_odd(&self) -> bool {
                !self.is_even()
            }

            /// Simultaneous truncated integer division and modulus.
            #[inline]
            fn div_rem(&self, other: &Self) -> (Self, Self) {
                (*self / *other, *self % *other)
            }
        }

        #[cfg(test)]
        mod $test_mod {
            use crate::Integer;
            use core::mem;

            #[test]
            fn test_div_mod_floor() {
                assert_eq!(<$T as Integer>::div_floor(&10, &3), 3 as $T);
                assert_eq!(<$T as Integer>::mod_floor(&10, &3), 1 as $T);
                assert_eq!(<$T as Integer>::div_mod_floor(&10, &3), (3 as $T, 1 as $T));
                assert_eq!(<$T as Integer>::div_floor(&5, &5), 1 as $T);
                assert_eq!(<$T as Integer>::mod_floor(&5, &5), 0 as $T);
                assert_eq!(<$T as Integer>::div_mod_floor(&5, &5), (1 as $T, 0 as $T));
                assert_eq!(<$T as Integer>::div_floor(&3, &7), 0 as $T);
                assert_eq!(<$T as Integer>::div_floor(&3, &7), 0 as $T);
                assert_eq!(<$T as Integer>::mod_floor(&3, &7), 3 as $T);
                assert_eq!(<$T as Integer>::div_mod_floor(&3, &7), (0 as $T, 3 as $T));
            }

            #[test]
            fn test_gcd() {
                assert_eq!((10 as $T).gcd(&2), 2 as $T);
                assert_eq!((10 as $T).gcd(&3), 1 as $T);
                assert_eq!((0 as $T).gcd(&3), 3 as $T);
                assert_eq!((3 as $T).gcd(&3), 3 as $T);
                assert_eq!((56 as $T).gcd(&42), 14 as $T);
            }

            #[test]
            fn test_gcd_cmp_with_euclidean() {
                fn euclidean_gcd(mut m: $T, mut n: $T) -> $T {
                    while m != 0 {
                        mem::swap(&mut m, &mut n);
                        m %= n;
                    }
                    n
                }

                for i in 0..=255 {
                    for j in 0..=255 {
                        assert_eq!(euclidean_gcd(i, j), i.gcd(&j));
                    }
                }
            }

            #[test]
            fn test_lcm() {
                assert_eq!((1 as $T).lcm(&0), 0 as $T);
                assert_eq!((0 as $T).lcm(&1), 0 as $T);
                assert_eq!((1 as $T).lcm(&1), 1 as $T);
                assert_eq!((8 as $T).lcm(&9), 72 as $T);
                assert_eq!((11 as $T).lcm(&5), 55 as $T);
                assert_eq!((15 as $T).lcm(&17), 255 as $T);
            }

            #[test]
            fn test_gcd_lcm() {
                for i in (0..).take(256) {
                    for j in (0..).take(256) {
                        assert_eq!(i.gcd_lcm(&j), (i.gcd(&j), i.lcm(&j)));
                    }
                }
            }

            #[test]
            fn test_is_multiple_of() {
                assert!((0 as $T).is_multiple_of(&(0 as $T)));
                assert!((6 as $T).is_multiple_of(&(6 as $T)));
                assert!((6 as $T).is_multiple_of(&(3 as $T)));
                assert!((6 as $T).is_multiple_of(&(1 as $T)));

                assert!(!(42 as $T).is_multiple_of(&(5 as $T)));
                assert!(!(5 as $T).is_multiple_of(&(3 as $T)));
                assert!(!(42 as $T).is_multiple_of(&(0 as $T)));
            }

            #[test]
            fn test_even() {
                assert_eq!((0 as $T).is_even(), true);
                assert_eq!((1 as $T).is_even(), false);
                assert_eq!((2 as $T).is_even(), true);
                assert_eq!((3 as $T).is_even(), false);
                assert_eq!((4 as $T).is_even(), true);
            }

            #[test]
            fn test_odd() {
                assert_eq!((0 as $T).is_odd(), false);
                assert_eq!((1 as $T).is_odd(), true);
                assert_eq!((2 as $T).is_odd(), false);
                assert_eq!((3 as $T).is_odd(), true);
                assert_eq!((4 as $T).is_odd(), false);
            }
        }
    };
}

impl_integer_for_usize!(u8, test_integer_u8);
impl_integer_for_usize!(u16, test_integer_u16);
impl_integer_for_usize!(u32, test_integer_u32);
impl_integer_for_usize!(u64, test_integer_u64);
impl_integer_for_usize!(u128, test_integer_u128);
impl_integer_for_usize!(usize, test_integer_usize);

/// An iterator over binomial coefficients.
pub struct IterBinomial<T> {
    a: T,
    n: T,
    k: T,
}

impl<T> IterBinomial<T>
where
    T: Integer,
{
    /// For a given n, iterate over all binomial coefficients binomial(n, k), for k=0...n.
    ///
    /// Note that this might overflow, depending on `T`. For the primitive
    /// integer types, the following n are the largest ones for which there will
    /// be no overflow:
    ///
    /// type | n
    /// -----|---
    /// u8   | 10
    /// i8   |  9
    /// u16  | 18
    /// i16  | 17
    /// u32  | 34
    /// i32  | 33
    /// u64  | 67
    /// i64  | 66
    ///
    /// For larger n, `T` should be a bigint type.
    pub fn new(n: T) -> IterBinomial<T> {
        IterBinomial {
            k: T::zero(),
            a: T::one(),
            n,
        }
    }
}

impl<T> Iterator for IterBinomial<T>
where
    T: Integer + Clone,
{
    type Item = T;

    fn next(&mut self) -> Option<T> {
        if self.k > self.n {
            return None;
        }
        self.a = if !self.k.is_zero() {
            multiply_and_divide(
                self.a.clone(),
                self.n.clone() - self.k.clone() + T::one(),
                self.k.clone(),
            )
        } else {
            T::one()
        };
        self.k = self.k.clone() + T::one();
        Some(self.a.clone())
    }
}

/// Calculate r * a / b, avoiding overflows and fractions.
///
/// Assumes that b divides r * a evenly.
fn multiply_and_divide<T: Integer + Clone>(r: T, a: T, b: T) -> T {
    // See http://blog.plover.com/math/choose-2.html for the idea.
    let g = gcd(r.clone(), b.clone());
    r / g.clone() * (a / (b / g))
}

/// Calculate the binomial coefficient.
///
/// Note that this might overflow, depending on `T`. For the primitive integer
/// types, the following n are the largest ones possible such that there will
/// be no overflow for any k:
///
/// type | n
/// -----|---
/// u8   | 10
/// i8   |  9
/// u16  | 18
/// i16  | 17
/// u32  | 34
/// i32  | 33
/// u64  | 67
/// i64  | 66
///
/// For larger n, consider using a bigint type for `T`.
pub fn binomial<T: Integer + Clone>(mut n: T, k: T) -> T {
    // See http://blog.plover.com/math/choose.html for the idea.
    if k > n {
        return T::zero();
    }
    if k > n.clone() - k.clone() {
        return binomial(n.clone(), n - k);
    }
    let mut r = T::one();
    let mut d = T::one();
    loop {
        if d > k {
            break;
        }
        r = multiply_and_divide(r, n.clone(), d.clone());
        n = n - T::one();
        d = d + T::one();
    }
    r
}

/// Calculate the multinomial coefficient.
pub fn multinomial<T: Integer + Clone>(k: &[T]) -> T
where
    for<'a> T: Add<&'a T, Output = T>,
{
    let mut r = T::one();
    let mut p = T::zero();
    for i in k {
        p = p + i;
        r = r * binomial(p.clone(), i.clone());
    }
    r
}

#[test]
fn test_lcm_overflow() {
    macro_rules! check {
        ($t:ty, $x:expr, $y:expr, $r:expr) => {{
            let x: $t = $x;
            let y: $t = $y;
            let o = x.checked_mul(y);
            assert!(
                o.is_none(),
                "sanity checking that {} input {} * {} overflows",
                stringify!($t),
                x,
                y
            );
            assert_eq!(x.lcm(&y), $r);
            assert_eq!(y.lcm(&x), $r);
        }};
    }

    // Original bug (Issue #166)
    check!(i64, 46656000000000000, 600, 46656000000000000);

    check!(i8, 0x40, 0x04, 0x40);
    check!(u8, 0x80, 0x02, 0x80);
    check!(i16, 0x40_00, 0x04, 0x40_00);
    check!(u16, 0x80_00, 0x02, 0x80_00);
    check!(i32, 0x4000_0000, 0x04, 0x4000_0000);
    check!(u32, 0x8000_0000, 0x02, 0x8000_0000);
    check!(i64, 0x4000_0000_0000_0000, 0x04, 0x4000_0000_0000_0000);
    check!(u64, 0x8000_0000_0000_0000, 0x02, 0x8000_0000_0000_0000);
}

#[test]
fn test_iter_binomial() {
    macro_rules! check_simple {
        ($t:ty) => {{
            let n: $t = 3;
            let expected = [1, 3, 3, 1];
            for (b, &e) in IterBinomial::new(n).zip(&expected) {
                assert_eq!(b, e);
            }
        }};
    }

    check_simple!(u8);
    check_simple!(i8);
    check_simple!(u16);
    check_simple!(i16);
    check_simple!(u32);
    check_simple!(i32);
    check_simple!(u64);
    check_simple!(i64);

    macro_rules! check_binomial {
        ($t:ty, $n:expr) => {{
            let n: $t = $n;
            let mut k: $t = 0;
            for b in IterBinomial::new(n) {
                assert_eq!(b, binomial(n, k));
                k += 1;
            }
        }};
    }

    // Check the largest n for which there is no overflow.
    check_binomial!(u8, 10);
    check_binomial!(i8, 9);
    check_binomial!(u16, 18);
    check_binomial!(i16, 17);
    check_binomial!(u32, 34);
    check_binomial!(i32, 33);
    check_binomial!(u64, 67);
    check_binomial!(i64, 66);
}

#[test]
fn test_binomial() {
    macro_rules! check {
        ($t:ty, $x:expr, $y:expr, $r:expr) => {{
            let x: $t = $x;
            let y: $t = $y;
            let expected: $t = $r;
            assert_eq!(binomial(x, y), expected);
            if y <= x {
                assert_eq!(binomial(x, x - y), expected);
            }
        }};
    }
    check!(u8, 9, 4, 126);
    check!(u8, 0, 0, 1);
    check!(u8, 2, 3, 0);

    check!(i8, 9, 4, 126);
    check!(i8, 0, 0, 1);
    check!(i8, 2, 3, 0);

    check!(u16, 100, 2, 4950);
    check!(u16, 14, 4, 1001);
    check!(u16, 0, 0, 1);
    check!(u16, 2, 3, 0);

    check!(i16, 100, 2, 4950);
    check!(i16, 14, 4, 1001);
    check!(i16, 0, 0, 1);
    check!(i16, 2, 3, 0);

    check!(u32, 100, 2, 4950);
    check!(u32, 35, 11, 417225900);
    check!(u32, 14, 4, 1001);
    check!(u32, 0, 0, 1);
    check!(u32, 2, 3, 0);

    check!(i32, 100, 2, 4950);
    check!(i32, 35, 11, 417225900);
    check!(i32, 14, 4, 1001);
    check!(i32, 0, 0, 1);
    check!(i32, 2, 3, 0);

    check!(u64, 100, 2, 4950);
    check!(u64, 35, 11, 417225900);
    check!(u64, 14, 4, 1001);
    check!(u64, 0, 0, 1);
    check!(u64, 2, 3, 0);

    check!(i64, 100, 2, 4950);
    check!(i64, 35, 11, 417225900);
    check!(i64, 14, 4, 1001);
    check!(i64, 0, 0, 1);
    check!(i64, 2, 3, 0);
}

#[test]
fn test_multinomial() {
    macro_rules! check_binomial {
        ($t:ty, $k:expr) => {{
            let n: $t = $k.iter().fold(0, |acc, &x| acc + x);
            let k: &[$t] = $k;
            assert_eq!(k.len(), 2);
            assert_eq!(multinomial(k), binomial(n, k[0]));
        }};
    }

    check_binomial!(u8, &[4, 5]);

    check_binomial!(i8, &[4, 5]);

    check_binomial!(u16, &[2, 98]);
    check_binomial!(u16, &[4, 10]);

    check_binomial!(i16, &[2, 98]);
    check_binomial!(i16, &[4, 10]);

    check_binomial!(u32, &[2, 98]);
    check_binomial!(u32, &[11, 24]);
    check_binomial!(u32, &[4, 10]);

    check_binomial!(i32, &[2, 98]);
    check_binomial!(i32, &[11, 24]);
    check_binomial!(i32, &[4, 10]);

    check_binomial!(u64, &[2, 98]);
    check_binomial!(u64, &[11, 24]);
    check_binomial!(u64, &[4, 10]);

    check_binomial!(i64, &[2, 98]);
    check_binomial!(i64, &[11, 24]);
    check_binomial!(i64, &[4, 10]);

    macro_rules! check_multinomial {
        ($t:ty, $k:expr, $r:expr) => {{
            let k: &[$t] = $k;
            let expected: $t = $r;
            assert_eq!(multinomial(k), expected);
        }};
    }

    check_multinomial!(u8, &[2, 1, 2], 30);
    check_multinomial!(u8, &[2, 3, 0], 10);

    check_multinomial!(i8, &[2, 1, 2], 30);
    check_multinomial!(i8, &[2, 3, 0], 10);

    check_multinomial!(u16, &[2, 1, 2], 30);
    check_multinomial!(u16, &[2, 3, 0], 10);

    check_multinomial!(i16, &[2, 1, 2], 30);
    check_multinomial!(i16, &[2, 3, 0], 10);

    check_multinomial!(u32, &[2, 1, 2], 30);
    check_multinomial!(u32, &[2, 3, 0], 10);

    check_multinomial!(i32, &[2, 1, 2], 30);
    check_multinomial!(i32, &[2, 3, 0], 10);

    check_multinomial!(u64, &[2, 1, 2], 30);
    check_multinomial!(u64, &[2, 3, 0], 10);

    check_multinomial!(i64, &[2, 1, 2], 30);
    check_multinomial!(i64, &[2, 3, 0], 10);

    check_multinomial!(u64, &[], 1);
    check_multinomial!(u64, &[0], 1);
    check_multinomial!(u64, &[12345], 1);
}
