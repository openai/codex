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

static Q: [(u32, u32); 3] = [
    (0x7f7fffff, 0x7f7fffff),
    (0x7f7fffff, 0x73000000),
    (0xbf800000, 0x32800000),
];

static TB: [u64; 16] = [
    0x3ff0000000000000,
    0x3ff0b5586cf9890f,
    0x3ff172b83c7d517b,
    0x3ff2387a6e756238,
    0x3ff306fe0a31b715,
    0x3ff3dea64c123422,
    0x3ff4bfdad5362a27,
    0x3ff5ab07dd485429,
    0x3ff6a09e667f3bcd,
    0x3ff7a11473eb0187,
    0x3ff8ace5422aa0da,
    0x3ff9c49182a3f090,
    0x3ffae89f995ad3ad,
    0x3ffc199bdd85529c,
    0x3ffd5818dcfba487,
    0x3ffea4afa2a490da,
];

// |x| < 8.44e-2/log(2)
#[cold]
fn exp2mf_small(ax: u32, z: f64, ux: u32) -> f32 {
    let z2: f64 = z * z;
    let mut r: f64;
    if ax < 0x3d67a4ccu32 {
        // |x| < 3.92e-2/log(2)
        if ax < 0x3caa2feeu32 {
            // |x| < 1.44e-2/log(2)
            if ax < 0x3bac1405u32 {
                // |x| < 3.64e-3/log(2)
                if ax < 0x3a358876u32 {
                    // |x| < 4.8e-4/log(2)
                    if ax < 0x37d32ef6u32 {
                        // |x| < 1.745e-5/log(2)
                        if ax < 0x331fdd82u32 {
                            // |x| < 2.58e-8/log(2)
                            if ax < 0x2538aa3bu32 {
                                // |x| < 0x1.715476p-53
                                r = f64::from_bits(0x3fe62e42fefa39ef);
                            } else {
                                r = f_fmla(
                                    z,
                                    f64::from_bits(0x3fcebfbdff82c58f),
                                    f64::from_bits(0x3fe62e42fefa39f0),
                                );
                            }
                        } else {
                            if ux == 0xb3d85005u32 {
                                return (black_box(f64::from_bits(0xbe72bdf760000000))
                                    - black_box(f64::from_bits(0x3b28000000000000)))
                                    as f32;
                            }
                            if ux == 0x3338428du32 {
                                return (black_box(f64::from_bits(0x3e5fee08a0000000))
                                    + black_box(f64::from_bits(0x3af0000000000000)))
                                    as f32;
                            }
                            const C: [u64; 3] =
                                [0x3fe62e42fefa39ef, 0x3fcebfbdff8548fd, 0x3fac6b08d704a06d];
                            let r0 = f_fmla(z, f64::from_bits(C[2]), f64::from_bits(C[1]));
                            r = f_fmla(z, r0, f64::from_bits(C[0]));
                        }
                    } else {
                        if ux == 0x388bca4fu32 {
                            return (black_box(f64::from_bits(0x3f08397020000000))
                                - black_box(f64::from_bits(0x3bb8000000000000)))
                                as f32;
                        }
                        const C: [u64; 4] = [
                            0x3fe62e42fefa39ef,
                            0x3fcebfbdff82c58f,
                            0x3fac6b08dc82b347,
                            0x3f83b2ab6fbad172,
                        ];
                        let r0 = f_fmla(z, f64::from_bits(C[3]), f64::from_bits(C[2]));
                        let r1 = f_fmla(z, f64::from_bits(C[1]), f64::from_bits(C[0]));

                        r = f_fmla(z2, r0, r1);
                    }
                } else {
                    const C: [u64; 5] = [
                        0x3fe62e42fefa39ef,
                        0x3fcebfbdff82c068,
                        0x3fac6b08d704a6dc,
                        0x3f83b2ac262c3eed,
                        0x3f55d87fe7af779a,
                    ];

                    let r0 = f_fmla(z, f64::from_bits(C[4]), f64::from_bits(C[3]));
                    let r1 = f_fmla(z, f64::from_bits(C[1]), f64::from_bits(C[0]));
                    let w0 = f_fmla(z, r0, f64::from_bits(C[2]));
                    r = f_fmla(z2, w0, r1);
                }
            } else {
                const C: [u64; 6] = [
                    0x3fe62e42fefa39f0,
                    0x3fcebfbdff82c58d,
                    0x3fac6b08d7011d13,
                    0x3f83b2ab6fbd267d,
                    0x3f55d88a81cea49e,
                    0x3f2430912ea9b963,
                ];

                let r0 = f_fmla(z, f64::from_bits(C[5]), f64::from_bits(C[4]));
                let r1 = f_fmla(z, f64::from_bits(C[3]), f64::from_bits(C[2]));
                let r2 = f_fmla(z, f64::from_bits(C[1]), f64::from_bits(C[0]));

                r = r2 + z2 * f_fmla(z2, r0, r1);
            }
        } else {
            const C: [u64; 7] = [
                0x3fe62e42fefa39ef,
                0x3fcebfbdff82c639,
                0x3fac6b08d7049f1c,
                0x3f83b2ab6f5243bd,
                0x3f55d87fe80a9e6c,
                0x3f2430d0b9257fa8,
                0x3eeffcbfc4cf0952,
            ];

            let r0 = f_fmla(z, f64::from_bits(C[6]), f64::from_bits(C[5]));
            let r1 = f_fmla(z, f64::from_bits(C[3]), f64::from_bits(C[2]));
            let r2 = f_fmla(z, f64::from_bits(C[1]), f64::from_bits(C[0]));

            let z0 = f_fmla(z, r0, f64::from_bits(C[4]));

            r = f_fmla(z2, f_fmla(z2, z0, r1), r2);
        }
    } else {
        const C: [u64; 8] = [
            0x3fe62e42fefa39ef,
            0x3fcebfbdff82c591,
            0x3fac6b08d704cf6b,
            0x3f83b2ab6fba00ce,
            0x3f55d87fdfdaadb4,
            0x3f24309137333066,
            0x3eeffe5e90daf7dd,
            0x3eb62c0220eed731,
        ];

        let r0 = f_fmla(z, f64::from_bits(C[7]), f64::from_bits(C[6]));
        let r1 = f_fmla(z, f64::from_bits(C[5]), f64::from_bits(C[4]));
        let r2 = f_fmla(z, f64::from_bits(C[3]), f64::from_bits(C[2]));
        let r3 = f_fmla(z, f64::from_bits(C[1]), f64::from_bits(C[0]));

        let w0 = f_fmla(z2, r0, r1);
        let w1 = f_fmla(z2, r2, r3);

        r = w1 + (z2 * z2) * w0;
    }
    r *= z;
    r as f32
}

/// Computes 2^x-1
///
/// Max found ULP 0.5
#[inline]
pub fn f_exp2m1f(x: f32) -> f32 {
    let t = x.to_bits();
    let z = x as f64;
    let ux = t;
    let ax = ux & 0x7fff_ffff;
    if ux >= 0xc1c80000u32 {
        // x <= -25
        if ax > (0xffu32 << 23) {
            return x + x;
        } // nan
        // avoid spurious inexact exception for -Inf
        if ux == 0xff800000 {
            f32::from_bits(Q[2].0)
        } else {
            let zq = Q[2];
            black_box(f32::from_bits(zq.0)) + black_box(f32::from_bits(zq.1))
        }
    } else if ax >= 0x43000000u32 {
        // x >= 128
        if ax > (0xffu32 << 23) {
            return x + x;
        } // nan
        // for x=128 and rounding downward or to zero, there is no overflow
        let special = if x == 128.0
            && (f32::from_bits(Q[1].0) + f32::from_bits(Q[1].1) == f32::from_bits(Q[1].0))
        {
            1
        } else {
            0
        };
        // avoid spurious inexact exception for +Inf
        if ux == 0x7f800000u32 {
            x
        } else {
            f32::from_bits(Q[special].0) + f32::from_bits(Q[special].1)
        }
    } else if ax < 0x3df95f1fu32 {
        // |x| < 8.44e-2/log(2)
        exp2mf_small(ax, z, ux)
    } else {
        const C: [u64; 6] = [
            0x3fa62e42fefa398b,
            0x3f4ebfbdff84555a,
            0x3eec6b08d4ad86d3,
            0x3e83b2ad1b1716a2,
            0x3e15d7472718ce9d,
            0x3da4a1d7f457ac56,
        ];

        let a = 16.0 * z;
        let ia = a.floor();
        let h = a - ia;
        let h2 = h * h;
        let i: i64 = ia as i64;
        let j = i & 0xf;
        let mut e = i.wrapping_sub(j);
        e >>= 4;
        let mut s = f64::from_bits(TB[j as usize]);
        let su = ((e as u64).wrapping_add(0x3ffu64)) << 52;
        s *= f64::from_bits(su);
        let mut c0 = f_fmla(h, f64::from_bits(C[1]), f64::from_bits(C[0]));
        let c2 = f_fmla(h, f64::from_bits(C[3]), f64::from_bits(C[2]));
        let c4 = f_fmla(h, f64::from_bits(C[5]), f64::from_bits(C[4]));
        c0 += h2 * f_fmla(h2, c4, c2);
        let w = s * h;
        f_fmla(w, c0, s - 1.0) as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exp2m1f() {
        assert_eq!(f_exp2m1f(0.432423), 0.34949815);
        assert_eq!(f_exp2m1f(-4.), -0.9375);
        assert_eq!(f_exp2m1f(5.43122), 42.14795);
        assert_eq!(f_exp2m1f(4.), 15.0);
        assert_eq!(f_exp2m1f(3.), 7.);
        assert_eq!(f_exp2m1f(0.1), 0.07177346);
        assert_eq!(f_exp2m1f(0.0543432432), 0.038386293);
        assert!(f_exp2m1f(f32::NAN).is_nan());
        assert_eq!(f_exp2m1f(f32::INFINITY), f32::INFINITY);
        assert_eq!(f_exp2m1f(f32::NEG_INFINITY), -1.0);
    }
}
