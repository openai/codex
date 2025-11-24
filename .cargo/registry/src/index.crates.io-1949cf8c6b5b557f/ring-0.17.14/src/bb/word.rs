// Copyright 2015-2024 Brian Smith.
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

use super::LeakyWord;

/// A native word that may hold a secret.
///
/// XXX: Currently this is a type alias of `LeakyWord` so it doesn't enforce,
/// except by convention, the prevention of leaks. This is a temporary state to
/// support the refactorings that will
///
/// XXX: This isn't the native word size on targets where a pointer isn't the
/// same size as a native word. TODO: Fix this.
///
/// XXX: Over time, we'll evolve Word into a newtype with an API that minimizes
/// leaks and makes all leaks explicit, like so:
pub(crate) type Word = LeakyWord;

/* TODO:
#[repr(transparent)]
pub(crate) struct Word(LeakyWord);

impl Word {
    pub fn leak_word(self) -> LeakyWord { self.0 }
}

impl From<LeakyWord> for Word {
    fn from(w: LeakyWord) -> Self {
        // TODO: Use a stronger `black_box`.
        Self(core::hint::black_box(w))
    }
}
*/
