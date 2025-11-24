/*
 * // Copyright (c) Radzivon Bartoshyk 4/2025. All rights reserved.
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
use crate::bits::{EXP_MASK, get_exponent_f64};
use crate::common::f_fmla;
use crate::double_double::DoubleDouble;
use crate::dyadic_float::{DyadicFloat128, DyadicSign};
use crate::exponents::exp;
use crate::logs::log_dyadic;
use crate::pow_exec::{exp_dyadic, pow_exp_1, pow_log_1};
use crate::triple_double::TripleDouble;
use crate::{f_exp2, f_exp10, log};

#[inline]
pub(crate) fn is_integer(n: f64) -> bool {
    n == n.round_ties_even()
}

#[inline]
pub(crate) fn is_odd_integer(x: f64) -> bool {
    let x_u = x.to_bits();
    let x_e = (x_u & EXP_MASK) >> 52;
    let lsb = (x_u | EXP_MASK).trailing_zeros();
    const E_BIAS: u64 = (1u64 << (11 - 1u64)) - 1u64;

    const UNIT_EXPONENT: u64 = E_BIAS + 52;
    x_e + lsb as u64 == UNIT_EXPONENT
}

#[cold]
fn pow_exp10_fallback(x: f64) -> f64 {
    f_exp10(x)
}

#[cold]
fn pow_exp2_fallback(x: f64) -> f64 {
    f_exp2(x)
}

#[cold]
#[inline(never)]
fn f_powi(x: f64, y: f64) -> f64 {
    let mut iter_count = y.abs() as usize;

    let mut s = DoubleDouble::new(0., x);

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
    f_powi_hard(x, y)
}

#[cold]
#[inline(never)]
fn f_powi_hard(x: f64, y: f64) -> f64 {
    let mut iter_count = y.abs() as usize;

    let mut s = TripleDouble::new(0., 0., x);

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

    let dz = if y.is_sign_negative() {
        acc.recip()
    } else {
        acc
    };
    dz.to_f64()
}

/// Power function
///
/// max found ULP 0.5
pub fn f_pow(x: f64, y: f64) -> f64 {
    let mut y = y;
    let x_sign = x.is_sign_negative();
    let y_sign = y.is_sign_negative();

    let x_abs = x.to_bits() & 0x7fff_ffff_ffff_ffff;
    let y_abs = y.to_bits() & 0x7fff_ffff_ffff_ffff;

    const MANTISSA_MASK: u64 = (1u64 << 52) - 1;

    let y_mant = y.to_bits() & MANTISSA_MASK;
    let x_u = x.to_bits();
    let x_a = x_abs;
    let y_a = y_abs;

    let mut x = x;

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
            return 1.0;
        }

        match y_a {
            0x3fe0_0000_0000_0000 => {
                if x == 0.0 || x_u == f64::NEG_INFINITY.to_bits() {
                    // pow(-0, 1/2) = +0
                    // pow(-inf, 1/2) = +inf
                    return if y_sign { 1.0 / (x * x) } else { x * x };
                }
                return if y_sign {
                    if x.is_infinite() {
                        return if x.is_sign_positive() { 0. } else { f64::NAN };
                    }
                    #[cfg(any(
                        all(
                            any(target_arch = "x86", target_arch = "x86_64"),
                            target_feature = "fma"
                        ),
                        all(target_arch = "aarch64", target_feature = "neon")
                    ))]
                    {
                        let r = x.sqrt() / x;
                        let rx = r * x;
                        let drx = f_fmla(r, x, -rx);
                        let h = f_fmla(r, rx, -1.0) + r * drx;
                        let dr = (r * 0.5) * h;
                        r - dr
                    }
                    #[cfg(not(any(
                        all(
                            any(target_arch = "x86", target_arch = "x86_64"),
                            target_feature = "fma"
                        ),
                        all(target_arch = "aarch64", target_feature = "neon")
                    )))]
                    {
                        let r = x.sqrt() / x;
                        let d2x = DoubleDouble::from_exact_mult(r, x);
                        let DoubleDouble { hi: h, lo: pr } = DoubleDouble::quick_mult_f64(d2x, r);
                        let DoubleDouble { hi: p, lo: q } =
                            DoubleDouble::from_full_exact_add(-1.0, h);
                        let h = DoubleDouble::from_exact_add(p, pr + q);
                        let dr = DoubleDouble::quick_mult_f64(h, r * 0.5);
                        r - dr.hi - dr.lo
                    }
                } else {
                    x.sqrt()
                };
            }
            0x3ff0_0000_0000_0000 => {
                return if y_sign { 1.0 / x } else { x };
            }
            0x4000_0000_0000_0000 => {
                let x_e = get_exponent_f64(x);
                if x_e > 511 {
                    return if y_sign { 0. } else { f64::INFINITY };
                }
                // not enough precision to make 0.5 ULP for subnormals
                if x_e.abs() < 70 {
                    let x_sqr = DoubleDouble::from_exact_mult(x, x);
                    return if y_sign {
                        let recip = x_sqr.recip();
                        recip.to_f64()
                    } else {
                        x_sqr.to_f64()
                    };
                }
            }
            _ => {}
        }

        // |y| > |1075 / log2(1 - 2^-53)|.
        if y_a > 0x43d7_4910_d52d_3052 {
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
                    return 1.0;
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
            // x^y will overflow / underflow in double precision.  Set y to a
            // large enough exponent but not too large, so that the computations
            // won't overflow in double precision.
            y = if y_sign {
                f64::from_bits(0xc630000000000000)
            } else {
                f64::from_bits(0x4630000000000000)
            };
        }

        // y is finite and non-zero.

        if x_u == 1f64.to_bits() {
            // pow(1, y) = 1
            return 1.0;
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
        } else if x == 2.0 {
            return pow_exp2_fallback(y);
        } else if x == 10.0 {
            return pow_exp10_fallback(y);
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

        let is_y_integer = is_integer(y);
        // y is integer and in [-102;102] and |x|<2^10

        // this is correct only for positive exponent number without FMA,
        // otherwise reciprocal may overflow.
        #[cfg(any(
            all(
                any(target_arch = "x86", target_arch = "x86_64"),
                target_feature = "fma"
            ),
            all(target_arch = "aarch64", target_feature = "neon")
        ))]
        if is_y_integer
            && y_a <= 0x4059800000000000u64
            && x_a <= 0x4090000000000000u64
            && x_a > 0x3cc0_0000_0000_0000
        {
            return f_powi(x, y);
        }
        #[cfg(not(any(
            all(
                any(target_arch = "x86", target_arch = "x86_64"),
                target_feature = "fma"
            ),
            all(target_arch = "aarch64", target_feature = "neon")
        )))]
        if is_y_integer
            && y_a <= 0x4059800000000000u64
            && x_a <= 0x4090000000000000u64
            && x_a > 0x3cc0_0000_0000_0000
            && y.is_sign_positive()
        {
            return f_powi(x, y);
        }

        if x_sign {
            if is_y_integer {
                x = -x;
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
            return 1.0;
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
    }

    // approximate log(x)
    let (mut l, cancel) = pow_log_1(x);

    /* We should avoid a spurious underflow/overflow in y*log(x).
    Underflow: for x<>1, the smallest absolute value of log(x) is obtained
    for x=1-2^-53, with |log(x)| ~ 2^-53. Thus to avoid a spurious underflow
    we require |y| >= 2^-969.
    Overflow: the largest absolute value of log(x) is obtained for x=2^-1074,
    with |log(x)| < 745. Thus to avoid a spurious overflow we require
    |y| < 2^1014. */
    let ey = ((y.to_bits() >> 52) & 0x7ff) as i32;
    if ey < 0x36 || ey >= 0x7f5 {
        l.lo = f64::NAN;
        l.hi = f64::NAN;
    }

    let r = DoubleDouble::quick_mult_f64(l, y);
    let res = pow_exp_1(r, s);
    static ERR: [u64; 2] = [0x3bf2700000000000, 0x3c55700000000000];
    let res_min = res.hi + f_fmla(f64::from_bits(ERR[cancel as usize]), -res.hi, res.lo);
    let res_max = res.hi + f_fmla(f64::from_bits(ERR[cancel as usize]), res.hi, res.lo);
    if res_min == res_max {
        return res_max;
    }
    pow_rational128(x, y, s)
}

#[cold]
fn pow_rational128(x: f64, y: f64, s: f64) -> f64 {
    /* the idea of returning res_max instead of res_min is due to Laurent
    Théry: it is better in case of underflow since res_max = +0 always. */

    let f_y = DyadicFloat128::new_from_f64(y);

    let r = log_dyadic(x) * f_y;
    let mut result = exp_dyadic(r);

    // 2^R.ex <= R < 2^(R.ex+1)

    // /* case R < 2^-1075: underflow case */
    // if result.exponent < -1075 {
    //     return 0.5 * (s * f64::from_bits(0x0000000000000001));
    // }

    result.sign = if s == -1.0 {
        DyadicSign::Neg
    } else {
        DyadicSign::Pos
    };

    result.fast_as_f64()
}

/// Pow for given value for const context.
/// This is simplified version just to make a good approximation on const context.
#[inline]
pub const fn pow(d: f64, n: f64) -> f64 {
    let value = d.abs();

    let r = n * log(value);
    let c = exp(r);
    if n == 0. {
        return 1.;
    }
    if d < 0.0 {
        let y = n as i32;
        if y % 2 == 0 { c } else { -c }
    } else {
        c
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn powf_test() {
        assert!(
            (pow(2f64, 3f64) - 8f64).abs() < 1e-9,
            "Invalid result {}",
            pow(2f64, 3f64)
        );
        assert!(
            (pow(0.5f64, 2f64) - 0.25f64).abs() < 1e-9,
            "Invalid result {}",
            pow(0.5f64, 2f64)
        );
    }

    #[test]
    fn f_pow_test() {
        assert_eq!(f_pow(
             0.00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000135264499699371,
            -0.5,
        ), 27189929701044785000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000.);
        assert_eq!(f_pow(
            0.000000000000000000000000000000000000000000000000000021798599361155193,
            -2.,
        ),2104470396771397700000000000000000000000000000000000000000000000000000000000000000000000000000000000000000.);
        assert_eq!(
            f_pow(-25192281723900620000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000.,
            -2.),
            0.
        );
        assert_eq!(
            f_pow(0.000000000000000000000000000000000000000000000000000021799650661798696,
                  -2.),
            2104267423084451500000000000000000000000000000000000000000000000000000000000000000000000000000000000000000.
        );
        assert_eq!(
            f_pow(0.00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000014916691520383755,
            -2.),
            44942267764413600000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000.
        );
        assert_eq!(
            f_pow(
                0.00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000135264499699371,
                -0.5,
            ),
            27189929701044785000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000.
        );
        assert_eq!(f_pow(1., f64::INFINITY), 1.);
        assert_eq!(f_pow(2., f64::INFINITY), f64::INFINITY);
        assert_eq!(f_pow(f64::INFINITY, -0.5), 0.);
        assert!(
            (f_pow(2f64, 3f64) - 8f64).abs() < 1e-9,
            "Invalid result {}",
            f_pow(2f64, 3f64)
        );
        assert!(
            (f_pow(0.5f64, 2f64) - 0.25f64).abs() < 1e-9,
            "Invalid result {}",
            f_pow(0.5f64, 2f64)
        );
        assert_eq!(f_pow(2.1f64, 2.7f64), 7.412967494768546);
        assert_eq!(f_pow(27., 1. / 3.), 3.);
    }

    #[test]
    fn powi_test() {
        assert_eq!(f_pow(f64::from_bits(0x3cc0_0000_0000_0000), 102.), 0.0);
        assert_eq!(f_pow(3., 3.), 27.);
        assert_eq!(f_pow(3., -3.), 1. / 27.);
        assert_eq!(f_pow(3., 102.), 4.638397686588102e48);
        assert_eq!(f_pow(0.000000000000011074474670636028, -22.), 10589880229528372000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000.);
    }
}
