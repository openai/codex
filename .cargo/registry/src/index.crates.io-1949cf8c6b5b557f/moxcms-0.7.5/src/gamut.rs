/*
 * // Copyright (c) Radzivon Bartoshyk 3/2025. All rights reserved.
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
use crate::Rgb;

#[inline]
fn filmlike_clip_rgb_tone(r: &mut f32, g: &mut f32, b: &mut f32, l: f32) {
    let new_r = r.min(l);
    let new_b = b.min(l);
    let new_g = new_b + ((new_r - new_b) * (*g - *b) / (*r - *b));
    *r = new_r;
    *g = new_g;
    *b = new_b;
}

/// Soft clipping out-of-bounds values in S-curve
///
/// Works only on highlights, negative values are skipped
#[inline]
pub fn filmlike_clip(rgb: Rgb<f32>) -> Rgb<f32> {
    const L: f32 = 1.;
    let mut rgb = rgb;
    if rgb.r >= rgb.g {
        if rgb.g > rgb.b {
            filmlike_clip_rgb_tone(&mut rgb.r, &mut rgb.g, &mut rgb.b, L);
        } else if rgb.b > rgb.r {
            filmlike_clip_rgb_tone(&mut rgb.b, &mut rgb.r, &mut rgb.g, L);
        } else if rgb.b > rgb.g {
            filmlike_clip_rgb_tone(&mut rgb.r, &mut rgb.b, &mut rgb.g, L);
        } else {
            Rgb::new(rgb.r.min(L), rgb.g.min(L), rgb.g);
        }
    } else if rgb.r >= rgb.b {
        filmlike_clip_rgb_tone(&mut rgb.g, &mut rgb.r, &mut rgb.b, L);
    } else if rgb.b > rgb.g {
        filmlike_clip_rgb_tone(&mut rgb.b, &mut rgb.g, &mut rgb.r, L);
    } else {
        filmlike_clip_rgb_tone(&mut rgb.g, &mut rgb.b, &mut rgb.r, L);
    }
    rgb
}
