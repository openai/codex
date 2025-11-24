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
use crate::conversions::LutBarycentricReduction;
use crate::conversions::interpolator::BarycentricWeight;
use crate::conversions::neon::interpolator_q0_15::*;
use crate::transform::PointeeSizeExpressible;
use crate::{CmsError, DataColorSpace, InterpolationMethod, Layout, TransformExecutor};
use num_traits::AsPrimitive;
use std::arch::aarch64::*;
use std::marker::PhantomData;

pub(crate) struct TransformLut4To3NeonQ0_15<
    T,
    U,
    const LAYOUT: u8,
    const GRID_SIZE: usize,
    const BIT_DEPTH: usize,
    const BINS: usize,
    const BARYCENTRIC_BINS: usize,
> {
    pub(crate) lut: Vec<NeonAlignedI16x4>,
    pub(crate) _phantom: PhantomData<T>,
    pub(crate) _phantom1: PhantomData<U>,
    pub(crate) interpolation_method: InterpolationMethod,
    pub(crate) weights: Box<[BarycentricWeight<i16>; BINS]>,
    pub(crate) color_space: DataColorSpace,
    pub(crate) is_linear: bool,
}

impl<
    T: Copy + AsPrimitive<f32> + Default + PointeeSizeExpressible,
    U: AsPrimitive<usize>,
    const LAYOUT: u8,
    const GRID_SIZE: usize,
    const BIT_DEPTH: usize,
    const BINS: usize,
    const BARYCENTRIC_BINS: usize,
> TransformLut4To3NeonQ0_15<T, U, LAYOUT, GRID_SIZE, BIT_DEPTH, BINS, BARYCENTRIC_BINS>
where
    f32: AsPrimitive<T>,
    u32: AsPrimitive<T>,
    (): LutBarycentricReduction<T, U>,
{
    #[allow(unused_unsafe)]
    #[target_feature(enable = "rdm")]
    unsafe fn transform_chunk<'b, Interpolator: NeonMdInterpolationQ0_15Double<'b, GRID_SIZE>>(
        &'b self,
        src: &[T],
        dst: &mut [T],
    ) {
        unsafe {
            let cn = Layout::from(LAYOUT);
            let channels = cn.channels();
            let grid_size = GRID_SIZE as i32;
            let grid_size3 = grid_size * grid_size * grid_size;

            let f_value_scale = vdupq_n_f32(1. / ((1 << 14i32) - 1) as f32);
            let max_value = ((1u32 << BIT_DEPTH) - 1).as_();
            let v_max_scale = if T::FINITE {
                vdup_n_s16(((1i32 << BIT_DEPTH) - 1) as i16)
            } else {
                vdup_n_s16(((1i32 << 14i32) - 1) as i16)
            };

            for (src, dst) in src.chunks_exact(4).zip(dst.chunks_exact_mut(channels)) {
                let c = <() as LutBarycentricReduction<T, U>>::reduce::<BIT_DEPTH, BARYCENTRIC_BINS>(
                    src[0],
                );
                let m = <() as LutBarycentricReduction<T, U>>::reduce::<BIT_DEPTH, BARYCENTRIC_BINS>(
                    src[1],
                );
                let y = <() as LutBarycentricReduction<T, U>>::reduce::<BIT_DEPTH, BARYCENTRIC_BINS>(
                    src[2],
                );
                let k = <() as LutBarycentricReduction<T, U>>::reduce::<BIT_DEPTH, BARYCENTRIC_BINS>(
                    src[3],
                );

                let k_weights = self.weights[k.as_()];

                let w: i32 = k_weights.x;
                let w_n: i32 = k_weights.x_n;
                let t: i16 = k_weights.w;

                let table1 = &self.lut[(w * grid_size3) as usize..];
                let table2 = &self.lut[(w_n * grid_size3) as usize..];

                let tetrahedral1 = Interpolator::new(table1, table2);
                let (a0, b0) = tetrahedral1.inter3_neon(c, m, y, &self.weights);
                let (a0, b0) = (a0.v, b0.v);

                let t0 = vdup_n_s16(t);
                let hp = vqrdmlsh_s16(a0, a0, t0);
                let mut v = vqrdmlah_s16(hp, b0, t0);

                if T::FINITE {
                    v = vmax_s16(v, vdup_n_s16(0));
                    v = vmin_s16(v, v_max_scale);
                    dst[cn.r_i()] = (vget_lane_s16::<0>(v) as u32).as_();
                    dst[cn.g_i()] = (vget_lane_s16::<1>(v) as u32).as_();
                    dst[cn.b_i()] = (vget_lane_s16::<2>(v) as u32).as_();
                } else {
                    let o = vcvtq_f32_s32(vmovl_s16(v));
                    let r = vmulq_f32(o, f_value_scale);
                    dst[cn.r_i()] = vgetq_lane_f32::<0>(r).as_();
                    dst[cn.g_i()] = vgetq_lane_f32::<1>(r).as_();
                    dst[cn.b_i()] = vgetq_lane_f32::<2>(r).as_();
                }
                if channels == 4 {
                    dst[cn.a_i()] = max_value;
                }
            }
        }
    }
}

impl<
    T: Copy + AsPrimitive<f32> + Default + PointeeSizeExpressible,
    U: AsPrimitive<usize>,
    const LAYOUT: u8,
    const GRID_SIZE: usize,
    const BIT_DEPTH: usize,
    const BINS: usize,
    const BARYCENTRIC_BINS: usize,
> TransformExecutor<T>
    for TransformLut4To3NeonQ0_15<T, U, LAYOUT, GRID_SIZE, BIT_DEPTH, BINS, BARYCENTRIC_BINS>
where
    f32: AsPrimitive<T>,
    u32: AsPrimitive<T>,
    (): LutBarycentricReduction<T, U>,
{
    fn transform(&self, src: &[T], dst: &mut [T]) -> Result<(), CmsError> {
        let cn = Layout::from(LAYOUT);
        let channels = cn.channels();
        if src.len() % 4 != 0 {
            return Err(CmsError::LaneMultipleOfChannels);
        }
        if dst.len() % channels != 0 {
            return Err(CmsError::LaneMultipleOfChannels);
        }
        let src_chunks = src.len() / 4;
        let dst_chunks = dst.len() / channels;
        if src_chunks != dst_chunks {
            return Err(CmsError::LaneSizeMismatch);
        }

        unsafe {
            if self.color_space == DataColorSpace::Lab
                || (self.is_linear && self.color_space == DataColorSpace::Rgb)
                || self.color_space == DataColorSpace::Xyz
            {
                self.transform_chunk::<TrilinearNeonQ0_15Double<GRID_SIZE>>(src, dst);
            } else {
                match self.interpolation_method {
                    #[cfg(feature = "options")]
                    InterpolationMethod::Tetrahedral => {
                        self.transform_chunk::<TetrahedralNeonQ0_15Double<GRID_SIZE>>(src, dst);
                    }
                    #[cfg(feature = "options")]
                    InterpolationMethod::Pyramid => {
                        self.transform_chunk::<PyramidalNeonQ0_15Double<GRID_SIZE>>(src, dst);
                    }
                    #[cfg(feature = "options")]
                    InterpolationMethod::Prism => {
                        self.transform_chunk::<PrismaticNeonQ0_15Double<GRID_SIZE>>(src, dst);
                    }
                    InterpolationMethod::Linear => {
                        self.transform_chunk::<TrilinearNeonQ0_15Double<GRID_SIZE>>(src, dst);
                    }
                }
            }
        }

        Ok(())
    }
}
