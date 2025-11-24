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
use crate::common::*;
use std::hint::black_box;

static TD: [u64; 32] = [
    0x3ff0000000000000,
    0x3ff059b0d3158574,
    0x3ff0b5586cf9890f,
    0x3ff11301d0125b51,
    0x3ff172b83c7d517b,
    0x3ff1d4873168b9aa,
    0x3ff2387a6e756238,
    0x3ff29e9df51fdee1,
    0x3ff306fe0a31b715,
    0x3ff371a7373aa9cb,
    0x3ff3dea64c123422,
    0x3ff44e086061892d,
    0x3ff4bfdad5362a27,
    0x3ff5342b569d4f82,
    0x3ff5ab07dd485429,
    0x3ff6247eb03a5585,
    0x3ff6a09e667f3bcd,
    0x3ff71f75e8ec5f74,
    0x3ff7a11473eb0187,
    0x3ff82589994cce13,
    0x3ff8ace5422aa0db,
    0x3ff93737b0cdc5e5,
    0x3ff9c49182a3f090,
    0x3ffa5503b23e255d,
    0x3ffae89f995ad3ad,
    0x3ffb7f76f2fb5e47,
    0x3ffc199bdd85529c,
    0x3ffcb720dcef9069,
    0x3ffd5818dcfba487,
    0x3ffdfc97337b9b5f,
    0x3ffea4afa2a490da,
    0x3fff50765b6e4540,
];

#[cold]
fn exp1m1f_accurate(ux: u32, z: f64, sv: u64, ia: f64) -> f32 {
    if ux > 0xc18aa123u32 {
        // x < -17.32
        return -1.0 + f32::from_bits(0x32800000);
    }
    const ILN2H: f64 = f64::from_bits(0x4047154765000000);
    const ILN2L: f64 = f64::from_bits(0x3e55c17f0bbbe880);
    let s = f64::from_bits(sv);
    let h = f_fmla(ILN2H, z, -ia) + ILN2L * z;
    let h2 = h * h;
    let w = s * h;
    const CH: [u64; 6] = [
        0x3f962e42fefa39ef,
        0x3f2ebfbdff82c58f,
        0x3ebc6b08d702e0ed,
        0x3e43b2ab6fb92e5e,
        0x3dc5d886e6d54203,
        0x3d4430976b8ce6ef,
    ];

    let h0 = f_fmla(h, f64::from_bits(CH[5]), f64::from_bits(CH[4]));
    let h1 = f_fmla(h, f64::from_bits(CH[3]), f64::from_bits(CH[2]));
    let h2t = f_fmla(h, f64::from_bits(CH[1]), f64::from_bits(CH[0]));

    let t0 = f_fmla(h2, h0, h1);
    let t1 = f_fmla(h2, t0, h2t);

    let r = f_fmla(w, t1, s - 1.);
    r as f32
}

#[cold]
fn expm1f_small(z: f64) -> f32 {
    const B: [u64; 8] = [
        0x3fdfffffffffffc2,
        0x3fc55555555555fe,
        0x3fa555555559767f,
        0x3f81111111098dc1,
        0x3f56c16bca988aa9,
        0x3f2a01a07658483f,
        0x3efa05b04d2c3503,
        0x3ec71de3a960b5e3,
    ];
    let z2 = z * z;
    let z4 = z2 * z2;

    let r0 = f_fmla(z, f64::from_bits(B[7]), f64::from_bits(B[6]));
    let r1 = f_fmla(z, f64::from_bits(B[5]), f64::from_bits(B[4]));
    let r2 = f_fmla(z, f64::from_bits(B[3]), f64::from_bits(B[2]));
    let r3 = f_fmla(z, f64::from_bits(B[1]), f64::from_bits(B[0]));

    let w0 = f_fmla(z2, r0, r1);
    let w1 = f_fmla(z2, r2, r3);

    let q0 = f_fmla(z4, w0, w1);

    let r = f_fmla(z2, q0, z);
    r as f32
}

/// Computes e^x - 1
///
/// Max ULP 0.5
#[inline]
pub fn f_expm1f(x: f32) -> f32 {
    const ILN2: f64 = f64::from_bits(0x40471547652b82fe);
    const BIG: f64 = f64::from_bits(0x4338000000000000);
    let t = x.to_bits();
    let z = x as f64;
    let ux = t;
    let ax = ux.wrapping_shl(1);
    if ax < 0x7c400000u32 {
        // |x| < 0.15625
        if ax < 0x676a09e8u32 {
            // |x| < 0x1.6a09e8p-24
            if ax == 0x0u32 {
                return x;
            } // x = +-0
            let res = dd_fmlaf(x.abs(), f32::from_bits(0x33000000), x);
            return res;
        }
        return expm1f_small(z);
    }
    if ax >= 0x8562e430u32 {
        // |x| > 88.72
        if ax > (0xffu32 << 24) {
            return x + x;
        } // nan
        if ux >> 31 != 0 {
            // x < 0
            if ax == (0xffu32 << 24) {
                return -1.0;
            }
            return black_box(-1.0) + black_box(f32::from_bits(0x32800000));
        }
        if ax == (0xffu32 << 24) {
            return f32::INFINITY;
        }
        let r = f64::from_bits(0x47efffffe0000000) * z;
        return r as f32;
    }
    let a = ILN2 * z;
    let ia = a.round_ties_even();
    let h = a - ia;
    let h2 = h * h;
    let u = (ia + BIG).to_bits();
    const C: [u64; 4] = [
        0x3ff0000000000000,
        0x3f962e42fef4c4e7,
        0x3f2ebfd1b232f475,
        0x3ebc6b19384ecd93,
    ];

    let c2 = f_fmla(h, f64::from_bits(C[3]), f64::from_bits(C[2]));
    let c0 = f_fmla(h, f64::from_bits(C[1]), f64::from_bits(C[0]));
    let tdl = TD[(u & 0x1f) as usize];
    let sv: u64 = tdl.wrapping_add((u >> 5).wrapping_shl(52));
    let r = f_fmla(h2, c2, c0) * f64::from_bits(sv) - 1.0;
    let ub: f32 = r as f32;
    let lb = (r - f64::from_bits(sv) * f64::from_bits(0x3de3b30000000000)) as f32;

    // Ziv's accuracy test
    if ub != lb {
        return exp1m1f_accurate(ux, z, sv, ia);
    }
    ub
}

#[cfg(test)]
mod tests {
    use crate::f_expm1f;

    #[test]
    fn test_expm1f() {
        assert_eq!(f_expm1f(2.213121), 8.144211);
        assert_eq!(f_expm1f(-3.213121), -0.9597691);
        assert_eq!(f_expm1f(-2.35099e-38), -2.35099e-38);
        assert_eq!(
            f_expm1f(0.00000000000000000000000000000000000004355616),
            0.00000000000000000000000000000000000004355616
        );
    }
}
