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
use crate::logs::log2dd_coeffs::LOG2_NEG_DD;
use crate::pow_tables::POW_INVERSE;

#[inline(always)]
fn log2_poly(z: f64) -> DoubleDouble {
    /*
      See ./notes/dd_log2.sollya
    */
    const P: [(u64, u64); 10] = [
        (0x3c7777d0ffdb7a88, 0x3ff71547652b82fe),
        (0xbc6777d0fff4382c, 0xbfe71547652b82fe),
        (0x3c7d27ad39ed5b3e, 0x3fdec709dc3a03fd),
        (0xbc57748c64010a70, 0xbfd71547652b82fe),
        (0x3c771aed4208f9c1, 0x3fd2776c50ef9c06),
        (0x3c6dd0178c438bee, 0xbfcec709dc3a041c),
        (0x3c6054a5812d9fc4, 0x3fca61762a515608),
        (0x3c5662d846baea2e, 0xbfc7154764f1db89),
        (0x3c66b62df1fd3c4b, 0x3fc484dddfe20481),
        (0xbc6b16c93b93c1e3, 0xbfc2779d6a07c6b3),
    ];
    let mut t = DoubleDouble::from_bit_pair(P[9]);
    t = DoubleDouble::quick_mul_f64_add(t, z, DoubleDouble::from_bit_pair(P[8]));
    t = DoubleDouble::quick_mul_f64_add(t, z, DoubleDouble::from_bit_pair(P[7]));
    t = DoubleDouble::quick_mul_f64_add(t, z, DoubleDouble::from_bit_pair(P[6]));
    t = DoubleDouble::quick_mul_f64_add(t, z, DoubleDouble::from_bit_pair(P[5]));
    t = DoubleDouble::quick_mul_f64_add(t, z, DoubleDouble::from_bit_pair(P[4]));
    t = DoubleDouble::quick_mul_f64_add(t, z, DoubleDouble::from_bit_pair(P[3]));
    t = DoubleDouble::quick_mul_f64_add(t, z, DoubleDouble::from_bit_pair(P[2]));
    t = DoubleDouble::quick_mul_f64_add(t, z, DoubleDouble::from_bit_pair(P[1]));
    t = DoubleDouble::quick_mul_f64_add(t, z, DoubleDouble::from_bit_pair(P[0]));
    DoubleDouble::quick_mult_f64(t, z)
}

#[inline]
pub(crate) fn log2_dd(x: f64) -> DoubleDouble {
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

    //   log2(x) = log2(t) + E · log(2)
    let mut t = f64::from_bits(t);

    // If m > sqrt(2) we divide it by 2 so ensure 1/sqrt(2) < t < sqrt(2)
    let c: usize = (m >= 0x16a09e667f3bcd) as usize;
    static CY: [f64; 2] = [1.0, 0.5];
    static CM: [u64; 2] = [44, 45];

    e = e.wrapping_add(c as i64);
    let be = e;
    let i = m >> CM[c];
    t *= CY[c];

    let idx = (i - 181) as usize;

    let r = f64::from_bits(POW_INVERSE[idx]);
    let log_r = DoubleDouble::from_bit_pair(LOG2_NEG_DD[idx]);

    let z = f64::mul_add(r, t, -1.0);

    let v = DoubleDouble::full_add_f64(log_r, be as f64);
    let p = log2_poly(z);
    let z0 = DoubleDouble::new(v.lo + p.lo, p.hi);
    DoubleDouble::full_add_f64(z0, v.hi)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log2dd_test() {
        assert_eq!(log2_dd(0.0040283203125 / 2.).to_f64(), -8.955605880641546);
    }
}
