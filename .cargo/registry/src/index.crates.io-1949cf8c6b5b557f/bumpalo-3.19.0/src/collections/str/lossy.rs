// Copyright 2012-2017 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::collections::str as core_str;
use core::char;
use core::fmt;
use core::fmt::Write;
use core::str;

/// Lossy UTF-8 string.
pub struct Utf8Lossy<'a> {
    bytes: &'a [u8],
}

impl<'a> Utf8Lossy<'a> {
    pub fn from_bytes(bytes: &'a [u8]) -> Utf8Lossy<'a> {
        Utf8Lossy { bytes }
    }

    pub fn chunks(&self) -> Utf8LossyChunksIter<'a> {
        Utf8LossyChunksIter {
            source: &self.bytes,
        }
    }
}

/// Iterator over lossy UTF-8 string
#[allow(missing_debug_implementations)]
pub struct Utf8LossyChunksIter<'a> {
    source: &'a [u8],
}

#[derive(PartialEq, Eq, Debug)]
pub struct Utf8LossyChunk<'a> {
    /// Sequence of valid chars.
    /// Can be empty between broken UTF-8 chars.
    pub valid: &'a str,
    /// Single broken char, empty if none.
    /// Empty iff iterator item is last.
    pub broken: &'a [u8],
}

impl<'a> Iterator for Utf8LossyChunksIter<'a> {
    type Item = Utf8LossyChunk<'a>;

    fn next(&mut self) -> Option<Utf8LossyChunk<'a>> {
        if self.source.is_empty() {
            return None;
        }

        const TAG_CONT_U8: u8 = 128;
        fn unsafe_get(xs: &[u8], i: usize) -> u8 {
            unsafe { *xs.get_unchecked(i) }
        }
        fn safe_get(xs: &[u8], i: usize) -> u8 {
            if i >= xs.len() {
                0
            } else {
                unsafe_get(xs, i)
            }
        }

        let mut i = 0;
        while i < self.source.len() {
            let i_ = i;

            let byte = unsafe_get(self.source, i);
            i += 1;

            if byte < 128 {
            } else {
                let w = core_str::utf8_char_width(byte);

                macro_rules! error {
                    () => {{
                        unsafe {
                            let r = Utf8LossyChunk {
                                valid: str::from_utf8_unchecked(&self.source[0..i_]),
                                broken: &self.source[i_..i],
                            };
                            self.source = &self.source[i..];
                            return Some(r);
                        }
                    }};
                }

                match w {
                    2 => {
                        if safe_get(self.source, i) & 192 != TAG_CONT_U8 {
                            error!();
                        }
                        i += 1;
                    }
                    3 => {
                        match (byte, safe_get(self.source, i)) {
                            (0xE0, 0xA0..=0xBF) => (),
                            (0xE1..=0xEC, 0x80..=0xBF) => (),
                            (0xED, 0x80..=0x9F) => (),
                            (0xEE..=0xEF, 0x80..=0xBF) => (),
                            _ => {
                                error!();
                            }
                        }
                        i += 1;
                        if safe_get(self.source, i) & 192 != TAG_CONT_U8 {
                            error!();
                        }
                        i += 1;
                    }
                    4 => {
                        match (byte, safe_get(self.source, i)) {
                            (0xF0, 0x90..=0xBF) => (),
                            (0xF1..=0xF3, 0x80..=0xBF) => (),
                            (0xF4, 0x80..=0x8F) => (),
                            _ => {
                                error!();
                            }
                        }
                        i += 1;
                        if safe_get(self.source, i) & 192 != TAG_CONT_U8 {
                            error!();
                        }
                        i += 1;
                        if safe_get(self.source, i) & 192 != TAG_CONT_U8 {
                            error!();
                        }
                        i += 1;
                    }
                    _ => {
                        error!();
                    }
                }
            }
        }

        let r = Utf8LossyChunk {
            valid: unsafe { str::from_utf8_unchecked(self.source) },
            broken: &[],
        };
        self.source = &[];
        Some(r)
    }
}

impl<'a> fmt::Display for Utf8Lossy<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // If we're the empty string then our iterator won't actually yield
        // anything, so perform the formatting manually
        if self.bytes.is_empty() {
            return "".fmt(f);
        }

        for Utf8LossyChunk { valid, broken } in self.chunks() {
            // If we successfully decoded the whole chunk as a valid string then
            // we can return a direct formatting of the string which will also
            // respect various formatting flags if possible.
            if valid.len() == self.bytes.len() {
                assert!(broken.is_empty());
                return valid.fmt(f);
            }

            f.write_str(valid)?;
            if !broken.is_empty() {
                f.write_char(char::REPLACEMENT_CHARACTER)?;
            }
        }
        Ok(())
    }
}

impl<'a> fmt::Debug for Utf8Lossy<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_char('"')?;

        for Utf8LossyChunk { valid, broken } in self.chunks() {
            // Valid part.
            // Here we partially parse UTF-8 again which is suboptimal.
            {
                let mut from = 0;
                for (i, c) in valid.char_indices() {
                    let esc = c.escape_debug();
                    // If char needs escaping, flush backlog so far and write, else skip
                    if esc.len() != 1 {
                        f.write_str(&valid[from..i])?;
                        for c in esc {
                            f.write_char(c)?;
                        }
                        from = i + c.len_utf8();
                    }
                }
                f.write_str(&valid[from..])?;
            }

            // Broken parts of string as hex escape.
            for &b in broken {
                write!(f, "\\x{:02x}", b)?;
            }
        }

        f.write_char('"')
    }
}
