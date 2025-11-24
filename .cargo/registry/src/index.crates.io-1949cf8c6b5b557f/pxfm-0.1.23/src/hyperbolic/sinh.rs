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
use crate::exponents::{EXP_REDUCE_T0, EXP_REDUCE_T1};
use crate::hyperbolic::acosh::lpoly_xd_generic;

#[cold]
pub(crate) fn hyperbolic_exp_accurate(x: f64, t: f64, zt: DoubleDouble) -> DoubleDouble {
    static CH: [(u64, u64); 3] = [
        (0x3a16c16bd194535d, 0x3ff0000000000000),
        (0xba28259d904fd34f, 0x3fe0000000000000),
        (0x3c653e93e9f26e62, 0x3fc5555555555555),
    ];
    const L2H: f64 = f64::from_bits(0x3f262e42ff000000);
    const L2L: f64 = f64::from_bits(0x3d0718432a1b0e26);
    const L2LL: f64 = f64::from_bits(0x3999ff0342542fc3);
    let dx = x - L2H * t;
    let mut dxl = L2L * t;
    let dxll = f_fmla(L2LL, t, dd_fmla(L2L, t, -dxl));
    let dxh = dx + dxl;
    dxl = ((dx - dxh) + dxl) + dxll;

    let fl0 = f_fmla(
        dxh,
        f64::from_bits(0x3f56c16c169400a7),
        f64::from_bits(0x3f811111113e93e9),
    );

    let fl = dxh * f_fmla(dxh, fl0, f64::from_bits(0x3fa5555555555555));
    let mut f = lpoly_xd_generic(DoubleDouble::new(dxl, dxh), CH, fl);
    f = DoubleDouble::quick_mult(DoubleDouble::new(dxl, dxh), f);
    f = DoubleDouble::quick_mult(zt, f);
    let zh = zt.hi + f.hi;
    let zl = (zt.hi - zh) + f.hi;
    let uh = zh + zt.lo;
    let ul = ((zh - uh) + zt.lo) + zl;
    let vh = uh + f.lo;
    let vl = ((uh - vh) + f.lo) + ul;
    DoubleDouble::new(vl, vh)
}

#[cold]
fn as_sinh_zero(x: f64) -> f64 {
    static CH: [(u64, u64); 5] = [
        (0x3c6555555555552f, 0x3fc5555555555555),
        (0x3c011111115cf00d, 0x3f81111111111111),
        (0x3b6a0011c925b85c, 0x3f2a01a01a01a01a),
        (0xbb6b4e2835532bcd, 0x3ec71de3a556c734),
        (0xbaedefcf17a6ab79, 0x3e5ae64567f54482),
    ];
    let d2x = DoubleDouble::from_exact_mult(x, x);

    let yw0 = f_fmla(
        d2x.hi,
        f64::from_bits(0x3ce95785063cd974),
        f64::from_bits(0x3d6ae7f36beea815),
    );

    let y2 = d2x.hi * f_fmla(d2x.hi, yw0, f64::from_bits(0x3de6124613aef206));
    let mut y1 = lpoly_xd_generic(d2x, CH, y2);
    y1 = DoubleDouble::quick_mult_f64(y1, x);
    y1 = DoubleDouble::quick_mult(y1, d2x); // y2 = y1.l
    let y0 = DoubleDouble::from_exact_add(x, y1.hi); // y0 = y0.hi
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

/// Hyperbolic sine function
///
/// Max ULP 0.5
pub fn f_sinh(x: f64) -> f64 {
    /*
     The function sinh(x) is approximated by a minimax polynomial for
     |x|<0.25. For other arguments the identity
     sinh(x)=(exp(|x|)-exp(-|x|))/2*copysign(1,x) is used. For |x|<5
     both exponents are calculated with slightly higher precision than
     double. For 5<|x|<36.736801 the exp(-|x|) is small and is
     calculated with double precision but exp(|x|) is calculated with
     higher than double precision. For 36.736801<|x|<710.47586
     exp(-|x|) becomes too small and only exp(|x|) is calculated.
    */

    const S: f64 = f64::from_bits(0x40b71547652b82fe);
    let ax = x.abs();
    let v0 = dd_fmla(ax, S, f64::from_bits(0x4198000002000000));
    let jt = v0.to_bits();
    let v = v0.to_bits() & 0xfffffffffc000000;
    let t = f64::from_bits(v) - f64::from_bits(0x4198000000000000);
    let ix = ax.to_bits();
    let aix = ix;
    if aix < 0x3fd0000000000000u64 {
        // |x| < 0x1p-2
        if aix < 0x3e57137449123ef7u64 {
            // |x| < 0x1.7137449123ef7p-26
            /* We have underflow exactly when 0 < |x| < 2^-1022:
            for RNDU, sinh(2^-1022-2^-1074) would round to 2^-1022-2^-1075
            with unbounded exponent range */
            return dd_fmla(x, f64::from_bits(0x3c80000000000000), x);
        }
        const C: [u64; 5] = [
            0x3fc5555555555555,
            0x3f81111111111087,
            0x3f2a01a01a12e1c3,
            0x3ec71de2e415aa36,
            0x3e5aed2bff4269e6,
        ];
        let x2 = x * x;
        let x3 = x2 * x;
        let x4 = x2 * x2;

        let pw0 = f_fmla(x2, f64::from_bits(C[3]), f64::from_bits(C[2]));
        let pw1 = f_fmla(x2, f64::from_bits(C[1]), f64::from_bits(C[0]));
        let pw2 = f_fmla(x4, f64::from_bits(C[4]), pw0);

        let p = x3 * f_fmla(x4, pw2, pw1);
        let e = x3 * f64::from_bits(0x3ca9000000000000);
        let lb = x + (p - e);
        let ub = x + (p + e);
        if lb == ub {
            return lb;
        }
        return as_sinh_zero(x);
    }

    if aix > 0x408633ce8fb9f87du64 {
        // |x| >~ 710.47586
        if aix >= 0x7ff0000000000000u64 {
            return x + x;
        } // nan Inf
        return f64::copysign(f64::from_bits(0x7fe0000000000000), x) * 2.0;
    }
    let il: i64 = ((jt.wrapping_shl(14)) >> 40) as i64;
    let jl = -il;
    let i1 = il & 0x3f;
    let i0 = (il >> 6) & 0x3f;
    let ie = il >> 12;
    let j1 = jl & 0x3f;
    let j0 = (jl >> 6) & 0x3f;
    let je = jl >> 12;
    let mut sp = (1022i64.wrapping_add(ie) as u64).wrapping_shl(52);
    let sm = (1022i64.wrapping_add(je) as u64).wrapping_shl(52);

    let sn0 = EXP_REDUCE_T0[i0 as usize];
    let sn1 = EXP_REDUCE_T1[i1 as usize];
    let t0h = f64::from_bits(sn0.1);
    let t0l = f64::from_bits(sn0.0);
    let t1h = f64::from_bits(sn1.1);
    let t1l = f64::from_bits(sn1.0);
    let mut th = t0h * t1h;
    let mut tl = f_fmla(t0h, t1l, t1h * t0l) + dd_fmla(t0h, t1h, -th);
    const L2H: f64 = f64::from_bits(0x3f262e42ff000000);
    const L2L: f64 = f64::from_bits(0x3d0718432a1b0e26);
    let dx = f_fmla(L2L, t, f_fmla(-L2H, t, ax));
    let dx2 = dx * dx;
    let mx = -dx;
    const CH: [u64; 4] = [
        0x3ff0000000000000,
        0x3fe0000000000000,
        0x3fc5555555aaaaae,
        0x3fa55555551c98c0,
    ];

    let (mut rl, mut rh);

    let pp0 = f_fmla(dx, f64::from_bits(CH[3]), f64::from_bits(CH[2]));
    let pp1 = f_fmla(dx, f64::from_bits(CH[1]), f64::from_bits(CH[0]));

    let pp = dx * f_fmla(dx2, pp0, pp1);
    if aix > 0x4014000000000000u64 {
        // |x| > 5
        if aix > 0x40425e4f7b2737fau64 {
            // |x| >~ 36.736801
            sp = (1021i64.wrapping_add(ie) as u64).wrapping_shl(52);
            let mut rh = th;
            let mut rl = tl + th * pp;
            rh *= f64::copysign(1., x);
            rl *= f64::copysign(1., x);
            let e = 0.11e-18 * th;
            let lb = rh + (rl - e);
            let ub = rh + (rl + e);
            if lb == ub {
                return (lb * f64::from_bits(sp)) * 2.;
            }
            let mut tt = hyperbolic_exp_accurate(ax, t, DoubleDouble::new(tl, th));
            tt = DoubleDouble::from_exact_add(tt.hi, tt.lo);
            th = tt.hi;
            tl = tt.lo;
            th *= f64::copysign(1., x);
            tl *= f64::copysign(1., x);
            th += tl;
            th *= 2.;
            th *= f64::from_bits(sp);
            return th;
        }

        let q0h = f64::from_bits(EXP_REDUCE_T0[j0 as usize].1);
        let q1h = f64::from_bits(EXP_REDUCE_T1[j1 as usize].1);
        let mut qh = q0h * q1h;
        th *= f64::from_bits(sp);
        tl *= f64::from_bits(sp);
        qh *= f64::from_bits(sm);

        let pm0 = f_fmla(mx, f64::from_bits(CH[3]), f64::from_bits(CH[2]));
        let pm1 = f_fmla(mx, f64::from_bits(CH[1]), f64::from_bits(CH[0]));

        let pm = mx * f_fmla(dx2, pm0, pm1);
        let em = f_fmla(qh, pm, qh);
        rh = th;
        rl = f_fmla(th, pp, tl - em);

        rh *= f64::copysign(1., x);
        rl *= f64::copysign(1., x);
        let e = 0.09e-18 * rh;
        let lb = rh + (rl - e);
        let ub = rh + (rl + e);
        if lb == ub {
            return lb;
        }

        let tt = hyperbolic_exp_accurate(ax, t, DoubleDouble::new(tl, th));
        th = tt.hi;
        tl = tt.lo;
        if aix > 0x403f666666666666u64 {
            rh = th - qh;
            rl = ((th - rh) - qh) + tl;
        } else {
            qh = q0h * q1h;
            let q0l = f64::from_bits(EXP_REDUCE_T0[j0 as usize].0);
            let q1l = f64::from_bits(EXP_REDUCE_T1[j1 as usize].0);
            let mut ql = f_fmla(q0h, q1l, q1h * q0l) + dd_fmla(q0h, q1h, -qh);
            qh *= f64::from_bits(sm);
            ql *= f64::from_bits(sm);
            let qq = hyperbolic_exp_accurate(-ax, -t, DoubleDouble::new(ql, qh));
            rh = th - qq.hi;
            rl = (((th - rh) - qq.hi) - qq.lo) + tl;
        }
    } else {
        let tq0 = EXP_REDUCE_T0[j0 as usize];
        let tq1 = EXP_REDUCE_T1[j1 as usize];
        let q0h = f64::from_bits(tq0.1);
        let q0l = f64::from_bits(tq0.0);
        let q1h = f64::from_bits(tq1.1);
        let q1l = f64::from_bits(tq1.0);
        let mut qh = q0h * q1h;
        let mut ql = f_fmla(q0h, q1l, q1h * q0l) + dd_fmla(q0h, q1h, -qh);
        th *= f64::from_bits(sp);
        tl *= f64::from_bits(sp);
        qh *= f64::from_bits(sm);
        ql *= f64::from_bits(sm);

        let pm0 = f_fmla(mx, f64::from_bits(CH[3]), f64::from_bits(CH[2]));
        let pm1 = f_fmla(mx, f64::from_bits(CH[1]), f64::from_bits(CH[0]));

        let pm = mx * f_fmla(dx2, pm0, pm1);
        let fph = th;
        let fpl = f_fmla(th, pp, tl);
        let fmh = qh;
        let fml = f_fmla(qh, pm, ql);

        rh = fph - fmh;
        rl = ((fph - rh) - fmh) - fml + fpl;
        rh *= f64::copysign(1., x);
        rl *= f64::copysign(1., x);
        let e = 0.28e-18 * rh;
        let lb = rh + (rl - e);
        let ub = rh + (rl + e);
        if lb == ub {
            return lb;
        }
        let tt = hyperbolic_exp_accurate(ax, t, DoubleDouble::new(tl, th));
        let qq = hyperbolic_exp_accurate(-ax, -t, DoubleDouble::new(ql, qh));
        rh = tt.hi - qq.hi;
        rl = ((tt.hi - rh) - qq.hi) - qq.lo + tt.lo;
    }
    let r = DoubleDouble::from_exact_add(rh, rl);
    rh = r.hi;
    rl = r.lo;
    rh *= f64::copysign(1., x);
    rl *= f64::copysign(1., x);
    rh += rl;
    rh
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_f_sinh() {
        assert_eq!(f_sinh(1.), 1.1752011936438014);
    }
}
