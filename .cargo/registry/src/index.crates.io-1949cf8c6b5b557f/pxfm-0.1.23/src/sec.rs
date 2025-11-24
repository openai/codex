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
use crate::common::min_normal_f64;
use crate::double_double::DoubleDouble;
use crate::sin::{get_sin_k_rational, range_reduction_small, sincos_eval};
use crate::sin_table::SIN_K_PI_OVER_128;
use crate::sincos_dyadic::{range_reduction_small_f128, sincos_eval_dyadic};
use crate::sincos_reduce::LargeArgumentReduction;

#[cold]
fn sec_accurate(x: f64, argument_reduction: &mut LargeArgumentReduction, x_e: u64, k: u64) -> f64 {
    const EXP_BIAS: u64 = (1u64 << (11 - 1u64)) - 1u64;
    let u_f128 = if x_e < EXP_BIAS + 16 {
        range_reduction_small_f128(x)
    } else {
        argument_reduction.accurate()
    };

    let sin_cos = sincos_eval_dyadic(&u_f128);

    // -sin(k * pi/128) = sin((k + 128) * pi/128)
    // cos(k * pi/128) = sin(k * pi/128 + pi/2) = sin((k + 64) * pi/128).
    let msin_k_f128 = get_sin_k_rational(k.wrapping_add(128));
    let cos_k_f128 = get_sin_k_rational(k.wrapping_add(64));

    // cos(x) = cos((k * pi/128 + u)
    //        = cos(u) * cos(k*pi/128) - sin(u) * sin(k*pi/128)
    let r = (cos_k_f128 * sin_cos.v_cos) + (msin_k_f128 * sin_cos.v_sin);
    r.reciprocal().fast_as_f64()
}

/// Secant for double precision
///
/// ULP 0.5
pub fn f_sec(x: f64) -> f64 {
    let x_e = (x.to_bits() >> 52) & 0x7ff;
    const E_BIAS: u64 = (1u64 << (11 - 1u64)) - 1u64;

    let y: DoubleDouble;
    let k;

    let mut argument_reduction = LargeArgumentReduction::default();

    // |x| < 2^32 (with FMA) or |x| < 2^23 (w/o FMA)
    if x_e < E_BIAS + 16 {
        // |x| < 2^-26
        if x_e < E_BIAS - 7 {
            // |x| < 2^-26
            if x_e < E_BIAS - 27 {
                // Signed zeros.
                if x == 0.0 {
                    return 1.0;
                }
                // For |x| < 2^-26, |sin(x) - x| < ulp(x)/2.
                return 1.0 - min_normal_f64();
            }
            k = 0;
            y = DoubleDouble::new(0.0, x);
        } else {
            // Small range reduction.
            (y, k) = range_reduction_small(x);
        }
    } else {
        // Inf or NaN
        if x_e > 2 * E_BIAS {
            // sec(+-Inf) = NaN
            return x + f64::NAN;
        }

        // Large range reduction.
        // k = argument_reduction.high_part(x);
        (k, y) = argument_reduction.reduce(x);
    }
    let r_sincos = sincos_eval(y);

    // Fast look up version, but needs 256-entry table.
    // cos(k * pi/128) = sin(k * pi/128 + pi/2) = sin((k + 64) * pi/128).
    let sk = SIN_K_PI_OVER_128[(k.wrapping_add(128) & 255) as usize];
    let ck = SIN_K_PI_OVER_128[((k.wrapping_add(64)) & 255) as usize];
    let msin_k = DoubleDouble::from_bit_pair(sk);
    let cos_k = DoubleDouble::from_bit_pair(ck);

    let sin_k_cos_y = DoubleDouble::quick_mult(r_sincos.v_cos, cos_k);
    let cos_k_sin_y = DoubleDouble::quick_mult(r_sincos.v_sin, msin_k);

    let mut rr = DoubleDouble::from_full_exact_add(sin_k_cos_y.hi, cos_k_sin_y.hi);
    rr.lo += sin_k_cos_y.lo + cos_k_sin_y.lo;

    rr = rr.recip_raphson();

    let rlp = rr.lo + r_sincos.err;
    let rlm = rr.lo - r_sincos.err;

    let r_upper = rr.hi + rlp; // (rr.lo + ERR);
    let r_lower = rr.hi + rlm; // (rr.lo - ERR);

    // Ziv's accuracy test
    if r_upper == r_lower {
        return rr.to_f64();
    }

    sec_accurate(x, &mut argument_reduction, x_e, k)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sec() {
        assert_eq!(f_sec(-175432.), 1.461049620895326);
        assert_eq!(f_sec(175432.), 1.461049620895326);
        assert_eq!(f_sec(-10.), -1.1917935066878957);
        assert_eq!(f_sec(10.), -1.1917935066878957);
        assert_eq!(f_sec(5.), 3.5253200858160882);
        assert_eq!(f_sec(-5.), 3.5253200858160882);
        assert_eq!(f_sec(0.), 1.0);
        assert!(f_sec(f64::NAN).is_nan());
        assert!(f_sec(f64::INFINITY).is_nan());
        assert!(f_sec(f64::NEG_INFINITY).is_nan());
    }
}
