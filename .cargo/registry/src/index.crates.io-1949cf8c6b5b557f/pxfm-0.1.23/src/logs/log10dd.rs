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
use crate::logs::log10dd_coeffs::LOG10_NEG_DD;
use crate::pow_tables::POW_INVERSE;

#[inline(always)]
fn log10_poly(z: f64) -> DoubleDouble {
    /*
      See ./notes/dd_log10.sollya
    */
    const P: [(u64, u64); 10] = [
        (0x3c695355baabd6d2, 0x3fdbcb7b1526e50e),
        (0xbc595355bababc0c, 0xbfcbcb7b1526e50e),
        (0xbc59c8c251be5fee, 0x3fc287a7636f435f),
        (0xbc49515e0a562cbe, 0xbfbbcb7b1526e50e),
        (0xbc5603802582ebad, 0x3fb63c62775250e2),
        (0x3c37a0e7de39c80c, 0xbfb287a7636f4371),
        (0x3c2c4afd722dc9ec, 0x3fafc3fa611f0291),
        (0xbc4a52f6541b67ee, 0xbfabcb7b14e178e5),
        (0x3c3ef541147ede8e, 0x3fa8b514ee02cfad),
        (0xbc28b9ea54e8f3b4, 0xbfa63c9d960501f1),
    ];
    let mut t = DoubleDouble::quick_mul_f64_add(
        DoubleDouble::from_bit_pair(P[9]),
        z,
        DoubleDouble::from_bit_pair(P[8]),
    );
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
pub(crate) fn log10_dd(x: f64) -> DoubleDouble {
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

    //   log10(x) = log10(t) + E · log10(e)
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
    let log10_r = DoubleDouble::from_bit_pair(LOG10_NEG_DD[(i - 181) as usize]);

    let z = f64::mul_add(r, t, -1.0);

    const LOG10_2_DD: DoubleDouble =
        DoubleDouble::from_bit_pair((0xbc49dc1da994fd21, 0x3fd34413509f79ff));

    let v = DoubleDouble::mul_f64_add(LOG10_2_DD, be as f64, log10_r);

    let p = log10_poly(z);
    DoubleDouble::f64_add(v.hi, DoubleDouble::new(v.lo + p.lo, p.hi))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log10_dd() {
        assert_eq!(log10_dd(10.).to_f64(), 1.);
    }
}
