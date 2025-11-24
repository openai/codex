// Copyright 2024 Brian Smith.
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

use super::Word;
use core::ops;

// BoolMask is either `BoolMask::TRUE` or `BoolMask::FALSE`.
#[repr(transparent)]
pub struct BoolMask(Word);

impl BoolMask {
    #[cfg(test)]
    pub(super) const TRUE: Self = Self(Word::MAX);
    #[cfg(test)]
    pub(super) const FALSE: Self = Self(0);

    /// Returns true if `self` is `BoolMask::TRUE`; otherwise, returns false
    /// (`self` is `BoolMask::FALSE`).
    pub(crate) fn leak(self) -> bool {
        self.0 != 0
    }
}

impl ops::BitAnd for BoolMask {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self {
        Self(self.0 & rhs.0)
    }
}
