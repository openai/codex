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
    ACOSH_ASINH_B, ACOSH_ASINH_L1, ACOSH_ASINH_L2, ACOSH_ASINH_LL, ACOSH_ASINH_R1, ACOSH_ASINH_R2,
    ACOSH_ASINH_REFINE_T2, ACOSH_ASINH_REFINE_T4, ACOSH_SINH_REFINE_T1, ACOSH_SINH_REFINE_T3,
    lpoly_xd_generic,
};

#[cold]
fn asinh_refine(x: f64, a: f64, z: DoubleDouble) -> f64 {
    let mut t = z.hi.to_bits();
    let ex: i32 = (t >> 52) as i32;
    let e = ex.wrapping_sub(0x3ff) + if z.lo == 0.0 { 1i32 } else { 0i32 };
    t &= 0x000fffffffffffff;
    t |= 0x3ffu64 << 52;
    let ed = e as f64;
    let v = (a - ed + f64::from_bits(0x3ff0000800000000)).to_bits();
    let i = (v.wrapping_sub(0x3ffu64 << 52)) >> (52 - 16);
    let i1 = (i >> 12) & 0x1f;
    let i2 = (i >> 8) & 0xf;
    let i3 = (i >> 4) & 0xf;
    let i4 = i & 0xf;
    const L20: f64 = f64::from_bits(0x3fd62e42fefa3800);
    const L21: f64 = f64::from_bits(0x3d1ef35793c76800);
    const L22: f64 = f64::from_bits(0xba49ff0342542fc3);
    let el2 = L22 * ed;
    let el1 = L21 * ed;
    let el0 = L20 * ed;
    let mut dl0: f64;

    let ll0i1 = ACOSH_ASINH_LL[0][i1 as usize];
    let ll1i2 = ACOSH_ASINH_LL[1][i2 as usize];
    let ll2i3 = ACOSH_ASINH_LL[2][i3 as usize];
    let ll3i4 = ACOSH_ASINH_LL[3][i4 as usize];

    dl0 = f64::from_bits(ll0i1.0)
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
    let tl = dd_fmla(t12, t34, -th);
    let dh = th * f64::from_bits(t);
    let dl = dd_fmla(th, f64::from_bits(t), -dh);
    let sh = tl * f64::from_bits(t);
    let sl = dd_fmla(tl, f64::from_bits(t), -sh);

    let mut dx = DoubleDouble::from_exact_add(dh - 1., dl);
    if z.lo != 0.0 {
        t = z.lo.to_bits();
        t = t.wrapping_sub((e as i64).wrapping_shl(52) as u64);
        dx.lo += th * f64::from_bits(t);
    }
    dx = DoubleDouble::add(dx, DoubleDouble::new(sl, sh));
    const CL: [u64; 3] = [0xbfc0000000000000, 0x3fb9999999a0754f, 0xbfb55555555c3157];

    let sl0 = f_fmla(dx.hi, f64::from_bits(CL[2]), f64::from_bits(CL[1]));
    let sl1 = f_fmla(dx.hi, sl0, f64::from_bits(CL[0]));

    let sl = dx.hi * sl1;
    const CH: [(u64, u64); 3] = [
        (0x39024b67ee516e3b, 0x3fe0000000000000),
        (0xb91932ce43199a8d, 0xbfd0000000000000),
        (0x3c655540c15cf91f, 0x3fc5555555555555),
    ];
    let mut s = lpoly_xd_generic(dx, CH, sl);
    s = DoubleDouble::quick_mult(dx, s);
    s = DoubleDouble::add(s, DoubleDouble::new(el2, el1));
    s = DoubleDouble::add(s, DoubleDouble::new(dl2, dl1));
    let mut v02 = DoubleDouble::from_exact_add(dl0, s.hi);
    let mut v12 = DoubleDouble::from_exact_add(v02.lo, s.lo);
    let scale = f64::copysign(2., x);
    v02.hi *= scale;
    v12.hi *= scale;
    v12.lo *= scale;
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
    v02.hi + v12.hi
}

#[cold]
fn as_asinh_zero(x: f64, x2h: f64, x2l: f64) -> f64 {
    static CH: [(u64, u64); 12] = [
        (0xbc65555555555555, 0xbfc5555555555555),
        (0x3c499999999949df, 0x3fb3333333333333),
        (0x3c32492496091b0c, 0xbfa6db6db6db6db7),
        (0x3c1c71a35cfa0671, 0x3f9f1c71c71c71c7),
        (0x3c317f937248cf81, 0xbf96e8ba2e8ba2e9),
        (0xbc374e3c1dfd4c3d, 0x3f91c4ec4ec4ec4f),
        (0xbc238e7a467ecc55, 0xbf8c999999999977),
        (0x3c2a83c7bace55eb, 0x3f87a87878786c7e),
        (0xbc2d024df7fa0542, 0xbf83fde50d764083),
        (0xbc2ba9c13deb261f, 0x3f812ef3ceae4d12),
        (0xbc1546da9bc5b32a, 0xbf7df3bd104aa267),
        (0x3c140d284a1d67f9, 0x3f7a685fc5de7a04),
    ];

    const CL: [u64; 5] = [
        0xbf77828d553ec800,
        0x3f751712f7bee368,
        0xbf72e6d98527bcc6,
        0x3f70095da47b392c,
        0xbf63b92d6368192c,
    ];

    let yw0 = f_fmla(x2h, f64::from_bits(CL[4]), f64::from_bits(CL[3]));
    let yw1 = f_fmla(x2h, yw0, f64::from_bits(CL[2]));
    let yw2 = f_fmla(x2h, yw1, f64::from_bits(CL[1]));

    let y2 = x2h * f_fmla(x2h, yw2, f64::from_bits(CL[0]));
    let mut y1 = lpoly_xd_generic(DoubleDouble::new(x2l, x2h), CH, y2);
    y1 = DoubleDouble::quick_mult(y1, DoubleDouble::new(x2l, x2h));
    y1 = DoubleDouble::quick_mult_f64(y1, x);

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

/// Huperbolic sine function
///
/// Max ULP 0.5
pub fn f_asinh(x: f64) -> f64 {
    let ax = x.abs();
    let ix = ax.to_bits();
    let u = ix;
    if u < 0x3fbb000000000000u64 {
        // |x| < 0x1.bp-4
        // for |x| < 0x1.7137449123ef7p-26, asinh(x) rounds to x to nearest
        // for |x| < 0x1p-1022 we have underflow but not for 0x1p-1022 (to nearest)
        if u < 0x3e57137449123ef7u64 {
            // |x| < 0x1.7137449123ef7p-26
            if u == 0 {
                return x;
            }
            let res = f_fmla(f64::from_bits(0xbc30000000000000), x, x);
            return res;
        }
        let x2h = x * x;
        let x2l = dd_fmla(x, x, -x2h);
        let x3h = x2h * x;
        let sl;
        if u < 0x3f93000000000000u64 {
            // |x| < 0x1.3p-6
            if u < 0x3f30000000000000u64 {
                // |x| < 0x1p-12
                if u < 0x3e5a000000000000u64 {
                    // |x| < 0x1.ap-26
                    sl = x3h * f64::from_bits(0xbfc5555555555555);
                } else {
                    const CL: [u64; 2] = [0xbfc5555555555555, 0x3fb3333327c57c60];
                    let sl0 = f_fmla(x2h, f64::from_bits(CL[1]), f64::from_bits(CL[0]));
                    sl = x3h * sl0;
                }
            } else {
                const CL: [u64; 4] = [
                    0xbfc5555555555555,
                    0x3fb333333332f2ff,
                    0xbfa6db6d9a665159,
                    0x3f9f186866d775f0,
                ];
                let sl0 = f_fmla(x2h, f64::from_bits(CL[3]), f64::from_bits(CL[2]));
                let sl1 = f_fmla(x2h, sl0, f64::from_bits(CL[1]));
                sl = x3h * f_fmla(x2h, sl1, f64::from_bits(CL[0]));
            }
        } else {
            const CL: [u64; 7] = [
                0xbfc5555555555555,
                0x3fb3333333333310,
                0xbfa6db6db6da466c,
                0x3f9f1c71c2ea7be4,
                0xbf96e8b651b09d72,
                0x3f91c309fc0e69c2,
                0xbf8bab7833c1e000,
            ];
            let c1 = f_fmla(x2h, f64::from_bits(CL[2]), f64::from_bits(CL[1]));
            let c3 = f_fmla(x2h, f64::from_bits(CL[4]), f64::from_bits(CL[3]));
            let c5 = f_fmla(x2h, f64::from_bits(CL[6]), f64::from_bits(CL[5]));
            let x4 = x2h * x2h;

            let sl0 = f_fmla(x4, c5, c3);
            let sl1 = f_fmla(x4, sl0, c1);

            sl = x3h * f_fmla(x2h, sl1, f64::from_bits(CL[0]));
        }
        let eps = f64::from_bits(0x3ca6000000000000) * x3h;
        let lb = x + (sl - eps);
        let ub = x + (sl + eps);
        if lb == ub {
            return lb;
        }
        return as_asinh_zero(x, x2h, x2l);
    }

    // |x| >= 0x1.bp-4
    let mut x2h: f64 = 0.;
    let mut x2l: f64 = 0.;
    let mut off: i32 = 0x3ff;

    let va: DoubleDouble = if u < 0x4190000000000000 {
        // x < 0x1p+26
        x2h = x * x;
        x2l = dd_fmla(x, x, -x2h);
        let mut dt = if u < 0x3ff0000000000000 {
            DoubleDouble::from_exact_add(1., x2h)
        } else {
            DoubleDouble::from_exact_add(x2h, 1.)
        };
        dt.lo += x2l;

        let ah = dt.hi.sqrt();
        let rs = 0.5 / dt.hi;
        let al = (dt.lo - dd_fmla(ah, ah, -dt.hi)) * (rs * ah);
        let mut ma = DoubleDouble::from_exact_add(ah, ax);
        ma.lo += al;
        ma
    } else if u < 0x4330000000000000 {
        DoubleDouble::new(0.5 / ax, 2. * ax)
    } else {
        if u >= 0x7ff0000000000000u64 {
            return x + x;
        } // +-inf or nan
        off = 0x3fe;
        DoubleDouble::new(0., ax)
    };

    let mut t = va.hi.to_bits();
    let ex = (t >> 52) as i32;
    let e = ex.wrapping_sub(off);
    t &= 0x000fffffffffffff;
    let ed = e as f64;
    let i = t >> (52 - 5);
    let d = (t & 0x00007fffffffffff) as i64;
    let b_i = ACOSH_ASINH_B[i as usize];
    let j: u64 = t
        .wrapping_add((b_i[0] as u64).wrapping_shl(33))
        .wrapping_add((b_i[1] as i64).wrapping_mul(d >> 16) as u64)
        >> (52 - 10);
    t |= 0x3ffu64 << 52;
    let i1 = (j >> 5) as i32;
    let i2 = j & 0x1f;
    let r =
        f64::from_bits(ACOSH_ASINH_R1[i1 as usize]) * f64::from_bits(ACOSH_ASINH_R2[i2 as usize]);
    let dx = dd_fmla(r, f64::from_bits(t), -1.);
    let dx2 = dx * dx;
    const C: [u64; 5] = [
        0xbfe0000000000000,
        0x3fd5555555555530,
        0xbfcfffffffffffa0,
        0x3fc99999e33a6366,
        0xbfc555559ef9525f,
    ];

    let fw0 = f_fmla(dx, f64::from_bits(C[3]), f64::from_bits(C[2]));
    let fw1 = f_fmla(dx, f64::from_bits(C[1]), f64::from_bits(C[0]));
    let fw2 = f_fmla(dx2, f64::from_bits(C[4]), fw0);

    let f = dx2 * f_fmla(dx2, fw2, fw1);
    const L2H: f64 = f64::from_bits(0x3fe62e42fefa3800);
    const L2L: f64 = f64::from_bits(0x3d2ef35793c76730);
    let l1i1 = ACOSH_ASINH_L1[i1 as usize];
    let l1i2 = ACOSH_ASINH_L2[i2 as usize];
    let mut lh = f_fmla(L2H, ed, f64::from_bits(l1i1.1) + f64::from_bits(l1i2.1));
    let mut ll = f_fmla(
        L2L,
        ed,
        f64::from_bits(l1i1.0) + f64::from_bits(l1i2.0) + va.lo / va.hi + f,
    );
    ll += dx;
    lh *= f64::copysign(1., x);
    ll *= f64::copysign(1., x);
    let eps = 1.63e-19;
    let lb = lh + (ll - eps);
    let ub = lh + (ll + eps);
    if lb == ub {
        return lb;
    }
    if ax < f64::from_bits(0x3fd0000000000000) {
        return as_asinh_zero(x, x2h, x2l);
    }
    asinh_refine(x, f64::from_bits(0x3ff71547652b82fe) * lb.abs(), va)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_asinh() {
        assert_eq!(f_asinh(-0.05268859863273256), -0.05266425100170862);
        assert_eq!(f_asinh(1.05268859863273256), 0.9181436936151385);
    }
}
