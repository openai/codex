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
use crate::logs::log2td_coeffs::LOG2_NEG_TD;
use crate::pow_tables::POW_INVERSE;
use crate::triple_double::TripleDouble;

#[inline(always)]
fn log2_poly(z: f64) -> TripleDouble {
    /*
      See ./notes/dd_log.sollya
    */
    const P: [(u64, u64, u64); 11] = [
        (0x38cb46674646bfff, 0x3c7777d0ffda0d23, 0x3ff71547652b82fe),
        (0xb90ea6e2f55cd900, 0xbc6777d0ffe87198, 0xbfe71547652b82fe),
        (0x391e004d54467330, 0x3c7d27f0556d8546, 0x3fdec709dc3a03fd),
        (0xb8b2d21aeeb27bff, 0xbc5775b3aa82c433, 0xbfd71547652b82fe),
        (0xb9164a4a186a0c00, 0x3c7e49c9bdb8b680, 0x3fd2776c50ef9bfe),
        (0xb8efe23702b5a940, 0x3c6195ba6326b1bf, 0xbfcec709dc3a0414),
        (0x38d7695a46fb4b00, 0x3c6f82e7add9bb4d, 0x3fca61762a7adf00),
        (0xb8c00e9d3285e000, 0x3c2caf76a9ee1e78, 0xbfc7154764fba5e4),
        (0xb8d1ff2b356eee80, 0xbc3f6102bc5ddc49, 0x3fc484b13d3bbed8),
        (0xb8edd22b4add09c0, 0x3c4c1da4a1a32f3b, 0xbfc2779952952c26),
        (0x388875bd65660001, 0x3c58e09839d588dd, 0x3fc0c9d962b39a7d),
    ];
    let mut t = TripleDouble::from_bit_pair(P[10]);
    t = TripleDouble::f64_mul_add(z, t, TripleDouble::from_bit_pair(P[9]));
    t = TripleDouble::f64_mul_add(z, t, TripleDouble::from_bit_pair(P[8]));
    t = TripleDouble::f64_mul_add(z, t, TripleDouble::from_bit_pair(P[7]));
    t = TripleDouble::f64_mul_add(z, t, TripleDouble::from_bit_pair(P[6]));
    t = TripleDouble::f64_mul_add(z, t, TripleDouble::from_bit_pair(P[5]));
    t = TripleDouble::f64_mul_add(z, t, TripleDouble::from_bit_pair(P[4]));
    t = TripleDouble::f64_mul_add(z, t, TripleDouble::from_bit_pair(P[3]));
    t = TripleDouble::f64_mul_add(z, t, TripleDouble::from_bit_pair(P[2]));
    t = TripleDouble::f64_mul_add(z, t, TripleDouble::from_bit_pair(P[1]));
    t = TripleDouble::f64_mul_add(z, t, TripleDouble::from_bit_pair(P[0]));
    TripleDouble::quick_mult_f64(t, z)
}

#[inline]
pub(crate) fn log2_td(x: f64) -> TripleDouble {
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

    let r = f64::from_bits(POW_INVERSE[(i - 181) as usize]);
    let log_r = TripleDouble::from_bit_pair(LOG2_NEG_TD[(i - 181) as usize]);

    let z = f64::mul_add(r, t, -1.0);

    let v = TripleDouble::add_f64(be as f64, log_r);
    let p = log2_poly(z);
    TripleDouble::add_f64(v.hi, TripleDouble::new(v.lo + p.lo, v.mid + p.mid, p.hi))
}

#[cfg(test)]
mod tests {
    use crate::logs::log2td::log2_td;

    #[test]
    fn log2td_test() {
        assert_eq!(log2_td(0.0040283203125 / 2.).to_f64(), -8.955605880641546);
    }
}
