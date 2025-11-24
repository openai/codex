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

pub(crate) fn is_curve_linear16(curve: &[u16]) -> bool {
    let scale = 1. / (curve.len() - 1) as f32 * 65535.;
    for (index, &value) in curve.iter().enumerate() {
        let quantized = (index as f32 * scale).round() as u16;
        let diff = (quantized as i32 - value as i32).abs();
        if diff > 0x0f {
            return false;
        }
    }
    true
}

pub(crate) fn is_curve_descending<T: PartialOrd>(v: &[T]) -> bool {
    if v.is_empty() {
        return false;
    }
    if v.len() == 1 {
        return false;
    }
    v[0] > v[v.len() - 1]
}

pub(crate) fn is_curve_ascending<T: PartialOrd>(v: &[T]) -> bool {
    if v.is_empty() {
        return false;
    }
    if v.len() == 1 {
        return false;
    }
    v[0] < v[v.len() - 1]
}

pub(crate) fn is_curve_linear8(curve: &[u8]) -> bool {
    let scale = 1. / (curve.len() - 1) as f32 * 255.;
    for (index, &value) in curve.iter().enumerate() {
        let quantized = (index as f32 * scale).round() as u16;
        let diff = (quantized as i32 - value as i32).abs();
        if diff > 0x03 {
            return false;
        }
    }
    true
}
