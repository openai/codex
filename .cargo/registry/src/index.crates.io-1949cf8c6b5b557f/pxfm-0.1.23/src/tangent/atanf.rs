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
use crate::common::{f_fmla, f_fmlaf};

/// Computes atan
///
/// Max found ULP 0.49999973
#[inline]
pub fn f_atanf(x: f32) -> f32 {
    const PI2: f64 = f64::from_bits(0x3ff921fb54442d18);
    let t = x.to_bits();
    let e = (t >> 23) & 0xff;
    let gt = e >= 127;
    let ta = t & 0x7fffffff;
    if ta >= 0x4c700518u32 {
        // |x| >= 6.29198e+07
        if ta > 0x7f800000u32 {
            return x + x;
        } // nan
        return f32::copysign(PI2 as f32, x); // inf or |x| >= 6.29198e+07
    }
    if e < 127 - 13 {
        // |x| < 2^-13
        if e < 127 - 25 {
            // |x| < 2^-25
            if t << 1 == 0 {
                return x;
            }
            let res = f_fmlaf(-x, x.abs(), x);
            return res;
        }
        return f_fmlaf(-f64::from_bits(0x3fd5555560000000) as f32 * x, x * x, x);
    }
    /* now |x| >= 0.00012207 */
    let mut z = x as f64;
    if gt {
        z = 1.0 / z;
    } /* gt is non-zero for |x| >= 1 */
    let z2 = z * z;
    let z4 = z2 * z2;
    let z8 = z4 * z4;
    /* polynomials generated using rminimax
       (https://gitlab.inria.fr/sfilip/rminimax) with the following command:
       ./ratapprox --function="atan(x)" --dom=[0.000122070,1] --num=[x,x^3,x^5,x^7,x^9,x^11,x^13] --den=[1,x^2,x^4,x^6,x^8,x^10,x^12] --output=atanf.sollya --log
       (see output atanf.sollya)
       The coefficient cd[0] was slightly reduced from the original value
       0.330005 to avoid an exceptional case for |x| = 0.069052
       and rounding to nearest.
    */
    const CN: [u64; 7] = [
        0x3fd51eccde075d67,
        0x3fea76bb5637f2f2,
        0x3fe81e0eed20de88,
        0x3fd376c8ca67d11d,
        0x3faaec7b69202ac6,
        0x3f69561899acc73e,
        0x3efbf9fa5b67e600,
    ];
    const CD: [u64; 7] = [
        0x3fd51eccde075d66,
        0x3fedfbdd7b392d28,
        0x3ff0000000000000,
        0x3fdfd22bf0e89b54,
        0x3fbd91ff8b576282,
        0x3f8653ea99fc9bb0,
        0x3f31e7fcc202340a,
    ];
    let mut cn0 = f_fmla(z2, f64::from_bits(CN[1]), f64::from_bits(CN[0]));
    let cn2 = f_fmla(z2, f64::from_bits(CN[3]), f64::from_bits(CN[2]));
    let mut cn4 = f_fmla(z2, f64::from_bits(CN[5]), f64::from_bits(CN[4]));
    let cn6 = f64::from_bits(CN[6]);
    cn0 = f_fmla(z4, cn2, cn0);
    cn4 = f_fmla(z4, cn6, cn4);
    cn0 = f_fmla(z8, cn4, cn0);
    cn0 *= z;
    let mut cd0 = f_fmla(z2, f64::from_bits(CD[1]), f64::from_bits(CD[0]));
    let cd2 = f_fmla(z2, f64::from_bits(CD[3]), f64::from_bits(CD[2]));
    let mut cd4 = f_fmla(z2, f64::from_bits(CD[5]), f64::from_bits(CD[4]));
    let cd6 = f64::from_bits(CD[6]);
    cd0 = f_fmla(z4, cd2, cd0);
    cd4 = f_fmla(z4, cd6, cd4);
    cd0 = f_fmla(z8, cd4, cd0);
    let r = cn0 / cd0;
    if !gt {
        return r as f32;
    } /* for |x| < 1, (float) r is correctly rounded */

    const PI_OVER2_H: f64 = f64::from_bits(0x3ff9000000000000);
    const PI_OVER2_L: f64 = f64::from_bits(0x3f80fdaa22168c23);
    /* now r approximates atan(1/x), we use atan(x) + atan(1/x) = sign(x)*pi/2,
    where PI_OVER2_H + PI_OVER2_L approximates pi/2.
    With sign(z)*L + (-r + sign(z)*H), it fails for x=0x1.98c252p+12 and
    rounding upward.
    With sign(z)*PI - r, where PI is a double approximation of pi to nearest,
    it fails for x=0x1.ddf9f6p+0 and rounding upward. */
    ((f64::copysign(PI_OVER2_L, z) - r) + f64::copysign(PI_OVER2_H, z)) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f_atan_test() {
        assert!(
            (f_atanf(1.0) - std::f32::consts::PI / 4f32).abs() < 1e-6,
            "Invalid result {}",
            f_atanf(1f32)
        );
        assert!(
            (f_atanf(2f32) - 1.107148717794090503017065f32).abs() < 1e-6,
            "Invalid result {}",
            f_atanf(2f32)
        );
        assert!(
            (f_atanf(5f32) - 1.3734007669450158608612719264f32).abs() < 1e-6,
            "Invalid result {}",
            f_atanf(5f32)
        );
    }
}
