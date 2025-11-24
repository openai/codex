//! An implementation of Clinger's Bellerophon algorithm.
//!
//! This is a moderate path algorithm that uses an extended-precision
//! float, represented in 80 bits, by calculating the bits of slop
//! and determining if those bits could prevent unambiguous rounding.
//!
//! This algorithm requires less static storage than the Lemire algorithm,
//! and has decent performance, and is therefore used when non-decimal,
//! non-power-of-two strings need to be parsed. Clinger's algorithm
//! is described in depth in "How to Read Floating Point Numbers Accurately.",
//! available online [here](http://citeseerx.ist.psu.edu/viewdoc/download?doi=10.1.1.45.4152&rep=rep1&type=pdf).
//!
//! This implementation is loosely based off the Golang implementation,
//! found [here](https://github.com/golang/go/blob/b10849fbb97a2244c086991b4623ae9f32c212d0/src/strconv/extfloat.go).
//! This code is therefore subject to a 3-clause BSD license.

#![cfg(feature = "compact")]
#![doc(hidden)]

use crate::extended_float::ExtendedFloat;
use crate::mask::{lower_n_halfway, lower_n_mask};
use crate::num::Float;
use crate::number::Number;
use crate::rounding::{round, round_nearest_tie_even};
use crate::table::BASE10_POWERS;

// ALGORITHM
// ---------

/// Core implementation of the Bellerophon algorithm.
///
/// Create an extended-precision float, scale it to the proper radix power,
/// calculate the bits of slop, and return the representation. The value
/// will always be guaranteed to be within 1 bit, rounded-down, of the real
/// value. If a negative exponent is returned, this represents we were
/// unable to unambiguously round the significant digits.
///
/// This has been modified to return a biased, rather than unbiased exponent.
pub fn bellerophon<F: Float>(num: &Number) -> ExtendedFloat {
    let fp_zero = ExtendedFloat {
        mant: 0,
        exp: 0,
    };
    let fp_inf = ExtendedFloat {
        mant: 0,
        exp: F::INFINITE_POWER,
    };

    // Early short-circuit, in case of literal 0 or infinity.
    // This allows us to avoid narrow casts causing numeric overflow,
    // and is a quick check for any radix.
    if num.mantissa == 0 || num.exponent <= -0x1000 {
        return fp_zero;
    } else if num.exponent >= 0x1000 {
        return fp_inf;
    }

    // Calculate our indexes for our extended-precision multiplication.
    // This narrowing cast is safe, since exponent must be in a valid range.
    let exponent = num.exponent as i32 + BASE10_POWERS.bias;
    let small_index = exponent % BASE10_POWERS.step;
    let large_index = exponent / BASE10_POWERS.step;

    if exponent < 0 {
        // Guaranteed underflow (assign 0).
        return fp_zero;
    }
    if large_index as usize >= BASE10_POWERS.large.len() {
        // Overflow (assign infinity)
        return fp_inf;
    }

    // Within the valid exponent range, multiply by the large and small
    // exponents and return the resulting value.

    // Track errors to as a factor of unit in last-precision.
    let mut errors: u32 = 0;
    if num.many_digits {
        errors += error_halfscale();
    }

    // Multiply by the small power.
    // Check if we can directly multiply by an integer, if not,
    // use extended-precision multiplication.
    let mut fp = ExtendedFloat {
        mant: num.mantissa,
        exp: 0,
    };
    match fp.mant.overflowing_mul(BASE10_POWERS.get_small_int(small_index as usize)) {
        // Overflow, multiplication unsuccessful, go slow path.
        (_, true) => {
            normalize(&mut fp);
            fp = mul(&fp, &BASE10_POWERS.get_small(small_index as usize));
            errors += error_halfscale();
        },
        // No overflow, multiplication successful.
        (mant, false) => {
            fp.mant = mant;
            normalize(&mut fp);
        },
    }

    // Multiply by the large power.
    fp = mul(&fp, &BASE10_POWERS.get_large(large_index as usize));
    if errors > 0 {
        errors += 1;
    }
    errors += error_halfscale();

    // Normalize the floating point (and the errors).
    let shift = normalize(&mut fp);
    errors <<= shift;
    fp.exp += F::EXPONENT_BIAS;

    // Check for literal overflow, even with halfway cases.
    if -fp.exp + 1 > 65 {
        return fp_zero;
    }

    // Too many errors accumulated, return an error.
    if !error_is_accurate::<F>(errors, &fp) {
        // Bias the exponent so we know it's invalid.
        fp.exp += F::INVALID_FP;
        return fp;
    }

    // Check if we have a literal 0 or overflow here.
    // If we have an exponent of -63, we can still have a valid shift,
    // giving a case where we have too many errors and need to round-up.
    if -fp.exp + 1 == 65 {
        // Have more than 64 bits below the minimum exponent, must be 0.
        return fp_zero;
    }

    round::<F, _>(&mut fp, |f, s| {
        round_nearest_tie_even(f, s, |is_odd, is_halfway, is_above| {
            is_above || (is_odd && is_halfway)
        });
    });
    fp
}

// ERRORS
// ------

// Calculate if the errors in calculating the extended-precision float.
//
// Specifically, we want to know if we are close to a halfway representation,
// or halfway between `b` and `b+1`, or `b+h`. The halfway representation
// has the form:
//     SEEEEEEEHMMMMMMMMMMMMMMMMMMMMMMM100...
// where:
//     S = Sign Bit
//     E = Exponent Bits
//     H = Hidden Bit
//     M = Mantissa Bits
//
// The halfway representation has a bit set 1-after the mantissa digits,
// and no bits set immediately afterward, making it impossible to
// round between `b` and `b+1` with this representation.

/// Get the full error scale.
#[inline(always)]
const fn error_scale() -> u32 {
    8
}

/// Get the half error scale.
#[inline(always)]
const fn error_halfscale() -> u32 {
    error_scale() / 2
}

/// Determine if the number of errors is tolerable for float precision.
fn error_is_accurate<F: Float>(errors: u32, fp: &ExtendedFloat) -> bool {
    // Check we can't have a literal 0 denormal float.
    debug_assert!(fp.exp >= -64);

    // Determine if extended-precision float is a good approximation.
    // If the error has affected too many units, the float will be
    // inaccurate, or if the representation is too close to halfway
    // that any operations could affect this halfway representation.
    // See the documentation for dtoa for more information.

    // This is always a valid u32, since `fp.exp >= -64`
    // will always be positive and the significand size is {23, 52}.
    let mantissa_shift = 64 - F::MANTISSA_SIZE - 1;

    // The unbiased exponent checks is `unbiased_exp <= F::MANTISSA_SIZE
    // - F::EXPONENT_BIAS -64 + 1`, or `biased_exp <= F::MANTISSA_SIZE - 63`,
    // or `biased_exp <= mantissa_shift`.
    let extrabits = match fp.exp <= -mantissa_shift {
        // Denormal, since shifting to the hidden bit still has a negative exponent.
        // The unbiased check calculation for bits is `1 - F::EXPONENT_BIAS - unbiased_exp`,
        // or `1 - biased_exp`.
        true => 1 - fp.exp,
        false => 64 - F::MANTISSA_SIZE - 1,
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
    let maskbits = extrabits as u64;
    let errors = errors as u64;

    // Round-to-nearest, need to use the halfway point.
    if extrabits > 64 {
        // Underflow, we have a shift larger than the mantissa.
        // Representation is valid **only** if the value is close enough
        // overflow to the next bit within errors. If it overflows,
        // the representation is **not** valid.
        !fp.mant.overflowing_add(errors).1
    } else {
        let mask = lower_n_mask(maskbits);
        let extra = fp.mant & mask;

        // Round-to-nearest, need to check if we're close to halfway.
        // IE, b10100 | 100000, where `|` signifies the truncation point.
        let halfway = lower_n_halfway(maskbits);
        let cmp1 = halfway.wrapping_sub(errors) < extra;
        let cmp2 = extra < halfway.wrapping_add(errors);

        // If both comparisons are true, we have significant rounding error,
        // and the value cannot be exactly represented. Otherwise, the
        // representation is valid.
        !(cmp1 && cmp2)
    }
}

// MATH
// ----

/// Normalize float-point number.
///
/// Shift the mantissa so the number of leading zeros is 0, or the value
/// itself is 0.
///
/// Get the number of bytes shifted.
pub fn normalize(fp: &mut ExtendedFloat) -> i32 {
    // Note:
    // Using the ctlz intrinsic via leading_zeros is way faster (~10x)
    // than shifting 1-bit at a time, via while loop, and also way
    // faster (~2x) than an unrolled loop that checks at 32, 16, 4,
    // 2, and 1 bit.
    //
    // Using a modulus of pow2 (which will get optimized to a bitwise
    // and with 0x3F or faster) is slightly slower than an if/then,
    // however, removing the if/then will likely optimize more branched
    // code as it removes conditional logic.

    // Calculate the number of leading zeros, and then zero-out
    // any overflowing bits, to avoid shl overflow when self.mant == 0.
    if fp.mant != 0 {
        let shift = fp.mant.leading_zeros() as i32;
        fp.mant <<= shift;
        fp.exp -= shift;
        shift
    } else {
        0
    }
}

/// Multiply two normalized extended-precision floats, as if by `a*b`.
///
/// The precision is maximal when the numbers are normalized, however,
/// decent precision will occur as long as both values have high bits
/// set. The result is not normalized.
///
/// Algorithm:
///     1. Non-signed multiplication of mantissas (requires 2x as many bits as input).
///     2. Normalization of the result (not done here).
///     3. Addition of exponents.
pub fn mul(x: &ExtendedFloat, y: &ExtendedFloat) -> ExtendedFloat {
    // Logic check, values must be decently normalized prior to multiplication.
    debug_assert!(x.mant >> 32 != 0);
    debug_assert!(y.mant >> 32 != 0);

    // Extract high-and-low masks.
    // Mask is u32::MAX for older Rustc versions.
    const LOMASK: u64 = 0xffff_ffff;
    let x1 = x.mant >> 32;
    let x0 = x.mant & LOMASK;
    let y1 = y.mant >> 32;
    let y0 = y.mant & LOMASK;

    // Get our products
    let x1_y0 = x1 * y0;
    let x0_y1 = x0 * y1;
    let x0_y0 = x0 * y0;
    let x1_y1 = x1 * y1;

    let mut tmp = (x1_y0 & LOMASK) + (x0_y1 & LOMASK) + (x0_y0 >> 32);
    // round up
    tmp += 1 << (32 - 1);

    ExtendedFloat {
        mant: x1_y1 + (x1_y0 >> 32) + (x0_y1 >> 32) + (tmp >> 32),
        exp: x.exp + y.exp + 64,
    }
}

// POWERS
// ------

/// Precalculated powers of base N for the Bellerophon algorithm.
pub struct BellerophonPowers {
    // Pre-calculated small powers.
    pub small: &'static [u64],
    // Pre-calculated large powers.
    pub large: &'static [u64],
    /// Pre-calculated small powers as 64-bit integers
    pub small_int: &'static [u64],
    // Step between large powers and number of small powers.
    pub step: i32,
    // Exponent bias for the large powers.
    pub bias: i32,
    /// ceil(log2(radix)) scaled as a multiplier.
    pub log2: i64,
    /// Bitshift for the log2 multiplier.
    pub log2_shift: i32,
}

/// Allow indexing of values without bounds checking
impl BellerophonPowers {
    #[inline]
    pub fn get_small(&self, index: usize) -> ExtendedFloat {
        let mant = self.small[index];
        let exp = (1 - 64) + ((self.log2 * index as i64) >> self.log2_shift);
        ExtendedFloat {
            mant,
            exp: exp as i32,
        }
    }

    #[inline]
    pub fn get_large(&self, index: usize) -> ExtendedFloat {
        let mant = self.large[index];
        let biased_e = index as i64 * self.step as i64 - self.bias as i64;
        let exp = (1 - 64) + ((self.log2 * biased_e) >> self.log2_shift);
        ExtendedFloat {
            mant,
            exp: exp as i32,
        }
    }

    #[inline]
    pub fn get_small_int(&self, index: usize) -> u64 {
        self.small_int[index]
    }
}
