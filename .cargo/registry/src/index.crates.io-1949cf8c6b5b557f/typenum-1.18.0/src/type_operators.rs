//! Useful **type operators** that are not defined in `core::ops`.

use crate::{
    private::{Internal, InternalMarker},
    Bit, NInt, NonZero, PInt, UInt, UTerm, Unsigned, Z0,
};

/// A **type operator** that ensures that `Rhs` is the same as `Self`, it is mainly useful
/// for writing macros that can take arbitrary binary or unary operators.
///
/// `Same` is implemented generically for all types; it should never need to be implemented
/// for anything else.
///
/// Note that Rust lazily evaluates types, so this will only fail for two different types if
/// the `Output` is used.
///
/// # Example
/// ```rust
/// use typenum::{Same, Unsigned, U4, U5};
///
/// assert_eq!(<U5 as Same<U5>>::Output::to_u32(), 5);
///
/// // Only an error if we use it:
/// # #[allow(dead_code)]
/// type Undefined = <U5 as Same<U4>>::Output;
/// // Compiler error:
/// // Undefined::to_u32();
/// ```
pub trait Same<Rhs = Self> {
    /// Should always be `Self`
    type Output;
}

impl<T> Same<T> for T {
    type Output = T;
}

/// A **type operator** that returns the absolute value.
///
/// # Example
/// ```rust
/// use typenum::{Abs, Integer, N5};
///
/// assert_eq!(<N5 as Abs>::Output::to_i32(), 5);
/// ```
pub trait Abs {
    /// The absolute value.
    type Output;
}

impl Abs for Z0 {
    type Output = Z0;
}

impl<U: Unsigned + NonZero> Abs for PInt<U> {
    type Output = Self;
}

impl<U: Unsigned + NonZero> Abs for NInt<U> {
    type Output = PInt<U>;
}

/// A **type operator** that provides exponentiation by repeated squaring.
///
/// # Example
/// ```rust
/// use typenum::{Integer, Pow, N3, P3};
///
/// assert_eq!(<N3 as Pow<P3>>::Output::to_i32(), -27);
/// ```
pub trait Pow<Exp> {
    /// The result of the exponentiation.
    type Output;
    /// This function isn't used in this crate, but may be useful for others.
    /// It is implemented for primitives.
    ///
    /// # Example
    /// ```rust
    /// use typenum::{Pow, U3};
    ///
    /// let a = 7u32.powi(U3::new());
    /// let b = 7u32.pow(3);
    /// assert_eq!(a, b);
    ///
    /// let x = 3.0.powi(U3::new());
    /// let y = 27.0;
    /// assert_eq!(x, y);
    /// ```
    fn powi(self, exp: Exp) -> Self::Output;
}

macro_rules! impl_pow_f {
    ($t:ty) => {
        impl Pow<UTerm> for $t {
            type Output = $t;
            #[inline]
            fn powi(self, _: UTerm) -> Self::Output {
                1.0
            }
        }

        impl<U: Unsigned, B: Bit> Pow<UInt<U, B>> for $t {
            type Output = $t;
            // powi is unstable in core, so we have to write this function ourselves.
            // copied from num::pow::pow
            #[inline]
            fn powi(self, _: UInt<U, B>) -> Self::Output {
                let mut exp = <UInt<U, B> as Unsigned>::to_u32();
                let mut base = self;

                if exp == 0 {
                    return 1.0;
                }

                while exp & 1 == 0 {
                    base *= base;
                    exp >>= 1;
                }
                if exp == 1 {
                    return base;
                }

                let mut acc = base.clone();
                while exp > 1 {
                    exp >>= 1;
                    base *= base;
                    if exp & 1 == 1 {
                        acc *= base.clone();
                    }
                }
                acc
            }
        }

        impl Pow<Z0> for $t {
            type Output = $t;
            #[inline]
            fn powi(self, _: Z0) -> Self::Output {
                1.0
            }
        }

        impl<U: Unsigned + NonZero> Pow<PInt<U>> for $t {
            type Output = $t;
            // powi is unstable in core, so we have to write this function ourselves.
            // copied from num::pow::pow
            #[inline]
            fn powi(self, _: PInt<U>) -> Self::Output {
                let mut exp = U::to_u32();
                let mut base = self;

                if exp == 0 {
                    return 1.0;
                }

                while exp & 1 == 0 {
                    base *= base;
                    exp >>= 1;
                }
                if exp == 1 {
                    return base;
                }

                let mut acc = base.clone();
                while exp > 1 {
                    exp >>= 1;
                    base *= base;
                    if exp & 1 == 1 {
                        acc *= base.clone();
                    }
                }
                acc
            }
        }

        impl<U: Unsigned + NonZero> Pow<NInt<U>> for $t {
            type Output = $t;

            #[inline]
            fn powi(self, _: NInt<U>) -> Self::Output {
                <$t as Pow<PInt<U>>>::powi(self, PInt::new()).recip()
            }
        }
    };
}

impl_pow_f!(f32);
impl_pow_f!(f64);

macro_rules! impl_pow_i {
    () => ();
    ($(#[$meta:meta])*  $t: ty $(, $tail:tt)*) => (
        $(#[$meta])*
        impl Pow<UTerm> for $t {
            type Output = $t;
            #[inline]
            fn powi(self, _: UTerm) -> Self::Output {
                1
            }
        }

        $(#[$meta])*
        impl<U: Unsigned, B: Bit> Pow<UInt<U, B>> for $t {
            type Output = $t;
            #[inline]
            fn powi(self, _: UInt<U, B>) -> Self::Output {
                self.pow(<UInt<U, B> as Unsigned>::to_u32())
            }
        }

        $(#[$meta])*
        impl Pow<Z0> for $t {
            type Output = $t;
            #[inline]
            fn powi(self, _: Z0) -> Self::Output {
                1
            }
        }

        $(#[$meta])*
        impl<U: Unsigned + NonZero> Pow<PInt<U>> for $t {
            type Output = $t;
            #[inline]
            fn powi(self, _: PInt<U>) -> Self::Output {
                self.pow(U::to_u32())
            }
        }

        impl_pow_i!($($tail),*);
    );
}

impl_pow_i!(u8, u16, u32, u64, usize, i8, i16, i32, i64, isize);
#[cfg(feature = "i128")]
impl_pow_i!(
    #[cfg_attr(docsrs, doc(cfg(feature = "i128")))]
    u128,
    i128
);

#[test]
fn pow_test() {
    use crate::consts::*;
    let z0 = Z0::new();
    let p3 = P3::new();

    let u0 = U0::new();
    let u3 = U3::new();
    let n3 = N3::new();

    macro_rules! check {
        ($x:ident) => {
            assert_eq!($x.powi(z0), 1);
            assert_eq!($x.powi(u0), 1);

            assert_eq!($x.powi(p3), $x * $x * $x);
            assert_eq!($x.powi(u3), $x * $x * $x);
        };
        ($x:ident, $f:ident) => {
            assert!((<$f as Pow<Z0>>::powi(*$x, z0) - 1.0).abs() < ::core::$f::EPSILON);
            assert!((<$f as Pow<U0>>::powi(*$x, u0) - 1.0).abs() < ::core::$f::EPSILON);

            assert!((<$f as Pow<P3>>::powi(*$x, p3) - $x * $x * $x).abs() < ::core::$f::EPSILON);
            assert!((<$f as Pow<U3>>::powi(*$x, u3) - $x * $x * $x).abs() < ::core::$f::EPSILON);

            if *$x == 0.0 {
                assert!(<$f as Pow<N3>>::powi(*$x, n3).is_infinite());
            } else {
                assert!(
                    (<$f as Pow<N3>>::powi(*$x, n3) - 1. / $x / $x / $x).abs()
                        < ::core::$f::EPSILON
                );
            }
        };
    }

    for x in &[0i8, -3, 2] {
        check!(x);
    }
    for x in &[0u8, 1, 5] {
        check!(x);
    }
    for x in &[0usize, 1, 5, 40] {
        check!(x);
    }
    for x in &[0isize, 1, 2, -30, -22, 48] {
        check!(x);
    }
    for x in &[0.0f32, 2.2, -3.5, 378.223] {
        check!(x, f32);
    }
    for x in &[0.0f64, 2.2, -3.5, -2387.2, 234.22] {
        check!(x, f64);
    }
}

/// A **type operator** for comparing `Self` and `Rhs`. It provides a similar functionality to
/// the function
/// [`core::cmp::Ord::cmp`](https://doc.rust-lang.org/nightly/core/cmp/trait.Ord.html#tymethod.cmp)
/// but for types.
///
/// # Example
/// ```rust
/// use typenum::{Cmp, Ord, N3, P2, P5};
/// use std::cmp::Ordering;
///
/// assert_eq!(<P2 as Cmp<N3>>::Output::to_ordering(), Ordering::Greater);
/// assert_eq!(<P2 as Cmp<P2>>::Output::to_ordering(), Ordering::Equal);
/// assert_eq!(<P2 as Cmp<P5>>::Output::to_ordering(), Ordering::Less);
pub trait Cmp<Rhs = Self> {
    /// The result of the comparison. It should only ever be one of `Greater`, `Less`, or `Equal`.
    type Output;

    #[doc(hidden)]
    fn compare<IM: InternalMarker>(&self, _: &Rhs) -> Self::Output;
}

/// A **type operator** that gives the length of an `Array` or the number of bits in a `UInt`.
#[allow(clippy::len_without_is_empty)]
pub trait Len {
    /// The length as a type-level unsigned integer.
    type Output: crate::Unsigned;
    /// This function isn't used in this crate, but may be useful for others.
    fn len(&self) -> Self::Output;
}

/// A **type operator** that gives the sum of all elements of an `Array`.
pub trait FoldAdd {
    /// The type of the result of the sum
    type Output;
}

/// A **type operator** that gives the product of all elements of an `Array`.
pub trait FoldMul {
    /// The type of the result of the product
    type Output;
}

#[test]
fn fold_test() {
    use crate::*;
    assert_eq!(10, <FoldSum::<tarr![U2, U3, U5]>>::to_u32());
    assert_eq!(30, <FoldProd::<tarr![U2, U3, U5]>>::to_u32());
}

/// Division as a partial function. This **type operator** performs division just as `Div`, but is
/// only defined when the result is an integer (i.e. there is no remainder).
pub trait PartialDiv<Rhs = Self> {
    /// The type of the result of the division
    type Output;
    /// Method for performing the division
    fn partial_div(self, _: Rhs) -> Self::Output;
}

/// A **type operator** that returns the minimum of `Self` and `Rhs`.
pub trait Min<Rhs = Self> {
    /// The type of the minimum of `Self` and `Rhs`
    type Output;
    /// Method returning the minimum
    fn min(self, rhs: Rhs) -> Self::Output;
}

/// A **type operator** that returns the maximum of `Self` and `Rhs`.
pub trait Max<Rhs = Self> {
    /// The type of the maximum of `Self` and `Rhs`
    type Output;
    /// Method returning the maximum
    fn max(self, rhs: Rhs) -> Self::Output;
}

use crate::Compare;

/// A **type operator** that returns `True` if `Self < Rhs`, otherwise returns `False`.
pub trait IsLess<Rhs = Self> {
    /// The type representing either `True` or `False`
    type Output: Bit;
    /// Method returning `True` or `False`.
    #[allow(clippy::wrong_self_convention)]
    fn is_less(self, rhs: Rhs) -> Self::Output;
}

use crate::private::IsLessPrivate;
impl<A, B> IsLess<B> for A
where
    A: Cmp<B> + IsLessPrivate<B, Compare<A, B>>,
{
    type Output = <A as IsLessPrivate<B, Compare<A, B>>>::Output;

    #[inline]
    fn is_less(self, rhs: B) -> Self::Output {
        let lhs_cmp_rhs = self.compare::<Internal>(&rhs);
        self.is_less_private(rhs, lhs_cmp_rhs)
    }
}

/// A **type operator** that returns `True` if `Self == Rhs`, otherwise returns `False`.
pub trait IsEqual<Rhs = Self> {
    /// The type representing either `True` or `False`
    type Output: Bit;
    /// Method returning `True` or `False`.
    #[allow(clippy::wrong_self_convention)]
    fn is_equal(self, rhs: Rhs) -> Self::Output;
}

use crate::private::IsEqualPrivate;
impl<A, B> IsEqual<B> for A
where
    A: Cmp<B> + IsEqualPrivate<B, Compare<A, B>>,
{
    type Output = <A as IsEqualPrivate<B, Compare<A, B>>>::Output;

    #[inline]
    fn is_equal(self, rhs: B) -> Self::Output {
        let lhs_cmp_rhs = self.compare::<Internal>(&rhs);
        self.is_equal_private(rhs, lhs_cmp_rhs)
    }
}

/// A **type operator** that returns `True` if `Self > Rhs`, otherwise returns `False`.
pub trait IsGreater<Rhs = Self> {
    /// The type representing either `True` or `False`
    type Output: Bit;
    /// Method returning `True` or `False`.
    #[allow(clippy::wrong_self_convention)]
    fn is_greater(self, rhs: Rhs) -> Self::Output;
}

use crate::private::IsGreaterPrivate;
impl<A, B> IsGreater<B> for A
where
    A: Cmp<B> + IsGreaterPrivate<B, Compare<A, B>>,
{
    type Output = <A as IsGreaterPrivate<B, Compare<A, B>>>::Output;

    #[inline]
    fn is_greater(self, rhs: B) -> Self::Output {
        let lhs_cmp_rhs = self.compare::<Internal>(&rhs);
        self.is_greater_private(rhs, lhs_cmp_rhs)
    }
}

/// A **type operator** that returns `True` if `Self <= Rhs`, otherwise returns `False`.
pub trait IsLessOrEqual<Rhs = Self> {
    /// The type representing either `True` or `False`
    type Output: Bit;
    /// Method returning `True` or `False`.
    #[allow(clippy::wrong_self_convention)]
    fn is_less_or_equal(self, rhs: Rhs) -> Self::Output;
}

use crate::private::IsLessOrEqualPrivate;
impl<A, B> IsLessOrEqual<B> for A
where
    A: Cmp<B> + IsLessOrEqualPrivate<B, Compare<A, B>>,
{
    type Output = <A as IsLessOrEqualPrivate<B, Compare<A, B>>>::Output;

    #[inline]
    fn is_less_or_equal(self, rhs: B) -> Self::Output {
        let lhs_cmp_rhs = self.compare::<Internal>(&rhs);
        self.is_less_or_equal_private(rhs, lhs_cmp_rhs)
    }
}

/// A **type operator** that returns `True` if `Self != Rhs`, otherwise returns `False`.
pub trait IsNotEqual<Rhs = Self> {
    /// The type representing either `True` or `False`
    type Output: Bit;
    /// Method returning `True` or `False`.
    #[allow(clippy::wrong_self_convention)]
    fn is_not_equal(self, rhs: Rhs) -> Self::Output;
}

use crate::private::IsNotEqualPrivate;
impl<A, B> IsNotEqual<B> for A
where
    A: Cmp<B> + IsNotEqualPrivate<B, Compare<A, B>>,
{
    type Output = <A as IsNotEqualPrivate<B, Compare<A, B>>>::Output;

    #[inline]
    fn is_not_equal(self, rhs: B) -> Self::Output {
        let lhs_cmp_rhs = self.compare::<Internal>(&rhs);
        self.is_not_equal_private(rhs, lhs_cmp_rhs)
    }
}

/// A **type operator** that returns `True` if `Self >= Rhs`, otherwise returns `False`.
pub trait IsGreaterOrEqual<Rhs = Self> {
    /// The type representing either `True` or `False`
    type Output: Bit;
    /// Method returning `True` or `False`.
    #[allow(clippy::wrong_self_convention)]
    fn is_greater_or_equal(self, rhs: Rhs) -> Self::Output;
}

use crate::private::IsGreaterOrEqualPrivate;
impl<A, B> IsGreaterOrEqual<B> for A
where
    A: Cmp<B> + IsGreaterOrEqualPrivate<B, Compare<A, B>>,
{
    type Output = <A as IsGreaterOrEqualPrivate<B, Compare<A, B>>>::Output;

    #[inline]
    fn is_greater_or_equal(self, rhs: B) -> Self::Output {
        let lhs_cmp_rhs = self.compare::<Internal>(&rhs);
        self.is_greater_or_equal_private(rhs, lhs_cmp_rhs)
    }
}

/**
A convenience macro for comparing type numbers. Use `op!` instead.

Due to the intricacies of the macro system, if the left-hand operand is more complex than a simple
`ident`, you must place a comma between it and the comparison sign.

For example, you can do `cmp!(P5 > P3)` or `cmp!(typenum::P5, > typenum::P3)` but not
`cmp!(typenum::P5 > typenum::P3)`.

The result of this comparison will always be one of `True` (aka `B1`) or `False` (aka `B0`).

# Example
```rust
#[macro_use] extern crate typenum;
use typenum::consts::*;
use typenum::Bit;

fn main() {
type Result = cmp!(P9 == op!(P1 + P2 * (P2 - N2)));
assert_eq!(Result::to_bool(), true);
}
```
 */
#[deprecated(since = "1.9.0", note = "use the `op!` macro instead")]
#[macro_export]
macro_rules! cmp {
    ($a:ident < $b:ty) => {
        <$a as $crate::IsLess<$b>>::Output
    };
    ($a:ty, < $b:ty) => {
        <$a as $crate::IsLess<$b>>::Output
    };

    ($a:ident == $b:ty) => {
        <$a as $crate::IsEqual<$b>>::Output
    };
    ($a:ty, == $b:ty) => {
        <$a as $crate::IsEqual<$b>>::Output
    };

    ($a:ident > $b:ty) => {
        <$a as $crate::IsGreater<$b>>::Output
    };
    ($a:ty, > $b:ty) => {
        <$a as $crate::IsGreater<$b>>::Output
    };

    ($a:ident <= $b:ty) => {
        <$a as $crate::IsLessOrEqual<$b>>::Output
    };
    ($a:ty, <= $b:ty) => {
        <$a as $crate::IsLessOrEqual<$b>>::Output
    };

    ($a:ident != $b:ty) => {
        <$a as $crate::IsNotEqual<$b>>::Output
    };
    ($a:ty, != $b:ty) => {
        <$a as $crate::IsNotEqual<$b>>::Output
    };

    ($a:ident >= $b:ty) => {
        <$a as $crate::IsGreaterOrEqual<$b>>::Output
    };
    ($a:ty, >= $b:ty) => {
        <$a as $crate::IsGreaterOrEqual<$b>>::Output
    };
}

/// A **type operator** for taking the integer square root of `Self`.
///
/// The integer square root of `n` is the largest integer `m` such
/// that `n >= m*m`. This definition is equivalent to truncating the
/// real-valued square root: `floor(real_sqrt(n))`.
pub trait SquareRoot {
    /// The result of the integer square root.
    type Output;
}

/// A **type operator** for taking the integer binary logarithm of `Self`.
///
/// The integer binary logarighm of `n` is the largest integer `m` such
/// that `n >= 2^m`. This definition is equivalent to truncating the
/// real-valued binary logarithm: `floor(log2(n))`.
pub trait Logarithm2 {
    /// The result of the integer binary logarithm.
    type Output;
}

/// A **type operator** that computes the [greatest common divisor][gcd] of `Self` and `Rhs`.
///
/// [gcd]: https://en.wikipedia.org/wiki/Greatest_common_divisor
///
/// # Example
///
/// ```rust
/// use typenum::{Gcd, Unsigned, U12, U8};
///
/// assert_eq!(<U12 as Gcd<U8>>::Output::to_i32(), 4);
/// ```
pub trait Gcd<Rhs> {
    /// The greatest common divisor.
    type Output;
}

/// A **type operator** for taking a concrete integer value from a type.
///
/// It returns arbitrary integer value without explicitly specifying the
/// type. It is useful when you pass the values to methods that accept
/// distinct types without runtime casting.
pub trait ToInt<T> {
    /// Method returning the concrete value for the type.
    fn to_int() -> T;
    /// The concrete value for the type. Can be used in `const` contexts.
    const INT: T;
}
