/*
 * // Copyright (c) Radzivon Bartoshyk 7/2025. All rights reserved.
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
use crate::common::{dd_fmla, f_fmla};
use crate::double_double::DoubleDouble;
use crate::hyperbolic::acosh::{
    ACOSH_ASINH_B, ACOSH_ASINH_LL, ACOSH_ASINH_R1, ACOSH_ASINH_R2, ACOSH_ASINH_REFINE_T2,
    ACOSH_ASINH_REFINE_T4, ACOSH_SINH_REFINE_T1, ACOSH_SINH_REFINE_T3, lpoly_xd_generic,
};

static ATANH_L1: [(u64, u64); 33] = [
    (0x0000000000000000, 0x0000000000000000),
    (0xbe4532c1269e2038, 0x3f862e5000000000),
    (0x3e4ce42d81b54e84, 0x3f962e3c00000000),
    (0xbe525826f815ec3d, 0x3fa0a2ac00000000),
    (0x3e50db1b1e7cee11, 0x3fa62e4a00000000),
    (0xbe51f3a8c6c95003, 0x3fabb9dc00000000),
    (0xbe5774cd4fb8c30d, 0x3fb0a2b200000000),
    (0x3e2452e56c030a0a, 0x3fb3687f00000000),
    (0x3e36b63c4966a79a, 0x3fb62e4100000000),
    (0xbe3b20a21ccb525e, 0x3fb8f40a00000000),
    (0x3e54006cfb3d8f85, 0x3fbbb9d100000000),
    (0xbe5cdb026b310c41, 0x3fbe7f9b00000000),
    (0xbe569124fdc0f16d, 0x3fc0a2b080000000),
    (0xbe5084656cdc2727, 0x3fc2059580000000),
    (0xbe5376fa8b0357fd, 0x3fc3687c00000000),
    (0x3e3e56ae55a47b4a, 0x3fc4cb5e80000000),
    (0x3e5070ff8834eeb4, 0x3fc62e4400000000),
    (0x3e5623516109f4fe, 0x3fc7912900000000),
    (0xbe2ec656b95fbdac, 0x3fc8f40b00000000),
    (0x3e3f0ca2e729f510, 0x3fca56ed80000000),
    (0xbe57d260a858354a, 0x3fcbb9d680000000),
    (0x3e4e7279075503d3, 0x3fcd1cb900000000),
    (0x3e439e1a0a503873, 0x3fce7f9d00000000),
    (0x3e5cd86d7b87c3d6, 0x3fcfe27d80000000),
    (0x3e5060ab88de341e, 0x3fd0a2b240000000),
    (0x3e320a860d3f9390, 0x3fd1542440000000),
    (0xbe4dacee95fc2f10, 0x3fd2059740000000),
    (0x3e545de3a86e0aca, 0x3fd2b70700000000),
    (0x3e4c164cbfb991af, 0x3fd3687b00000000),
    (0x3e5d3f66b24225ef, 0x3fd419ec40000000),
    (0x3e5fc023efa144ba, 0x3fd4cb5f80000000),
    (0x3e3086a8af6f26c0, 0x3fd57cd280000000),
    (0xbe105c610ca86c39, 0x3fd62e4300000000),
];

static ATANH_L2: [(u64, u64); 33] = [
    (0x0000000000000000, 0x0000000000000000),
    (0xbe337e152a129e4e, 0x3f36320000000000),
    (0xbe53f6c916b8be9c, 0x3f46300000000000),
    (0x3e520505936739d5, 0x3f50a24000000000),
    (0xbe523e2e8cb541ba, 0x3f562dc000000000),
    (0xbdfacb7983ac4f5e, 0x3f5bba0000000000),
    (0x3e36f7c7689c63ae, 0x3f60a2a000000000),
    (0x3e1f5ca695b4c58b, 0x3f6368c000000000),
    (0xbe4c6c18bd953226, 0x3f662e6000000000),
    (0x3e57a516c34846bd, 0x3f68f46000000000),
    (0xbe4f3b83dd8b8530, 0x3f6bba0000000000),
    (0xbe0c3459046e4e57, 0x3f6e800000000000),
    (0x3d9b5c7e34cb79f6, 0x3f70a2c000000000),
    (0xbe42487e9af9a692, 0x3f7205c000000000),
    (0x3e5f21bbc4ad79ce, 0x3f73687000000000),
    (0xbe2550ffc857b731, 0x3f74cb7000000000),
    (0x3e487458ec1b7b34, 0x3f762e2000000000),
    (0x3e5103d4fe83ee81, 0x3f77911000000000),
    (0x3e4810483d3b398c, 0x3f78f44000000000),
    (0xbe42085cb340608e, 0x3f7a573000000000),
    (0x3e512698a119c42f, 0x3f7bb9d000000000),
    (0xbe5edb8c172b4c33, 0x3f7d1cc000000000),
    (0xbe58b55b87a5e238, 0x3f7e7fe000000000),
    (0x3e5be5e17763f78a, 0x3f7fe2b000000000),
    (0xbe1c2d496790073e, 0x3f80a2a800000000),
    (0x3e56542f523abeec, 0x3f81541000000000),
    (0xbe5b7fdbe5b193f8, 0x3f8205a000000000),
    (0x3e5fa4d42fe30c7c, 0x3f82b70000000000),
    (0x3e50d46ad04adc86, 0x3f83688800000000),
    (0xbe51c22d02d17c4c, 0x3f8419f000000000),
    (0x3e1a7d1e330dccce, 0x3f84cb7000000000),
    (0x3e0187025e656ba3, 0x3f857cd000000000),
    (0xbe4532c1269e2038, 0x3f862e5000000000),
];

#[cold]
fn as_atanh_zero(x: f64) -> f64 {
    static CH: [(u64, u64); 13] = [
        (0x3c75555555555555, 0x3fd5555555555555),
        (0xbc6999999999611c, 0x3fc999999999999a),
        (0x3c62492490f76b25, 0x3fc2492492492492),
        (0x3c5c71cd5c38a112, 0x3fbc71c71c71c71c),
        (0xbc47556c4165f4ca, 0x3fb745d1745d1746),
        (0xbc4b893c3b36052e, 0x3fb3b13b13b13b14),
        (0x3c44e1afd723ed1f, 0x3fb1111111111105),
        (0xbc4f86ea96fb1435, 0x3fae1e1e1e1e2678),
        (0x3c31e51a6e54fde9, 0x3faaf286bc9f90cc),
        (0xbc2ab913de95c3bf, 0x3fa8618618c779b6),
        (0x3c4632e747641b12, 0x3fa642c84aa383eb),
        (0xbc30c9617e7bcff2, 0x3fa47ae2d205013c),
        (0x3c23adb3e2b7f35e, 0x3fa2f664d60473f9),
    ];

    const CL: [u64; 5] = [
        0x3fa1a9a91fd692af,
        0x3fa06dfbb35e7f44,
        0x3fa037bed4d7588f,
        0x3f95aca6d6d720d6,
        0x3fa99ea5700d53a5,
    ];

    let dx2 = DoubleDouble::from_exact_mult(x, x);

    let yw0 = f_fmla(dx2.hi, f64::from_bits(CL[4]), f64::from_bits(CL[3]));
    let yw1 = f_fmla(dx2.hi, yw0, f64::from_bits(CL[2]));
    let yw2 = f_fmla(dx2.hi, yw1, f64::from_bits(CL[1]));

    let y2 = dx2.hi * f_fmla(dx2.hi, yw2, f64::from_bits(CL[0]));

    let mut y1 = lpoly_xd_generic(dx2, CH, y2);
    y1 = DoubleDouble::quick_mult_f64(y1, x);
    y1 = DoubleDouble::quick_mult(y1, dx2);

    let y0 = DoubleDouble::from_exact_add(x, y1.hi);
    let mut p = DoubleDouble::from_exact_add(y0.lo, y1.lo);

    let mut t = p.hi.to_bits();
    if (t & 0x000fffffffffffff) == 0 {
        let w = p.lo.to_bits();
        if ((w ^ t) >> 63) != 0 {
            t = t.wrapping_sub(1);
        } else {
            t = t.wrapping_add(1);
        }
        p.hi = f64::from_bits(t);
    }
    y0.hi + p.hi
}

#[cold]
fn atanh_refine(x: f64, a: f64, z: DoubleDouble) -> f64 {
    let mut t = z.hi.to_bits();
    let ex: i32 = (t >> 52) as i32;
    let e = ex.wrapping_sub(0x3ff);
    t &= 0x000fffffffffffff;
    t |= 0x3ffu64 << 52;
    let ed = e as f64;
    let v = (a - ed + f64::from_bits(0x3ff0000800000000)).to_bits();
    let i = (v.wrapping_sub(0x3ffu64 << 52)) >> (52 - 16);
    let i1: i32 = ((i >> 12) as i32) & 0x1f;
    let i2 = (i >> 8) & 0xf;
    let i3 = (i >> 4) & 0xf;
    let i4 = i & 0xf;

    const L20: f64 = f64::from_bits(0x3fd62e42fefa3a00);
    const L21: f64 = f64::from_bits(0xbcd0ca86c3898d00);
    const L22: f64 = f64::from_bits(0x397f97b57a079a00);

    let el2 = L22 * ed;
    let el1 = L21 * ed;
    let el0 = L20 * ed;

    let ll0i1 = ACOSH_ASINH_LL[0][i1 as usize];
    let ll1i2 = ACOSH_ASINH_LL[1][i2 as usize];
    let ll2i3 = ACOSH_ASINH_LL[2][i3 as usize];
    let ll3i4 = ACOSH_ASINH_LL[3][i4 as usize];

    let mut dl0 = f64::from_bits(ll0i1.0)
        + f64::from_bits(ll1i2.0)
        + (f64::from_bits(ll2i3.0) + f64::from_bits(ll3i4.0));
    let dl1 = f64::from_bits(ll0i1.1)
        + f64::from_bits(ll1i2.1)
        + (f64::from_bits(ll2i3.1) + f64::from_bits(ll3i4.1));
    let dl2 = f64::from_bits(ll0i1.2)
        + f64::from_bits(ll1i2.2)
        + (f64::from_bits(ll2i3.2) + f64::from_bits(ll3i4.2));
    dl0 += el0;

    let t12 = f64::from_bits(ACOSH_SINH_REFINE_T1[i1 as usize])
        * f64::from_bits(ACOSH_ASINH_REFINE_T2[i2 as usize]);
    let t34 = f64::from_bits(ACOSH_SINH_REFINE_T3[i3 as usize])
        * f64::from_bits(ACOSH_ASINH_REFINE_T4[i4 as usize]);
    let th = t12 * t34;
    let tl = f_fmla(t12, t34, -th);
    let dh = th * f64::from_bits(t);
    let dl = f_fmla(th, f64::from_bits(t), -dh);
    let sh = tl * f64::from_bits(t);
    let sl = f_fmla(tl, f64::from_bits(t), -sh);
    let mut dx = DoubleDouble::from_exact_add(dh - 1., dl);

    t = z.lo.to_bits();
    t = t.wrapping_sub(((e as i64) << 52) as u64);
    dx.lo += th * f64::from_bits(t);

    dx = DoubleDouble::add(dx, DoubleDouble::new(sl, sh));
    const CL: [u64; 3] = [0xbfc0000000000000, 0x3fb9999999a0754f, 0xbfb55555555c3157];

    let slw0 = f_fmla(dx.hi, f64::from_bits(CL[2]), f64::from_bits(CL[1]));

    let sl = dx.hi * f_fmla(dx.hi, slw0, f64::from_bits(CL[0]));

    static CH: [(u64, u64); 3] = [
        (0x39024b67ee516e3b, 0x3fe0000000000000),
        (0xb91932ce43199a8d, 0xbfd0000000000000),
        (0x3c655540c15cf91f, 0x3fc5555555555555),
    ];

    let mut s = lpoly_xd_generic(dx, CH, sl);
    s = DoubleDouble::mult(dx, s);
    s = DoubleDouble::add(s, DoubleDouble::new(el2, el1));
    s = DoubleDouble::add(s, DoubleDouble::new(dl2, dl1));
    let mut v02 = DoubleDouble::from_exact_add(dl0, s.hi);
    let mut v12 = DoubleDouble::from_exact_add(v02.lo, s.lo);
    t = v12.hi.to_bits();
    if (t & 0x000fffffffffffff) == 0 {
        let w = v12.lo.to_bits();
        if ((w ^ t) >> 63) != 0 {
            t = t.wrapping_sub(1);
        } else {
            t = t.wrapping_add(1);
        }
        v12.hi = f64::from_bits(t);
    }
    v02.hi *= f64::copysign(1., x);
    v12.hi *= f64::copysign(1., x);

    v02.hi + v12.hi
}

/// Hyperbolic tangent
///
/// Max ULP 0.5
pub fn f_atanh(x: f64) -> f64 {
    let ax = x.abs();
    let ix = ax.to_bits();
    let aix = ix;
    if aix >= 0x3ff0000000000000u64 {
        // |x| >= 1
        if aix == 0x3ff0000000000000u64 {
            // |x| = 1
            return f64::copysign(1., x) / 0.0;
        }
        if aix > 0x7ff0000000000000u64 {
            return x + x;
        } // nan
        return (-1.0f64).sqrt();
    }

    if aix < 0x3fd0000000000000u64 {
        // |x| < 0x1p-2
        // atanh(x) rounds to x to nearest for |x| < 0x1.d12ed0af1a27fp-27
        if aix < 0x3e4d12ed0af1a27fu64 {
            // |x| < 0x1.d12ed0af1a27fp-27
            /* We have underflow exactly when 0 < |x| < 2^-1022:
            for RNDU, atanh(2^-1022-2^-1074) would round to 2^-1022-2^-1075
            with unbounded exponent range */
            return f_fmla(x, f64::from_bits(0x3c80000000000000), x);
        }
        let x2 = x * x;
        const C: [u64; 9] = [
            0x3fc999999999999a,
            0x3fc2492492492244,
            0x3fbc71c71c79715f,
            0x3fb745d16f777723,
            0x3fb3b13ca4174634,
            0x3fb110c9724989bd,
            0x3fae2d17608a5b2e,
            0x3faa0b56308cba0b,
            0x3fafb6341208ad2e,
        ];
        let dx2: f64 = dd_fmla(x, x, -x2);

        let x4 = x2 * x2;
        let x3 = x2 * x;
        let x8 = x4 * x4;

        let zdx3: f64 = dd_fmla(x2, x, -x3);

        let dx3 = f_fmla(dx2, x, zdx3);

        let pw0 = f_fmla(x2, f64::from_bits(C[7]), f64::from_bits(C[6]));
        let pw1 = f_fmla(x2, f64::from_bits(C[5]), f64::from_bits(C[4]));
        let pw2 = f_fmla(x2, f64::from_bits(C[3]), f64::from_bits(C[2]));
        let pw3 = f_fmla(x2, f64::from_bits(C[1]), f64::from_bits(C[0]));

        let pw4 = f_fmla(x4, pw0, pw1);
        let pw5 = f_fmla(x8, f64::from_bits(C[8]), pw4);
        let pw6 = f_fmla(x4, pw2, pw3);

        let p = f_fmla(x8, pw5, pw6);
        let t = f64::from_bits(0x3c75555555555555) + x2 * p;
        let mut p = DoubleDouble::from_exact_add(f64::from_bits(0x3fd5555555555555), t);
        p = DoubleDouble::mult(p, DoubleDouble::new(dx3, x3));
        let z0 = DoubleDouble::from_exact_add(x, p.hi);
        p.lo += z0.lo;
        p.hi = z0.hi;
        let eps = x * f_fmla(
            x4,
            f64::from_bits(0x3cad000000000000),
            f64::from_bits(0x3980000000000000),
        );
        let lb = p.hi + (p.lo - eps);
        let ub = p.hi + (p.lo + eps);
        if lb == ub {
            return lb;
        }
        return as_atanh_zero(x);
    }

    let p = DoubleDouble::from_exact_add(1.0, ax);
    let q = DoubleDouble::from_exact_sub(1.0, ax);
    let iqh = 1.0 / q.hi;
    let th = p.hi * iqh;
    let tl = dd_fmla(p.hi, iqh, -th) + (p.lo + p.hi * (dd_fmla(-q.hi, iqh, 1.) - q.lo * iqh)) * iqh;

    const C: [u64; 5] = [
        0xbff0000000000000,
        0x3ff5555555555530,
        0xbfffffffffffffa0,
        0x40099999e33a6366,
        0xc01555559ef9525f,
    ];

    let mut t = th.to_bits();
    let ex: i32 = (t >> 52) as i32;
    let e = ex.wrapping_sub(0x3ff);
    t &= 0x000fffffffffffff;
    let ed = e as f64;
    let i = t >> (52 - 5);
    let d: i64 = (t & 0x00007fffffffffff) as i64;
    let b_i = ACOSH_ASINH_B[i as usize];
    let j: u64 = t
        .wrapping_add((b_i[0] as u64).wrapping_shl(33))
        .wrapping_add((b_i[1] as i64).wrapping_mul(d >> 16) as u64)
        >> (52 - 10);
    t |= 0x3ffu64 << 52;
    let i1: i32 = (j >> 5) as i32;
    let i2 = j & 0x1f;
    let r = (0.5 * f64::from_bits(ACOSH_ASINH_R1[i1 as usize]))
        * f64::from_bits(ACOSH_ASINH_R2[i2 as usize]);
    let dx = dd_fmla(r, f64::from_bits(t), -0.5);
    let dx2 = dx * dx;
    let rx = r * f64::from_bits(t);
    let dxl = dd_fmla(r, f64::from_bits(t), -rx);

    let fw0 = f_fmla(dx, f64::from_bits(C[3]), f64::from_bits(C[2]));
    let fw2 = f_fmla(dx, f64::from_bits(C[1]), f64::from_bits(C[0]));
    let fw1 = f_fmla(dx2, f64::from_bits(C[4]), fw0);

    let f = dx2 * f_fmla(dx2, fw1, fw2);
    const L2H: f64 = f64::from_bits(0x3fd62e42fefa3a00);
    const L2L: f64 = f64::from_bits(0xbcd0ca86c3898d00);
    let l1i1 = ATANH_L1[i1 as usize];
    let l1i2 = ATANH_L2[i2 as usize];
    let lh = f_fmla(L2H, ed, f64::from_bits(l1i1.1) + f64::from_bits(l1i2.1));
    let mut zl = DoubleDouble::from_exact_add(lh, rx - 0.5);

    zl.lo += f_fmla(L2L, ed, f64::from_bits(l1i1.0) + f64::from_bits(l1i2.0)) + dxl + 0.5 * tl / th;
    zl.lo += f;
    zl.hi *= f64::copysign(1., x);
    zl.lo *= f64::copysign(1., x);
    let eps = 31e-24 + dx2 * f64::from_bits(0x3ce0000000000000);
    let lb = zl.hi + (zl.lo - eps);
    let ub = zl.hi + (zl.lo + eps);
    if lb == ub {
        return lb;
    }
    let t = DoubleDouble::from_exact_add(th, tl);
    atanh_refine(
        x,
        f64::from_bits(0x40071547652b82fe) * (zl.hi + zl.lo).abs(),
        t,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_atanh() {
        assert_eq!(f_atanh(-0.5000824928283691), -0.5494161408216048);
        assert_eq!(f_atanh(-0.24218760943040252), -0.24709672810738792);
    }
}
