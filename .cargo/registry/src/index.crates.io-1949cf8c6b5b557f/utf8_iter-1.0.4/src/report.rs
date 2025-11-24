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

use crate::in_inclusive_range8;
use crate::UTF8_DATA;
use core::fmt::Formatter;
use core::iter::FusedIterator;

/// A type for signaling UTF-8 errors.
///
/// Note: `core::error::Error` is not implemented due to implementing it
/// being an [unstable feature][1] at the time of writing.
///
/// [1]: https://github.com/rust-lang/rust/issues/103765
#[derive(Debug, PartialEq)]
#[non_exhaustive]
pub struct Utf8CharsError;

impl core::fmt::Display for Utf8CharsError {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        write!(f, "byte sequence not well-formed UTF-8")
    }
}

/// Iterator by `Result<char,Utf8CharsError>` over `&[u8]` that contains
/// potentially-invalid UTF-8. There is exactly one `Utf8CharsError` per
/// each error as defined by the WHATWG Encoding Standard.
///
/// ```
/// let s = b"a\xFFb\xFF\x80c\xF0\x9F\xA4\xA6\xF0\x9F\xA4\xF0\x9F\xF0d";
/// let plain = utf8_iter::Utf8Chars::new(s);
/// let reporting = utf8_iter::ErrorReportingUtf8Chars::new(s);
/// assert!(plain.eq(reporting.map(|r| r.unwrap_or('\u{FFFD}'))));
/// ```
#[derive(Debug, Clone)]
pub struct ErrorReportingUtf8Chars<'a> {
    remaining: &'a [u8],
}

impl<'a> ErrorReportingUtf8Chars<'a> {
    #[inline(always)]
    /// Creates the iterator from a byte slice.
    pub fn new(bytes: &'a [u8]) -> Self {
        ErrorReportingUtf8Chars::<'a> { remaining: bytes }
    }

    /// Views the current remaining data in the iterator as a subslice
    /// of the original slice.
    #[inline(always)]
    pub fn as_slice(&self) -> &'a [u8] {
        self.remaining
    }

    #[inline(never)]
    fn next_fallback(&mut self) -> Option<Result<char, Utf8CharsError>> {
        if self.remaining.is_empty() {
            return None;
        }
        let first = self.remaining[0];
        if first < 0x80 {
            self.remaining = &self.remaining[1..];
            return Some(Ok(char::from(first)));
        }
        if !in_inclusive_range8(first, 0xC2, 0xF4) || self.remaining.len() == 1 {
            self.remaining = &self.remaining[1..];
            return Some(Err(Utf8CharsError));
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
            return Some(Err(Utf8CharsError));
        }
        if first < 0xE0 {
            self.remaining = &self.remaining[2..];
            let point = ((u32::from(first) & 0x1F) << 6) | (u32::from(second) & 0x3F);
            return Some(Ok(unsafe { char::from_u32_unchecked(point) }));
        }
        if self.remaining.len() == 2 {
            self.remaining = &self.remaining[2..];
            return Some(Err(Utf8CharsError));
        }
        let third = self.remaining[2];
        if !in_inclusive_range8(third, 0x80, 0xBF) {
            self.remaining = &self.remaining[2..];
            return Some(Err(Utf8CharsError));
        }
        if first < 0xF0 {
            self.remaining = &self.remaining[3..];
            let point = ((u32::from(first) & 0xF) << 12)
                | ((u32::from(second) & 0x3F) << 6)
                | (u32::from(third) & 0x3F);
            return Some(Ok(unsafe { char::from_u32_unchecked(point) }));
        }
        // At this point, we have a valid 3-byte prefix of a
        // four-byte sequence that has to be incomplete, because
        // otherwise `next()` would have succeeded.
        self.remaining = &self.remaining[3..];
        Some(Err(Utf8CharsError))
    }
}

impl<'a> Iterator for ErrorReportingUtf8Chars<'a> {
    type Item = Result<char, Utf8CharsError>;

    #[inline]
    fn next(&mut self) -> Option<Result<char, Utf8CharsError>> {
        // This loop is only broken out of as goto forward
        #[allow(clippy::never_loop)]
        loop {
            if self.remaining.len() < 4 {
                break;
            }
            let first = self.remaining[0];
            if first < 0x80 {
                self.remaining = &self.remaining[1..];
                return Some(Ok(char::from(first)));
            }
            let second = self.remaining[1];
            if in_inclusive_range8(first, 0xC2, 0xDF) {
                if !in_inclusive_range8(second, 0x80, 0xBF) {
                    break;
                }
                let point = ((u32::from(first) & 0x1F) << 6) | (u32::from(second) & 0x3F);
                self.remaining = &self.remaining[2..];
                return Some(Ok(unsafe { char::from_u32_unchecked(point) }));
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
                return Some(Ok(unsafe { char::from_u32_unchecked(point) }));
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
            return Some(Ok(unsafe { char::from_u32_unchecked(point) }));
        }
        self.next_fallback()
    }
}

impl<'a> DoubleEndedIterator for ErrorReportingUtf8Chars<'a> {
    #[inline]
    fn next_back(&mut self) -> Option<Result<char, Utf8CharsError>> {
        if self.remaining.is_empty() {
            return None;
        }
        let mut attempt = 1;
        for b in self.remaining.iter().rev() {
            if b & 0xC0 != 0x80 {
                let (head, tail) = self.remaining.split_at(self.remaining.len() - attempt);
                let mut inner = ErrorReportingUtf8Chars::new(tail);
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
        Some(Err(Utf8CharsError))
    }
}

impl FusedIterator for ErrorReportingUtf8Chars<'_> {}

#[cfg(test)]
mod tests {
    use crate::ErrorReportingUtf8Chars;

    // Should be a static assert, but not taking a dependency for this.
    #[test]
    fn test_size() {
        assert_eq!(
            core::mem::size_of::<Option<<ErrorReportingUtf8Chars<'_> as Iterator>::Item>>(),
            core::mem::size_of::<Option<char>>()
        );
    }

    #[test]
    fn test_eq() {
        let a: <ErrorReportingUtf8Chars<'_> as Iterator>::Item = Ok('a');
        let a_again: <ErrorReportingUtf8Chars<'_> as Iterator>::Item = Ok('a');
        assert_eq!(a, a_again);
    }
}
