// Adapted from https://github.com/Alexhuszagh/rust-lexical.

use super::algorithm::*;
use super::bhcomp::*;
use super::digit::*;
use super::exponent::*;
use super::num::*;

// PARSERS
// -------

/// Parse float for which the entire integer and fraction parts fit into a 64
/// bit mantissa.
pub fn parse_concise_float<F>(mantissa: u64, mant_exp: i32) -> F
where
    F: Float,
{
    if let Some(float) = fast_path(mantissa, mant_exp) {
        return float;
    }

    // Moderate path (use an extended 80-bit representation).
    let truncated = false;
    let (fp, valid) = moderate_path::<F>(mantissa, mant_exp, truncated);
    if valid {
        return fp.into_float::<F>();
    }

    let b = fp.into_downward_float::<F>();
    if b.is_special() {
        // We have a non-finite number, we get to leave early.
        return b;
    }

    // Slow path, fast path didn't work.
    let mut buffer = itoa::Buffer::new();
    let integer = buffer.format(mantissa).as_bytes();
    let fraction = &[];
    bhcomp(b, integer, fraction, mant_exp)
}

/// Parse float from extracted float components.
///
/// * `integer`     - Slice containing the integer digits.
/// * `fraction`    - Slice containing the fraction digits.
/// * `exponent`    - Parsed, 32-bit exponent.
///
/// Precondition: The integer must not have leading zeros.
pub fn parse_truncated_float<F>(integer: &[u8], mut fraction: &[u8], exponent: i32) -> F
where
    F: Float,
{
    // Trim trailing zeroes from the fraction part.
    while fraction.last() == Some(&b'0') {
        fraction = &fraction[..fraction.len() - 1];
    }

    // Calculate the number of truncated digits.
    let mut truncated = 0;
    let mut mantissa: u64 = 0;
    let mut iter = integer.iter().chain(fraction);
    for &c in &mut iter {
        mantissa = match add_digit(mantissa, to_digit(c).unwrap()) {
            Some(v) => v,
            None => {
                truncated = 1 + iter.count();
                break;
            }
        };
    }

    let mant_exp = mantissa_exponent(exponent, fraction.len(), truncated);
    let is_truncated = true;

    fallback_path(
        integer,
        fraction,
        mantissa,
        exponent,
        mant_exp,
        is_truncated,
    )
}
