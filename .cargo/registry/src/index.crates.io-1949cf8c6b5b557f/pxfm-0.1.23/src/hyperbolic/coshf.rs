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
use crate::common::{f_fmla, f_fmlaf};
use std::hint::black_box;

static TB: [u64; 32] = [
    0x3fe0000000000000,
    0x3fe059b0d3158574,
    0x3fe0b5586cf9890f,
    0x3fe11301d0125b51,
    0x3fe172b83c7d517b,
    0x3fe1d4873168b9aa,
    0x3fe2387a6e756238,
    0x3fe29e9df51fdee1,
    0x3fe306fe0a31b715,
    0x3fe371a7373aa9cb,
    0x3fe3dea64c123422,
    0x3fe44e086061892d,
    0x3fe4bfdad5362a27,
    0x3fe5342b569d4f82,
    0x3fe5ab07dd485429,
    0x3fe6247eb03a5585,
    0x3fe6a09e667f3bcd,
    0x3fe71f75e8ec5f74,
    0x3fe7a11473eb0187,
    0x3fe82589994cce13,
    0x3fe8ace5422aa0db,
    0x3fe93737b0cdc5e5,
    0x3fe9c49182a3f090,
    0x3fea5503b23e255d,
    0x3feae89f995ad3ad,
    0x3feb7f76f2fb5e47,
    0x3fec199bdd85529c,
    0x3fecb720dcef9069,
    0x3fed5818dcfba487,
    0x3fedfc97337b9b5f,
    0x3feea4afa2a490da,
    0x3fef50765b6e4540,
];

#[cold]
fn coshf_accurate(z: f64, ia: f64, sp: u64, sm: u64) -> f32 {
    const CH: [u64; 7] = [
        0x3ff0000000000000,
        0x3f962e42fefa39ef,
        0x3f2ebfbdff82c58f,
        0x3ebc6b08d702e0ed,
        0x3e43b2ab6fb92e5e,
        0x3dc5d886e6d54203,
        0x3d4430976b8ce6ef,
    ];

    const ILN2H: f64 = f64::from_bits(0x4047154765000000);
    const ILN2L: f64 = f64::from_bits(0x3e55c17f0bbbe880);
    let h = f_fmla(ILN2L, z, f_fmla(ILN2H, z, -ia));
    let h2 = h * h;

    let q0 = f_fmla(h2, f64::from_bits(CH[6]), f64::from_bits(CH[4]));
    let q1 = f_fmla(h2, f64::from_bits(CH[2]), f64::from_bits(CH[0]));

    let te = f_fmla(h2 * h2, q0, q1);

    let j0 = f_fmla(h2, f64::from_bits(CH[5]), f64::from_bits(CH[3]));

    let to = f_fmla(h2, j0, f64::from_bits(CH[1]));

    let z0 = f_fmla(h, to, te);
    let z1 = f_fmla(-h, to, te);
    f_fmla(f64::from_bits(sp), z0, f64::from_bits(sm) * z1) as f32
}

/// Hyperbolic cos
///
/// Max found ULP 0.4999996
#[inline]
pub fn f_coshf(x: f32) -> f32 {
    const C: [u64; 4] = [
        0x3ff0000000000000,
        0x3f962e42fef4c4e7,
        0x3f2ebfd1b232f475,
        0x3ebc6b19384ecd93,
    ];

    const I_LN2: f64 = f64::from_bits(0x40471547652b82fe);
    let t = x.to_bits();
    let z = x as f64;
    let ax = t.wrapping_shl(1);
    if ax > 0x8565a9f8u32 {
        // |x| >~ 89.4
        if ax >= 0xff000000u32 {
            if ax.wrapping_shl(8) != 0 {
                return x + x;
            } // nan
            return f32::INFINITY; // +-inf
        }
        let r = black_box(2.0) * black_box(f64::from_bits(0x47efffffe0000000) as f32);
        return r;
    }
    if ax < 0x7c000000u32 {
        // |x| < 0.125
        if ax < 0x74000000u32 {
            // |x| < 0.000488281
            if ax < 0x66000000u32 {
                // |x| < 5.96046e-08
                return f_fmlaf(x.abs(), f64::from_bits(0x3e60000000000000) as f32, 1.0);
            }
            return f_fmlaf(0.5 * x, x, 1.0);
        }
        const CP: [u64; 4] = [
            0x3fdfffffffffffe3,
            0x3fa55555555723cf,
            0x3f56c16bee4a5986,
            0x3efa0483fc0328f7,
        ];
        let z2 = z * z;
        let z4 = z2 * z2;
        let w0 = f_fmla(z2, f64::from_bits(CP[1]), f64::from_bits(CP[0]));
        let w1 = f_fmla(z2, f64::from_bits(CP[3]), f64::from_bits(CP[2]));
        return f_fmla(z2, f_fmla(z4, w1, w0), 1.0) as f32;
    }
    let a = I_LN2 * z;
    let ia = a.round_ties_even();
    let h = a - ia;
    let h2 = h * h;
    let ja = (ia + f64::from_bits(0x4338000000000000)).to_bits();
    let jp: i64 = ja as i64;
    let jm = -jp;
    let sp = TB[(jp & 31) as usize].wrapping_add(jp.wrapping_shr(5).wrapping_shl(52) as u64);
    let sm = TB[(jm & 31) as usize].wrapping_add(jm.wrapping_shr(5).wrapping_shl(52) as u64);
    let te = f_fmla(h2, f64::from_bits(C[2]), f64::from_bits(C[0]));
    let to = f_fmla(h2, f64::from_bits(C[3]), f64::from_bits(C[1]));
    let rp = f64::from_bits(sp) * f_fmla(h, to, te);
    let rm = f64::from_bits(sm) * f_fmla(-h, to, te);
    let r = rp + rm;
    let ub = r;
    let lb = r - f64::from_bits(0x3de3edbbe4560327) * r;
    // Ziv's accuracy test
    if ub != lb {
        return coshf_accurate(z, ia, sp, sm);
    }
    ub as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coshf() {
        assert_eq!(f_coshf(-0.5), 1.127626);
        assert_eq!(f_coshf(0.5), 1.127626);
        assert_eq!(f_coshf(7.), 548.317);
    }
}
