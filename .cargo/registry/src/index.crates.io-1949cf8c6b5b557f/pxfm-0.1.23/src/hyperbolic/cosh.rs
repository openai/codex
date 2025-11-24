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
use crate::hyperbolic::sinh::hyperbolic_exp_accurate;

#[cold]
fn as_cosh_zero(x: f64) -> f64 {
    static CH: [(u64, u64); 4] = [
        (0xb90c7e8db669f624, 0x3fe0000000000000),
        (0x3c45555555556135, 0x3fa5555555555555),
        (0xbbef49f4a6e838f2, 0x3f56c16c16c16c17),
        (0x3b3a4ffbe15316aa, 0x3efa01a01a01a01a),
    ];
    const CL: [u64; 4] = [
        0x3e927e4fb7789f5c,
        0x3e21eed8eff9089c,
        0x3da939749ce13dad,
        0x3d2ae9891efb6691,
    ];

    let dx2 = DoubleDouble::from_exact_mult(x, x);

    let yw0 = f_fmla(dx2.hi, f64::from_bits(CL[3]), f64::from_bits(CL[2]));
    let yw1 = f_fmla(dx2.hi, yw0, f64::from_bits(CL[1]));

    let y2 = dx2.hi * f_fmla(dx2.hi, yw1, f64::from_bits(CL[0]));

    let mut y1 = lpoly_xd_generic(dx2, CH, y2);
    y1 = DoubleDouble::quick_mult(y1, dx2); // y2 = y1.l
    let y0 = DoubleDouble::from_exact_add(1.0, y1.hi); // y0 = y0.hi
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

/// Hyperbolic cosine function
///
/// Max ULP 0.5
pub fn f_cosh(x: f64) -> f64 {
    /*
     The function sinh(x) is approximated by a minimax polynomial
     cosh(x)~1+x^2*P(x^2) for |x|<0.125. For other arguments the
     identity cosh(x)=(exp(|x|)+exp(-|x|))/2 is used. For |x|<5 both
     exponents are calculated with slightly higher precision than
     double. For 5<|x|<36.736801 the exp(-|x|) is rather small and is
     calculated with double precision but exp(|x|) is calculated with
     higher than double precision. For 36.736801<|x|<710.47586
     exp(-|x|) becomes too small and only exp(|x|) is calculated.
    */

    const S: f64 = f64::from_bits(0x40b71547652b82fe);
    let ax = x.abs();
    let v0 = f_fmla(ax, S, f64::from_bits(0x4198000002000000));
    let jt = v0.to_bits();
    let mut v = v0.to_bits();
    v &= 0xfffffffffc000000;
    let t = f64::from_bits(v) - f64::from_bits(0x4198000000000000);
    let ix = ax.to_bits();
    let aix = ix;
    if aix < 0x3fc0000000000000u64 {
        if aix < 0x3e50000000000000u64 {
            return f_fmla(ax, f64::from_bits(0x3c80000000000000), 1.);
        }
        const C: [u64; 5] = [
            0x3fe0000000000000,
            0x3fa555555555554e,
            0x3f56c16c16c26737,
            0x3efa019ffbbcdbda,
            0x3e927ffe2df106cb,
        ];
        let x2 = x * x;
        let x4 = x2 * x2;

        let p0 = f_fmla(x2, f64::from_bits(C[3]), f64::from_bits(C[2]));
        let p1 = f_fmla(x2, f64::from_bits(C[1]), f64::from_bits(C[0]));
        let p2 = f_fmla(x4, f64::from_bits(C[4]), p0);

        let p = x2 * f_fmla(x4, p2, p1);
        let e = x2 * (4. * f64::from_bits(0x3ca0000000000000));
        let lb = 1. + (p - e);
        let ub = 1. + (p + e);
        if lb == ub {
            return lb;
        }
        return as_cosh_zero(x);
    }

    // treat large values apart to avoid a spurious invalid exception
    if aix > 0x408633ce8fb9f87du64 {
        // |x| > 0x1.633ce8fb9f87dp+9
        if aix > 0x7ff0000000000000u64 {
            return x + x;
        } // nan
        if aix == 0x7ff0000000000000u64 {
            return x.abs();
        } // inf
        return f64::from_bits(0x7fe0000000000000) * 2.0;
    }

    let il: i64 = ((jt.wrapping_shl(14)) >> 40) as i64;
    let jl: i64 = -il;
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

    let pw0 = f_fmla(dx, f64::from_bits(CH[3]), f64::from_bits(CH[2]));
    let pw1 = f_fmla(dx, f64::from_bits(CH[1]), f64::from_bits(CH[0]));

    let pp = dx * f_fmla(dx2, pw0, pw1);
    let (mut rh, mut rl);
    if aix > 0x4014000000000000u64 {
        // |x| > 5
        if aix > 0x40425e4f7b2737fau64 {
            // |x| >~ 36.736801
            sp = (1021i64.wrapping_add(ie) as u64).wrapping_shl(52);
            rh = th;
            rl = f_fmla(th, pp, tl);
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

        let pmw0 = f_fmla(mx, f64::from_bits(CH[3]), f64::from_bits(CH[2]));
        let pmw1 = f_fmla(mx, f64::from_bits(CH[1]), f64::from_bits(CH[0]));

        let pm = mx * f_fmla(dx2, pmw0, pmw1);
        let em = f_fmla(qh, pm, qh);
        rh = th;
        rl = f_fmla(th, pp, tl + em);
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
            rh = th + qh;
            rl = ((th - rh) + qh) + tl;
        } else {
            qh = q0h * q1h;
            let q0l = f64::from_bits(EXP_REDUCE_T0[j0 as usize].0);
            let q1l = f64::from_bits(EXP_REDUCE_T1[j1 as usize].0);
            let mut ql = f_fmla(q0h, q1l, q1h * q0l) + dd_fmla(q0h, q1h, -qh);
            qh *= f64::from_bits(sm);
            ql *= f64::from_bits(sm);
            let qq = hyperbolic_exp_accurate(-ax, -t, DoubleDouble::new(ql, qh));
            qh = qq.hi;
            ql = qq.lo;
            rh = th + qh;
            rl = (((th - rh) + qh) + ql) + tl;
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

        let pmw0 = f_fmla(mx, f64::from_bits(CH[3]), f64::from_bits(CH[2]));
        let pmw1 = f_fmla(mx, f64::from_bits(CH[1]), f64::from_bits(CH[0]));

        let pm = mx * f_fmla(dx2, pmw0, pmw1);
        let fph = th;
        let fpl = f_fmla(th, pp, tl);
        let fmh = qh;
        let fml = f_fmla(qh, pm, ql);

        rh = fph + fmh;
        rl = ((fph - rh) + fmh) + fml + fpl;
        let e = 0.28e-18 * rh;
        let lb = rh + (rl - e);
        let ub = rh + (rl + e);
        if lb == ub {
            return lb;
        }
        let tt = hyperbolic_exp_accurate(ax, t, DoubleDouble::new(tl, th));
        let qq = hyperbolic_exp_accurate(-ax, -t, DoubleDouble::new(ql, qh));
        rh = tt.hi + qq.hi;
        rl = ((tt.hi - rh) + qq.hi) + qq.lo + tt.lo;
    }
    let r = DoubleDouble::from_exact_add(rh, rl);
    rh = r.hi;
    rl = r.lo;
    rh += rl;
    rh
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_cosh() {
        assert_eq!(f_cosh(1.), 1.5430806348152437);
        assert_eq!(f_cosh(1.5454354343), 2.451616191647056);
        assert_eq!(f_cosh(15.5454354343), 2820115.088877147);
    }
}
