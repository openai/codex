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
use crate::logs::log_td_table::LOG_NEG_TD;
use crate::pow_tables::POW_INVERSE;
use crate::triple_double::TripleDouble;

#[inline(always)]
fn log_td_poly(z: f64) -> TripleDouble {
    /*
      See ./notes/td_log.sollya
    */
    const P: [(u64, u64, u64); 10] = [
        (0x38fc6038ddcc5680, 0x3c755555556795ff, 0x3fd5555555555555),
        (0x372720effda9e638, 0xba86f68986749d55, 0xbfd0000000000000),
        (0x38deb738e67c5280, 0xbc699b2852652c20, 0x3fc999999999999a),
        (0x3900b719d69f9950, 0xbc65526cf5c3f935, 0xbfc5555555555555),
        (0xb8ad0f51dccb7c02, 0xbc34fc5a6b8b9f2d, 0x3fc24924924924aa),
        (0xb8faeee1e6f2b960, 0xbc52e65780d7da8f, 0xbfc0000000000023),
        (0xb8f306e8001c6280, 0x3c5d0f68afb70c37, 0x3fbc71c71c2042d4),
        (0x38d6940b092bf340, 0xbc3280fbebf81ea0, 0xbfb999999934f78b),
        (0xb8daa1cce08da780, 0x3c48da171d2f9c73, 0x3fb74612a55c4784),
        (0x38e9232517db1c80, 0x3c553b70480fc993, 0xbfb5559a592ab15c),
    ];
    let x2 = DoubleDouble::from_exact_mult(z, z);
    let mut t = TripleDouble::f64_mul_add(
        z,
        TripleDouble::from_bit_pair(P[9]),
        TripleDouble::from_bit_pair(P[8]),
    );
    t = TripleDouble::f64_mul_add(z, t, TripleDouble::from_bit_pair(P[7]));
    t = TripleDouble::f64_mul_add(z, t, TripleDouble::from_bit_pair(P[6]));
    t = TripleDouble::f64_mul_add(z, t, TripleDouble::from_bit_pair(P[5]));
    t = TripleDouble::f64_mul_add(z, t, TripleDouble::from_bit_pair(P[4]));
    t = TripleDouble::f64_mul_add(z, t, TripleDouble::from_bit_pair(P[3]));
    t = TripleDouble::f64_mul_add(z, t, TripleDouble::from_bit_pair(P[2]));
    t = TripleDouble::f64_mul_add(z, t, TripleDouble::from_bit_pair(P[1]));
    t = TripleDouble::f64_mul_add(z, t, TripleDouble::from_bit_pair(P[0]));
    t = TripleDouble::quick_mult_dd(t, x2);
    t = TripleDouble::quick_mult_f64(t, z);
    TripleDouble::f64_mul_dd_add(-0.5, x2, t)
}

#[inline]
pub(crate) fn log_td(x: f64) -> TripleDouble {
    let x_u = x.to_bits();
    let mut m = x_u & 0xfffffffffffff;
    let mut e: i64 = ((x_u >> 52) & 0x7ff) as i64;

    let t;
    if e != 0 {
        t = m | (0x3ffu64 << 52);
        m = m.wrapping_add(1u64 << 52);
        e -= 0x3ff;
    } else {
        /* x is a subnormal double */
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

    let log_r = TripleDouble::from_bit_pair(LOG_NEG_TD[(i - 181) as usize]);

    let z = f64::mul_add(r, t, -1.0);

    const LOG2_DD: TripleDouble =
        TripleDouble::from_bit_pair((0x3907b57a079a1934, 0x3c7abc9e3b39803f, 0x3fe62e42fefa39ef));

    let tt = TripleDouble::f64_mul_add(be as f64, LOG2_DD, log_r);

    let v = TripleDouble::add_f64(z, tt);
    let p = log_td_poly(z);
    TripleDouble::add_f64(v.hi, TripleDouble::new(v.lo + p.lo, v.mid + p.mid, p.hi))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_td() {
        assert_eq!(log_td(184467440737095500000.).to_f64(), 46.66400464883055);
    }
}
