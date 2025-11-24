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
use crate::common::f_fmla;
use crate::double_double::DoubleDouble;
use crate::dyadic_float::{DyadicFloat128, DyadicSign};
use crate::logs::{log1p_f64_dyadic, log1p_fast_dd};
use crate::pow::{is_integer, is_odd_integer};
use crate::pow_exec::{exp_dyadic, pow_exp_dd};
use crate::triple_double::TripleDouble;

/// Computes (1+x)^y
///
pub fn f_compound(x: f64, y: f64) -> f64 {
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

    let ax = x.to_bits() & 0x7fff_ffff_ffff_ffff;
    let ay = y.to_bits() & 0x7fff_ffff_ffff_ffff;

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
            return 1.0;
        }

        // (h) compound(qNaN, n) is qNaN for n ≠ 0
        if x.is_nan() {
            if y != 0. {
                return x;
            } // propagate qNaN
            return 1.0;
        }

        // (d) compound(±0, n) is 1
        if x == 0.0 {
            return 1.0;
        }

        // (e, f) compound(+Inf, n)
        if x.is_infinite() && x > 0.0 {
            return if y > 0. { x } else { 0.0 };
        }

        // (g) compound(x, n) is qNaN and signals invalid for x < -1
        if x < -1.0 {
            // Optional: raise invalid explicitly
            return f64::NAN;
        }

        // (b, c) compound(-1, n)
        if x == -1.0 {
            return if y < 0. { f64::INFINITY } else { 0.0 };
        }

        match y_a {
            0x3fe0_0000_0000_0000 => {
                // TODO: speed up x^(-1/2) with rsqrt(x) when available.
                if x == 0.0 {
                    return 1.0;
                }
                let z = DoubleDouble::from_full_exact_add(x, 1.0).sqrt();
                return if y_sign {
                    z.recip().to_f64()
                } else {
                    z.to_f64()
                };
            }
            0x3ff0_0000_0000_0000 => {
                return if y_sign {
                    const ONES: DyadicFloat128 = DyadicFloat128 {
                        sign: DyadicSign::Pos,
                        exponent: -127,
                        mantissa: 0x80000000_00000000_00000000_00000000_u128,
                    };
                    let z = DyadicFloat128::new_from_f64(x) + ONES;
                    z.reciprocal().fast_as_f64()
                } else {
                    DoubleDouble::from_full_exact_add(x, 1.0).to_f64()
                };
            }
            0x4000_0000_0000_0000 => {
                let z0 = DoubleDouble::from_full_exact_add(x, 1.0);
                let z = DoubleDouble::quick_mult(z0, z0);
                return if y_sign {
                    z.recip().to_f64()
                } else {
                    f64::copysign(z.to_f64(), x)
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
                0.0
            };
        }

        // y is finite and non-zero.

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
                return if out_is_neg { -0.0 } else { 0.0 };
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
            // compound(1, y) = 1
            return 2.0;
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
                return if out_is_neg { -0.0 } else { 0.0 };
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

        let min_abs = f64::min(f64::from_bits(ax), f64::from_bits(ay)).to_bits();
        let max_abs = f64::max(f64::from_bits(ax), f64::from_bits(ay)).to_bits();
        let min_exp = min_abs.wrapping_shr(52);
        let max_exp = max_abs.wrapping_shr(52);

        if max_exp > 0x7ffu64 - 128u64 || min_exp < 128u64 {
            let scale_up = min_exp < 128u64;
            let scale_down = max_exp > 0x7ffu64 - 128u64;
            // At least one input is denormal, multiply both numerator and denominator
            // then will go with hard path
            if scale_up || scale_down {
                return compound_accurate(x, y, s);
            }
        }
    }

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
    // y is integer and in [-102;102] and |x|<2^10
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

        let dz = if y.is_sign_negative() {
            acc.recip()
        } else {
            acc
        };
        let ub = dz.hi + f_fmla(f64::from_bits(0x3c40000000000000), -dz.hi, dz.lo); // 2^-59
        let lb = dz.hi + f_fmla(f64::from_bits(0x3c40000000000000), dz.hi, dz.lo); // 2^-59
        if ub == lb {
            return dz.to_f64();
        }
        return mul_fixed_power_hard(x, y);
    }

    let l = log1p_fast_dd(x);
    let ey = ((y.to_bits() >> 52) & 0x7ff) as i32;
    if ey < 0x36 || ey >= 0x7f5 {
        return compound_accurate(x, y, s);
    }

    let r = DoubleDouble::quick_mult_f64(l, y);
    let res = pow_exp_dd(r, s);
    let res_min = res.hi + f_fmla(f64::from_bits(0x3bf0000000000000), -res.hi, res.lo);
    let res_max = res.hi + f_fmla(f64::from_bits(0x3bf0000000000000), res.hi, res.lo);
    if res_min == res_max {
        return res_max;
    }

    compound_accurate(x, y, s)
}

#[cold]
fn compound_accurate(x: f64, y: f64, s: f64) -> f64 {
    /* the idea of returning res_max instead of res_min is due to Laurent
    Théry: it is better in case of underflow since res_max = +0 always. */

    let f_y = DyadicFloat128::new_from_f64(y);

    let r = log1p_f64_dyadic(x) * f_y;
    let mut result = exp_dyadic(r);

    // 2^R.ex <= R < 2^(R.ex+1)

    /* case R < 2^-1075: underflow case */
    if result.exponent < -1075 {
        return 0.5 * (s * f64::from_bits(0x0000000000000001));
    }
    if result.exponent >= 1025 {
        return 1.0;
    }

    result.sign = if s == -1.0 {
        DyadicSign::Neg
    } else {
        DyadicSign::Pos
    };

    result.fast_as_f64()
}

#[cold]
#[inline(never)]
fn mul_fixed_power_hard(x: f64, y: f64) -> f64 {
    let mut s = TripleDouble::from_full_exact_add(1.0, x);
    let mut iter_count = y.abs() as usize;

    // exponentiation by squaring: O(log(y)) complexity
    let mut acc = if iter_count % 2 != 0 {
        s
    } else {
        TripleDouble::new(0., 0., 1.)
    };

    while {
        iter_count >>= 1;
        iter_count
    } != 0
    {
        s = TripleDouble::quick_mult(s, s);
        if iter_count % 2 != 0 {
            acc = TripleDouble::quick_mult(acc, s);
        }
    }

    if y.is_sign_negative() {
        acc.recip().to_f64()
    } else {
        acc.to_f64()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_compound() {
        assert_eq!(f_compound(4831835136., -13.),0.0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000012780345669344118 );
        assert_eq!(
            f_compound(11468322278342656., 2.9995136260713475),
            1481455956234813000000000000000000000000000000000.
        );
        assert_eq!(f_compound(0.9999999999999999, 3.), 7.999999999999999);
        assert_eq!(
            f_compound(1.0039215087890625, 10.000000000349134),
            1044.2562119607103
        );
        assert_eq!(f_compound(10., 18.0), 5559917313492231000.0);
        assert_eq!(
            f_compound(131071.65137729312, 2.000001423060894),
            17180328027.532265
        );
        assert_eq!(f_compound(2., 5.), 243.);
        assert_eq!(f_compound(126.4324324, 126.4324324), 1.4985383310514043e266);
        assert_eq!(f_compound(0.4324324, 126.4324324), 5.40545942023447e19);
        assert!(f_compound(-0.4324324, 126.4324324).is_nan());
        assert_eq!(f_compound(0.0, 0.0), 1.0);
        assert_eq!(f_compound(0.0, -1. / 2.), 1.0);
        assert_eq!(f_compound(-1., -1. / 2.), f64::INFINITY);
        assert_eq!(f_compound(f64::INFINITY, -1. / 2.), 0.0);
        assert_eq!(f_compound(f64::INFINITY, 1. / 2.), f64::INFINITY);
        assert_eq!(f_compound(46.3828125, 46.3828125), 5.248159634773675e77);
    }

    #[test]
    fn test_compound_exotic_cases() {
        assert_eq!(f_compound(0.9999999850987819, -1.), 0.5000000037253046);
        assert_eq!(
            f_compound(22427285907987670000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000.,
                       -1.),
            0.00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000004458854290718438
        );
        assert_eq!(f_compound(0.786438105629145,  607.999512419221),
                   1616461095392737200000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000.);
        assert_eq!(f_compound( 1.0000002381857613, 960.8218657970428),
                   17228671476562465000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000.);
        assert_eq!(f_compound(1., 1.0000000000000284), 2.);
        assert_eq!(f_compound(1., f64::INFINITY), f64::INFINITY);
        assert_eq!(
            f_compound(10.000000000000007, -8.),
            0.00000000466507380209731
        );
    }
}
