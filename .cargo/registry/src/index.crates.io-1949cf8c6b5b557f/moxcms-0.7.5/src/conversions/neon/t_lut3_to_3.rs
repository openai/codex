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
use crate::conversions::lut_transforms::Lut3x3Factory;
use crate::conversions::neon::interpolator::*;
use crate::conversions::neon::interpolator_q0_15::NeonAlignedI16x4;
use crate::conversions::neon::rgb_xyz::NeonAlignedF32;
use crate::conversions::neon::t_lut3_to_3_q0_15::TransformLut3x3NeonQ0_15;
use crate::transform::PointeeSizeExpressible;
use crate::{
    BarycentricWeightScale, CmsError, DataColorSpace, InterpolationMethod, Layout,
    TransformExecutor, TransformOptions,
};
use num_traits::AsPrimitive;
use std::arch::aarch64::*;
use std::marker::PhantomData;

struct TransformLut3x3Neon<
    T,
    U,
    const SRC_LAYOUT: u8,
    const DST_LAYOUT: u8,
    const GRID_SIZE: usize,
    const BIT_DEPTH: usize,
    const BINS: usize,
    const BARYCENTRIC_BINS: usize,
> {
    lut: Vec<NeonAlignedF32>,
    _phantom: PhantomData<T>,
    _phantom1: PhantomData<U>,
    interpolation_method: InterpolationMethod,
    weights: Box<[BarycentricWeight<f32>; BINS]>,
    color_space: DataColorSpace,
    is_linear: bool,
}

impl<
    T: Copy + AsPrimitive<f32> + Default + PointeeSizeExpressible,
    U: AsPrimitive<usize>,
    const SRC_LAYOUT: u8,
    const DST_LAYOUT: u8,
    const GRID_SIZE: usize,
    const BIT_DEPTH: usize,
    const BINS: usize,
    const BARYCENTRIC_BINS: usize,
> TransformLut3x3Neon<T, U, SRC_LAYOUT, DST_LAYOUT, GRID_SIZE, BIT_DEPTH, BINS, BARYCENTRIC_BINS>
where
    f32: AsPrimitive<T>,
    u32: AsPrimitive<T>,
    (): LutBarycentricReduction<T, U>,
{
    #[inline(always)]
    fn transform_chunk<'b, Interpolator: NeonMdInterpolation<'b, GRID_SIZE>>(
        &'b self,
        src: &[T],
        dst: &mut [T],
    ) {
        unsafe {
            let src_cn = Layout::from(SRC_LAYOUT);
            let src_channels = src_cn.channels();

            let dst_cn = Layout::from(DST_LAYOUT);
            let dst_channels = dst_cn.channels();

            let value_scale = vdupq_n_f32(((1 << BIT_DEPTH) - 1) as f32);
            let max_value = ((1u32 << BIT_DEPTH) - 1).as_();

            for (src, dst) in src
                .chunks_exact(src_channels)
                .zip(dst.chunks_exact_mut(dst_channels))
            {
                let x = <() as LutBarycentricReduction<T, U>>::reduce::<BIT_DEPTH, BARYCENTRIC_BINS>(
                    src[src_cn.r_i()],
                );
                let y = <() as LutBarycentricReduction<T, U>>::reduce::<BIT_DEPTH, BARYCENTRIC_BINS>(
                    src[src_cn.g_i()],
                );
                let z = <() as LutBarycentricReduction<T, U>>::reduce::<BIT_DEPTH, BARYCENTRIC_BINS>(
                    src[src_cn.b_i()],
                );

                let a = if src_channels == 4 {
                    src[src_cn.a_i()]
                } else {
                    max_value
                };

                let tetrahedral = Interpolator::new(&self.lut);
                let v = tetrahedral.inter3_neon(x, y, z, &self.weights);
                if T::FINITE {
                    let mut r = vfmaq_f32(vdupq_n_f32(0.5f32), v.v, value_scale);
                    r = vminq_f32(r, value_scale);
                    let jvx = vcvtaq_u32_f32(r);

                    dst[dst_cn.r_i()] = vgetq_lane_u32::<0>(jvx).as_();
                    dst[dst_cn.g_i()] = vgetq_lane_u32::<1>(jvx).as_();
                    dst[dst_cn.b_i()] = vgetq_lane_u32::<2>(jvx).as_();
                } else {
                    dst[dst_cn.r_i()] = vgetq_lane_f32::<0>(v.v).as_();
                    dst[dst_cn.g_i()] = vgetq_lane_f32::<1>(v.v).as_();
                    dst[dst_cn.b_i()] = vgetq_lane_f32::<2>(v.v).as_();
                }
                if dst_channels == 4 {
                    dst[dst_cn.a_i()] = a;
                }
            }
        }
    }
}

impl<
    T: Copy + AsPrimitive<f32> + Default + PointeeSizeExpressible,
    U: AsPrimitive<usize>,
    const SRC_LAYOUT: u8,
    const DST_LAYOUT: u8,
    const GRID_SIZE: usize,
    const BIT_DEPTH: usize,
    const BINS: usize,
    const BARYCENTRIC_BINS: usize,
> TransformExecutor<T>
    for TransformLut3x3Neon<
        T,
        U,
        SRC_LAYOUT,
        DST_LAYOUT,
        GRID_SIZE,
        BIT_DEPTH,
        BINS,
        BARYCENTRIC_BINS,
    >
where
    f32: AsPrimitive<T>,
    u32: AsPrimitive<T>,
    (): LutBarycentricReduction<T, U>,
{
    fn transform(&self, src: &[T], dst: &mut [T]) -> Result<(), CmsError> {
        let src_cn = Layout::from(SRC_LAYOUT);
        let src_channels = src_cn.channels();

        let dst_cn = Layout::from(DST_LAYOUT);
        let dst_channels = dst_cn.channels();
        if src.len() % src_channels != 0 {
            return Err(CmsError::LaneMultipleOfChannels);
        }
        if dst.len() % dst_channels != 0 {
            return Err(CmsError::LaneMultipleOfChannels);
        }
        let src_chunks = src.len() / src_channels;
        let dst_chunks = dst.len() / dst_channels;
        if src_chunks != dst_chunks {
            return Err(CmsError::LaneSizeMismatch);
        }

        if self.color_space == DataColorSpace::Lab
            || (self.is_linear && self.color_space == DataColorSpace::Rgb)
            || self.color_space == DataColorSpace::Xyz
        {
            self.transform_chunk::<TrilinearNeon<GRID_SIZE>>(src, dst);
        } else {
            match self.interpolation_method {
                #[cfg(feature = "options")]
                InterpolationMethod::Tetrahedral => {
                    self.transform_chunk::<TetrahedralNeon<GRID_SIZE>>(src, dst);
                }
                #[cfg(feature = "options")]
                InterpolationMethod::Pyramid => {
                    self.transform_chunk::<PyramidalNeon<GRID_SIZE>>(src, dst);
                }
                #[cfg(feature = "options")]
                InterpolationMethod::Prism => {
                    self.transform_chunk::<PrismaticNeon<GRID_SIZE>>(src, dst);
                }
                InterpolationMethod::Linear => {
                    self.transform_chunk::<TrilinearNeon<GRID_SIZE>>(src, dst);
                }
            }
        }

        Ok(())
    }
}

pub(crate) struct NeonLut3x3Factory {}

impl Lut3x3Factory for NeonLut3x3Factory {
    fn make_transform_3x3<
        T: Copy + AsPrimitive<f32> + Default + PointeeSizeExpressible + 'static + Send + Sync,
        const SRC_LAYOUT: u8,
        const DST_LAYOUT: u8,
        const GRID_SIZE: usize,
        const BIT_DEPTH: usize,
    >(
        lut: Vec<f32>,
        options: TransformOptions,
        color_space: DataColorSpace,
        is_linear: bool,
    ) -> Box<dyn TransformExecutor<T> + Send + Sync>
    where
        f32: AsPrimitive<T>,
        u32: AsPrimitive<T>,
        (): LutBarycentricReduction<T, u8>,
        (): LutBarycentricReduction<T, u16>,
    {
        if options.prefer_fixed_point
            && BIT_DEPTH < 16
            && std::arch::is_aarch64_feature_detected!("rdm")
        {
            let q: f32 = if T::FINITE {
                ((1i32 << BIT_DEPTH as i32) - 1) as f32
            } else {
                ((1i32 << 14i32) - 1) as f32
            };
            let lut = lut
                .chunks_exact(3)
                .map(|x| {
                    NeonAlignedI16x4([
                        (x[0] * q).round() as i16,
                        (x[1] * q).round() as i16,
                        (x[2] * q).round() as i16,
                        0,
                    ])
                })
                .collect::<Vec<_>>();
            return match options.barycentric_weight_scale {
                BarycentricWeightScale::Low => Box::new(TransformLut3x3NeonQ0_15::<
                    T,
                    u8,
                    SRC_LAYOUT,
                    DST_LAYOUT,
                    GRID_SIZE,
                    BIT_DEPTH,
                    256,
                    256,
                > {
                    lut,
                    _phantom: PhantomData,
                    _phantom1: PhantomData,
                    interpolation_method: options.interpolation_method,
                    weights: BarycentricWeight::<i16>::create_ranged_256::<GRID_SIZE>(),
                    color_space,
                    is_linear,
                }),
                #[cfg(feature = "options")]
                BarycentricWeightScale::High => Box::new(TransformLut3x3NeonQ0_15::<
                    T,
                    u16,
                    SRC_LAYOUT,
                    DST_LAYOUT,
                    GRID_SIZE,
                    BIT_DEPTH,
                    65536,
                    65536,
                > {
                    lut,
                    _phantom: PhantomData,
                    _phantom1: PhantomData,
                    interpolation_method: options.interpolation_method,
                    weights: BarycentricWeight::<i16>::create_binned::<GRID_SIZE, 65536>(),
                    color_space,
                    is_linear,
                }),
            };
        }
        let lut = lut
            .chunks_exact(3)
            .map(|x| NeonAlignedF32([x[0], x[1], x[2], 0f32]))
            .collect::<Vec<_>>();
        match options.barycentric_weight_scale {
            BarycentricWeightScale::Low => Box::new(TransformLut3x3Neon::<
                T,
                u8,
                SRC_LAYOUT,
                DST_LAYOUT,
                GRID_SIZE,
                BIT_DEPTH,
                256,
                256,
            > {
                lut,
                _phantom: PhantomData,
                _phantom1: PhantomData,
                interpolation_method: options.interpolation_method,
                weights: BarycentricWeight::<f32>::create_ranged_256::<GRID_SIZE>(),
                color_space,
                is_linear,
            }),
            #[cfg(feature = "options")]
            BarycentricWeightScale::High => Box::new(TransformLut3x3Neon::<
                T,
                u16,
                SRC_LAYOUT,
                DST_LAYOUT,
                GRID_SIZE,
                BIT_DEPTH,
                65536,
                65536,
            > {
                lut,
                _phantom: PhantomData,
                _phantom1: PhantomData,
                interpolation_method: options.interpolation_method,
                weights: BarycentricWeight::<f32>::create_binned::<GRID_SIZE, 65536>(),
                color_space,
                is_linear,
            }),
        }
    }
}
