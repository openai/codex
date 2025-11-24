// Copyright 2015 Brian Smith.
//
// Permission to use, copy, modify, and/or distribute this software for any
// purpose with or without fee is hereby granted, provided that the above
// copyright notice and this permission notice appear in all copies.
//
// THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
// WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
// MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR ANY
// SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
// WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN ACTION
// OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF OR IN
// CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.

//! Building blocks for parsing DER-encoded ASN.1 structures.
//!
//! This module contains the foundational parts of an ASN.1 DER parser.

use super::Positive;
use crate::error;

pub const CONSTRUCTED: u8 = 1 << 5;
pub const CONTEXT_SPECIFIC: u8 = 2 << 6;

#[derive(Clone, Copy, PartialEq)]
#[repr(u8)]
pub enum Tag {
    Boolean = 0x01,
    Integer = 0x02,
    BitString = 0x03,
    OctetString = 0x04,
    Null = 0x05,
    OID = 0x06,
    Sequence = CONSTRUCTED | 0x10, // 0x30
    UTCTime = 0x17,
    GeneralizedTime = 0x18,

    ContextSpecific1 = CONTEXT_SPECIFIC | 1,

    ContextSpecificConstructed0 = CONTEXT_SPECIFIC | CONSTRUCTED | 0,
    ContextSpecificConstructed1 = CONTEXT_SPECIFIC | CONSTRUCTED | 1,
    ContextSpecificConstructed3 = CONTEXT_SPECIFIC | CONSTRUCTED | 3,
}

impl From<Tag> for usize {
    fn from(tag: Tag) -> Self {
        Self::from(Tag::into(tag))
    }
}

impl From<Tag> for u8 {
    fn from(tag: Tag) -> Self {
        Tag::into(tag)
    }
}

// `impl From<Tag> for u8` but as a `const fn`.
impl Tag {
    pub const fn into(self) -> u8 {
        self as u8
    }
}

pub fn expect_tag_and_get_value<'a>(
    input: &mut untrusted::Reader<'a>,
    tag: Tag,
) -> Result<untrusted::Input<'a>, error::Unspecified> {
    let (actual_tag, inner) = read_tag_and_get_value(input)?;
    if usize::from(tag) != usize::from(actual_tag) {
        return Err(error::Unspecified);
    }
    Ok(inner)
}

pub fn read_tag_and_get_value<'a>(
    input: &mut untrusted::Reader<'a>,
) -> Result<(u8, untrusted::Input<'a>), error::Unspecified> {
    let tag = input.read_byte()?;
    if (tag & 0x1F) == 0x1F {
        return Err(error::Unspecified); // High tag number form is not allowed.
    }

    // If the high order bit of the first byte is set to zero then the length
    // is encoded in the seven remaining bits of that byte. Otherwise, those
    // seven bits represent the number of bytes used to encode the length.
    let length = match input.read_byte()? {
        n if (n & 0x80) == 0 => usize::from(n),
        0x81 => {
            let second_byte = input.read_byte()?;
            if second_byte < 128 {
                return Err(error::Unspecified); // Not the canonical encoding.
            }
            usize::from(second_byte)
        }
        0x82 => {
            let second_byte = usize::from(input.read_byte()?);
            let third_byte = usize::from(input.read_byte()?);
            let combined = (second_byte << 8) | third_byte;
            if combined < 256 {
                return Err(error::Unspecified); // Not the canonical encoding.
            }
            combined
        }
        _ => {
            return Err(error::Unspecified); // We don't support longer lengths.
        }
    };

    let inner = input.read_bytes(length)?;
    Ok((tag, inner))
}

#[inline]
pub fn bit_string_with_no_unused_bits<'a>(
    input: &mut untrusted::Reader<'a>,
) -> Result<untrusted::Input<'a>, error::Unspecified> {
    bit_string_tagged_with_no_unused_bits(Tag::BitString, input)
}

pub(crate) fn bit_string_tagged_with_no_unused_bits<'a>(
    tag: Tag,
    input: &mut untrusted::Reader<'a>,
) -> Result<untrusted::Input<'a>, error::Unspecified> {
    nested(input, tag, error::Unspecified, |value| {
        let unused_bits_at_end = value.read_byte().map_err(|_| error::Unspecified)?;
        if unused_bits_at_end != 0 {
            return Err(error::Unspecified);
        }
        Ok(value.read_bytes_to_end())
    })
}

// TODO: investigate taking decoder as a reference to reduce generated code
// size.
pub fn nested<'a, F, R, E: Copy>(
    input: &mut untrusted::Reader<'a>,
    tag: Tag,
    error: E,
    decoder: F,
) -> Result<R, E>
where
    F: FnOnce(&mut untrusted::Reader<'a>) -> Result<R, E>,
{
    let inner = expect_tag_and_get_value(input, tag).map_err(|_| error)?;
    inner.read_all(error, decoder)
}

pub(crate) fn nonnegative_integer<'a>(
    input: &mut untrusted::Reader<'a>,
) -> Result<untrusted::Input<'a>, error::Unspecified> {
    let value = expect_tag_and_get_value(input, Tag::Integer)?;
    match value
        .as_slice_less_safe()
        .split_first()
        .ok_or(error::Unspecified)?
    {
        // Zero or leading zero.
        (0, rest) => {
            match rest.first() {
                // Zero.
                None => Ok(value),
                // Necessary leading zero.
                Some(&second) if second & 0x80 == 0x80 => Ok(untrusted::Input::from(rest)),
                // Unnecessary leading zero.
                _ => Err(error::Unspecified),
            }
        }
        // Positive value with no leading zero.
        (first, _) if first & 0x80 == 0 => Ok(value),
        // Negative value.
        (_, _) => Err(error::Unspecified),
    }
}

/// Parse as integer with a value in the in the range [0, 255], returning its
/// numeric value. This is typically used for parsing version numbers.
#[inline]
pub fn small_nonnegative_integer(input: &mut untrusted::Reader) -> Result<u8, error::Unspecified> {
    let value = nonnegative_integer(input)?;
    match *value.as_slice_less_safe() {
        [b] => Ok(b),
        _ => Err(error::Unspecified),
    }
}

/// Parses a positive DER integer, returning the big-endian-encoded value,
/// sans any leading zero byte.
pub fn positive_integer<'a>(
    input: &mut untrusted::Reader<'a>,
) -> Result<Positive<'a>, error::Unspecified> {
    let value = nonnegative_integer(input)?;
    Positive::from_be_bytes(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error;

    fn with_i<'a, F, R>(value: &'a [u8], f: F) -> Result<R, error::Unspecified>
    where
        F: FnOnce(&mut untrusted::Reader<'a>) -> Result<R, error::Unspecified>,
    {
        untrusted::Input::from(value).read_all(error::Unspecified, f)
    }

    static ZERO_INTEGER: &[u8] = &[0x02, 0x01, 0x00];

    static GOOD_POSITIVE_INTEGERS_SMALL: &[(&[u8], u8)] = &[
        (&[0x02, 0x01, 0x01], 0x01),
        (&[0x02, 0x01, 0x02], 0x02),
        (&[0x02, 0x01, 0x7e], 0x7e),
        (&[0x02, 0x01, 0x7f], 0x7f),
        // Values that need to have an 0x00 prefix to disambiguate them from
        // them from negative values.
        (&[0x02, 0x02, 0x00, 0x80], 0x80),
        (&[0x02, 0x02, 0x00, 0x81], 0x81),
        (&[0x02, 0x02, 0x00, 0xfe], 0xfe),
        (&[0x02, 0x02, 0x00, 0xff], 0xff),
    ];

    static GOOD_POSITIVE_INTEGERS_LARGE: &[(&[u8], &[u8])] = &[
        (&[0x02, 0x02, 0x01, 0x00], &[0x01, 0x00]),
        (&[0x02, 0x02, 0x02, 0x01], &[0x02, 0x01]),
        (&[0x02, 0x02, 0x7e, 0xfe], &[0x7e, 0xfe]),
        (&[0x02, 0x02, 0x7f, 0xff], &[0x7f, 0xff]),
        // Values that need to have an 0x00 prefix to disambiguate them from
        // them from negative values.
        (&[0x02, 0x03, 0x00, 0x80, 0x00], &[0x80, 0x00]),
        (&[0x02, 0x03, 0x00, 0x81, 0x01], &[0x81, 0x01]),
        (&[0x02, 0x03, 0x00, 0xfe, 0xfe], &[0xfe, 0xfe]),
        (&[0x02, 0x03, 0x00, 0xff, 0xff], &[0xff, 0xff]),
    ];

    static BAD_NONNEGATIVE_INTEGERS: &[&[u8]] = &[
        &[],           // At end of input
        &[0x02],       // Tag only
        &[0x02, 0x00], // Empty value
        // Length mismatch
        &[0x02, 0x00, 0x01],
        &[0x02, 0x01],
        // Would be valid if leading zero is ignored when comparing length.
        &[0x02, 0x01, 0x00, 0x01],
        &[0x02, 0x01, 0x01, 0x00], // Would be valid if last byte is ignored.
        &[0x02, 0x02, 0x01],
        // Values that are missing a necessary leading 0x00
        &[0x02, 0x01, 0x80],
        &[0x02, 0x01, 0x81],
        &[0x02, 0x01, 0xfe],
        &[0x02, 0x01, 0xff],
        // Values that have an unnecessary leading 0x00
        &[0x02, 0x02, 0x00, 0x00],
        &[0x02, 0x02, 0x00, 0x01],
        &[0x02, 0x02, 0x00, 0x02],
        &[0x02, 0x02, 0x00, 0x7e],
        &[0x02, 0x02, 0x00, 0x7f],
    ];

    #[test]
    fn test_small_nonnegative_integer() {
        let zero = (ZERO_INTEGER, 0x00);
        for &(test_in, test_out) in
            core::iter::once(&zero).chain(GOOD_POSITIVE_INTEGERS_SMALL.iter())
        {
            let result = with_i(test_in, |input| {
                assert_eq!(small_nonnegative_integer(input)?, test_out);
                Ok(())
            });
            assert_eq!(result, Ok(()));
        }
        for &test_in in BAD_NONNEGATIVE_INTEGERS
            .iter()
            .chain(GOOD_POSITIVE_INTEGERS_LARGE.iter().map(|(input, _)| input))
        {
            let result = with_i(test_in, small_nonnegative_integer);
            assert_eq!(result, Err(error::Unspecified));
        }
    }

    #[test]
    fn test_positive_integer() {
        for (test_in, test_out) in GOOD_POSITIVE_INTEGERS_SMALL
            .iter()
            .map(|(test_in, test_out)| (*test_in, core::slice::from_ref(test_out)))
            .chain(GOOD_POSITIVE_INTEGERS_LARGE.iter().copied())
        {
            let result = with_i(test_in, |input| {
                assert_eq!(
                    positive_integer(input)?.big_endian_without_leading_zero(),
                    test_out
                );
                Ok(())
            });
            assert_eq!(result, Ok(()))
        }
        for &test_in in core::iter::once(&ZERO_INTEGER).chain(BAD_NONNEGATIVE_INTEGERS.iter()) {
            let result = with_i(test_in, positive_integer);
            assert!(matches!(result, Err(error::Unspecified)));
        }
    }
}
