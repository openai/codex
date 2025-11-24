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
use crate::err::inverff::erfinv_core;

/// Complementary inverse error function
///
/// Max ulp 0.5
pub fn f_erfcinvf(x: f32) -> f32 {
    if !x.is_normal() {
        if x.is_nan() || x.is_infinite() {
            return f32::INFINITY;
        }
        if x == 0. {
            return f32::INFINITY;
        }
    }
    if x < 0. {
        return f32::NAN;
    }

    if x >= 1. {
        if x == 1. {
            return 0.;
        }
        if x >= 2. {
            // x > 2
            if x == 2. {
                return f32::NEG_INFINITY;
            }
            return f32::NAN;
        }
    }

    let z = x as f64;
    static SIGN: [f32; 2] = [1.0, -1.0];
    // inferfc(x) = -inverf(1-x)
    // ax doesn't need to be extremely accurate,
    // it's just boundary detection so will do subtraction in f32
    erfinv_core(1. - z, (1. - x).abs().to_bits(), SIGN[(x > 1.) as usize])
}

#[cfg(test)]
mod tests {
    use super::f_erfcinvf;
    #[test]
    fn m_test() {
        assert!(f_erfcinvf(-1.).is_nan());
        assert_eq!(f_erfcinvf(0.), f32::INFINITY);
        assert_eq!(f_erfcinvf(2.), f32::NEG_INFINITY);
        assert!(f_erfcinvf(2.1).is_nan());
        assert_eq!(f_erfcinvf(0.5), 0.47693628);
        assert_eq!(f_erfcinvf(1.5), -0.47693628);
        assert_eq!(f_erfcinvf(0.002), 2.1851242);
        assert_eq!(f_erfcinvf(1.002), -0.0017724329);
    }
}
