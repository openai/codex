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
use crate::common::{f_fmla, f_fmlaf};

/// Hyperbolic tangent
///
/// Max found ULP 0.4999994
#[inline]
pub fn f_tanhf(x: f32) -> f32 {
    let z = x as f64;
    let t = x.to_bits();
    let ux = t;
    let e = ux.wrapping_shr(23) & 0xff;
    if e == 0xff {
        if ux << 9 != 0 {
            return x + x;
        } // x = nan
        const IR: [f32; 2] = [1.0, -1.0];
        return IR[ux.wrapping_shr(31) as usize]; // x = +-inf
    }
    if e < 115 {
        // |x| < 2^-13
        if e < 102 {
            // |x| < 2^-26
            if ux.wrapping_shl(1) == 0 {
                return x;
            }
            let res = f_fmlaf(-x, x.abs(), x);
            return res;
        }
        let x2 = x * x;
        return f_fmlaf(x, -f64::from_bits(0x3fd5555560000000) as f32 * x2, x);
    }
    if ux.wrapping_shl(1) > (0x41102cb3u32 << 1) {
        return f32::copysign(1.0, x) - f32::copysign(f64::from_bits(0x3e60000000000000) as f32, x);
    }
    let z2 = z * z;
    let z4 = z2 * z2;
    let z8 = z4 * z4;
    const CN: [u64; 8] = [
        0x3ff0000000000000,
        0x3fc30877b8b72d33,
        0x3f7694aa09ae9e5e,
        0x3f14101377abb729,
        0x3e9e0392b1db0018,
        0x3e12533756e546f7,
        0x3d6d62e5abe6ae8a,
        0x3c9b06be534182de,
    ];
    const CD: [u64; 8] = [
        0x3ff0000000000000,
        0x3fded99131b0ebea,
        0x3fa0d27ed6c95a69,
        0x3f47cbdaca0e9fcc,
        0x3edb4e60b892578e,
        0x3e5a6f707c5c71ab,
        0x3dc35a8b6e2cd94c,
        0x3d0ca8230677aa01,
    ];
    let mut n0 = f_fmla(z2, f64::from_bits(CN[1]), f64::from_bits(CN[0]));
    let n2 = f_fmla(z2, f64::from_bits(CN[3]), f64::from_bits(CN[2]));
    let mut n4 = f_fmla(z2, f64::from_bits(CN[5]), f64::from_bits(CN[4]));
    let n6 = f_fmla(z2, f64::from_bits(CN[7]), f64::from_bits(CN[6]));
    n0 = f_fmla(z4, n2, n0);
    n4 = f_fmla(z4, n6, n4);
    n0 = f_fmla(z8, n4, n0);
    let mut d0 = f_fmla(z2, f64::from_bits(CD[1]), f64::from_bits(CD[0]));
    let d2 = f_fmla(z2, f64::from_bits(CD[3]), f64::from_bits(CD[2]));
    let mut d4 = f_fmla(z2, f64::from_bits(CD[5]), f64::from_bits(CD[4]));
    let d6 = f_fmla(z2, f64::from_bits(CD[7]), f64::from_bits(CD[6]));
    d0 = f_fmla(z4, d2, d0);
    d4 = f_fmla(z4, d6, d4);
    d0 = f_fmla(z8, d4, d0);
    let r = z * n0 / d0;
    r as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tanhf() {
        assert_eq!(f_tanhf(-0.5), -0.46211717);
        assert_eq!(f_tanhf(0.5), 0.46211717);
        assert_eq!(f_tanhf(7.), 0.99999833);
    }
}
