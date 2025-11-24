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

use super::Overlapping;
use crate::error::InputTooLongError;

pub struct PartialBlock<'i, T, const BLOCK_LEN: usize> {
    // invariant: `self.in_out.len() < BLOCK_LEN`.
    in_out: Overlapping<'i, T>,
}

impl<'i, T, const BLOCK_LEN: usize> PartialBlock<'i, T, BLOCK_LEN> {
    pub fn new(in_out: Overlapping<'i, T>) -> Result<Self, InputTooLongError> {
        let len = in_out.len();
        if len >= BLOCK_LEN {
            return Err(InputTooLongError::new(len));
        }
        Ok(Self { in_out })
    }

    pub fn overwrite_at_start(self, padded: [T; BLOCK_LEN])
    where
        T: Copy,
    {
        let len = self.len();
        let output = self.in_out.into_unwritten_output();
        assert!(output.len() <= padded.len());
        output.copy_from_slice(&padded[..len]);
    }
}

impl<T, const BLOCK_LEN: usize> PartialBlock<'_, T, BLOCK_LEN> {
    #[inline(always)]
    pub fn input(&self) -> &[T] {
        let r = self.in_out.input();
        // Help the optimizer optimize the caller using the invariant.
        // TODO: Does this actually help?
        if r.len() >= BLOCK_LEN {
            unreachable!()
        }
        r
    }

    #[inline(always)]
    pub fn len(&self) -> usize {
        self.input().len()
    }
}
