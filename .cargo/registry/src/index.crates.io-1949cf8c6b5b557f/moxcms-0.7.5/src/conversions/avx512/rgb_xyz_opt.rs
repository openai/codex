/*
 * // Copyright (c) Radzivon Bartoshyk 5/2025. All rights reserved.
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
use crate::conversions::TransformMatrixShaperOptimized;
use crate::conversions::avx512::rgb_xyz_q2_13_opt::{
    AvxAlignedU16, split_by_twos, split_by_twos_mut,
};
use crate::transform::PointeeSizeExpressible;
use crate::{CmsError, Layout, TransformExecutor};
use num_traits::AsPrimitive;
use std::arch::x86_64::*;

pub(crate) struct TransformShaperRgbOptAvx512<
    T: Clone + Copy + 'static + PointeeSizeExpressible + Default,
    const SRC_LAYOUT: u8,
    const DST_LAYOUT: u8,
    const LINEAR_CAP: usize,
    const GAMMA_LUT: usize,
> {
    pub(crate) profile: TransformMatrixShaperOptimized<T, LINEAR_CAP>,
    pub(crate) bit_depth: usize,
}

impl<
    T: Clone + Copy + 'static + PointeeSizeExpressible + Default,
    const SRC_LAYOUT: u8,
    const DST_LAYOUT: u8,
    const LINEAR_CAP: usize,
    const GAMMA_LUT: usize,
> TransformShaperRgbOptAvx512<T, SRC_LAYOUT, DST_LAYOUT, LINEAR_CAP, GAMMA_LUT>
where
    u32: AsPrimitive<T>,
{
    #[target_feature(enable = "avx512bw", enable = "avx512vl", enable = "fma")]
    unsafe fn transform_impl(&self, src: &[T], dst: &mut [T]) -> Result<(), CmsError> {
        let src_cn = Layout::from(SRC_LAYOUT);
        let dst_cn = Layout::from(DST_LAYOUT);
        let src_channels = src_cn.channels();
        let dst_channels = dst_cn.channels();

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

        let (src_chunks, src_remainder) = split_by_twos(src, src_channels);
        let (dst_chunks, dst_remainder) = split_by_twos_mut(dst, dst_channels);

        let mut temporary0 = AvxAlignedU16([0; 16]);
        let mut temporary1 = AvxAlignedU16([0; 16]);

        unsafe {
            let m0 = _mm256_setr_ps(
                t.v[0][0], t.v[0][1], t.v[0][2], 0f32, t.v[0][0], t.v[0][1], t.v[0][2], 0f32,
            );
            let m1 = _mm256_setr_ps(
                t.v[1][0], t.v[1][1], t.v[1][2], 0f32, t.v[1][0], t.v[1][1], t.v[1][2], 0f32,
            );
            let m2 = _mm256_setr_ps(
                t.v[2][0], t.v[2][1], t.v[2][2], 0f32, t.v[2][0], t.v[2][1], t.v[2][2], 0f32,
            );

            let zeros = _mm_setzero_ps();

            let v_scale = _mm256_set1_ps(scale);

            if !src_chunks.is_empty() {
                let (src0, src1) = src_chunks.split_at(src_chunks.len() / 2);
                let (dst0, dst1) = dst_chunks.split_at_mut(dst_chunks.len() / 2);
                let mut src_iter0 = src0.chunks_exact(src_channels * 2);
                let mut src_iter1 = src1.chunks_exact(src_channels * 2);

                let (mut r0, mut g0, mut b0, mut a0);
                let (mut r1, mut g1, mut b1, mut a1);
                let (mut r2, mut g2, mut b2, mut a2);
                let (mut r3, mut g3, mut b3, mut a3);

                if let (Some(src0), Some(src1)) = (src_iter0.next(), src_iter1.next()) {
                    r0 = _mm_broadcast_ss(&self.profile.linear[src0[src_cn.r_i()]._as_usize()]);
                    g0 = _mm_broadcast_ss(&self.profile.linear[src0[src_cn.g_i()]._as_usize()]);
                    b0 = _mm_broadcast_ss(&self.profile.linear[src0[src_cn.b_i()]._as_usize()]);

                    r1 = _mm_broadcast_ss(
                        &self.profile.linear[src0[src_cn.r_i() + src_channels]._as_usize()],
                    );
                    g1 = _mm_broadcast_ss(
                        &self.profile.linear[src0[src_cn.g_i() + src_channels]._as_usize()],
                    );
                    b1 = _mm_broadcast_ss(
                        &self.profile.linear[src0[src_cn.b_i() + src_channels]._as_usize()],
                    );

                    r2 = _mm_broadcast_ss(&self.profile.linear[src1[src_cn.r_i()]._as_usize()]);
                    g2 = _mm_broadcast_ss(&self.profile.linear[src1[src_cn.g_i()]._as_usize()]);
                    b2 = _mm_broadcast_ss(&self.profile.linear[src1[src_cn.b_i()]._as_usize()]);

                    r3 = _mm_broadcast_ss(
                        &self.profile.linear[src1[src_cn.r_i() + src_channels]._as_usize()],
                    );
                    g3 = _mm_broadcast_ss(
                        &self.profile.linear[src1[src_cn.g_i() + src_channels]._as_usize()],
                    );
                    b3 = _mm_broadcast_ss(
                        &self.profile.linear[src1[src_cn.b_i() + src_channels]._as_usize()],
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

                    a2 = if src_channels == 4 {
                        src1[src_cn.a_i()]
                    } else {
                        max_colors
                    };
                    a3 = if src_channels == 4 {
                        src1[src_cn.a_i() + src_channels]
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
                    r2 = _mm_setzero_ps();
                    g2 = _mm_setzero_ps();
                    b2 = _mm_setzero_ps();
                    a2 = max_colors;
                    r3 = _mm_setzero_ps();
                    g3 = _mm_setzero_ps();
                    b3 = _mm_setzero_ps();
                    a3 = max_colors;
                }

                for (((src0, src1), dst0), dst1) in src_iter0
                    .zip(src_iter1)
                    .zip(dst0.chunks_exact_mut(dst_channels * 2))
                    .zip(dst1.chunks_exact_mut(dst_channels * 2))
                {
                    let rz0 = _mm256_insertf128_ps::<1>(_mm256_castps128_ps256(r0), r1);
                    let gz0 = _mm256_insertf128_ps::<1>(_mm256_castps128_ps256(g0), g1);
                    let bz0 = _mm256_insertf128_ps::<1>(_mm256_castps128_ps256(b0), b1);

                    let rz1 = _mm256_insertf128_ps::<1>(_mm256_castps128_ps256(r2), r3);
                    let gz1 = _mm256_insertf128_ps::<1>(_mm256_castps128_ps256(g2), g3);
                    let bz1 = _mm256_insertf128_ps::<1>(_mm256_castps128_ps256(b2), b3);

                    let v0 = _mm256_mul_ps(rz0, m0);
                    let v1 = _mm256_fmadd_ps(gz0, m1, v0);
                    let mut vz0 = _mm256_fmadd_ps(bz0, m2, v1);

                    let v2 = _mm256_mul_ps(rz1, m0);
                    let v3 = _mm256_fmadd_ps(gz1, m1, v2);
                    let mut vz1 = _mm256_fmadd_ps(bz1, m2, v3);

                    vz0 = _mm256_max_ps(vz0, _mm256_setzero_ps());
                    vz0 = _mm256_mul_ps(vz0, v_scale);
                    vz0 = _mm256_min_ps(vz0, v_scale);

                    vz1 = _mm256_max_ps(vz1, _mm256_setzero_ps());
                    vz1 = _mm256_mul_ps(vz1, v_scale);
                    vz1 = _mm256_min_ps(vz1, v_scale);

                    let zx0 = _mm256_cvtps_epi32(vz0);
                    let zx1 = _mm256_cvtps_epi32(vz1);
                    _mm256_store_si256(temporary0.0.as_mut_ptr() as *mut _, zx0);
                    _mm256_store_si256(temporary1.0.as_mut_ptr() as *mut _, zx1);

                    r0 = _mm_broadcast_ss(&self.profile.linear[src0[src_cn.r_i()]._as_usize()]);
                    g0 = _mm_broadcast_ss(&self.profile.linear[src0[src_cn.g_i()]._as_usize()]);
                    b0 = _mm_broadcast_ss(&self.profile.linear[src0[src_cn.b_i()]._as_usize()]);
                    r1 = _mm_broadcast_ss(
                        &self.profile.linear[src0[src_cn.r_i() + src_channels]._as_usize()],
                    );
                    g1 = _mm_broadcast_ss(
                        &self.profile.linear[src0[src_cn.g_i() + src_channels]._as_usize()],
                    );
                    b1 = _mm_broadcast_ss(
                        &self.profile.linear[src0[src_cn.b_i() + src_channels]._as_usize()],
                    );

                    r2 = _mm_broadcast_ss(&self.profile.linear[src1[src_cn.r_i()]._as_usize()]);
                    g2 = _mm_broadcast_ss(&self.profile.linear[src1[src_cn.g_i()]._as_usize()]);
                    b2 = _mm_broadcast_ss(&self.profile.linear[src1[src_cn.b_i()]._as_usize()]);

                    r3 = _mm_broadcast_ss(
                        &self.profile.linear[src1[src_cn.r_i() + src_channels]._as_usize()],
                    );
                    g3 = _mm_broadcast_ss(
                        &self.profile.linear[src1[src_cn.g_i() + src_channels]._as_usize()],
                    );
                    b3 = _mm_broadcast_ss(
                        &self.profile.linear[src1[src_cn.b_i() + src_channels]._as_usize()],
                    );

                    dst0[dst_cn.r_i()] = self.profile.gamma[temporary0.0[0] as usize];
                    dst0[dst_cn.g_i()] = self.profile.gamma[temporary0.0[2] as usize];
                    dst0[dst_cn.b_i()] = self.profile.gamma[temporary0.0[4] as usize];
                    if dst_channels == 4 {
                        dst0[dst_cn.a_i()] = a0;
                    }

                    dst0[dst_cn.r_i() + dst_channels] =
                        self.profile.gamma[temporary0.0[8] as usize];
                    dst0[dst_cn.g_i() + dst_channels] =
                        self.profile.gamma[temporary0.0[10] as usize];
                    dst0[dst_cn.b_i() + dst_channels] =
                        self.profile.gamma[temporary0.0[12] as usize];
                    if dst_channels == 4 {
                        dst0[dst_cn.a_i() + dst_channels] = a1;
                    }

                    dst1[dst_cn.r_i()] = self.profile.gamma[temporary1.0[0] as usize];
                    dst1[dst_cn.g_i()] = self.profile.gamma[temporary1.0[2] as usize];
                    dst1[dst_cn.b_i()] = self.profile.gamma[temporary1.0[4] as usize];
                    if dst_channels == 4 {
                        dst1[dst_cn.a_i()] = a2;
                    }

                    dst1[dst_cn.r_i() + dst_channels] =
                        self.profile.gamma[temporary1.0[8] as usize];
                    dst1[dst_cn.g_i() + dst_channels] =
                        self.profile.gamma[temporary1.0[10] as usize];
                    dst1[dst_cn.b_i() + dst_channels] =
                        self.profile.gamma[temporary1.0[12] as usize];
                    if dst_channels == 4 {
                        dst1[dst_cn.a_i() + dst_channels] = a3;
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

                    a2 = if src_channels == 4 {
                        src1[src_cn.a_i()]
                    } else {
                        max_colors
                    };
                    a3 = if src_channels == 4 {
                        src1[src_cn.a_i() + src_channels]
                    } else {
                        max_colors
                    };
                }

                if let (Some(dst0), Some(dst1)) = (
                    dst0.chunks_exact_mut(dst_channels * 2).last(),
                    dst1.chunks_exact_mut(dst_channels * 2).last(),
                ) {
                    let rz0 = _mm256_insertf128_ps::<1>(_mm256_castps128_ps256(r0), r1);
                    let gz0 = _mm256_insertf128_ps::<1>(_mm256_castps128_ps256(g0), g1);
                    let bz0 = _mm256_insertf128_ps::<1>(_mm256_castps128_ps256(b0), b1);

                    let rz1 = _mm256_insertf128_ps::<1>(_mm256_castps128_ps256(r2), r3);
                    let gz1 = _mm256_insertf128_ps::<1>(_mm256_castps128_ps256(g2), g3);
                    let bz1 = _mm256_insertf128_ps::<1>(_mm256_castps128_ps256(b2), b3);

                    let v0 = _mm256_mul_ps(rz0, m0);
                    let v1 = _mm256_fmadd_ps(gz0, m1, v0);
                    let mut vz0 = _mm256_fmadd_ps(bz0, m2, v1);

                    let v2 = _mm256_mul_ps(rz1, m0);
                    let v3 = _mm256_fmadd_ps(gz1, m1, v2);
                    let mut vz1 = _mm256_fmadd_ps(bz1, m2, v3);

                    vz0 = _mm256_max_ps(vz0, _mm256_setzero_ps());
                    vz0 = _mm256_mul_ps(vz0, v_scale);
                    vz0 = _mm256_min_ps(vz0, v_scale);

                    vz1 = _mm256_max_ps(vz1, _mm256_setzero_ps());
                    vz1 = _mm256_mul_ps(vz1, v_scale);
                    vz1 = _mm256_min_ps(vz1, v_scale);

                    let zx0 = _mm256_cvtps_epi32(vz0);
                    let zx1 = _mm256_cvtps_epi32(vz1);
                    _mm256_store_si256(temporary0.0.as_mut_ptr() as *mut _, zx0);
                    _mm256_store_si256(temporary1.0.as_mut_ptr() as *mut _, zx1);

                    dst0[dst_cn.r_i()] = self.profile.gamma[temporary0.0[0] as usize];
                    dst0[dst_cn.g_i()] = self.profile.gamma[temporary0.0[2] as usize];
                    dst0[dst_cn.b_i()] = self.profile.gamma[temporary0.0[4] as usize];
                    if dst_channels == 4 {
                        dst0[dst_cn.a_i()] = a0;
                    }

                    dst0[dst_cn.r_i() + dst_channels] =
                        self.profile.gamma[temporary0.0[8] as usize];
                    dst0[dst_cn.g_i() + dst_channels] =
                        self.profile.gamma[temporary0.0[10] as usize];
                    dst0[dst_cn.b_i() + dst_channels] =
                        self.profile.gamma[temporary0.0[12] as usize];
                    if dst_channels == 4 {
                        dst0[dst_cn.a_i() + dst_channels] = a1;
                    }

                    dst1[dst_cn.r_i()] = self.profile.gamma[temporary1.0[0] as usize];
                    dst1[dst_cn.g_i()] = self.profile.gamma[temporary1.0[2] as usize];
                    dst1[dst_cn.b_i()] = self.profile.gamma[temporary1.0[4] as usize];
                    if dst_channels == 4 {
                        dst1[dst_cn.a_i()] = a2;
                    }

                    dst1[dst_cn.r_i() + dst_channels] =
                        self.profile.gamma[temporary1.0[8] as usize];
                    dst1[dst_cn.g_i() + dst_channels] =
                        self.profile.gamma[temporary1.0[10] as usize];
                    dst1[dst_cn.b_i() + dst_channels] =
                        self.profile.gamma[temporary1.0[12] as usize];
                    if dst_channels == 4 {
                        dst1[dst_cn.a_i() + dst_channels] = a3;
                    }
                }
            }

            for (src, dst) in src_remainder
                .chunks_exact(src_channels)
                .zip(dst_remainder.chunks_exact_mut(dst_channels))
            {
                let r = _mm_broadcast_ss(&self.profile.linear[src[src_cn.r_i()]._as_usize()]);
                let g = _mm_broadcast_ss(&self.profile.linear[src[src_cn.g_i()]._as_usize()]);
                let b = _mm_broadcast_ss(&self.profile.linear[src[src_cn.b_i()]._as_usize()]);
                let a = if src_channels == 4 {
                    src[src_cn.a_i()]
                } else {
                    max_colors
                };

                let v0 = _mm_mul_ps(r, _mm256_castps256_ps128(m0));
                let v1 = _mm_fmadd_ps(g, _mm256_castps256_ps128(m1), v0);
                let mut v = _mm_fmadd_ps(b, _mm256_castps256_ps128(m2), v1);

                v = _mm_max_ps(v, zeros);
                v = _mm_mul_ps(v, _mm256_castps256_ps128(v_scale));
                v = _mm_min_ps(v, _mm256_castps256_ps128(v_scale));

                let zx = _mm_cvtps_epi32(v);
                _mm_store_si128(temporary0.0.as_mut_ptr() as *mut _, zx);

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
    T: Clone + Copy + 'static + PointeeSizeExpressible + Default,
    const SRC_LAYOUT: u8,
    const DST_LAYOUT: u8,
    const LINEAR_CAP: usize,
    const GAMMA_LUT: usize,
> TransformExecutor<T>
    for TransformShaperRgbOptAvx512<T, SRC_LAYOUT, DST_LAYOUT, LINEAR_CAP, GAMMA_LUT>
where
    u32: AsPrimitive<T>,
{
    fn transform(&self, src: &[T], dst: &mut [T]) -> Result<(), CmsError> {
        unsafe { self.transform_impl(src, dst) }
    }
}
