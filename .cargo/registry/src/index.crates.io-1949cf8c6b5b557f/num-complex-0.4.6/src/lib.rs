// Copyright 2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Complex numbers.
//!
//! ## Compatibility
//!
//! The `num-complex` crate is tested for rustc 1.60 and greater.

#![doc(html_root_url = "https://docs.rs/num-complex/0.4")]
#![no_std]

#[cfg(any(test, feature = "std"))]
#[cfg_attr(test, macro_use)]
extern crate std;

use core::fmt;
#[cfg(test)]
use core::hash;
use core::iter::{Product, Sum};
use core::ops::{Add, Div, Mul, Neg, Rem, Sub};
use core::str::FromStr;
#[cfg(feature = "std")]
use std::error::Error;

use num_traits::{ConstOne, ConstZero, Inv, MulAdd, Num, One, Pow, Signed, Zero};

use num_traits::float::FloatCore;
#[cfg(any(feature = "std", feature = "libm"))]
use num_traits::float::{Float, FloatConst};

mod cast;
mod pow;

#[cfg(any(feature = "std", feature = "libm"))]
mod complex_float;
#[cfg(any(feature = "std", feature = "libm"))]
pub use crate::complex_float::ComplexFloat;

#[cfg(feature = "rand")]
mod crand;
#[cfg(feature = "rand")]
pub use crate::crand::ComplexDistribution;

// FIXME #1284: handle complex NaN & infinity etc. This
// probably doesn't map to C's _Complex correctly.

/// A complex number in Cartesian form.
///
/// ## Representation and Foreign Function Interface Compatibility
///
/// `Complex<T>` is memory layout compatible with an array `[T; 2]`.
///
/// Note that `Complex<F>` where F is a floating point type is **only** memory
/// layout compatible with C's complex types, **not** necessarily calling
/// convention compatible.  This means that for FFI you can only pass
/// `Complex<F>` behind a pointer, not as a value.
///
/// ## Examples
///
/// Example of extern function declaration.
///
/// ```
/// use num_complex::Complex;
/// use std::os::raw::c_int;
///
/// extern "C" {
///     fn zaxpy_(n: *const c_int, alpha: *const Complex<f64>,
///               x: *const Complex<f64>, incx: *const c_int,
///               y: *mut Complex<f64>, incy: *const c_int);
/// }
/// ```
#[derive(PartialEq, Eq, Copy, Clone, Hash, Debug, Default)]
#[repr(C)]
#[cfg_attr(
    feature = "rkyv",
    derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)
)]
#[cfg_attr(feature = "rkyv", archive(as = "Complex<T::Archived>"))]
#[cfg_attr(feature = "bytecheck", derive(bytecheck::CheckBytes))]
pub struct Complex<T> {
    /// Real portion of the complex number
    pub re: T,
    /// Imaginary portion of the complex number
    pub im: T,
}

/// Alias for a [`Complex<f32>`]
pub type Complex32 = Complex<f32>;

/// Create a new [`Complex<f32>`] with arguments that can convert [`Into<f32>`].
///
/// ```
/// use num_complex::{c32, Complex32};
/// assert_eq!(c32(1u8, 2), Complex32::new(1.0, 2.0));
/// ```
///
/// Note: ambiguous integer literals in Rust will [default] to `i32`, which does **not** implement
/// `Into<f32>`, so a call like `c32(1, 2)` will result in a type error. The example above uses a
/// suffixed `1u8` to set its type, and then the `2` can be inferred as the same type.
///
/// [default]: https://doc.rust-lang.org/reference/expressions/literal-expr.html#integer-literal-expressions
#[inline]
pub fn c32<T: Into<f32>>(re: T, im: T) -> Complex32 {
    Complex::new(re.into(), im.into())
}

/// Alias for a [`Complex<f64>`]
pub type Complex64 = Complex<f64>;

/// Create a new [`Complex<f64>`] with arguments that can convert [`Into<f64>`].
///
/// ```
/// use num_complex::{c64, Complex64};
/// assert_eq!(c64(1, 2), Complex64::new(1.0, 2.0));
/// ```
#[inline]
pub fn c64<T: Into<f64>>(re: T, im: T) -> Complex64 {
    Complex::new(re.into(), im.into())
}

impl<T> Complex<T> {
    /// Create a new `Complex`
    #[inline]
    pub const fn new(re: T, im: T) -> Self {
        Complex { re, im }
    }
}

impl<T: Clone + Num> Complex<T> {
    /// Returns the imaginary unit.
    ///
    /// See also [`Complex::I`].
    #[inline]
    pub fn i() -> Self {
        Self::new(T::zero(), T::one())
    }

    /// Returns the square of the norm (since `T` doesn't necessarily
    /// have a sqrt function), i.e. `re^2 + im^2`.
    #[inline]
    pub fn norm_sqr(&self) -> T {
        self.re.clone() * self.re.clone() + self.im.clone() * self.im.clone()
    }

    /// Multiplies `self` by the scalar `t`.
    #[inline]
    pub fn scale(&self, t: T) -> Self {
        Self::new(self.re.clone() * t.clone(), self.im.clone() * t)
    }

    /// Divides `self` by the scalar `t`.
    #[inline]
    pub fn unscale(&self, t: T) -> Self {
        Self::new(self.re.clone() / t.clone(), self.im.clone() / t)
    }

    /// Raises `self` to an unsigned integer power.
    #[inline]
    pub fn powu(&self, exp: u32) -> Self {
        Pow::pow(self, exp)
    }
}

impl<T: Clone + Num + Neg<Output = T>> Complex<T> {
    /// Returns the complex conjugate. i.e. `re - i im`
    #[inline]
    pub fn conj(&self) -> Self {
        Self::new(self.re.clone(), -self.im.clone())
    }

    /// Returns `1/self`
    #[inline]
    pub fn inv(&self) -> Self {
        let norm_sqr = self.norm_sqr();
        Self::new(
            self.re.clone() / norm_sqr.clone(),
            -self.im.clone() / norm_sqr,
        )
    }

    /// Raises `self` to a signed integer power.
    #[inline]
    pub fn powi(&self, exp: i32) -> Self {
        Pow::pow(self, exp)
    }
}

impl<T: Clone + Signed> Complex<T> {
    /// Returns the L1 norm `|re| + |im|` -- the [Manhattan distance] from the origin.
    ///
    /// [Manhattan distance]: https://en.wikipedia.org/wiki/Taxicab_geometry
    #[inline]
    pub fn l1_norm(&self) -> T {
        self.re.abs() + self.im.abs()
    }
}

#[cfg(any(feature = "std", feature = "libm"))]
impl<T: Float> Complex<T> {
    /// Create a new Complex with a given phase: `exp(i * phase)`.
    /// See [cis (mathematics)](https://en.wikipedia.org/wiki/Cis_(mathematics)).
    #[inline]
    pub fn cis(phase: T) -> Self {
        Self::new(phase.cos(), phase.sin())
    }

    /// Calculate |self|
    #[inline]
    pub fn norm(self) -> T {
        self.re.hypot(self.im)
    }
    /// Calculate the principal Arg of self.
    #[inline]
    pub fn arg(self) -> T {
        self.im.atan2(self.re)
    }
    /// Convert to polar form (r, theta), such that
    /// `self = r * exp(i * theta)`
    #[inline]
    pub fn to_polar(self) -> (T, T) {
        (self.norm(), self.arg())
    }
    /// Convert a polar representation into a complex number.
    #[inline]
    pub fn from_polar(r: T, theta: T) -> Self {
        Self::new(r * theta.cos(), r * theta.sin())
    }

    /// Computes `e^(self)`, where `e` is the base of the natural logarithm.
    #[inline]
    pub fn exp(self) -> Self {
        // formula: e^(a + bi) = e^a (cos(b) + i*sin(b)) = from_polar(e^a, b)

        let Complex { re, mut im } = self;
        // Treat the corner cases +∞, -∞, and NaN
        if re.is_infinite() {
            if re < T::zero() {
                if !im.is_finite() {
                    return Self::new(T::zero(), T::zero());
                }
            } else if im == T::zero() || !im.is_finite() {
                if im.is_infinite() {
                    im = T::nan();
                }
                return Self::new(re, im);
            }
        } else if re.is_nan() && im == T::zero() {
            return self;
        }

        Self::from_polar(re.exp(), im)
    }

    /// Computes the principal value of natural logarithm of `self`.
    ///
    /// This function has one branch cut:
    ///
    /// * `(-∞, 0]`, continuous from above.
    ///
    /// The branch satisfies `-π ≤ arg(ln(z)) ≤ π`.
    #[inline]
    pub fn ln(self) -> Self {
        // formula: ln(z) = ln|z| + i*arg(z)
        let (r, theta) = self.to_polar();
        Self::new(r.ln(), theta)
    }

    /// Computes the principal value of the square root of `self`.
    ///
    /// This function has one branch cut:
    ///
    /// * `(-∞, 0)`, continuous from above.
    ///
    /// The branch satisfies `-π/2 ≤ arg(sqrt(z)) ≤ π/2`.
    #[inline]
    pub fn sqrt(self) -> Self {
        if self.im.is_zero() {
            if self.re.is_sign_positive() {
                // simple positive real √r, and copy `im` for its sign
                Self::new(self.re.sqrt(), self.im)
            } else {
                // √(r e^(iπ)) = √r e^(iπ/2) = i√r
                // √(r e^(-iπ)) = √r e^(-iπ/2) = -i√r
                let re = T::zero();
                let im = (-self.re).sqrt();
                if self.im.is_sign_positive() {
                    Self::new(re, im)
                } else {
                    Self::new(re, -im)
                }
            }
        } else if self.re.is_zero() {
            // √(r e^(iπ/2)) = √r e^(iπ/4) = √(r/2) + i√(r/2)
            // √(r e^(-iπ/2)) = √r e^(-iπ/4) = √(r/2) - i√(r/2)
            let one = T::one();
            let two = one + one;
            let x = (self.im.abs() / two).sqrt();
            if self.im.is_sign_positive() {
                Self::new(x, x)
            } else {
                Self::new(x, -x)
            }
        } else {
            // formula: sqrt(r e^(it)) = sqrt(r) e^(it/2)
            let one = T::one();
            let two = one + one;
            let (r, theta) = self.to_polar();
            Self::from_polar(r.sqrt(), theta / two)
        }
    }

    /// Computes the principal value of the cube root of `self`.
    ///
    /// This function has one branch cut:
    ///
    /// * `(-∞, 0)`, continuous from above.
    ///
    /// The branch satisfies `-π/3 ≤ arg(cbrt(z)) ≤ π/3`.
    ///
    /// Note that this does not match the usual result for the cube root of
    /// negative real numbers. For example, the real cube root of `-8` is `-2`,
    /// but the principal complex cube root of `-8` is `1 + i√3`.
    #[inline]
    pub fn cbrt(self) -> Self {
        if self.im.is_zero() {
            if self.re.is_sign_positive() {
                // simple positive real ∛r, and copy `im` for its sign
                Self::new(self.re.cbrt(), self.im)
            } else {
                // ∛(r e^(iπ)) = ∛r e^(iπ/3) = ∛r/2 + i∛r√3/2
                // ∛(r e^(-iπ)) = ∛r e^(-iπ/3) = ∛r/2 - i∛r√3/2
                let one = T::one();
                let two = one + one;
                let three = two + one;
                let re = (-self.re).cbrt() / two;
                let im = three.sqrt() * re;
                if self.im.is_sign_positive() {
                    Self::new(re, im)
                } else {
                    Self::new(re, -im)
                }
            }
        } else if self.re.is_zero() {
            // ∛(r e^(iπ/2)) = ∛r e^(iπ/6) = ∛r√3/2 + i∛r/2
            // ∛(r e^(-iπ/2)) = ∛r e^(-iπ/6) = ∛r√3/2 - i∛r/2
            let one = T::one();
            let two = one + one;
            let three = two + one;
            let im = self.im.abs().cbrt() / two;
            let re = three.sqrt() * im;
            if self.im.is_sign_positive() {
                Self::new(re, im)
            } else {
                Self::new(re, -im)
            }
        } else {
            // formula: cbrt(r e^(it)) = cbrt(r) e^(it/3)
            let one = T::one();
            let three = one + one + one;
            let (r, theta) = self.to_polar();
            Self::from_polar(r.cbrt(), theta / three)
        }
    }

    /// Raises `self` to a floating point power.
    #[inline]
    pub fn powf(self, exp: T) -> Self {
        if exp.is_zero() {
            return Self::one();
        }
        // formula: x^y = (ρ e^(i θ))^y = ρ^y e^(i θ y)
        // = from_polar(ρ^y, θ y)
        let (r, theta) = self.to_polar();
        Self::from_polar(r.powf(exp), theta * exp)
    }

    /// Returns the logarithm of `self` with respect to an arbitrary base.
    #[inline]
    pub fn log(self, base: T) -> Self {
        // formula: log_y(x) = log_y(ρ e^(i θ))
        // = log_y(ρ) + log_y(e^(i θ)) = log_y(ρ) + ln(e^(i θ)) / ln(y)
        // = log_y(ρ) + i θ / ln(y)
        let (r, theta) = self.to_polar();
        Self::new(r.log(base), theta / base.ln())
    }

    /// Raises `self` to a complex power.
    #[inline]
    pub fn powc(self, exp: Self) -> Self {
        if exp.is_zero() {
            return Self::one();
        }
        // formula: x^y = exp(y * ln(x))
        (exp * self.ln()).exp()
    }

    /// Raises a floating point number to the complex power `self`.
    #[inline]
    pub fn expf(self, base: T) -> Self {
        // formula: x^(a+bi) = x^a x^bi = x^a e^(b ln(x) i)
        // = from_polar(x^a, b ln(x))
        Self::from_polar(base.powf(self.re), self.im * base.ln())
    }

    /// Computes the sine of `self`.
    #[inline]
    pub fn sin(self) -> Self {
        // formula: sin(a + bi) = sin(a)cosh(b) + i*cos(a)sinh(b)
        Self::new(
            self.re.sin() * self.im.cosh(),
            self.re.cos() * self.im.sinh(),
        )
    }

    /// Computes the cosine of `self`.
    #[inline]
    pub fn cos(self) -> Self {
        // formula: cos(a + bi) = cos(a)cosh(b) - i*sin(a)sinh(b)
        Self::new(
            self.re.cos() * self.im.cosh(),
            -self.re.sin() * self.im.sinh(),
        )
    }

    /// Computes the tangent of `self`.
    #[inline]
    pub fn tan(self) -> Self {
        // formula: tan(a + bi) = (sin(2a) + i*sinh(2b))/(cos(2a) + cosh(2b))
        let (two_re, two_im) = (self.re + self.re, self.im + self.im);
        Self::new(two_re.sin(), two_im.sinh()).unscale(two_re.cos() + two_im.cosh())
    }

    /// Computes the principal value of the inverse sine of `self`.
    ///
    /// This function has two branch cuts:
    ///
    /// * `(-∞, -1)`, continuous from above.
    /// * `(1, ∞)`, continuous from below.
    ///
    /// The branch satisfies `-π/2 ≤ Re(asin(z)) ≤ π/2`.
    #[inline]
    pub fn asin(self) -> Self {
        // formula: arcsin(z) = -i ln(sqrt(1-z^2) + iz)
        let i = Self::i();
        -i * ((Self::one() - self * self).sqrt() + i * self).ln()
    }

    /// Computes the principal value of the inverse cosine of `self`.
    ///
    /// This function has two branch cuts:
    ///
    /// * `(-∞, -1)`, continuous from above.
    /// * `(1, ∞)`, continuous from below.
    ///
    /// The branch satisfies `0 ≤ Re(acos(z)) ≤ π`.
    #[inline]
    pub fn acos(self) -> Self {
        // formula: arccos(z) = -i ln(i sqrt(1-z^2) + z)
        let i = Self::i();
        -i * (i * (Self::one() - self * self).sqrt() + self).ln()
    }

    /// Computes the principal value of the inverse tangent of `self`.
    ///
    /// This function has two branch cuts:
    ///
    /// * `(-∞i, -i]`, continuous from the left.
    /// * `[i, ∞i)`, continuous from the right.
    ///
    /// The branch satisfies `-π/2 ≤ Re(atan(z)) ≤ π/2`.
    #[inline]
    pub fn atan(self) -> Self {
        // formula: arctan(z) = (ln(1+iz) - ln(1-iz))/(2i)
        let i = Self::i();
        let one = Self::one();
        let two = one + one;
        if self == i {
            return Self::new(T::zero(), T::infinity());
        } else if self == -i {
            return Self::new(T::zero(), -T::infinity());
        }
        ((one + i * self).ln() - (one - i * self).ln()) / (two * i)
    }

    /// Computes the hyperbolic sine of `self`.
    #[inline]
    pub fn sinh(self) -> Self {
        // formula: sinh(a + bi) = sinh(a)cos(b) + i*cosh(a)sin(b)
        Self::new(
            self.re.sinh() * self.im.cos(),
            self.re.cosh() * self.im.sin(),
        )
    }

    /// Computes the hyperbolic cosine of `self`.
    #[inline]
    pub fn cosh(self) -> Self {
        // formula: cosh(a + bi) = cosh(a)cos(b) + i*sinh(a)sin(b)
        Self::new(
            self.re.cosh() * self.im.cos(),
            self.re.sinh() * self.im.sin(),
        )
    }

    /// Computes the hyperbolic tangent of `self`.
    #[inline]
    pub fn tanh(self) -> Self {
        // formula: tanh(a + bi) = (sinh(2a) + i*sin(2b))/(cosh(2a) + cos(2b))
        let (two_re, two_im) = (self.re + self.re, self.im + self.im);
        Self::new(two_re.sinh(), two_im.sin()).unscale(two_re.cosh() + two_im.cos())
    }

    /// Computes the principal value of inverse hyperbolic sine of `self`.
    ///
    /// This function has two branch cuts:
    ///
    /// * `(-∞i, -i)`, continuous from the left.
    /// * `(i, ∞i)`, continuous from the right.
    ///
    /// The branch satisfies `-π/2 ≤ Im(asinh(z)) ≤ π/2`.
    #[inline]
    pub fn asinh(self) -> Self {
        // formula: arcsinh(z) = ln(z + sqrt(1+z^2))
        let one = Self::one();
        (self + (one + self * self).sqrt()).ln()
    }

    /// Computes the principal value of inverse hyperbolic cosine of `self`.
    ///
    /// This function has one branch cut:
    ///
    /// * `(-∞, 1)`, continuous from above.
    ///
    /// The branch satisfies `-π ≤ Im(acosh(z)) ≤ π` and `0 ≤ Re(acosh(z)) < ∞`.
    #[inline]
    pub fn acosh(self) -> Self {
        // formula: arccosh(z) = 2 ln(sqrt((z+1)/2) + sqrt((z-1)/2))
        let one = Self::one();
        let two = one + one;
        two * (((self + one) / two).sqrt() + ((self - one) / two).sqrt()).ln()
    }

    /// Computes the principal value of inverse hyperbolic tangent of `self`.
    ///
    /// This function has two branch cuts:
    ///
    /// * `(-∞, -1]`, continuous from above.
    /// * `[1, ∞)`, continuous from below.
    ///
    /// The branch satisfies `-π/2 ≤ Im(atanh(z)) ≤ π/2`.
    #[inline]
    pub fn atanh(self) -> Self {
        // formula: arctanh(z) = (ln(1+z) - ln(1-z))/2
        let one = Self::one();
        let two = one + one;
        if self == one {
            return Self::new(T::infinity(), T::zero());
        } else if self == -one {
            return Self::new(-T::infinity(), T::zero());
        }
        ((one + self).ln() - (one - self).ln()) / two
    }

    /// Returns `1/self` using floating-point operations.
    ///
    /// This may be more accurate than the generic `self.inv()` in cases
    /// where `self.norm_sqr()` would overflow to ∞ or underflow to 0.
    ///
    /// # Examples
    ///
    /// ```
    /// use num_complex::Complex64;
    /// let c = Complex64::new(1e300, 1e300);
    ///
    /// // The generic `inv()` will overflow.
    /// assert!(!c.inv().is_normal());
    ///
    /// // But we can do better for `Float` types.
    /// let inv = c.finv();
    /// assert!(inv.is_normal());
    /// println!("{:e}", inv);
    ///
    /// let expected = Complex64::new(5e-301, -5e-301);
    /// assert!((inv - expected).norm() < 1e-315);
    /// ```
    #[inline]
    pub fn finv(self) -> Complex<T> {
        let norm = self.norm();
        self.conj() / norm / norm
    }

    /// Returns `self/other` using floating-point operations.
    ///
    /// This may be more accurate than the generic `Div` implementation in cases
    /// where `other.norm_sqr()` would overflow to ∞ or underflow to 0.
    ///
    /// # Examples
    ///
    /// ```
    /// use num_complex::Complex64;
    /// let a = Complex64::new(2.0, 3.0);
    /// let b = Complex64::new(1e300, 1e300);
    ///
    /// // Generic division will overflow.
    /// assert!(!(a / b).is_normal());
    ///
    /// // But we can do better for `Float` types.
    /// let quotient = a.fdiv(b);
    /// assert!(quotient.is_normal());
    /// println!("{:e}", quotient);
    ///
    /// let expected = Complex64::new(2.5e-300, 5e-301);
    /// assert!((quotient - expected).norm() < 1e-315);
    /// ```
    #[inline]
    pub fn fdiv(self, other: Complex<T>) -> Complex<T> {
        self * other.finv()
    }
}

#[cfg(any(feature = "std", feature = "libm"))]
impl<T: Float + FloatConst> Complex<T> {
    /// Computes `2^(self)`.
    #[inline]
    pub fn exp2(self) -> Self {
        // formula: 2^(a + bi) = 2^a (cos(b*log2) + i*sin(b*log2))
        // = from_polar(2^a, b*log2)
        Self::from_polar(self.re.exp2(), self.im * T::LN_2())
    }

    /// Computes the principal value of log base 2 of `self`.
    #[inline]
    pub fn log2(self) -> Self {
        Self::ln(self) / T::LN_2()
    }

    /// Computes the principal value of log base 10 of `self`.
    #[inline]
    pub fn log10(self) -> Self {
        Self::ln(self) / T::LN_10()
    }
}

impl<T: FloatCore> Complex<T> {
    /// Checks if the given complex number is NaN
    #[inline]
    pub fn is_nan(self) -> bool {
        self.re.is_nan() || self.im.is_nan()
    }

    /// Checks if the given complex number is infinite
    #[inline]
    pub fn is_infinite(self) -> bool {
        !self.is_nan() && (self.re.is_infinite() || self.im.is_infinite())
    }

    /// Checks if the given complex number is finite
    #[inline]
    pub fn is_finite(self) -> bool {
        self.re.is_finite() && self.im.is_finite()
    }

    /// Checks if the given complex number is normal
    #[inline]
    pub fn is_normal(self) -> bool {
        self.re.is_normal() && self.im.is_normal()
    }
}

// Safety: `Complex<T>` is `repr(C)` and contains only instances of `T`, so we
// can guarantee it contains no *added* padding. Thus, if `T: Zeroable`,
// `Complex<T>` is also `Zeroable`
#[cfg(feature = "bytemuck")]
unsafe impl<T: bytemuck::Zeroable> bytemuck::Zeroable for Complex<T> {}

// Safety: `Complex<T>` is `repr(C)` and contains only instances of `T`, so we
// can guarantee it contains no *added* padding. Thus, if `T: Pod`,
// `Complex<T>` is also `Pod`
#[cfg(feature = "bytemuck")]
unsafe impl<T: bytemuck::Pod> bytemuck::Pod for Complex<T> {}

impl<T: Clone + Num> From<T> for Complex<T> {
    #[inline]
    fn from(re: T) -> Self {
        Self::new(re, T::zero())
    }
}

impl<'a, T: Clone + Num> From<&'a T> for Complex<T> {
    #[inline]
    fn from(re: &T) -> Self {
        From::from(re.clone())
    }
}

macro_rules! forward_ref_ref_binop {
    (impl $imp:ident, $method:ident) => {
        impl<'a, 'b, T: Clone + Num> $imp<&'b Complex<T>> for &'a Complex<T> {
            type Output = Complex<T>;

            #[inline]
            fn $method(self, other: &Complex<T>) -> Self::Output {
                self.clone().$method(other.clone())
            }
        }
    };
}

macro_rules! forward_ref_val_binop {
    (impl $imp:ident, $method:ident) => {
        impl<'a, T: Clone + Num> $imp<Complex<T>> for &'a Complex<T> {
            type Output = Complex<T>;

            #[inline]
            fn $method(self, other: Complex<T>) -> Self::Output {
                self.clone().$method(other)
            }
        }
    };
}

macro_rules! forward_val_ref_binop {
    (impl $imp:ident, $method:ident) => {
        impl<'a, T: Clone + Num> $imp<&'a Complex<T>> for Complex<T> {
            type Output = Complex<T>;

            #[inline]
            fn $method(self, other: &Complex<T>) -> Self::Output {
                self.$method(other.clone())
            }
        }
    };
}

macro_rules! forward_all_binop {
    (impl $imp:ident, $method:ident) => {
        forward_ref_ref_binop!(impl $imp, $method);
        forward_ref_val_binop!(impl $imp, $method);
        forward_val_ref_binop!(impl $imp, $method);
    };
}

// arithmetic
forward_all_binop!(impl Add, add);

// (a + i b) + (c + i d) == (a + c) + i (b + d)
impl<T: Clone + Num> Add<Complex<T>> for Complex<T> {
    type Output = Self;

    #[inline]
    fn add(self, other: Self) -> Self::Output {
        Self::Output::new(self.re + other.re, self.im + other.im)
    }
}

forward_all_binop!(impl Sub, sub);

// (a + i b) - (c + i d) == (a - c) + i (b - d)
impl<T: Clone + Num> Sub<Complex<T>> for Complex<T> {
    type Output = Self;

    #[inline]
    fn sub(self, other: Self) -> Self::Output {
        Self::Output::new(self.re - other.re, self.im - other.im)
    }
}

forward_all_binop!(impl Mul, mul);

// (a + i b) * (c + i d) == (a*c - b*d) + i (a*d + b*c)
impl<T: Clone + Num> Mul<Complex<T>> for Complex<T> {
    type Output = Self;

    #[inline]
    fn mul(self, other: Self) -> Self::Output {
        let re = self.re.clone() * other.re.clone() - self.im.clone() * other.im.clone();
        let im = self.re * other.im + self.im * other.re;
        Self::Output::new(re, im)
    }
}

// (a + i b) * (c + i d) + (e + i f) == ((a*c + e) - b*d) + i (a*d + (b*c + f))
impl<T: Clone + Num + MulAdd<Output = T>> MulAdd<Complex<T>> for Complex<T> {
    type Output = Complex<T>;

    #[inline]
    fn mul_add(self, other: Complex<T>, add: Complex<T>) -> Complex<T> {
        let re = self.re.clone().mul_add(other.re.clone(), add.re)
            - (self.im.clone() * other.im.clone()); // FIXME: use mulsub when available in rust
        let im = self.re.mul_add(other.im, self.im.mul_add(other.re, add.im));
        Complex::new(re, im)
    }
}
impl<'a, 'b, T: Clone + Num + MulAdd<Output = T>> MulAdd<&'b Complex<T>> for &'a Complex<T> {
    type Output = Complex<T>;

    #[inline]
    fn mul_add(self, other: &Complex<T>, add: &Complex<T>) -> Complex<T> {
        self.clone().mul_add(other.clone(), add.clone())
    }
}

forward_all_binop!(impl Div, div);

// (a + i b) / (c + i d) == [(a + i b) * (c - i d)] / (c*c + d*d)
//   == [(a*c + b*d) / (c*c + d*d)] + i [(b*c - a*d) / (c*c + d*d)]
impl<T: Clone + Num> Div<Complex<T>> for Complex<T> {
    type Output = Self;

    #[inline]
    fn div(self, other: Self) -> Self::Output {
        let norm_sqr = other.norm_sqr();
        let re = self.re.clone() * other.re.clone() + self.im.clone() * other.im.clone();
        let im = self.im * other.re - self.re * other.im;
        Self::Output::new(re / norm_sqr.clone(), im / norm_sqr)
    }
}

forward_all_binop!(impl Rem, rem);

impl<T: Clone + Num> Complex<T> {
    /// Find the gaussian integer corresponding to the true ratio rounded towards zero.
    fn div_trunc(&self, divisor: &Self) -> Self {
        let Complex { re, im } = self / divisor;
        Complex::new(re.clone() - re % T::one(), im.clone() - im % T::one())
    }
}

impl<T: Clone + Num> Rem<Complex<T>> for Complex<T> {
    type Output = Self;

    #[inline]
    fn rem(self, modulus: Self) -> Self::Output {
        let gaussian = self.div_trunc(&modulus);
        self - modulus * gaussian
    }
}

// Op Assign

mod opassign {
    use core::ops::{AddAssign, DivAssign, MulAssign, RemAssign, SubAssign};

    use num_traits::{MulAddAssign, NumAssign};

    use crate::Complex;

    impl<T: Clone + NumAssign> AddAssign for Complex<T> {
        fn add_assign(&mut self, other: Self) {
            self.re += other.re;
            self.im += other.im;
        }
    }

    impl<T: Clone + NumAssign> SubAssign for Complex<T> {
        fn sub_assign(&mut self, other: Self) {
            self.re -= other.re;
            self.im -= other.im;
        }
    }

    // (a + i b) * (c + i d) == (a*c - b*d) + i (a*d + b*c)
    impl<T: Clone + NumAssign> MulAssign for Complex<T> {
        fn mul_assign(&mut self, other: Self) {
            let a = self.re.clone();

            self.re *= other.re.clone();
            self.re -= self.im.clone() * other.im.clone();

            self.im *= other.re;
            self.im += a * other.im;
        }
    }

    // (a + i b) * (c + i d) + (e + i f) == ((a*c + e) - b*d) + i (b*c + (a*d + f))
    impl<T: Clone + NumAssign + MulAddAssign> MulAddAssign for Complex<T> {
        fn mul_add_assign(&mut self, other: Complex<T>, add: Complex<T>) {
            let a = self.re.clone();

            self.re.mul_add_assign(other.re.clone(), add.re); // (a*c + e)
            self.re -= self.im.clone() * other.im.clone(); // ((a*c + e) - b*d)

            let mut adf = a;
            adf.mul_add_assign(other.im, add.im); // (a*d + f)
            self.im.mul_add_assign(other.re, adf); // (b*c + (a*d + f))
        }
    }

    impl<'a, 'b, T: Clone + NumAssign + MulAddAssign> MulAddAssign<&'a Complex<T>, &'b Complex<T>>
        for Complex<T>
    {
        fn mul_add_assign(&mut self, other: &Complex<T>, add: &Complex<T>) {
            self.mul_add_assign(other.clone(), add.clone());
        }
    }

    // (a + i b) / (c + i d) == [(a + i b) * (c - i d)] / (c*c + d*d)
    //   == [(a*c + b*d) / (c*c + d*d)] + i [(b*c - a*d) / (c*c + d*d)]
    impl<T: Clone + NumAssign> DivAssign for Complex<T> {
        fn div_assign(&mut self, other: Self) {
            let a = self.re.clone();
            let norm_sqr = other.norm_sqr();

            self.re *= other.re.clone();
            self.re += self.im.clone() * other.im.clone();
            self.re /= norm_sqr.clone();

            self.im *= other.re;
            self.im -= a * other.im;
            self.im /= norm_sqr;
        }
    }

    impl<T: Clone + NumAssign> RemAssign for Complex<T> {
        fn rem_assign(&mut self, modulus: Self) {
            let gaussian = self.div_trunc(&modulus);
            *self -= modulus * gaussian;
        }
    }

    impl<T: Clone + NumAssign> AddAssign<T> for Complex<T> {
        fn add_assign(&mut self, other: T) {
            self.re += other;
        }
    }

    impl<T: Clone + NumAssign> SubAssign<T> for Complex<T> {
        fn sub_assign(&mut self, other: T) {
            self.re -= other;
        }
    }

    impl<T: Clone + NumAssign> MulAssign<T> for Complex<T> {
        fn mul_assign(&mut self, other: T) {
            self.re *= other.clone();
            self.im *= other;
        }
    }

    impl<T: Clone + NumAssign> DivAssign<T> for Complex<T> {
        fn div_assign(&mut self, other: T) {
            self.re /= other.clone();
            self.im /= other;
        }
    }

    impl<T: Clone + NumAssign> RemAssign<T> for Complex<T> {
        fn rem_assign(&mut self, other: T) {
            self.re %= other.clone();
            self.im %= other;
        }
    }

    macro_rules! forward_op_assign {
        (impl $imp:ident, $method:ident) => {
            impl<'a, T: Clone + NumAssign> $imp<&'a Complex<T>> for Complex<T> {
                #[inline]
                fn $method(&mut self, other: &Self) {
                    self.$method(other.clone())
                }
            }
            impl<'a, T: Clone + NumAssign> $imp<&'a T> for Complex<T> {
                #[inline]
                fn $method(&mut self, other: &T) {
                    self.$method(other.clone())
                }
            }
        };
    }

    forward_op_assign!(impl AddAssign, add_assign);
    forward_op_assign!(impl SubAssign, sub_assign);
    forward_op_assign!(impl MulAssign, mul_assign);
    forward_op_assign!(impl DivAssign, div_assign);
    forward_op_assign!(impl RemAssign, rem_assign);
}

impl<T: Clone + Num + Neg<Output = T>> Neg for Complex<T> {
    type Output = Self;

    #[inline]
    fn neg(self) -> Self::Output {
        Self::Output::new(-self.re, -self.im)
    }
}

impl<'a, T: Clone + Num + Neg<Output = T>> Neg for &'a Complex<T> {
    type Output = Complex<T>;

    #[inline]
    fn neg(self) -> Self::Output {
        -self.clone()
    }
}

impl<T: Clone + Num + Neg<Output = T>> Inv for Complex<T> {
    type Output = Self;

    #[inline]
    fn inv(self) -> Self::Output {
        Complex::inv(&self)
    }
}

impl<'a, T: Clone + Num + Neg<Output = T>> Inv for &'a Complex<T> {
    type Output = Complex<T>;

    #[inline]
    fn inv(self) -> Self::Output {
        Complex::inv(self)
    }
}

macro_rules! real_arithmetic {
    (@forward $imp:ident::$method:ident for $($real:ident),*) => (
        impl<'a, T: Clone + Num> $imp<&'a T> for Complex<T> {
            type Output = Complex<T>;

            #[inline]
            fn $method(self, other: &T) -> Self::Output {
                self.$method(other.clone())
            }
        }
        impl<'a, T: Clone + Num> $imp<T> for &'a Complex<T> {
            type Output = Complex<T>;

            #[inline]
            fn $method(self, other: T) -> Self::Output {
                self.clone().$method(other)
            }
        }
        impl<'a, 'b, T: Clone + Num> $imp<&'a T> for &'b Complex<T> {
            type Output = Complex<T>;

            #[inline]
            fn $method(self, other: &T) -> Self::Output {
                self.clone().$method(other.clone())
            }
        }
        $(
            impl<'a> $imp<&'a Complex<$real>> for $real {
                type Output = Complex<$real>;

                #[inline]
                fn $method(self, other: &Complex<$real>) -> Complex<$real> {
                    self.$method(other.clone())
                }
            }
            impl<'a> $imp<Complex<$real>> for &'a $real {
                type Output = Complex<$real>;

                #[inline]
                fn $method(self, other: Complex<$real>) -> Complex<$real> {
                    self.clone().$method(other)
                }
            }
            impl<'a, 'b> $imp<&'a Complex<$real>> for &'b $real {
                type Output = Complex<$real>;

                #[inline]
                fn $method(self, other: &Complex<$real>) -> Complex<$real> {
                    self.clone().$method(other.clone())
                }
            }
        )*
    );
    ($($real:ident),*) => (
        real_arithmetic!(@forward Add::add for $($real),*);
        real_arithmetic!(@forward Sub::sub for $($real),*);
        real_arithmetic!(@forward Mul::mul for $($real),*);
        real_arithmetic!(@forward Div::div for $($real),*);
        real_arithmetic!(@forward Rem::rem for $($real),*);

        $(
            impl Add<Complex<$real>> for $real {
                type Output = Complex<$real>;

                #[inline]
                fn add(self, other: Complex<$real>) -> Self::Output {
                    Self::Output::new(self + other.re, other.im)
                }
            }

            impl Sub<Complex<$real>> for $real {
                type Output = Complex<$real>;

                #[inline]
                fn sub(self, other: Complex<$real>) -> Self::Output  {
                    Self::Output::new(self - other.re, $real::zero() - other.im)
                }
            }

            impl Mul<Complex<$real>> for $real {
                type Output = Complex<$real>;

                #[inline]
                fn mul(self, other: Complex<$real>) -> Self::Output {
                    Self::Output::new(self * other.re, self * other.im)
                }
            }

            impl Div<Complex<$real>> for $real {
                type Output = Complex<$real>;

                #[inline]
                fn div(self, other: Complex<$real>) -> Self::Output {
                    // a / (c + i d) == [a * (c - i d)] / (c*c + d*d)
                    let norm_sqr = other.norm_sqr();
                    Self::Output::new(self * other.re / norm_sqr.clone(),
                                      $real::zero() - self * other.im / norm_sqr)
                }
            }

            impl Rem<Complex<$real>> for $real {
                type Output = Complex<$real>;

                #[inline]
                fn rem(self, other: Complex<$real>) -> Self::Output {
                    Self::Output::new(self, Self::zero()) % other
                }
            }
        )*
    );
}

impl<T: Clone + Num> Add<T> for Complex<T> {
    type Output = Complex<T>;

    #[inline]
    fn add(self, other: T) -> Self::Output {
        Self::Output::new(self.re + other, self.im)
    }
}

impl<T: Clone + Num> Sub<T> for Complex<T> {
    type Output = Complex<T>;

    #[inline]
    fn sub(self, other: T) -> Self::Output {
        Self::Output::new(self.re - other, self.im)
    }
}

impl<T: Clone + Num> Mul<T> for Complex<T> {
    type Output = Complex<T>;

    #[inline]
    fn mul(self, other: T) -> Self::Output {
        Self::Output::new(self.re * other.clone(), self.im * other)
    }
}

impl<T: Clone + Num> Div<T> for Complex<T> {
    type Output = Self;

    #[inline]
    fn div(self, other: T) -> Self::Output {
        Self::Output::new(self.re / other.clone(), self.im / other)
    }
}

impl<T: Clone + Num> Rem<T> for Complex<T> {
    type Output = Complex<T>;

    #[inline]
    fn rem(self, other: T) -> Self::Output {
        Self::Output::new(self.re % other.clone(), self.im % other)
    }
}

real_arithmetic!(usize, u8, u16, u32, u64, u128, isize, i8, i16, i32, i64, i128, f32, f64);

// constants
impl<T: ConstZero> Complex<T> {
    /// A constant `Complex` 0.
    pub const ZERO: Self = Self::new(T::ZERO, T::ZERO);
}

impl<T: Clone + Num + ConstZero> ConstZero for Complex<T> {
    const ZERO: Self = Self::ZERO;
}

impl<T: Clone + Num> Zero for Complex<T> {
    #[inline]
    fn zero() -> Self {
        Self::new(Zero::zero(), Zero::zero())
    }

    #[inline]
    fn is_zero(&self) -> bool {
        self.re.is_zero() && self.im.is_zero()
    }

    #[inline]
    fn set_zero(&mut self) {
        self.re.set_zero();
        self.im.set_zero();
    }
}

impl<T: ConstOne + ConstZero> Complex<T> {
    /// A constant `Complex` 1.
    pub const ONE: Self = Self::new(T::ONE, T::ZERO);

    /// A constant `Complex` _i_, the imaginary unit.
    pub const I: Self = Self::new(T::ZERO, T::ONE);
}

impl<T: Clone + Num + ConstOne + ConstZero> ConstOne for Complex<T> {
    const ONE: Self = Self::ONE;
}

impl<T: Clone + Num> One for Complex<T> {
    #[inline]
    fn one() -> Self {
        Self::new(One::one(), Zero::zero())
    }

    #[inline]
    fn is_one(&self) -> bool {
        self.re.is_one() && self.im.is_zero()
    }

    #[inline]
    fn set_one(&mut self) {
        self.re.set_one();
        self.im.set_zero();
    }
}

macro_rules! write_complex {
    ($f:ident, $t:expr, $prefix:expr, $re:expr, $im:expr, $T:ident) => {{
        let abs_re = if $re < Zero::zero() {
            $T::zero() - $re.clone()
        } else {
            $re.clone()
        };
        let abs_im = if $im < Zero::zero() {
            $T::zero() - $im.clone()
        } else {
            $im.clone()
        };

        return if let Some(prec) = $f.precision() {
            fmt_re_im(
                $f,
                $re < $T::zero(),
                $im < $T::zero(),
                format_args!(concat!("{:.1$", $t, "}"), abs_re, prec),
                format_args!(concat!("{:.1$", $t, "}"), abs_im, prec),
            )
        } else {
            fmt_re_im(
                $f,
                $re < $T::zero(),
                $im < $T::zero(),
                format_args!(concat!("{:", $t, "}"), abs_re),
                format_args!(concat!("{:", $t, "}"), abs_im),
            )
        };

        fn fmt_re_im(
            f: &mut fmt::Formatter<'_>,
            re_neg: bool,
            im_neg: bool,
            real: fmt::Arguments<'_>,
            imag: fmt::Arguments<'_>,
        ) -> fmt::Result {
            let prefix = if f.alternate() { $prefix } else { "" };
            let sign = if re_neg {
                "-"
            } else if f.sign_plus() {
                "+"
            } else {
                ""
            };

            if im_neg {
                fmt_complex(
                    f,
                    format_args!(
                        "{}{pre}{re}-{pre}{im}i",
                        sign,
                        re = real,
                        im = imag,
                        pre = prefix
                    ),
                )
            } else {
                fmt_complex(
                    f,
                    format_args!(
                        "{}{pre}{re}+{pre}{im}i",
                        sign,
                        re = real,
                        im = imag,
                        pre = prefix
                    ),
                )
            }
        }

        #[cfg(feature = "std")]
        // Currently, we can only apply width using an intermediate `String` (and thus `std`)
        fn fmt_complex(f: &mut fmt::Formatter<'_>, complex: fmt::Arguments<'_>) -> fmt::Result {
            use std::string::ToString;
            if let Some(width) = f.width() {
                write!(f, "{0: >1$}", complex.to_string(), width)
            } else {
                write!(f, "{}", complex)
            }
        }

        #[cfg(not(feature = "std"))]
        fn fmt_complex(f: &mut fmt::Formatter<'_>, complex: fmt::Arguments<'_>) -> fmt::Result {
            write!(f, "{}", complex)
        }
    }};
}

// string conversions
impl<T> fmt::Display for Complex<T>
where
    T: fmt::Display + Num + PartialOrd + Clone,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_complex!(f, "", "", self.re, self.im, T)
    }
}

impl<T> fmt::LowerExp for Complex<T>
where
    T: fmt::LowerExp + Num + PartialOrd + Clone,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_complex!(f, "e", "", self.re, self.im, T)
    }
}

impl<T> fmt::UpperExp for Complex<T>
where
    T: fmt::UpperExp + Num + PartialOrd + Clone,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_complex!(f, "E", "", self.re, self.im, T)
    }
}

impl<T> fmt::LowerHex for Complex<T>
where
    T: fmt::LowerHex + Num + PartialOrd + Clone,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_complex!(f, "x", "0x", self.re, self.im, T)
    }
}

impl<T> fmt::UpperHex for Complex<T>
where
    T: fmt::UpperHex + Num + PartialOrd + Clone,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_complex!(f, "X", "0x", self.re, self.im, T)
    }
}

impl<T> fmt::Octal for Complex<T>
where
    T: fmt::Octal + Num + PartialOrd + Clone,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_complex!(f, "o", "0o", self.re, self.im, T)
    }
}

impl<T> fmt::Binary for Complex<T>
where
    T: fmt::Binary + Num + PartialOrd + Clone,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_complex!(f, "b", "0b", self.re, self.im, T)
    }
}

fn from_str_generic<T, E, F>(s: &str, from: F) -> Result<Complex<T>, ParseComplexError<E>>
where
    F: Fn(&str) -> Result<T, E>,
    T: Clone + Num,
{
    let imag = match s.rfind('j') {
        None => 'i',
        _ => 'j',
    };

    let mut neg_b = false;
    let mut a = s;
    let mut b = "";

    for (i, w) in s.as_bytes().windows(2).enumerate() {
        let p = w[0];
        let c = w[1];

        // ignore '+'/'-' if part of an exponent
        if (c == b'+' || c == b'-') && !(p == b'e' || p == b'E') {
            // trim whitespace around the separator
            a = s[..=i].trim_end_matches(char::is_whitespace);
            b = s[i + 2..].trim_start_matches(char::is_whitespace);
            neg_b = c == b'-';

            if b.is_empty() || (neg_b && b.starts_with('-')) {
                return Err(ParseComplexError::expr_error());
            }
            break;
        }
    }

    // split off real and imaginary parts
    if b.is_empty() {
        // input was either pure real or pure imaginary
        b = if a.ends_with(imag) { "0" } else { "0i" };
    }

    let re;
    let neg_re;
    let im;
    let neg_im;
    if a.ends_with(imag) {
        im = a;
        neg_im = false;
        re = b;
        neg_re = neg_b;
    } else if b.ends_with(imag) {
        re = a;
        neg_re = false;
        im = b;
        neg_im = neg_b;
    } else {
        return Err(ParseComplexError::expr_error());
    }

    // parse re
    let re = from(re).map_err(ParseComplexError::from_error)?;
    let re = if neg_re { T::zero() - re } else { re };

    // pop imaginary unit off
    let mut im = &im[..im.len() - 1];
    // handle im == "i" or im == "-i"
    if im.is_empty() || im == "+" {
        im = "1";
    } else if im == "-" {
        im = "-1";
    }

    // parse im
    let im = from(im).map_err(ParseComplexError::from_error)?;
    let im = if neg_im { T::zero() - im } else { im };

    Ok(Complex::new(re, im))
}

impl<T> FromStr for Complex<T>
where
    T: FromStr + Num + Clone,
{
    type Err = ParseComplexError<T::Err>;

    /// Parses `a +/- bi`; `ai +/- b`; `a`; or `bi` where `a` and `b` are of type `T`
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        from_str_generic(s, T::from_str)
    }
}

impl<T: Num + Clone> Num for Complex<T> {
    type FromStrRadixErr = ParseComplexError<T::FromStrRadixErr>;

    /// Parses `a +/- bi`; `ai +/- b`; `a`; or `bi` where `a` and `b` are of type `T`
    ///
    /// `radix` must be <= 18; larger radix would include *i* and *j* as digits,
    /// which cannot be supported.
    ///
    /// The conversion returns an error if 18 <= radix <= 36; it panics if radix > 36.
    ///
    /// The elements of `T` are parsed using `Num::from_str_radix` too, and errors
    /// (or panics) from that are reflected here as well.
    fn from_str_radix(s: &str, radix: u32) -> Result<Self, Self::FromStrRadixErr> {
        assert!(
            radix <= 36,
            "from_str_radix: radix is too high (maximum 36)"
        );

        // larger radix would include 'i' and 'j' as digits, which cannot be supported
        if radix > 18 {
            return Err(ParseComplexError::unsupported_radix());
        }

        from_str_generic(s, |x| -> Result<T, T::FromStrRadixErr> {
            T::from_str_radix(x, radix)
        })
    }
}

impl<T: Num + Clone> Sum for Complex<T> {
    fn sum<I>(iter: I) -> Self
    where
        I: Iterator<Item = Self>,
    {
        iter.fold(Self::zero(), |acc, c| acc + c)
    }
}

impl<'a, T: 'a + Num + Clone> Sum<&'a Complex<T>> for Complex<T> {
    fn sum<I>(iter: I) -> Self
    where
        I: Iterator<Item = &'a Complex<T>>,
    {
        iter.fold(Self::zero(), |acc, c| acc + c)
    }
}

impl<T: Num + Clone> Product for Complex<T> {
    fn product<I>(iter: I) -> Self
    where
        I: Iterator<Item = Self>,
    {
        iter.fold(Self::one(), |acc, c| acc * c)
    }
}

impl<'a, T: 'a + Num + Clone> Product<&'a Complex<T>> for Complex<T> {
    fn product<I>(iter: I) -> Self
    where
        I: Iterator<Item = &'a Complex<T>>,
    {
        iter.fold(Self::one(), |acc, c| acc * c)
    }
}

#[cfg(feature = "serde")]
impl<T> serde::Serialize for Complex<T>
where
    T: serde::Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        (&self.re, &self.im).serialize(serializer)
    }
}

#[cfg(feature = "serde")]
impl<'de, T> serde::Deserialize<'de> for Complex<T>
where
    T: serde::Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let (re, im) = serde::Deserialize::deserialize(deserializer)?;
        Ok(Self::new(re, im))
    }
}

#[derive(Debug, PartialEq)]
pub struct ParseComplexError<E> {
    kind: ComplexErrorKind<E>,
}

#[derive(Debug, PartialEq)]
enum ComplexErrorKind<E> {
    ParseError(E),
    ExprError,
    UnsupportedRadix,
}

impl<E> ParseComplexError<E> {
    fn expr_error() -> Self {
        ParseComplexError {
            kind: ComplexErrorKind::ExprError,
        }
    }

    fn unsupported_radix() -> Self {
        ParseComplexError {
            kind: ComplexErrorKind::UnsupportedRadix,
        }
    }

    fn from_error(error: E) -> Self {
        ParseComplexError {
            kind: ComplexErrorKind::ParseError(error),
        }
    }
}

#[cfg(feature = "std")]
impl<E: Error> Error for ParseComplexError<E> {
    #[allow(deprecated)]
    fn description(&self) -> &str {
        match self.kind {
            ComplexErrorKind::ParseError(ref e) => e.description(),
            ComplexErrorKind::ExprError => "invalid or unsupported complex expression",
            ComplexErrorKind::UnsupportedRadix => "unsupported radix for conversion",
        }
    }
}

impl<E: fmt::Display> fmt::Display for ParseComplexError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            ComplexErrorKind::ParseError(ref e) => e.fmt(f),
            ComplexErrorKind::ExprError => "invalid or unsupported complex expression".fmt(f),
            ComplexErrorKind::UnsupportedRadix => "unsupported radix for conversion".fmt(f),
        }
    }
}

#[cfg(test)]
fn hash<T: hash::Hash>(x: &T) -> u64 {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};
    let mut hasher = <RandomState as BuildHasher>::Hasher::new();
    x.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
pub(crate) mod test {
    #![allow(non_upper_case_globals)]

    use super::{Complex, Complex64};
    use super::{ComplexErrorKind, ParseComplexError};
    use core::f64;
    use core::str::FromStr;

    use std::string::{String, ToString};

    use num_traits::{Num, One, Zero};

    pub const _0_0i: Complex64 = Complex::new(0.0, 0.0);
    pub const _1_0i: Complex64 = Complex::new(1.0, 0.0);
    pub const _1_1i: Complex64 = Complex::new(1.0, 1.0);
    pub const _0_1i: Complex64 = Complex::new(0.0, 1.0);
    pub const _neg1_1i: Complex64 = Complex::new(-1.0, 1.0);
    pub const _05_05i: Complex64 = Complex::new(0.5, 0.5);
    pub const all_consts: [Complex64; 5] = [_0_0i, _1_0i, _1_1i, _neg1_1i, _05_05i];
    pub const _4_2i: Complex64 = Complex::new(4.0, 2.0);
    pub const _1_infi: Complex64 = Complex::new(1.0, f64::INFINITY);
    pub const _neg1_infi: Complex64 = Complex::new(-1.0, f64::INFINITY);
    pub const _1_nani: Complex64 = Complex::new(1.0, f64::NAN);
    pub const _neg1_nani: Complex64 = Complex::new(-1.0, f64::NAN);
    pub const _inf_0i: Complex64 = Complex::new(f64::INFINITY, 0.0);
    pub const _neginf_1i: Complex64 = Complex::new(f64::NEG_INFINITY, 1.0);
    pub const _neginf_neg1i: Complex64 = Complex::new(f64::NEG_INFINITY, -1.0);
    pub const _inf_1i: Complex64 = Complex::new(f64::INFINITY, 1.0);
    pub const _inf_neg1i: Complex64 = Complex::new(f64::INFINITY, -1.0);
    pub const _neginf_infi: Complex64 = Complex::new(f64::NEG_INFINITY, f64::INFINITY);
    pub const _inf_infi: Complex64 = Complex::new(f64::INFINITY, f64::INFINITY);
    pub const _neginf_nani: Complex64 = Complex::new(f64::NEG_INFINITY, f64::NAN);
    pub const _inf_nani: Complex64 = Complex::new(f64::INFINITY, f64::NAN);
    pub const _nan_0i: Complex64 = Complex::new(f64::NAN, 0.0);
    pub const _nan_1i: Complex64 = Complex::new(f64::NAN, 1.0);
    pub const _nan_neg1i: Complex64 = Complex::new(f64::NAN, -1.0);
    pub const _nan_nani: Complex64 = Complex::new(f64::NAN, f64::NAN);

    #[test]
    fn test_consts() {
        // check our constants are what Complex::new creates
        fn test(c: Complex64, r: f64, i: f64) {
            assert_eq!(c, Complex::new(r, i));
        }
        test(_0_0i, 0.0, 0.0);
        test(_1_0i, 1.0, 0.0);
        test(_1_1i, 1.0, 1.0);
        test(_neg1_1i, -1.0, 1.0);
        test(_05_05i, 0.5, 0.5);

        assert_eq!(_0_0i, Zero::zero());
        assert_eq!(_1_0i, One::one());
    }

    #[test]
    fn test_scale_unscale() {
        assert_eq!(_05_05i.scale(2.0), _1_1i);
        assert_eq!(_1_1i.unscale(2.0), _05_05i);
        for &c in all_consts.iter() {
            assert_eq!(c.scale(2.0).unscale(2.0), c);
        }
    }

    #[test]
    fn test_conj() {
        for &c in all_consts.iter() {
            assert_eq!(c.conj(), Complex::new(c.re, -c.im));
            assert_eq!(c.conj().conj(), c);
        }
    }

    #[test]
    fn test_inv() {
        assert_eq!(_1_1i.inv(), _05_05i.conj());
        assert_eq!(_1_0i.inv(), _1_0i.inv());
    }

    #[test]
    #[should_panic]
    fn test_divide_by_zero_natural() {
        let n = Complex::new(2, 3);
        let d = Complex::new(0, 0);
        let _x = n / d;
    }

    #[test]
    fn test_inv_zero() {
        // FIXME #20: should this really fail, or just NaN?
        assert!(_0_0i.inv().is_nan());
    }

    #[test]
    #[allow(clippy::float_cmp)]
    fn test_l1_norm() {
        assert_eq!(_0_0i.l1_norm(), 0.0);
        assert_eq!(_1_0i.l1_norm(), 1.0);
        assert_eq!(_1_1i.l1_norm(), 2.0);
        assert_eq!(_0_1i.l1_norm(), 1.0);
        assert_eq!(_neg1_1i.l1_norm(), 2.0);
        assert_eq!(_05_05i.l1_norm(), 1.0);
        assert_eq!(_4_2i.l1_norm(), 6.0);
    }

    #[test]
    fn test_pow() {
        for c in all_consts.iter() {
            assert_eq!(c.powi(0), _1_0i);
            let mut pos = _1_0i;
            let mut neg = _1_0i;
            for i in 1i32..20 {
                pos *= c;
                assert_eq!(pos, c.powi(i));
                if c.is_zero() {
                    assert!(c.powi(-i).is_nan());
                } else {
                    neg /= c;
                    assert_eq!(neg, c.powi(-i));
                }
            }
        }
    }

    #[cfg(any(feature = "std", feature = "libm"))]
    pub(crate) mod float {

        use core::f64::INFINITY;

        use super::*;
        use num_traits::{Float, Pow};

        #[test]
        fn test_cis() {
            assert!(close(Complex::cis(0.0 * f64::consts::PI), _1_0i));
            assert!(close(Complex::cis(0.5 * f64::consts::PI), _0_1i));
            assert!(close(Complex::cis(1.0 * f64::consts::PI), -_1_0i));
            assert!(close(Complex::cis(1.5 * f64::consts::PI), -_0_1i));
            assert!(close(Complex::cis(2.0 * f64::consts::PI), _1_0i));
        }

        #[test]
        #[cfg_attr(target_arch = "x86", ignore)]
        // FIXME #7158: (maybe?) currently failing on x86.
        #[allow(clippy::float_cmp)]
        fn test_norm() {
            fn test(c: Complex64, ns: f64) {
                assert_eq!(c.norm_sqr(), ns);
                assert_eq!(c.norm(), ns.sqrt())
            }
            test(_0_0i, 0.0);
            test(_1_0i, 1.0);
            test(_1_1i, 2.0);
            test(_neg1_1i, 2.0);
            test(_05_05i, 0.5);
        }

        #[test]
        fn test_arg() {
            fn test(c: Complex64, arg: f64) {
                assert!((c.arg() - arg).abs() < 1.0e-6)
            }
            test(_1_0i, 0.0);
            test(_1_1i, 0.25 * f64::consts::PI);
            test(_neg1_1i, 0.75 * f64::consts::PI);
            test(_05_05i, 0.25 * f64::consts::PI);
        }

        #[test]
        fn test_polar_conv() {
            fn test(c: Complex64) {
                let (r, theta) = c.to_polar();
                assert!((c - Complex::from_polar(r, theta)).norm() < 1e-6);
            }
            for &c in all_consts.iter() {
                test(c);
            }
        }

        pub(crate) fn close(a: Complex64, b: Complex64) -> bool {
            close_to_tol(a, b, 1e-10)
        }

        fn close_to_tol(a: Complex64, b: Complex64, tol: f64) -> bool {
            // returns true if a and b are reasonably close
            let close = (a == b) || (a - b).norm() < tol;
            if !close {
                println!("{:?} != {:?}", a, b);
            }
            close
        }

        // Version that also works if re or im are +inf, -inf, or nan
        fn close_naninf(a: Complex64, b: Complex64) -> bool {
            close_naninf_to_tol(a, b, 1.0e-10)
        }

        fn close_naninf_to_tol(a: Complex64, b: Complex64, tol: f64) -> bool {
            let mut close = true;

            // Compare the real parts
            if a.re.is_finite() {
                if b.re.is_finite() {
                    close = (a.re == b.re) || (a.re - b.re).abs() < tol;
                } else {
                    close = false;
                }
            } else if (a.re.is_nan() && !b.re.is_nan())
                || (a.re.is_infinite()
                    && a.re.is_sign_positive()
                    && !(b.re.is_infinite() && b.re.is_sign_positive()))
                || (a.re.is_infinite()
                    && a.re.is_sign_negative()
                    && !(b.re.is_infinite() && b.re.is_sign_negative()))
            {
                close = false;
            }

            // Compare the imaginary parts
            if a.im.is_finite() {
                if b.im.is_finite() {
                    close &= (a.im == b.im) || (a.im - b.im).abs() < tol;
                } else {
                    close = false;
                }
            } else if (a.im.is_nan() && !b.im.is_nan())
                || (a.im.is_infinite()
                    && a.im.is_sign_positive()
                    && !(b.im.is_infinite() && b.im.is_sign_positive()))
                || (a.im.is_infinite()
                    && a.im.is_sign_negative()
                    && !(b.im.is_infinite() && b.im.is_sign_negative()))
            {
                close = false;
            }

            if close == false {
                println!("{:?} != {:?}", a, b);
            }
            close
        }

        #[test]
        fn test_exp2() {
            assert!(close(_0_0i.exp2(), _1_0i));
        }

        #[test]
        fn test_exp() {
            assert!(close(_1_0i.exp(), _1_0i.scale(f64::consts::E)));
            assert!(close(_0_0i.exp(), _1_0i));
            assert!(close(_0_1i.exp(), Complex::new(1.0.cos(), 1.0.sin())));
            assert!(close(_05_05i.exp() * _05_05i.exp(), _1_1i.exp()));
            assert!(close(
                _0_1i.scale(-f64::consts::PI).exp(),
                _1_0i.scale(-1.0)
            ));
            for &c in all_consts.iter() {
                // e^conj(z) = conj(e^z)
                assert!(close(c.conj().exp(), c.exp().conj()));
                // e^(z + 2 pi i) = e^z
                assert!(close(
                    c.exp(),
                    (c + _0_1i.scale(f64::consts::PI * 2.0)).exp()
                ));
            }

            // The test values below were taken from https://en.cppreference.com/w/cpp/numeric/complex/exp
            assert!(close_naninf(_1_infi.exp(), _nan_nani));
            assert!(close_naninf(_neg1_infi.exp(), _nan_nani));
            assert!(close_naninf(_1_nani.exp(), _nan_nani));
            assert!(close_naninf(_neg1_nani.exp(), _nan_nani));
            assert!(close_naninf(_inf_0i.exp(), _inf_0i));
            assert!(close_naninf(_neginf_1i.exp(), 0.0 * Complex::cis(1.0)));
            assert!(close_naninf(_neginf_neg1i.exp(), 0.0 * Complex::cis(-1.0)));
            assert!(close_naninf(
                _inf_1i.exp(),
                f64::INFINITY * Complex::cis(1.0)
            ));
            assert!(close_naninf(
                _inf_neg1i.exp(),
                f64::INFINITY * Complex::cis(-1.0)
            ));
            assert!(close_naninf(_neginf_infi.exp(), _0_0i)); // Note: ±0±0i: signs of zeros are unspecified
            assert!(close_naninf(_inf_infi.exp(), _inf_nani)); // Note: ±∞+NaN*i: sign of the real part is unspecified
            assert!(close_naninf(_neginf_nani.exp(), _0_0i)); // Note: ±0±0i: signs of zeros are unspecified
            assert!(close_naninf(_inf_nani.exp(), _inf_nani)); // Note: ±∞+NaN*i: sign of the real part is unspecified
            assert!(close_naninf(_nan_0i.exp(), _nan_0i));
            assert!(close_naninf(_nan_1i.exp(), _nan_nani));
            assert!(close_naninf(_nan_neg1i.exp(), _nan_nani));
            assert!(close_naninf(_nan_nani.exp(), _nan_nani));
        }

        #[test]
        fn test_ln() {
            assert!(close(_1_0i.ln(), _0_0i));
            assert!(close(_0_1i.ln(), _0_1i.scale(f64::consts::PI / 2.0)));
            assert!(close(_0_0i.ln(), Complex::new(f64::neg_infinity(), 0.0)));
            assert!(close(
                (_neg1_1i * _05_05i).ln(),
                _neg1_1i.ln() + _05_05i.ln()
            ));
            for &c in all_consts.iter() {
                // ln(conj(z() = conj(ln(z))
                assert!(close(c.conj().ln(), c.ln().conj()));
                // for this branch, -pi <= arg(ln(z)) <= pi
                assert!(-f64::consts::PI <= c.ln().arg() && c.ln().arg() <= f64::consts::PI);
            }
        }

        #[test]
        fn test_powc() {
            let a = Complex::new(2.0, -3.0);
            let b = Complex::new(3.0, 0.0);
            assert!(close(a.powc(b), a.powf(b.re)));
            assert!(close(b.powc(a), a.expf(b.re)));
            let c = Complex::new(1.0 / 3.0, 0.1);
            assert!(close_to_tol(
                a.powc(c),
                Complex::new(1.65826, -0.33502),
                1e-5
            ));
            let z = Complex::new(0.0, 0.0);
            assert!(close(z.powc(b), z));
            assert!(z.powc(Complex64::new(0., INFINITY)).is_nan());
            assert!(z.powc(Complex64::new(10., INFINITY)).is_nan());
            assert!(z.powc(Complex64::new(INFINITY, INFINITY)).is_nan());
            assert!(close(z.powc(Complex64::new(INFINITY, 0.)), z));
            assert!(z.powc(Complex64::new(-1., 0.)).re.is_infinite());
            assert!(z.powc(Complex64::new(-1., 0.)).im.is_nan());

            for c in all_consts.iter() {
                assert_eq!(c.powc(_0_0i), _1_0i);
            }
            assert_eq!(_nan_nani.powc(_0_0i), _1_0i);
        }

        #[test]
        fn test_powf() {
            let c = Complex64::new(2.0, -1.0);
            let expected = Complex64::new(-0.8684746, -16.695934);
            assert!(close_to_tol(c.powf(3.5), expected, 1e-5));
            assert!(close_to_tol(Pow::pow(c, 3.5_f64), expected, 1e-5));
            assert!(close_to_tol(Pow::pow(c, 3.5_f32), expected, 1e-5));

            for c in all_consts.iter() {
                assert_eq!(c.powf(0.0), _1_0i);
            }
            assert_eq!(_nan_nani.powf(0.0), _1_0i);
        }

        #[test]
        fn test_log() {
            let c = Complex::new(2.0, -1.0);
            let r = c.log(10.0);
            assert!(close_to_tol(r, Complex::new(0.349485, -0.20135958), 1e-5));
        }

        #[test]
        fn test_log2() {
            assert!(close(_1_0i.log2(), _0_0i));
        }

        #[test]
        fn test_log10() {
            assert!(close(_1_0i.log10(), _0_0i));
        }

        #[test]
        fn test_some_expf_cases() {
            let c = Complex::new(2.0, -1.0);
            let r = c.expf(10.0);
            assert!(close_to_tol(r, Complex::new(-66.82015, -74.39803), 1e-5));

            let c = Complex::new(5.0, -2.0);
            let r = c.expf(3.4);
            assert!(close_to_tol(r, Complex::new(-349.25, -290.63), 1e-2));

            let c = Complex::new(-1.5, 2.0 / 3.0);
            let r = c.expf(1.0 / 3.0);
            assert!(close_to_tol(r, Complex::new(3.8637, -3.4745), 1e-2));
        }

        #[test]
        fn test_sqrt() {
            assert!(close(_0_0i.sqrt(), _0_0i));
            assert!(close(_1_0i.sqrt(), _1_0i));
            assert!(close(Complex::new(-1.0, 0.0).sqrt(), _0_1i));
            assert!(close(Complex::new(-1.0, -0.0).sqrt(), _0_1i.scale(-1.0)));
            assert!(close(_0_1i.sqrt(), _05_05i.scale(2.0.sqrt())));
            for &c in all_consts.iter() {
                // sqrt(conj(z() = conj(sqrt(z))
                assert!(close(c.conj().sqrt(), c.sqrt().conj()));
                // for this branch, -pi/2 <= arg(sqrt(z)) <= pi/2
                assert!(
                    -f64::consts::FRAC_PI_2 <= c.sqrt().arg()
                        && c.sqrt().arg() <= f64::consts::FRAC_PI_2
                );
                // sqrt(z) * sqrt(z) = z
                assert!(close(c.sqrt() * c.sqrt(), c));
            }
        }

        #[test]
        fn test_sqrt_real() {
            for n in (0..100).map(f64::from) {
                // √(n² + 0i) = n + 0i
                let n2 = n * n;
                assert_eq!(Complex64::new(n2, 0.0).sqrt(), Complex64::new(n, 0.0));
                // √(-n² + 0i) = 0 + ni
                assert_eq!(Complex64::new(-n2, 0.0).sqrt(), Complex64::new(0.0, n));
                // √(-n² - 0i) = 0 - ni
                assert_eq!(Complex64::new(-n2, -0.0).sqrt(), Complex64::new(0.0, -n));
            }
        }

        #[test]
        fn test_sqrt_imag() {
            for n in (0..100).map(f64::from) {
                // √(0 + n²i) = n e^(iπ/4)
                let n2 = n * n;
                assert!(close(
                    Complex64::new(0.0, n2).sqrt(),
                    Complex64::from_polar(n, f64::consts::FRAC_PI_4)
                ));
                // √(0 - n²i) = n e^(-iπ/4)
                assert!(close(
                    Complex64::new(0.0, -n2).sqrt(),
                    Complex64::from_polar(n, -f64::consts::FRAC_PI_4)
                ));
            }
        }

        #[test]
        fn test_cbrt() {
            assert!(close(_0_0i.cbrt(), _0_0i));
            assert!(close(_1_0i.cbrt(), _1_0i));
            assert!(close(
                Complex::new(-1.0, 0.0).cbrt(),
                Complex::new(0.5, 0.75.sqrt())
            ));
            assert!(close(
                Complex::new(-1.0, -0.0).cbrt(),
                Complex::new(0.5, -(0.75.sqrt()))
            ));
            assert!(close(_0_1i.cbrt(), Complex::new(0.75.sqrt(), 0.5)));
            assert!(close(_0_1i.conj().cbrt(), Complex::new(0.75.sqrt(), -0.5)));
            for &c in all_consts.iter() {
                // cbrt(conj(z() = conj(cbrt(z))
                assert!(close(c.conj().cbrt(), c.cbrt().conj()));
                // for this branch, -pi/3 <= arg(cbrt(z)) <= pi/3
                assert!(
                    -f64::consts::FRAC_PI_3 <= c.cbrt().arg()
                        && c.cbrt().arg() <= f64::consts::FRAC_PI_3
                );
                // cbrt(z) * cbrt(z) cbrt(z) = z
                assert!(close(c.cbrt() * c.cbrt() * c.cbrt(), c));
            }
        }

        #[test]
        fn test_cbrt_real() {
            for n in (0..100).map(f64::from) {
                // ∛(n³ + 0i) = n + 0i
                let n3 = n * n * n;
                assert!(close(
                    Complex64::new(n3, 0.0).cbrt(),
                    Complex64::new(n, 0.0)
                ));
                // ∛(-n³ + 0i) = n e^(iπ/3)
                assert!(close(
                    Complex64::new(-n3, 0.0).cbrt(),
                    Complex64::from_polar(n, f64::consts::FRAC_PI_3)
                ));
                // ∛(-n³ - 0i) = n e^(-iπ/3)
                assert!(close(
                    Complex64::new(-n3, -0.0).cbrt(),
                    Complex64::from_polar(n, -f64::consts::FRAC_PI_3)
                ));
            }
        }

        #[test]
        fn test_cbrt_imag() {
            for n in (0..100).map(f64::from) {
                // ∛(0 + n³i) = n e^(iπ/6)
                let n3 = n * n * n;
                assert!(close(
                    Complex64::new(0.0, n3).cbrt(),
                    Complex64::from_polar(n, f64::consts::FRAC_PI_6)
                ));
                // ∛(0 - n³i) = n e^(-iπ/6)
                assert!(close(
                    Complex64::new(0.0, -n3).cbrt(),
                    Complex64::from_polar(n, -f64::consts::FRAC_PI_6)
                ));
            }
        }

        #[test]
        fn test_sin() {
            assert!(close(_0_0i.sin(), _0_0i));
            assert!(close(_1_0i.scale(f64::consts::PI * 2.0).sin(), _0_0i));
            assert!(close(_0_1i.sin(), _0_1i.scale(1.0.sinh())));
            for &c in all_consts.iter() {
                // sin(conj(z)) = conj(sin(z))
                assert!(close(c.conj().sin(), c.sin().conj()));
                // sin(-z) = -sin(z)
                assert!(close(c.scale(-1.0).sin(), c.sin().scale(-1.0)));
            }
        }

        #[test]
        fn test_cos() {
            assert!(close(_0_0i.cos(), _1_0i));
            assert!(close(_1_0i.scale(f64::consts::PI * 2.0).cos(), _1_0i));
            assert!(close(_0_1i.cos(), _1_0i.scale(1.0.cosh())));
            for &c in all_consts.iter() {
                // cos(conj(z)) = conj(cos(z))
                assert!(close(c.conj().cos(), c.cos().conj()));
                // cos(-z) = cos(z)
                assert!(close(c.scale(-1.0).cos(), c.cos()));
            }
        }

        #[test]
        fn test_tan() {
            assert!(close(_0_0i.tan(), _0_0i));
            assert!(close(_1_0i.scale(f64::consts::PI / 4.0).tan(), _1_0i));
            assert!(close(_1_0i.scale(f64::consts::PI).tan(), _0_0i));
            for &c in all_consts.iter() {
                // tan(conj(z)) = conj(tan(z))
                assert!(close(c.conj().tan(), c.tan().conj()));
                // tan(-z) = -tan(z)
                assert!(close(c.scale(-1.0).tan(), c.tan().scale(-1.0)));
            }
        }

        #[test]
        fn test_asin() {
            assert!(close(_0_0i.asin(), _0_0i));
            assert!(close(_1_0i.asin(), _1_0i.scale(f64::consts::PI / 2.0)));
            assert!(close(
                _1_0i.scale(-1.0).asin(),
                _1_0i.scale(-f64::consts::PI / 2.0)
            ));
            assert!(close(_0_1i.asin(), _0_1i.scale((1.0 + 2.0.sqrt()).ln())));
            for &c in all_consts.iter() {
                // asin(conj(z)) = conj(asin(z))
                assert!(close(c.conj().asin(), c.asin().conj()));
                // asin(-z) = -asin(z)
                assert!(close(c.scale(-1.0).asin(), c.asin().scale(-1.0)));
                // for this branch, -pi/2 <= asin(z).re <= pi/2
                assert!(
                    -f64::consts::PI / 2.0 <= c.asin().re && c.asin().re <= f64::consts::PI / 2.0
                );
            }
        }

        #[test]
        fn test_acos() {
            assert!(close(_0_0i.acos(), _1_0i.scale(f64::consts::PI / 2.0)));
            assert!(close(_1_0i.acos(), _0_0i));
            assert!(close(
                _1_0i.scale(-1.0).acos(),
                _1_0i.scale(f64::consts::PI)
            ));
            assert!(close(
                _0_1i.acos(),
                Complex::new(f64::consts::PI / 2.0, (2.0.sqrt() - 1.0).ln())
            ));
            for &c in all_consts.iter() {
                // acos(conj(z)) = conj(acos(z))
                assert!(close(c.conj().acos(), c.acos().conj()));
                // for this branch, 0 <= acos(z).re <= pi
                assert!(0.0 <= c.acos().re && c.acos().re <= f64::consts::PI);
            }
        }

        #[test]
        fn test_atan() {
            assert!(close(_0_0i.atan(), _0_0i));
            assert!(close(_1_0i.atan(), _1_0i.scale(f64::consts::PI / 4.0)));
            assert!(close(
                _1_0i.scale(-1.0).atan(),
                _1_0i.scale(-f64::consts::PI / 4.0)
            ));
            assert!(close(_0_1i.atan(), Complex::new(0.0, f64::infinity())));
            for &c in all_consts.iter() {
                // atan(conj(z)) = conj(atan(z))
                assert!(close(c.conj().atan(), c.atan().conj()));
                // atan(-z) = -atan(z)
                assert!(close(c.scale(-1.0).atan(), c.atan().scale(-1.0)));
                // for this branch, -pi/2 <= atan(z).re <= pi/2
                assert!(
                    -f64::consts::PI / 2.0 <= c.atan().re && c.atan().re <= f64::consts::PI / 2.0
                );
            }
        }

        #[test]
        fn test_sinh() {
            assert!(close(_0_0i.sinh(), _0_0i));
            assert!(close(
                _1_0i.sinh(),
                _1_0i.scale((f64::consts::E - 1.0 / f64::consts::E) / 2.0)
            ));
            assert!(close(_0_1i.sinh(), _0_1i.scale(1.0.sin())));
            for &c in all_consts.iter() {
                // sinh(conj(z)) = conj(sinh(z))
                assert!(close(c.conj().sinh(), c.sinh().conj()));
                // sinh(-z) = -sinh(z)
                assert!(close(c.scale(-1.0).sinh(), c.sinh().scale(-1.0)));
            }
        }

        #[test]
        fn test_cosh() {
            assert!(close(_0_0i.cosh(), _1_0i));
            assert!(close(
                _1_0i.cosh(),
                _1_0i.scale((f64::consts::E + 1.0 / f64::consts::E) / 2.0)
            ));
            assert!(close(_0_1i.cosh(), _1_0i.scale(1.0.cos())));
            for &c in all_consts.iter() {
                // cosh(conj(z)) = conj(cosh(z))
                assert!(close(c.conj().cosh(), c.cosh().conj()));
                // cosh(-z) = cosh(z)
                assert!(close(c.scale(-1.0).cosh(), c.cosh()));
            }
        }

        #[test]
        fn test_tanh() {
            assert!(close(_0_0i.tanh(), _0_0i));
            assert!(close(
                _1_0i.tanh(),
                _1_0i.scale((f64::consts::E.powi(2) - 1.0) / (f64::consts::E.powi(2) + 1.0))
            ));
            assert!(close(_0_1i.tanh(), _0_1i.scale(1.0.tan())));
            for &c in all_consts.iter() {
                // tanh(conj(z)) = conj(tanh(z))
                assert!(close(c.conj().tanh(), c.conj().tanh()));
                // tanh(-z) = -tanh(z)
                assert!(close(c.scale(-1.0).tanh(), c.tanh().scale(-1.0)));
            }
        }

        #[test]
        fn test_asinh() {
            assert!(close(_0_0i.asinh(), _0_0i));
            assert!(close(_1_0i.asinh(), _1_0i.scale(1.0 + 2.0.sqrt()).ln()));
            assert!(close(_0_1i.asinh(), _0_1i.scale(f64::consts::PI / 2.0)));
            assert!(close(
                _0_1i.asinh().scale(-1.0),
                _0_1i.scale(-f64::consts::PI / 2.0)
            ));
            for &c in all_consts.iter() {
                // asinh(conj(z)) = conj(asinh(z))
                assert!(close(c.conj().asinh(), c.conj().asinh()));
                // asinh(-z) = -asinh(z)
                assert!(close(c.scale(-1.0).asinh(), c.asinh().scale(-1.0)));
                // for this branch, -pi/2 <= asinh(z).im <= pi/2
                assert!(
                    -f64::consts::PI / 2.0 <= c.asinh().im && c.asinh().im <= f64::consts::PI / 2.0
                );
            }
        }

        #[test]
        fn test_acosh() {
            assert!(close(_0_0i.acosh(), _0_1i.scale(f64::consts::PI / 2.0)));
            assert!(close(_1_0i.acosh(), _0_0i));
            assert!(close(
                _1_0i.scale(-1.0).acosh(),
                _0_1i.scale(f64::consts::PI)
            ));
            for &c in all_consts.iter() {
                // acosh(conj(z)) = conj(acosh(z))
                assert!(close(c.conj().acosh(), c.conj().acosh()));
                // for this branch, -pi <= acosh(z).im <= pi and 0 <= acosh(z).re
                assert!(
                    -f64::consts::PI <= c.acosh().im
                        && c.acosh().im <= f64::consts::PI
                        && 0.0 <= c.cosh().re
                );
            }
        }

        #[test]
        fn test_atanh() {
            assert!(close(_0_0i.atanh(), _0_0i));
            assert!(close(_0_1i.atanh(), _0_1i.scale(f64::consts::PI / 4.0)));
            assert!(close(_1_0i.atanh(), Complex::new(f64::infinity(), 0.0)));
            for &c in all_consts.iter() {
                // atanh(conj(z)) = conj(atanh(z))
                assert!(close(c.conj().atanh(), c.conj().atanh()));
                // atanh(-z) = -atanh(z)
                assert!(close(c.scale(-1.0).atanh(), c.atanh().scale(-1.0)));
                // for this branch, -pi/2 <= atanh(z).im <= pi/2
                assert!(
                    -f64::consts::PI / 2.0 <= c.atanh().im && c.atanh().im <= f64::consts::PI / 2.0
                );
            }
        }

        #[test]
        fn test_exp_ln() {
            for &c in all_consts.iter() {
                // e^ln(z) = z
                assert!(close(c.ln().exp(), c));
            }
        }

        #[test]
        fn test_exp2_log() {
            for &c in all_consts.iter() {
                // 2^log2(z) = z
                assert!(close(c.log2().exp2(), c));
            }
        }

        #[test]
        fn test_trig_to_hyperbolic() {
            for &c in all_consts.iter() {
                // sin(iz) = i sinh(z)
                assert!(close((_0_1i * c).sin(), _0_1i * c.sinh()));
                // cos(iz) = cosh(z)
                assert!(close((_0_1i * c).cos(), c.cosh()));
                // tan(iz) = i tanh(z)
                assert!(close((_0_1i * c).tan(), _0_1i * c.tanh()));
            }
        }

        #[test]
        fn test_trig_identities() {
            for &c in all_consts.iter() {
                // tan(z) = sin(z)/cos(z)
                assert!(close(c.tan(), c.sin() / c.cos()));
                // sin(z)^2 + cos(z)^2 = 1
                assert!(close(c.sin() * c.sin() + c.cos() * c.cos(), _1_0i));

                // sin(asin(z)) = z
                assert!(close(c.asin().sin(), c));
                // cos(acos(z)) = z
                assert!(close(c.acos().cos(), c));
                // tan(atan(z)) = z
                // i and -i are branch points
                if c != _0_1i && c != _0_1i.scale(-1.0) {
                    assert!(close(c.atan().tan(), c));
                }

                // sin(z) = (e^(iz) - e^(-iz))/(2i)
                assert!(close(
                    ((_0_1i * c).exp() - (_0_1i * c).exp().inv()) / _0_1i.scale(2.0),
                    c.sin()
                ));
                // cos(z) = (e^(iz) + e^(-iz))/2
                assert!(close(
                    ((_0_1i * c).exp() + (_0_1i * c).exp().inv()).unscale(2.0),
                    c.cos()
                ));
                // tan(z) = i (1 - e^(2iz))/(1 + e^(2iz))
                assert!(close(
                    _0_1i * (_1_0i - (_0_1i * c).scale(2.0).exp())
                        / (_1_0i + (_0_1i * c).scale(2.0).exp()),
                    c.tan()
                ));
            }
        }

        #[test]
        fn test_hyperbolic_identites() {
            for &c in all_consts.iter() {
                // tanh(z) = sinh(z)/cosh(z)
                assert!(close(c.tanh(), c.sinh() / c.cosh()));
                // cosh(z)^2 - sinh(z)^2 = 1
                assert!(close(c.cosh() * c.cosh() - c.sinh() * c.sinh(), _1_0i));

                // sinh(asinh(z)) = z
                assert!(close(c.asinh().sinh(), c));
                // cosh(acosh(z)) = z
                assert!(close(c.acosh().cosh(), c));
                // tanh(atanh(z)) = z
                // 1 and -1 are branch points
                if c != _1_0i && c != _1_0i.scale(-1.0) {
                    assert!(close(c.atanh().tanh(), c));
                }

                // sinh(z) = (e^z - e^(-z))/2
                assert!(close((c.exp() - c.exp().inv()).unscale(2.0), c.sinh()));
                // cosh(z) = (e^z + e^(-z))/2
                assert!(close((c.exp() + c.exp().inv()).unscale(2.0), c.cosh()));
                // tanh(z) = ( e^(2z) - 1)/(e^(2z) + 1)
                assert!(close(
                    (c.scale(2.0).exp() - _1_0i) / (c.scale(2.0).exp() + _1_0i),
                    c.tanh()
                ));
            }
        }
    }

    // Test both a + b and a += b
    macro_rules! test_a_op_b {
        ($a:ident + $b:expr, $answer:expr) => {
            assert_eq!($a + $b, $answer);
            assert_eq!(
                {
                    let mut x = $a;
                    x += $b;
                    x
                },
                $answer
            );
        };
        ($a:ident - $b:expr, $answer:expr) => {
            assert_eq!($a - $b, $answer);
            assert_eq!(
                {
                    let mut x = $a;
                    x -= $b;
                    x
                },
                $answer
            );
        };
        ($a:ident * $b:expr, $answer:expr) => {
            assert_eq!($a * $b, $answer);
            assert_eq!(
                {
                    let mut x = $a;
                    x *= $b;
                    x
                },
                $answer
            );
        };
        ($a:ident / $b:expr, $answer:expr) => {
            assert_eq!($a / $b, $answer);
            assert_eq!(
                {
                    let mut x = $a;
                    x /= $b;
                    x
                },
                $answer
            );
        };
        ($a:ident % $b:expr, $answer:expr) => {
            assert_eq!($a % $b, $answer);
            assert_eq!(
                {
                    let mut x = $a;
                    x %= $b;
                    x
                },
                $answer
            );
        };
    }

    // Test both a + b and a + &b
    macro_rules! test_op {
        ($a:ident $op:tt $b:expr, $answer:expr) => {
            test_a_op_b!($a $op $b, $answer);
            test_a_op_b!($a $op &$b, $answer);
        };
    }

    mod complex_arithmetic {
        use super::{_05_05i, _0_0i, _0_1i, _1_0i, _1_1i, _4_2i, _neg1_1i, all_consts};
        use num_traits::{MulAdd, MulAddAssign, Zero};

        #[test]
        fn test_add() {
            test_op!(_05_05i + _05_05i, _1_1i);
            test_op!(_0_1i + _1_0i, _1_1i);
            test_op!(_1_0i + _neg1_1i, _0_1i);

            for &c in all_consts.iter() {
                test_op!(_0_0i + c, c);
                test_op!(c + _0_0i, c);
            }
        }

        #[test]
        fn test_sub() {
            test_op!(_05_05i - _05_05i, _0_0i);
            test_op!(_0_1i - _1_0i, _neg1_1i);
            test_op!(_0_1i - _neg1_1i, _1_0i);

            for &c in all_consts.iter() {
                test_op!(c - _0_0i, c);
                test_op!(c - c, _0_0i);
            }
        }

        #[test]
        fn test_mul() {
            test_op!(_05_05i * _05_05i, _0_1i.unscale(2.0));
            test_op!(_1_1i * _0_1i, _neg1_1i);

            // i^2 & i^4
            test_op!(_0_1i * _0_1i, -_1_0i);
            assert_eq!(_0_1i * _0_1i * _0_1i * _0_1i, _1_0i);

            for &c in all_consts.iter() {
                test_op!(c * _1_0i, c);
                test_op!(_1_0i * c, c);
            }
        }

        #[test]
        #[cfg(any(feature = "std", feature = "libm"))]
        fn test_mul_add_float() {
            assert_eq!(_05_05i.mul_add(_05_05i, _0_0i), _05_05i * _05_05i + _0_0i);
            assert_eq!(_05_05i * _05_05i + _0_0i, _05_05i.mul_add(_05_05i, _0_0i));
            assert_eq!(_0_1i.mul_add(_0_1i, _0_1i), _neg1_1i);
            assert_eq!(_1_0i.mul_add(_1_0i, _1_0i), _1_0i * _1_0i + _1_0i);
            assert_eq!(_1_0i * _1_0i + _1_0i, _1_0i.mul_add(_1_0i, _1_0i));

            let mut x = _1_0i;
            x.mul_add_assign(_1_0i, _1_0i);
            assert_eq!(x, _1_0i * _1_0i + _1_0i);

            for &a in &all_consts {
                for &b in &all_consts {
                    for &c in &all_consts {
                        let abc = a * b + c;
                        assert_eq!(a.mul_add(b, c), abc);
                        let mut x = a;
                        x.mul_add_assign(b, c);
                        assert_eq!(x, abc);
                    }
                }
            }
        }

        #[test]
        fn test_mul_add() {
            use super::Complex;
            const _0_0i: Complex<i32> = Complex { re: 0, im: 0 };
            const _1_0i: Complex<i32> = Complex { re: 1, im: 0 };
            const _1_1i: Complex<i32> = Complex { re: 1, im: 1 };
            const _0_1i: Complex<i32> = Complex { re: 0, im: 1 };
            const _neg1_1i: Complex<i32> = Complex { re: -1, im: 1 };
            const all_consts: [Complex<i32>; 5] = [_0_0i, _1_0i, _1_1i, _0_1i, _neg1_1i];

            assert_eq!(_1_0i.mul_add(_1_0i, _0_0i), _1_0i * _1_0i + _0_0i);
            assert_eq!(_1_0i * _1_0i + _0_0i, _1_0i.mul_add(_1_0i, _0_0i));
            assert_eq!(_0_1i.mul_add(_0_1i, _0_1i), _neg1_1i);
            assert_eq!(_1_0i.mul_add(_1_0i, _1_0i), _1_0i * _1_0i + _1_0i);
            assert_eq!(_1_0i * _1_0i + _1_0i, _1_0i.mul_add(_1_0i, _1_0i));

            let mut x = _1_0i;
            x.mul_add_assign(_1_0i, _1_0i);
            assert_eq!(x, _1_0i * _1_0i + _1_0i);

            for &a in &all_consts {
                for &b in &all_consts {
                    for &c in &all_consts {
                        let abc = a * b + c;
                        assert_eq!(a.mul_add(b, c), abc);
                        let mut x = a;
                        x.mul_add_assign(b, c);
                        assert_eq!(x, abc);
                    }
                }
            }
        }

        #[test]
        fn test_div() {
            test_op!(_neg1_1i / _0_1i, _1_1i);
            for &c in all_consts.iter() {
                if c != Zero::zero() {
                    test_op!(c / c, _1_0i);
                }
            }
        }

        #[test]
        fn test_rem() {
            test_op!(_neg1_1i % _0_1i, _0_0i);
            test_op!(_4_2i % _0_1i, _0_0i);
            test_op!(_05_05i % _0_1i, _05_05i);
            test_op!(_05_05i % _1_1i, _05_05i);
            assert_eq!((_4_2i + _05_05i) % _0_1i, _05_05i);
            assert_eq!((_4_2i + _05_05i) % _1_1i, _05_05i);
        }

        #[test]
        fn test_neg() {
            assert_eq!(-_1_0i + _0_1i, _neg1_1i);
            assert_eq!((-_0_1i) * _0_1i, _1_0i);
            for &c in all_consts.iter() {
                assert_eq!(-(-c), c);
            }
        }
    }

    mod real_arithmetic {
        use super::super::Complex;
        use super::{_4_2i, _neg1_1i};

        #[test]
        fn test_add() {
            test_op!(_4_2i + 0.5, Complex::new(4.5, 2.0));
            assert_eq!(0.5 + _4_2i, Complex::new(4.5, 2.0));
        }

        #[test]
        fn test_sub() {
            test_op!(_4_2i - 0.5, Complex::new(3.5, 2.0));
            assert_eq!(0.5 - _4_2i, Complex::new(-3.5, -2.0));
        }

        #[test]
        fn test_mul() {
            assert_eq!(_4_2i * 0.5, Complex::new(2.0, 1.0));
            assert_eq!(0.5 * _4_2i, Complex::new(2.0, 1.0));
        }

        #[test]
        fn test_div() {
            assert_eq!(_4_2i / 0.5, Complex::new(8.0, 4.0));
            assert_eq!(0.5 / _4_2i, Complex::new(0.1, -0.05));
        }

        #[test]
        fn test_rem() {
            assert_eq!(_4_2i % 2.0, Complex::new(0.0, 0.0));
            assert_eq!(_4_2i % 3.0, Complex::new(1.0, 2.0));
            assert_eq!(3.0 % _4_2i, Complex::new(3.0, 0.0));
            assert_eq!(_neg1_1i % 2.0, _neg1_1i);
            assert_eq!(-_4_2i % 3.0, Complex::new(-1.0, -2.0));
        }

        #[test]
        fn test_div_rem_gaussian() {
            // These would overflow with `norm_sqr` division.
            let max = Complex::new(255u8, 255u8);
            assert_eq!(max / 200, Complex::new(1, 1));
            assert_eq!(max % 200, Complex::new(55, 55));
        }
    }

    #[test]
    fn test_to_string() {
        fn test(c: Complex64, s: String) {
            assert_eq!(c.to_string(), s);
        }
        test(_0_0i, "0+0i".to_string());
        test(_1_0i, "1+0i".to_string());
        test(_0_1i, "0+1i".to_string());
        test(_1_1i, "1+1i".to_string());
        test(_neg1_1i, "-1+1i".to_string());
        test(-_neg1_1i, "1-1i".to_string());
        test(_05_05i, "0.5+0.5i".to_string());
    }

    #[test]
    fn test_string_formatting() {
        let a = Complex::new(1.23456, 123.456);
        assert_eq!(format!("{}", a), "1.23456+123.456i");
        assert_eq!(format!("{:.2}", a), "1.23+123.46i");
        assert_eq!(format!("{:.2e}", a), "1.23e0+1.23e2i");
        assert_eq!(format!("{:+.2E}", a), "+1.23E0+1.23E2i");
        #[cfg(feature = "std")]
        assert_eq!(format!("{:+20.2E}", a), "     +1.23E0+1.23E2i");

        let b = Complex::new(0x80, 0xff);
        assert_eq!(format!("{:X}", b), "80+FFi");
        assert_eq!(format!("{:#x}", b), "0x80+0xffi");
        assert_eq!(format!("{:+#b}", b), "+0b10000000+0b11111111i");
        assert_eq!(format!("{:+#o}", b), "+0o200+0o377i");
        #[cfg(feature = "std")]
        assert_eq!(format!("{:+#16o}", b), "   +0o200+0o377i");

        let c = Complex::new(-10, -10000);
        assert_eq!(format!("{}", c), "-10-10000i");
        #[cfg(feature = "std")]
        assert_eq!(format!("{:16}", c), "      -10-10000i");
    }

    #[test]
    fn test_hash() {
        let a = Complex::new(0i32, 0i32);
        let b = Complex::new(1i32, 0i32);
        let c = Complex::new(0i32, 1i32);
        assert!(crate::hash(&a) != crate::hash(&b));
        assert!(crate::hash(&b) != crate::hash(&c));
        assert!(crate::hash(&c) != crate::hash(&a));
    }

    #[test]
    fn test_hashset() {
        use std::collections::HashSet;
        let a = Complex::new(0i32, 0i32);
        let b = Complex::new(1i32, 0i32);
        let c = Complex::new(0i32, 1i32);

        let set: HashSet<_> = [a, b, c].iter().cloned().collect();
        assert!(set.contains(&a));
        assert!(set.contains(&b));
        assert!(set.contains(&c));
        assert!(!set.contains(&(a + b + c)));
    }

    #[test]
    fn test_is_nan() {
        assert!(!_1_1i.is_nan());
        let a = Complex::new(f64::NAN, f64::NAN);
        assert!(a.is_nan());
    }

    #[test]
    fn test_is_nan_special_cases() {
        let a = Complex::new(0f64, f64::NAN);
        let b = Complex::new(f64::NAN, 0f64);
        assert!(a.is_nan());
        assert!(b.is_nan());
    }

    #[test]
    fn test_is_infinite() {
        let a = Complex::new(2f64, f64::INFINITY);
        assert!(a.is_infinite());
    }

    #[test]
    fn test_is_finite() {
        assert!(_1_1i.is_finite())
    }

    #[test]
    fn test_is_normal() {
        let a = Complex::new(0f64, f64::NAN);
        let b = Complex::new(2f64, f64::INFINITY);
        assert!(!a.is_normal());
        assert!(!b.is_normal());
        assert!(_1_1i.is_normal());
    }

    #[test]
    fn test_from_str() {
        fn test(z: Complex64, s: &str) {
            assert_eq!(FromStr::from_str(s), Ok(z));
        }
        test(_0_0i, "0 + 0i");
        test(_0_0i, "0+0j");
        test(_0_0i, "0 - 0j");
        test(_0_0i, "0-0i");
        test(_0_0i, "0i + 0");
        test(_0_0i, "0");
        test(_0_0i, "-0");
        test(_0_0i, "0i");
        test(_0_0i, "0j");
        test(_0_0i, "+0j");
        test(_0_0i, "-0i");

        test(_1_0i, "1 + 0i");
        test(_1_0i, "1+0j");
        test(_1_0i, "1 - 0j");
        test(_1_0i, "+1-0i");
        test(_1_0i, "-0j+1");
        test(_1_0i, "1");

        test(_1_1i, "1 + i");
        test(_1_1i, "1+j");
        test(_1_1i, "1 + 1j");
        test(_1_1i, "1+1i");
        test(_1_1i, "i + 1");
        test(_1_1i, "1i+1");
        test(_1_1i, "+j+1");

        test(_0_1i, "0 + i");
        test(_0_1i, "0+j");
        test(_0_1i, "-0 + j");
        test(_0_1i, "-0+i");
        test(_0_1i, "0 + 1i");
        test(_0_1i, "0+1j");
        test(_0_1i, "-0 + 1j");
        test(_0_1i, "-0+1i");
        test(_0_1i, "j + 0");
        test(_0_1i, "i");
        test(_0_1i, "j");
        test(_0_1i, "1j");

        test(_neg1_1i, "-1 + i");
        test(_neg1_1i, "-1+j");
        test(_neg1_1i, "-1 + 1j");
        test(_neg1_1i, "-1+1i");
        test(_neg1_1i, "1i-1");
        test(_neg1_1i, "j + -1");

        test(_05_05i, "0.5 + 0.5i");
        test(_05_05i, "0.5+0.5j");
        test(_05_05i, "5e-1+0.5j");
        test(_05_05i, "5E-1 + 0.5j");
        test(_05_05i, "5E-1i + 0.5");
        test(_05_05i, "0.05e+1j + 50E-2");
    }

    #[test]
    fn test_from_str_radix() {
        fn test(z: Complex64, s: &str, radix: u32) {
            let res: Result<Complex64, <Complex64 as Num>::FromStrRadixErr> =
                Num::from_str_radix(s, radix);
            assert_eq!(res.unwrap(), z)
        }
        test(_4_2i, "4+2i", 10);
        test(Complex::new(15.0, 32.0), "F+20i", 16);
        test(Complex::new(15.0, 32.0), "1111+100000i", 2);
        test(Complex::new(-15.0, -32.0), "-F-20i", 16);
        test(Complex::new(-15.0, -32.0), "-1111-100000i", 2);

        fn test_error(s: &str, radix: u32) -> ParseComplexError<<f64 as Num>::FromStrRadixErr> {
            let res = Complex64::from_str_radix(s, radix);

            res.expect_err(&format!("Expected failure on input {:?}", s))
        }

        let err = test_error("1ii", 19);
        if let ComplexErrorKind::UnsupportedRadix = err.kind {
            /* pass */
        } else {
            panic!("Expected failure on invalid radix, got {:?}", err);
        }

        let err = test_error("1 + 0", 16);
        if let ComplexErrorKind::ExprError = err.kind {
            /* pass */
        } else {
            panic!("Expected failure on expr error, got {:?}", err);
        }
    }

    #[test]
    #[should_panic(expected = "radix is too high")]
    fn test_from_str_radix_fail() {
        // ensure we preserve the underlying panic on radix > 36
        let _complex = Complex64::from_str_radix("1", 37);
    }

    #[test]
    fn test_from_str_fail() {
        fn test(s: &str) {
            let complex: Result<Complex64, _> = FromStr::from_str(s);
            assert!(
                complex.is_err(),
                "complex {:?} -> {:?} should be an error",
                s,
                complex
            );
        }
        test("foo");
        test("6E");
        test("0 + 2.718");
        test("1 - -2i");
        test("314e-2ij");
        test("4.3j - i");
        test("1i - 2i");
        test("+ 1 - 3.0i");
    }

    #[test]
    fn test_sum() {
        let v = vec![_0_1i, _1_0i];
        assert_eq!(v.iter().sum::<Complex64>(), _1_1i);
        assert_eq!(v.into_iter().sum::<Complex64>(), _1_1i);
    }

    #[test]
    fn test_prod() {
        let v = vec![_0_1i, _1_0i];
        assert_eq!(v.iter().product::<Complex64>(), _0_1i);
        assert_eq!(v.into_iter().product::<Complex64>(), _0_1i);
    }

    #[test]
    fn test_zero() {
        let zero = Complex64::zero();
        assert!(zero.is_zero());

        let mut c = Complex::new(1.23, 4.56);
        assert!(!c.is_zero());
        assert_eq!(c + zero, c);

        c.set_zero();
        assert!(c.is_zero());
    }

    #[test]
    fn test_one() {
        let one = Complex64::one();
        assert!(one.is_one());

        let mut c = Complex::new(1.23, 4.56);
        assert!(!c.is_one());
        assert_eq!(c * one, c);

        c.set_one();
        assert!(c.is_one());
    }

    #[test]
    #[allow(clippy::float_cmp)]
    fn test_const() {
        const R: f64 = 12.3;
        const I: f64 = -4.5;
        const C: Complex64 = Complex::new(R, I);

        assert_eq!(C.re, 12.3);
        assert_eq!(C.im, -4.5);
    }
}
