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
use crate::conversions::TransformMatrixShaper;
use crate::transform::PointeeSizeExpressible;
use crate::{CmsError, Layout, TransformExecutor};
use num_traits::AsPrimitive;
#[cfg(target_arch = "x86")]
use std::arch::x86::*;
#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

#[repr(align(16), C)]
pub(crate) struct SseAlignedU16(pub(crate) [u16; 8]);

pub(crate) struct TransformShaperRgbSse<
    T: Clone + Copy + 'static + PointeeSizeExpressible + Default,
    const SRC_LAYOUT: u8,
    const DST_LAYOUT: u8,
    const LINEAR_CAP: usize,
    const GAMMA_LUT: usize,
> {
    pub(crate) profile: TransformMatrixShaper<T, LINEAR_CAP>,
    pub(crate) bit_depth: usize,
}

impl<
    T: Clone + Copy + 'static + PointeeSizeExpressible + Default,
    const SRC_LAYOUT: u8,
    const DST_LAYOUT: u8,
    const LINEAR_CAP: usize,
    const GAMMA_LUT: usize,
> TransformShaperRgbSse<T, SRC_LAYOUT, DST_LAYOUT, LINEAR_CAP, GAMMA_LUT>
where
    u32: AsPrimitive<T>,
{
    #[target_feature(enable = "sse4.1")]
    unsafe fn transform_impl(&self, src: &[T], dst: &mut [T]) -> Result<(), CmsError> {
        let src_cn = Layout::from(SRC_LAYOUT);
        let dst_cn = Layout::from(DST_LAYOUT);
        let src_channels = src_cn.channels();
        let dst_channels = dst_cn.channels();

        let mut temporary = SseAlignedU16([0; 8]);

        if src.len() / src_channels != dst.len() / dst_channels {
            return Err(CmsError::LaneSizeMismatch);
        }
        if src.len() % src_channels != 0 {
            return Err(CmsError::LaneMultipleOfChannels);
        }
        if dst.len() % dst_channels != 0 {
            return Err(CmsError::LaneMultipleOfChannels);
        }

        let t = self.profile.adaptation_matrix.transpose();

        let scale = (GAMMA_LUT - 1) as f32;
        let max_colors: T = ((1 << self.bit_depth) - 1).as_();

        unsafe {
            let m0 = _mm_setr_ps(t.v[0][0], t.v[0][1], t.v[0][2], 0f32);
            let m1 = _mm_setr_ps(t.v[1][0], t.v[1][1], t.v[1][2], 0f32);
            let m2 = _mm_setr_ps(t.v[2][0], t.v[2][1], t.v[2][2], 0f32);

            let zeros = _mm_setzero_ps();

            let v_scale = _mm_set1_ps(scale);

            for (src, dst) in src
                .chunks_exact(src_channels)
                .zip(dst.chunks_exact_mut(dst_channels))
            {
                let rp = &self.profile.r_linear[src[src_cn.r_i()]._as_usize()];
                let gp = &self.profile.g_linear[src[src_cn.g_i()]._as_usize()];
                let bp = &self.profile.b_linear[src[src_cn.b_i()]._as_usize()];

                let mut r = _mm_load_ss(rp);
                let mut g = _mm_load_ss(gp);
                let mut b = _mm_load_ss(bp);
                let a = if src_channels == 4 {
                    src[src_cn.a_i()]
                } else {
                    max_colors
                };

                r = _mm_shuffle_ps::<0>(r, r);
                g = _mm_shuffle_ps::<0>(g, g);
                b = _mm_shuffle_ps::<0>(b, b);

                let v0 = _mm_mul_ps(r, m0);
                let v1 = _mm_mul_ps(g, m1);
                let v2 = _mm_mul_ps(b, m2);

                let mut v = _mm_add_ps(_mm_add_ps(v0, v1), v2);
                v = _mm_max_ps(v, zeros);
                v = _mm_mul_ps(v, v_scale);
                v = _mm_min_ps(v, v_scale);

                let zx = _mm_cvtps_epi32(v);
                _mm_store_si128(temporary.0.as_mut_ptr() as *mut _, zx);

                dst[dst_cn.r_i()] = self.profile.r_gamma[temporary.0[0] as usize];
                dst[dst_cn.g_i()] = self.profile.g_gamma[temporary.0[2] as usize];
                dst[dst_cn.b_i()] = self.profile.b_gamma[temporary.0[4] as usize];
                if dst_channels == 4 {
                    dst[dst_cn.a_i()] = a;
                }
            }
        }

        Ok(())
    }
}

impl<
    T: Clone + Copy + 'static + PointeeSizeExpressible + Default,
    const SRC_LAYOUT: u8,
    const DST_LAYOUT: u8,
    const LINEAR_CAP: usize,
    const GAMMA_LUT: usize,
> TransformExecutor<T> for TransformShaperRgbSse<T, SRC_LAYOUT, DST_LAYOUT, LINEAR_CAP, GAMMA_LUT>
where
    u32: AsPrimitive<T>,
{
    fn transform(&self, src: &[T], dst: &mut [T]) -> Result<(), CmsError> {
        unsafe { self.transform_impl(src, dst) }
    }
}
