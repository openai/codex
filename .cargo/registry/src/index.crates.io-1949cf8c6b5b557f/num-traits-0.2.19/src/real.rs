#![cfg(any(feature = "std", feature = "libm"))]

use core::ops::Neg;

use crate::{Float, Num, NumCast};

// NOTE: These doctests have the same issue as those in src/float.rs.
// They're testing the inherent methods directly, and not those of `Real`.

/// A trait for real number types that do not necessarily have
/// floating-point-specific characteristics such as NaN and infinity.
///
/// See [this Wikipedia article](https://en.wikipedia.org/wiki/Real_data_type)
/// for a list of data types that could meaningfully implement this trait.
///
/// This trait is only available with the `std` feature, or with the `libm` feature otherwise.
pub trait Real: Num + Copy + NumCast + PartialOrd + Neg<Output = Self> {
    /// Returns the smallest finite value that this type can represent.
    ///
    /// ```
    /// use num_traits::real::Real;
    /// use std::f64;
    ///
    /// let x: f64 = Real::min_value();
    ///
    /// assert_eq!(x, f64::MIN);
    /// ```
    fn min_value() -> Self;

    /// Returns the smallest positive, normalized value that this type can represent.
    ///
    /// ```
    /// use num_traits::real::Real;
    /// use std::f64;
    ///
    /// let x: f64 = Real::min_positive_value();
    ///
    /// assert_eq!(x, f64::MIN_POSITIVE);
    /// ```
    fn min_positive_value() -> Self;

    /// Returns epsilon, a small positive value.
    ///
    /// ```
    /// use num_traits::real::Real;
    /// use std::f64;
    ///
    /// let x: f64 = Real::epsilon();
    ///
    /// assert_eq!(x, f64::EPSILON);
    /// ```
    ///
    /// # Panics
    ///
    /// The default implementation will panic if `f32::EPSILON` cannot
    /// be cast to `Self`.
    fn epsilon() -> Self;

    /// Returns the largest finite value that this type can represent.
    ///
    /// ```
    /// use num_traits::real::Real;
    /// use std::f64;
    ///
    /// let x: f64 = Real::max_value();
    /// assert_eq!(x, f64::MAX);
    /// ```
    fn max_value() -> Self;

    /// Returns the largest integer less than or equal to a number.
    ///
    /// ```
    /// use num_traits::real::Real;
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
    /// use num_traits::real::Real;
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
    /// use num_traits::real::Real;
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
    /// use num_traits::real::Real;
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
    /// use num_traits::real::Real;
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
    /// use num_traits::real::Real;
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
    /// assert!(::num_traits::Float::is_nan(f64::NAN.abs()));
    /// ```
    fn abs(self) -> Self;

    /// Returns a number that represents the sign of `self`.
    ///
    /// - `1.0` if the number is positive, `+0.0` or `Float::infinity()`
    /// - `-1.0` if the number is negative, `-0.0` or `Float::neg_infinity()`
    /// - `Float::nan()` if the number is `Float::nan()`
    ///
    /// ```
    /// use num_traits::real::Real;
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
    /// `Float::infinity()`, and with newer versions of Rust `f64::NAN`.
    ///
    /// ```
    /// use num_traits::real::Real;
    /// use std::f64;
    ///
    /// let neg_nan: f64 = -f64::NAN;
    ///
    /// let f = 7.0;
    /// let g = -7.0;
    ///
    /// assert!(f.is_sign_positive());
    /// assert!(!g.is_sign_positive());
    /// assert!(!neg_nan.is_sign_positive());
    /// ```
    fn is_sign_positive(self) -> bool;

    /// Returns `true` if `self` is negative, including `-0.0`,
    /// `Float::neg_infinity()`, and with newer versions of Rust `-f64::NAN`.
    ///
    /// ```
    /// use num_traits::real::Real;
    /// use std::f64;
    ///
    /// let nan: f64 = f64::NAN;
    ///
    /// let f = 7.0;
    /// let g = -7.0;
    ///
    /// assert!(!f.is_sign_negative());
    /// assert!(g.is_sign_negative());
    /// assert!(!nan.is_sign_negative());
    /// ```
    fn is_sign_negative(self) -> bool;

    /// Fused multiply-add. Computes `(self * a) + b` with only one rounding
    /// error, yielding a more accurate result than an unfused multiply-add.
    ///
    /// Using `mul_add` can be more performant than an unfused multiply-add if
    /// the target architecture has a dedicated `fma` CPU instruction.
    ///
    /// ```
    /// use num_traits::real::Real;
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
    /// use num_traits::real::Real;
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
    /// use num_traits::real::Real;
    ///
    /// let x = 2.0;
    /// let abs_difference = (x.powi(2) - x*x).abs();
    ///
    /// assert!(abs_difference < 1e-10);
    /// ```
    fn powi(self, n: i32) -> Self;

    /// Raise a number to a real number power.
    ///
    /// ```
    /// use num_traits::real::Real;
    ///
    /// let x = 2.0;
    /// let abs_difference = (x.powf(2.0) - x*x).abs();
    ///
    /// assert!(abs_difference < 1e-10);
    /// ```
    fn powf(self, n: Self) -> Self;

    /// Take the square root of a number.
    ///
    /// Returns NaN if `self` is a negative floating-point number.
    ///
    /// # Panics
    ///
    /// If the implementing type doesn't support NaN, this method should panic if `self < 0`.
    ///
    /// ```
    /// use num_traits::real::Real;
    ///
    /// let positive = 4.0;
    /// let negative = -4.0;
    ///
    /// let abs_difference = (positive.sqrt() - 2.0).abs();
    ///
    /// assert!(abs_difference < 1e-10);
    /// assert!(::num_traits::Float::is_nan(negative.sqrt()));
    /// ```
    fn sqrt(self) -> Self;

    /// Returns `e^(self)`, (the exponential function).
    ///
    /// ```
    /// use num_traits::real::Real;
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
    /// use num_traits::real::Real;
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
    /// # Panics
    ///
    /// If `self <= 0` and this type does not support a NaN representation, this function should panic.
    ///
    /// ```
    /// use num_traits::real::Real;
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
    /// # Panics
    ///
    /// If `self <= 0` and this type does not support a NaN representation, this function should panic.
    ///
    /// ```
    /// use num_traits::real::Real;
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
    /// # Panics
    ///
    /// If `self <= 0` and this type does not support a NaN representation, this function should panic.
    ///
    /// ```
    /// use num_traits::real::Real;
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
    /// # Panics
    ///
    /// If `self <= 0` and this type does not support a NaN representation, this function should panic.
    ///
    ///
    /// ```
    /// use num_traits::real::Real;
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
    fn to_degrees(self) -> Self;

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
    fn to_radians(self) -> Self;

    /// Returns the maximum of the two numbers.
    ///
    /// ```
    /// use num_traits::real::Real;
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
    /// use num_traits::real::Real;
    ///
    /// let x = 1.0;
    /// let y = 2.0;
    ///
    /// assert_eq!(x.min(y), x);
    /// ```
    fn min(self, other: Self) -> Self;

    /// The positive difference of two numbers.
    ///
    /// * If `self <= other`: `0:0`
    /// * Else: `self - other`
    ///
    /// ```
    /// use num_traits::real::Real;
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
    /// use num_traits::real::Real;
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
    /// use num_traits::real::Real;
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
    /// use num_traits::real::Real;
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
    /// use num_traits::real::Real;
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
    /// use num_traits::real::Real;
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
    /// # Panics
    ///
    /// If this type does not support a NaN representation, this function should panic
    /// if the number is outside the range [-1, 1].
    ///
    /// ```
    /// use num_traits::real::Real;
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
    /// # Panics
    ///
    /// If this type does not support a NaN representation, this function should panic
    /// if the number is outside the range [-1, 1].
    ///
    /// ```
    /// use num_traits::real::Real;
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
    /// use num_traits::real::Real;
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
    /// use num_traits::real::Real;
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
    /// use num_traits::real::Real;
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
    /// use num_traits::real::Real;
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
    /// # Panics
    ///
    /// If this type does not support a NaN representation, this function should panic
    /// if `self-1 <= 0`.
    ///
    /// ```
    /// use num_traits::real::Real;
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
    /// use num_traits::real::Real;
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
    /// use num_traits::real::Real;
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
    /// use num_traits::real::Real;
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
    /// use num_traits::real::Real;
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
    /// use num_traits::real::Real;
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
    /// use num_traits::real::Real;
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
}

impl<T: Float> Real for T {
    forward! {
        Float::min_value() -> Self;
        Float::min_positive_value() -> Self;
        Float::epsilon() -> Self;
        Float::max_value() -> Self;
    }
    forward! {
        Float::floor(self) -> Self;
        Float::ceil(self) -> Self;
        Float::round(self) -> Self;
        Float::trunc(self) -> Self;
        Float::fract(self) -> Self;
        Float::abs(self) -> Self;
        Float::signum(self) -> Self;
        Float::is_sign_positive(self) -> bool;
        Float::is_sign_negative(self) -> bool;
        Float::mul_add(self, a: Self, b: Self) -> Self;
        Float::recip(self) -> Self;
        Float::powi(self, n: i32) -> Self;
        Float::powf(self, n: Self) -> Self;
        Float::sqrt(self) -> Self;
        Float::exp(self) -> Self;
        Float::exp2(self) -> Self;
        Float::ln(self) -> Self;
        Float::log(self, base: Self) -> Self;
        Float::log2(self) -> Self;
        Float::log10(self) -> Self;
        Float::to_degrees(self) -> Self;
        Float::to_radians(self) -> Self;
        Float::max(self, other: Self) -> Self;
        Float::min(self, other: Self) -> Self;
        Float::abs_sub(self, other: Self) -> Self;
        Float::cbrt(self) -> Self;
        Float::hypot(self, other: Self) -> Self;
        Float::sin(self) -> Self;
        Float::cos(self) -> Self;
        Float::tan(self) -> Self;
        Float::asin(self) -> Self;
        Float::acos(self) -> Self;
        Float::atan(self) -> Self;
        Float::atan2(self, other: Self) -> Self;
        Float::sin_cos(self) -> (Self, Self);
        Float::exp_m1(self) -> Self;
        Float::ln_1p(self) -> Self;
        Float::sinh(self) -> Self;
        Float::cosh(self) -> Self;
        Float::tanh(self) -> Self;
        Float::asinh(self) -> Self;
        Float::acosh(self) -> Self;
        Float::atanh(self) -> Self;
    }
}
