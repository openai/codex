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

pub(crate) static ASINCOSF_PI_TABLE: [[u64; 8]; 16] = [
    [
        0x3fd45f306dc9c882,
        0x3fab2995e7b7dc2f,
        0x3f98723a1cf50c7e,
        0x3f8d1a4591d16a29,
        0x3f83ce3aa68ddaee,
        0x3f7d3182ab0cc1bf,
        0x3f762b379a8b88e3,
        0x3f76811411fcfec2,
    ],
    [
        0x3fdffffffffd3cda,
        0xbfb17cc1b3355fdd,
        0x3f9d067a1e8d5a99,
        0xbf908e16fb09314a,
        0x3f85eed43d42dcb2,
        0xbf7f58baca7acc71,
        0x3f75dab64e2dcf15,
        0xbf659270e30797ac,
    ],
    [
        0x3fdfffffff7c4617,
        0xbfb17cc149ded3a2,
        0x3f9d0654d4cb2c1a,
        0xbf908c3ba713d33a,
        0x3f85d2053481079c,
        0xbf7e485ebc545e7e,
        0x3f7303baca167ddd,
        0xbf5dee8d16d06b38,
    ],
    [
        0x3fdffffffa749848,
        0xbfb17cbe71559350,
        0x3f9d05a312269adf,
        0xbf90862b3ee617d7,
        0x3f85920708db2a73,
        0xbf7cb0463b3862c3,
        0x3f702b82478f95d7,
        0xbf552a7b8579e729,
    ],
    [
        0x3fdfffffe1f92bb5,
        0xbfb17cb3e74c64e3,
        0x3f9d03af67311cbf,
        0xbf9079441cbfc7a0,
        0x3f852b4287805a61,
        0xbf7ac3286d604a98,
        0x3f6b2f1210d9701b,
        0xbf4e740ddc25afd6,
    ],
    [
        0x3fdfffff92beb6e2,
        0xbfb17c986fe9518b,
        0x3f9cff98167c9a5e,
        0xbf90638b591eae52,
        0x3f84a0803828959e,
        0xbf78adeca229f11d,
        0x3f66b9a7ba05dfce,
        0xbf4640521a43b2d0,
    ],
    [
        0x3fdffffeccee5bfc,
        0xbfb17c5f1753f5ea,
        0x3f9cf874e4fe258f,
        0xbf9043e6cf77b256,
        0x3f83f7db42227d92,
        0xbf7691a6fa2a2882,
        0x3f62f6543162bc61,
        0xbf407d5da05822b6,
    ],
    [
        0x3fdffffd2f64431d,
        0xbfb17bf8208c10c1,
        0x3f9ced7487cdb124,
        0xbf901a0d30932905,
        0x3f83388f99b254da,
        0xbf74844e245c65bd,
        0x3f5fa777150197c6,
        0xbf38c1ecf16a05c8,
    ],
    [
        0x3fdffffa36d1712e,
        0xbfb17b523971bd4e,
        0x3f9cddee26de2dee,
        0xbf8fccb00abaaabc,
        0x3f8269afc3622342,
        0xbf72933152686752,
        0x3f5a76d4956cc9a3,
        0xbf32ce7d6dc651ce,
    ],
    [
        0x3fdffff5402ab3a1,
        0xbfb17a5ba85da77a,
        0x3f9cc96894e05c02,
        0xbf8f532143cb832e,
        0x3f819180b660ff09,
        0xbf70c57417a78b3c,
        0x3f562e26cbd7bb1e,
        0xbf2ce28d33fe1df3,
    ],
    [
        0x3fdfffed8d639751,
        0xbfb1790349f3ae76,
        0x3f9caf9a4fd1b398,
        0xbf8ec986b111342e,
        0x3f80b53c3ad4baa4,
        0xbf6e3c2282eeace4,
        0x3f52a55369f55bbe,
        0xbf2667fe48c396e8,
    ],
    [
        0x3fdfffe24b714161,
        0xbfb177394fbcb719,
        0x3f9c90652d920ebd,
        0xbf8e3239197bddf1,
        0x3f7fb2188525b025,
        0xbf6b3aadd451afc7,
        0x3f4f74020f31fdab,
        0xbf218b0cb246768d,
    ],
    [
        0x3fdfffd298bec9e2,
        0xbfb174efbfd34648,
        0x3f9c6bcfe48ea92b,
        0xbf8d8f9f2a16157c,
        0x3f7e0044f56c8864,
        0xbf6883e2347fe76c,
        0x3f4a9f0e3c1b7af5,
        0xbf1bb5acc0e60825,
    ],
    [
        0x3fdfffbd8b784c4d,
        0xbfb1721abdd3722e,
        0x3f9c41fee756d4b0,
        0xbf8ce40bccf8065f,
        0x3f7c59b684b70ef9,
        0xbf66133d027996b3,
        0x3f469cad01106397,
        0xbf160f8e45494156,
    ],
    [
        0x3fdfffa23749cf88,
        0xbfb16eb0a8285c06,
        0x3f9c132d762e1b0d,
        0xbf8c31a959398f4e,
        0x3f7ac1c5b46bc8a0,
        0xbf63e34f1abe51dc,
        0x3f4346738737c0b9,
        0xbf11b227a3f5c750,
    ],
    [
        0x3fdfff7fb25bb407,
        0xbfb16aaa14d75640,
        0x3f9bdfa75fca5ff2,
        0xbf8b7a6e260d079c,
        0x3f793ab06911033c,
        0xbf61ee5560967fd5,
        0x3f407d31060838bf,
        0xbf0c96f33a283115,
    ],
];

/// Computes asin(x)/PI
///
/// Max ULP 0.5
#[inline]
pub fn f_asinpif(x: f32) -> f32 {
    let ax = x.abs();
    let az = ax as f64;
    let z = x as f64;
    let t = x.to_bits();
    let e: i32 = ((t >> 23) & 0xff) as i32;
    if e >= 127 {
        // |x| >= 1 or nan
        if ax == 1.0 {
            return f32::copysign(0.5, x);
        } // |x| = 1
        if e == 0xff && (t.wrapping_shl(9)) != 0 {
            return x + x;
        } // x = nan
        return f32::NAN; // |x| > 1
    }
    let s: i32 = 146i32.wrapping_sub(e);
    let mut i = 0i32;
    // s<32 corresponds to |x| >= 2^-12
    if s < 32 {
        i = (((t & 0x007fffff) | 1 << 23) >> s) as i32;
    }
    let z2 = z * z;
    let z4 = z2 * z2;
    let c = ASINCOSF_PI_TABLE[i as usize & 15];
    if i == 0 {
        // |x| < 2^-4
        let mut c0 = f_fmla(z2, f64::from_bits(c[1]), f64::from_bits(c[0]));
        let c2 = f_fmla(z2, f64::from_bits(c[3]), f64::from_bits(c[2]));
        let mut c4 = f_fmla(z2, f64::from_bits(c[5]), f64::from_bits(c[4]));
        let c6 = f_fmla(z2, f64::from_bits(c[7]), f64::from_bits(c[6]));
        c0 = f_fmla(c2, z4, c0);
        c4 = f_fmla(c6, z4, c4);
        c0 += c4 * (z4 * z4);
        (z * c0) as f32
    } else {
        // |x| >= 2^-4
        let f = (1. - az).sqrt();
        let mut c0 = f_fmla(az, f64::from_bits(c[1]), f64::from_bits(c[0]));
        let c2 = f_fmla(az, f64::from_bits(c[3]), f64::from_bits(c[2]));
        let mut c4 = f_fmla(az, f64::from_bits(c[5]), f64::from_bits(c[4]));
        let c6 = f_fmla(az, f64::from_bits(c[7]), f64::from_bits(c[6]));
        c0 = f_fmla(c2, z2, c0);
        c4 = f_fmla(c6, z2, c4);
        c0 += c4 * z4;
        let r = f_fmla(
            -c0,
            f64::copysign(f, x as f64),
            f64::copysign(0.5, x as f64),
        );
        r as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_asinpif() {
        assert_eq!(f_asinpif(0.0), 0.);
        assert_eq!(f_asinpif(0.5), 0.16666667);
        assert!(f_asinpif(1.5).is_nan());
    }
}
