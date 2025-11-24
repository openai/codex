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
use num_traits::AsPrimitive;

pub(crate) trait DiscontinuitySpike {
    const SPIKE: f64;
}

impl DiscontinuitySpike for u8 {
    const SPIKE: f64 = 16.0;
}

impl DiscontinuitySpike for u16 {
    const SPIKE: f64 = 2100.;
}

impl DiscontinuitySpike for f32 {
    const SPIKE: f64 = 0.07;
}

/// Searches LUT curve for discontinuity
pub(crate) fn does_curve_have_discontinuity<
    T: Copy + PartialEq + DiscontinuitySpike + AsPrimitive<f64> + 'static,
>(
    curve: &[T],
) -> bool {
    if curve.len() < 2 {
        return false;
    }
    let threshold: f64 = T::SPIKE;
    let mut discontinuities = 0u64;
    let mut previous_element: f64 = curve[0].as_();
    let diff: f64 = (curve[1].as_() - previous_element).abs();
    if diff > threshold {
        discontinuities += 1;
    }
    for element in curve.iter().skip(1) {
        let new_diff: f64 = (element.as_() - previous_element).abs();
        if new_diff > threshold {
            discontinuities += 1;
            if discontinuities > 3 {
                break;
            }
        }
        previous_element = element.as_();
    }
    discontinuities > 3
}
