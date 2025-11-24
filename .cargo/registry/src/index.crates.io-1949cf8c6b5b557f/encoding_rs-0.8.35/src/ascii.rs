// Copyright Mozilla Foundation. See the COPYRIGHT
// file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// It's assumed that in due course Rust will have explicit SIMD but will not
// be good at run-time selection of SIMD vs. no-SIMD. In such a future,
// x86_64 will always use SSE2 and 32-bit x86 will use SSE2 when compiled with
// a Mozilla-shipped rustc. SIMD support and especially detection on ARM is a
// mess. Under the circumstances, it seems to make sense to optimize the ALU
// case for ARMv7 rather than x86. Annoyingly, I was unable to get useful
// numbers of the actual ARMv7 CPU I have access to, because (thermal?)
// throttling kept interfering. Since Raspberry Pi 3 (ARMv8 core but running
// ARMv7 code) produced reproducible performance numbers, that's the ARM
// computer that this code ended up being optimized for in the ALU case.
// Less popular CPU architectures simply get the approach that was chosen based
// on Raspberry Pi 3 measurements. The UTF-16 and UTF-8 ALU cases take
// different approaches based on benchmarking on Raspberry Pi 3.

#[cfg(all(
    feature = "simd-accel",
    any(
        target_feature = "sse2",
        all(target_endian = "little", target_arch = "aarch64"),
        all(target_endian = "little", target_feature = "neon")
    )
))]
use crate::simd_funcs::*;

cfg_if! {
    if #[cfg(feature = "simd-accel")] {
        #[allow(unused_imports)]
        use ::core::intrinsics::unlikely;
        #[allow(unused_imports)]
        use ::core::intrinsics::likely;
    } else {
        #[allow(dead_code)]
        #[inline(always)]
        fn unlikely(b: bool) -> bool {
            b
        }
        #[allow(dead_code)]
        #[inline(always)]
        fn likely(b: bool) -> bool {
            b
        }
    }
}

// Safety invariants for masks: data & mask = 0 for valid ASCII or basic latin utf-16

// `as` truncates, so works on 32-bit, too.
#[allow(dead_code)]
pub const ASCII_MASK: usize = 0x8080_8080_8080_8080u64 as usize;

// `as` truncates, so works on 32-bit, too.
#[allow(dead_code)]
pub const BASIC_LATIN_MASK: usize = 0xFF80_FF80_FF80_FF80u64 as usize;

#[allow(unused_macros)]
macro_rules! ascii_naive {
    ($name:ident, $src_unit:ty, $dst_unit:ty) => {
        /// Safety: src and dst must have len_unit elements and be aligned
        /// Safety-usable invariant: will return Some() when it fails
        /// to convert. The first value will be a u8 that is > 127.
        #[inline(always)]
        pub unsafe fn $name(
            src: *const $src_unit,
            dst: *mut $dst_unit,
            len: usize,
        ) -> Option<($src_unit, usize)> {
            // Yes, manually omitting the bound check here matters
            // a lot for perf.
            for i in 0..len {
                // Safety: len invariant used here
                let code_unit = *(src.add(i));
                // Safety: Upholds safety-usable invariant here
                if code_unit > 127 {
                    return Some((code_unit, i));
                }
                // Safety: len invariant used here
                *(dst.add(i)) = code_unit as $dst_unit;
            }
            return None;
        }
    };
}

#[allow(unused_macros)]
macro_rules! ascii_alu {
    ($name:ident,
     // safety invariant: src/dst MUST be u8
     $src_unit:ty,
     $dst_unit:ty,
     // Safety invariant: stride_fn must consume and produce two usizes, and return the index of the first non-ascii when it fails
     $stride_fn:ident) => {
        /// Safety: src and dst must have len elements, src is valid for read, dst is valid for
        /// write
        /// Safety-usable invariant: will return Some() when it fails
        /// to convert. The first value will be a u8 that is > 127.
        #[cfg_attr(feature = "cargo-clippy", allow(never_loop, cast_ptr_alignment))]
        #[inline(always)]
        pub unsafe fn $name(
            src: *const $src_unit,
            dst: *mut $dst_unit,
            len: usize,
        ) -> Option<($src_unit, usize)> {
            let mut offset = 0usize;
            // This loop is only broken out of as a `goto` forward
            loop {
                // Safety: until_alignment becomes the number of bytes we need to munch until we are aligned to usize
                let mut until_alignment = {
                    // Check if the other unit aligns if we move the narrower unit
                    // to alignment.
                    //               if ::core::mem::size_of::<$src_unit>() == ::core::mem::size_of::<$dst_unit>() {
                    // ascii_to_ascii
                    let src_alignment = (src as usize) & ALU_ALIGNMENT_MASK;
                    let dst_alignment = (dst as usize) & ALU_ALIGNMENT_MASK;
                    if src_alignment != dst_alignment {
                        // Safety: bails early and ends up in the naïve branch where usize-alignment doesn't matter
                        break;
                    }
                    (ALU_ALIGNMENT - src_alignment) & ALU_ALIGNMENT_MASK
                    //               } else if ::core::mem::size_of::<$src_unit>() < ::core::mem::size_of::<$dst_unit>() {
                    // ascii_to_basic_latin
                    //                   let src_until_alignment = (ALIGNMENT - ((src as usize) & ALIGNMENT_MASK)) & ALIGNMENT_MASK;
                    //                   if (dst.add(src_until_alignment) as usize) & ALIGNMENT_MASK != 0 {
                    //                       break;
                    //                   }
                    //                   src_until_alignment
                    //               } else {
                    // basic_latin_to_ascii
                    //                   let dst_until_alignment = (ALIGNMENT - ((dst as usize) & ALIGNMENT_MASK)) & ALIGNMENT_MASK;
                    //                   if (src.add(dst_until_alignment) as usize) & ALIGNMENT_MASK != 0 {
                    //                       break;
                    //                   }
                    //                   dst_until_alignment
                    //               }
                };
                if until_alignment + ALU_STRIDE_SIZE <= len {
                    // Moving pointers to alignment seems to be a pessimization on
                    // x86_64 for operations that have UTF-16 as the internal
                    // Unicode representation. However, since it seems to be a win
                    // on ARM (tested ARMv7 code running on ARMv8 [rpi3]), except
                    // mixed results when encoding from UTF-16 and since x86 and
                    // x86_64 should be using SSE2 in due course, keeping the move
                    // to alignment here. It would be good to test on more ARM CPUs
                    // and on real MIPS and POWER hardware.
                    //
                    // Safety: This is the naïve code once again, for `until_alignment` bytes
                    while until_alignment != 0 {
                        let code_unit = *(src.add(offset));
                        if code_unit > 127 {
                            // Safety: Upholds safety-usable invariant here
                            return Some((code_unit, offset));
                        }
                        *(dst.add(offset)) = code_unit as $dst_unit;
                        // Safety: offset is the number of bytes copied so far
                        offset += 1;
                        until_alignment -= 1;
                    }
                    let len_minus_stride = len - ALU_STRIDE_SIZE;
                    loop {
                        // Safety: num_ascii is known to be a byte index of a non-ascii byte due to stride_fn's invariant
                        if let Some(num_ascii) = $stride_fn(
                            // Safety: These are known to be valid and aligned since we have at
                            // least ALU_STRIDE_SIZE data in these buffers, and offset is the
                            // number of elements copied so far, which according to the
                            // until_alignment calculation above will cause both src and dst to be
                            // aligned to usize after this add
                            src.add(offset) as *const usize,
                            dst.add(offset) as *mut usize,
                        ) {
                            offset += num_ascii;
                            // Safety: Upholds safety-usable invariant here by indexing into non-ascii byte
                            return Some((*(src.add(offset)), offset));
                        }
                        // Safety: offset continues to be the number of bytes copied so far, and
                        // maintains usize alignment for the next loop iteration
                        offset += ALU_STRIDE_SIZE;
                        // Safety: This is `offset > len - stride. This loop will continue as long as
                        // `offset <= len - stride`, which means there are `stride` bytes to still be read.
                        if offset > len_minus_stride {
                            break;
                        }
                    }
                }
                break;
            }

            // Safety: This is the naïve code, same as ascii_naive, and has no requirements
            // other than src/dst being valid for the the right lens
            while offset < len {
                // Safety: len invariant used here
                let code_unit = *(src.add(offset));
                if code_unit > 127 {
                    // Safety: Upholds safety-usable invariant here
                    return Some((code_unit, offset));
                }
                // Safety: len invariant used here
                *(dst.add(offset)) = code_unit as $dst_unit;
                offset += 1;
            }
            None
        }
    };
}

#[allow(unused_macros)]
macro_rules! basic_latin_alu {
    ($name:ident,
    // safety invariant: use u8 for src/dest for ascii, and u16 for basic_latin
     $src_unit:ty,
     $dst_unit:ty,
    // safety invariant: stride function must munch ALU_STRIDE_SIZE*size(src_unit) bytes off of src and
    // write ALU_STRIDE_SIZE*size(dst_unit) bytes to dst
     $stride_fn:ident) => {
        /// Safety: src and dst must have len elements, src is valid for read, dst is valid for
        /// write
        /// Safety-usable invariant: will return Some() when it fails
        /// to convert. The first value will be a u8 that is > 127.
        #[cfg_attr(
            feature = "cargo-clippy",
            allow(never_loop, cast_ptr_alignment, cast_lossless)
        )]
        #[inline(always)]
        pub unsafe fn $name(
            src: *const $src_unit,
            dst: *mut $dst_unit,
            len: usize,
        ) -> Option<($src_unit, usize)> {
            let mut offset = 0usize;
            // This loop is only broken out of as a `goto` forward
            loop {
                // Safety: until_alignment becomes the number of bytes we need to munch from src/dest until we are aligned to usize
                // We ensure basic-latin has the same alignment as ascii, starting with ascii since it is smaller.
                let mut until_alignment = {
                    // Check if the other unit aligns if we move the narrower unit
                    // to alignment.
                    //               if ::core::mem::size_of::<$src_unit>() == ::core::mem::size_of::<$dst_unit>() {
                    // ascii_to_ascii
                    //                   let src_alignment = (src as usize) & ALIGNMENT_MASK;
                    //                   let dst_alignment = (dst as usize) & ALIGNMENT_MASK;
                    //                   if src_alignment != dst_alignment {
                    //                       break;
                    //                   }
                    //                   (ALIGNMENT - src_alignment) & ALIGNMENT_MASK
                    //               } else
                    if ::core::mem::size_of::<$src_unit>() < ::core::mem::size_of::<$dst_unit>() {
                        // ascii_to_basic_latin
                        let src_until_alignment = (ALU_ALIGNMENT
                            - ((src as usize) & ALU_ALIGNMENT_MASK))
                            & ALU_ALIGNMENT_MASK;
                        if (dst.wrapping_add(src_until_alignment) as usize) & ALU_ALIGNMENT_MASK
                            != 0
                        {
                            break;
                        }
                        src_until_alignment
                    } else {
                        // basic_latin_to_ascii
                        let dst_until_alignment = (ALU_ALIGNMENT
                            - ((dst as usize) & ALU_ALIGNMENT_MASK))
                            & ALU_ALIGNMENT_MASK;
                        if (src.wrapping_add(dst_until_alignment) as usize) & ALU_ALIGNMENT_MASK
                            != 0
                        {
                            break;
                        }
                        dst_until_alignment
                    }
                };
                if until_alignment + ALU_STRIDE_SIZE <= len {
                    // Moving pointers to alignment seems to be a pessimization on
                    // x86_64 for operations that have UTF-16 as the internal
                    // Unicode representation. However, since it seems to be a win
                    // on ARM (tested ARMv7 code running on ARMv8 [rpi3]), except
                    // mixed results when encoding from UTF-16 and since x86 and
                    // x86_64 should be using SSE2 in due course, keeping the move
                    // to alignment here. It would be good to test on more ARM CPUs
                    // and on real MIPS and POWER hardware.
                    //
                    // Safety: This is the naïve code once again, for `until_alignment` bytes
                    while until_alignment != 0 {
                        let code_unit = *(src.add(offset));
                        if code_unit > 127 {
                            // Safety: Upholds safety-usable invariant here
                            return Some((code_unit, offset));
                        }
                        *(dst.add(offset)) = code_unit as $dst_unit;
                        // Safety: offset is the number of bytes copied so far
                        offset += 1;
                        until_alignment -= 1;
                    }
                    let len_minus_stride = len - ALU_STRIDE_SIZE;
                    loop {
                        if !$stride_fn(
                            // Safety: These are known to be valid and aligned since we have at
                            // least ALU_STRIDE_SIZE data in these buffers, and offset is the
                            // number of elements copied so far, which according to the
                            // until_alignment calculation above will cause both src and dst to be
                            // aligned to usize after this add
                            src.add(offset) as *const usize,
                            dst.add(offset) as *mut usize,
                        ) {
                            break;
                        }
                        // Safety: offset continues to be the number of bytes copied so far, and
                        // maintains usize alignment for the next loop iteration
                        offset += ALU_STRIDE_SIZE;
                        // Safety: This is `offset > len - stride. This loop will continue as long as
                        // `offset <= len - stride`, which means there are `stride` bytes to still be read.
                        if offset > len_minus_stride {
                            break;
                        }
                    }
                }
                break;
            }
            // Safety: This is the naïve code once again, for leftover bytes
            while offset < len {
                // Safety: len invariant used here
                let code_unit = *(src.add(offset));
                if code_unit > 127 {
                    // Safety: Upholds safety-usable invariant here
                    return Some((code_unit, offset));
                }
                // Safety: len invariant used here
                *(dst.add(offset)) = code_unit as $dst_unit;
                offset += 1;
            }
            None
        }
    };
}

#[allow(unused_macros)]
macro_rules! latin1_alu {
    // safety invariant: stride function must munch ALU_STRIDE_SIZE*size(src_unit) bytes off of src and
    // write ALU_STRIDE_SIZE*size(dst_unit) bytes to dst
    ($name:ident, $src_unit:ty, $dst_unit:ty, $stride_fn:ident) => {
        /// Safety: src and dst must have len elements, src is valid for read, dst is valid for
        /// write
        #[cfg_attr(
            feature = "cargo-clippy",
            allow(never_loop, cast_ptr_alignment, cast_lossless)
        )]
        #[inline(always)]
        pub unsafe fn $name(src: *const $src_unit, dst: *mut $dst_unit, len: usize) {
            let mut offset = 0usize;
            // This loop is only broken out of as a `goto` forward
            loop {
                // Safety: until_alignment becomes the number of bytes we need to munch from src/dest until we are aligned to usize
                // We ensure the UTF-16 side has the same alignment as the Latin-1 side, starting with Latin-1 since it is smaller.
                let mut until_alignment = {
                    if ::core::mem::size_of::<$src_unit>() < ::core::mem::size_of::<$dst_unit>() {
                        // unpack
                        let src_until_alignment = (ALU_ALIGNMENT
                            - ((src as usize) & ALU_ALIGNMENT_MASK))
                            & ALU_ALIGNMENT_MASK;
                        if (dst.wrapping_add(src_until_alignment) as usize) & ALU_ALIGNMENT_MASK
                            != 0
                        {
                            break;
                        }
                        src_until_alignment
                    } else {
                        // pack
                        let dst_until_alignment = (ALU_ALIGNMENT
                            - ((dst as usize) & ALU_ALIGNMENT_MASK))
                            & ALU_ALIGNMENT_MASK;
                        if (src.wrapping_add(dst_until_alignment) as usize) & ALU_ALIGNMENT_MASK
                            != 0
                        {
                            break;
                        }
                        dst_until_alignment
                    }
                };
                if until_alignment + ALU_STRIDE_SIZE <= len {
                    // Safety: This is the naïve code once again, for `until_alignment` bytes
                    while until_alignment != 0 {
                        let code_unit = *(src.add(offset));
                        *(dst.add(offset)) = code_unit as $dst_unit;
                        // Safety: offset is the number of bytes copied so far
                        offset += 1;
                        until_alignment -= 1;
                    }
                    let len_minus_stride = len - ALU_STRIDE_SIZE;
                    loop {
                        $stride_fn(
                            // Safety: These are known to be valid and aligned since we have at
                            // least ALU_STRIDE_SIZE data in these buffers, and offset is the
                            // number of elements copied so far, which according to the
                            // until_alignment calculation above will cause both src and dst to be
                            // aligned to usize after this add
                            src.add(offset) as *const usize,
                            dst.add(offset) as *mut usize,
                        );
                        // Safety: offset continues to be the number of bytes copied so far, and
                        // maintains usize alignment for the next loop iteration
                        offset += ALU_STRIDE_SIZE;
                        // Safety: This is `offset > len - stride. This loop will continue as long as
                        // `offset <= len - stride`, which means there are `stride` bytes to still be read.
                        if offset > len_minus_stride {
                            break;
                        }
                    }
                }
                break;
            }
            // Safety: This is the naïve code once again, for leftover bytes
            while offset < len {
                // Safety: len invariant used here
                let code_unit = *(src.add(offset));
                *(dst.add(offset)) = code_unit as $dst_unit;
                offset += 1;
            }
        }
    };
}

#[allow(unused_macros)]
macro_rules! ascii_simd_check_align {
    (
        $name:ident,
        $src_unit:ty,
        $dst_unit:ty,
        // Safety: This function must require aligned src/dest that are valid for reading/writing SIMD_STRIDE_SIZE src_unit/dst_unit
        $stride_both_aligned:ident,
        // Safety: This function must require aligned/unaligned src/dest that are valid for reading/writing SIMD_STRIDE_SIZE src_unit/dst_unit
        $stride_src_aligned:ident,
        // Safety: This function must require unaligned/aligned src/dest that are valid for reading/writing SIMD_STRIDE_SIZE src_unit/dst_unit
        $stride_dst_aligned:ident,
        // Safety: This function must require unaligned src/dest that are valid for reading/writing SIMD_STRIDE_SIZE src_unit/dst_unit
        $stride_neither_aligned:ident
    ) => {
        /// Safety: src/dst must be valid for reads/writes of `len` elements of their units.
        ///
        /// Safety-usable invariant: will return Some() when it encounters non-ASCII, with the first element in the Some being
        /// guaranteed to be non-ASCII (> 127), and the second being the offset where it is found
        #[inline(always)]
        pub unsafe fn $name(
            src: *const $src_unit,
            dst: *mut $dst_unit,
            len: usize,
        ) -> Option<($src_unit, usize)> {
            let mut offset = 0usize;
            // Safety: if this check succeeds we're valid for reading/writing at least `SIMD_STRIDE_SIZE` elements.
            if SIMD_STRIDE_SIZE <= len {
                let len_minus_stride = len - SIMD_STRIDE_SIZE;
                // XXX Should we first process one stride unconditionally as unaligned to
                // avoid the cost of the branchiness below if the first stride fails anyway?
                // XXX Should we just use unaligned SSE2 access unconditionally? It seems that
                // on Haswell, it would make sense to just use unaligned and not bother
                // checking. Need to benchmark older architectures before deciding.
                let dst_masked = (dst as usize) & SIMD_ALIGNMENT_MASK;
                // Safety: checking whether src is aligned
                if ((src as usize) & SIMD_ALIGNMENT_MASK) == 0 {
                    // Safety: Checking whether dst is aligned
                    if dst_masked == 0 {
                        loop {
                            // Safety: We're valid to read/write SIMD_STRIDE_SIZE elements and have the appropriate alignments
                            if !$stride_both_aligned(src.add(offset), dst.add(offset)) {
                                break;
                            }
                            offset += SIMD_STRIDE_SIZE;
                            // Safety: This is `offset > len - SIMD_STRIDE_SIZE` which means we always have at least `SIMD_STRIDE_SIZE` elements to munch next time.
                            if offset > len_minus_stride {
                                break;
                            }
                        }
                    } else {
                        loop {
                            // Safety: We're valid to read/write SIMD_STRIDE_SIZE elements and have the appropriate alignments
                            if !$stride_src_aligned(src.add(offset), dst.add(offset)) {
                                break;
                            }
                            offset += SIMD_STRIDE_SIZE;
                            // Safety: This is `offset > len - SIMD_STRIDE_SIZE` which means we always have at least `SIMD_STRIDE_SIZE` elements to munch next time.
                            if offset > len_minus_stride {
                                break;
                            }
                        }
                    }
                } else {
                    if dst_masked == 0 {
                        loop {
                            // Safety: We're valid to read/write SIMD_STRIDE_SIZE elements and have the appropriate alignments
                            if !$stride_dst_aligned(src.add(offset), dst.add(offset)) {
                                break;
                            }
                            offset += SIMD_STRIDE_SIZE;
                            // Safety: This is `offset > len - SIMD_STRIDE_SIZE` which means we always have at least `SIMD_STRIDE_SIZE` elements to munch next time.
                            if offset > len_minus_stride {
                                break;
                            }
                        }
                    } else {
                        loop {
                            // Safety: We're valid to read/write SIMD_STRIDE_SIZE elements and have the appropriate alignments
                            if !$stride_neither_aligned(src.add(offset), dst.add(offset)) {
                                break;
                            }
                            offset += SIMD_STRIDE_SIZE;
                            // Safety: This is `offset > len - SIMD_STRIDE_SIZE` which means we always have at least `SIMD_STRIDE_SIZE` elements to munch next time.
                            if offset > len_minus_stride {
                                break;
                            }
                        }
                    }
                }
            }
            while offset < len {
                // Safety: uses len invariant here and below
                let code_unit = *(src.add(offset));
                if code_unit > 127 {
                    // Safety: upholds safety-usable invariant
                    return Some((code_unit, offset));
                }
                *(dst.add(offset)) = code_unit as $dst_unit;
                offset += 1;
            }
            None
        }
    };
}

#[allow(unused_macros)]
macro_rules! ascii_simd_check_align_unrolled {
    (
        $name:ident,
        $src_unit:ty,
        $dst_unit:ty,
        // Safety: This function must require aligned src/dest that are valid for reading/writing SIMD_STRIDE_SIZE src_unit/dst_unit
        $stride_both_aligned:ident,
        // Safety: This function must require aligned/unaligned src/dest that are valid for reading/writing SIMD_STRIDE_SIZE src_unit/dst_unit
        $stride_src_aligned:ident,
        // Safety: This function must require unaligned src/dest that are valid for reading/writing SIMD_STRIDE_SIZE src_unit/dst_unit
        $stride_neither_aligned:ident,
        // Safety: This function must require aligned src/dest that are valid for reading/writing 2*SIMD_STRIDE_SIZE src_unit/dst_unit
        $double_stride_both_aligned:ident,
        // Safety: This function must require aligned/unaligned src/dest that are valid for reading/writing 2*SIMD_STRIDE_SIZE src_unit/dst_unit
        $double_stride_src_aligned:ident
    ) => {
        /// Safety: src/dst must be valid for reads/writes of `len` elements of their units.
        ///
        /// Safety-usable invariant: will return Some() when it encounters non-ASCII, with the first element in the Some being
        /// guaranteed to be non-ASCII (> 127), and the second being the offset where it is found        #[inline(always)]
        pub unsafe fn $name(
            src: *const $src_unit,
            dst: *mut $dst_unit,
            len: usize,
        ) -> Option<($src_unit, usize)> {
            let unit_size = ::core::mem::size_of::<$src_unit>();
            let mut offset = 0usize;
            // This loop is only broken out of as a goto forward without
            // actually looping
            'outer: loop {
                // Safety: if this check succeeds we're valid for reading/writing at least `SIMD_STRIDE_SIZE` elements.
                if SIMD_STRIDE_SIZE <= len {
                    // First, process one unaligned
                    // Safety: this is safe to call since we're valid for this read/write
                    if !$stride_neither_aligned(src, dst) {
                        break 'outer;
                    }
                    offset = SIMD_STRIDE_SIZE;

                    // We have now seen 16 ASCII bytes. Let's guess that
                    // there will be enough more to justify more expense
                    // in the case of non-ASCII.
                    // Use aligned reads for the sake of old microachitectures.
                    //
                    // Safety: this correctly calculates the number of src_units that need to be read before the remaining list is aligned.
                    // This is less that SIMD_ALIGNMENT, which is also SIMD_STRIDE_SIZE (as documented)
                    let until_alignment = ((SIMD_ALIGNMENT
                        - ((src.add(offset) as usize) & SIMD_ALIGNMENT_MASK))
                        & SIMD_ALIGNMENT_MASK)
                        / unit_size;
                    // Safety: This addition won't overflow, because even in the 32-bit PAE case the
                    // address space holds enough code that the slice length can't be that
                    // close to address space size.
                    // offset now equals SIMD_STRIDE_SIZE, hence times 3 below.
                    //
                    // Safety: if this check succeeds we're valid for reading/writing at least `2 * SIMD_STRIDE_SIZE` elements plus `until_alignment`.
                    // The extra SIMD_STRIDE_SIZE in the condition is because `offset` is already `SIMD_STRIDE_SIZE`.
                    if until_alignment + (SIMD_STRIDE_SIZE * 3) <= len {
                        if until_alignment != 0 {
                            // Safety: this is safe to call since we're valid for this read/write (and more), and don't care about alignment
                            // This will copy over bytes that get decoded twice since it's not incrementing `offset` by SIMD_STRIDE_SIZE. This is fine.
                            if !$stride_neither_aligned(src.add(offset), dst.add(offset)) {
                                break;
                            }
                            offset += until_alignment;
                        }
                        // Safety: At this point we're valid for reading/writing 2*SIMD_STRIDE_SIZE elements
                        // Safety: Now `offset` is aligned for `src`
                        let len_minus_stride_times_two = len - (SIMD_STRIDE_SIZE * 2);
                        // Safety: This is whether dst is aligned
                        let dst_masked = (dst.add(offset) as usize) & SIMD_ALIGNMENT_MASK;
                        if dst_masked == 0 {
                            loop {
                                // Safety: both are aligned, we can call the aligned function. We're valid for reading/writing double stride from the initial condition
                                // and the loop break condition below
                                if let Some(advance) =
                                    $double_stride_both_aligned(src.add(offset), dst.add(offset))
                                {
                                    offset += advance;
                                    let code_unit = *(src.add(offset));
                                    // Safety: uses safety-usable invariant on ascii_to_ascii_simd_double_stride to return
                                    // guaranteed non-ascii
                                    return Some((code_unit, offset));
                                }
                                offset += SIMD_STRIDE_SIZE * 2;
                                // Safety: This is `offset > len - 2 * SIMD_STRIDE_SIZE` which means we always have at least `2 * SIMD_STRIDE_SIZE` elements to munch next time.
                                if offset > len_minus_stride_times_two {
                                    break;
                                }
                            }
                            // Safety: We're valid for reading/writing one more, and can still assume alignment
                            if offset + SIMD_STRIDE_SIZE <= len {
                                if !$stride_both_aligned(src.add(offset), dst.add(offset)) {
                                    break 'outer;
                                }
                                offset += SIMD_STRIDE_SIZE;
                            }
                        } else {
                            loop {
                                // Safety: only src is aligned here. We're valid for reading/writing double stride from the initial condition
                                // and the loop break condition below
                                if let Some(advance) =
                                    $double_stride_src_aligned(src.add(offset), dst.add(offset))
                                {
                                    offset += advance;
                                    let code_unit = *(src.add(offset));
                                    // Safety: uses safety-usable invariant on ascii_to_ascii_simd_double_stride to return
                                    // guaranteed non-ascii
                                    return Some((code_unit, offset));
                                }
                                offset += SIMD_STRIDE_SIZE * 2;
                                // Safety: This is `offset > len - 2 * SIMD_STRIDE_SIZE` which means we always have at least `2 * SIMD_STRIDE_SIZE` elements to munch next time.

                                if offset > len_minus_stride_times_two {
                                    break;
                                }
                            }
                            // Safety: We're valid for reading/writing one more, and can still assume alignment
                            if offset + SIMD_STRIDE_SIZE <= len {
                                if !$stride_src_aligned(src.add(offset), dst.add(offset)) {
                                    break 'outer;
                                }
                                offset += SIMD_STRIDE_SIZE;
                            }
                        }
                    } else {
                        // At most two iterations, so unroll
                        if offset + SIMD_STRIDE_SIZE <= len {
                            // Safety: The check above ensures we're allowed to read/write this, and we don't use alignment
                            if !$stride_neither_aligned(src.add(offset), dst.add(offset)) {
                                break;
                            }
                            offset += SIMD_STRIDE_SIZE;
                            if offset + SIMD_STRIDE_SIZE <= len {
                                // Safety: The check above ensures we're allowed to read/write this, and we don't use alignment
                                if !$stride_neither_aligned(src.add(offset), dst.add(offset)) {
                                    break;
                                }
                                offset += SIMD_STRIDE_SIZE;
                            }
                        }
                    }
                }
                break 'outer;
            }
            while offset < len {
                // Safety: relies straightforwardly on the `len` invariant
                let code_unit = *(src.add(offset));
                if code_unit > 127 {
                    // Safety-usable invariant upheld here
                    return Some((code_unit, offset));
                }
                *(dst.add(offset)) = code_unit as $dst_unit;
                offset += 1;
            }
            None
        }
    };
}

#[allow(unused_macros)]
macro_rules! latin1_simd_check_align {
    (
        $name:ident,
        $src_unit:ty,
        $dst_unit:ty,
        // Safety: This function must require aligned src/dest that are valid for reading/writing SIMD_STRIDE_SIZE src_unit/dst_unit
        $stride_both_aligned:ident,
        // Safety: This function must require aligned/unaligned src/dest that are valid for reading/writing SIMD_STRIDE_SIZE src_unit/dst_unit
        $stride_src_aligned:ident,
        // Safety: This function must require unaligned/aligned src/dest that are valid for reading/writing SIMD_STRIDE_SIZE src_unit/dst_unit
        $stride_dst_aligned:ident,
        // Safety: This function must require unaligned src/dest that are valid for reading/writing SIMD_STRIDE_SIZE src_unit/dst_unit
        $stride_neither_aligned:ident

    ) => {
        /// Safety: src/dst must be valid for reads/writes of `len` elements of their units.
        #[inline(always)]
        pub unsafe fn $name(src: *const $src_unit, dst: *mut $dst_unit, len: usize) {
            let mut offset = 0usize;
            // Safety: if this check succeeds we're valid for reading/writing at least `SIMD_STRIDE_SIZE` elements.
            if SIMD_STRIDE_SIZE <= len {
                let len_minus_stride = len - SIMD_STRIDE_SIZE;
                // Whether dst is aligned
                let dst_masked = (dst as usize) & SIMD_ALIGNMENT_MASK;
                // Whether src is aligned
                if ((src as usize) & SIMD_ALIGNMENT_MASK) == 0 {
                    if dst_masked == 0 {
                        loop {
                            // Safety: Both were aligned, we can use the aligned function
                            $stride_both_aligned(src.add(offset), dst.add(offset));
                            offset += SIMD_STRIDE_SIZE;
                            // Safety: This is `offset > len - SIMD_STRIDE_SIZE`, which means in the next iteration we're valid for
                            // reading/writing at least SIMD_STRIDE_SIZE elements.
                            if offset > len_minus_stride {
                                break;
                            }
                        }
                    } else {
                        loop {
                            // Safety: src was aligned, dst was not
                            $stride_src_aligned(src.add(offset), dst.add(offset));
                            offset += SIMD_STRIDE_SIZE;
                            // Safety: This is `offset > len - SIMD_STRIDE_SIZE`, which means in the next iteration we're valid for
                            // reading/writing at least SIMD_STRIDE_SIZE elements.
                            if offset > len_minus_stride {
                                break;
                            }
                        }
                    }
                } else {
                    if dst_masked == 0 {
                        loop {
                            // Safety: src was aligned, dst was not
                            $stride_dst_aligned(src.add(offset), dst.add(offset));
                            offset += SIMD_STRIDE_SIZE;
                            // Safety: This is `offset > len - SIMD_STRIDE_SIZE`, which means in the next iteration we're valid for
                            // reading/writing at least SIMD_STRIDE_SIZE elements.
                            if offset > len_minus_stride {
                                break;
                            }
                        }
                    } else {
                        loop {
                            // Safety: Neither were aligned
                            $stride_neither_aligned(src.add(offset), dst.add(offset));
                            offset += SIMD_STRIDE_SIZE;
                            // Safety: This is `offset > len - SIMD_STRIDE_SIZE`, which means in the next iteration we're valid for
                            // reading/writing at least SIMD_STRIDE_SIZE elements.
                            if offset > len_minus_stride {
                                break;
                            }
                        }
                    }
                }
            }
            while offset < len {
                // Safety: relies straightforwardly on the `len` invariant
                let code_unit = *(src.add(offset));
                *(dst.add(offset)) = code_unit as $dst_unit;
                offset += 1;
            }
        }
    };
}

#[allow(unused_macros)]
macro_rules! latin1_simd_check_align_unrolled {
    (
        $name:ident,
        $src_unit:ty,
        $dst_unit:ty,
        // Safety: This function must require aligned src/dest that are valid for reading/writing SIMD_STRIDE_SIZE src_unit/dst_unit
        $stride_both_aligned:ident,
        // Safety: This function must require aligned/unaligned src/dest that are valid for reading/writing SIMD_STRIDE_SIZE src_unit/dst_unit
        $stride_src_aligned:ident,
        // Safety: This function must require unaligned/aligned src/dest that are valid for reading/writing SIMD_STRIDE_SIZE src_unit/dst_unit
        $stride_dst_aligned:ident,
        // Safety: This function must require unaligned src/dest that are valid for reading/writing SIMD_STRIDE_SIZE src_unit/dst_unit
        $stride_neither_aligned:ident
    ) => {
        /// Safety: src/dst must be valid for reads/writes of `len` elements of their units.
        #[inline(always)]
        pub unsafe fn $name(src: *const $src_unit, dst: *mut $dst_unit, len: usize) {
            let unit_size = ::core::mem::size_of::<$src_unit>();
            let mut offset = 0usize;
            // Safety: if this check succeeds we're valid for reading/writing at least `SIMD_STRIDE_SIZE` elements.
            if SIMD_STRIDE_SIZE <= len {
                // Safety: this correctly calculates the number of src_units that need to be read before the remaining list is aligned.
                // This is by definition less than SIMD_STRIDE_SIZE.
                let mut until_alignment = ((SIMD_STRIDE_SIZE
                    - ((src as usize) & SIMD_ALIGNMENT_MASK))
                    & SIMD_ALIGNMENT_MASK)
                    / unit_size;
                while until_alignment != 0 {
                    // Safety: This is a straightforward copy, since until_alignment is < SIMD_STRIDE_SIZE < len, this is in-bounds
                    *(dst.add(offset)) = *(src.add(offset)) as $dst_unit;
                    offset += 1;
                    until_alignment -= 1;
                }
                // Safety: here offset will be `until_alignment`, i.e. enough to align `src`.
                let len_minus_stride = len - SIMD_STRIDE_SIZE;
                // Safety: if this check succeeds we're valid for reading/writing at least `2 * SIMD_STRIDE_SIZE` elements.
                if offset + SIMD_STRIDE_SIZE * 2 <= len {
                    let len_minus_stride_times_two = len_minus_stride - SIMD_STRIDE_SIZE;
                    // Safety: at this point src is known to be aligned at offset, dst is not.
                    if (dst.add(offset) as usize) & SIMD_ALIGNMENT_MASK == 0 {
                        loop {
                            // Safety: We checked alignment of dst above, we can use the alignment functions. We're allowed to read/write 2*SIMD_STRIDE_SIZE elements, which we do.
                            $stride_both_aligned(src.add(offset), dst.add(offset));
                            offset += SIMD_STRIDE_SIZE;
                            $stride_both_aligned(src.add(offset), dst.add(offset));
                            offset += SIMD_STRIDE_SIZE;
                            // Safety: This is `offset > len - 2 * SIMD_STRIDE_SIZE` which means we always have at least `2 * SIMD_STRIDE_SIZE` elements to munch next time.
                            if offset > len_minus_stride_times_two {
                                break;
                            }
                        }
                    } else {
                        loop {
                            // Safety: we ensured alignment of src already.
                            $stride_src_aligned(src.add(offset), dst.add(offset));
                            offset += SIMD_STRIDE_SIZE;
                            $stride_src_aligned(src.add(offset), dst.add(offset));
                            offset += SIMD_STRIDE_SIZE;
                            // Safety: This is `offset > len - 2 * SIMD_STRIDE_SIZE` which means we always have at least `2 * SIMD_STRIDE_SIZE` elements to munch next time.
                            if offset > len_minus_stride_times_two {
                                break;
                            }
                        }
                    }
                }
                // Safety: This is `offset > len - SIMD_STRIDE_SIZE` which means we are valid to munch SIMD_STRIDE_SIZE more elements, which we do
                if offset < len_minus_stride {
                    $stride_src_aligned(src.add(offset), dst.add(offset));
                    offset += SIMD_STRIDE_SIZE;
                }
            }
            while offset < len {
                // Safety: uses len invariant here and below
                let code_unit = *(src.add(offset));
                // On x86_64, this loop autovectorizes but in the pack
                // case there are instructions whose purpose is to make sure
                // each u16 in the vector is truncated before packing. However,
                // since we don't care about saturating behavior of SSE2 packing
                // when the input isn't Latin1, those instructions are useless.
                // Unfortunately, using the `assume` intrinsic to lie to the
                // optimizer doesn't make LLVM omit the trunctation that we
                // don't need. Possibly this loop could be manually optimized
                // to do the sort of thing that LLVM does but without the
                // ANDing the read vectors of u16 with a constant that discards
                // the high half of each u16. As far as I can tell, the
                // optimization assumes that doing a SIMD read past the end of
                // the array is OK.
                *(dst.add(offset)) = code_unit as $dst_unit;
                offset += 1;
            }
        }
    };
}

#[allow(unused_macros)]
macro_rules! ascii_simd_unalign {
    // Safety: stride_neither_aligned must be a function that requires src/dest be valid for unaligned reads/writes for SIMD_STRIDE_SIZE elements of type src_unit/dest_unit
    ($name:ident, $src_unit:ty, $dst_unit:ty, $stride_neither_aligned:ident) => {
        /// Safety: src and dst must be valid for reads/writes of len elements of type src_unit/dst_unit
        ///
        /// Safety-usable invariant: will return Some() when it encounters non-ASCII, with the first element in the Some being
        /// guaranteed to be non-ASCII (> 127), and the second being the offset where it is found
        #[inline(always)]
        pub unsafe fn $name(
            src: *const $src_unit,
            dst: *mut $dst_unit,
            len: usize,
        ) -> Option<($src_unit, usize)> {
            let mut offset = 0usize;
            // Safety: if this check succeeds we're valid for reading/writing at least `stride` elements.
            if SIMD_STRIDE_SIZE <= len {
                let len_minus_stride = len - SIMD_STRIDE_SIZE;
                loop {
                    // Safety: We know we're valid for `stride` reads/writes, so we can call this function. We don't need alignment.
                    if !$stride_neither_aligned(src.add(offset), dst.add(offset)) {
                        break;
                    }
                    offset += SIMD_STRIDE_SIZE;
                    // This is `offset > len - stride` which means we always have at least `stride` elements to munch next time.
                    if offset > len_minus_stride {
                        break;
                    }
                }
            }
            while offset < len {
                // Safety: Uses len invariant here and below
                let code_unit = *(src.add(offset));
                if code_unit > 127 {
                    // Safety-usable invariant upheld here
                    return Some((code_unit, offset));
                }
                *(dst.add(offset)) = code_unit as $dst_unit;
                offset += 1;
            }
            None
        }
    };
}

#[allow(unused_macros)]
macro_rules! latin1_simd_unalign {
    // Safety: stride_neither_aligned must be a function that requires src/dest be valid for unaligned reads/writes for SIMD_STRIDE_SIZE elements of type src_unit/dest_unit
    ($name:ident, $src_unit:ty, $dst_unit:ty, $stride_neither_aligned:ident) => {
        /// Safety: src and dst must be valid for unaligned reads/writes of len elements of type src_unit/dst_unit
        #[inline(always)]
        pub unsafe fn $name(src: *const $src_unit, dst: *mut $dst_unit, len: usize) {
            let mut offset = 0usize;
            // Safety: if this check succeeds we're valid for reading/writing at least `stride` elements.
            if SIMD_STRIDE_SIZE <= len {
                let len_minus_stride = len - SIMD_STRIDE_SIZE;
                loop {
                    // Safety: We know we're valid for `stride` reads/writes, so we can call this function. We don't need alignment.
                    $stride_neither_aligned(src.add(offset), dst.add(offset));
                    offset += SIMD_STRIDE_SIZE;
                    // This is `offset > len - stride` which means we always have at least `stride` elements to munch next time.
                    if offset > len_minus_stride {
                        break;
                    }
                }
            }
            while offset < len {
                // Safety: Uses len invariant here
                let code_unit = *(src.add(offset));
                *(dst.add(offset)) = code_unit as $dst_unit;
                offset += 1;
            }
        }
    };
}

#[allow(unused_macros)]
macro_rules! ascii_to_ascii_simd_stride {
    // Safety: load/store must be valid for 16 bytes of read/write, which may be unaligned. (candidates: `(load|store)(16|8)_(unaligned|aligned)` functions)
    ($name:ident, $load:ident, $store:ident) => {
        /// Safety: src and dst must be valid for 16 bytes of read/write according to
        /// the $load/$store fn, which may allow for unaligned reads/writes or require
        /// alignment to either 16x8 or u8x16.
        #[inline(always)]
        pub unsafe fn $name(src: *const u8, dst: *mut u8) -> bool {
            let simd = $load(src);
            if !simd_is_ascii(simd) {
                return false;
            }
            $store(dst, simd);
            true
        }
    };
}

#[allow(unused_macros)]
macro_rules! ascii_to_ascii_simd_double_stride {
    // Safety: store must be valid for 32 bytes of write, which may be unaligned (candidates: `store(8|16)_(aligned|unaligned)`)
    ($name:ident, $store:ident) => {
        /// Safety: src must be valid for 32 bytes of aligned u8x16 read
        /// dst must be valid for 32 bytes of unaligned write according to
        /// the $store fn, which may allow for unaligned writes or require
        /// alignment to either 16x8 or u8x16.
        ///
        /// Safety-usable invariant: Returns Some(index) if the element at `index` is invalid ASCII
        #[inline(always)]
        pub unsafe fn $name(src: *const u8, dst: *mut u8) -> Option<usize> {
            let first = load16_aligned(src);
            let second = load16_aligned(src.add(SIMD_STRIDE_SIZE));
            $store(dst, first);
            if unlikely(!simd_is_ascii(first | second)) {
                // Safety: mask_ascii produces a mask of all the high bits.
                let mask_first = mask_ascii(first);
                if mask_first != 0 {
                    // Safety: on little endian systems this will be the number of ascii bytes
                    // before the first non-ascii, i.e. valid for indexing src
                    // TODO SAFETY: What about big-endian systems?
                    return Some(mask_first.trailing_zeros() as usize);
                }
                $store(dst.add(SIMD_STRIDE_SIZE), second);
                let mask_second = mask_ascii(second);
                // Safety: on little endian systems this will be the number of ascii bytes
                // before the first non-ascii, i.e. valid for indexing src
                return Some(SIMD_STRIDE_SIZE + mask_second.trailing_zeros() as usize);
            }
            $store(dst.add(SIMD_STRIDE_SIZE), second);
            None
        }
    };
}

#[allow(unused_macros)]
macro_rules! ascii_to_basic_latin_simd_stride {
    // Safety: load/store must be valid for 16 bytes of read/write, which may be unaligned. (candidates: `(load|store)(16|8)_(unaligned|aligned)` functions)
    ($name:ident, $load:ident, $store:ident) => {
        /// Safety: src and dst must be valid for 16/32 bytes of read/write according to
        /// the $load/$store fn, which may allow for unaligned reads/writes or require
        /// alignment to either 16x8 or u8x16.
        #[inline(always)]
        pub unsafe fn $name(src: *const u8, dst: *mut u16) -> bool {
            let simd = $load(src);
            if !simd_is_ascii(simd) {
                return false;
            }
            let (first, second) = simd_unpack(simd);
            $store(dst, first);
            $store(dst.add(8), second);
            true
        }
    };
}

#[allow(unused_macros)]
macro_rules! ascii_to_basic_latin_simd_double_stride {
    // Safety: store must be valid for 16 bytes of write, which may be unaligned
    ($name:ident, $store:ident) => {
        /// Safety: src must be valid for 2*SIMD_STRIDE_SIZE bytes of aligned reads,
        /// aligned to either 16x8 or u8x16.
        /// dst must be valid for 2*SIMD_STRIDE_SIZE bytes of aligned or unaligned reads
        #[inline(always)]
        pub unsafe fn $name(src: *const u8, dst: *mut u16) -> Option<usize> {
            let first = load16_aligned(src);
            let second = load16_aligned(src.add(SIMD_STRIDE_SIZE));
            let (a, b) = simd_unpack(first);
            $store(dst, a);
            // Safety: divide by 2 since it's a u16 pointer
            $store(dst.add(SIMD_STRIDE_SIZE / 2), b);
            if unlikely(!simd_is_ascii(first | second)) {
                let mask_first = mask_ascii(first);
                if mask_first != 0 {
                    return Some(mask_first.trailing_zeros() as usize);
                }
                let (c, d) = simd_unpack(second);
                $store(dst.add(SIMD_STRIDE_SIZE), c);
                $store(dst.add(SIMD_STRIDE_SIZE + (SIMD_STRIDE_SIZE / 2)), d);
                let mask_second = mask_ascii(second);
                return Some(SIMD_STRIDE_SIZE + mask_second.trailing_zeros() as usize);
            }
            let (c, d) = simd_unpack(second);
            $store(dst.add(SIMD_STRIDE_SIZE), c);
            $store(dst.add(SIMD_STRIDE_SIZE + (SIMD_STRIDE_SIZE / 2)), d);
            None
        }
    };
}

#[allow(unused_macros)]
macro_rules! unpack_simd_stride {
    // Safety: load/store must be valid for 16 bytes of read/write, which may be unaligned. (candidates: `(load|store)(16|8)_(unaligned|aligned)` functions)
    ($name:ident, $load:ident, $store:ident) => {
        /// Safety: src and dst must be valid for 16 bytes of read/write according to
        /// the $load/$store fn, which may allow for unaligned reads/writes or require
        /// alignment to either 16x8 or u8x16.
        #[inline(always)]
        pub unsafe fn $name(src: *const u8, dst: *mut u16) {
            let simd = $load(src);
            let (first, second) = simd_unpack(simd);
            $store(dst, first);
            $store(dst.add(8), second);
        }
    };
}

#[allow(unused_macros)]
macro_rules! basic_latin_to_ascii_simd_stride {
    // Safety: load/store must be valid for 16 bytes of read/write, which may be unaligned. (candidates: `(load|store)(16|8)_(unaligned|aligned)` functions)
    ($name:ident, $load:ident, $store:ident) => {
        /// Safety: src and dst must be valid for 32/16 bytes of read/write according to
        /// the $load/$store fn, which may allow for unaligned reads/writes or require
        /// alignment to either 16x8 or u8x16.
        #[inline(always)]
        pub unsafe fn $name(src: *const u16, dst: *mut u8) -> bool {
            let first = $load(src);
            let second = $load(src.add(8));
            if simd_is_basic_latin(first | second) {
                $store(dst, simd_pack(first, second));
                true
            } else {
                false
            }
        }
    };
}

#[allow(unused_macros)]
macro_rules! pack_simd_stride {
    // Safety: load/store must be valid for 16 bytes of read/write, which may be unaligned. (candidates: `(load|store)(16|8)_(unaligned|aligned)` functions)
    ($name:ident, $load:ident, $store:ident) => {
        /// Safety: src and dst must be valid for 32/16 bytes of read/write according to
        /// the $load/$store fn, which may allow for unaligned reads/writes or require
        /// alignment to either 16x8 or u8x16.
        #[inline(always)]
        pub unsafe fn $name(src: *const u16, dst: *mut u8) {
            let first = $load(src);
            let second = $load(src.add(8));
            $store(dst, simd_pack(first, second));
        }
    };
}

cfg_if! {
    if #[cfg(all(feature = "simd-accel", target_endian = "little", target_arch = "aarch64"))] {
        // SIMD with the same instructions for aligned and unaligned loads and stores

        pub const SIMD_STRIDE_SIZE: usize = 16;

        pub const MAX_STRIDE_SIZE: usize = 16;

//        pub const ALIGNMENT: usize = 8;

        pub const ALU_STRIDE_SIZE: usize = 16;

        pub const ALU_ALIGNMENT: usize = 8;

        pub const ALU_ALIGNMENT_MASK: usize = 7;

        // Safety for stride macros: We stick to the load8_aligned/etc family of functions. We consistently produce
        // neither_unaligned variants using only unaligned inputs.
        ascii_to_ascii_simd_stride!(ascii_to_ascii_stride_neither_aligned, load16_unaligned, store16_unaligned);

        ascii_to_basic_latin_simd_stride!(ascii_to_basic_latin_stride_neither_aligned, load16_unaligned, store8_unaligned);
        unpack_simd_stride!(unpack_stride_neither_aligned, load16_unaligned, store8_unaligned);

        basic_latin_to_ascii_simd_stride!(basic_latin_to_ascii_stride_neither_aligned, load8_unaligned, store16_unaligned);
        pack_simd_stride!(pack_stride_neither_aligned, load8_unaligned, store16_unaligned);

        // Safety for conversion macros: We use the unalign macro with unalign functions above. All stride functions were produced
        // by stride macros that universally munch a single SIMD_STRIDE_SIZE worth of elements.
        ascii_simd_unalign!(ascii_to_ascii, u8, u8, ascii_to_ascii_stride_neither_aligned);
        ascii_simd_unalign!(ascii_to_basic_latin, u8, u16, ascii_to_basic_latin_stride_neither_aligned);
        ascii_simd_unalign!(basic_latin_to_ascii, u16, u8, basic_latin_to_ascii_stride_neither_aligned);
        latin1_simd_unalign!(unpack_latin1, u8, u16, unpack_stride_neither_aligned);
        latin1_simd_unalign!(pack_latin1, u16, u8, pack_stride_neither_aligned);
    } else if #[cfg(all(feature = "simd-accel", target_endian = "little", target_feature = "neon"))] {
        // SIMD with different instructions for aligned and unaligned loads and stores.
        //
        // Newer microarchitectures are not supposed to have a performance difference between
        // aligned and unaligned SSE2 loads and stores when the address is actually aligned,
        // but the benchmark results I see don't agree.

        pub const SIMD_STRIDE_SIZE: usize = 16;

        pub const MAX_STRIDE_SIZE: usize = 16;

        pub const SIMD_ALIGNMENT_MASK: usize = 15;

        // Safety for stride macros: We stick to the load8_aligned/etc family of functions. We consistently name
        // aligned/unaligned functions according to src/dst being aligned/unaligned

        ascii_to_ascii_simd_stride!(ascii_to_ascii_stride_both_aligned, load16_aligned, store16_aligned);
        ascii_to_ascii_simd_stride!(ascii_to_ascii_stride_src_aligned, load16_aligned, store16_unaligned);
        ascii_to_ascii_simd_stride!(ascii_to_ascii_stride_dst_aligned, load16_unaligned, store16_aligned);
        ascii_to_ascii_simd_stride!(ascii_to_ascii_stride_neither_aligned, load16_unaligned, store16_unaligned);

        ascii_to_basic_latin_simd_stride!(ascii_to_basic_latin_stride_both_aligned, load16_aligned, store8_aligned);
        ascii_to_basic_latin_simd_stride!(ascii_to_basic_latin_stride_src_aligned, load16_aligned, store8_unaligned);
        ascii_to_basic_latin_simd_stride!(ascii_to_basic_latin_stride_dst_aligned, load16_unaligned, store8_aligned);
        ascii_to_basic_latin_simd_stride!(ascii_to_basic_latin_stride_neither_aligned, load16_unaligned, store8_unaligned);

        unpack_simd_stride!(unpack_stride_both_aligned, load16_aligned, store8_aligned);
        unpack_simd_stride!(unpack_stride_src_aligned, load16_aligned, store8_unaligned);
        unpack_simd_stride!(unpack_stride_dst_aligned, load16_unaligned, store8_aligned);
        unpack_simd_stride!(unpack_stride_neither_aligned, load16_unaligned, store8_unaligned);

        basic_latin_to_ascii_simd_stride!(basic_latin_to_ascii_stride_both_aligned, load8_aligned, store16_aligned);
        basic_latin_to_ascii_simd_stride!(basic_latin_to_ascii_stride_src_aligned, load8_aligned, store16_unaligned);
        basic_latin_to_ascii_simd_stride!(basic_latin_to_ascii_stride_dst_aligned, load8_unaligned, store16_aligned);
        basic_latin_to_ascii_simd_stride!(basic_latin_to_ascii_stride_neither_aligned, load8_unaligned, store16_unaligned);

        pack_simd_stride!(pack_stride_both_aligned, load8_aligned, store16_aligned);
        pack_simd_stride!(pack_stride_src_aligned, load8_aligned, store16_unaligned);
        pack_simd_stride!(pack_stride_dst_aligned, load8_unaligned, store16_aligned);
        pack_simd_stride!(pack_stride_neither_aligned, load8_unaligned, store16_unaligned);

        // Safety for conversion macros: We use the correct pattern of both/src/dst/neither here. All stride functions were produced
        // by stride macros that universally munch a single SIMD_STRIDE_SIZE worth of elements.

        ascii_simd_check_align!(ascii_to_ascii, u8, u8, ascii_to_ascii_stride_both_aligned, ascii_to_ascii_stride_src_aligned, ascii_to_ascii_stride_dst_aligned, ascii_to_ascii_stride_neither_aligned);
        ascii_simd_check_align!(ascii_to_basic_latin, u8, u16, ascii_to_basic_latin_stride_both_aligned, ascii_to_basic_latin_stride_src_aligned, ascii_to_basic_latin_stride_dst_aligned, ascii_to_basic_latin_stride_neither_aligned);
        ascii_simd_check_align!(basic_latin_to_ascii, u16, u8, basic_latin_to_ascii_stride_both_aligned, basic_latin_to_ascii_stride_src_aligned, basic_latin_to_ascii_stride_dst_aligned, basic_latin_to_ascii_stride_neither_aligned);
        latin1_simd_check_align!(unpack_latin1, u8, u16, unpack_stride_both_aligned, unpack_stride_src_aligned, unpack_stride_dst_aligned, unpack_stride_neither_aligned);
        latin1_simd_check_align!(pack_latin1, u16, u8, pack_stride_both_aligned, pack_stride_src_aligned, pack_stride_dst_aligned, pack_stride_neither_aligned);
    } else if #[cfg(all(feature = "simd-accel", target_feature = "sse2"))] {
        // SIMD with different instructions for aligned and unaligned loads and stores.
        //
        // Newer microarchitectures are not supposed to have a performance difference between
        // aligned and unaligned SSE2 loads and stores when the address is actually aligned,
        // but the benchmark results I see don't agree.

        pub const SIMD_STRIDE_SIZE: usize = 16;

        /// Safety-usable invariant: This should be identical to SIMD_STRIDE_SIZE (used by ascii_simd_check_align_unrolled)
        pub const SIMD_ALIGNMENT: usize = 16;

        pub const MAX_STRIDE_SIZE: usize = 16;

        pub const SIMD_ALIGNMENT_MASK: usize = 15;

        // Safety for stride macros: We stick to the load8_aligned/etc family of functions. We consistently name
        // aligned/unaligned functions according to src/dst being aligned/unaligned

        ascii_to_ascii_simd_double_stride!(ascii_to_ascii_simd_double_stride_both_aligned, store16_aligned);
        ascii_to_ascii_simd_double_stride!(ascii_to_ascii_simd_double_stride_src_aligned, store16_unaligned);

        ascii_to_basic_latin_simd_double_stride!(ascii_to_basic_latin_simd_double_stride_both_aligned, store8_aligned);
        ascii_to_basic_latin_simd_double_stride!(ascii_to_basic_latin_simd_double_stride_src_aligned, store8_unaligned);

        ascii_to_ascii_simd_stride!(ascii_to_ascii_stride_both_aligned, load16_aligned, store16_aligned);
        ascii_to_ascii_simd_stride!(ascii_to_ascii_stride_src_aligned, load16_aligned, store16_unaligned);
        ascii_to_ascii_simd_stride!(ascii_to_ascii_stride_neither_aligned, load16_unaligned, store16_unaligned);

        ascii_to_basic_latin_simd_stride!(ascii_to_basic_latin_stride_both_aligned, load16_aligned, store8_aligned);
        ascii_to_basic_latin_simd_stride!(ascii_to_basic_latin_stride_src_aligned, load16_aligned, store8_unaligned);
        ascii_to_basic_latin_simd_stride!(ascii_to_basic_latin_stride_neither_aligned, load16_unaligned, store8_unaligned);

        unpack_simd_stride!(unpack_stride_both_aligned, load16_aligned, store8_aligned);
        unpack_simd_stride!(unpack_stride_src_aligned, load16_aligned, store8_unaligned);

        basic_latin_to_ascii_simd_stride!(basic_latin_to_ascii_stride_both_aligned, load8_aligned, store16_aligned);
        basic_latin_to_ascii_simd_stride!(basic_latin_to_ascii_stride_src_aligned, load8_aligned, store16_unaligned);
        basic_latin_to_ascii_simd_stride!(basic_latin_to_ascii_stride_dst_aligned, load8_unaligned, store16_aligned);
        basic_latin_to_ascii_simd_stride!(basic_latin_to_ascii_stride_neither_aligned, load8_unaligned, store16_unaligned);

        pack_simd_stride!(pack_stride_both_aligned, load8_aligned, store16_aligned);
        pack_simd_stride!(pack_stride_src_aligned, load8_aligned, store16_unaligned);

        // Safety for conversion macros: We use the correct pattern of both/src/dst/neither/double_both/double_src here. All stride functions were produced
        // by stride macros that universally munch a single SIMD_STRIDE_SIZE worth of elements.

        ascii_simd_check_align_unrolled!(ascii_to_ascii, u8, u8, ascii_to_ascii_stride_both_aligned, ascii_to_ascii_stride_src_aligned, ascii_to_ascii_stride_neither_aligned, ascii_to_ascii_simd_double_stride_both_aligned, ascii_to_ascii_simd_double_stride_src_aligned);
        ascii_simd_check_align_unrolled!(ascii_to_basic_latin, u8, u16, ascii_to_basic_latin_stride_both_aligned, ascii_to_basic_latin_stride_src_aligned, ascii_to_basic_latin_stride_neither_aligned, ascii_to_basic_latin_simd_double_stride_both_aligned, ascii_to_basic_latin_simd_double_stride_src_aligned);

        ascii_simd_check_align!(basic_latin_to_ascii, u16, u8, basic_latin_to_ascii_stride_both_aligned, basic_latin_to_ascii_stride_src_aligned, basic_latin_to_ascii_stride_dst_aligned, basic_latin_to_ascii_stride_neither_aligned);
        latin1_simd_check_align_unrolled!(unpack_latin1, u8, u16, unpack_stride_both_aligned, unpack_stride_src_aligned, unpack_stride_dst_aligned, unpack_stride_neither_aligned);
        latin1_simd_check_align_unrolled!(pack_latin1, u16, u8, pack_stride_both_aligned, pack_stride_src_aligned, pack_stride_dst_aligned, pack_stride_neither_aligned);
    } else if #[cfg(all(target_endian = "little", target_pointer_width = "64"))] {
        // Aligned ALU word, little-endian, 64-bit

        /// Safety invariant: this is the amount of bytes consumed by
        /// unpack_alu. This will be twice the pointer width, as it consumes two usizes.
        /// This is also the number of bytes produced by pack_alu.
        /// This is also the number of u16 code units produced/consumed by unpack_alu/pack_alu respectively.
        pub const ALU_STRIDE_SIZE: usize = 16;

        pub const MAX_STRIDE_SIZE: usize = 16;

        // Safety invariant: this is the pointer width in bytes
        pub const ALU_ALIGNMENT: usize = 8;

        // Safety invariant: this is a mask for getting the bits of a pointer not aligned to ALU_ALIGNMENT
        pub const ALU_ALIGNMENT_MASK: usize = 7;

        /// Safety: dst must point to valid space for writing four `usize`s
        #[inline(always)]
        unsafe fn unpack_alu(word: usize, second_word: usize, dst: *mut usize) {
            let first = ((0x0000_0000_FF00_0000usize & word) << 24) |
                        ((0x0000_0000_00FF_0000usize & word) << 16) |
                        ((0x0000_0000_0000_FF00usize & word) << 8) |
                        (0x0000_0000_0000_00FFusize & word);
            let second = ((0xFF00_0000_0000_0000usize & word) >> 8) |
                         ((0x00FF_0000_0000_0000usize & word) >> 16) |
                         ((0x0000_FF00_0000_0000usize & word) >> 24) |
                         ((0x0000_00FF_0000_0000usize & word) >> 32);
            let third = ((0x0000_0000_FF00_0000usize & second_word) << 24) |
                        ((0x0000_0000_00FF_0000usize & second_word) << 16) |
                        ((0x0000_0000_0000_FF00usize & second_word) << 8) |
                        (0x0000_0000_0000_00FFusize & second_word);
            let fourth = ((0xFF00_0000_0000_0000usize & second_word) >> 8) |
                         ((0x00FF_0000_0000_0000usize & second_word) >> 16) |
                         ((0x0000_FF00_0000_0000usize & second_word) >> 24) |
                         ((0x0000_00FF_0000_0000usize & second_word) >> 32);
            // Safety: fn invariant used here
            *dst = first;
            *(dst.add(1)) = second;
            *(dst.add(2)) = third;
            *(dst.add(3)) = fourth;
        }

        /// Safety: dst must point to valid space for writing two `usize`s
        #[inline(always)]
        unsafe fn pack_alu(first: usize, second: usize, third: usize, fourth: usize, dst: *mut usize) {
            let word = ((0x00FF_0000_0000_0000usize & second) << 8) |
                       ((0x0000_00FF_0000_0000usize & second) << 16) |
                       ((0x0000_0000_00FF_0000usize & second) << 24) |
                       ((0x0000_0000_0000_00FFusize & second) << 32) |
                       ((0x00FF_0000_0000_0000usize & first) >> 24) |
                       ((0x0000_00FF_0000_0000usize & first) >> 16) |
                       ((0x0000_0000_00FF_0000usize & first) >> 8) |
                       (0x0000_0000_0000_00FFusize & first);
            let second_word = ((0x00FF_0000_0000_0000usize & fourth) << 8) |
                              ((0x0000_00FF_0000_0000usize & fourth) << 16) |
                              ((0x0000_0000_00FF_0000usize & fourth) << 24) |
                              ((0x0000_0000_0000_00FFusize & fourth) << 32) |
                              ((0x00FF_0000_0000_0000usize & third) >> 24) |
                              ((0x0000_00FF_0000_0000usize & third) >> 16) |
                              ((0x0000_0000_00FF_0000usize & third) >> 8) |
                              (0x0000_0000_0000_00FFusize & third);
            // Safety: fn invariant used here
            *dst = word;
            *(dst.add(1)) = second_word;
        }
    } else if #[cfg(all(target_endian = "little", target_pointer_width = "32"))] {
        // Aligned ALU word, little-endian, 32-bit

        /// Safety invariant: this is the amount of bytes consumed by
        /// unpack_alu. This will be twice the pointer width, as it consumes two usizes.
        /// This is also the number of bytes produced by pack_alu.
        /// This is also the number of u16 code units produced/consumed by unpack_alu/pack_alu respectively.
        pub const ALU_STRIDE_SIZE: usize = 8;

        pub const MAX_STRIDE_SIZE: usize = 8;

        // Safety invariant: this is the pointer width in bytes
        pub const ALU_ALIGNMENT: usize = 4;

        // Safety invariant: this is a mask for getting the bits of a pointer not aligned to ALU_ALIGNMENT
        pub const ALU_ALIGNMENT_MASK: usize = 3;

        /// Safety: dst must point to valid space for writing four `usize`s
        #[inline(always)]
        unsafe fn unpack_alu(word: usize, second_word: usize, dst: *mut usize) {
            let first = ((0x0000_FF00usize & word) << 8) |
                        (0x0000_00FFusize & word);
            let second = ((0xFF00_0000usize & word) >> 8) |
                         ((0x00FF_0000usize & word) >> 16);
            let third = ((0x0000_FF00usize & second_word) << 8) |
                        (0x0000_00FFusize & second_word);
            let fourth = ((0xFF00_0000usize & second_word) >> 8) |
                         ((0x00FF_0000usize & second_word) >> 16);
            // Safety: fn invariant used here
            *dst = first;
            *(dst.add(1)) = second;
            *(dst.add(2)) = third;
            *(dst.add(3)) = fourth;
        }

        /// Safety: dst must point to valid space for writing two `usize`s
        #[inline(always)]
        unsafe fn pack_alu(first: usize, second: usize, third: usize, fourth: usize, dst: *mut usize) {
            let word = ((0x00FF_0000usize & second) << 8) |
                       ((0x0000_00FFusize & second) << 16) |
                       ((0x00FF_0000usize & first) >> 8) |
                       (0x0000_00FFusize & first);
            let second_word = ((0x00FF_0000usize & fourth) << 8) |
                              ((0x0000_00FFusize & fourth) << 16) |
                              ((0x00FF_0000usize & third) >> 8) |
                              (0x0000_00FFusize & third);
            // Safety: fn invariant used here
            *dst = word;
            *(dst.add(1)) = second_word;
        }
    } else if #[cfg(all(target_endian = "big", target_pointer_width = "64"))] {
        // Aligned ALU word, big-endian, 64-bit

        /// Safety invariant: this is the amount of bytes consumed by
        /// unpack_alu. This will be twice the pointer width, as it consumes two usizes.
        /// This is also the number of bytes produced by pack_alu.
        /// This is also the number of u16 code units produced/consumed by unpack_alu/pack_alu respectively.
        pub const ALU_STRIDE_SIZE: usize = 16;

        pub const MAX_STRIDE_SIZE: usize = 16;

        // Safety invariant: this is the pointer width in bytes
        pub const ALU_ALIGNMENT: usize = 8;

        // Safety invariant: this is a mask for getting the bits of a pointer not aligned to ALU_ALIGNMENT
        pub const ALU_ALIGNMENT_MASK: usize = 7;

        /// Safety: dst must point to valid space for writing four `usize`s
        #[inline(always)]
        unsafe fn unpack_alu(word: usize, second_word: usize, dst: *mut usize) {
            let first = ((0xFF00_0000_0000_0000usize & word) >> 8) |
                         ((0x00FF_0000_0000_0000usize & word) >> 16) |
                         ((0x0000_FF00_0000_0000usize & word) >> 24) |
                         ((0x0000_00FF_0000_0000usize & word) >> 32);
            let second = ((0x0000_0000_FF00_0000usize & word) << 24) |
                        ((0x0000_0000_00FF_0000usize & word) << 16) |
                        ((0x0000_0000_0000_FF00usize & word) << 8) |
                        (0x0000_0000_0000_00FFusize & word);
            let third = ((0xFF00_0000_0000_0000usize & second_word) >> 8) |
                         ((0x00FF_0000_0000_0000usize & second_word) >> 16) |
                         ((0x0000_FF00_0000_0000usize & second_word) >> 24) |
                         ((0x0000_00FF_0000_0000usize & second_word) >> 32);
            let fourth = ((0x0000_0000_FF00_0000usize & second_word) << 24) |
                        ((0x0000_0000_00FF_0000usize & second_word) << 16) |
                        ((0x0000_0000_0000_FF00usize & second_word) << 8) |
                        (0x0000_0000_0000_00FFusize & second_word);
            // Safety: fn invariant used here
            *dst = first;
            *(dst.add(1)) = second;
            *(dst.add(2)) = third;
            *(dst.add(3)) = fourth;
        }

        /// Safety: dst must point to valid space for writing two `usize`s
        #[inline(always)]
        unsafe fn pack_alu(first: usize, second: usize, third: usize, fourth: usize, dst: *mut usize) {
            let word = ((0x00FF0000_00000000usize & first) << 8) |
                       ((0x000000FF_00000000usize & first) << 16) |
                       ((0x00000000_00FF0000usize & first) << 24) |
                       ((0x00000000_000000FFusize & first) << 32) |
                       ((0x00FF0000_00000000usize & second) >> 24) |
                       ((0x000000FF_00000000usize & second) >> 16) |
                       ((0x00000000_00FF0000usize & second) >> 8) |
                       (0x00000000_000000FFusize & second);
            let second_word = ((0x00FF0000_00000000usize & third) << 8) |
                              ((0x000000FF_00000000usize & third) << 16) |
                              ((0x00000000_00FF0000usize & third) << 24) |
                              ((0x00000000_000000FFusize & third) << 32) |
                              ((0x00FF0000_00000000usize & fourth) >> 24) |
                              ((0x000000FF_00000000usize & fourth) >> 16) |
                              ((0x00000000_00FF0000usize & fourth) >> 8) |
                              (0x00000000_000000FFusize &  fourth);
            // Safety: fn invariant used here
            *dst = word;
            *(dst.add(1)) = second_word;
        }
    } else if #[cfg(all(target_endian = "big", target_pointer_width = "32"))] {
        // Aligned ALU word, big-endian, 32-bit

        /// Safety invariant: this is the amount of bytes consumed by
        /// unpack_alu. This will be twice the pointer width, as it consumes two usizes.
        /// This is also the number of bytes produced by pack_alu.
        /// This is also the number of u16 code units produced/consumed by unpack_alu/pack_alu respectively.
        pub const ALU_STRIDE_SIZE: usize = 8;

        pub const MAX_STRIDE_SIZE: usize = 8;

        // Safety invariant: this is the pointer width in bytes
        pub const ALU_ALIGNMENT: usize = 4;

        // Safety invariant: this is a mask for getting the bits of a pointer not aligned to ALU_ALIGNMENT
        pub const ALU_ALIGNMENT_MASK: usize = 3;

        /// Safety: dst must point to valid space for writing four `usize`s
        #[inline(always)]
        unsafe fn unpack_alu(word: usize, second_word: usize, dst: *mut usize) {
            let first = ((0xFF00_0000usize & word) >> 8) |
                         ((0x00FF_0000usize & word) >> 16);
            let second = ((0x0000_FF00usize & word) << 8) |
                        (0x0000_00FFusize & word);
            let third = ((0xFF00_0000usize & second_word) >> 8) |
                         ((0x00FF_0000usize & second_word) >> 16);
            let fourth = ((0x0000_FF00usize & second_word) << 8) |
                        (0x0000_00FFusize & second_word);
            // Safety: fn invariant used here
            *dst = first;
            *(dst.add(1)) = second;
            *(dst.add(2)) = third;
            *(dst.add(3)) = fourth;
        }

        /// Safety: dst must point to valid space for writing two `usize`s
        #[inline(always)]
        unsafe fn pack_alu(first: usize, second: usize, third: usize, fourth: usize, dst: *mut usize) {
            let word = ((0x00FF_0000usize & first) << 8) |
                       ((0x0000_00FFusize & first) << 16) |
                       ((0x00FF_0000usize & second) >> 8) |
                       (0x0000_00FFusize & second);
            let second_word = ((0x00FF_0000usize & third) << 8) |
                              ((0x0000_00FFusize & third) << 16) |
                              ((0x00FF_0000usize & fourth) >> 8) |
                              (0x0000_00FFusize & fourth);
            // Safety: fn invariant used here
            *dst = word;
            *(dst.add(1)) = second_word;
        }
    } else {
        ascii_naive!(ascii_to_ascii, u8, u8);
        ascii_naive!(ascii_to_basic_latin, u8, u16);
        ascii_naive!(basic_latin_to_ascii, u16, u8);
    }
}

cfg_if! {
    // Safety-usable invariant: this counts the zeroes from the "first byte" of utf-8 data packed into a usize
    // with the target endianness
    if #[cfg(target_endian = "little")] {
        #[allow(dead_code)]
        #[inline(always)]
        fn count_zeros(word: usize) -> u32 {
            word.trailing_zeros()
        }
    } else {
        #[allow(dead_code)]
        #[inline(always)]
        fn count_zeros(word: usize) -> u32 {
            word.leading_zeros()
        }
    }
}

cfg_if! {
    if #[cfg(all(feature = "simd-accel", target_endian = "little", target_arch = "disabled"))] {
        /// Safety-usable invariant: Will return the value and position of the first non-ASCII byte in the slice in a Some if found.
        /// In other words, the first element of the Some is always `> 127`
        #[inline(always)]
        pub fn validate_ascii(slice: &[u8]) -> Option<(u8, usize)> {
            let src = slice.as_ptr();
            let len = slice.len();
            let mut offset = 0usize;
            // Safety: if this check succeeds we're valid for reading/writing at least `stride` elements.
            if SIMD_STRIDE_SIZE <= len {
                let len_minus_stride = len - SIMD_STRIDE_SIZE;
                loop {
                    // Safety: src at offset is valid for a `SIMD_STRIDE_SIZE` read
                    let simd = unsafe { load16_unaligned(src.add(offset)) };
                    if !simd_is_ascii(simd) {
                        break;
                    }
                    offset += SIMD_STRIDE_SIZE;
                    // This is `offset > len - SIMD_STRIDE_SIZE` which means we always have at least `SIMD_STRIDE_SIZE` elements to munch next time.
                    if offset > len_minus_stride {
                        break;
                    }
                }
            }
            while offset < len {
                let code_unit = slice[offset];
                if code_unit > 127 {
                    // Safety: Safety-usable invariant upheld here
                    return Some((code_unit, offset));
                }
                offset += 1;
            }
            None
        }
    } else if #[cfg(all(feature = "simd-accel", target_feature = "sse2"))] {
        /// Safety-usable invariant: will return Some() when it encounters non-ASCII, with the first element in the Some being
        /// guaranteed to be non-ASCII (> 127), and the second being the offset where it is found
        #[inline(always)]
        pub fn validate_ascii(slice: &[u8]) -> Option<(u8, usize)> {
            let src = slice.as_ptr();
            let len = slice.len();
            let mut offset = 0usize;
            // Safety: if this check succeeds we're valid for reading at least `stride` elements.
            if SIMD_STRIDE_SIZE <= len {
                // First, process one unaligned vector
                // Safety: src is valid for a `SIMD_STRIDE_SIZE` read
                let simd = unsafe { load16_unaligned(src) };
                let mask = mask_ascii(simd);
                if mask != 0 {
                    offset = mask.trailing_zeros() as usize;
                    let non_ascii = unsafe { *src.add(offset) };
                    return Some((non_ascii, offset));
                }
                offset = SIMD_STRIDE_SIZE;
                // Safety: Now that offset has changed we don't yet know how much it is valid for

                // We have now seen 16 ASCII bytes. Let's guess that
                // there will be enough more to justify more expense
                // in the case of non-ASCII.
                // Use aligned reads for the sake of old microachitectures.
                // Safety: this correctly calculates the number of src_units that need to be read before the remaining list is aligned.
                // This is by definition less than SIMD_ALIGNMENT, which is defined to be equal to SIMD_STRIDE_SIZE.
                let until_alignment = unsafe { (SIMD_ALIGNMENT - ((src.add(offset) as usize) & SIMD_ALIGNMENT_MASK)) & SIMD_ALIGNMENT_MASK };
                // This addition won't overflow, because even in the 32-bit PAE case the
                // address space holds enough code that the slice length can't be that
                // close to address space size.
                // offset now equals SIMD_STRIDE_SIZE, hence times 3 below.
                //
                // Safety: if this check succeeds we're valid for reading at least `2 * SIMD_STRIDE_SIZE` elements plus `until_alignment`.
                // The extra SIMD_STRIDE_SIZE in the condition is because `offset` is already `SIMD_STRIDE_SIZE`.
                if until_alignment + (SIMD_STRIDE_SIZE * 3) <= len {
                    if until_alignment != 0 {
                        // Safety: this is safe to call since we're valid for this read (and more), and don't care about alignment
                        // This will copy over bytes that get decoded twice since it's not incrementing `offset` by SIMD_STRIDE_SIZE. This is fine.
                        let simd = unsafe { load16_unaligned(src.add(offset)) };
                        let mask = mask_ascii(simd);
                        if mask != 0 {
                            offset += mask.trailing_zeros() as usize;
                            let non_ascii = unsafe { *src.add(offset) };
                            return Some((non_ascii, offset));
                        }
                        offset += until_alignment;
                    }
                    // Safety: At this point we're valid for reading 2*SIMD_STRIDE_SIZE elements
                    // Safety: Now `offset` is aligned for `src`
                    let len_minus_stride_times_two = len - (SIMD_STRIDE_SIZE * 2);
                    loop {
                        // Safety: We were valid for this read, and were aligned.
                        let first = unsafe { load16_aligned(src.add(offset)) };
                        let second = unsafe { load16_aligned(src.add(offset + SIMD_STRIDE_SIZE)) };
                        if !simd_is_ascii(first | second) {
                            // Safety: mask_ascii produces a mask of all the high bits.
                            let mask_first = mask_ascii(first);
                            if mask_first != 0 {
                                // Safety: on little endian systems this will be the number of ascii bytes
                                // before the first non-ascii, i.e. valid for indexing src
                                // TODO SAFETY: What about big-endian systems?
                                offset += mask_first.trailing_zeros() as usize;
                            } else {
                                let mask_second = mask_ascii(second);
                                // Safety: on little endian systems this will be the number of ascii bytes
                                // before the first non-ascii, i.e. valid for indexing src
                                offset += SIMD_STRIDE_SIZE + mask_second.trailing_zeros() as usize;
                            }
                            // Safety: We know this is non-ASCII, and can uphold the safety-usable invariant here
                            let non_ascii = unsafe { *src.add(offset) };

                            return Some((non_ascii, offset));
                        }
                        offset += SIMD_STRIDE_SIZE * 2;
                        // Safety: This is `offset > len - 2 * SIMD_STRIDE_SIZE` which means we always have at least `2 * SIMD_STRIDE_SIZE` elements to munch next time.
                        if offset > len_minus_stride_times_two {
                            break;
                        }
                    }
                    // Safety: if this check succeeds we're valid for reading at least `SIMD_STRIDE_SIZE`
                    if offset + SIMD_STRIDE_SIZE <= len {
                        // Safety: We were valid for this read, and were aligned.
                        let simd = unsafe { load16_aligned(src.add(offset)) };
                        // Safety: mask_ascii produces a mask of all the high bits.
                        let mask = mask_ascii(simd);
                        if mask != 0 {
                            // Safety: on little endian systems this will be the number of ascii bytes
                            // before the first non-ascii, i.e. valid for indexing src
                            offset += mask.trailing_zeros() as usize;
                            let non_ascii = unsafe { *src.add(offset) };
                            // Safety: We know this is non-ASCII, and can uphold the safety-usable invariant here
                            return Some((non_ascii, offset));
                        }
                        offset += SIMD_STRIDE_SIZE;
                    }
                } else {
                    // Safety: this is the unaligned branch
                    // At most two iterations, so unroll
                    // Safety: if this check succeeds we're valid for reading at least `SIMD_STRIDE_SIZE`
                    if offset + SIMD_STRIDE_SIZE <= len {
                        // Safety: We're valid for this read but must use an unaligned read
                        let simd = unsafe { load16_unaligned(src.add(offset)) };
                        let mask = mask_ascii(simd);
                        if mask != 0 {
                            offset += mask.trailing_zeros() as usize;
                            let non_ascii = unsafe { *src.add(offset) };
                            // Safety-usable invariant upheld here (same as above)
                            return Some((non_ascii, offset));
                        }
                        offset += SIMD_STRIDE_SIZE;
                        // Safety: if this check succeeds we're valid for reading at least `SIMD_STRIDE_SIZE`
                        if offset + SIMD_STRIDE_SIZE <= len {
                            // Safety: We're valid for this read but must use an unaligned read
                             let simd = unsafe { load16_unaligned(src.add(offset)) };
                             let mask = mask_ascii(simd);
                            if mask != 0 {
                                offset += mask.trailing_zeros() as usize;
                                let non_ascii = unsafe { *src.add(offset) };
                                // Safety-usable invariant upheld here (same as above)
                                return Some((non_ascii, offset));
                            }
                            offset += SIMD_STRIDE_SIZE;
                        }
                    }
                }
            }
            while offset < len {
                // Safety: relies straightforwardly on the `len` invariant
                let code_unit = unsafe { *(src.add(offset)) };
                if code_unit > 127 {
                    // Safety-usable invariant upheld here
                    return Some((code_unit, offset));
                }
                offset += 1;
            }
            None
        }
    } else {
        // Safety-usable invariant: returns byte index of first non-ascii byte
        #[inline(always)]
        fn find_non_ascii(word: usize, second_word: usize) -> Option<usize> {
            let word_masked = word & ASCII_MASK;
            let second_masked = second_word & ASCII_MASK;
            if (word_masked | second_masked) == 0 {
                // Both are ascii, invariant upheld
                return None;
            }
            if word_masked != 0 {
                let zeros = count_zeros(word_masked);
                // `zeros` now contains 0 to 7 (for the seven bits of masked ASCII in little endian,
                // or up to 7 bits of non-ASCII in big endian if the first byte is non-ASCII)
                // plus 8 times the number of ASCII in text order before the
                // non-ASCII byte in the little-endian case or 8 times the number of ASCII in
                // text order before the non-ASCII byte in the big-endian case.
                let num_ascii = (zeros >> 3) as usize;
                // Safety-usable invariant upheld here
                return Some(num_ascii);
            }
            let zeros = count_zeros(second_masked);
            // `zeros` now contains 0 to 7 (for the seven bits of masked ASCII in little endian,
            // or up to 7 bits of non-ASCII in big endian if the first byte is non-ASCII)
            // plus 8 times the number of ASCII in text order before the
            // non-ASCII byte in the little-endian case or 8 times the number of ASCII in
            // text order before the non-ASCII byte in the big-endian case.
            let num_ascii = (zeros >> 3) as usize;
            // Safety-usable invariant upheld here
            Some(ALU_ALIGNMENT + num_ascii)
        }

        /// Safety: `src` must be valid for the reads of two `usize`s
        ///
        /// Safety-usable invariant: will return byte index of first non-ascii byte
        #[inline(always)]
        unsafe fn validate_ascii_stride(src: *const usize) -> Option<usize> {
            let word = *src;
            let second_word = *(src.add(1));
            find_non_ascii(word, second_word)
        }

        /// Safety-usable invariant: will return Some() when it encounters non-ASCII, with the first element in the Some being
        /// guaranteed to be non-ASCII (> 127), and the second being the offset where it is found
        #[cfg_attr(feature = "cargo-clippy", allow(cast_ptr_alignment))]
        #[inline(always)]
        pub fn validate_ascii(slice: &[u8]) -> Option<(u8, usize)> {
            let src = slice.as_ptr();
            let len = slice.len();
            let mut offset = 0usize;
            let mut until_alignment = (ALU_ALIGNMENT - ((src as usize) & ALU_ALIGNMENT_MASK)) & ALU_ALIGNMENT_MASK;
            // Safety: If this check fails we're valid to read `until_alignment + ALU_STRIDE_SIZE` elements
            if until_alignment + ALU_STRIDE_SIZE <= len {
                while until_alignment != 0 {
                    let code_unit = slice[offset];
                    if code_unit > 127 {
                        // Safety-usable invairant upheld here
                        return Some((code_unit, offset));
                    }
                    offset += 1;
                    until_alignment -= 1;
                }
                // Safety: At this point we have read until_alignment elements and
                // are valid for `ALU_STRIDE_SIZE` more.
                let len_minus_stride = len - ALU_STRIDE_SIZE;
                loop {
                    // Safety: we were valid for this read
                    let ptr = unsafe { src.add(offset) as *const usize };
                    if let Some(num_ascii) = unsafe { validate_ascii_stride(ptr) } {
                        offset += num_ascii;
                        // Safety-usable invairant upheld here using the invariant from validate_ascii_stride()
                        return Some((unsafe { *(src.add(offset)) }, offset));
                    }
                    offset += ALU_STRIDE_SIZE;
                    // Safety: This is `offset > ALU_STRIDE_SIZE` which means we always have at least `2 * ALU_STRIDE_SIZE` elements to munch next time.
                    if offset > len_minus_stride {
                        break;
                    }
                }
            }
            while offset < len {
                let code_unit = slice[offset];
                if code_unit > 127 {
                    // Safety-usable invairant upheld here
                    return Some((code_unit, offset));
                }
                offset += 1;
           }
           None
        }

    }
}

cfg_if! {
    if #[cfg(all(feature = "simd-accel", any(target_feature = "sse2", all(target_endian = "little", target_arch = "aarch64"))))] {

    } else if #[cfg(all(feature = "simd-accel", target_endian = "little", target_feature = "neon"))] {
        // Even with NEON enabled, we use the ALU path for ASCII validation, because testing
        // on Exynos 5 indicated that using NEON isn't worthwhile where there are only
        // vector reads without vector writes.

        pub const ALU_STRIDE_SIZE: usize = 8;

        pub const ALU_ALIGNMENT: usize = 4;

        pub const ALU_ALIGNMENT_MASK: usize = 3;
    } else {
        // Safety: src points to two valid `usize`s, dst points to four valid `usize`s
        #[inline(always)]
        unsafe fn unpack_latin1_stride_alu(src: *const usize, dst: *mut usize) {
            // Safety: src safety invariant used here
            let word = *src;
            let second_word = *(src.add(1));
            // Safety: dst safety invariant passed down
            unpack_alu(word, second_word, dst);
        }

        // Safety: src points to four valid `usize`s, dst points to two valid `usize`s
        #[inline(always)]
        unsafe fn pack_latin1_stride_alu(src: *const usize, dst: *mut usize) {
            // Safety: src safety invariant used here
            let first = *src;
            let second = *(src.add(1));
            let third = *(src.add(2));
            let fourth = *(src.add(3));
            // Safety: dst safety invariant passed down
            pack_alu(first, second, third, fourth, dst);
        }

        // Safety: src points to two valid `usize`s, dst points to four valid `usize`s
        #[inline(always)]
        unsafe fn ascii_to_basic_latin_stride_alu(src: *const usize, dst: *mut usize) -> bool {
            // Safety: src safety invariant used here
            let word = *src;
            let second_word = *(src.add(1));
            // Check if the words contains non-ASCII
            if (word & ASCII_MASK) | (second_word & ASCII_MASK) != 0 {
                return false;
            }
            // Safety: dst safety invariant passed down
            unpack_alu(word, second_word, dst);
            true
        }

        // Safety: src points four valid `usize`s, dst points to two valid `usize`s
        #[inline(always)]
        unsafe fn basic_latin_to_ascii_stride_alu(src: *const usize, dst: *mut usize) -> bool {
            // Safety: src safety invariant used here
            let first = *src;
            let second = *(src.add(1));
            let third = *(src.add(2));
            let fourth = *(src.add(3));
            if (first & BASIC_LATIN_MASK) | (second & BASIC_LATIN_MASK) | (third & BASIC_LATIN_MASK) | (fourth & BASIC_LATIN_MASK) != 0 {
                return false;
            }
            // Safety: dst safety invariant passed down
            pack_alu(first, second, third, fourth, dst);
            true
        }

        // Safety: src, dst both point to two valid `usize`s each
        // Safety-usable invariant: Will return byte index of first non-ascii byte.
        #[inline(always)]
        unsafe fn ascii_to_ascii_stride(src: *const usize, dst: *mut usize) -> Option<usize> {
            // Safety: src safety invariant used here
            let word = *src;
            let second_word = *(src.add(1));
            // Safety: src safety invariant used here
            *dst = word;
            *(dst.add(1)) = second_word;
            // Relies on safety-usable invariant here
            find_non_ascii(word, second_word)
        }

        basic_latin_alu!(ascii_to_basic_latin, u8, u16, ascii_to_basic_latin_stride_alu);
        basic_latin_alu!(basic_latin_to_ascii, u16, u8, basic_latin_to_ascii_stride_alu);
        latin1_alu!(unpack_latin1, u8, u16, unpack_latin1_stride_alu);
        latin1_alu!(pack_latin1, u16, u8, pack_latin1_stride_alu);
        // Safety invariant upheld: ascii_to_ascii_stride will return byte index of first non-ascii if found
        ascii_alu!(ascii_to_ascii, u8, u8, ascii_to_ascii_stride);
    }
}

pub fn ascii_valid_up_to(bytes: &[u8]) -> usize {
    match validate_ascii(bytes) {
        None => bytes.len(),
        Some((_, num_valid)) => num_valid,
    }
}

pub fn iso_2022_jp_ascii_valid_up_to(bytes: &[u8]) -> usize {
    for (i, b_ref) in bytes.iter().enumerate() {
        let b = *b_ref;
        if b >= 0x80 || b == 0x1B || b == 0x0E || b == 0x0F {
            return i;
        }
    }
    bytes.len()
}

// Any copyright to the test code below this comment is dedicated to the
// Public Domain. http://creativecommons.org/publicdomain/zero/1.0/

#[cfg(all(test, feature = "alloc"))]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    macro_rules! test_ascii {
        ($test_name:ident, $fn_tested:ident, $src_unit:ty, $dst_unit:ty) => {
            #[test]
            fn $test_name() {
                let mut src: Vec<$src_unit> = Vec::with_capacity(32);
                let mut dst: Vec<$dst_unit> = Vec::with_capacity(32);
                for i in 0..32 {
                    src.clear();
                    dst.clear();
                    dst.resize(32, 0);
                    for j in 0..32 {
                        let c = if i == j { 0xAA } else { j + 0x40 };
                        src.push(c as $src_unit);
                    }
                    match unsafe { $fn_tested(src.as_ptr(), dst.as_mut_ptr(), 32) } {
                        None => unreachable!("Should always find non-ASCII"),
                        Some((non_ascii, num_ascii)) => {
                            assert_eq!(non_ascii, 0xAA);
                            assert_eq!(num_ascii, i);
                            for j in 0..i {
                                assert_eq!(dst[j], (j + 0x40) as $dst_unit);
                            }
                        }
                    }
                }
            }
        };
    }

    test_ascii!(test_ascii_to_ascii, ascii_to_ascii, u8, u8);
    test_ascii!(test_ascii_to_basic_latin, ascii_to_basic_latin, u8, u16);
    test_ascii!(test_basic_latin_to_ascii, basic_latin_to_ascii, u16, u8);
}
