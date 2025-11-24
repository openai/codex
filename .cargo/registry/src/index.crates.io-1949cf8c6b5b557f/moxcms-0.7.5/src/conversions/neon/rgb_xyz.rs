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
use crate::conversions::neon::rgb_xyz_q2_13::{split_by_twos, split_by_twos_mut};
use crate::transform::PointeeSizeExpressible;
use crate::{CmsError, Layout, TransformExecutor};
use num_traits::AsPrimitive;
use std::arch::aarch64::*;

#[repr(align(16), C)]
pub(crate) struct NeonAlignedU16(pub(crate) [u16; 8]);

#[repr(align(16), C)]
pub(crate) struct NeonAlignedF32(pub(crate) [f32; 4]);

pub(crate) struct TransformShaperRgbNeon<
    T: Clone + PointeeSizeExpressible + Copy + Default + 'static,
    const SRC_LAYOUT: u8,
    const DST_LAYOUT: u8,
    const LINEAR_CAP: usize,
    const GAMMA_LUT: usize,
> {
    pub(crate) profile: TransformMatrixShaper<T, LINEAR_CAP>,
    pub(crate) bit_depth: usize,
}

impl<
    T: Clone + PointeeSizeExpressible + Copy + Default + 'static,
    const SRC_LAYOUT: u8,
    const DST_LAYOUT: u8,
    const LINEAR_CAP: usize,
    const GAMMA_LUT: usize,
> TransformExecutor<T> for TransformShaperRgbNeon<T, SRC_LAYOUT, DST_LAYOUT, LINEAR_CAP, GAMMA_LUT>
where
    u32: AsPrimitive<T>,
{
    fn transform(&self, src: &[T], dst: &mut [T]) -> Result<(), CmsError> {
        let src_cn = Layout::from(SRC_LAYOUT);
        let dst_cn = Layout::from(DST_LAYOUT);
        let src_channels = src_cn.channels();
        let dst_channels = dst_cn.channels();

        let mut temporary0 = NeonAlignedU16([0; 8]);
        let mut temporary1 = NeonAlignedU16([0; 8]);
        let mut temporary2 = NeonAlignedU16([0; 8]);
        let mut temporary3 = NeonAlignedU16([0; 8]);

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

        unsafe {
            let m0 = vld1q_f32([t.v[0][0], t.v[0][1], t.v[0][2], 0.].as_ptr());
            let m1 = vld1q_f32([t.v[1][0], t.v[1][1], t.v[1][2], 0.].as_ptr());
            let m2 = vld1q_f32([t.v[2][0], t.v[2][1], t.v[2][2], 0.].as_ptr());

            let v_scale = vdupq_n_f32(scale);

            let rnd = vdupq_n_f32(0.5);

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
                    let r0p = &self.profile.r_linear[src0[src_cn.r_i()]._as_usize()];
                    let g0p = &self.profile.g_linear[src0[src_cn.g_i()]._as_usize()];
                    let b0p = &self.profile.b_linear[src0[src_cn.b_i()]._as_usize()];

                    let r1p = &self.profile.r_linear[src0[src_cn.r_i() + src_channels]._as_usize()];
                    let g1p = &self.profile.g_linear[src0[src_cn.g_i() + src_channels]._as_usize()];
                    let b1p = &self.profile.b_linear[src0[src_cn.b_i() + src_channels]._as_usize()];

                    let r2p = &self.profile.r_linear[src1[src_cn.r_i()]._as_usize()];
                    let g2p = &self.profile.g_linear[src1[src_cn.g_i()]._as_usize()];
                    let b2p = &self.profile.b_linear[src1[src_cn.b_i()]._as_usize()];

                    let r3p = &self.profile.r_linear[src1[src_cn.r_i() + src_channels]._as_usize()];
                    let g3p = &self.profile.g_linear[src1[src_cn.g_i() + src_channels]._as_usize()];
                    let b3p = &self.profile.b_linear[src1[src_cn.b_i() + src_channels]._as_usize()];

                    r0 = vld1q_dup_f32(r0p);
                    g0 = vld1q_dup_f32(g0p);
                    b0 = vld1q_dup_f32(b0p);

                    r1 = vld1q_dup_f32(r1p);
                    g1 = vld1q_dup_f32(g1p);
                    b1 = vld1q_dup_f32(b1p);

                    r2 = vld1q_dup_f32(r2p);
                    g2 = vld1q_dup_f32(g2p);
                    b2 = vld1q_dup_f32(b2p);

                    r3 = vld1q_dup_f32(r3p);
                    g3 = vld1q_dup_f32(g3p);
                    b3 = vld1q_dup_f32(b3p);

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
                    r0 = vdupq_n_f32(0.);
                    g0 = vdupq_n_f32(0.);
                    b0 = vdupq_n_f32(0.);
                    r1 = vdupq_n_f32(0.);
                    g1 = vdupq_n_f32(0.);
                    b1 = vdupq_n_f32(0.);
                    r2 = vdupq_n_f32(0.);
                    g2 = vdupq_n_f32(0.);
                    b2 = vdupq_n_f32(0.);
                    r3 = vdupq_n_f32(0.);
                    g3 = vdupq_n_f32(0.);
                    b3 = vdupq_n_f32(0.);
                    a0 = max_colors;
                    a1 = max_colors;
                    a2 = max_colors;
                    a3 = max_colors;
                }

                for (((src0, src1), dst0), dst1) in src_iter0
                    .zip(src_iter1)
                    .zip(dst0.chunks_exact_mut(dst_channels * 2))
                    .zip(dst1.chunks_exact_mut(dst_channels * 2))
                {
                    let v0_0 = vmulq_f32(r0, m0);
                    let v0_1 = vmulq_f32(r1, m0);
                    let v0_2 = vmulq_f32(r2, m0);
                    let v0_3 = vmulq_f32(r3, m0);

                    let v1_0 = vfmaq_f32(v0_0, g0, m1);
                    let v1_1 = vfmaq_f32(v0_1, g1, m1);
                    let v1_2 = vfmaq_f32(v0_2, g2, m1);
                    let v1_3 = vfmaq_f32(v0_3, g3, m1);

                    let mut vr0 = vfmaq_f32(v1_0, b0, m2);
                    let mut vr1 = vfmaq_f32(v1_1, b1, m2);
                    let mut vr2 = vfmaq_f32(v1_2, b2, m2);
                    let mut vr3 = vfmaq_f32(v1_3, b3, m2);

                    vr0 = vfmaq_f32(rnd, vr0, v_scale);
                    vr1 = vfmaq_f32(rnd, vr1, v_scale);
                    vr2 = vfmaq_f32(rnd, vr2, v_scale);
                    vr3 = vfmaq_f32(rnd, vr3, v_scale);

                    vr0 = vminq_f32(vr0, v_scale);
                    vr1 = vminq_f32(vr1, v_scale);
                    vr2 = vminq_f32(vr2, v_scale);
                    vr3 = vminq_f32(vr3, v_scale);

                    let zx0 = vcvtq_u32_f32(vr0);
                    let zx1 = vcvtq_u32_f32(vr1);
                    let zx2 = vcvtq_u32_f32(vr2);
                    let zx3 = vcvtq_u32_f32(vr3);
                    vst1q_u32(temporary0.0.as_mut_ptr() as *mut _, zx0);
                    vst1q_u32(temporary1.0.as_mut_ptr() as *mut _, zx1);
                    vst1q_u32(temporary2.0.as_mut_ptr() as *mut _, zx2);
                    vst1q_u32(temporary3.0.as_mut_ptr() as *mut _, zx3);

                    let r0p = &self.profile.r_linear[src0[src_cn.r_i()]._as_usize()];
                    let g0p = &self.profile.g_linear[src0[src_cn.g_i()]._as_usize()];
                    let b0p = &self.profile.b_linear[src0[src_cn.b_i()]._as_usize()];

                    let r1p = &self.profile.r_linear[src0[src_cn.r_i() + src_channels]._as_usize()];
                    let g1p = &self.profile.g_linear[src0[src_cn.g_i() + src_channels]._as_usize()];
                    let b1p = &self.profile.b_linear[src0[src_cn.b_i() + src_channels]._as_usize()];

                    let r2p = &self.profile.r_linear[src1[src_cn.r_i()]._as_usize()];
                    let g2p = &self.profile.g_linear[src1[src_cn.g_i()]._as_usize()];
                    let b2p = &self.profile.b_linear[src1[src_cn.b_i()]._as_usize()];

                    let r3p = &self.profile.r_linear[src1[src_cn.r_i() + src_channels]._as_usize()];
                    let g3p = &self.profile.g_linear[src1[src_cn.g_i() + src_channels]._as_usize()];
                    let b3p = &self.profile.b_linear[src1[src_cn.b_i() + src_channels]._as_usize()];

                    r0 = vld1q_dup_f32(r0p);
                    g0 = vld1q_dup_f32(g0p);
                    b0 = vld1q_dup_f32(b0p);

                    r1 = vld1q_dup_f32(r1p);
                    g1 = vld1q_dup_f32(g1p);
                    b1 = vld1q_dup_f32(b1p);

                    r2 = vld1q_dup_f32(r2p);
                    g2 = vld1q_dup_f32(g2p);
                    b2 = vld1q_dup_f32(b2p);

                    r3 = vld1q_dup_f32(r3p);
                    g3 = vld1q_dup_f32(g3p);
                    b3 = vld1q_dup_f32(b3p);

                    dst0[dst_cn.r_i()] = self.profile.r_gamma[temporary0.0[0] as usize];
                    dst0[dst_cn.g_i()] = self.profile.g_gamma[temporary0.0[2] as usize];
                    dst0[dst_cn.b_i()] = self.profile.b_gamma[temporary0.0[4] as usize];
                    if dst_channels == 4 {
                        dst0[dst_cn.a_i()] = a0;
                    }

                    dst0[dst_cn.r_i() + dst_channels] =
                        self.profile.r_gamma[temporary1.0[0] as usize];
                    dst0[dst_cn.g_i() + dst_channels] =
                        self.profile.g_gamma[temporary1.0[2] as usize];
                    dst0[dst_cn.b_i() + dst_channels] =
                        self.profile.b_gamma[temporary1.0[4] as usize];
                    if dst_channels == 4 {
                        dst0[dst_cn.a_i() + dst_channels] = a1;
                    }

                    dst1[dst_cn.r_i()] = self.profile.r_gamma[temporary2.0[0] as usize];
                    dst1[dst_cn.g_i()] = self.profile.g_gamma[temporary2.0[2] as usize];
                    dst1[dst_cn.b_i()] = self.profile.b_gamma[temporary2.0[4] as usize];
                    if dst_channels == 4 {
                        dst1[dst_cn.a_i()] = a2;
                    }

                    dst1[dst_cn.r_i() + dst_channels] =
                        self.profile.r_gamma[temporary3.0[0] as usize];
                    dst1[dst_cn.g_i() + dst_channels] =
                        self.profile.g_gamma[temporary3.0[2] as usize];
                    dst1[dst_cn.b_i() + dst_channels] =
                        self.profile.b_gamma[temporary3.0[4] as usize];
                    if dst_channels == 4 {
                        dst1[dst_cn.a_i() + dst_channels] = a3;
                    }

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
                }

                if let (Some(dst0), Some(dst1)) = (
                    dst0.chunks_exact_mut(dst_channels * 2).last(),
                    dst1.chunks_exact_mut(dst_channels * 2).last(),
                ) {
                    let v0_0 = vmulq_f32(r0, m0);
                    let v0_1 = vmulq_f32(r1, m0);
                    let v0_2 = vmulq_f32(r2, m0);
                    let v0_3 = vmulq_f32(r3, m0);

                    let v1_0 = vfmaq_f32(v0_0, g0, m1);
                    let v1_1 = vfmaq_f32(v0_1, g1, m1);
                    let v1_2 = vfmaq_f32(v0_2, g2, m1);
                    let v1_3 = vfmaq_f32(v0_3, g3, m1);

                    let mut vr0 = vfmaq_f32(v1_0, b0, m2);
                    let mut vr1 = vfmaq_f32(v1_1, b1, m2);
                    let mut vr2 = vfmaq_f32(v1_2, b2, m2);
                    let mut vr3 = vfmaq_f32(v1_3, b3, m2);

                    vr0 = vfmaq_f32(rnd, vr0, v_scale);
                    vr1 = vfmaq_f32(rnd, vr1, v_scale);
                    vr2 = vfmaq_f32(rnd, vr2, v_scale);
                    vr3 = vfmaq_f32(rnd, vr3, v_scale);

                    vr0 = vminq_f32(vr0, v_scale);
                    vr1 = vminq_f32(vr1, v_scale);
                    vr2 = vminq_f32(vr2, v_scale);
                    vr3 = vminq_f32(vr3, v_scale);

                    let zx0 = vcvtq_u32_f32(vr0);
                    let zx1 = vcvtq_u32_f32(vr1);
                    let zx2 = vcvtq_u32_f32(vr2);
                    let zx3 = vcvtq_u32_f32(vr3);
                    vst1q_u32(temporary0.0.as_mut_ptr() as *mut _, zx0);
                    vst1q_u32(temporary1.0.as_mut_ptr() as *mut _, zx1);
                    vst1q_u32(temporary2.0.as_mut_ptr() as *mut _, zx2);
                    vst1q_u32(temporary3.0.as_mut_ptr() as *mut _, zx3);

                    dst0[dst_cn.r_i()] = self.profile.r_gamma[temporary0.0[0] as usize];
                    dst0[dst_cn.g_i()] = self.profile.g_gamma[temporary0.0[2] as usize];
                    dst0[dst_cn.b_i()] = self.profile.b_gamma[temporary0.0[4] as usize];
                    if dst_channels == 4 {
                        dst0[dst_cn.a_i()] = a0;
                    }

                    dst0[dst_cn.r_i() + dst_channels] =
                        self.profile.r_gamma[temporary1.0[0] as usize];
                    dst0[dst_cn.g_i() + dst_channels] =
                        self.profile.g_gamma[temporary1.0[2] as usize];
                    dst0[dst_cn.b_i() + dst_channels] =
                        self.profile.b_gamma[temporary1.0[4] as usize];
                    if dst_channels == 4 {
                        dst0[dst_cn.a_i() + dst_channels] = a1;
                    }

                    dst1[dst_cn.r_i()] = self.profile.r_gamma[temporary2.0[0] as usize];
                    dst1[dst_cn.g_i()] = self.profile.g_gamma[temporary2.0[2] as usize];
                    dst1[dst_cn.b_i()] = self.profile.b_gamma[temporary2.0[4] as usize];
                    if dst_channels == 4 {
                        dst1[dst_cn.a_i()] = a2;
                    }

                    dst1[dst_cn.r_i() + dst_channels] =
                        self.profile.r_gamma[temporary3.0[0] as usize];
                    dst1[dst_cn.g_i() + dst_channels] =
                        self.profile.g_gamma[temporary3.0[2] as usize];
                    dst1[dst_cn.b_i() + dst_channels] =
                        self.profile.b_gamma[temporary3.0[4] as usize];
                    if dst_channels == 4 {
                        dst1[dst_cn.a_i() + dst_channels] = a3;
                    }
                }
            }

            for (src, dst) in src_remainder
                .chunks_exact(src_channels)
                .zip(dst_remainder.chunks_exact_mut(dst_channels))
            {
                let rp = &self.profile.r_linear[src[src_cn.r_i()]._as_usize()];
                let gp = &self.profile.g_linear[src[src_cn.g_i()]._as_usize()];
                let bp = &self.profile.b_linear[src[src_cn.b_i()]._as_usize()];
                let r = vld1q_dup_f32(rp);
                let g = vld1q_dup_f32(gp);
                let b = vld1q_dup_f32(bp);
                let a = if src_channels == 4 {
                    src[src_cn.a_i()]
                } else {
                    max_colors
                };

                let v0 = vmulq_f32(r, m0);
                let v1 = vfmaq_f32(v0, g, m1);
                let mut v = vfmaq_f32(v1, b, m2);

                v = vfmaq_f32(rnd, v, v_scale);
                v = vminq_f32(v, v_scale);

                let zx = vcvtq_u32_f32(v);
                vst1q_u32(temporary0.0.as_mut_ptr() as *mut _, zx);

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
}
