// Keeps us from accidentally creating a recursive impl rather than a real one.
#![deny(unconditional_recursion)]

use core::ops::Neg;

use num_traits::{Float, FloatConst, Num, NumCast};

use crate::Complex;

mod private {
    use num_traits::{Float, FloatConst};

    use crate::Complex;

    pub trait Seal {}

    impl<T> Seal for T where T: Float + FloatConst {}
    impl<T: Float + FloatConst> Seal for Complex<T> {}
}

/// Generic trait for floating point complex numbers.
///
/// This trait defines methods which are common to complex floating point
/// numbers and regular floating point numbers.
///
/// This trait is sealed to prevent it from being implemented by anything other
/// than floating point scalars and [Complex] floats.
pub trait ComplexFloat: Num + NumCast + Copy + Neg<Output = Self> + private::Seal {
    /// The type used to represent the real coefficients of this complex number.
    type Real: Float + FloatConst;

    /// Returns `true` if this value is `NaN` and false otherwise.
    fn is_nan(self) -> bool;

    /// Returns `true` if this value is positive infinity or negative infinity and
    /// false otherwise.
    fn is_infinite(self) -> bool;

    /// Returns `true` if this number is neither infinite nor `NaN`.
    fn is_finite(self) -> bool;

    /// Returns `true` if the number is neither zero, infinite,
    /// [subnormal](http://en.wikipedia.org/wiki/Denormal_number), or `NaN`.
    fn is_normal(self) -> bool;

    /// Take the reciprocal (inverse) of a number, `1/x`. See also [Complex::finv].
    fn recip(self) -> Self;

    /// Raises `self` to a signed integer power.
    fn powi(self, exp: i32) -> Self;

    /// Raises `self` to a real power.
    fn powf(self, exp: Self::Real) -> Self;

    /// Raises `self` to a complex power.
    fn powc(self, exp: Complex<Self::Real>) -> Complex<Self::Real>;

    /// Take the square root of a number.
    fn sqrt(self) -> Self;

    /// Returns `e^(self)`, (the exponential function).
    fn exp(self) -> Self;

    /// Returns `2^(self)`.
    fn exp2(self) -> Self;

    /// Returns `base^(self)`.
    fn expf(self, base: Self::Real) -> Self;

    /// Returns the natural logarithm of the number.
    fn ln(self) -> Self;

    /// Returns the logarithm of the number with respect to an arbitrary base.
    fn log(self, base: Self::Real) -> Self;

    /// Returns the base 2 logarithm of the number.
    fn log2(self) -> Self;

    /// Returns the base 10 logarithm of the number.
    fn log10(self) -> Self;

    /// Take the cubic root of a number.
    fn cbrt(self) -> Self;

    /// Computes the sine of a number (in radians).
    fn sin(self) -> Self;

    /// Computes the cosine of a number (in radians).
    fn cos(self) -> Self;

    /// Computes the tangent of a number (in radians).
    fn tan(self) -> Self;

    /// Computes the arcsine of a number. Return value is in radians in
    /// the range [-pi/2, pi/2] or NaN if the number is outside the range
    /// [-1, 1].
    fn asin(self) -> Self;

    /// Computes the arccosine of a number. Return value is in radians in
    /// the range [0, pi] or NaN if the number is outside the range
    /// [-1, 1].
    fn acos(self) -> Self;

    /// Computes the arctangent of a number. Return value is in radians in the
    /// range [-pi/2, pi/2];
    fn atan(self) -> Self;

    /// Hyperbolic sine function.
    fn sinh(self) -> Self;

    /// Hyperbolic cosine function.
    fn cosh(self) -> Self;

    /// Hyperbolic tangent function.
    fn tanh(self) -> Self;

    /// Inverse hyperbolic sine function.
    fn asinh(self) -> Self;

    /// Inverse hyperbolic cosine function.
    fn acosh(self) -> Self;

    /// Inverse hyperbolic tangent function.
    fn atanh(self) -> Self;

    /// Returns the real part of the number.
    fn re(self) -> Self::Real;

    /// Returns the imaginary part of the number.
    fn im(self) -> Self::Real;

    /// Returns the absolute value of the number. See also [Complex::norm]
    fn abs(self) -> Self::Real;

    /// Returns the L1 norm `|re| + |im|` -- the [Manhattan distance] from the origin.
    ///
    /// [Manhattan distance]: https://en.wikipedia.org/wiki/Taxicab_geometry
    fn l1_norm(&self) -> Self::Real;

    /// Computes the argument of the number.
    fn arg(self) -> Self::Real;

    /// Computes the complex conjugate of the number.
    ///
    /// Formula: `a+bi -> a-bi`
    fn conj(self) -> Self;
}

macro_rules! forward {
    ($( $base:ident :: $method:ident ( self $( , $arg:ident : $ty:ty )* ) -> $ret:ty ; )*)
        => {$(
            #[inline]
            fn $method(self $( , $arg : $ty )* ) -> $ret {
                $base::$method(self $( , $arg )* )
            }
        )*};
}

macro_rules! forward_ref {
    ($( Self :: $method:ident ( & self $( , $arg:ident : $ty:ty )* ) -> $ret:ty ; )*)
        => {$(
            #[inline]
            fn $method(self $( , $arg : $ty )* ) -> $ret {
                Self::$method(&self $( , $arg )* )
            }
        )*};
}

impl<T> ComplexFloat for T
where
    T: Float + FloatConst,
{
    type Real = T;

    fn re(self) -> Self::Real {
        self
    }

    fn im(self) -> Self::Real {
        T::zero()
    }

    fn l1_norm(&self) -> Self::Real {
        self.abs()
    }

    fn arg(self) -> Self::Real {
        if self.is_nan() {
            self
        } else if self.is_sign_negative() {
            T::PI()
        } else {
            T::zero()
        }
    }

    fn powc(self, exp: Complex<Self::Real>) -> Complex<Self::Real> {
        Complex::new(self, T::zero()).powc(exp)
    }

    fn conj(self) -> Self {
        self
    }

    fn expf(self, base: Self::Real) -> Self {
        base.powf(self)
    }

    forward! {
        Float::is_normal(self) -> bool;
        Float::is_infinite(self) -> bool;
        Float::is_finite(self) -> bool;
        Float::is_nan(self) -> bool;
        Float::recip(self) -> Self;
        Float::powi(self, n: i32) -> Self;
        Float::powf(self, f: Self) -> Self;
        Float::sqrt(self) -> Self;
        Float::cbrt(self) -> Self;
        Float::exp(self) -> Self;
        Float::exp2(self) -> Self;
        Float::ln(self) -> Self;
        Float::log(self, base: Self) -> Self;
        Float::log2(self) -> Self;
        Float::log10(self) -> Self;
        Float::sin(self) -> Self;
        Float::cos(self) -> Self;
        Float::tan(self) -> Self;
        Float::asin(self) -> Self;
        Float::acos(self) -> Self;
        Float::atan(self) -> Self;
        Float::sinh(self) -> Self;
        Float::cosh(self) -> Self;
        Float::tanh(self) -> Self;
        Float::asinh(self) -> Self;
        Float::acosh(self) -> Self;
        Float::atanh(self) -> Self;
        Float::abs(self) -> Self;
    }
}

impl<T: Float + FloatConst> ComplexFloat for Complex<T> {
    type Real = T;

    fn re(self) -> Self::Real {
        self.re
    }

    fn im(self) -> Self::Real {
        self.im
    }

    fn abs(self) -> Self::Real {
        self.norm()
    }

    fn recip(self) -> Self {
        self.finv()
    }

    // `Complex::l1_norm` uses `Signed::abs` to let it work
    // for integers too, but we can just use `Float::abs`.
    fn l1_norm(&self) -> Self::Real {
        self.re.abs() + self.im.abs()
    }

    // `Complex::is_*` methods use `T: FloatCore`, but we
    // have `T: Float` that can do them as well.
    fn is_nan(self) -> bool {
        self.re.is_nan() || self.im.is_nan()
    }

    fn is_infinite(self) -> bool {
        !self.is_nan() && (self.re.is_infinite() || self.im.is_infinite())
    }

    fn is_finite(self) -> bool {
        self.re.is_finite() && self.im.is_finite()
    }

    fn is_normal(self) -> bool {
        self.re.is_normal() && self.im.is_normal()
    }

    forward! {
        Complex::arg(self) -> Self::Real;
        Complex::powc(self, exp: Complex<Self::Real>) -> Complex<Self::Real>;
        Complex::exp2(self) -> Self;
        Complex::log(self, base: Self::Real) -> Self;
        Complex::log2(self) -> Self;
        Complex::log10(self) -> Self;
        Complex::powf(self, f: Self::Real) -> Self;
        Complex::sqrt(self) -> Self;
        Complex::cbrt(self) -> Self;
        Complex::exp(self) -> Self;
        Complex::expf(self, base: Self::Real) -> Self;
        Complex::ln(self) -> Self;
        Complex::sin(self) -> Self;
        Complex::cos(self) -> Self;
        Complex::tan(self) -> Self;
        Complex::asin(self) -> Self;
        Complex::acos(self) -> Self;
        Complex::atan(self) -> Self;
        Complex::sinh(self) -> Self;
        Complex::cosh(self) -> Self;
        Complex::tanh(self) -> Self;
        Complex::asinh(self) -> Self;
        Complex::acosh(self) -> Self;
        Complex::atanh(self) -> Self;
    }

    forward_ref! {
        Self::powi(&self, n: i32) -> Self;
        Self::conj(&self) -> Self;
    }
}

#[cfg(test)]
mod test {
    use crate::{
        complex_float::ComplexFloat,
        test::{_0_0i, _0_1i, _1_0i, _1_1i, float::close},
        Complex,
    };
    use std::f64; // for constants before Rust 1.43.

    fn closef(a: f64, b: f64) -> bool {
        close_to_tolf(a, b, 1e-10)
    }

    fn close_to_tolf(a: f64, b: f64, tol: f64) -> bool {
        // returns true if a and b are reasonably close
        let close = (a == b) || (a - b).abs() < tol;
        if !close {
            println!("{:?} != {:?}", a, b);
        }
        close
    }

    #[test]
    fn test_exp2() {
        assert!(close(ComplexFloat::exp2(_0_0i), _1_0i));
        assert!(closef(<f64 as ComplexFloat>::exp2(0.), 1.));
    }

    #[test]
    fn test_exp() {
        assert!(close(ComplexFloat::exp(_0_0i), _1_0i));
        assert!(closef(ComplexFloat::exp(0.), 1.));
    }

    #[test]
    fn test_powi() {
        assert!(close(ComplexFloat::powi(_0_1i, 4), _1_0i));
        assert!(closef(ComplexFloat::powi(-1., 4), 1.));
    }

    #[test]
    fn test_powz() {
        assert!(close(ComplexFloat::powc(_1_0i, _0_1i), _1_0i));
        assert!(close(ComplexFloat::powc(1., _0_1i), _1_0i));
    }

    #[test]
    fn test_log2() {
        assert!(close(ComplexFloat::log2(_1_0i), _0_0i));
        assert!(closef(ComplexFloat::log2(1.), 0.));
    }

    #[test]
    fn test_log10() {
        assert!(close(ComplexFloat::log10(_1_0i), _0_0i));
        assert!(closef(ComplexFloat::log10(1.), 0.));
    }

    #[test]
    fn test_conj() {
        assert_eq!(ComplexFloat::conj(_0_1i), Complex::new(0., -1.));
        assert_eq!(ComplexFloat::conj(1.), 1.);
    }

    #[test]
    fn test_is_nan() {
        assert!(!ComplexFloat::is_nan(_1_0i));
        assert!(!ComplexFloat::is_nan(1.));

        assert!(ComplexFloat::is_nan(Complex::new(f64::NAN, f64::NAN)));
        assert!(ComplexFloat::is_nan(f64::NAN));
    }

    #[test]
    fn test_is_infinite() {
        assert!(!ComplexFloat::is_infinite(_1_0i));
        assert!(!ComplexFloat::is_infinite(1.));

        assert!(ComplexFloat::is_infinite(Complex::new(
            f64::INFINITY,
            f64::INFINITY
        )));
        assert!(ComplexFloat::is_infinite(f64::INFINITY));
    }

    #[test]
    fn test_is_finite() {
        assert!(ComplexFloat::is_finite(_1_0i));
        assert!(ComplexFloat::is_finite(1.));

        assert!(!ComplexFloat::is_finite(Complex::new(
            f64::INFINITY,
            f64::INFINITY
        )));
        assert!(!ComplexFloat::is_finite(f64::INFINITY));
    }

    #[test]
    fn test_is_normal() {
        assert!(ComplexFloat::is_normal(_1_1i));
        assert!(ComplexFloat::is_normal(1.));

        assert!(!ComplexFloat::is_normal(Complex::new(
            f64::INFINITY,
            f64::INFINITY
        )));
        assert!(!ComplexFloat::is_normal(f64::INFINITY));
    }

    #[test]
    fn test_arg() {
        assert!(closef(
            ComplexFloat::arg(_0_1i),
            core::f64::consts::FRAC_PI_2
        ));

        assert!(closef(ComplexFloat::arg(-1.), core::f64::consts::PI));
        assert!(closef(ComplexFloat::arg(-0.), core::f64::consts::PI));
        assert!(closef(ComplexFloat::arg(0.), 0.));
        assert!(closef(ComplexFloat::arg(1.), 0.));
        assert!(ComplexFloat::arg(f64::NAN).is_nan());
    }
}
