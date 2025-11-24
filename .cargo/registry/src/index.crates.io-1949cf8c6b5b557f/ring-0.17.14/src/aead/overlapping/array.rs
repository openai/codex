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

#![cfg_attr(not(test), allow(dead_code))]

use super::Overlapping;
use crate::error::LenMismatchError;
use core::array::TryFromSliceError;

pub struct Array<'o, T, const N: usize> {
    // Invariant: N != 0.
    // Invariant: `self.in_out.len() == N`.
    in_out: Overlapping<'o, T>,
}

impl<'o, T, const N: usize> Array<'o, T, N> {
    pub(super) fn new(in_out: Overlapping<'o, T>) -> Result<Self, LenMismatchError> {
        if N == 0 || in_out.len() != N {
            return Err(LenMismatchError::new(N));
        }
        Ok(Self { in_out })
    }

    pub fn into_unwritten_output(self) -> &'o mut [T; N]
    where
        &'o mut [T]: TryInto<&'o mut [T; N], Error = TryFromSliceError>,
    {
        self.in_out
            .into_unwritten_output()
            .try_into()
            .unwrap_or_else(|TryFromSliceError { .. }| {
                unreachable!() // Due to invariant
            })
    }
}

impl<T, const N: usize> Array<'_, T, N> {
    pub fn input<'s>(&'s self) -> &'s [T; N]
    where
        &'s [T]: TryInto<&'s [T; N], Error = TryFromSliceError>,
    {
        self.in_out
            .input()
            .try_into()
            .unwrap_or_else(|TryFromSliceError { .. }| {
                unreachable!() // Due to invariant
            })
    }
}
