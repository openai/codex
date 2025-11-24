/// Find and parse sign and get remaining bytes.
#[inline]
fn parse_sign<'a>(bytes: &'a [u8]) -> (bool, &'a [u8]) {
    match bytes.get(0) {
        Some(&b'+') => (true, &bytes[1..]),
        Some(&b'-') => (false, &bytes[1..]),
        _ => (true, bytes),
    }
}

// Convert u8 to digit.
#[inline]
fn to_digit(c: u8) -> Option<u32> {
    (c as char).to_digit(10)
}

// Add digit from exponent.
#[inline]
fn add_digit_i32(value: i32, digit: u32) -> Option<i32> {
    return value.checked_mul(10)?.checked_add(digit as i32);
}

// Subtract digit from exponent.
#[inline]
fn sub_digit_i32(value: i32, digit: u32) -> Option<i32> {
    return value.checked_mul(10)?.checked_sub(digit as i32);
}

// Convert character to digit.
#[inline]
fn is_digit(c: u8) -> bool {
    to_digit(c).is_some()
}

// Split buffer at index.
#[inline]
fn split_at_index<'a>(digits: &'a [u8], index: usize) -> (&'a [u8], &'a [u8]) {
    (&digits[..index], &digits[index..])
}

/// Consume until a an invalid digit is found.
///
/// - `digits`      - Slice containing 0 or more digits.
#[inline]
fn consume_digits<'a>(digits: &'a [u8]) -> (&'a [u8], &'a [u8]) {
    // Consume all digits.
    let mut index = 0;
    while index < digits.len() && is_digit(digits[index]) {
        index += 1;
    }
    split_at_index(digits, index)
}

// Trim leading 0s.
#[inline]
fn ltrim_zero<'a>(bytes: &'a [u8]) -> &'a [u8] {
    let count = bytes.iter().take_while(|&&si| si == b'0').count();
    &bytes[count..]
}

// Trim trailing 0s.
#[inline]
fn rtrim_zero<'a>(bytes: &'a [u8]) -> &'a [u8] {
    let count = bytes.iter().rev().take_while(|&&si| si == b'0').count();
    let index = bytes.len() - count;
    &bytes[..index]
}

// PARSERS
// -------

/// Parse the exponent of the float.
///
/// * `exponent`    - Slice containing the exponent digits.
/// * `is_positive` - If the exponent sign is positive.
fn parse_exponent(exponent: &[u8], is_positive: bool) -> i32 {
    // Parse the sign bit or current data.
    let mut value: i32 = 0;
    match is_positive {
        true => {
            for c in exponent {
                value = match add_digit_i32(value, to_digit(*c).unwrap()) {
                    Some(v) => v,
                    None => return i32::max_value(),
                };
            }
        },
        false => {
            for c in exponent {
                value = match sub_digit_i32(value, to_digit(*c).unwrap()) {
                    Some(v) => v,
                    None => return i32::min_value(),
                };
            }
        },
    }

    value
}

pub fn case_insensitive_starts_with<'a, 'b, Iter1, Iter2>(mut x: Iter1, mut y: Iter2) -> bool
where
    Iter1: Iterator<Item = &'a u8>,
    Iter2: Iterator<Item = &'b u8>,
{
    // We use a faster optimization here for ASCII letters, which NaN
    // and infinite strings **must** be. [A-Z] is 0x41-0x5A, while
    // [a-z] is 0x61-0x7A. Therefore, the xor must be 0 or 32 if they
    // are case-insensitive equal, but only if at least 1 of the inputs
    // is an ASCII letter.
    loop {
        let yi = y.next();
        if yi.is_none() {
            return true;
        }
        let yi = *yi.unwrap();
        let is_not_equal = x.next().map_or(true, |&xi| {
            let xor = xi ^ yi;
            xor != 0 && xor != 0x20
        });
        if is_not_equal {
            return false;
        }
    }
}

/// Parse float from input bytes, returning the float and the remaining bytes.
///
/// * `bytes`    - Array of bytes leading with float-data.
pub fn parse_float<'a, F>(bytes: &'a [u8]) -> (F, &'a [u8])
where
    F: minimal_lexical::Float,
{
    let start = bytes;

    // Parse the sign.
    let (is_positive, bytes) = parse_sign(bytes);

    // Check NaN, Inf, Infinity
    if case_insensitive_starts_with(bytes.iter(), b"NaN".iter()) {
        let mut float = F::from_bits(F::EXPONENT_MASK | (F::HIDDEN_BIT_MASK >> 1));
        if !is_positive {
            float = -float;
        }
        return (float, &bytes[3..]);
    } else if case_insensitive_starts_with(bytes.iter(), b"Infinity".iter()) {
        let mut float = F::from_bits(F::EXPONENT_MASK);
        if !is_positive {
            float = -float;
        }
        return (float, &bytes[8..]);
    } else if case_insensitive_starts_with(bytes.iter(), b"inf".iter()) {
        let mut float = F::from_bits(F::EXPONENT_MASK);
        if !is_positive {
            float = -float;
        }
        return (float, &bytes[3..]);
    }

    // Extract and parse the float components:
    //  1. Integer
    //  2. Fraction
    //  3. Exponent
    let (integer_slc, bytes) = consume_digits(bytes);
    let (fraction_slc, bytes) = match bytes.first() {
        Some(&b'.') => consume_digits(&bytes[1..]),
        _ => (&bytes[..0], bytes),
    };
    let (exponent, bytes) = match bytes.first() {
        Some(&b'e') | Some(&b'E') => {
            // Extract and parse the exponent.
            let (is_positive, bytes) = parse_sign(&bytes[1..]);
            let (exponent, bytes) = consume_digits(bytes);
            (parse_exponent(exponent, is_positive), bytes)
        },
        _ => (0, bytes),
    };

    if bytes.len() == start.len() {
        return (F::from_u64(0), bytes);
    }

    // Note: You may want to check and validate the float data here:
    //  1). Many floats require integer or fraction digits, if a fraction
    //      is present.
    //  2). All floats require either integer or fraction digits.
    //  3). Some floats do not allow a '+' sign before the significant digits.
    //  4). Many floats require exponent digits after the exponent symbol.
    //  5). Some floats do not allow a '+' sign before the exponent.

    // We now need to trim leading and trailing 0s from the integer
    // and fraction, respectively. This is required to make the
    // fast and moderate paths more efficient, and for the slow
    // path.
    let integer_slc = ltrim_zero(integer_slc);
    let fraction_slc = rtrim_zero(fraction_slc);

    // Create the float and return our data.
    let mut float: F =
        minimal_lexical::parse_float(integer_slc.iter(), fraction_slc.iter(), exponent);
    if !is_positive {
        float = -float;
    }

    (float, bytes)
}

macro_rules! b {
    ($x:literal) => {
        $x.as_bytes()
    };
}

#[test]
fn f32_test() {
    assert_eq!(
        (184467440000000000000.0, b!("\x00\x00006")),
        parse_float::<f32>(b"000184467440737095516150\x00\x00006")
    );
}

#[test]
fn f64_test() {
    assert_eq!(
        (184467440737095500000.0, b!("\x00\x00006")),
        parse_float::<f64>(b"000184467440737095516150\x00\x00006")
    );
}
