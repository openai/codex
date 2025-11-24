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
use crate::bits::EXP_MASK;
use crate::common::f_fmla;
use crate::double_double::DoubleDouble;
use crate::sin::{get_sin_k_rational, range_reduction_small};
use crate::sin_table::SIN_K_PI_OVER_128;
use crate::sincos_dyadic::range_reduction_small_f128;
use crate::sincos_reduce::LargeArgumentReduction;
use crate::tangent::tan::{newton_raphson_div, tan_eval, tan_eval_rational};

#[cold]
fn cot_accurate(
    x: f64,
    k: u64,
    argument_reduction: &mut LargeArgumentReduction,
    den_dd: DoubleDouble,
) -> f64 {
    let x_e = (x.to_bits() >> 52) & 0x7ff;
    const E_BIAS: u64 = (1u64 << (11 - 1u64)) - 1u64;
    let u_f128 = if x_e < E_BIAS + 16 {
        range_reduction_small_f128(x)
    } else {
        argument_reduction.accurate()
    };

    let tan_u = tan_eval_rational(&u_f128);

    // cos(k * pi/128) = sin(k * pi/128 + pi/2) = sin((k + 64) * pi/128).
    let sin_k_f128 = get_sin_k_rational(k);
    let cos_k_f128 = get_sin_k_rational(k.wrapping_add(64));
    let msin_k_f128 = get_sin_k_rational(k.wrapping_add(128));

    // num_f128 = sin(k*pi/128) + tan(y) * cos(k*pi/128)
    let num_f128 = sin_k_f128 + (cos_k_f128 * tan_u);
    // den_f128 = cos(k*pi/128) - tan(y) * sin(k*pi/128)
    let den_f128 = cos_k_f128 + (msin_k_f128 * tan_u);

    // tan(x) = (sin(k*pi/128) + tan(y) * cos(k*pi/128)) /
    //          / (cos(k*pi/128) - tan(y) * sin(k*pi/128))

    // num and den is shuffled for cot
    let result = newton_raphson_div(&den_f128, &num_f128, 1.0 / den_dd.hi);
    result.fast_as_f64()
}

/// Cotangent in double precision
///
/// ULP 0.5
pub fn f_cot(x: f64) -> f64 {
    let x_e = (x.to_bits() >> 52) & 0x7ff;
    const E_BIAS: u64 = (1u64 << (11 - 1u64)) - 1u64;

    let y: DoubleDouble;
    let k;

    let mut argument_reduction = LargeArgumentReduction::default();

    // |x| < 2^16
    if x_e < E_BIAS + 16 {
        // |x| < 2^-7
        if x_e < E_BIAS - 7 {
            // |x| < 2^-27, |cot(x) - x| < ulp(x)/2.
            if x_e < E_BIAS - 27 {
                // Signed zeros.
                if x == 0.0 {
                    return if x.is_sign_negative() {
                        f64::NEG_INFINITY
                    } else {
                        f64::INFINITY
                    };
                }

                if x_e < E_BIAS - 53 {
                    return 1. / x;
                }

                let dx = DoubleDouble::from_quick_recip(x);
                // taylor order 3
                return DoubleDouble::f64_mul_f64_add(x, f64::from_bits(0xbfd5555555555555), dx)
                    .to_f64();
            }
            // No range reduction needed.
            k = 0;
            y = DoubleDouble::new(0., x);
        } else {
            // Small range reduction.
            (y, k) = range_reduction_small(x);
        }
    } else {
        // Inf or NaN
        if x_e > 2 * E_BIAS {
            if x.is_nan() {
                return f64::NAN;
            }
            // tan(+-Inf) = NaN
            return x + f64::NAN;
        }

        // Large range reduction.
        (k, y) = argument_reduction.reduce(x);
    }

    let (tan_y, err) = tan_eval(y);

    // Fast look up version, but needs 256-entry table.
    // cos(k * pi/128) = sin(k * pi/128 + pi/2) = sin((k + 64) * pi/128).
    let sk = SIN_K_PI_OVER_128[(k.wrapping_add(128) & 255) as usize];
    let ck = SIN_K_PI_OVER_128[((k.wrapping_add(64)) & 255) as usize];

    let msin_k = DoubleDouble::from_bit_pair(sk);
    let cos_k = DoubleDouble::from_bit_pair(ck);

    let cos_k_tan_y = DoubleDouble::quick_mult(tan_y, cos_k);
    let msin_k_tan_y = DoubleDouble::quick_mult(tan_y, msin_k);

    // num_dd = sin(k*pi/128) + tan(y) * cos(k*pi/128)
    let mut num_dd = DoubleDouble::from_full_exact_add(cos_k_tan_y.hi, -msin_k.hi);
    // den_dd = cos(k*pi/128) - tan(y) * sin(k*pi/128)
    let mut den_dd = DoubleDouble::from_full_exact_add(msin_k_tan_y.hi, cos_k.hi);
    num_dd.lo += cos_k_tan_y.lo - msin_k.lo;
    den_dd.lo += msin_k_tan_y.lo + cos_k.lo;

    // num and den is shuffled for cot
    let tan_x = DoubleDouble::div(den_dd, num_dd);

    // Simple error bound: |1 / den_dd| < 2^(1 + floor(-log2(den_dd)))).
    let den_inv = ((E_BIAS + 1) << (52 + 1)) - (den_dd.hi.to_bits() & EXP_MASK);
    // For tan_x = (num_dd + err) / (den_dd + err), the error is bounded by:
    //   | tan_x - num_dd / den_dd |  <= err * ( 1 + | tan_x * den_dd | ).
    let tan_err = err * f_fmla(f64::from_bits(den_inv), tan_x.hi.abs(), 1.0);

    let err_higher = tan_x.lo + tan_err;
    let err_lower = tan_x.lo - tan_err;

    let tan_upper = tan_x.hi + err_higher;
    let tan_lower = tan_x.hi + err_lower;

    // Ziv_s rounding test.
    if tan_upper == tan_lower {
        return tan_upper;
    }

    cot_accurate(x, k, &mut argument_reduction, num_dd)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cot_test() {
        assert_eq!(f_cot(2.3006805685393681E-308), 4.346539948546049e307);
        assert_eq!(f_cot(5070552515158872000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000.), 25.068466719883585);
        assert_eq!(f_cot(4.9406564584124654E-324), f64::INFINITY);
        assert_eq!(f_cot(0.0), f64::INFINITY);
        assert_eq!(f_cot(1.0), 0.6420926159343308);
        assert_eq!(f_cot(-0.5), -1.830487721712452);
        assert_eq!(f_cot(12.0), -1.5726734063976893);
        assert_eq!(f_cot(-12.0), 1.5726734063976893);
        assert!(f_cot(f64::INFINITY).is_nan());
        assert!(f_cot(f64::NEG_INFINITY).is_nan());
        assert!(f_cot(f64::NAN).is_nan());
    }
}
