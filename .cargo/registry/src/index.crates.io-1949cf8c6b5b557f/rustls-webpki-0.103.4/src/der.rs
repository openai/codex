// Copyright 2015 Brian Smith.
//
// Permission to use, copy, modify, and/or distribute this software for any
// purpose with or without fee is hereby granted, provided that the above
// copyright notice and this permission notice appear in all copies.
//
// THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHORS DISCLAIM ALL WARRANTIES
// WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
// MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHORS BE LIABLE FOR
// ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
// WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN
// ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF
// OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.

#[cfg(feature = "alloc")]
use alloc::vec::Vec;
use core::marker::PhantomData;

use crate::{Error, error::DerTypeId};

#[derive(Debug)]
pub struct DerIterator<'a, T> {
    reader: untrusted::Reader<'a>,
    marker: PhantomData<T>,
}

impl<'a, T> DerIterator<'a, T> {
    /// [`DerIterator`] will consume all of the bytes in `input` reading values of type `T`.
    pub(crate) fn new(input: untrusted::Input<'a>) -> Self {
        Self {
            reader: untrusted::Reader::new(input),
            marker: PhantomData,
        }
    }
}

impl<'a, T: FromDer<'a>> Iterator for DerIterator<'a, T> {
    type Item = Result<T, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        (!self.reader.at_end()).then(|| T::from_der(&mut self.reader))
    }
}

pub(crate) trait FromDer<'a>: Sized + 'a {
    /// Parse a value of type `Self` from the given DER-encoded input.
    fn from_der(reader: &mut untrusted::Reader<'a>) -> Result<Self, Error>;

    const TYPE_ID: DerTypeId;
}

pub(crate) fn read_all<'a, T: FromDer<'a>>(input: untrusted::Input<'a>) -> Result<T, Error> {
    input.read_all(Error::TrailingData(T::TYPE_ID), T::from_der)
}

// Copied (and extended) from ring's src/der.rs
#[allow(clippy::upper_case_acronyms)]
#[derive(Clone, Copy, Eq, PartialEq)]
#[repr(u8)]
pub(crate) enum Tag {
    Boolean = 0x01,
    Integer = 0x02,
    BitString = 0x03,
    OctetString = 0x04,
    OID = 0x06,
    Enum = 0x0A,
    Sequence = CONSTRUCTED | 0x10, // 0x30
    UTCTime = 0x17,
    GeneralizedTime = 0x18,

    #[allow(clippy::identity_op)]
    ContextSpecificConstructed0 = CONTEXT_SPECIFIC | CONSTRUCTED | 0,
    ContextSpecificConstructed1 = CONTEXT_SPECIFIC | CONSTRUCTED | 1,
    ContextSpecificConstructed3 = CONTEXT_SPECIFIC | CONSTRUCTED | 3,
}

pub(crate) const CONSTRUCTED: u8 = 0x20;
pub(crate) const CONTEXT_SPECIFIC: u8 = 0x80;

impl From<Tag> for usize {
    #[allow(clippy::as_conversions)]
    fn from(tag: Tag) -> Self {
        tag as Self
    }
}

impl From<Tag> for u8 {
    #[allow(clippy::as_conversions)]
    fn from(tag: Tag) -> Self {
        tag as Self
    } // XXX: narrowing conversion.
}

#[inline(always)]
pub(crate) fn expect_tag_and_get_value_limited<'a>(
    input: &mut untrusted::Reader<'a>,
    tag: Tag,
    size_limit: usize,
) -> Result<untrusted::Input<'a>, Error> {
    let (actual_tag, inner) = read_tag_and_get_value_limited(input, size_limit)?;
    if usize::from(tag) != usize::from(actual_tag) {
        return Err(Error::BadDer);
    }
    Ok(inner)
}

pub(crate) fn nested_limited<'a, R>(
    input: &mut untrusted::Reader<'a>,
    tag: Tag,
    error: Error,
    decoder: impl FnOnce(&mut untrusted::Reader<'a>) -> Result<R, Error>,
    size_limit: usize,
) -> Result<R, Error> {
    match expect_tag_and_get_value_limited(input, tag, size_limit) {
        Ok(value) => value.read_all(error, decoder),
        Err(_) => Err(error),
    }
}

// TODO: investigate taking decoder as a reference to reduce generated code
// size.
pub(crate) fn nested<'a, R>(
    input: &mut untrusted::Reader<'a>,
    tag: Tag,
    error: Error,
    decoder: impl FnOnce(&mut untrusted::Reader<'a>) -> Result<R, Error>,
) -> Result<R, Error> {
    nested_limited(input, tag, error, decoder, TWO_BYTE_DER_SIZE)
}

pub(crate) fn expect_tag<'a>(
    input: &mut untrusted::Reader<'a>,
    tag: Tag,
) -> Result<untrusted::Input<'a>, Error> {
    let (actual_tag, value) = read_tag_and_get_value_limited(input, TWO_BYTE_DER_SIZE)?;
    if usize::from(tag) != usize::from(actual_tag) {
        return Err(Error::BadDer);
    }

    Ok(value)
}

#[inline(always)]
pub(crate) fn read_tag_and_get_value<'a>(
    input: &mut untrusted::Reader<'a>,
) -> Result<(u8, untrusted::Input<'a>), Error> {
    read_tag_and_get_value_limited(input, TWO_BYTE_DER_SIZE)
}

#[inline(always)]
pub(crate) fn read_tag_and_get_value_limited<'a>(
    input: &mut untrusted::Reader<'a>,
    size_limit: usize,
) -> Result<(u8, untrusted::Input<'a>), Error> {
    let tag = input.read_byte().map_err(end_of_input_err)?;
    if (tag & HIGH_TAG_RANGE_START) == HIGH_TAG_RANGE_START {
        return Err(Error::BadDer); // High tag number form is not allowed.
    }

    // If the high order bit of the first byte is set to zero then the length
    // is encoded in the seven remaining bits of that byte. Otherwise, those
    // seven bits represent the number of bytes used to encode the length.
    let length = match input.read_byte().map_err(end_of_input_err)? {
        n if (n & SHORT_FORM_LEN_MAX) == 0 => usize::from(n),
        LONG_FORM_LEN_ONE_BYTE => {
            let length_byte = input.read_byte().map_err(end_of_input_err)?;
            if length_byte < SHORT_FORM_LEN_MAX {
                return Err(Error::BadDer); // Not the canonical encoding.
            }
            usize::from(length_byte)
        }
        LONG_FORM_LEN_TWO_BYTES => {
            let length_byte_one = usize::from(input.read_byte().map_err(end_of_input_err)?);
            let length_byte_two = usize::from(input.read_byte().map_err(end_of_input_err)?);
            let combined = (length_byte_one << 8) | length_byte_two;
            if combined <= LONG_FORM_LEN_ONE_BYTE_MAX {
                return Err(Error::BadDer); // Not the canonical encoding.
            }
            combined
        }
        LONG_FORM_LEN_THREE_BYTES => {
            let length_byte_one = usize::from(input.read_byte().map_err(end_of_input_err)?);
            let length_byte_two = usize::from(input.read_byte().map_err(end_of_input_err)?);
            let length_byte_three = usize::from(input.read_byte().map_err(end_of_input_err)?);
            let combined = (length_byte_one << 16) | (length_byte_two << 8) | length_byte_three;
            if combined <= LONG_FORM_LEN_TWO_BYTES_MAX {
                return Err(Error::BadDer); // Not the canonical encoding.
            }
            combined
        }
        LONG_FORM_LEN_FOUR_BYTES => {
            let length_byte_one = usize::from(input.read_byte().map_err(end_of_input_err)?);
            let length_byte_two = usize::from(input.read_byte().map_err(end_of_input_err)?);
            let length_byte_three = usize::from(input.read_byte().map_err(end_of_input_err)?);
            let length_byte_four = usize::from(input.read_byte().map_err(end_of_input_err)?);
            let combined = (length_byte_one << 24)
                | (length_byte_two << 16)
                | (length_byte_three << 8)
                | length_byte_four;
            if combined <= LONG_FORM_LEN_THREE_BYTES_MAX {
                return Err(Error::BadDer); // Not the canonical encoding.
            }
            combined
        }
        _ => {
            return Err(Error::BadDer); // We don't support longer lengths.
        }
    };

    if length >= size_limit {
        return Err(Error::BadDer); // The length is larger than the caller accepts.
    }

    let inner = input.read_bytes(length).map_err(end_of_input_err)?;
    Ok((tag, inner))
}

/// Prepend `bytes` with the given ASN.1 [`Tag`] and appropriately encoded length byte(s).
/// Useful for "adding back" ASN.1 bytes to parsed content.
#[cfg(feature = "alloc")]
#[allow(clippy::as_conversions)]
pub(crate) fn asn1_wrap(tag: Tag, bytes: &[u8]) -> Vec<u8> {
    let len = bytes.len();
    // The length is encoded differently depending on how many bytes there are
    if len < usize::from(SHORT_FORM_LEN_MAX) {
        // Short form: the length is encoded using a single byte
        // Contents: Tag byte, single length byte, and passed bytes
        let mut ret = Vec::with_capacity(2 + len);
        ret.push(tag.into()); // Tag byte
        ret.push(len as u8); // Single length byte
        ret.extend_from_slice(bytes); // Passed bytes
        ret
    } else {
        // Long form: The length is encoded using multiple bytes
        // Contents: Tag byte, number-of-length-bytes byte, length bytes, and passed bytes
        // The first byte indicates how many more bytes will be used to encode the length
        // First, get a big-endian representation of the byte slice's length
        let size = len.to_be_bytes();
        // Find the number of leading empty bytes in that representation
        // This will determine the smallest number of bytes we need to encode the length
        let leading_zero_bytes = size
            .iter()
            .position(|&byte| byte != 0)
            .unwrap_or(size.len());
        assert!(leading_zero_bytes < size.len());
        // Number of bytes used - number of not needed bytes = smallest number needed
        let encoded_bytes = size.len() - leading_zero_bytes;
        let mut ret = Vec::with_capacity(2 + encoded_bytes + len);
        // Indicate this is a number-of-length-bytes byte by setting the high order bit
        let number_of_length_bytes_byte = SHORT_FORM_LEN_MAX + encoded_bytes as u8;
        ret.push(tag.into()); // Tag byte
        ret.push(number_of_length_bytes_byte); // Number-of-length-bytes byte
        ret.extend_from_slice(&size[leading_zero_bytes..]); // Length bytes
        ret.extend_from_slice(bytes); // Passed bytes
        ret
    }
}

// Long-form DER encoded lengths of two bytes can express lengths up to the following limit.
//
// The upstream ring::io::der::read_tag_and_get_value() function limits itself to up to two byte
// long-form DER lengths, and so this limit represents the maximum length that was possible to
// read before the introduction of the read_tag_and_get_value_limited function.
pub(crate) const TWO_BYTE_DER_SIZE: usize = LONG_FORM_LEN_TWO_BYTES_MAX;

// The maximum size of a DER value that Webpki can support reading.
//
// Webpki limits itself to four byte long-form DER lengths, and so this limit represents
// the maximum size tagged DER value that can be read for any purpose.
pub(crate) const MAX_DER_SIZE: usize = LONG_FORM_LEN_FOUR_BYTES_MAX;

// DER Tag identifiers have two forms:
// * Low tag number form (for tags values in the range [0..30]
// * High tag number form (for tag values in the range [31..]
// We only support low tag number form.
const HIGH_TAG_RANGE_START: u8 = 31;

// DER length octets have two forms:
// * Short form: 1 octet supporting lengths between 0 and 127.
// * Long definite form: 2 to 127 octets, number of octets encoded into first octet.
const SHORT_FORM_LEN_MAX: u8 = 128;

// Leading octet for long definite form DER length expressed in second byte.
const LONG_FORM_LEN_ONE_BYTE: u8 = 0x81;

// Maximum size that can be expressed in a one byte long form len.
const LONG_FORM_LEN_ONE_BYTE_MAX: usize = 0xff;

// Leading octet for long definite form DER length expressed in subsequent two bytes.
const LONG_FORM_LEN_TWO_BYTES: u8 = 0x82;

// Maximum size that can be expressed in a two byte long form len.
const LONG_FORM_LEN_TWO_BYTES_MAX: usize = 0xff_ff;

// Leading octet for long definite form DER length expressed in subsequent three bytes.
const LONG_FORM_LEN_THREE_BYTES: u8 = 0x83;

// Maximum size that can be expressed in a three byte long form len.
const LONG_FORM_LEN_THREE_BYTES_MAX: usize = 0xff_ff_ff;

// Leading octet for long definite form DER length expressed in subsequent four bytes.
const LONG_FORM_LEN_FOUR_BYTES: u8 = 0x84;

// Maximum size that can be expressed in a four byte long form der len.
const LONG_FORM_LEN_FOUR_BYTES_MAX: usize = 0xff_ff_ff_ff;

// TODO: investigate taking decoder as a reference to reduce generated code
// size.
pub(crate) fn nested_of_mut<'a>(
    input: &mut untrusted::Reader<'a>,
    outer_tag: Tag,
    inner_tag: Tag,
    error: Error,
    allow_empty: bool,
    mut decoder: impl FnMut(&mut untrusted::Reader<'a>) -> Result<(), Error>,
) -> Result<(), Error> {
    nested(input, outer_tag, error.clone(), |outer| {
        if allow_empty && outer.at_end() {
            return Ok(());
        }
        loop {
            nested(outer, inner_tag, error.clone(), |inner| decoder(inner))?;
            if outer.at_end() {
                break;
            }
        }
        Ok(())
    })
}

pub(crate) fn bit_string_with_no_unused_bits<'a>(
    input: &mut untrusted::Reader<'a>,
) -> Result<untrusted::Input<'a>, Error> {
    nested(
        input,
        Tag::BitString,
        Error::TrailingData(DerTypeId::BitString),
        |value| {
            let unused_bits_at_end = value.read_byte().map_err(|_| Error::BadDer)?;
            if unused_bits_at_end != 0 {
                return Err(Error::BadDer);
            }
            Ok(value.read_bytes_to_end())
        },
    )
}

pub(crate) struct BitStringFlags<'a> {
    raw_bits: &'a [u8],
}

impl BitStringFlags<'_> {
    pub(crate) fn bit_set(&self, bit: usize) -> bool {
        let byte_index = bit / 8;
        let bit_shift = 7 - (bit % 8);

        if self.raw_bits.len() < (byte_index + 1) {
            false
        } else {
            ((self.raw_bits[byte_index] >> bit_shift) & 1) != 0
        }
    }
}

// ASN.1 BIT STRING fields for sets of flags are encoded in DER with some peculiar details related
// to padding. Notably this means we expect an indicator of the number of bits of padding, and then
// the actual bit values. See this Stack Overflow discussion[0], and ITU X690-0207[1] Section 8.6
// and Section 11.2 for more information.
//
// [0]: https://security.stackexchange.com/a/10396
// [1]: https://www.itu.int/ITU-T/studygroups/com17/languages/X.690-0207.pdf
pub(crate) fn bit_string_flags(input: untrusted::Input<'_>) -> Result<BitStringFlags<'_>, Error> {
    input.read_all(Error::BadDer, |bit_string| {
        // ITU X690-0207 11.2:
        //   "The initial octet shall encode, as an unsigned binary integer with bit 1 as the least
        //   significant bit, the number of unused bits in the final subsequent octet.
        //   The number shall be in the range zero to seven"
        let padding_bits = bit_string.read_byte().map_err(|_| Error::BadDer)?;
        let raw_bits = bit_string.read_bytes_to_end().as_slice_less_safe();

        // It's illegal to have more than 7 bits of padding. Similarly, if the raw bitflags
        // are empty there should be no padding.
        if padding_bits > 7 || (raw_bits.is_empty() && padding_bits != 0) {
            return Err(Error::BadDer);
        }

        // If there are padding bits then the last bit of the last raw byte must be 0 or the
        // distinguished encoding rules are not being followed.
        let last_byte = raw_bits[raw_bits.len() - 1];
        let padding_mask = (1 << padding_bits) - 1;

        match padding_bits > 0 && (last_byte & padding_mask) != 0 {
            true => Err(Error::BadDer),
            false => Ok(BitStringFlags { raw_bits }),
        }
    })
}

impl<'a> FromDer<'a> for u8 {
    fn from_der(reader: &mut untrusted::Reader<'a>) -> Result<Self, Error> {
        match *nonnegative_integer(reader)?.as_slice_less_safe() {
            [b] => Ok(b),
            _ => Err(Error::BadDer),
        }
    }

    const TYPE_ID: DerTypeId = DerTypeId::U8;
}

pub(crate) fn nonnegative_integer<'a>(
    input: &mut untrusted::Reader<'a>,
) -> Result<untrusted::Input<'a>, Error> {
    let value = expect_tag(input, Tag::Integer)?;
    match value
        .as_slice_less_safe()
        .split_first()
        .ok_or(Error::BadDer)?
    {
        // Zero or leading zero.
        (0, rest) => {
            match rest.first() {
                // Zero
                None => Ok(value),
                // Necessary leading zero.
                Some(&second) if second & 0x80 == 0x80 => Ok(untrusted::Input::from(rest)),
                // Unnecessary leading zero.
                _ => Err(Error::BadDer),
            }
        }
        // Positive value with no leading zero.
        (first, _) if first & 0x80 == 0x00 => Ok(value),
        // Negative value.
        (_, _) => Err(Error::BadDer),
    }
}

pub(crate) fn end_of_input_err(_: untrusted::EndOfInput) -> Error {
    Error::BadDer
}

// Like mozilla::pkix, we accept the nonconformant explicit encoding of
// the default value (false) for compatibility with real-world certificates.
impl<'a> FromDer<'a> for bool {
    fn from_der(reader: &mut untrusted::Reader<'a>) -> Result<Self, Error> {
        if !reader.peek(Tag::Boolean.into()) {
            return Ok(false);
        }

        nested(
            reader,
            Tag::Boolean,
            Error::TrailingData(Self::TYPE_ID),
            |input| match input.read_byte() {
                Ok(0xff) => Ok(true),
                Ok(0x00) => Ok(false),
                _ => Err(Error::BadDer),
            },
        )
    }

    const TYPE_ID: DerTypeId = DerTypeId::Bool;
}

macro_rules! oid {
    ( $first:expr, $second:expr, $( $tail:expr ),* ) =>
    (
        [(40 * $first) + $second, $( $tail ),*]
    )
}

#[cfg(test)]
mod tests {
    use super::DerTypeId;
    use std::prelude::v1::*;

    #[cfg(feature = "alloc")]
    #[test]
    fn test_asn1_wrap() {
        // Prepend stuff to `bytes` to put it in a DER SEQUENCE.
        let wrap_in_sequence = |bytes: &[u8]| super::asn1_wrap(super::Tag::Sequence, bytes);

        // Empty slice
        assert_eq!(vec![0x30, 0x00], wrap_in_sequence(&[]));

        // Small size
        assert_eq!(
            vec![0x30, 0x04, 0x00, 0x11, 0x22, 0x33],
            wrap_in_sequence(&[0x00, 0x11, 0x22, 0x33])
        );

        // Medium size
        let mut val = Vec::new();
        val.resize(255, 0x12);
        assert_eq!(
            vec![0x30, 0x81, 0xff, 0x12, 0x12, 0x12],
            wrap_in_sequence(&val)[..6]
        );

        // Large size
        let mut val = Vec::new();
        val.resize(4660, 0x12);
        wrap_in_sequence(&val);
        assert_eq!(
            vec![0x30, 0x82, 0x12, 0x34, 0x12, 0x12],
            wrap_in_sequence(&val)[..6]
        );

        // Huge size
        let mut val = Vec::new();
        val.resize(0xffff, 0x12);
        let result = wrap_in_sequence(&val);
        assert_eq!(vec![0x30, 0x82, 0xff, 0xff, 0x12, 0x12], result[..6]);
        assert_eq!(result.len(), 0xffff + 4);

        // Gigantic size
        let mut val = Vec::new();
        val.resize(0x100000, 0x12);
        let result = wrap_in_sequence(&val);
        assert_eq!(vec![0x30, 0x83, 0x10, 0x00, 0x00, 0x12, 0x12], result[..7]);
        assert_eq!(result.len(), 0x100000 + 5);

        // Ludicrous size
        let mut val = Vec::new();
        val.resize(0x1000000, 0x12);
        let result = wrap_in_sequence(&val);
        assert_eq!(
            vec![0x30, 0x84, 0x01, 0x00, 0x00, 0x00, 0x12, 0x12],
            result[..8]
        );
        assert_eq!(result.len(), 0x1000000 + 6);
    }

    #[test]
    fn test_optional_boolean() {
        use super::{Error, FromDer};

        // Empty input results in false
        assert!(!bool::from_der(&mut bytes_reader(&[])).unwrap());

        // Optional, so another data type results in false
        assert!(!bool::from_der(&mut bytes_reader(&[0x05, 0x00])).unwrap());

        // Only 0x00 and 0xff are accepted values
        assert_eq!(
            Err(Error::BadDer),
            bool::from_der(&mut bytes_reader(&[0x01, 0x01, 0x42]))
        );

        // True
        assert!(bool::from_der(&mut bytes_reader(&[0x01, 0x01, 0xff])).unwrap());

        // False
        assert!(!bool::from_der(&mut bytes_reader(&[0x01, 0x01, 0x00])).unwrap());
    }

    #[test]
    fn test_bit_string_with_no_unused_bits() {
        use super::{Error, bit_string_with_no_unused_bits};

        // Unexpected type
        assert_eq!(
            bit_string_with_no_unused_bits(&mut bytes_reader(&[0x01, 0x01, 0xff])).unwrap_err(),
            Error::TrailingData(DerTypeId::BitString),
        );

        // Unexpected nonexistent type
        assert_eq!(
            bit_string_with_no_unused_bits(&mut bytes_reader(&[0x42, 0xff, 0xff])).unwrap_err(),
            Error::TrailingData(DerTypeId::BitString),
        );

        // Unexpected empty input
        assert_eq!(
            bit_string_with_no_unused_bits(&mut bytes_reader(&[])).unwrap_err(),
            Error::TrailingData(DerTypeId::BitString),
        );

        // Valid input with non-zero unused bits
        assert_eq!(
            bit_string_with_no_unused_bits(&mut bytes_reader(&[0x03, 0x03, 0x04, 0x12, 0x34]))
                .unwrap_err(),
            Error::BadDer,
        );

        // Valid input
        assert_eq!(
            bit_string_with_no_unused_bits(&mut bytes_reader(&[0x03, 0x03, 0x00, 0x12, 0x34]))
                .unwrap()
                .as_slice_less_safe(),
            &[0x12, 0x34],
        );
    }

    fn bytes_reader(bytes: &[u8]) -> untrusted::Reader<'_> {
        untrusted::Reader::new(untrusted::Input::from(bytes))
    }

    #[test]
    fn read_tag_and_get_value_default_limit() {
        use super::{Error, read_tag_and_get_value};

        let inputs = &[
            // DER with short-form length encoded as three bytes.
            &[EXAMPLE_TAG, 0x83, 0xFF, 0xFF, 0xFF].as_slice(),
            // DER with short-form length encoded as four bytes.
            &[EXAMPLE_TAG, 0x84, 0xFF, 0xFF, 0xFF, 0xFF].as_slice(),
        ];

        for input in inputs {
            let mut bytes = untrusted::Reader::new(untrusted::Input::from(input));
            // read_tag_and_get_value should reject DER with encoded lengths larger than two
            // bytes as BadDer.
            assert!(matches!(
                read_tag_and_get_value(&mut bytes),
                Err(Error::BadDer)
            ));
        }
    }

    #[test]
    fn read_tag_and_get_value_limited_high_form() {
        use super::{Error, LONG_FORM_LEN_TWO_BYTES_MAX, read_tag_and_get_value_limited};

        let mut bytes = untrusted::Reader::new(untrusted::Input::from(&[0xFF]));
        // read_tag_and_get_value_limited_high_form should reject DER with "high tag number form" tags.
        assert!(matches!(
            read_tag_and_get_value_limited(&mut bytes, LONG_FORM_LEN_TWO_BYTES_MAX),
            Err(Error::BadDer)
        ));
    }

    #[test]
    fn read_tag_and_get_value_limited_non_canonical() {
        use super::{Error, LONG_FORM_LEN_TWO_BYTES_MAX, read_tag_and_get_value_limited};

        let inputs = &[
            // Two byte length, with expressed length < 128.
            &[EXAMPLE_TAG, 0x81, 0x01].as_slice(),
            // Three byte length, with expressed length < 256.
            &[EXAMPLE_TAG, 0x82, 0x00, 0x01].as_slice(),
            // Four byte length, with expressed length, < 65536.
            &[EXAMPLE_TAG, 0x83, 0x00, 0x00, 0x01].as_slice(),
            // Five byte length, with expressed length < 16777216.
            &[EXAMPLE_TAG, 0x84, 0x00, 0x00, 0x00, 0x01].as_slice(),
        ];

        for input in inputs {
            let mut bytes = untrusted::Reader::new(untrusted::Input::from(input));
            // read_tag_and_get_value_limited should reject DER with non-canonical lengths.
            assert!(matches!(
                read_tag_and_get_value_limited(&mut bytes, LONG_FORM_LEN_TWO_BYTES_MAX),
                Err(Error::BadDer)
            ));
        }
    }

    #[test]
    #[cfg(feature = "alloc")]
    fn read_tag_and_get_value_limited_limits() {
        use super::{Error, read_tag_and_get_value_limited};

        let short_input = &[0xFF];
        let short_input_encoded = &[
            &[EXAMPLE_TAG],
            der_encode_length(short_input.len()).as_slice(),
            short_input,
        ]
        .concat();

        let long_input = &[1_u8; 65537];
        let long_input_encoded = &[
            &[EXAMPLE_TAG],
            der_encode_length(long_input.len()).as_slice(),
            long_input,
        ]
        .concat();

        struct Testcase<'a> {
            input: &'a [u8],
            limit: usize,
            err: Option<Error>,
        }

        let testcases = &[
            Testcase {
                input: short_input_encoded,
                limit: 1,
                err: Some(Error::BadDer),
            },
            Testcase {
                input: short_input_encoded,
                limit: short_input_encoded.len() + 1,
                err: None,
            },
            Testcase {
                input: long_input_encoded,
                limit: long_input.len(),
                err: Some(Error::BadDer),
            },
            Testcase {
                input: long_input_encoded,
                limit: long_input.len() + 1,
                err: None,
            },
        ];

        for tc in testcases {
            let mut bytes = untrusted::Reader::new(untrusted::Input::from(tc.input));

            let res = read_tag_and_get_value_limited(&mut bytes, tc.limit);
            match &tc.err {
                None => assert!(res.is_ok()),
                Some(e) => {
                    let actual = res.unwrap_err();
                    assert_eq!(&actual, e)
                }
            }
        }
    }

    #[allow(clippy::as_conversions)] // infallible.
    const EXAMPLE_TAG: u8 = super::Tag::Sequence as u8;

    #[cfg(feature = "alloc")]
    #[allow(clippy::as_conversions)] // test code.
    fn der_encode_length(length: usize) -> Vec<u8> {
        if length < 128 {
            vec![length as u8]
        } else {
            let mut encoded: Vec<u8> = Vec::new();
            let mut remaining_length = length;

            while remaining_length > 0 {
                let byte = (remaining_length & 0xFF) as u8;
                encoded.insert(0, byte);
                remaining_length >>= 8;
            }

            let length_octet = encoded.len() as u8 | 0x80;
            encoded.insert(0, length_octet);

            encoded
        }
    }

    #[test]
    fn misencoded_bit_string_flags() {
        use super::{Error, bit_string_flags};

        let bad_padding_example = untrusted::Input::from(&[
            0x08, // 8 bit of padding (illegal!).
            0x06, // 1 byte of bit flags asserting bits 5 and 6.
        ]);
        assert!(matches!(
            bit_string_flags(bad_padding_example),
            Err(Error::BadDer)
        ));

        let bad_padding_example = untrusted::Input::from(&[
            0x01, // 1 bit of padding.
                 // No flags value (illegal with padding!).
        ]);
        assert!(matches!(
            bit_string_flags(bad_padding_example),
            Err(Error::BadDer)
        ));
    }

    #[test]
    fn valid_bit_string_flags() {
        use super::bit_string_flags;

        let example_key_usage = untrusted::Input::from(&[
            0x01, // 1 bit of padding.
            0x06, // 1 byte of bit flags asserting bits 5 and 6.
        ]);
        let res = bit_string_flags(example_key_usage).unwrap();

        assert!(!res.bit_set(0));
        assert!(!res.bit_set(1));
        assert!(!res.bit_set(2));
        assert!(!res.bit_set(3));
        assert!(!res.bit_set(4));
        // NB: Bits 5 and 6 should be set.
        assert!(res.bit_set(5));
        assert!(res.bit_set(6));
        assert!(!res.bit_set(7));
        assert!(!res.bit_set(8));
        // Bits outside the range of values shouldn't be considered set.
        assert!(!res.bit_set(256));
    }

    #[test]
    fn test_small_nonnegative_integer() {
        use super::{Error, FromDer, Tag};

        for value in 0..=127 {
            let data = [Tag::Integer.into(), 1, value];
            let mut rd = untrusted::Reader::new(untrusted::Input::from(&data));
            assert_eq!(u8::from_der(&mut rd), Ok(value),);
        }

        for value in 128..=255 {
            let data = [Tag::Integer.into(), 2, 0x00, value];
            let mut rd = untrusted::Reader::new(untrusted::Input::from(&data));
            assert_eq!(u8::from_der(&mut rd), Ok(value),);
        }

        // not an integer
        assert_eq!(
            u8::from_der(&mut untrusted::Reader::new(untrusted::Input::from(&[
                Tag::Sequence.into(),
                1,
                1
            ]))),
            Err(Error::BadDer)
        );

        // negative
        assert_eq!(
            u8::from_der(&mut untrusted::Reader::new(untrusted::Input::from(&[
                Tag::Integer.into(),
                1,
                0xff
            ]))),
            Err(Error::BadDer)
        );

        // positive but too large
        assert_eq!(
            u8::from_der(&mut untrusted::Reader::new(untrusted::Input::from(&[
                Tag::Integer.into(),
                2,
                0x01,
                0x00
            ]))),
            Err(Error::BadDer)
        );

        // unnecessary leading zero
        assert_eq!(
            u8::from_der(&mut untrusted::Reader::new(untrusted::Input::from(&[
                Tag::Integer.into(),
                2,
                0x00,
                0x05
            ]))),
            Err(Error::BadDer)
        );

        // truncations
        assert_eq!(
            u8::from_der(&mut untrusted::Reader::new(untrusted::Input::from(&[]))),
            Err(Error::BadDer)
        );

        assert_eq!(
            u8::from_der(&mut untrusted::Reader::new(untrusted::Input::from(&[
                Tag::Integer.into(),
            ]))),
            Err(Error::BadDer)
        );

        assert_eq!(
            u8::from_der(&mut untrusted::Reader::new(untrusted::Input::from(&[
                Tag::Integer.into(),
                1,
            ]))),
            Err(Error::BadDer)
        );

        assert_eq!(
            u8::from_der(&mut untrusted::Reader::new(untrusted::Input::from(&[
                Tag::Integer.into(),
                2,
                0
            ]))),
            Err(Error::BadDer)
        );
    }
}
