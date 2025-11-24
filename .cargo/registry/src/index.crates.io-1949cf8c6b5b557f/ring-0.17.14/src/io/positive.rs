// Copyright 2018 Brian Smith.
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

//! Serialization and deserialization.

use crate::error;

/// A serialized positive integer.
#[derive(Copy, Clone)]
pub struct Positive<'a>(untrusted::Input<'a>);

impl<'a> Positive<'a> {
    #[inline]
    pub(crate) fn from_be_bytes(input: untrusted::Input<'a>) -> Result<Self, error::Unspecified> {
        // Empty inputs are not allowed.
        let &first_byte = input
            .as_slice_less_safe()
            .first()
            .ok_or(error::Unspecified)?;
        // Zero isn't allowed and leading zeros aren't allowed.
        if first_byte == 0 {
            return Err(error::Unspecified);
        }
        Ok(Self(input))
    }

    /// Returns the value, ordered from significant byte to least significant
    /// byte, without any leading zeros. The result is guaranteed to be
    /// non-empty.
    #[inline]
    pub fn big_endian_without_leading_zero(&self) -> &'a [u8] {
        self.big_endian_without_leading_zero_as_input()
            .as_slice_less_safe()
    }

    #[inline]
    pub(crate) fn big_endian_without_leading_zero_as_input(&self) -> untrusted::Input<'a> {
        self.0
    }
}

impl Positive<'_> {
    /// Returns the first byte.
    ///
    /// Will not panic because the value is guaranteed to have at least one
    /// byte.
    pub fn first_byte(&self) -> u8 {
        // This won't panic because
        self.0.as_slice_less_safe()[0]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_be_bytes() {
        static TEST_CASES: &[(&[u8], Result<&[u8], error::Unspecified>)] = &[
            // An empty input isn't a number.
            (&[], Err(error::Unspecified)),
            // Zero is not positive.
            (&[0x00], Err(error::Unspecified)),
            // Minimum value. No leading zero required or allowed.
            (&[0x00, 0x01], Err(error::Unspecified)),
            (&[0x01], Ok(&[0x01])),
            // Maximum first byte. No leading zero required or allowed.
            (&[0xff], Ok(&[0xff])),
            (&[0x00, 0xff], Err(error::Unspecified)),
            // The last byte can be zero.
            (&[0x01, 0x00], Ok(&[0x01, 0x00])),
            (&[0x01, 0x00, 0x00], Ok(&[0x01, 0x00, 0x00])),
            // Having no zero bytes are also allowed.
            (&[0x01, 0x01], Ok(&[0x01, 0x01])),
            // A middle byte can be zero.
            (&[0x01, 0x00, 0x01], Ok(&[0x01, 0x00, 0x01])),
            (&[0x01, 0x01, 0x01], Ok(&[0x01, 0x01, 0x01])),
        ];
        for &(input, result) in TEST_CASES {
            let input = untrusted::Input::from(input);
            assert_eq!(
                Positive::from_be_bytes(input).map(|p| p.big_endian_without_leading_zero()),
                result
            );
        }
    }
}
