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

static OFF: [f32; 8] = [0.0, 0.5, 1.0, 0.5, -0.0, -0.5, -1.0, -0.5];
static SGNF: [f32; 2] = [1., -1.];
static SGN: [f64; 2] = [1., -1.];

/// Computes atan(x/y) / PI
///
/// Max found ULP 0.5
#[inline]
pub fn f_atan2pif(y: f32, x: f32) -> f32 {
    let tx = x.to_bits();
    let ty: u32 = y.to_bits();
    let ux: u32 = tx;
    let uy: u32 = ty;
    let ax: u32 = ux & 0x7fff_ffff;
    let ay = uy & 0x7fff_ffff;
    if ay >= (0xff << 23) || ax >= (0xff << 23) {
        if ay > (0xff << 23) {
            return x + y;
        } // nan
        if ax > (0xff << 23) {
            return x + y;
        } // nan
        let yinf = ay == (0xff << 23);
        let xinf = ax == (0xff << 23);
        if yinf & xinf {
            return if (ux >> 31) != 0 {
                0.75 * SGNF[(uy >> 31) as usize]
            } else {
                0.25 * SGNF[(uy >> 31) as usize]
            };
        }
        if xinf {
            return if (ux >> 31) != 0 {
                SGNF[(uy >> 31) as usize]
            } else {
                0.0 * SGNF[(uy >> 31) as usize]
            };
        }
        if yinf {
            return 0.5 * SGNF[(uy >> 31) as usize];
        }
    }
    if ay == 0 {
        if (ay | ax) == 0 {
            let i: u32 = (uy >> 31) * 4 + (ux >> 31) * 2;
            return OFF[i as usize];
        }
        if (ux >> 31) == 0 {
            return 0.0 * SGNF[(uy >> 31) as usize];
        }
    }
    if ax == ay {
        static S: [f32; 4] = [0.25, 0.75, -0.25, -0.75];
        let i = (uy >> 31) * 2 + (ux >> 31);
        return S[i as usize];
    }
    let gt: usize = (ay > ax) as usize;
    let i: u32 = (uy >> 31) * 4 + (ux >> 31) * 2 + gt as u32;

    let zx = x as f64;
    let zy = y as f64;
    static M: [f64; 2] = [0., 1.];

    let mut z = f_fmla(M[gt], zx, M[1 - gt] * zy) / f_fmla(M[gt], zy, M[1 - gt] * zx);

    const CN: [u64; 7] = [
        0x3fd45f306dc9c883,
        0x3fe988d83a142ada,
        0x3fe747bebf492057,
        0x3fd2cc5645094ff3,
        0x3faa0521c711ab66,
        0x3f6881b8058b9a0d,
        0x3efb16ff514a0af0,
    ];

    let mut r = f64::from_bits(CN[0]);
    let z2 = z * z;
    z *= SGN[gt];
    // avoid spurious underflow in the polynomial evaluation excluding tiny arguments
    if z2 > f64::from_bits(0x3c90000000000000) {
        let z4 = z2 * z2;
        let z8 = z4 * z4;
        let mut cn0 = f_fmla(z2, f64::from_bits(CN[1]), r);
        let cn2 = f_fmla(z2, f64::from_bits(CN[3]), f64::from_bits(CN[2]));
        let mut cn4 = f_fmla(z2, f64::from_bits(CN[5]), f64::from_bits(CN[4]));
        let cn6 = f64::from_bits(CN[6]);
        cn0 += z4 * cn2;
        cn4 += z4 * cn6;
        cn0 += z8 * cn4;

        const CD: [u64; 7] = [
            0x3ff0000000000000,
            0x4006b8b143a3f6da,
            0x4008421201d18ed5,
            0x3ff8221d086914eb,
            0x3fd670657e3a07ba,
            0x3fa0f4951fd1e72d,
            0x3f4b3874b8798286,
        ];

        let mut cd0 = f_fmla(z2, f64::from_bits(CD[1]), f64::from_bits(CD[0]));
        let cd2 = f_fmla(z2, f64::from_bits(CD[3]), f64::from_bits(CD[2]));
        let mut cd4 = f_fmla(z2, f64::from_bits(CD[5]), f64::from_bits(CD[4]));
        let cd6 = f64::from_bits(CD[6]);
        cd0 += z4 * cd2;
        cd4 += z4 * cd6;
        cd0 += z8 * cd4;

        r = cn0 / cd0;
    }
    f_fmla(z, r, OFF[i as usize] as f64) as f32
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_atan2pif() {
        assert_eq!(f_atan2pif(0.32131, 0.987565), 0.10012555);
        assert_eq!(f_atan2pif(532.32131, 12.987565), 0.49223542);
        assert_eq!(f_atan2pif(-754.32131, 12.987565), -0.494520042);
    }
}
