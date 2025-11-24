// Adapted from https://github.com/Alexhuszagh/rust-lexical.

//! Utilities for Rust numbers.

use core::ops;

/// Precalculated values of radix**i for i in range [0, arr.len()-1].
/// Each value can be **exactly** represented as that type.
const F32_POW10: [f32; 11] = [
    1.0,
    10.0,
    100.0,
    1000.0,
    10000.0,
    100000.0,
    1000000.0,
    10000000.0,
    100000000.0,
    1000000000.0,
    10000000000.0,
];

/// Precalculated values of radix**i for i in range [0, arr.len()-1].
/// Each value can be **exactly** represented as that type.
const F64_POW10: [f64; 23] = [
    1.0,
    10.0,
    100.0,
    1000.0,
    10000.0,
    100000.0,
    1000000.0,
    10000000.0,
    100000000.0,
    1000000000.0,
    10000000000.0,
    100000000000.0,
    1000000000000.0,
    10000000000000.0,
    100000000000000.0,
    1000000000000000.0,
    10000000000000000.0,
    100000000000000000.0,
    1000000000000000000.0,
    10000000000000000000.0,
    100000000000000000000.0,
    1000000000000000000000.0,
    10000000000000000000000.0,
];

/// Type that can be converted to primitive with `as`.
pub trait AsPrimitive: Sized + Copy + PartialOrd {
    fn as_u32(self) -> u32;
    fn as_u64(self) -> u64;
    fn as_u128(self) -> u128;
    fn as_usize(self) -> usize;
    fn as_f32(self) -> f32;
    fn as_f64(self) -> f64;
}

macro_rules! as_primitive_impl {
    ($($ty:ident)*) => {
        $(
            impl AsPrimitive for $ty {
                #[inline]
                fn as_u32(self) -> u32 {
                    self as u32
                }

                #[inline]
                fn as_u64(self) -> u64 {
                    self as u64
                }

                #[inline]
                fn as_u128(self) -> u128 {
                    self as u128
                }

                #[inline]
                fn as_usize(self) -> usize {
                    self as usize
                }

                #[inline]
                fn as_f32(self) -> f32 {
                    self as f32
                }

                #[inline]
                fn as_f64(self) -> f64 {
                    self as f64
                }
            }
        )*
    };
}

as_primitive_impl! { u32 u64 u128 usize f32 f64 }

/// An interface for casting between machine scalars.
pub trait AsCast: AsPrimitive {
    /// Creates a number from another value that can be converted into
    /// a primitive via the `AsPrimitive` trait.
    fn as_cast<N: AsPrimitive>(n: N) -> Self;
}

macro_rules! as_cast_impl {
    ($ty:ident, $method:ident) => {
        impl AsCast for $ty {
            #[inline]
            fn as_cast<N: AsPrimitive>(n: N) -> Self {
                n.$method()
            }
        }
    };
}

as_cast_impl!(u32, as_u32);
as_cast_impl!(u64, as_u64);
as_cast_impl!(u128, as_u128);
as_cast_impl!(usize, as_usize);
as_cast_impl!(f32, as_f32);
as_cast_impl!(f64, as_f64);

/// Numerical type trait.
pub trait Number: AsCast + ops::Add<Output = Self> {}

macro_rules! number_impl {
    ($($ty:ident)*) => {
        $(
            impl Number for $ty {}
        )*
    };
}

number_impl! { u32 u64 u128 usize f32 f64 }

/// Defines a trait that supports integral operations.
pub trait Integer: Number + ops::BitAnd<Output = Self> + ops::Shr<i32, Output = Self> {
    const ZERO: Self;
}

macro_rules! integer_impl {
    ($($ty:tt)*) => {
        $(
            impl Integer for $ty {
                const ZERO: Self = 0;
            }
        )*
    };
}

integer_impl! { u32 u64 u128 usize }

/// Type trait for the mantissa type.
pub trait Mantissa: Integer {
    /// Mask to extract the high bits from the integer.
    const HIMASK: Self;
    /// Mask to extract the low bits from the integer.
    const LOMASK: Self;
    /// Full size of the integer, in bits.
    const FULL: i32;
    /// Half size of the integer, in bits.
    const HALF: i32 = Self::FULL / 2;
}

impl Mantissa for u64 {
    const HIMASK: u64 = 0xFFFFFFFF00000000;
    const LOMASK: u64 = 0x00000000FFFFFFFF;
    const FULL: i32 = 64;
}

/// Get exact exponent limit for radix.
pub trait Float: Number {
    /// Unsigned type of the same size.
    type Unsigned: Integer;

    /// Literal zero.
    const ZERO: Self;
    /// Maximum number of digits that can contribute in the mantissa.
    ///
    /// We can exactly represent a float in radix `b` from radix 2 if
    /// `b` is divisible by 2. This function calculates the exact number of
    /// digits required to exactly represent that float.
    ///
    /// According to the "Handbook of Floating Point Arithmetic",
    /// for IEEE754, with emin being the min exponent, p2 being the
    /// precision, and b being the radix, the number of digits follows as:
    ///
    /// `−emin + p2 + ⌊(emin + 1) log(2, b) − log(1 − 2^(−p2), b)⌋`
    ///
    /// For f32, this follows as:
    ///     emin = -126
    ///     p2 = 24
    ///
    /// For f64, this follows as:
    ///     emin = -1022
    ///     p2 = 53
    ///
    /// In Python:
    ///     `-emin + p2 + math.floor((emin+1)*math.log(2, b) - math.log(1-2**(-p2), b))`
    ///
    /// This was used to calculate the maximum number of digits for [2, 36].
    const MAX_DIGITS: usize;

    // MASKS

    /// Bitmask for the exponent, including the hidden bit.
    const EXPONENT_MASK: Self::Unsigned;
    /// Bitmask for the hidden bit in exponent, which is an implicit 1 in the fraction.
    const HIDDEN_BIT_MASK: Self::Unsigned;
    /// Bitmask for the mantissa (fraction), excluding the hidden bit.
    const MANTISSA_MASK: Self::Unsigned;

    // PROPERTIES

    /// Positive infinity as bits.
    const INFINITY_BITS: Self::Unsigned;
    /// Size of the significand (mantissa) without hidden bit.
    const MANTISSA_SIZE: i32;
    /// Bias of the exponent
    const EXPONENT_BIAS: i32;
    /// Exponent portion of a denormal float.
    const DENORMAL_EXPONENT: i32;
    /// Maximum exponent value in float.
    const MAX_EXPONENT: i32;

    // ROUNDING

    /// Default number of bits to shift (or 64 - mantissa size - 1).
    const DEFAULT_SHIFT: i32;
    /// Mask to determine if a full-carry occurred (1 in bit above hidden bit).
    const CARRY_MASK: u64;

    /// Get min and max exponent limits (exact) from radix.
    fn exponent_limit() -> (i32, i32);

    /// Get the number of digits that can be shifted from exponent to mantissa.
    fn mantissa_limit() -> i32;

    // Re-exported methods from std.
    fn pow10(self, n: i32) -> Self;
    fn from_bits(u: Self::Unsigned) -> Self;
    fn to_bits(self) -> Self::Unsigned;
    fn is_sign_positive(self) -> bool;

    /// Returns true if the float is a denormal.
    #[inline]
    fn is_denormal(self) -> bool {
        self.to_bits() & Self::EXPONENT_MASK == Self::Unsigned::ZERO
    }

    /// Returns true if the float is a NaN or Infinite.
    #[inline]
    fn is_special(self) -> bool {
        self.to_bits() & Self::EXPONENT_MASK == Self::EXPONENT_MASK
    }

    /// Returns true if the float is infinite.
    #[inline]
    fn is_inf(self) -> bool {
        self.is_special() && (self.to_bits() & Self::MANTISSA_MASK) == Self::Unsigned::ZERO
    }

    /// Get exponent component from the float.
    #[inline]
    fn exponent(self) -> i32 {
        if self.is_denormal() {
            return Self::DENORMAL_EXPONENT;
        }

        let bits = self.to_bits();
        let biased_e = ((bits & Self::EXPONENT_MASK) >> Self::MANTISSA_SIZE).as_u32();
        biased_e as i32 - Self::EXPONENT_BIAS
    }

    /// Get mantissa (significand) component from float.
    #[inline]
    fn mantissa(self) -> Self::Unsigned {
        let bits = self.to_bits();
        let s = bits & Self::MANTISSA_MASK;
        if !self.is_denormal() {
            s + Self::HIDDEN_BIT_MASK
        } else {
            s
        }
    }

    /// Get next greater float for a positive float.
    /// Value must be >= 0.0 and < INFINITY.
    #[inline]
    fn next_positive(self) -> Self {
        debug_assert!(self.is_sign_positive() && !self.is_inf());
        Self::from_bits(self.to_bits() + Self::Unsigned::as_cast(1u32))
    }

    /// Round a positive number to even.
    #[inline]
    fn round_positive_even(self) -> Self {
        if self.mantissa() & Self::Unsigned::as_cast(1u32) == Self::Unsigned::as_cast(1u32) {
            self.next_positive()
        } else {
            self
        }
    }
}

impl Float for f32 {
    type Unsigned = u32;

    const ZERO: f32 = 0.0;
    const MAX_DIGITS: usize = 114;
    const EXPONENT_MASK: u32 = 0x7F800000;
    const HIDDEN_BIT_MASK: u32 = 0x00800000;
    const MANTISSA_MASK: u32 = 0x007FFFFF;
    const INFINITY_BITS: u32 = 0x7F800000;
    const MANTISSA_SIZE: i32 = 23;
    const EXPONENT_BIAS: i32 = 127 + Self::MANTISSA_SIZE;
    const DENORMAL_EXPONENT: i32 = 1 - Self::EXPONENT_BIAS;
    const MAX_EXPONENT: i32 = 0xFF - Self::EXPONENT_BIAS;
    const DEFAULT_SHIFT: i32 = u64::FULL - f32::MANTISSA_SIZE - 1;
    const CARRY_MASK: u64 = 0x1000000;

    #[inline]
    fn exponent_limit() -> (i32, i32) {
        (-10, 10)
    }

    #[inline]
    fn mantissa_limit() -> i32 {
        7
    }

    #[inline]
    fn pow10(self, n: i32) -> f32 {
        // Check the exponent is within bounds in debug builds.
        debug_assert!({
            let (min, max) = Self::exponent_limit();
            n >= min && n <= max
        });

        if n > 0 {
            self * F32_POW10[n as usize]
        } else {
            self / F32_POW10[-n as usize]
        }
    }

    #[inline]
    fn from_bits(u: u32) -> f32 {
        f32::from_bits(u)
    }

    #[inline]
    fn to_bits(self) -> u32 {
        f32::to_bits(self)
    }

    #[inline]
    fn is_sign_positive(self) -> bool {
        f32::is_sign_positive(self)
    }
}

impl Float for f64 {
    type Unsigned = u64;

    const ZERO: f64 = 0.0;
    const MAX_DIGITS: usize = 769;
    const EXPONENT_MASK: u64 = 0x7FF0000000000000;
    const HIDDEN_BIT_MASK: u64 = 0x0010000000000000;
    const MANTISSA_MASK: u64 = 0x000FFFFFFFFFFFFF;
    const INFINITY_BITS: u64 = 0x7FF0000000000000;
    const MANTISSA_SIZE: i32 = 52;
    const EXPONENT_BIAS: i32 = 1023 + Self::MANTISSA_SIZE;
    const DENORMAL_EXPONENT: i32 = 1 - Self::EXPONENT_BIAS;
    const MAX_EXPONENT: i32 = 0x7FF - Self::EXPONENT_BIAS;
    const DEFAULT_SHIFT: i32 = u64::FULL - f64::MANTISSA_SIZE - 1;
    const CARRY_MASK: u64 = 0x20000000000000;

    #[inline]
    fn exponent_limit() -> (i32, i32) {
        (-22, 22)
    }

    #[inline]
    fn mantissa_limit() -> i32 {
        15
    }

    #[inline]
    fn pow10(self, n: i32) -> f64 {
        // Check the exponent is within bounds in debug builds.
        debug_assert!({
            let (min, max) = Self::exponent_limit();
            n >= min && n <= max
        });

        if n > 0 {
            self * F64_POW10[n as usize]
        } else {
            self / F64_POW10[-n as usize]
        }
    }

    #[inline]
    fn from_bits(u: u64) -> f64 {
        f64::from_bits(u)
    }

    #[inline]
    fn to_bits(self) -> u64 {
        f64::to_bits(self)
    }

    #[inline]
    fn is_sign_positive(self) -> bool {
        f64::is_sign_positive(self)
    }
}
