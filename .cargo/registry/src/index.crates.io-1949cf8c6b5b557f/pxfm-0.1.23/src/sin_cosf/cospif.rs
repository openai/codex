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
use crate::polyeval::f_polyeval5;
use crate::sin_cosf::argument_reduction_pi::ArgumentReducerPi;
use crate::sin_cosf::sincosf_eval::{cospif_eval, sinpif_eval};

/// Computes cos(PI*x)
///
/// Max ULP 0.5
#[inline]
pub fn f_cospif(x: f32) -> f32 {
    let x_abs = x.to_bits() & 0x7fff_ffffu32;
    let x = f32::from_bits(x_abs);
    let xd = x as f64;

    // |x| <= 1/16
    if x_abs <= 0x3d80_0000u32 {
        // |x| < 0.00000009546391
        if x_abs < 0x38a2_f984u32 {
            #[cfg(any(
                all(
                    any(target_arch = "x86", target_arch = "x86_64"),
                    target_feature = "fma"
                ),
                all(target_arch = "aarch64", target_feature = "neon")
            ))]
            {
                use crate::common::f_fmlaf;
                return f_fmlaf(x, f32::from_bits(0xb3000000), 1.);
            }
            #[cfg(not(any(
                all(
                    any(target_arch = "x86", target_arch = "x86_64"),
                    target_feature = "fma"
                ),
                all(target_arch = "aarch64", target_feature = "neon")
            )))]
            {
                use crate::common::f_fmla;
                return f_fmla(xd, f64::from_bits(0xbe60000000000000), 1.) as f32;
            }
        }

        // Cos(x*PI)
        // Generated poly by Sollya:
        // d = [0, 1/16];
        // f_cos = cos(y*pi);
        // Q = fpminimax(f_cos, [|0, 2, 4, 6, 8|], [|D...|], d, relative, floating);
        //
        // See ./notes/cospif.sollya

        let x2 = xd * xd;
        let p = f_polyeval5(
            x2,
            f64::from_bits(0x3ff0000000000000),
            f64::from_bits(0xc013bd3cc9be43f7),
            f64::from_bits(0x40103c1f08091fe0),
            f64::from_bits(0xbff55d3ba3d94835),
            f64::from_bits(0x3fce173c2a00e74e),
        );
        return p as f32;
    }

    // Numbers greater or equal to 2^23 are always integers or NaN
    if x_abs >= 0x4b00_0000u32 {
        if x_abs >= 0x7f80_0000u32 {
            return x + f32::NAN;
        }
        if x_abs < 0x4b80_0000u32 {
            static CF: [f32; 2] = [1., -1.];
            return CF[((x_abs & 0x1) != 0) as usize];
        }
        return 1.;
    }

    // We're computing cos(y) after argument reduction then return valid value
    // based on quadrant
    let reducer = ArgumentReducerPi { x: x as f64 };
    let (y, k) = reducer.reduce_0p25();
    // Decide based on quadrant what kernel function to use
    (match k & 3 {
        0 => cospif_eval(y),
        1 => sinpif_eval(-y),
        2 => -cospif_eval(y),
        _ => sinpif_eval(y),
    }) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_f_cospif() {
        assert_eq!(f_cospif(115.30706), -0.5696978);
        assert!(f_cospif(f32::INFINITY).is_nan());
    }
}
