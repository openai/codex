/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

#![allow(
    clippy::if_not_else,
    clippy::similar_names,
    clippy::inline_always,
    clippy::doc_markdown,
    clippy::cast_sign_loss,
    clippy::cast_possible_truncation
)]

//! This file exposes a single struct that can decode a huffman encoded
//! Bitstream in a JPEG file
//!
//! This code is optimized for speed.
//! It's meant to be super duper super fast, because everyone else depends on this being fast.
//! It's (annoyingly) serial hence we cant use parallel bitstreams(it's variable length coding.)
//!
//! Furthermore, on the case of refills, we have to do bytewise processing because the standard decided
//! that we want to support markers in the middle of streams(seriously few people use RST markers).
//!
//! So we pull in all optimization steps:
//! - use `inline[always]`? ✅ ,
//! - pre-execute most common cases ✅,
//! - add random comments ✅
//! -  fast paths ✅.
//!
//! Speed-wise: It is probably the fastest JPEG BitStream decoder to ever sail the seven seas because of
//! a couple of optimization tricks.
//! 1. Fast refills from libjpeg-turbo
//! 2. As few as possible branches in decoder fast paths.
//! 3. Accelerated AC table decoding borrowed from stb_image.h written by Fabian Gissen (@ rygorous),
//! improved by me to handle more cases.
//! 4. Safe and extensible routines(e.g. cool ways to eliminate bounds check)
//! 5. No unsafe here
//!
//! Readability comes as a second priority(I tried with variable names this time, and we are wayy better than libjpeg).
//!
//! Anyway if you are reading this it means your cool and I hope you get whatever part of the code you are looking for
//! (or learn something cool)
//!
//! Knock yourself out.
use alloc::string::ToString;
use core::cmp::min;
use alloc::format;

use zune_core::bytestream::{ZByteReader, ZReaderTrait};

use crate::errors::DecodeErrors;
use crate::huffman::{HuffmanTable, HUFF_LOOKAHEAD};
use crate::marker::Marker;
use crate::mcu::DCT_BLOCK;
use crate::misc::UN_ZIGZAG;

macro_rules! decode_huff {
    ($stream:tt,$symbol:tt,$table:tt) => {
        let mut code_length = $symbol >> HUFF_LOOKAHEAD;

        ($symbol) &= (1 << HUFF_LOOKAHEAD) - 1;

        if code_length > i32::from(HUFF_LOOKAHEAD)
        {
            // if the symbol cannot be resolved in the first HUFF_LOOKAHEAD bits,
            // we know it lies somewhere between HUFF_LOOKAHEAD and 16 bits since jpeg imposes 16 bit
            // limit, we can therefore look 16 bits ahead and try to resolve the symbol
            // starting from 1+HUFF_LOOKAHEAD bits.
            $symbol = ($stream).peek_bits::<16>() as i32;
            // (Credits to Sean T. Barrett stb library for this optimization)
            // maxcode is pre-shifted 16 bytes long so that it has (16-code_length)
            // zeroes at the end hence we do not need to shift in the inner loop.
            while code_length < 17{
                if $symbol < $table.maxcode[code_length as usize]  {
                    break;
                }
                code_length += 1;
            }

            if code_length == 17{
                // symbol could not be decoded.
                //
                // We may think, lets fake zeroes, noo
                // panic, because Huffman codes are sensitive, probably everything
                // after this will be corrupt, so no need to continue.
                // panic!("Bad Huffman code length");
                return Err(DecodeErrors::Format(format!("Bad Huffman Code 0x{:X}, corrupt JPEG",$symbol)))
            }

            $symbol >>= (16-code_length);
            ($symbol) = i32::from(
                ($table).values
                    [(($symbol + ($table).offset[code_length as usize]) & 0xFF) as usize],
            );
        }
        // drop bits read
        ($stream).drop_bits(code_length as u8);
    };
}

/// A `BitStream` struct, a bit by bit reader with super powers
///
pub(crate) struct BitStream {
    /// A MSB type buffer that is used for some certain operations
    pub buffer: u64,
    /// A TOP  aligned MSB type buffer that is used to accelerate some operations like
    /// peek_bits and get_bits.
    ///
    /// By top aligned, I mean the top bit (63) represents the top bit in the buffer.
    aligned_buffer: u64,
    /// Tell us the bits left the two buffer
    pub(crate) bits_left: u8,
    /// Did we find a marker(RST/EOF) during decoding?
    pub marker: Option<Marker>,

    /// Progressive decoding
    pub successive_high: u8,
    pub successive_low: u8,
    spec_start: u8,
    spec_end: u8,
    pub eob_run: i32,
    pub overread_by: usize,
    /// True if we have seen end of image marker.
    /// Don't read anything after that.
    pub seen_eoi: bool,
}

impl BitStream {
    /// Create a new BitStream
    pub(crate) const fn new() -> BitStream {
        BitStream {
            buffer: 0,
            aligned_buffer: 0,
            bits_left: 0,
            marker: None,
            successive_high: 0,
            successive_low: 0,
            spec_start: 0,
            spec_end: 0,
            eob_run: 0,
            overread_by: 0,
            seen_eoi: false,
        }
    }

    /// Create a new Bitstream for progressive decoding
    #[allow(clippy::redundant_field_names)]
    pub(crate) fn new_progressive(ah: u8, al: u8, spec_start: u8, spec_end: u8) -> BitStream {
        BitStream {
            buffer: 0,
            aligned_buffer: 0,
            bits_left: 0,
            marker: None,
            successive_high: ah,
            successive_low: al,
            spec_start: spec_start,
            spec_end: spec_end,
            eob_run: 0,
            overread_by: 0,
            seen_eoi: false,
        }
    }

    /// Refill the bit buffer by (a maximum of) 32 bits
    ///
    /// # Arguments
    ///  - `reader`:`&mut BufReader<R>`: A mutable reference to an underlying
    ///    File/Memory buffer containing a valid JPEG stream
    ///
    /// This function will only refill if `self.count` is less than 32
    #[inline(always)] // to many call sites? ( perf improvement by 4%)
    pub(crate) fn refill<T>(&mut self, reader: &mut ZByteReader<T>) -> Result<bool, DecodeErrors>
    where
        T: ZReaderTrait
    {
        /// Macro version of a single byte refill.
        /// Arguments
        /// buffer-> our io buffer, because rust macros cannot get values from
        /// the surrounding environment bits_left-> number of bits left
        /// to full refill
        macro_rules! refill {
            ($buffer:expr,$byte:expr,$bits_left:expr) => {
                // read a byte from the stream
                $byte = u64::from(reader.get_u8());
                self.overread_by += usize::from(reader.eof());
                // append to the buffer
                // JPEG is a MSB type buffer so that means we append this
                // to the lower end (0..8) of the buffer and push the rest bits above..
                $buffer = ($buffer << 8) | $byte;
                // Increment bits left
                $bits_left += 8;
                // Check for special case  of OxFF, to see if it's a stream or a marker
                if $byte == 0xff {
                    // read next byte
                    let mut next_byte = u64::from(reader.get_u8());
                    // Byte snuffing, if we encounter byte snuff, we skip the byte
                    if next_byte != 0x00 {
                        // skip that byte we read
                        while next_byte == 0xFF {
                            next_byte = u64::from(reader.get_u8());
                        }

                        if next_byte != 0x00 {
                            // Undo the byte append and return
                            $buffer >>= 8;
                            $bits_left -= 8;

                            if $bits_left != 0 {
                                self.aligned_buffer = $buffer << (64 - $bits_left);
                            }

                            self.marker = Marker::from_u8(next_byte as u8);
                            if next_byte == 0xD9 {
                                // special handling for eoi, fill some bytes,even if its zero,
                                // removes some panics
                                self.buffer <<= 8;
                                self.bits_left += 8;
                                self.aligned_buffer = self.buffer << (64 - self.bits_left);

                            }

                            return Ok(false);
                        }
                    }
                }
            };
        }


        // 32 bits is enough for a decode(16 bits) and receive_extend(max 16 bits)
        if self.bits_left < 32 {
            if self.marker.is_some() || self.overread_by > 0  || self.seen_eoi{
                // found a marker, or we are in EOI
                // also we are in over-reading mode, where we fill it with zeroes

                // fill with zeroes
                self.buffer <<= 32;
                self.bits_left += 32;
                self.aligned_buffer = self.buffer << (64 - self.bits_left);
                return Ok(true);
            }


            // we optimize for the case where we don't have 255 in the stream and have 4 bytes left
            // as it is the common case
            //
            // so we always read 4 bytes, if read_fixed_bytes errors out, the cursor is
            // guaranteed not to advance in case of failure (is this true), so
            // we revert the read later on (if we have 255), if this fails, we use the normal
            // byte at a time read

            if let Ok(bytes) = reader.get_fixed_bytes_or_err::<4>() {
                // we have 4 bytes to spare, read the 4 bytes into a temporary buffer
                // create buffer
                let msb_buf = u32::from_be_bytes(bytes);
                // check if we have 0xff
                if !has_byte(msb_buf, 255) {
                    self.bits_left += 32;
                    self.buffer <<= 32;
                    self.buffer |= u64::from(msb_buf);
                    self.aligned_buffer = self.buffer << (64 - self.bits_left);
                    return Ok(true);
                }

                reader.rewind(4);
            }
            // This serves two reasons,
            // 1: Make clippy shut up
            // 2: Favour register reuse
            let mut byte;
            // 4 refills, if all succeed the stream should contain enough bits to decode a
            // value
            refill!(self.buffer, byte, self.bits_left);
            refill!(self.buffer, byte, self.bits_left);
            refill!(self.buffer, byte, self.bits_left);
            refill!(self.buffer, byte, self.bits_left);
            // Construct an MSB buffer whose top bits are the bitstream we are currently holding.
            self.aligned_buffer = self.buffer << (64 - self.bits_left);
        }
        return Ok(true);
    }
    /// Decode the DC coefficient in a MCU block.
    ///
    /// The decoded coefficient is written to `dc_prediction`
    ///
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::unwrap_used
    )]
    #[inline(always)]
    fn decode_dc<T>(
        &mut self, reader: &mut ZByteReader<T>, dc_table: &HuffmanTable, dc_prediction: &mut i32
    ) -> Result<bool, DecodeErrors>
    where
        T: ZReaderTrait
    {
        let (mut symbol, r);

        if self.bits_left < 32 {
            self.refill(reader)?;
        };
        // look a head HUFF_LOOKAHEAD bits into the bitstream
        symbol = self.peek_bits::<HUFF_LOOKAHEAD>();
        symbol = dc_table.lookup[symbol as usize];

        decode_huff!(self, symbol, dc_table);

        if symbol != 0 {
            r = self.get_bits(symbol as u8);
            symbol = huff_extend(r, symbol);
        }
        // Update DC prediction
        *dc_prediction = dc_prediction.wrapping_add(symbol);

        return Ok(true);
    }

    /// Decode a Minimum Code Unit(MCU) as quickly as possible
    ///
    /// # Arguments
    /// - reader: The bitstream from where we read more bits.
    /// - dc_table: The Huffman table used to decode the DC coefficient
    /// - ac_table: The Huffman table used to decode AC values
    /// - block: A memory region where we will write out the decoded values
    /// - DC prediction: Last DC value for this component
    ///
    #[allow(
        clippy::many_single_char_names,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss
    )]
    #[inline(never)]
    pub fn decode_mcu_block<T>(
        &mut self, reader: &mut ZByteReader<T>, dc_table: &HuffmanTable, ac_table: &HuffmanTable,
        qt_table: &[i32; DCT_BLOCK], block: &mut [i32; 64], dc_prediction: &mut i32
    ) -> Result<(), DecodeErrors>
    where
        T: ZReaderTrait
    {
        // Get fast AC table as a reference before we enter the hot path
        let ac_lookup = ac_table.ac_lookup.as_ref().unwrap();

        let (mut symbol, mut r, mut fast_ac);
        // Decode AC coefficients
        let mut pos: usize = 1;

        // decode DC, dc prediction will contain the value
        self.decode_dc(reader, dc_table, dc_prediction)?;

        // set dc to be the dc prediction.
        block[0] = *dc_prediction * qt_table[0];

        while pos < 64 {
            self.refill(reader)?;
            symbol = self.peek_bits::<HUFF_LOOKAHEAD>();
            fast_ac = ac_lookup[symbol as usize];
            symbol = ac_table.lookup[symbol as usize];

            if fast_ac != 0 {
                //  FAST AC path
                pos += ((fast_ac >> 4) & 15) as usize; // run
                let t_pos = UN_ZIGZAG[min(pos, 63)] & 63;

                block[t_pos] = i32::from(fast_ac >> 8) * (qt_table[t_pos]); // Value
                self.drop_bits((fast_ac & 15) as u8);
                pos += 1;
            } else {
                decode_huff!(self, symbol, ac_table);

                r = symbol >> 4;
                symbol &= 15;

                if symbol != 0 {
                    pos += r as usize;
                    r = self.get_bits(symbol as u8);
                    symbol = huff_extend(r, symbol);
                    let t_pos = UN_ZIGZAG[pos & 63] & 63;

                    block[t_pos] = symbol * qt_table[t_pos];

                    pos += 1;
                } else if r != 15 {
                    return Ok(());
                } else {
                    pos += 16;
                }
            }
        }
        return Ok(());
    }

    /// Peek `look_ahead` bits ahead without discarding them from the buffer
    #[inline(always)]
    #[allow(clippy::cast_possible_truncation)]
    const fn peek_bits<const LOOKAHEAD: u8>(&self) -> i32 {
        (self.aligned_buffer >> (64 - LOOKAHEAD)) as i32
    }

    /// Discard the next `N` bits without checking
    #[inline]
    fn drop_bits(&mut self, n: u8) {
        debug_assert!(self.bits_left >= n);
        //self.bits_left -= n;
        self.bits_left = self.bits_left.saturating_sub(n);
        self.aligned_buffer <<= n;
    }

    /// Read `n_bits` from the buffer  and discard them
    #[inline(always)]
    #[allow(clippy::cast_possible_truncation)]
    fn get_bits(&mut self, n_bits: u8) -> i32 {
        let mask = (1_u64 << n_bits) - 1;

        self.aligned_buffer = self.aligned_buffer.rotate_left(u32::from(n_bits));
        let bits = (self.aligned_buffer & mask) as i32;
        self.bits_left = self.bits_left.wrapping_sub(n_bits);
        bits
    }

    /// Decode a DC block
    #[allow(clippy::cast_possible_truncation)]
    #[inline]
    pub(crate) fn decode_prog_dc_first<T>(
        &mut self, reader: &mut ZByteReader<T>, dc_table: &HuffmanTable, block: &mut i16,
        dc_prediction: &mut i32
    ) -> Result<(), DecodeErrors>
    where
        T: ZReaderTrait
    {
        self.decode_dc(reader, dc_table, dc_prediction)?;
        *block = (*dc_prediction as i16).wrapping_mul(1_i16 << self.successive_low);
        return Ok(());
    }
    #[inline]
    pub(crate) fn decode_prog_dc_refine<T>(
        &mut self, reader: &mut ZByteReader<T>, block: &mut i16
    ) -> Result<(), DecodeErrors>
    where
        T: ZReaderTrait
    {
        // refinement scan
        if self.bits_left < 1 {
            self.refill(reader)?;
        }

        if self.get_bit() == 1 {
            *block = block.wrapping_add(1 << self.successive_low);
        }

        Ok(())
    }

    /// Get a single bit from the bitstream
    fn get_bit(&mut self) -> u8 {
        let k = (self.aligned_buffer >> 63) as u8;
        // discard a bit
        self.drop_bits(1);
        return k;
    }
    pub(crate) fn decode_mcu_ac_first<T>(
        &mut self, reader: &mut ZByteReader<T>, ac_table: &HuffmanTable, block: &mut [i16; 64]
    ) -> Result<bool, DecodeErrors>
    where
        T: ZReaderTrait
    {
        let shift = self.successive_low;
        let fast_ac = ac_table.ac_lookup.as_ref().unwrap();

        let mut k = self.spec_start as usize;
        let (mut symbol, mut r, mut fac);

        // EOB runs are handled in mcu_prog.rs
        'block: loop {
            self.refill(reader)?;

            symbol = self.peek_bits::<HUFF_LOOKAHEAD>();
            fac = fast_ac[symbol as usize];
            symbol = ac_table.lookup[symbol as usize];

            if fac != 0 {
                // fast ac path
                k += ((fac >> 4) & 15) as usize; // run
                block[UN_ZIGZAG[min(k, 63)] & 63] = (fac >> 8).wrapping_mul(1 << shift); // value
                self.drop_bits((fac & 15) as u8);
                k += 1;
            } else {
                decode_huff!(self, symbol, ac_table);

                r = symbol >> 4;
                symbol &= 15;

                if symbol != 0 {
                    k += r as usize;
                    r = self.get_bits(symbol as u8);
                    symbol = huff_extend(r, symbol);
                    block[UN_ZIGZAG[k & 63] & 63] = (symbol as i16).wrapping_mul(1 << shift);
                    k += 1;
                } else {
                    if r != 15 {
                        self.eob_run = 1 << r;
                        self.eob_run += self.get_bits(r as u8);
                        self.eob_run -= 1;
                        break;
                    }

                    k += 16;
                }
            }

            if k > self.spec_end as usize {
                break 'block;
            }
        }
        return Ok(true);
    }
    #[allow(clippy::too_many_lines, clippy::op_ref)]
    pub(crate) fn decode_mcu_ac_refine<T>(
        &mut self, reader: &mut ZByteReader<T>, table: &HuffmanTable, block: &mut [i16; 64]
    ) -> Result<bool, DecodeErrors>
    where
        T: ZReaderTrait
    {
        let bit = (1 << self.successive_low) as i16;

        let mut k = self.spec_start;
        let (mut symbol, mut r);

        if self.eob_run == 0 {
            'no_eob: loop {
                // Decode a coefficient from the bit stream
                self.refill(reader)?;

                symbol = self.peek_bits::<HUFF_LOOKAHEAD>();
                symbol = table.lookup[symbol as usize];

                decode_huff!(self, symbol, table);

                r = symbol >> 4;
                symbol &= 15;

                if symbol == 0 {
                    if r != 15 {
                        // EOB run is 2^r + bits
                        self.eob_run = 1 << r;
                        self.eob_run += self.get_bits(r as u8);
                        // EOB runs are handled by the eob logic
                        break 'no_eob;
                    }
                } else {
                    if symbol != 1 {
                        return Err(DecodeErrors::HuffmanDecode(
                            "Bad Huffman code, corrupt JPEG?".to_string()
                        ));
                    }
                    // get sign bit
                    // We assume we have enough bits, which should be correct for sane images
                    // since we refill by 32 above
                    if self.get_bit() == 1 {
                        symbol = i32::from(bit);
                    } else {
                        symbol = i32::from(-bit);
                    }
                }

                // Advance over already nonzero coefficients  appending
                // correction bits to the non-zeroes.
                // A correction bit is 1 if the absolute value of the coefficient must be increased

                if k <= self.spec_end {
                    'advance_nonzero: loop {
                        let coefficient = &mut block[UN_ZIGZAG[k as usize & 63] & 63];

                        if *coefficient != 0 {
                            if self.get_bit() == 1 && (*coefficient & bit) == 0 {
                                if *coefficient >= 0 {
                                    *coefficient += bit;
                                } else {
                                    *coefficient -= bit;
                                }
                            }

                            if self.bits_left < 1 {
                                self.refill(reader)?;
                            }
                        } else {
                            r -= 1;

                            if r < 0 {
                                // reached target zero coefficient.
                                break 'advance_nonzero;
                            }
                        };

                        if k == self.spec_end {
                            break 'advance_nonzero;
                        }

                        k += 1;
                    }
                }

                if symbol != 0 {
                    let pos = UN_ZIGZAG[k as usize & 63];
                    // output new non-zero coefficient.
                    block[pos & 63] = symbol as i16;
                }

                k += 1;

                if k > self.spec_end {
                    break 'no_eob;
                }
            }
        }
        if self.eob_run > 0 {
            // only run if block does not consists of purely zeroes
            if &block[1..] != &[0; 63] {
                self.refill(reader)?;

                while k <= self.spec_end {
                    let coefficient = &mut block[UN_ZIGZAG[k as usize & 63] & 63];

                    if *coefficient != 0 && self.get_bit() == 1 {
                        // check if we already modified it, if so do nothing, otherwise
                        // append the correction bit.
                        if (*coefficient & bit) == 0 {
                            if *coefficient >= 0 {
                                *coefficient = coefficient.wrapping_add(bit);
                            } else {
                                *coefficient = coefficient.wrapping_sub(bit);
                            }
                        }
                    }
                    if self.bits_left < 1 {
                        // refill at the last possible moment
                        self.refill(reader)?;
                    }
                    k += 1;
                }
            }
            // count a block completed in EOB run
            self.eob_run -= 1;
        }
        return Ok(true);
    }

    pub fn update_progressive_params(&mut self, ah: u8, al: u8, spec_start: u8, spec_end: u8) {
        self.successive_high = ah;
        self.successive_low = al;
        self.spec_start = spec_start;
        self.spec_end = spec_end;
    }

    /// Reset the stream if we have a restart marker
    ///
    /// Restart markers indicate drop those bits in the stream and zero out
    /// everything
    #[cold]
    pub fn reset(&mut self) {
        self.bits_left = 0;
        self.marker = None;
        self.buffer = 0;
        self.aligned_buffer = 0;
        self.eob_run = 0;
    }
}

/// Do the equivalent of JPEG HUFF_EXTEND
#[inline(always)]
fn huff_extend(x: i32, s: i32) -> i32 {
    // if x<s return x else return x+offset[s] where offset[s] = ( (-1<<s)+1)
    (x) + ((((x) - (1 << ((s) - 1))) >> 31) & (((-1) << (s)) + 1))
}

const fn has_zero(v: u32) -> bool {
    // Retrieved from Stanford bithacks
    // @ https://graphics.stanford.edu/~seander/bithacks.html#ZeroInWord
    return !((((v & 0x7F7F_7F7F) + 0x7F7F_7F7F) | v) | 0x7F7F_7F7F) != 0;
}

const fn has_byte(b: u32, val: u8) -> bool {
    // Retrieved from Stanford bithacks
    // @ https://graphics.stanford.edu/~seander/bithacks.html#ZeroInWord
    has_zero(b ^ ((!0_u32 / 255) * (val as u32)))
}

// mod tests {
//     use zune_core::bytestream::ZCursor;
//     use crate::JpegDecoder;
//
//     #[test]
//     fn test_image() {
//         let img = "/Users/etemesi/Downloads/PHO00008.JPG";
//         let data = std::fs::read(img).unwrap();
//         let mut decoder = JpegDecoder::new(ZCursor::new(&data[..]));
//         decoder.decode().unwrap();
//     }
// }