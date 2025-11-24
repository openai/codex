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
use crate::common::f_fmla;

// // y1 = y0 * (2+x*y0^3)/(1+2*x*y0^3)
// #[inline(always)]
// fn halley_refine_d(x: f64, a: f64) -> f64 {
//     let tx = x * x * x;
//     x * f_fmla(tx, a, 2.0) / f_fmla(2. * a, tx, 1.0)
// }

#[inline(always)]
fn rapshon_refine_inv_cbrt(x: f64, a: f64) -> f64 {
    x * f_fmla(-1. / 3. * a, x * x * x, 4. / 3.)
}

// y1 = y0(k1 − c(k2 − k3c), c = x*y0*y0*y0
// k1 = 14/9 , k2 = 7/9 , k3 = 2/9
#[inline(always)]
fn halleys_div_free(x: f64, a: f64) -> f64 {
    const K3: f64 = 2. / 9.;
    const K2: f64 = 7. / 9.;
    const K1: f64 = 14. / 9.;
    let c = a * x * x * x;
    let mut y = f_fmla(-K3, c, K2);
    y = f_fmla(-c, y, K1);
    y * x
}

/// Computes 1/cbrt(x)
///
/// ULP 0.5
#[inline]
pub fn f_rcbrtf(x: f32) -> f32 {
    let u = x.to_bits();
    let au = u.wrapping_shl(1);
    if au < (1u32 << 24) || au >= (0xffu32 << 24) {
        if x.is_infinite() {
            return if x.is_sign_negative() { -0.0 } else { 0.0 };
        }
        if au >= (0xffu32 << 24) {
            return x + x; /* inf, nan */
        }
        if x == 0. {
            return if x.is_sign_positive() {
                f32::INFINITY
            } else {
                f32::NEG_INFINITY
            }; /* +-inf */
        }
    }

    let mut ui: u32 = x.to_bits();
    let mut hx: u32 = ui & 0x7fffffff;

    if hx < 0x00800000 {
        /* zero or subnormal? */
        if hx == 0 {
            return x; /* cbrt(+-0) is itself */
        }
        const TWO_EXP_24: f32 = f32::from_bits(0x4b800000);
        ui = (x * TWO_EXP_24).to_bits();
        hx = ui & 0x7fffffff;
        const B: u32 = 0x54a21d2au32 + (8u32 << 23);
        hx = B.wrapping_sub(hx / 3);
    } else {
        hx = 0x54a21d2au32.wrapping_sub(hx / 3);
    }
    ui &= 0x80000000;
    ui |= hx;

    let t = f32::from_bits(ui) as f64;
    let dx = x as f64;
    let mut t = halleys_div_free(t, dx);
    t = halleys_div_free(t, dx);
    t = rapshon_refine_inv_cbrt(t, dx);
    t as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fcbrtf() {
        assert_eq!(f_rcbrtf(0.0), f32::INFINITY);
        assert_eq!(f_rcbrtf(-0.0), f32::NEG_INFINITY);
        assert_eq!(f_rcbrtf(-27.0), -1. / 3.);
        assert_eq!(f_rcbrtf(27.0), 1. / 3.);
        assert_eq!(f_rcbrtf(64.0), 0.25);
        assert_eq!(f_rcbrtf(-64.0), -0.25);
        assert_eq!(f_rcbrtf(f32::NEG_INFINITY), -0.0);
        assert_eq!(f_rcbrtf(f32::INFINITY), 0.0);
        assert!(f_rcbrtf(f32::NAN).is_nan());
    }
}
