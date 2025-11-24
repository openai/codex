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

/// Computes 1/sqrt(x)
///
/// Max ULP 0.5
#[inline]
pub fn f_rsqrtf(x: f32) -> f32 {
    let ix = x.to_bits();

    // filter out exceptional cases
    if ix >= 0xffu32 << 23 || ix == 0 {
        if ix.wrapping_shl(1) == 0 {
            return 1.0 / x; // +/-0
        }
        if (ix >> 31) != 0 {
            return f32::NAN;
        }
        if ix.wrapping_shl(9) == 0 {
            return 0.;
        }
        return x + x; // nan
    }

    let dx = x as f64;
    ((1. / dx) * dx.sqrt()) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rsqrtf() {
        assert_eq!(f_rsqrtf(0.0), f32::INFINITY);
        assert_eq!(f_rsqrtf(4.0), 0.5);
        assert_eq!(f_rsqrtf(9.0), 1. / 3.);
        assert_eq!(f_rsqrtf(-0.0), f32::NEG_INFINITY);
        assert!(f_rsqrtf(f32::NAN).is_nan());
    }
}
