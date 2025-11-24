// Copyright 2018 Brian Smith.
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

use alloc::{boxed::Box, vec::Vec};

pub trait Accumulator {
    fn write_byte(&mut self, value: u8) -> Result<(), TooLongError>;
    fn write_bytes(&mut self, value: &[u8]) -> Result<(), TooLongError>;
}

pub(super) struct LengthMeasurement {
    len: usize,
}

impl From<LengthMeasurement> for usize {
    fn from(len: LengthMeasurement) -> usize {
        len.len
    }
}

impl LengthMeasurement {
    pub fn zero() -> Self {
        Self { len: 0 }
    }
}

impl Accumulator for LengthMeasurement {
    fn write_byte(&mut self, _value: u8) -> Result<(), TooLongError> {
        self.len = self.len.checked_add(1).ok_or_else(TooLongError::new)?;
        Ok(())
    }
    fn write_bytes(&mut self, value: &[u8]) -> Result<(), TooLongError> {
        self.len = self
            .len
            .checked_add(value.len())
            .ok_or_else(TooLongError::new)?;
        Ok(())
    }
}

pub(super) struct Writer {
    bytes: Vec<u8>,
    requested_capacity: usize,
}

impl Writer {
    pub(super) fn with_capacity(capacity: LengthMeasurement) -> Self {
        Self {
            bytes: Vec::with_capacity(capacity.len),
            requested_capacity: capacity.len,
        }
    }
}

impl From<Writer> for Box<[u8]> {
    fn from(writer: Writer) -> Self {
        assert_eq!(writer.requested_capacity, writer.bytes.len());
        writer.bytes.into_boxed_slice()
    }
}

impl Accumulator for Writer {
    fn write_byte(&mut self, value: u8) -> Result<(), TooLongError> {
        self.bytes.push(value);
        Ok(())
    }
    fn write_bytes(&mut self, value: &[u8]) -> Result<(), TooLongError> {
        self.bytes.extend(value);
        Ok(())
    }
}

pub fn write_copy(
    accumulator: &mut dyn Accumulator,
    to_copy: untrusted::Input,
) -> Result<(), TooLongError> {
    accumulator.write_bytes(to_copy.as_slice_less_safe())
}

pub struct TooLongError(());

impl TooLongError {
    pub fn new() -> Self {
        Self(())
    }
}
