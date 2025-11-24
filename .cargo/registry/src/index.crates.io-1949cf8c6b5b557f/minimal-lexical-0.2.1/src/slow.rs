//! Slow, fallback cases where we cannot unambiguously round a float.
//!
//! This occurs when we cannot determine the exact representation using
//! both the fast path (native) cases nor the Lemire/Bellerophon algorithms,
//! and therefore must fallback to a slow, arbitrary-precision representation.

#![doc(hidden)]

use crate::bigint::{Bigint, Limb, LIMB_BITS};
use crate::extended_float::{extended_to_float, ExtendedFloat};
use crate::num::Float;
use crate::number::Number;
use crate::rounding::{round, round_down, round_nearest_tie_even};
use core::cmp;

// ALGORITHM
// ---------

/// Parse the significant digits and biased, binary exponent of a float.
///
/// This is a fallback algorithm that uses a big-integer representation
/// of the float, and therefore is considerably slower than faster
/// approximations. However, it will always determine how to round
/// the significant digits to the nearest machine float, allowing
/// use to handle near half-way cases.
///
/// Near half-way cases are halfway between two consecutive machine floats.
/// For example, the float `16777217.0` has a bitwise representation of
/// `100000000000000000000000 1`. Rounding to a single-precision float,
/// the trailing `1` is truncated. Using round-nearest, tie-even, any
/// value above `16777217.0` must be rounded up to `16777218.0`, while
/// any value before or equal to `16777217.0` must be rounded down
/// to `16777216.0`. These near-halfway conversions therefore may require
/// a large number of digits to unambiguously determine how to round.
#[inline]
pub fn slow<'a, F, Iter1, Iter2>(
    num: Number,
    fp: ExtendedFloat,
    integer: Iter1,
    fraction: Iter2,
) -> ExtendedFloat
where
    F: Float,
    Iter1: Iterator<Item = &'a u8> + Clone,
    Iter2: Iterator<Item = &'a u8> + Clone,
{
    // Ensure our preconditions are valid:
    //  1. The significant digits are not shifted into place.
    debug_assert!(fp.mant & (1 << 63) != 0);

    // This assumes the sign bit has already been parsed, and we're
    // starting with the integer digits, and the float format has been
    // correctly validated.
    let sci_exp = scientific_exponent(&num);

    // We have 2 major algorithms we use for this:
    //  1. An algorithm with a finite number of digits and a positive exponent.
    //  2. An algorithm with a finite number of digits and a negative exponent.
    let (bigmant, digits) = parse_mantissa(integer, fraction, F::MAX_DIGITS);
    let exponent = sci_exp + 1 - digits as i32;
    if exponent >= 0 {
        positive_digit_comp::<F>(bigmant, exponent)
    } else {
        negative_digit_comp::<F>(bigmant, fp, exponent)
    }
}

/// Generate the significant digits with a positive exponent relative to mantissa.
pub fn positive_digit_comp<F: Float>(mut bigmant: Bigint, exponent: i32) -> ExtendedFloat {
    // Simple, we just need to multiply by the power of the radix.
    // Now, we can calculate the mantissa and the exponent from this.
    // The binary exponent is the binary exponent for the mantissa
    // shifted to the hidden bit.
    bigmant.pow(10, exponent as u32).unwrap();

    // Get the exact representation of the float from the big integer.
    // hi64 checks **all** the remaining bits after the mantissa,
    // so it will check if **any** truncated digits exist.
    let (mant, is_truncated) = bigmant.hi64();
    let exp = bigmant.bit_length() as i32 - 64 + F::EXPONENT_BIAS;
    let mut fp = ExtendedFloat {
        mant,
        exp,
    };

    // Shift the digits into position and determine if we need to round-up.
    round::<F, _>(&mut fp, |f, s| {
        round_nearest_tie_even(f, s, |is_odd, is_halfway, is_above| {
            is_above || (is_halfway && is_truncated) || (is_odd && is_halfway)
        });
    });
    fp
}

/// Generate the significant digits with a negative exponent relative to mantissa.
///
/// This algorithm is quite simple: we have the significant digits `m1 * b^N1`,
/// where `m1` is the bigint mantissa, `b` is the radix, and `N1` is the radix
/// exponent. We then calculate the theoretical representation of `b+h`, which
/// is `m2 * 2^N2`, where `m2` is the bigint mantissa and `N2` is the binary
/// exponent. If we had infinite, efficient floating precision, this would be
/// equal to `m1 / b^-N1` and then compare it to `m2 * 2^N2`.
///
/// Since we cannot divide and keep precision, we must multiply the other:
/// if we want to do `m1 / b^-N1 >= m2 * 2^N2`, we can do
/// `m1 >= m2 * b^-N1 * 2^N2` Going to the decimal case, we can show and example
/// and simplify this further: `m1 >= m2 * 2^N2 * 10^-N1`. Since we can remove
/// a power-of-two, this is `m1 >= m2 * 2^(N2 - N1) * 5^-N1`. Therefore, if
/// `N2 - N1 > 0`, we need have `m1 >= m2 * 2^(N2 - N1) * 5^-N1`, otherwise,
/// we have `m1 * 2^(N1 - N2) >= m2 * 5^-N1`, where the resulting exponents
/// are all positive.
///
/// This allows us to compare both floats using integers efficiently
/// without any loss of precision.
#[allow(clippy::comparison_chain)]
pub fn negative_digit_comp<F: Float>(
    bigmant: Bigint,
    mut fp: ExtendedFloat,
    exponent: i32,
) -> ExtendedFloat {
    // Ensure our preconditions are valid:
    //  1. The significant digits are not shifted into place.
    debug_assert!(fp.mant & (1 << 63) != 0);

    // Get the significant digits and radix exponent for the real digits.
    let mut real_digits = bigmant;
    let real_exp = exponent;
    debug_assert!(real_exp < 0);

    // Round down our extended-precision float and calculate `b`.
    let mut b = fp;
    round::<F, _>(&mut b, round_down);
    let b = extended_to_float::<F>(b);

    // Get the significant digits and the binary exponent for `b+h`.
    let theor = bh(b);
    let mut theor_digits = Bigint::from_u64(theor.mant);
    let theor_exp = theor.exp;

    // We need to scale the real digits and `b+h` digits to be the same
    // order. We currently have `real_exp`, in `radix`, that needs to be
    // shifted to `theor_digits` (since it is negative), and `theor_exp`
    // to either `theor_digits` or `real_digits` as a power of 2 (since it
    // may be positive or negative). Try to remove as many powers of 2
    // as possible. All values are relative to `theor_digits`, that is,
    // reflect the power you need to multiply `theor_digits` by.
    //
    // Both are on opposite-sides of equation, can factor out a
    // power of two.
    //
    // Example: 10^-10, 2^-10   -> ( 0, 10, 0)
    // Example: 10^-10, 2^-15   -> (-5, 10, 0)
    // Example: 10^-10, 2^-5    -> ( 5, 10, 0)
    // Example: 10^-10, 2^5     -> (15, 10, 0)
    let binary_exp = theor_exp - real_exp;
    let halfradix_exp = -real_exp;
    if halfradix_exp != 0 {
        theor_digits.pow(5, halfradix_exp as u32).unwrap();
    }
    if binary_exp > 0 {
        theor_digits.pow(2, binary_exp as u32).unwrap();
    } else if binary_exp < 0 {
        real_digits.pow(2, (-binary_exp) as u32).unwrap();
    }

    // Compare our theoretical and real digits and round nearest, tie even.
    let ord = real_digits.data.cmp(&theor_digits.data);
    round::<F, _>(&mut fp, |f, s| {
        round_nearest_tie_even(f, s, |is_odd, _, _| {
            // Can ignore `is_halfway` and `is_above`, since those were
            // calculates using less significant digits.
            match ord {
                cmp::Ordering::Greater => true,
                cmp::Ordering::Less => false,
                cmp::Ordering::Equal if is_odd => true,
                cmp::Ordering::Equal => false,
            }
        });
    });
    fp
}

/// Add a digit to the temporary value.
macro_rules! add_digit {
    ($c:ident, $value:ident, $counter:ident, $count:ident) => {{
        let digit = $c - b'0';
        $value *= 10 as Limb;
        $value += digit as Limb;

        // Increment our counters.
        $counter += 1;
        $count += 1;
    }};
}

/// Add a temporary value to our mantissa.
macro_rules! add_temporary {
    // Multiply by the small power and add the native value.
    (@mul $result:ident, $power:expr, $value:expr) => {
        $result.data.mul_small($power).unwrap();
        $result.data.add_small($value).unwrap();
    };

    // # Safety
    //
    // Safe is `counter <= step`, or smaller than the table size.
    ($format:ident, $result:ident, $counter:ident, $value:ident) => {
        if $counter != 0 {
            // SAFETY: safe, since `counter <= step`, or smaller than the table size.
            let small_power = unsafe { f64::int_pow_fast_path($counter, 10) };
            add_temporary!(@mul $result, small_power as Limb, $value);
            $counter = 0;
            $value = 0;
        }
    };

    // Add a temporary where we won't read the counter results internally.
    //
    // # Safety
    //
    // Safe is `counter <= step`, or smaller than the table size.
    (@end $format:ident, $result:ident, $counter:ident, $value:ident) => {
        if $counter != 0 {
            // SAFETY: safe, since `counter <= step`, or smaller than the table size.
            let small_power = unsafe { f64::int_pow_fast_path($counter, 10) };
            add_temporary!(@mul $result, small_power as Limb, $value);
        }
    };

    // Add the maximum native value.
    (@max $format:ident, $result:ident, $counter:ident, $value:ident, $max:ident) => {
        add_temporary!(@mul $result, $max, $value);
        $counter = 0;
        $value = 0;
    };
}

/// Round-up a truncated value.
macro_rules! round_up_truncated {
    ($format:ident, $result:ident, $count:ident) => {{
        // Need to round-up.
        // Can't just add 1, since this can accidentally round-up
        // values to a halfway point, which can cause invalid results.
        add_temporary!(@mul $result, 10, 1);
        $count += 1;
    }};
}

/// Check and round-up the fraction if any non-zero digits exist.
macro_rules! round_up_nonzero {
    ($format:ident, $iter:expr, $result:ident, $count:ident) => {{
        for &digit in $iter {
            if digit != b'0' {
                round_up_truncated!($format, $result, $count);
                return ($result, $count);
            }
        }
    }};
}

/// Parse the full mantissa into a big integer.
///
/// Returns the parsed mantissa and the number of digits in the mantissa.
/// The max digits is the maximum number of digits plus one.
pub fn parse_mantissa<'a, Iter1, Iter2>(
    mut integer: Iter1,
    mut fraction: Iter2,
    max_digits: usize,
) -> (Bigint, usize)
where
    Iter1: Iterator<Item = &'a u8> + Clone,
    Iter2: Iterator<Item = &'a u8> + Clone,
{
    // Iteratively process all the data in the mantissa.
    // We do this via small, intermediate values which once we reach
    // the maximum number of digits we can process without overflow,
    // we add the temporary to the big integer.
    let mut counter: usize = 0;
    let mut count: usize = 0;
    let mut value: Limb = 0;
    let mut result = Bigint::new();

    // Now use our pre-computed small powers iteratively.
    // This is calculated as `⌊log(2^BITS - 1, 10)⌋`.
    let step: usize = if LIMB_BITS == 32 {
        9
    } else {
        19
    };
    let max_native = (10 as Limb).pow(step as u32);

    // Process the integer digits.
    'integer: loop {
        // Parse a digit at a time, until we reach step.
        while counter < step && count < max_digits {
            if let Some(&c) = integer.next() {
                add_digit!(c, value, counter, count);
            } else {
                break 'integer;
            }
        }

        // Check if we've exhausted our max digits.
        if count == max_digits {
            // Need to check if we're truncated, and round-up accordingly.
            // SAFETY: safe since `counter <= step`.
            add_temporary!(@end format, result, counter, value);
            round_up_nonzero!(format, integer, result, count);
            round_up_nonzero!(format, fraction, result, count);
            return (result, count);
        } else {
            // Add our temporary from the loop.
            // SAFETY: safe since `counter <= step`.
            add_temporary!(@max format, result, counter, value, max_native);
        }
    }

    // Skip leading fraction zeros.
    // Required to get an accurate count.
    if count == 0 {
        for &c in &mut fraction {
            if c != b'0' {
                add_digit!(c, value, counter, count);
                break;
            }
        }
    }

    // Process the fraction digits.
    'fraction: loop {
        // Parse a digit at a time, until we reach step.
        while counter < step && count < max_digits {
            if let Some(&c) = fraction.next() {
                add_digit!(c, value, counter, count);
            } else {
                break 'fraction;
            }
        }

        // Check if we've exhausted our max digits.
        if count == max_digits {
            // SAFETY: safe since `counter <= step`.
            add_temporary!(@end format, result, counter, value);
            round_up_nonzero!(format, fraction, result, count);
            return (result, count);
        } else {
            // Add our temporary from the loop.
            // SAFETY: safe since `counter <= step`.
            add_temporary!(@max format, result, counter, value, max_native);
        }
    }

    // We will always have a remainder, as long as we entered the loop
    // once, or counter % step is 0.
    // SAFETY: safe since `counter <= step`.
    add_temporary!(@end format, result, counter, value);

    (result, count)
}

// SCALING
// -------

/// Calculate the scientific exponent from a `Number` value.
/// Any other attempts would require slowdowns for faster algorithms.
#[inline]
pub fn scientific_exponent(num: &Number) -> i32 {
    // Use power reduction to make this faster.
    let mut mantissa = num.mantissa;
    let mut exponent = num.exponent;
    while mantissa >= 10000 {
        mantissa /= 10000;
        exponent += 4;
    }
    while mantissa >= 100 {
        mantissa /= 100;
        exponent += 2;
    }
    while mantissa >= 10 {
        mantissa /= 10;
        exponent += 1;
    }
    exponent as i32
}

/// Calculate `b` from a a representation of `b` as a float.
#[inline]
pub fn b<F: Float>(float: F) -> ExtendedFloat {
    ExtendedFloat {
        mant: float.mantissa(),
        exp: float.exponent(),
    }
}

/// Calculate `b+h` from a a representation of `b` as a float.
#[inline]
pub fn bh<F: Float>(float: F) -> ExtendedFloat {
    let fp = b(float);
    ExtendedFloat {
        mant: (fp.mant << 1) + 1,
        exp: fp.exp - 1,
    }
}
