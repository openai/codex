// Copyright 2013-2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Numeric traits for generic mathematics
//!
//! ## Compatibility
//!
//! The `num-traits` crate is tested for rustc 1.60 and greater.

#![doc(html_root_url = "https://docs.rs/num-traits/0.2")]
#![deny(unconditional_recursion)]
#![no_std]

// Need to explicitly bring the crate in for inherent float methods
#[cfg(feature = "std")]
extern crate std;

use core::fmt;
use core::num::Wrapping;
use core::ops::{Add, Div, Mul, Rem, Sub};
use core::ops::{AddAssign, DivAssign, MulAssign, RemAssign, SubAssign};

pub use crate::bounds::Bounded;
#[cfg(any(feature = "std", feature = "libm"))]
pub use crate::float::Float;
pub use crate::float::FloatConst;
// pub use real::{FloatCore, Real}; // NOTE: Don't do this, it breaks `use num_traits::*;`.
pub use crate::cast::{cast, AsPrimitive, FromPrimitive, NumCast, ToPrimitive};
pub use crate::identities::{one, zero, ConstOne, ConstZero, One, Zero};
pub use crate::int::PrimInt;
pub use crate::ops::bytes::{FromBytes, ToBytes};
pub use crate::ops::checked::{
    CheckedAdd, CheckedDiv, CheckedMul, CheckedNeg, CheckedRem, CheckedShl, CheckedShr, CheckedSub,
};
pub use crate::ops::euclid::{CheckedEuclid, Euclid};
pub use crate::ops::inv::Inv;
pub use crate::ops::mul_add::{MulAdd, MulAddAssign};
pub use crate::ops::saturating::{Saturating, SaturatingAdd, SaturatingMul, SaturatingSub};
pub use crate::ops::wrapping::{
    WrappingAdd, WrappingMul, WrappingNeg, WrappingShl, WrappingShr, WrappingSub,
};
pub use crate::pow::{checked_pow, pow, Pow};
pub use crate::sign::{abs, abs_sub, signum, Signed, Unsigned};

#[macro_use]
mod macros;

pub mod bounds;
pub mod cast;
pub mod float;
pub mod identities;
pub mod int;
pub mod ops;
pub mod pow;
pub mod real;
pub mod sign;

/// The base trait for numeric types, covering `0` and `1` values,
/// comparisons, basic numeric operations, and string conversion.
pub trait Num: PartialEq + Zero + One + NumOps {
    type FromStrRadixErr;

    /// Convert from a string and radix (typically `2..=36`).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use num_traits::Num;
    ///
    /// let result = <i32 as Num>::from_str_radix("27", 10);
    /// assert_eq!(result, Ok(27));
    ///
    /// let result = <i32 as Num>::from_str_radix("foo", 10);
    /// assert!(result.is_err());
    /// ```
    ///
    /// # Supported radices
    ///
    /// The exact range of supported radices is at the discretion of each type implementation. For
    /// primitive integers, this is implemented by the inherent `from_str_radix` methods in the
    /// standard library, which **panic** if the radix is not in the range from 2 to 36. The
    /// implementation in this crate for primitive floats is similar.
    ///
    /// For third-party types, it is suggested that implementations should follow suit and at least
    /// accept `2..=36` without panicking, but an `Err` may be returned for any unsupported radix.
    /// It's possible that a type might not even support the common radix 10, nor any, if string
    /// parsing doesn't make sense for that type.
    fn from_str_radix(str: &str, radix: u32) -> Result<Self, Self::FromStrRadixErr>;
}

/// Generic trait for types implementing basic numeric operations
///
/// This is automatically implemented for types which implement the operators.
pub trait NumOps<Rhs = Self, Output = Self>:
    Add<Rhs, Output = Output>
    + Sub<Rhs, Output = Output>
    + Mul<Rhs, Output = Output>
    + Div<Rhs, Output = Output>
    + Rem<Rhs, Output = Output>
{
}

impl<T, Rhs, Output> NumOps<Rhs, Output> for T where
    T: Add<Rhs, Output = Output>
        + Sub<Rhs, Output = Output>
        + Mul<Rhs, Output = Output>
        + Div<Rhs, Output = Output>
        + Rem<Rhs, Output = Output>
{
}

/// The trait for `Num` types which also implement numeric operations taking
/// the second operand by reference.
///
/// This is automatically implemented for types which implement the operators.
pub trait NumRef: Num + for<'r> NumOps<&'r Self> {}
impl<T> NumRef for T where T: Num + for<'r> NumOps<&'r T> {}

/// The trait for `Num` references which implement numeric operations, taking the
/// second operand either by value or by reference.
///
/// This is automatically implemented for all types which implement the operators. It covers
/// every type implementing the operations though, regardless of it being a reference or
/// related to `Num`.
pub trait RefNum<Base>: NumOps<Base, Base> + for<'r> NumOps<&'r Base, Base> {}
impl<T, Base> RefNum<Base> for T where T: NumOps<Base, Base> + for<'r> NumOps<&'r Base, Base> {}

/// Generic trait for types implementing numeric assignment operators (like `+=`).
///
/// This is automatically implemented for types which implement the operators.
pub trait NumAssignOps<Rhs = Self>:
    AddAssign<Rhs> + SubAssign<Rhs> + MulAssign<Rhs> + DivAssign<Rhs> + RemAssign<Rhs>
{
}

impl<T, Rhs> NumAssignOps<Rhs> for T where
    T: AddAssign<Rhs> + SubAssign<Rhs> + MulAssign<Rhs> + DivAssign<Rhs> + RemAssign<Rhs>
{
}

/// The trait for `Num` types which also implement assignment operators.
///
/// This is automatically implemented for types which implement the operators.
pub trait NumAssign: Num + NumAssignOps {}
impl<T> NumAssign for T where T: Num + NumAssignOps {}

/// The trait for `NumAssign` types which also implement assignment operations
/// taking the second operand by reference.
///
/// This is automatically implemented for types which implement the operators.
pub trait NumAssignRef: NumAssign + for<'r> NumAssignOps<&'r Self> {}
impl<T> NumAssignRef for T where T: NumAssign + for<'r> NumAssignOps<&'r T> {}

macro_rules! int_trait_impl {
    ($name:ident for $($t:ty)*) => ($(
        impl $name for $t {
            type FromStrRadixErr = ::core::num::ParseIntError;
            #[inline]
            fn from_str_radix(s: &str, radix: u32)
                              -> Result<Self, ::core::num::ParseIntError>
            {
                <$t>::from_str_radix(s, radix)
            }
        }
    )*)
}
int_trait_impl!(Num for usize u8 u16 u32 u64 u128);
int_trait_impl!(Num for isize i8 i16 i32 i64 i128);

impl<T: Num> Num for Wrapping<T>
where
    Wrapping<T>: NumOps,
{
    type FromStrRadixErr = T::FromStrRadixErr;
    fn from_str_radix(str: &str, radix: u32) -> Result<Self, Self::FromStrRadixErr> {
        T::from_str_radix(str, radix).map(Wrapping)
    }
}

#[derive(Debug)]
pub enum FloatErrorKind {
    Empty,
    Invalid,
}
// FIXME: core::num::ParseFloatError is stable in 1.0, but opaque to us,
// so there's not really any way for us to reuse it.
#[derive(Debug)]
pub struct ParseFloatError {
    pub kind: FloatErrorKind,
}

impl fmt::Display for ParseFloatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let description = match self.kind {
            FloatErrorKind::Empty => "cannot parse float from empty string",
            FloatErrorKind::Invalid => "invalid float literal",
        };

        description.fmt(f)
    }
}

fn str_to_ascii_lower_eq_str(a: &str, b: &str) -> bool {
    a.len() == b.len()
        && a.bytes().zip(b.bytes()).all(|(a, b)| {
            let a_to_ascii_lower = a | (((b'A' <= a && a <= b'Z') as u8) << 5);
            a_to_ascii_lower == b
        })
}

// FIXME: The standard library from_str_radix on floats was deprecated, so we're stuck
// with this implementation ourselves until we want to make a breaking change.
// (would have to drop it from `Num` though)
macro_rules! float_trait_impl {
    ($name:ident for $($t:ident)*) => ($(
        impl $name for $t {
            type FromStrRadixErr = ParseFloatError;

            fn from_str_radix(src: &str, radix: u32)
                              -> Result<Self, Self::FromStrRadixErr>
            {
                use self::FloatErrorKind::*;
                use self::ParseFloatError as PFE;

                // Special case radix 10 to use more accurate standard library implementation
                if radix == 10 {
                    return src.parse().map_err(|_| PFE {
                        kind: if src.is_empty() { Empty } else { Invalid },
                    });
                }

                // Special values
                if str_to_ascii_lower_eq_str(src, "inf")
                    || str_to_ascii_lower_eq_str(src, "infinity")
                {
                    return Ok(core::$t::INFINITY);
                } else if str_to_ascii_lower_eq_str(src, "-inf")
                    || str_to_ascii_lower_eq_str(src, "-infinity")
                {
                    return Ok(core::$t::NEG_INFINITY);
                } else if str_to_ascii_lower_eq_str(src, "nan") {
                    return Ok(core::$t::NAN);
                } else if str_to_ascii_lower_eq_str(src, "-nan") {
                    return Ok(-core::$t::NAN);
                }

                fn slice_shift_char(src: &str) -> Option<(char, &str)> {
                    let mut chars = src.chars();
                    Some((chars.next()?, chars.as_str()))
                }

                let (is_positive, src) =  match slice_shift_char(src) {
                    None             => return Err(PFE { kind: Empty }),
                    Some(('-', ""))  => return Err(PFE { kind: Empty }),
                    Some(('-', src)) => (false, src),
                    Some((_, _))     => (true,  src),
                };

                // The significand to accumulate
                let mut sig = if is_positive { 0.0 } else { -0.0 };
                // Necessary to detect overflow
                let mut prev_sig = sig;
                let mut cs = src.chars().enumerate();
                // Exponent prefix and exponent index offset
                let mut exp_info = None::<(char, usize)>;

                // Parse the integer part of the significand
                for (i, c) in cs.by_ref() {
                    match c.to_digit(radix) {
                        Some(digit) => {
                            // shift significand one digit left
                            sig *= radix as $t;

                            // add/subtract current digit depending on sign
                            if is_positive {
                                sig += (digit as isize) as $t;
                            } else {
                                sig -= (digit as isize) as $t;
                            }

                            // Detect overflow by comparing to last value, except
                            // if we've not seen any non-zero digits.
                            if prev_sig != 0.0 {
                                if is_positive && sig <= prev_sig
                                    { return Ok(core::$t::INFINITY); }
                                if !is_positive && sig >= prev_sig
                                    { return Ok(core::$t::NEG_INFINITY); }

                                // Detect overflow by reversing the shift-and-add process
                                if is_positive && (prev_sig != (sig - digit as $t) / radix as $t)
                                    { return Ok(core::$t::INFINITY); }
                                if !is_positive && (prev_sig != (sig + digit as $t) / radix as $t)
                                    { return Ok(core::$t::NEG_INFINITY); }
                            }
                            prev_sig = sig;
                        },
                        None => match c {
                            'e' | 'E' | 'p' | 'P' => {
                                exp_info = Some((c, i + 1));
                                break;  // start of exponent
                            },
                            '.' => {
                                break;  // start of fractional part
                            },
                            _ => {
                                return Err(PFE { kind: Invalid });
                            },
                        },
                    }
                }

                // If we are not yet at the exponent parse the fractional
                // part of the significand
                if exp_info.is_none() {
                    let mut power = 1.0;
                    for (i, c) in cs.by_ref() {
                        match c.to_digit(radix) {
                            Some(digit) => {
                                // Decrease power one order of magnitude
                                power /= radix as $t;
                                // add/subtract current digit depending on sign
                                sig = if is_positive {
                                    sig + (digit as $t) * power
                                } else {
                                    sig - (digit as $t) * power
                                };
                                // Detect overflow by comparing to last value
                                if is_positive && sig < prev_sig
                                    { return Ok(core::$t::INFINITY); }
                                if !is_positive && sig > prev_sig
                                    { return Ok(core::$t::NEG_INFINITY); }
                                prev_sig = sig;
                            },
                            None => match c {
                                'e' | 'E' | 'p' | 'P' => {
                                    exp_info = Some((c, i + 1));
                                    break; // start of exponent
                                },
                                _ => {
                                    return Err(PFE { kind: Invalid });
                                },
                            },
                        }
                    }
                }

                // Parse and calculate the exponent
                let exp = match exp_info {
                    Some((c, offset)) => {
                        let base = match c {
                            'E' | 'e' if radix == 10 => 10.0,
                            'P' | 'p' if radix == 16 => 2.0,
                            _ => return Err(PFE { kind: Invalid }),
                        };

                        // Parse the exponent as decimal integer
                        let src = &src[offset..];
                        let (is_positive, exp) = match slice_shift_char(src) {
                            Some(('-', src)) => (false, src.parse::<usize>()),
                            Some(('+', src)) => (true,  src.parse::<usize>()),
                            Some((_, _))     => (true,  src.parse::<usize>()),
                            None             => return Err(PFE { kind: Invalid }),
                        };

                        #[cfg(feature = "std")]
                        fn pow(base: $t, exp: usize) -> $t {
                            Float::powi(base, exp as i32)
                        }
                        // otherwise uses the generic `pow` from the root

                        match (is_positive, exp) {
                            (true,  Ok(exp)) => pow(base, exp),
                            (false, Ok(exp)) => 1.0 / pow(base, exp),
                            (_, Err(_))      => return Err(PFE { kind: Invalid }),
                        }
                    },
                    None => 1.0, // no exponent
                };

                Ok(sig * exp)
            }
        }
    )*)
}
float_trait_impl!(Num for f32 f64);

/// A value bounded by a minimum and a maximum
///
///  If input is less than min then this returns min.
///  If input is greater than max then this returns max.
///  Otherwise this returns input.
///
/// **Panics** in debug mode if `!(min <= max)`.
#[inline]
pub fn clamp<T: PartialOrd>(input: T, min: T, max: T) -> T {
    debug_assert!(min <= max, "min must be less than or equal to max");
    if input < min {
        min
    } else if input > max {
        max
    } else {
        input
    }
}

/// A value bounded by a minimum value
///
///  If input is less than min then this returns min.
///  Otherwise this returns input.
///  `clamp_min(std::f32::NAN, 1.0)` preserves `NAN` different from `f32::min(std::f32::NAN, 1.0)`.
///
/// **Panics** in debug mode if `!(min == min)`. (This occurs if `min` is `NAN`.)
#[inline]
#[allow(clippy::eq_op)]
pub fn clamp_min<T: PartialOrd>(input: T, min: T) -> T {
    debug_assert!(min == min, "min must not be NAN");
    if input < min {
        min
    } else {
        input
    }
}

/// A value bounded by a maximum value
///
///  If input is greater than max then this returns max.
///  Otherwise this returns input.
///  `clamp_max(std::f32::NAN, 1.0)` preserves `NAN` different from `f32::max(std::f32::NAN, 1.0)`.
///
/// **Panics** in debug mode if `!(max == max)`. (This occurs if `max` is `NAN`.)
#[inline]
#[allow(clippy::eq_op)]
pub fn clamp_max<T: PartialOrd>(input: T, max: T) -> T {
    debug_assert!(max == max, "max must not be NAN");
    if input > max {
        max
    } else {
        input
    }
}

#[test]
fn clamp_test() {
    // Int test
    assert_eq!(1, clamp(1, -1, 2));
    assert_eq!(-1, clamp(-2, -1, 2));
    assert_eq!(2, clamp(3, -1, 2));
    assert_eq!(1, clamp_min(1, -1));
    assert_eq!(-1, clamp_min(-2, -1));
    assert_eq!(-1, clamp_max(1, -1));
    assert_eq!(-2, clamp_max(-2, -1));

    // Float test
    assert_eq!(1.0, clamp(1.0, -1.0, 2.0));
    assert_eq!(-1.0, clamp(-2.0, -1.0, 2.0));
    assert_eq!(2.0, clamp(3.0, -1.0, 2.0));
    assert_eq!(1.0, clamp_min(1.0, -1.0));
    assert_eq!(-1.0, clamp_min(-2.0, -1.0));
    assert_eq!(-1.0, clamp_max(1.0, -1.0));
    assert_eq!(-2.0, clamp_max(-2.0, -1.0));
    assert!(clamp(::core::f32::NAN, -1.0, 1.0).is_nan());
    assert!(clamp_min(::core::f32::NAN, 1.0).is_nan());
    assert!(clamp_max(::core::f32::NAN, 1.0).is_nan());
}

#[test]
#[should_panic]
#[cfg(debug_assertions)]
fn clamp_nan_min() {
    clamp(0., ::core::f32::NAN, 1.);
}

#[test]
#[should_panic]
#[cfg(debug_assertions)]
fn clamp_nan_max() {
    clamp(0., -1., ::core::f32::NAN);
}

#[test]
#[should_panic]
#[cfg(debug_assertions)]
fn clamp_nan_min_max() {
    clamp(0., ::core::f32::NAN, ::core::f32::NAN);
}

#[test]
#[should_panic]
#[cfg(debug_assertions)]
fn clamp_min_nan_min() {
    clamp_min(0., ::core::f32::NAN);
}

#[test]
#[should_panic]
#[cfg(debug_assertions)]
fn clamp_max_nan_max() {
    clamp_max(0., ::core::f32::NAN);
}

#[test]
fn from_str_radix_unwrap() {
    // The Result error must impl Debug to allow unwrap()

    let i: i32 = Num::from_str_radix("0", 10).unwrap();
    assert_eq!(i, 0);

    let f: f32 = Num::from_str_radix("0.0", 10).unwrap();
    assert_eq!(f, 0.0);
}

#[test]
fn from_str_radix_multi_byte_fail() {
    // Ensure parsing doesn't panic, even on invalid sign characters
    assert!(f32::from_str_radix("™0.2", 10).is_err());

    // Even when parsing the exponent sign
    assert!(f32::from_str_radix("0.2E™1", 10).is_err());
}

#[test]
fn from_str_radix_ignore_case() {
    assert_eq!(
        f32::from_str_radix("InF", 16).unwrap(),
        ::core::f32::INFINITY
    );
    assert_eq!(
        f32::from_str_radix("InfinitY", 16).unwrap(),
        ::core::f32::INFINITY
    );
    assert_eq!(
        f32::from_str_radix("-InF", 8).unwrap(),
        ::core::f32::NEG_INFINITY
    );
    assert_eq!(
        f32::from_str_radix("-InfinitY", 8).unwrap(),
        ::core::f32::NEG_INFINITY
    );
    assert!(f32::from_str_radix("nAn", 4).unwrap().is_nan());
    assert!(f32::from_str_radix("-nAn", 4).unwrap().is_nan());
}

#[test]
fn wrapping_is_num() {
    fn require_num<T: Num>(_: &T) {}
    require_num(&Wrapping(42_u32));
    require_num(&Wrapping(-42));
}

#[test]
fn wrapping_from_str_radix() {
    macro_rules! test_wrapping_from_str_radix {
        ($($t:ty)+) => {
            $(
                for &(s, r) in &[("42", 10), ("42", 2), ("-13.0", 10), ("foo", 10)] {
                    let w = Wrapping::<$t>::from_str_radix(s, r).map(|w| w.0);
                    assert_eq!(w, <$t as Num>::from_str_radix(s, r));
                }
            )+
        };
    }

    test_wrapping_from_str_radix!(usize u8 u16 u32 u64 isize i8 i16 i32 i64);
}

#[test]
fn check_num_ops() {
    fn compute<T: Num + Copy>(x: T, y: T) -> T {
        x * y / y % y + y - y
    }
    assert_eq!(compute(1, 2), 1)
}

#[test]
fn check_numref_ops() {
    fn compute<T: NumRef>(x: T, y: &T) -> T {
        x * y / y % y + y - y
    }
    assert_eq!(compute(1, &2), 1)
}

#[test]
fn check_refnum_ops() {
    fn compute<T: Copy>(x: &T, y: T) -> T
    where
        for<'a> &'a T: RefNum<T>,
    {
        &(&(&(&(x * y) / y) % y) + y) - y
    }
    assert_eq!(compute(&1, 2), 1)
}

#[test]
fn check_refref_ops() {
    fn compute<T>(x: &T, y: &T) -> T
    where
        for<'a> &'a T: RefNum<T>,
    {
        &(&(&(&(x * y) / y) % y) + y) - y
    }
    assert_eq!(compute(&1, &2), 1)
}

#[test]
fn check_numassign_ops() {
    fn compute<T: NumAssign + Copy>(mut x: T, y: T) -> T {
        x *= y;
        x /= y;
        x %= y;
        x += y;
        x -= y;
        x
    }
    assert_eq!(compute(1, 2), 1)
}

#[test]
fn check_numassignref_ops() {
    fn compute<T: NumAssignRef + Copy>(mut x: T, y: &T) -> T {
        x *= y;
        x /= y;
        x %= y;
        x += y;
        x -= y;
        x
    }
    assert_eq!(compute(1, &2), 1)
}
