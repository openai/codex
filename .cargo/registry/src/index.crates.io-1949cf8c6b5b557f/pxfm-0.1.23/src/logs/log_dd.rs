/*
 * // Copyright (c) Radzivon Bartoshyk 8/2025. All rights reserved.
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
use crate::double_double::DoubleDouble;
use crate::logs::log_dd_coeffs::LOG_NEG_DD;
use crate::polyeval::f_polyeval7;
use crate::pow_tables::POW_INVERSE;

#[inline(always)]
pub(crate) fn log_poly(z: f64) -> DoubleDouble {
    /*
      See ./notes/dd_log.sollya
    */
    const P: [(u64, u64); 9] = [
        (0x3c755555556a4311, 0x3fd5555555555555),
        (0x3b8ffa82859b4000, 0xbfd0000000000000),
        (0xbc699b6b44796cd4, 0x3fc999999999999a),
        (0x3c489642b3424250, 0xbfc5555555555563),
        (0x3c60e3ef2c7fc443, 0x3fc24924924924ad),
        (0x3c535269fe0ce5df, 0xbfbfffffffc0b25c),
        (0x3c24f14f55e95858, 0x3fbc71c71c187ea4),
        (0x3c5b15951c5e1a17, 0xbfb999d71e5042d5),
        (0x3c5ecb0133e43410, 0x3fb74615b842a94d),
    ];
    let x2 = DoubleDouble::from_exact_mult(z, z);
    let mut t = DoubleDouble::quick_mul_f64_add(
        DoubleDouble::from_bit_pair(P[8]),
        z,
        DoubleDouble::from_bit_pair(P[7]),
    );
    t = DoubleDouble::quick_mul_f64_add(t, z, DoubleDouble::from_bit_pair(P[6]));
    t = DoubleDouble::quick_mul_f64_add(t, z, DoubleDouble::from_bit_pair(P[5]));
    t = DoubleDouble::quick_mul_f64_add(t, z, DoubleDouble::from_bit_pair(P[4]));
    t = DoubleDouble::quick_mul_f64_add(t, z, DoubleDouble::from_bit_pair(P[3]));
    t = DoubleDouble::quick_mul_f64_add(t, z, DoubleDouble::from_bit_pair(P[2]));
    t = DoubleDouble::quick_mul_f64_add(t, z, DoubleDouble::from_bit_pair(P[1]));
    t = DoubleDouble::quick_mul_f64_add(t, z, DoubleDouble::from_bit_pair(P[0]));
    t = DoubleDouble::quick_mult(t, x2);
    t = DoubleDouble::quick_mult_f64(t, z);
    DoubleDouble::mul_f64_add(x2, -0.5, t)
}

#[inline(always)]
pub(crate) fn log_poly_fast(z: f64) -> DoubleDouble {
    /*
      See ./notes/dd_log_fast.sollya
    */
    let x2 = DoubleDouble::from_exact_mult(z, z);
    let r = f_polyeval7(
        z,
        f64::from_bits(0xbfc5555555555555),
        f64::from_bits(0x3fc24924924924aa),
        f64::from_bits(0xbfc000000000bc96),
        f64::from_bits(0x3fbc71c71c202bf0),
        f64::from_bits(0xbfb9999839f1aa36),
        f64::from_bits(0x3fb74612adef67e0),
        f64::from_bits(0xbfb5cb6ab20b8efa),
    );
    let mut p = DoubleDouble::f64_mul_f64_add(
        z,
        r,
        DoubleDouble::from_bit_pair((0xbc699b293fa3344b, 0x3fc999999999999a)),
    );
    p = DoubleDouble::mul_f64_add(
        p,
        z,
        DoubleDouble::from_bit_pair((0xbb3b08905e500000, 0xbfd0000000000000)),
    );
    p = DoubleDouble::mul_f64_add(
        p,
        z,
        DoubleDouble::from_bit_pair((0x3c7555555567a1b0, 0x3fd5555555555555)),
    );
    let mut t = DoubleDouble::quick_mult(x2, p);
    t = DoubleDouble::quick_mult_f64(t, z);
    DoubleDouble::mul_f64_add(x2, -0.5, t)
}

#[inline]
pub(crate) fn log_dd(x: f64) -> DoubleDouble {
    let x_u = x.to_bits();
    let mut m = x_u & 0xfffffffffffff;
    let mut e: i64 = ((x_u >> 52) & 0x7ff) as i64;

    let t;
    if e != 0 {
        t = m | (0x3ffu64 << 52);
        m = m.wrapping_add(1u64 << 52);
        e -= 0x3ff;
    } else {
        /* x is a subnormal double  */
        let k = m.leading_zeros() - 11;

        e = -0x3fei64 - k as i64;
        m = m.wrapping_shl(k);
        t = m | (0x3ffu64 << 52);
    }

    /* now |x| = 2^_e*_t = 2^(_e-52)*m with 1 <= _t < 2,
    and 2^52 <= _m < 2^53 */

    //   log(x) = log(t) + E · log(2)
    let mut t = f64::from_bits(t);

    // If m > sqrt(2) we divide it by 2 so ensure 1/sqrt(2) < t < sqrt(2)
    let c: usize = (m >= 0x16a09e667f3bcd) as usize;
    static CY: [f64; 2] = [1.0, 0.5];
    static CM: [u64; 2] = [44, 45];

    e = e.wrapping_add(c as i64);
    let be = e;
    let i = m >> CM[c];
    t *= CY[c];

    let r = f64::from_bits(POW_INVERSE[(i - 181) as usize]);
    let log_r = DoubleDouble::from_bit_pair(LOG_NEG_DD[(i - 181) as usize]);

    let z = f64::mul_add(r, t, -1.0);

    const LOG2_DD: DoubleDouble = DoubleDouble::new(
        f64::from_bits(0x3c7abc9e3b39803f),
        f64::from_bits(0x3fe62e42fefa39ef),
    );

    let tt = DoubleDouble::mul_f64_add(LOG2_DD, be as f64, log_r);

    let v = DoubleDouble::full_add_f64(tt, z);
    let p = log_poly(z);
    DoubleDouble::f64_add(v.hi, DoubleDouble::new(v.lo + p.lo, p.hi))
}

#[inline]
pub(crate) fn log_dd_fast(x: f64) -> DoubleDouble {
    let x_u = x.to_bits();
    let mut m = x_u & 0xfffffffffffff;
    let mut e: i64 = ((x_u >> 52) & 0x7ff) as i64;

    let t;
    if e != 0 {
        t = m | (0x3ffu64 << 52);
        m = m.wrapping_add(1u64 << 52);
        e -= 0x3ff;
    } else {
        /* x is a subnormal double  */
        let k = m.leading_zeros() - 11;

        e = -0x3fei64 - k as i64;
        m = m.wrapping_shl(k);
        t = m | (0x3ffu64 << 52);
    }

    /* now |x| = 2^_e*_t = 2^(_e-52)*m with 1 <= _t < 2,
    and 2^52 <= _m < 2^53 */

    //   log(x) = log(t) + E · log(2)
    let mut t = f64::from_bits(t);

    // If m > sqrt(2) we divide it by 2 so ensure 1/sqrt(2) < t < sqrt(2)
    let c: usize = (m >= 0x16a09e667f3bcd) as usize;
    static CY: [f64; 2] = [1.0, 0.5];
    static CM: [u64; 2] = [44, 45];

    e = e.wrapping_add(c as i64);
    let be = e;
    let i = m >> CM[c];
    t *= CY[c];

    let r = f64::from_bits(POW_INVERSE[(i - 181) as usize]);
    let log_r = DoubleDouble::from_bit_pair(LOG_NEG_DD[(i - 181) as usize]);

    let z = f64::mul_add(r, t, -1.0);

    const LOG2_DD: DoubleDouble = DoubleDouble::new(
        f64::from_bits(0x3c7abc9e3b39803f),
        f64::from_bits(0x3fe62e42fefa39ef),
    );

    let tt = DoubleDouble::mul_f64_add(LOG2_DD, be as f64, log_r);

    let v = DoubleDouble::full_add_f64(tt, z);
    let p = log_poly_fast(z);
    DoubleDouble::f64_add(v.hi, DoubleDouble::new(v.lo + p.lo, p.hi))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_dd() {
        assert_eq!(log_dd(std::f64::consts::E).to_f64(), 1.);
    }
}
