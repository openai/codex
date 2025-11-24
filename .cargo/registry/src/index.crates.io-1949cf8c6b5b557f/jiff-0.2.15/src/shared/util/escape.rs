/*!
Provides convenience routines for escaping raw bytes.

This was copied from `regex-automata` with a few light edits.
*/

use super::utf8;

/// Provides a convenient `Debug` implementation for a `u8`.
///
/// The `Debug` impl treats the byte as an ASCII, and emits a human
/// readable representation of it. If the byte isn't ASCII, then it's
/// emitted as a hex escape sequence.
#[derive(Clone, Copy)]
pub(crate) struct Byte(pub u8);

impl core::fmt::Display for Byte {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        if self.0 == b' ' {
            return write!(f, " ");
        }
        // 10 bytes is enough for any output from ascii::escape_default.
        let mut bytes = [0u8; 10];
        let mut len = 0;
        for (i, mut b) in core::ascii::escape_default(self.0).enumerate() {
            // capitalize \xab to \xAB
            if i >= 2 && b'a' <= b && b <= b'f' {
                b -= 32;
            }
            bytes[len] = b;
            len += 1;
        }
        write!(f, "{}", core::str::from_utf8(&bytes[..len]).unwrap())
    }
}

impl core::fmt::Debug for Byte {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "\"")?;
        core::fmt::Display::fmt(self, f)?;
        write!(f, "\"")?;
        Ok(())
    }
}

/// Provides a convenient `Debug` implementation for `&[u8]`.
///
/// This generally works best when the bytes are presumed to be mostly
/// UTF-8, but will work for anything. For any bytes that aren't UTF-8,
/// they are emitted as hex escape sequences.
#[derive(Clone, Copy)]
pub(crate) struct Bytes<'a>(pub &'a [u8]);

impl<'a> core::fmt::Display for Bytes<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        // This is a sad re-implementation of a similar impl found in bstr.
        let mut bytes = self.0;
        while let Some(result) = utf8::decode(bytes) {
            let ch = match result {
                Ok(ch) => ch,
                Err(byte) => {
                    write!(f, r"\x{:02x}", byte)?;
                    bytes = &bytes[1..];
                    continue;
                }
            };
            bytes = &bytes[ch.len_utf8()..];
            match ch {
                '\0' => write!(f, "\\0")?,
                '\x01'..='\x7f' => {
                    write!(f, "{}", (ch as u8).escape_ascii())?;
                }
                _ => write!(f, "{}", ch.escape_debug())?,
            }
        }
        Ok(())
    }
}

impl<'a> core::fmt::Debug for Bytes<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "\"")?;
        core::fmt::Display::fmt(self, f)?;
        write!(f, "\"")?;
        Ok(())
    }
}
