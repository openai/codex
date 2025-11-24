/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use core::convert::TryInto;

// Bt.601 Full Range inverse coefficients computed with 14 bits of precision with MPFR.
// This is important to keep them in i16.
// In most cases LLVM will detect what we're doing i16 widening to i32 math and will use
// appropriate optimizations.
pub(crate) const Y_CF: i16 = 16384;
pub(crate) const CR_CF: i16 = 22970;
pub(crate) const CB_CF: i16 = 29032;
pub(crate) const C_G_CR_COEF_1: i16 = -11700;
pub(crate) const C_G_CB_COEF_2: i16 = -5638;
pub(crate) const YUV_PREC: i16 = 14;
// Rounding const for YUV -> RGB conversion: floating equivalent 0.499(9).
pub(crate) const YUV_RND: i16 = (1 << (YUV_PREC - 1)) - 1;

/// Limit values to 0 and 255
#[inline]
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, dead_code)]
fn clamp(a: i32) -> u8 {
    a.clamp(0, 255) as u8
}

/// YCbCr to RGBA color conversion

/// Convert YCbCr to RGB/BGR
///
/// Converts to RGB if const BGRA is false
///
/// Converts to BGR if const BGRA is true
pub fn ycbcr_to_rgba_inner_16_scalar<const BGRA: bool>(
    y: &[i16; 16], cb: &[i16; 16], cr: &[i16; 16], output: &mut [u8], pos: &mut usize
) {
    let (_, output_position) = output.split_at_mut(*pos);

    // Convert into a slice with 64 elements for Rust to see we won't go out of bounds.
    let opt: &mut [u8; 64] = output_position
        .get_mut(0..64)
        .expect("Slice to small cannot write")
        .try_into()
        .unwrap();
    for ((&y, (cb, cr)), out) in y
        .iter()
        .zip(cb.iter().zip(cr.iter()))
        .zip(opt.chunks_exact_mut(4))
    {
        let cr = cr - 128;
        let cb = cb - 128;

        let y0 = i32::from(y) * i32::from(Y_CF) + i32::from(YUV_RND);

        let r = (y0 + i32::from(cr) * i32::from(CR_CF)) >> YUV_PREC;
        let g = (y0
            + i32::from(cr) * i32::from(C_G_CR_COEF_1)
            + i32::from(cb) * i32::from(C_G_CB_COEF_2))
            >> YUV_PREC;
        let b = (y0 + i32::from(cb) * i32::from(CB_CF)) >> YUV_PREC;

        if BGRA {
            out[0] = clamp(b);
            out[1] = clamp(g);
            out[2] = clamp(r);
            out[3] = 255;
        } else {
            out[0] = clamp(r);
            out[1] = clamp(g);
            out[2] = clamp(b);
            out[3] = 255;
        }
    }
    *pos += 64;
}

/// Convert YCbCr to RGB/BGR
///
/// Converts to RGB if const BGRA is false
///
/// Converts to BGR if const BGRA is true
pub fn ycbcr_to_rgb_inner_16_scalar<const BGRA: bool>(
    y: &[i16; 16], cb: &[i16; 16], cr: &[i16; 16], output: &mut [u8], pos: &mut usize
) {
    let (_, output_position) = output.split_at_mut(*pos);

    // Convert into a slice with 48 elements
    let opt: &mut [u8; 48] = output_position
        .get_mut(0..48)
        .expect("Slice to small cannot write")
        .try_into()
        .unwrap();

    for ((&y, (cb, cr)), out) in y
        .iter()
        .zip(cb.iter().zip(cr.iter()))
        .zip(opt.chunks_exact_mut(3))
    {
        let cr = cr - 128;
        let cb = cb - 128;

        let y0 = i32::from(y) * i32::from(Y_CF) + i32::from(YUV_RND);

        let r = (y0 + i32::from(cr) * i32::from(CR_CF)) >> YUV_PREC;
        let g = (y0
            + i32::from(cr) * i32::from(C_G_CR_COEF_1)
            + i32::from(cb) * i32::from(C_G_CB_COEF_2))
            >> YUV_PREC;
        let b = (y0 + i32::from(cb) * i32::from(CB_CF)) >> YUV_PREC;

        if BGRA {
            out[0] = clamp(b);
            out[1] = clamp(g);
            out[2] = clamp(r);
        } else {
            out[0] = clamp(r);
            out[1] = clamp(g);
            out[2] = clamp(b);
        }
    }

    // Increment pos
    *pos += 48;
}

pub fn ycbcr_to_grayscale(y: &[i16], width: usize, padded_width: usize, output: &mut [u8]) {
    for (y_in, out) in y
        .chunks_exact(padded_width)
        .zip(output.chunks_exact_mut(width))
    {
        for (y, out) in y_in.iter().zip(out.iter_mut()) {
            *out = *y as u8;
        }
    }
}
