// Copyright Mozilla Foundation
//
// Licensed under the Apache License (Version 2.0), or the MIT license,
// (the "Licenses") at your option. You may not use this file except in
// compliance with one of the Licenses. You may obtain copies of the
// Licenses at:
//
//    https://www.apache.org/licenses/LICENSE-2.0
//    https://opensource.org/licenses/MIT
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the Licenses is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the Licenses for the specific language governing permissions and
// limitations under the Licenses.

#![no_std]

//! Provides iteration by `char` over `&[u8]` containing potentially-invalid
//! UTF-8 such that errors are handled according to the [WHATWG Encoding
//! Standard](https://encoding.spec.whatwg.org/#utf-8-decoder) (i.e. the same
//! way as in `String::from_utf8_lossy`).
//!
//! The trait `Utf8CharsEx` provides the convenience method `chars()` on
//! byte slices themselves instead of having to use the more verbose
//! `Utf8Chars::new(slice)`.
//!
//! ```rust
//! use utf8_iter::Utf8CharsEx;
//! let data = b"\xFF\xC2\xE2\xE2\x98\xF0\xF0\x9F\xF0\x9F\x92\xE2\x98\x83";
//! let from_iter: String = data.chars().collect();
//! let from_std = String::from_utf8_lossy(data);
//! assert_eq!(from_iter, from_std);
//! ```

mod indices;
mod report;

pub use crate::indices::Utf8CharIndices;
pub use crate::report::ErrorReportingUtf8Chars;
pub use crate::report::Utf8CharsError;
use core::iter::FusedIterator;

#[repr(align(64))] // Align to cache lines
struct Utf8Data {
    pub table: [u8; 384],
}

// This is generated code copied and pasted from utf_8.rs of encoding_rs.
// Please don't edit by hand but instead regenerate as instructed in that
// file.

static UTF8_DATA: Utf8Data = Utf8Data {
    table: [
        252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252,
        252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252,
        252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252,
        252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252,
        252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252,
        252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252,
        252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252,
        252, 252, 84, 84, 84, 84, 84, 84, 84, 84, 84, 84, 84, 84, 84, 84, 84, 84, 148, 148, 148,
        148, 148, 148, 148, 148, 148, 148, 148, 148, 148, 148, 148, 148, 164, 164, 164, 164, 164,
        164, 164, 164, 164, 164, 164, 164, 164, 164, 164, 164, 164, 164, 164, 164, 164, 164, 164,
        164, 164, 164, 164, 164, 164, 164, 164, 164, 252, 252, 252, 252, 252, 252, 252, 252, 252,
        252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252,
        252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252,
        252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252,
        252, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
        4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
        4, 4, 4, 4, 4, 4, 4, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8,
        8, 8, 8, 8, 8, 8, 8, 16, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 32, 8, 8, 64, 8, 8, 8, 128, 4,
        4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
    ],
};

// End manually copypasted generated code.

#[inline(always)]
fn in_inclusive_range8(i: u8, start: u8, end: u8) -> bool {
    i.wrapping_sub(start) <= (end - start)
}

/// Iterator by `char` over `&[u8]` that contains
/// potentially-invalid UTF-8. See the crate documentation.
#[derive(Debug, Clone)]
pub struct Utf8Chars<'a> {
    remaining: &'a [u8],
}

impl<'a> Utf8Chars<'a> {
    #[inline(always)]
    /// Creates the iterator from a byte slice.
    pub fn new(bytes: &'a [u8]) -> Self {
        Utf8Chars::<'a> { remaining: bytes }
    }

    /// Views the current remaining data in the iterator as a subslice
    /// of the original slice.
    #[inline(always)]
    pub fn as_slice(&self) -> &'a [u8] {
        self.remaining
    }

    #[inline(never)]
    fn next_fallback(&mut self) -> Option<char> {
        if self.remaining.is_empty() {
            return None;
        }
        let first = self.remaining[0];
        if first < 0x80 {
            self.remaining = &self.remaining[1..];
            return Some(char::from(first));
        }
        if !in_inclusive_range8(first, 0xC2, 0xF4) || self.remaining.len() == 1 {
            self.remaining = &self.remaining[1..];
            return Some('\u{FFFD}');
        }
        let second = self.remaining[1];
        let (lower_bound, upper_bound) = match first {
            0xE0 => (0xA0, 0xBF),
            0xED => (0x80, 0x9F),
            0xF0 => (0x90, 0xBF),
            0xF4 => (0x80, 0x8F),
            _ => (0x80, 0xBF),
        };
        if !in_inclusive_range8(second, lower_bound, upper_bound) {
            self.remaining = &self.remaining[1..];
            return Some('\u{FFFD}');
        }
        if first < 0xE0 {
            self.remaining = &self.remaining[2..];
            let point = ((u32::from(first) & 0x1F) << 6) | (u32::from(second) & 0x3F);
            return Some(unsafe { char::from_u32_unchecked(point) });
        }
        if self.remaining.len() == 2 {
            self.remaining = &self.remaining[2..];
            return Some('\u{FFFD}');
        }
        let third = self.remaining[2];
        if !in_inclusive_range8(third, 0x80, 0xBF) {
            self.remaining = &self.remaining[2..];
            return Some('\u{FFFD}');
        }
        if first < 0xF0 {
            self.remaining = &self.remaining[3..];
            let point = ((u32::from(first) & 0xF) << 12)
                | ((u32::from(second) & 0x3F) << 6)
                | (u32::from(third) & 0x3F);
            return Some(unsafe { char::from_u32_unchecked(point) });
        }
        // At this point, we have a valid 3-byte prefix of a
        // four-byte sequence that has to be incomplete, because
        // otherwise `next()` would have succeeded.
        self.remaining = &self.remaining[3..];
        Some('\u{FFFD}')
    }
}

impl<'a> Iterator for Utf8Chars<'a> {
    type Item = char;

    #[inline]
    fn next(&mut self) -> Option<char> {
        // Not delegating directly to `ErrorReportingUtf8Chars` to avoid
        // an extra branch in the common case based on a cursory inspection
        // of generated code in a similar case. Be sure to inspect the
        // generated code as inlined into an actual usage site carefully
        // if attempting to consolidate the source code here.

        // This loop is only broken out of as goto forward
        #[allow(clippy::never_loop)]
        loop {
            if self.remaining.len() < 4 {
                break;
            }
            let first = self.remaining[0];
            if first < 0x80 {
                self.remaining = &self.remaining[1..];
                return Some(char::from(first));
            }
            let second = self.remaining[1];
            if in_inclusive_range8(first, 0xC2, 0xDF) {
                if !in_inclusive_range8(second, 0x80, 0xBF) {
                    break;
                }
                let point = ((u32::from(first) & 0x1F) << 6) | (u32::from(second) & 0x3F);
                self.remaining = &self.remaining[2..];
                return Some(unsafe { char::from_u32_unchecked(point) });
            }
            // This table-based formulation was benchmark-based in encoding_rs,
            // but it hasn't been re-benchmarked in this iterator context.
            let third = self.remaining[2];
            if first < 0xF0 {
                if ((UTF8_DATA.table[usize::from(second)]
                    & UTF8_DATA.table[usize::from(first) + 0x80])
                    | (third >> 6))
                    != 2
                {
                    break;
                }
                let point = ((u32::from(first) & 0xF) << 12)
                    | ((u32::from(second) & 0x3F) << 6)
                    | (u32::from(third) & 0x3F);
                self.remaining = &self.remaining[3..];
                return Some(unsafe { char::from_u32_unchecked(point) });
            }
            let fourth = self.remaining[3];
            if (u16::from(
                UTF8_DATA.table[usize::from(second)] & UTF8_DATA.table[usize::from(first) + 0x80],
            ) | u16::from(third >> 6)
                | (u16::from(fourth & 0xC0) << 2))
                != 0x202
            {
                break;
            }
            let point = ((u32::from(first) & 0x7) << 18)
                | ((u32::from(second) & 0x3F) << 12)
                | ((u32::from(third) & 0x3F) << 6)
                | (u32::from(fourth) & 0x3F);
            self.remaining = &self.remaining[4..];
            return Some(unsafe { char::from_u32_unchecked(point) });
        }
        self.next_fallback()
    }
}

impl<'a> DoubleEndedIterator for Utf8Chars<'a> {
    #[inline]
    fn next_back(&mut self) -> Option<char> {
        if self.remaining.is_empty() {
            return None;
        }
        let mut attempt = 1;
        for b in self.remaining.iter().rev() {
            if b & 0xC0 != 0x80 {
                let (head, tail) = self.remaining.split_at(self.remaining.len() - attempt);
                let mut inner = Utf8Chars::new(tail);
                let candidate = inner.next();
                if inner.as_slice().is_empty() {
                    self.remaining = head;
                    return candidate;
                }
                break;
            }
            if attempt == 4 {
                break;
            }
            attempt += 1;
        }

        self.remaining = &self.remaining[..self.remaining.len() - 1];
        Some('\u{FFFD}')
    }
}

impl FusedIterator for Utf8Chars<'_> {}

/// Convenience trait that adds `chars()` and `char_indices()` methods
/// similar to the ones on string slices to byte slices.
pub trait Utf8CharsEx {
    fn chars(&self) -> Utf8Chars<'_>;
    fn char_indices(&self) -> Utf8CharIndices<'_>;
}

impl Utf8CharsEx for [u8] {
    /// Convenience method for creating an UTF-8 iterator
    /// for the slice.
    #[inline]
    fn chars(&self) -> Utf8Chars<'_> {
        Utf8Chars::new(self)
    }
    /// Convenience method for creating a byte index and
    /// UTF-8 iterator for the slice.
    #[inline]
    fn char_indices(&self) -> Utf8CharIndices<'_> {
        Utf8CharIndices::new(self)
    }
}

// No manually-written tests for forward-iteration, because the code passed multiple
// days of fuzzing comparing with known-good behavior.
