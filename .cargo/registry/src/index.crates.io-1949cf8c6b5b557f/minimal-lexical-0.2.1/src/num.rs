//! Utilities for Rust numbers.

#![doc(hidden)]

#[cfg(all(not(feature = "std"), feature = "compact"))]
use crate::libm::{powd, powf};
#[cfg(not(feature = "compact"))]
use crate::table::{SMALL_F32_POW10, SMALL_F64_POW10, SMALL_INT_POW10, SMALL_INT_POW5};
#[cfg(not(feature = "compact"))]
use core::hint;
use core::ops;

/// Generic floating-point type, to be used in generic code for parsing.
///
/// Although the trait is part of the public API, the trait provides methods
/// and constants that are effectively non-public: they may be removed
/// at any time without any breaking changes.
pub trait Float:
    Sized
    + Copy
    + PartialEq
    + PartialOrd
    + Send
    + Sync
    + ops::Add<Output = Self>
    + ops::AddAssign
    + ops::Div<Output = Self>
    + ops::DivAssign
    + ops::Mul<Output = Self>
    + ops::MulAssign
    + ops::Rem<Output = Self>
    + ops::RemAssign
    + ops::Sub<Output = Self>
    + ops::SubAssign
    + ops::Neg<Output = Self>
{
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

    /// Bitmask for the sign bit.
    const SIGN_MASK: u64;
    /// Bitmask for the exponent, including the hidden bit.
    const EXPONENT_MASK: u64;
    /// Bitmask for the hidden bit in exponent, which is an implicit 1 in the fraction.
    const HIDDEN_BIT_MASK: u64;
    /// Bitmask for the mantissa (fraction), excluding the hidden bit.
    const MANTISSA_MASK: u64;

    // PROPERTIES

    /// Size of the significand (mantissa) without hidden bit.
    const MANTISSA_SIZE: i32;
    /// Bias of the exponet
    const EXPONENT_BIAS: i32;
    /// Exponent portion of a denormal float.
    const DENORMAL_EXPONENT: i32;
    /// Maximum exponent value in float.
    const MAX_EXPONENT: i32;

    // ROUNDING

    /// Mask to determine if a full-carry occurred (1 in bit above hidden bit).
    const CARRY_MASK: u64;

    /// Bias for marking an invalid extended float.
    // Value is `i16::MIN`, using hard-coded constants for older Rustc versions.
    const INVALID_FP: i32 = -0x8000;

    // Maximum mantissa for the fast-path (`1 << 53` for f64).
    const MAX_MANTISSA_FAST_PATH: u64 = 2_u64 << Self::MANTISSA_SIZE;

    // Largest exponent value `(1 << EXP_BITS) - 1`.
    const INFINITE_POWER: i32 = Self::MAX_EXPONENT + Self::EXPONENT_BIAS;

    // Round-to-even only happens for negative values of q
    // when q ≥ −4 in the 64-bit case and when q ≥ −17 in
    // the 32-bitcase.
    //
    // When q ≥ 0,we have that 5^q ≤ 2m+1. In the 64-bit case,we
    // have 5^q ≤ 2m+1 ≤ 2^54 or q ≤ 23. In the 32-bit case,we have
    // 5^q ≤ 2m+1 ≤ 2^25 or q ≤ 10.
    //
    // When q < 0, we have w ≥ (2m+1)×5^−q. We must have that w < 2^64
    // so (2m+1)×5^−q < 2^64. We have that 2m+1 > 2^53 (64-bit case)
    // or 2m+1 > 2^24 (32-bit case). Hence,we must have 2^53×5^−q < 2^64
    // (64-bit) and 2^24×5^−q < 2^64 (32-bit). Hence we have 5^−q < 2^11
    // or q ≥ −4 (64-bit case) and 5^−q < 2^40 or q ≥ −17 (32-bitcase).
    //
    // Thus we have that we only need to round ties to even when
    // we have that q ∈ [−4,23](in the 64-bit case) or q∈[−17,10]
    // (in the 32-bit case). In both cases,the power of five(5^|q|)
    // fits in a 64-bit word.
    const MIN_EXPONENT_ROUND_TO_EVEN: i32;
    const MAX_EXPONENT_ROUND_TO_EVEN: i32;

    /// Minimum normal exponent value `-(1 << (EXPONENT_SIZE - 1)) + 1`.
    const MINIMUM_EXPONENT: i32;

    /// Smallest decimal exponent for a non-zero value.
    const SMALLEST_POWER_OF_TEN: i32;

    /// Largest decimal exponent for a non-infinite value.
    const LARGEST_POWER_OF_TEN: i32;

    /// Minimum exponent that for a fast path case, or `-⌊(MANTISSA_SIZE+1)/log2(10)⌋`
    const MIN_EXPONENT_FAST_PATH: i32;

    /// Maximum exponent that for a fast path case, or `⌊(MANTISSA_SIZE+1)/log2(5)⌋`
    const MAX_EXPONENT_FAST_PATH: i32;

    /// Maximum exponent that can be represented for a disguised-fast path case.
    /// This is `MAX_EXPONENT_FAST_PATH + ⌊(MANTISSA_SIZE+1)/log2(10)⌋`
    const MAX_EXPONENT_DISGUISED_FAST_PATH: i32;

    /// Convert 64-bit integer to float.
    fn from_u64(u: u64) -> Self;

    // Re-exported methods from std.
    fn from_bits(u: u64) -> Self;
    fn to_bits(self) -> u64;

    /// Get a small power-of-radix for fast-path multiplication.
    ///
    /// # Safety
    ///
    /// Safe as long as the exponent is smaller than the table size.
    unsafe fn pow_fast_path(exponent: usize) -> Self;

    /// Get a small, integral power-of-radix for fast-path multiplication.
    ///
    /// # Safety
    ///
    /// Safe as long as the exponent is smaller than the table size.
    #[inline(always)]
    unsafe fn int_pow_fast_path(exponent: usize, radix: u32) -> u64 {
        // SAFETY: safe as long as the exponent is smaller than the radix table.
        #[cfg(not(feature = "compact"))]
        return match radix {
            5 => unsafe { *SMALL_INT_POW5.get_unchecked(exponent) },
            10 => unsafe { *SMALL_INT_POW10.get_unchecked(exponent) },
            _ => unsafe { hint::unreachable_unchecked() },
        };

        #[cfg(feature = "compact")]
        return (radix as u64).pow(exponent as u32);
    }

    /// Returns true if the float is a denormal.
    #[inline]
    fn is_denormal(self) -> bool {
        self.to_bits() & Self::EXPONENT_MASK == 0
    }

    /// Get exponent component from the float.
    #[inline]
    fn exponent(self) -> i32 {
        if self.is_denormal() {
            return Self::DENORMAL_EXPONENT;
        }

        let bits = self.to_bits();
        let biased_e: i32 = ((bits & Self::EXPONENT_MASK) >> Self::MANTISSA_SIZE) as i32;
        biased_e - Self::EXPONENT_BIAS
    }

    /// Get mantissa (significand) component from float.
    #[inline]
    fn mantissa(self) -> u64 {
        let bits = self.to_bits();
        let s = bits & Self::MANTISSA_MASK;
        if !self.is_denormal() {
            s + Self::HIDDEN_BIT_MASK
        } else {
            s
        }
    }
}

impl Float for f32 {
    const MAX_DIGITS: usize = 114;
    const SIGN_MASK: u64 = 0x80000000;
    const EXPONENT_MASK: u64 = 0x7F800000;
    const HIDDEN_BIT_MASK: u64 = 0x00800000;
    const MANTISSA_MASK: u64 = 0x007FFFFF;
    const MANTISSA_SIZE: i32 = 23;
    const EXPONENT_BIAS: i32 = 127 + Self::MANTISSA_SIZE;
    const DENORMAL_EXPONENT: i32 = 1 - Self::EXPONENT_BIAS;
    const MAX_EXPONENT: i32 = 0xFF - Self::EXPONENT_BIAS;
    const CARRY_MASK: u64 = 0x1000000;
    const MIN_EXPONENT_ROUND_TO_EVEN: i32 = -17;
    const MAX_EXPONENT_ROUND_TO_EVEN: i32 = 10;
    const MINIMUM_EXPONENT: i32 = -127;
    const SMALLEST_POWER_OF_TEN: i32 = -65;
    const LARGEST_POWER_OF_TEN: i32 = 38;
    const MIN_EXPONENT_FAST_PATH: i32 = -10;
    const MAX_EXPONENT_FAST_PATH: i32 = 10;
    const MAX_EXPONENT_DISGUISED_FAST_PATH: i32 = 17;

    #[inline(always)]
    unsafe fn pow_fast_path(exponent: usize) -> Self {
        // SAFETY: safe as long as the exponent is smaller than the radix table.
        #[cfg(not(feature = "compact"))]
        return unsafe { *SMALL_F32_POW10.get_unchecked(exponent) };

        #[cfg(feature = "compact")]
        return powf(10.0f32, exponent as f32);
    }

    #[inline]
    fn from_u64(u: u64) -> f32 {
        u as _
    }

    #[inline]
    fn from_bits(u: u64) -> f32 {
        // Constant is `u32::MAX` for older Rustc versions.
        debug_assert!(u <= 0xffff_ffff);
        f32::from_bits(u as u32)
    }

    #[inline]
    fn to_bits(self) -> u64 {
        f32::to_bits(self) as u64
    }
}

impl Float for f64 {
    const MAX_DIGITS: usize = 769;
    const SIGN_MASK: u64 = 0x8000000000000000;
    const EXPONENT_MASK: u64 = 0x7FF0000000000000;
    const HIDDEN_BIT_MASK: u64 = 0x0010000000000000;
    const MANTISSA_MASK: u64 = 0x000FFFFFFFFFFFFF;
    const MANTISSA_SIZE: i32 = 52;
    const EXPONENT_BIAS: i32 = 1023 + Self::MANTISSA_SIZE;
    const DENORMAL_EXPONENT: i32 = 1 - Self::EXPONENT_BIAS;
    const MAX_EXPONENT: i32 = 0x7FF - Self::EXPONENT_BIAS;
    const CARRY_MASK: u64 = 0x20000000000000;
    const MIN_EXPONENT_ROUND_TO_EVEN: i32 = -4;
    const MAX_EXPONENT_ROUND_TO_EVEN: i32 = 23;
    const MINIMUM_EXPONENT: i32 = -1023;
    const SMALLEST_POWER_OF_TEN: i32 = -342;
    const LARGEST_POWER_OF_TEN: i32 = 308;
    const MIN_EXPONENT_FAST_PATH: i32 = -22;
    const MAX_EXPONENT_FAST_PATH: i32 = 22;
    const MAX_EXPONENT_DISGUISED_FAST_PATH: i32 = 37;

    #[inline(always)]
    unsafe fn pow_fast_path(exponent: usize) -> Self {
        // SAFETY: safe as long as the exponent is smaller than the radix table.
        #[cfg(not(feature = "compact"))]
        return unsafe { *SMALL_F64_POW10.get_unchecked(exponent) };

        #[cfg(feature = "compact")]
        return powd(10.0f64, exponent as f64);
    }

    #[inline]
    fn from_u64(u: u64) -> f64 {
        u as _
    }

    #[inline]
    fn from_bits(u: u64) -> f64 {
        f64::from_bits(u)
    }

    #[inline]
    fn to_bits(self) -> u64 {
        f64::to_bits(self)
    }
}

#[inline(always)]
#[cfg(all(feature = "std", feature = "compact"))]
pub fn powf(x: f32, y: f32) -> f32 {
    x.powf(y)
}

#[inline(always)]
#[cfg(all(feature = "std", feature = "compact"))]
pub fn powd(x: f64, y: f64) -> f64 {
    x.powf(y)
}
