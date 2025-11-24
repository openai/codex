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
use crate::compound::compound_m1f::compoundf_expf_poly;
use crate::compound::compoundf::{
    COMPOUNDF_EXP2_T, COMPOUNDF_EXP2_U, LOG2P1_COMPOUNDF_INV, LOG2P1_COMPOUNDF_LOG2_INV,
};
use crate::powf::{is_integer, is_odd_integer};
use std::hint::black_box;

#[inline]
fn powm1f_log2_fast(x: f64) -> f64 {
    /* for x > 0, 1+x is exact when 2^-29 <=  x < 2^53
    for x < 0, 1+x is exact when -1 < x <= 2^-30 */

    //  double u = (x >= 0x1p53) ? x : 1.0 + x;
    /* For x < 0x1p53, x + 1 is exact thus u = x+1.
    For x >= 2^53, we estimate log2(x) instead of log2(1+x),
    since log2(1+x) = log2(x) + log2(1+1/x),
    log2(x) >= 53 and |log2(1+1/x)| < 2^-52.471, the additional relative
    error is bounded by 2^-52.471/53 < 2^-58.198 */

    let mut v = x.to_bits();
    let m: u64 = v & 0xfffffffffffffu64;
    let e: i64 = (v >> 52) as i64 - 0x3ff + (m >= 0x6a09e667f3bcdu64) as i64;
    // 2^e/sqrt(2) < u < 2^e*sqrt(2), with -29 <= e <= 128
    v = v.wrapping_sub((e << 52) as u64);
    let t = f64::from_bits(v);
    // u = 2^e*t with 1/sqrt(2) < t < sqrt(2)
    // thus log2(u) = e + log2(t)
    v = (f64::from_bits(v) + 2.0).to_bits(); // add 2 so that v.f is always in the binade [2, 4)
    let i = (v >> 45) as i32 - 0x2002d; // 0 <= i <= 45
    let r = f64::from_bits(LOG2P1_COMPOUNDF_INV[i as usize]);
    let z = dd_fmla(r, t, -1.0); // exact, -1/64 <= z <= 1/64
    // we approximates log2(t) by -log2(r) + log2(r*t)
    let p = crate::compound::compoundf::log2p1_polyeval_1(z);
    // p approximates log2(r*t) with rel. error < 2^-49.642, and |p| < 2^-5.459
    e as f64 + (f64::from_bits(LOG2P1_COMPOUNDF_LOG2_INV[i as usize].1) + p)
}

/// Computes x^y - 1
pub fn f_powm1f(x: f32, y: f32) -> f32 {
    let ax: u32 = x.to_bits().wrapping_shl(1);
    let ay: u32 = y.to_bits().wrapping_shl(1);

    // filter out exceptional cases
    if ax == 0 || ax >= 0xffu32 << 24 || ay == 0 || ay >= 0xffu32 << 24 {
        if x.is_nan() || y.is_nan() {
            return f32::NAN;
        }

        // Handle infinities
        if x.is_infinite() {
            return if x.is_sign_positive() {
                if y.is_infinite() {
                    return f32::INFINITY;
                } else if y > 0.0 {
                    f32::INFINITY // inf^positive -> inf
                } else if y < 0.0 {
                    -1.0 // inf^negative -> 0, so powm1 = -1
                } else {
                    f32::NAN // inf^0 -> NaN (0^0 conventionally 1, inf^0 = NaN)
                }
            } else {
                // x = -inf
                if y.is_infinite() {
                    return -1.0;
                }
                if is_integer(y) {
                    // Negative base: (-inf)^even = +inf, (-inf)^odd = -inf
                    let pow = if y as i32 % 2 == 0 {
                        f32::INFINITY
                    } else {
                        f32::NEG_INFINITY
                    };
                    pow - 1.0
                } else {
                    f32::NAN // Negative base with non-integer exponent
                }
            };
        }

        // Handle y infinite
        if y.is_infinite() {
            return if x.abs() > 1.0 {
                if y.is_sign_positive() {
                    f32::INFINITY
                } else {
                    -1.0
                }
            } else if x.abs() < 1.0 {
                if y.is_sign_positive() {
                    -1.0
                } else {
                    f32::INFINITY
                }
            } else {
                // |x| == 1
                f32::NAN // 1^inf or -1^inf is undefined
            };
        }

        // Handle zero base
        if x == 0.0 {
            return if y > 0.0 {
                -1.0 // 0^positive -> 0, powm1 = -1
            } else if y < 0.0 {
                f32::INFINITY // 0^negative -> inf
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
            return f32::NAN; // x < 0 and non-integer y
        }
        x = x.abs();
        if is_odd_integer(y) {
            negative_parity = true;
        }
    }

    let xd = x as f64;
    let yd = y as f64;
    let tx = xd.to_bits();
    let ty = yd.to_bits();

    let l: f64 = powm1f_log2_fast(f64::from_bits(tx));

    /* l approximates log2(1+x) with relative error < 2^-47.997,
    and 2^-149 <= |l| < 128 */

    let dt = l * f64::from_bits(ty);
    let t: u64 = dt.to_bits();

    // detect overflow/underflow
    if (t.wrapping_shl(1)) >= (0x406u64 << 53) {
        // |t| >= 128
        if t >= 0x3018bu64 << 46 {
            // t <= -150
            return -1.;
        } else if (t >> 63) == 0 {
            // t >= 128: overflow
            return black_box(f32::from_bits(0x7e800000)) * black_box(f32::from_bits(0x7e800000));
        }
    }

    let res = powm1_exp2m1_fast(f64::from_bits(t));
    // For x < 0 and integer y = n:
    // if n is even: x^n = |x|^n → powm1 = |x|^n - 1 (same sign as res).
    // if n is odd: x^n = -|x|^n → powm1 = -|x|^n - 1 = - (|x|^n + 1).
    if negative_parity {
        (-res - 2.) as f32
    } else {
        res as f32
    }
}

#[inline]
pub(crate) fn powm1_exp2m1_fast(t: f64) -> f64 {
    let k = t.round_ties_even(); // 0 <= |k| <= 150
    let mut r = t - k; // |r| <= 1/2, exact
    let mut v: f64 = 3.015625 + r; // 2.5 <= v <= 3.5015625
    // we add 2^-6 so that i is rounded to nearest
    let i: i32 = (v.to_bits() >> 46) as i32 - 0x10010; // 0 <= i <= 32
    r -= f64::from_bits(COMPOUNDF_EXP2_T[i as usize]); // exact
    // now |r| <= 2^-6
    // 2^t = 2^k * exp2_U[i][0] * 2^r
    let mut s = f64::from_bits(COMPOUNDF_EXP2_U[i as usize].1);
    let su = (k as i64) << 52;
    s = f64::from_bits(s.to_bits().wrapping_add(su as u64));
    let q_poly = compoundf_expf_poly(r);
    v = q_poly;

    #[cfg(any(
        all(
            any(target_arch = "x86", target_arch = "x86_64"),
            target_feature = "fma"
        ),
        all(target_arch = "aarch64", target_feature = "neon")
    ))]
    {
        v = f_fmla(v, s, s - 1f64);
    }
    #[cfg(not(any(
        all(
            any(target_arch = "x86", target_arch = "x86_64"),
            target_feature = "fma"
        ),
        all(target_arch = "aarch64", target_feature = "neon")
    )))]
    {
        use crate::double_double::DoubleDouble;
        let p0 = DoubleDouble::from_full_exact_add(s, -1.);
        let z = DoubleDouble::from_exact_mult(v, s);
        v = DoubleDouble::add(z, p0).to_f64();
    }

    v
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_powm1f() {
        assert_eq!(f_powm1f(1.83329e-40, 2.4645883e-32), -2.2550315e-30);
        assert_eq!(f_powm1f(f32::INFINITY, f32::INFINITY), f32::INFINITY);
        assert_eq!(f_powm1f(-3.375, -9671689000000000000000000.), -1.);
        assert_eq!(f_powm1f(3., 2.), 8.);
        assert_eq!(f_powm1f(3., 3.), 26.);
        assert_eq!(f_powm1f(5., 2.), 24.);
        assert_eq!(f_powm1f(5., -2.), 1. / 25. - 1.);
        assert_eq!(f_powm1f(-5., 2.), 24.);
        assert_eq!(f_powm1f(-5., 3.), -126.);
        assert_eq!(
            f_powm1f(196560., 0.000000000000000000000000000000000000001193773),
            1.455057e-38
        );
    }
}
