// Copyright Mozilla Foundation. See the COPYRIGHT
// file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! This module provides structs that use lifetimes to couple bounds checking
//! and space availability checking and detaching those from actual slice
//! reading/writing.
//!
//! At present, the internals of the implementation are safe code, so the
//! bound checks currently also happen on read/write. Once this code works,
//! the plan is to replace the internals with unsafe code that omits the
//! bound check at the read/write time.

#[cfg(all(
    feature = "simd-accel",
    any(
        target_feature = "sse2",
        all(target_endian = "little", target_arch = "aarch64"),
        all(target_endian = "little", target_feature = "neon")
    )
))]
use crate::simd_funcs::*;

#[cfg(all(
    feature = "simd-accel",
    any(
        target_feature = "sse2",
        all(target_endian = "little", target_arch = "aarch64"),
        all(target_endian = "little", target_feature = "neon")
    )
))]
use core::simd::u16x8;

use super::DecoderResult;
use super::EncoderResult;
use crate::ascii::*;
use crate::utf_8::convert_utf8_to_utf16_up_to_invalid;
use crate::utf_8::utf8_valid_up_to;

pub enum Space<T> {
    Available(T),
    Full(usize),
}

pub enum CopyAsciiResult<T, U> {
    Stop(T),
    GoOn(U),
}

pub enum NonAscii {
    BmpExclAscii(u16),
    Astral(char),
}

pub enum Unicode {
    Ascii(u8),
    NonAscii(NonAscii),
}

// Start UTF-16LE/BE fast path

pub trait Endian {
    const OPPOSITE_ENDIAN: bool;
}

pub struct BigEndian;

impl Endian for BigEndian {
    #[cfg(target_endian = "little")]
    const OPPOSITE_ENDIAN: bool = true;

    #[cfg(target_endian = "big")]
    const OPPOSITE_ENDIAN: bool = false;
}

pub struct LittleEndian;

impl Endian for LittleEndian {
    #[cfg(target_endian = "little")]
    const OPPOSITE_ENDIAN: bool = false;

    #[cfg(target_endian = "big")]
    const OPPOSITE_ENDIAN: bool = true;
}

#[derive(Debug, Copy, Clone)]
struct UnalignedU16Slice {
    // Safety invariant: ptr must be valid for reading 2*len bytes
    ptr: *const u8,
    len: usize,
}

impl UnalignedU16Slice {
    /// Safety: ptr must be valid for reading 2*len bytes
    #[inline(always)]
    pub unsafe fn new(ptr: *const u8, len: usize) -> UnalignedU16Slice {
        // Safety: field invariant passed up to caller here
        UnalignedU16Slice { ptr, len }
    }

    #[inline(always)]
    pub fn trim_last(&mut self) {
        assert!(self.len > 0);
        // Safety: invariant upheld here: a slice is still valid with a shorter len
        self.len -= 1;
    }

    #[inline(always)]
    pub fn at(&self, i: usize) -> u16 {
        use core::mem::MaybeUninit;

        assert!(i < self.len);
        unsafe {
            let mut u: MaybeUninit<u16> = MaybeUninit::uninit();
            // Safety: i is at most len - 1, which works here
            ::core::ptr::copy_nonoverlapping(self.ptr.add(i * 2), u.as_mut_ptr() as *mut u8, 2);
            // Safety: valid read above lets us do this
            u.assume_init()
        }
    }

    #[cfg(feature = "simd-accel")]
    #[inline(always)]
    pub fn simd_at(&self, i: usize) -> u16x8 {
        // Safety: i/len are on the scale of u16s, each one corresponds to 2 u8s
        assert!(i + SIMD_STRIDE_SIZE / 2 <= self.len);
        let byte_index = i * 2;
        // Safety: load16_unaligned needs SIMD_STRIDE_SIZE=16 u8 elements to read,
        // or 16/2 = 8 u16 elements to read.
        // We have checked that we have at least that many above.

        unsafe { to_u16_lanes(load16_unaligned(self.ptr.add(byte_index))) }
    }

    #[inline(always)]
    pub fn len(&self) -> usize {
        self.len
    }

    #[inline(always)]
    pub fn tail(&self, from: usize) -> UnalignedU16Slice {
        // XXX the return value should be restricted not to
        // outlive self.
        assert!(from <= self.len);
        // Safety: This upholds the same invariant: `from` is in bounds and we're returning a shorter slice
        unsafe { UnalignedU16Slice::new(self.ptr.add(from * 2), self.len - from) }
    }

    #[cfg(feature = "simd-accel")]
    #[inline(always)]
    pub fn copy_bmp_to<E: Endian>(&self, other: &mut [u16]) -> Option<(u16, usize)> {
        assert!(self.len <= other.len());
        let mut offset = 0;
        // Safety: SIMD_STRIDE_SIZE is measured in bytes, whereas len is in u16s. We check we can
        // munch SIMD_STRIDE_SIZE / 2 u16s which means we can write SIMD_STRIDE_SIZE u8s
        if SIMD_STRIDE_SIZE / 2 <= self.len {
            let len_minus_stride = self.len - SIMD_STRIDE_SIZE / 2;
            loop {
                let mut simd = self.simd_at(offset);
                if E::OPPOSITE_ENDIAN {
                    simd = simd_byte_swap(simd);
                }
                // Safety: we have enough space on the other side to write this
                unsafe {
                    store8_unaligned(other.as_mut_ptr().add(offset), simd);
                }
                if contains_surrogates(simd) {
                    break;
                }
                offset += SIMD_STRIDE_SIZE / 2;
                // Safety: This ensures we still have space for writing SIMD_STRIDE_SIZE u8s
                if offset > len_minus_stride {
                    break;
                }
            }
        }
        while offset < self.len {
            let unit = swap_if_opposite_endian::<E>(self.at(offset));
            other[offset] = unit;
            if super::in_range16(unit, 0xD800, 0xE000) {
                return Some((unit, offset));
            }
            offset += 1;
        }
        None
    }

    #[cfg(not(feature = "simd-accel"))]
    #[inline(always)]
    fn copy_bmp_to<E: Endian>(&self, other: &mut [u16]) -> Option<(u16, usize)> {
        assert!(self.len <= other.len());
        for (i, target) in other.iter_mut().enumerate().take(self.len) {
            let unit = swap_if_opposite_endian::<E>(self.at(i));
            *target = unit;
            if super::in_range16(unit, 0xD800, 0xE000) {
                return Some((unit, i));
            }
        }
        None
    }
}

#[inline(always)]
fn copy_unaligned_basic_latin_to_ascii_alu<E: Endian>(
    src: UnalignedU16Slice,
    dst: &mut [u8],
    offset: usize,
) -> CopyAsciiResult<usize, (u16, usize)> {
    let len = ::core::cmp::min(src.len(), dst.len());
    let mut i = 0usize;
    loop {
        if i == len {
            return CopyAsciiResult::Stop(i + offset);
        }
        let unit = swap_if_opposite_endian::<E>(src.at(i));
        if unit > 0x7F {
            return CopyAsciiResult::GoOn((unit, i + offset));
        }
        dst[i] = unit as u8;
        i += 1;
    }
}

#[inline(always)]
fn swap_if_opposite_endian<E: Endian>(unit: u16) -> u16 {
    if E::OPPOSITE_ENDIAN {
        unit.swap_bytes()
    } else {
        unit
    }
}

#[cfg(not(feature = "simd-accel"))]
#[inline(always)]
fn copy_unaligned_basic_latin_to_ascii<E: Endian>(
    src: UnalignedU16Slice,
    dst: &mut [u8],
) -> CopyAsciiResult<usize, (u16, usize)> {
    copy_unaligned_basic_latin_to_ascii_alu::<E>(src, dst, 0)
}

#[cfg(feature = "simd-accel")]
#[inline(always)]
fn copy_unaligned_basic_latin_to_ascii<E: Endian>(
    src: UnalignedU16Slice,
    dst: &mut [u8],
) -> CopyAsciiResult<usize, (u16, usize)> {
    let len = ::core::cmp::min(src.len(), dst.len());
    let mut offset = 0;
    // Safety: This check ensures we are able to read/write at least SIMD_STRIDE_SIZE elements
    if SIMD_STRIDE_SIZE <= len {
        let len_minus_stride = len - SIMD_STRIDE_SIZE;
        loop {
            let mut first = src.simd_at(offset);
            let mut second = src.simd_at(offset + (SIMD_STRIDE_SIZE / 2));
            if E::OPPOSITE_ENDIAN {
                first = simd_byte_swap(first);
                second = simd_byte_swap(second);
            }
            if !simd_is_basic_latin(first | second) {
                break;
            }
            let packed = simd_pack(first, second);
            // Safety: We are able to write SIMD_STRIDE_SIZE elements in this iteration
            unsafe {
                store16_unaligned(dst.as_mut_ptr().add(offset), packed);
            }
            offset += SIMD_STRIDE_SIZE;
            // Safety: This is `offset > len - SIMD_STRIDE_SIZE`, which ensures that we can write at least SIMD_STRIDE_SIZE elements
            // in the next iteration
            if offset > len_minus_stride {
                break;
            }
        }
    }
    copy_unaligned_basic_latin_to_ascii_alu::<E>(src.tail(offset), &mut dst[offset..], offset)
}

#[inline(always)]
fn convert_unaligned_utf16_to_utf8<E: Endian>(
    src: UnalignedU16Slice,
    dst: &mut [u8],
) -> (usize, usize, bool) {
    if dst.len() < 4 {
        return (0, 0, false);
    }
    let mut src_pos = 0usize;
    let mut dst_pos = 0usize;
    let src_len = src.len();
    let dst_len_minus_three = dst.len() - 3;
    'outer: loop {
        let mut non_ascii = match copy_unaligned_basic_latin_to_ascii::<E>(
            src.tail(src_pos),
            &mut dst[dst_pos..],
        ) {
            CopyAsciiResult::GoOn((unit, read_written)) => {
                src_pos += read_written;
                dst_pos += read_written;
                unit
            }
            CopyAsciiResult::Stop(read_written) => {
                return (src_pos + read_written, dst_pos + read_written, false);
            }
        };
        if dst_pos >= dst_len_minus_three {
            break 'outer;
        }
        // We have enough destination space to commit to
        // having read `non_ascii`.
        src_pos += 1;
        'inner: loop {
            let non_ascii_minus_surrogate_start = non_ascii.wrapping_sub(0xD800);
            if non_ascii_minus_surrogate_start > (0xDFFF - 0xD800) {
                if non_ascii < 0x800 {
                    dst[dst_pos] = ((non_ascii >> 6) | 0xC0) as u8;
                    dst_pos += 1;
                    dst[dst_pos] = ((non_ascii & 0x3F) | 0x80) as u8;
                    dst_pos += 1;
                } else {
                    dst[dst_pos] = ((non_ascii >> 12) | 0xE0) as u8;
                    dst_pos += 1;
                    dst[dst_pos] = (((non_ascii & 0xFC0) >> 6) | 0x80) as u8;
                    dst_pos += 1;
                    dst[dst_pos] = ((non_ascii & 0x3F) | 0x80) as u8;
                    dst_pos += 1;
                }
            } else if non_ascii_minus_surrogate_start <= (0xDBFF - 0xD800) {
                // high surrogate
                if src_pos < src_len {
                    let second = swap_if_opposite_endian::<E>(src.at(src_pos));
                    let second_minus_low_surrogate_start = second.wrapping_sub(0xDC00);
                    if second_minus_low_surrogate_start <= (0xDFFF - 0xDC00) {
                        // The next code unit is a low surrogate. Advance position.
                        src_pos += 1;
                        let point = (u32::from(non_ascii) << 10) + u32::from(second)
                            - (((0xD800u32 << 10) - 0x10000u32) + 0xDC00u32);

                        dst[dst_pos] = ((point >> 18) | 0xF0u32) as u8;
                        dst_pos += 1;
                        dst[dst_pos] = (((point & 0x3F000u32) >> 12) | 0x80u32) as u8;
                        dst_pos += 1;
                        dst[dst_pos] = (((point & 0xFC0u32) >> 6) | 0x80u32) as u8;
                        dst_pos += 1;
                        dst[dst_pos] = ((point & 0x3Fu32) | 0x80u32) as u8;
                        dst_pos += 1;
                    } else {
                        // The next code unit is not a low surrogate. Don't advance
                        // position and treat the high surrogate as unpaired.
                        return (src_pos, dst_pos, true);
                    }
                } else {
                    // Unpaired surrogate at the end of buffer
                    return (src_pos, dst_pos, true);
                }
            } else {
                // Unpaired low surrogate
                return (src_pos, dst_pos, true);
            }
            if dst_pos >= dst_len_minus_three || src_pos == src_len {
                break 'outer;
            }
            let unit = swap_if_opposite_endian::<E>(src.at(src_pos));
            src_pos += 1;
            if unit > 0x7F {
                non_ascii = unit;
                continue 'inner;
            }
            dst[dst_pos] = unit as u8;
            dst_pos += 1;
            continue 'outer;
        }
    }
    (src_pos, dst_pos, false)
}

// Byte source

pub struct ByteSource<'a> {
    slice: &'a [u8],
    pos: usize,
}

impl<'a> ByteSource<'a> {
    #[inline(always)]
    pub fn new(src: &[u8]) -> ByteSource {
        ByteSource { slice: src, pos: 0 }
    }
    #[inline(always)]
    pub fn check_available<'b>(&'b mut self) -> Space<ByteReadHandle<'b, 'a>> {
        if self.pos < self.slice.len() {
            Space::Available(ByteReadHandle::new(self))
        } else {
            Space::Full(self.consumed())
        }
    }
    #[inline(always)]
    fn read(&mut self) -> u8 {
        let ret = self.slice[self.pos];
        self.pos += 1;
        ret
    }
    #[inline(always)]
    fn unread(&mut self) -> usize {
        self.pos -= 1;
        self.pos
    }
    #[inline(always)]
    pub fn consumed(&self) -> usize {
        self.pos
    }
}

pub struct ByteReadHandle<'a, 'b>
where
    'b: 'a,
{
    source: &'a mut ByteSource<'b>,
}

impl<'a, 'b> ByteReadHandle<'a, 'b>
where
    'b: 'a,
{
    #[inline(always)]
    fn new(src: &'a mut ByteSource<'b>) -> ByteReadHandle<'a, 'b> {
        ByteReadHandle { source: src }
    }
    #[inline(always)]
    pub fn read(self) -> (u8, ByteUnreadHandle<'a, 'b>) {
        let byte = self.source.read();
        let handle = ByteUnreadHandle::new(self.source);
        (byte, handle)
    }
    #[inline(always)]
    pub fn consumed(&self) -> usize {
        self.source.consumed()
    }
}

pub struct ByteUnreadHandle<'a, 'b>
where
    'b: 'a,
{
    source: &'a mut ByteSource<'b>,
}

impl<'a, 'b> ByteUnreadHandle<'a, 'b>
where
    'b: 'a,
{
    #[inline(always)]
    fn new(src: &'a mut ByteSource<'b>) -> ByteUnreadHandle<'a, 'b> {
        ByteUnreadHandle { source: src }
    }
    #[inline(always)]
    pub fn unread(self) -> usize {
        self.source.unread()
    }
    #[inline(always)]
    pub fn consumed(&self) -> usize {
        self.source.consumed()
    }
    #[inline(always)]
    pub fn commit(self) -> &'a mut ByteSource<'b> {
        self.source
    }
}

// UTF-16 destination

pub struct Utf16BmpHandle<'a, 'b>
where
    'b: 'a,
{
    dest: &'a mut Utf16Destination<'b>,
}

impl<'a, 'b> Utf16BmpHandle<'a, 'b>
where
    'b: 'a,
{
    #[inline(always)]
    fn new(dst: &'a mut Utf16Destination<'b>) -> Utf16BmpHandle<'a, 'b> {
        Utf16BmpHandle { dest: dst }
    }
    #[inline(always)]
    pub fn written(&self) -> usize {
        self.dest.written()
    }
    #[inline(always)]
    pub fn write_ascii(self, ascii: u8) -> &'a mut Utf16Destination<'b> {
        self.dest.write_ascii(ascii);
        self.dest
    }
    #[inline(always)]
    pub fn write_bmp(self, bmp: u16) -> &'a mut Utf16Destination<'b> {
        self.dest.write_bmp(bmp);
        self.dest
    }
    #[inline(always)]
    pub fn write_bmp_excl_ascii(self, bmp: u16) -> &'a mut Utf16Destination<'b> {
        self.dest.write_bmp_excl_ascii(bmp);
        self.dest
    }
    #[inline(always)]
    pub fn write_mid_bmp(self, bmp: u16) -> &'a mut Utf16Destination<'b> {
        self.dest.write_mid_bmp(bmp);
        self.dest
    }
    #[inline(always)]
    pub fn write_upper_bmp(self, bmp: u16) -> &'a mut Utf16Destination<'b> {
        self.dest.write_upper_bmp(bmp);
        self.dest
    }
    #[inline(always)]
    pub fn commit(self) -> &'a mut Utf16Destination<'b> {
        self.dest
    }
}

pub struct Utf16AstralHandle<'a, 'b>
where
    'b: 'a,
{
    dest: &'a mut Utf16Destination<'b>,
}

impl<'a, 'b> Utf16AstralHandle<'a, 'b>
where
    'b: 'a,
{
    #[inline(always)]
    fn new(dst: &'a mut Utf16Destination<'b>) -> Utf16AstralHandle<'a, 'b> {
        Utf16AstralHandle { dest: dst }
    }
    #[inline(always)]
    pub fn written(&self) -> usize {
        self.dest.written()
    }
    #[inline(always)]
    pub fn write_ascii(self, ascii: u8) -> &'a mut Utf16Destination<'b> {
        self.dest.write_ascii(ascii);
        self.dest
    }
    #[inline(always)]
    pub fn write_bmp(self, bmp: u16) -> &'a mut Utf16Destination<'b> {
        self.dest.write_bmp(bmp);
        self.dest
    }
    #[inline(always)]
    pub fn write_bmp_excl_ascii(self, bmp: u16) -> &'a mut Utf16Destination<'b> {
        self.dest.write_bmp_excl_ascii(bmp);
        self.dest
    }
    #[inline(always)]
    pub fn write_upper_bmp(self, bmp: u16) -> &'a mut Utf16Destination<'b> {
        self.dest.write_upper_bmp(bmp);
        self.dest
    }
    #[inline(always)]
    pub fn write_astral(self, astral: u32) -> &'a mut Utf16Destination<'b> {
        self.dest.write_astral(astral);
        self.dest
    }
    #[inline(always)]
    pub fn write_surrogate_pair(self, high: u16, low: u16) -> &'a mut Utf16Destination<'b> {
        self.dest.write_surrogate_pair(high, low);
        self.dest
    }
    #[inline(always)]
    pub fn write_big5_combination(
        self,
        combined: u16,
        combining: u16,
    ) -> &'a mut Utf16Destination<'b> {
        self.dest.write_big5_combination(combined, combining);
        self.dest
    }
    #[inline(always)]
    pub fn commit(self) -> &'a mut Utf16Destination<'b> {
        self.dest
    }
}

pub struct Utf16Destination<'a> {
    slice: &'a mut [u16],
    pos: usize,
}

impl<'a> Utf16Destination<'a> {
    #[inline(always)]
    pub fn new(dst: &mut [u16]) -> Utf16Destination {
        Utf16Destination { slice: dst, pos: 0 }
    }
    #[inline(always)]
    pub fn check_space_bmp<'b>(&'b mut self) -> Space<Utf16BmpHandle<'b, 'a>> {
        if self.pos < self.slice.len() {
            Space::Available(Utf16BmpHandle::new(self))
        } else {
            Space::Full(self.written())
        }
    }
    #[inline(always)]
    pub fn check_space_astral<'b>(&'b mut self) -> Space<Utf16AstralHandle<'b, 'a>> {
        if self.pos + 1 < self.slice.len() {
            Space::Available(Utf16AstralHandle::new(self))
        } else {
            Space::Full(self.written())
        }
    }
    #[inline(always)]
    pub fn written(&self) -> usize {
        self.pos
    }
    #[inline(always)]
    fn write_code_unit(&mut self, u: u16) {
        unsafe {
            // OK, because we checked before handing out a handle.
            *(self.slice.get_unchecked_mut(self.pos)) = u;
        }
        self.pos += 1;
    }
    #[inline(always)]
    fn write_ascii(&mut self, ascii: u8) {
        debug_assert!(ascii < 0x80);
        self.write_code_unit(u16::from(ascii));
    }
    #[inline(always)]
    fn write_bmp(&mut self, bmp: u16) {
        self.write_code_unit(bmp);
    }
    #[inline(always)]
    fn write_bmp_excl_ascii(&mut self, bmp: u16) {
        debug_assert!(bmp >= 0x80);
        self.write_code_unit(bmp);
    }
    #[inline(always)]
    fn write_mid_bmp(&mut self, bmp: u16) {
        debug_assert!(bmp >= 0x80); // XXX
        self.write_code_unit(bmp);
    }
    #[inline(always)]
    fn write_upper_bmp(&mut self, bmp: u16) {
        debug_assert!(bmp >= 0x80);
        self.write_code_unit(bmp);
    }
    #[inline(always)]
    fn write_astral(&mut self, astral: u32) {
        debug_assert!(astral > 0xFFFF);
        debug_assert!(astral <= 0x10_FFFF);
        self.write_code_unit((0xD7C0 + (astral >> 10)) as u16);
        self.write_code_unit((0xDC00 + (astral & 0x3FF)) as u16);
    }
    #[inline(always)]
    fn write_surrogate_pair(&mut self, high: u16, low: u16) {
        self.write_code_unit(high);
        self.write_code_unit(low);
    }
    #[inline(always)]
    fn write_big5_combination(&mut self, combined: u16, combining: u16) {
        self.write_bmp_excl_ascii(combined);
        self.write_bmp_excl_ascii(combining);
    }
    // Safety-usable invariant: CopyAsciiResult::GoOn will only contain bytes >=0x80
    #[inline(always)]
    pub fn copy_ascii_from_check_space_bmp<'b>(
        &'b mut self,
        source: &mut ByteSource,
    ) -> CopyAsciiResult<(DecoderResult, usize, usize), (u8, Utf16BmpHandle<'b, 'a>)> {
        let non_ascii_ret = {
            let src_remaining = &source.slice[source.pos..];
            let dst_remaining = &mut self.slice[self.pos..];
            let (pending, length) = if dst_remaining.len() < src_remaining.len() {
                (DecoderResult::OutputFull, dst_remaining.len())
            } else {
                (DecoderResult::InputEmpty, src_remaining.len())
            };
            // Safety: This function is documented as needing valid pointers for src/dest and len, which
            // is true since we've passed the minumum length of the two
            match unsafe {
                ascii_to_basic_latin(src_remaining.as_ptr(), dst_remaining.as_mut_ptr(), length)
            } {
                None => {
                    source.pos += length;
                    self.pos += length;
                    return CopyAsciiResult::Stop((pending, source.pos, self.pos));
                }
                // Safety: the function is documented as returning bytes >=0x80 in the Some
                Some((non_ascii, consumed)) => {
                    source.pos += consumed;
                    self.pos += consumed;
                    source.pos += 1; // +1 for non_ascii
                                     // Safety: non-ascii bubbled out here
                    non_ascii
                }
            }
        };
        // Safety: non-ascii returned here
        CopyAsciiResult::GoOn((non_ascii_ret, Utf16BmpHandle::new(self)))
    }
    // Safety-usable invariant: CopyAsciiResult::GoOn will only contain bytes >=0x80
    #[inline(always)]
    pub fn copy_ascii_from_check_space_astral<'b>(
        &'b mut self,
        source: &mut ByteSource,
    ) -> CopyAsciiResult<(DecoderResult, usize, usize), (u8, Utf16AstralHandle<'b, 'a>)> {
        let non_ascii_ret = {
            let dst_len = self.slice.len();
            let src_remaining = &source.slice[source.pos..];
            let dst_remaining = &mut self.slice[self.pos..];
            let (pending, length) = if dst_remaining.len() < src_remaining.len() {
                (DecoderResult::OutputFull, dst_remaining.len())
            } else {
                (DecoderResult::InputEmpty, src_remaining.len())
            };
            // Safety: This function is documented as needing valid pointers for src/dest and len, which
            // is true since we've passed the minumum length of the two
            match unsafe {
                ascii_to_basic_latin(src_remaining.as_ptr(), dst_remaining.as_mut_ptr(), length)
            } {
                None => {
                    source.pos += length;
                    self.pos += length;
                    return CopyAsciiResult::Stop((pending, source.pos, self.pos));
                }
                // Safety: the function is documented as returning bytes >=0x80 in the Some
                Some((non_ascii, consumed)) => {
                    source.pos += consumed;
                    self.pos += consumed;
                    if self.pos + 1 < dst_len {
                        source.pos += 1; // +1 for non_ascii
                                         // Safety: non-ascii bubbled out here
                        non_ascii
                    } else {
                        return CopyAsciiResult::Stop((
                            DecoderResult::OutputFull,
                            source.pos,
                            self.pos,
                        ));
                    }
                }
            }
        };
        // Safety: non-ascii returned here
        CopyAsciiResult::GoOn((non_ascii_ret, Utf16AstralHandle::new(self)))
    }
    #[inline(always)]
    pub fn copy_utf8_up_to_invalid_from(&mut self, source: &mut ByteSource) {
        let src_remaining = &source.slice[source.pos..];
        let dst_remaining = &mut self.slice[self.pos..];
        let (read, written) = convert_utf8_to_utf16_up_to_invalid(src_remaining, dst_remaining);
        source.pos += read;
        self.pos += written;
    }
    #[inline(always)]
    pub fn copy_utf16_from<E: Endian>(
        &mut self,
        source: &mut ByteSource,
    ) -> Option<(usize, usize)> {
        let src_remaining = &source.slice[source.pos..];
        let dst_remaining = &mut self.slice[self.pos..];

        let mut src_unaligned = unsafe {
            UnalignedU16Slice::new(
                src_remaining.as_ptr(),
                ::core::cmp::min(src_remaining.len() / 2, dst_remaining.len()),
            )
        };
        if src_unaligned.len() == 0 {
            return None;
        }
        let last_unit = swap_if_opposite_endian::<E>(src_unaligned.at(src_unaligned.len() - 1));
        if super::in_range16(last_unit, 0xD800, 0xDC00) {
            // Last code unit is a high surrogate. It might
            // legitimately form a pair later, so let's not
            // include it.
            src_unaligned.trim_last();
        }
        let mut offset = 0usize;
        loop {
            if let Some((surrogate, bmp_len)) = {
                let src_left = src_unaligned.tail(offset);
                let dst_left = &mut dst_remaining[offset..src_unaligned.len()];
                src_left.copy_bmp_to::<E>(dst_left)
            } {
                offset += bmp_len; // surrogate has not been consumed yet
                let second_pos = offset + 1;
                if surrogate > 0xDBFF || second_pos == src_unaligned.len() {
                    // Unpaired surrogate
                    source.pos += second_pos * 2;
                    self.pos += offset;
                    return Some((source.pos, self.pos));
                }
                let second = swap_if_opposite_endian::<E>(src_unaligned.at(second_pos));
                if !super::in_range16(second, 0xDC00, 0xE000) {
                    // Unpaired surrogate
                    source.pos += second_pos * 2;
                    self.pos += offset;
                    return Some((source.pos, self.pos));
                }
                // `surrogate` was already speculatively written
                dst_remaining[second_pos] = second;
                offset += 2;
                continue;
            } else {
                source.pos += src_unaligned.len() * 2;
                self.pos += src_unaligned.len();
                return None;
            }
        }
    }
}

// UTF-8 destination

pub struct Utf8BmpHandle<'a, 'b>
where
    'b: 'a,
{
    dest: &'a mut Utf8Destination<'b>,
}

impl<'a, 'b> Utf8BmpHandle<'a, 'b>
where
    'b: 'a,
{
    #[inline(always)]
    fn new(dst: &'a mut Utf8Destination<'b>) -> Utf8BmpHandle<'a, 'b> {
        Utf8BmpHandle { dest: dst }
    }
    #[inline(always)]
    pub fn written(&self) -> usize {
        self.dest.written()
    }
    #[inline(always)]
    pub fn write_ascii(self, ascii: u8) -> &'a mut Utf8Destination<'b> {
        self.dest.write_ascii(ascii);
        self.dest
    }
    #[inline(always)]
    pub fn write_bmp(self, bmp: u16) -> &'a mut Utf8Destination<'b> {
        self.dest.write_bmp(bmp);
        self.dest
    }
    #[inline(always)]
    pub fn write_bmp_excl_ascii(self, bmp: u16) -> &'a mut Utf8Destination<'b> {
        self.dest.write_bmp_excl_ascii(bmp);
        self.dest
    }
    #[inline(always)]
    pub fn write_mid_bmp(self, bmp: u16) -> &'a mut Utf8Destination<'b> {
        self.dest.write_mid_bmp(bmp);
        self.dest
    }
    #[inline(always)]
    pub fn write_upper_bmp(self, bmp: u16) -> &'a mut Utf8Destination<'b> {
        self.dest.write_upper_bmp(bmp);
        self.dest
    }
    #[inline(always)]
    pub fn commit(self) -> &'a mut Utf8Destination<'b> {
        self.dest
    }
}

pub struct Utf8AstralHandle<'a, 'b>
where
    'b: 'a,
{
    dest: &'a mut Utf8Destination<'b>,
}

impl<'a, 'b> Utf8AstralHandle<'a, 'b>
where
    'b: 'a,
{
    #[inline(always)]
    fn new(dst: &'a mut Utf8Destination<'b>) -> Utf8AstralHandle<'a, 'b> {
        Utf8AstralHandle { dest: dst }
    }
    #[inline(always)]
    pub fn written(&self) -> usize {
        self.dest.written()
    }
    #[inline(always)]
    pub fn write_ascii(self, ascii: u8) -> &'a mut Utf8Destination<'b> {
        self.dest.write_ascii(ascii);
        self.dest
    }
    #[inline(always)]
    pub fn write_bmp(self, bmp: u16) -> &'a mut Utf8Destination<'b> {
        self.dest.write_bmp(bmp);
        self.dest
    }
    #[inline(always)]
    pub fn write_bmp_excl_ascii(self, bmp: u16) -> &'a mut Utf8Destination<'b> {
        self.dest.write_bmp_excl_ascii(bmp);
        self.dest
    }
    #[inline(always)]
    pub fn write_upper_bmp(self, bmp: u16) -> &'a mut Utf8Destination<'b> {
        self.dest.write_upper_bmp(bmp);
        self.dest
    }
    #[inline(always)]
    pub fn write_astral(self, astral: u32) -> &'a mut Utf8Destination<'b> {
        self.dest.write_astral(astral);
        self.dest
    }
    #[inline(always)]
    pub fn write_surrogate_pair(self, high: u16, low: u16) -> &'a mut Utf8Destination<'b> {
        self.dest.write_surrogate_pair(high, low);
        self.dest
    }
    #[inline(always)]
    pub fn write_big5_combination(
        self,
        combined: u16,
        combining: u16,
    ) -> &'a mut Utf8Destination<'b> {
        self.dest.write_big5_combination(combined, combining);
        self.dest
    }
    #[inline(always)]
    pub fn commit(self) -> &'a mut Utf8Destination<'b> {
        self.dest
    }
}

pub struct Utf8Destination<'a> {
    slice: &'a mut [u8],
    pos: usize,
}

impl<'a> Utf8Destination<'a> {
    #[inline(always)]
    pub fn new(dst: &mut [u8]) -> Utf8Destination {
        Utf8Destination { slice: dst, pos: 0 }
    }
    #[inline(always)]
    pub fn check_space_bmp<'b>(&'b mut self) -> Space<Utf8BmpHandle<'b, 'a>> {
        if self.pos + 2 < self.slice.len() {
            Space::Available(Utf8BmpHandle::new(self))
        } else {
            Space::Full(self.written())
        }
    }
    #[inline(always)]
    pub fn check_space_astral<'b>(&'b mut self) -> Space<Utf8AstralHandle<'b, 'a>> {
        if self.pos + 3 < self.slice.len() {
            Space::Available(Utf8AstralHandle::new(self))
        } else {
            Space::Full(self.written())
        }
    }
    #[inline(always)]
    pub fn written(&self) -> usize {
        self.pos
    }
    #[inline(always)]
    fn write_code_unit(&mut self, u: u8) {
        unsafe {
            // OK, because we checked before handing out a handle.
            *(self.slice.get_unchecked_mut(self.pos)) = u;
        }
        self.pos += 1;
    }
    #[inline(always)]
    fn write_ascii(&mut self, ascii: u8) {
        debug_assert!(ascii < 0x80);
        self.write_code_unit(ascii);
    }
    #[inline(always)]
    fn write_bmp(&mut self, bmp: u16) {
        if bmp < 0x80u16 {
            self.write_ascii(bmp as u8);
        } else if bmp < 0x800u16 {
            self.write_mid_bmp(bmp);
        } else {
            self.write_upper_bmp(bmp);
        }
    }
    #[inline(always)]
    fn write_mid_bmp(&mut self, mid_bmp: u16) {
        debug_assert!(mid_bmp >= 0x80);
        debug_assert!(mid_bmp < 0x800);
        self.write_code_unit(((mid_bmp >> 6) | 0xC0) as u8);
        self.write_code_unit(((mid_bmp & 0x3F) | 0x80) as u8);
    }
    #[inline(always)]
    fn write_upper_bmp(&mut self, upper_bmp: u16) {
        debug_assert!(upper_bmp >= 0x800);
        self.write_code_unit(((upper_bmp >> 12) | 0xE0) as u8);
        self.write_code_unit((((upper_bmp & 0xFC0) >> 6) | 0x80) as u8);
        self.write_code_unit(((upper_bmp & 0x3F) | 0x80) as u8);
    }
    #[inline(always)]
    fn write_bmp_excl_ascii(&mut self, bmp: u16) {
        if bmp < 0x800u16 {
            self.write_mid_bmp(bmp);
        } else {
            self.write_upper_bmp(bmp);
        }
    }
    #[inline(always)]
    fn write_astral(&mut self, astral: u32) {
        debug_assert!(astral > 0xFFFF);
        debug_assert!(astral <= 0x10_FFFF);
        self.write_code_unit(((astral >> 18) | 0xF0) as u8);
        self.write_code_unit((((astral & 0x3F000) >> 12) | 0x80) as u8);
        self.write_code_unit((((astral & 0xFC0) >> 6) | 0x80) as u8);
        self.write_code_unit(((astral & 0x3F) | 0x80) as u8);
    }
    #[inline(always)]
    pub fn write_surrogate_pair(&mut self, high: u16, low: u16) {
        self.write_astral(
            (u32::from(high) << 10) + u32::from(low)
                - (((0xD800u32 << 10) - 0x10000u32) + 0xDC00u32),
        );
    }
    #[inline(always)]
    fn write_big5_combination(&mut self, combined: u16, combining: u16) {
        self.write_mid_bmp(combined);
        self.write_mid_bmp(combining);
    }
    #[inline(always)]
    pub fn copy_ascii_from_check_space_bmp<'b>(
        &'b mut self,
        source: &mut ByteSource,
    ) -> CopyAsciiResult<(DecoderResult, usize, usize), (u8, Utf8BmpHandle<'b, 'a>)> {
        let non_ascii_ret = {
            let dst_len = self.slice.len();
            let src_remaining = &source.slice[source.pos..];
            let dst_remaining = &mut self.slice[self.pos..];
            let (pending, length) = if dst_remaining.len() < src_remaining.len() {
                (DecoderResult::OutputFull, dst_remaining.len())
            } else {
                (DecoderResult::InputEmpty, src_remaining.len())
            };
            match unsafe {
                ascii_to_ascii(src_remaining.as_ptr(), dst_remaining.as_mut_ptr(), length)
            } {
                None => {
                    source.pos += length;
                    self.pos += length;
                    return CopyAsciiResult::Stop((pending, source.pos, self.pos));
                }
                Some((non_ascii, consumed)) => {
                    source.pos += consumed;
                    self.pos += consumed;
                    if self.pos + 2 < dst_len {
                        source.pos += 1; // +1 for non_ascii
                        non_ascii
                    } else {
                        return CopyAsciiResult::Stop((
                            DecoderResult::OutputFull,
                            source.pos,
                            self.pos,
                        ));
                    }
                }
            }
        };
        CopyAsciiResult::GoOn((non_ascii_ret, Utf8BmpHandle::new(self)))
    }
    #[inline(always)]
    pub fn copy_ascii_from_check_space_astral<'b>(
        &'b mut self,
        source: &mut ByteSource,
    ) -> CopyAsciiResult<(DecoderResult, usize, usize), (u8, Utf8AstralHandle<'b, 'a>)> {
        let non_ascii_ret = {
            let dst_len = self.slice.len();
            let src_remaining = &source.slice[source.pos..];
            let dst_remaining = &mut self.slice[self.pos..];
            let (pending, length) = if dst_remaining.len() < src_remaining.len() {
                (DecoderResult::OutputFull, dst_remaining.len())
            } else {
                (DecoderResult::InputEmpty, src_remaining.len())
            };
            match unsafe {
                ascii_to_ascii(src_remaining.as_ptr(), dst_remaining.as_mut_ptr(), length)
            } {
                None => {
                    source.pos += length;
                    self.pos += length;
                    return CopyAsciiResult::Stop((pending, source.pos, self.pos));
                }
                Some((non_ascii, consumed)) => {
                    source.pos += consumed;
                    self.pos += consumed;
                    if self.pos + 3 < dst_len {
                        source.pos += 1; // +1 for non_ascii
                        non_ascii
                    } else {
                        return CopyAsciiResult::Stop((
                            DecoderResult::OutputFull,
                            source.pos,
                            self.pos,
                        ));
                    }
                }
            }
        };
        CopyAsciiResult::GoOn((non_ascii_ret, Utf8AstralHandle::new(self)))
    }
    #[inline(always)]
    pub fn copy_utf8_up_to_invalid_from(&mut self, source: &mut ByteSource) {
        let src_remaining = &source.slice[source.pos..];
        let dst_remaining = &mut self.slice[self.pos..];
        let min_len = ::core::cmp::min(src_remaining.len(), dst_remaining.len());
        // Validate first, then memcpy to let memcpy do its thing even for
        // non-ASCII. (And potentially do something better than SSE2 for ASCII.)
        let valid_len = utf8_valid_up_to(&src_remaining[..min_len]);
        (&mut dst_remaining[..valid_len]).copy_from_slice(&src_remaining[..valid_len]);
        source.pos += valid_len;
        self.pos += valid_len;
    }
    #[inline(always)]
    pub fn copy_utf16_from<E: Endian>(
        &mut self,
        source: &mut ByteSource,
    ) -> Option<(usize, usize)> {
        let src_remaining = &source.slice[source.pos..];
        let dst_remaining = &mut self.slice[self.pos..];

        let mut src_unaligned =
            unsafe { UnalignedU16Slice::new(src_remaining.as_ptr(), src_remaining.len() / 2) };
        if src_unaligned.len() == 0 {
            return None;
        }
        let mut last_unit = src_unaligned.at(src_unaligned.len() - 1);
        if E::OPPOSITE_ENDIAN {
            last_unit = last_unit.swap_bytes();
        }
        if super::in_range16(last_unit, 0xD800, 0xDC00) {
            // Last code unit is a high surrogate. It might
            // legitimately form a pair later, so let's not
            // include it.
            src_unaligned.trim_last();
        }
        let (read, written, had_error) =
            convert_unaligned_utf16_to_utf8::<E>(src_unaligned, dst_remaining);
        source.pos += read * 2;
        self.pos += written;
        if had_error {
            Some((source.pos, self.pos))
        } else {
            None
        }
    }
}

// UTF-16 source

pub struct Utf16Source<'a> {
    slice: &'a [u16],
    pos: usize,
    old_pos: usize,
}

impl<'a> Utf16Source<'a> {
    #[inline(always)]
    pub fn new(src: &[u16]) -> Utf16Source {
        Utf16Source {
            slice: src,
            pos: 0,
            old_pos: 0,
        }
    }
    #[inline(always)]
    pub fn check_available<'b>(&'b mut self) -> Space<Utf16ReadHandle<'b, 'a>> {
        if self.pos < self.slice.len() {
            Space::Available(Utf16ReadHandle::new(self))
        } else {
            Space::Full(self.consumed())
        }
    }
    #[cfg_attr(feature = "cargo-clippy", allow(collapsible_if))]
    #[inline(always)]
    fn read(&mut self) -> char {
        self.old_pos = self.pos;
        let unit = self.slice[self.pos];
        self.pos += 1;
        let unit_minus_surrogate_start = unit.wrapping_sub(0xD800);
        if unit_minus_surrogate_start > (0xDFFF - 0xD800) {
            return unsafe { ::core::char::from_u32_unchecked(u32::from(unit)) };
        }
        if unit_minus_surrogate_start <= (0xDBFF - 0xD800) {
            // high surrogate
            if self.pos < self.slice.len() {
                let second = self.slice[self.pos];
                let second_minus_low_surrogate_start = second.wrapping_sub(0xDC00);
                if second_minus_low_surrogate_start <= (0xDFFF - 0xDC00) {
                    // The next code unit is a low surrogate. Advance position.
                    self.pos += 1;
                    return unsafe {
                        ::core::char::from_u32_unchecked(
                            (u32::from(unit) << 10) + u32::from(second)
                                - (((0xD800u32 << 10) - 0x10000u32) + 0xDC00u32),
                        )
                    };
                }
                // The next code unit is not a low surrogate. Don't advance
                // position and treat the high surrogate as unpaired.
                // fall through
            }
            // Unpaired surrogate at the end of buffer, fall through
        }
        // Unpaired low surrogate
        '\u{FFFD}'
    }
    #[cfg_attr(feature = "cargo-clippy", allow(collapsible_if))]
    #[inline(always)]
    fn read_enum(&mut self) -> Unicode {
        self.old_pos = self.pos;
        let unit = self.slice[self.pos];
        self.pos += 1;
        if unit < 0x80 {
            return Unicode::Ascii(unit as u8);
        }
        let unit_minus_surrogate_start = unit.wrapping_sub(0xD800);
        if unit_minus_surrogate_start > (0xDFFF - 0xD800) {
            return Unicode::NonAscii(NonAscii::BmpExclAscii(unit));
        }
        if unit_minus_surrogate_start <= (0xDBFF - 0xD800) {
            // high surrogate
            if self.pos < self.slice.len() {
                let second = self.slice[self.pos];
                let second_minus_low_surrogate_start = second.wrapping_sub(0xDC00);
                if second_minus_low_surrogate_start <= (0xDFFF - 0xDC00) {
                    // The next code unit is a low surrogate. Advance position.
                    self.pos += 1;
                    return Unicode::NonAscii(NonAscii::Astral(unsafe {
                        ::core::char::from_u32_unchecked(
                            (u32::from(unit) << 10) + u32::from(second)
                                - (((0xD800u32 << 10) - 0x10000u32) + 0xDC00u32),
                        )
                    }));
                }
                // The next code unit is not a low surrogate. Don't advance
                // position and treat the high surrogate as unpaired.
                // fall through
            }
            // Unpaired surrogate at the end of buffer, fall through
        }
        // Unpaired low surrogate
        Unicode::NonAscii(NonAscii::BmpExclAscii(0xFFFDu16))
    }
    #[inline(always)]
    fn unread(&mut self) -> usize {
        self.pos = self.old_pos;
        self.pos
    }
    #[inline(always)]
    pub fn consumed(&self) -> usize {
        self.pos
    }
    #[inline(always)]
    pub fn copy_ascii_to_check_space_two<'b>(
        &mut self,
        dest: &'b mut ByteDestination<'a>,
    ) -> CopyAsciiResult<(EncoderResult, usize, usize), (NonAscii, ByteTwoHandle<'b, 'a>)> {
        let non_ascii_ret = {
            let dst_len = dest.slice.len();
            let src_remaining = &self.slice[self.pos..];
            let dst_remaining = &mut dest.slice[dest.pos..];
            let (pending, length) = if dst_remaining.len() < src_remaining.len() {
                (EncoderResult::OutputFull, dst_remaining.len())
            } else {
                (EncoderResult::InputEmpty, src_remaining.len())
            };
            match unsafe {
                basic_latin_to_ascii(src_remaining.as_ptr(), dst_remaining.as_mut_ptr(), length)
            } {
                None => {
                    self.pos += length;
                    dest.pos += length;
                    return CopyAsciiResult::Stop((pending, self.pos, dest.pos));
                }
                Some((non_ascii, consumed)) => {
                    self.pos += consumed;
                    dest.pos += consumed;
                    if dest.pos + 1 < dst_len {
                        self.pos += 1; // commit to reading `non_ascii`
                        let unit = non_ascii;
                        let unit_minus_surrogate_start = unit.wrapping_sub(0xD800);
                        if unit_minus_surrogate_start > (0xDFFF - 0xD800) {
                            NonAscii::BmpExclAscii(unit)
                        } else if unit_minus_surrogate_start <= (0xDBFF - 0xD800) {
                            // high surrogate
                            if self.pos < self.slice.len() {
                                let second = self.slice[self.pos];
                                let second_minus_low_surrogate_start = second.wrapping_sub(0xDC00);
                                if second_minus_low_surrogate_start <= (0xDFFF - 0xDC00) {
                                    // The next code unit is a low surrogate. Advance position.
                                    self.pos += 1;
                                    NonAscii::Astral(unsafe {
                                        ::core::char::from_u32_unchecked(
                                            (u32::from(unit) << 10) + u32::from(second)
                                                - (((0xD800u32 << 10) - 0x10000u32) + 0xDC00u32),
                                        )
                                    })
                                } else {
                                    // The next code unit is not a low surrogate. Don't advance
                                    // position and treat the high surrogate as unpaired.
                                    NonAscii::BmpExclAscii(0xFFFDu16)
                                }
                            } else {
                                // Unpaired surrogate at the end of the buffer.
                                NonAscii::BmpExclAscii(0xFFFDu16)
                            }
                        } else {
                            // Unpaired low surrogate
                            NonAscii::BmpExclAscii(0xFFFDu16)
                        }
                    } else {
                        return CopyAsciiResult::Stop((
                            EncoderResult::OutputFull,
                            self.pos,
                            dest.pos,
                        ));
                    }
                }
            }
        };
        CopyAsciiResult::GoOn((non_ascii_ret, ByteTwoHandle::new(dest)))
    }
    #[inline(always)]
    pub fn copy_ascii_to_check_space_four<'b>(
        &mut self,
        dest: &'b mut ByteDestination<'a>,
    ) -> CopyAsciiResult<(EncoderResult, usize, usize), (NonAscii, ByteFourHandle<'b, 'a>)> {
        let non_ascii_ret = {
            let dst_len = dest.slice.len();
            let src_remaining = &self.slice[self.pos..];
            let dst_remaining = &mut dest.slice[dest.pos..];
            let (pending, length) = if dst_remaining.len() < src_remaining.len() {
                (EncoderResult::OutputFull, dst_remaining.len())
            } else {
                (EncoderResult::InputEmpty, src_remaining.len())
            };
            match unsafe {
                basic_latin_to_ascii(src_remaining.as_ptr(), dst_remaining.as_mut_ptr(), length)
            } {
                None => {
                    self.pos += length;
                    dest.pos += length;
                    return CopyAsciiResult::Stop((pending, self.pos, dest.pos));
                }
                Some((non_ascii, consumed)) => {
                    self.pos += consumed;
                    dest.pos += consumed;
                    if dest.pos + 3 < dst_len {
                        self.pos += 1; // commit to reading `non_ascii`
                        let unit = non_ascii;
                        let unit_minus_surrogate_start = unit.wrapping_sub(0xD800);
                        if unit_minus_surrogate_start > (0xDFFF - 0xD800) {
                            NonAscii::BmpExclAscii(unit)
                        } else if unit_minus_surrogate_start <= (0xDBFF - 0xD800) {
                            // high surrogate
                            if self.pos == self.slice.len() {
                                // Unpaired surrogate at the end of the buffer.
                                NonAscii::BmpExclAscii(0xFFFDu16)
                            } else {
                                let second = self.slice[self.pos];
                                let second_minus_low_surrogate_start = second.wrapping_sub(0xDC00);
                                if second_minus_low_surrogate_start <= (0xDFFF - 0xDC00) {
                                    // The next code unit is a low surrogate. Advance position.
                                    self.pos += 1;
                                    NonAscii::Astral(unsafe {
                                        ::core::char::from_u32_unchecked(
                                            (u32::from(unit) << 10) + u32::from(second)
                                                - (((0xD800u32 << 10) - 0x1_0000u32) + 0xDC00u32),
                                        )
                                    })
                                } else {
                                    // The next code unit is not a low surrogate. Don't advance
                                    // position and treat the high surrogate as unpaired.
                                    NonAscii::BmpExclAscii(0xFFFDu16)
                                }
                            }
                        } else {
                            // Unpaired low surrogate
                            NonAscii::BmpExclAscii(0xFFFDu16)
                        }
                    } else {
                        return CopyAsciiResult::Stop((
                            EncoderResult::OutputFull,
                            self.pos,
                            dest.pos,
                        ));
                    }
                }
            }
        };
        CopyAsciiResult::GoOn((non_ascii_ret, ByteFourHandle::new(dest)))
    }
}

pub struct Utf16ReadHandle<'a, 'b>
where
    'b: 'a,
{
    source: &'a mut Utf16Source<'b>,
}

impl<'a, 'b> Utf16ReadHandle<'a, 'b>
where
    'b: 'a,
{
    #[inline(always)]
    fn new(src: &'a mut Utf16Source<'b>) -> Utf16ReadHandle<'a, 'b> {
        Utf16ReadHandle { source: src }
    }
    #[inline(always)]
    pub fn read(self) -> (char, Utf16UnreadHandle<'a, 'b>) {
        let character = self.source.read();
        let handle = Utf16UnreadHandle::new(self.source);
        (character, handle)
    }
    #[inline(always)]
    pub fn read_enum(self) -> (Unicode, Utf16UnreadHandle<'a, 'b>) {
        let character = self.source.read_enum();
        let handle = Utf16UnreadHandle::new(self.source);
        (character, handle)
    }
    #[inline(always)]
    pub fn consumed(&self) -> usize {
        self.source.consumed()
    }
}

pub struct Utf16UnreadHandle<'a, 'b>
where
    'b: 'a,
{
    source: &'a mut Utf16Source<'b>,
}

impl<'a, 'b> Utf16UnreadHandle<'a, 'b>
where
    'b: 'a,
{
    #[inline(always)]
    fn new(src: &'a mut Utf16Source<'b>) -> Utf16UnreadHandle<'a, 'b> {
        Utf16UnreadHandle { source: src }
    }
    #[inline(always)]
    pub fn unread(self) -> usize {
        self.source.unread()
    }
    #[inline(always)]
    pub fn consumed(&self) -> usize {
        self.source.consumed()
    }
    #[inline(always)]
    pub fn commit(self) -> &'a mut Utf16Source<'b> {
        self.source
    }
}

// UTF-8 source

pub struct Utf8Source<'a> {
    slice: &'a [u8],
    pos: usize,
    old_pos: usize,
}

impl<'a> Utf8Source<'a> {
    #[inline(always)]
    pub fn new(src: &str) -> Utf8Source {
        Utf8Source {
            slice: src.as_bytes(),
            pos: 0,
            old_pos: 0,
        }
    }
    #[inline(always)]
    pub fn check_available<'b>(&'b mut self) -> Space<Utf8ReadHandle<'b, 'a>> {
        if self.pos < self.slice.len() {
            Space::Available(Utf8ReadHandle::new(self))
        } else {
            Space::Full(self.consumed())
        }
    }
    #[inline(always)]
    fn read(&mut self) -> char {
        self.old_pos = self.pos;
        let unit = self.slice[self.pos];
        if unit < 0x80 {
            self.pos += 1;
            return char::from(unit);
        }
        if unit < 0xE0 {
            let point =
                ((u32::from(unit) & 0x1F) << 6) | (u32::from(self.slice[self.pos + 1]) & 0x3F);
            self.pos += 2;
            return unsafe { ::core::char::from_u32_unchecked(point) };
        }
        if unit < 0xF0 {
            let point = ((u32::from(unit) & 0xF) << 12)
                | ((u32::from(self.slice[self.pos + 1]) & 0x3F) << 6)
                | (u32::from(self.slice[self.pos + 2]) & 0x3F);
            self.pos += 3;
            return unsafe { ::core::char::from_u32_unchecked(point) };
        }
        let point = ((u32::from(unit) & 0x7) << 18)
            | ((u32::from(self.slice[self.pos + 1]) & 0x3F) << 12)
            | ((u32::from(self.slice[self.pos + 2]) & 0x3F) << 6)
            | (u32::from(self.slice[self.pos + 3]) & 0x3F);
        self.pos += 4;
        unsafe { ::core::char::from_u32_unchecked(point) }
    }
    #[inline(always)]
    fn read_enum(&mut self) -> Unicode {
        self.old_pos = self.pos;
        let unit = self.slice[self.pos];
        if unit < 0x80 {
            self.pos += 1;
            return Unicode::Ascii(unit);
        }
        if unit < 0xE0 {
            let point =
                ((u16::from(unit) & 0x1F) << 6) | (u16::from(self.slice[self.pos + 1]) & 0x3F);
            self.pos += 2;
            return Unicode::NonAscii(NonAscii::BmpExclAscii(point));
        }
        if unit < 0xF0 {
            let point = ((u16::from(unit) & 0xF) << 12)
                | ((u16::from(self.slice[self.pos + 1]) & 0x3F) << 6)
                | (u16::from(self.slice[self.pos + 2]) & 0x3F);
            self.pos += 3;
            return Unicode::NonAscii(NonAscii::BmpExclAscii(point));
        }
        let point = ((u32::from(unit) & 0x7) << 18)
            | ((u32::from(self.slice[self.pos + 1]) & 0x3F) << 12)
            | ((u32::from(self.slice[self.pos + 2]) & 0x3F) << 6)
            | (u32::from(self.slice[self.pos + 3]) & 0x3F);
        self.pos += 4;
        Unicode::NonAscii(NonAscii::Astral(unsafe {
            ::core::char::from_u32_unchecked(point)
        }))
    }
    #[inline(always)]
    fn unread(&mut self) -> usize {
        self.pos = self.old_pos;
        self.pos
    }
    #[inline(always)]
    pub fn consumed(&self) -> usize {
        self.pos
    }
    #[inline(always)]
    pub fn copy_ascii_to_check_space_one<'b>(
        &mut self,
        dest: &'b mut ByteDestination<'a>,
    ) -> CopyAsciiResult<(EncoderResult, usize, usize), (NonAscii, ByteOneHandle<'b, 'a>)> {
        let non_ascii_ret = {
            let src_remaining = &self.slice[self.pos..];
            let dst_remaining = &mut dest.slice[dest.pos..];
            let (pending, length) = if dst_remaining.len() < src_remaining.len() {
                (EncoderResult::OutputFull, dst_remaining.len())
            } else {
                (EncoderResult::InputEmpty, src_remaining.len())
            };
            match unsafe {
                ascii_to_ascii(src_remaining.as_ptr(), dst_remaining.as_mut_ptr(), length)
            } {
                None => {
                    self.pos += length;
                    dest.pos += length;
                    return CopyAsciiResult::Stop((pending, self.pos, dest.pos));
                }
                Some((non_ascii, consumed)) => {
                    self.pos += consumed;
                    dest.pos += consumed;
                    // We don't need to check space in destination, because
                    // `ascii_to_ascii()` already did.
                    if non_ascii < 0xE0 {
                        let point = ((u16::from(non_ascii) & 0x1F) << 6)
                            | (u16::from(self.slice[self.pos + 1]) & 0x3F);
                        self.pos += 2;
                        NonAscii::BmpExclAscii(point)
                    } else if non_ascii < 0xF0 {
                        let point = ((u16::from(non_ascii) & 0xF) << 12)
                            | ((u16::from(self.slice[self.pos + 1]) & 0x3F) << 6)
                            | (u16::from(self.slice[self.pos + 2]) & 0x3F);
                        self.pos += 3;
                        NonAscii::BmpExclAscii(point)
                    } else {
                        let point = ((u32::from(non_ascii) & 0x7) << 18)
                            | ((u32::from(self.slice[self.pos + 1]) & 0x3F) << 12)
                            | ((u32::from(self.slice[self.pos + 2]) & 0x3F) << 6)
                            | (u32::from(self.slice[self.pos + 3]) & 0x3F);
                        self.pos += 4;
                        NonAscii::Astral(unsafe { ::core::char::from_u32_unchecked(point) })
                    }
                }
            }
        };
        CopyAsciiResult::GoOn((non_ascii_ret, ByteOneHandle::new(dest)))
    }
    #[inline(always)]
    pub fn copy_ascii_to_check_space_two<'b>(
        &mut self,
        dest: &'b mut ByteDestination<'a>,
    ) -> CopyAsciiResult<(EncoderResult, usize, usize), (NonAscii, ByteTwoHandle<'b, 'a>)> {
        let non_ascii_ret = {
            let dst_len = dest.slice.len();
            let src_remaining = &self.slice[self.pos..];
            let dst_remaining = &mut dest.slice[dest.pos..];
            let (pending, length) = if dst_remaining.len() < src_remaining.len() {
                (EncoderResult::OutputFull, dst_remaining.len())
            } else {
                (EncoderResult::InputEmpty, src_remaining.len())
            };
            match unsafe {
                ascii_to_ascii(src_remaining.as_ptr(), dst_remaining.as_mut_ptr(), length)
            } {
                None => {
                    self.pos += length;
                    dest.pos += length;
                    return CopyAsciiResult::Stop((pending, self.pos, dest.pos));
                }
                Some((non_ascii, consumed)) => {
                    self.pos += consumed;
                    dest.pos += consumed;
                    if dest.pos + 1 < dst_len {
                        if non_ascii < 0xE0 {
                            let point = ((u16::from(non_ascii) & 0x1F) << 6)
                                | (u16::from(self.slice[self.pos + 1]) & 0x3F);
                            self.pos += 2;
                            NonAscii::BmpExclAscii(point)
                        } else if non_ascii < 0xF0 {
                            let point = ((u16::from(non_ascii) & 0xF) << 12)
                                | ((u16::from(self.slice[self.pos + 1]) & 0x3F) << 6)
                                | (u16::from(self.slice[self.pos + 2]) & 0x3F);
                            self.pos += 3;
                            NonAscii::BmpExclAscii(point)
                        } else {
                            let point = ((u32::from(non_ascii) & 0x7) << 18)
                                | ((u32::from(self.slice[self.pos + 1]) & 0x3F) << 12)
                                | ((u32::from(self.slice[self.pos + 2]) & 0x3F) << 6)
                                | (u32::from(self.slice[self.pos + 3]) & 0x3F);
                            self.pos += 4;
                            NonAscii::Astral(unsafe { ::core::char::from_u32_unchecked(point) })
                        }
                    } else {
                        return CopyAsciiResult::Stop((
                            EncoderResult::OutputFull,
                            self.pos,
                            dest.pos,
                        ));
                    }
                }
            }
        };
        CopyAsciiResult::GoOn((non_ascii_ret, ByteTwoHandle::new(dest)))
    }
    #[inline(always)]
    pub fn copy_ascii_to_check_space_four<'b>(
        &mut self,
        dest: &'b mut ByteDestination<'a>,
    ) -> CopyAsciiResult<(EncoderResult, usize, usize), (NonAscii, ByteFourHandle<'b, 'a>)> {
        let non_ascii_ret = {
            let dst_len = dest.slice.len();
            let src_remaining = &self.slice[self.pos..];
            let dst_remaining = &mut dest.slice[dest.pos..];
            let (pending, length) = if dst_remaining.len() < src_remaining.len() {
                (EncoderResult::OutputFull, dst_remaining.len())
            } else {
                (EncoderResult::InputEmpty, src_remaining.len())
            };
            match unsafe {
                ascii_to_ascii(src_remaining.as_ptr(), dst_remaining.as_mut_ptr(), length)
            } {
                None => {
                    self.pos += length;
                    dest.pos += length;
                    return CopyAsciiResult::Stop((pending, self.pos, dest.pos));
                }
                Some((non_ascii, consumed)) => {
                    self.pos += consumed;
                    dest.pos += consumed;
                    if dest.pos + 3 < dst_len {
                        if non_ascii < 0xE0 {
                            let point = ((u16::from(non_ascii) & 0x1F) << 6)
                                | (u16::from(self.slice[self.pos + 1]) & 0x3F);
                            self.pos += 2;
                            NonAscii::BmpExclAscii(point)
                        } else if non_ascii < 0xF0 {
                            let point = ((u16::from(non_ascii) & 0xF) << 12)
                                | ((u16::from(self.slice[self.pos + 1]) & 0x3F) << 6)
                                | (u16::from(self.slice[self.pos + 2]) & 0x3F);
                            self.pos += 3;
                            NonAscii::BmpExclAscii(point)
                        } else {
                            let point = ((u32::from(non_ascii) & 0x7) << 18)
                                | ((u32::from(self.slice[self.pos + 1]) & 0x3F) << 12)
                                | ((u32::from(self.slice[self.pos + 2]) & 0x3F) << 6)
                                | (u32::from(self.slice[self.pos + 3]) & 0x3F);
                            self.pos += 4;
                            NonAscii::Astral(unsafe { ::core::char::from_u32_unchecked(point) })
                        }
                    } else {
                        return CopyAsciiResult::Stop((
                            EncoderResult::OutputFull,
                            self.pos,
                            dest.pos,
                        ));
                    }
                }
            }
        };
        CopyAsciiResult::GoOn((non_ascii_ret, ByteFourHandle::new(dest)))
    }
}

pub struct Utf8ReadHandle<'a, 'b>
where
    'b: 'a,
{
    source: &'a mut Utf8Source<'b>,
}

impl<'a, 'b> Utf8ReadHandle<'a, 'b>
where
    'b: 'a,
{
    #[inline(always)]
    fn new(src: &'a mut Utf8Source<'b>) -> Utf8ReadHandle<'a, 'b> {
        Utf8ReadHandle { source: src }
    }
    #[inline(always)]
    pub fn read(self) -> (char, Utf8UnreadHandle<'a, 'b>) {
        let character = self.source.read();
        let handle = Utf8UnreadHandle::new(self.source);
        (character, handle)
    }
    #[inline(always)]
    pub fn read_enum(self) -> (Unicode, Utf8UnreadHandle<'a, 'b>) {
        let character = self.source.read_enum();
        let handle = Utf8UnreadHandle::new(self.source);
        (character, handle)
    }
    #[inline(always)]
    pub fn consumed(&self) -> usize {
        self.source.consumed()
    }
}

pub struct Utf8UnreadHandle<'a, 'b>
where
    'b: 'a,
{
    source: &'a mut Utf8Source<'b>,
}

impl<'a, 'b> Utf8UnreadHandle<'a, 'b>
where
    'b: 'a,
{
    #[inline(always)]
    fn new(src: &'a mut Utf8Source<'b>) -> Utf8UnreadHandle<'a, 'b> {
        Utf8UnreadHandle { source: src }
    }
    #[inline(always)]
    pub fn unread(self) -> usize {
        self.source.unread()
    }
    #[inline(always)]
    pub fn consumed(&self) -> usize {
        self.source.consumed()
    }
    #[inline(always)]
    pub fn commit(self) -> &'a mut Utf8Source<'b> {
        self.source
    }
}

// Byte destination

pub struct ByteOneHandle<'a, 'b>
where
    'b: 'a,
{
    dest: &'a mut ByteDestination<'b>,
}

impl<'a, 'b> ByteOneHandle<'a, 'b>
where
    'b: 'a,
{
    #[inline(always)]
    fn new(dst: &'a mut ByteDestination<'b>) -> ByteOneHandle<'a, 'b> {
        ByteOneHandle { dest: dst }
    }
    #[inline(always)]
    pub fn written(&self) -> usize {
        self.dest.written()
    }
    #[inline(always)]
    pub fn write_one(self, first: u8) -> &'a mut ByteDestination<'b> {
        self.dest.write_one(first);
        self.dest
    }
}

pub struct ByteTwoHandle<'a, 'b>
where
    'b: 'a,
{
    dest: &'a mut ByteDestination<'b>,
}

impl<'a, 'b> ByteTwoHandle<'a, 'b>
where
    'b: 'a,
{
    #[inline(always)]
    fn new(dst: &'a mut ByteDestination<'b>) -> ByteTwoHandle<'a, 'b> {
        ByteTwoHandle { dest: dst }
    }
    #[inline(always)]
    pub fn written(&self) -> usize {
        self.dest.written()
    }
    #[inline(always)]
    pub fn write_one(self, first: u8) -> &'a mut ByteDestination<'b> {
        self.dest.write_one(first);
        self.dest
    }
    #[inline(always)]
    pub fn write_two(self, first: u8, second: u8) -> &'a mut ByteDestination<'b> {
        self.dest.write_two(first, second);
        self.dest
    }
}

pub struct ByteThreeHandle<'a, 'b>
where
    'b: 'a,
{
    dest: &'a mut ByteDestination<'b>,
}

impl<'a, 'b> ByteThreeHandle<'a, 'b>
where
    'b: 'a,
{
    #[inline(always)]
    fn new(dst: &'a mut ByteDestination<'b>) -> ByteThreeHandle<'a, 'b> {
        ByteThreeHandle { dest: dst }
    }
    #[inline(always)]
    pub fn written(&self) -> usize {
        self.dest.written()
    }
    #[inline(always)]
    pub fn write_one(self, first: u8) -> &'a mut ByteDestination<'b> {
        self.dest.write_one(first);
        self.dest
    }
    #[inline(always)]
    pub fn write_two(self, first: u8, second: u8) -> &'a mut ByteDestination<'b> {
        self.dest.write_two(first, second);
        self.dest
    }
    #[inline(always)]
    pub fn write_three(self, first: u8, second: u8, third: u8) -> &'a mut ByteDestination<'b> {
        self.dest.write_three(first, second, third);
        self.dest
    }
    #[inline(always)]
    pub fn write_three_return_written(self, first: u8, second: u8, third: u8) -> usize {
        self.dest.write_three(first, second, third);
        self.dest.written()
    }
}

pub struct ByteFourHandle<'a, 'b>
where
    'b: 'a,
{
    dest: &'a mut ByteDestination<'b>,
}

impl<'a, 'b> ByteFourHandle<'a, 'b>
where
    'b: 'a,
{
    #[inline(always)]
    fn new(dst: &'a mut ByteDestination<'b>) -> ByteFourHandle<'a, 'b> {
        ByteFourHandle { dest: dst }
    }
    #[inline(always)]
    pub fn written(&self) -> usize {
        self.dest.written()
    }
    #[inline(always)]
    pub fn write_one(self, first: u8) -> &'a mut ByteDestination<'b> {
        self.dest.write_one(first);
        self.dest
    }
    #[inline(always)]
    pub fn write_two(self, first: u8, second: u8) -> &'a mut ByteDestination<'b> {
        self.dest.write_two(first, second);
        self.dest
    }
    #[inline(always)]
    pub fn write_four(
        self,
        first: u8,
        second: u8,
        third: u8,
        fourth: u8,
    ) -> &'a mut ByteDestination<'b> {
        self.dest.write_four(first, second, third, fourth);
        self.dest
    }
}

pub struct ByteDestination<'a> {
    slice: &'a mut [u8],
    pos: usize,
}

impl<'a> ByteDestination<'a> {
    #[inline(always)]
    pub fn new(dst: &mut [u8]) -> ByteDestination {
        ByteDestination { slice: dst, pos: 0 }
    }
    #[inline(always)]
    pub fn check_space_one<'b>(&'b mut self) -> Space<ByteOneHandle<'b, 'a>> {
        if self.pos < self.slice.len() {
            Space::Available(ByteOneHandle::new(self))
        } else {
            Space::Full(self.written())
        }
    }
    #[inline(always)]
    pub fn check_space_two<'b>(&'b mut self) -> Space<ByteTwoHandle<'b, 'a>> {
        if self.pos + 1 < self.slice.len() {
            Space::Available(ByteTwoHandle::new(self))
        } else {
            Space::Full(self.written())
        }
    }
    #[inline(always)]
    pub fn check_space_three<'b>(&'b mut self) -> Space<ByteThreeHandle<'b, 'a>> {
        if self.pos + 2 < self.slice.len() {
            Space::Available(ByteThreeHandle::new(self))
        } else {
            Space::Full(self.written())
        }
    }
    #[inline(always)]
    pub fn check_space_four<'b>(&'b mut self) -> Space<ByteFourHandle<'b, 'a>> {
        if self.pos + 3 < self.slice.len() {
            Space::Available(ByteFourHandle::new(self))
        } else {
            Space::Full(self.written())
        }
    }
    #[inline(always)]
    pub fn written(&self) -> usize {
        self.pos
    }
    #[inline(always)]
    fn write_one(&mut self, first: u8) {
        self.slice[self.pos] = first;
        self.pos += 1;
    }
    #[inline(always)]
    fn write_two(&mut self, first: u8, second: u8) {
        self.slice[self.pos] = first;
        self.slice[self.pos + 1] = second;
        self.pos += 2;
    }
    #[inline(always)]
    fn write_three(&mut self, first: u8, second: u8, third: u8) {
        self.slice[self.pos] = first;
        self.slice[self.pos + 1] = second;
        self.slice[self.pos + 2] = third;
        self.pos += 3;
    }
    #[inline(always)]
    fn write_four(&mut self, first: u8, second: u8, third: u8, fourth: u8) {
        self.slice[self.pos] = first;
        self.slice[self.pos + 1] = second;
        self.slice[self.pos + 2] = third;
        self.slice[self.pos + 3] = fourth;
        self.pos += 4;
    }
}
