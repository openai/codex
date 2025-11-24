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
use crate::common::f_fmla;
use crate::sin_cosf::{ArgumentReducerPi, tanpif_eval};

/// Computes tan(PI*x)
///
/// Max found ULP 0.5
#[inline]
pub fn f_tanpif(x: f32) -> f32 {
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
        return f32::copysign(0.0, x);
    }

    let argument_reduction = ArgumentReducerPi { x: x as f64 };

    let (y, k) = argument_reduction.reduce();

    if y == 0.0 {
        let km = (k.abs() & 31) as i32; // k mod 32

        match km {
            0 => return 0.0f32.copysign(x),               // tanpi(n) = 0
            16 => return f32::copysign(f32::INFINITY, x), // tanpi(n+0.5) = ±∞
            8 => return f32::copysign(1.0, x),            // tanpi(n+0.25) = ±1
            24 => return -f32::copysign(1.0, x),          // tanpi(n+0.75) = ∓1
            _ => {}
        }
    }

    let ax = ix & 0x7fff_ffff;
    if ax < 0x38d1b717u32 {
        // taylor series for tan(PI*x) where |x| < 0.0001
        let dx = x as f64;
        let dx_sqr = dx * dx;
        // tan(PI*x) ~ PI*x + PI^3*x^3/3 + O(x^5)
        let r = f_fmla(
            dx_sqr,
            f64::from_bits(0x4024abbce625be53),
            f64::from_bits(0x400921fb54442d18),
        );
        return (r * dx) as f32;
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
    // tan = num / den
    (num / den) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tanpif() {
        assert_eq!(f_tanpif(3.666738e-5), 0.00011519398);
        assert_eq!(f_tanpif(1.0355987e-25), 3.2534293e-25);
        assert_eq!(f_tanpif(5.5625), -5.0273395);
        assert_eq!(f_tanpif(-29.75), 1.0);
        assert_eq!(f_tanpif(-21.5625), 5.0273395);
        assert_eq!(f_tanpif(-15.611655), 2.7329326);
        assert_eq!(f_tanpif(115.30706), 1.4426143);
        assert!(f_tanpif(f32::INFINITY).is_nan());
        assert!(f_tanpif(f32::NAN).is_nan());
    }
}
