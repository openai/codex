/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

#![cfg(target_arch = "aarch64")]
//! AVX optimised IDCT.
//!
//! Okay not thaat optimised.
//!
//!
//! # The implementation
//! The implementation is neatly broken down into two operations.
//!
//! 1. Test for zeroes
//! > There is a shortcut method for idct  where when all AC values are zero, we can get the answer really quickly.
//!  by scaling the 1/8th of the DCT coefficient of the block to the whole block and level shifting.
//!
//! 2. If above fails, we proceed to carry out IDCT as a two pass one dimensional algorithm.
//! IT does two whole scans where it carries out IDCT on all items
//! After each successive scan, data is transposed in register(thank you x86 SIMD powers). and the second
//! pass is carried out.
//!
//! The code is not super optimized, it produces bit identical results with scalar code hence it's
//! `mm256_add_epi16`
//! and it also has the advantage of making this implementation easy to maintain.

#![cfg(feature = "neon")]

use core::arch::aarch64::*;

use crate::unsafe_utils::{transpose, YmmRegister};

const SCALE_BITS: i32 = 512 + 65536 + (128 << 17);

/// SAFETY
/// ------
///
/// It is the responsibility of the CALLER to ensure that  this function is
/// called in contexts where the CPU supports it
///
///
/// For documentation see module docs.

pub fn idct_neon(in_vector: &mut [i32; 64], out_vector: &mut [i16], stride: usize) {
    unsafe {
        // We don't call this method directly because we need to flag the code function
        // with #[target_feature] so that the compiler does do weird stuff with
        // it
        idct_int_neon_inner(in_vector, out_vector, stride);
    }
}

#[inline]
#[target_feature(enable = "neon")]
unsafe fn pack_16(a: int32x4x2_t) -> int16x8_t {
    vcombine_s16(vqmovn_s32(a.0), vqmovn_s32(a.1))
}

#[inline]
#[target_feature(enable = "neon")]
unsafe fn condense_bottom_16(a: int32x4x2_t, b: int32x4x2_t) -> int16x8x2_t {
    int16x8x2_t(pack_16(a), pack_16(b))
}

#[target_feature(enable = "neon")]
#[allow(
    clippy::too_many_lines,
    clippy::cast_possible_truncation,
    clippy::similar_names,
    clippy::op_ref,
    unused_assignments,
    clippy::zero_prefixed_literal
)]
pub unsafe fn idct_int_neon_inner(
    in_vector: &mut [i32; 64], out_vector: &mut [i16], stride: usize
) {
    let mut pos = 0;

    // load into registers
    //
    // We sign extend i16's to i32's and calculate them with extended precision and
    // later reduce them to i16's when we are done carrying out IDCT

    let mut row0 = YmmRegister::load(in_vector[00..].as_ptr().cast());
    let mut row1 = YmmRegister::load(in_vector[08..].as_ptr().cast());
    let mut row2 = YmmRegister::load(in_vector[16..].as_ptr().cast());
    let mut row3 = YmmRegister::load(in_vector[24..].as_ptr().cast());
    let mut row4 = YmmRegister::load(in_vector[32..].as_ptr().cast());
    let mut row5 = YmmRegister::load(in_vector[40..].as_ptr().cast());
    let mut row6 = YmmRegister::load(in_vector[48..].as_ptr().cast());
    let mut row7 = YmmRegister::load(in_vector[56..].as_ptr().cast());

    // Forward DCT and quantization may cause all the AC terms to be zero, for such
    // cases we can try to accelerate it

    // Basically the poop is that whenever the array has 63 zeroes, its idct is
    // (arr[0]>>3)or (arr[0]/8) propagated to all the elements.
    // We first test to see if the array contains zero elements and if it does, we go the
    // short way.
    //
    // This reduces IDCT overhead from about 39% to 18 %, almost half

    // Do another load for the first row, we don't want to check DC value, because
    // we only care about AC terms
    // TODO this should be a shift/shuffle, not a likely unaligned load
    let row8 = YmmRegister::load(in_vector[1..].as_ptr().cast());

    let or_tree = (((row1 | row8) | (row2 | row3)) | ((row4 | row5) | (row6 | row7)));

    if or_tree.all_zero() {
        // AC terms all zero, idct of the block is  is ( coeff[0] * qt[0] )/8 + 128 (bias)
        // (and clamped to 255)
        let coeff = ((in_vector[0] + 4 + 1024) >> 3).clamp(0, 255) as i16;
        let idct_value = vdupq_n_s16(coeff);
        macro_rules! store {
            ($pos:tt,$value:tt) => {
                // store
                vst1q_s16(
                    out_vector
                        .get_mut($pos..$pos + 8)
                        .unwrap()
                        .as_mut_ptr()
                        .cast(),
                    $value
                );
                $pos += stride;
            };
        }
        store!(pos, idct_value);
        store!(pos, idct_value);
        store!(pos, idct_value);
        store!(pos, idct_value);

        store!(pos, idct_value);
        store!(pos, idct_value);
        store!(pos, idct_value);
        store!(pos, idct_value);

        return;
    }

    macro_rules! dct_pass {
        ($SCALE_BITS:tt,$scale:tt) => {
            // There are a lot of ways to do this
            // but to keep it simple(and beautiful), ill make a direct translation of the
            // scalar code to also make this code fully transparent(this version and the non
            // avx one should produce identical code.)

            // Compiler does a pretty good job of optimizing add + mul pairs
            // into multiply-acumulate pairs

            // even part
            let p1 = (row2 + row6) * 2217;

            let mut t2 = p1 + row6 * -7567;
            let mut t3 = p1 + row2 * 3135;

            let mut t0 = (row0 + row4).const_shl::<12>();
            let mut t1 = (row0 - row4).const_shl::<12>();

            let x0 = t0 + t3 + $SCALE_BITS;
            let x3 = t0 - t3 + $SCALE_BITS;
            let x1 = t1 + t2 + $SCALE_BITS;
            let x2 = t1 - t2 + $SCALE_BITS;

            let p3 = row7 + row3;
            let p4 = row5 + row1;
            let p1 = row7 + row1;
            let p2 = row5 + row3;
            let p5 = (p3 + p4) * 4816;

            t0 = row7 * 1223;
            t1 = row5 * 8410;
            t2 = row3 * 12586;
            t3 = row1 * 6149;

            let p1 = p5 + p1 * -3685;
            let p2 = p5 + (p2 * -10497);
            let p3 = p3 * -8034;
            let p4 = p4 * -1597;

            t3 += p1 + p4;
            t2 += p2 + p3;
            t1 += p2 + p4;
            t0 += p1 + p3;

            row0 = (x0 + t3).const_shra::<$scale>();
            row1 = (x1 + t2).const_shra::<$scale>();
            row2 = (x2 + t1).const_shra::<$scale>();
            row3 = (x3 + t0).const_shra::<$scale>();

            row4 = (x3 - t0).const_shra::<$scale>();
            row5 = (x2 - t1).const_shra::<$scale>();
            row6 = (x1 - t2).const_shra::<$scale>();
            row7 = (x0 - t3).const_shra::<$scale>();
        };
    }

    // Process rows
    dct_pass!(512, 10);
    transpose(
        &mut row0, &mut row1, &mut row2, &mut row3, &mut row4, &mut row5, &mut row6, &mut row7
    );

    // process columns
    dct_pass!(SCALE_BITS, 17);
    transpose(
        &mut row0, &mut row1, &mut row2, &mut row3, &mut row4, &mut row5, &mut row6, &mut row7
    );

    // Pack i32 to i16's,
    // clamp them to be between 0-255
    // Undo shuffling
    // Store back to array

    // This could potentially be reorganized to take advantage of the multi-register stores
    macro_rules! permute_store {
        ($x:tt,$y:tt,$index:tt,$out:tt) => {
            let a = condense_bottom_16($x, $y);

            // Clamp the values after packing, we can clamp more values at once
            let b = clamp256_neon(a);

            // store first vector
            vst1q_s16(
                ($out)
                    .get_mut($index..$index + 8)
                    .unwrap()
                    .as_mut_ptr()
                    .cast(),
                b.0
            );
            $index += stride;
            // second vector
            vst1q_s16(
                ($out)
                    .get_mut($index..$index + 8)
                    .unwrap()
                    .as_mut_ptr()
                    .cast(),
                b.1
            );
            $index += stride;
        };
    }
    // Pack and write the values back to the array
    permute_store!((row0.mm256), (row1.mm256), pos, out_vector);
    permute_store!((row2.mm256), (row3.mm256), pos, out_vector);
    permute_store!((row4.mm256), (row5.mm256), pos, out_vector);
    permute_store!((row6.mm256), (row7.mm256), pos, out_vector);
}

#[inline]
#[target_feature(enable = "neon")]
unsafe fn clamp_neon(reg: int16x8_t) -> int16x8_t {
    let min_s = vdupq_n_s16(0);
    let max_s = vdupq_n_s16(255);

    let max_v = vmaxq_s16(reg, min_s); //max(a,0)
    let min_v = vminq_s16(max_v, max_s); //min(max(a,0),255)
    min_v
}

#[inline]
#[target_feature(enable = "neon")]
unsafe fn clamp256_neon(reg: int16x8x2_t) -> int16x8x2_t {
    int16x8x2_t(clamp_neon(reg.0), clamp_neon(reg.1))
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_neon_clamp_256() {
        unsafe {
            let vals: [i16; 16] = [-1, -2, -3, 4, 256, 257, 258, 240, -1, 290, 2, 3, 4, 5, 6, 7];
            let loaded = vld1q_s16_x2(vals.as_ptr().cast());
            let shuffled = clamp256_neon(loaded);

            let mut result: [i16; 16] = [0; 16];

            vst1q_s16_x2(result.as_mut_ptr().cast(), shuffled);

            assert_eq!(
                result,
                [0, 0, 0, 4, 255, 255, 255, 240, 0, 255, 2, 3, 4, 5, 6, 7]
            )
        }
    }
}
