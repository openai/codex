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

/// Computes atan(x)/PI
///
/// Max ULP 0.5
#[inline]
pub fn f_atanpif(x: f32) -> f32 {
    let t = x.to_bits();
    let e: i32 = ((t >> 23) & 0xff) as i32;
    let gt = e >= 127;
    if e > 127 + 24 {
        // |x| >= 2^25
        let f = f32::copysign(0.5, x);
        if e == 0xff {
            if (t.wrapping_shl(9)) != 0 {
                return x + x;
            } // nan
            return f; // inf
        }
        // Warning: 0x1.45f306p-2f / x underflows for |x| >= 0x1.45f306p+124
        return if x.abs() >= f32::from_bits(0x7da2f983) {
            f - f32::copysign(f32::from_bits(0x32800000), x)
        } else {
            f - f32::from_bits(0x3ea2f983) / x
        };
    }
    let mut z = x as f64;
    if e < 127 - 13 {
        // |x| < 2^-13
        let sx = z * f64::from_bits(0x3fd45f306dc9c883);
        if e < 127 - 25 {
            // |x| < 2^-25
            return sx as f32;
        }
        let zz0 = sx - (f64::from_bits(0x3fd5555555555555) * sx) * (x as f64 * x as f64);
        return zz0 as f32;
    }
    let ax = t & 0x7fff_ffff;
    if ax == 0x3fa267ddu32 {
        return f32::copysign(f32::from_bits(0x3e933802), x)
            - f32::copysign(f32::from_bits(0x24000000), x);
    };
    if ax == 0x3f693531u32 {
        return f32::copysign(f32::from_bits(0x3e70d331), x)
            + f32::copysign(f32::from_bits(0x31800000), x);
    };
    if ax == 0x3f800000u32 {
        return f32::copysign(f32::from_bits(0x3e800000), x);
    };
    if gt {
        z = 1. / z;
    }
    let z2 = z * z;
    let z4 = z2 * z2;
    let z8 = z4 * z4;
    const CN: [u64; 6] = [
        0x3fd45f306dc9c882,
        0x3fe733b561bc23d5,
        0x3fe28d9805bdfbf2,
        0x3fc8c3ba966ae287,
        0x3f994a7f81ee634b,
        0x3f4a6bbf6127a6df,
    ];
    let mut cn0 = f_fmla(z2, f64::from_bits(CN[1]), f64::from_bits(CN[0]));
    let cn2 = f_fmla(z2, f64::from_bits(CN[3]), f64::from_bits(CN[2]));
    let cn4 = f_fmla(z2, f64::from_bits(CN[5]), f64::from_bits(CN[4]));
    cn0 += z4 * cn2;
    cn0 += z8 * cn4;
    cn0 *= z;

    const CD: [u64; 7] = [
        0x3ff0000000000000,
        0x4004e3b3ecc2518f,
        0x4003ef4a360ff063,
        0x3ff0f1dc55bad551,
        0x3fc8da0fecc018a4,
        0x3f88fa87803776bf,
        0x3f1dadf2ca0acb43,
    ];

    let mut cd0 = f_fmla(z2, f64::from_bits(CD[1]), f64::from_bits(CD[0]));
    let cd2 = f_fmla(z2, f64::from_bits(CD[3]), f64::from_bits(CD[2]));
    let mut cd4 = f_fmla(z2, f64::from_bits(CD[5]), f64::from_bits(CD[4]));
    let cd6 = f64::from_bits(CD[6]);
    cd0 += z4 * cd2;
    cd4 += z4 * cd6;
    cd0 += z8 * cd4;
    let mut r = cn0 / cd0;
    if gt {
        r = f64::copysign(0.5, z) - r;
    }
    r as f32
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_atanpif() {
        assert_eq!(f_atanpif(0.0), 0.0);
        assert_eq!(f_atanpif(1.0), 0.25);
        assert_eq!(f_atanpif(1.5), 0.31283295);
        assert_eq!(f_atanpif(-1.0), -0.25);
        assert_eq!(f_atanpif(-1.5), -0.31283295);
    }
}
