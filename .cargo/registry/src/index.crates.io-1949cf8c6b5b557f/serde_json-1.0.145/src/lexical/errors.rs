// Adapted from https://github.com/Alexhuszagh/rust-lexical.

//! Estimate the error in an 80-bit approximation of a float.
//!
//! This estimates the error in a floating-point representation.
//!
//! This implementation is loosely based off the Golang implementation,
//! found here: <https://golang.org/src/strconv/atof.go>

use super::float::*;
use super::num::*;
use super::rounding::*;

pub(crate) trait FloatErrors {
    /// Get the full error scale.
    fn error_scale() -> u32;
    /// Get the half error scale.
    fn error_halfscale() -> u32;
    /// Determine if the number of errors is tolerable for float precision.
    fn error_is_accurate<F: Float>(count: u32, fp: &ExtendedFloat) -> bool;
}

/// Check if the error is accurate with a round-nearest rounding scheme.
#[inline]
fn nearest_error_is_accurate(errors: u64, fp: &ExtendedFloat, extrabits: u64) -> bool {
    // Round-to-nearest, need to use the halfway point.
    if extrabits == 65 {
        // Underflow, we have a shift larger than the mantissa.
        // Representation is valid **only** if the value is close enough
        // overflow to the next bit within errors. If it overflows,
        // the representation is **not** valid.
        !fp.mant.overflowing_add(errors).1
    } else {
        let mask: u64 = lower_n_mask(extrabits);
        let extra: u64 = fp.mant & mask;

        // Round-to-nearest, need to check if we're close to halfway.
        // IE, b10100 | 100000, where `|` signifies the truncation point.
        let halfway: u64 = lower_n_halfway(extrabits);
        let cmp1 = halfway.wrapping_sub(errors) < extra;
        let cmp2 = extra < halfway.wrapping_add(errors);

        // If both comparisons are true, we have significant rounding error,
        // and the value cannot be exactly represented. Otherwise, the
        // representation is valid.
        !(cmp1 && cmp2)
    }
}

impl FloatErrors for u64 {
    #[inline]
    fn error_scale() -> u32 {
        8
    }

    #[inline]
    fn error_halfscale() -> u32 {
        u64::error_scale() / 2
    }

    #[inline]
    fn error_is_accurate<F: Float>(count: u32, fp: &ExtendedFloat) -> bool {
        // Determine if extended-precision float is a good approximation.
        // If the error has affected too many units, the float will be
        // inaccurate, or if the representation is too close to halfway
        // that any operations could affect this halfway representation.
        // See the documentation for dtoa for more information.
        let bias = -(F::EXPONENT_BIAS - F::MANTISSA_SIZE);
        let denormal_exp = bias - 63;
        // This is always a valid u32, since (denormal_exp - fp.exp)
        // will always be positive and the significand size is {23, 52}.
        let extrabits = if fp.exp <= denormal_exp {
            64 - F::MANTISSA_SIZE + denormal_exp - fp.exp
        } else {
            63 - F::MANTISSA_SIZE
        };

        // Our logic is as follows: we want to determine if the actual
        // mantissa and the errors during calculation differ significantly
        // from the rounding point. The rounding point for round-nearest
        // is the halfway point, IE, this when the truncated bits start
        // with b1000..., while the rounding point for the round-toward
        // is when the truncated bits are equal to 0.
        // To do so, we can check whether the rounding point +/- the error
        // are >/< the actual lower n bits.
        //
        // For whether we need to use signed or unsigned types for this
        // analysis, see this example, using u8 rather than u64 to simplify
        // things.
        //
        // # Comparisons
        //      cmp1 = (halfway - errors) < extra
        //      cmp1 = extra < (halfway + errors)
        //
        // # Large Extrabits, Low Errors
        //
        //      extrabits = 8
        //      halfway          =  0b10000000
        //      extra            =  0b10000010
        //      errors           =  0b00000100
        //      halfway - errors =  0b01111100
        //      halfway + errors =  0b10000100
        //
        //      Unsigned:
        //          halfway - errors = 124
        //          halfway + errors = 132
        //          extra            = 130
        //          cmp1             = true
        //          cmp2             = true
        //      Signed:
        //          halfway - errors = 124
        //          halfway + errors = -124
        //          extra            = -126
        //          cmp1             = false
        //          cmp2             = true
        //
        // # Conclusion
        //
        // Since errors will always be small, and since we want to detect
        // if the representation is accurate, we need to use an **unsigned**
        // type for comparisons.

        let extrabits = extrabits as u64;
        let errors = count as u64;
        if extrabits > 65 {
            // Underflow, we have a literal 0.
            return true;
        }

        nearest_error_is_accurate(errors, fp, extrabits)
    }
}
