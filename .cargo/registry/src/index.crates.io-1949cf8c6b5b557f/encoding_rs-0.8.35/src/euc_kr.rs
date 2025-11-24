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
use super::in_inclusive_range16;
use super::in_range16;

pub struct EucKrDecoder {
    lead: Option<u8>,
}

impl EucKrDecoder {
    pub fn new() -> VariantDecoder {
        VariantDecoder::EucKr(EucKrDecoder { lead: None })
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
        self.plus_one_if_lead(byte_length)
    }

    pub fn max_utf8_buffer_length_without_replacement(&self, byte_length: usize) -> Option<usize> {
        // worst case: 2 to 3
        let len = self.plus_one_if_lead(byte_length);
        checked_add(2, checked_add_opt(len, checked_div(checked_add(1, len), 2)))
    }

    pub fn max_utf8_buffer_length(&self, byte_length: usize) -> Option<usize> {
        checked_mul(3, self.plus_one_if_lead(byte_length))
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
            if lead_minus_offset >= 0x20 {
                // Not the extension range above KS X 1001
                let trail_minus_offset =
                    byte.wrapping_sub(0xA1);
                if trail_minus_offset <= (0xFE - 0xA1) {
                    // KS X 1001
                    let ksx_pointer = mul_94(lead_minus_offset - 0x20) + trail_minus_offset as usize;
                    let hangul_pointer = ksx_pointer.wrapping_sub((0x2F - 0x20) * 94);
                    if hangul_pointer < KSX1001_HANGUL.len() {
                        let upper_bmp = KSX1001_HANGUL[hangul_pointer];
                        handle.write_upper_bmp(upper_bmp)
                    } else if ksx_pointer < KSX1001_SYMBOLS.len() {
                        let bmp = KSX1001_SYMBOLS[ksx_pointer];
                        handle.write_bmp_excl_ascii(bmp)
                    } else {
                        let hanja_pointer = ksx_pointer.wrapping_sub((0x49 - 0x20) * 94);
                        if hanja_pointer < KSX1001_HANJA.len() {
                            let upper_bmp = KSX1001_HANJA[hanja_pointer];
                            handle.write_upper_bmp(upper_bmp)
                        } else if (lead_minus_offset == 0x27) && ((trail_minus_offset as usize) < KSX1001_UPPERCASE.len()) {
                            let mid_bmp = KSX1001_UPPERCASE[trail_minus_offset as usize];
                            if mid_bmp == 0 {
                                return (DecoderResult::Malformed(2, 0),
                                        unread_handle_trail.consumed(),
                                        handle.written());
                            }
                            handle.write_mid_bmp(mid_bmp)
                        } else if (lead_minus_offset == 0x28) && ((trail_minus_offset as usize) < KSX1001_LOWERCASE.len()) {
                            let mid_bmp = KSX1001_LOWERCASE[trail_minus_offset as usize];
                            handle.write_mid_bmp(mid_bmp)
                        } else if (lead_minus_offset == 0x25) && ((trail_minus_offset as usize) < KSX1001_BOX.len()) {
                            let upper_bmp = KSX1001_BOX[trail_minus_offset as usize];
                            handle.write_upper_bmp(upper_bmp)
                        } else {
                            let other_pointer = ksx_pointer.wrapping_sub(2 * 94);
                            if other_pointer < 0x039F {
                                let bmp = ksx1001_other_decode(other_pointer as u16);
                                // ASCII range means unassigned
                                if bmp < 0x80 {
                                    return (DecoderResult::Malformed(2, 0),
                                            unread_handle_trail.consumed(),
                                            handle.written());
                                }
                                handle.write_bmp_excl_ascii(bmp)
                            } else {
                                return (DecoderResult::Malformed(2, 0),
                                        unread_handle_trail.consumed(),
                                        handle.written());
                            }
                        }
                    }
                } else {
                    // Extension range to the left of
                    // KS X 1001
                    let left_lead = lead_minus_offset - 0x20;
                    let left_trail = if byte.wrapping_sub(0x40 + 0x41) < (0x60 - 0x40) {
                        byte - (12 + 0x41)
                    } else if byte.wrapping_sub(0x20 + 0x41) < (0x3A - 0x20) {
                        byte - (6 + 0x41)
                    } else if byte.wrapping_sub(0x41) < 0x1A {
                        byte - 0x41
                    } else {
                        if byte < 0x80 {
                            return (DecoderResult::Malformed(1, 0),
                                    unread_handle_trail.unread(),
                                    handle.written());
                        }
                        return (DecoderResult::Malformed(2, 0),
                                unread_handle_trail.consumed(),
                                handle.written());
                    };
                    let left_pointer = ((left_lead as usize) * (190 - 94 - 12)) + left_trail as usize;
                    if left_pointer < (0x45 - 0x20) * (190 - 94 - 12) + 0x12 {
                        let upper_bmp = cp949_left_hangul_decode(left_pointer as u16);
                        handle.write_upper_bmp(upper_bmp)
                    } else {
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
            } else {
                // Extension range above KS X 1001
                let top_trail = if byte.wrapping_sub(0x40 + 0x41) < (0xBE - 0x40) {
                    byte - (12 + 0x41)
                } else if byte.wrapping_sub(0x20 + 0x41) < (0x3A - 0x20) {
                    byte - (6 + 0x41)
                } else if byte.wrapping_sub(0x41) < 0x1A {
                    byte - 0x41
                } else {
                    if byte < 0x80 {
                        return (DecoderResult::Malformed(1, 0),
                                unread_handle_trail.unread(),
                                handle.written());
                    }
                    return (DecoderResult::Malformed(2, 0),
                            unread_handle_trail.consumed(),
                            handle.written());
                };
                let top_pointer = ((lead_minus_offset as usize) * (190 - 12)) + top_trail as usize;
                let upper_bmp = cp949_top_hangul_decode(top_pointer as u16);
                handle.write_upper_bmp(upper_bmp)
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
        copy_ascii_from_check_space_bmp,
        check_space_bmp,
        true);
}

fn ksx1001_encode_misc(bmp: u16) -> Option<(usize, usize)> {
    if in_inclusive_range16(bmp, 0x3000, 0x3015) {
        if let Some(pos) = position(&KSX1001_SYMBOLS[..(0xAB - 0x60)], bmp) {
            return Some((0xA1, pos + 0xA1));
        }
    }
    if let Some(other_pointer) = ksx1001_other_encode(bmp) {
        let other_lead = ((other_pointer as usize) / 94) + (0x81 + 0x22);
        let other_trail = ((other_pointer as usize) % 94) + 0xA1;
        return Some((other_lead, other_trail));
    }
    if in_range16(bmp, 0x00AA, 0x0168) {
        // Latin
        if let Some(pos) = position(&KSX1001_LOWERCASE[..], bmp) {
            return Some((0x81 + 0x28, 0xA1 + pos));
        }
        if let Some(pos) = position(&KSX1001_UPPERCASE[..], bmp) {
            return Some((0x81 + 0x27, 0xA1 + pos));
        }
    } else if in_range16(bmp, 0x2500, 0x254C) {
        if let Some(pos) = position(&KSX1001_BOX[..], bmp) {
            return Some((0x81 + 0x25, 0xA1 + pos));
        }
    }
    if in_inclusive_range16(bmp, 0x2015, 0x266D)
        || in_inclusive_range16(bmp, 0x321C, 0x33D8)
        || in_inclusive_range16(bmp, 0xFF3C, 0xFFE5)
        || in_inclusive_range16(bmp, 0x00A1, 0x00F7)
        || in_inclusive_range16(bmp, 0x02C7, 0x02DD)
    {
        if let Some(pos) = position(&KSX1001_SYMBOLS[3..], bmp) {
            if pos < (94 - 3) {
                return Some((0xA1, pos + 0xA1 + 3));
            }
            return Some((0xA2, pos - (94 - 3) + 0xA1));
        }
    }
    None
}

#[cfg(not(feature = "fast-hangul-encode"))]
#[inline(always)]
fn ksx1001_encode_hangul(bmp: u16, _: u16) -> (u8, u8) {
    match KSX1001_HANGUL.binary_search(&bmp) {
        Ok(ksx_hangul_pointer) => {
            let ksx_hangul_lead = (ksx_hangul_pointer / 94) + (0x81 + 0x2F);
            let ksx_hangul_trail = (ksx_hangul_pointer % 94) + 0xA1;
            (ksx_hangul_lead as u8, ksx_hangul_trail as u8)
        }
        Err(_) => {
            let (lead, cp949_trail) = if bmp < 0xC8A5 {
                // Above KS X 1001
                let top_pointer = cp949_top_hangul_encode(bmp) as usize;
                let top_lead = (top_pointer / (190 - 12)) + 0x81;
                let top_trail = top_pointer % (190 - 12);
                (top_lead as u8, top_trail as u8)
            } else {
                // To the left of KS X 1001
                let left_pointer = cp949_left_hangul_encode(bmp) as usize;
                let left_lead = (left_pointer / (190 - 94 - 12)) + (0x81 + 0x20);
                let left_trail = left_pointer % (190 - 94 - 12);
                (left_lead as u8, left_trail as u8)
            };
            let offset = if cp949_trail >= (0x40 - 12) {
                0x41 + 12
            } else if cp949_trail >= (0x20 - 6) {
                0x41 + 6
            } else {
                0x41
            };
            (lead as u8, (cp949_trail + offset) as u8)
        }
    }
}

#[cfg(feature = "fast-hangul-encode")]
#[inline(always)]
fn ksx1001_encode_hangul(_: u16, bmp_minus_hangul_start: u16) -> (u8, u8) {
    cp949_hangul_encode(bmp_minus_hangul_start)
}

#[cfg(not(feature = "fast-hanja-encode"))]
#[inline(always)]
fn ksx1001_encode_hanja(bmp: u16) -> Option<(u8, u8)> {
    if let Some(hanja_pointer) = position(&KSX1001_HANJA[..], bmp) {
        let hanja_lead = (hanja_pointer / 94) + (0x81 + 0x49);
        let hanja_trail = (hanja_pointer % 94) + 0xA1;
        Some((hanja_lead as u8, hanja_trail as u8))
    } else {
        None
    }
}

#[cfg(feature = "fast-hanja-encode")]
#[inline(always)]
fn ksx1001_encode_hanja(bmp: u16) -> Option<(u8, u8)> {
    if bmp < 0xF900 {
        ksx1001_unified_hangul_encode(bmp)
    } else {
        Some(ksx1001_compatibility_hangul_encode(bmp))
    }
}

pub struct EucKrEncoder;

impl EucKrEncoder {
    pub fn new(encoding: &'static Encoding) -> Encoder {
        Encoder::new(encoding, VariantEncoder::EucKr(EucKrEncoder))
    }

    pub fn max_buffer_length_from_utf16_without_replacement(
        &self,
        u16_length: usize,
    ) -> Option<usize> {
        u16_length.checked_mul(2)
    }

    pub fn max_buffer_length_from_utf8_without_replacement(
        &self,
        byte_length: usize,
    ) -> Option<usize> {
        byte_length.checked_add(1)
    }

    ascii_compatible_bmp_encoder_functions!(
        {
            let bmp_minus_hangul_start = bmp.wrapping_sub(0xAC00);
            let (lead, trail) = if bmp_minus_hangul_start < (0xD7A4 - 0xAC00) {
                // Hangul
                ksx1001_encode_hangul(bmp, bmp_minus_hangul_start)
            } else if in_range16(bmp, 0x33DE, 0xFF01) {
                // Vast range that includes no other
                // mappables except Hangul (already
                // processed) and Hanja.
                // Narrow the range further to Unified and
                // Compatibility ranges of Hanja.
                if in_range16(bmp, 0x4E00, 0x9F9D) || in_range16(bmp, 0xF900, 0xFA0C) {
                    if let Some((hanja_lead, hanja_trail)) = ksx1001_encode_hanja(bmp) {
                        (hanja_lead, hanja_trail)
                    } else {
                        return (
                            EncoderResult::unmappable_from_bmp(bmp),
                            source.consumed(),
                            handle.written(),
                        );
                    }
                } else {
                    return (
                        EncoderResult::unmappable_from_bmp(bmp),
                        source.consumed(),
                        handle.written(),
                    );
                }
            } else if let Some((lead, trail)) = ksx1001_encode_misc(bmp) {
                (lead as u8, trail as u8)
            } else {
                return (
                    EncoderResult::unmappable_from_bmp(bmp),
                    source.consumed(),
                    handle.written(),
                );
            };
            handle.write_two(lead, trail)
        },
        bmp,
        self,
        source,
        handle,
        copy_ascii_to_check_space_two,
        check_space_two,
        true
    );
}

// Any copyright to the test code below this comment is dedicated to the
// Public Domain. http://creativecommons.org/publicdomain/zero/1.0/

#[cfg(all(test, feature = "alloc"))]
mod tests {
    use super::super::testing::*;
    use super::super::*;

    fn decode_euc_kr(bytes: &[u8], expect: &str) {
        decode(EUC_KR, bytes, expect);
    }

    fn encode_euc_kr(string: &str, expect: &[u8]) {
        encode(EUC_KR, string, expect);
    }

    #[test]
    fn test_euc_kr_decode() {
        // Empty
        decode_euc_kr(b"", &"");

        // ASCII
        decode_euc_kr(b"\x61\x62", "\u{0061}\u{0062}");

        decode_euc_kr(b"\x81\x41", "\u{AC02}");
        decode_euc_kr(b"\x81\x5B", "\u{FFFD}\x5B");
        decode_euc_kr(b"\xFD\xFE", "\u{8A70}");
        decode_euc_kr(b"\xFE\x41", "\u{FFFD}\x41");
        decode_euc_kr(b"\xFF\x41", "\u{FFFD}\x41");
        decode_euc_kr(b"\x80\x41", "\u{FFFD}\x41");
        decode_euc_kr(b"\xA1\xFF", "\u{FFFD}");
        decode_euc_kr(b"\x81\xFF", "\u{FFFD}");
    }

    #[test]
    fn test_euc_kr_encode() {
        // Empty
        encode_euc_kr("", b"");

        // ASCII
        encode_euc_kr("\u{0061}\u{0062}", b"\x61\x62");

        encode_euc_kr("\u{AC02}", b"\x81\x41");
        encode_euc_kr("\u{8A70}", b"\xFD\xFE");
    }

    #[test]
    #[cfg_attr(miri, ignore)] // Miri is too slow
    fn test_euc_kr_decode_all() {
        let input = include_bytes!("test_data/euc_kr_in.txt");
        let expectation = include_str!("test_data/euc_kr_in_ref.txt");
        let (cow, had_errors) = EUC_KR.decode_without_bom_handling(input);
        assert!(had_errors, "Should have had errors.");
        assert_eq!(&cow[..], expectation);
    }

    #[test]
    #[cfg_attr(miri, ignore)] // Miri is too slow
    fn test_euc_kr_encode_all() {
        let input = include_str!("test_data/euc_kr_out.txt");
        let expectation = include_bytes!("test_data/euc_kr_out_ref.txt");
        let (cow, encoding, had_errors) = EUC_KR.encode(input);
        assert!(!had_errors, "Should not have had errors.");
        assert_eq!(encoding, EUC_KR);
        assert_eq!(&cow[..], &expectation[..]);
    }

    #[test]
    fn test_euc_kr_encode_from_two_low_surrogates() {
        let expectation = b"&#65533;&#65533;";
        let mut output = [0u8; 40];
        let mut encoder = EUC_KR.new_encoder();
        let (result, read, written, had_errors) =
            encoder.encode_from_utf16(&[0xDC00u16, 0xDEDEu16], &mut output[..], true);
        assert_eq!(result, CoderResult::InputEmpty);
        assert_eq!(read, 2);
        assert_eq!(written, expectation.len());
        assert!(had_errors);
        assert_eq!(&output[..written], expectation);
    }
}
