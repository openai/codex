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
use crate::acospi::{INV_PI_DD, INV_PI_F128};
use crate::common::f_fmla;
use crate::double_double::DoubleDouble;
use crate::dyadic_float::DyadicFloat128;
use crate::tangent::atan2::{ATAN_I, atan_eval, atan2_hard};

/// If one of arguments is too huge or too small, extended precision is required for
/// case with big exponent difference
#[cold]
fn atan2pi_big_exp_difference_hard(
    num: f64,
    den: f64,
    x_sign: usize,
    y_sign: usize,
    recip: bool,
    final_sign: f64,
) -> f64 {
    const ZERO: DoubleDouble = DoubleDouble::new(0.0, 0.0);
    const MZERO: DoubleDouble = DoubleDouble::new(-0.0, -0.0);
    static CONST_ADJ_INV_PI: [[[DoubleDouble; 2]; 2]; 2] = [
        [
            [ZERO, DoubleDouble::new(0., -1. / 2.)],
            [MZERO, DoubleDouble::new(0., -1. / 2.)],
        ],
        [
            [DoubleDouble::new(0., -1.), DoubleDouble::new(0., 1. / 2.)],
            [DoubleDouble::new(0., -1.), DoubleDouble::new(0., 1. / 2.)],
        ],
    ];
    let const_term = CONST_ADJ_INV_PI[x_sign][y_sign][recip as usize];
    let scaled_div = DyadicFloat128::from_div_f64(num, den) * INV_PI_F128;
    let sign_f128 = DyadicFloat128::new_from_f64(final_sign);
    let p = DyadicFloat128::new_from_f64(const_term.hi * final_sign);
    let p1 = sign_f128 * (DyadicFloat128::new_from_f64(const_term.lo) + scaled_div);
    let r = p + p1;
    r.fast_as_f64()
}

/// Computes atan(x)/PI
///
/// Max found ULP 0.5
pub fn f_atan2pi(y: f64, x: f64) -> f64 {
    static IS_NEG: [f64; 2] = [1.0, -1.0];
    const ZERO: DoubleDouble = DoubleDouble::new(0.0, 0.0);
    const MZERO: DoubleDouble = DoubleDouble::new(-0.0, -0.0);
    const PI: DoubleDouble = DoubleDouble::new(
        f64::from_bits(0x3ca1a62633145c07),
        f64::from_bits(0x400921fb54442d18),
    );
    const MPI: DoubleDouble = DoubleDouble::new(
        f64::from_bits(0xbca1a62633145c07),
        f64::from_bits(0xc00921fb54442d18),
    );
    const PI_OVER_2: DoubleDouble = DoubleDouble::new(
        f64::from_bits(0x3c91a62633145c07),
        f64::from_bits(0x3ff921fb54442d18),
    );
    const MPI_OVER_2: DoubleDouble = DoubleDouble::new(
        f64::from_bits(0xbc91a62633145c07),
        f64::from_bits(0xbff921fb54442d18),
    );
    const PI_OVER_4: DoubleDouble = DoubleDouble::new(
        f64::from_bits(0x3c81a62633145c07),
        f64::from_bits(0x3fe921fb54442d18),
    );
    const THREE_PI_OVER_4: DoubleDouble = DoubleDouble::new(
        f64::from_bits(0x3c9a79394c9e8a0a),
        f64::from_bits(0x4002d97c7f3321d2),
    );

    // Adjustment for constant term:
    //   CONST_ADJ[x_sign][y_sign][recip]
    static CONST_ADJ: [[[DoubleDouble; 2]; 2]; 2] = [
        [[ZERO, MPI_OVER_2], [MZERO, MPI_OVER_2]],
        [[MPI, PI_OVER_2], [MPI, PI_OVER_2]],
    ];

    let x_sign = x.is_sign_negative() as usize;
    let y_sign = y.is_sign_negative() as usize;
    let x_bits = x.to_bits() & 0x7fff_ffff_ffff_ffff;
    let y_bits = y.to_bits() & 0x7fff_ffff_ffff_ffff;
    let x_abs = x_bits;
    let y_abs = y_bits;
    let recip = x_abs < y_abs;
    let mut min_abs = if recip { x_abs } else { y_abs };
    let mut max_abs = if !recip { x_abs } else { y_abs };
    let mut min_exp = min_abs.wrapping_shr(52);
    let mut max_exp = max_abs.wrapping_shr(52);

    let mut num = f64::from_bits(min_abs);
    let mut den = f64::from_bits(max_abs);

    // Check for exceptional cases, whether inputs are 0, inf, nan, or close to
    // overflow, or close to underflow.
    if max_exp > 0x7ffu64 - 128u64 || min_exp < 128u64 {
        if x.is_nan() || y.is_nan() {
            return f64::NAN;
        }
        let x_except = if x == 0.0 {
            0
        } else if x.is_infinite() {
            2
        } else {
            1
        };
        let y_except = if y == 0.0 {
            0
        } else if y.is_infinite() {
            2
        } else {
            1
        };

        // Exceptional cases:
        //   EXCEPT[y_except][x_except][x_is_neg]
        // with x_except & y_except:
        //   0: zero
        //   1: finite, non-zero
        //   2: infinity
        static EXCEPTS: [[[DoubleDouble; 2]; 3]; 3] = [
            [[ZERO, PI], [ZERO, PI], [ZERO, PI]],
            [[PI_OVER_2, PI_OVER_2], [ZERO, ZERO], [ZERO, PI]],
            [
                [PI_OVER_2, PI_OVER_2],
                [PI_OVER_2, PI_OVER_2],
                [PI_OVER_4, THREE_PI_OVER_4],
            ],
        ];

        if (x_except != 1) || (y_except != 1) {
            let mut r = EXCEPTS[y_except][x_except][x_sign];
            r = DoubleDouble::quick_mult(r, INV_PI_DD);
            return f_fmla(IS_NEG[y_sign], r.hi, IS_NEG[y_sign] * r.lo);
        }
        let scale_up = min_exp < 128u64;
        let scale_down = max_exp > 0x7ffu64 - 128u64;
        // At least one input is denormal, multiply both numerator and denominator
        // by some large enough power of 2 to normalize denormal inputs.
        if scale_up {
            num *= f64::from_bits(0x43f0000000000000);
            if !scale_down {
                den *= f64::from_bits(0x43f0000000000000);
            }
        } else if scale_down {
            den *= f64::from_bits(0x3bf0000000000000);
            if !scale_up {
                num *= f64::from_bits(0x3bf0000000000000);
            }
        }

        min_abs = num.to_bits();
        max_abs = den.to_bits();
        min_exp = min_abs.wrapping_shr(52);
        max_exp = max_abs.wrapping_shr(52);
    }

    let final_sign = IS_NEG[((x_sign != y_sign) != recip) as usize];
    let const_term = CONST_ADJ[x_sign][y_sign][recip as usize];
    let exp_diff = max_exp - min_exp;
    // We have the following bound for normalized n and d:
    //   2^(-exp_diff - 1) < n/d < 2^(-exp_diff + 1).
    if exp_diff > 54 {
        if max_exp >= 1075 || min_exp < 970 {
            return atan2pi_big_exp_difference_hard(num, den, x_sign, y_sign, recip, final_sign);
        }
        let z = DoubleDouble::from_exact_mult(final_sign, const_term.hi);
        let mut divided = DoubleDouble::from_exact_div(num, den);
        divided = DoubleDouble::f64_add(const_term.lo, divided);
        divided = DoubleDouble::quick_mult_f64(divided, final_sign);
        let r = DoubleDouble::add(z, divided);
        let p = DoubleDouble::quick_mult(INV_PI_DD, r);
        return p.to_f64();
    }

    let mut k = (64.0 * num / den).round();
    let idx = k as u64;
    // k = idx / 64
    k *= f64::from_bits(0x3f90000000000000);

    // Range reduction:
    // atan(n/d) - atan(k/64) = atan((n/d - k/64) / (1 + (n/d) * (k/64)))
    //                        = atan((n - d * k/64)) / (d + n * k/64))
    let num_k = DoubleDouble::from_exact_mult(num, k);
    let den_k = DoubleDouble::from_exact_mult(den, k);

    // num_dd = n - d * k
    let num_dd = DoubleDouble::from_exact_add(num - den_k.hi, -den_k.lo);
    // den_dd = d + n * k
    let mut den_dd = DoubleDouble::from_exact_add(den, num_k.hi);
    den_dd.lo += num_k.lo;

    // q = (n - d * k) / (d + n * k)
    let q = DoubleDouble::div(num_dd, den_dd);
    // p ~ atan(q)
    let p = atan_eval(q);

    let vl = ATAN_I[idx as usize];
    let vlo = DoubleDouble::from_bit_pair(vl);
    let mut r = DoubleDouble::add(const_term, DoubleDouble::add(vlo, p));

    r = DoubleDouble::quick_mult(r, INV_PI_DD);

    let err = f_fmla(
        p.hi,
        f64::from_bits(0x3bd0000000000000),
        f64::from_bits(0x3c00000000000000),
    );

    let ub = r.hi + (r.lo + err);
    let lb = r.hi + (r.lo - err);

    if ub == lb {
        r.hi *= final_sign;
        r.lo *= final_sign;

        return r.to_f64();
    }

    (atan2_hard(y, x) * INV_PI_F128).fast_as_f64()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_atan2pi() {
        assert_eq!(
            f_atan2pi(-0.000000000000010659658919444194, 2088960.4374061823),
            -0.0000000000000000000016242886924270424
        );
        assert_eq!(f_atan2pi(-3.9999999981625933, 0.000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000003577142133480227), -0.5);
        assert_eq!(f_atan2pi(0.0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000472842255026406,
            0.000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000008045886150098693
        ),1.8706499392673612e-162);
        assert_eq!(f_atan2pi(0.000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000002670088630208647,
                             2.0000019071157054
        ), 4.249573987697093e-307);
        assert_eq!(f_atan2pi(-5., 2.), -0.3788810584091566);
        assert_eq!(f_atan2pi(2., -5.), 0.8788810584091566);
    }
}
