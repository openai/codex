/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

//! Platform independent IDCT algorithm
//!
//! Not as fast as AVX one.

const SCALE_BITS: i32 = 512 + 65536 + (128 << 17);

#[allow(unused_assignments)]
#[allow(
    clippy::too_many_lines,
    clippy::op_ref,
    clippy::cast_possible_truncation
)]
pub fn idct_int(in_vector: &mut [i32; 64], out_vector: &mut [i16], stride: usize) {
    // Temporary variables.

    let mut pos = 0;

    let mut i = 0;
    // Don't check for zeroes inside loop, lift it and check outside
    // we want to accelerate the case with 63 0 ac coeff
    if &in_vector[1..] == &[0_i32; 63] {
        // okay then if you work, yay, let's write you really quick
        let coeff = [((in_vector[0] + 4 + 1024) >> 3).clamp(0, 255) as i16; 8];

        macro_rules! store {
            ($index:tt) => {
                // position of the MCU
                let mcu_stride: &mut [i16; 8] = out_vector
                    .get_mut($index..$index + 8)
                    .unwrap()
                    .try_into()
                    .unwrap();
                // copy coefficients
                mcu_stride.copy_from_slice(&coeff);
                // increment index
                $index += stride;
            };
        }
        // write to four positions
        store!(pos);
        store!(pos);
        store!(pos);
        store!(pos);

        store!(pos);
        store!(pos);
        store!(pos);
        store!(pos);
    } else {
        // because the compiler fails to see that it can be auto_vectorised so i'll
        // leave it here check out [idct_int_slow, and idct_int_1D to get what i mean ] https://godbolt.org/z/8hqW9z9j9
        for ptr in 0..8 {
            let p2 = in_vector[ptr + 16];
            let p3 = in_vector[ptr + 48];

            let p1 = (p2 + p3).wrapping_mul(2217);

            let t2 = p1 + p3 * -7567;
            let t3 = p1 + p2 * 3135;

            let p2 = in_vector[ptr];
            let p3 = in_vector[32 + ptr];
            let t0 = fsh(p2 + p3);
            let t1 = fsh(p2 - p3);

            let x0 = t0 + t3 + 512;
            let x3 = t0 - t3 + 512;
            let x1 = t1 + t2 + 512;
            let x2 = t1 - t2 + 512;

            // odd part
            let mut t0 = in_vector[ptr + 56];
            let mut t1 = in_vector[ptr + 40];
            let mut t2 = in_vector[ptr + 24];
            let mut t3 = in_vector[ptr + 8];

            let p3 = t0 + t2;
            let p4 = t1 + t3;
            let p1 = t0 + t3;
            let p2 = t1 + t2;
            let p5 = (p3 + p4) * 4816;

            t0 *= 1223;
            t1 *= 8410;
            t2 *= 12586;
            t3 *= 6149;

            let p1 = p5 + p1 * -3685;
            let p2 = p5 + p2 * -10497;
            let p3 = p3 * -8034;
            let p4 = p4 * -1597;

            t3 += p1 + p4;
            t2 += p2 + p3;
            t1 += p2 + p4;
            t0 += p1 + p3;

            // constants scaled things up by 1<<12; let's bring them back
            // down, but keep 2 extra bits of precision
            in_vector[ptr] = (x0 + t3) >> 10;
            in_vector[ptr + 8] = (x1 + t2) >> 10;
            in_vector[ptr + 16] = (x2 + t1) >> 10;
            in_vector[ptr + 24] = (x3 + t0) >> 10;
            in_vector[ptr + 32] = (x3 - t0) >> 10;
            in_vector[ptr + 40] = (x2 - t1) >> 10;
            in_vector[ptr + 48] = (x1 - t2) >> 10;
            in_vector[ptr + 56] = (x0 - t3) >> 10;
        }

        // This is vectorised in architectures supporting SSE 4.1
        while i < 64 {
            // We won't try to short circuit here because it rarely works

            // Even part
            let p2 = in_vector[i + 2];
            let p3 = in_vector[i + 6];

            let p1 = (p2 + p3) * 2217;
            let t2 = p1 + p3 * -7567;
            let t3 = p1 + p2 * 3135;

            let p2 = in_vector[i];
            let p3 = in_vector[i + 4];

            let t0 = fsh(p2 + p3);
            let t1 = fsh(p2 - p3);
            // constants scaled things up by 1<<12, plus we had 1<<2 from first
            // loop, plus horizontal and vertical each scale by sqrt(8) so together
            // we've got an extra 1<<3, so 1<<17 total we need to remove.
            // so we want to round that, which means adding 0.5 * 1<<17,
            // aka 65536. Also, we'll end up with -128 to 127 that we want
            // to encode as 0..255 by adding 128, so we'll add that before the shift
            let x0 = t0 + t3 + SCALE_BITS;
            let x3 = t0 - t3 + SCALE_BITS;
            let x1 = t1 + t2 + SCALE_BITS;
            let x2 = t1 - t2 + SCALE_BITS;
            // odd part
            let mut t0 = in_vector[i + 7];
            let mut t1 = in_vector[i + 5];
            let mut t2 = in_vector[i + 3];
            let mut t3 = in_vector[i + 1];

            let p3 = t0 + t2;
            let p4 = t1 + t3;
            let p1 = t0 + t3;
            let p2 = t1 + t2;
            let p5 = (p3 + p4) * f2f(1.175875602);

            t0 = t0.wrapping_mul(1223);
            t1 = t1.wrapping_mul(8410);
            t2 = t2.wrapping_mul(12586);
            t3 = t3.wrapping_mul(6149);

            let p1 = p5 + p1 * -3685;
            let p2 = p5 + p2 * -10497;
            let p3 = p3 * -8034;
            let p4 = p4 * -1597;

            t3 += p1 + p4;
            t2 += p2 + p3;
            t1 += p2 + p4;
            t0 += p1 + p3;

            let out: &mut [i16; 8] = out_vector
                .get_mut(pos..pos + 8)
                .unwrap()
                .try_into()
                .unwrap();

            out[0] = clamp((x0 + t3) >> 17);
            out[1] = clamp((x1 + t2) >> 17);
            out[2] = clamp((x2 + t1) >> 17);
            out[3] = clamp((x3 + t0) >> 17);
            out[4] = clamp((x3 - t0) >> 17);
            out[5] = clamp((x2 - t1) >> 17);
            out[6] = clamp((x1 - t2) >> 17);
            out[7] = clamp((x0 - t3) >> 17);

            i += 8;

            pos += stride;
        }
    }
}

#[inline]
#[allow(clippy::cast_possible_truncation)]
/// Multiply a number by 4096
fn f2f(x: f32) -> i32 {
    (x * 4096.0 + 0.5) as i32
}

#[inline]
/// Multiply a number by 4096
fn fsh(x: i32) -> i32 {
    x << 12
}

/// Clamp values between 0 and 255
#[inline]
#[allow(clippy::cast_possible_truncation)]
fn clamp(a: i32) -> i16 {
    a.clamp(0, 255) as i16
}
