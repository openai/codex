// Copyright Mozilla Foundation. See the COPYRIGHT
// file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use super::*;
use crate::data::*;
use crate::handles::*;
use crate::variant::*;
// Rust 1.14.0 requires the following despite the asterisk above.
use super::in_inclusive_range32;

pub struct Big5Decoder {
    lead: Option<u8>,
}

impl Big5Decoder {
    pub fn new() -> VariantDecoder {
        VariantDecoder::Big5(Big5Decoder { lead: None })
    }

    pub fn in_neutral_state(&self) -> bool {
        self.lead.is_none()
    }

    fn plus_one_if_lead(&self, byte_length: usize) -> Option<usize> {
        byte_length.checked_add(match self.lead {
            None => 0,
            Some(_) => 1,
        })
    }

    pub fn max_utf16_buffer_length(&self, byte_length: usize) -> Option<usize> {
        // If there is a lead but the next byte isn't a valid trail, an
        // error is generated for the lead (+1). Then another iteration checks
        // space, which needs +1 to account for the possibility of astral
        // output or combining pair.
        checked_add(1, self.plus_one_if_lead(byte_length))
    }

    pub fn max_utf8_buffer_length_without_replacement(&self, byte_length: usize) -> Option<usize> {
        // No need to account for REPLACEMENT CHARACTERS.
        // Cases:
        // ASCII: 1 to 1
        // Valid pair: 2 to 2, 2 to 3 or 2 to 4, i.e. worst case 2 to 4
        // lead set and first byte is trail: 1 to 4 worst case
        //
        // When checking for space for the last byte:
        // no lead: the last byte must be ASCII (or fatal error): 1 to 1
        // lead set: space for 4 bytes was already checked when reading the
        // lead, hence the last lead and the last trail together are worst
        // case 2 to 4.
        //
        // If lead set and the input is a single trail byte, the worst-case
        // output is 4, so we need to add one before multiplying if lead is
        // set.
        //
        // Finally, add two so that if input is non-zero, the output is at
        // least 4.
        checked_add(2, checked_mul(2, self.plus_one_if_lead(byte_length)))
    }

    pub fn max_utf8_buffer_length(&self, byte_length: usize) -> Option<usize> {
        // If there is a lead but the next byte isn't a valid trail, an
        // error is generated for the lead (+(1*3)). Then another iteration
        // checks space, which needs +3 to account for the possibility of astral
        // output or combining pair. In between start and end, the worst case
        // is that every byte is bad: *3.
        checked_add(3, checked_mul(3, self.plus_one_if_lead(byte_length)))
    }

    ascii_compatible_two_byte_decoder_functions!(
        {
            // If lead is between 0x81 and 0xFE, inclusive,
            // subtract offset 0x81.
            let non_ascii_minus_offset =
                non_ascii.wrapping_sub(0x81);
            if non_ascii_minus_offset > (0xFE - 0x81) {
                return (DecoderResult::Malformed(1, 0),
                        source.consumed(),
                        handle.written());
            }
            non_ascii_minus_offset
        },
        {
            // If trail is between 0x40 and 0x7E, inclusive,
            // subtract offset 0x40. Else if trail is
            // between 0xA1 and 0xFE, inclusive, subtract
            // offset 0x62.
            // TODO: Find out which range is more probable.
            let mut trail_minus_offset =
                byte.wrapping_sub(0x40);
            if trail_minus_offset > (0x7E - 0x40) {
                let trail_minus_range_start =
                    byte.wrapping_sub(0xA1);
                if trail_minus_range_start >
                   (0xFE - 0xA1) {
                    if byte < 0x80 {
                        return (DecoderResult::Malformed(1, 0),
                                unread_handle_trail.unread(),
                                handle.written());
                    }
                    return (DecoderResult::Malformed(2, 0),
                            unread_handle_trail.consumed(),
                            handle.written());
                }
                trail_minus_offset = byte - 0x62;
            }
            let pointer = lead_minus_offset as usize *
                          157usize +
                          trail_minus_offset as usize;
            let rebased_pointer = pointer.wrapping_sub(942);
            let low_bits = big5_low_bits(rebased_pointer);
            if low_bits == 0 {
                match pointer {
                    1133 => {
                        handle.write_big5_combination(0x00CAu16,
                                                      0x0304u16)
                    }
                    1135 => {
                        handle.write_big5_combination(0x00CAu16,
                                                      0x030Cu16)
                    }
                    1164 => {
                        handle.write_big5_combination(0x00EAu16,
                                                      0x0304u16)
                    }
                    1166 => {
                        handle.write_big5_combination(0x00EAu16,
                                                      0x030Cu16)
                    }
                    _ => {
                        if byte < 0x80 {
                            return (DecoderResult::Malformed(1, 0),
                                    unread_handle_trail.unread(),
                                    handle.written());
                        }
                        return (DecoderResult::Malformed(2, 0),
                                unread_handle_trail.consumed(),
                                handle.written());
                    }
                }
            } else if big5_is_astral(rebased_pointer) {
                handle.write_astral(u32::from(low_bits) |
                                    0x20000u32)
            } else {
                handle.write_bmp_excl_ascii(low_bits)
            }
        },
        self,
        non_ascii,
        byte,
        lead_minus_offset,
        unread_handle_trail,
        source,
        handle,
        'outermost,
        copy_ascii_from_check_space_astral,
        check_space_astral,
        false);
}

pub struct Big5Encoder;

impl Big5Encoder {
    pub fn new(encoding: &'static Encoding) -> Encoder {
        Encoder::new(encoding, VariantEncoder::Big5(Big5Encoder))
    }

    pub fn max_buffer_length_from_utf16_without_replacement(
        &self,
        u16_length: usize,
    ) -> Option<usize> {
        // Astral: 2 to 2
        // ASCII: 1 to 1
        // Other: 1 to 2
        u16_length.checked_mul(2)
    }

    pub fn max_buffer_length_from_utf8_without_replacement(
        &self,
        byte_length: usize,
    ) -> Option<usize> {
        // Astral: 4 to 2
        // Upper BMP: 3 to 2
        // Lower BMP: 2 to 2
        // ASCII: 1 to 1
        byte_length.checked_add(1)
    }

    ascii_compatible_encoder_functions!(
        {
            // For simplicity, unified ideographs
            // in the pointer range 11206...11212 are handled
            // as Level 1 Hanzi.
            if let Some((lead, trail)) = big5_level1_hanzi_encode(bmp) {
                handle.write_two(lead, trail)
            } else {
                let pointer = if let Some(pointer) = big5_box_encode(bmp) {
                    pointer
                } else if let Some(pointer) = big5_other_encode(bmp) {
                    pointer
                } else {
                    return (
                        EncoderResult::unmappable_from_bmp(bmp),
                        source.consumed(),
                        handle.written(),
                    );
                };
                let lead = pointer / 157 + 0x81;
                let remainder = pointer % 157;
                let trail = if remainder < 0x3F {
                    remainder + 0x40
                } else {
                    remainder + 0x62
                };
                handle.write_two(lead as u8, trail as u8)
            }
        },
        {
            if in_inclusive_range32(astral as u32, 0x2008A, 0x2F8A6) {
                if let Some(rebased_pointer) = big5_astral_encode(astral as u16) {
                    // big5_astral_encode returns rebased pointer,
                    // so adding 0x87 instead of 0x81.
                    let lead = rebased_pointer / 157 + 0x87;
                    let remainder = rebased_pointer % 157;
                    let trail = if remainder < 0x3F {
                        remainder + 0x40
                    } else {
                        remainder + 0x62
                    };
                    handle.write_two(lead as u8, trail as u8)
                } else {
                    return (
                        EncoderResult::Unmappable(astral),
                        source.consumed(),
                        handle.written(),
                    );
                }
            } else {
                return (
                    EncoderResult::Unmappable(astral),
                    source.consumed(),
                    handle.written(),
                );
            }
        },
        bmp,
        astral,
        self,
        source,
        handle,
        copy_ascii_to_check_space_two,
        check_space_two,
        false
    );
}

// Any copyright to the test code below this comment is dedicated to the
// Public Domain. http://creativecommons.org/publicdomain/zero/1.0/

#[cfg(all(test, feature = "alloc"))]
mod tests {
    use super::super::testing::*;
    use super::super::*;

    fn decode_big5(bytes: &[u8], expect: &str) {
        decode(BIG5, bytes, expect);
    }

    fn encode_big5(string: &str, expect: &[u8]) {
        encode(BIG5, string, expect);
    }

    #[test]
    fn test_big5_decode() {
        // Empty
        decode_big5(b"", &"");

        // ASCII
        decode_big5(&[0x61u8, 0x62u8], &"\u{0061}\u{0062}");

        // Edge cases
        decode_big5(&[0x87u8, 0x40u8], &"\u{43F0}");
        decode_big5(&[0xFEu8, 0xFEu8], &"\u{79D4}");
        decode_big5(&[0xFEu8, 0xFDu8], &"\u{2910D}");
        decode_big5(&[0x88u8, 0x62u8], &"\u{00CA}\u{0304}");
        decode_big5(&[0x88u8, 0x64u8], &"\u{00CA}\u{030C}");
        decode_big5(&[0x88u8, 0x66u8], &"\u{00CA}");
        decode_big5(&[0x88u8, 0xA3u8], &"\u{00EA}\u{0304}");
        decode_big5(&[0x88u8, 0xA5u8], &"\u{00EA}\u{030C}");
        decode_big5(&[0x88u8, 0xA7u8], &"\u{00EA}");
        decode_big5(&[0x99u8, 0xD4u8], &"\u{8991}");
        decode_big5(&[0x99u8, 0xD5u8], &"\u{27967}");
        decode_big5(&[0x99u8, 0xD6u8], &"\u{8A29}");

        // Edge cases surrounded with ASCII
        decode_big5(
            &[0x61u8, 0x87u8, 0x40u8, 0x62u8],
            &"\u{0061}\u{43F0}\u{0062}",
        );
        decode_big5(
            &[0x61u8, 0xFEu8, 0xFEu8, 0x62u8],
            &"\u{0061}\u{79D4}\u{0062}",
        );
        decode_big5(
            &[0x61u8, 0xFEu8, 0xFDu8, 0x62u8],
            &"\u{0061}\u{2910D}\u{0062}",
        );
        decode_big5(
            &[0x61u8, 0x88u8, 0x62u8, 0x62u8],
            &"\u{0061}\u{00CA}\u{0304}\u{0062}",
        );
        decode_big5(
            &[0x61u8, 0x88u8, 0x64u8, 0x62u8],
            &"\u{0061}\u{00CA}\u{030C}\u{0062}",
        );
        decode_big5(
            &[0x61u8, 0x88u8, 0x66u8, 0x62u8],
            &"\u{0061}\u{00CA}\u{0062}",
        );
        decode_big5(
            &[0x61u8, 0x88u8, 0xA3u8, 0x62u8],
            &"\u{0061}\u{00EA}\u{0304}\u{0062}",
        );
        decode_big5(
            &[0x61u8, 0x88u8, 0xA5u8, 0x62u8],
            &"\u{0061}\u{00EA}\u{030C}\u{0062}",
        );
        decode_big5(
            &[0x61u8, 0x88u8, 0xA7u8, 0x62u8],
            &"\u{0061}\u{00EA}\u{0062}",
        );
        decode_big5(
            &[0x61u8, 0x99u8, 0xD4u8, 0x62u8],
            &"\u{0061}\u{8991}\u{0062}",
        );
        decode_big5(
            &[0x61u8, 0x99u8, 0xD5u8, 0x62u8],
            &"\u{0061}\u{27967}\u{0062}",
        );
        decode_big5(
            &[0x61u8, 0x99u8, 0xD6u8, 0x62u8],
            &"\u{0061}\u{8A29}\u{0062}",
        );

        // Bad sequences
        decode_big5(&[0x80u8, 0x61u8], &"\u{FFFD}\u{0061}");
        decode_big5(&[0xFFu8, 0x61u8], &"\u{FFFD}\u{0061}");
        decode_big5(&[0xFEu8, 0x39u8], &"\u{FFFD}\u{0039}");
        decode_big5(&[0x87u8, 0x66u8], &"\u{FFFD}\u{0066}");
        decode_big5(&[0x81u8, 0x40u8], &"\u{FFFD}\u{0040}");
        decode_big5(&[0x61u8, 0x81u8], &"\u{0061}\u{FFFD}");
    }

    #[test]
    fn test_big5_encode() {
        // Empty
        encode_big5("", b"");

        // ASCII
        encode_big5("\u{0061}\u{0062}", b"\x61\x62");

        if !cfg!(miri) {
            // Miri is too slow
            // Edge cases
            encode_big5("\u{9EA6}\u{0061}", b"&#40614;\x61");
            encode_big5("\u{2626B}\u{0061}", b"&#156267;\x61");
            encode_big5("\u{3000}", b"\xA1\x40");
            encode_big5("\u{20AC}", b"\xA3\xE1");
            encode_big5("\u{4E00}", b"\xA4\x40");
            encode_big5("\u{27607}", b"\xC8\xA4");
            encode_big5("\u{FFE2}", b"\xC8\xCD");
            encode_big5("\u{79D4}", b"\xFE\xFE");

            // Not in index
            encode_big5("\u{2603}\u{0061}", b"&#9731;\x61");
        }

        // duplicate low bits
        encode_big5("\u{203B5}", b"\xFD\x6A");
        encode_big5("\u{25605}", b"\xFE\x46");

        // prefer last
        encode_big5("\u{2550}", b"\xF9\xF9");
    }

    #[test]
    #[cfg_attr(miri, ignore)] // Miri is too slow
    fn test_big5_decode_all() {
        let input = include_bytes!("test_data/big5_in.txt");
        let expectation = include_str!("test_data/big5_in_ref.txt");
        let (cow, had_errors) = BIG5.decode_without_bom_handling(input);
        assert!(had_errors, "Should have had errors.");
        assert_eq!(&cow[..], expectation);
    }

    #[test]
    #[cfg_attr(miri, ignore)] // Miri is too slow
    fn test_big5_encode_all() {
        let input = include_str!("test_data/big5_out.txt");
        let expectation = include_bytes!("test_data/big5_out_ref.txt");
        let (cow, encoding, had_errors) = BIG5.encode(input);
        assert!(!had_errors, "Should not have had errors.");
        assert_eq!(encoding, BIG5);
        assert_eq!(&cow[..], &expectation[..]);
    }

    #[test]
    #[cfg_attr(miri, ignore)] // Miri is too slow
    fn test_big5_encode_from_two_low_surrogates() {
        let expectation = b"&#65533;&#65533;";
        let mut output = [0u8; 40];
        let mut encoder = BIG5.new_encoder();
        let (result, read, written, had_errors) =
            encoder.encode_from_utf16(&[0xDC00u16, 0xDEDEu16], &mut output[..], true);
        assert_eq!(result, CoderResult::InputEmpty);
        assert_eq!(read, 2);
        assert_eq!(written, expectation.len());
        assert!(had_errors);
        assert_eq!(&output[..written], expectation);
    }
}
