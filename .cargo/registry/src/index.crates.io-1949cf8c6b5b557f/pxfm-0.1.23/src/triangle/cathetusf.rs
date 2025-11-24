/*
 * // Copyright (c) Radzivon Bartoshyk 9/2025. All rights reserved.
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
use crate::common::EXP_MASK_F32;

/// Computes the missing leg of a right triangle
///
/// Given a hypotenuse `x` and a known leg `y`, returns
/// `sqrt(x^2 - y^2)` = the length of the other leg.
///
/// Domain: requires `|x| >= |y|`. Returns NaN if the input
/// is outside this range.
pub fn f_cathetusf(x: f32, y: f32) -> f32 {
    let x_abs = x.abs();
    let y_abs = y.abs();

    let x_bits = x_abs.to_bits();
    let y_bits = y_abs.to_bits();

    let a_u = x_bits.max(y_bits);

    if a_u >= EXP_MASK_F32 {
        // x or y is inf or nan
        if f32::from_bits(x_bits).is_nan() || f32::from_bits(y_bits).is_nan() {
            return f32::NAN;
        }
        if f32::from_bits(x_bits).is_infinite() || f32::from_bits(y_bits).is_infinite() {
            if f32::from_bits(x_bits).is_infinite() && f32::from_bits(y_bits).is_infinite() {
                // ∞² - ∞² is undefined
                return f32::NAN;
            }
            return f32::INFINITY;
        }
        return f32::from_bits(x_bits);
    }
    if x_abs < y_abs {
        // Would yield sqrt(negative), undefined
        return f32::NAN;
    }
    if x_abs == y_abs {
        // sqrt(c² - c²) = 0
        return 0.0;
    }

    let dx = x as f64;
    let dy = y as f64;

    #[cfg(any(
        all(
            any(target_arch = "x86", target_arch = "x86_64"),
            target_feature = "fma"
        ),
        all(target_arch = "aarch64", target_feature = "neon")
    ))]
    {
        use crate::common::f_fmla;
        // for FMA environment we're using Kahan style summation which is short and reliable.
        let w = dy * dy; // RN(bc)
        let e = f_fmla(-dy, dy, w); // RN(w − bc)
        let f = f_fmla(dx, dx, -w); // RN(ad − w)
        let r = e + f; // RN(f + e)
        let cath = r.sqrt(); // sqrt(x^2 - y^2)
        cath as f32
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
        let dy2 = DoubleDouble::from_exact_mult(dy, dy);
        let fdx = DoubleDouble::from_exact_mult(dx, dx);
        // element must follow condition |x| > |y| so it always follows fasttwosum requirements
        let f = DoubleDouble::add_f64(fdx, -dy2.hi).to_f64();
        let r = dy2.lo + f;
        let cath = r.sqrt();
        cath as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_cathetusf_edge() {
        assert_eq!(f_cathetusf(5., 3.), 4.);
        assert_eq!(f_cathetusf(5., 4.), 3.);
        assert_eq!(f_cathetusf(13., 12.), 5.);
        assert_eq!(f_cathetusf(65., 16.), 63.);
        assert_eq!(f_cathetusf(25., 24.), 7.);
        assert!(f_cathetusf(24., 25.).is_nan());
    }

    #[test]
    fn test_cathetusf_edge_cases() {
        assert_eq!(f_cathetusf(0.0, 0.0), 0.0);
        assert_eq!(f_cathetusf(f32::INFINITY, 0.0), f32::INFINITY);
        assert_eq!(f_cathetusf(0.0, f32::INFINITY), f32::INFINITY);
        assert!(f_cathetusf(f32::INFINITY, f32::INFINITY).is_nan());
        assert_eq!(f_cathetusf(f32::NEG_INFINITY, 0.0), f32::INFINITY);
        assert_eq!(f_cathetusf(0.0, f32::NEG_INFINITY), f32::INFINITY);
        assert!(f_cathetusf(f32::NEG_INFINITY, f32::NEG_INFINITY).is_nan());
        assert!(f_cathetusf(f32::NAN, 1.0).is_nan());
        assert!(f_cathetusf(1.0, f32::NAN).is_nan());
    }
}
