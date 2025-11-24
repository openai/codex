//! Implementation of the Eisel-Lemire algorithm.
//!
//! This is adapted from [fast-float-rust](https://github.com/aldanor/fast-float-rust),
//! a port of [fast_float](https://github.com/fastfloat/fast_float) to Rust.

#![cfg(not(feature = "compact"))]
#![doc(hidden)]

use crate::extended_float::ExtendedFloat;
use crate::num::Float;
use crate::number::Number;
use crate::table::{LARGEST_POWER_OF_FIVE, POWER_OF_FIVE_128, SMALLEST_POWER_OF_FIVE};

/// Ensure truncation of digits doesn't affect our computation, by doing 2 passes.
#[inline]
pub fn lemire<F: Float>(num: &Number) -> ExtendedFloat {
    // If significant digits were truncated, then we can have rounding error
    // only if `mantissa + 1` produces a different result. We also avoid
    // redundantly using the Eisel-Lemire algorithm if it was unable to
    // correctly round on the first pass.
    let mut fp = compute_float::<F>(num.exponent, num.mantissa);
    if num.many_digits && fp.exp >= 0 && fp != compute_float::<F>(num.exponent, num.mantissa + 1) {
        // Need to re-calculate, since the previous values are rounded
        // when the slow path algorithm expects a normalized extended float.
        fp = compute_error::<F>(num.exponent, num.mantissa);
    }
    fp
}

/// Compute a float using an extended-precision representation.
///
/// Fast conversion of a the significant digits and decimal exponent
/// a float to a extended representation with a binary float. This
/// algorithm will accurately parse the vast majority of cases,
/// and uses a 128-bit representation (with a fallback 192-bit
/// representation).
///
/// This algorithm scales the exponent by the decimal exponent
/// using pre-computed powers-of-5, and calculates if the
/// representation can be unambiguously rounded to the nearest
/// machine float. Near-halfway cases are not handled here,
/// and are represented by a negative, biased binary exponent.
///
/// The algorithm is described in detail in "Daniel Lemire, Number Parsing
/// at a Gigabyte per Second" in section 5, "Fast Algorithm", and
/// section 6, "Exact Numbers And Ties", available online:
/// <https://arxiv.org/abs/2101.11408.pdf>.
pub fn compute_float<F: Float>(q: i32, mut w: u64) -> ExtendedFloat {
    let fp_zero = ExtendedFloat {
        mant: 0,
        exp: 0,
    };
    let fp_inf = ExtendedFloat {
        mant: 0,
        exp: F::INFINITE_POWER,
    };

    // Short-circuit if the value can only be a literal 0 or infinity.
    if w == 0 || q < F::SMALLEST_POWER_OF_TEN {
        return fp_zero;
    } else if q > F::LARGEST_POWER_OF_TEN {
        return fp_inf;
    }
    // Normalize our significant digits, so the most-significant bit is set.
    let lz = w.leading_zeros() as i32;
    w <<= lz;
    let (lo, hi) = compute_product_approx(q, w, F::MANTISSA_SIZE as usize + 3);
    if lo == 0xFFFF_FFFF_FFFF_FFFF {
        // If we have failed to approximate w x 5^-q with our 128-bit value.
        // Since the addition of 1 could lead to an overflow which could then
        // round up over the half-way point, this can lead to improper rounding
        // of a float.
        //
        // However, this can only occur if q ∈ [-27, 55]. The upper bound of q
        // is 55 because 5^55 < 2^128, however, this can only happen if 5^q > 2^64,
        // since otherwise the product can be represented in 64-bits, producing
        // an exact result. For negative exponents, rounding-to-even can
        // only occur if 5^-q < 2^64.
        //
        // For detailed explanations of rounding for negative exponents, see
        // <https://arxiv.org/pdf/2101.11408.pdf#section.9.1>. For detailed
        // explanations of rounding for positive exponents, see
        // <https://arxiv.org/pdf/2101.11408.pdf#section.8>.
        let inside_safe_exponent = (q >= -27) && (q <= 55);
        if !inside_safe_exponent {
            return compute_error_scaled::<F>(q, hi, lz);
        }
    }
    let upperbit = (hi >> 63) as i32;
    let mut mantissa = hi >> (upperbit + 64 - F::MANTISSA_SIZE - 3);
    let mut power2 = power(q) + upperbit - lz - F::MINIMUM_EXPONENT;
    if power2 <= 0 {
        if -power2 + 1 >= 64 {
            // Have more than 64 bits below the minimum exponent, must be 0.
            return fp_zero;
        }
        // Have a subnormal value.
        mantissa >>= -power2 + 1;
        mantissa += mantissa & 1;
        mantissa >>= 1;
        power2 = (mantissa >= (1_u64 << F::MANTISSA_SIZE)) as i32;
        return ExtendedFloat {
            mant: mantissa,
            exp: power2,
        };
    }
    // Need to handle rounding ties. Normally, we need to round up,
    // but if we fall right in between and and we have an even basis, we
    // need to round down.
    //
    // This will only occur if:
    //  1. The lower 64 bits of the 128-bit representation is 0.
    //      IE, 5^q fits in single 64-bit word.
    //  2. The least-significant bit prior to truncated mantissa is odd.
    //  3. All the bits truncated when shifting to mantissa bits + 1 are 0.
    //
    // Or, we may fall between two floats: we are exactly halfway.
    if lo <= 1
        && q >= F::MIN_EXPONENT_ROUND_TO_EVEN
        && q <= F::MAX_EXPONENT_ROUND_TO_EVEN
        && mantissa & 3 == 1
        && (mantissa << (upperbit + 64 - F::MANTISSA_SIZE - 3)) == hi
    {
        // Zero the lowest bit, so we don't round up.
        mantissa &= !1_u64;
    }
    // Round-to-even, then shift the significant digits into place.
    mantissa += mantissa & 1;
    mantissa >>= 1;
    if mantissa >= (2_u64 << F::MANTISSA_SIZE) {
        // Rounding up overflowed, so the carry bit is set. Set the
        // mantissa to 1 (only the implicit, hidden bit is set) and
        // increase the exponent.
        mantissa = 1_u64 << F::MANTISSA_SIZE;
        power2 += 1;
    }
    // Zero out the hidden bit.
    mantissa &= !(1_u64 << F::MANTISSA_SIZE);
    if power2 >= F::INFINITE_POWER {
        // Exponent is above largest normal value, must be infinite.
        return fp_inf;
    }
    ExtendedFloat {
        mant: mantissa,
        exp: power2,
    }
}

/// Fallback algorithm to calculate the non-rounded representation.
/// This calculates the extended representation, and then normalizes
/// the resulting representation, so the high bit is set.
#[inline]
pub fn compute_error<F: Float>(q: i32, mut w: u64) -> ExtendedFloat {
    let lz = w.leading_zeros() as i32;
    w <<= lz;
    let hi = compute_product_approx(q, w, F::MANTISSA_SIZE as usize + 3).1;
    compute_error_scaled::<F>(q, hi, lz)
}

/// Compute the error from a mantissa scaled to the exponent.
#[inline]
pub fn compute_error_scaled<F: Float>(q: i32, mut w: u64, lz: i32) -> ExtendedFloat {
    // Want to normalize the float, but this is faster than ctlz on most architectures.
    let hilz = (w >> 63) as i32 ^ 1;
    w <<= hilz;
    let power2 = power(q as i32) + F::EXPONENT_BIAS - hilz - lz - 62;

    ExtendedFloat {
        mant: w,
        exp: power2 + F::INVALID_FP,
    }
}

/// Calculate a base 2 exponent from a decimal exponent.
/// This uses a pre-computed integer approximation for
/// log2(10), where 217706 / 2^16 is accurate for the
/// entire range of non-finite decimal exponents.
#[inline]
fn power(q: i32) -> i32 {
    (q.wrapping_mul(152_170 + 65536) >> 16) + 63
}

#[inline]
fn full_multiplication(a: u64, b: u64) -> (u64, u64) {
    let r = (a as u128) * (b as u128);
    (r as u64, (r >> 64) as u64)
}

// This will compute or rather approximate w * 5**q and return a pair of 64-bit words
// approximating the result, with the "high" part corresponding to the most significant
// bits and the low part corresponding to the least significant bits.
fn compute_product_approx(q: i32, w: u64, precision: usize) -> (u64, u64) {
    debug_assert!(q >= SMALLEST_POWER_OF_FIVE);
    debug_assert!(q <= LARGEST_POWER_OF_FIVE);
    debug_assert!(precision <= 64);

    let mask = if precision < 64 {
        0xFFFF_FFFF_FFFF_FFFF_u64 >> precision
    } else {
        0xFFFF_FFFF_FFFF_FFFF_u64
    };

    // 5^q < 2^64, then the multiplication always provides an exact value.
    // That means whenever we need to round ties to even, we always have
    // an exact value.
    let index = (q - SMALLEST_POWER_OF_FIVE) as usize;
    let (lo5, hi5) = POWER_OF_FIVE_128[index];
    // Only need one multiplication as long as there is 1 zero but
    // in the explicit mantissa bits, +1 for the hidden bit, +1 to
    // determine the rounding direction, +1 for if the computed
    // product has a leading zero.
    let (mut first_lo, mut first_hi) = full_multiplication(w, lo5);
    if first_hi & mask == mask {
        // Need to do a second multiplication to get better precision
        // for the lower product. This will always be exact
        // where q is < 55, since 5^55 < 2^128. If this wraps,
        // then we need to need to round up the hi product.
        let (_, second_hi) = full_multiplication(w, hi5);
        first_lo = first_lo.wrapping_add(second_hi);
        if second_hi > first_lo {
            first_hi += 1;
        }
    }
    (first_lo, first_hi)
}
