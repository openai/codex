// Copyright Mozilla Foundation. See the COPYRIGHT
// file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use any_all_workaround::all_mask16x8;
use any_all_workaround::all_mask8x16;
use any_all_workaround::any_mask16x8;
use any_all_workaround::any_mask8x16;
use core::simd::cmp::SimdPartialEq;
use core::simd::cmp::SimdPartialOrd;
use core::simd::mask16x8;
use core::simd::mask8x16;
use core::simd::simd_swizzle;
use core::simd::u16x8;
use core::simd::u8x16;
use core::simd::ToBytes;

// TODO: Migrate unaligned access to stdlib code if/when the RFC
// https://github.com/rust-lang/rfcs/pull/1725 is implemented.

/// Safety invariant: ptr must be valid for an unaligned read of 16 bytes
#[inline(always)]
pub unsafe fn load16_unaligned(ptr: *const u8) -> u8x16 {
    let mut simd = ::core::mem::MaybeUninit::<u8x16>::uninit();
    ::core::ptr::copy_nonoverlapping(ptr, simd.as_mut_ptr() as *mut u8, 16);
    // Safety: copied 16 bytes of initialized memory into this, it is now initialized
    simd.assume_init()
}

/// Safety invariant: ptr must be valid for an aligned-for-u8x16 read of 16 bytes
#[allow(dead_code)]
#[inline(always)]
pub unsafe fn load16_aligned(ptr: *const u8) -> u8x16 {
    *(ptr as *const u8x16)
}

/// Safety invariant: ptr must be valid for an unaligned store of 16 bytes
#[inline(always)]
pub unsafe fn store16_unaligned(ptr: *mut u8, s: u8x16) {
    ::core::ptr::copy_nonoverlapping(&s as *const u8x16 as *const u8, ptr, 16);
}

/// Safety invariant: ptr must be valid for an aligned-for-u8x16 store of 16 bytes
#[allow(dead_code)]
#[inline(always)]
pub unsafe fn store16_aligned(ptr: *mut u8, s: u8x16) {
    *(ptr as *mut u8x16) = s;
}

/// Safety invariant: ptr must be valid for an unaligned read of 16 bytes
#[inline(always)]
pub unsafe fn load8_unaligned(ptr: *const u16) -> u16x8 {
    let mut simd = ::core::mem::MaybeUninit::<u16x8>::uninit();
    ::core::ptr::copy_nonoverlapping(ptr as *const u8, simd.as_mut_ptr() as *mut u8, 16);
    // Safety: copied 16 bytes of initialized memory into this, it is now initialized
    simd.assume_init()
}

/// Safety invariant: ptr must be valid for an aligned-for-u16x8 read of 16 bytes
#[allow(dead_code)]
#[inline(always)]
pub unsafe fn load8_aligned(ptr: *const u16) -> u16x8 {
    *(ptr as *const u16x8)
}

/// Safety invariant: ptr must be valid for an unaligned store of 16 bytes
#[inline(always)]
pub unsafe fn store8_unaligned(ptr: *mut u16, s: u16x8) {
    ::core::ptr::copy_nonoverlapping(&s as *const u16x8 as *const u8, ptr as *mut u8, 16);
}

/// Safety invariant: ptr must be valid for an aligned-for-u16x8 store of 16 bytes
#[allow(dead_code)]
#[inline(always)]
pub unsafe fn store8_aligned(ptr: *mut u16, s: u16x8) {
    *(ptr as *mut u16x8) = s;
}

cfg_if! {
    if #[cfg(all(target_feature = "sse2", target_arch = "x86_64"))] {
        use core::arch::x86_64::__m128i;
        use core::arch::x86_64::_mm_movemask_epi8;
        use core::arch::x86_64::_mm_packus_epi16;
    } else if #[cfg(all(target_feature = "sse2", target_arch = "x86"))] {
        use core::arch::x86::__m128i;
        use core::arch::x86::_mm_movemask_epi8;
        use core::arch::x86::_mm_packus_epi16;
    } else if #[cfg(target_arch = "aarch64")]{
        use core::arch::aarch64::vmaxvq_u8;
        use core::arch::aarch64::vmaxvq_u16;
    } else {

    }
}

// #[inline(always)]
// fn simd_byte_swap_u8(s: u8x16) -> u8x16 {
//     unsafe {
//         shuffle!(s, s, [1, 0, 3, 2, 5, 4, 7, 6, 9, 8, 11, 10, 13, 12, 15, 14])
//     }
// }

// #[inline(always)]
// pub fn simd_byte_swap(s: u16x8) -> u16x8 {
//     to_u16_lanes(simd_byte_swap_u8(to_u8_lanes(s)))
// }

#[inline(always)]
pub fn simd_byte_swap(s: u16x8) -> u16x8 {
    let left = s << 8;
    let right = s >> 8;
    left | right
}

#[inline(always)]
pub fn to_u16_lanes(s: u8x16) -> u16x8 {
    u16x8::from_ne_bytes(s)
}

cfg_if! {
    if #[cfg(target_feature = "sse2")] {

        // Expose low-level mask instead of higher-level conclusion,
        // because the non-ASCII case would perform less well otherwise.
        // Safety-usable invariant: This returned value is whether each high bit is set
        #[inline(always)]
        pub fn mask_ascii(s: u8x16) -> i32 {
            unsafe {
                _mm_movemask_epi8(s.into())
            }
        }

    } else {

    }
}

cfg_if! {
    if #[cfg(target_feature = "sse2")] {
        #[inline(always)]
        pub fn simd_is_ascii(s: u8x16) -> bool {
            unsafe {
                // Safety: We have cfg()d the correct platform
                _mm_movemask_epi8(s.into()) == 0
            }
        }
    } else if #[cfg(target_arch = "aarch64")]{
        #[inline(always)]
        pub fn simd_is_ascii(s: u8x16) -> bool {
            unsafe {
                // Safety: We have cfg()d the correct platform
                vmaxvq_u8(s.into()) < 0x80
            }
        }
    } else {
        #[inline(always)]
        pub fn simd_is_ascii(s: u8x16) -> bool {
            // This optimizes better on ARM than
            // the lt formulation.
            let highest_ascii = u8x16::splat(0x7F);
            !any_mask8x16(s.simd_gt(highest_ascii))
        }
    }
}

cfg_if! {
    if #[cfg(target_feature = "sse2")] {
        #[inline(always)]
        pub fn simd_is_str_latin1(s: u8x16) -> bool {
            if simd_is_ascii(s) {
                return true;
            }
            let above_str_latin1 = u8x16::splat(0xC4);
            s.simd_lt(above_str_latin1).all()
        }
    } else if #[cfg(target_arch = "aarch64")]{
        #[inline(always)]
        pub fn simd_is_str_latin1(s: u8x16) -> bool {
            unsafe {
                // Safety: We have cfg()d the correct platform
                vmaxvq_u8(s.into()) < 0xC4
            }
        }
    } else {
        #[inline(always)]
        pub fn simd_is_str_latin1(s: u8x16) -> bool {
            let above_str_latin1 = u8x16::splat(0xC4);
            all_mask8x16(s.simd_lt(above_str_latin1))
        }
    }
}

cfg_if! {
    if #[cfg(target_arch = "aarch64")]{
        #[inline(always)]
        pub fn simd_is_basic_latin(s: u16x8) -> bool {
            unsafe {
                // Safety: We have cfg()d the correct platform
                vmaxvq_u16(s.into()) < 0x80
            }
        }

        #[inline(always)]
        pub fn simd_is_latin1(s: u16x8) -> bool {
            unsafe {
                // Safety: We have cfg()d the correct platform
                vmaxvq_u16(s.into()) < 0x100
            }
        }
    } else {
        #[inline(always)]
        pub fn simd_is_basic_latin(s: u16x8) -> bool {
            let above_ascii = u16x8::splat(0x80);
            all_mask16x8(s.simd_lt(above_ascii))
        }

        #[inline(always)]
        pub fn simd_is_latin1(s: u16x8) -> bool {
            // For some reason, on SSE2 this formulation
            // seems faster in this case while the above
            // function is better the other way round...
            let highest_latin1 = u16x8::splat(0xFF);
            !any_mask16x8(s.simd_gt(highest_latin1))
        }
    }
}

#[inline(always)]
pub fn contains_surrogates(s: u16x8) -> bool {
    let mask = u16x8::splat(0xF800);
    let surrogate_bits = u16x8::splat(0xD800);
    any_mask16x8((s & mask).simd_eq(surrogate_bits))
}

cfg_if! {
    if #[cfg(target_arch = "aarch64")]{
        macro_rules! aarch64_return_false_if_below_hebrew {
            ($s:ident) => ({
                unsafe {
                    // Safety: We have cfg()d the correct platform
                    if vmaxvq_u16($s.into()) < 0x0590 {
                        return false;
                    }
                }
            })
        }

        macro_rules! non_aarch64_return_false_if_all {
            ($s:ident) => ()
        }
    } else {
        macro_rules! aarch64_return_false_if_below_hebrew {
            ($s:ident) => ()
        }

        macro_rules! non_aarch64_return_false_if_all {
            ($s:ident) => ({
                if all_mask16x8($s) {
                    return false;
                }
            })
        }
    }
}

macro_rules! in_range16x8 {
    ($s:ident, $start:expr, $end:expr) => {{
        // SIMD sub is wrapping
        ($s - u16x8::splat($start)).simd_lt(u16x8::splat($end - $start))
    }};
}

#[inline(always)]
pub fn is_u16x8_bidi(s: u16x8) -> bool {
    // We try to first quickly refute the RTLness of the vector. If that
    // fails, we do the real RTL check, so in that case we end up wasting
    // the work for the up-front quick checks. Even the quick-check is
    // two-fold in order to return `false` ASAP if everything is below
    // Hebrew.

    aarch64_return_false_if_below_hebrew!(s);

    let below_hebrew = s.simd_lt(u16x8::splat(0x0590));

    non_aarch64_return_false_if_all!(below_hebrew);

    if all_mask16x8(
        below_hebrew | in_range16x8!(s, 0x0900, 0x200F) | in_range16x8!(s, 0x2068, 0xD802),
    ) {
        return false;
    }

    // Quick refutation failed. Let's do the full check.

    any_mask16x8(
        (in_range16x8!(s, 0x0590, 0x0900)
            | in_range16x8!(s, 0xFB1D, 0xFE00)
            | in_range16x8!(s, 0xFE70, 0xFEFF)
            | in_range16x8!(s, 0xD802, 0xD804)
            | in_range16x8!(s, 0xD83A, 0xD83C)
            | s.simd_eq(u16x8::splat(0x200F))
            | s.simd_eq(u16x8::splat(0x202B))
            | s.simd_eq(u16x8::splat(0x202E))
            | s.simd_eq(u16x8::splat(0x2067))),
    )
}

#[inline(always)]
pub fn simd_unpack(s: u8x16) -> (u16x8, u16x8) {
    let first: u8x16 = simd_swizzle!(
        s,
        u8x16::splat(0),
        [0, 16, 1, 17, 2, 18, 3, 19, 4, 20, 5, 21, 6, 22, 7, 23]
    );
    let second: u8x16 = simd_swizzle!(
        s,
        u8x16::splat(0),
        [8, 24, 9, 25, 10, 26, 11, 27, 12, 28, 13, 29, 14, 30, 15, 31]
    );
    (u16x8::from_ne_bytes(first), u16x8::from_ne_bytes(second))
}

cfg_if! {
    if #[cfg(target_feature = "sse2")] {
        #[inline(always)]
        pub fn simd_pack(a: u16x8, b: u16x8) -> u8x16 {
            unsafe {
                // Safety: We have cfg()d the correct platform
                _mm_packus_epi16(a.into(), b.into()).into()
            }
        }
    } else {
        #[inline(always)]
        pub fn simd_pack(a: u16x8, b: u16x8) -> u8x16 {
            let first: u8x16 = a.to_ne_bytes();
            let second: u8x16 = b.to_ne_bytes();
            simd_swizzle!(
                first,
                second,
                [0, 2, 4, 6, 8, 10, 12, 14, 16, 18, 20, 22, 24, 26, 28, 30]
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    #[test]
    fn test_unpack() {
        let ascii: [u8; 16] = [
            0x61, 0x62, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68, 0x69, 0x70, 0x71, 0x72, 0x73, 0x74,
            0x75, 0x76,
        ];
        let basic_latin: [u16; 16] = [
            0x61, 0x62, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68, 0x69, 0x70, 0x71, 0x72, 0x73, 0x74,
            0x75, 0x76,
        ];
        let simd = unsafe { load16_unaligned(ascii.as_ptr()) };
        let mut vec = Vec::with_capacity(16);
        vec.resize(16, 0u16);
        let (first, second) = simd_unpack(simd);
        let ptr = vec.as_mut_ptr();
        unsafe {
            store8_unaligned(ptr, first);
            store8_unaligned(ptr.add(8), second);
        }
        assert_eq!(&vec[..], &basic_latin[..]);
    }

    #[test]
    fn test_simd_is_basic_latin_success() {
        let ascii: [u8; 16] = [
            0x61, 0x62, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68, 0x69, 0x70, 0x71, 0x72, 0x73, 0x74,
            0x75, 0x76,
        ];
        let basic_latin: [u16; 16] = [
            0x61, 0x62, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68, 0x69, 0x70, 0x71, 0x72, 0x73, 0x74,
            0x75, 0x76,
        ];
        let first = unsafe { load8_unaligned(basic_latin.as_ptr()) };
        let second = unsafe { load8_unaligned(basic_latin.as_ptr().add(8)) };
        let mut vec = Vec::with_capacity(16);
        vec.resize(16, 0u8);
        let ptr = vec.as_mut_ptr();
        assert!(simd_is_basic_latin(first | second));
        unsafe {
            store16_unaligned(ptr, simd_pack(first, second));
        }
        assert_eq!(&vec[..], &ascii[..]);
    }

    #[test]
    fn test_simd_is_basic_latin_c0() {
        let input: [u16; 16] = [
            0x61, 0x62, 0x63, 0x81, 0x65, 0x66, 0x67, 0x68, 0x69, 0x70, 0x71, 0x72, 0x73, 0x74,
            0x75, 0x76,
        ];
        let first = unsafe { load8_unaligned(input.as_ptr()) };
        let second = unsafe { load8_unaligned(input.as_ptr().add(8)) };
        assert!(!simd_is_basic_latin(first | second));
    }

    #[test]
    fn test_simd_is_basic_latin_0fff() {
        let input: [u16; 16] = [
            0x61, 0x62, 0x63, 0x0FFF, 0x65, 0x66, 0x67, 0x68, 0x69, 0x70, 0x71, 0x72, 0x73, 0x74,
            0x75, 0x76,
        ];
        let first = unsafe { load8_unaligned(input.as_ptr()) };
        let second = unsafe { load8_unaligned(input.as_ptr().add(8)) };
        assert!(!simd_is_basic_latin(first | second));
    }

    #[test]
    fn test_simd_is_basic_latin_ffff() {
        let input: [u16; 16] = [
            0x61, 0x62, 0x63, 0xFFFF, 0x65, 0x66, 0x67, 0x68, 0x69, 0x70, 0x71, 0x72, 0x73, 0x74,
            0x75, 0x76,
        ];
        let first = unsafe { load8_unaligned(input.as_ptr()) };
        let second = unsafe { load8_unaligned(input.as_ptr().add(8)) };
        assert!(!simd_is_basic_latin(first | second));
    }

    #[test]
    fn test_simd_is_ascii_success() {
        let ascii: [u8; 16] = [
            0x61, 0x62, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68, 0x69, 0x70, 0x71, 0x72, 0x73, 0x74,
            0x75, 0x76,
        ];
        let simd = unsafe { load16_unaligned(ascii.as_ptr()) };
        assert!(simd_is_ascii(simd));
    }

    #[test]
    fn test_simd_is_ascii_failure() {
        let input: [u8; 16] = [
            0x61, 0x62, 0x63, 0x64, 0x81, 0x66, 0x67, 0x68, 0x69, 0x70, 0x71, 0x72, 0x73, 0x74,
            0x75, 0x76,
        ];
        let simd = unsafe { load16_unaligned(input.as_ptr()) };
        assert!(!simd_is_ascii(simd));
    }

    #[cfg(target_feature = "sse2")]
    #[test]
    fn test_check_ascii() {
        let input: [u8; 16] = [
            0x61, 0x62, 0x63, 0x64, 0x81, 0x66, 0x67, 0x68, 0x69, 0x70, 0x71, 0x72, 0x73, 0x74,
            0x75, 0x76,
        ];
        let simd = unsafe { load16_unaligned(input.as_ptr()) };
        let mask = mask_ascii(simd);
        assert_ne!(mask, 0);
        assert_eq!(mask.trailing_zeros(), 4);
    }

    #[test]
    fn test_alu() {
        let input: [u8; 16] = [
            0x61, 0x62, 0x63, 0x64, 0x81, 0x66, 0x67, 0x68, 0x69, 0x70, 0x71, 0x72, 0x73, 0x74,
            0x75, 0x76,
        ];
        let mut alu = 0u64;
        unsafe {
            ::core::ptr::copy_nonoverlapping(input.as_ptr(), &mut alu as *mut u64 as *mut u8, 8);
        }
        let masked = alu & 0x8080808080808080;
        assert_eq!(masked.trailing_zeros(), 39);
    }
}
