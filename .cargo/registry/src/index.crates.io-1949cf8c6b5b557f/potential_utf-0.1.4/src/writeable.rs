// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use crate::{PotentialUtf16, PotentialUtf8};
use core::fmt::Write;
use writeable::{LengthHint, Part, PartsWrite, TryWriteable};

use core::{char::DecodeUtf16Error, fmt, str::Utf8Error};

/// This impl requires enabling the optional `writeable` Cargo feature
impl TryWriteable for &'_ PotentialUtf8 {
    type Error = Utf8Error;

    fn try_write_to_parts<S: PartsWrite + ?Sized>(
        &self,
        sink: &mut S,
    ) -> Result<Result<(), Self::Error>, fmt::Error> {
        let mut remaining = &self.0;
        let mut r = Ok(());
        loop {
            match core::str::from_utf8(remaining) {
                Ok(valid) => {
                    sink.write_str(valid)?;
                    return Ok(r);
                }
                Err(e) => {
                    // SAFETY: By Utf8Error invariants
                    let valid = unsafe {
                        core::str::from_utf8_unchecked(remaining.get_unchecked(..e.valid_up_to()))
                    };
                    sink.write_str(valid)?;
                    sink.with_part(Part::ERROR, |s| s.write_char(char::REPLACEMENT_CHARACTER))?;
                    if r.is_ok() {
                        r = Err(e);
                    }
                    let Some(error_len) = e.error_len() else {
                        return Ok(r); // end of string
                    };
                    // SAFETY: By Utf8Error invariants
                    remaining = unsafe { remaining.get_unchecked(e.valid_up_to() + error_len..) }
                }
            }
        }
    }

    fn writeable_length_hint(&self) -> LengthHint {
        // Lower bound is all valid UTF-8, upper bound is all bytes with the high bit, which become replacement characters.
        LengthHint::between(self.0.len(), self.0.len() * 3)
    }
}

/// This impl requires enabling the optional `writeable` Cargo feature
impl TryWriteable for &'_ PotentialUtf16 {
    type Error = DecodeUtf16Error;

    fn try_write_to_parts<S: PartsWrite + ?Sized>(
        &self,
        sink: &mut S,
    ) -> Result<Result<(), Self::Error>, fmt::Error> {
        let mut r = Ok(());
        for c in core::char::decode_utf16(self.0.iter().copied()) {
            match c {
                Ok(c) => sink.write_char(c)?,
                Err(e) => {
                    if r.is_ok() {
                        r = Err(e);
                    }
                    sink.with_part(Part::ERROR, |s| s.write_char(char::REPLACEMENT_CHARACTER))?;
                }
            }
        }
        Ok(r)
    }

    fn writeable_length_hint(&self) -> LengthHint {
        // Lower bound is all ASCII, upper bound is all 3-byte code points (including replacement character)
        LengthHint::between(self.0.len(), self.0.len() * 3)
    }
}

#[cfg(test)]
mod test {
    #![allow(invalid_from_utf8)] // only way to construct the error
    use super::*;
    use writeable::assert_try_writeable_parts_eq;

    #[test]
    fn test_utf8() {
        assert_try_writeable_parts_eq!(
            PotentialUtf8::from_bytes(b"Foo Bar"),
            "Foo Bar",
            Ok(()),
            []
        );
        assert_try_writeable_parts_eq!(
            PotentialUtf8::from_bytes(b"Foo\xFDBar"),
            "Foo�Bar",
            Err(core::str::from_utf8(b"Foo\xFDBar").unwrap_err()),
            [(3, 6, Part::ERROR)]
        );
        assert_try_writeable_parts_eq!(
            PotentialUtf8::from_bytes(b"Foo\xFDBar\xff"),
            "Foo�Bar�",
            Err(core::str::from_utf8(b"Foo\xFDBar\xff").unwrap_err()),
            [(3, 6, Part::ERROR), (9, 12, Part::ERROR)],
        );
    }

    #[test]
    fn test_utf16() {
        assert_try_writeable_parts_eq!(
            PotentialUtf16::from_slice(&[0xD83E, 0xDD73]),
            "🥳",
            Ok(()),
            []
        );
        assert_try_writeable_parts_eq!(
            PotentialUtf16::from_slice(&[0xD83E, 0x20, 0xDD73]),
            "� �",
            Err(core::char::decode_utf16([0xD83E].into_iter())
                .next()
                .unwrap()
                .unwrap_err()),
            [(0, 3, Part::ERROR), (4, 7, Part::ERROR)]
        );
    }
}
