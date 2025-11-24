use crate::{CheckedMul, One};
use core::num::Wrapping;
use core::ops::Mul;

/// Binary operator for raising a value to a power.
pub trait Pow<RHS> {
    /// The result after applying the operator.
    type Output;

    /// Returns `self` to the power `rhs`.
    ///
    /// # Examples
    ///
    /// ```
    /// use num_traits::Pow;
    /// assert_eq!(Pow::pow(10u32, 2u32), 100);
    /// ```
    fn pow(self, rhs: RHS) -> Self::Output;
}

macro_rules! pow_impl {
    ($t:ty) => {
        pow_impl!($t, u8);
        pow_impl!($t, usize);

        // FIXME: these should be possible
        // pow_impl!($t, u16);
        // pow_impl!($t, u32);
        // pow_impl!($t, u64);
    };
    ($t:ty, $rhs:ty) => {
        pow_impl!($t, $rhs, usize, pow);
    };
    ($t:ty, $rhs:ty, $desired_rhs:ty, $method:expr) => {
        impl Pow<$rhs> for $t {
            type Output = $t;
            #[inline]
            fn pow(self, rhs: $rhs) -> $t {
                ($method)(self, <$desired_rhs>::from(rhs))
            }
        }

        impl<'a> Pow<&'a $rhs> for $t {
            type Output = $t;
            #[inline]
            fn pow(self, rhs: &'a $rhs) -> $t {
                ($method)(self, <$desired_rhs>::from(*rhs))
            }
        }

        impl<'a> Pow<$rhs> for &'a $t {
            type Output = $t;
            #[inline]
            fn pow(self, rhs: $rhs) -> $t {
                ($method)(*self, <$desired_rhs>::from(rhs))
            }
        }

        impl<'a, 'b> Pow<&'a $rhs> for &'b $t {
            type Output = $t;
            #[inline]
            fn pow(self, rhs: &'a $rhs) -> $t {
                ($method)(*self, <$desired_rhs>::from(*rhs))
            }
        }
    };
}

pow_impl!(u8, u8, u32, u8::pow);
pow_impl!(u8, u16, u32, u8::pow);
pow_impl!(u8, u32, u32, u8::pow);
pow_impl!(u8, usize);
pow_impl!(i8, u8, u32, i8::pow);
pow_impl!(i8, u16, u32, i8::pow);
pow_impl!(i8, u32, u32, i8::pow);
pow_impl!(i8, usize);
pow_impl!(u16, u8, u32, u16::pow);
pow_impl!(u16, u16, u32, u16::pow);
pow_impl!(u16, u32, u32, u16::pow);
pow_impl!(u16, usize);
pow_impl!(i16, u8, u32, i16::pow);
pow_impl!(i16, u16, u32, i16::pow);
pow_impl!(i16, u32, u32, i16::pow);
pow_impl!(i16, usize);
pow_impl!(u32, u8, u32, u32::pow);
pow_impl!(u32, u16, u32, u32::pow);
pow_impl!(u32, u32, u32, u32::pow);
pow_impl!(u32, usize);
pow_impl!(i32, u8, u32, i32::pow);
pow_impl!(i32, u16, u32, i32::pow);
pow_impl!(i32, u32, u32, i32::pow);
pow_impl!(i32, usize);
pow_impl!(u64, u8, u32, u64::pow);
pow_impl!(u64, u16, u32, u64::pow);
pow_impl!(u64, u32, u32, u64::pow);
pow_impl!(u64, usize);
pow_impl!(i64, u8, u32, i64::pow);
pow_impl!(i64, u16, u32, i64::pow);
pow_impl!(i64, u32, u32, i64::pow);
pow_impl!(i64, usize);

pow_impl!(u128, u8, u32, u128::pow);
pow_impl!(u128, u16, u32, u128::pow);
pow_impl!(u128, u32, u32, u128::pow);
pow_impl!(u128, usize);

pow_impl!(i128, u8, u32, i128::pow);
pow_impl!(i128, u16, u32, i128::pow);
pow_impl!(i128, u32, u32, i128::pow);
pow_impl!(i128, usize);

pow_impl!(usize, u8, u32, usize::pow);
pow_impl!(usize, u16, u32, usize::pow);
pow_impl!(usize, u32, u32, usize::pow);
pow_impl!(usize, usize);
pow_impl!(isize, u8, u32, isize::pow);
pow_impl!(isize, u16, u32, isize::pow);
pow_impl!(isize, u32, u32, isize::pow);
pow_impl!(isize, usize);
pow_impl!(Wrapping<u8>);
pow_impl!(Wrapping<i8>);
pow_impl!(Wrapping<u16>);
pow_impl!(Wrapping<i16>);
pow_impl!(Wrapping<u32>);
pow_impl!(Wrapping<i32>);
pow_impl!(Wrapping<u64>);
pow_impl!(Wrapping<i64>);
pow_impl!(Wrapping<u128>);
pow_impl!(Wrapping<i128>);
pow_impl!(Wrapping<usize>);
pow_impl!(Wrapping<isize>);

// FIXME: these should be possible
// pow_impl!(u8, u64);
// pow_impl!(i16, u64);
// pow_impl!(i8, u64);
// pow_impl!(u16, u64);
// pow_impl!(u32, u64);
// pow_impl!(i32, u64);
// pow_impl!(u64, u64);
// pow_impl!(i64, u64);
// pow_impl!(usize, u64);
// pow_impl!(isize, u64);

#[cfg(any(feature = "std", feature = "libm"))]
mod float_impls {
    use super::Pow;
    use crate::Float;

    pow_impl!(f32, i8, i32, <f32 as Float>::powi);
    pow_impl!(f32, u8, i32, <f32 as Float>::powi);
    pow_impl!(f32, i16, i32, <f32 as Float>::powi);
    pow_impl!(f32, u16, i32, <f32 as Float>::powi);
    pow_impl!(f32, i32, i32, <f32 as Float>::powi);
    pow_impl!(f64, i8, i32, <f64 as Float>::powi);
    pow_impl!(f64, u8, i32, <f64 as Float>::powi);
    pow_impl!(f64, i16, i32, <f64 as Float>::powi);
    pow_impl!(f64, u16, i32, <f64 as Float>::powi);
    pow_impl!(f64, i32, i32, <f64 as Float>::powi);
    pow_impl!(f32, f32, f32, <f32 as Float>::powf);
    pow_impl!(f64, f32, f64, <f64 as Float>::powf);
    pow_impl!(f64, f64, f64, <f64 as Float>::powf);
}

/// Raises a value to the power of exp, using exponentiation by squaring.
///
/// Note that `0⁰` (`pow(0, 0)`) returns `1`. Mathematically this is undefined.
///
/// # Example
///
/// ```rust
/// use num_traits::pow;
///
/// assert_eq!(pow(2i8, 4), 16);
/// assert_eq!(pow(6u8, 3), 216);
/// assert_eq!(pow(0u8, 0), 1); // Be aware if this case affects you
/// ```
#[inline]
pub fn pow<T: Clone + One + Mul<T, Output = T>>(mut base: T, mut exp: usize) -> T {
    if exp == 0 {
        return T::one();
    }

    while exp & 1 == 0 {
        base = base.clone() * base;
        exp >>= 1;
    }
    if exp == 1 {
        return base;
    }

    let mut acc = base.clone();
    while exp > 1 {
        exp >>= 1;
        base = base.clone() * base;
        if exp & 1 == 1 {
            acc = acc * base.clone();
        }
    }
    acc
}

/// Raises a value to the power of exp, returning `None` if an overflow occurred.
///
/// Note that `0⁰` (`checked_pow(0, 0)`) returns `Some(1)`. Mathematically this is undefined.
///
/// Otherwise same as the `pow` function.
///
/// # Example
///
/// ```rust
/// use num_traits::checked_pow;
///
/// assert_eq!(checked_pow(2i8, 4), Some(16));
/// assert_eq!(checked_pow(7i8, 8), None);
/// assert_eq!(checked_pow(7u32, 8), Some(5_764_801));
/// assert_eq!(checked_pow(0u32, 0), Some(1)); // Be aware if this case affect you
/// ```
#[inline]
pub fn checked_pow<T: Clone + One + CheckedMul>(mut base: T, mut exp: usize) -> Option<T> {
    if exp == 0 {
        return Some(T::one());
    }

    while exp & 1 == 0 {
        base = base.checked_mul(&base)?;
        exp >>= 1;
    }
    if exp == 1 {
        return Some(base);
    }

    let mut acc = base.clone();
    while exp > 1 {
        exp >>= 1;
        base = base.checked_mul(&base)?;
        if exp & 1 == 1 {
            acc = acc.checked_mul(&base)?;
        }
    }
    Some(acc)
}
