/*!
Provides convenience routines for escaping raw bytes.

Since this crate tends to deal with `&[u8]` everywhere and the default
`Debug` implementation just shows decimal integers, it makes debugging those
representations quite difficult. This module provides types that show `&[u8]`
as if it were a string, with invalid UTF-8 escaped into its byte-by-byte hex
representation.
*/

use crate::util::utf8;

/// Provides a convenient `Debug` implementation for a `u8`.
///
/// The `Debug` impl treats the byte as an ASCII, and emits a human readable
/// representation of it. If the byte isn't ASCII, then it's emitted as a hex
/// escape sequence.
#[derive(Clone, Copy)]
pub struct DebugByte(pub u8);

impl core::fmt::Debug for DebugByte {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        // Special case ASCII space. It's too hard to read otherwise, so
        // put quotes around it. I sometimes wonder whether just '\x20' would
        // be better...
        if self.0 == b' ' {
            return write!(f, "' '");
        }
        // 10 bytes is enough to cover any output from ascii::escape_default.
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

/// Provides a convenient `Debug` implementation for `&[u8]`.
///
/// This generally works best when the bytes are presumed to be mostly UTF-8,
/// but will work for anything. For any bytes that aren't UTF-8, they are
/// emitted as hex escape sequences.
pub struct DebugHaystack<'a>(pub &'a [u8]);

impl<'a> core::fmt::Debug for DebugHaystack<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "\"")?;
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
                // ASCII control characters except \0, \n, \r, \t
                '\x01'..='\x08'
                | '\x0b'
                | '\x0c'
                | '\x0e'..='\x19'
                | '\x7f' => {
                    write!(f, "\\x{:02x}", u32::from(ch))?;
                }
                '\n' | '\r' | '\t' | _ => {
                    write!(f, "{}", ch.escape_debug())?;
                }
            }
        }
        write!(f, "\"")?;
        Ok(())
    }
}
