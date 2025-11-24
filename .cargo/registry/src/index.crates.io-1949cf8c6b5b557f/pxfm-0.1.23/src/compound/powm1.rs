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
use crate::exponents::{EXPM1_T0, EXPM1_T1, ldexp};
use crate::pow::{is_integer, is_odd_integer};
use crate::pow_exec::pow_log_1;

/// Computes x^y - 1
pub fn f_powm1(x: f64, y: f64) -> f64 {
    let ax: u64 = x.to_bits().wrapping_shl(1);
    let ay: u64 = y.to_bits().wrapping_shl(1);

    // filter out exceptional cases
    if ax == 0 || ax >= 0x7ffu64 << 53 || ay == 0 || ay >= 0x7ff64 << 53 {
        if x.is_nan() || y.is_nan() {
            return f64::NAN;
        }

        // Handle infinities
        if x.is_infinite() {
            return if x.is_sign_positive() {
                if y.is_infinite() {
                    return f64::INFINITY;
                } else if y > 0.0 {
                    f64::INFINITY // inf^positive -> inf
                } else if y < 0.0 {
                    -1.0 // inf^negative -> 0, so powm1 = -1
                } else {
                    f64::NAN // inf^0 -> NaN (0^0 conventionally 1, inf^0 = NaN)
                }
            } else {
                // x = -inf
                if y.is_infinite() {
                    return -1.0;
                }
                if is_integer(y) {
                    // Negative base: (-inf)^even = +inf, (-inf)^odd = -inf
                    let pow = if y as i32 % 2 == 0 {
                        f64::INFINITY
                    } else {
                        f64::NEG_INFINITY
                    };
                    pow - 1.0
                } else {
                    f64::NAN // Negative base with non-integer exponent
                }
            };
        }

        // Handle y infinite
        if y.is_infinite() {
            return if x.abs() > 1.0 {
                if y.is_sign_positive() {
                    f64::INFINITY
                } else {
                    -1.0
                }
            } else if x.abs() < 1.0 {
                if y.is_sign_positive() {
                    -1.0
                } else {
                    f64::INFINITY
                }
            } else {
                // |x| == 1
                f64::NAN // 1^inf or -1^inf is undefined
            };
        }

        // Handle zero base
        if x == 0.0 {
            return if y > 0.0 {
                -1.0 // 0^positive -> 0, powm1 = -1
            } else if y < 0.0 {
                f64::INFINITY // 0^negative -> inf
            } else {
                0.0 // 0^0 -> conventionally 1, powm1 = 0
            };
        }
    }

    let y_integer = is_integer(y);

    let mut negative_parity: bool = false;

    let mut x = x;

    // Handle negative base with non-integer exponent
    if x < 0.0 {
        if !y_integer {
            return f64::NAN; // x < 0 and non-integer y
        }
        x = x.abs();
        if is_odd_integer(y) {
            negative_parity = true;
        }
    }

    let (mut l, _) = pow_log_1(x);
    l = DoubleDouble::from_exact_add(l.hi, l.lo);

    let r = DoubleDouble::quick_mult_f64(l, y);
    if r.hi < -37.42994775023705 {
        // underflow
        return -1.;
    }
    let res = powm1_expm1_1(r);
    // For x < 0 and integer y = n:
    // if n is even: x^n = |x|^n → powm1 = |x|^n - 1 (same sign as res).
    // if n is odd: x^n = -|x|^n → powm1 = -|x|^n - 1 = - (|x|^n + 1).
    if negative_parity {
        DoubleDouble::full_add_f64(-res, -2.).to_f64()
    } else {
        res.to_f64()
    }
}

#[inline]
pub(crate) fn powm1_expm1_1(r: DoubleDouble) -> DoubleDouble {
    let ax = r.hi.to_bits() & 0x7fffffffffffffffu64;

    const LOG2H: f64 = f64::from_bits(0x3f262e42fefa39ef);
    const LOG2L: f64 = f64::from_bits(0x3bbabc9e3b39803f);

    if ax <= 0x3f80000000000000 {
        // |x| < 2^-7
        if ax < 0x3970000000000000 {
            // |x| < 2^-104
            return r;
        }
        let d = crate::pow_exec::expm1_poly_dd_tiny(r);
        return d;
    }

    const INVLOG2: f64 = f64::from_bits(0x40b71547652b82fe);

    let k = (r.hi * INVLOG2).round_ties_even();

    let z = DoubleDouble::mul_f64_add(DoubleDouble::new(LOG2L, LOG2H), -k, r);

    let bk = k as i64; /* Note: k is an integer, this is just a conversion. */
    let mk = (bk >> 12) + 0x3ff;
    let i2 = (bk >> 6) & 0x3f;
    let i1 = bk & 0x3f;

    let t0 = DoubleDouble::from_bit_pair(EXPM1_T0[i2 as usize]);
    let t1 = DoubleDouble::from_bit_pair(EXPM1_T1[i1 as usize]);
    let tbh = DoubleDouble::quick_mult(t1, t0);
    let mut de = tbh;
    // exp(k)=2^k*exp(r) + (2^k - 1)
    let q = crate::pow_exec::expm1_poly_fast(z);
    de = DoubleDouble::quick_mult(de, q);
    de = DoubleDouble::add(tbh, de);

    let ie = mk - 0x3ff;

    let off: f64 = f64::from_bits((2048i64 + 1023i64).wrapping_sub(ie).wrapping_shl(52) as u64);

    let e: f64;
    if ie < 53 {
        let fhz = DoubleDouble::from_exact_add(off, de.hi);
        de.hi = fhz.hi;
        e = fhz.lo;
    } else if ie < 104 {
        let fhz = DoubleDouble::from_exact_add(de.hi, off);
        de.hi = fhz.hi;
        e = fhz.lo;
    } else {
        e = 0.;
    }
    de.lo += e;
    de.hi = ldexp(de.to_f64(), ie as i32);
    de.lo = 0.;
    de
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_powm1() {
        assert_eq!(f_powm1(f64::INFINITY, f64::INFINITY), f64::INFINITY);
        assert_eq!(f_powm1(50850368932909610000000000., 0.000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000023201985303960773), 1.3733470789307166e-303);
        assert_eq!(f_powm1(-3.375, -9671689000000000000000000.), -1.);
        assert_eq!(f_powm1(1.83329e-40, 2.4645883e-32), -2.255031542428047e-30);
        assert_eq!(f_powm1(3., 2.), 8.);
        assert_eq!(f_powm1(3., 3.), 26.);
        assert_eq!(f_powm1(5., 2.), 24.);
        assert_eq!(f_powm1(5., -2.), 1. / 25. - 1.);
        assert_eq!(f_powm1(-5., 2.), 24.);
        assert_eq!(f_powm1(-5., 3.), -126.);
        assert_eq!(
            f_powm1(196560., 0.000000000000000000000000000000000000001193773),
            1.4550568430468268e-38
        );
    }
}
