// Adapted from https://github.com/Alexhuszagh/rust-lexical.

//! Defines rounding schemes for floating-point numbers.

use super::float::ExtendedFloat;
use super::num::*;
use super::shift::*;
use core::mem;

// MASKS

/// Calculate a scalar factor of 2 above the halfway point.
#[inline]
pub(crate) fn nth_bit(n: u64) -> u64 {
    let bits: u64 = mem::size_of::<u64>() as u64 * 8;
    debug_assert!(n < bits, "nth_bit() overflow in shl.");

    1 << n
}

/// Generate a bitwise mask for the lower `n` bits.
#[inline]
pub(crate) fn lower_n_mask(n: u64) -> u64 {
    let bits: u64 = mem::size_of::<u64>() as u64 * 8;
    debug_assert!(n <= bits, "lower_n_mask() overflow in shl.");

    if n == bits {
        u64::MAX
    } else {
        (1 << n) - 1
    }
}

/// Calculate the halfway point for the lower `n` bits.
#[inline]
pub(crate) fn lower_n_halfway(n: u64) -> u64 {
    let bits: u64 = mem::size_of::<u64>() as u64 * 8;
    debug_assert!(n <= bits, "lower_n_halfway() overflow in shl.");

    if n == 0 {
        0
    } else {
        nth_bit(n - 1)
    }
}

/// Calculate a bitwise mask with `n` 1 bits starting at the `bit` position.
#[inline]
pub(crate) fn internal_n_mask(bit: u64, n: u64) -> u64 {
    let bits: u64 = mem::size_of::<u64>() as u64 * 8;
    debug_assert!(bit <= bits, "internal_n_halfway() overflow in shl.");
    debug_assert!(n <= bits, "internal_n_halfway() overflow in shl.");
    debug_assert!(bit >= n, "internal_n_halfway() overflow in sub.");

    lower_n_mask(bit) ^ lower_n_mask(bit - n)
}

// NEAREST ROUNDING

// Shift right N-bytes and round to the nearest.
//
// Return if we are above halfway and if we are halfway.
#[inline]
pub(crate) fn round_nearest(fp: &mut ExtendedFloat, shift: i32) -> (bool, bool) {
    // Extract the truncated bits using mask.
    // Calculate if the value of the truncated bits are either above
    // the mid-way point, or equal to it.
    //
    // For example, for 4 truncated bytes, the mask would be b1111
    // and the midway point would be b1000.
    let mask: u64 = lower_n_mask(shift as u64);
    let halfway: u64 = lower_n_halfway(shift as u64);

    let truncated_bits = fp.mant & mask;
    let is_above = truncated_bits > halfway;
    let is_halfway = truncated_bits == halfway;

    // Bit shift so the leading bit is in the hidden bit.
    overflowing_shr(fp, shift);

    (is_above, is_halfway)
}

// Tie rounded floating point to event.
#[inline]
pub(crate) fn tie_even(fp: &mut ExtendedFloat, is_above: bool, is_halfway: bool) {
    // Extract the last bit after shifting (and determine if it is odd).
    let is_odd = fp.mant & 1 == 1;

    // Calculate if we need to roundup.
    // We need to roundup if we are above halfway, or if we are odd
    // and at half-way (need to tie-to-even).
    if is_above || (is_odd && is_halfway) {
        fp.mant += 1;
    }
}

// Shift right N-bytes and round nearest, tie-to-even.
//
// Floating-point arithmetic uses round to nearest, ties to even,
// which rounds to the nearest value, if the value is halfway in between,
// round to an even value.
#[inline]
pub(crate) fn round_nearest_tie_even(fp: &mut ExtendedFloat, shift: i32) {
    let (is_above, is_halfway) = round_nearest(fp, shift);
    tie_even(fp, is_above, is_halfway);
}

// DIRECTED ROUNDING

// Shift right N-bytes and round towards a direction.
//
// Return if we have any truncated bytes.
#[inline]
fn round_toward(fp: &mut ExtendedFloat, shift: i32) -> bool {
    let mask: u64 = lower_n_mask(shift as u64);
    let truncated_bits = fp.mant & mask;

    // Bit shift so the leading bit is in the hidden bit.
    overflowing_shr(fp, shift);

    truncated_bits != 0
}

// Round down.
#[inline]
fn downard(_: &mut ExtendedFloat, _: bool) {}

// Shift right N-bytes and round toward zero.
//
// Floating-point arithmetic defines round toward zero, which rounds
// towards positive zero.
#[inline]
pub(crate) fn round_downward(fp: &mut ExtendedFloat, shift: i32) {
    // Bit shift so the leading bit is in the hidden bit.
    // No rounding schemes, so we just ignore everything else.
    let is_truncated = round_toward(fp, shift);
    downard(fp, is_truncated);
}

// ROUND TO FLOAT

// Shift the ExtendedFloat fraction to the fraction bits in a native float.
//
// Floating-point arithmetic uses round to nearest, ties to even,
// which rounds to the nearest value, if the value is halfway in between,
// round to an even value.
#[inline]
pub(crate) fn round_to_float<F, Algorithm>(fp: &mut ExtendedFloat, algorithm: Algorithm)
where
    F: Float,
    Algorithm: FnOnce(&mut ExtendedFloat, i32),
{
    // Calculate the difference to allow a single calculation
    // rather than a loop, to minimize the number of ops required.
    // This does underflow detection.
    let final_exp = fp.exp + F::DEFAULT_SHIFT;
    if final_exp < F::DENORMAL_EXPONENT {
        // We would end up with a denormal exponent, try to round to more
        // digits. Only shift right if we can avoid zeroing out the value,
        // which requires the exponent diff to be < M::BITS. The value
        // is already normalized, so we shouldn't have any issue zeroing
        // out the value.
        let diff = F::DENORMAL_EXPONENT - fp.exp;
        if diff <= u64::FULL {
            // We can avoid underflow, can get a valid representation.
            algorithm(fp, diff);
        } else {
            // Certain underflow, assign literal 0s.
            fp.mant = 0;
            fp.exp = 0;
        }
    } else {
        algorithm(fp, F::DEFAULT_SHIFT);
    }

    if fp.mant & F::CARRY_MASK == F::CARRY_MASK {
        // Roundup carried over to 1 past the hidden bit.
        shr(fp, 1);
    }
}

// AVOID OVERFLOW/UNDERFLOW

// Avoid overflow for large values, shift left as needed.
//
// Shift until a 1-bit is in the hidden bit, if the mantissa is not 0.
#[inline]
pub(crate) fn avoid_overflow<F>(fp: &mut ExtendedFloat)
where
    F: Float,
{
    // Calculate the difference to allow a single calculation
    // rather than a loop, minimizing the number of ops required.
    if fp.exp >= F::MAX_EXPONENT {
        let diff = fp.exp - F::MAX_EXPONENT;
        if diff <= F::MANTISSA_SIZE {
            // Our overflow mask needs to start at the hidden bit, or at
            // `F::MANTISSA_SIZE+1`, and needs to have `diff+1` bits set,
            // to see if our value overflows.
            let bit = (F::MANTISSA_SIZE + 1) as u64;
            let n = (diff + 1) as u64;
            let mask = internal_n_mask(bit, n);
            if (fp.mant & mask) == 0 {
                // If we have no 1-bit in the hidden-bit position,
                // which is index 0, we need to shift 1.
                let shift = diff + 1;
                shl(fp, shift);
            }
        }
    }
}

// ROUND TO NATIVE

// Round an extended-precision float to a native float representation.
#[inline]
pub(crate) fn round_to_native<F, Algorithm>(fp: &mut ExtendedFloat, algorithm: Algorithm)
where
    F: Float,
    Algorithm: FnOnce(&mut ExtendedFloat, i32),
{
    // Shift all the way left, to ensure a consistent representation.
    // The following right-shifts do not work for a non-normalized number.
    fp.normalize();

    // Round so the fraction is in a native mantissa representation,
    // and avoid overflow/underflow.
    round_to_float::<F, _>(fp, algorithm);
    avoid_overflow::<F>(fp);
}
