// Adapted from https://github.com/Alexhuszagh/rust-lexical.

//! Algorithms to efficiently convert strings to floats.

use super::bhcomp::*;
use super::cached::*;
use super::errors::*;
use super::float::ExtendedFloat;
use super::num::*;
use super::small_powers::*;

// FAST
// ----

/// Convert mantissa to exact value for a non-base2 power.
///
/// Returns the resulting float and if the value can be represented exactly.
pub(crate) fn fast_path<F>(mantissa: u64, exponent: i32) -> Option<F>
where
    F: Float,
{
    // `mantissa >> (F::MANTISSA_SIZE+1) != 0` effectively checks if the
    // value has a no bits above the hidden bit, which is what we want.
    let (min_exp, max_exp) = F::exponent_limit();
    let shift_exp = F::mantissa_limit();
    let mantissa_size = F::MANTISSA_SIZE + 1;
    if mantissa == 0 {
        Some(F::ZERO)
    } else if mantissa >> mantissa_size != 0 {
        // Would require truncation of the mantissa.
        None
    } else if exponent == 0 {
        // 0 exponent, same as value, exact representation.
        let float = F::as_cast(mantissa);
        Some(float)
    } else if exponent >= min_exp && exponent <= max_exp {
        // Value can be exactly represented, return the value.
        // Do not use powi, since powi can incrementally introduce
        // error.
        let float = F::as_cast(mantissa);
        Some(float.pow10(exponent))
    } else if exponent >= 0 && exponent <= max_exp + shift_exp {
        // Check to see if we have a disguised fast-path, where the
        // number of digits in the mantissa is very small, but and
        // so digits can be shifted from the exponent to the mantissa.
        // https://www.exploringbinary.com/fast-path-decimal-to-floating-point-conversion/
        let small_powers = POW10_64;
        let shift = exponent - max_exp;
        let power = small_powers[shift as usize];

        // Compute the product of the power, if it overflows,
        // prematurely return early, otherwise, if we didn't overshoot,
        // we can get an exact value.
        let value = match mantissa.checked_mul(power) {
            None => return None,
            Some(value) => value,
        };
        if value >> mantissa_size != 0 {
            None
        } else {
            // Use powi, since it's correct, and faster on
            // the fast-path.
            let float = F::as_cast(value);
            Some(float.pow10(max_exp))
        }
    } else {
        // Cannot be exactly represented, exponent too small or too big,
        // would require truncation.
        None
    }
}

// MODERATE
// --------

/// Multiply the floating-point by the exponent.
///
/// Multiply by pre-calculated powers of the base, modify the extended-
/// float, and return if new value and if the value can be represented
/// accurately.
fn multiply_exponent_extended<F>(fp: &mut ExtendedFloat, exponent: i32, truncated: bool) -> bool
where
    F: Float,
{
    let powers = ExtendedFloat::get_powers();
    let exponent = exponent.saturating_add(powers.bias);
    let small_index = exponent % powers.step;
    let large_index = exponent / powers.step;
    if exponent < 0 {
        // Guaranteed underflow (assign 0).
        fp.mant = 0;
        true
    } else if large_index as usize >= powers.large.len() {
        // Overflow (assign infinity)
        fp.mant = 1 << 63;
        fp.exp = 0x7FF;
        true
    } else {
        // Within the valid exponent range, multiply by the large and small
        // exponents and return the resulting value.

        // Track errors to as a factor of unit in last-precision.
        let mut errors: u32 = 0;
        if truncated {
            errors += u64::error_halfscale();
        }

        // Multiply by the small power.
        // Check if we can directly multiply by an integer, if not,
        // use extended-precision multiplication.
        match fp
            .mant
            .overflowing_mul(powers.get_small_int(small_index as usize))
        {
            // Overflow, multiplication unsuccessful, go slow path.
            (_, true) => {
                fp.normalize();
                fp.imul(&powers.get_small(small_index as usize));
                errors += u64::error_halfscale();
            }
            // No overflow, multiplication successful.
            (mant, false) => {
                fp.mant = mant;
                fp.normalize();
            }
        }

        // Multiply by the large power
        fp.imul(&powers.get_large(large_index as usize));
        if errors > 0 {
            errors += 1;
        }
        errors += u64::error_halfscale();

        // Normalize the floating point (and the errors).
        let shift = fp.normalize();
        errors <<= shift;

        u64::error_is_accurate::<F>(errors, fp)
    }
}

/// Create a precise native float using an intermediate extended-precision float.
///
/// Return the float approximation and if the value can be accurately
/// represented with mantissa bits of precision.
#[inline]
pub(crate) fn moderate_path<F>(
    mantissa: u64,
    exponent: i32,
    truncated: bool,
) -> (ExtendedFloat, bool)
where
    F: Float,
{
    let mut fp = ExtendedFloat {
        mant: mantissa,
        exp: 0,
    };
    let valid = multiply_exponent_extended::<F>(&mut fp, exponent, truncated);
    (fp, valid)
}

// FALLBACK
// --------

/// Fallback path when the fast path does not work.
///
/// Uses the moderate path, if applicable, otherwise, uses the slow path
/// as required.
pub(crate) fn fallback_path<F>(
    integer: &[u8],
    fraction: &[u8],
    mantissa: u64,
    exponent: i32,
    mantissa_exponent: i32,
    truncated: bool,
) -> F
where
    F: Float,
{
    // Moderate path (use an extended 80-bit representation).
    let (fp, valid) = moderate_path::<F>(mantissa, mantissa_exponent, truncated);
    if valid {
        return fp.into_float::<F>();
    }

    // Slow path, fast path didn't work.
    let b = fp.into_downward_float::<F>();
    if b.is_special() {
        // We have a non-finite number, we get to leave early.
        b
    } else {
        bhcomp(b, integer, fraction, exponent)
    }
}
