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
use crate::asinpif::ASINCOSF_PI_TABLE;
use crate::common::{dd_fmla, f_fmla};

/// Computes acos(x)/PI
///
/// Max ULP 0.5
#[inline]
pub fn f_acospif(x: f32) -> f32 {
    let ax = x.abs();
    let az = ax as f64;
    let z = x as f64;
    let t: u32 = x.to_bits();
    let e: i32 = ((t >> 23) & 0xff) as i32;
    if e >= 127 {
        if x == 1.0 {
            return 0.0;
        }
        if x == -1.0 {
            return 1.0;
        }
        if e == 0xff && (t.wrapping_shl(9)) != 0 {
            return x + x;
        } // nan
        return f32::NAN;
    }
    let s: i32 = 146i32.wrapping_sub(e);
    let mut i = 0i32;
    if s < 32 {
        i = (((t & 0x007fffff) | 1 << 23) >> s) as i32;
    }
    let c = ASINCOSF_PI_TABLE[i as usize & 15];
    let z2 = z * z;
    let z4 = z2 * z2;
    if i == 0 {
        let mut c0 = f_fmla(z2, f64::from_bits(c[1]), f64::from_bits(c[0]));
        let c2 = f_fmla(z2, f64::from_bits(c[3]), f64::from_bits(c[2]));
        let mut c4 = f_fmla(z2, f64::from_bits(c[5]), f64::from_bits(c[4]));
        let c6 = f_fmla(z2, f64::from_bits(c[7]), f64::from_bits(c[6]));
        c0 += c2 * z4;
        c4 += c6 * z4;
        /* For |x| <= 0x1.0fd288p-127, c0 += c4*(z4*z4) would raise a spurious
        underflow exception, we use an FMA instead, where c4 * z4 does not
        underflow. */
        c0 = dd_fmla(c4 * z4, z4, c0);
        f_fmla(-z, c0, 0.5) as f32
    } else {
        let f = (1. - az).sqrt();
        let mut c0 = f_fmla(az, f64::from_bits(c[1]), f64::from_bits(c[0]));
        let c2 = f_fmla(az, f64::from_bits(c[3]), f64::from_bits(c[2]));
        let mut c4 = f_fmla(az, f64::from_bits(c[5]), f64::from_bits(c[4]));
        let c6 = f_fmla(az, f64::from_bits(c[7]), f64::from_bits(c[6]));
        c0 += c2 * z2;
        c4 += c6 * z2;
        c0 += c4 * z4;
        static SIGN: [f64; 2] = [0., 1.];
        let r = SIGN[(t >> 31) as usize] + c0 * f64::copysign(f, x as f64);
        r as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_acospif() {
        assert_eq!(f_acospif(0.0), 0.5);
        assert_eq!(f_acospif(0.5), 0.33333334);
        assert_eq!(f_acospif(1.0), 0.0);
    }
}
