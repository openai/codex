// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

#![allow(clippy::upper_case_acronyms)]
//! ULE implementation for the `char` type.

use super::*;
use crate::impl_ule_from_array;
use core::cmp::Ordering;
use core::convert::TryFrom;

/// A u8 array of little-endian data corresponding to a Unicode scalar value.
///
/// The bytes of a `CharULE` are guaranteed to represent a little-endian-encoded u32 that is a
/// valid `char` and can be converted without validation.
///
/// # Examples
///
/// Convert a `char` to a `CharULE` and back again:
///
/// ```
/// use zerovec::ule::{AsULE, CharULE, ULE};
///
/// let c1 = '𑄃';
/// let ule = c1.to_unaligned();
/// assert_eq!(CharULE::slice_as_bytes(&[ule]), &[0x03, 0x11, 0x01]);
/// let c2 = char::from_unaligned(ule);
/// assert_eq!(c1, c2);
/// ```
///
/// Attempt to parse invalid bytes to a `CharULE`:
///
/// ```
/// use zerovec::ule::{CharULE, ULE};
///
/// let bytes: &[u8] = &[0xFF, 0xFF, 0xFF, 0xFF];
/// CharULE::parse_bytes_to_slice(bytes).expect_err("Invalid bytes");
/// ```
#[repr(transparent)]
#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct CharULE([u8; 3]);

impl CharULE {
    /// Converts a [`char`] to a [`CharULE`]. This is equivalent to calling
    /// [`AsULE::to_unaligned()`]
    ///
    /// See the type-level documentation for [`CharULE`] for more information.
    #[inline]
    pub const fn from_aligned(c: char) -> Self {
        let [u0, u1, u2, _u3] = (c as u32).to_le_bytes();
        Self([u0, u1, u2])
    }

    /// Converts this [`CharULE`] to a [`char`]. This is equivalent to calling
    /// [`AsULE::from_unaligned`]
    ///
    /// See the type-level documentation for [`CharULE`] for more information.
    #[inline]
    pub fn to_char(self) -> char {
        let [b0, b1, b2] = self.0;
        // Safe because the bytes of CharULE are defined to represent a valid Unicode scalar value.
        unsafe { char::from_u32_unchecked(u32::from_le_bytes([b0, b1, b2, 0])) }
    }

    impl_ule_from_array!(char, CharULE, Self([0; 3]));
}

// Safety (based on the safety checklist on the ULE trait):
//  1. CharULE does not include any uninitialized or padding bytes.
//     (achieved by `#[repr(transparent)]` on a type that satisfies this invariant)
//  2. CharULE is aligned to 1 byte.
//     (achieved by `#[repr(transparent)]` on a type that satisfies this invariant)
//  3. The impl of validate_bytes() returns an error if any byte is not valid.
//  4. The impl of validate_bytes() returns an error if there are extra bytes.
//  5. The other ULE methods use the default impl.
//  6. CharULE byte equality is semantic equality
unsafe impl ULE for CharULE {
    #[inline]
    fn validate_bytes(bytes: &[u8]) -> Result<(), UleError> {
        if bytes.len() % 3 != 0 {
            return Err(UleError::length::<Self>(bytes.len()));
        }
        // Validate the bytes
        for chunk in bytes.chunks_exact(3) {
            // TODO: Use slice::as_chunks() when stabilized
            #[expect(clippy::indexing_slicing)]
            // Won't panic because the chunks are always 3 bytes long
            let u = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], 0]);
            char::try_from(u).map_err(|_| UleError::parse::<Self>())?;
        }
        Ok(())
    }
}

impl AsULE for char {
    type ULE = CharULE;

    #[inline]
    fn to_unaligned(self) -> Self::ULE {
        CharULE::from_aligned(self)
    }

    #[inline]
    fn from_unaligned(unaligned: Self::ULE) -> Self {
        unaligned.to_char()
    }
}

impl PartialOrd for CharULE {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for CharULE {
    fn cmp(&self, other: &Self) -> Ordering {
        char::from_unaligned(*self).cmp(&char::from_unaligned(*other))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_from_array() {
        const CHARS: [char; 2] = ['a', '🙃'];
        const CHARS_ULE: [CharULE; 2] = CharULE::from_array(CHARS);
        assert_eq!(
            CharULE::slice_as_bytes(&CHARS_ULE),
            &[0x61, 0x00, 0x00, 0x43, 0xF6, 0x01]
        );
    }

    #[test]
    fn test_from_array_zst() {
        const CHARS: [char; 0] = [];
        const CHARS_ULE: [CharULE; 0] = CharULE::from_array(CHARS);
        let bytes = CharULE::slice_as_bytes(&CHARS_ULE);
        let empty: &[u8] = &[];
        assert_eq!(bytes, empty);
    }

    #[test]
    fn test_parse() {
        // 1-byte, 2-byte, 3-byte, and two 4-byte character in UTF-8 (not as relevant in UTF-32)
        let chars = ['w', 'ω', '文', '𑄃', '🙃'];
        let char_ules: Vec<CharULE> = chars.iter().copied().map(char::to_unaligned).collect();
        let char_bytes: &[u8] = CharULE::slice_as_bytes(&char_ules);

        // Check parsing
        let parsed_ules: &[CharULE] = CharULE::parse_bytes_to_slice(char_bytes).unwrap();
        assert_eq!(char_ules, parsed_ules);
        let parsed_chars: Vec<char> = parsed_ules
            .iter()
            .copied()
            .map(char::from_unaligned)
            .collect();
        assert_eq!(&chars, parsed_chars.as_slice());

        // Compare to golden expected data
        assert_eq!(
            &[119, 0, 0, 201, 3, 0, 135, 101, 0, 3, 17, 1, 67, 246, 1],
            char_bytes
        );
    }

    #[test]
    fn test_failures() {
        // 119 and 120 are valid, but not 0xD800 (high surrogate)
        let u32s = [119, 0xD800, 120];
        let u32_ules: Vec<RawBytesULE<4>> = u32s
            .iter()
            .copied()
            .map(<u32 as AsULE>::to_unaligned)
            .collect();
        let u32_bytes: &[u8] = RawBytesULE::<4>::slice_as_bytes(&u32_ules);
        let parsed_ules_result = CharULE::parse_bytes_to_slice(u32_bytes);
        assert!(parsed_ules_result.is_err());

        // 0x20FFFF is out of range for a char
        let u32s = [0x20FFFF];
        let u32_ules: Vec<RawBytesULE<4>> = u32s
            .iter()
            .copied()
            .map(<u32 as AsULE>::to_unaligned)
            .collect();
        let u32_bytes: &[u8] = RawBytesULE::<4>::slice_as_bytes(&u32_ules);
        let parsed_ules_result = CharULE::parse_bytes_to_slice(u32_bytes);
        assert!(parsed_ules_result.is_err());
    }
}
