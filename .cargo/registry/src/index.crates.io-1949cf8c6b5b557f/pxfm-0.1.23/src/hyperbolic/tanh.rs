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
fn as_tanh_zero(x: f64) -> f64 {
    static CH: [(u64, u64); 10] = [
        (0xbc75555555555555, 0xbfd5555555555555),
        (0x3c41111111110916, 0x3fc1111111111111),
        (0x3c47917917a46f2c, 0xbfaba1ba1ba1ba1c),
        (0xbc09a52a06f1e599, 0x3f9664f4882c10fa),
        (0x3c2c297394c24e38, 0xbf8226e355e6c23d),
        (0xbc0311087e5b1526, 0x3f6d6d3d0e157de0),
        (0xbbe2868cde54ea0c, 0xbf57da36452b75e1),
        (0x3bd2cd8fc406c3f7, 0x3f4355824803667b),
        (0x3b9da22861b4ca80, 0xbf2f57d7734c821d),
        (0xbbb0831108273a74, 0x3f1967e18ad3facf),
    ];
    let dx2 = DoubleDouble::from_exact_mult(x, x);
    const CL: [u64; 6] = [
        0xbf0497d8e6462927,
        0x3ef0b1318c243bd7,
        0xbedb0f2935e9a120,
        0x3ec5e9444536e654,
        0xbeb174ff2a31908c,
        0x3e9749698c8d338d,
    ];

    let yw0 = f_fmla(dx2.hi, f64::from_bits(CL[5]), f64::from_bits(CL[4]));
    let yw1 = f_fmla(dx2.hi, yw0, f64::from_bits(CL[3]));
    let yw2 = f_fmla(dx2.hi, yw1, f64::from_bits(CL[2]));
    let yw3 = f_fmla(dx2.hi, yw2, f64::from_bits(CL[1]));
    let yw4 = f_fmla(dx2.hi, yw3, f64::from_bits(CL[0]));

    let y2 = dx2.hi * yw4;

    let mut y1 = lpoly_xd_generic(dx2, CH, y2);
    y1 = DoubleDouble::quick_mult_f64(y1, x);
    y1 = DoubleDouble::quick_mult(y1, dx2); // y2 = y1.l
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

/// Hyperbolic tan
///
/// Max ULP 0.5
pub fn f_tanh(x: f64) -> f64 {
    /*
      The function tanh(x) is approximated by minimax polynomial for
      |x|<0.25.  For other values we use this identity tanh(|x|) = 1 -
      2*exp(-2*|x|)/(1 + exp(-2*|x|)).  For large |x|>3.683 the term
      2*exp(-2*|x|)/(1 + exp(-2*|x|)) becomes small and we can use less
      precise formula for exponent.
    */
    let ax = x.abs();
    let ix = ax.to_bits();
    let aix: u64 = ix;
    /* for |x| >= 0x1.30fc1931f09cap+4, tanh(x) rounds to +1 or -1 to nearest,
    this avoid a spurious overflow in the computation of v0 below */
    if aix >= 0x40330fc1931f09cau64 {
        if aix > 0x7ff0000000000000u64 {
            return x + x;
        } // nan
        let f = f64::copysign(1.0, x);
        if aix == 0x7ff0000000000000u64 {
            return f;
        }
        let df = f64::copysign(f64::from_bits(0x3c80000000000000), x);
        return f - df;
    }
    const S: f64 = f64::from_bits(0xc0c71547652b82fe);
    let v0 = dd_fmla(ax, S, f64::from_bits(0x4188000004000000));
    let jt = v0.to_bits();
    let v = v0.to_bits() & 0xfffffffff8000000;
    let t = f64::from_bits(v) - f64::from_bits(0x4188000000000000);

    let i1: i64 = ((jt >> 27) & 0x3f) as i64;
    let i0 = (jt >> 33) & 0x3f;
    let ie = ((jt.wrapping_shl(13)) >> 52) as i64;
    let sp = (1023i64.wrapping_add(ie) as u64).wrapping_shl(52);
    const CH: [u64; 4] = [
        0x4000000000000000,
        0x4000000000000000,
        0x3ff55555557e54ff,
        0x3fe55555553a12f4,
    ];
    let t0h = f64::from_bits(EXP_REDUCE_T0[i0 as usize].1);
    let t1h = f64::from_bits(EXP_REDUCE_T1[i1 as usize].1);
    let mut th = t0h * t1h;
    let mut tl;
    if aix < 0x400d76c8b4395810u64 {
        // |x| ~< 3.683
        if aix < 0x3fd0000000000000u64 {
            // |x| < 0x1p-2
            if aix < 0x3e10000000000000u64 {
                // |x| < 0x1p-30
                if aix < 0x3df0000000000000u64 {
                    // |x| < 0x1p-32
                    if aix == 0 {
                        return x;
                    }
                    /* We have underflow when 0 < |x| < 2^-1022 or when |x| = 2^-1022
                    and rounding towards zero. */
                    let res = dd_fmla(x, f64::from_bits(0xbc80000000000000), x);
                    return res;
                }
                let x3 = x * x * x;
                return x - x3 / 3.;
            }

            const C: [u64; 8] = [
                0xbfd5555555555554,
                0x3fc1111111110d61,
                0xbfaba1ba1b983d8b,
                0x3f9664f4820e99f0,
                0xbf8226e11e4ac7cf,
                0x3f6d6c4ab70668b6,
                0xbf57bbecb57ce996,
                0x3f41451443697dd8,
            ];
            let x2 = x * x;
            let x3 = x2 * x;
            let x4 = x2 * x2;
            let x8 = x4 * x4;

            let p1w0 = f_fmla(x2, f64::from_bits(C[7]), f64::from_bits(C[6]));
            let p1w1 = f_fmla(x2, f64::from_bits(C[5]), f64::from_bits(C[4]));

            let p0w0 = f_fmla(x2, f64::from_bits(C[3]), f64::from_bits(C[2]));
            let p0w1 = f_fmla(x2, f64::from_bits(C[1]), f64::from_bits(C[0]));

            let p1 = f_fmla(x4, p1w0, p1w1);
            let mut p0 = f_fmla(x4, p0w0, p0w1);
            p0 += x8 * p1;
            p0 *= x3;
            let r = DoubleDouble::from_exact_add(x, p0);
            let e = x3 * f64::from_bits(0x3cba000000000000);
            let lb = r.hi + (r.lo - e);
            let ub = r.hi + (r.lo + e);
            if lb == ub {
                return lb;
            }
            return as_tanh_zero(x);
        }

        let t0l = f64::from_bits(EXP_REDUCE_T0[i0 as usize].0);
        let t1l = f64::from_bits(EXP_REDUCE_T1[i1 as usize].0);
        tl = f_fmla(t0h, t1l, t1h * t0l) + dd_fmla(t0h, t1h, -th);
        th *= f64::from_bits(sp);
        tl *= f64::from_bits(sp);
        const L2H: f64 = f64::from_bits(0xbf162e42ff000000);
        const L2L: f64 = f64::from_bits(0xbcf718432a1b0e26);
        let dx = f_fmla(-L2L, t, f_fmla(L2H, t, -ax));
        let dx2 = dx * dx;

        let pw0 = f_fmla(dx, f64::from_bits(CH[3]), f64::from_bits(CH[2]));
        let pw1 = f_fmla(dx, f64::from_bits(CH[1]), f64::from_bits(CH[0]));

        let p = dx * f_fmla(dx2, pw0, pw1);
        let mut rh = th;
        let mut rl = tl + rh * p;
        let mut r = DoubleDouble::from_exact_add(rh, rl);

        let ph = r.hi;
        let pl = r.lo;
        let mut qh = r.hi;
        let mut ql = r.lo;
        let qq = DoubleDouble::from_exact_add(1.0, qh);
        qh = qq.hi;
        ql += qq.lo;

        let rqh = 1.0 / qh;
        let rql = f_fmla(ql, rqh, dd_fmla(rqh, qh, -1.)) * -rqh;
        let p = DoubleDouble::mult(DoubleDouble::new(pl, ph), DoubleDouble::new(rql, rqh));

        let e = r.hi * f64::from_bits(0x3c10000000000000);
        r = DoubleDouble::from_exact_sub(0.5, p.hi);
        r.lo -= p.lo;
        rh = r.hi * f64::copysign(2., x);
        rl = r.lo * f64::copysign(2., x);
        let lb = rh + (rl - e);
        let ub = rh + (rl + e);
        if lb == ub {
            return lb;
        }
    } else {
        const L2: f64 = f64::from_bits(0xbf162e42fefa39ef);
        let dx = dd_fmla(L2, t, -ax);
        let dx2 = dx * dx;

        let pw0 = f_fmla(dx, f64::from_bits(CH[3]), f64::from_bits(CH[2]));
        let pw1 = f_fmla(dx, f64::from_bits(CH[1]), f64::from_bits(CH[0]));

        let p = dx * f_fmla(dx2, pw0, pw1);
        let mut rh = th * f64::from_bits(sp);
        rh += (p + ((2. * f64::from_bits(0x3c83000000000000)) * ax)) * rh;
        let e = rh * f64::from_bits(0x3ce1000000000000);
        rh = (2. * rh) / (1. + rh);
        let one = f64::copysign(1., x);
        rh = f64::copysign(rh, x);
        let lb = one - (rh + e);
        let ub = one - (rh - e);
        if lb == ub {
            return lb;
        }

        let t0l = f64::from_bits(EXP_REDUCE_T0[i0 as usize].0);
        let t1l = f64::from_bits(EXP_REDUCE_T1[i1 as usize].0);
        tl = f_fmla(t0h, t1l, t1h * t0l) + dd_fmla(t0h, t1h, -th);
        th *= f64::from_bits(sp);
        tl *= f64::from_bits(sp);
    }

    let mut r = hyperbolic_exp_accurate(-2. * ax, t, DoubleDouble::new(tl, th));
    let mut q = DoubleDouble::from_exact_add(1.0, r.hi);
    q.lo += r.lo;
    q = DoubleDouble::from_exact_add(q.hi, q.lo);
    let rqh = 1. / q.hi;
    let rql = f_fmla(q.lo, rqh, dd_fmla(rqh, q.hi, -1.)) * -rqh;
    let p = DoubleDouble::mult(r, DoubleDouble::new(rql, rqh));
    r = DoubleDouble::from_exact_sub(0.5, p.hi);
    r.lo -= p.lo;
    r = DoubleDouble::from_exact_add(r.hi, r.lo);
    f_fmla(f64::copysign(2., x), r.hi, f64::copysign(2., x) * r.lo)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tanh() {
        assert_eq!(f_tanh(4.799980150947863), 0.9998645463239773);
        assert_eq!(f_tanh(2.549980150947863), 0.9878799187977153);
    }
}
