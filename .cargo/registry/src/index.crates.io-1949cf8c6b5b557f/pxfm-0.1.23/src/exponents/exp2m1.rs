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
use crate::exponents::fast_ldexp;

const LN2H: f64 = f64::from_bits(0x3fe62e42fefa39ef);
const LN2L: f64 = f64::from_bits(0x3c7abc9e3b39803f);

struct Exp2m1 {
    exp: DoubleDouble,
    err: f64,
}

/* For 0 <= i < 64, T1[i] = (h,l) such that h+l is the best double-double
approximation of 2^(i/64). The approximation error is bounded as follows:
|h + l - 2^(i/64)| < 2^-107. */
pub(crate) static EXP_M1_2_TABLE1: [(u64, u64); 64] = [
    (0x0000000000000000, 0x3ff0000000000000),
    (0xbc719083535b085d, 0x3ff02c9a3e778061),
    (0x3c8d73e2a475b465, 0x3ff059b0d3158574),
    (0x3c6186be4bb284ff, 0x3ff0874518759bc8),
    (0x3c98a62e4adc610b, 0x3ff0b5586cf9890f),
    (0x3c403a1727c57b53, 0x3ff0e3ec32d3d1a2),
    (0xbc96c51039449b3a, 0x3ff11301d0125b51),
    (0xbc932fbf9af1369e, 0x3ff1429aaea92de0),
    (0xbc819041b9d78a76, 0x3ff172b83c7d517b),
    (0x3c8e5b4c7b4968e4, 0x3ff1a35beb6fcb75),
    (0x3c9e016e00a2643c, 0x3ff1d4873168b9aa),
    (0x3c8dc775814a8495, 0x3ff2063b88628cd6),
    (0x3c99b07eb6c70573, 0x3ff2387a6e756238),
    (0x3c82bd339940e9d9, 0x3ff26b4565e27cdd),
    (0x3c8612e8afad1255, 0x3ff29e9df51fdee1),
    (0x3c90024754db41d5, 0x3ff2d285a6e4030b),
    (0x3c86f46ad23182e4, 0x3ff306fe0a31b715),
    (0x3c932721843659a6, 0x3ff33c08b26416ff),
    (0xbc963aeabf42eae2, 0x3ff371a7373aa9cb),
    (0xbc75e436d661f5e3, 0x3ff3a7db34e59ff7),
    (0x3c8ada0911f09ebc, 0x3ff3dea64c123422),
    (0xbc5ef3691c309278, 0x3ff4160a21f72e2a),
    (0x3c489b7a04ef80d0, 0x3ff44e086061892d),
    (0x3c73c1a3b69062f0, 0x3ff486a2b5c13cd0),
    (0x3c7d4397afec42e2, 0x3ff4bfdad5362a27),
    (0xbc94b309d25957e3, 0x3ff4f9b2769d2ca7),
    (0xbc807abe1db13cad, 0x3ff5342b569d4f82),
    (0x3c99bb2c011d93ad, 0x3ff56f4736b527da),
    (0x3c96324c054647ad, 0x3ff5ab07dd485429),
    (0x3c9ba6f93080e65e, 0x3ff5e76f15ad2148),
    (0xbc9383c17e40b497, 0x3ff6247eb03a5585),
    (0xbc9bb60987591c34, 0x3ff6623882552225),
    (0xbc9bdd3413b26456, 0x3ff6a09e667f3bcd),
    (0xbc6bbe3a683c88ab, 0x3ff6dfb23c651a2f),
    (0xbc816e4786887a99, 0x3ff71f75e8ec5f74),
    (0xbc90245957316dd3, 0x3ff75feb564267c9),
    (0xbc841577ee04992f, 0x3ff7a11473eb0187),
    (0x3c705d02ba15797e, 0x3ff7e2f336cf4e62),
    (0xbc9d4c1dd41532d8, 0x3ff82589994cce13),
    (0xbc9fc6f89bd4f6ba, 0x3ff868d99b4492ed),
    (0x3c96e9f156864b27, 0x3ff8ace5422aa0db),
    (0x3c85cc13a2e3976c, 0x3ff8f1ae99157736),
    (0xbc675fc781b57ebc, 0x3ff93737b0cdc5e5),
    (0xbc9d185b7c1b85d1, 0x3ff97d829fde4e50),
    (0x3c7c7c46b071f2be, 0x3ff9c49182a3f090),
    (0xbc9359495d1cd533, 0x3ffa0c667b5de565),
    (0xbc9d2f6edb8d41e1, 0x3ffa5503b23e255d),
    (0x3c90fac90ef7fd31, 0x3ffa9e6b5579fdbf),
    (0x3c97a1cd345dcc81, 0x3ffae89f995ad3ad),
    (0xbc62805e3084d708, 0x3ffb33a2b84f15fb),
    (0xbc75584f7e54ac3b, 0x3ffb7f76f2fb5e47),
    (0x3c823dd07a2d9e84, 0x3ffbcc1e904bc1d2),
    (0x3c811065895048dd, 0x3ffc199bdd85529c),
    (0x3c92884dff483cad, 0x3ffc67f12e57d14b),
    (0x3c7503cbd1e949db, 0x3ffcb720dcef9069),
    (0xbc9cbc3743797a9c, 0x3ffd072d4a07897c),
    (0x3c82ed02d75b3707, 0x3ffd5818dcfba487),
    (0x3c9c2300696db532, 0x3ffda9e603db3285),
    (0xbc91a5cd4f184b5c, 0x3ffdfc97337b9b5f),
    (0x3c839e8980a9cc8f, 0x3ffe502ee78b3ff6),
    (0xbc9e9c23179c2893, 0x3ffea4afa2a490da),
    (0x3c9dc7f486a4b6b0, 0x3ffefa1bee615a27),
    (0x3c99d3e12dd8a18b, 0x3fff50765b6e4540),
    (0x3c874853f3a5931e, 0x3fffa7c1819e90d8),
];

/* For 0 <= i < 64, T2[i] = (h,l) such that h+l is the best double-double
approximation of 2^(i/2^12). The approximation error is bounded as follows:
|h + l - 2^(i/2^12)| < 2^-107. */
pub(crate) static EXP_M1_2_TABLE2: [(u64, u64); 64] = [
    (0x0000000000000000, 0x3ff0000000000000),
    (0x3c9ae8e38c59c72a, 0x3ff000b175effdc7),
    (0xbc57b5d0d58ea8f4, 0x3ff00162f3904052),
    (0x3c94115cb6b16a8e, 0x3ff0021478e11ce6),
    (0xbc8d7c96f201bb2f, 0x3ff002c605e2e8cf),
    (0x3c984711d4c35e9f, 0x3ff003779a95f959),
    (0xbc80484245243777, 0x3ff0042936faa3d8),
    (0xbc94b237da2025f9, 0x3ff004dadb113da0),
    (0xbc75e00e62d6b30d, 0x3ff0058c86da1c0a),
    (0x3c9a1d6cedbb9481, 0x3ff0063e3a559473),
    (0xbc94acf197a00142, 0x3ff006eff583fc3d),
    (0xbc6eaf2ea42391a5, 0x3ff007a1b865a8ca),
    (0x3c7da93f90835f75, 0x3ff0085382faef83),
    (0xbc86a79084ab093c, 0x3ff00905554425d4),
    (0x3c986364f8fbe8f8, 0x3ff009b72f41a12b),
    (0xbc882e8e14e3110e, 0x3ff00a6910f3b6fd),
    (0xbc84f6b2a7609f71, 0x3ff00b1afa5abcbf),
    (0xbc7e1a258ea8f71b, 0x3ff00bcceb7707ec),
    (0x3c74362ca5bc26f1, 0x3ff00c7ee448ee02),
    (0x3c9095a56c919d02, 0x3ff00d30e4d0c483),
    (0xbc6406ac4e81a645, 0x3ff00de2ed0ee0f5),
    (0x3c9b5a6902767e09, 0x3ff00e94fd0398e0),
    (0xbc991b2060859321, 0x3ff00f4714af41d3),
    (0x3c8427068ab22306, 0x3ff00ff93412315c),
    (0x3c9c1d0660524e08, 0x3ff010ab5b2cbd11),
    (0xbc9e7bdfb3204be8, 0x3ff0115d89ff3a8b),
    (0x3c8843aa8b9cbbc6, 0x3ff0120fc089ff63),
    (0xbc734104ee7edae9, 0x3ff012c1fecd613b),
    (0xbc72b6aeb6176892, 0x3ff0137444c9b5b5),
    (0x3c7a8cd33b8a1bb3, 0x3ff01426927f5278),
    (0x3c72edc08e5da99a, 0x3ff014d8e7ee8d2f),
    (0x3c857ba2dc7e0c73, 0x3ff0158b4517bb88),
    (0x3c9b61299ab8cdb7, 0x3ff0163da9fb3335),
    (0xbc990565902c5f44, 0x3ff016f0169949ed),
    (0x3c870fc41c5c2d53, 0x3ff017a28af25567),
    (0x3c94b9a6e145d76c, 0x3ff018550706ab62),
    (0xbc7008eff5142bf9, 0x3ff019078ad6a19f),
    (0xbc977669f033c7de, 0x3ff019ba16628de2),
    (0xbc909bb78eeead0a, 0x3ff01a6ca9aac5f3),
    (0x3c9371231477ece5, 0x3ff01b1f44af9f9e),
    (0x3c75e7626621eb5b, 0x3ff01bd1e77170b4),
    (0xbc9bc72b100828a5, 0x3ff01c8491f08f08),
    (0xbc6ce39cbbab8bbe, 0x3ff01d37442d5070),
    (0x3c816996709da2e2, 0x3ff01de9fe280ac8),
    (0xbc8c11f5239bf535, 0x3ff01e9cbfe113ef),
    (0x3c8e1d4eb5edc6b3, 0x3ff01f4f8958c1c6),
    (0xbc9afb99946ee3f0, 0x3ff020025a8f6a35),
    (0xbc98f06d8a148a32, 0x3ff020b533856324),
    (0xbc82bf310fc54eb6, 0x3ff02168143b0281),
    (0xbc9c95a035eb4175, 0x3ff0221afcb09e3e),
    (0xbc9491793e46834d, 0x3ff022cdece68c4f),
    (0xbc73e8d0d9c49091, 0x3ff02380e4dd22ad),
    (0xbc9314aa16278aa3, 0x3ff02433e494b755),
    (0x3c848daf888e9651, 0x3ff024e6ec0da046),
    (0x3c856dc8046821f4, 0x3ff02599fb483385),
    (0x3c945b42356b9d47, 0x3ff0264d1244c719),
    (0xbc7082ef51b61d7e, 0x3ff027003103b10e),
    (0x3c72106ed0920a34, 0x3ff027b357854772),
    (0xbc9fd4cf26ea5d0f, 0x3ff0286685c9e059),
    (0xbc909f8775e78084, 0x3ff02919bbd1d1d8),
    (0x3c564cbba902ca27, 0x3ff029ccf99d720a),
    (0x3c94383ef231d207, 0x3ff02a803f2d170d),
    (0x3c94a47a505b3a47, 0x3ff02b338c811703),
    (0x3c9e47120223467f, 0x3ff02be6e199c811),
];

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
    const LOG2DD: DoubleDouble = DoubleDouble::new(LOG2L, LOG2H);
    let zk = DoubleDouble::quick_mult_f64(LOG2DD, k);

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
fn exp2m1_fast(x: f64, tiny: bool) -> Exp2m1 {
    if tiny {
        return exp2m1_fast_tiny(x);
    }
    /* now -54 < x < -0.125 or 0.125 < x < 1024: we approximate exp(x*log(2))
    and subtract 1 */
    let mut v = DoubleDouble::from_exact_mult(LN2H, x);
    v.lo = f_fmla(x, LN2L, v.lo);
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
    Exp2m1 {
        exp: p,
        err: f64::from_bits(0x3b84200000000000) * p.hi, /* 2^-70.67 < 0x1.42p-71 */
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

// returns a double-double approximation hi+lo of exp(x*log(2)) for |x| < 745
#[inline]
fn exp_2(x: f64) -> DoubleDouble {
    let k = (x * f64::from_bits(0x40b0000000000000)).round_ties_even();
    // since |x| <= 745 we have k <= 3051520

    let yhh = f_fmla(-k, f64::from_bits(0x3f30000000000000), x); // exact, |yh| <= 2^-13

    /* now x = k + yh, thus 2^x = 2^k * 2^yh, and we multiply yh by log(2)
    to use the accurate path of exp() */
    let ky = DoubleDouble::quick_f64_mult(yhh, DoubleDouble::new(LN2L, LN2H));

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
pub(crate) fn exp2m1_accurate_tiny(x: f64) -> f64 {
    let x2 = x * x;
    let x4 = x2 * x2;
    const Q: [u64; 22] = [
        0x3fe62e42fefa39ef,
        0x3c7abc9e3b398040,
        0x3fcebfbdff82c58f,
        0xbc65e43a53e44dcf,
        0x3fac6b08d704a0c0,
        0xbc4d331627517168,
        0x3f83b2ab6fba4e77,
        0x3c14e65df0779f8c,
        0x3f55d87fe78a6731,
        0x3bd0717fbf4bd050,
        0x3f2430912f86c787,
        0x3bcbd2bdec9bcd42,
        0x3eeffcbfc588b0c7,
        0xbb8e60aa6d5e4aa9,
        0x3eb62c0223a5c824,
        0x3e7b5253d395e7d4,
        0x3e3e4cf5158b9160,
        0x3dfe8cac734c6058,
        0x3dbc3bd64f17199d,
        0x3d78161a17e05651,
        0x3d33150b3d792231,
        0x3cec184260bfad7e,
    ];
    let mut c13 = dd_fmla(f64::from_bits(Q[20]), x, f64::from_bits(Q[19])); // degree 13
    let c11 = dd_fmla(f64::from_bits(Q[18]), x, f64::from_bits(Q[17])); // degree 11
    c13 = dd_fmla(f64::from_bits(Q[21]), x2, c13); // degree 13
    // add Q[16]*x+c11*x2+c13*x4 to Q[15] (degree 9)
    let mut p = DoubleDouble::from_exact_add(
        f64::from_bits(Q[15]),
        f_fmla(f64::from_bits(Q[16]), x, f_fmla(c11, x2, c13 * x4)),
    );
    // multiply h+l by x and add Q[14] (degree 8)
    p = DoubleDouble::quick_f64_mult(x, p);
    let p0 = DoubleDouble::from_exact_add(f64::from_bits(Q[14]), p.hi);
    p.lo += p0.lo;
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
fn exp2m1_accurate(x: f64) -> f64 {
    let t = x.to_bits();
    let ux = t;
    let ax = ux & 0x7fffffffffffffffu64;

    if ax <= 0x3fc0000000000000u64 {
        // |x| <= 0.125
        return exp2m1_accurate_tiny(x);
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
fn exp2m1_fast_tiny(x: f64) -> Exp2m1 {
    /* The maximal value of |c4*x^4/exp2m1(x)| over [-0.125,0.125]
    is less than 2^-15.109, where c4 is the degree-4 coefficient,
    thus we can compute the coefficients of degree 4 or higher
    using double precision only. */
    const P: [u64; 12] = [
        0x3fe62e42fefa39ef,
        0x3c7abd1697afcaf8,
        0x3fcebfbdff82c58f,
        0xbc65e5a1d09e1599,
        0x3fac6b08d704a0bf,
        0x3f83b2ab6fba4e78,
        0x3f55d87fe78a84e6,
        0x3f2430912f86a480,
        0x3eeffcbfbc1f2b36,
        0x3eb62c0226c7f6d1,
        0x3e7b539529819e63,
        0x3e3e4d552bed5b9c,
    ];
    let x2 = x * x;
    let x4 = x2 * x2;
    let mut c8 = dd_fmla(f64::from_bits(P[10]), x, f64::from_bits(P[9])); // degree 8
    let c6 = dd_fmla(f64::from_bits(P[8]), x, f64::from_bits(P[7])); // degree 6
    let mut c4 = dd_fmla(f64::from_bits(P[6]), x, f64::from_bits(P[5])); // degree 4
    c8 = dd_fmla(f64::from_bits(P[11]), x2, c8); // degree 8
    c4 = dd_fmla(c6, x2, c4); // degree 4
    c4 = dd_fmla(c8, x4, c4); // degree 4

    let mut p = DoubleDouble::from_exact_mult(c4, x);
    let p0 = DoubleDouble::from_exact_add(f64::from_bits(P[4]), p.hi);
    p.lo += p0.lo;
    p.hi = p0.hi;

    p = DoubleDouble::quick_f64_mult(x, p);

    let p1 = DoubleDouble::from_exact_add(f64::from_bits(P[2]), p.hi);
    p.lo += p1.lo + f64::from_bits(P[3]);
    p.hi = p1.hi;

    p = DoubleDouble::quick_f64_mult(x, p);
    let p2 = DoubleDouble::from_exact_add(f64::from_bits(P[0]), p.hi);
    p.lo += p2.lo + f64::from_bits(P[1]);
    p.hi = p2.hi;

    p = DoubleDouble::quick_f64_mult(x, p);

    Exp2m1 {
        exp: p,
        err: f64::from_bits(0x3bd4e00000000000) * p.hi, // 2^-65.62 < 0x1.4ep-66
    }
}

/// Computes 2^x - 1
///
/// Max found ULP 0.5
pub fn f_exp2m1(d: f64) -> f64 {
    let mut x = d;
    let t = x.to_bits();
    let ux = t;
    let ax = ux & 0x7fffffffffffffffu64;

    if ux >= 0xc04b000000000000u64 {
        // x = -NaN or x <= -54
        if (ux >> 52) == 0xfff {
            // -NaN or -Inf
            return if ux > 0xfff0000000000000u64 {
                x + x
            } else {
                -1.0
            };
        }
        // for x <= -54, exp2m1(x) rounds to -1 to nearest
        return -1.0 + f64::from_bits(0x3c90000000000000);
    } else if ax >= 0x4090000000000000u64 {
        // x = +NaN or x >= 1024
        if (ux >> 52) == 0x7ff {
            // +NaN
            return x + x;
        }
        /* for x >= 1024, exp2m1(x) rounds to +Inf to nearest,
        but for RNDZ/RNDD, we should have no overflow for x=1024 */
        return f_fmla(
            x,
            f64::from_bits(0x7bffffffffffffff),
            f64::from_bits(0x7fefffffffffffff),
        );
    } else if ax <= 0x3cc0527dbd87e24du64
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
            // scale x by 2^106
            x *= f64::from_bits(0x4690000000000000);
            let z = DoubleDouble::quick_mult_f64(DoubleDouble::new(LN2L, LN2H), x);
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
            const C2: f64 = f64::from_bits(0x3fcebfbdff82c58f); // log(2)^2/2
            let mut z = DoubleDouble::from_exact_mult(LN2H, x);
            z.lo = dyad_fmla(LN2L, x, z.lo);
            /* h+l approximates the first term x*log(2) */
            /* we add C2*x^2 last, so that in case there is a cancellation in
            LN2L*x+l, it will contribute more bits */
            z.lo += C2 * x * x;
            z.to_f64()
        };
    }

    /* now -54 < x < -0x1.0527dbd87e24dp-51
    or 0x1.0527dbd87e24dp-51 < x < 1024 */

    /* 2^x-1 is exact for x integer, -53 <= x <= 53 */
    if ux.wrapping_shl(17) == 0 {
        let i = x.floor() as i32;
        if x == i as f64 && -53 <= i && i <= 53 {
            return if i >= 0 {
                ((1u64 << i) - 1) as f64
            } else {
                -1.0 + fast_ldexp(1.0, i)
            };
        }
    }

    let result = exp2m1_fast(x, ax <= 0x3fc0000000000000u64);
    let left = result.exp.hi + (result.exp.lo - result.err);
    let right = result.exp.hi + (result.exp.lo + result.err);
    if left != right {
        return exp2m1_accurate(x);
    }
    left
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exp2m1() {
        assert_eq!(f_exp2m1(5.4172231599824623E-312), 3.75493295981e-312);
        assert_eq!(f_exp2m1( 0.00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000017800593653177087), 0.00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000012338431302992956);
        assert_eq!(3., f_exp2m1(2.0));
        assert_eq!(4.656854249492381, f_exp2m1(2.5));
        assert_eq!(-0.30801352040368324, f_exp2m1(-0.5311842449009418));
    }
}
