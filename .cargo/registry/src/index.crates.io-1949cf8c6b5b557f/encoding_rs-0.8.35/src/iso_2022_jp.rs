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

#[derive(Copy, Clone, PartialEq)]
enum Iso2022JpDecoderState {
    Ascii,
    Roman,
    Katakana,
    LeadByte,
    TrailByte,
    EscapeStart,
    Escape,
}

pub struct Iso2022JpDecoder {
    decoder_state: Iso2022JpDecoderState,
    output_state: Iso2022JpDecoderState, // only takes 1 of first 4 values
    lead: u8,
    output_flag: bool,
    pending_prepended: bool,
}

impl Iso2022JpDecoder {
    pub fn new() -> VariantDecoder {
        VariantDecoder::Iso2022Jp(Iso2022JpDecoder {
            decoder_state: Iso2022JpDecoderState::Ascii,
            output_state: Iso2022JpDecoderState::Ascii,
            lead: 0u8,
            output_flag: false,
            pending_prepended: false,
        })
    }

    pub fn in_neutral_state(&self) -> bool {
        self.decoder_state == Iso2022JpDecoderState::Ascii
            && self.output_state == Iso2022JpDecoderState::Ascii
            && self.lead == 0u8
            && !self.output_flag
            && !self.pending_prepended
    }

    fn extra_to_input_from_state(&self, byte_length: usize) -> Option<usize> {
        byte_length.checked_add(
            if self.lead == 0 || self.pending_prepended {
                0
            } else {
                1
            } + match self.decoder_state {
                Iso2022JpDecoderState::Escape | Iso2022JpDecoderState::EscapeStart => 1,
                _ => 0,
            },
        )
    }

    fn extra_to_output_from_state(&self) -> usize {
        if self.lead != 0 && self.pending_prepended {
            1 + self.output_flag as usize
        } else {
            self.output_flag as usize
        }
    }

    pub fn max_utf16_buffer_length(&self, byte_length: usize) -> Option<usize> {
        checked_add(
            self.extra_to_output_from_state(),
            self.extra_to_input_from_state(byte_length),
        )
    }

    pub fn max_utf8_buffer_length_without_replacement(&self, byte_length: usize) -> Option<usize> {
        // worst case: 1 to 3 (half-width katakana)
        self.max_utf8_buffer_length(byte_length)
    }

    pub fn max_utf8_buffer_length(&self, byte_length: usize) -> Option<usize> {
        checked_mul(
            3,
            checked_add(
                self.extra_to_output_from_state(),
                self.extra_to_input_from_state(byte_length),
            ),
        )
    }

    decoder_functions!(
        {
            if self.pending_prepended {
                // lead was set in EscapeStart and "prepended"
                // in Escape.
                debug_assert!(self.lead == 0x24u8 || self.lead == 0x28u8);
                match dest.check_space_bmp() {
                    Space::Full(_) => {
                        return (DecoderResult::OutputFull, 0, 0);
                    }
                    Space::Available(destination_handle) => {
                        self.pending_prepended = false;
                        self.output_flag = false;
                        match self.decoder_state {
                            Iso2022JpDecoderState::Ascii | Iso2022JpDecoderState::Roman => {
                                destination_handle.write_ascii(self.lead);
                                self.lead = 0x0u8;
                            }
                            Iso2022JpDecoderState::Katakana => {
                                destination_handle
                                    .write_upper_bmp(u16::from(self.lead) - 0x21u16 + 0xFF61u16);
                                self.lead = 0x0u8;
                            }
                            Iso2022JpDecoderState::LeadByte => {
                                self.decoder_state = Iso2022JpDecoderState::TrailByte;
                            }
                            _ => unreachable!(),
                        }
                    }
                }
            }
        },
        {},
        {
            match self.decoder_state {
                Iso2022JpDecoderState::TrailByte | Iso2022JpDecoderState::EscapeStart => {
                    self.decoder_state = self.output_state;
                    return (DecoderResult::Malformed(1, 0), src_consumed, dest.written());
                }
                Iso2022JpDecoderState::Escape => {
                    self.pending_prepended = true;
                    self.decoder_state = self.output_state;
                    return (DecoderResult::Malformed(1, 1), src_consumed, dest.written());
                }
                _ => {}
            }
        },
        {
            match self.decoder_state {
                Iso2022JpDecoderState::Ascii => {
                    if b == 0x1Bu8 {
                        self.decoder_state = Iso2022JpDecoderState::EscapeStart;
                        continue;
                    }
                    self.output_flag = false;
                    if b > 0x7Fu8 || b == 0x0Eu8 || b == 0x0Fu8 {
                        return (
                            DecoderResult::Malformed(1, 0),
                            unread_handle.consumed(),
                            destination_handle.written(),
                        );
                    }
                    destination_handle.write_ascii(b);
                    continue;
                }
                Iso2022JpDecoderState::Roman => {
                    if b == 0x1Bu8 {
                        self.decoder_state = Iso2022JpDecoderState::EscapeStart;
                        continue;
                    }
                    self.output_flag = false;
                    if b == 0x5Cu8 {
                        destination_handle.write_mid_bmp(0x00A5u16);
                        continue;
                    }
                    if b == 0x7Eu8 {
                        destination_handle.write_upper_bmp(0x203Eu16);
                        continue;
                    }
                    if b > 0x7Fu8 || b == 0x0Eu8 || b == 0x0Fu8 {
                        return (
                            DecoderResult::Malformed(1, 0),
                            unread_handle.consumed(),
                            destination_handle.written(),
                        );
                    }
                    destination_handle.write_ascii(b);
                    continue;
                }
                Iso2022JpDecoderState::Katakana => {
                    if b == 0x1Bu8 {
                        self.decoder_state = Iso2022JpDecoderState::EscapeStart;
                        continue;
                    }
                    self.output_flag = false;
                    if b >= 0x21u8 && b <= 0x5Fu8 {
                        destination_handle.write_upper_bmp(u16::from(b) - 0x21u16 + 0xFF61u16);
                        continue;
                    }
                    return (
                        DecoderResult::Malformed(1, 0),
                        unread_handle.consumed(),
                        destination_handle.written(),
                    );
                }
                Iso2022JpDecoderState::LeadByte => {
                    if b == 0x1Bu8 {
                        self.decoder_state = Iso2022JpDecoderState::EscapeStart;
                        continue;
                    }
                    self.output_flag = false;
                    if b >= 0x21u8 && b <= 0x7Eu8 {
                        self.lead = b;
                        self.decoder_state = Iso2022JpDecoderState::TrailByte;
                        continue;
                    }
                    return (
                        DecoderResult::Malformed(1, 0),
                        unread_handle.consumed(),
                        destination_handle.written(),
                    );
                }
                Iso2022JpDecoderState::TrailByte => {
                    if b == 0x1Bu8 {
                        self.decoder_state = Iso2022JpDecoderState::EscapeStart;
                        // The byte in error is the previous
                        // lead byte.
                        return (
                            DecoderResult::Malformed(1, 1),
                            unread_handle.consumed(),
                            destination_handle.written(),
                        );
                    }
                    self.decoder_state = Iso2022JpDecoderState::LeadByte;
                    let jis0208_lead_minus_offset = self.lead - 0x21;
                    let byte = b;
                    let handle = destination_handle;
                    // The code below uses else after continue in
                    // order to retain the structure seen in EUC-JP.
                    let trail_minus_offset = byte.wrapping_sub(0x21);
                    // Fast-track Hiragana (60% according to Lunde)
                    // and Katakana (10% acconding to Lunde).
                    if jis0208_lead_minus_offset == 0x03 && trail_minus_offset < 0x53 {
                        // Hiragana
                        handle.write_upper_bmp(0x3041 + u16::from(trail_minus_offset));
                        continue;
                    } else if jis0208_lead_minus_offset == 0x04 && trail_minus_offset < 0x56 {
                        // Katakana
                        handle.write_upper_bmp(0x30A1 + u16::from(trail_minus_offset));
                        continue;
                    } else if trail_minus_offset > (0xFE - 0xA1) {
                        return (
                            DecoderResult::Malformed(2, 0),
                            unread_handle.consumed(),
                            handle.written(),
                        );
                    } else {
                        let pointer =
                            mul_94(jis0208_lead_minus_offset) + trail_minus_offset as usize;
                        let level1_pointer = pointer.wrapping_sub(1410);
                        if level1_pointer < JIS0208_LEVEL1_KANJI.len() {
                            handle.write_upper_bmp(JIS0208_LEVEL1_KANJI[level1_pointer]);
                            continue;
                        } else {
                            let level2_pointer = pointer.wrapping_sub(4418);
                            if level2_pointer < JIS0208_LEVEL2_AND_ADDITIONAL_KANJI.len() {
                                handle.write_upper_bmp(
                                    JIS0208_LEVEL2_AND_ADDITIONAL_KANJI[level2_pointer],
                                );
                                continue;
                            } else {
                                let ibm_pointer = pointer.wrapping_sub(8272);
                                if ibm_pointer < IBM_KANJI.len() {
                                    handle.write_upper_bmp(IBM_KANJI[ibm_pointer]);
                                    continue;
                                } else if let Some(bmp) = jis0208_symbol_decode(pointer) {
                                    handle.write_bmp_excl_ascii(bmp);
                                    continue;
                                } else if let Some(bmp) = jis0208_range_decode(pointer) {
                                    handle.write_bmp_excl_ascii(bmp);
                                    continue;
                                } else {
                                    return (
                                        DecoderResult::Malformed(2, 0),
                                        unread_handle.consumed(),
                                        handle.written(),
                                    );
                                }
                            }
                        }
                    }
                }
                Iso2022JpDecoderState::EscapeStart => {
                    if b == 0x24u8 || b == 0x28u8 {
                        self.lead = b;
                        self.decoder_state = Iso2022JpDecoderState::Escape;
                        continue;
                    }
                    self.output_flag = false;
                    self.decoder_state = self.output_state;
                    return (
                        DecoderResult::Malformed(1, 0),
                        unread_handle.unread(),
                        destination_handle.written(),
                    );
                }
                Iso2022JpDecoderState::Escape => {
                    let mut state: Option<Iso2022JpDecoderState> = None;
                    if self.lead == 0x28u8 && b == 0x42u8 {
                        state = Some(Iso2022JpDecoderState::Ascii);
                    } else if self.lead == 0x28u8 && b == 0x4Au8 {
                        state = Some(Iso2022JpDecoderState::Roman);
                    } else if self.lead == 0x28u8 && b == 0x49u8 {
                        state = Some(Iso2022JpDecoderState::Katakana);
                    } else if self.lead == 0x24u8 && (b == 0x40u8 || b == 0x42u8) {
                        state = Some(Iso2022JpDecoderState::LeadByte);
                    }
                    match state {
                        Some(s) => {
                            self.lead = 0x0u8;
                            self.decoder_state = s;
                            self.output_state = s;
                            let flag = self.output_flag;
                            self.output_flag = true;
                            if flag {
                                // We had an escape sequence
                                // immediately following another
                                // escape sequence. Therefore,
                                // the first one of these was
                                // useless.
                                return (
                                    DecoderResult::Malformed(3, 3),
                                    unread_handle.consumed(),
                                    destination_handle.written(),
                                );
                            }
                            continue;
                        }
                        None => {
                            // self.lead is still the previous
                            // byte. It will be processed in
                            // the preabmle upon next call.
                            self.pending_prepended = true;
                            self.output_flag = false;
                            self.decoder_state = self.output_state;
                            // The byte in error is not the
                            // current or the previous byte but
                            // the one before those (lone 0x1B).
                            return (
                                DecoderResult::Malformed(1, 1),
                                unread_handle.unread(),
                                destination_handle.written(),
                            );
                        }
                    }
                }
            }
        },
        self,
        src_consumed,
        dest,
        source,
        b,
        destination_handle,
        unread_handle,
        check_space_bmp
    );
}

#[cfg(feature = "fast-kanji-encode")]
#[inline(always)]
fn is_kanji_mapped(bmp: u16) -> bool {
    // Use the shift_jis variant, because we don't care about the
    // byte values here.
    jis0208_kanji_shift_jis_encode(bmp).is_some()
}

#[cfg(not(feature = "fast-kanji-encode"))]
#[cfg_attr(
    feature = "cargo-clippy",
    allow(if_let_redundant_pattern_matching, if_same_then_else)
)]
#[inline(always)]
fn is_kanji_mapped(bmp: u16) -> bool {
    if 0x4EDD == bmp {
        true
    } else if let Some(_) = jis0208_level1_kanji_shift_jis_encode(bmp) {
        // Use the shift_jis variant, because we don't care about the
        // byte values here.
        true
    } else if let Some(_) = jis0208_level2_and_additional_kanji_encode(bmp) {
        true
    } else if let Some(_) = position(&IBM_KANJI[..], bmp) {
        true
    } else {
        false
    }
}

#[cfg_attr(
    feature = "cargo-clippy",
    allow(if_let_redundant_pattern_matching, if_same_then_else)
)]
fn is_mapped_for_two_byte_encode(bmp: u16) -> bool {
    // The code below uses else after return to
    // keep the same structure as in EUC-JP.
    // Lunde says 60% Hiragana, 30% Kanji, 10% Katakana
    let bmp_minus_hiragana = bmp.wrapping_sub(0x3041);
    if bmp_minus_hiragana < 0x53 {
        true
    } else if in_inclusive_range16(bmp, 0x4E00, 0x9FA0) {
        is_kanji_mapped(bmp)
    } else {
        let bmp_minus_katakana = bmp.wrapping_sub(0x30A1);
        if bmp_minus_katakana < 0x56 {
            true
        } else {
            let bmp_minus_space = bmp.wrapping_sub(0x3000);
            if bmp_minus_space < 3 {
                // fast-track common punctuation
                true
            } else if in_inclusive_range16(bmp, 0xFF61, 0xFF9F) {
                true
            } else if bmp == 0x2212 {
                true
            } else if let Some(_) = jis0208_range_encode(bmp) {
                true
            } else if in_inclusive_range16(bmp, 0xFA0E, 0xFA2D) || bmp == 0xF929 || bmp == 0xF9DC {
                true
            } else if let Some(_) = ibm_symbol_encode(bmp) {
                true
            } else if let Some(_) = jis0208_symbol_encode(bmp) {
                true
            } else {
                false
            }
        }
    }
}

#[cfg(feature = "fast-kanji-encode")]
#[inline(always)]
fn encode_kanji(bmp: u16) -> Option<(u8, u8)> {
    jis0208_kanji_iso_2022_jp_encode(bmp)
}

#[cfg(not(feature = "fast-kanji-encode"))]
#[inline(always)]
fn encode_kanji(bmp: u16) -> Option<(u8, u8)> {
    if 0x4EDD == bmp {
        // Ideograph on the symbol row!
        Some((0x21, 0xB8 - 0x80))
    } else if let Some((lead, trail)) = jis0208_level1_kanji_iso_2022_jp_encode(bmp) {
        Some((lead, trail))
    } else if let Some(pos) = jis0208_level2_and_additional_kanji_encode(bmp) {
        let lead = (pos / 94) + (0xD0 - 0x80);
        let trail = (pos % 94) + 0x21;
        Some((lead as u8, trail as u8))
    } else if let Some(pos) = position(&IBM_KANJI[..], bmp) {
        let lead = (pos / 94) + (0xF9 - 0x80);
        let trail = (pos % 94) + 0x21;
        Some((lead as u8, trail as u8))
    } else {
        None
    }
}

enum Iso2022JpEncoderState {
    Ascii,
    Roman,
    Jis0208,
}

pub struct Iso2022JpEncoder {
    state: Iso2022JpEncoderState,
}

impl Iso2022JpEncoder {
    pub fn new(encoding: &'static Encoding) -> Encoder {
        Encoder::new(
            encoding,
            VariantEncoder::Iso2022Jp(Iso2022JpEncoder {
                state: Iso2022JpEncoderState::Ascii,
            }),
        )
    }

    pub fn has_pending_state(&self) -> bool {
        match self.state {
            Iso2022JpEncoderState::Ascii => false,
            _ => true,
        }
    }

    pub fn max_buffer_length_from_utf16_without_replacement(
        &self,
        u16_length: usize,
    ) -> Option<usize> {
        // Worst case: every other character is ASCII/Roman and every other
        // JIS0208.
        // Two UTF-16 input units:
        // Transition to Roman: 3
        // Roman/ASCII: 1
        // Transition to JIS0208: 3
        // JIS0208: 2
        // End transition: 3
        checked_add_opt(
            checked_add(3, u16_length.checked_mul(4)),
            checked_div(u16_length.checked_add(1), 2),
        )
    }

    pub fn max_buffer_length_from_utf8_without_replacement(
        &self,
        byte_length: usize,
    ) -> Option<usize> {
        // Worst case: every other character is ASCII/Roman and every other
        // JIS0208.
        // Three UTF-8 input units: 1 ASCII, 2 JIS0208
        // Transition to ASCII: 3
        // Roman/ASCII: 1
        // Transition to JIS0208: 3
        // JIS0208: 2
        // End transition: 3
        checked_add(3, byte_length.checked_mul(3))
    }

    encoder_functions!(
        {
            match self.state {
                Iso2022JpEncoderState::Ascii => {}
                _ => match dest.check_space_three() {
                    Space::Full(dst_written) => {
                        return (EncoderResult::OutputFull, src_consumed, dst_written);
                    }
                    Space::Available(destination_handle) => {
                        self.state = Iso2022JpEncoderState::Ascii;
                        destination_handle.write_three(0x1Bu8, 0x28u8, 0x42u8);
                    }
                },
            }
        },
        {
            match self.state {
                Iso2022JpEncoderState::Ascii => {
                    if c == '\u{0E}' || c == '\u{0F}' || c == '\u{1B}' {
                        return (
                            EncoderResult::Unmappable('\u{FFFD}'),
                            unread_handle.consumed(),
                            destination_handle.written(),
                        );
                    }
                    if c <= '\u{7F}' {
                        destination_handle.write_one(c as u8);
                        continue;
                    }
                    if c == '\u{A5}' || c == '\u{203E}' {
                        self.state = Iso2022JpEncoderState::Roman;
                        destination_handle.write_three(0x1Bu8, 0x28u8, 0x4Au8);
                        unread_handle.unread();
                        continue;
                    }
                    if c > '\u{FFFF}' {
                        return (
                            EncoderResult::Unmappable(c),
                            unread_handle.consumed(),
                            destination_handle.written(),
                        );
                    }
                    // Yes, if c is in index, we'll search
                    // again in the Jis0208 state, but this
                    // encoder is not worth optimizing.
                    if is_mapped_for_two_byte_encode(c as u16) {
                        self.state = Iso2022JpEncoderState::Jis0208;
                        destination_handle.write_three(0x1Bu8, 0x24u8, 0x42u8);
                        unread_handle.unread();
                        continue;
                    }
                    return (
                        EncoderResult::Unmappable(c),
                        unread_handle.consumed(),
                        destination_handle.written(),
                    );
                }
                Iso2022JpEncoderState::Roman => {
                    if c == '\u{0E}' || c == '\u{0F}' || c == '\u{1B}' {
                        return (
                            EncoderResult::Unmappable('\u{FFFD}'),
                            unread_handle.consumed(),
                            destination_handle.written(),
                        );
                    }
                    if c == '\u{5C}' || c == '\u{7E}' {
                        self.state = Iso2022JpEncoderState::Ascii;
                        destination_handle.write_three(0x1Bu8, 0x28u8, 0x42u8);
                        unread_handle.unread();
                        continue;
                    }
                    if c <= '\u{7F}' {
                        destination_handle.write_one(c as u8);
                        continue;
                    }
                    if c == '\u{A5}' {
                        destination_handle.write_one(0x5Cu8);
                        continue;
                    }
                    if c == '\u{203E}' {
                        destination_handle.write_one(0x7Eu8);
                        continue;
                    }
                    if c > '\u{FFFF}' {
                        return (
                            EncoderResult::Unmappable(c),
                            unread_handle.consumed(),
                            destination_handle.written(),
                        );
                    }
                    // Yes, if c is in index, we'll search
                    // again in the Jis0208 state, but this
                    // encoder is not worth optimizing.
                    if is_mapped_for_two_byte_encode(c as u16) {
                        self.state = Iso2022JpEncoderState::Jis0208;
                        destination_handle.write_three(0x1Bu8, 0x24u8, 0x42u8);
                        unread_handle.unread();
                        continue;
                    }
                    return (
                        EncoderResult::Unmappable(c),
                        unread_handle.consumed(),
                        destination_handle.written(),
                    );
                }
                Iso2022JpEncoderState::Jis0208 => {
                    if c <= '\u{7F}' {
                        self.state = Iso2022JpEncoderState::Ascii;
                        destination_handle.write_three(0x1Bu8, 0x28u8, 0x42u8);
                        unread_handle.unread();
                        continue;
                    }
                    if c == '\u{A5}' || c == '\u{203E}' {
                        self.state = Iso2022JpEncoderState::Roman;
                        destination_handle.write_three(0x1Bu8, 0x28u8, 0x4Au8);
                        unread_handle.unread();
                        continue;
                    }
                    if c > '\u{FFFF}' {
                        // Transition to ASCII here in order
                        // not to make it the responsibility
                        // of the caller.
                        self.state = Iso2022JpEncoderState::Ascii;
                        return (
                            EncoderResult::Unmappable(c),
                            unread_handle.consumed(),
                            destination_handle.write_three_return_written(0x1Bu8, 0x28u8, 0x42u8),
                        );
                    }
                    let bmp = c as u16;
                    let handle = destination_handle;
                    // The code below uses else after continue to
                    // keep the same structure as in EUC-JP.
                    // Lunde says 60% Hiragana, 30% Kanji, 10% Katakana
                    let bmp_minus_hiragana = bmp.wrapping_sub(0x3041);
                    if bmp_minus_hiragana < 0x53 {
                        handle.write_two(0x24, 0x21 + bmp_minus_hiragana as u8);
                        continue;
                    } else if in_inclusive_range16(bmp, 0x4E00, 0x9FA0) {
                        if let Some((lead, trail)) = encode_kanji(bmp) {
                            handle.write_two(lead, trail);
                            continue;
                        } else {
                            self.state = Iso2022JpEncoderState::Ascii;
                            return (
                                EncoderResult::Unmappable(c),
                                unread_handle.consumed(),
                                handle.write_three_return_written(0x1Bu8, 0x28u8, 0x42u8),
                            );
                        }
                    } else {
                        let bmp_minus_katakana = bmp.wrapping_sub(0x30A1);
                        if bmp_minus_katakana < 0x56 {
                            handle.write_two(0x25, 0x21 + bmp_minus_katakana as u8);
                            continue;
                        } else {
                            let bmp_minus_space = bmp.wrapping_sub(0x3000);
                            if bmp_minus_space < 3 {
                                // fast-track common punctuation
                                handle.write_two(0x21, 0x21 + bmp_minus_space as u8);
                                continue;
                            }
                            let bmp_minus_half_width = bmp.wrapping_sub(0xFF61);
                            if bmp_minus_half_width <= (0xFF9F - 0xFF61) {
                                // We have half-width katakana. The lead is either
                                // row 1 or 5 of JIS X 0208, so the lookup table
                                // only stores the trail.
                                let lead =
                                    if bmp != 0xFF70 && in_inclusive_range16(bmp, 0xFF66, 0xFF9D) {
                                        0x25u8
                                    } else {
                                        0x21u8
                                    };
                                let trail =
                                    ISO_2022_JP_HALF_WIDTH_TRAIL[bmp_minus_half_width as usize];
                                handle.write_two(lead, trail);
                                continue;
                            } else if bmp == 0x2212 {
                                handle.write_two(0x21, 0x5D);
                                continue;
                            } else if let Some(pointer) = jis0208_range_encode(bmp) {
                                let lead = (pointer / 94) + 0x21;
                                let trail = (pointer % 94) + 0x21;
                                handle.write_two(lead as u8, trail as u8);
                                continue;
                            } else if in_inclusive_range16(bmp, 0xFA0E, 0xFA2D)
                                || bmp == 0xF929
                                || bmp == 0xF9DC
                            {
                                // Guaranteed to be found in IBM_KANJI
                                let pos = position(&IBM_KANJI[..], bmp).unwrap();
                                let lead = (pos / 94) + (0xF9 - 0x80);
                                let trail = (pos % 94) + 0x21;
                                handle.write_two(lead as u8, trail as u8);
                                continue;
                            } else if let Some(pointer) = ibm_symbol_encode(bmp) {
                                let lead = (pointer / 94) + 0x21;
                                let trail = (pointer % 94) + 0x21;
                                handle.write_two(lead as u8, trail as u8);
                                continue;
                            } else if let Some(pointer) = jis0208_symbol_encode(bmp) {
                                let lead = (pointer / 94) + 0x21;
                                let trail = (pointer % 94) + 0x21;
                                handle.write_two(lead as u8, trail as u8);
                                continue;
                            } else {
                                self.state = Iso2022JpEncoderState::Ascii;
                                return (
                                    EncoderResult::Unmappable(c),
                                    unread_handle.consumed(),
                                    handle.write_three_return_written(0x1Bu8, 0x28u8, 0x42u8),
                                );
                            }
                        }
                    }
                }
            }
        },
        self,
        src_consumed,
        source,
        dest,
        c,
        destination_handle,
        unread_handle,
        check_space_three
    );
}

// Any copyright to the test code below this comment is dedicated to the
// Public Domain. http://creativecommons.org/publicdomain/zero/1.0/

#[cfg(all(test, feature = "alloc"))]
mod tests {
    use super::super::testing::*;
    use super::super::*;

    fn decode_iso_2022_jp(bytes: &[u8], expect: &str) {
        decode(ISO_2022_JP, bytes, expect);
    }

    fn encode_iso_2022_jp(string: &str, expect: &[u8]) {
        encode(ISO_2022_JP, string, expect);
    }

    #[test]
    fn test_iso_2022_jp_decode() {
        // Empty
        decode_iso_2022_jp(b"", &"");

        // ASCII
        decode_iso_2022_jp(b"\x61\x62", "\u{0061}\u{0062}");
        decode_iso_2022_jp(b"\x7F\x0E\x0F", "\u{007F}\u{FFFD}\u{FFFD}");

        // Partial escapes
        decode_iso_2022_jp(b"\x1B", "\u{FFFD}");
        decode_iso_2022_jp(b"\x1B$", "\u{FFFD}$");
        decode_iso_2022_jp(b"\x1B(", "\u{FFFD}(");
        decode_iso_2022_jp(b"\x1B.", "\u{FFFD}.");

        // ISO escapes
        decode_iso_2022_jp(b"\x1B(B", ""); // ASCII
        decode_iso_2022_jp(b"\x1B(J", ""); // Roman
        decode_iso_2022_jp(b"\x1B$@", ""); // 0208
        decode_iso_2022_jp(b"\x1B$B", ""); // 0208
        decode_iso_2022_jp(b"\x1B$(D", "\u{FFFD}$(D"); // 2012
        decode_iso_2022_jp(b"\x1B$A", "\u{FFFD}$A"); // GB2312
        decode_iso_2022_jp(b"\x1B$(C", "\u{FFFD}$(C"); // KR
        decode_iso_2022_jp(b"\x1B.A", "\u{FFFD}.A"); // Latin-1
        decode_iso_2022_jp(b"\x1B.F", "\u{FFFD}.F"); // Greek
        decode_iso_2022_jp(b"\x1B(I", ""); // Half-width Katakana
        decode_iso_2022_jp(b"\x1B$(O", "\u{FFFD}$(O"); // 2013
        decode_iso_2022_jp(b"\x1B$(P", "\u{FFFD}$(P"); // 2013
        decode_iso_2022_jp(b"\x1B$(Q", "\u{FFFD}$(Q"); // 2013
        decode_iso_2022_jp(b"\x1B$)C", "\u{FFFD}$)C"); // KR
        decode_iso_2022_jp(b"\x1B$)A", "\u{FFFD}$)A"); // GB2312
        decode_iso_2022_jp(b"\x1B$)G", "\u{FFFD}$)G"); // CNS
        decode_iso_2022_jp(b"\x1B$*H", "\u{FFFD}$*H"); // CNS
        decode_iso_2022_jp(b"\x1B$)E", "\u{FFFD}$)E"); // IR
        decode_iso_2022_jp(b"\x1B$+I", "\u{FFFD}$+I"); // CNS
        decode_iso_2022_jp(b"\x1B$+J", "\u{FFFD}$+J"); // CNS
        decode_iso_2022_jp(b"\x1B$+K", "\u{FFFD}$+K"); // CNS
        decode_iso_2022_jp(b"\x1B$+L", "\u{FFFD}$+L"); // CNS
        decode_iso_2022_jp(b"\x1B$+M", "\u{FFFD}$+M"); // CNS
        decode_iso_2022_jp(b"\x1B$(@", "\u{FFFD}$(@"); // 0208
        decode_iso_2022_jp(b"\x1B$(A", "\u{FFFD}$(A"); // GB2312
        decode_iso_2022_jp(b"\x1B$(B", "\u{FFFD}$(B"); // 0208
        decode_iso_2022_jp(b"\x1B%G", "\u{FFFD}%G"); // UTF-8

        // ASCII
        decode_iso_2022_jp(b"\x5B", "\u{005B}");
        decode_iso_2022_jp(b"\x5C", "\u{005C}");
        decode_iso_2022_jp(b"\x7E", "\u{007E}");
        decode_iso_2022_jp(b"\x0E", "\u{FFFD}");
        decode_iso_2022_jp(b"\x0F", "\u{FFFD}");
        decode_iso_2022_jp(b"\x80", "\u{FFFD}");
        decode_iso_2022_jp(b"\xFF", "\u{FFFD}");
        decode_iso_2022_jp(b"\x1B(B\x5B", "\u{005B}");
        decode_iso_2022_jp(b"\x1B(B\x5C", "\u{005C}");
        decode_iso_2022_jp(b"\x1B(B\x7E", "\u{007E}");
        decode_iso_2022_jp(b"\x1B(B\x0E", "\u{FFFD}");
        decode_iso_2022_jp(b"\x1B(B\x0F", "\u{FFFD}");
        decode_iso_2022_jp(b"\x1B(B\x80", "\u{FFFD}");
        decode_iso_2022_jp(b"\x1B(B\xFF", "\u{FFFD}");

        // Roman
        decode_iso_2022_jp(b"\x1B(J\x5B", "\u{005B}");
        decode_iso_2022_jp(b"\x1B(J\x5C", "\u{00A5}");
        decode_iso_2022_jp(b"\x1B(J\x7E", "\u{203E}");
        decode_iso_2022_jp(b"\x1B(J\x0E", "\u{FFFD}");
        decode_iso_2022_jp(b"\x1B(J\x0F", "\u{FFFD}");
        decode_iso_2022_jp(b"\x1B(J\x80", "\u{FFFD}");
        decode_iso_2022_jp(b"\x1B(J\xFF", "\u{FFFD}");

        // Katakana
        decode_iso_2022_jp(b"\x1B(I\x20", "\u{FFFD}");
        decode_iso_2022_jp(b"\x1B(I\x21", "\u{FF61}");
        decode_iso_2022_jp(b"\x1B(I\x5F", "\u{FF9F}");
        decode_iso_2022_jp(b"\x1B(I\x60", "\u{FFFD}");
        decode_iso_2022_jp(b"\x1B(I\x0E", "\u{FFFD}");
        decode_iso_2022_jp(b"\x1B(I\x0F", "\u{FFFD}");
        decode_iso_2022_jp(b"\x1B(I\x80", "\u{FFFD}");
        decode_iso_2022_jp(b"\x1B(I\xFF", "\u{FFFD}");

        // 0208 differences from 1978 to 1983
        decode_iso_2022_jp(b"\x1B$@\x54\x64", "\u{58FA}");
        decode_iso_2022_jp(b"\x1B$@\x44\x5B", "\u{58F7}");
        decode_iso_2022_jp(b"\x1B$@\x74\x21", "\u{582F}");
        decode_iso_2022_jp(b"\x1B$@\x36\x46", "\u{5C2D}");
        decode_iso_2022_jp(b"\x1B$@\x28\x2E", "\u{250F}");
        decode_iso_2022_jp(b"\x1B$B\x54\x64", "\u{58FA}");
        decode_iso_2022_jp(b"\x1B$B\x44\x5B", "\u{58F7}");
        decode_iso_2022_jp(b"\x1B$B\x74\x21", "\u{582F}");
        decode_iso_2022_jp(b"\x1B$B\x36\x46", "\u{5C2D}");
        decode_iso_2022_jp(b"\x1B$B\x28\x2E", "\u{250F}");

        // Broken 0208
        decode_iso_2022_jp(b"\x1B$B\x28\x41", "\u{FFFD}");
        decode_iso_2022_jp(b"\x1B$@\x80\x54\x64", "\u{FFFD}\u{58FA}");
        decode_iso_2022_jp(b"\x1B$B\x28\x80", "\u{FFFD}");

        if cfg!(miri) {
            // Miri is too slow
            return;
        }

        // Transitions
        decode_iso_2022_jp(b"\x1B(B\x5C\x1B(J\x5C", "\u{005C}\u{00A5}");
        decode_iso_2022_jp(b"\x1B(B\x5C\x1B(I\x21", "\u{005C}\u{FF61}");
        decode_iso_2022_jp(b"\x1B(B\x5C\x1B$@\x54\x64", "\u{005C}\u{58FA}");
        decode_iso_2022_jp(b"\x1B(B\x5C\x1B$B\x54\x64", "\u{005C}\u{58FA}");

        decode_iso_2022_jp(b"\x1B(J\x5C\x1B(B\x5C", "\u{00A5}\u{005C}");
        decode_iso_2022_jp(b"\x1B(J\x5C\x1B(I\x21", "\u{00A5}\u{FF61}");
        decode_iso_2022_jp(b"\x1B(J\x5C\x1B$@\x54\x64", "\u{00A5}\u{58FA}");
        decode_iso_2022_jp(b"\x1B(J\x5C\x1B$B\x54\x64", "\u{00A5}\u{58FA}");

        decode_iso_2022_jp(b"\x1B(I\x21\x1B(J\x5C", "\u{FF61}\u{00A5}");
        decode_iso_2022_jp(b"\x1B(I\x21\x1B(B\x5C", "\u{FF61}\u{005C}");
        decode_iso_2022_jp(b"\x1B(I\x21\x1B$@\x54\x64", "\u{FF61}\u{58FA}");
        decode_iso_2022_jp(b"\x1B(I\x21\x1B$B\x54\x64", "\u{FF61}\u{58FA}");

        decode_iso_2022_jp(b"\x1B$@\x54\x64\x1B(J\x5C", "\u{58FA}\u{00A5}");
        decode_iso_2022_jp(b"\x1B$@\x54\x64\x1B(I\x21", "\u{58FA}\u{FF61}");
        decode_iso_2022_jp(b"\x1B$@\x54\x64\x1B(B\x5C", "\u{58FA}\u{005C}");
        decode_iso_2022_jp(b"\x1B$@\x54\x64\x1B$B\x54\x64", "\u{58FA}\u{58FA}");

        decode_iso_2022_jp(b"\x1B$B\x54\x64\x1B(J\x5C", "\u{58FA}\u{00A5}");
        decode_iso_2022_jp(b"\x1B$B\x54\x64\x1B(I\x21", "\u{58FA}\u{FF61}");
        decode_iso_2022_jp(b"\x1B$B\x54\x64\x1B$@\x54\x64", "\u{58FA}\u{58FA}");
        decode_iso_2022_jp(b"\x1B$B\x54\x64\x1B(B\x5C", "\u{58FA}\u{005C}");

        // Empty transitions
        decode_iso_2022_jp(b"\x1B(B\x1B(J", "\u{FFFD}");
        decode_iso_2022_jp(b"\x1B(B\x1B(I", "\u{FFFD}");
        decode_iso_2022_jp(b"\x1B(B\x1B$@", "\u{FFFD}");
        decode_iso_2022_jp(b"\x1B(B\x1B$B", "\u{FFFD}");

        decode_iso_2022_jp(b"\x1B(J\x1B(B", "\u{FFFD}");
        decode_iso_2022_jp(b"\x1B(J\x1B(I", "\u{FFFD}");
        decode_iso_2022_jp(b"\x1B(J\x1B$@", "\u{FFFD}");
        decode_iso_2022_jp(b"\x1B(J\x1B$B", "\u{FFFD}");

        decode_iso_2022_jp(b"\x1B(I\x1B(J", "\u{FFFD}");
        decode_iso_2022_jp(b"\x1B(I\x1B(B", "\u{FFFD}");
        decode_iso_2022_jp(b"\x1B(I\x1B$@", "\u{FFFD}");
        decode_iso_2022_jp(b"\x1B(I\x1B$B", "\u{FFFD}");

        decode_iso_2022_jp(b"\x1B$@\x1B(J", "\u{FFFD}");
        decode_iso_2022_jp(b"\x1B$@\x1B(I", "\u{FFFD}");
        decode_iso_2022_jp(b"\x1B$@\x1B(B", "\u{FFFD}");
        decode_iso_2022_jp(b"\x1B$@\x1B$B", "\u{FFFD}");

        decode_iso_2022_jp(b"\x1B$B\x1B(J", "\u{FFFD}");
        decode_iso_2022_jp(b"\x1B$B\x1B(I", "\u{FFFD}");
        decode_iso_2022_jp(b"\x1B$B\x1B$@", "\u{FFFD}");
        decode_iso_2022_jp(b"\x1B$B\x1B(B", "\u{FFFD}");

        // Transitions to self
        decode_iso_2022_jp(b"\x1B(B\x5C\x1B(B\x5C", "\u{005C}\u{005C}");
        decode_iso_2022_jp(b"\x1B(J\x5C\x1B(J\x5C", "\u{00A5}\u{00A5}");
        decode_iso_2022_jp(b"\x1B(I\x21\x1B(I\x21", "\u{FF61}\u{FF61}");
        decode_iso_2022_jp(b"\x1B$@\x54\x64\x1B$@\x54\x64", "\u{58FA}\u{58FA}");
        decode_iso_2022_jp(b"\x1B$B\x54\x64\x1B$B\x54\x64", "\u{58FA}\u{58FA}");
    }

    #[test]
    fn test_iso_2022_jp_encode() {
        // Empty
        encode_iso_2022_jp("", b"");

        // ASCII
        encode_iso_2022_jp("ab", b"ab");
        encode_iso_2022_jp("\u{1F4A9}", b"&#128169;");
        encode_iso_2022_jp("\x1B", b"&#65533;");
        encode_iso_2022_jp("\x0E", b"&#65533;");
        encode_iso_2022_jp("\x0F", b"&#65533;");

        // Roman
        encode_iso_2022_jp("a\u{00A5}b", b"a\x1B(J\x5Cb\x1B(B");
        encode_iso_2022_jp("a\u{203E}b", b"a\x1B(J\x7Eb\x1B(B");
        if !cfg!(miri) {
            // Miri is too slow
            encode_iso_2022_jp("a\u{00A5}b\x5C", b"a\x1B(J\x5Cb\x1B(B\x5C");
            encode_iso_2022_jp("a\u{203E}b\x7E", b"a\x1B(J\x7Eb\x1B(B\x7E");
            encode_iso_2022_jp("\u{00A5}\u{1F4A9}", b"\x1B(J\x5C&#128169;\x1B(B");
            encode_iso_2022_jp("\u{00A5}\x1B", b"\x1B(J\x5C&#65533;\x1B(B");
            encode_iso_2022_jp("\u{00A5}\x0E", b"\x1B(J\x5C&#65533;\x1B(B");
            encode_iso_2022_jp("\u{00A5}\x0F", b"\x1B(J\x5C&#65533;\x1B(B");
            encode_iso_2022_jp("\u{00A5}\u{58FA}", b"\x1B(J\x5C\x1B$B\x54\x64\x1B(B");
        }

        // Half-width Katakana
        encode_iso_2022_jp("\u{FF61}", b"\x1B$B\x21\x23\x1B(B");
        encode_iso_2022_jp("\u{FF65}", b"\x1B$B\x21\x26\x1B(B");
        if !cfg!(miri) {
            // Miri is too slow
            encode_iso_2022_jp("\u{FF66}", b"\x1B$B\x25\x72\x1B(B");
            encode_iso_2022_jp("\u{FF70}", b"\x1B$B\x21\x3C\x1B(B");
            encode_iso_2022_jp("\u{FF9D}", b"\x1B$B\x25\x73\x1B(B");
            encode_iso_2022_jp("\u{FF9E}", b"\x1B$B\x21\x2B\x1B(B");
            encode_iso_2022_jp("\u{FF9F}", b"\x1B$B\x21\x2C\x1B(B");
        }

        // 0208
        encode_iso_2022_jp("\u{58FA}", b"\x1B$B\x54\x64\x1B(B");
        encode_iso_2022_jp("\u{58FA}\u{250F}", b"\x1B$B\x54\x64\x28\x2E\x1B(B");
        if !cfg!(miri) {
            // Miri is too slow
            encode_iso_2022_jp("\u{58FA}\u{1F4A9}", b"\x1B$B\x54\x64\x1B(B&#128169;");
            encode_iso_2022_jp("\u{58FA}\x1B", b"\x1B$B\x54\x64\x1B(B&#65533;");
            encode_iso_2022_jp("\u{58FA}\x0E", b"\x1B$B\x54\x64\x1B(B&#65533;");
            encode_iso_2022_jp("\u{58FA}\x0F", b"\x1B$B\x54\x64\x1B(B&#65533;");
            encode_iso_2022_jp("\u{58FA}\u{00A5}", b"\x1B$B\x54\x64\x1B(J\x5C\x1B(B");
            encode_iso_2022_jp("\u{58FA}a", b"\x1B$B\x54\x64\x1B(Ba");
        }
    }

    #[test]
    #[cfg_attr(miri, ignore)] // Miri is too slow
    fn test_iso_2022_jp_decode_all() {
        let input = include_bytes!("test_data/iso_2022_jp_in.txt");
        let expectation = include_str!("test_data/iso_2022_jp_in_ref.txt");
        let (cow, had_errors) = ISO_2022_JP.decode_without_bom_handling(input);
        assert!(had_errors, "Should have had errors.");
        assert_eq!(&cow[..], expectation);
    }

    #[test]
    #[cfg_attr(miri, ignore)] // Miri is too slow
    fn test_iso_2022_jp_encode_all() {
        let input = include_str!("test_data/iso_2022_jp_out.txt");
        let expectation = include_bytes!("test_data/iso_2022_jp_out_ref.txt");
        let (cow, encoding, had_errors) = ISO_2022_JP.encode(input);
        assert!(!had_errors, "Should not have had errors.");
        assert_eq!(encoding, ISO_2022_JP);
        assert_eq!(&cow[..], &expectation[..]);
    }

    #[test]
    fn test_iso_2022_jp_half_width_katakana_length() {
        let mut output = [0u8; 20];
        let mut decoder = ISO_2022_JP.new_decoder();
        {
            let (result, read, written) =
                decoder.decode_to_utf8_without_replacement(b"\x1B\x28\x49", &mut output, false);
            assert_eq!(result, DecoderResult::InputEmpty);
            assert_eq!(read, 3);
            assert_eq!(written, 0);
        }
        {
            let needed = decoder
                .max_utf8_buffer_length_without_replacement(1)
                .unwrap();
            let (result, read, written) =
                decoder.decode_to_utf8_without_replacement(b"\x21", &mut output[..needed], true);
            assert_eq!(result, DecoderResult::InputEmpty);
            assert_eq!(read, 1);
            assert_eq!(written, 3);
            assert_eq!(output[0], 0xEF);
            assert_eq!(output[1], 0xBD);
            assert_eq!(output[2], 0xA1);
        }
    }

    #[test]
    fn test_iso_2022_jp_length_after_escape() {
        let mut output = [0u16; 20];
        let mut decoder = ISO_2022_JP.new_decoder();
        {
            let (result, read, written, had_errors) =
                decoder.decode_to_utf16(b"\x1B", &mut output, false);
            assert_eq!(result, CoderResult::InputEmpty);
            assert_eq!(read, 1);
            assert_eq!(written, 0);
            assert!(!had_errors);
        }
        {
            let needed = decoder.max_utf16_buffer_length(1).unwrap();
            let (result, read, written, had_errors) =
                decoder.decode_to_utf16(b"A", &mut output[..needed], true);
            assert_eq!(result, CoderResult::InputEmpty);
            assert_eq!(read, 1);
            assert_eq!(written, 2);
            assert!(had_errors);
            assert_eq!(output[0], 0xFFFD);
            assert_eq!(output[1], 0x0041);
        }
    }

    #[test]
    fn test_iso_2022_jp_encode_from_two_low_surrogates() {
        let expectation = b"&#65533;&#65533;";
        let mut output = [0u8; 40];
        let mut encoder = ISO_2022_JP.new_encoder();
        let (result, read, written, had_errors) =
            encoder.encode_from_utf16(&[0xDC00u16, 0xDEDEu16], &mut output[..], true);
        assert_eq!(result, CoderResult::InputEmpty);
        assert_eq!(read, 2);
        assert_eq!(written, expectation.len());
        assert!(had_errors);
        assert_eq!(&output[..written], expectation);
    }
}
