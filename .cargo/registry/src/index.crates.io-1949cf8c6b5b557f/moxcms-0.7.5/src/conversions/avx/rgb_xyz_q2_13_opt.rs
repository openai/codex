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
use crate::conversions::avx::rgb_xyz::AvxAlignedU16;
use crate::conversions::avx::rgb_xyz_q2_13::_xmm_broadcast_epi32;
use crate::conversions::rgbxyz_fixed::TransformMatrixShaperFixedPointOpt;
use crate::transform::PointeeSizeExpressible;
use crate::{CmsError, Layout, TransformExecutor};
use num_traits::AsPrimitive;
use std::arch::x86_64::*;

pub(crate) struct TransformShaperRgbQ2_13OptAvx<
    T: Copy,
    const SRC_LAYOUT: u8,
    const DST_LAYOUT: u8,
    const LINEAR_CAP: usize,
    const GAMMA_LUT: usize,
    const PRECISION: i32,
> {
    pub(crate) profile: TransformMatrixShaperFixedPointOpt<i32, i16, T, LINEAR_CAP>,
    pub(crate) bit_depth: usize,
}

impl<
    T: Copy + PointeeSizeExpressible + 'static,
    const SRC_LAYOUT: u8,
    const DST_LAYOUT: u8,
    const LINEAR_CAP: usize,
    const GAMMA_LUT: usize,
    const PRECISION: i32,
> TransformShaperRgbQ2_13OptAvx<T, SRC_LAYOUT, DST_LAYOUT, LINEAR_CAP, GAMMA_LUT, PRECISION>
where
    u32: AsPrimitive<T>,
{
    #[target_feature(enable = "avx2")]
    unsafe fn transform_avx2(&self, src: &[T], dst: &mut [T]) -> Result<(), CmsError> {
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

        let max_colors = ((1 << self.bit_depth) - 1).as_();

        unsafe {
            let m0 = _mm256_setr_epi16(
                t.v[0][0], t.v[1][0], t.v[0][1], t.v[1][1], t.v[0][2], t.v[1][2], 0, 0, t.v[0][0],
                t.v[1][0], t.v[0][1], t.v[1][1], t.v[0][2], t.v[1][2], 0, 0,
            );
            let m2 = _mm256_setr_epi16(
                t.v[2][0], 1, t.v[2][1], 1, t.v[2][2], 1, 0, 0, t.v[2][0], 1, t.v[2][1], 1,
                t.v[2][2], 1, 0, 0,
            );

            let rnd_val = ((1i32 << (PRECISION - 1)) as i16).to_ne_bytes();
            let rnd = _mm256_set1_epi32(i32::from_ne_bytes([0, 0, rnd_val[0], rnd_val[1]]));

            let zeros = _mm256_setzero_si256();

            let v_max_value = _mm256_set1_epi32(GAMMA_LUT as i32 - 1);

            let (mut r0, mut g0, mut b0, mut a0);
            let (mut r1, mut g1, mut b1, mut a1);

            let mut src_iter = src.chunks_exact(src_channels * 2);

            if let Some(src0) = src_iter.next() {
                r0 = _xmm_broadcast_epi32(&self.profile.linear[src0[src_cn.r_i()]._as_usize()]);
                g0 = _xmm_broadcast_epi32(&self.profile.linear[src0[src_cn.g_i()]._as_usize()]);
                b0 = _xmm_broadcast_epi32(&self.profile.linear[src0[src_cn.b_i()]._as_usize()]);

                r1 = _xmm_broadcast_epi32(
                    &self.profile.linear[src0[src_cn.r_i() + src_channels]._as_usize()],
                );
                g1 = _xmm_broadcast_epi32(
                    &self.profile.linear[src0[src_cn.g_i() + src_channels]._as_usize()],
                );
                b1 = _xmm_broadcast_epi32(
                    &self.profile.linear[src0[src_cn.b_i() + src_channels]._as_usize()],
                );

                a0 = if src_channels == 4 {
                    src0[src_cn.a_i()]
                } else {
                    max_colors
                };
                a1 = if src_channels == 4 {
                    src0[src_cn.a_i() + src_channels]
                } else {
                    max_colors
                };
            } else {
                r0 = _mm_setzero_si128();
                g0 = _mm_setzero_si128();
                b0 = _mm_setzero_si128();
                a0 = max_colors;
                r1 = _mm_setzero_si128();
                g1 = _mm_setzero_si128();
                b1 = _mm_setzero_si128();
                a1 = max_colors;
            }

            for (src, dst) in src_iter.zip(dst.chunks_exact_mut(dst_channels * 2)) {
                let zr0 = _mm256_inserti128_si256::<1>(_mm256_castsi128_si256(r0), r1);
                let mut zg0 = _mm256_inserti128_si256::<1>(_mm256_castsi128_si256(g0), g1);
                let zb0 = _mm256_inserti128_si256::<1>(_mm256_castsi128_si256(b0), b1);
                zg0 = _mm256_slli_epi32::<16>(zg0);

                let zrg0 = _mm256_or_si256(zr0, zg0);
                let zbz0 = _mm256_or_si256(zb0, rnd);

                let va0 = _mm256_madd_epi16(zrg0, m0);
                let va1 = _mm256_madd_epi16(zbz0, m2);

                let mut v0 = _mm256_add_epi32(va0, va1);

                v0 = _mm256_srai_epi32::<PRECISION>(v0);
                v0 = _mm256_max_epi32(v0, zeros);
                v0 = _mm256_min_epi32(v0, v_max_value);

                _mm256_store_si256(temporary0.0.as_mut_ptr() as *mut _, v0);

                r0 = _xmm_broadcast_epi32(&self.profile.linear[src[src_cn.r_i()]._as_usize()]);
                g0 = _xmm_broadcast_epi32(&self.profile.linear[src[src_cn.g_i()]._as_usize()]);
                b0 = _xmm_broadcast_epi32(&self.profile.linear[src[src_cn.b_i()]._as_usize()]);

                r1 = _xmm_broadcast_epi32(
                    &self.profile.linear[src[src_cn.r_i() + src_channels]._as_usize()],
                );
                g1 = _xmm_broadcast_epi32(
                    &self.profile.linear[src[src_cn.g_i() + src_channels]._as_usize()],
                );
                b1 = _xmm_broadcast_epi32(
                    &self.profile.linear[src[src_cn.b_i() + src_channels]._as_usize()],
                );

                dst[dst_cn.r_i()] = self.profile.gamma[temporary0.0[0] as usize];
                dst[dst_cn.g_i()] = self.profile.gamma[temporary0.0[2] as usize];
                dst[dst_cn.b_i()] = self.profile.gamma[temporary0.0[4] as usize];
                if dst_channels == 4 {
                    dst[dst_cn.a_i()] = a0;
                }

                dst[dst_cn.r_i() + dst_channels] = self.profile.gamma[temporary0.0[8] as usize];
                dst[dst_cn.g_i() + dst_channels] = self.profile.gamma[temporary0.0[10] as usize];
                dst[dst_cn.b_i() + dst_channels] = self.profile.gamma[temporary0.0[12] as usize];
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
                let zr0 = _mm256_inserti128_si256::<1>(_mm256_castsi128_si256(r0), r1);
                let mut zg0 = _mm256_inserti128_si256::<1>(_mm256_castsi128_si256(g0), g1);
                let zb0 = _mm256_inserti128_si256::<1>(_mm256_castsi128_si256(b0), b1);
                zg0 = _mm256_slli_epi32::<16>(zg0);

                let zrg0 = _mm256_or_si256(zr0, zg0);
                let zbz0 = _mm256_or_si256(zb0, rnd);

                let va0 = _mm256_madd_epi16(zrg0, m0);
                let va1 = _mm256_madd_epi16(zbz0, m2);

                let mut v0 = _mm256_add_epi32(va0, va1);

                v0 = _mm256_srai_epi32::<PRECISION>(v0);
                v0 = _mm256_max_epi32(v0, zeros);
                v0 = _mm256_min_epi32(v0, v_max_value);

                _mm256_store_si256(temporary0.0.as_mut_ptr() as *mut _, v0);

                dst[dst_cn.r_i()] = self.profile.gamma[temporary0.0[0] as usize];
                dst[dst_cn.g_i()] = self.profile.gamma[temporary0.0[2] as usize];
                dst[dst_cn.b_i()] = self.profile.gamma[temporary0.0[4] as usize];
                if dst_channels == 4 {
                    dst[dst_cn.a_i()] = a0;
                }

                dst[dst_cn.r_i() + dst_channels] = self.profile.gamma[temporary0.0[8] as usize];
                dst[dst_cn.g_i() + dst_channels] = self.profile.gamma[temporary0.0[10] as usize];
                dst[dst_cn.b_i() + dst_channels] = self.profile.gamma[temporary0.0[12] as usize];
                if dst_channels == 4 {
                    dst[dst_cn.a_i() + dst_channels] = a1;
                }
            }

            let src = src.chunks_exact(src_channels * 2).remainder();
            let dst = dst.chunks_exact_mut(dst_channels * 2).into_remainder();

            for (src, dst) in src
                .chunks_exact(src_channels)
                .zip(dst.chunks_exact_mut(dst_channels))
            {
                let r = _xmm_broadcast_epi32(&self.profile.linear[src[src_cn.r_i()]._as_usize()]);
                let mut g =
                    _xmm_broadcast_epi32(&self.profile.linear[src[src_cn.g_i()]._as_usize()]);
                let b = _xmm_broadcast_epi32(&self.profile.linear[src[src_cn.b_i()]._as_usize()]);

                g = _mm_slli_epi32::<16>(g);

                let a = if src_channels == 4 {
                    src[src_cn.a_i()]
                } else {
                    max_colors
                };

                let zrg0 = _mm_or_si128(r, g);
                let zbz0 = _mm_or_si128(b, _mm256_castsi256_si128(rnd));

                let v0 = _mm_madd_epi16(zrg0, _mm256_castsi256_si128(m0));
                let v1 = _mm_madd_epi16(zbz0, _mm256_castsi256_si128(m2));

                let mut v = _mm_add_epi32(v0, v1);

                v = _mm_srai_epi32::<PRECISION>(v);
                v = _mm_max_epi32(v, _mm_setzero_si128());
                v = _mm_min_epi32(v, _mm256_castsi256_si128(v_max_value));

                _mm_store_si128(temporary0.0.as_mut_ptr() as *mut _, v);

                dst[dst_cn.r_i()] = self.profile.gamma[temporary0.0[0] as usize];
                dst[dst_cn.g_i()] = self.profile.gamma[temporary0.0[2] as usize];
                dst[dst_cn.b_i()] = self.profile.gamma[temporary0.0[4] as usize];
                if dst_channels == 4 {
                    dst[dst_cn.a_i()] = a;
                }
            }
        }

        Ok(())
    }
}

impl<
    T: Copy + PointeeSizeExpressible + 'static + Default,
    const SRC_LAYOUT: u8,
    const DST_LAYOUT: u8,
    const LINEAR_CAP: usize,
    const GAMMA_LUT: usize,
    const PRECISION: i32,
> TransformExecutor<T>
    for TransformShaperRgbQ2_13OptAvx<T, SRC_LAYOUT, DST_LAYOUT, LINEAR_CAP, GAMMA_LUT, PRECISION>
where
    u32: AsPrimitive<T>,
{
    fn transform(&self, src: &[T], dst: &mut [T]) -> Result<(), CmsError> {
        unsafe { self.transform_avx2(src, dst) }
    }
}
