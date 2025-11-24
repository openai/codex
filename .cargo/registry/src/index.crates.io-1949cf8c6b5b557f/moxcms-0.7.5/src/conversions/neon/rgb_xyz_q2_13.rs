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
use crate::conversions::rgbxyz_fixed::TransformMatrixShaperFixedPoint;
use crate::transform::PointeeSizeExpressible;
use crate::{CmsError, Layout, TransformExecutor};
use num_traits::AsPrimitive;
use std::arch::aarch64::*;

#[allow(dead_code)]
#[inline]
pub(crate) fn split_by_twos<T: Copy>(data: &[T], channels: usize) -> (&[T], &[T]) {
    let len = data.len() / (channels * 4);
    let split_point = len * 4;
    data.split_at(split_point * channels)
}

#[allow(dead_code)]
#[inline]
pub(crate) fn split_by_twos_mut<T: Copy>(data: &mut [T], channels: usize) -> (&mut [T], &mut [T]) {
    let len = data.len() / (channels * 4);
    let split_point = len * 4;
    data.split_at_mut(split_point * channels)
}

pub(crate) struct TransformShaperQ2_13Neon<
    T: Copy,
    const SRC_LAYOUT: u8,
    const DST_LAYOUT: u8,
    const LINEAR_CAP: usize,
    const GAMMA_LUT: usize,
    const PRECISION: i32,
> {
    pub(crate) profile: TransformMatrixShaperFixedPoint<i16, T, LINEAR_CAP>,
    pub(crate) bit_depth: usize,
}

impl<
    T: Copy + PointeeSizeExpressible + 'static + Default,
    const SRC_LAYOUT: u8,
    const DST_LAYOUT: u8,
    const LINEAR_CAP: usize,
    const GAMMA_LUT: usize,
    const PRECISION: i32,
> TransformExecutor<T>
    for TransformShaperQ2_13Neon<T, SRC_LAYOUT, DST_LAYOUT, LINEAR_CAP, GAMMA_LUT, PRECISION>
where
    u32: AsPrimitive<T>,
{
    fn transform(&self, src: &[T], dst: &mut [T]) -> Result<(), CmsError> {
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
        let max_colors: T = ((1 << self.bit_depth) - 1).as_();

        let (src_chunks, src_remainder) = split_by_twos(src, src_channels);
        let (dst_chunks, dst_remainder) = split_by_twos_mut(dst, dst_channels);

        unsafe {
            let m0 = vld1_s16([t.v[0][0], t.v[0][1], t.v[0][2], 0].as_ptr());
            let m1 = vld1_s16([t.v[1][0], t.v[1][1], t.v[1][2], 0].as_ptr());
            let m2 = vld1_s16([t.v[2][0], t.v[2][1], t.v[2][2], 0].as_ptr());

            let v_max_value = vdup_n_u16((GAMMA_LUT - 1) as u16);

            let rnd = vdupq_n_s32(1 << (PRECISION - 1));

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

                    r0 = vld1_dup_s16(r0p);
                    g0 = vld1_dup_s16(g0p);
                    b0 = vld1_dup_s16(b0p);

                    r1 = vld1_dup_s16(r1p);
                    g1 = vld1_dup_s16(g1p);
                    b1 = vld1_dup_s16(b1p);

                    r2 = vld1_dup_s16(r2p);
                    g2 = vld1_dup_s16(g2p);
                    b2 = vld1_dup_s16(b2p);

                    r3 = vld1_dup_s16(r3p);
                    g3 = vld1_dup_s16(g3p);
                    b3 = vld1_dup_s16(b3p);

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
                    r0 = vdup_n_s16(0);
                    g0 = vdup_n_s16(0);
                    b0 = vdup_n_s16(0);
                    r1 = vdup_n_s16(0);
                    g1 = vdup_n_s16(0);
                    b1 = vdup_n_s16(0);
                    r2 = vdup_n_s16(0);
                    g2 = vdup_n_s16(0);
                    b2 = vdup_n_s16(0);
                    r3 = vdup_n_s16(0);
                    g3 = vdup_n_s16(0);
                    b3 = vdup_n_s16(0);
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
                    let v0_0 = vmlal_s16(rnd, r0, m0);
                    let v0_1 = vmlal_s16(rnd, r1, m0);
                    let v0_2 = vmlal_s16(rnd, r2, m0);
                    let v0_3 = vmlal_s16(rnd, r3, m0);

                    let v1_0 = vmlal_s16(v0_0, g0, m1);
                    let v1_1 = vmlal_s16(v0_1, g1, m1);
                    let v1_2 = vmlal_s16(v0_2, g2, m1);
                    let v1_3 = vmlal_s16(v0_3, g3, m1);

                    let vr0 = vmlal_s16(v1_0, b0, m2);
                    let vr1 = vmlal_s16(v1_1, b1, m2);
                    let vr2 = vmlal_s16(v1_2, b2, m2);
                    let vr3 = vmlal_s16(v1_3, b3, m2);

                    let mut vr0 = vqshrun_n_s32::<PRECISION>(vr0);
                    let mut vr1 = vqshrun_n_s32::<PRECISION>(vr1);
                    let mut vr2 = vqshrun_n_s32::<PRECISION>(vr2);
                    let mut vr3 = vqshrun_n_s32::<PRECISION>(vr3);

                    vr0 = vmin_u16(vr0, v_max_value);
                    vr1 = vmin_u16(vr1, v_max_value);
                    vr2 = vmin_u16(vr2, v_max_value);
                    vr3 = vmin_u16(vr3, v_max_value);

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

                    r0 = vld1_dup_s16(r0p);
                    g0 = vld1_dup_s16(g0p);
                    b0 = vld1_dup_s16(b0p);

                    r1 = vld1_dup_s16(r1p);
                    g1 = vld1_dup_s16(g1p);
                    b1 = vld1_dup_s16(b1p);

                    r2 = vld1_dup_s16(r2p);
                    g2 = vld1_dup_s16(g2p);
                    b2 = vld1_dup_s16(b2p);

                    r3 = vld1_dup_s16(r3p);
                    g3 = vld1_dup_s16(g3p);
                    b3 = vld1_dup_s16(b3p);

                    dst0[dst_cn.r_i()] = self.profile.r_gamma[vget_lane_u16::<0>(vr0) as usize];
                    dst0[dst_cn.g_i()] = self.profile.g_gamma[vget_lane_u16::<1>(vr0) as usize];
                    dst0[dst_cn.b_i()] = self.profile.b_gamma[vget_lane_u16::<2>(vr0) as usize];
                    if dst_channels == 4 {
                        dst0[dst_cn.a_i()] = a0;
                    }

                    dst0[dst_cn.r_i() + dst_channels] =
                        self.profile.r_gamma[vget_lane_u16::<0>(vr1) as usize];
                    dst0[dst_cn.g_i() + dst_channels] =
                        self.profile.g_gamma[vget_lane_u16::<1>(vr1) as usize];
                    dst0[dst_cn.b_i() + dst_channels] =
                        self.profile.b_gamma[vget_lane_u16::<2>(vr0) as usize];
                    if dst_channels == 4 {
                        dst0[dst_cn.a_i() + dst_channels] = a1;
                    }

                    dst1[dst_cn.r_i()] = self.profile.r_gamma[vget_lane_u16::<0>(vr2) as usize];
                    dst1[dst_cn.g_i()] = self.profile.g_gamma[vget_lane_u16::<1>(vr2) as usize];
                    dst1[dst_cn.b_i()] = self.profile.b_gamma[vget_lane_u16::<2>(vr2) as usize];
                    if dst_channels == 4 {
                        dst1[dst_cn.a_i()] = a2;
                    }

                    dst1[dst_cn.r_i() + dst_channels] =
                        self.profile.r_gamma[vget_lane_u16::<0>(vr3) as usize];
                    dst1[dst_cn.g_i() + dst_channels] =
                        self.profile.g_gamma[vget_lane_u16::<1>(vr3) as usize];
                    dst1[dst_cn.b_i() + dst_channels] =
                        self.profile.b_gamma[vget_lane_u16::<2>(vr3) as usize];
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
                    let v0_0 = vmlal_s16(rnd, r0, m0);
                    let v0_1 = vmlal_s16(rnd, r1, m0);
                    let v0_2 = vmlal_s16(rnd, r2, m0);
                    let v0_3 = vmlal_s16(rnd, r3, m0);

                    let v1_0 = vmlal_s16(v0_0, g0, m1);
                    let v1_1 = vmlal_s16(v0_1, g1, m1);
                    let v1_2 = vmlal_s16(v0_2, g2, m1);
                    let v1_3 = vmlal_s16(v0_3, g3, m1);

                    let vr0 = vmlal_s16(v1_0, b0, m2);
                    let vr1 = vmlal_s16(v1_1, b1, m2);
                    let vr2 = vmlal_s16(v1_2, b2, m2);
                    let vr3 = vmlal_s16(v1_3, b3, m2);

                    let mut vr0 = vqshrun_n_s32::<PRECISION>(vr0);
                    let mut vr1 = vqshrun_n_s32::<PRECISION>(vr1);
                    let mut vr2 = vqshrun_n_s32::<PRECISION>(vr2);
                    let mut vr3 = vqshrun_n_s32::<PRECISION>(vr3);

                    vr0 = vmin_u16(vr0, v_max_value);
                    vr1 = vmin_u16(vr1, v_max_value);
                    vr2 = vmin_u16(vr2, v_max_value);
                    vr3 = vmin_u16(vr3, v_max_value);

                    dst0[dst_cn.r_i()] = self.profile.r_gamma[vget_lane_u16::<0>(vr0) as usize];
                    dst0[dst_cn.g_i()] = self.profile.g_gamma[vget_lane_u16::<1>(vr0) as usize];
                    dst0[dst_cn.b_i()] = self.profile.b_gamma[vget_lane_u16::<2>(vr0) as usize];
                    if dst_channels == 4 {
                        dst0[dst_cn.a_i()] = a0;
                    }

                    dst0[dst_cn.r_i() + dst_channels] =
                        self.profile.r_gamma[vget_lane_u16::<0>(vr1) as usize];
                    dst0[dst_cn.g_i() + dst_channels] =
                        self.profile.g_gamma[vget_lane_u16::<1>(vr1) as usize];
                    dst0[dst_cn.b_i() + dst_channels] =
                        self.profile.b_gamma[vget_lane_u16::<2>(vr0) as usize];
                    if dst_channels == 4 {
                        dst0[dst_cn.a_i() + dst_channels] = a1;
                    }

                    dst1[dst_cn.r_i()] = self.profile.r_gamma[vget_lane_u16::<0>(vr2) as usize];
                    dst1[dst_cn.g_i()] = self.profile.g_gamma[vget_lane_u16::<1>(vr2) as usize];
                    dst1[dst_cn.b_i()] = self.profile.b_gamma[vget_lane_u16::<2>(vr2) as usize];
                    if dst_channels == 4 {
                        dst1[dst_cn.a_i()] = a2;
                    }

                    dst1[dst_cn.r_i() + dst_channels] =
                        self.profile.r_gamma[vget_lane_u16::<0>(vr3) as usize];
                    dst1[dst_cn.g_i() + dst_channels] =
                        self.profile.g_gamma[vget_lane_u16::<1>(vr3) as usize];
                    dst1[dst_cn.b_i() + dst_channels] =
                        self.profile.b_gamma[vget_lane_u16::<2>(vr3) as usize];
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
                let r = vld1_dup_s16(rp);
                let g = vld1_dup_s16(gp);
                let b = vld1_dup_s16(bp);
                let a = if src_channels == 4 {
                    src[src_cn.a_i()]
                } else {
                    max_colors
                };

                let v0 = vmlal_s16(rnd, r, m0);
                let v1 = vmlal_s16(v0, g, m1);
                let v = vmlal_s16(v1, b, m2);

                let mut vr0 = vqshrun_n_s32::<PRECISION>(v);
                vr0 = vmin_u16(vr0, v_max_value);

                dst[dst_cn.r_i()] = self.profile.r_gamma[vget_lane_u16::<0>(vr0) as usize];
                dst[dst_cn.g_i()] = self.profile.g_gamma[vget_lane_u16::<1>(vr0) as usize];
                dst[dst_cn.b_i()] = self.profile.b_gamma[vget_lane_u16::<2>(vr0) as usize];
                if dst_channels == 4 {
                    dst[dst_cn.a_i()] = a;
                }
            }
        }

        Ok(())
    }
}
