// Adapted from https://github.com/Alexhuszagh/rust-lexical.

//! Utilities to calculate exponents.

/// Convert usize into i32 without overflow.
///
/// This is needed to ensure when adjusting the exponent relative to
/// the mantissa we do not overflow for comically-long exponents.
#[inline]
fn into_i32(value: usize) -> i32 {
    if value > i32::MAX as usize {
        i32::MAX
    } else {
        value as i32
    }
}

// EXPONENT CALCULATION

// Calculate the scientific notation exponent without overflow.
//
// For example, 0.1 would be -1, and 10 would be 1 in base 10.
#[inline]
pub(crate) fn scientific_exponent(
    exponent: i32,
    integer_digits: usize,
    fraction_start: usize,
) -> i32 {
    if integer_digits == 0 {
        let fraction_start = into_i32(fraction_start);
        exponent.saturating_sub(fraction_start).saturating_sub(1)
    } else {
        let integer_shift = into_i32(integer_digits - 1);
        exponent.saturating_add(integer_shift)
    }
}

// Calculate the mantissa exponent without overflow.
//
// Remove the number of digits that contributed to the mantissa past
// the dot, and add the number of truncated digits from the mantissa,
// to calculate the scaling factor for the mantissa from a raw exponent.
#[inline]
pub(crate) fn mantissa_exponent(exponent: i32, fraction_digits: usize, truncated: usize) -> i32 {
    if fraction_digits > truncated {
        exponent.saturating_sub(into_i32(fraction_digits - truncated))
    } else {
        exponent.saturating_add(into_i32(truncated - fraction_digits))
    }
}
