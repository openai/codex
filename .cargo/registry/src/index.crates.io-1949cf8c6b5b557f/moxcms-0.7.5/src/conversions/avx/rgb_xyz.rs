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
use std::arch::x86_64::*;

#[repr(align(32), C)]
#[derive(Debug)]
pub(crate) struct AvxAlignedU16(pub(crate) [u16; 16]);

pub(crate) struct TransformShaperRgbAvx<
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
> TransformShaperRgbAvx<T, SRC_LAYOUT, DST_LAYOUT, LINEAR_CAP, GAMMA_LUT>
where
    u32: AsPrimitive<T>,
{
    #[inline(always)]
    unsafe fn transform_impl<const FMA: bool>(
        &self,
        src: &[T],
        dst: &mut [T],
    ) -> Result<(), CmsError> {
        let src_cn = Layout::from(SRC_LAYOUT);
        let dst_cn = Layout::from(DST_LAYOUT);
        let src_channels = src_cn.channels();
        let dst_channels = dst_cn.channels();

        let mut temporary0 = AvxAlignedU16([0; 16]);

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
            let m0 = _mm256_setr_ps(
                t.v[0][0], t.v[0][1], t.v[0][2], 0., t.v[0][0], t.v[0][1], t.v[0][2], 0.,
            );
            let m1 = _mm256_setr_ps(
                t.v[1][0], t.v[1][1], t.v[1][2], 0., t.v[1][0], t.v[1][1], t.v[1][2], 0.,
            );
            let m2 = _mm256_setr_ps(
                t.v[2][0], t.v[2][1], t.v[2][2], 0., t.v[2][0], t.v[2][1], t.v[2][2], 0.,
            );

            let zeros = _mm_setzero_ps();

            let v_scale = _mm256_set1_ps(scale);

            let mut src = src;
            let mut dst = dst;

            let mut src_iter = src.chunks_exact(src_channels * 2);
            let dst_iter = dst.chunks_exact_mut(dst_channels * 2);

            let (mut r0, mut g0, mut b0, mut a0);
            let (mut r1, mut g1, mut b1, mut a1);

            if let Some(src) = src_iter.next() {
                r0 = _mm_broadcast_ss(&self.profile.r_linear[src[src_cn.r_i()]._as_usize()]);
                g0 = _mm_broadcast_ss(&self.profile.g_linear[src[src_cn.g_i()]._as_usize()]);
                b0 = _mm_broadcast_ss(&self.profile.b_linear[src[src_cn.b_i()]._as_usize()]);
                r1 = _mm_broadcast_ss(
                    &self.profile.r_linear[src[src_cn.r_i() + src_channels]._as_usize()],
                );
                g1 = _mm_broadcast_ss(
                    &self.profile.g_linear[src[src_cn.g_i() + src_channels]._as_usize()],
                );
                b1 = _mm_broadcast_ss(
                    &self.profile.b_linear[src[src_cn.b_i() + src_channels]._as_usize()],
                );
                a0 = if src_channels == 4 {
                    src[src_cn.a_i()]
                } else {
                    max_colors
                };
                a1 = if src_channels == 4 {
                    src[src_cn.a_i() + src_channels]
                } else {
                    max_colors
                };
            } else {
                r0 = _mm_setzero_ps();
                g0 = _mm_setzero_ps();
                b0 = _mm_setzero_ps();
                a0 = max_colors;
                r1 = _mm_setzero_ps();
                g1 = _mm_setzero_ps();
                b1 = _mm_setzero_ps();
                a1 = max_colors;
            }

            for (src, dst) in src_iter.zip(dst_iter) {
                let r = _mm256_insertf128_ps::<1>(_mm256_castps128_ps256(r0), r1);
                let g = _mm256_insertf128_ps::<1>(_mm256_castps128_ps256(g0), g1);
                let b = _mm256_insertf128_ps::<1>(_mm256_castps128_ps256(b0), b1);

                let mut v = if FMA {
                    let v0 = _mm256_mul_ps(r, m0);
                    let v1 = _mm256_fmadd_ps(g, m1, v0);
                    _mm256_fmadd_ps(b, m2, v1)
                } else {
                    let v0 = _mm256_mul_ps(r, m0);
                    let v1 = _mm256_mul_ps(g, m1);
                    let v2 = _mm256_mul_ps(b, m2);

                    _mm256_add_ps(_mm256_add_ps(v0, v1), v2)
                };

                v = _mm256_max_ps(v, _mm256_setzero_ps());
                v = _mm256_mul_ps(v, v_scale);
                v = _mm256_min_ps(v, v_scale);

                let zx = _mm256_cvtps_epi32(v);
                _mm256_store_si256(temporary0.0.as_mut_ptr() as *mut _, zx);

                r0 = _mm_broadcast_ss(&self.profile.r_linear[src[src_cn.r_i()]._as_usize()]);
                g0 = _mm_broadcast_ss(&self.profile.g_linear[src[src_cn.g_i()]._as_usize()]);
                b0 = _mm_broadcast_ss(&self.profile.b_linear[src[src_cn.b_i()]._as_usize()]);
                r1 = _mm_broadcast_ss(
                    &self.profile.r_linear[src[src_cn.r_i() + src_channels]._as_usize()],
                );
                g1 = _mm_broadcast_ss(
                    &self.profile.g_linear[src[src_cn.g_i() + src_channels]._as_usize()],
                );
                b1 = _mm_broadcast_ss(
                    &self.profile.b_linear[src[src_cn.b_i() + src_channels]._as_usize()],
                );

                dst[dst_cn.r_i()] = self.profile.r_gamma[temporary0.0[0] as usize];
                dst[dst_cn.g_i()] = self.profile.g_gamma[temporary0.0[2] as usize];
                dst[dst_cn.b_i()] = self.profile.b_gamma[temporary0.0[4] as usize];
                if dst_channels == 4 {
                    dst[dst_cn.a_i()] = a0;
                }

                dst[dst_cn.r_i() + dst_channels] = self.profile.r_gamma[temporary0.0[8] as usize];
                dst[dst_cn.g_i() + dst_channels] = self.profile.g_gamma[temporary0.0[10] as usize];
                dst[dst_cn.b_i() + dst_channels] = self.profile.b_gamma[temporary0.0[12] as usize];
                if dst_channels == 4 {
                    dst[dst_cn.a_i() + dst_channels] = a1;
                }

                a0 = if src_channels == 4 {
                    src[src_cn.a_i()]
                } else {
                    max_colors
                };
                a1 = if src_channels == 4 {
                    src[src_cn.a_i() + src_channels]
                } else {
                    max_colors
                };
            }

            if let Some(dst) = dst.chunks_exact_mut(dst_channels * 2).last() {
                let r = _mm256_insertf128_ps::<1>(_mm256_castps128_ps256(r0), r1);
                let g = _mm256_insertf128_ps::<1>(_mm256_castps128_ps256(g0), g1);
                let b = _mm256_insertf128_ps::<1>(_mm256_castps128_ps256(b0), b1);

                let mut v = if FMA {
                    let v0 = _mm256_mul_ps(r, m0);
                    let v1 = _mm256_fmadd_ps(g, m1, v0);
                    _mm256_fmadd_ps(b, m2, v1)
                } else {
                    let v0 = _mm256_mul_ps(r, m0);
                    let v1 = _mm256_mul_ps(g, m1);
                    let v2 = _mm256_mul_ps(b, m2);

                    _mm256_add_ps(_mm256_add_ps(v0, v1), v2)
                };

                v = _mm256_max_ps(v, _mm256_setzero_ps());
                v = _mm256_mul_ps(v, v_scale);
                v = _mm256_min_ps(v, v_scale);

                let zx = _mm256_cvtps_epi32(v);
                _mm256_store_si256(temporary0.0.as_mut_ptr() as *mut _, zx);

                dst[dst_cn.r_i()] = self.profile.r_gamma[temporary0.0[0] as usize];
                dst[dst_cn.g_i()] = self.profile.g_gamma[temporary0.0[2] as usize];
                dst[dst_cn.b_i()] = self.profile.b_gamma[temporary0.0[4] as usize];
                if dst_channels == 4 {
                    dst[dst_cn.a_i()] = a0;
                }

                dst[dst_cn.r_i() + dst_channels] = self.profile.r_gamma[temporary0.0[8] as usize];
                dst[dst_cn.g_i() + dst_channels] = self.profile.g_gamma[temporary0.0[10] as usize];
                dst[dst_cn.b_i() + dst_channels] = self.profile.b_gamma[temporary0.0[12] as usize];
                if dst_channels == 4 {
                    dst[dst_cn.a_i() + dst_channels] = a1;
                }
            }

            src = src.chunks_exact(src_channels * 2).remainder();
            dst = dst.chunks_exact_mut(dst_channels * 2).into_remainder();

            for (src, dst) in src
                .chunks_exact(src_channels)
                .zip(dst.chunks_exact_mut(dst_channels))
            {
                let r = _mm_broadcast_ss(&self.profile.r_linear[src[src_cn.r_i()]._as_usize()]);
                let g = _mm_broadcast_ss(&self.profile.g_linear[src[src_cn.g_i()]._as_usize()]);
                let b = _mm_broadcast_ss(&self.profile.b_linear[src[src_cn.b_i()]._as_usize()]);
                let a = if src_channels == 4 {
                    src[src_cn.a_i()]
                } else {
                    max_colors
                };

                let mut v = if FMA {
                    let v0 = _mm_mul_ps(r, _mm256_castps256_ps128(m0));
                    let v1 = _mm_fmadd_ps(g, _mm256_castps256_ps128(m1), v0);
                    _mm_fmadd_ps(b, _mm256_castps256_ps128(m2), v1)
                } else {
                    let v0 = _mm_mul_ps(r, _mm256_castps256_ps128(m0));
                    let v1 = _mm_mul_ps(g, _mm256_castps256_ps128(m1));
                    let v2 = _mm_mul_ps(b, _mm256_castps256_ps128(m2));

                    _mm_add_ps(_mm_add_ps(v0, v1), v2)
                };

                v = _mm_max_ps(v, zeros);
                v = _mm_mul_ps(v, _mm256_castps256_ps128(v_scale));
                v = _mm_min_ps(v, _mm256_castps256_ps128(v_scale));

                let zx = _mm_cvtps_epi32(v);
                _mm_store_si128(temporary0.0.as_mut_ptr() as *mut _, zx);

                dst[dst_cn.r_i()] = self.profile.r_gamma[temporary0.0[0] as usize];
                dst[dst_cn.g_i()] = self.profile.g_gamma[temporary0.0[2] as usize];
                dst[dst_cn.b_i()] = self.profile.b_gamma[temporary0.0[4] as usize];
                if dst_channels == 4 {
                    dst[dst_cn.a_i()] = a;
                }
            }
        }

        Ok(())
    }

    #[target_feature(enable = "avx2", enable = "fma")]
    unsafe fn transform_fma(&self, src: &[T], dst: &mut [T]) -> Result<(), CmsError> {
        unsafe { self.transform_impl::<true>(src, dst) }
    }

    #[target_feature(enable = "avx2")]
    unsafe fn transform_avx(&self, src: &[T], dst: &mut [T]) -> Result<(), CmsError> {
        unsafe { self.transform_impl::<false>(src, dst) }
    }
}

impl<
    T: Clone + Copy + 'static + PointeeSizeExpressible + Default,
    const SRC_LAYOUT: u8,
    const DST_LAYOUT: u8,
    const LINEAR_CAP: usize,
    const GAMMA_LUT: usize,
> TransformExecutor<T> for TransformShaperRgbAvx<T, SRC_LAYOUT, DST_LAYOUT, LINEAR_CAP, GAMMA_LUT>
where
    u32: AsPrimitive<T>,
{
    fn transform(&self, src: &[T], dst: &mut [T]) -> Result<(), CmsError> {
        unsafe {
            if std::arch::is_x86_feature_detected!("fma") {
                self.transform_fma(src, dst)
            } else {
                self.transform_avx(src, dst)
            }
        }
    }
}
