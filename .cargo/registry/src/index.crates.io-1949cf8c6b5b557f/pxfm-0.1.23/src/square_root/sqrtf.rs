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

/// Computes Square root.
/// Most of CPU have built-in instruction with higher precision,
/// prefer use this only for const contexts.
#[inline]
pub const fn sqrtf(d: f32) -> f32 {
    let mut q = 0.5f32;

    let mut d = if d < 0f32 { f32::NAN } else { d };

    if d < 5.2939559203393770e-23f32 {
        d *= 1.8889465931478580e+22f32;
        q = 7.2759576141834260e-12f32;
    }

    if d > 1.8446744073709552e+19f32 {
        d *= 5.4210108624275220e-20f32;
        q = 4294967296.0f32;
    }

    // http://en.wikipedia.org/wiki/Fast_inverse_square_root
    let mut x = f32::from_bits(0x5f375a86 - ((d + 1e-45).to_bits() >> 1)) as f64;

    x = x * (1.5 - 0.5 * (d as f64) * x * x);
    x = x * (1.5 - 0.5 * (d as f64) * x * x);
    x = x * (1.5 - 0.5 * (d as f64) * x * x) * (d as f64);

    let d2 = ((d as f64) + x * x) * (1. / x);

    if d.is_infinite() {
        return f32::INFINITY;
    }
    (d2 * q as f64) as f32
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn sqrtf_test() {
        assert!(
            (sqrtf(4f32) - 2f32).abs() < 1e-6,
            "Invalid result {}",
            sqrtf(4f32)
        );
        assert!(
            (sqrtf(9f32) - 3f32).abs() < 1e-6,
            "Invalid result {}",
            sqrtf(9f32)
        );
        assert_eq!(sqrtf(4.), 2.);
        assert_eq!(sqrtf(9.), 3.);
        assert_eq!(sqrtf(16.), 4.);
        assert_eq!(sqrtf(25.), 5.);
    }
}
