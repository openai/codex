/*
 * Copyright (c) 2025.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

//! Aarch64 color conversion routines
//! NEON is mandatory on aarch64.

#![cfg(all(feature = "neon", target_arch = "aarch64"))]
use std::arch::aarch64::*;

use crate::color_convert::scalar::{CB_CF, CR_CF, C_G_CB_COEF_2, C_G_CR_COEF_1, YUV_RND, Y_CF};

const C_1: u64 = u64::from_ne_bytes([
    Y_CF.to_ne_bytes()[0],
    Y_CF.to_ne_bytes()[1],
    CR_CF.to_ne_bytes()[0],
    CR_CF.to_ne_bytes()[1],
    CB_CF.to_ne_bytes()[0],
    CB_CF.to_ne_bytes()[1],
    C_G_CR_COEF_1.to_ne_bytes()[0],
    C_G_CR_COEF_1.to_ne_bytes()[1]
]);
const C_2: u64 = u64::from_ne_bytes([
    C_G_CB_COEF_2.to_ne_bytes()[0],
    C_G_CB_COEF_2.to_ne_bytes()[1],
    0,
    0,
    0,
    0,
    0,
    0
]);

#[inline(always)]
unsafe fn ycbcr_to_rgb_baseline_no_clamp(
    y: &[i16; 16], cb: &[i16; 16], cr: &[i16; 16]
) -> (uint8x16_t, uint8x16_t, uint8x16_t) {
    // NEON has 32 registers, so it is good idea to utilize a lot of variables at once

    let cb_cr_bias = vdupq_n_s16(128);
    // 0 - Y coeff, 1 - Cr, 2 - Cb, 3 - G1, 4 - G2
    let coefficients = vcombine_s16(vcreate_s16(C_1), vcreate_s16(C_2));

    let y0 = vld1q_s16(y.as_ptr().cast());
    let y1 = vld1q_s16(y[8..].as_ptr().cast());

    let mut cb0 = vld1q_s16(cb.as_ptr().cast());
    let mut cb1 = vld1q_s16(cb[8..].as_ptr().cast());

    let mut cr0 = vld1q_s16(cr.as_ptr().cast());
    let mut cr1 = vld1q_s16(cr[8..].as_ptr().cast());

    cb0 = vsubq_s16(cb0, cb_cr_bias);
    cb1 = vsubq_s16(cb1, cb_cr_bias);

    cr0 = vsubq_s16(cr0, cb_cr_bias);
    cr1 = vsubq_s16(cr1, cb_cr_bias);

    let bias = vdupq_n_s32(i32::from(YUV_RND));

    let acc0 = vmlal_laneq_s16::<0>(bias, vget_low_s16(y0), coefficients);
    let acc1 = vmlal_high_laneq_s16::<0>(bias, y0, coefficients);
    let acc2 = vmlal_laneq_s16::<0>(bias, vget_low_s16(y1), coefficients);
    let acc3 = vmlal_high_laneq_s16::<0>(bias, y1, coefficients);

    let r0 = vmlal_laneq_s16::<1>(acc0, vget_low_s16(cr0), coefficients);
    let r1 = vmlal_high_laneq_s16::<1>(acc1, cr0, coefficients);
    let r2 = vmlal_laneq_s16::<1>(acc2, vget_low_s16(cr1), coefficients);
    let r3 = vmlal_high_laneq_s16::<1>(acc3, cr1, coefficients);

    let b0 = vmlal_laneq_s16::<2>(acc0, vget_low_s16(cb0), coefficients);
    let b1 = vmlal_high_laneq_s16::<2>(acc1, cb0, coefficients);
    let b2 = vmlal_laneq_s16::<2>(acc2, vget_low_s16(cb1), coefficients);
    let b3 = vmlal_high_laneq_s16::<2>(acc3, cb1, coefficients);

    // Saturating shift right with signed -> unsigned saturation
    let qr0 = vqshrun_n_s32::<14>(r0);
    let qr1 = vqshrun_n_s32::<14>(r1);
    let qr2 = vqshrun_n_s32::<14>(r2);
    let qr3 = vqshrun_n_s32::<14>(r3);

    let mut g0 = vmlal_laneq_s16::<4>(acc0, vget_low_s16(cb0), coefficients);
    let mut g1 = vmlal_high_laneq_s16::<4>(acc1, cb0, coefficients);
    let mut g2 = vmlal_laneq_s16::<4>(acc2, vget_low_s16(cb1), coefficients);
    let mut g3 = vmlal_high_laneq_s16::<4>(acc3, cb1, coefficients);

    let qb0 = vqshrun_n_s32::<14>(b0);
    let qb1 = vqshrun_n_s32::<14>(b1);
    let qb2 = vqshrun_n_s32::<14>(b2);
    let qb3 = vqshrun_n_s32::<14>(b3);

    let r0 = vqmovn_u16(vcombine_u16(qr0, qr1));
    let r1 = vqmovn_u16(vcombine_u16(qr2, qr3));

    let b0 = vqmovn_u16(vcombine_u16(qb0, qb1));
    let b1 = vqmovn_u16(vcombine_u16(qb2, qb3));

    g0 = vmlal_laneq_s16::<3>(g0, vget_low_s16(cr0), coefficients);
    g1 = vmlal_high_laneq_s16::<3>(g1, cr0, coefficients);
    g2 = vmlal_laneq_s16::<3>(g2, vget_low_s16(cr1), coefficients);
    g3 = vmlal_high_laneq_s16::<3>(g3, cr1, coefficients);

    let qg0 = vqshrun_n_s32::<14>(g0);
    let qg1 = vqshrun_n_s32::<14>(g1);
    let qg2 = vqshrun_n_s32::<14>(g2);
    let qg3 = vqshrun_n_s32::<14>(g3);

    let g0 = vqmovn_u16(vcombine_u16(qg0, qg1));
    let g1 = vqmovn_u16(vcombine_u16(qg2, qg3));

    (
        vcombine_u8(r0, r1),
        vcombine_u8(g0, g1),
        vcombine_u8(b0, b1)
    )
}

#[inline(always)]
pub fn ycbcr_to_rgb_neon(
    y: &[i16; 16], cb: &[i16; 16], cr: &[i16; 16], out: &mut [u8], offset: &mut usize
) {
    // call this in another function to tell RUST to vectorize this
    // storing
    unsafe {
        let (r, g, b) = ycbcr_to_rgb_baseline_no_clamp(y, cb, cr);
        vst3q_u8(out.as_mut_ptr(), uint8x16x3_t(r, g, b));
        *offset += 48;
    }
}

#[inline(always)]
pub fn ycbcr_to_rgba_neon(
    y: &[i16; 16], cb: &[i16; 16], cr: &[i16; 16], out: &mut [u8], offset: &mut usize
) {
    unsafe {
        let (r, g, b) = ycbcr_to_rgb_baseline_no_clamp(y, cb, cr);
        vst4q_u8(out.as_mut_ptr(), uint8x16x4_t(r, g, b, vdupq_n_u8(255)));
        *offset += 64;
    }
}
