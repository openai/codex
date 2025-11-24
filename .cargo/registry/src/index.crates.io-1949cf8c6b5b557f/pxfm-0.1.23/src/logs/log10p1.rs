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
use crate::common::*;
use crate::double_double::DoubleDouble;
use crate::dyadic_float::{DyadicFloat128, DyadicSign};
use crate::logs::log2p1::{log_fast, log_p_1a, log2_dyadic};
use crate::logs::log10p1_tables::{LOG10P1_EXACT_INT_S_TABLE, LOG10P1_EXACT_INT_TABLE};

const INV_LOG10_DD: DoubleDouble = DoubleDouble::new(
    f64::from_bits(0x3c695355baaafad3),
    f64::from_bits(0x3fdbcb7b1526e50e),
);

/* deal with |x| < 2^-900, then log10p1(x) ~ x/log(10) */
#[cold]
fn log10p1_accurate_tiny(x: f64) -> f64 {
    /* first scale x to avoid truncation of l in the underflow region */
    let sx = x * f64::from_bits(0x4690000000000000);
    let mut px = DoubleDouble::quick_f64_mult(sx, INV_LOG10_DD);

    let res = px.to_f64() * f64::from_bits(0x3950000000000000); // expected result
    px.lo += dd_fmla(-res, f64::from_bits(0x4690000000000000), px.hi);
    // the correction to apply to res is l*2^-106
    /* For RNDN, we have underflow for |x| <= 0x1.26bb1bbb55515p-1021,
    and for rounding away, for |x| < 0x1.26bb1bbb55515p-1021. */

    dyad_fmla(px.lo, f64::from_bits(0x3950000000000000), res)
}

fn log10p1_accurate_small(x: f64) -> f64 {
    /* the following is a degree-17 polynomial approximating log10p1(x) for
    |x| <= 2^-5 with relative error < 2^-105.067, cf log10p1_accurate.sollya */

    static P_ACC: [u64; 25] = [
        0x3fdbcb7b1526e50e,
        0x3c695355baaafad4,
        0xbfcbcb7b1526e50e,
        0xbc595355baaae078,
        0x3fc287a7636f435f,
        0xbc59c871838f83ac,
        0xbfbbcb7b1526e50e,
        0xbc495355e23285f2,
        0x3fb63c62775250d8,
        0x3c4442abd5831422,
        0xbfb287a7636f435f,
        0x3c49d116f225c4e4,
        0x3fafc3fa615105c7,
        0x3c24e1d7b4790510,
        0xbfabcb7b1526e512,
        0x3c49f884199ab0ce,
        0x3fa8b4df2f3f0486,
        0xbfa63c6277522391,
        0x3fa436e526a79e5c,
        0xbfa287a764c5a762,
        0x3fa11ac1e784daec,
        0xbf9fc3eedc920817,
        0x3f9da5cac3522edb,
        0xbf9be5ca1f9a97cd,
        0x3f9a44b64ca06e9b,
    ];

    /* for degree 11 or more, ulp(c[d]*x^d) < 2^-105.7*|log10p1(x)|
    where c[d] is the degree-d coefficient of Pacc, thus we can compute
    with a double only, and even with degree 10 (this does not increase
    the number of exceptional cases) */

    let mut h = dd_fmla(f64::from_bits(P_ACC[24]), x, f64::from_bits(P_ACC[23])); // degree 16
    for i in (10..=15).rev() {
        h = dd_fmla(h, x, f64::from_bits(P_ACC[(i + 7) as usize])); // degree i
    }

    // degree 9
    let px = DoubleDouble::from_exact_mult(x, h);
    let hl = DoubleDouble::from_exact_add(f64::from_bits(P_ACC[9 + 7]), px.hi);
    h = hl.hi;
    let mut l = px.lo + hl.lo;

    for i in (1..=8).rev() {
        let mut p = DoubleDouble::quick_f64_mult(x, DoubleDouble::new(l, h));
        l = p.lo;
        p = DoubleDouble::from_exact_add(f64::from_bits(P_ACC[(2 * i - 2) as usize]), p.hi);
        h = p.hi;
        l += p.lo + f64::from_bits(P_ACC[(2 * i - 1) as usize]);
    }
    let pz = DoubleDouble::quick_f64_mult(x, DoubleDouble::new(l, h));
    pz.to_f64()
}

#[cold]
fn log10p1_accurate(x: f64) -> f64 {
    let ax = x.abs();

    if ax < f64::from_bits(0x3fa0000000000000) {
        return if ax < f64::from_bits(0x07b0000000000000) {
            log10p1_accurate_tiny(x)
        } else {
            log10p1_accurate_small(x)
        };
    }
    let dx = if x > 1.0 {
        DoubleDouble::from_exact_add(x, 1.0)
    } else {
        DoubleDouble::from_exact_add(1.0, x)
    };
    let x_d = DyadicFloat128::new_from_f64(dx.hi);
    let mut y = log2_dyadic(x_d, dx.hi);
    let mut c = DyadicFloat128::from_div_f64(dx.lo, dx.hi);
    let mut bx = c * c;
    /* multiply X by -1/2 */
    bx.exponent -= 1;
    bx.sign = DyadicSign::Neg;
    /* C <- C - C^2/2 */
    c = c + bx;
    /* |C-log(1+xl/xh)| ~ 2e-64 */
    y = y + c;

    // Sage Math:
    // from sage.all import *
    //
    // def format_hex2(value):
    //     l = hex(value)[2:]
    //     n = 4
    //     x = [l[i:i + n] for i in range(0, len(l), n)]
    //     return "0x" + "_".join(x) + "_u128"
    // (s, m, e) = (RealField(128)(1)/RealField(128)(10)).log().sign_mantissa_exponent();
    // print(format_hex2(m));
    const LOG10_INV: DyadicFloat128 = DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -129,
        mantissa: 0xde5b_d8a9_3728_7195_355b_aaaf_ad33_dc32_u128,
    };
    y = y * LOG10_INV;
    y.fast_as_f64()
}

#[inline]
fn log10p1_fast(x: f64, e: i32) -> (DoubleDouble, f64) {
    if e < -5
    /* e <= -6 thus |x| < 2^-5 */
    {
        if e <= -968 {
            /* then |x| might be as small as 2^-968, thus h=x/log(10) might in the
            binade [2^-970,2^-969), with ulp(h) = 2^-1022, and if |l| < ulp(h),
            then l.ulp() might be smaller than 2^-1074. We defer that case to
            the accurate path. */
            let ax = x.abs();
            let result = if ax < f64::from_bits(0x07b0000000000000) {
                log10p1_accurate_tiny(x)
            } else {
                log10p1_accurate_small(x)
            };
            return (DoubleDouble::new(0.0, result), 0.0);
        }
        let mut p = log_p_1a(x);
        let p_lo = p.lo;
        p = DoubleDouble::from_exact_add(x, p.hi);
        p.lo += p_lo;

        /* from analyze_x_plus_p1a(rel=true,Xmax=2^-5.) in the accompanying file
        log1p.sage, the relative error is bounded by 2^-61.14 with respect to
        h. We use the fact that we don't need the return value err to be
        positive, since we add/subtract it in the rounding test.
        We also get that the ratio |l/h| is bounded by 2^-50.96. */
        /* now we multiply h+l by 1/log(2) */
        p = DoubleDouble::quick_mult(p, INV_LOG10_DD);
        /* the d_mul() call decomposes into:
         a_mul (h_out, l1, h, INVLOG10H)
         l2 = __builtin_fma (h, INVLOG10L, l1)
         l_out = __builtin_fma (l, INVLOG10H, l2)
         we have |l1| <= ulp(h_out)
         since |INVLOG10L/INVLOG10H| < 2^-55, then |h*INVLOG10L| <= 2^-55*|h_out|
         and since |x| < 2^53*ulp(x): |h*INVLOG10L| <= ulp(h_out)/4
         thus |l2| <= 5/4*ulp(h_out).
         Now since |l/h| < 2^-50.96, |l*INVLOG10H| < 2^-50.96*|h*INVLOG10H|
         < 2^-50.96*(1+2^-52)*|h_out| < 2^-50.95*|h_out| < 4.15*ulp(h_out),
         thus |l_out| < o(4.15*ulp(h_out)+5/4*ulp(h_out)) < 5.5*ulp(h_out).
         The rounding errors are bounded by ulp(l2)+ulp(l_out)
         <= ulp(5/4*ulp(h_out)) + ulp(5.5*ulp(h_out))
         <= 2^-52*(5/4*ulp(h_out)+5.5*ulp(h_out)) [using ulp(x) <= 2^-52*|x|]
         <= 2^-49.2*ulp(h_out)
         We also have to take into account the ignored term l*INVLOG10L:
         |l*INVLOG10L| < 2^-50.96*|h|*2^-55.97*|INVLOG10H|
                       < 2^-106.93*(1+2^-52)*|h_out|
                       < 2^-106.92*|h_out|
                       < 2^-51.92*ulp(h_out) [using |x| < 2^53*ulp(x)]
        and the approximation error in INVLOG10H+INVLOG10L:
        |INVLOG10H + INVLOG10L - 1/log(10)| < 2^-109.84/log(10)
        The total error of d_mul() is thus bounded by:
        (2^-49.2+2^-51.92)*ulp(h_out) < 2^-48.99*ulp(h_out) < 2^-100.99*|h_out|,
        using again ulp(x) <= 2^-52*|x|.

        The relative error is thus bounded by
        (1+2^-61.14)*(1+2^-100.99)*(1+2^-109.84)-1 < 2^-61.13 */
        return (p, f64::from_bits(0x3c1d400000000000) * p.hi); /* 2^-61.13 < 0x1.d4p-62 */
    }

    /* (xh,xl) <- 1+x */
    let zx = DoubleDouble::from_full_exact_add(x, 1.0);

    let mut v_u = zx.hi.to_bits();
    let e = ((v_u >> 52) as i32).wrapping_sub(0x3ff);
    v_u = (0x3ffu64 << 52) | (v_u & 0xfffffffffffff);
    let mut p = log_fast(e, v_u);

    /* log(xh+xl) = log(xh) + log(1+xl/xh) */
    let c = if zx.hi <= f64::from_bits(0x7fd0000000000000) || zx.lo.abs() >= 4.0 {
        zx.lo / zx.hi
    } else {
        0.
    }; // avoid spurious underflow

    /* Since |xl| < ulp(xh), we have |xl| < 2^-52 |xh|,
    thus |c| < 2^-52, and since |log(1+x)-x| < x^2 for |x| < 0.5,
    we have |log(1+c)-c)| < c^2 < 2^-104. */
    p.lo += c;
    /* Since |l_in| < 2^-18.69 (from the analysis of cr_log_fast, see file
    ../log/log.c), and |c| < 2^-52, we have |l| < 2^-18.68, thus the
    rounding error in *l += c is bounded by ulp(2^-18.68) = 2^-71.
    The total absolute error is thus bounded by:
    0x1.b6p-69 + 2^-104 + 2^-71 < 2^-68.02. */

    /* now multiply h+l by 1/log(2) */
    p = DoubleDouble::quick_mult(p, INV_LOG10_DD);
    /* the d_mul() call decomposes into:
       a_mul (h_out, l1, h, INVLOG10H)
       l2 = __builtin_fma (h, INVLOG10L, l1)
       l_out = __builtin_fma (l, INVLOG10H, l2)
       We have three errors:
       * the rounding error in l2 = __builtin_fma (h, INVLOG10L, l1)
       * the rounding error in l_out = __builtin_fma (l, INVLOG10H, l2)
       * the ignored term l * INVLOG10L
       We have |h| < 745 thus |h*INVLOG10H| < 324 thus |h_out| <= 324
       and |l1| <= ulp(h_out) <= 2^-44.
       Then |h*INVLOG10L+l1| <= 745*INVLOG2L+2^-44 < 2^-43.6
       thus |l2| < 2^-43.6*(1+2^-52) < 2^-43.5
       and the first rounding error is bounded by ulp(2^-43.5) = 2^-96.
       Now |l*INVLOG10H+l2| < 2^-18.68*INVLOG10H+2^-43.5 < 2^-19.8
       thus |l_out| < 2^-19.8*(1+2^-52) < 2^-19.7
       and the second rounding error is bounded by ulp(2^-19.7) = 2^-72.
       The ignored term is bounded by |l*INVLOG10L| < 2^-18.68*INVLOG10L
       < 2^-75.0.
       Thus the absolute error from d_mul() is bounded by:
       2^-96 + 2^-72 + 2^-75.0 < 2^-71.83.

       Adding to the maximal absolute error of 2^-68.02 before d_mul(),
       we get 2^-68.02 + 2^-71.83 < 2^-67.92.
    */

    (p, f64::from_bits(0x3bb0a00000000000)) /* 2^-67.92 < 0x1.0ap-68 */
}

/// Computes log10(x+1)
///
/// Max ULP 0.5
pub fn f_log10p1(x: f64) -> f64 {
    let x_u = x.to_bits();
    let e = (((x_u >> 52) & 0x7ff) as i32).wrapping_sub(0x3ff);
    if e == 0x400 || x == 0. || x <= -1.0 {
        /* case NaN/Inf, +/-0 or x <= -1 */
        if e == 0x400 && x.to_bits() != 0xfffu64 << 52 {
            /* NaN or + Inf*/
            return x + x;
        }
        if x <= -1.0
        /* we use the fact that NaN < -1 is false */
        {
            /* log2p(x<-1) is NaN, log2p(-1) is -Inf and raises DivByZero */
            return if x < -1.0 {
                f64::NAN
            } else {
                // x=-1
                f64::NEG_INFINITY
            };
        }
        return x + x; /* +/-0 */
    }

    /* check x=10^n-1 for 1 <= n <= 15, where log10p1(x) is exact,
    and we shouldn't raise the inexact flag */
    if 3 <= e && e <= 49 && x == f64::from_bits(LOG10P1_EXACT_INT_TABLE[e as usize]) {
        return LOG10P1_EXACT_INT_S_TABLE[e as usize] as f64;
    }

    /* now x = m*2^e with 1 <= m < 2 (m = v.f) and -1074 <= e <= 1023 */
    let (p, err) = log10p1_fast(x, e);
    let left = p.hi + (p.lo - err);
    let right = p.hi + (p.lo + err);
    if left == right {
        return left;
    }

    log10p1_accurate(x)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log10p1() {
        assert_eq!(f_log10p1(0.00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000013904929147106097),
                   0.00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000006038833999843867 );
        assert!(f_log10p1(-2.0).is_nan());
        assert_eq!(f_log10p1(9.0), 1.0);
        assert_eq!(f_log10p1(2.0), 0.47712125471966244);
        assert_eq!(f_log10p1(-0.5), -0.3010299956639812);
    }
}
