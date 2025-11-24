/*
 * // Copyright (c) Radzivon Bartoshyk 6/2025. All rights reserved.
 * //
 * // Redistribution and use in source and binary forms, with or without modification,
 * // are permitted provided that the following conditions are met:
 * //
 * // 1.  Redistributions of source code must retain the above copyright notice, this
 * // list of conditions and the following disclaimer.
 * //
 * // 2.  Redistributions in binary form must reproduce the above copyright notice,
 * // this list of conditions and the following disclaimer in the documentation
 * // and/or other materials provided with the distribution.
 * //
 * // 3.  Neither the name of the copyright holder nor the names of its
 * // contributors may be used to endorse or promote products derived from
 * // this software without specific prior written permission.
 * //
 * // THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
 * // AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
 * // IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
 * // DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE
 * // FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
 * // DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
 * // SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
 * // CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
 * // OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
 * // OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
 */
use crate::common::f_fmla;
use std::hint::black_box;

#[inline]
pub(crate) fn poly12(z: f64, c: [u64; 12]) -> f64 {
    let z2 = z * z;
    let z4 = z2 * z2;
    let mut c0 = f_fmla(z, f64::from_bits(c[1]), f64::from_bits(c[0]));
    let c2 = f_fmla(z, f64::from_bits(c[3]), f64::from_bits(c[2]));
    let mut c4 = f_fmla(z, f64::from_bits(c[5]), f64::from_bits(c[4]));
    let c6 = f_fmla(z, f64::from_bits(c[7]), f64::from_bits(c[6]));
    let mut c8 = f_fmla(z, f64::from_bits(c[9]), f64::from_bits(c[8]));
    let c10 = f_fmla(z, f64::from_bits(c[11]), f64::from_bits(c[10]));
    c0 = f_fmla(c2, z2, c0);
    c4 = f_fmla(c6, z2, c4);
    c8 = f_fmla(z2, c10, c8);
    f_fmla(z4, f_fmla(z4, c8, c4), c0)
}

#[cold]
fn as_special(x: f32) -> f32 {
    const PIH: f32 = f64::from_bits(0x400921fb60000000) as f32;
    const PIL: f32 = -f64::from_bits(0x3e70000000000000) as f32;
    let t = x.to_bits();
    if t == (0x7fu32 << 23) {
        return 0.0;
    } // x=1
    if t == (0x17fu32 << 23) {
        return PIH + PIL;
    } // x=-1
    let ax = t.wrapping_shl(1);
    if ax > (0xffu32 << 24) {
        return x + x;
    } // nan
    f32::NAN
}

/// Compute acos
///
/// Max found ULP 0.49999982
#[inline]
pub fn f_acosf(x: f32) -> f32 {
    const PI2: f64 = f64::from_bits(0x3ff921fb54442d18);
    const O: [f64; 2] = [0., f64::from_bits(0x400921fb54442d18)];
    let xs = x as f64;
    let mut r: f64;
    let t = x.to_bits();
    let ax = t.wrapping_shl(1);
    if ax >= 0x7f << 24 {
        return as_special(x);
    }
    if ax < 0x7ec2a1dcu32 {
        // |x| < 0.880141
        const B: [u64; 16] = [
            0x3fefffffffd9ccb8,
            0x3fc5555c94838007,
            0x3fb32ded4b7c20fa,
            0x3fa8566df703309e,
            0xbf9980c959bec9a3,
            0x3fe56fbb04998344,
            0xc01403d8e4c49f52,
            0x403b06c3e9f311ea,
            0xc059ea97c4e2c21f,
            0x407200b8261cc61b,
            0xc082274c2799a5c7,
            0x408a558a59cc19d3,
            0xc08aca4b6a529ff0,
            0x408228744703f813,
            0xc06d7dbb0b322228,
            0x4045c2018c0c0105,
        ];
        /* avoid spurious underflow */
        if ax < 0x40000000u32 {
            // |x| < 2^-63
            return PI2 as f32;
        }
        let z = xs;
        let z2 = z * z;

        let w0 = f_fmla(z2, f64::from_bits(B[1]), f64::from_bits(B[0]));
        let w1 = f_fmla(z2, f64::from_bits(B[3]), f64::from_bits(B[2]));
        let w2 = f_fmla(z2, f64::from_bits(B[5]), f64::from_bits(B[4]));
        let w3 = f_fmla(z2, f64::from_bits(B[7]), f64::from_bits(B[6]));
        let w4 = f_fmla(z2, f64::from_bits(B[9]), f64::from_bits(B[8]));
        let w5 = f_fmla(z2, f64::from_bits(B[11]), f64::from_bits(B[10]));
        let w6 = f_fmla(z2, f64::from_bits(B[13]), f64::from_bits(B[12]));
        let w7 = f_fmla(z2, f64::from_bits(B[15]), f64::from_bits(B[14]));

        let z4 = z2 * z2;
        let z8 = z4 * z4;
        let z16 = z8 * z8;

        r = z
            * ((f_fmla(z4, w1, w0) + z8 * f_fmla(z4, w3, w2))
                + z16 * (f_fmla(z4, w5, w4) + z8 * f_fmla(z4, w7, w6)));

        let ub = f64::from_bits(0x3ff921fb54574191) - r;
        let lb = f64::from_bits(0x3ff921fb543118a0) - r;
        // Ziv's accuracy test
        if ub == lb {
            return ub as f32;
        }
    }
    // accurate path
    if ax < (0x7eu32 << 24) {
        const C: [u64; 12] = [
            0x3fc555555555529c,
            0x3fb333333337e0dd,
            0x3fa6db6db3b4465e,
            0x3f9f1c72e13ac306,
            0x3f96e89cebe06bc4,
            0x3f91c6dcf5289094,
            0x3f8c6dbbcc7c6315,
            0x3f88f8dc2615e996,
            0x3f7a5833b7bf15e8,
            0x3f943f44ace1665c,
            0xbf90fb17df881c73,
            0x3fa07520c026b2d6,
        ];
        if t == 0x328885a3u32 {
            return black_box(f64::from_bits(0x3ff921fb60000000) as f32)
                + black_box(f64::from_bits(0x3e60000000000000) as f32);
        }
        if t == 0x39826222u32 {
            return black_box(f64::from_bits(0x3ff920f6a0000000) as f32)
                + black_box(f64::from_bits(0x3e60000000000000) as f32);
        }
        let x2 = xs * xs;
        r = f_fmla(-(xs * x2), poly12(x2, C), PI2 - xs);
    } else {
        const C: [u64; 12] = [
            0x3ff6a09e667f3bcb,
            0x3fbe2b7dddff2db9,
            0x3f9b27247ab42dbc,
            0x3f802995cc4e0744,
            0x3f65ffb0276ec8ea,
            0x3f5033885a928dec,
            0x3f3911f2be23f8c7,
            0x3f24c3c55d2437fd,
            0x3f0af477e1d7b461,
            0x3f0abd6bdff67dcb,
            0xbef1717e86d0fa28,
            0x3ef6ff526de46023,
        ];
        let bx = xs.abs();
        let z = 1.0 - bx;
        let s = f64::copysign(z.sqrt(), xs);
        r = f_fmla(s, poly12(z, C), O[t.wrapping_shr(31) as usize]);
    }
    r as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_acosf() {
        assert_eq!(f_acosf(-0.5), 2.0943952);
        assert_eq!(f_acosf(0.5), std::f32::consts::FRAC_PI_3);
        assert!(f_acosf(7.).is_nan());
    }
}
