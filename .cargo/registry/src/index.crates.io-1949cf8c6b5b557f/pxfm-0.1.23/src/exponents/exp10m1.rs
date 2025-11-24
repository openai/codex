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
use crate::exponents::exp2m1::{EXP_M1_2_TABLE1, EXP_M1_2_TABLE2};

const LN10H: f64 = f64::from_bits(0x40026bb1bbb55516);
const LN10L: f64 = f64::from_bits(0xbcaf48ad494ea3e9);

struct Exp10m1 {
    exp: DoubleDouble,
    err: f64,
}

// Approximation for the fast path of exp(z) for z=zh+zl,
// with |z| < 0.000130273 < 2^-12.88 and |zl| < 2^-42.6
// (assuming x^y does not overflow or underflow)
#[inline]
fn q_1(dz: DoubleDouble) -> DoubleDouble {
    const Q_1: [u64; 5] = [
        0x3ff0000000000000,
        0x3ff0000000000000,
        0x3fe0000000000000,
        0x3fc5555555995d37,
        0x3fa55555558489dc,
    ];
    let z = dz.to_f64();
    let mut q = f_fmla(f64::from_bits(Q_1[4]), dz.hi, f64::from_bits(Q_1[3]));
    q = f_fmla(q, z, f64::from_bits(Q_1[2]));

    let mut p0 = DoubleDouble::from_exact_add(f64::from_bits(Q_1[1]), q * z);
    p0 = DoubleDouble::quick_mult(dz, p0);
    p0 = DoubleDouble::f64_add(f64::from_bits(Q_1[0]), p0);
    p0
}

#[inline]
fn exp1(x: DoubleDouble) -> DoubleDouble {
    const INVLOG2: f64 = f64::from_bits(0x40b71547652b82fe); /* |INVLOG2-2^12/log(2)| < 2^-43.4 */
    let k = (x.hi * INVLOG2).round_ties_even();

    const LOG2H: f64 = f64::from_bits(0x3f262e42fefa39ef);
    const LOG2L: f64 = f64::from_bits(0x3bbabc9e3b39803f);
    let mut zk = DoubleDouble::from_exact_mult(LOG2H, k);
    zk.lo = f_fmla(k, LOG2L, zk.lo);

    let mut yz = DoubleDouble::from_exact_add(x.hi - zk.hi, x.lo);
    yz.lo -= zk.lo;

    let ik: i64 = k as i64; /* Note: k is an integer, this is just a conversion. */
    let im: i64 = (ik >> 12).wrapping_add(0x3ff);
    let i2: i64 = (ik >> 6) & 0x3f;
    let i1: i64 = ik & 0x3f;

    let t1 = DoubleDouble::from_bit_pair(EXP_M1_2_TABLE1[i2 as usize]);
    let t2 = DoubleDouble::from_bit_pair(EXP_M1_2_TABLE2[i1 as usize]);

    let p0 = DoubleDouble::quick_mult(t2, t1);

    let mut q = q_1(yz);
    q = DoubleDouble::quick_mult(p0, q);

    /* Scale by 2^k. Warning: for x near 1024, we can have k=2^22, thus
    M = 2047, which encodes Inf */
    let mut du = (im as u64).wrapping_shl(52);
    if im == 0x7ff {
        q.hi *= 2.0;
        q.lo *= 2.0;
        du = (im.wrapping_sub(1) as u64).wrapping_shl(52);
    }
    q.hi *= f64::from_bits(du);
    q.lo *= f64::from_bits(du);
    q
}

#[inline]
fn exp10m1_fast(x: f64, tiny: bool) -> Exp10m1 {
    if tiny {
        return exp10m1_fast_tiny(x);
    }
    /* now -54 < x < -0.125 or 0.125 < x < 1024: we approximate exp(x*log(2))
    and subtract 1 */
    let v = DoubleDouble::quick_mult_f64(DoubleDouble::new(LN10L, LN10H), x);
    /*
    The a_mul() call is exact, and the error of the fma() is bounded by
     ulp(l).
     We have |t| <= ulp(h) <= ulp(LN2H*1024) = 2^-43,
     |t+x*LN2L| <= 2^-43 * 1024*LN2L < 2^-42.7,
     thus |l| <= |t| + |x*LN2L| + ulp(t+x*LN2L)
              <= 2^-42.7 + 2^-95 <= 2^-42.6, and ulp(l) <= 2^-95.
     Thus:
     |h + l - x*log(2)| <= |h + l - x*(LN2H+LN2L)| + |x|*|LN2H+LN2L-log(2)|
                        <= 2^-95 + 1024*2^-110.4 < 2^-94.9 */

    let mut p = exp1(v);

    let zf: DoubleDouble = if x >= 0. {
        // implies h >= 1 and the fast_two_sum pre-condition holds
        DoubleDouble::from_exact_add(p.hi, -1.0)
    } else {
        DoubleDouble::from_exact_add(-1.0, p.hi)
    };
    p.lo += zf.lo;
    p.hi = zf.hi;
    /* The error in the above fast_two_sum is bounded by 2^-105*|h|,
    with the new value of h, thus the total absolute error is bounded
    by eps1*|h_in|+2^-105*|h|.
    Relatively to h this yields eps1*|h_in/h| + 2^-105, where the maximum
    of |h_in/h| is obtained for x near -0.125, with |2^x/(2^x-1)| < 11.05.
    We get a relative error bound of 2^-74.138*11.05 + 2^-105 < 2^-70.67. */
    Exp10m1 {
        exp: p,
        err: f64::from_bits(0x3b77a00000000000) * p.hi, /* 2^-70.67 < 0x1.42p-71 */
    }
}

// Approximation for the accurate path of exp(z) for z=zh+zl,
// with |z| < 0.000130273 < 2^-12.88 and |zl| < 2^-42.6
// (assuming x^y does not overflow or underflow)
#[inline]
fn q_2(dz: DoubleDouble) -> DoubleDouble {
    /* Let q[0]..q[7] be the coefficients of degree 0..7 of Q_2.
    The ulp of q[7]*z^7 is at most 2^-155, thus we can compute q[7]*z^7
    in double precision only.
    The ulp of q[6]*z^6 is at most 2^-139, thus we can compute q[6]*z^6
    in double precision only.
    The ulp of q[5]*z^5 is at most 2^-124, thus we can compute q[5]*z^5
    in double precision only. */

    /* The following is a degree-7 polynomial generated by Sollya for exp(z)
    over [-0.000130273,0.000130273] with absolute error < 2^-113.218
    (see file exp_accurate.sollya). Since we use this code only for
    |x| > 0.125 in exp2m1(x), the corresponding relative error for exp2m1
    is about 2^-113.218/|exp2m1(-0.125)| which is about 2^-110. */
    const Q_2: [u64; 9] = [
        0x3ff0000000000000,
        0x3ff0000000000000,
        0x3fe0000000000000,
        0x3fc5555555555555,
        0x3c655555555c4d26,
        0x3fa5555555555555,
        0x3f81111111111111,
        0x3f56c16c3fbb4213,
        0x3f2a01a023ede0d7,
    ];

    let z = dz.to_f64();
    let mut q = dd_fmla(f64::from_bits(Q_2[8]), dz.hi, f64::from_bits(Q_2[7]));
    q = dd_fmla(q, z, f64::from_bits(Q_2[6]));
    q = dd_fmla(q, z, f64::from_bits(Q_2[5]));

    // multiply q by z and add Q_2[3] + Q_2[4]

    let mut p = DoubleDouble::from_exact_mult(q, z);
    let r0 = DoubleDouble::from_exact_add(f64::from_bits(Q_2[3]), p.hi);
    p.hi = r0.hi;
    p.lo += r0.lo + f64::from_bits(Q_2[4]);

    // multiply hi+lo by zh+zl and add Q_2[2]

    p = DoubleDouble::quick_mult(p, dz);
    let r1 = DoubleDouble::from_exact_add(f64::from_bits(Q_2[2]), p.hi);
    p.hi = r1.hi;
    p.lo += r1.lo;

    // multiply hi+lo by zh+zl and add Q_2[1]
    p = DoubleDouble::quick_mult(p, dz);
    let r1 = DoubleDouble::from_exact_add(f64::from_bits(Q_2[1]), p.hi);
    p.hi = r1.hi;
    p.lo += r1.lo;

    // multiply hi+lo by zh+zl and add Q_2[0]
    p = DoubleDouble::quick_mult(p, dz);
    let r1 = DoubleDouble::from_exact_add(f64::from_bits(Q_2[0]), p.hi);
    p.hi = r1.hi;
    p.lo += r1.lo;
    p
}

// returns a double-double approximation hi+lo of exp(x*log(10))
// assumes -0x1.041704c068efp+4 < x <= 0x1.34413509f79fep+8
#[inline]
fn exp_2(x: f64) -> DoubleDouble {
    let mut k = (x * f64::from_bits(0x40ca934f0979a371)).round_ties_even();
    if k == 4194304. {
        k = 4194303.; // ensures M < 2047 below
    }
    // since |x| <= 745 we have k <= 3051520

    const LOG2_10H: f64 = f64::from_bits(0x3f134413509f79ff);
    const LOG2_10M: f64 = f64::from_bits(0xbb89dc1da9800000);
    const LOG2_10L: f64 = f64::from_bits(0xb984fd20dba1f655);

    let yhh = dd_fmla(-k, LOG2_10H, x); // exact, |yh| <= 2^-13

    let mut ky0 = DoubleDouble::from_exact_add(yhh, -k * LOG2_10M);
    ky0.lo = dd_fmla(-k, LOG2_10L, ky0.lo);

    /* now x = k + yh, thus 2^x = 2^k * 2^yh, and we multiply yh by log(10)
    to use the accurate path of exp() */

    let ky = DoubleDouble::quick_mult(ky0, DoubleDouble::new(LN10L, LN10H));

    let ik = k as i64;
    let im = (ik >> 12).wrapping_add(0x3ff);
    let i2 = (ik >> 6) & 0x3f;
    let i1 = ik & 0x3f;

    let t1 = DoubleDouble::from_bit_pair(EXP_M1_2_TABLE1[i2 as usize]);
    let t2 = DoubleDouble::from_bit_pair(EXP_M1_2_TABLE2[i1 as usize]);

    let p = DoubleDouble::quick_mult(t2, t1);

    let mut q = q_2(ky);
    q = DoubleDouble::quick_mult(p, q);
    let mut ud: u64 = (im as u64).wrapping_shl(52);

    if im == 0x7ff {
        q.hi *= 2.0;
        q.lo *= 2.0;
        ud = (im.wrapping_sub(1) as u64).wrapping_shl(52);
    }
    q.hi *= f64::from_bits(ud);
    q.lo *= f64::from_bits(ud);
    q
}

#[cold]
fn exp10m1_accurate_tiny(x: f64) -> f64 {
    let x2 = x * x;
    let x4 = x2 * x2;
    /* The following is a degree-17 polynomial generated by Sollya
    (file exp10m1_accurate.sollya),
    which approximates exp10m1(x) with relative error bounded by 2^-107.506
    for |x| <= 0.0625. */

    const Q: [u64; 25] = [
        0x40026bb1bbb55516,
        0xbcaf48ad494ea3e9,
        0x40053524c73cea69,
        0xbcae2bfab318d696,
        0x4000470591de2ca4,
        0x3ca823527cebf918,
        0x3ff2bd7609fd98c4,
        0x3c931ea51f6641df,
        0x3fe1429ffd1d4d76,
        0x3c7117195be7f232,
        0x3fca7ed70847c8b6,
        0xbc54260c5e23d0c8,
        0x3fb16e4dfc333a87,
        0xbc533fd284110905,
        0x3f94116b05fdaa5d,
        0xbc20721de44d79a8,
        0x3f74897c45d93d43,
        0x3f52ea52b2d182ac,
        0x3f2facfd5d905b22,
        0x3f084fe12df8bde3,
        0x3ee1398ad75d01bf,
        0x3eb6a9e96fbf6be7,
        0x3e8bd456a29007c2,
        0x3e6006cf8378cf9b,
        0x3e368862b132b6e2,
    ];
    let mut c13 = dd_fmla(f64::from_bits(Q[23]), x, f64::from_bits(Q[22])); // degree 15
    let c11 = dd_fmla(f64::from_bits(Q[21]), x, f64::from_bits(Q[20])); // degree 14
    c13 = dd_fmla(f64::from_bits(Q[24]), x2, c13); // degree 15
    // add Q[19]*x+c13*x2+c15*x4 to Q[18] (degree 11)
    let mut p = DoubleDouble::from_exact_add(
        f64::from_bits(Q[18]),
        f_fmla(f64::from_bits(Q[19]), x, f_fmla(c11, x2, c13 * x4)),
    );
    // multiply h+l by x and add Q[17] (degree 10)
    p = DoubleDouble::quick_f64_mult(x, p);
    let p0 = DoubleDouble::from_exact_add(f64::from_bits(Q[17]), p.hi);
    p.lo += p0.lo;
    p.hi = p0.hi;

    // multiply h+l by x and add Q[16] (degree 9)
    p = DoubleDouble::quick_f64_mult(x, p);
    let p0 = DoubleDouble::from_exact_add(f64::from_bits(Q[16]), p.hi);
    p.lo += p0.lo;
    p.hi = p0.hi;
    // multiply h+l by x and add Q[14]+Q[15] (degree 8)
    p = DoubleDouble::quick_f64_mult(x, p);
    let p0 = DoubleDouble::from_exact_add(f64::from_bits(Q[14]), p.hi);
    p.lo += p0.lo + f64::from_bits(Q[15]);
    p.hi = p0.hi;
    // multiply h+l by x and add Q[12]+Q[13] (degree 7)
    p = DoubleDouble::quick_f64_mult(x, p);
    let p0 = DoubleDouble::from_exact_add(f64::from_bits(Q[12]), p.hi);
    p.lo += p0.lo + f64::from_bits(Q[13]);
    p.hi = p0.hi;

    // multiply h+l by x and add Q[10]+Q[11] (degree 6)
    p = DoubleDouble::quick_f64_mult(x, p);
    let p0 = DoubleDouble::from_exact_add(f64::from_bits(Q[10]), p.hi);
    p.lo += p0.lo + f64::from_bits(Q[11]);
    p.hi = p0.hi;

    // multiply h+l by x and add Q[8]+Q[9] (degree 5)
    p = DoubleDouble::quick_f64_mult(x, p);
    let p0 = DoubleDouble::from_exact_add(f64::from_bits(Q[8]), p.hi);
    p.lo += p0.lo + f64::from_bits(Q[9]);
    p.hi = p0.hi;

    // multiply h+l by x and add Q[6]+Q[7] (degree 4)
    p = DoubleDouble::quick_f64_mult(x, p);
    let p0 = DoubleDouble::from_exact_add(f64::from_bits(Q[6]), p.hi);
    p.lo += p0.lo + f64::from_bits(Q[7]);
    p.hi = p0.hi;

    // multiply h+l by x and add Q[4]+Q[5] (degree 3)
    p = DoubleDouble::quick_f64_mult(x, p);
    let p0 = DoubleDouble::from_exact_add(f64::from_bits(Q[4]), p.hi);
    p.lo += p0.lo + f64::from_bits(Q[5]);
    p.hi = p0.hi;

    // multiply h+l by x and add Q[2]+Q[3] (degree 2)
    p = DoubleDouble::quick_f64_mult(x, p);
    let p0 = DoubleDouble::from_exact_add(f64::from_bits(Q[2]), p.hi);
    p.lo += p0.lo + f64::from_bits(Q[3]);
    p.hi = p0.hi;

    // multiply h+l by x and add Q[0]+Q[1] (degree 2)
    p = DoubleDouble::quick_f64_mult(x, p);
    let p0 = DoubleDouble::from_exact_add(f64::from_bits(Q[0]), p.hi);
    p.lo += p0.lo + f64::from_bits(Q[1]);
    p.hi = p0.hi;

    // multiply h+l by x
    p = DoubleDouble::quick_f64_mult(x, p);
    p.to_f64()
}

#[cold]
fn exp10m1_accurate(x: f64) -> f64 {
    let t = x.to_bits();
    let ux = t;
    let ax = ux & 0x7fffffffffffffffu64;

    if ax <= 0x3fc0000000000000u64 {
        // |x| <= 0.125
        return exp10m1_accurate_tiny(x);
    }

    let mut p = exp_2(x);

    let zf: DoubleDouble = DoubleDouble::from_full_exact_add(p.hi, -1.0);
    p.lo += zf.lo;
    p.hi = zf.hi;
    p.to_f64()
}

/* |x| <= 0.125, put in h + l a double-double approximation of exp2m1(x),
and return the maximal corresponding absolute error.
We also have |x| > 0x1.0527dbd87e24dp-51.
With xmin=RR("0x1.0527dbd87e24dp-51",16), the routine
exp2m1_fast_tiny_all(xmin,0.125,2^-65.73) in exp2m1.sage returns
1.63414352331297e-20 < 2^-65.73, and
exp2m1_fast_tiny_all(-0.125,-xmin,2^-65.62) returns
1.76283772822891e-20 < 2^-65.62, which proves the relative
error is bounded by 2^-65.62. */
#[inline]
fn exp10m1_fast_tiny(x: f64) -> Exp10m1 {
    /* The following is a degree-11 polynomial generated by Sollya
    (file exp10m1_fast.sollya),
    which approximates exp10m1(x) with relative error bounded by 2^-69.58
    for |x| <= 0.0625. */
    const P: [u64; 14] = [
        0x40026bb1bbb55516,
        0xbcaf48abcf79e094,
        0x40053524c73cea69,
        0xbcae1badf796d704,
        0x4000470591de2ca4,
        0x3ca7db8caacb2cea,
        0x3ff2bd7609fd98ba,
        0x3fe1429ffd1d4d98,
        0x3fca7ed7084998e1,
        0x3fb16e4dfc30944b,
        0x3f94116ae4b57526,
        0x3f74897c6a90f61c,
        0x3f52ec689c32b3a0,
        0x3f2faced20d698fe,
    ];
    let x2 = x * x;
    let x4 = x2 * x2;
    let mut c9 = dd_fmla(f64::from_bits(P[12]), x, f64::from_bits(P[11])); // degree 9
    let c7 = dd_fmla(f64::from_bits(P[10]), x, f64::from_bits(P[9])); // degree 7
    let mut c5 = dd_fmla(f64::from_bits(P[8]), x, f64::from_bits(P[7])); // degree 5
    c9 = dd_fmla(f64::from_bits(P[13]), x2, c9); // degree 9
    c5 = dd_fmla(c7, x2, c5); // degree 5
    c5 = dd_fmla(c9, x4, c5); // degree 5

    let mut p = DoubleDouble::from_exact_mult(c5, x);
    let p0 = DoubleDouble::from_exact_add(f64::from_bits(P[6]), p.hi);
    p.lo += p0.lo;
    p.hi = p0.hi;

    p = DoubleDouble::quick_f64_mult(x, p);

    let p1 = DoubleDouble::from_exact_add(f64::from_bits(P[4]), p.hi);
    p.lo += p1.lo + f64::from_bits(P[5]);
    p.hi = p1.hi;

    p = DoubleDouble::quick_f64_mult(x, p);

    let p2 = DoubleDouble::from_exact_add(f64::from_bits(P[2]), p.hi);
    p.lo += p2.lo + f64::from_bits(P[3]);
    p.hi = p2.hi;

    p = DoubleDouble::quick_f64_mult(x, p);

    let p2 = DoubleDouble::from_exact_add(f64::from_bits(P[0]), p.hi);
    p.lo += p2.lo + f64::from_bits(P[1]);
    p.hi = p2.hi;

    p = DoubleDouble::quick_f64_mult(x, p);

    Exp10m1 {
        exp: p,
        err: f64::from_bits(0x3bb0a00000000000) * p.hi, // 2^-65.62 < 0x1.4ep-66
    }
}

/// Computes 10^x - 1
///
/// Max found ULP 0.5
pub fn f_exp10m1(d: f64) -> f64 {
    let mut x = d;
    let t = x.to_bits();
    let ux = t;
    let ax = ux & 0x7fffffffffffffffu64;

    if ux >= 0xc03041704c068ef0u64 {
        // x = -NaN or x <= -0x1.041704c068efp+4
        if (ux >> 52) == 0xfff {
            // -NaN or -Inf
            return if ux > 0xfff0000000000000u64 {
                x + x
            } else {
                -1.0
            };
        }
        // for x <= -0x1.041704c068efp+4, exp10m1(x) rounds to -1 to nearest
        return -1.0 + f64::from_bits(0x3c90000000000000);
    } else if ax > 0x40734413509f79feu64 {
        // x = +NaN or x >= 1024
        if (ux >> 52) == 0x7ff {
            // +NaN
            return x + x;
        }
        return f64::from_bits(0x7fefffffffffffff) * x;
    } else if ax <= 0x3c90000000000000u64
    // |x| <= 0x1.0527dbd87e24dp-51
    /* then the second term of the Taylor expansion of 2^x-1 at x=0 is
    smaller in absolute value than 1/2 ulp(first term):
    log(2)*x + log(2)^2*x^2/2 + ... */
    {
        /* we use special code when log(2)*|x| is very small, in which case
        the double-double approximation h+l has its lower part l
        "truncated" */
        return if ax <= 0x3970000000000000u64
        // |x| <= 2^-104
        {
            // special case for 0
            if x == 0. {
                return x;
            }
            if x.abs() == f64::from_bits(0x000086c73059343c) {
                return dd_fmla(
                    -f64::copysign(f64::from_bits(0x1e60010000000000), x),
                    f64::from_bits(0x1e50000000000000),
                    f64::copysign(f64::from_bits(0x000136568740cb56), x),
                );
            }
            if x.abs() == f64::from_bits(0x00013a7b70d0248c) {
                return dd_fmla(
                    f64::copysign(f64::from_bits(0x1e5ffe0000000000), x),
                    f64::from_bits(0x1e50000000000000),
                    f64::copysign(f64::from_bits(0x0002d41f3b972fc7), x),
                );
            }

            // scale x by 2^106
            x *= f64::from_bits(0x4690000000000000);
            let mut z = DoubleDouble::from_exact_mult(LN10H, x);
            z.lo = dd_fmla(LN10L, x, z.lo);
            let mut h2 = z.to_f64(); // round to 53-bit precision
            // scale back, hoping to avoid double rounding
            h2 *= f64::from_bits(0x3950000000000000);
            // now subtract back h2 * 2^106 from h to get the correction term
            let mut h = dd_fmla(-h2, f64::from_bits(0x4690000000000000), z.hi);
            // add l
            h += z.lo;
            /* add h2 + h * 2^-106. Warning: when h=0, 2^-106*h2 might be exact,
            thus no underflow will be raised. We have underflow for
            0 < x <= 0x1.71547652b82fep-1022 for RNDZ, and for
            0 < x <= 0x1.71547652b82fdp-1022 for RNDN/RNDU. */
            dyad_fmla(h, f64::from_bits(0x3950000000000000), h2)
        } else {
            const C2: f64 = f64::from_bits(0x40053524c73cea69); // log(2)^2/2
            let mut z = DoubleDouble::quick_mult_f64(DoubleDouble::new(LN10L, LN10H), x);
            /* h+l approximates the first term x*log(2) */
            /* we add C2*x^2 last, so that in case there is a cancellation in
            LN10L*x+l, it will contribute more bits */
            z.lo = dd_fmla(C2 * x, x, z.lo);
            z.to_f64()
        };
    }

    /* now -0x1.041704c068efp+4 < x < -2^-54
    or 2^-54 < x <= 0x1.34413509f79fep+8 */

    /* 10^x-1 is exact for x integer, 1 <= x <= 15 */
    if ux << 15 == 0 {
        let i = x.floor() as i32;
        if x == i as f64 && 1 <= i && i <= 15 {
            static EXP10_1_15: [u64; 16] = [
                0x0000000000000000,
                0x4022000000000000,
                0x4058c00000000000,
                0x408f380000000000,
                0x40c3878000000000,
                0x40f869f000000000,
                0x412e847e00000000,
                0x416312cfe0000000,
                0x4197d783fc000000,
                0x41cdcd64ff800000,
                0x4202a05f1ff80000,
                0x42374876e7ff0000,
                0x426d1a94a1ffe000,
                0x42a2309ce53ffe00,
                0x42d6bcc41e8fffc0,
                0x430c6bf52633fff8,
            ];
            return f64::from_bits(EXP10_1_15[i as usize]);
        }
    }

    let result = exp10m1_fast(x, ax <= 0x3fb0000000000000u64);
    let left = result.exp.hi + (result.exp.lo - result.err);
    let right = result.exp.hi + (result.exp.lo + result.err);
    if left != right {
        return exp10m1_accurate(x);
    }
    left
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exp10m1() {
        assert_eq!(f_exp10m1(0.00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000002364140972981833),
                   0.00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000005443635762124408);
        assert_eq!(99., f_exp10m1(2.0));
        assert_eq!(315.22776601683796, f_exp10m1(2.5));
        assert_eq!(-0.7056827241416722, f_exp10m1(-0.5311842449009418));
    }
}
