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
use crate::common::*;
use crate::double_double::DoubleDouble;
use crate::dyadic_float::{DyadicFloat128, DyadicSign};
use crate::logs::log1p_fast_dd;
use crate::pow::{is_integer, is_odd_integer};
use crate::pow_exec::pow_expm1_1;

/// Computes (1+x)^y - 1
///
/// max found ULP 0.56
pub fn f_compound_m1(x: f64, y: f64) -> f64 {
    /*
       Rules from IEEE 754-2019 for compound (x, n) with n integer:
           (a) compound (x, 0) is 1 for x >= -1 or quiet NaN
           (b) compound (-1, n) is +Inf and signals the divideByZero exception for n < 0
           (c) compound (-1, n) is +0 for n > 0
           (d) compound (+/-0, n) is 1
           (e) compound (+Inf, n) is +Inf for n > 0
           (f) compound (+Inf, n) is +0 for n < 0
           (g) compound (x, n) is qNaN and signals the invalid exception for x < -1
           (h) compound (qNaN, n) is qNaN for n <> 0.
    */

    let x_sign = x.is_sign_negative();
    let y_sign = y.is_sign_negative();

    let x_abs = x.to_bits() & 0x7fff_ffff_ffff_ffff;
    let y_abs = y.to_bits() & 0x7fff_ffff_ffff_ffff;

    const MANTISSA_MASK: u64 = (1u64 << 52) - 1;

    let y_mant = y.to_bits() & MANTISSA_MASK;
    let x_u = x.to_bits();
    let x_a = x_abs;
    let y_a = y_abs;

    // If x or y is signaling NaN
    if x.is_nan() || y.is_nan() {
        return f64::NAN;
    }

    let mut s = 1.0;

    // The double precision number that is closest to 1 is (1 - 2^-53), which has
    //   log2(1 - 2^-53) ~ -1.715...p-53.
    // So if |y| > |1075 / log2(1 - 2^-53)|, and x is finite:
    //   |y * log2(x)| = 0 or > 1075.
    // Hence, x^y will either overflow or underflow if x is not zero.
    if y_mant == 0
        || y_a > 0x43d7_4910_d52d_3052
        || x_u == 1f64.to_bits()
        || x_u >= f64::INFINITY.to_bits()
        || x_u < f64::MIN.to_bits()
    {
        // Exceptional exponents.
        if y == 0.0 {
            return 0.0;
        }

        // (h) compound(qNaN, n) is qNaN for n ≠ 0
        if x.is_nan() {
            if y != 0. {
                return x;
            } // propagate qNaN
            return 0.0;
        }

        // (d) compound(±0, n) is 1
        if x == 0.0 {
            return 0.0;
        }

        // (e, f) compound(+Inf, n)
        if x.is_infinite() && x > 0.0 {
            return if y > 0. { x } else { -1.0 };
        }

        // (g) compound(x, n) is qNaN and signals invalid for x < -1
        if x < -1.0 {
            // Optional: raise invalid explicitly
            return f64::NAN;
        }

        // (b, c) compound(-1, n)
        if x == -1.0 {
            return if y < 0. { f64::INFINITY } else { -1.0 };
        }

        match y_a {
            // 0x3fe0_0000_0000_0000 => {
            //     if x == 0.0 {
            //         return 0.0;
            //     }
            //     let z = Dekker::from_full_exact_add(x, 1.0).sqrt();
            //     if y_sign {
            //         const M_ONES: DyadicFloat128 = DyadicFloat128 {
            //             sign: DyadicSign::Neg,
            //             exponent: -127,
            //             mantissa: 0x80000000_00000000_00000000_00000000_u128,
            //         };
            //         let z = DyadicFloat128::new_from_f64(z.to_f64());
            //         (z.reciprocal() + M_ONES).fast_as_f64()
            //     } else {
            //         const M_ONES: DyadicFloat128 = DyadicFloat128 {
            //             sign: DyadicSign::Neg,
            //             exponent: -127,
            //             mantissa: 0x80000000_00000000_00000000_00000000_u128,
            //         };
            //         let z = DyadicFloat128::new_from_f64(z.to_f64());
            //         (z + M_ONES).fast_as_f64()
            //     };
            // }
            0x3ff0_0000_0000_0000 => {
                return if y_sign {
                    let z = DyadicFloat128::new_from_f64(x);
                    const ONES: DyadicFloat128 = DyadicFloat128 {
                        sign: DyadicSign::Pos,
                        exponent: -127,
                        mantissa: 0x80000000_00000000_00000000_00000000_u128,
                    };
                    const M_ONES: DyadicFloat128 = DyadicFloat128 {
                        sign: DyadicSign::Neg,
                        exponent: -127,
                        mantissa: 0x80000000_00000000_00000000_00000000_u128,
                    };
                    let p = (z + ONES).reciprocal() + M_ONES;
                    p.fast_as_f64()
                } else {
                    x
                };
            }
            0x4000_0000_0000_0000 => {
                const ONES: DyadicFloat128 = DyadicFloat128 {
                    sign: DyadicSign::Pos,
                    exponent: -127,
                    mantissa: 0x80000000_00000000_00000000_00000000_u128,
                };
                let z0 = DyadicFloat128::new_from_f64(x) + ONES;
                let z = z0 * z0;
                const M_ONES: DyadicFloat128 = DyadicFloat128 {
                    sign: DyadicSign::Neg,
                    exponent: -127,
                    mantissa: 0x80000000_00000000_00000000_00000000_u128,
                };
                return if y_sign {
                    (z.reciprocal() + M_ONES).fast_as_f64()
                } else {
                    f64::copysign((z + M_ONES).fast_as_f64(), x)
                };
            }
            _ => {}
        }

        // |y| > |1075 / log2(1 - 2^-53)|.
        if y_a >= 0x7ff0_0000_0000_0000 {
            // y is inf or nan
            if y_mant != 0 {
                // y is NaN
                // pow(1, NaN) = 1
                // pow(x, NaN) = NaN
                return if x_u == 1f64.to_bits() { 1.0 } else { y };
            }

            // Now y is +-Inf
            if f64::from_bits(x_abs).is_nan() {
                // pow(NaN, +-Inf) = NaN
                return x;
            }

            if x_a == 0x3ff0_0000_0000_0000 {
                // pow(+-1, +-Inf) = 1.0
                return 0.0;
            }

            if x == 0.0 && y_sign {
                // pow(+-0, -Inf) = +inf and raise FE_DIVBYZERO
                return f64::INFINITY;
            }
            // pow (|x| < 1, -inf) = +inf
            // pow (|x| < 1, +inf) = 0.0
            // pow (|x| > 1, -inf) = 0.0
            // pow (|x| > 1, +inf) = +inf
            return if (x_a < 1f64.to_bits()) == y_sign {
                f64::INFINITY
            } else {
                -1.0
            };
        }

        // y is finite and non-zero.

        if x_u == 1f64.to_bits() {
            // pow(1, y) = 1
            return 0.0;
        }

        if x == 0.0 {
            let out_is_neg = x_sign && is_odd_integer(y);
            if y_sign {
                // pow(0, negative number) = inf
                return if out_is_neg {
                    f64::NEG_INFINITY
                } else {
                    f64::INFINITY
                };
            }
            // pow(0, positive number) = 0
            return -1.0;
        }

        if x_a == f64::INFINITY.to_bits() {
            let out_is_neg = x_sign && is_odd_integer(y);
            if y_sign {
                return if out_is_neg { -1.0 } else { 1.0 };
            }
            return if out_is_neg {
                f64::NEG_INFINITY
            } else {
                f64::INFINITY
            };
        }

        if x_a > f64::INFINITY.to_bits() {
            // x is NaN.
            // pow (aNaN, 0) is already taken care above.
            return x;
        }

        // x is finite and negative, and y is a finite integer.
        if x_sign {
            if is_integer(y) {
                if is_odd_integer(y) {
                    // sign = -1.0;
                    static CS: [f64; 2] = [1.0, -1.0];

                    // set sign to 1 for y even, to -1 for y odd
                    let y_parity = if (y.abs()) >= f64::from_bits(0x4340000000000000) {
                        0usize
                    } else {
                        (y as i64 & 0x1) as usize
                    };
                    s = CS[y_parity];
                }
            } else {
                // pow( negative, non-integer ) = NaN
                return f64::NAN;
            }
        }

        // y is finite and non-zero.

        if x_u == 1f64.to_bits() {
            // pow(1, y) = 1
            return 0.0;
        }

        if x == 0.0 {
            let out_is_neg = x_sign && is_odd_integer(y);
            if y_sign {
                // pow(0, negative number) = inf
                return if out_is_neg {
                    f64::NEG_INFINITY
                } else {
                    f64::INFINITY
                };
            }
            // pow(0, positive number) = 0
            return if out_is_neg { -0.0 } else { 0.0 };
        }

        if x_a == f64::INFINITY.to_bits() {
            let out_is_neg = x_sign && is_odd_integer(y);
            if y_sign {
                return -1.;
            }
            return if out_is_neg {
                f64::NEG_INFINITY
            } else {
                f64::INFINITY
            };
        }

        if x_a > f64::INFINITY.to_bits() {
            // x is NaN.
            // pow (aNaN, 0) is already taken care above.
            return x;
        }
    }

    // evaluate (1+x)^y explicitly for integer y in [-1024,1024] range and |x|<2^64

    #[cfg(any(
        all(
            any(target_arch = "x86", target_arch = "x86_64"),
            target_feature = "fma"
        ),
        all(target_arch = "aarch64", target_feature = "neon")
    ))]
    let straight_path_precondition: bool = true;
    #[cfg(not(any(
        all(
            any(target_arch = "x86", target_arch = "x86_64"),
            target_feature = "fma"
        ),
        all(target_arch = "aarch64", target_feature = "neon")
    )))]
    let straight_path_precondition: bool = y.is_sign_positive();
    // this is correct only for positive exponent number without FMA,
    // otherwise reciprocal may overflow.
    if y.floor() == y
        && y_a <= 0x4059800000000000u64
        && x_a <= 0x4090000000000000u64
        && x_a > 0x3cc0_0000_0000_0000
        && straight_path_precondition
    {
        let mut s = DoubleDouble::from_full_exact_add(1.0, x);
        let mut iter_count = y.abs() as usize;

        // exponentiation by squaring: O(log(y)) complexity
        let mut acc = if iter_count % 2 != 0 {
            s
        } else {
            DoubleDouble::new(0., 1.)
        };

        while {
            iter_count >>= 1;
            iter_count
        } != 0
        {
            s = DoubleDouble::mult(s, s);
            if iter_count % 2 != 0 {
                acc = DoubleDouble::mult(acc, s);
            }
        }

        let mut dz = if y.is_sign_negative() {
            acc.recip()
        } else {
            acc
        };

        dz = DoubleDouble::full_add_f64(dz, -1.);
        let ub = dz.hi + f_fmla(f64::from_bits(0x3c40000000000000), -dz.hi, dz.lo); // 2^-59
        let lb = dz.hi + f_fmla(f64::from_bits(0x3c40000000000000), dz.hi, dz.lo); // 2^-59
        if ub == lb {
            return dz.to_f64();
        }
        return mul_fixed_power_hard(x, y);
    }

    // approximate log1p(x)
    let l = log1p_fast_dd(x);

    let ey = ((y.to_bits() >> 52) & 0x7ff) as i32;
    if ey < 0x36 || ey >= 0x7f5 {
        return 0.;
    }

    let r = DoubleDouble::quick_mult_f64(l, y);
    let res = pow_expm1_1(r, s);

    res.to_f64()
}

#[cold]
#[inline(never)]
fn mul_fixed_power_hard(x: f64, y: f64) -> f64 {
    const ONE: DyadicFloat128 = DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0x80000000_00000000_00000000_00000000_u128,
    };
    const M_ONE: DyadicFloat128 = DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -127,
        mantissa: 0x80000000_00000000_00000000_00000000_u128,
    };
    let mut s = DyadicFloat128::new_from_f64(x) + ONE;
    let mut iter_count = y.abs() as usize;

    // exponentiation by squaring: O(log(y)) complexity
    let mut acc = if iter_count % 2 != 0 { s } else { ONE };

    while {
        iter_count >>= 1;
        iter_count
    } != 0
    {
        s = s * s;
        if iter_count % 2 != 0 {
            acc = acc * s;
        }
    }

    if y.is_sign_negative() {
        (acc.reciprocal() + M_ONE).fast_as_f64()
    } else {
        (acc + M_ONE).fast_as_f64()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compound_exotic() {
        assert_eq!(
            f_compound_m1(0.000152587890625, -8.484374999999998),
            -0.0012936766014690006
        );
        assert_eq!(
            f_compound_m1(
                0.00000000000000799360578102344,
                -0.000000000000000000000001654361225106131
            ),
            -0.000000000000000000000000000000000000013224311452909338
        );
        assert_eq!(
            f_compound_m1( 4.517647064592699, 0.0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000055329046628180653),
            0.000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000009449932890153435
        );
        assert_eq!(f_compound_m1(
    11944758478933760000000000000000000000000000000000000000000000000000000000000000000000000000000000000000.,
                -1242262631503757300000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000.,
            ), -1.);
    }

    #[test]
    fn test_compound_m1() {
        assert_eq!(
            f_compound_m1(0.0000000000000009991998751296936, -4.),
            -0.000000000000003996799500518764
        );
        assert_eq!(f_compound_m1(-0.003173828125, 25.), -0.0763960132649781);
        assert_eq!(f_compound_m1(3., 2.8927001953125), 54.154259038961406);
        assert_eq!(
            f_compound_m1(-0.43750000000000044, 19.),
            -0.9999821216263793
        );
        assert_eq!(
            f_compound_m1(127712., -2.0000000000143525),
            -0.9999999999386903
        );
        assert_eq!(
            f_compound_m1(-0.11718749767214207, 2893226081485815000000000000000.),
            -1.
        );
        assert_eq!(
            f_compound_m1(2418441935074801400000000., 512.),
            f64::INFINITY
        );
        assert_eq!(
            f_compound_m1(32.50198364245834, 128000.00000000093),
            f64::INFINITY
        );
        assert_eq!(
            f_compound_m1(1.584716796877785, 0.0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000004168916810703412),
            0.0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000003958869879428553
        );
        assert_eq!(
            f_compound_m1(
                -0.000000000000000000000000000000001997076793037533,
                366577337071337140000000000000000f64
            ),
            -0.5190938261758579
        );
        assert_eq!(f_compound_m1(2.1075630259863374, 0.5), 00.7628281328553664);
        assert_eq!(f_compound_m1(2.1078916412661783, 0.5), 0.7629213372315222);
        assert_eq!(f_compound_m1(3.0000000000001115, -0.5), -0.500000000000007);
        assert_eq!(
            f_compound_m1(0.0004873839215895903, 3.),
            0.0014628645098045245
        );

        assert_eq!(f_compound_m1(-0.483765364602732, 3.), -0.862424399516842);
        assert_eq!(f_compound_m1(3.0000001192092896, -2.), -0.9375000037252902);
        assert_eq!(f_compound_m1(29.38323424607434, -1.), -0.9670871115332561);

        assert_eq!(f_compound_m1(-0.4375, 4.), -0.8998870849609375);
        assert_eq!(
            f_compound_m1(-0.0039033182037826464, 3.),
            -0.011664306402886494
        );
        assert_eq!(
            f_compound_m1(0.000000000000000000000000000000000000007715336350455947,
                          -262034087537726030000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000.),
            -1.,
        );
        assert_eq!(f_compound_m1(10.000000059604645, 10.), 25937426005.44638);
        assert_eq!(f_compound_m1(10., -308.25471555814863), -1.0);
        assert_eq!(
            f_compound_m1(5.4172231599824623E-312, 9.4591068440831498E+164),
            5.124209266851586e-147
        );
        assert_eq!(
            f_compound_m1(5.8776567263633397E-39, 3.4223548116804511E-310),
            0.0
        );
        assert_eq!(
            f_compound_m1(5.8639503496997932E-148, -7.1936801558778956E+305),
            0.0
        );
        assert_eq!(
            f_compound_m1(0.9908447265624999,
                          -19032028850336152000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000.),
            -1.
        );
        assert_eq!(
            f_compound_m1(0.00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000006952247559980936,
                          5069789834563405000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000.),
            3.524643400695958e-163
        );
        assert_eq!(
            f_compound_m1(1.000000000000341,
                          -69261261804788370000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000.),
            -1.
        );
        assert_eq!(
            f_compound_m1(
                0.0000000000000001053438024827798,
                0.0000000000000001053438024827798
            ),
            0.000000000000000000000000000000011097316721530923
        );
        assert_eq!(
            f_compound_m1(
                0.00000000000000010755285551056508,
                0.00000000000000010755285551056508
            ),
            0.00000000000000000000000000000001156761672847649
        );

        assert_eq!(f_compound_m1(2.4324324, 1.4324324), 4.850778380908823);
        assert_eq!(f_compound_m1(2., 5.), 242.);
        assert_eq!(f_compound_m1(0.4324324, 126.4324324), 5.40545942023447e19);
        assert!(f_compound_m1(-0.4324324, 126.4324324).is_nan());
        assert_eq!(f_compound_m1(0.0, 0.0), 0.0);
        assert_eq!(f_compound_m1(0.0, -1. / 2.), 0.0);
        assert_eq!(f_compound_m1(-1., -1. / 2.), f64::INFINITY);
        assert_eq!(f_compound_m1(f64::INFINITY, -1. / 2.), -1.0);
        assert_eq!(f_compound_m1(f64::INFINITY, 1. / 2.), f64::INFINITY);
        assert_eq!(f_compound_m1(46.3828125, 46.3828125), 5.248159634773675e77);
    }
}
