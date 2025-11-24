use core::cmp::Ordering;
use core::num::FpCategory;
use core::ops::{Add, Div, Neg};

use core::f32;
use core::f64;

use crate::{Num, NumCast, ToPrimitive};

/// Generic trait for floating point numbers that works with `no_std`.
///
/// This trait implements a subset of the `Float` trait.
pub trait FloatCore: Num + NumCast + Neg<Output = Self> + PartialOrd + Copy {
    /// Returns positive infinity.
    ///
    /// # Examples
    ///
    /// ```
    /// use num_traits::float::FloatCore;
    /// use std::{f32, f64};
    ///
    /// fn check<T: FloatCore>(x: T) {
    ///     assert!(T::infinity() == x);
    /// }
    ///
    /// check(f32::INFINITY);
    /// check(f64::INFINITY);
    /// ```
    fn infinity() -> Self;

    /// Returns negative infinity.
    ///
    /// # Examples
    ///
    /// ```
    /// use num_traits::float::FloatCore;
    /// use std::{f32, f64};
    ///
    /// fn check<T: FloatCore>(x: T) {
    ///     assert!(T::neg_infinity() == x);
    /// }
    ///
    /// check(f32::NEG_INFINITY);
    /// check(f64::NEG_INFINITY);
    /// ```
    fn neg_infinity() -> Self;

    /// Returns NaN.
    ///
    /// # Examples
    ///
    /// ```
    /// use num_traits::float::FloatCore;
    ///
    /// fn check<T: FloatCore>() {
    ///     let n = T::nan();
    ///     assert!(n != n);
    /// }
    ///
    /// check::<f32>();
    /// check::<f64>();
    /// ```
    fn nan() -> Self;

    /// Returns `-0.0`.
    ///
    /// # Examples
    ///
    /// ```
    /// use num_traits::float::FloatCore;
    /// use std::{f32, f64};
    ///
    /// fn check<T: FloatCore>(n: T) {
    ///     let z = T::neg_zero();
    ///     assert!(z.is_zero());
    ///     assert!(T::one() / z == n);
    /// }
    ///
    /// check(f32::NEG_INFINITY);
    /// check(f64::NEG_INFINITY);
    /// ```
    fn neg_zero() -> Self;

    /// Returns the smallest finite value that this type can represent.
    ///
    /// # Examples
    ///
    /// ```
    /// use num_traits::float::FloatCore;
    /// use std::{f32, f64};
    ///
    /// fn check<T: FloatCore>(x: T) {
    ///     assert!(T::min_value() == x);
    /// }
    ///
    /// check(f32::MIN);
    /// check(f64::MIN);
    /// ```
    fn min_value() -> Self;

    /// Returns the smallest positive, normalized value that this type can represent.
    ///
    /// # Examples
    ///
    /// ```
    /// use num_traits::float::FloatCore;
    /// use std::{f32, f64};
    ///
    /// fn check<T: FloatCore>(x: T) {
    ///     assert!(T::min_positive_value() == x);
    /// }
    ///
    /// check(f32::MIN_POSITIVE);
    /// check(f64::MIN_POSITIVE);
    /// ```
    fn min_positive_value() -> Self;

    /// Returns epsilon, a small positive value.
    ///
    /// # Examples
    ///
    /// ```
    /// use num_traits::float::FloatCore;
    /// use std::{f32, f64};
    ///
    /// fn check<T: FloatCore>(x: T) {
    ///     assert!(T::epsilon() == x);
    /// }
    ///
    /// check(f32::EPSILON);
    /// check(f64::EPSILON);
    /// ```
    fn epsilon() -> Self;

    /// Returns the largest finite value that this type can represent.
    ///
    /// # Examples
    ///
    /// ```
    /// use num_traits::float::FloatCore;
    /// use std::{f32, f64};
    ///
    /// fn check<T: FloatCore>(x: T) {
    ///     assert!(T::max_value() == x);
    /// }
    ///
    /// check(f32::MAX);
    /// check(f64::MAX);
    /// ```
    fn max_value() -> Self;

    /// Returns `true` if the number is NaN.
    ///
    /// # Examples
    ///
    /// ```
    /// use num_traits::float::FloatCore;
    /// use std::{f32, f64};
    ///
    /// fn check<T: FloatCore>(x: T, p: bool) {
    ///     assert!(x.is_nan() == p);
    /// }
    ///
    /// check(f32::NAN, true);
    /// check(f32::INFINITY, false);
    /// check(f64::NAN, true);
    /// check(0.0f64, false);
    /// ```
    #[inline]
    #[allow(clippy::eq_op)]
    fn is_nan(self) -> bool {
        self != self
    }

    /// Returns `true` if the number is infinite.
    ///
    /// # Examples
    ///
    /// ```
    /// use num_traits::float::FloatCore;
    /// use std::{f32, f64};
    ///
    /// fn check<T: FloatCore>(x: T, p: bool) {
    ///     assert!(x.is_infinite() == p);
    /// }
    ///
    /// check(f32::INFINITY, true);
    /// check(f32::NEG_INFINITY, true);
    /// check(f32::NAN, false);
    /// check(f64::INFINITY, true);
    /// check(f64::NEG_INFINITY, true);
    /// check(0.0f64, false);
    /// ```
    #[inline]
    fn is_infinite(self) -> bool {
        self == Self::infinity() || self == Self::neg_infinity()
    }

    /// Returns `true` if the number is neither infinite or NaN.
    ///
    /// # Examples
    ///
    /// ```
    /// use num_traits::float::FloatCore;
    /// use std::{f32, f64};
    ///
    /// fn check<T: FloatCore>(x: T, p: bool) {
    ///     assert!(x.is_finite() == p);
    /// }
    ///
    /// check(f32::INFINITY, false);
    /// check(f32::MAX, true);
    /// check(f64::NEG_INFINITY, false);
    /// check(f64::MIN_POSITIVE, true);
    /// check(f64::NAN, false);
    /// ```
    #[inline]
    fn is_finite(self) -> bool {
        !(self.is_nan() || self.is_infinite())
    }

    /// Returns `true` if the number is neither zero, infinite, subnormal or NaN.
    ///
    /// # Examples
    ///
    /// ```
    /// use num_traits::float::FloatCore;
    /// use std::{f32, f64};
    ///
    /// fn check<T: FloatCore>(x: T, p: bool) {
    ///     assert!(x.is_normal() == p);
    /// }
    ///
    /// check(f32::INFINITY, false);
    /// check(f32::MAX, true);
    /// check(f64::NEG_INFINITY, false);
    /// check(f64::MIN_POSITIVE, true);
    /// check(0.0f64, false);
    /// ```
    #[inline]
    fn is_normal(self) -> bool {
        self.classify() == FpCategory::Normal
    }

    /// Returns `true` if the number is [subnormal].
    ///
    /// ```
    /// use num_traits::float::FloatCore;
    /// use std::f64;
    ///
    /// let min = f64::MIN_POSITIVE; // 2.2250738585072014e-308_f64
    /// let max = f64::MAX;
    /// let lower_than_min = 1.0e-308_f64;
    /// let zero = 0.0_f64;
    ///
    /// assert!(!min.is_subnormal());
    /// assert!(!max.is_subnormal());
    ///
    /// assert!(!zero.is_subnormal());
    /// assert!(!f64::NAN.is_subnormal());
    /// assert!(!f64::INFINITY.is_subnormal());
    /// // Values between `0` and `min` are Subnormal.
    /// assert!(lower_than_min.is_subnormal());
    /// ```
    /// [subnormal]: https://en.wikipedia.org/wiki/Subnormal_number
    #[inline]
    fn is_subnormal(self) -> bool {
        self.classify() == FpCategory::Subnormal
    }

    /// Returns the floating point category of the number. If only one property
    /// is going to be tested, it is generally faster to use the specific
    /// predicate instead.
    ///
    /// # Examples
    ///
    /// ```
    /// use num_traits::float::FloatCore;
    /// use std::{f32, f64};
    /// use std::num::FpCategory;
    ///
    /// fn check<T: FloatCore>(x: T, c: FpCategory) {
    ///     assert!(x.classify() == c);
    /// }
    ///
    /// check(f32::INFINITY, FpCategory::Infinite);
    /// check(f32::MAX, FpCategory::Normal);
    /// check(f64::NAN, FpCategory::Nan);
    /// check(f64::MIN_POSITIVE, FpCategory::Normal);
    /// check(f64::MIN_POSITIVE / 2.0, FpCategory::Subnormal);
    /// check(0.0f64, FpCategory::Zero);
    /// ```
    fn classify(self) -> FpCategory;

    /// Returns the largest integer less than or equal to a number.
    ///
    /// # Examples
    ///
    /// ```
    /// use num_traits::float::FloatCore;
    /// use std::{f32, f64};
    ///
    /// fn check<T: FloatCore>(x: T, y: T) {
    ///     assert!(x.floor() == y);
    /// }
    ///
    /// check(f32::INFINITY, f32::INFINITY);
    /// check(0.9f32, 0.0);
    /// check(1.0f32, 1.0);
    /// check(1.1f32, 1.0);
    /// check(-0.0f64, 0.0);
    /// check(-0.9f64, -1.0);
    /// check(-1.0f64, -1.0);
    /// check(-1.1f64, -2.0);
    /// check(f64::MIN, f64::MIN);
    /// ```
    #[inline]
    fn floor(self) -> Self {
        let f = self.fract();
        if f.is_nan() || f.is_zero() {
            self
        } else if self < Self::zero() {
            self - f - Self::one()
        } else {
            self - f
        }
    }

    /// Returns the smallest integer greater than or equal to a number.
    ///
    /// # Examples
    ///
    /// ```
    /// use num_traits::float::FloatCore;
    /// use std::{f32, f64};
    ///
    /// fn check<T: FloatCore>(x: T, y: T) {
    ///     assert!(x.ceil() == y);
    /// }
    ///
    /// check(f32::INFINITY, f32::INFINITY);
    /// check(0.9f32, 1.0);
    /// check(1.0f32, 1.0);
    /// check(1.1f32, 2.0);
    /// check(-0.0f64, 0.0);
    /// check(-0.9f64, -0.0);
    /// check(-1.0f64, -1.0);
    /// check(-1.1f64, -1.0);
    /// check(f64::MIN, f64::MIN);
    /// ```
    #[inline]
    fn ceil(self) -> Self {
        let f = self.fract();
        if f.is_nan() || f.is_zero() {
            self
        } else if self > Self::zero() {
            self - f + Self::one()
        } else {
            self - f
        }
    }

    /// Returns the nearest integer to a number. Round half-way cases away from `0.0`.
    ///
    /// # Examples
    ///
    /// ```
    /// use num_traits::float::FloatCore;
    /// use std::{f32, f64};
    ///
    /// fn check<T: FloatCore>(x: T, y: T) {
    ///     assert!(x.round() == y);
    /// }
    ///
    /// check(f32::INFINITY, f32::INFINITY);
    /// check(0.4f32, 0.0);
    /// check(0.5f32, 1.0);
    /// check(0.6f32, 1.0);
    /// check(-0.4f64, 0.0);
    /// check(-0.5f64, -1.0);
    /// check(-0.6f64, -1.0);
    /// check(f64::MIN, f64::MIN);
    /// ```
    #[inline]
    fn round(self) -> Self {
        let one = Self::one();
        let h = Self::from(0.5).expect("Unable to cast from 0.5");
        let f = self.fract();
        if f.is_nan() || f.is_zero() {
            self
        } else if self > Self::zero() {
            if f < h {
                self - f
            } else {
                self - f + one
            }
        } else if -f < h {
            self - f
        } else {
            self - f - one
        }
    }

    /// Return the integer part of a number.
    ///
    /// # Examples
    ///
    /// ```
    /// use num_traits::float::FloatCore;
    /// use std::{f32, f64};
    ///
    /// fn check<T: FloatCore>(x: T, y: T) {
    ///     assert!(x.trunc() == y);
    /// }
    ///
    /// check(f32::INFINITY, f32::INFINITY);
    /// check(0.9f32, 0.0);
    /// check(1.0f32, 1.0);
    /// check(1.1f32, 1.0);
    /// check(-0.0f64, 0.0);
    /// check(-0.9f64, -0.0);
    /// check(-1.0f64, -1.0);
    /// check(-1.1f64, -1.0);
    /// check(f64::MIN, f64::MIN);
    /// ```
    #[inline]
    fn trunc(self) -> Self {
        let f = self.fract();
        if f.is_nan() {
            self
        } else {
            self - f
        }
    }

    /// Returns the fractional part of a number.
    ///
    /// # Examples
    ///
    /// ```
    /// use num_traits::float::FloatCore;
    /// use std::{f32, f64};
    ///
    /// fn check<T: FloatCore>(x: T, y: T) {
    ///     assert!(x.fract() == y);
    /// }
    ///
    /// check(f32::MAX, 0.0);
    /// check(0.75f32, 0.75);
    /// check(1.0f32, 0.0);
    /// check(1.25f32, 0.25);
    /// check(-0.0f64, 0.0);
    /// check(-0.75f64, -0.75);
    /// check(-1.0f64, 0.0);
    /// check(-1.25f64, -0.25);
    /// check(f64::MIN, 0.0);
    /// ```
    #[inline]
    fn fract(self) -> Self {
        if self.is_zero() {
            Self::zero()
        } else {
            self % Self::one()
        }
    }

    /// Computes the absolute value of `self`. Returns `FloatCore::nan()` if the
    /// number is `FloatCore::nan()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use num_traits::float::FloatCore;
    /// use std::{f32, f64};
    ///
    /// fn check<T: FloatCore>(x: T, y: T) {
    ///     assert!(x.abs() == y);
    /// }
    ///
    /// check(f32::INFINITY, f32::INFINITY);
    /// check(1.0f32, 1.0);
    /// check(0.0f64, 0.0);
    /// check(-0.0f64, 0.0);
    /// check(-1.0f64, 1.0);
    /// check(f64::MIN, f64::MAX);
    /// ```
    #[inline]
    fn abs(self) -> Self {
        if self.is_sign_positive() {
            return self;
        }
        if self.is_sign_negative() {
            return -self;
        }
        Self::nan()
    }

    /// Returns a number that represents the sign of `self`.
    ///
    /// - `1.0` if the number is positive, `+0.0` or `FloatCore::infinity()`
    /// - `-1.0` if the number is negative, `-0.0` or `FloatCore::neg_infinity()`
    /// - `FloatCore::nan()` if the number is `FloatCore::nan()`
    ///
    /// # Examples
    ///
    /// ```
    /// use num_traits::float::FloatCore;
    /// use std::{f32, f64};
    ///
    /// fn check<T: FloatCore>(x: T, y: T) {
    ///     assert!(x.signum() == y);
    /// }
    ///
    /// check(f32::INFINITY, 1.0);
    /// check(3.0f32, 1.0);
    /// check(0.0f32, 1.0);
    /// check(-0.0f64, -1.0);
    /// check(-3.0f64, -1.0);
    /// check(f64::MIN, -1.0);
    /// ```
    #[inline]
    fn signum(self) -> Self {
        if self.is_nan() {
            Self::nan()
        } else if self.is_sign_negative() {
            -Self::one()
        } else {
            Self::one()
        }
    }

    /// Returns `true` if `self` is positive, including `+0.0` and
    /// `FloatCore::infinity()`, and `FloatCore::nan()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use num_traits::float::FloatCore;
    /// use std::{f32, f64};
    ///
    /// fn check<T: FloatCore>(x: T, p: bool) {
    ///     assert!(x.is_sign_positive() == p);
    /// }
    ///
    /// check(f32::INFINITY, true);
    /// check(f32::MAX, true);
    /// check(0.0f32, true);
    /// check(-0.0f64, false);
    /// check(f64::NEG_INFINITY, false);
    /// check(f64::MIN_POSITIVE, true);
    /// check(f64::NAN, true);
    /// check(-f64::NAN, false);
    /// ```
    #[inline]
    fn is_sign_positive(self) -> bool {
        !self.is_sign_negative()
    }

    /// Returns `true` if `self` is negative, including `-0.0` and
    /// `FloatCore::neg_infinity()`, and `-FloatCore::nan()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use num_traits::float::FloatCore;
    /// use std::{f32, f64};
    ///
    /// fn check<T: FloatCore>(x: T, p: bool) {
    ///     assert!(x.is_sign_negative() == p);
    /// }
    ///
    /// check(f32::INFINITY, false);
    /// check(f32::MAX, false);
    /// check(0.0f32, false);
    /// check(-0.0f64, true);
    /// check(f64::NEG_INFINITY, true);
    /// check(f64::MIN_POSITIVE, false);
    /// check(f64::NAN, false);
    /// check(-f64::NAN, true);
    /// ```
    #[inline]
    fn is_sign_negative(self) -> bool {
        let (_, _, sign) = self.integer_decode();
        sign < 0
    }

    /// Returns the minimum of the two numbers.
    ///
    /// If one of the arguments is NaN, then the other argument is returned.
    ///
    /// # Examples
    ///
    /// ```
    /// use num_traits::float::FloatCore;
    /// use std::{f32, f64};
    ///
    /// fn check<T: FloatCore>(x: T, y: T, min: T) {
    ///     assert!(x.min(y) == min);
    /// }
    ///
    /// check(1.0f32, 2.0, 1.0);
    /// check(f32::NAN, 2.0, 2.0);
    /// check(1.0f64, -2.0, -2.0);
    /// check(1.0f64, f64::NAN, 1.0);
    /// ```
    #[inline]
    fn min(self, other: Self) -> Self {
        if self.is_nan() {
            return other;
        }
        if other.is_nan() {
            return self;
        }
        if self < other {
            self
        } else {
            other
        }
    }

    /// Returns the maximum of the two numbers.
    ///
    /// If one of the arguments is NaN, then the other argument is returned.
    ///
    /// # Examples
    ///
    /// ```
    /// use num_traits::float::FloatCore;
    /// use std::{f32, f64};
    ///
    /// fn check<T: FloatCore>(x: T, y: T, max: T) {
    ///     assert!(x.max(y) == max);
    /// }
    ///
    /// check(1.0f32, 2.0, 2.0);
    /// check(1.0f32, f32::NAN, 1.0);
    /// check(-1.0f64, 2.0, 2.0);
    /// check(-1.0f64, f64::NAN, -1.0);
    /// ```
    #[inline]
    fn max(self, other: Self) -> Self {
        if self.is_nan() {
            return other;
        }
        if other.is_nan() {
            return self;
        }
        if self > other {
            self
        } else {
            other
        }
    }

    /// A value bounded by a minimum and a maximum
    ///
    ///  If input is less than min then this returns min.
    ///  If input is greater than max then this returns max.
    ///  Otherwise this returns input.
    ///
    /// **Panics** in debug mode if `!(min <= max)`.
    ///
    /// # Examples
    ///
    /// ```
    /// use num_traits::float::FloatCore;
    ///
    /// fn check<T: FloatCore>(val: T, min: T, max: T, expected: T) {
    ///     assert!(val.clamp(min, max) == expected);
    /// }
    ///
    ///
    /// check(1.0f32, 0.0, 2.0, 1.0);
    /// check(1.0f32, 2.0, 3.0, 2.0);
    /// check(3.0f32, 0.0, 2.0, 2.0);
    ///
    /// check(1.0f64, 0.0, 2.0, 1.0);
    /// check(1.0f64, 2.0, 3.0, 2.0);
    /// check(3.0f64, 0.0, 2.0, 2.0);
    /// ```
    fn clamp(self, min: Self, max: Self) -> Self {
        crate::clamp(self, min, max)
    }

    /// Returns the reciprocal (multiplicative inverse) of the number.
    ///
    /// # Examples
    ///
    /// ```
    /// use num_traits::float::FloatCore;
    /// use std::{f32, f64};
    ///
    /// fn check<T: FloatCore>(x: T, y: T) {
    ///     assert!(x.recip() == y);
    ///     assert!(y.recip() == x);
    /// }
    ///
    /// check(f32::INFINITY, 0.0);
    /// check(2.0f32, 0.5);
    /// check(-0.25f64, -4.0);
    /// check(-0.0f64, f64::NEG_INFINITY);
    /// ```
    #[inline]
    fn recip(self) -> Self {
        Self::one() / self
    }

    /// Raise a number to an integer power.
    ///
    /// Using this function is generally faster than using `powf`
    ///
    /// # Examples
    ///
    /// ```
    /// use num_traits::float::FloatCore;
    ///
    /// fn check<T: FloatCore>(x: T, exp: i32, powi: T) {
    ///     assert!(x.powi(exp) == powi);
    /// }
    ///
    /// check(9.0f32, 2, 81.0);
    /// check(1.0f32, -2, 1.0);
    /// check(10.0f64, 20, 1e20);
    /// check(4.0f64, -2, 0.0625);
    /// check(-1.0f64, std::i32::MIN, 1.0);
    /// ```
    #[inline]
    fn powi(mut self, mut exp: i32) -> Self {
        if exp < 0 {
            exp = exp.wrapping_neg();
            self = self.recip();
        }
        // It should always be possible to convert a positive `i32` to a `usize`.
        // Note, `i32::MIN` will wrap and still be negative, so we need to convert
        // to `u32` without sign-extension before growing to `usize`.
        super::pow(self, (exp as u32).to_usize().unwrap())
    }

    /// Converts to degrees, assuming the number is in radians.
    ///
    /// # Examples
    ///
    /// ```
    /// use num_traits::float::FloatCore;
    /// use std::{f32, f64};
    ///
    /// fn check<T: FloatCore>(rad: T, deg: T) {
    ///     assert!(rad.to_degrees() == deg);
    /// }
    ///
    /// check(0.0f32, 0.0);
    /// check(f32::consts::PI, 180.0);
    /// check(f64::consts::FRAC_PI_4, 45.0);
    /// check(f64::INFINITY, f64::INFINITY);
    /// ```
    fn to_degrees(self) -> Self;

    /// Converts to radians, assuming the number is in degrees.
    ///
    /// # Examples
    ///
    /// ```
    /// use num_traits::float::FloatCore;
    /// use std::{f32, f64};
    ///
    /// fn check<T: FloatCore>(deg: T, rad: T) {
    ///     assert!(deg.to_radians() == rad);
    /// }
    ///
    /// check(0.0f32, 0.0);
    /// check(180.0, f32::consts::PI);
    /// check(45.0, f64::consts::FRAC_PI_4);
    /// check(f64::INFINITY, f64::INFINITY);
    /// ```
    fn to_radians(self) -> Self;

    /// Returns the mantissa, base 2 exponent, and sign as integers, respectively.
    /// The original number can be recovered by `sign * mantissa * 2 ^ exponent`.
    ///
    /// # Examples
    ///
    /// ```
    /// use num_traits::float::FloatCore;
    /// use std::{f32, f64};
    ///
    /// fn check<T: FloatCore>(x: T, m: u64, e: i16, s:i8) {
    ///     let (mantissa, exponent, sign) = x.integer_decode();
    ///     assert_eq!(mantissa, m);
    ///     assert_eq!(exponent, e);
    ///     assert_eq!(sign, s);
    /// }
    ///
    /// check(2.0f32, 1 << 23, -22, 1);
    /// check(-2.0f32, 1 << 23, -22, -1);
    /// check(f32::INFINITY, 1 << 23, 105, 1);
    /// check(f64::NEG_INFINITY, 1 << 52, 972, -1);
    /// ```
    fn integer_decode(self) -> (u64, i16, i8);
}

impl FloatCore for f32 {
    constant! {
        infinity() -> f32::INFINITY;
        neg_infinity() -> f32::NEG_INFINITY;
        nan() -> f32::NAN;
        neg_zero() -> -0.0;
        min_value() -> f32::MIN;
        min_positive_value() -> f32::MIN_POSITIVE;
        epsilon() -> f32::EPSILON;
        max_value() -> f32::MAX;
    }

    #[inline]
    fn integer_decode(self) -> (u64, i16, i8) {
        integer_decode_f32(self)
    }

    forward! {
        Self::is_nan(self) -> bool;
        Self::is_infinite(self) -> bool;
        Self::is_finite(self) -> bool;
        Self::is_normal(self) -> bool;
        Self::is_subnormal(self) -> bool;
        Self::clamp(self, min: Self, max: Self) -> Self;
        Self::classify(self) -> FpCategory;
        Self::is_sign_positive(self) -> bool;
        Self::is_sign_negative(self) -> bool;
        Self::min(self, other: Self) -> Self;
        Self::max(self, other: Self) -> Self;
        Self::recip(self) -> Self;
        Self::to_degrees(self) -> Self;
        Self::to_radians(self) -> Self;
    }

    #[cfg(feature = "std")]
    forward! {
        Self::floor(self) -> Self;
        Self::ceil(self) -> Self;
        Self::round(self) -> Self;
        Self::trunc(self) -> Self;
        Self::fract(self) -> Self;
        Self::abs(self) -> Self;
        Self::signum(self) -> Self;
        Self::powi(self, n: i32) -> Self;
    }

    #[cfg(all(not(feature = "std"), feature = "libm"))]
    forward! {
        libm::floorf as floor(self) -> Self;
        libm::ceilf as ceil(self) -> Self;
        libm::roundf as round(self) -> Self;
        libm::truncf as trunc(self) -> Self;
        libm::fabsf as abs(self) -> Self;
    }

    #[cfg(all(not(feature = "std"), feature = "libm"))]
    #[inline]
    fn fract(self) -> Self {
        self - libm::truncf(self)
    }
}

impl FloatCore for f64 {
    constant! {
        infinity() -> f64::INFINITY;
        neg_infinity() -> f64::NEG_INFINITY;
        nan() -> f64::NAN;
        neg_zero() -> -0.0;
        min_value() -> f64::MIN;
        min_positive_value() -> f64::MIN_POSITIVE;
        epsilon() -> f64::EPSILON;
        max_value() -> f64::MAX;
    }

    #[inline]
    fn integer_decode(self) -> (u64, i16, i8) {
        integer_decode_f64(self)
    }

    forward! {
        Self::is_nan(self) -> bool;
        Self::is_infinite(self) -> bool;
        Self::is_finite(self) -> bool;
        Self::is_normal(self) -> bool;
        Self::is_subnormal(self) -> bool;
        Self::clamp(self, min: Self, max: Self) -> Self;
        Self::classify(self) -> FpCategory;
        Self::is_sign_positive(self) -> bool;
        Self::is_sign_negative(self) -> bool;
        Self::min(self, other: Self) -> Self;
        Self::max(self, other: Self) -> Self;
        Self::recip(self) -> Self;
        Self::to_degrees(self) -> Self;
        Self::to_radians(self) -> Self;
    }

    #[cfg(feature = "std")]
    forward! {
        Self::floor(self) -> Self;
        Self::ceil(self) -> Self;
        Self::round(self) -> Self;
        Self::trunc(self) -> Self;
        Self::fract(self) -> Self;
        Self::abs(self) -> Self;
        Self::signum(self) -> Self;
        Self::powi(self, n: i32) -> Self;
    }

    #[cfg(all(not(feature = "std"), feature = "libm"))]
    forward! {
        libm::floor as floor(self) -> Self;
        libm::ceil as ceil(self) -> Self;
        libm::round as round(self) -> Self;
        libm::trunc as trunc(self) -> Self;
        libm::fabs as abs(self) -> Self;
    }

    #[cfg(all(not(feature = "std"), feature = "libm"))]
    #[inline]
    fn fract(self) -> Self {
        self - libm::trunc(self)
    }
}

// FIXME: these doctests aren't actually helpful, because they're using and
// testing the inherent methods directly, not going through `Float`.

/// Generic trait for floating point numbers
///
/// This trait is only available with the `std` feature, or with the `libm` feature otherwise.
#[cfg(any(feature = "std", feature = "libm"))]
pub trait Float: Num + Copy + NumCast + PartialOrd + Neg<Output = Self> {
    /// Returns the `NaN` value.
    ///
    /// ```
    /// use num_traits::Float;
    ///
    /// let nan: f32 = Float::nan();
    ///
    /// assert!(nan.is_nan());
    /// ```
    fn nan() -> Self;
    /// Returns the infinite value.
    ///
    /// ```
    /// use num_traits::Float;
    /// use std::f32;
    ///
    /// let infinity: f32 = Float::infinity();
    ///
    /// assert!(infinity.is_infinite());
    /// assert!(!infinity.is_finite());
    /// assert!(infinity > f32::MAX);
    /// ```
    fn infinity() -> Self;
    /// Returns the negative infinite value.
    ///
    /// ```
    /// use num_traits::Float;
    /// use std::f32;
    ///
    /// let neg_infinity: f32 = Float::neg_infinity();
    ///
    /// assert!(neg_infinity.is_infinite());
    /// assert!(!neg_infinity.is_finite());
    /// assert!(neg_infinity < f32::MIN);
    /// ```
    fn neg_infinity() -> Self;
    /// Returns `-0.0`.
    ///
    /// ```
    /// use num_traits::{Zero, Float};
    ///
    /// let inf: f32 = Float::infinity();
    /// let zero: f32 = Zero::zero();
    /// let neg_zero: f32 = Float::neg_zero();
    ///
    /// assert_eq!(zero, neg_zero);
    /// assert_eq!(7.0f32/inf, zero);
    /// assert_eq!(zero * 10.0, zero);
    /// ```
    fn neg_zero() -> Self;

    /// Returns the smallest finite value that this type can represent.
    ///
    /// ```
    /// use num_traits::Float;
    /// use std::f64;
    ///
    /// let x: f64 = Float::min_value();
    ///
    /// assert_eq!(x, f64::MIN);
    /// ```
    fn min_value() -> Self;

    /// Returns the smallest positive, normalized value that this type can represent.
    ///
    /// ```
    /// use num_traits::Float;
    /// use std::f64;
    ///
    /// let x: f64 = Float::min_positive_value();
    ///
    /// assert_eq!(x, f64::MIN_POSITIVE);
    /// ```
    fn min_positive_value() -> Self;

    /// Returns epsilon, a small positive value.
    ///
    /// ```
    /// use num_traits::Float;
    /// use std::f64;
    ///
    /// let x: f64 = Float::epsilon();
    ///
    /// assert_eq!(x, f64::EPSILON);
    /// ```
    ///
    /// # Panics
    ///
    /// The default implementation will panic if `f32::EPSILON` cannot
    /// be cast to `Self`.
    fn epsilon() -> Self {
        Self::from(f32::EPSILON).expect("Unable to cast from f32::EPSILON")
    }

    /// Returns the largest finite value that this type can represent.
    ///
    /// ```
    /// use num_traits::Float;
    /// use std::f64;
    ///
    /// let x: f64 = Float::max_value();
    /// assert_eq!(x, f64::MAX);
    /// ```
    fn max_value() -> Self;

    /// Returns `true` if this value is `NaN` and false otherwise.
    ///
    /// ```
    /// use num_traits::Float;
    /// use std::f64;
    ///
    /// let nan = f64::NAN;
    /// let f = 7.0;
    ///
    /// assert!(nan.is_nan());
    /// assert!(!f.is_nan());
    /// ```
    fn is_nan(self) -> bool;

    /// Returns `true` if this value is positive infinity or negative infinity and
    /// false otherwise.
    ///
    /// ```
    /// use num_traits::Float;
    /// use std::f32;
    ///
    /// let f = 7.0f32;
    /// let inf: f32 = Float::infinity();
    /// let neg_inf: f32 = Float::neg_infinity();
    /// let nan: f32 = f32::NAN;
    ///
    /// assert!(!f.is_infinite());
    /// assert!(!nan.is_infinite());
    ///
    /// assert!(inf.is_infinite());
    /// assert!(neg_inf.is_infinite());
    /// ```
    fn is_infinite(self) -> bool;

    /// Returns `true` if this number is neither infinite nor `NaN`.
    ///
    /// ```
    /// use num_traits::Float;
    /// use std::f32;
    ///
    /// let f = 7.0f32;
    /// let inf: f32 = Float::infinity();
    /// let neg_inf: f32 = Float::neg_infinity();
    /// let nan: f32 = f32::NAN;
    ///
    /// assert!(f.is_finite());
    ///
    /// assert!(!nan.is_finite());
    /// assert!(!inf.is_finite());
    /// assert!(!neg_inf.is_finite());
    /// ```
    fn is_finite(self) -> bool;

    /// Returns `true` if the number is neither zero, infinite,
    /// [subnormal][subnormal], or `NaN`.
    ///
    /// ```
    /// use num_traits::Float;
    /// use std::f32;
    ///
    /// let min = f32::MIN_POSITIVE; // 1.17549435e-38f32
    /// let max = f32::MAX;
    /// let lower_than_min = 1.0e-40_f32;
    /// let zero = 0.0f32;
    ///
    /// assert!(min.is_normal());
    /// assert!(max.is_normal());
    ///
    /// assert!(!zero.is_normal());
    /// assert!(!f32::NAN.is_normal());
    /// assert!(!f32::INFINITY.is_normal());
    /// // Values between `0` and `min` are Subnormal.
    /// assert!(!lower_than_min.is_normal());
    /// ```
    /// [subnormal]: http://en.wikipedia.org/wiki/Subnormal_number
    fn is_normal(self) -> bool;

    /// Returns `true` if the number is [subnormal].
    ///
    /// ```
    /// use num_traits::Float;
    /// use std::f64;
    ///
    /// let min = f64::MIN_POSITIVE; // 2.2250738585072014e-308_f64
    /// let max = f64::MAX;
    /// let lower_than_min = 1.0e-308_f64;
    /// let zero = 0.0_f64;
    ///
    /// assert!(!min.is_subnormal());
    /// assert!(!max.is_subnormal());
    ///
    /// assert!(!zero.is_subnormal());
    /// assert!(!f64::NAN.is_subnormal());
    /// assert!(!f64::INFINITY.is_subnormal());
    /// // Values between `0` and `min` are Subnormal.
    /// assert!(lower_than_min.is_subnormal());
    /// ```
    /// [subnormal]: https://en.wikipedia.org/wiki/Subnormal_number
    #[inline]
    fn is_subnormal(self) -> bool {
        self.classify() == FpCategory::Subnormal
    }

    /// Returns the floating point category of the number. If only one property
    /// is going to be tested, it is generally faster to use the specific
    /// predicate instead.
    ///
    /// ```
    /// use num_traits::Float;
    /// use std::num::FpCategory;
    /// use std::f32;
    ///
    /// let num = 12.4f32;
    /// let inf = f32::INFINITY;
    ///
    /// assert_eq!(num.classify(), FpCategory::Normal);
    /// assert_eq!(inf.classify(), FpCategory::Infinite);
    /// ```
    fn classify(self) -> FpCategory;

    /// Returns the largest integer less than or equal to a number.
    ///
    /// ```
    /// use num_traits::Float;
    ///
    /// let f = 3.99;
    /// let g = 3.0;
    ///
    /// assert_eq!(f.floor(), 3.0);
    /// assert_eq!(g.floor(), 3.0);
    /// ```
    fn floor(self) -> Self;

    /// Returns the smallest integer greater than or equal to a number.
    ///
    /// ```
    /// use num_traits::Float;
    ///
    /// let f = 3.01;
    /// let g = 4.0;
    ///
    /// assert_eq!(f.ceil(), 4.0);
    /// assert_eq!(g.ceil(), 4.0);
    /// ```
    fn ceil(self) -> Self;

    /// Returns the nearest integer to a number. Round half-way cases away from
    /// `0.0`.
    ///
    /// ```
    /// use num_traits::Float;
    ///
    /// let f = 3.3;
    /// let g = -3.3;
    ///
    /// assert_eq!(f.round(), 3.0);
    /// assert_eq!(g.round(), -3.0);
    /// ```
    fn round(self) -> Self;

    /// Return the integer part of a number.
    ///
    /// ```
    /// use num_traits::Float;
    ///
    /// let f = 3.3;
    /// let g = -3.7;
    ///
    /// assert_eq!(f.trunc(), 3.0);
    /// assert_eq!(g.trunc(), -3.0);
    /// ```
    fn trunc(self) -> Self;

    /// Returns the fractional part of a number.
    ///
    /// ```
    /// use num_traits::Float;
    ///
    /// let x = 3.5;
    /// let y = -3.5;
    /// let abs_difference_x = (x.fract() - 0.5).abs();
    /// let abs_difference_y = (y.fract() - (-0.5)).abs();
    ///
    /// assert!(abs_difference_x < 1e-10);
    /// assert!(abs_difference_y < 1e-10);
    /// ```
    fn fract(self) -> Self;

    /// Computes the absolute value of `self`. Returns `Float::nan()` if the
    /// number is `Float::nan()`.
    ///
    /// ```
    /// use num_traits::Float;
    /// use std::f64;
    ///
    /// let x = 3.5;
    /// let y = -3.5;
    ///
    /// let abs_difference_x = (x.abs() - x).abs();
    /// let abs_difference_y = (y.abs() - (-y)).abs();
    ///
    /// assert!(abs_difference_x < 1e-10);
    /// assert!(abs_difference_y < 1e-10);
    ///
    /// assert!(f64::NAN.abs().is_nan());
    /// ```
    fn abs(self) -> Self;

    /// Returns a number that represents the sign of `self`.
    ///
    /// - `1.0` if the number is positive, `+0.0` or `Float::infinity()`
    /// - `-1.0` if the number is negative, `-0.0` or `Float::neg_infinity()`
    /// - `Float::nan()` if the number is `Float::nan()`
    ///
    /// ```
    /// use num_traits::Float;
    /// use std::f64;
    ///
    /// let f = 3.5;
    ///
    /// assert_eq!(f.signum(), 1.0);
    /// assert_eq!(f64::NEG_INFINITY.signum(), -1.0);
    ///
    /// assert!(f64::NAN.signum().is_nan());
    /// ```
    fn signum(self) -> Self;

    /// Returns `true` if `self` is positive, including `+0.0`,
    /// `Float::infinity()`, and `Float::nan()`.
    ///
    /// ```
    /// use num_traits::Float;
    /// use std::f64;
    ///
    /// let nan: f64 = f64::NAN;
    /// let neg_nan: f64 = -f64::NAN;
    ///
    /// let f = 7.0;
    /// let g = -7.0;
    ///
    /// assert!(f.is_sign_positive());
    /// assert!(!g.is_sign_positive());
    /// assert!(nan.is_sign_positive());
    /// assert!(!neg_nan.is_sign_positive());
    /// ```
    fn is_sign_positive(self) -> bool;

    /// Returns `true` if `self` is negative, including `-0.0`,
    /// `Float::neg_infinity()`, and `-Float::nan()`.
    ///
    /// ```
    /// use num_traits::Float;
    /// use std::f64;
    ///
    /// let nan: f64 = f64::NAN;
    /// let neg_nan: f64 = -f64::NAN;
    ///
    /// let f = 7.0;
    /// let g = -7.0;
    ///
    /// assert!(!f.is_sign_negative());
    /// assert!(g.is_sign_negative());
    /// assert!(!nan.is_sign_negative());
    /// assert!(neg_nan.is_sign_negative());
    /// ```
    fn is_sign_negative(self) -> bool;

    /// Fused multiply-add. Computes `(self * a) + b` with only one rounding
    /// error, yielding a more accurate result than an unfused multiply-add.
    ///
    /// Using `mul_add` can be more performant than an unfused multiply-add if
    /// the target architecture has a dedicated `fma` CPU instruction.
    ///
    /// ```
    /// use num_traits::Float;
    ///
    /// let m = 10.0;
    /// let x = 4.0;
    /// let b = 60.0;
    ///
    /// // 100.0
    /// let abs_difference = (m.mul_add(x, b) - (m*x + b)).abs();
    ///
    /// assert!(abs_difference < 1e-10);
    /// ```
    fn mul_add(self, a: Self, b: Self) -> Self;
    /// Take the reciprocal (inverse) of a number, `1/x`.
    ///
    /// ```
    /// use num_traits::Float;
    ///
    /// let x = 2.0;
    /// let abs_difference = (x.recip() - (1.0/x)).abs();
    ///
    /// assert!(abs_difference < 1e-10);
    /// ```
    fn recip(self) -> Self;

    /// Raise a number to an integer power.
    ///
    /// Using this function is generally faster than using `powf`
    ///
    /// ```
    /// use num_traits::Float;
    ///
    /// let x = 2.0;
    /// let abs_difference = (x.powi(2) - x*x).abs();
    ///
    /// assert!(abs_difference < 1e-10);
    /// ```
    fn powi(self, n: i32) -> Self;

    /// Raise a number to a floating point power.
    ///
    /// ```
    /// use num_traits::Float;
    ///
    /// let x = 2.0;
    /// let abs_difference = (x.powf(2.0) - x*x).abs();
    ///
    /// assert!(abs_difference < 1e-10);
    /// ```
    fn powf(self, n: Self) -> Self;

    /// Take the square root of a number.
    ///
    /// Returns NaN if `self` is a negative number.
    ///
    /// ```
    /// use num_traits::Float;
    ///
    /// let positive = 4.0;
    /// let negative = -4.0;
    ///
    /// let abs_difference = (positive.sqrt() - 2.0).abs();
    ///
    /// assert!(abs_difference < 1e-10);
    /// assert!(negative.sqrt().is_nan());
    /// ```
    fn sqrt(self) -> Self;

    /// Returns `e^(self)`, (the exponential function).
    ///
    /// ```
    /// use num_traits::Float;
    ///
    /// let one = 1.0;
    /// // e^1
    /// let e = one.exp();
    ///
    /// // ln(e) - 1 == 0
    /// let abs_difference = (e.ln() - 1.0).abs();
    ///
    /// assert!(abs_difference < 1e-10);
    /// ```
    fn exp(self) -> Self;

    /// Returns `2^(self)`.
    ///
    /// ```
    /// use num_traits::Float;
    ///
    /// let f = 2.0;
    ///
    /// // 2^2 - 4 == 0
    /// let abs_difference = (f.exp2() - 4.0).abs();
    ///
    /// assert!(abs_difference < 1e-10);
    /// ```
    fn exp2(self) -> Self;

    /// Returns the natural logarithm of the number.
    ///
    /// ```
    /// use num_traits::Float;
    ///
    /// let one = 1.0;
    /// // e^1
    /// let e = one.exp();
    ///
    /// // ln(e) - 1 == 0
    /// let abs_difference = (e.ln() - 1.0).abs();
    ///
    /// assert!(abs_difference < 1e-10);
    /// ```
    fn ln(self) -> Self;

    /// Returns the logarithm of the number with respect to an arbitrary base.
    ///
    /// ```
    /// use num_traits::Float;
    ///
    /// let ten = 10.0;
    /// let two = 2.0;
    ///
    /// // log10(10) - 1 == 0
    /// let abs_difference_10 = (ten.log(10.0) - 1.0).abs();
    ///
    /// // log2(2) - 1 == 0
    /// let abs_difference_2 = (two.log(2.0) - 1.0).abs();
    ///
    /// assert!(abs_difference_10 < 1e-10);
    /// assert!(abs_difference_2 < 1e-10);
    /// ```
    fn log(self, base: Self) -> Self;

    /// Returns the base 2 logarithm of the number.
    ///
    /// ```
    /// use num_traits::Float;
    ///
    /// let two = 2.0;
    ///
    /// // log2(2) - 1 == 0
    /// let abs_difference = (two.log2() - 1.0).abs();
    ///
    /// assert!(abs_difference < 1e-10);
    /// ```
    fn log2(self) -> Self;

    /// Returns the base 10 logarithm of the number.
    ///
    /// ```
    /// use num_traits::Float;
    ///
    /// let ten = 10.0;
    ///
    /// // log10(10) - 1 == 0
    /// let abs_difference = (ten.log10() - 1.0).abs();
    ///
    /// assert!(abs_difference < 1e-10);
    /// ```
    fn log10(self) -> Self;

    /// Converts radians to degrees.
    ///
    /// ```
    /// use std::f64::consts;
    ///
    /// let angle = consts::PI;
    ///
    /// let abs_difference = (angle.to_degrees() - 180.0).abs();
    ///
    /// assert!(abs_difference < 1e-10);
    /// ```
    #[inline]
    fn to_degrees(self) -> Self {
        let halfpi = Self::zero().acos();
        let ninety = Self::from(90u8).unwrap();
        self * ninety / halfpi
    }

    /// Converts degrees to radians.
    ///
    /// ```
    /// use std::f64::consts;
    ///
    /// let angle = 180.0_f64;
    ///
    /// let abs_difference = (angle.to_radians() - consts::PI).abs();
    ///
    /// assert!(abs_difference < 1e-10);
    /// ```
    #[inline]
    fn to_radians(self) -> Self {
        let halfpi = Self::zero().acos();
        let ninety = Self::from(90u8).unwrap();
        self * halfpi / ninety
    }

    /// Returns the maximum of the two numbers.
    ///
    /// ```
    /// use num_traits::Float;
    ///
    /// let x = 1.0;
    /// let y = 2.0;
    ///
    /// assert_eq!(x.max(y), y);
    /// ```
    fn max(self, other: Self) -> Self;

    /// Returns the minimum of the two numbers.
    ///
    /// ```
    /// use num_traits::Float;
    ///
    /// let x = 1.0;
    /// let y = 2.0;
    ///
    /// assert_eq!(x.min(y), x);
    /// ```
    fn min(self, other: Self) -> Self;

    /// Clamps a value between a min and max.
    ///
    /// **Panics** in debug mode if `!(min <= max)`.
    ///
    /// ```
    /// use num_traits::Float;
    ///
    /// let x = 1.0;
    /// let y = 2.0;
    /// let z = 3.0;
    ///
    /// assert_eq!(x.clamp(y, z), 2.0);
    /// ```
    fn clamp(self, min: Self, max: Self) -> Self {
        crate::clamp(self, min, max)
    }

    /// The positive difference of two numbers.
    ///
    /// * If `self <= other`: `0:0`
    /// * Else: `self - other`
    ///
    /// ```
    /// use num_traits::Float;
    ///
    /// let x = 3.0;
    /// let y = -3.0;
    ///
    /// let abs_difference_x = (x.abs_sub(1.0) - 2.0).abs();
    /// let abs_difference_y = (y.abs_sub(1.0) - 0.0).abs();
    ///
    /// assert!(abs_difference_x < 1e-10);
    /// assert!(abs_difference_y < 1e-10);
    /// ```
    fn abs_sub(self, other: Self) -> Self;

    /// Take the cubic root of a number.
    ///
    /// ```
    /// use num_traits::Float;
    ///
    /// let x = 8.0;
    ///
    /// // x^(1/3) - 2 == 0
    /// let abs_difference = (x.cbrt() - 2.0).abs();
    ///
    /// assert!(abs_difference < 1e-10);
    /// ```
    fn cbrt(self) -> Self;

    /// Calculate the length of the hypotenuse of a right-angle triangle given
    /// legs of length `x` and `y`.
    ///
    /// ```
    /// use num_traits::Float;
    ///
    /// let x = 2.0;
    /// let y = 3.0;
    ///
    /// // sqrt(x^2 + y^2)
    /// let abs_difference = (x.hypot(y) - (x.powi(2) + y.powi(2)).sqrt()).abs();
    ///
    /// assert!(abs_difference < 1e-10);
    /// ```
    fn hypot(self, other: Self) -> Self;

    /// Computes the sine of a number (in radians).
    ///
    /// ```
    /// use num_traits::Float;
    /// use std::f64;
    ///
    /// let x = f64::consts::PI/2.0;
    ///
    /// let abs_difference = (x.sin() - 1.0).abs();
    ///
    /// assert!(abs_difference < 1e-10);
    /// ```
    fn sin(self) -> Self;

    /// Computes the cosine of a number (in radians).
    ///
    /// ```
    /// use num_traits::Float;
    /// use std::f64;
    ///
    /// let x = 2.0*f64::consts::PI;
    ///
    /// let abs_difference = (x.cos() - 1.0).abs();
    ///
    /// assert!(abs_difference < 1e-10);
    /// ```
    fn cos(self) -> Self;

    /// Computes the tangent of a number (in radians).
    ///
    /// ```
    /// use num_traits::Float;
    /// use std::f64;
    ///
    /// let x = f64::consts::PI/4.0;
    /// let abs_difference = (x.tan() - 1.0).abs();
    ///
    /// assert!(abs_difference < 1e-14);
    /// ```
    fn tan(self) -> Self;

    /// Computes the arcsine of a number. Return value is in radians in
    /// the range [-pi/2, pi/2] or NaN if the number is outside the range
    /// [-1, 1].
    ///
    /// ```
    /// use num_traits::Float;
    /// use std::f64;
    ///
    /// let f = f64::consts::PI / 2.0;
    ///
    /// // asin(sin(pi/2))
    /// let abs_difference = (f.sin().asin() - f64::consts::PI / 2.0).abs();
    ///
    /// assert!(abs_difference < 1e-10);
    /// ```
    fn asin(self) -> Self;

    /// Computes the arccosine of a number. Return value is in radians in
    /// the range [0, pi] or NaN if the number is outside the range
    /// [-1, 1].
    ///
    /// ```
    /// use num_traits::Float;
    /// use std::f64;
    ///
    /// let f = f64::consts::PI / 4.0;
    ///
    /// // acos(cos(pi/4))
    /// let abs_difference = (f.cos().acos() - f64::consts::PI / 4.0).abs();
    ///
    /// assert!(abs_difference < 1e-10);
    /// ```
    fn acos(self) -> Self;

    /// Computes the arctangent of a number. Return value is in radians in the
    /// range [-pi/2, pi/2];
    ///
    /// ```
    /// use num_traits::Float;
    ///
    /// let f = 1.0;
    ///
    /// // atan(tan(1))
    /// let abs_difference = (f.tan().atan() - 1.0).abs();
    ///
    /// assert!(abs_difference < 1e-10);
    /// ```
    fn atan(self) -> Self;

    /// Computes the four quadrant arctangent of `self` (`y`) and `other` (`x`).
    ///
    /// * `x = 0`, `y = 0`: `0`
    /// * `x >= 0`: `arctan(y/x)` -> `[-pi/2, pi/2]`
    /// * `y >= 0`: `arctan(y/x) + pi` -> `(pi/2, pi]`
    /// * `y < 0`: `arctan(y/x) - pi` -> `(-pi, -pi/2)`
    ///
    /// ```
    /// use num_traits::Float;
    /// use std::f64;
    ///
    /// let pi = f64::consts::PI;
    /// // All angles from horizontal right (+x)
    /// // 45 deg counter-clockwise
    /// let x1 = 3.0;
    /// let y1 = -3.0;
    ///
    /// // 135 deg clockwise
    /// let x2 = -3.0;
    /// let y2 = 3.0;
    ///
    /// let abs_difference_1 = (y1.atan2(x1) - (-pi/4.0)).abs();
    /// let abs_difference_2 = (y2.atan2(x2) - 3.0*pi/4.0).abs();
    ///
    /// assert!(abs_difference_1 < 1e-10);
    /// assert!(abs_difference_2 < 1e-10);
    /// ```
    fn atan2(self, other: Self) -> Self;

    /// Simultaneously computes the sine and cosine of the number, `x`. Returns
    /// `(sin(x), cos(x))`.
    ///
    /// ```
    /// use num_traits::Float;
    /// use std::f64;
    ///
    /// let x = f64::consts::PI/4.0;
    /// let f = x.sin_cos();
    ///
    /// let abs_difference_0 = (f.0 - x.sin()).abs();
    /// let abs_difference_1 = (f.1 - x.cos()).abs();
    ///
    /// assert!(abs_difference_0 < 1e-10);
    /// assert!(abs_difference_0 < 1e-10);
    /// ```
    fn sin_cos(self) -> (Self, Self);

    /// Returns `e^(self) - 1` in a way that is accurate even if the
    /// number is close to zero.
    ///
    /// ```
    /// use num_traits::Float;
    ///
    /// let x = 7.0;
    ///
    /// // e^(ln(7)) - 1
    /// let abs_difference = (x.ln().exp_m1() - 6.0).abs();
    ///
    /// assert!(abs_difference < 1e-10);
    /// ```
    fn exp_m1(self) -> Self;

    /// Returns `ln(1+n)` (natural logarithm) more accurately than if
    /// the operations were performed separately.
    ///
    /// ```
    /// use num_traits::Float;
    /// use std::f64;
    ///
    /// let x = f64::consts::E - 1.0;
    ///
    /// // ln(1 + (e - 1)) == ln(e) == 1
    /// let abs_difference = (x.ln_1p() - 1.0).abs();
    ///
    /// assert!(abs_difference < 1e-10);
    /// ```
    fn ln_1p(self) -> Self;

    /// Hyperbolic sine function.
    ///
    /// ```
    /// use num_traits::Float;
    /// use std::f64;
    ///
    /// let e = f64::consts::E;
    /// let x = 1.0;
    ///
    /// let f = x.sinh();
    /// // Solving sinh() at 1 gives `(e^2-1)/(2e)`
    /// let g = (e*e - 1.0)/(2.0*e);
    /// let abs_difference = (f - g).abs();
    ///
    /// assert!(abs_difference < 1e-10);
    /// ```
    fn sinh(self) -> Self;

    /// Hyperbolic cosine function.
    ///
    /// ```
    /// use num_traits::Float;
    /// use std::f64;
    ///
    /// let e = f64::consts::E;
    /// let x = 1.0;
    /// let f = x.cosh();
    /// // Solving cosh() at 1 gives this result
    /// let g = (e*e + 1.0)/(2.0*e);
    /// let abs_difference = (f - g).abs();
    ///
    /// // Same result
    /// assert!(abs_difference < 1.0e-10);
    /// ```
    fn cosh(self) -> Self;

    /// Hyperbolic tangent function.
    ///
    /// ```
    /// use num_traits::Float;
    /// use std::f64;
    ///
    /// let e = f64::consts::E;
    /// let x = 1.0;
    ///
    /// let f = x.tanh();
    /// // Solving tanh() at 1 gives `(1 - e^(-2))/(1 + e^(-2))`
    /// let g = (1.0 - e.powi(-2))/(1.0 + e.powi(-2));
    /// let abs_difference = (f - g).abs();
    ///
    /// assert!(abs_difference < 1.0e-10);
    /// ```
    fn tanh(self) -> Self;

    /// Inverse hyperbolic sine function.
    ///
    /// ```
    /// use num_traits::Float;
    ///
    /// let x = 1.0;
    /// let f = x.sinh().asinh();
    ///
    /// let abs_difference = (f - x).abs();
    ///
    /// assert!(abs_difference < 1.0e-10);
    /// ```
    fn asinh(self) -> Self;

    /// Inverse hyperbolic cosine function.
    ///
    /// ```
    /// use num_traits::Float;
    ///
    /// let x = 1.0;
    /// let f = x.cosh().acosh();
    ///
    /// let abs_difference = (f - x).abs();
    ///
    /// assert!(abs_difference < 1.0e-10);
    /// ```
    fn acosh(self) -> Self;

    /// Inverse hyperbolic tangent function.
    ///
    /// ```
    /// use num_traits::Float;
    /// use std::f64;
    ///
    /// let e = f64::consts::E;
    /// let f = e.tanh().atanh();
    ///
    /// let abs_difference = (f - e).abs();
    ///
    /// assert!(abs_difference < 1.0e-10);
    /// ```
    fn atanh(self) -> Self;

    /// Returns the mantissa, base 2 exponent, and sign as integers, respectively.
    /// The original number can be recovered by `sign * mantissa * 2 ^ exponent`.
    ///
    /// ```
    /// use num_traits::Float;
    ///
    /// let num = 2.0f32;
    ///
    /// // (8388608, -22, 1)
    /// let (mantissa, exponent, sign) = Float::integer_decode(num);
    /// let sign_f = sign as f32;
    /// let mantissa_f = mantissa as f32;
    /// let exponent_f = num.powf(exponent as f32);
    ///
    /// // 1 * 8388608 * 2^(-22) == 2
    /// let abs_difference = (sign_f * mantissa_f * exponent_f - num).abs();
    ///
    /// assert!(abs_difference < 1e-10);
    /// ```
    fn integer_decode(self) -> (u64, i16, i8);

    /// Returns a number composed of the magnitude of `self` and the sign of
    /// `sign`.
    ///
    /// Equal to `self` if the sign of `self` and `sign` are the same, otherwise
    /// equal to `-self`. If `self` is a `NAN`, then a `NAN` with the sign of
    /// `sign` is returned.
    ///
    /// # Examples
    ///
    /// ```
    /// use num_traits::Float;
    ///
    /// let f = 3.5_f32;
    ///
    /// assert_eq!(f.copysign(0.42), 3.5_f32);
    /// assert_eq!(f.copysign(-0.42), -3.5_f32);
    /// assert_eq!((-f).copysign(0.42), 3.5_f32);
    /// assert_eq!((-f).copysign(-0.42), -3.5_f32);
    ///
    /// assert!(f32::nan().copysign(1.0).is_nan());
    /// ```
    fn copysign(self, sign: Self) -> Self {
        if self.is_sign_negative() == sign.is_sign_negative() {
            self
        } else {
            self.neg()
        }
    }
}

#[cfg(feature = "std")]
macro_rules! float_impl_std {
    ($T:ident $decode:ident) => {
        impl Float for $T {
            constant! {
                nan() -> $T::NAN;
                infinity() -> $T::INFINITY;
                neg_infinity() -> $T::NEG_INFINITY;
                neg_zero() -> -0.0;
                min_value() -> $T::MIN;
                min_positive_value() -> $T::MIN_POSITIVE;
                epsilon() -> $T::EPSILON;
                max_value() -> $T::MAX;
            }

            #[inline]
            #[allow(deprecated)]
            fn abs_sub(self, other: Self) -> Self {
                <$T>::abs_sub(self, other)
            }

            #[inline]
            fn integer_decode(self) -> (u64, i16, i8) {
                $decode(self)
            }

            forward! {
                Self::is_nan(self) -> bool;
                Self::is_infinite(self) -> bool;
                Self::is_finite(self) -> bool;
                Self::is_normal(self) -> bool;
                Self::is_subnormal(self) -> bool;
                Self::classify(self) -> FpCategory;
                Self::clamp(self, min: Self, max: Self) -> Self;
                Self::floor(self) -> Self;
                Self::ceil(self) -> Self;
                Self::round(self) -> Self;
                Self::trunc(self) -> Self;
                Self::fract(self) -> Self;
                Self::abs(self) -> Self;
                Self::signum(self) -> Self;
                Self::is_sign_positive(self) -> bool;
                Self::is_sign_negative(self) -> bool;
                Self::mul_add(self, a: Self, b: Self) -> Self;
                Self::recip(self) -> Self;
                Self::powi(self, n: i32) -> Self;
                Self::powf(self, n: Self) -> Self;
                Self::sqrt(self) -> Self;
                Self::exp(self) -> Self;
                Self::exp2(self) -> Self;
                Self::ln(self) -> Self;
                Self::log(self, base: Self) -> Self;
                Self::log2(self) -> Self;
                Self::log10(self) -> Self;
                Self::to_degrees(self) -> Self;
                Self::to_radians(self) -> Self;
                Self::max(self, other: Self) -> Self;
                Self::min(self, other: Self) -> Self;
                Self::cbrt(self) -> Self;
                Self::hypot(self, other: Self) -> Self;
                Self::sin(self) -> Self;
                Self::cos(self) -> Self;
                Self::tan(self) -> Self;
                Self::asin(self) -> Self;
                Self::acos(self) -> Self;
                Self::atan(self) -> Self;
                Self::atan2(self, other: Self) -> Self;
                Self::sin_cos(self) -> (Self, Self);
                Self::exp_m1(self) -> Self;
                Self::ln_1p(self) -> Self;
                Self::sinh(self) -> Self;
                Self::cosh(self) -> Self;
                Self::tanh(self) -> Self;
                Self::asinh(self) -> Self;
                Self::acosh(self) -> Self;
                Self::atanh(self) -> Self;
                Self::copysign(self, sign: Self) -> Self;
            }
        }
    };
}

#[cfg(all(not(feature = "std"), feature = "libm"))]
macro_rules! float_impl_libm {
    ($T:ident $decode:ident) => {
        constant! {
            nan() -> $T::NAN;
            infinity() -> $T::INFINITY;
            neg_infinity() -> $T::NEG_INFINITY;
            neg_zero() -> -0.0;
            min_value() -> $T::MIN;
            min_positive_value() -> $T::MIN_POSITIVE;
            epsilon() -> $T::EPSILON;
            max_value() -> $T::MAX;
        }

        #[inline]
        fn integer_decode(self) -> (u64, i16, i8) {
            $decode(self)
        }

        #[inline]
        fn fract(self) -> Self {
            self - Float::trunc(self)
        }

        #[inline]
        fn log(self, base: Self) -> Self {
            self.ln() / base.ln()
        }

        forward! {
            Self::is_nan(self) -> bool;
            Self::is_infinite(self) -> bool;
            Self::is_finite(self) -> bool;
            Self::is_normal(self) -> bool;
            Self::is_subnormal(self) -> bool;
            Self::clamp(self, min: Self, max: Self) -> Self;
            Self::classify(self) -> FpCategory;
            Self::is_sign_positive(self) -> bool;
            Self::is_sign_negative(self) -> bool;
            Self::min(self, other: Self) -> Self;
            Self::max(self, other: Self) -> Self;
            Self::recip(self) -> Self;
            Self::to_degrees(self) -> Self;
            Self::to_radians(self) -> Self;
        }

        forward! {
            FloatCore::signum(self) -> Self;
            FloatCore::powi(self, n: i32) -> Self;
        }
    };
}

fn integer_decode_f32(f: f32) -> (u64, i16, i8) {
    let bits: u32 = f.to_bits();
    let sign: i8 = if bits >> 31 == 0 { 1 } else { -1 };
    let mut exponent: i16 = ((bits >> 23) & 0xff) as i16;
    let mantissa = if exponent == 0 {
        (bits & 0x7fffff) << 1
    } else {
        (bits & 0x7fffff) | 0x800000
    };
    // Exponent bias + mantissa shift
    exponent -= 127 + 23;
    (mantissa as u64, exponent, sign)
}

fn integer_decode_f64(f: f64) -> (u64, i16, i8) {
    let bits: u64 = f.to_bits();
    let sign: i8 = if bits >> 63 == 0 { 1 } else { -1 };
    let mut exponent: i16 = ((bits >> 52) & 0x7ff) as i16;
    let mantissa = if exponent == 0 {
        (bits & 0xfffffffffffff) << 1
    } else {
        (bits & 0xfffffffffffff) | 0x10000000000000
    };
    // Exponent bias + mantissa shift
    exponent -= 1023 + 52;
    (mantissa, exponent, sign)
}

#[cfg(feature = "std")]
float_impl_std!(f32 integer_decode_f32);
#[cfg(feature = "std")]
float_impl_std!(f64 integer_decode_f64);

#[cfg(all(not(feature = "std"), feature = "libm"))]
impl Float for f32 {
    float_impl_libm!(f32 integer_decode_f32);

    #[inline]
    #[allow(deprecated)]
    fn abs_sub(self, other: Self) -> Self {
        libm::fdimf(self, other)
    }

    forward! {
        libm::floorf as floor(self) -> Self;
        libm::ceilf as ceil(self) -> Self;
        libm::roundf as round(self) -> Self;
        libm::truncf as trunc(self) -> Self;
        libm::fabsf as abs(self) -> Self;
        libm::fmaf as mul_add(self, a: Self, b: Self) -> Self;
        libm::powf as powf(self, n: Self) -> Self;
        libm::sqrtf as sqrt(self) -> Self;
        libm::expf as exp(self) -> Self;
        libm::exp2f as exp2(self) -> Self;
        libm::logf as ln(self) -> Self;
        libm::log2f as log2(self) -> Self;
        libm::log10f as log10(self) -> Self;
        libm::cbrtf as cbrt(self) -> Self;
        libm::hypotf as hypot(self, other: Self) -> Self;
        libm::sinf as sin(self) -> Self;
        libm::cosf as cos(self) -> Self;
        libm::tanf as tan(self) -> Self;
        libm::asinf as asin(self) -> Self;
        libm::acosf as acos(self) -> Self;
        libm::atanf as atan(self) -> Self;
        libm::atan2f as atan2(self, other: Self) -> Self;
        libm::sincosf as sin_cos(self) -> (Self, Self);
        libm::expm1f as exp_m1(self) -> Self;
        libm::log1pf as ln_1p(self) -> Self;
        libm::sinhf as sinh(self) -> Self;
        libm::coshf as cosh(self) -> Self;
        libm::tanhf as tanh(self) -> Self;
        libm::asinhf as asinh(self) -> Self;
        libm::acoshf as acosh(self) -> Self;
        libm::atanhf as atanh(self) -> Self;
        libm::copysignf as copysign(self, other: Self) -> Self;
    }
}

#[cfg(all(not(feature = "std"), feature = "libm"))]
impl Float for f64 {
    float_impl_libm!(f64 integer_decode_f64);

    #[inline]
    #[allow(deprecated)]
    fn abs_sub(self, other: Self) -> Self {
        libm::fdim(self, other)
    }

    forward! {
        libm::floor as floor(self) -> Self;
        libm::ceil as ceil(self) -> Self;
        libm::round as round(self) -> Self;
        libm::trunc as trunc(self) -> Self;
        libm::fabs as abs(self) -> Self;
        libm::fma as mul_add(self, a: Self, b: Self) -> Self;
        libm::pow as powf(self, n: Self) -> Self;
        libm::sqrt as sqrt(self) -> Self;
        libm::exp as exp(self) -> Self;
        libm::exp2 as exp2(self) -> Self;
        libm::log as ln(self) -> Self;
        libm::log2 as log2(self) -> Self;
        libm::log10 as log10(self) -> Self;
        libm::cbrt as cbrt(self) -> Self;
        libm::hypot as hypot(self, other: Self) -> Self;
        libm::sin as sin(self) -> Self;
        libm::cos as cos(self) -> Self;
        libm::tan as tan(self) -> Self;
        libm::asin as asin(self) -> Self;
        libm::acos as acos(self) -> Self;
        libm::atan as atan(self) -> Self;
        libm::atan2 as atan2(self, other: Self) -> Self;
        libm::sincos as sin_cos(self) -> (Self, Self);
        libm::expm1 as exp_m1(self) -> Self;
        libm::log1p as ln_1p(self) -> Self;
        libm::sinh as sinh(self) -> Self;
        libm::cosh as cosh(self) -> Self;
        libm::tanh as tanh(self) -> Self;
        libm::asinh as asinh(self) -> Self;
        libm::acosh as acosh(self) -> Self;
        libm::atanh as atanh(self) -> Self;
        libm::copysign as copysign(self, sign: Self) -> Self;
    }
}

macro_rules! float_const_impl {
    ($(#[$doc:meta] $constant:ident,)+) => (
        #[allow(non_snake_case)]
        pub trait FloatConst {
            $(#[$doc] fn $constant() -> Self;)+
            #[doc = "Return the full circle constant `τ`."]
            #[inline]
            fn TAU() -> Self where Self: Sized + Add<Self, Output = Self> {
                Self::PI() + Self::PI()
            }
            #[doc = "Return `log10(2.0)`."]
            #[inline]
            fn LOG10_2() -> Self where Self: Sized + Div<Self, Output = Self> {
                Self::LN_2() / Self::LN_10()
            }
            #[doc = "Return `log2(10.0)`."]
            #[inline]
            fn LOG2_10() -> Self where Self: Sized + Div<Self, Output = Self> {
                Self::LN_10() / Self::LN_2()
            }
        }
        float_const_impl! { @float f32, $($constant,)+ }
        float_const_impl! { @float f64, $($constant,)+ }
    );
    (@float $T:ident, $($constant:ident,)+) => (
        impl FloatConst for $T {
            constant! {
                $( $constant() -> $T::consts::$constant; )+
                TAU() -> 6.28318530717958647692528676655900577;
                LOG10_2() -> 0.301029995663981195213738894724493027;
                LOG2_10() -> 3.32192809488736234787031942948939018;
            }
        }
    );
}

float_const_impl! {
    #[doc = "Return Euler’s number."]
    E,
    #[doc = "Return `1.0 / π`."]
    FRAC_1_PI,
    #[doc = "Return `1.0 / sqrt(2.0)`."]
    FRAC_1_SQRT_2,
    #[doc = "Return `2.0 / π`."]
    FRAC_2_PI,
    #[doc = "Return `2.0 / sqrt(π)`."]
    FRAC_2_SQRT_PI,
    #[doc = "Return `π / 2.0`."]
    FRAC_PI_2,
    #[doc = "Return `π / 3.0`."]
    FRAC_PI_3,
    #[doc = "Return `π / 4.0`."]
    FRAC_PI_4,
    #[doc = "Return `π / 6.0`."]
    FRAC_PI_6,
    #[doc = "Return `π / 8.0`."]
    FRAC_PI_8,
    #[doc = "Return `ln(10.0)`."]
    LN_10,
    #[doc = "Return `ln(2.0)`."]
    LN_2,
    #[doc = "Return `log10(e)`."]
    LOG10_E,
    #[doc = "Return `log2(e)`."]
    LOG2_E,
    #[doc = "Return Archimedes’ constant `π`."]
    PI,
    #[doc = "Return `sqrt(2.0)`."]
    SQRT_2,
}

/// Trait for floating point numbers that provide an implementation
/// of the `totalOrder` predicate as defined in the IEEE 754 (2008 revision)
/// floating point standard.
pub trait TotalOrder {
    /// Return the ordering between `self` and `other`.
    ///
    /// Unlike the standard partial comparison between floating point numbers,
    /// this comparison always produces an ordering in accordance to
    /// the `totalOrder` predicate as defined in the IEEE 754 (2008 revision)
    /// floating point standard. The values are ordered in the following sequence:
    ///
    /// - negative quiet NaN
    /// - negative signaling NaN
    /// - negative infinity
    /// - negative numbers
    /// - negative subnormal numbers
    /// - negative zero
    /// - positive zero
    /// - positive subnormal numbers
    /// - positive numbers
    /// - positive infinity
    /// - positive signaling NaN
    /// - positive quiet NaN.
    ///
    /// The ordering established by this function does not always agree with the
    /// [`PartialOrd`] and [`PartialEq`] implementations. For example,
    /// they consider negative and positive zero equal, while `total_cmp`
    /// doesn't.
    ///
    /// The interpretation of the signaling NaN bit follows the definition in
    /// the IEEE 754 standard, which may not match the interpretation by some of
    /// the older, non-conformant (e.g. MIPS) hardware implementations.
    ///
    /// # Examples
    /// ```
    /// use num_traits::float::TotalOrder;
    /// use std::cmp::Ordering;
    /// use std::{f32, f64};
    ///
    /// fn check_eq<T: TotalOrder>(x: T, y: T) {
    ///     assert_eq!(x.total_cmp(&y), Ordering::Equal);
    /// }
    ///
    /// check_eq(f64::NAN, f64::NAN);
    /// check_eq(f32::NAN, f32::NAN);
    ///
    /// fn check_lt<T: TotalOrder>(x: T, y: T) {
    ///     assert_eq!(x.total_cmp(&y), Ordering::Less);
    /// }
    ///
    /// check_lt(-f64::NAN, f64::NAN);
    /// check_lt(f64::INFINITY, f64::NAN);
    /// check_lt(-0.0_f64, 0.0_f64);
    /// ```
    fn total_cmp(&self, other: &Self) -> Ordering;
}
macro_rules! totalorder_impl {
    ($T:ident, $I:ident, $U:ident, $bits:expr) => {
        impl TotalOrder for $T {
            #[inline]
            #[cfg(has_total_cmp)]
            fn total_cmp(&self, other: &Self) -> Ordering {
                // Forward to the core implementation
                Self::total_cmp(&self, other)
            }
            #[inline]
            #[cfg(not(has_total_cmp))]
            fn total_cmp(&self, other: &Self) -> Ordering {
                // Backport the core implementation (since 1.62)
                let mut left = self.to_bits() as $I;
                let mut right = other.to_bits() as $I;

                left ^= (((left >> ($bits - 1)) as $U) >> 1) as $I;
                right ^= (((right >> ($bits - 1)) as $U) >> 1) as $I;

                left.cmp(&right)
            }
        }
    };
}
totalorder_impl!(f64, i64, u64, 64);
totalorder_impl!(f32, i32, u32, 32);

#[cfg(test)]
mod tests {
    use core::f64::consts;

    const DEG_RAD_PAIRS: [(f64, f64); 7] = [
        (0.0, 0.),
        (22.5, consts::FRAC_PI_8),
        (30.0, consts::FRAC_PI_6),
        (45.0, consts::FRAC_PI_4),
        (60.0, consts::FRAC_PI_3),
        (90.0, consts::FRAC_PI_2),
        (180.0, consts::PI),
    ];

    #[test]
    fn convert_deg_rad() {
        use crate::float::FloatCore;

        for &(deg, rad) in &DEG_RAD_PAIRS {
            assert!((FloatCore::to_degrees(rad) - deg).abs() < 1e-6);
            assert!((FloatCore::to_radians(deg) - rad).abs() < 1e-6);

            let (deg, rad) = (deg as f32, rad as f32);
            assert!((FloatCore::to_degrees(rad) - deg).abs() < 1e-5);
            assert!((FloatCore::to_radians(deg) - rad).abs() < 1e-5);
        }
    }

    #[cfg(any(feature = "std", feature = "libm"))]
    #[test]
    fn convert_deg_rad_std() {
        for &(deg, rad) in &DEG_RAD_PAIRS {
            use crate::Float;

            assert!((Float::to_degrees(rad) - deg).abs() < 1e-6);
            assert!((Float::to_radians(deg) - rad).abs() < 1e-6);

            let (deg, rad) = (deg as f32, rad as f32);
            assert!((Float::to_degrees(rad) - deg).abs() < 1e-5);
            assert!((Float::to_radians(deg) - rad).abs() < 1e-5);
        }
    }

    #[test]
    fn to_degrees_rounding() {
        use crate::float::FloatCore;

        assert_eq!(
            FloatCore::to_degrees(1_f32),
            57.2957795130823208767981548141051703
        );
    }

    #[test]
    #[cfg(any(feature = "std", feature = "libm"))]
    fn extra_logs() {
        use crate::float::{Float, FloatConst};

        fn check<F: Float + FloatConst>(diff: F) {
            let _2 = F::from(2.0).unwrap();
            assert!((F::LOG10_2() - F::log10(_2)).abs() < diff);
            assert!((F::LOG10_2() - F::LN_2() / F::LN_10()).abs() < diff);

            let _10 = F::from(10.0).unwrap();
            assert!((F::LOG2_10() - F::log2(_10)).abs() < diff);
            assert!((F::LOG2_10() - F::LN_10() / F::LN_2()).abs() < diff);
        }

        check::<f32>(1e-6);
        check::<f64>(1e-12);
    }

    #[test]
    #[cfg(any(feature = "std", feature = "libm"))]
    fn copysign() {
        use crate::float::Float;
        test_copysign_generic(2.0_f32, -2.0_f32, f32::nan());
        test_copysign_generic(2.0_f64, -2.0_f64, f64::nan());
        test_copysignf(2.0_f32, -2.0_f32, f32::nan());
    }

    #[cfg(any(feature = "std", feature = "libm"))]
    fn test_copysignf(p: f32, n: f32, nan: f32) {
        use crate::float::Float;
        use core::ops::Neg;

        assert!(p.is_sign_positive());
        assert!(n.is_sign_negative());
        assert!(nan.is_nan());

        assert_eq!(p, Float::copysign(p, p));
        assert_eq!(p.neg(), Float::copysign(p, n));

        assert_eq!(n, Float::copysign(n, n));
        assert_eq!(n.neg(), Float::copysign(n, p));

        assert!(Float::copysign(nan, p).is_sign_positive());
        assert!(Float::copysign(nan, n).is_sign_negative());
    }

    #[cfg(any(feature = "std", feature = "libm"))]
    fn test_copysign_generic<F: crate::float::Float + ::core::fmt::Debug>(p: F, n: F, nan: F) {
        assert!(p.is_sign_positive());
        assert!(n.is_sign_negative());
        assert!(nan.is_nan());
        assert!(!nan.is_subnormal());

        assert_eq!(p, p.copysign(p));
        assert_eq!(p.neg(), p.copysign(n));

        assert_eq!(n, n.copysign(n));
        assert_eq!(n.neg(), n.copysign(p));

        assert!(nan.copysign(p).is_sign_positive());
        assert!(nan.copysign(n).is_sign_negative());
    }

    #[cfg(any(feature = "std", feature = "libm"))]
    fn test_subnormal<F: crate::float::Float + ::core::fmt::Debug>() {
        let min_positive = F::min_positive_value();
        let lower_than_min = min_positive / F::from(2.0f32).unwrap();
        assert!(!min_positive.is_subnormal());
        assert!(lower_than_min.is_subnormal());
    }

    #[test]
    #[cfg(any(feature = "std", feature = "libm"))]
    fn subnormal() {
        test_subnormal::<f64>();
        test_subnormal::<f32>();
    }

    #[test]
    fn total_cmp() {
        use crate::float::TotalOrder;
        use core::cmp::Ordering;
        use core::{f32, f64};

        fn check_eq<T: TotalOrder>(x: T, y: T) {
            assert_eq!(x.total_cmp(&y), Ordering::Equal);
        }
        fn check_lt<T: TotalOrder>(x: T, y: T) {
            assert_eq!(x.total_cmp(&y), Ordering::Less);
        }
        fn check_gt<T: TotalOrder>(x: T, y: T) {
            assert_eq!(x.total_cmp(&y), Ordering::Greater);
        }

        check_eq(f64::NAN, f64::NAN);
        check_eq(f32::NAN, f32::NAN);

        check_lt(-0.0_f64, 0.0_f64);
        check_lt(-0.0_f32, 0.0_f32);

        // x87 registers don't preserve the exact value of signaling NaN:
        // https://github.com/rust-lang/rust/issues/115567
        #[cfg(not(target_arch = "x86"))]
        {
            let s_nan = f64::from_bits(0x7ff4000000000000);
            let q_nan = f64::from_bits(0x7ff8000000000000);
            check_lt(s_nan, q_nan);

            let neg_s_nan = f64::from_bits(0xfff4000000000000);
            let neg_q_nan = f64::from_bits(0xfff8000000000000);
            check_lt(neg_q_nan, neg_s_nan);

            let s_nan = f32::from_bits(0x7fa00000);
            let q_nan = f32::from_bits(0x7fc00000);
            check_lt(s_nan, q_nan);

            let neg_s_nan = f32::from_bits(0xffa00000);
            let neg_q_nan = f32::from_bits(0xffc00000);
            check_lt(neg_q_nan, neg_s_nan);
        }

        check_lt(-f64::NAN, f64::NEG_INFINITY);
        check_gt(1.0_f64, -f64::NAN);
        check_lt(f64::INFINITY, f64::NAN);
        check_gt(f64::NAN, 1.0_f64);

        check_lt(-f32::NAN, f32::NEG_INFINITY);
        check_gt(1.0_f32, -f32::NAN);
        check_lt(f32::INFINITY, f32::NAN);
        check_gt(f32::NAN, 1.0_f32);
    }
}
