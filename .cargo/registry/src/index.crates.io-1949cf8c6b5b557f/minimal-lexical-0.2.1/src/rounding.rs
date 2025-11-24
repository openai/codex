//! Defines rounding schemes for floating-point numbers.

#![doc(hidden)]

use crate::extended_float::ExtendedFloat;
use crate::mask::{lower_n_halfway, lower_n_mask};
use crate::num::Float;

// ROUNDING
// --------

/// Round an extended-precision float to the nearest machine float.
///
/// Shifts the significant digits into place, adjusts the exponent,
/// so it can be easily converted to a native float.
#[cfg_attr(not(feature = "compact"), inline)]
pub fn round<F, Cb>(fp: &mut ExtendedFloat, cb: Cb)
where
    F: Float,
    Cb: Fn(&mut ExtendedFloat, i32),
{
    let fp_inf = ExtendedFloat {
        mant: 0,
        exp: F::INFINITE_POWER,
    };

    // Calculate our shift in significant digits.
    let mantissa_shift = 64 - F::MANTISSA_SIZE - 1;

    // Check for a denormal float, if after the shift the exponent is negative.
    if -fp.exp >= mantissa_shift {
        // Have a denormal float that isn't a literal 0.
        // The extra 1 is to adjust for the denormal float, which is
        // `1 - F::EXPONENT_BIAS`. This works as before, because our
        // old logic rounded to `F::DENORMAL_EXPONENT` (now 1), and then
        // checked if `exp == F::DENORMAL_EXPONENT` and no hidden mask
        // bit was set. Here, we handle that here, rather than later.
        //
        // This might round-down to 0, but shift will be at **max** 65,
        // for halfway cases rounding towards 0.
        let shift = -fp.exp + 1;
        debug_assert!(shift <= 65);
        cb(fp, shift.min(64));
        // Check for round-up: if rounding-nearest carried us to the hidden bit.
        fp.exp = (fp.mant >= F::HIDDEN_BIT_MASK) as i32;
        return;
    }

    // The float is normal, round to the hidden bit.
    cb(fp, mantissa_shift);

    // Check if we carried, and if so, shift the bit to the hidden bit.
    let carry_mask = F::CARRY_MASK;
    if fp.mant & carry_mask == carry_mask {
        fp.mant >>= 1;
        fp.exp += 1;
    }

    // Handle if we carried and check for overflow again.
    if fp.exp >= F::INFINITE_POWER {
        // Exponent is above largest normal value, must be infinite.
        *fp = fp_inf;
        return;
    }

    // Remove the hidden bit.
    fp.mant &= F::MANTISSA_MASK;
}

/// Shift right N-bytes and round towards a direction.
///
/// Callback should take the following parameters:
///     1. is_odd
///     1. is_halfway
///     1. is_above
#[cfg_attr(not(feature = "compact"), inline)]
pub fn round_nearest_tie_even<Cb>(fp: &mut ExtendedFloat, shift: i32, cb: Cb)
where
    // is_odd, is_halfway, is_above
    Cb: Fn(bool, bool, bool) -> bool,
{
    // Ensure we've already handled denormal values that underflow.
    debug_assert!(shift <= 64);

    // Extract the truncated bits using mask.
    // Calculate if the value of the truncated bits are either above
    // the mid-way point, or equal to it.
    //
    // For example, for 4 truncated bytes, the mask would be 0b1111
    // and the midway point would be 0b1000.
    let mask = lower_n_mask(shift as u64);
    let halfway = lower_n_halfway(shift as u64);
    let truncated_bits = fp.mant & mask;
    let is_above = truncated_bits > halfway;
    let is_halfway = truncated_bits == halfway;

    // Bit shift so the leading bit is in the hidden bit.
    // This optimixes pretty well:
    //  ```text
    //   mov     ecx, esi
    //   shr     rdi, cl
    //   xor     eax, eax
    //   cmp     esi, 64
    //   cmovne  rax, rdi
    //   ret
    //  ```
    fp.mant = match shift == 64 {
        true => 0,
        false => fp.mant >> shift,
    };
    fp.exp += shift;

    // Extract the last bit after shifting (and determine if it is odd).
    let is_odd = fp.mant & 1 == 1;

    // Calculate if we need to roundup.
    // We need to roundup if we are above halfway, or if we are odd
    // and at half-way (need to tie-to-even). Avoid the branch here.
    fp.mant += cb(is_odd, is_halfway, is_above) as u64;
}

/// Round our significant digits into place, truncating them.
#[cfg_attr(not(feature = "compact"), inline)]
pub fn round_down(fp: &mut ExtendedFloat, shift: i32) {
    // Might have a shift greater than 64 if we have an error.
    fp.mant = match shift == 64 {
        true => 0,
        false => fp.mant >> shift,
    };
    fp.exp += shift;
}
