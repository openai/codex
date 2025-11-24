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
use crate::common::{dd_fmla, dyad_fmla, f_fmla};
use crate::double_double::DoubleDouble;
use crate::err::erf_poly::{ERF_POLY, ERF_POLY_C2};

/* double-double approximation of 2/sqrt(pi) to nearest */
const TWO_OVER_SQRT_PI: DoubleDouble = DoubleDouble::new(
    f64::from_bits(0x3c71ae3a914fed80),
    f64::from_bits(0x3ff20dd750429b6d),
);

pub(crate) struct Erf {
    pub(crate) result: DoubleDouble,
    pub(crate) err: f64,
}

/* for |z| < 1/8, assuming z >= 2^-61, thus no underflow can occur */
#[cold]
fn cr_erf_accurate_tiny(x: f64) -> DoubleDouble {
    static P: [u64; 15] = [
        0x3ff20dd750429b6d,
        0x3c71ae3a914fed80,
        0xbfd812746b0379e7,
        0x3c6ee12e49ca96ba,
        0x3fbce2f21a042be2,
        0xbc52871bc0a0a0d0,
        0xbf9b82ce31288b51,
        0x3c21003accf1355c,
        0x3f7565bcd0e6a53f,
        0xbf4c02db40040cc3,
        0x3f1f9a326fa3cf50,
        0xbeef4d25e3c73ce9,
        0x3ebb9eb332b31646,
        0xbe864a4bd5eca4d7,
        0x3e6c0acc2502e94e,
    ];
    let z2 = x * x;
    let mut h = f64::from_bits(P[21 / 2 + 4]); /* degree 21 */
    for a in (12..=19).rev().step_by(2) {
        h = dd_fmla(h, z2, f64::from_bits(P[(a / 2 + 4) as usize]))
    }
    let mut l = 0.;
    for a in (8..=11).rev().step_by(2) {
        let mut t = DoubleDouble::from_exact_mult(h, x);
        t.lo = dd_fmla(l, x, t.lo);
        let mut k = DoubleDouble::from_exact_mult(t.hi, x);
        k.lo = dd_fmla(t.lo, x, k.lo);
        let p = DoubleDouble::from_exact_add(f64::from_bits(P[(a / 2 + 4) as usize]), k.hi);
        l = k.lo + p.lo;
        h = p.hi;
    }
    for a in (1..=7).rev().step_by(2) {
        let mut t = DoubleDouble::from_exact_mult(h, x);
        t.lo = dd_fmla(l, x, t.lo);
        let mut k = DoubleDouble::from_exact_mult(t.hi, x);
        k.lo = dd_fmla(t.lo, x, k.lo);
        let p = DoubleDouble::from_exact_add(f64::from_bits(P[a - 1]), k.hi);
        l = k.lo + p.lo + f64::from_bits(P[a]);
        h = p.hi;
    }
    /* multiply by z */
    let p = DoubleDouble::from_exact_mult(h, x);
    l = dd_fmla(l, x, p.lo);
    DoubleDouble::new(l, p.hi)
}

/* Assuming 0 <= z <= 0x1.7afb48dc96626p+2, put in h+l an accurate
approximation of erf(z).
Assumes z >= 2^-61, thus no underflow can occur. */
#[cold]
#[inline(never)]
pub(crate) fn erf_accurate(x: f64) -> DoubleDouble {
    if x < 0.125
    /* z < 1/8 */
    {
        return cr_erf_accurate_tiny(x);
    }

    let v = (8.0 * x).floor();
    let i: u32 = (8.0 * x) as u32;
    let z = (x - 0.0625) - 0.125 * v;
    /* now |z| <= 1/16 */
    let p = ERF_POLY_C2[(i - 1) as usize];
    let mut h = f64::from_bits(p[26]); /* degree-18 */
    for a in (11..=17).rev() {
        h = dd_fmla(h, z, f64::from_bits(p[(8 + a) as usize])); /* degree j */
    }
    let mut l: f64 = 0.;
    for a in (8..=10).rev() {
        let mut t = DoubleDouble::from_exact_mult(h, z);
        t.lo = dd_fmla(l, z, t.lo);
        let p = DoubleDouble::from_exact_add(f64::from_bits(p[(8 + a) as usize]), t.hi);
        h = p.hi;
        l = p.lo + t.lo;
    }
    for a in (0..=7).rev() {
        let mut t = DoubleDouble::from_exact_mult(h, z);
        t.lo = dd_fmla(l, z, t.lo);
        /* add p[2*j] + p[2*j+1] to th + tl: we use two_sum() instead of
        fast_two_sum because for example for i=3, the coefficient of
        degree 7 is tiny (0x1.060b78c935b8ep-13) with respect to that
        of degree 8 (0x1.678b51a9c4b0ap-7) */
        let v = DoubleDouble::from_exact_add(f64::from_bits(p[(2 * a) as usize]), t.hi);
        h = v.hi;
        l = v.lo + t.lo + f64::from_bits(p[(2 * a + 1) as usize]);
    }
    DoubleDouble::new(l, h)
}

/* Assuming 0 <= z <= 5.9215871957945065, put in h+l an approximation
of erf(z). Return err the maximal relative error:
|(h + l)/erf(z) - 1| < err*|h+l| */
#[inline]
pub(crate) fn erf_fast(x: f64) -> Erf {
    /* we split [0,5.9215871957945065] into intervals i/16 <= z < (i+1)/16,
       and for each interval, we use a minimax polynomial:
       * for i=0 (0 <= z < 1/16) we use a polynomial evaluated at zero,
         since if we evaluate in the middle 1/32, we will get bad accuracy
         for tiny z, and moreover z-1/32 might not be exact
       * for 1 <= i <= 94, we use a polynomial evaluated in the middle of
         the interval, namely i/16+1/32
    */
    if x < 0.0625
    /* z < 1/16 */
    {
        /* the following is a degree-11 minimax polynomial for erf(x) on [0,1/16]
        generated by Sollya, with double-double coefficients for degree 1 and 3,
        and double coefficients for degrees 5 to 11 (file erf0.sollya).
        The maximal relative error is 2^-68.935. */
        let z2 = DoubleDouble::from_exact_mult(x, x);
        const C: [u64; 8] = [
            0x3ff20dd750429b6d,
            0x3c71ae3a7862d9c4,
            0xbfd812746b0379e7,
            0x3c6f1a64d72722a2,
            0x3fbce2f21a042b7f,
            0xbf9b82ce31189904,
            0x3f7565bbf8a0fe0b,
            0xbf4bf9f8d2c202e4,
        ];
        let z4 = z2.hi * z2.hi;
        let c9 = dd_fmla(f64::from_bits(C[7]), z2.hi, f64::from_bits(C[6]));
        let mut c5 = dd_fmla(f64::from_bits(C[5]), z2.hi, f64::from_bits(C[4]));
        c5 = dd_fmla(c9, z4, c5);
        /* compute c0[2] + c0[3] + z2h*c5 */
        let mut t = DoubleDouble::from_exact_mult(z2.hi, c5);
        let mut v = DoubleDouble::from_exact_add(f64::from_bits(C[2]), t.hi);
        v.lo += t.lo + f64::from_bits(C[3]);
        /* compute c0[0] + c0[1] + (z2h + z2l)*(h + l) */
        t = DoubleDouble::from_exact_mult(z2.hi, v.hi);
        let h_c = v.hi;
        t.lo += dd_fmla(z2.hi, v.lo, f64::from_bits(C[1]));
        v = DoubleDouble::from_exact_add(f64::from_bits(C[0]), t.hi);
        v.lo += dd_fmla(z2.lo, h_c, t.lo);
        v = DoubleDouble::quick_mult_f64(v, x);
        return Erf {
            result: v,
            err: f64::from_bits(0x3ba7800000000000),
        }; /* err < 2.48658249618372e-21, cf Analyze0() */
    }

    let v = (16.0 * x).floor();
    let i: u32 = (16.0 * x) as u32;
    /* i/16 <= z < (i+1)/16 */
    /* For 0.0625 0 <= z <= 0x1.7afb48dc96626p+2, z - 0.03125 is exact:
    (1) either z - 0.03125 is in the same binade as z, then 0.03125 is
        an integer multiple of ulp(z), so is z - 0.03125
    (2) if z - 0.03125 is in a smaller binade, both z and 0.03125 are
        integer multiple of the ulp() of that smaller binade.
    Also, subtracting 0.0625 * v is exact. */
    let z = (x - 0.03125) - 0.0625 * v;
    /* now |z| <= 1/32 */
    let c = ERF_POLY[(i - 1) as usize];
    let z2 = z * z;
    let z4 = z2 * z2;
    /* the degree-10 coefficient is c[12] */
    let c9 = dd_fmla(f64::from_bits(c[12]), z, f64::from_bits(c[11]));
    let mut c7 = dd_fmla(f64::from_bits(c[10]), z, f64::from_bits(c[9]));
    let c5 = dd_fmla(f64::from_bits(c[8]), z, f64::from_bits(c[7]));
    /* c3h, c3l <- c[5] + z*c[6] */
    let mut c3 = DoubleDouble::from_exact_add(f64::from_bits(c[5]), z * f64::from_bits(c[6]));
    c7 = dd_fmla(c9, z2, c7);
    /* c3h, c3l <- c3h, c3l + c5*z2 */
    let p = DoubleDouble::from_exact_add(c3.hi, c5 * z2);
    c3.hi = p.hi;
    c3.lo += p.lo;
    /* c3h, c3l <- c3h, c3l + c7*z4 */
    let p = DoubleDouble::from_exact_add(c3.hi, c7 * z4);
    c3.hi = p.hi;
    c3.lo += p.lo;
    /* c2h, c2l <- c[4] + z*(c3h + c3l) */
    let mut t = DoubleDouble::from_exact_mult(z, c3.hi);
    let mut c2 = DoubleDouble::from_exact_add(f64::from_bits(c[4]), t.hi);
    c2.lo += dd_fmla(z, c3.lo, t.lo);
    /* compute c[2] + c[3] + z*(c2h + c2l) */
    t = DoubleDouble::from_exact_mult(z, c2.hi);
    let mut v = DoubleDouble::from_exact_add(f64::from_bits(c[2]), t.hi);
    v.lo += t.lo + dd_fmla(z, c2.lo, f64::from_bits(c[3]));
    /* compute c[0] + c[1] + z*(h + l) */
    t = DoubleDouble::from_exact_mult(z, v.hi);
    t.lo = dd_fmla(z, v.lo, t.lo);
    v = DoubleDouble::from_exact_add(f64::from_bits(c[0]), t.hi);
    v.lo += t.lo + f64::from_bits(c[1]);
    Erf {
        result: v,
        err: f64::from_bits(0x3ba1100000000000),
    } /* err < 1.80414390200020e-21, cf analyze_p(1)
    (larger values of i yield smaller error bounds) */
}

/// Error function
///
/// Max ULP 0.5
pub fn f_erf(x: f64) -> f64 {
    let z = f64::from_bits(x.to_bits() & 0x7fff_ffff_ffff_ffff);
    let mut t = z.to_bits();
    let ux = t;
    /* erf(x) rounds to +/-1 for RNDN for |x| > 0x4017afb48dc96626 */
    if ux > 0x4017afb48dc96626
    // |x| > 0x4017afb48dc96626
    {
        let os = f64::copysign(1.0, x);
        const MASK: u64 = 0x7ff0000000000000u64;
        if ux > MASK {
            return x + x; /* NaN */
        }
        if ux == MASK {
            return os; /* +/-Inf */
        }
        return f_fmla(-f64::from_bits(0x3c90000000000000), os, os);
    }

    /* now |x| <= 0x4017afb48dc96626 */
    if z < f64::from_bits(0x3c20000000000000) {
        /* for x=-0 the code below returns +0 which is wrong */
        if x == 0. {
            return x;
        }
        /* tiny x: erf(x) ~ 2/sqrt(pi) * x + O(x^3), where the ratio of the O(x^3)
        term to the main term is in x^2/3, thus less than 2^-123 */
        let y = TWO_OVER_SQRT_PI.hi * x; /* tentative result */
        /* scale x by 2^106 to get out the subnormal range */
        let sx = x * f64::from_bits(0x4690000000000000);
        let mut p = DoubleDouble::quick_mult_f64(TWO_OVER_SQRT_PI, sx);
        /* now compute the residual h + l - y */
        p.lo += f_fmla(-y, f64::from_bits(0x4690000000000000), p.hi); /* h-y*2^106 is exact since h and y are very close */
        let res = dyad_fmla(p.lo, f64::from_bits(0x3950000000000000), y);
        return res;
    }

    let result = erf_fast(z);
    let mut u = result.result.hi.to_bits();
    let mut v = result.result.lo.to_bits();
    t = x.to_bits();

    const SIGN_MASK: u64 = 0x8000000000000000u64;
    u ^= t & SIGN_MASK;
    v ^= t & SIGN_MASK;

    let left = f64::from_bits(u) + f_fmla(result.err, -f64::from_bits(u), f64::from_bits(v));
    let right = f64::from_bits(u) + f_fmla(result.err, f64::from_bits(u), f64::from_bits(v));

    if left == right {
        return left;
    }

    let a_results = erf_accurate(z);

    if x >= 0. {
        a_results.to_f64()
    } else {
        (-a_results.hi) + (-a_results.lo)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_erf() {
        assert_eq!(f_erf(0.00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000009456563898732),
                   0.00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000010670589695636709);
        assert_eq!(f_erf(0.), 0.);
        assert_eq!(f_erf(1.), 0.8427007929497149);
        assert_eq!(f_erf(0.49866735123), 0.5193279892991808);
        assert_eq!(f_erf(-0.49866735123), -0.5193279892991808);
    }
}
