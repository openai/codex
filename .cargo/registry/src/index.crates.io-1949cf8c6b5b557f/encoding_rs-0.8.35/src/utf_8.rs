// Copyright Mozilla Foundation. See the COPYRIGHT
// file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use super::*;
use crate::ascii::ascii_to_basic_latin;
use crate::ascii::basic_latin_to_ascii;
use crate::ascii::validate_ascii;
use crate::handles::*;
use crate::mem::convert_utf16_to_utf8_partial;
use crate::variant::*;

cfg_if! {
    if #[cfg(feature = "simd-accel")] {
        use ::core::intrinsics::unlikely;
        use ::core::intrinsics::likely;
    } else {
        #[inline(always)]
        fn unlikely(b: bool) -> bool {
            b
        }
        #[inline(always)]
        fn likely(b: bool) -> bool {
            b
        }
    }
}

#[repr(align(64))] // Align to cache lines
pub struct Utf8Data {
    pub table: [u8; 384],
}

// BEGIN GENERATED CODE. PLEASE DO NOT EDIT.
// Instead, please regenerate using generate-encoding-data.py

pub static UTF8_DATA: Utf8Data = Utf8Data {
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

// END GENERATED CODE

pub fn utf8_valid_up_to(src: &[u8]) -> usize {
    let mut read = 0;
    'outer: loop {
        let mut byte = {
            let src_remaining = &src[read..];
            match validate_ascii(src_remaining) {
                None => {
                    return src.len();
                }
                Some((non_ascii, consumed)) => {
                    read += consumed;
                    non_ascii
                }
            }
        };
        // Check for the longest sequence to avoid checking twice for the
        // multi-byte sequences. This can't overflow with 64-bit address space,
        // because full 64 bits aren't in use. In the 32-bit PAE case, for this
        // to overflow would mean that the source slice would be so large that
        // the address space of the process would not have space for any code.
        // Therefore, the slice cannot be so long that this would overflow.
        if likely(read + 4 <= src.len()) {
            'inner: loop {
                // At this point, `byte` is not included in `read`, because we
                // don't yet know that a) the UTF-8 sequence is valid and b) that there
                // is output space if it is an astral sequence.
                // Inspecting the lead byte directly is faster than what the
                // std lib does!
                if likely(in_inclusive_range8(byte, 0xC2, 0xDF)) {
                    // Two-byte
                    let second = unsafe { *(src.get_unchecked(read + 1)) };
                    if !in_inclusive_range8(second, 0x80, 0xBF) {
                        break 'outer;
                    }
                    read += 2;

                    // Next lead (manually inlined)
                    if likely(read + 4 <= src.len()) {
                        byte = unsafe { *(src.get_unchecked(read)) };
                        if byte < 0x80 {
                            read += 1;
                            continue 'outer;
                        }
                        continue 'inner;
                    }
                    break 'inner;
                }
                if likely(byte < 0xF0) {
                    'three: loop {
                        // Three-byte
                        let second = unsafe { *(src.get_unchecked(read + 1)) };
                        let third = unsafe { *(src.get_unchecked(read + 2)) };
                        if ((UTF8_DATA.table[usize::from(second)]
                            & unsafe { *(UTF8_DATA.table.get_unchecked(byte as usize + 0x80)) })
                            | (third >> 6))
                            != 2
                        {
                            break 'outer;
                        }
                        read += 3;

                        // Next lead (manually inlined)
                        if likely(read + 4 <= src.len()) {
                            byte = unsafe { *(src.get_unchecked(read)) };
                            if in_inclusive_range8(byte, 0xE0, 0xEF) {
                                continue 'three;
                            }
                            if likely(byte < 0x80) {
                                read += 1;
                                continue 'outer;
                            }
                            continue 'inner;
                        }
                        break 'inner;
                    }
                }
                // Four-byte
                let second = unsafe { *(src.get_unchecked(read + 1)) };
                let third = unsafe { *(src.get_unchecked(read + 2)) };
                let fourth = unsafe { *(src.get_unchecked(read + 3)) };
                if (u16::from(
                    UTF8_DATA.table[usize::from(second)]
                        & unsafe { *(UTF8_DATA.table.get_unchecked(byte as usize + 0x80)) },
                ) | u16::from(third >> 6)
                    | (u16::from(fourth & 0xC0) << 2))
                    != 0x202
                {
                    break 'outer;
                }
                read += 4;

                // Next lead
                if likely(read + 4 <= src.len()) {
                    byte = unsafe { *(src.get_unchecked(read)) };
                    if byte < 0x80 {
                        read += 1;
                        continue 'outer;
                    }
                    continue 'inner;
                }
                break 'inner;
            }
        }
        // We can't have a complete 4-byte sequence, but we could still have
        // one to three shorter sequences.
        'tail: loop {
            // >= is better for bound check elision than ==
            if read >= src.len() {
                break 'outer;
            }
            byte = src[read];
            // At this point, `byte` is not included in `read`, because we
            // don't yet know that a) the UTF-8 sequence is valid and b) that there
            // is output space if it is an astral sequence.
            // Inspecting the lead byte directly is faster than what the
            // std lib does!
            if byte < 0x80 {
                read += 1;
                continue 'tail;
            }
            if in_inclusive_range8(byte, 0xC2, 0xDF) {
                // Two-byte
                let new_read = read + 2;
                if new_read > src.len() {
                    break 'outer;
                }
                let second = src[read + 1];
                if !in_inclusive_range8(second, 0x80, 0xBF) {
                    break 'outer;
                }
                read += 2;
                continue 'tail;
            }
            // We need to exclude valid four byte lead bytes, because
            // `UTF8_DATA.second_mask` covers
            if byte < 0xF0 {
                // Three-byte
                let new_read = read + 3;
                if new_read > src.len() {
                    break 'outer;
                }
                let second = src[read + 1];
                let third = src[read + 2];
                if ((UTF8_DATA.table[usize::from(second)]
                    & unsafe { *(UTF8_DATA.table.get_unchecked(byte as usize + 0x80)) })
                    | (third >> 6))
                    != 2
                {
                    break 'outer;
                }
                read += 3;
                // `'tail` handles sequences shorter than 4, so
                // there can't be another sequence after this one.
                break 'outer;
            }
            break 'outer;
        }
    }
    read
}

#[cfg_attr(feature = "cargo-clippy", allow(never_loop, cyclomatic_complexity))]
pub fn convert_utf8_to_utf16_up_to_invalid(src: &[u8], dst: &mut [u16]) -> (usize, usize) {
    let mut read = 0;
    let mut written = 0;
    'outer: loop {
        let mut byte = {
            let src_remaining = &src[read..];
            let dst_remaining = &mut dst[written..];
            let length = ::core::cmp::min(src_remaining.len(), dst_remaining.len());
            match unsafe {
                ascii_to_basic_latin(src_remaining.as_ptr(), dst_remaining.as_mut_ptr(), length)
            } {
                None => {
                    read += length;
                    written += length;
                    break 'outer;
                }
                Some((non_ascii, consumed)) => {
                    read += consumed;
                    written += consumed;
                    non_ascii
                }
            }
        };
        // Check for the longest sequence to avoid checking twice for the
        // multi-byte sequences. This can't overflow with 64-bit address space,
        // because full 64 bits aren't in use. In the 32-bit PAE case, for this
        // to overflow would mean that the source slice would be so large that
        // the address space of the process would not have space for any code.
        // Therefore, the slice cannot be so long that this would overflow.
        if likely(read + 4 <= src.len()) {
            'inner: loop {
                // At this point, `byte` is not included in `read`, because we
                // don't yet know that a) the UTF-8 sequence is valid and b) that there
                // is output space if it is an astral sequence.
                // We know, thanks to `ascii_to_basic_latin` that there is output
                // space for at least one UTF-16 code unit, so no need to check
                // for output space in the BMP cases.
                // Inspecting the lead byte directly is faster than what the
                // std lib does!
                if likely(in_inclusive_range8(byte, 0xC2, 0xDF)) {
                    // Two-byte
                    let second = unsafe { *(src.get_unchecked(read + 1)) };
                    if !in_inclusive_range8(second, 0x80, 0xBF) {
                        break 'outer;
                    }
                    unsafe {
                        *(dst.get_unchecked_mut(written)) =
                            ((u16::from(byte) & 0x1F) << 6) | (u16::from(second) & 0x3F)
                    };
                    read += 2;
                    written += 1;

                    // Next lead (manually inlined)
                    if written == dst.len() {
                        break 'outer;
                    }
                    if likely(read + 4 <= src.len()) {
                        byte = unsafe { *(src.get_unchecked(read)) };
                        if byte < 0x80 {
                            unsafe { *(dst.get_unchecked_mut(written)) = u16::from(byte) };
                            read += 1;
                            written += 1;
                            continue 'outer;
                        }
                        continue 'inner;
                    }
                    break 'inner;
                }
                if likely(byte < 0xF0) {
                    'three: loop {
                        // Three-byte
                        let second = unsafe { *(src.get_unchecked(read + 1)) };
                        let third = unsafe { *(src.get_unchecked(read + 2)) };
                        if ((UTF8_DATA.table[usize::from(second)]
                            & unsafe { *(UTF8_DATA.table.get_unchecked(byte as usize + 0x80)) })
                            | (third >> 6))
                            != 2
                        {
                            break 'outer;
                        }
                        let point = ((u16::from(byte) & 0xF) << 12)
                            | ((u16::from(second) & 0x3F) << 6)
                            | (u16::from(third) & 0x3F);
                        unsafe { *(dst.get_unchecked_mut(written)) = point };
                        read += 3;
                        written += 1;

                        // Next lead (manually inlined)
                        if written == dst.len() {
                            break 'outer;
                        }
                        if likely(read + 4 <= src.len()) {
                            byte = unsafe { *(src.get_unchecked(read)) };
                            if in_inclusive_range8(byte, 0xE0, 0xEF) {
                                continue 'three;
                            }
                            if likely(byte < 0x80) {
                                unsafe { *(dst.get_unchecked_mut(written)) = u16::from(byte) };
                                read += 1;
                                written += 1;
                                continue 'outer;
                            }
                            continue 'inner;
                        }
                        break 'inner;
                    }
                }
                // Four-byte
                if written + 1 == dst.len() {
                    break 'outer;
                }
                let second = unsafe { *(src.get_unchecked(read + 1)) };
                let third = unsafe { *(src.get_unchecked(read + 2)) };
                let fourth = unsafe { *(src.get_unchecked(read + 3)) };
                if (u16::from(
                    UTF8_DATA.table[usize::from(second)]
                        & unsafe { *(UTF8_DATA.table.get_unchecked(byte as usize + 0x80)) },
                ) | u16::from(third >> 6)
                    | (u16::from(fourth & 0xC0) << 2))
                    != 0x202
                {
                    break 'outer;
                }
                let point = ((u32::from(byte) & 0x7) << 18)
                    | ((u32::from(second) & 0x3F) << 12)
                    | ((u32::from(third) & 0x3F) << 6)
                    | (u32::from(fourth) & 0x3F);
                unsafe { *(dst.get_unchecked_mut(written)) = (0xD7C0 + (point >> 10)) as u16 };
                unsafe {
                    *(dst.get_unchecked_mut(written + 1)) = (0xDC00 + (point & 0x3FF)) as u16
                };
                read += 4;
                written += 2;

                // Next lead
                if written == dst.len() {
                    break 'outer;
                }
                if likely(read + 4 <= src.len()) {
                    byte = unsafe { *(src.get_unchecked(read)) };
                    if byte < 0x80 {
                        unsafe { *(dst.get_unchecked_mut(written)) = u16::from(byte) };
                        read += 1;
                        written += 1;
                        continue 'outer;
                    }
                    continue 'inner;
                }
                break 'inner;
            }
        }
        // We can't have a complete 4-byte sequence, but we could still have
        // one to three shorter sequences.
        'tail: loop {
            // >= is better for bound check elision than ==
            if read >= src.len() || written >= dst.len() {
                break 'outer;
            }
            byte = src[read];
            // At this point, `byte` is not included in `read`, because we
            // don't yet know that a) the UTF-8 sequence is valid and b) that there
            // is output space if it is an astral sequence.
            // Inspecting the lead byte directly is faster than what the
            // std lib does!
            if byte < 0x80 {
                dst[written] = u16::from(byte);
                read += 1;
                written += 1;
                continue 'tail;
            }
            if in_inclusive_range8(byte, 0xC2, 0xDF) {
                // Two-byte
                let new_read = read + 2;
                if new_read > src.len() {
                    break 'outer;
                }
                let second = src[read + 1];
                if !in_inclusive_range8(second, 0x80, 0xBF) {
                    break 'outer;
                }
                dst[written] = ((u16::from(byte) & 0x1F) << 6) | (u16::from(second) & 0x3F);
                read += 2;
                written += 1;
                continue 'tail;
            }
            // We need to exclude valid four byte lead bytes, because
            // `UTF8_DATA.second_mask` covers
            if byte < 0xF0 {
                // Three-byte
                let new_read = read + 3;
                if new_read > src.len() {
                    break 'outer;
                }
                let second = src[read + 1];
                let third = src[read + 2];
                if ((UTF8_DATA.table[usize::from(second)]
                    & unsafe { *(UTF8_DATA.table.get_unchecked(byte as usize + 0x80)) })
                    | (third >> 6))
                    != 2
                {
                    break 'outer;
                }
                let point = ((u16::from(byte) & 0xF) << 12)
                    | ((u16::from(second) & 0x3F) << 6)
                    | (u16::from(third) & 0x3F);
                dst[written] = point;
                read += 3;
                written += 1;
                // `'tail` handles sequences shorter than 4, so
                // there can't be another sequence after this one.
                break 'outer;
            }
            break 'outer;
        }
    }
    (read, written)
}

pub struct Utf8Decoder {
    code_point: u32,
    bytes_seen: usize,   // 1, 2 or 3: counts continuations only
    bytes_needed: usize, // 1, 2 or 3: counts continuations only
    lower_boundary: u8,
    upper_boundary: u8,
}

impl Utf8Decoder {
    pub fn new_inner() -> Utf8Decoder {
        Utf8Decoder {
            code_point: 0,
            bytes_seen: 0,
            bytes_needed: 0,
            lower_boundary: 0x80u8,
            upper_boundary: 0xBFu8,
        }
    }

    pub fn new() -> VariantDecoder {
        VariantDecoder::Utf8(Utf8Decoder::new_inner())
    }

    pub fn in_neutral_state(&self) -> bool {
        self.bytes_needed == 0
    }

    fn extra_from_state(&self) -> usize {
        if self.bytes_needed == 0 {
            0
        } else {
            self.bytes_seen + 1
        }
    }

    pub fn max_utf16_buffer_length(&self, byte_length: usize) -> Option<usize> {
        byte_length.checked_add(1 + self.extra_from_state())
    }

    pub fn max_utf8_buffer_length_without_replacement(&self, byte_length: usize) -> Option<usize> {
        byte_length.checked_add(3 + self.extra_from_state())
    }

    pub fn max_utf8_buffer_length(&self, byte_length: usize) -> Option<usize> {
        checked_add(
            3,
            checked_mul(3, byte_length.checked_add(self.extra_from_state())),
        )
    }

    decoder_functions!(
        {},
        {
            // This is the fast path. The rest runs only at the
            // start and end for partial sequences.
            if self.bytes_needed == 0 {
                dest.copy_utf8_up_to_invalid_from(&mut source);
            }
        },
        {
            if self.bytes_needed != 0 {
                let bad_bytes = (self.bytes_seen + 1) as u8;
                self.code_point = 0;
                self.bytes_needed = 0;
                self.bytes_seen = 0;
                return (
                    DecoderResult::Malformed(bad_bytes, 0),
                    src_consumed,
                    dest.written(),
                );
            }
        },
        {
            if self.bytes_needed == 0 {
                if b < 0x80u8 {
                    destination_handle.write_ascii(b);
                    continue;
                }
                if b < 0xC2u8 {
                    return (
                        DecoderResult::Malformed(1, 0),
                        unread_handle.consumed(),
                        destination_handle.written(),
                    );
                }
                if b < 0xE0u8 {
                    self.bytes_needed = 1;
                    self.code_point = u32::from(b) & 0x1F;
                    continue;
                }
                if b < 0xF0u8 {
                    if b == 0xE0u8 {
                        self.lower_boundary = 0xA0u8;
                    } else if b == 0xEDu8 {
                        self.upper_boundary = 0x9Fu8;
                    }
                    self.bytes_needed = 2;
                    self.code_point = u32::from(b) & 0xF;
                    continue;
                }
                if b < 0xF5u8 {
                    if b == 0xF0u8 {
                        self.lower_boundary = 0x90u8;
                    } else if b == 0xF4u8 {
                        self.upper_boundary = 0x8Fu8;
                    }
                    self.bytes_needed = 3;
                    self.code_point = u32::from(b) & 0x7;
                    continue;
                }
                return (
                    DecoderResult::Malformed(1, 0),
                    unread_handle.consumed(),
                    destination_handle.written(),
                );
            }
            // self.bytes_needed != 0
            if !(b >= self.lower_boundary && b <= self.upper_boundary) {
                let bad_bytes = (self.bytes_seen + 1) as u8;
                self.code_point = 0;
                self.bytes_needed = 0;
                self.bytes_seen = 0;
                self.lower_boundary = 0x80u8;
                self.upper_boundary = 0xBFu8;
                return (
                    DecoderResult::Malformed(bad_bytes, 0),
                    unread_handle.unread(),
                    destination_handle.written(),
                );
            }
            self.lower_boundary = 0x80u8;
            self.upper_boundary = 0xBFu8;
            self.code_point = (self.code_point << 6) | (u32::from(b) & 0x3F);
            self.bytes_seen += 1;
            if self.bytes_seen != self.bytes_needed {
                continue;
            }
            if self.bytes_needed == 3 {
                destination_handle.write_astral(self.code_point);
            } else {
                destination_handle.write_bmp_excl_ascii(self.code_point as u16);
            }
            self.code_point = 0;
            self.bytes_needed = 0;
            self.bytes_seen = 0;
            continue;
        },
        self,
        src_consumed,
        dest,
        source,
        b,
        destination_handle,
        unread_handle,
        check_space_astral
    );
}

#[cfg_attr(feature = "cargo-clippy", allow(never_loop))]
#[inline(never)]
pub fn convert_utf16_to_utf8_partial_inner(src: &[u16], dst: &mut [u8]) -> (usize, usize) {
    let mut read = 0;
    let mut written = 0;
    'outer: loop {
        let mut unit = {
            let src_remaining = &src[read..];
            let dst_remaining = &mut dst[written..];
            let length = if dst_remaining.len() < src_remaining.len() {
                dst_remaining.len()
            } else {
                src_remaining.len()
            };
            match unsafe {
                basic_latin_to_ascii(src_remaining.as_ptr(), dst_remaining.as_mut_ptr(), length)
            } {
                None => {
                    read += length;
                    written += length;
                    return (read, written);
                }
                Some((non_ascii, consumed)) => {
                    read += consumed;
                    written += consumed;
                    non_ascii
                }
            }
        };
        'inner: loop {
            // The following loop is only broken out of as a goto forward.
            loop {
                // Unfortunately, this check isn't enough for the compiler to elide
                // the bound checks on writes to dst, which is why they are manually
                // elided, which makes a measurable difference.
                if written.checked_add(4).unwrap() > dst.len() {
                    return (read, written);
                }
                read += 1;
                if unit < 0x800 {
                    unsafe {
                        *(dst.get_unchecked_mut(written)) = (unit >> 6) as u8 | 0xC0u8;
                        written += 1;
                        *(dst.get_unchecked_mut(written)) = (unit & 0x3F) as u8 | 0x80u8;
                        written += 1;
                    }
                    break;
                }
                let unit_minus_surrogate_start = unit.wrapping_sub(0xD800);
                if likely(unit_minus_surrogate_start > (0xDFFF - 0xD800)) {
                    unsafe {
                        *(dst.get_unchecked_mut(written)) = (unit >> 12) as u8 | 0xE0u8;
                        written += 1;
                        *(dst.get_unchecked_mut(written)) = ((unit & 0xFC0) >> 6) as u8 | 0x80u8;
                        written += 1;
                        *(dst.get_unchecked_mut(written)) = (unit & 0x3F) as u8 | 0x80u8;
                        written += 1;
                    }
                    break;
                }
                if likely(unit_minus_surrogate_start <= (0xDBFF - 0xD800)) {
                    // high surrogate
                    // read > src.len() is impossible, but using
                    // >= instead of == allows the compiler to elide a bound check.
                    if read >= src.len() {
                        debug_assert_eq!(read, src.len());
                        // Unpaired surrogate at the end of the buffer.
                        unsafe {
                            *(dst.get_unchecked_mut(written)) = 0xEFu8;
                            written += 1;
                            *(dst.get_unchecked_mut(written)) = 0xBFu8;
                            written += 1;
                            *(dst.get_unchecked_mut(written)) = 0xBDu8;
                            written += 1;
                        }
                        return (read, written);
                    }
                    let second = src[read];
                    let second_minus_low_surrogate_start = second.wrapping_sub(0xDC00);
                    if likely(second_minus_low_surrogate_start <= (0xDFFF - 0xDC00)) {
                        // The next code unit is a low surrogate. Advance position.
                        read += 1;
                        let astral = (u32::from(unit) << 10) + u32::from(second)
                            - (((0xD800u32 << 10) - 0x10000u32) + 0xDC00u32);
                        unsafe {
                            *(dst.get_unchecked_mut(written)) = (astral >> 18) as u8 | 0xF0u8;
                            written += 1;
                            *(dst.get_unchecked_mut(written)) =
                                ((astral & 0x3F000u32) >> 12) as u8 | 0x80u8;
                            written += 1;
                            *(dst.get_unchecked_mut(written)) =
                                ((astral & 0xFC0u32) >> 6) as u8 | 0x80u8;
                            written += 1;
                            *(dst.get_unchecked_mut(written)) = (astral & 0x3F) as u8 | 0x80u8;
                            written += 1;
                        }
                        break;
                    }
                    // The next code unit is not a low surrogate. Don't advance
                    // position and treat the high surrogate as unpaired.
                    // Fall through
                }
                // Unpaired low surrogate
                unsafe {
                    *(dst.get_unchecked_mut(written)) = 0xEFu8;
                    written += 1;
                    *(dst.get_unchecked_mut(written)) = 0xBFu8;
                    written += 1;
                    *(dst.get_unchecked_mut(written)) = 0xBDu8;
                    written += 1;
                }
                break;
            }
            // Now see if the next unit is Basic Latin
            // read > src.len() is impossible, but using
            // >= instead of == allows the compiler to elide a bound check.
            if read >= src.len() {
                debug_assert_eq!(read, src.len());
                return (read, written);
            }
            unit = src[read];
            if unlikely(unit < 0x80) {
                // written > dst.len() is impossible, but using
                // >= instead of == allows the compiler to elide a bound check.
                if written >= dst.len() {
                    debug_assert_eq!(written, dst.len());
                    return (read, written);
                }
                dst[written] = unit as u8;
                read += 1;
                written += 1;
                // Mysteriously, adding a punctuation check here makes
                // the expected benificiary cases *slower*!
                continue 'outer;
            }
            continue 'inner;
        }
    }
}

#[inline(never)]
pub fn convert_utf16_to_utf8_partial_tail(src: &[u16], dst: &mut [u8]) -> (usize, usize) {
    // Everything below is cold code!
    let mut read = 0;
    let mut written = 0;
    let mut unit = src[read];
    // We now have up to 3 output slots, so an astral character
    // will not fit.
    if unit < 0x800 {
        loop {
            if unit < 0x80 {
                if written >= dst.len() {
                    return (read, written);
                }
                read += 1;
                dst[written] = unit as u8;
                written += 1;
            } else if unit < 0x800 {
                if written + 2 > dst.len() {
                    return (read, written);
                }
                read += 1;
                dst[written] = (unit >> 6) as u8 | 0xC0u8;
                written += 1;
                dst[written] = (unit & 0x3F) as u8 | 0x80u8;
                written += 1;
            } else {
                return (read, written);
            }
            // read > src.len() is impossible, but using
            // >= instead of == allows the compiler to elide a bound check.
            if read >= src.len() {
                debug_assert_eq!(read, src.len());
                return (read, written);
            }
            unit = src[read];
        }
    }
    // Could be an unpaired surrogate, but we'll need 3 output
    // slots in any case.
    if written + 3 > dst.len() {
        return (read, written);
    }
    read += 1;
    let unit_minus_surrogate_start = unit.wrapping_sub(0xD800);
    if unit_minus_surrogate_start <= (0xDFFF - 0xD800) {
        // Got surrogate
        if unit_minus_surrogate_start <= (0xDBFF - 0xD800) {
            // Got high surrogate
            if read >= src.len() {
                // Unpaired high surrogate
                unit = 0xFFFD;
            } else {
                let second = src[read];
                if in_inclusive_range16(second, 0xDC00, 0xDFFF) {
                    // Valid surrogate pair, but we know it won't fit.
                    read -= 1;
                    return (read, written);
                }
                // Unpaired high
                unit = 0xFFFD;
            }
        } else {
            // Unpaired low
            unit = 0xFFFD;
        }
    }
    dst[written] = (unit >> 12) as u8 | 0xE0u8;
    written += 1;
    dst[written] = ((unit & 0xFC0) >> 6) as u8 | 0x80u8;
    written += 1;
    dst[written] = (unit & 0x3F) as u8 | 0x80u8;
    written += 1;
    debug_assert_eq!(written, dst.len());
    (read, written)
}

pub struct Utf8Encoder;

impl Utf8Encoder {
    pub fn new(encoding: &'static Encoding) -> Encoder {
        Encoder::new(encoding, VariantEncoder::Utf8(Utf8Encoder))
    }

    pub fn max_buffer_length_from_utf16_without_replacement(
        &self,
        u16_length: usize,
    ) -> Option<usize> {
        u16_length.checked_mul(3)
    }

    pub fn max_buffer_length_from_utf8_without_replacement(
        &self,
        byte_length: usize,
    ) -> Option<usize> {
        Some(byte_length)
    }

    pub fn encode_from_utf16_raw(
        &mut self,
        src: &[u16],
        dst: &mut [u8],
        _last: bool,
    ) -> (EncoderResult, usize, usize) {
        let (read, written) = convert_utf16_to_utf8_partial(src, dst);
        (
            if read == src.len() {
                EncoderResult::InputEmpty
            } else {
                EncoderResult::OutputFull
            },
            read,
            written,
        )
    }

    pub fn encode_from_utf8_raw(
        &mut self,
        src: &str,
        dst: &mut [u8],
        _last: bool,
    ) -> (EncoderResult, usize, usize) {
        let bytes = src.as_bytes();
        let mut to_write = bytes.len();
        if to_write <= dst.len() {
            (&mut dst[..to_write]).copy_from_slice(bytes);
            return (EncoderResult::InputEmpty, to_write, to_write);
        }
        to_write = dst.len();
        // Move back until we find a UTF-8 sequence boundary.
        while (bytes[to_write] & 0xC0) == 0x80 {
            to_write -= 1;
        }
        (&mut dst[..to_write]).copy_from_slice(&bytes[..to_write]);
        (EncoderResult::OutputFull, to_write, to_write)
    }
}

// Any copyright to the test code below this comment is dedicated to the
// Public Domain. http://creativecommons.org/publicdomain/zero/1.0/

#[cfg(all(test, feature = "alloc"))]
mod tests {
    use super::super::testing::*;
    use super::super::*;

    //    fn decode_utf8_to_utf16(bytes: &[u8], expect: &[u16]) {
    //        decode_to_utf16_without_replacement(UTF_8, bytes, expect);
    //    }

    fn decode_utf8_to_utf8(bytes: &[u8], expect: &str) {
        decode_to_utf8(UTF_8, bytes, expect);
    }

    fn decode_valid_utf8(string: &str) {
        decode_utf8_to_utf8(string.as_bytes(), string);
    }

    fn encode_utf8_from_utf16(string: &[u16], expect: &[u8]) {
        encode_from_utf16(UTF_8, string, expect);
    }

    fn encode_utf8_from_utf8(string: &str, expect: &[u8]) {
        encode_from_utf8(UTF_8, string, expect);
    }

    fn encode_utf8_from_utf16_with_output_limit(
        string: &[u16],
        expect: &str,
        limit: usize,
        expect_result: EncoderResult,
    ) {
        let mut dst = Vec::new();
        {
            dst.resize(limit, 0u8);
            let mut encoder = UTF_8.new_encoder();
            let (result, read, written) =
                encoder.encode_from_utf16_without_replacement(string, &mut dst, false);
            assert_eq!(result, expect_result);
            if expect_result == EncoderResult::InputEmpty {
                assert_eq!(read, string.len());
            }
            assert_eq!(&dst[..written], expect.as_bytes());
        }
        {
            dst.resize(64, 0u8);
            for (i, elem) in dst.iter_mut().enumerate() {
                *elem = i as u8;
            }
            let mut encoder = UTF_8.new_encoder();
            let (_, _, mut j) =
                encoder.encode_from_utf16_without_replacement(string, &mut dst, false);
            while j < dst.len() {
                assert_eq!(usize::from(dst[j]), j);
                j += 1;
            }
        }
    }

    #[test]
    fn test_utf8_decode() {
        // Empty
        decode_valid_utf8("");
        // ASCII
        decode_valid_utf8("ab");
        // Low BMP
        decode_valid_utf8("a\u{E4}Z");
        // High BMP
        decode_valid_utf8("a\u{2603}Z");
        // Astral
        decode_valid_utf8("a\u{1F4A9}Z");
        // Low BMP with last byte missing
        decode_utf8_to_utf8(b"a\xC3Z", "a\u{FFFD}Z");
        decode_utf8_to_utf8(b"a\xC3", "a\u{FFFD}");
        // High BMP with last byte missing
        decode_utf8_to_utf8(b"a\xE2\x98Z", "a\u{FFFD}Z");
        decode_utf8_to_utf8(b"a\xE2\x98", "a\u{FFFD}");
        // Astral with last byte missing
        decode_utf8_to_utf8(b"a\xF0\x9F\x92Z", "a\u{FFFD}Z");
        decode_utf8_to_utf8(b"a\xF0\x9F\x92", "a\u{FFFD}");
        // Lone highest continuation
        decode_utf8_to_utf8(b"a\xBFZ", "a\u{FFFD}Z");
        decode_utf8_to_utf8(b"a\xBF", "a\u{FFFD}");
        // Two lone highest continuations
        decode_utf8_to_utf8(b"a\xBF\xBFZ", "a\u{FFFD}\u{FFFD}Z");
        decode_utf8_to_utf8(b"a\xBF\xBF", "a\u{FFFD}\u{FFFD}");
        // Low BMP followed by lowest lone continuation
        decode_utf8_to_utf8(b"a\xC3\xA4\x80Z", "a\u{E4}\u{FFFD}Z");
        decode_utf8_to_utf8(b"a\xC3\xA4\x80", "a\u{E4}\u{FFFD}");
        // Low BMP followed by highest lone continuation
        decode_utf8_to_utf8(b"a\xC3\xA4\xBFZ", "a\u{E4}\u{FFFD}Z");
        decode_utf8_to_utf8(b"a\xC3\xA4\xBF", "a\u{E4}\u{FFFD}");
        // High BMP followed by lowest lone continuation
        decode_utf8_to_utf8(b"a\xE2\x98\x83\x80Z", "a\u{2603}\u{FFFD}Z");
        decode_utf8_to_utf8(b"a\xE2\x98\x83\x80", "a\u{2603}\u{FFFD}");
        // High BMP followed by highest lone continuation
        decode_utf8_to_utf8(b"a\xE2\x98\x83\xBFZ", "a\u{2603}\u{FFFD}Z");
        decode_utf8_to_utf8(b"a\xE2\x98\x83\xBF", "a\u{2603}\u{FFFD}");
        // Astral followed by lowest lone continuation
        decode_utf8_to_utf8(b"a\xF0\x9F\x92\xA9\x80Z", "a\u{1F4A9}\u{FFFD}Z");
        decode_utf8_to_utf8(b"a\xF0\x9F\x92\xA9\x80", "a\u{1F4A9}\u{FFFD}");
        // Astral followed by highest lone continuation
        decode_utf8_to_utf8(b"a\xF0\x9F\x92\xA9\xBFZ", "a\u{1F4A9}\u{FFFD}Z");
        decode_utf8_to_utf8(b"a\xF0\x9F\x92\xA9\xBF", "a\u{1F4A9}\u{FFFD}");

        // Boundary conditions
        // Lowest single-byte
        decode_valid_utf8("Z\x00");
        decode_valid_utf8("Z\x00Z");
        // Lowest single-byte as two-byte overlong sequence
        decode_utf8_to_utf8(b"a\xC0\x80", "a\u{FFFD}\u{FFFD}");
        decode_utf8_to_utf8(b"a\xC0\x80Z", "a\u{FFFD}\u{FFFD}Z");
        // Lowest single-byte as three-byte overlong sequence
        decode_utf8_to_utf8(b"a\xE0\x80\x80", "a\u{FFFD}\u{FFFD}\u{FFFD}");
        decode_utf8_to_utf8(b"a\xE0\x80\x80Z", "a\u{FFFD}\u{FFFD}\u{FFFD}Z");
        // Lowest single-byte as four-byte overlong sequence
        decode_utf8_to_utf8(b"a\xF0\x80\x80\x80", "a\u{FFFD}\u{FFFD}\u{FFFD}\u{FFFD}");
        decode_utf8_to_utf8(b"a\xF0\x80\x80\x80Z", "a\u{FFFD}\u{FFFD}\u{FFFD}\u{FFFD}Z");
        // One below lowest single-byte
        decode_utf8_to_utf8(b"a\xFF", "a\u{FFFD}");
        decode_utf8_to_utf8(b"a\xFFZ", "a\u{FFFD}Z");
        // Highest single-byte
        decode_valid_utf8("a\x7F");
        decode_valid_utf8("a\x7FZ");
        // Highest single-byte as two-byte overlong sequence
        decode_utf8_to_utf8(b"a\xC1\xBF", "a\u{FFFD}\u{FFFD}");
        decode_utf8_to_utf8(b"a\xC1\xBFZ", "a\u{FFFD}\u{FFFD}Z");
        // Highest single-byte as three-byte overlong sequence
        decode_utf8_to_utf8(b"a\xE0\x81\xBF", "a\u{FFFD}\u{FFFD}\u{FFFD}");
        decode_utf8_to_utf8(b"a\xE0\x81\xBFZ", "a\u{FFFD}\u{FFFD}\u{FFFD}Z");
        // Highest single-byte as four-byte overlong sequence
        decode_utf8_to_utf8(b"a\xF0\x80\x81\xBF", "a\u{FFFD}\u{FFFD}\u{FFFD}\u{FFFD}");
        decode_utf8_to_utf8(b"a\xF0\x80\x81\xBFZ", "a\u{FFFD}\u{FFFD}\u{FFFD}\u{FFFD}Z");
        // One past highest single byte (also lone continuation)
        decode_utf8_to_utf8(b"a\x80Z", "a\u{FFFD}Z");
        decode_utf8_to_utf8(b"a\x80", "a\u{FFFD}");
        // Two lone continuations
        decode_utf8_to_utf8(b"a\x80\x80Z", "a\u{FFFD}\u{FFFD}Z");
        decode_utf8_to_utf8(b"a\x80\x80", "a\u{FFFD}\u{FFFD}");
        // Three lone continuations
        decode_utf8_to_utf8(b"a\x80\x80\x80Z", "a\u{FFFD}\u{FFFD}\u{FFFD}Z");
        decode_utf8_to_utf8(b"a\x80\x80\x80", "a\u{FFFD}\u{FFFD}\u{FFFD}");
        // Four lone continuations
        decode_utf8_to_utf8(b"a\x80\x80\x80\x80Z", "a\u{FFFD}\u{FFFD}\u{FFFD}\u{FFFD}Z");
        decode_utf8_to_utf8(b"a\x80\x80\x80\x80", "a\u{FFFD}\u{FFFD}\u{FFFD}\u{FFFD}");
        // Lowest two-byte
        decode_utf8_to_utf8(b"a\xC2\x80", "a\u{0080}");
        decode_utf8_to_utf8(b"a\xC2\x80Z", "a\u{0080}Z");
        // Lowest two-byte as three-byte overlong sequence
        decode_utf8_to_utf8(b"a\xE0\x82\x80", "a\u{FFFD}\u{FFFD}\u{FFFD}");
        decode_utf8_to_utf8(b"a\xE0\x82\x80Z", "a\u{FFFD}\u{FFFD}\u{FFFD}Z");
        // Lowest two-byte as four-byte overlong sequence
        decode_utf8_to_utf8(b"a\xF0\x80\x82\x80", "a\u{FFFD}\u{FFFD}\u{FFFD}\u{FFFD}");
        decode_utf8_to_utf8(b"a\xF0\x80\x82\x80Z", "a\u{FFFD}\u{FFFD}\u{FFFD}\u{FFFD}Z");
        // Lead one below lowest two-byte
        decode_utf8_to_utf8(b"a\xC1\x80", "a\u{FFFD}\u{FFFD}");
        decode_utf8_to_utf8(b"a\xC1\x80Z", "a\u{FFFD}\u{FFFD}Z");
        // Trail one below lowest two-byte
        decode_utf8_to_utf8(b"a\xC2\x7F", "a\u{FFFD}\u{007F}");
        decode_utf8_to_utf8(b"a\xC2\x7FZ", "a\u{FFFD}\u{007F}Z");
        // Highest two-byte
        decode_utf8_to_utf8(b"a\xDF\xBF", "a\u{07FF}");
        decode_utf8_to_utf8(b"a\xDF\xBFZ", "a\u{07FF}Z");
        // Highest two-byte as three-byte overlong sequence
        decode_utf8_to_utf8(b"a\xE0\x9F\xBF", "a\u{FFFD}\u{FFFD}\u{FFFD}");
        decode_utf8_to_utf8(b"a\xE0\x9F\xBFZ", "a\u{FFFD}\u{FFFD}\u{FFFD}Z");
        // Highest two-byte as four-byte overlong sequence
        decode_utf8_to_utf8(b"a\xF0\x80\x9F\xBF", "a\u{FFFD}\u{FFFD}\u{FFFD}\u{FFFD}");
        decode_utf8_to_utf8(b"a\xF0\x80\x9F\xBFZ", "a\u{FFFD}\u{FFFD}\u{FFFD}\u{FFFD}Z");
        // Lowest three-byte
        decode_utf8_to_utf8(b"a\xE0\xA0\x80", "a\u{0800}");
        decode_utf8_to_utf8(b"a\xE0\xA0\x80Z", "a\u{0800}Z");
        // Lowest three-byte as four-byte overlong sequence
        decode_utf8_to_utf8(b"a\xF0\x80\xA0\x80", "a\u{FFFD}\u{FFFD}\u{FFFD}\u{FFFD}");
        decode_utf8_to_utf8(b"a\xF0\x80\xA0\x80Z", "a\u{FFFD}\u{FFFD}\u{FFFD}\u{FFFD}Z");
        // Highest below surrogates
        decode_utf8_to_utf8(b"a\xED\x9F\xBF", "a\u{D7FF}");
        decode_utf8_to_utf8(b"a\xED\x9F\xBFZ", "a\u{D7FF}Z");
        // Highest below surrogates as four-byte overlong sequence
        decode_utf8_to_utf8(b"a\xF0\x8D\x9F\xBF", "a\u{FFFD}\u{FFFD}\u{FFFD}\u{FFFD}");
        decode_utf8_to_utf8(b"a\xF0\x8D\x9F\xBFZ", "a\u{FFFD}\u{FFFD}\u{FFFD}\u{FFFD}Z");
        // First surrogate
        decode_utf8_to_utf8(b"a\xED\xA0\x80", "a\u{FFFD}\u{FFFD}\u{FFFD}");
        decode_utf8_to_utf8(b"a\xED\xA0\x80Z", "a\u{FFFD}\u{FFFD}\u{FFFD}Z");
        // First surrogate as four-byte overlong sequence
        decode_utf8_to_utf8(b"a\xF0\x8D\xA0\x80", "a\u{FFFD}\u{FFFD}\u{FFFD}\u{FFFD}");
        decode_utf8_to_utf8(b"a\xF0\x8D\xA0\x80Z", "a\u{FFFD}\u{FFFD}\u{FFFD}\u{FFFD}Z");
        // Last surrogate
        decode_utf8_to_utf8(b"a\xED\xBF\xBF", "a\u{FFFD}\u{FFFD}\u{FFFD}");
        decode_utf8_to_utf8(b"a\xED\xBF\xBFZ", "a\u{FFFD}\u{FFFD}\u{FFFD}Z");
        // Last surrogate as four-byte overlong sequence
        decode_utf8_to_utf8(b"a\xF0\x8D\xBF\xBF", "a\u{FFFD}\u{FFFD}\u{FFFD}\u{FFFD}");
        decode_utf8_to_utf8(b"a\xF0\x8D\xBF\xBFZ", "a\u{FFFD}\u{FFFD}\u{FFFD}\u{FFFD}Z");
        // Lowest above surrogates
        decode_utf8_to_utf8(b"a\xEE\x80\x80", "a\u{E000}");
        decode_utf8_to_utf8(b"a\xEE\x80\x80Z", "a\u{E000}Z");
        // Lowest above surrogates as four-byte overlong sequence
        decode_utf8_to_utf8(b"a\xF0\x8E\x80\x80", "a\u{FFFD}\u{FFFD}\u{FFFD}\u{FFFD}");
        decode_utf8_to_utf8(b"a\xF0\x8E\x80\x80Z", "a\u{FFFD}\u{FFFD}\u{FFFD}\u{FFFD}Z");
        // Highest three-byte
        decode_utf8_to_utf8(b"a\xEF\xBF\xBF", "a\u{FFFF}");
        decode_utf8_to_utf8(b"a\xEF\xBF\xBFZ", "a\u{FFFF}Z");
        // Highest three-byte as four-byte overlong sequence
        decode_utf8_to_utf8(b"a\xF0\x8F\xBF\xBF", "a\u{FFFD}\u{FFFD}\u{FFFD}\u{FFFD}");
        decode_utf8_to_utf8(b"a\xF0\x8F\xBF\xBFZ", "a\u{FFFD}\u{FFFD}\u{FFFD}\u{FFFD}Z");
        // Lowest four-byte
        decode_utf8_to_utf8(b"a\xF0\x90\x80\x80", "a\u{10000}");
        decode_utf8_to_utf8(b"a\xF0\x90\x80\x80Z", "a\u{10000}Z");
        // Highest four-byte
        decode_utf8_to_utf8(b"a\xF4\x8F\xBF\xBF", "a\u{10FFFF}");
        decode_utf8_to_utf8(b"a\xF4\x8F\xBF\xBFZ", "a\u{10FFFF}Z");
        // One past highest four-byte
        decode_utf8_to_utf8(b"a\xF4\x90\x80\x80", "a\u{FFFD}\u{FFFD}\u{FFFD}\u{FFFD}");
        decode_utf8_to_utf8(b"a\xF4\x90\x80\x80Z", "a\u{FFFD}\u{FFFD}\u{FFFD}\u{FFFD}Z");

        // Highest four-byte with last byte replaced with 0xFF
        decode_utf8_to_utf8(b"a\xF4\x8F\xBF\xFF", "a\u{FFFD}\u{FFFD}");
        decode_utf8_to_utf8(b"a\xF4\x8F\xBF\xFFZ", "a\u{FFFD}\u{FFFD}Z");
    }

    #[test]
    fn test_utf8_encode() {
        // Empty
        encode_utf8_from_utf16(&[], b"");
        encode_utf8_from_utf8("", b"");

        encode_utf8_from_utf16(&[0x0000], "\u{0000}".as_bytes());
        encode_utf8_from_utf16(&[0x007F], "\u{007F}".as_bytes());
        encode_utf8_from_utf16(&[0x0080], "\u{0080}".as_bytes());
        encode_utf8_from_utf16(&[0x07FF], "\u{07FF}".as_bytes());
        encode_utf8_from_utf16(&[0x0800], "\u{0800}".as_bytes());
        encode_utf8_from_utf16(&[0xD7FF], "\u{D7FF}".as_bytes());
        encode_utf8_from_utf16(&[0xD800], "\u{FFFD}".as_bytes());
        encode_utf8_from_utf16(&[0xD800, 0x0062], "\u{FFFD}\u{0062}".as_bytes());
        encode_utf8_from_utf16(&[0xDFFF], "\u{FFFD}".as_bytes());
        encode_utf8_from_utf16(&[0xDFFF, 0x0062], "\u{FFFD}\u{0062}".as_bytes());
        encode_utf8_from_utf16(&[0xE000], "\u{E000}".as_bytes());
        encode_utf8_from_utf16(&[0xFFFF], "\u{FFFF}".as_bytes());
        encode_utf8_from_utf16(&[0xD800, 0xDC00], "\u{10000}".as_bytes());
        encode_utf8_from_utf16(&[0xDBFF, 0xDFFF], "\u{10FFFF}".as_bytes());
        encode_utf8_from_utf16(&[0xDC00, 0xDEDE], "\u{FFFD}\u{FFFD}".as_bytes());
    }

    #[test]
    fn test_encode_utf8_from_utf16_with_output_limit() {
        encode_utf8_from_utf16_with_output_limit(&[0x0062], "\u{62}", 1, EncoderResult::InputEmpty);
        encode_utf8_from_utf16_with_output_limit(&[0x00A7], "\u{A7}", 2, EncoderResult::InputEmpty);
        encode_utf8_from_utf16_with_output_limit(
            &[0x2603],
            "\u{2603}",
            3,
            EncoderResult::InputEmpty,
        );
        encode_utf8_from_utf16_with_output_limit(
            &[0xD83D, 0xDCA9],
            "\u{1F4A9}",
            4,
            EncoderResult::InputEmpty,
        );

        encode_utf8_from_utf16_with_output_limit(&[0x00A7], "", 1, EncoderResult::OutputFull);
        encode_utf8_from_utf16_with_output_limit(&[0x2603], "", 2, EncoderResult::OutputFull);
        encode_utf8_from_utf16_with_output_limit(
            &[0xD83D, 0xDCA9],
            "",
            3,
            EncoderResult::OutputFull,
        );

        encode_utf8_from_utf16_with_output_limit(
            &[0x0063, 0x0062],
            "\u{63}\u{62}",
            2,
            EncoderResult::InputEmpty,
        );
        encode_utf8_from_utf16_with_output_limit(
            &[0x0063, 0x00A7],
            "\u{63}\u{A7}",
            3,
            EncoderResult::InputEmpty,
        );
        encode_utf8_from_utf16_with_output_limit(
            &[0x0063, 0x2603],
            "\u{63}\u{2603}",
            4,
            EncoderResult::InputEmpty,
        );
        encode_utf8_from_utf16_with_output_limit(
            &[0x0063, 0xD83D, 0xDCA9],
            "\u{63}\u{1F4A9}",
            5,
            EncoderResult::InputEmpty,
        );

        encode_utf8_from_utf16_with_output_limit(
            &[0x0063, 0x00A7],
            "\u{63}",
            2,
            EncoderResult::OutputFull,
        );
        encode_utf8_from_utf16_with_output_limit(
            &[0x0063, 0x2603],
            "\u{63}",
            3,
            EncoderResult::OutputFull,
        );
        encode_utf8_from_utf16_with_output_limit(
            &[0x0063, 0xD83D, 0xDCA9],
            "\u{63}",
            4,
            EncoderResult::OutputFull,
        );

        encode_utf8_from_utf16_with_output_limit(
            &[0x00B6, 0x0062],
            "\u{B6}\u{62}",
            3,
            EncoderResult::InputEmpty,
        );
        encode_utf8_from_utf16_with_output_limit(
            &[0x00B6, 0x00A7],
            "\u{B6}\u{A7}",
            4,
            EncoderResult::InputEmpty,
        );
        encode_utf8_from_utf16_with_output_limit(
            &[0x00B6, 0x2603],
            "\u{B6}\u{2603}",
            5,
            EncoderResult::InputEmpty,
        );
        encode_utf8_from_utf16_with_output_limit(
            &[0x00B6, 0xD83D, 0xDCA9],
            "\u{B6}\u{1F4A9}",
            6,
            EncoderResult::InputEmpty,
        );

        encode_utf8_from_utf16_with_output_limit(
            &[0x00B6, 0x00A7],
            "\u{B6}",
            3,
            EncoderResult::OutputFull,
        );
        encode_utf8_from_utf16_with_output_limit(
            &[0x00B6, 0x2603],
            "\u{B6}",
            4,
            EncoderResult::OutputFull,
        );
        encode_utf8_from_utf16_with_output_limit(
            &[0x00B6, 0xD83D, 0xDCA9],
            "\u{B6}",
            5,
            EncoderResult::OutputFull,
        );

        encode_utf8_from_utf16_with_output_limit(
            &[0x263A, 0x0062],
            "\u{263A}\u{62}",
            4,
            EncoderResult::InputEmpty,
        );
        encode_utf8_from_utf16_with_output_limit(
            &[0x263A, 0x00A7],
            "\u{263A}\u{A7}",
            5,
            EncoderResult::InputEmpty,
        );
        encode_utf8_from_utf16_with_output_limit(
            &[0x263A, 0x2603],
            "\u{263A}\u{2603}",
            6,
            EncoderResult::InputEmpty,
        );
        encode_utf8_from_utf16_with_output_limit(
            &[0x263A, 0xD83D, 0xDCA9],
            "\u{263A}\u{1F4A9}",
            7,
            EncoderResult::InputEmpty,
        );

        encode_utf8_from_utf16_with_output_limit(
            &[0x263A, 0x00A7],
            "\u{263A}",
            4,
            EncoderResult::OutputFull,
        );
        encode_utf8_from_utf16_with_output_limit(
            &[0x263A, 0x2603],
            "\u{263A}",
            5,
            EncoderResult::OutputFull,
        );
        encode_utf8_from_utf16_with_output_limit(
            &[0x263A, 0xD83D, 0xDCA9],
            "\u{263A}",
            6,
            EncoderResult::OutputFull,
        );

        encode_utf8_from_utf16_with_output_limit(
            &[0xD83D, 0xDE0E, 0x0062],
            "\u{1F60E}\u{62}",
            5,
            EncoderResult::InputEmpty,
        );
        encode_utf8_from_utf16_with_output_limit(
            &[0xD83D, 0xDE0E, 0x00A7],
            "\u{1F60E}\u{A7}",
            6,
            EncoderResult::InputEmpty,
        );
        encode_utf8_from_utf16_with_output_limit(
            &[0xD83D, 0xDE0E, 0x2603],
            "\u{1F60E}\u{2603}",
            7,
            EncoderResult::InputEmpty,
        );
        encode_utf8_from_utf16_with_output_limit(
            &[0xD83D, 0xDE0E, 0xD83D, 0xDCA9],
            "\u{1F60E}\u{1F4A9}",
            8,
            EncoderResult::InputEmpty,
        );

        encode_utf8_from_utf16_with_output_limit(
            &[0xD83D, 0xDE0E, 0x00A7],
            "\u{1F60E}",
            5,
            EncoderResult::OutputFull,
        );
        encode_utf8_from_utf16_with_output_limit(
            &[0xD83D, 0xDE0E, 0x2603],
            "\u{1F60E}",
            6,
            EncoderResult::OutputFull,
        );
        encode_utf8_from_utf16_with_output_limit(
            &[0xD83D, 0xDE0E, 0xD83D, 0xDCA9],
            "\u{1F60E}",
            7,
            EncoderResult::OutputFull,
        );

        encode_utf8_from_utf16_with_output_limit(
            &[0x0063, 0x00B6, 0x0062, 0x0062],
            "\u{63}\u{B6}\u{62}\u{62}",
            5,
            EncoderResult::InputEmpty,
        );
        encode_utf8_from_utf16_with_output_limit(
            &[0x0063, 0x00B6, 0x0062, 0x0062],
            "\u{63}\u{B6}\u{62}",
            4,
            EncoderResult::OutputFull,
        );

        encode_utf8_from_utf16_with_output_limit(
            &[0x0063, 0x00B6, 0x0062, 0x0062, 0x0062],
            "\u{63}\u{B6}\u{62}\u{62}\u{62}",
            6,
            EncoderResult::InputEmpty,
        );
        encode_utf8_from_utf16_with_output_limit(
            &[0x0063, 0x00B6, 0x0062, 0x0062, 0x0062],
            "\u{63}\u{B6}\u{62}\u{62}",
            5,
            EncoderResult::OutputFull,
        );

        encode_utf8_from_utf16_with_output_limit(
            &[0x263A, 0x0062, 0x0062],
            "\u{263A}\u{62}\u{62}",
            5,
            EncoderResult::InputEmpty,
        );
        encode_utf8_from_utf16_with_output_limit(
            &[0x263A, 0x0062, 0x0062],
            "\u{263A}\u{62}",
            4,
            EncoderResult::OutputFull,
        );

        encode_utf8_from_utf16_with_output_limit(
            &[0x263A, 0x0062, 0x0062, 0x0062],
            "\u{263A}\u{62}\u{62}\u{62}",
            6,
            EncoderResult::InputEmpty,
        );
        encode_utf8_from_utf16_with_output_limit(
            &[0x263A, 0x0062, 0x0062, 0x0062],
            "\u{263A}\u{62}\u{62}",
            5,
            EncoderResult::OutputFull,
        );

        encode_utf8_from_utf16_with_output_limit(
            &[0x0063, 0x00B6, 0x00A7],
            "\u{63}\u{B6}\u{A7}",
            5,
            EncoderResult::InputEmpty,
        );
        encode_utf8_from_utf16_with_output_limit(
            &[0x0063, 0x00B6, 0x00A7],
            "\u{63}\u{B6}",
            4,
            EncoderResult::OutputFull,
        );

        encode_utf8_from_utf16_with_output_limit(
            &[0x0063, 0x00B6, 0x00A7, 0x0062],
            "\u{63}\u{B6}\u{A7}\u{62}",
            6,
            EncoderResult::InputEmpty,
        );
        encode_utf8_from_utf16_with_output_limit(
            &[0x0063, 0x00B6, 0x00A7, 0x0062],
            "\u{63}\u{B6}\u{A7}",
            5,
            EncoderResult::OutputFull,
        );

        encode_utf8_from_utf16_with_output_limit(
            &[0x263A, 0x00A7, 0x0062],
            "\u{263A}\u{A7}\u{62}",
            6,
            EncoderResult::InputEmpty,
        );
        encode_utf8_from_utf16_with_output_limit(
            &[0x263A, 0x00A7, 0x0062],
            "\u{263A}\u{A7}",
            5,
            EncoderResult::OutputFull,
        );

        encode_utf8_from_utf16_with_output_limit(
            &[0x0063, 0x00B6, 0x0062, 0x00A7],
            "\u{63}\u{B6}\u{62}\u{A7}",
            6,
            EncoderResult::InputEmpty,
        );
        encode_utf8_from_utf16_with_output_limit(
            &[0x0063, 0x00B6, 0x0062, 0x00A7],
            "\u{63}\u{B6}\u{62}",
            5,
            EncoderResult::OutputFull,
        );

        encode_utf8_from_utf16_with_output_limit(
            &[0x263A, 0x0062, 0x00A7],
            "\u{263A}\u{62}\u{A7}",
            6,
            EncoderResult::InputEmpty,
        );
        encode_utf8_from_utf16_with_output_limit(
            &[0x263A, 0x0062, 0x00A7],
            "\u{263A}\u{62}",
            5,
            EncoderResult::OutputFull,
        );

        encode_utf8_from_utf16_with_output_limit(
            &[0x0063, 0x00B6, 0x2603],
            "\u{63}\u{B6}\u{2603}",
            6,
            EncoderResult::InputEmpty,
        );
        encode_utf8_from_utf16_with_output_limit(
            &[0x0063, 0x00B6, 0x2603],
            "\u{63}\u{B6}",
            5,
            EncoderResult::OutputFull,
        );

        encode_utf8_from_utf16_with_output_limit(
            &[0x263A, 0x2603],
            "\u{263A}\u{2603}",
            6,
            EncoderResult::InputEmpty,
        );
        encode_utf8_from_utf16_with_output_limit(
            &[0x263A, 0x2603],
            "\u{263A}",
            5,
            EncoderResult::OutputFull,
        );

        encode_utf8_from_utf16_with_output_limit(
            &[0x0063, 0x00B6, 0xD83D],
            "\u{63}\u{B6}\u{FFFD}",
            6,
            EncoderResult::InputEmpty,
        );
        encode_utf8_from_utf16_with_output_limit(
            &[0x0063, 0x00B6, 0xD83D],
            "\u{63}\u{B6}",
            5,
            EncoderResult::OutputFull,
        );

        encode_utf8_from_utf16_with_output_limit(
            &[0x263A, 0xD83D],
            "\u{263A}\u{FFFD}",
            6,
            EncoderResult::InputEmpty,
        );
        encode_utf8_from_utf16_with_output_limit(
            &[0x263A, 0xD83D],
            "\u{263A}",
            5,
            EncoderResult::OutputFull,
        );

        encode_utf8_from_utf16_with_output_limit(
            &[0x0063, 0x00B6, 0xDCA9],
            "\u{63}\u{B6}\u{FFFD}",
            6,
            EncoderResult::InputEmpty,
        );
        encode_utf8_from_utf16_with_output_limit(
            &[0x0063, 0x00B6, 0xDCA9],
            "\u{63}\u{B6}",
            5,
            EncoderResult::OutputFull,
        );

        encode_utf8_from_utf16_with_output_limit(
            &[0x263A, 0xDCA9],
            "\u{263A}\u{FFFD}",
            6,
            EncoderResult::InputEmpty,
        );
        encode_utf8_from_utf16_with_output_limit(
            &[0x263A, 0xDCA9],
            "\u{263A}",
            5,
            EncoderResult::OutputFull,
        );
    }

    #[test]
    fn test_utf8_max_length_from_utf16() {
        let mut encoder = UTF_8.new_encoder();
        let mut output = [0u8; 13];
        let input = &[0x2C9Fu16, 0x2CA9u16, 0x2CA3u16, 0x2C9Fu16];
        let needed = encoder
            .max_buffer_length_from_utf16_without_replacement(input.len())
            .unwrap();
        let (result, _, _) =
            encoder.encode_from_utf16_without_replacement(input, &mut output[..needed], true);
        assert_eq!(result, EncoderResult::InputEmpty);
    }

    #[test]
    fn test_decode_bom_prefixed_split_byte_triple() {
        let mut output = [0u16; 20];
        let mut decoder = UTF_8.new_decoder();
        {
            let needed = decoder.max_utf16_buffer_length(1).unwrap();
            let (result, read, written, had_errors) =
                decoder.decode_to_utf16(b"\xEF", &mut output[..needed], false);
            assert_eq!(result, CoderResult::InputEmpty);
            assert_eq!(read, 1);
            assert_eq!(written, 0);
            assert!(!had_errors);
        }
        {
            let needed = decoder.max_utf16_buffer_length(1).unwrap();
            let (result, read, written, had_errors) =
                decoder.decode_to_utf16(b"\xBF", &mut output[..needed], false);
            assert_eq!(result, CoderResult::InputEmpty);
            assert_eq!(read, 1);
            assert_eq!(written, 0);
            assert!(!had_errors);
        }
        {
            let needed = decoder.max_utf16_buffer_length(1).unwrap();
            let (result, read, written, had_errors) =
                decoder.decode_to_utf16(b"\xBE", &mut output[..needed], true);
            assert_eq!(result, CoderResult::InputEmpty);
            assert_eq!(read, 1);
            assert_eq!(written, 1);
            assert!(!had_errors);
            assert_eq!(output[0], 0xFFFE);
        }
    }

    #[test]
    fn test_decode_bom_prefixed_split_byte_pair() {
        let mut output = [0u16; 20];
        let mut decoder = UTF_8.new_decoder();
        {
            let needed = decoder.max_utf16_buffer_length(1).unwrap();
            let (result, read, written, had_errors) =
                decoder.decode_to_utf16(b"\xEF", &mut output[..needed], false);
            assert_eq!(result, CoderResult::InputEmpty);
            assert_eq!(read, 1);
            assert_eq!(written, 0);
            assert!(!had_errors);
        }
        {
            let needed = decoder.max_utf16_buffer_length(1).unwrap();
            let (result, read, written, had_errors) =
                decoder.decode_to_utf16(b"\xBC", &mut output[..needed], true);
            assert_eq!(result, CoderResult::InputEmpty);
            assert_eq!(read, 1);
            assert_eq!(written, 1);
            assert!(had_errors);
            assert_eq!(output[0], 0xFFFD);
        }
    }

    #[test]
    fn test_decode_bom_prefix() {
        let mut output = [0u16; 20];
        let mut decoder = UTF_8.new_decoder();
        {
            let needed = decoder.max_utf16_buffer_length(1).unwrap();
            let (result, read, written, had_errors) =
                decoder.decode_to_utf16(b"\xEF", &mut output[..needed], true);
            assert_eq!(result, CoderResult::InputEmpty);
            assert_eq!(read, 1);
            assert_eq!(written, 1);
            assert!(had_errors);
            assert_eq!(output[0], 0xFFFD);
        }
    }

    #[test]
    fn test_tail() {
        let mut output = [0u16; 1];
        let mut decoder = UTF_8.new_decoder_without_bom_handling();
        {
            let (result, read, written, had_errors) =
                decoder.decode_to_utf16("\u{E4}a".as_bytes(), &mut output[..], false);
            assert_eq!(result, CoderResult::OutputFull);
            assert_eq!(read, 2);
            assert_eq!(written, 1);
            assert!(!had_errors);
            assert_eq!(output[0], 0x00E4);
        }
    }
}
