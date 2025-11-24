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
#![allow(dead_code)]
use crate::PointeeSizeExpressible;
use crate::matan::is_curve_descending;
use num_traits::AsPrimitive;

pub(crate) fn limit_slope<T: Copy + AsPrimitive<f32> + PartialOrd + PointeeSizeExpressible>(
    curve: &mut [T],
    value_cap: f32,
) where
    f32: AsPrimitive<T>,
{
    let at_begin = (curve.len() as f32 * 0.02 + 0.5).floor() as usize; // Cutoff at 2%
    if at_begin == 0 {
        return;
    }
    let at_end = curve.len() - at_begin - 1; // And 98%
    let (begin_val, end_val) = if is_curve_descending(curve) {
        (value_cap, 0.)
    } else {
        (0., value_cap)
    };
    let val = curve[at_begin].as_();
    let slope = (val - begin_val) / at_begin as f32;
    let beta = val - slope * at_begin as f32;
    if T::FINITE {
        for v in curve.iter_mut().take(at_begin) {
            *v = (v.as_() * slope + beta)
                .round()
                .min(value_cap)
                .max(0.0)
                .as_();
        }
    } else {
        for v in curve.iter_mut().take(at_begin) {
            *v = (v.as_() * slope + beta).min(value_cap).max(0.0).as_();
        }
    }

    let val = curve[at_end].as_();
    let slope = (end_val - val) / at_begin as f32;
    let beta = val - slope * at_end as f32;

    if T::FINITE {
        for v in curve.iter_mut().skip(at_end) {
            *v = (v.as_() * slope + beta)
                .round()
                .min(value_cap)
                .max(0.0)
                .as_();
        }
    } else {
        for v in curve.iter_mut().skip(at_end) {
            *v = (v.as_() * slope + beta).min(value_cap).max(0.0).as_();
        }
    }
}
