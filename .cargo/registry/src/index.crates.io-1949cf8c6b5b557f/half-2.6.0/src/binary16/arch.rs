#![allow(dead_code, unused_imports)]
use crate::leading_zeros::leading_zeros_u16;
use core::mem;

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
mod x86;

#[cfg(target_arch = "aarch64")]
mod aarch64;

macro_rules! convert_fn {
    (if x86_feature("f16c") { $f16c:expr }
    else if aarch64_feature("fp16") { $aarch64:expr }
    else { $fallback:expr }) => {
        cfg_if::cfg_if! {
            // Use intrinsics directly when a compile target or using no_std
            if #[cfg(all(
                any(target_arch = "x86", target_arch = "x86_64"),
                target_feature = "f16c"
            ))] {
                $f16c
            }
            else if #[cfg(all(
                target_arch = "aarch64",
                target_feature = "fp16"
            ))] {
                $aarch64

            }

            // Use CPU feature detection if using std
            else if #[cfg(all(
                feature = "std",
                any(target_arch = "x86", target_arch = "x86_64")
            ))] {
                use std::arch::is_x86_feature_detected;
                if is_x86_feature_detected!("f16c") {
                    $f16c
                } else {
                    $fallback
                }
            }
            else if #[cfg(all(
                feature = "std",
                target_arch = "aarch64",
            ))] {
                use std::arch::is_aarch64_feature_detected;
                if is_aarch64_feature_detected!("fp16") {
                    $aarch64
                } else {
                    $fallback
                }
            }

            // Fallback to software
            else {
                $fallback
            }
        }
    };
}

#[inline]
pub(crate) fn f32_to_f16(f: f32) -> u16 {
    convert_fn! {
        if x86_feature("f16c") {
            unsafe { x86::f32_to_f16_x86_f16c(f) }
        } else if aarch64_feature("fp16") {
            unsafe { aarch64::f32_to_f16_fp16(f) }
        } else {
            f32_to_f16_fallback(f)
        }
    }
}

#[inline]
pub(crate) fn f64_to_f16(f: f64) -> u16 {
    convert_fn! {
        if x86_feature("f16c") {
            unsafe { x86::f32_to_f16_x86_f16c(f as f32) }
        } else if aarch64_feature("fp16") {
            unsafe { aarch64::f64_to_f16_fp16(f) }
        } else {
            f64_to_f16_fallback(f)
        }
    }
}

#[inline]
pub(crate) fn f16_to_f32(i: u16) -> f32 {
    convert_fn! {
        if x86_feature("f16c") {
            unsafe { x86::f16_to_f32_x86_f16c(i) }
        } else if aarch64_feature("fp16") {
            unsafe { aarch64::f16_to_f32_fp16(i) }
        } else {
            f16_to_f32_fallback(i)
        }
    }
}

#[inline]
pub(crate) fn f16_to_f64(i: u16) -> f64 {
    convert_fn! {
        if x86_feature("f16c") {
            unsafe { x86::f16_to_f32_x86_f16c(i) as f64 }
        } else if aarch64_feature("fp16") {
            unsafe { aarch64::f16_to_f64_fp16(i) }
        } else {
            f16_to_f64_fallback(i)
        }
    }
}

#[inline]
pub(crate) fn f32x4_to_f16x4(f: &[f32; 4]) -> [u16; 4] {
    convert_fn! {
        if x86_feature("f16c") {
            unsafe { x86::f32x4_to_f16x4_x86_f16c(f) }
        } else if aarch64_feature("fp16") {
            unsafe { aarch64::f32x4_to_f16x4_fp16(f) }
        } else {
            f32x4_to_f16x4_fallback(f)
        }
    }
}

#[inline]
pub(crate) fn f16x4_to_f32x4(i: &[u16; 4]) -> [f32; 4] {
    convert_fn! {
        if x86_feature("f16c") {
            unsafe { x86::f16x4_to_f32x4_x86_f16c(i) }
        } else if aarch64_feature("fp16") {
            unsafe { aarch64::f16x4_to_f32x4_fp16(i) }
        } else {
            f16x4_to_f32x4_fallback(i)
        }
    }
}

#[inline]
pub(crate) fn f64x4_to_f16x4(f: &[f64; 4]) -> [u16; 4] {
    convert_fn! {
        if x86_feature("f16c") {
            unsafe { x86::f64x4_to_f16x4_x86_f16c(f) }
        } else if aarch64_feature("fp16") {
            unsafe { aarch64::f64x4_to_f16x4_fp16(f) }
        } else {
            f64x4_to_f16x4_fallback(f)
        }
    }
}

#[inline]
pub(crate) fn f16x4_to_f64x4(i: &[u16; 4]) -> [f64; 4] {
    convert_fn! {
        if x86_feature("f16c") {
            unsafe { x86::f16x4_to_f64x4_x86_f16c(i) }
        } else if aarch64_feature("fp16") {
            unsafe { aarch64::f16x4_to_f64x4_fp16(i) }
        } else {
            f16x4_to_f64x4_fallback(i)
        }
    }
}

#[inline]
pub(crate) fn f32x8_to_f16x8(f: &[f32; 8]) -> [u16; 8] {
    convert_fn! {
        if x86_feature("f16c") {
            unsafe { x86::f32x8_to_f16x8_x86_f16c(f) }
        } else if aarch64_feature("fp16") {
            {
                let mut result = [0u16; 8];
                convert_chunked_slice_4(f.as_slice(), result.as_mut_slice(),
                    aarch64::f32x4_to_f16x4_fp16);
                result
            }
        } else {
            f32x8_to_f16x8_fallback(f)
        }
    }
}

#[inline]
pub(crate) fn f16x8_to_f32x8(i: &[u16; 8]) -> [f32; 8] {
    convert_fn! {
        if x86_feature("f16c") {
            unsafe { x86::f16x8_to_f32x8_x86_f16c(i) }
        } else if aarch64_feature("fp16") {
            {
                let mut result = [0f32; 8];
                convert_chunked_slice_4(i.as_slice(), result.as_mut_slice(),
                    aarch64::f16x4_to_f32x4_fp16);
                result
            }
        } else {
            f16x8_to_f32x8_fallback(i)
        }
    }
}

#[inline]
pub(crate) fn f64x8_to_f16x8(f: &[f64; 8]) -> [u16; 8] {
    convert_fn! {
        if x86_feature("f16c") {
            unsafe { x86::f64x8_to_f16x8_x86_f16c(f) }
        } else if aarch64_feature("fp16") {
            {
                let mut result = [0u16; 8];
                convert_chunked_slice_4(f.as_slice(), result.as_mut_slice(),
                    aarch64::f64x4_to_f16x4_fp16);
                result
            }
        } else {
            f64x8_to_f16x8_fallback(f)
        }
    }
}

#[inline]
pub(crate) fn f16x8_to_f64x8(i: &[u16; 8]) -> [f64; 8] {
    convert_fn! {
        if x86_feature("f16c") {
            unsafe { x86::f16x8_to_f64x8_x86_f16c(i) }
        } else if aarch64_feature("fp16") {
            {
                let mut result = [0f64; 8];
                convert_chunked_slice_4(i.as_slice(), result.as_mut_slice(),
                    aarch64::f16x4_to_f64x4_fp16);
                result
            }
        } else {
            f16x8_to_f64x8_fallback(i)
        }
    }
}

#[inline]
pub(crate) fn f32_to_f16_slice(src: &[f32], dst: &mut [u16]) {
    convert_fn! {
        if x86_feature("f16c") {
            convert_chunked_slice_8(src, dst, x86::f32x8_to_f16x8_x86_f16c,
                x86::f32x4_to_f16x4_x86_f16c)
        } else if aarch64_feature("fp16") {
            convert_chunked_slice_4(src, dst, aarch64::f32x4_to_f16x4_fp16)
        } else {
            slice_fallback(src, dst, f32_to_f16_fallback)
        }
    }
}

#[inline]
pub(crate) fn f16_to_f32_slice(src: &[u16], dst: &mut [f32]) {
    convert_fn! {
        if x86_feature("f16c") {
            convert_chunked_slice_8(src, dst, x86::f16x8_to_f32x8_x86_f16c,
                x86::f16x4_to_f32x4_x86_f16c)
        } else if aarch64_feature("fp16") {
            convert_chunked_slice_4(src, dst, aarch64::f16x4_to_f32x4_fp16)
        } else {
            slice_fallback(src, dst, f16_to_f32_fallback)
        }
    }
}

#[inline]
pub(crate) fn f64_to_f16_slice(src: &[f64], dst: &mut [u16]) {
    convert_fn! {
        if x86_feature("f16c") {
            convert_chunked_slice_8(src, dst, x86::f64x8_to_f16x8_x86_f16c,
                x86::f64x4_to_f16x4_x86_f16c)
        } else if aarch64_feature("fp16") {
            convert_chunked_slice_4(src, dst, aarch64::f64x4_to_f16x4_fp16)
        } else {
            slice_fallback(src, dst, f64_to_f16_fallback)
        }
    }
}

#[inline]
pub(crate) fn f16_to_f64_slice(src: &[u16], dst: &mut [f64]) {
    convert_fn! {
        if x86_feature("f16c") {
            convert_chunked_slice_8(src, dst, x86::f16x8_to_f64x8_x86_f16c,
                x86::f16x4_to_f64x4_x86_f16c)
        } else if aarch64_feature("fp16") {
            convert_chunked_slice_4(src, dst, aarch64::f16x4_to_f64x4_fp16)
        } else {
            slice_fallback(src, dst, f16_to_f64_fallback)
        }
    }
}

macro_rules! math_fn {
    (if aarch64_feature("fp16") { $aarch64:expr }
    else { $fallback:expr }) => {
        cfg_if::cfg_if! {
            // Use intrinsics directly when a compile target or using no_std
            if #[cfg(all(
                target_arch = "aarch64",
                target_feature = "fp16"
            ))] {
                $aarch64
            }

            // Use CPU feature detection if using std
            else if #[cfg(all(
                feature = "std",
                target_arch = "aarch64",
                not(target_feature = "fp16")
            ))] {
                use std::arch::is_aarch64_feature_detected;
                if is_aarch64_feature_detected!("fp16") {
                    $aarch64
                } else {
                    $fallback
                }
            }

            // Fallback to software
            else {
                $fallback
            }
        }
    };
}

#[inline]
pub(crate) fn add_f16(a: u16, b: u16) -> u16 {
    math_fn! {
        if aarch64_feature("fp16") {
            unsafe { aarch64::add_f16_fp16(a, b) }
        } else {
            add_f16_fallback(a, b)
        }
    }
}

#[inline]
pub(crate) fn subtract_f16(a: u16, b: u16) -> u16 {
    math_fn! {
        if aarch64_feature("fp16") {
            unsafe { aarch64::subtract_f16_fp16(a, b) }
        } else {
            subtract_f16_fallback(a, b)
        }
    }
}

#[inline]
pub(crate) fn multiply_f16(a: u16, b: u16) -> u16 {
    math_fn! {
        if aarch64_feature("fp16") {
            unsafe { aarch64::multiply_f16_fp16(a, b) }
        } else {
            multiply_f16_fallback(a, b)
        }
    }
}

#[inline]
pub(crate) fn divide_f16(a: u16, b: u16) -> u16 {
    math_fn! {
        if aarch64_feature("fp16") {
            unsafe { aarch64::divide_f16_fp16(a, b) }
        } else {
            divide_f16_fallback(a, b)
        }
    }
}

#[inline]
pub(crate) fn remainder_f16(a: u16, b: u16) -> u16 {
    remainder_f16_fallback(a, b)
}

#[inline]
pub(crate) fn product_f16<I: Iterator<Item = u16>>(iter: I) -> u16 {
    math_fn! {
        if aarch64_feature("fp16") {
            iter.fold(0, |acc, x| unsafe { aarch64::multiply_f16_fp16(acc, x) })
        } else {
            product_f16_fallback(iter)
        }
    }
}

#[inline]
pub(crate) fn sum_f16<I: Iterator<Item = u16>>(iter: I) -> u16 {
    math_fn! {
        if aarch64_feature("fp16") {
            iter.fold(0, |acc, x| unsafe { aarch64::add_f16_fp16(acc, x) })
        } else {
            sum_f16_fallback(iter)
        }
    }
}

/// Chunks sliced into x8 or x4 arrays
#[inline]
fn convert_chunked_slice_8<S: Copy + Default, D: Copy>(
    src: &[S],
    dst: &mut [D],
    fn8: unsafe fn(&[S; 8]) -> [D; 8],
    fn4: unsafe fn(&[S; 4]) -> [D; 4],
) {
    assert_eq!(src.len(), dst.len());

    // TODO: Can be further optimized with array_chunks when it becomes stabilized

    let src_chunks = src.chunks_exact(8);
    let mut dst_chunks = dst.chunks_exact_mut(8);
    let src_remainder = src_chunks.remainder();
    for (s, d) in src_chunks.zip(&mut dst_chunks) {
        let chunk: &[S; 8] = s.try_into().unwrap();
        d.copy_from_slice(unsafe { &fn8(chunk) });
    }

    // Process remainder
    if src_remainder.len() > 4 {
        let mut buf: [S; 8] = Default::default();
        buf[..src_remainder.len()].copy_from_slice(src_remainder);
        let vec = unsafe { fn8(&buf) };
        let dst_remainder = dst_chunks.into_remainder();
        dst_remainder.copy_from_slice(&vec[..dst_remainder.len()]);
    } else if !src_remainder.is_empty() {
        let mut buf: [S; 4] = Default::default();
        buf[..src_remainder.len()].copy_from_slice(src_remainder);
        let vec = unsafe { fn4(&buf) };
        let dst_remainder = dst_chunks.into_remainder();
        dst_remainder.copy_from_slice(&vec[..dst_remainder.len()]);
    }
}

/// Chunks sliced into x4 arrays
#[inline]
fn convert_chunked_slice_4<S: Copy + Default, D: Copy>(
    src: &[S],
    dst: &mut [D],
    f: unsafe fn(&[S; 4]) -> [D; 4],
) {
    assert_eq!(src.len(), dst.len());

    // TODO: Can be further optimized with array_chunks when it becomes stabilized

    let src_chunks = src.chunks_exact(4);
    let mut dst_chunks = dst.chunks_exact_mut(4);
    let src_remainder = src_chunks.remainder();
    for (s, d) in src_chunks.zip(&mut dst_chunks) {
        let chunk: &[S; 4] = s.try_into().unwrap();
        d.copy_from_slice(unsafe { &f(chunk) });
    }

    // Process remainder
    if !src_remainder.is_empty() {
        let mut buf: [S; 4] = Default::default();
        buf[..src_remainder.len()].copy_from_slice(src_remainder);
        let vec = unsafe { f(&buf) };
        let dst_remainder = dst_chunks.into_remainder();
        dst_remainder.copy_from_slice(&vec[..dst_remainder.len()]);
    }
}

/////////////// Fallbacks ////////////////

// In the below functions, round to nearest, with ties to even.
// Let us call the most significant bit that will be shifted out the round_bit.
//
// Round up if either
//  a) Removed part > tie.
//     (mantissa & round_bit) != 0 && (mantissa & (round_bit - 1)) != 0
//  b) Removed part == tie, and retained part is odd.
//     (mantissa & round_bit) != 0 && (mantissa & (2 * round_bit)) != 0
// (If removed part == tie and retained part is even, do not round up.)
// These two conditions can be combined into one:
//     (mantissa & round_bit) != 0 && (mantissa & ((round_bit - 1) | (2 * round_bit))) != 0
// which can be simplified into
//     (mantissa & round_bit) != 0 && (mantissa & (3 * round_bit - 1)) != 0

#[inline]
pub(crate) const fn f32_to_f16_fallback(value: f32) -> u16 {
    // TODO: Replace mem::transmute with to_bits() once to_bits is const-stabilized
    // Convert to raw bytes
    let x: u32 = unsafe { mem::transmute::<f32, u32>(value) };

    // Extract IEEE754 components
    let sign = x & 0x8000_0000u32;
    let exp = x & 0x7F80_0000u32;
    let man = x & 0x007F_FFFFu32;

    // Check for all exponent bits being set, which is Infinity or NaN
    if exp == 0x7F80_0000u32 {
        // Set mantissa MSB for NaN (and also keep shifted mantissa bits)
        let nan_bit = if man == 0 { 0 } else { 0x0200u32 };
        return ((sign >> 16) | 0x7C00u32 | nan_bit | (man >> 13)) as u16;
    }

    // The number is normalized, start assembling half precision version
    let half_sign = sign >> 16;
    // Unbias the exponent, then bias for half precision
    let unbiased_exp = ((exp >> 23) as i32) - 127;
    let half_exp = unbiased_exp + 15;

    // Check for exponent overflow, return +infinity
    if half_exp >= 0x1F {
        return (half_sign | 0x7C00u32) as u16;
    }

    // Check for underflow
    if half_exp <= 0 {
        // Check mantissa for what we can do
        if 14 - half_exp > 24 {
            // No rounding possibility, so this is a full underflow, return signed zero
            return half_sign as u16;
        }
        // Don't forget about hidden leading mantissa bit when assembling mantissa
        let man = man | 0x0080_0000u32;
        let mut half_man = man >> (14 - half_exp);
        // Check for rounding (see comment above functions)
        let round_bit = 1 << (13 - half_exp);
        if (man & round_bit) != 0 && (man & (3 * round_bit - 1)) != 0 {
            half_man += 1;
        }
        // No exponent for subnormals
        return (half_sign | half_man) as u16;
    }

    // Rebias the exponent
    let half_exp = (half_exp as u32) << 10;
    let half_man = man >> 13;
    // Check for rounding (see comment above functions)
    let round_bit = 0x0000_1000u32;
    if (man & round_bit) != 0 && (man & (3 * round_bit - 1)) != 0 {
        // Round it
        ((half_sign | half_exp | half_man) + 1) as u16
    } else {
        (half_sign | half_exp | half_man) as u16
    }
}

#[inline]
pub(crate) const fn f64_to_f16_fallback(value: f64) -> u16 {
    // Convert to raw bytes, truncating the last 32-bits of mantissa; that precision will always
    // be lost on half-precision.
    // TODO: Replace mem::transmute with to_bits() once to_bits is const-stabilized
    let val: u64 = unsafe { mem::transmute::<f64, u64>(value) };
    let x = (val >> 32) as u32;

    // Extract IEEE754 components
    let sign = x & 0x8000_0000u32;
    let exp = x & 0x7FF0_0000u32;
    let man = x & 0x000F_FFFFu32;

    // Check for all exponent bits being set, which is Infinity or NaN
    if exp == 0x7FF0_0000u32 {
        // Set mantissa MSB for NaN (and also keep shifted mantissa bits).
        // We also have to check the last 32 bits.
        let nan_bit = if man == 0 && (val as u32 == 0) {
            0
        } else {
            0x0200u32
        };
        return ((sign >> 16) | 0x7C00u32 | nan_bit | (man >> 10)) as u16;
    }

    // The number is normalized, start assembling half precision version
    let half_sign = sign >> 16;
    // Unbias the exponent, then bias for half precision
    let unbiased_exp = ((exp >> 20) as i64) - 1023;
    let half_exp = unbiased_exp + 15;

    // Check for exponent overflow, return +infinity
    if half_exp >= 0x1F {
        return (half_sign | 0x7C00u32) as u16;
    }

    // Check for underflow
    if half_exp <= 0 {
        // Check mantissa for what we can do
        if 10 - half_exp > 21 {
            // No rounding possibility, so this is a full underflow, return signed zero
            return half_sign as u16;
        }
        // Don't forget about hidden leading mantissa bit when assembling mantissa
        let man = man | 0x0010_0000u32;
        let mut half_man = man >> (11 - half_exp);
        // Check for rounding (see comment above functions)
        let round_bit = 1 << (10 - half_exp);
        if (man & round_bit) != 0 && (man & (3 * round_bit - 1)) != 0 {
            half_man += 1;
        }
        // No exponent for subnormals
        return (half_sign | half_man) as u16;
    }

    // Rebias the exponent
    let half_exp = (half_exp as u32) << 10;
    let half_man = man >> 10;
    // Check for rounding (see comment above functions)
    let round_bit = 0x0000_0200u32;
    if (man & round_bit) != 0 && (man & (3 * round_bit - 1)) != 0 {
        // Round it
        ((half_sign | half_exp | half_man) + 1) as u16
    } else {
        (half_sign | half_exp | half_man) as u16
    }
}

#[inline]
pub(crate) const fn f16_to_f32_fallback(i: u16) -> f32 {
    // Check for signed zero
    // TODO: Replace mem::transmute with from_bits() once from_bits is const-stabilized
    if i & 0x7FFFu16 == 0 {
        return unsafe { mem::transmute::<u32, f32>((i as u32) << 16) };
    }

    let half_sign = (i & 0x8000u16) as u32;
    let half_exp = (i & 0x7C00u16) as u32;
    let half_man = (i & 0x03FFu16) as u32;

    // Check for an infinity or NaN when all exponent bits set
    if half_exp == 0x7C00u32 {
        // Check for signed infinity if mantissa is zero
        if half_man == 0 {
            return unsafe { mem::transmute::<u32, f32>((half_sign << 16) | 0x7F80_0000u32) };
        } else {
            // NaN, keep current mantissa but also set most significiant mantissa bit
            return unsafe {
                mem::transmute::<u32, f32>((half_sign << 16) | 0x7FC0_0000u32 | (half_man << 13))
            };
        }
    }

    // Calculate single-precision components with adjusted exponent
    let sign = half_sign << 16;
    // Unbias exponent
    let unbiased_exp = ((half_exp as i32) >> 10) - 15;

    // Check for subnormals, which will be normalized by adjusting exponent
    if half_exp == 0 {
        // Calculate how much to adjust the exponent by
        let e = leading_zeros_u16(half_man as u16) - 6;

        // Rebias and adjust exponent
        let exp = (127 - 15 - e) << 23;
        let man = (half_man << (14 + e)) & 0x7F_FF_FFu32;
        return unsafe { mem::transmute::<u32, f32>(sign | exp | man) };
    }

    // Rebias exponent for a normalized normal
    let exp = ((unbiased_exp + 127) as u32) << 23;
    let man = (half_man & 0x03FFu32) << 13;
    unsafe { mem::transmute::<u32, f32>(sign | exp | man) }
}

#[inline]
pub(crate) const fn f16_to_f64_fallback(i: u16) -> f64 {
    // Check for signed zero
    // TODO: Replace mem::transmute with from_bits() once from_bits is const-stabilized
    if i & 0x7FFFu16 == 0 {
        return unsafe { mem::transmute::<u64, f64>((i as u64) << 48) };
    }

    let half_sign = (i & 0x8000u16) as u64;
    let half_exp = (i & 0x7C00u16) as u64;
    let half_man = (i & 0x03FFu16) as u64;

    // Check for an infinity or NaN when all exponent bits set
    if half_exp == 0x7C00u64 {
        // Check for signed infinity if mantissa is zero
        if half_man == 0 {
            return unsafe {
                mem::transmute::<u64, f64>((half_sign << 48) | 0x7FF0_0000_0000_0000u64)
            };
        } else {
            // NaN, keep current mantissa but also set most significiant mantissa bit
            return unsafe {
                mem::transmute::<u64, f64>(
                    (half_sign << 48) | 0x7FF8_0000_0000_0000u64 | (half_man << 42),
                )
            };
        }
    }

    // Calculate double-precision components with adjusted exponent
    let sign = half_sign << 48;
    // Unbias exponent
    let unbiased_exp = ((half_exp as i64) >> 10) - 15;

    // Check for subnormals, which will be normalized by adjusting exponent
    if half_exp == 0 {
        // Calculate how much to adjust the exponent by
        let e = leading_zeros_u16(half_man as u16) - 6;

        // Rebias and adjust exponent
        let exp = ((1023 - 15 - e) as u64) << 52;
        let man = (half_man << (43 + e)) & 0xF_FFFF_FFFF_FFFFu64;
        return unsafe { mem::transmute::<u64, f64>(sign | exp | man) };
    }

    // Rebias exponent for a normalized normal
    let exp = ((unbiased_exp + 1023) as u64) << 52;
    let man = (half_man & 0x03FFu64) << 42;
    unsafe { mem::transmute::<u64, f64>(sign | exp | man) }
}

#[inline]
fn f16x4_to_f32x4_fallback(v: &[u16; 4]) -> [f32; 4] {
    [
        f16_to_f32_fallback(v[0]),
        f16_to_f32_fallback(v[1]),
        f16_to_f32_fallback(v[2]),
        f16_to_f32_fallback(v[3]),
    ]
}

#[inline]
fn f32x4_to_f16x4_fallback(v: &[f32; 4]) -> [u16; 4] {
    [
        f32_to_f16_fallback(v[0]),
        f32_to_f16_fallback(v[1]),
        f32_to_f16_fallback(v[2]),
        f32_to_f16_fallback(v[3]),
    ]
}

#[inline]
fn f16x4_to_f64x4_fallback(v: &[u16; 4]) -> [f64; 4] {
    [
        f16_to_f64_fallback(v[0]),
        f16_to_f64_fallback(v[1]),
        f16_to_f64_fallback(v[2]),
        f16_to_f64_fallback(v[3]),
    ]
}

#[inline]
fn f64x4_to_f16x4_fallback(v: &[f64; 4]) -> [u16; 4] {
    [
        f64_to_f16_fallback(v[0]),
        f64_to_f16_fallback(v[1]),
        f64_to_f16_fallback(v[2]),
        f64_to_f16_fallback(v[3]),
    ]
}

#[inline]
fn f16x8_to_f32x8_fallback(v: &[u16; 8]) -> [f32; 8] {
    [
        f16_to_f32_fallback(v[0]),
        f16_to_f32_fallback(v[1]),
        f16_to_f32_fallback(v[2]),
        f16_to_f32_fallback(v[3]),
        f16_to_f32_fallback(v[4]),
        f16_to_f32_fallback(v[5]),
        f16_to_f32_fallback(v[6]),
        f16_to_f32_fallback(v[7]),
    ]
}

#[inline]
fn f32x8_to_f16x8_fallback(v: &[f32; 8]) -> [u16; 8] {
    [
        f32_to_f16_fallback(v[0]),
        f32_to_f16_fallback(v[1]),
        f32_to_f16_fallback(v[2]),
        f32_to_f16_fallback(v[3]),
        f32_to_f16_fallback(v[4]),
        f32_to_f16_fallback(v[5]),
        f32_to_f16_fallback(v[6]),
        f32_to_f16_fallback(v[7]),
    ]
}

#[inline]
fn f16x8_to_f64x8_fallback(v: &[u16; 8]) -> [f64; 8] {
    [
        f16_to_f64_fallback(v[0]),
        f16_to_f64_fallback(v[1]),
        f16_to_f64_fallback(v[2]),
        f16_to_f64_fallback(v[3]),
        f16_to_f64_fallback(v[4]),
        f16_to_f64_fallback(v[5]),
        f16_to_f64_fallback(v[6]),
        f16_to_f64_fallback(v[7]),
    ]
}

#[inline]
fn f64x8_to_f16x8_fallback(v: &[f64; 8]) -> [u16; 8] {
    [
        f64_to_f16_fallback(v[0]),
        f64_to_f16_fallback(v[1]),
        f64_to_f16_fallback(v[2]),
        f64_to_f16_fallback(v[3]),
        f64_to_f16_fallback(v[4]),
        f64_to_f16_fallback(v[5]),
        f64_to_f16_fallback(v[6]),
        f64_to_f16_fallback(v[7]),
    ]
}

#[inline]
fn slice_fallback<S: Copy, D>(src: &[S], dst: &mut [D], f: fn(S) -> D) {
    assert_eq!(src.len(), dst.len());
    for (s, d) in src.iter().copied().zip(dst.iter_mut()) {
        *d = f(s);
    }
}

#[inline]
fn add_f16_fallback(a: u16, b: u16) -> u16 {
    f32_to_f16(f16_to_f32(a) + f16_to_f32(b))
}

#[inline]
fn subtract_f16_fallback(a: u16, b: u16) -> u16 {
    f32_to_f16(f16_to_f32(a) - f16_to_f32(b))
}

#[inline]
fn multiply_f16_fallback(a: u16, b: u16) -> u16 {
    f32_to_f16(f16_to_f32(a) * f16_to_f32(b))
}

#[inline]
fn divide_f16_fallback(a: u16, b: u16) -> u16 {
    f32_to_f16(f16_to_f32(a) / f16_to_f32(b))
}

#[inline]
fn remainder_f16_fallback(a: u16, b: u16) -> u16 {
    f32_to_f16(f16_to_f32(a) % f16_to_f32(b))
}

#[inline]
fn product_f16_fallback<I: Iterator<Item = u16>>(iter: I) -> u16 {
    f32_to_f16(iter.map(f16_to_f32).product())
}

#[inline]
fn sum_f16_fallback<I: Iterator<Item = u16>>(iter: I) -> u16 {
    f32_to_f16(iter.map(f16_to_f32).sum())
}

// TODO SIMD arithmetic
