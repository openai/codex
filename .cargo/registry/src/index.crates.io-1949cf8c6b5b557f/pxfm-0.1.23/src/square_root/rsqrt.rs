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

/// Computes 1/sqrt(x)
///
/// Max ULP 0.5
pub fn f_rsqrt(x: f64) -> f64 {
    let ix = x.to_bits();
    let r: f64 = if ix < 1u64 << 52 {
        // 0 <= x < 0x1p-1022
        if ix != 0 {
            // x <> +0
            x.sqrt() / x
        } else {
            return f64::INFINITY; // case x = +0
        }
    } else if ix >= 0x7ffu64 << 52 {
        // NaN, Inf, x <= 0
        if ix.wrapping_shl(1) == 0 {
            return f64::NEG_INFINITY; // x=-0
        }
        if ix > 0xfff0000000000000u64 {
            return x + x;
        } // -NaN
        if (ix >> 63) != 0 {
            // x < 0
            return f64::NAN;
        }
        if ix.wrapping_shl(12) == 0 {
            return 0.0;
        } // +/-Inf
        return x + x; // +NaN
    } else {
        // 0x1p-1022 <= x < 2^1024
        if ix > 0x7fd000000000000u64 {
            // x > 2^1022
            // avoid spurious underflow in 1/x
            (4.0 / x) * (0.25 * x.sqrt())
        } else {
            (1.0 / x) * x.sqrt()
        }
    };

    #[cfg(any(
        all(
            any(target_arch = "x86", target_arch = "x86_64"),
            target_feature = "fma"
        ),
        all(target_arch = "aarch64", target_feature = "neon")
    ))]
    {
        let d2x = DoubleDouble::from_exact_mult(r, x);
        use crate::common::f_fmla;
        let h = f_fmla(r, d2x.lo, f_fmla(r, d2x.hi, -1.0));
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
        use crate::double_double::two_product_compatible;
        if !two_product_compatible(x) {
            recip_hard_dyadic(x, r)
        } else {
            let d2x = DoubleDouble::from_exact_mult(r, x);
            let DoubleDouble { hi: h, lo: pr } = DoubleDouble::quick_mult_f64(d2x, r);
            let DoubleDouble { hi: p, lo: q } = DoubleDouble::from_full_exact_add(-1.0, h);
            let h = DoubleDouble::from_exact_add(p, pr + q);
            let dr = DoubleDouble::quick_mult_f64(h, r * 0.5);
            r - dr.hi - dr.lo
        }
    }
}

#[cfg(not(any(
    all(
        any(target_arch = "x86", target_arch = "x86_64"),
        target_feature = "fma"
    ),
    all(target_arch = "aarch64", target_feature = "neon")
)))]
#[cold]
#[inline(never)]
fn recip_hard_dyadic(x: f64, r: f64) -> f64 {
    use crate::dyadic_float::{DyadicFloat128, DyadicSign};
    let dx = DyadicFloat128::new_from_f64(x);
    let dr = DyadicFloat128::new_from_f64(r);
    const M_ONE: DyadicFloat128 = DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -127,
        mantissa: 0x80000000_00000000_00000000_00000000_u128,
    };
    let d2 = dx * dr;
    let h = d2 * dr + M_ONE;
    let mut half_dr = dr;
    half_dr.exponent -= 1; // * 0.5;
    let ddr = half_dr * h;
    (dr - ddr).fast_as_f64()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rsqrt() {
        assert_eq!(f_rsqrt(7518001163502890000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000.),
                   0.000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000011533172976634968);
        assert_eq!(f_rsqrt(0.000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001984274103353),
                   709903255474595300000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000.);
        assert_eq!(f_rsqrt(0.0), f64::INFINITY);
        assert_eq!(f_rsqrt(4.0), 0.5);
        assert_eq!(f_rsqrt(9.0), 1. / 3.);
        assert_eq!(f_rsqrt(-0.0), f64::NEG_INFINITY);
        assert!(f_rsqrt(f64::NAN).is_nan());
    }
}
