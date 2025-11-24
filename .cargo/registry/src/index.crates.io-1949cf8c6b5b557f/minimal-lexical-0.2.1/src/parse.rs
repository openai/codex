//! Parse byte iterators to float.

#![doc(hidden)]

#[cfg(feature = "compact")]
use crate::bellerophon::bellerophon;
use crate::extended_float::{extended_to_float, ExtendedFloat};
#[cfg(not(feature = "compact"))]
use crate::lemire::lemire;
use crate::num::Float;
use crate::number::Number;
use crate::slow::slow;

/// Try to parse the significant digits quickly.
///
/// This attempts a very quick parse, to deal with common cases.
///
/// * `integer`     - Slice containing the integer digits.
/// * `fraction`    - Slice containing the fraction digits.
#[inline]
fn parse_number_fast<'a, Iter1, Iter2>(
    integer: Iter1,
    fraction: Iter2,
    exponent: i32,
) -> Option<Number>
where
    Iter1: Iterator<Item = &'a u8>,
    Iter2: Iterator<Item = &'a u8>,
{
    let mut num = Number::default();
    let mut integer_count: usize = 0;
    let mut fraction_count: usize = 0;
    for &c in integer {
        integer_count += 1;
        let digit = c - b'0';
        num.mantissa = num.mantissa.wrapping_mul(10).wrapping_add(digit as u64);
    }
    for &c in fraction {
        fraction_count += 1;
        let digit = c - b'0';
        num.mantissa = num.mantissa.wrapping_mul(10).wrapping_add(digit as u64);
    }

    if integer_count + fraction_count <= 19 {
        // Can't overflow, since must be <= 19.
        num.exponent = exponent.saturating_sub(fraction_count as i32);
        Some(num)
    } else {
        None
    }
}

/// Parse the significant digits of the float and adjust the exponent.
///
/// * `integer`     - Slice containing the integer digits.
/// * `fraction`    - Slice containing the fraction digits.
#[inline]
fn parse_number<'a, Iter1, Iter2>(mut integer: Iter1, mut fraction: Iter2, exponent: i32) -> Number
where
    Iter1: Iterator<Item = &'a u8> + Clone,
    Iter2: Iterator<Item = &'a u8> + Clone,
{
    // NOTE: for performance, we do this in 2 passes:
    if let Some(num) = parse_number_fast(integer.clone(), fraction.clone(), exponent) {
        return num;
    }

    // Can only add 19 digits.
    let mut num = Number::default();
    let mut count = 0;
    while let Some(&c) = integer.next() {
        count += 1;
        if count == 20 {
            // Only the integer digits affect the exponent.
            num.many_digits = true;
            num.exponent = exponent.saturating_add(into_i32(1 + integer.count()));
            return num;
        } else {
            let digit = c - b'0';
            num.mantissa = num.mantissa * 10 + digit as u64;
        }
    }

    // Skip leading fraction zeros.
    // This is required otherwise we might have a 0 mantissa and many digits.
    let mut fraction_count: usize = 0;
    if count == 0 {
        for &c in &mut fraction {
            fraction_count += 1;
            if c != b'0' {
                count += 1;
                let digit = c - b'0';
                num.mantissa = num.mantissa * 10 + digit as u64;
                break;
            }
        }
    }
    for c in fraction {
        fraction_count += 1;
        count += 1;
        if count == 20 {
            num.many_digits = true;
            // This can't wrap, since we have at most 20 digits.
            // We've adjusted the exponent too high by `fraction_count - 1`.
            // Note: -1 is due to incrementing this loop iteration, which we
            // didn't use.
            num.exponent = exponent.saturating_sub(fraction_count as i32 - 1);
            return num;
        } else {
            let digit = c - b'0';
            num.mantissa = num.mantissa * 10 + digit as u64;
        }
    }

    // No truncated digits: easy.
    // Cannot overflow: <= 20 digits.
    num.exponent = exponent.saturating_sub(fraction_count as i32);
    num
}

/// Parse float from extracted float components.
///
/// * `integer`     - Cloneable, forward iterator over integer digits.
/// * `fraction`    - Cloneable, forward iterator over integer digits.
/// * `exponent`    - Parsed, 32-bit exponent.
///
/// # Preconditions
/// 1. The integer should not have leading zeros.
/// 2. The fraction should not have trailing zeros.
/// 3. All bytes in `integer` and `fraction` should be valid digits,
///     in the range [`b'0', b'9'].
///
/// # Panics
///
/// Although passing garbage input will not cause memory safety issues,
/// it is very likely to cause a panic with a large number of digits, or
/// in debug mode. The big-integer arithmetic without the `alloc` feature
/// assumes a maximum, fixed-width input, which assumes at maximum a
/// value of `10^(769 + 342)`, or ~4000 bits of storage. Passing in
/// nonsensical digits may require up to ~6000 bits of storage, which will
/// panic when attempting to add it to the big integer. It is therefore
/// up to the caller to validate this input.
///
/// We cannot efficiently remove trailing zeros while only accepting a
/// forward iterator.
pub fn parse_float<'a, F, Iter1, Iter2>(integer: Iter1, fraction: Iter2, exponent: i32) -> F
where
    F: Float,
    Iter1: Iterator<Item = &'a u8> + Clone,
    Iter2: Iterator<Item = &'a u8> + Clone,
{
    // Parse the mantissa and attempt the fast and moderate-path algorithms.
    let num = parse_number(integer.clone(), fraction.clone(), exponent);
    // Try the fast-path algorithm.
    if let Some(value) = num.try_fast_path() {
        return value;
    }

    // Now try the moderate path algorithm.
    let mut fp = moderate_path::<F>(&num);
    if fp.exp < 0 {
        // Undo the invalid extended float biasing.
        fp.exp -= F::INVALID_FP;
        fp = slow::<F, _, _>(num, fp, integer, fraction);
    }

    // Unable to correctly round the float using the fast or moderate algorithms.
    // Fallback to a slower, but always correct algorithm. If we have
    // lossy, we can't be here.
    extended_to_float::<F>(fp)
}

/// Wrapper for different moderate-path algorithms.
/// A return exponent of `-1` indicates an invalid value.
#[inline]
pub fn moderate_path<F: Float>(num: &Number) -> ExtendedFloat {
    #[cfg(not(feature = "compact"))]
    return lemire::<F>(num);

    #[cfg(feature = "compact")]
    return bellerophon::<F>(num);
}

/// Convert usize into i32 without overflow.
///
/// This is needed to ensure when adjusting the exponent relative to
/// the mantissa we do not overflow for comically-long exponents.
#[inline]
fn into_i32(value: usize) -> i32 {
    if value > i32::max_value() as usize {
        i32::max_value()
    } else {
        value as i32
    }
}

// Add digit to mantissa.
#[inline]
pub fn add_digit(value: u64, digit: u8) -> Option<u64> {
    value.checked_mul(10)?.checked_add(digit as u64)
}
