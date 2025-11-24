/*
 * // Copyright (c) Radzivon Bartoshyk 6/2025. All rights reserved.
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
use crate::common::{dyad_fmla, f_fmla};
use crate::double_double::DoubleDouble;
use crate::dyadic_float::{DyadicFloat128, DyadicSign};
use crate::polyeval::f_polyeval9;
use crate::sin::{get_sin_k_rational, range_reduction_small};
use crate::sin_table::SIN_K_PI_OVER_128;
use crate::sincos_dyadic::range_reduction_small_f128;
use crate::sincos_reduce::LargeArgumentReduction;

#[inline]
pub(crate) fn tan_eval(u: DoubleDouble) -> (DoubleDouble, f64) {
    // Evaluate tan(y) = tan(x - k * (pi/128))
    // We use the degree-9 Taylor approximation:
    //   tan(y) ~ P(y) = y + y^3/3 + 2*y^5/15 + 17*y^7/315 + 62*y^9/2835
    // Then the error is bounded by:
    //   |tan(y) - P(y)| < 2^-6 * |y|^11 < 2^-6 * 2^-66 = 2^-72.
    // For y ~ u_hi + u_lo, fully expanding the polynomial and drop any terms
    // < ulp(u_hi^3) gives us:
    //   P(y) = y + y^3/3 + 2*y^5/15 + 17*y^7/315 + 62*y^9/2835 = ...
    // ~ u_hi + u_hi^3 * (1/3 + u_hi^2 * (2/15 + u_hi^2 * (17/315 +
    //                                                     + u_hi^2 * 62/2835))) +
    //        + u_lo (1 + u_hi^2 * (1 + u_hi^2 * 2/3))
    let u_hi_sq = u.hi * u.hi; // Error < ulp(u_hi^2) < 2^(-6 - 52) = 2^-58.
    // p1 ~ 17/315 + u_hi^2 62 / 2835.
    let p1 = f_fmla(
        u_hi_sq,
        f64::from_bits(0x3f9664f4882c10fa),
        f64::from_bits(0x3faba1ba1ba1ba1c),
    );
    // p2 ~ 1/3 + u_hi^2 2 / 15.
    let p2 = f_fmla(
        u_hi_sq,
        f64::from_bits(0x3fc1111111111111),
        f64::from_bits(0x3fd5555555555555),
    );
    // q1 ~ 1 + u_hi^2 * 2/3.
    let q1 = f_fmla(u_hi_sq, f64::from_bits(0x3fe5555555555555), 1.0);
    let u_hi_3 = u_hi_sq * u.hi;
    let u_hi_4 = u_hi_sq * u_hi_sq;
    // p3 ~ 1/3 + u_hi^2 * (2/15 + u_hi^2 * (17/315 + u_hi^2 * 62/2835))
    let p3 = f_fmla(u_hi_4, p1, p2);
    // q2 ~ 1 + u_hi^2 * (1 + u_hi^2 * 2/3)
    let q2 = f_fmla(u_hi_sq, q1, 1.0);
    let tan_lo = f_fmla(u_hi_3, p3, u.lo * q2);
    // Overall, |tan(y) - (u_hi + tan_lo)| < ulp(u_hi^3) <= 2^-71.
    // And the relative errors is:
    // |(tan(y) - (u_hi + tan_lo)) / tan(y) | <= 2*ulp(u_hi^2) < 2^-64
    let err = f_fmla(
        u_hi_3.abs(),
        f64::from_bits(0x3cc0000000000000),
        f64::from_bits(0x3990000000000000),
    );
    (DoubleDouble::from_exact_add(u.hi, tan_lo), err)
}

#[inline]
pub(crate) fn tan_eval_rational(u: &DyadicFloat128) -> DyadicFloat128 {
    let u_sq = u.quick_mul(u);

    // tan(x) ~ x + x^3/3 + x^5 * 2/15 + x^7 * 17/315 + x^9 * 62/2835 +
    //          + x^11 * 1382/155925 + x^13 * 21844/6081075 +
    //          + x^15 * 929569/638512875 + x^17 * 6404582/10854718875
    // Relative errors < 2^-127 for |u| < pi/256.
    const TAN_COEFFS: [DyadicFloat128; 9] = [
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -127,
            mantissa: 0x80000000_00000000_00000000_00000000_u128,
        }, // 1
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -129,
            mantissa: 0xaaaaaaaa_aaaaaaaa_aaaaaaaa_aaaaaaab_u128,
        }, // 1
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -130,
            mantissa: 0x88888888_88888888_88888888_88888889_u128,
        }, // 2/15
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -132,
            mantissa: 0xdd0dd0dd_0dd0dd0d_d0dd0dd0_dd0dd0dd_u128,
        }, // 17/315
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -133,
            mantissa: 0xb327a441_6087cf99_6b5dd24e_ec0b327a_u128,
        }, // 62/2835
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -134,
            mantissa: 0x91371aaf_3611e47a_da8e1cba_7d900eca_u128,
        }, // 1382/155925
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -136,
            mantissa: 0xeb69e870_abeefdaf_e606d2e4_d1e65fbc_u128,
        }, // 21844/6081075
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -137,
            mantissa: 0xbed1b229_5baf15b5_0ec9af45_a2619971_u128,
        }, // 929569/638512875
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -138,
            mantissa: 0x9aac1240_1b3a2291_1b2ac7e3_e4627d0a_u128,
        }, // 6404582/10854718875
    ];

    u.quick_mul(&f_polyeval9(
        u_sq,
        TAN_COEFFS[0],
        TAN_COEFFS[1],
        TAN_COEFFS[2],
        TAN_COEFFS[3],
        TAN_COEFFS[4],
        TAN_COEFFS[5],
        TAN_COEFFS[6],
        TAN_COEFFS[7],
        TAN_COEFFS[8],
    ))
}

// Calculation a / b = a * (1/b) for Float128.
// Using the initial approximation of q ~ (1/b), then apply 2 Newton-Raphson
// iterations, before multiplying by a.
#[inline]
pub(crate) fn newton_raphson_div(a: &DyadicFloat128, b: &DyadicFloat128, q: f64) -> DyadicFloat128 {
    let q0 = DyadicFloat128::new_from_f64(q);
    const TWO: DyadicFloat128 = DyadicFloat128::new_from_f64(2.0);
    let mut b = *b;
    b.sign = if b.sign == DyadicSign::Pos {
        DyadicSign::Neg
    } else {
        DyadicSign::Pos
    };
    let q1 = q0.quick_mul(&TWO.quick_add(&b.quick_mul(&q0)));
    let q2 = q1.quick_mul(&TWO.quick_add(&b.quick_mul(&q1)));
    a.quick_mul(&q2)
}

#[cold]
fn tan_accurate(
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
    // reused from DoubleDouble fputil::div in the fast pass.
    let result = newton_raphson_div(&num_f128, &den_f128, 1.0 / den_dd.hi);
    result.fast_as_f64()
}

/// Tangent in double precision
///
/// ULP 0.5
pub fn f_tan(x: f64) -> f64 {
    let x_e = (x.to_bits() >> 52) & 0x7ff;
    const E_BIAS: u64 = (1u64 << (11 - 1u64)) - 1u64;

    let y: DoubleDouble;
    let k;

    let mut argument_reduction = LargeArgumentReduction::default();

    // |x| < 2^16
    if x_e < E_BIAS + 16 {
        // |x| < 2^-7
        if x_e < E_BIAS - 7 {
            // |x| < 2^-27, |tan(x) - x| < ulp(x)/2.
            if x_e < E_BIAS - 27 {
                // Signed zeros.
                if x == 0.0 {
                    return x + x;
                }
                return dyad_fmla(x, f64::from_bits(0x3c90000000000000), x);
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

    let tan_x = DoubleDouble::div(num_dd, den_dd);

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

    tan_accurate(x, k, &mut argument_reduction, den_dd)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tan_test() {
        assert_eq!(f_tan(0.0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000007291122019556397),
            0.0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000007291122019556397);
        assert_eq!(f_tan(0.0), 0.0);
        assert_eq!(f_tan(1.0), 1.5574077246549023);
        assert_eq!(f_tan(-0.5), -0.5463024898437905);
    }
}
