// Copyright Mozilla Foundation. See the COPYRIGHT
// file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// THIS IS A GENERATED FILE. PLEASE DO NOT EDIT.
// Instead, please regenerate using generate-encoding-data.py

//! This module provides enums that wrap the various decoders and encoders.
//! The purpose is to make `Decoder` and `Encoder` `Sized` by writing the
//! dispatch explicitly for a finite set of specialized decoders and encoders.
//! Unfortunately, this means the compiler doesn't generate the dispatch code
//! and it has to be written here instead.
//!
//! The purpose of making `Decoder` and `Encoder` `Sized` is to allow stack
//! allocation in Rust code, including the convenience methods on `Encoding`.

use super::*;
use big5::*;
use euc_jp::*;
use euc_kr::*;
use gb18030::*;
use iso_2022_jp::*;
use replacement::*;
use shift_jis::*;
use single_byte::*;
use utf_16::*;
use utf_8::*;
use x_user_defined::*;

pub enum VariantDecoder {
    SingleByte(SingleByteDecoder),
    Utf8(Utf8Decoder),
    Gb18030(Gb18030Decoder),
    Big5(Big5Decoder),
    EucJp(EucJpDecoder),
    Iso2022Jp(Iso2022JpDecoder),
    ShiftJis(ShiftJisDecoder),
    EucKr(EucKrDecoder),
    Replacement(ReplacementDecoder),
    UserDefined(UserDefinedDecoder),
    Utf16(Utf16Decoder),
}

impl VariantDecoder {
    pub fn max_utf16_buffer_length(&self, byte_length: usize) -> Option<usize> {
        match *self {
            VariantDecoder::SingleByte(ref v) => v.max_utf16_buffer_length(byte_length),
            VariantDecoder::Utf8(ref v) => v.max_utf16_buffer_length(byte_length),
            VariantDecoder::Gb18030(ref v) => v.max_utf16_buffer_length(byte_length),
            VariantDecoder::Big5(ref v) => v.max_utf16_buffer_length(byte_length),
            VariantDecoder::EucJp(ref v) => v.max_utf16_buffer_length(byte_length),
            VariantDecoder::Iso2022Jp(ref v) => v.max_utf16_buffer_length(byte_length),
            VariantDecoder::ShiftJis(ref v) => v.max_utf16_buffer_length(byte_length),
            VariantDecoder::EucKr(ref v) => v.max_utf16_buffer_length(byte_length),
            VariantDecoder::Replacement(ref v) => v.max_utf16_buffer_length(byte_length),
            VariantDecoder::UserDefined(ref v) => v.max_utf16_buffer_length(byte_length),
            VariantDecoder::Utf16(ref v) => v.max_utf16_buffer_length(byte_length),
        }
    }

    pub fn max_utf8_buffer_length_without_replacement(&self, byte_length: usize) -> Option<usize> {
        match *self {
            VariantDecoder::SingleByte(ref v) => {
                v.max_utf8_buffer_length_without_replacement(byte_length)
            }
            VariantDecoder::Utf8(ref v) => {
                v.max_utf8_buffer_length_without_replacement(byte_length)
            }
            VariantDecoder::Gb18030(ref v) => {
                v.max_utf8_buffer_length_without_replacement(byte_length)
            }
            VariantDecoder::Big5(ref v) => {
                v.max_utf8_buffer_length_without_replacement(byte_length)
            }
            VariantDecoder::EucJp(ref v) => {
                v.max_utf8_buffer_length_without_replacement(byte_length)
            }
            VariantDecoder::Iso2022Jp(ref v) => {
                v.max_utf8_buffer_length_without_replacement(byte_length)
            }
            VariantDecoder::ShiftJis(ref v) => {
                v.max_utf8_buffer_length_without_replacement(byte_length)
            }
            VariantDecoder::EucKr(ref v) => {
                v.max_utf8_buffer_length_without_replacement(byte_length)
            }
            VariantDecoder::Replacement(ref v) => {
                v.max_utf8_buffer_length_without_replacement(byte_length)
            }
            VariantDecoder::UserDefined(ref v) => {
                v.max_utf8_buffer_length_without_replacement(byte_length)
            }
            VariantDecoder::Utf16(ref v) => {
                v.max_utf8_buffer_length_without_replacement(byte_length)
            }
        }
    }

    pub fn max_utf8_buffer_length(&self, byte_length: usize) -> Option<usize> {
        match *self {
            VariantDecoder::SingleByte(ref v) => v.max_utf8_buffer_length(byte_length),
            VariantDecoder::Utf8(ref v) => v.max_utf8_buffer_length(byte_length),
            VariantDecoder::Gb18030(ref v) => v.max_utf8_buffer_length(byte_length),
            VariantDecoder::Big5(ref v) => v.max_utf8_buffer_length(byte_length),
            VariantDecoder::EucJp(ref v) => v.max_utf8_buffer_length(byte_length),
            VariantDecoder::Iso2022Jp(ref v) => v.max_utf8_buffer_length(byte_length),
            VariantDecoder::ShiftJis(ref v) => v.max_utf8_buffer_length(byte_length),
            VariantDecoder::EucKr(ref v) => v.max_utf8_buffer_length(byte_length),
            VariantDecoder::Replacement(ref v) => v.max_utf8_buffer_length(byte_length),
            VariantDecoder::UserDefined(ref v) => v.max_utf8_buffer_length(byte_length),
            VariantDecoder::Utf16(ref v) => v.max_utf8_buffer_length(byte_length),
        }
    }

    pub fn decode_to_utf16_raw(
        &mut self,
        src: &[u8],
        dst: &mut [u16],
        last: bool,
    ) -> (DecoderResult, usize, usize) {
        match *self {
            VariantDecoder::SingleByte(ref mut v) => v.decode_to_utf16_raw(src, dst, last),
            VariantDecoder::Utf8(ref mut v) => v.decode_to_utf16_raw(src, dst, last),
            VariantDecoder::Gb18030(ref mut v) => v.decode_to_utf16_raw(src, dst, last),
            VariantDecoder::Big5(ref mut v) => v.decode_to_utf16_raw(src, dst, last),
            VariantDecoder::EucJp(ref mut v) => v.decode_to_utf16_raw(src, dst, last),
            VariantDecoder::Iso2022Jp(ref mut v) => v.decode_to_utf16_raw(src, dst, last),
            VariantDecoder::ShiftJis(ref mut v) => v.decode_to_utf16_raw(src, dst, last),
            VariantDecoder::EucKr(ref mut v) => v.decode_to_utf16_raw(src, dst, last),
            VariantDecoder::Replacement(ref mut v) => v.decode_to_utf16_raw(src, dst, last),
            VariantDecoder::UserDefined(ref mut v) => v.decode_to_utf16_raw(src, dst, last),
            VariantDecoder::Utf16(ref mut v) => v.decode_to_utf16_raw(src, dst, last),
        }
    }

    pub fn decode_to_utf8_raw(
        &mut self,
        src: &[u8],
        dst: &mut [u8],
        last: bool,
    ) -> (DecoderResult, usize, usize) {
        match *self {
            VariantDecoder::SingleByte(ref mut v) => v.decode_to_utf8_raw(src, dst, last),
            VariantDecoder::Utf8(ref mut v) => v.decode_to_utf8_raw(src, dst, last),
            VariantDecoder::Gb18030(ref mut v) => v.decode_to_utf8_raw(src, dst, last),
            VariantDecoder::Big5(ref mut v) => v.decode_to_utf8_raw(src, dst, last),
            VariantDecoder::EucJp(ref mut v) => v.decode_to_utf8_raw(src, dst, last),
            VariantDecoder::Iso2022Jp(ref mut v) => v.decode_to_utf8_raw(src, dst, last),
            VariantDecoder::ShiftJis(ref mut v) => v.decode_to_utf8_raw(src, dst, last),
            VariantDecoder::EucKr(ref mut v) => v.decode_to_utf8_raw(src, dst, last),
            VariantDecoder::Replacement(ref mut v) => v.decode_to_utf8_raw(src, dst, last),
            VariantDecoder::UserDefined(ref mut v) => v.decode_to_utf8_raw(src, dst, last),
            VariantDecoder::Utf16(ref mut v) => v.decode_to_utf8_raw(src, dst, last),
        }
    }

    pub fn latin1_byte_compatible_up_to(&self, buffer: &[u8]) -> Option<usize> {
        match *self {
            VariantDecoder::SingleByte(ref v) => {
                return Some(v.latin1_byte_compatible_up_to(buffer));
            }
            VariantDecoder::Utf8(ref v) => {
                if !v.in_neutral_state() {
                    return None;
                }
            }
            VariantDecoder::Gb18030(ref v) => {
                if !v.in_neutral_state() {
                    return None;
                }
            }
            VariantDecoder::Big5(ref v) => {
                if !v.in_neutral_state() {
                    return None;
                }
            }
            VariantDecoder::EucJp(ref v) => {
                if !v.in_neutral_state() {
                    return None;
                }
            }
            VariantDecoder::Iso2022Jp(ref v) => {
                if v.in_neutral_state() {
                    return Some(Encoding::iso_2022_jp_ascii_valid_up_to(buffer));
                }
                return None;
            }
            VariantDecoder::ShiftJis(ref v) => {
                if !v.in_neutral_state() {
                    return None;
                }
            }
            VariantDecoder::EucKr(ref v) => {
                if !v.in_neutral_state() {
                    return None;
                }
            }
            VariantDecoder::UserDefined(_) => {}
            VariantDecoder::Replacement(_) | VariantDecoder::Utf16(_) => {
                return None;
            }
        };
        Some(Encoding::ascii_valid_up_to(buffer))
    }
}

pub enum VariantEncoder {
    SingleByte(SingleByteEncoder),
    Utf8(Utf8Encoder),
    Gb18030(Gb18030Encoder),
    Big5(Big5Encoder),
    EucJp(EucJpEncoder),
    Iso2022Jp(Iso2022JpEncoder),
    ShiftJis(ShiftJisEncoder),
    EucKr(EucKrEncoder),
    UserDefined(UserDefinedEncoder),
}

impl VariantEncoder {
    pub fn has_pending_state(&self) -> bool {
        match *self {
            VariantEncoder::Iso2022Jp(ref v) => v.has_pending_state(),
            _ => false,
        }
    }
    pub fn max_buffer_length_from_utf16_without_replacement(
        &self,
        u16_length: usize,
    ) -> Option<usize> {
        match *self {
            VariantEncoder::SingleByte(ref v) => {
                v.max_buffer_length_from_utf16_without_replacement(u16_length)
            }
            VariantEncoder::Utf8(ref v) => {
                v.max_buffer_length_from_utf16_without_replacement(u16_length)
            }
            VariantEncoder::Gb18030(ref v) => {
                v.max_buffer_length_from_utf16_without_replacement(u16_length)
            }
            VariantEncoder::Big5(ref v) => {
                v.max_buffer_length_from_utf16_without_replacement(u16_length)
            }
            VariantEncoder::EucJp(ref v) => {
                v.max_buffer_length_from_utf16_without_replacement(u16_length)
            }
            VariantEncoder::Iso2022Jp(ref v) => {
                v.max_buffer_length_from_utf16_without_replacement(u16_length)
            }
            VariantEncoder::ShiftJis(ref v) => {
                v.max_buffer_length_from_utf16_without_replacement(u16_length)
            }
            VariantEncoder::EucKr(ref v) => {
                v.max_buffer_length_from_utf16_without_replacement(u16_length)
            }
            VariantEncoder::UserDefined(ref v) => {
                v.max_buffer_length_from_utf16_without_replacement(u16_length)
            }
        }
    }

    pub fn max_buffer_length_from_utf8_without_replacement(
        &self,
        byte_length: usize,
    ) -> Option<usize> {
        match *self {
            VariantEncoder::SingleByte(ref v) => {
                v.max_buffer_length_from_utf8_without_replacement(byte_length)
            }
            VariantEncoder::Utf8(ref v) => {
                v.max_buffer_length_from_utf8_without_replacement(byte_length)
            }
            VariantEncoder::Gb18030(ref v) => {
                v.max_buffer_length_from_utf8_without_replacement(byte_length)
            }
            VariantEncoder::Big5(ref v) => {
                v.max_buffer_length_from_utf8_without_replacement(byte_length)
            }
            VariantEncoder::EucJp(ref v) => {
                v.max_buffer_length_from_utf8_without_replacement(byte_length)
            }
            VariantEncoder::Iso2022Jp(ref v) => {
                v.max_buffer_length_from_utf8_without_replacement(byte_length)
            }
            VariantEncoder::ShiftJis(ref v) => {
                v.max_buffer_length_from_utf8_without_replacement(byte_length)
            }
            VariantEncoder::EucKr(ref v) => {
                v.max_buffer_length_from_utf8_without_replacement(byte_length)
            }
            VariantEncoder::UserDefined(ref v) => {
                v.max_buffer_length_from_utf8_without_replacement(byte_length)
            }
        }
    }

    pub fn encode_from_utf16_raw(
        &mut self,
        src: &[u16],
        dst: &mut [u8],
        last: bool,
    ) -> (EncoderResult, usize, usize) {
        match *self {
            VariantEncoder::SingleByte(ref mut v) => v.encode_from_utf16_raw(src, dst, last),
            VariantEncoder::Utf8(ref mut v) => v.encode_from_utf16_raw(src, dst, last),
            VariantEncoder::Gb18030(ref mut v) => v.encode_from_utf16_raw(src, dst, last),
            VariantEncoder::Big5(ref mut v) => v.encode_from_utf16_raw(src, dst, last),
            VariantEncoder::EucJp(ref mut v) => v.encode_from_utf16_raw(src, dst, last),
            VariantEncoder::Iso2022Jp(ref mut v) => v.encode_from_utf16_raw(src, dst, last),
            VariantEncoder::ShiftJis(ref mut v) => v.encode_from_utf16_raw(src, dst, last),
            VariantEncoder::EucKr(ref mut v) => v.encode_from_utf16_raw(src, dst, last),
            VariantEncoder::UserDefined(ref mut v) => v.encode_from_utf16_raw(src, dst, last),
        }
    }

    pub fn encode_from_utf8_raw(
        &mut self,
        src: &str,
        dst: &mut [u8],
        last: bool,
    ) -> (EncoderResult, usize, usize) {
        match *self {
            VariantEncoder::SingleByte(ref mut v) => v.encode_from_utf8_raw(src, dst, last),
            VariantEncoder::Utf8(ref mut v) => v.encode_from_utf8_raw(src, dst, last),
            VariantEncoder::Gb18030(ref mut v) => v.encode_from_utf8_raw(src, dst, last),
            VariantEncoder::Big5(ref mut v) => v.encode_from_utf8_raw(src, dst, last),
            VariantEncoder::EucJp(ref mut v) => v.encode_from_utf8_raw(src, dst, last),
            VariantEncoder::Iso2022Jp(ref mut v) => v.encode_from_utf8_raw(src, dst, last),
            VariantEncoder::ShiftJis(ref mut v) => v.encode_from_utf8_raw(src, dst, last),
            VariantEncoder::EucKr(ref mut v) => v.encode_from_utf8_raw(src, dst, last),
            VariantEncoder::UserDefined(ref mut v) => v.encode_from_utf8_raw(src, dst, last),
        }
    }
}

pub enum VariantEncoding {
    SingleByte(&'static [u16; 128], u16, u8, u8),
    Utf8,
    Gbk,
    Gb18030,
    Big5,
    EucJp,
    Iso2022Jp,
    ShiftJis,
    EucKr,
    Replacement,
    Utf16Be,
    Utf16Le,
    UserDefined,
}

impl VariantEncoding {
    pub fn new_variant_decoder(&self) -> VariantDecoder {
        match *self {
            VariantEncoding::SingleByte(table, _, _, _) => SingleByteDecoder::new(table),
            VariantEncoding::Utf8 => Utf8Decoder::new(),
            VariantEncoding::Gbk | VariantEncoding::Gb18030 => Gb18030Decoder::new(),
            VariantEncoding::Big5 => Big5Decoder::new(),
            VariantEncoding::EucJp => EucJpDecoder::new(),
            VariantEncoding::Iso2022Jp => Iso2022JpDecoder::new(),
            VariantEncoding::ShiftJis => ShiftJisDecoder::new(),
            VariantEncoding::EucKr => EucKrDecoder::new(),
            VariantEncoding::Replacement => ReplacementDecoder::new(),
            VariantEncoding::UserDefined => UserDefinedDecoder::new(),
            VariantEncoding::Utf16Be => Utf16Decoder::new(true),
            VariantEncoding::Utf16Le => Utf16Decoder::new(false),
        }
    }

    pub fn new_encoder(&self, encoding: &'static Encoding) -> Encoder {
        match *self {
            VariantEncoding::SingleByte(table, run_bmp_offset, run_byte_offset, run_length) => {
                SingleByteEncoder::new(encoding, table, run_bmp_offset, run_byte_offset, run_length)
            }
            VariantEncoding::Utf8 => Utf8Encoder::new(encoding),
            VariantEncoding::Gbk => Gb18030Encoder::new(encoding, false),
            VariantEncoding::Gb18030 => Gb18030Encoder::new(encoding, true),
            VariantEncoding::Big5 => Big5Encoder::new(encoding),
            VariantEncoding::EucJp => EucJpEncoder::new(encoding),
            VariantEncoding::Iso2022Jp => Iso2022JpEncoder::new(encoding),
            VariantEncoding::ShiftJis => ShiftJisEncoder::new(encoding),
            VariantEncoding::EucKr => EucKrEncoder::new(encoding),
            VariantEncoding::UserDefined => UserDefinedEncoder::new(encoding),
            VariantEncoding::Utf16Be | VariantEncoding::Replacement | VariantEncoding::Utf16Le => {
                unreachable!()
            }
        }
    }

    pub fn is_single_byte(&self) -> bool {
        match *self {
            VariantEncoding::SingleByte(_, _, _, _) | VariantEncoding::UserDefined => true,
            _ => false,
        }
    }
}
