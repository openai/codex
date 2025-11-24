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
use crate::double_double::DoubleDouble;

/// Computes the missing leg of a right triangle
///
/// Given a hypotenuse `x` and a known leg `y`, returns
/// `sqrt(x^2 - y^2)` = the length of the other leg.
///
/// Domain: requires `|x| >= |y|`. Returns NaN if the input
/// is outside this range.
pub fn f_cathetus(x: f64, y: f64) -> f64 {
    let x_abs = x.abs();
    let y_abs = y.abs();

    let x_bits = x_abs.to_bits();
    let y_bits = y_abs.to_bits();

    let a_u = x_bits.max(y_bits);

    let mut dx = x;
    let mut dy = y;

    const EXP_MASK_F64: u64 = 0x7FF0_0000_0000_0000;
    if a_u >= EXP_MASK_F64 {
        // x or y is inf or nan
        if f64::from_bits(x_bits).is_nan() || f64::from_bits(y_bits).is_nan() {
            return f64::NAN;
        }
        if f64::from_bits(x_bits).is_infinite() || f64::from_bits(y_bits).is_infinite() {
            if f64::from_bits(x_bits).is_infinite() && f64::from_bits(y_bits).is_infinite() {
                // ∞² - ∞² is undefined
                return f64::NAN;
            }
            return f64::INFINITY;
        }
        return f64::from_bits(x_bits);
    }
    if x_abs < y_abs {
        // Would yield sqrt(negative), undefined
        return f64::NAN;
    }
    if x_abs == y_abs {
        // sqrt(c² - c²) = 0
        return 0.0;
    }

    let e_x = x_bits >> 52;
    let e_y = y_bits >> 52;
    let unbiased_e_x = (e_x as i32).wrapping_sub(1023);
    let mut scale = 1f64;

    if e_y == 0 {
        if e_x - e_y > 52 {
            // y is too small to make difference, so result is just |x|
            return x_abs;
        }
        dx *= f64::from_bits(0x6bb0000000000000); // 2^700
        dy *= f64::from_bits(0x6bb0000000000000); // 2^700
        scale = f64::from_bits(0x1430000000000000); // 2^(-700 / 2)
    } else if unbiased_e_x >= 510 {
        dx *= f64::from_bits(0x1430000000000000); // 2^-700
        dy *= f64::from_bits(0x1430000000000000); // 2^-700
        scale = f64::from_bits(0x6bb0000000000000); // 2^(700 / 2)
    } else if unbiased_e_x <= -450 {
        dx *= f64::from_bits(0x6bb0000000000000); // 2^700
        dy *= f64::from_bits(0x6bb0000000000000); // 2^700
        scale = f64::from_bits(0x1430000000000000); // 2^(-700)
    }

    let dy2 = DoubleDouble::from_exact_mult(dy, dy);
    let dx2 = DoubleDouble::from_exact_mult(dx, dx);
    let p = DoubleDouble::sub(dx2, dy2);
    let cath = p.fast_sqrt(); // sqrt(x^2 - y^2)
    cath.to_f64() * scale
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cathethus() {
        assert_eq!(
            f_cathetus(0.00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000002248996583584318,
                       0.000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000002842248694776204),
            0.00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000002248996583584318
        );
        assert_eq!(
            f_cathetus(0.00003241747618121237, 0.00003241747618121195),
            5.219099637789996e-12
        );
        assert_eq!(f_cathetus(0.000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000003382112264930946,
                              -0.00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000005284550413954603),
                   0.000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000003381699384228079);
        assert_eq!(f_cathetus(5., 3.), 4.);
        assert_eq!(f_cathetus(5., 4.), 3.);
        assert_eq!(f_cathetus(13., 12.), 5.);
        assert_eq!(f_cathetus(65., 16.), 63.);
        assert_eq!(f_cathetus(25., 24.), 7.);
        assert!(f_cathetus(24., 25.).is_nan());
    }

    #[test]
    fn test_cathetus_edge_cases() {
        assert_eq!(f_cathetus(0.0, 0.0), 0.0);
        assert_eq!(f_cathetus(f64::INFINITY, 0.0), f64::INFINITY);
        assert_eq!(f_cathetus(0.0, f64::INFINITY), f64::INFINITY);
        assert!(f_cathetus(f64::INFINITY, f64::INFINITY).is_nan());
        assert_eq!(f_cathetus(f64::NEG_INFINITY, 0.0), f64::INFINITY);
        assert_eq!(f_cathetus(0.0, f64::NEG_INFINITY), f64::INFINITY);
        assert!(f_cathetus(f64::NEG_INFINITY, f64::NEG_INFINITY).is_nan());
        assert!(f_cathetus(f64::NAN, 1.0).is_nan());
        assert!(f_cathetus(1.0, f64::NAN).is_nan());
    }
}
