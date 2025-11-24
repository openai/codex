// Copyright 2015-2021 Brian Smith.
//
// Permission to use, copy, modify, and/or distribute this software for any
// purpose with or without fee is hereby granted, provided that the above
// copyright notice and this permission notice appear in all copies.
//
// THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHORS DISCLAIM ALL WARRANTIES
// WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
// MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHORS BE LIABLE FOR
// ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
// WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN
// ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF
// OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.

use crate::{no_panic, Input};

/// A read-only, forward-only cursor into the data in an `Input`.
///
/// Using `Reader` to parse input helps to ensure that no byte of the input
/// will be accidentally processed more than once. Using `Reader` in
/// conjunction with `read_all` and `read_all_optional` helps ensure that no
/// byte of the input is accidentally left unprocessed. The methods of `Reader`
/// never panic, so `Reader` also assists the writing of panic-free code.
///
/// Intentionally avoids implementing `PartialEq` and `Eq` to avoid implicit
/// non-constant-time comparisons.
pub struct Reader<'a> {
    input: no_panic::Slice<'a>,
    i: usize,
}

/// Avoids writing the value or position to avoid creating a side channel,
/// though `Reader` can't avoid leaking the position via timing.
impl core::fmt::Debug for Reader<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Reader").finish()
    }
}

impl<'a> Reader<'a> {
    /// Construct a new Reader for the given input. Use `read_all` or
    /// `read_all_optional` instead of `Reader::new` whenever possible.
    #[inline]
    pub fn new(input: Input<'a>) -> Self {
        Self {
            input: input.into_value(),
            i: 0,
        }
    }

    /// Returns `true` if the reader is at the end of the input, and `false`
    /// otherwise.
    #[inline]
    pub fn at_end(&self) -> bool {
        self.i == self.input.len()
    }

    /// Returns `true` if there is at least one more byte in the input and that
    /// byte is equal to `b`, and false otherwise.
    #[inline]
    pub fn peek(&self, b: u8) -> bool {
        match self.input.get(self.i) {
            Some(actual_b) => b == *actual_b,
            None => false,
        }
    }

    /// Reads the next input byte.
    ///
    /// Returns `Ok(b)` where `b` is the next input byte, or `Err(EndOfInput)`
    /// if the `Reader` is at the end of the input.
    #[inline]
    pub fn read_byte(&mut self) -> Result<u8, EndOfInput> {
        match self.input.get(self.i) {
            Some(b) => {
                self.i += 1; // safe from overflow; see Input::from().
                Ok(*b)
            }
            None => Err(EndOfInput),
        }
    }

    /// Skips `num_bytes` of the input, returning the skipped input as an
    /// `Input`.
    ///
    /// Returns `Ok(i)` if there are at least `num_bytes` of input remaining,
    /// and `Err(EndOfInput)` otherwise.
    #[inline]
    pub fn read_bytes(&mut self, num_bytes: usize) -> Result<Input<'a>, EndOfInput> {
        let new_i = self.i.checked_add(num_bytes).ok_or(EndOfInput)?;
        let ret = self
            .input
            .subslice(self.i..new_i)
            .map(From::from)
            .ok_or(EndOfInput)?;
        self.i = new_i;
        Ok(ret)
    }

    /// Skips the reader to the end of the input, returning the skipped input
    /// as an `Input`.
    #[inline]
    pub fn read_bytes_to_end(&mut self) -> Input<'a> {
        let to_skip = self.input.len() - self.i;
        self.read_bytes(to_skip).unwrap()
    }

    /// Calls `read()` with the given input as a `Reader`. On success, returns a
    /// pair `(bytes_read, r)` where `bytes_read` is what `read()` consumed and
    /// `r` is `read()`'s return value.
    pub fn read_partial<F, R, E>(&mut self, read: F) -> Result<(Input<'a>, R), E>
    where
        F: FnOnce(&mut Reader<'a>) -> Result<R, E>,
    {
        let start = self.i;
        let r = read(self)?;
        let bytes_read = self.input.subslice(start..self.i).unwrap().into();
        Ok((bytes_read, r))
    }

    /// Skips `num_bytes` of the input.
    ///
    /// Returns `Ok(i)` if there are at least `num_bytes` of input remaining,
    /// and `Err(EndOfInput)` otherwise.
    #[inline]
    pub fn skip(&mut self, num_bytes: usize) -> Result<(), EndOfInput> {
        self.read_bytes(num_bytes).map(|_| ())
    }

    /// Skips the reader to the end of the input.
    #[inline]
    pub fn skip_to_end(&mut self) {
        let _ = self.read_bytes_to_end();
    }
}

/// The error type used to indicate the end of the input was reached before the
/// operation could be completed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EndOfInput;
