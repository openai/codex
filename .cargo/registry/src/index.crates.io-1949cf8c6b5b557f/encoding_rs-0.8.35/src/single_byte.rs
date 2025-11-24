// Copyright Mozilla Foundation. See the COPYRIGHT
// file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use super::*;
use crate::ascii::*;
use crate::data::position;
use crate::handles::*;
use crate::variant::*;

pub struct SingleByteDecoder {
    table: &'static [u16; 128],
}

impl SingleByteDecoder {
    pub fn new(data: &'static [u16; 128]) -> VariantDecoder {
        VariantDecoder::SingleByte(SingleByteDecoder { table: data })
    }

    pub fn max_utf16_buffer_length(&self, byte_length: usize) -> Option<usize> {
        Some(byte_length)
    }

    pub fn max_utf8_buffer_length_without_replacement(&self, byte_length: usize) -> Option<usize> {
        byte_length.checked_mul(3)
    }

    pub fn max_utf8_buffer_length(&self, byte_length: usize) -> Option<usize> {
        byte_length.checked_mul(3)
    }

    pub fn decode_to_utf8_raw(
        &mut self,
        src: &[u8],
        dst: &mut [u8],
        _last: bool,
    ) -> (DecoderResult, usize, usize) {
        let mut source = ByteSource::new(src);
        let mut dest = Utf8Destination::new(dst);
        'outermost: loop {
            match dest.copy_ascii_from_check_space_bmp(&mut source) {
                CopyAsciiResult::Stop(ret) => return ret,
                CopyAsciiResult::GoOn((mut non_ascii, mut handle)) => 'middle: loop {
                    // Start non-boilerplate
                    //
                    // Since the non-ASCIIness of `non_ascii` is hidden from
                    // the optimizer, it can't figure out that it's OK to
                    // statically omit the bound check when accessing
                    // `[u16; 128]` with an index
                    // `non_ascii as usize - 0x80usize`.
                    //
                    // Safety: `non_ascii` is a u8 byte >=0x80, from the invariants
                    // on Utf8Destination::copy_ascii_from_check_space_bmp()
                    let mapped =
                        unsafe { *(self.table.get_unchecked(non_ascii as usize - 0x80usize)) };
                    // let mapped = self.table[non_ascii as usize - 0x80usize];
                    if mapped == 0u16 {
                        return (
                            DecoderResult::Malformed(1, 0),
                            source.consumed(),
                            handle.written(),
                        );
                    }
                    let dest_again = handle.write_bmp_excl_ascii(mapped);
                    // End non-boilerplate
                    match source.check_available() {
                        Space::Full(src_consumed) => {
                            return (
                                DecoderResult::InputEmpty,
                                src_consumed,
                                dest_again.written(),
                            );
                        }
                        Space::Available(source_handle) => {
                            match dest_again.check_space_bmp() {
                                Space::Full(dst_written) => {
                                    return (
                                        DecoderResult::OutputFull,
                                        source_handle.consumed(),
                                        dst_written,
                                    );
                                }
                                Space::Available(mut destination_handle) => {
                                    let (mut b, unread_handle) = source_handle.read();
                                    let source_again = unread_handle.commit();
                                    'innermost: loop {
                                        if b > 127 {
                                            non_ascii = b;
                                            handle = destination_handle;
                                            continue 'middle;
                                        }
                                        // Testing on Haswell says that we should write the
                                        // byte unconditionally instead of trying to unread it
                                        // to make it part of the next SIMD stride.
                                        let dest_again_again = destination_handle.write_ascii(b);
                                        if b < 60 {
                                            // We've got punctuation
                                            match source_again.check_available() {
                                                Space::Full(src_consumed_again) => {
                                                    return (
                                                        DecoderResult::InputEmpty,
                                                        src_consumed_again,
                                                        dest_again_again.written(),
                                                    );
                                                }
                                                Space::Available(source_handle_again) => {
                                                    match dest_again_again.check_space_bmp() {
                                                        Space::Full(dst_written_again) => {
                                                            return (
                                                                DecoderResult::OutputFull,
                                                                source_handle_again.consumed(),
                                                                dst_written_again,
                                                            );
                                                        }
                                                        Space::Available(
                                                            destination_handle_again,
                                                        ) => {
                                                            let (b_again, _unread_handle_again) =
                                                                source_handle_again.read();
                                                            b = b_again;
                                                            destination_handle =
                                                                destination_handle_again;
                                                            continue 'innermost;
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        // We've got markup or ASCII text
                                        continue 'outermost;
                                    }
                                }
                            }
                        }
                    }
                },
            }
        }
    }

    pub fn decode_to_utf16_raw(
        &mut self,
        src: &[u8],
        dst: &mut [u16],
        _last: bool,
    ) -> (DecoderResult, usize, usize) {
        let (pending, length) = if dst.len() < src.len() {
            (DecoderResult::OutputFull, dst.len())
        } else {
            (DecoderResult::InputEmpty, src.len())
        };
        // Safety invariant: converted <= length. Quite often we have `converted < length`
        // which will be separately marked.
        let mut converted = 0usize;
        'outermost: loop {
            match unsafe {
                // Safety: length is the minimum length, `src/dst + x` will always be valid for reads/writes of `len - x`
                ascii_to_basic_latin(
                    src.as_ptr().add(converted),
                    dst.as_mut_ptr().add(converted),
                    length - converted,
                )
            } {
                None => {
                    return (pending, length, length);
                }
                Some((mut non_ascii, consumed)) => {
                    // Safety invariant: `converted <= length` upheld, since this can only consume
                    // up to `length - converted` bytes.
                    //
                    // Furthermore, in this context,
                    // we can assume `converted < length` since this branch is only ever hit when
                    // ascii_to_basic_latin fails to consume the entire slice
                    converted += consumed;
                    'middle: loop {
                        // `converted` doesn't count the reading of `non_ascii` yet.
                        // Since the non-ASCIIness of `non_ascii` is hidden from
                        // the optimizer, it can't figure out that it's OK to
                        // statically omit the bound check when accessing
                        // `[u16; 128]` with an index
                        // `non_ascii as usize - 0x80usize`.
                        //
                        // Safety: We can rely on `non_ascii` being between `0x80` and `0xFF` due to
                        // the invariants of `ascii_to_basic_latin()`, and our table has enough space for that.
                        let mapped =
                            unsafe { *(self.table.get_unchecked(non_ascii as usize - 0x80usize)) };
                        // let mapped = self.table[non_ascii as usize - 0x80usize];
                        if mapped == 0u16 {
                            return (
                                DecoderResult::Malformed(1, 0),
                                converted + 1, // +1 `for non_ascii`
                                converted,
                            );
                        }
                        unsafe {
                            // Safety: As mentioned above, `converted < length`
                            *(dst.get_unchecked_mut(converted)) = mapped;
                        }
                        // Safety: `converted <= length` upheld, since `converted < length` before this
                        converted += 1;
                        // Next, handle ASCII punctuation and non-ASCII without
                        // going back to ASCII acceleration. Non-ASCII scripts
                        // use ASCII punctuation, so this avoid going to
                        // acceleration just for punctuation/space and then
                        // failing. This is a significant boost to non-ASCII
                        // scripts.
                        // TODO: Split out Latin converters without this part
                        // this stuff makes Latin script-conversion slower.
                        if converted == length {
                            return (pending, length, length);
                        }
                        // Safety: We are back to `converted < length` because of the == above
                        // and can perform this check.
                        let mut b = unsafe { *(src.get_unchecked(converted)) };
                        // Safety: `converted < length` is upheld for this loop
                        'innermost: loop {
                            if b > 127 {
                                non_ascii = b;
                                continue 'middle;
                            }
                            // Testing on Haswell says that we should write the
                            // byte unconditionally instead of trying to unread it
                            // to make it part of the next SIMD stride.
                            unsafe {
                                // Safety: `converted < length` is true for this loop
                                *(dst.get_unchecked_mut(converted)) = u16::from(b);
                            }
                            // Safety: We are now at `converted <= length`. We should *not* `continue`
                            // the loop without reverifying
                            converted += 1;
                            if b < 60 {
                                // We've got punctuation
                                if converted == length {
                                    return (pending, length, length);
                                }
                                // Safety: we're back to `converted <= length` because of the == above
                                b = unsafe { *(src.get_unchecked(converted)) };
                                // Safety: The loop continues as `converted < length`
                                continue 'innermost;
                            }
                            // We've got markup or ASCII text
                            continue 'outermost;
                        }
                    }
                }
            }
        }
    }

    pub fn latin1_byte_compatible_up_to(&self, buffer: &[u8]) -> usize {
        let mut bytes = buffer;
        let mut total = 0;
        loop {
            if let Some((non_ascii, offset)) = validate_ascii(bytes) {
                total += offset;
                // Safety: We can rely on `non_ascii` being between `0x80` and `0xFF` due to
                // the invariants of `ascii_to_basic_latin()`, and our table has enough space for that.
                let mapped = unsafe { *(self.table.get_unchecked(non_ascii as usize - 0x80usize)) };
                if mapped != u16::from(non_ascii) {
                    return total;
                }
                total += 1;
                bytes = &bytes[offset + 1..];
            } else {
                return total;
            }
        }
    }
}

pub struct SingleByteEncoder {
    table: &'static [u16; 128],
    run_bmp_offset: usize,
    run_byte_offset: usize,
    run_length: usize,
}

impl SingleByteEncoder {
    pub fn new(
        encoding: &'static Encoding,
        data: &'static [u16; 128],
        run_bmp_offset: u16,
        run_byte_offset: u8,
        run_length: u8,
    ) -> Encoder {
        Encoder::new(
            encoding,
            VariantEncoder::SingleByte(SingleByteEncoder {
                table: data,
                run_bmp_offset: run_bmp_offset as usize,
                run_byte_offset: run_byte_offset as usize,
                run_length: run_length as usize,
            }),
        )
    }

    pub fn max_buffer_length_from_utf16_without_replacement(
        &self,
        u16_length: usize,
    ) -> Option<usize> {
        Some(u16_length)
    }

    pub fn max_buffer_length_from_utf8_without_replacement(
        &self,
        byte_length: usize,
    ) -> Option<usize> {
        Some(byte_length)
    }

    #[inline(always)]
    fn encode_u16(&self, code_unit: u16) -> Option<u8> {
        // First, we see if the code unit falls into a run of consecutive
        // code units that can be mapped by offset. This is very efficient
        // for most non-Latin encodings as well as Latin1-ish encodings.
        //
        // For encodings that don't fit this pattern, the run (which may
        // have the length of just one) just establishes the starting point
        // for the next rule.
        //
        // Next, we do a forward linear search in the part of the index
        // after the run. Even in non-Latin1-ish Latin encodings (except
        // macintosh), the lower case letters are here.
        //
        // Next, we search the third quadrant up to the start of the run
        // (upper case letters in Latin encodings except macintosh, in
        // Greek and in KOI encodings) and then the second quadrant,
        // except if the run stared before the third quadrant, we search
        // the second quadrant up to the run.
        //
        // Last, we search the first quadrant, which has unused controls
        // or punctuation in most encodings. This is bad for macintosh
        // and IBM866, but those are rare.

        // Run of consecutive units
        let unit_as_usize = code_unit as usize;
        let offset = unit_as_usize.wrapping_sub(self.run_bmp_offset);
        if offset < self.run_length {
            return Some((128 + self.run_byte_offset + offset) as u8);
        }

        // Search after the run
        let tail_start = self.run_byte_offset + self.run_length;
        if let Some(pos) = position(&self.table[tail_start..], code_unit) {
            return Some((128 + tail_start + pos) as u8);
        }

        if self.run_byte_offset >= 64 {
            // Search third quadrant before the run
            if let Some(pos) = position(&self.table[64..self.run_byte_offset], code_unit) {
                return Some(((128 + 64) + pos) as u8);
            }

            // Search second quadrant
            if let Some(pos) = position(&self.table[32..64], code_unit) {
                return Some(((128 + 32) + pos) as u8);
            }
        } else if let Some(pos) = position(&self.table[32..self.run_byte_offset], code_unit) {
            // windows-1252, windows-874, ISO-8859-15 and ISO-8859-5
            // Search second quadrant before the run
            return Some(((128 + 32) + pos) as u8);
        }

        // Search first quadrant
        if let Some(pos) = position(&self.table[..32], code_unit) {
            return Some((128 + pos) as u8);
        }

        None
    }

    ascii_compatible_bmp_encoder_function!(
        {
            match self.encode_u16(bmp) {
                Some(byte) => handle.write_one(byte),
                None => {
                    return (
                        EncoderResult::unmappable_from_bmp(bmp),
                        source.consumed(),
                        handle.written(),
                    );
                }
            }
        },
        bmp,
        self,
        source,
        handle,
        copy_ascii_to_check_space_one,
        check_space_one,
        encode_from_utf8_raw,
        str,
        Utf8Source,
        true
    );

    pub fn encode_from_utf16_raw(
        &mut self,
        src: &[u16],
        dst: &mut [u8],
        _last: bool,
    ) -> (EncoderResult, usize, usize) {
        let (pending, length) = if dst.len() < src.len() {
            (EncoderResult::OutputFull, dst.len())
        } else {
            (EncoderResult::InputEmpty, src.len())
        };
        // Safety invariant: converted <= length. Quite often we have `converted < length`
        // which will be separately marked.
        let mut converted = 0usize;
        'outermost: loop {
            match unsafe {
                // Safety: length is the minimum length, `src/dst + x` will always be valid for reads/writes of `len - x`
                basic_latin_to_ascii(
                    src.as_ptr().add(converted),
                    dst.as_mut_ptr().add(converted),
                    length - converted,
                )
            } {
                None => {
                    return (pending, length, length);
                }
                Some((mut non_ascii, consumed)) => {
                    // Safety invariant: `converted <= length` upheld, since this can only consume
                    // up to `length - converted` bytes.
                    //
                    // Furthermore, in this context,
                    // we can assume `converted < length` since this branch is only ever hit when
                    // ascii_to_basic_latin fails to consume the entire slice
                    converted += consumed;
                    'middle: loop {
                        // `converted` doesn't count the reading of `non_ascii` yet.
                        match self.encode_u16(non_ascii) {
                            Some(byte) => {
                                unsafe {
                                    // Safety: we're allowed this access since `converted < length`
                                    *(dst.get_unchecked_mut(converted)) = byte;
                                }
                                converted += 1;
                                // `converted <= length` now
                            }
                            None => {
                                // At this point, we need to know if we
                                // have a surrogate.
                                let high_bits = non_ascii & 0xFC00u16;
                                if high_bits == 0xD800u16 {
                                    // high surrogate
                                    if converted + 1 == length {
                                        // End of buffer. This surrogate is unpaired.
                                        return (
                                            EncoderResult::Unmappable('\u{FFFD}'),
                                            converted + 1, // +1 `for non_ascii`
                                            converted,
                                        );
                                    }
                                    // Safety: convered < length from outside the match, and `converted + 1 != length`,
                                    // So `converted + 1 < length` as well. We're in bounds
                                    let second =
                                        u32::from(unsafe { *src.get_unchecked(converted + 1) });
                                    if second & 0xFC00u32 != 0xDC00u32 {
                                        return (
                                            EncoderResult::Unmappable('\u{FFFD}'),
                                            converted + 1, // +1 `for non_ascii`
                                            converted,
                                        );
                                    }
                                    // The next code unit is a low surrogate.
                                    let astral: char = unsafe {
                                        // Safety: We can rely on non_ascii being 0xD800-0xDBFF since the high bits are 0xD800
                                        // Then, (non_ascii << 10 - 0xD800 << 10) becomes between (0 to 0x3FF) << 10, which is between
                                        // 0x400 to 0xffc00. Adding the 0x10000 gives a range of 0x10400 to 0x10fc00. Subtracting the 0xDC00
                                        // gives 0x2800 to 0x102000
                                        // The second term is between 0xDC00 and 0xDFFF from the check above. This gives a maximum
                                        // possible range of (0x10400 + 0xDC00) to (0x102000 + 0xDFFF) which is 0x1E000 to 0x10ffff.
                                        // This is in range.
                                        //
                                        // From a Unicode principles perspective this can also be verified as we have checked that `non_ascii` is a high surrogate
                                        // (0xD800..=0xDBFF), and that `second` is a low surrogate (`0xDC00..=0xDFFF`), and we are applying reverse of the UTC16 transformation
                                        // algorithm <https://en.wikipedia.org/wiki/UTF-16#Code_points_from_U+010000_to_U+10FFFF>, by applying the high surrogate - 0xD800 to the
                                        // high ten bits, and the low surrogate - 0xDc00 to the low ten bits, and then adding 0x10000
                                        ::core::char::from_u32_unchecked(
                                            (u32::from(non_ascii) << 10) + second
                                                - (((0xD800u32 << 10) - 0x1_0000u32) + 0xDC00u32),
                                        )
                                    };
                                    return (
                                        EncoderResult::Unmappable(astral),
                                        converted + 2, // +2 `for non_ascii` and `second`
                                        converted,
                                    );
                                }
                                if high_bits == 0xDC00u16 {
                                    // Unpaired low surrogate
                                    return (
                                        EncoderResult::Unmappable('\u{FFFD}'),
                                        converted + 1, // +1 `for non_ascii`
                                        converted,
                                    );
                                }
                                return (
                                    EncoderResult::unmappable_from_bmp(non_ascii),
                                    converted + 1, // +1 `for non_ascii`
                                    converted,
                                );
                                // Safety: This branch diverges, so no need to uphold invariants on `converted`
                            }
                        }
                        // Next, handle ASCII punctuation and non-ASCII without
                        // going back to ASCII acceleration. Non-ASCII scripts
                        // use ASCII punctuation, so this avoid going to
                        // acceleration just for punctuation/space and then
                        // failing. This is a significant boost to non-ASCII
                        // scripts.
                        // TODO: Split out Latin converters without this part
                        // this stuff makes Latin script-conversion slower.
                        if converted == length {
                            return (pending, length, length);
                        }
                        // Safety: we're back to `converted < length` due to the == above and can perform
                        // the unchecked read
                        let mut unit = unsafe { *(src.get_unchecked(converted)) };
                        'innermost: loop {
                            // Safety: This loop always begins with `converted < length`, see
                            // the invariant outside and the comment on the continue below
                            if unit > 127 {
                                non_ascii = unit;
                                continue 'middle;
                            }
                            // Testing on Haswell says that we should write the
                            // byte unconditionally instead of trying to unread it
                            // to make it part of the next SIMD stride.
                            unsafe {
                                // Safety: Can rely on converted < length
                                *(dst.get_unchecked_mut(converted)) = unit as u8;
                            }
                            converted += 1;
                            // `converted <= length` here
                            if unit < 60 {
                                // We've got punctuation
                                if converted == length {
                                    return (pending, length, length);
                                }
                                // Safety: `converted < length` due to the == above. The read is safe.
                                unit = unsafe { *(src.get_unchecked(converted)) };
                                // Safety: This only happens if `converted < length`, maintaining it
                                continue 'innermost;
                            }
                            // We've got markup or ASCII text
                            continue 'outermost;
                            // Safety: All other routes to here diverge so the continue is the only
                            // way to run the innermost loop.
                        }
                    }
                }
            }
        }
    }
}

// Any copyright to the test code below this comment is dedicated to the
// Public Domain. http://creativecommons.org/publicdomain/zero/1.0/

#[cfg(all(test, feature = "alloc"))]
mod tests {
    use super::super::testing::*;
    use super::super::*;

    #[test]
    fn test_windows_1255_ca() {
        decode(WINDOWS_1255, b"\xCA", "\u{05BA}");
        encode(WINDOWS_1255, "\u{05BA}", b"\xCA");
    }

    #[test]
    fn test_ascii_punctuation() {
        let bytes = b"\xC1\xF5\xF4\xFC \xE5\xDF\xED\xE1\xE9 \xDD\xED\xE1 \xF4\xE5\xF3\xF4. \xC1\xF5\xF4\xFC \xE5\xDF\xED\xE1\xE9 \xDD\xED\xE1 \xF4\xE5\xF3\xF4.";
        let characters = "\u{0391}\u{03C5}\u{03C4}\u{03CC} \
                          \u{03B5}\u{03AF}\u{03BD}\u{03B1}\u{03B9} \u{03AD}\u{03BD}\u{03B1} \
                          \u{03C4}\u{03B5}\u{03C3}\u{03C4}. \u{0391}\u{03C5}\u{03C4}\u{03CC} \
                          \u{03B5}\u{03AF}\u{03BD}\u{03B1}\u{03B9} \u{03AD}\u{03BD}\u{03B1} \
                          \u{03C4}\u{03B5}\u{03C3}\u{03C4}.";
        decode(WINDOWS_1253, bytes, characters);
        encode(WINDOWS_1253, characters, bytes);
    }

    #[test]
    fn test_decode_malformed() {
        decode(
            WINDOWS_1253,
            b"\xC1\xF5\xD2\xF4\xFC",
            "\u{0391}\u{03C5}\u{FFFD}\u{03C4}\u{03CC}",
        );
    }

    #[test]
    fn test_encode_unmappables() {
        encode(
            WINDOWS_1253,
            "\u{0391}\u{03C5}\u{2603}\u{03C4}\u{03CC}",
            b"\xC1\xF5&#9731;\xF4\xFC",
        );
        encode(
            WINDOWS_1253,
            "\u{0391}\u{03C5}\u{1F4A9}\u{03C4}\u{03CC}",
            b"\xC1\xF5&#128169;\xF4\xFC",
        );
    }

    #[test]
    fn test_encode_unpaired_surrogates() {
        encode_from_utf16(
            WINDOWS_1253,
            &[0x0391u16, 0x03C5u16, 0xDCA9u16, 0x03C4u16, 0x03CCu16],
            b"\xC1\xF5&#65533;\xF4\xFC",
        );
        encode_from_utf16(
            WINDOWS_1253,
            &[0x0391u16, 0x03C5u16, 0xD83Du16, 0x03C4u16, 0x03CCu16],
            b"\xC1\xF5&#65533;\xF4\xFC",
        );
        encode_from_utf16(
            WINDOWS_1253,
            &[0x0391u16, 0x03C5u16, 0x03C4u16, 0x03CCu16, 0xD83Du16],
            b"\xC1\xF5\xF4\xFC&#65533;",
        );
    }

    pub const HIGH_BYTES: &'static [u8; 128] = &[
        0x80, 0x81, 0x82, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88, 0x89, 0x8A, 0x8B, 0x8C, 0x8D, 0x8E,
        0x8F, 0x90, 0x91, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9A, 0x9B, 0x9C, 0x9D,
        0x9E, 0x9F, 0xA0, 0xA1, 0xA2, 0xA3, 0xA4, 0xA5, 0xA6, 0xA7, 0xA8, 0xA9, 0xAA, 0xAB, 0xAC,
        0xAD, 0xAE, 0xAF, 0xB0, 0xB1, 0xB2, 0xB3, 0xB4, 0xB5, 0xB6, 0xB7, 0xB8, 0xB9, 0xBA, 0xBB,
        0xBC, 0xBD, 0xBE, 0xBF, 0xC0, 0xC1, 0xC2, 0xC3, 0xC4, 0xC5, 0xC6, 0xC7, 0xC8, 0xC9, 0xCA,
        0xCB, 0xCC, 0xCD, 0xCE, 0xCF, 0xD0, 0xD1, 0xD2, 0xD3, 0xD4, 0xD5, 0xD6, 0xD7, 0xD8, 0xD9,
        0xDA, 0xDB, 0xDC, 0xDD, 0xDE, 0xDF, 0xE0, 0xE1, 0xE2, 0xE3, 0xE4, 0xE5, 0xE6, 0xE7, 0xE8,
        0xE9, 0xEA, 0xEB, 0xEC, 0xED, 0xEE, 0xEF, 0xF0, 0xF1, 0xF2, 0xF3, 0xF4, 0xF5, 0xF6, 0xF7,
        0xF8, 0xF9, 0xFA, 0xFB, 0xFC, 0xFD, 0xFE, 0xFF,
    ];

    fn decode_single_byte(encoding: &'static Encoding, data: &'static [u16; 128]) {
        let mut with_replacement = [0u16; 128];
        let mut it = data.iter().enumerate();
        loop {
            match it.next() {
                Some((i, code_point)) => {
                    if *code_point == 0 {
                        with_replacement[i] = 0xFFFD;
                    } else {
                        with_replacement[i] = *code_point;
                    }
                }
                None => {
                    break;
                }
            }
        }

        decode_to_utf16(encoding, HIGH_BYTES, &with_replacement[..]);
    }

    fn encode_single_byte(encoding: &'static Encoding, data: &'static [u16; 128]) {
        let mut with_zeros = [0u8; 128];
        let mut it = data.iter().enumerate();
        loop {
            match it.next() {
                Some((i, code_point)) => {
                    if *code_point == 0 {
                        with_zeros[i] = 0;
                    } else {
                        with_zeros[i] = HIGH_BYTES[i];
                    }
                }
                None => {
                    break;
                }
            }
        }

        encode_from_utf16(encoding, data, &with_zeros[..]);
    }

    #[test]
    fn test_single_byte_from_two_low_surrogates() {
        let expectation = b"&#65533;&#65533;";
        let mut output = [0u8; 40];
        let mut encoder = WINDOWS_1253.new_encoder();
        let (result, read, written, had_errors) =
            encoder.encode_from_utf16(&[0xDC00u16, 0xDEDEu16], &mut output[..], true);
        assert_eq!(result, CoderResult::InputEmpty);
        assert_eq!(read, 2);
        assert_eq!(written, expectation.len());
        assert!(had_errors);
        assert_eq!(&output[..written], expectation);
    }

    // These tests are so self-referential that they are pretty useless.

    // BEGIN GENERATED CODE. PLEASE DO NOT EDIT.
    // Instead, please regenerate using generate-encoding-data.py

    #[test]
    fn test_single_byte_decode() {
        decode_single_byte(IBM866, &data::SINGLE_BYTE_DATA.ibm866);
        decode_single_byte(ISO_8859_10, &data::SINGLE_BYTE_DATA.iso_8859_10);
        if cfg!(miri) {
            // Miri is too slow
            return;
        }
        decode_single_byte(ISO_8859_13, &data::SINGLE_BYTE_DATA.iso_8859_13);
        decode_single_byte(ISO_8859_14, &data::SINGLE_BYTE_DATA.iso_8859_14);
        decode_single_byte(ISO_8859_15, &data::SINGLE_BYTE_DATA.iso_8859_15);
        decode_single_byte(ISO_8859_16, &data::SINGLE_BYTE_DATA.iso_8859_16);
        decode_single_byte(ISO_8859_2, &data::SINGLE_BYTE_DATA.iso_8859_2);
        decode_single_byte(ISO_8859_3, &data::SINGLE_BYTE_DATA.iso_8859_3);
        decode_single_byte(ISO_8859_4, &data::SINGLE_BYTE_DATA.iso_8859_4);
        decode_single_byte(ISO_8859_5, &data::SINGLE_BYTE_DATA.iso_8859_5);
        decode_single_byte(ISO_8859_6, &data::SINGLE_BYTE_DATA.iso_8859_6);
        decode_single_byte(ISO_8859_7, &data::SINGLE_BYTE_DATA.iso_8859_7);
        decode_single_byte(ISO_8859_8, &data::SINGLE_BYTE_DATA.iso_8859_8);
        decode_single_byte(KOI8_R, &data::SINGLE_BYTE_DATA.koi8_r);
        decode_single_byte(KOI8_U, &data::SINGLE_BYTE_DATA.koi8_u);
        decode_single_byte(MACINTOSH, &data::SINGLE_BYTE_DATA.macintosh);
        decode_single_byte(WINDOWS_1250, &data::SINGLE_BYTE_DATA.windows_1250);
        decode_single_byte(WINDOWS_1251, &data::SINGLE_BYTE_DATA.windows_1251);
        decode_single_byte(WINDOWS_1252, &data::SINGLE_BYTE_DATA.windows_1252);
        decode_single_byte(WINDOWS_1253, &data::SINGLE_BYTE_DATA.windows_1253);
        decode_single_byte(WINDOWS_1254, &data::SINGLE_BYTE_DATA.windows_1254);
        decode_single_byte(WINDOWS_1255, &data::SINGLE_BYTE_DATA.windows_1255);
        decode_single_byte(WINDOWS_1256, &data::SINGLE_BYTE_DATA.windows_1256);
        decode_single_byte(WINDOWS_1257, &data::SINGLE_BYTE_DATA.windows_1257);
        decode_single_byte(WINDOWS_1258, &data::SINGLE_BYTE_DATA.windows_1258);
        decode_single_byte(WINDOWS_874, &data::SINGLE_BYTE_DATA.windows_874);
        decode_single_byte(X_MAC_CYRILLIC, &data::SINGLE_BYTE_DATA.x_mac_cyrillic);
    }

    #[test]
    fn test_single_byte_encode() {
        encode_single_byte(IBM866, &data::SINGLE_BYTE_DATA.ibm866);
        encode_single_byte(ISO_8859_10, &data::SINGLE_BYTE_DATA.iso_8859_10);
        if cfg!(miri) {
            // Miri is too slow
            return;
        }
        encode_single_byte(ISO_8859_13, &data::SINGLE_BYTE_DATA.iso_8859_13);
        encode_single_byte(ISO_8859_14, &data::SINGLE_BYTE_DATA.iso_8859_14);
        encode_single_byte(ISO_8859_15, &data::SINGLE_BYTE_DATA.iso_8859_15);
        encode_single_byte(ISO_8859_16, &data::SINGLE_BYTE_DATA.iso_8859_16);
        encode_single_byte(ISO_8859_2, &data::SINGLE_BYTE_DATA.iso_8859_2);
        encode_single_byte(ISO_8859_3, &data::SINGLE_BYTE_DATA.iso_8859_3);
        encode_single_byte(ISO_8859_4, &data::SINGLE_BYTE_DATA.iso_8859_4);
        encode_single_byte(ISO_8859_5, &data::SINGLE_BYTE_DATA.iso_8859_5);
        encode_single_byte(ISO_8859_6, &data::SINGLE_BYTE_DATA.iso_8859_6);
        encode_single_byte(ISO_8859_7, &data::SINGLE_BYTE_DATA.iso_8859_7);
        encode_single_byte(ISO_8859_8, &data::SINGLE_BYTE_DATA.iso_8859_8);
        encode_single_byte(KOI8_R, &data::SINGLE_BYTE_DATA.koi8_r);
        encode_single_byte(KOI8_U, &data::SINGLE_BYTE_DATA.koi8_u);
        encode_single_byte(MACINTOSH, &data::SINGLE_BYTE_DATA.macintosh);
        encode_single_byte(WINDOWS_1250, &data::SINGLE_BYTE_DATA.windows_1250);
        encode_single_byte(WINDOWS_1251, &data::SINGLE_BYTE_DATA.windows_1251);
        encode_single_byte(WINDOWS_1252, &data::SINGLE_BYTE_DATA.windows_1252);
        encode_single_byte(WINDOWS_1253, &data::SINGLE_BYTE_DATA.windows_1253);
        encode_single_byte(WINDOWS_1254, &data::SINGLE_BYTE_DATA.windows_1254);
        encode_single_byte(WINDOWS_1255, &data::SINGLE_BYTE_DATA.windows_1255);
        encode_single_byte(WINDOWS_1256, &data::SINGLE_BYTE_DATA.windows_1256);
        encode_single_byte(WINDOWS_1257, &data::SINGLE_BYTE_DATA.windows_1257);
        encode_single_byte(WINDOWS_1258, &data::SINGLE_BYTE_DATA.windows_1258);
        encode_single_byte(WINDOWS_874, &data::SINGLE_BYTE_DATA.windows_874);
        encode_single_byte(X_MAC_CYRILLIC, &data::SINGLE_BYTE_DATA.x_mac_cyrillic);
    }
    // END GENERATED CODE
}
