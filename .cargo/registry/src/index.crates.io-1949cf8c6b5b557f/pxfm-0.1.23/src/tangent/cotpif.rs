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
use crate::sin_cosf::{ArgumentReducerPi, tanpif_eval};

/// Computes 1/tan(PI*x)
///
/// Max found ULP 0.5
#[inline]
pub fn f_cotpif(x: f32) -> f32 {
    let ix = x.to_bits();
    let e = ix & (0xff << 23);
    if e > (150 << 23) {
        // |x| > 2^23
        if e == (0xff << 23) {
            // x = nan or inf
            if (ix.wrapping_shl(9)) == 0 {
                // x = inf
                return f32::NAN;
            }
            return x + x; // x = nan
        }
        return f32::INFINITY;
    }

    let argument_reduction = ArgumentReducerPi { x: x as f64 };

    let (y, k) = argument_reduction.reduce();

    if y == 0.0 {
        let km = (k.abs() & 31) as i32; // k mod 32

        match km {
            0 => return f32::copysign(f32::INFINITY, x), // cotpi(n) = ∞
            16 => return 0.0f32.copysign(x),             // cotpi(n+0.5) = 0
            8 => return f32::copysign(1.0, x),           // cotpi(n+0.25) = 1
            24 => return -f32::copysign(1.0, x),         // cotpi(n+0.75) = -1
            _ => {}
        }
    }

    let ax = ix & 0x7fff_ffff;
    if ax < 0x3bc49ba6u32 {
        // taylor series for cot(PI*x) where |x| < 0.006
        let dx = x as f64;
        let dx_sqr = dx * dx;
        // cot(PI*x) ~ 1/(PI*x) - PI*x/3 - PI^3*x^3/45 + O(x^5)
        const ONE_OVER_PI: f64 = f64::from_bits(0x3fd45f306dc9c883);
        let r = f_fmla(
            dx_sqr,
            f64::from_bits(0xbfe60c8539c1dc14),
            f64::from_bits(0xbff0c152382d7366),
        );
        let rcp = 1. / dx;
        return f_fmla(rcp, ONE_OVER_PI, r * dx) as f32;
    }

    // tanpif_eval returns:
    // - rs.tan_y = tan(pi/32 * y)          -> tangent of the remainder
    // - rs.msin_k = sin(pi/32 * k)         -> sine of the main angle multiple
    // - rs.cos_k  = cos(pi/32 * k)         -> cosine of the main angle multiple
    let rs = tanpif_eval(y, k);
    // num = sin(k*pi/32) + tan(y*pi/32) * cos(k*pi/32)
    let num = f_fmla(rs.tan_y, rs.cos_k, -rs.msin_k);
    // den = cos(k*pi/32) - tan(y*pi/32) * sin(k*pi/32)
    let den = f_fmla(rs.tan_y, rs.msin_k, rs.cos_k);
    // hence tan = num / den then
    // cot = den / num
    (den / num) as f32
}

#[inline]
pub(crate) fn cotpif_core(x: f64) -> f64 {
    let argument_reduction = ArgumentReducerPi { x };

    let (y, k) = argument_reduction.reduce();

    // tanpif_eval returns:
    // - rs.tan_y = tan(pi/32 * y)          -> tangent of the remainder
    // - rs.msin_k = sin(pi/32 * k)         -> sine of the main angle multiple
    // - rs.cos_k  = cos(pi/32 * k)         -> cosine of the main angle multiple
    let rs = tanpif_eval(y, k);
    // num = sin(k*pi/32) + tan(y*pi/32) * cos(k*pi/32)
    let num = f_fmla(rs.tan_y, rs.cos_k, -rs.msin_k);
    // den = cos(k*pi/32) - tan(y*pi/32) * sin(k*pi/32)
    let den = f_fmla(rs.tan_y, rs.msin_k, rs.cos_k);
    // hence tan = num / den then
    // cot = den / num
    den / num
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cotpif() {
        println!("0x{:8x}", 0.006f32.to_bits());
        assert_eq!(f_cotpif(0.00046277765), 687.82416);
        assert_eq!(f_cotpif(2.3588752e-6), 134941.39);
        assert_eq!(f_cotpif(10775313000000000000000000000000.), f32::INFINITY);
        assert_eq!(f_cotpif(5.5625), -0.19891237);
        assert_eq!(f_cotpif(-29.75), 1.0);
        assert_eq!(f_cotpif(-21.5625), 0.19891237);
        assert_eq!(f_cotpif(-15.611655), 0.3659073);
        assert_eq!(f_cotpif(115.30706), 0.693186);
        assert_eq!(f_cotpif(0.), f32::INFINITY);
        assert!(f_cotpif(f32::INFINITY).is_nan());
        assert!(f_cotpif(f32::NAN).is_nan());
    }
}
