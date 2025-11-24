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

use super::{der::*, writer::*, *};
use alloc::boxed::Box;

pub(crate) fn write_positive_integer(
    output: &mut dyn Accumulator,
    value: &Positive,
) -> Result<(), TooLongError> {
    let first_byte = value.first_byte();
    let value = value.big_endian_without_leading_zero_as_input();
    write_tlv(output, Tag::Integer, |output| {
        if (first_byte & 0x80) != 0 {
            output.write_byte(0)?; // Disambiguate negative number.
        }
        write_copy(output, value)
    })
}

pub(crate) fn write_all(
    tag: Tag,
    write_value: &dyn Fn(&mut dyn Accumulator) -> Result<(), TooLongError>,
) -> Result<Box<[u8]>, TooLongError> {
    let length = {
        let mut length = LengthMeasurement::zero();
        write_tlv(&mut length, tag, write_value)?;
        length
    };

    let mut output = Writer::with_capacity(length);
    write_tlv(&mut output, tag, write_value)?;

    Ok(output.into())
}

fn write_tlv<F>(output: &mut dyn Accumulator, tag: Tag, write_value: F) -> Result<(), TooLongError>
where
    F: Fn(&mut dyn Accumulator) -> Result<(), TooLongError>,
{
    let length: usize = {
        let mut length = LengthMeasurement::zero();
        write_value(&mut length)?;
        length.into()
    };
    let length: u16 = length.try_into().map_err(|_| TooLongError::new())?;

    output.write_byte(tag.into())?;

    let [lo, hi] = length.to_le_bytes();
    if length >= 0x1_00 {
        output.write_byte(0x82)?;
        output.write_byte(hi)?;
    } else if length >= 0x80 {
        output.write_byte(0x81)?;
    }
    output.write_byte(lo)?;

    write_value(output)
}
