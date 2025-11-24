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
use crate::conversions::interpolator::{BarycentricWeight, MultidimensionalInterpolation};
use crate::transform::PointeeSizeExpressible;
use crate::{
    BarycentricWeightScale, CmsError, DataColorSpace, InterpolationMethod, Layout,
    TransformExecutor, TransformOptions,
};
use num_traits::AsPrimitive;
use std::marker::PhantomData;

pub(crate) struct TransformLut3x4<
    T,
    U: AsPrimitive<usize>,
    const LAYOUT: u8,
    const GRID_SIZE: usize,
    const BIT_DEPTH: usize,
    const BINS: usize,
    const BARYCENTRIC_BINS: usize,
> {
    pub(crate) lut: Vec<f32>,
    pub(crate) _phantom: PhantomData<T>,
    pub(crate) _phantom1: PhantomData<U>,
    pub(crate) interpolation_method: InterpolationMethod,
    pub(crate) weights: Box<[BarycentricWeight<f32>; BINS]>,
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
> TransformLut3x4<T, U, LAYOUT, GRID_SIZE, BIT_DEPTH, BINS, BARYCENTRIC_BINS>
where
    f32: AsPrimitive<T>,
    u32: AsPrimitive<T>,
    (): LutBarycentricReduction<T, U>,
{
    #[inline(always)]
    fn transform_chunk<'b, Tetrahedral: MultidimensionalInterpolation<'b, GRID_SIZE>>(
        &'b self,
        src: &[T],
        dst: &mut [T],
    ) {
        let cn = Layout::from(LAYOUT);
        let channels = cn.channels();

        let value_scale = ((1 << BIT_DEPTH) - 1) as f32;

        for (src, dst) in src.chunks_exact(channels).zip(dst.chunks_exact_mut(4)) {
            let x = <() as LutBarycentricReduction<T, U>>::reduce::<BIT_DEPTH, BARYCENTRIC_BINS>(
                src[cn.r_i()],
            );
            let y = <() as LutBarycentricReduction<T, U>>::reduce::<BIT_DEPTH, BARYCENTRIC_BINS>(
                src[cn.g_i()],
            );
            let z = <() as LutBarycentricReduction<T, U>>::reduce::<BIT_DEPTH, BARYCENTRIC_BINS>(
                src[cn.b_i()],
            );

            let tetrahedral = Tetrahedral::new(&self.lut);
            let v = tetrahedral.inter4(x, y, z, &self.weights);
            if T::FINITE {
                let r = v * value_scale + 0.5;
                dst[0] = r.v[0].min(value_scale).max(0.).as_();
                dst[1] = r.v[1].min(value_scale).max(0.).as_();
                dst[2] = r.v[2].min(value_scale).max(0.).as_();
                dst[3] = r.v[3].min(value_scale).max(0.).as_();
            } else {
                dst[0] = v.v[0].as_();
                dst[1] = v.v[1].as_();
                dst[2] = v.v[2].as_();
                dst[3] = v.v[3].as_();
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
    for TransformLut3x4<T, U, LAYOUT, GRID_SIZE, BIT_DEPTH, BINS, BARYCENTRIC_BINS>
where
    f32: AsPrimitive<T>,
    u32: AsPrimitive<T>,
    (): LutBarycentricReduction<T, U>,
{
    fn transform(&self, src: &[T], dst: &mut [T]) -> Result<(), CmsError> {
        let cn = Layout::from(LAYOUT);
        let channels = cn.channels();
        if src.len() % channels != 0 {
            return Err(CmsError::LaneMultipleOfChannels);
        }
        if dst.len() % 4 != 0 {
            return Err(CmsError::LaneMultipleOfChannels);
        }
        let src_chunks = src.len() / channels;
        let dst_chunks = dst.len() / 4;
        if src_chunks != dst_chunks {
            return Err(CmsError::LaneSizeMismatch);
        }

        if self.color_space == DataColorSpace::Lab
            || (self.is_linear && self.color_space == DataColorSpace::Rgb)
            || self.color_space == DataColorSpace::Xyz
        {
            use crate::conversions::interpolator::Trilinear;
            self.transform_chunk::<Trilinear<GRID_SIZE>>(src, dst);
        } else {
            match self.interpolation_method {
                #[cfg(feature = "options")]
                InterpolationMethod::Tetrahedral => {
                    use crate::conversions::interpolator::Tetrahedral;
                    self.transform_chunk::<Tetrahedral<GRID_SIZE>>(src, dst);
                }
                #[cfg(feature = "options")]
                InterpolationMethod::Pyramid => {
                    use crate::conversions::interpolator::Pyramidal;
                    self.transform_chunk::<Pyramidal<GRID_SIZE>>(src, dst);
                }
                #[cfg(feature = "options")]
                InterpolationMethod::Prism => {
                    use crate::conversions::interpolator::Prismatic;
                    self.transform_chunk::<Prismatic<GRID_SIZE>>(src, dst);
                }
                InterpolationMethod::Linear => {
                    use crate::conversions::interpolator::Trilinear;
                    self.transform_chunk::<Trilinear<GRID_SIZE>>(src, dst);
                }
            }
        }

        Ok(())
    }
}

pub(crate) fn make_transform_3x4<
    T: Copy + AsPrimitive<f32> + Default + PointeeSizeExpressible + 'static + Send + Sync,
    const GRID_SIZE: usize,
    const BIT_DEPTH: usize,
>(
    layout: Layout,
    lut: Vec<f32>,
    options: TransformOptions,
    color_space: DataColorSpace,
    is_linear: bool,
) -> Box<dyn TransformExecutor<T> + Sync + Send>
where
    f32: AsPrimitive<T>,
    u32: AsPrimitive<T>,
    (): LutBarycentricReduction<T, u8>,
    (): LutBarycentricReduction<T, u16>,
{
    match layout {
        Layout::Rgb => match options.barycentric_weight_scale {
            BarycentricWeightScale::Low => Box::new(TransformLut3x4::<
                T,
                u8,
                { Layout::Rgb as u8 },
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
            BarycentricWeightScale::High => Box::new(TransformLut3x4::<
                T,
                u16,
                { Layout::Rgb as u8 },
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
        },
        Layout::Rgba => match options.barycentric_weight_scale {
            BarycentricWeightScale::Low => Box::new(TransformLut3x4::<
                T,
                u8,
                { Layout::Rgba as u8 },
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
            BarycentricWeightScale::High => Box::new(TransformLut3x4::<
                T,
                u16,
                { Layout::Rgba as u8 },
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
        },
        _ => unimplemented!(),
    }
}
