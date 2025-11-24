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
use crate::conversions::interpolator::*;
use crate::conversions::lut_transforms::Lut4x3Factory;
use crate::math::{FusedMultiplyAdd, FusedMultiplyNegAdd, m_clamp};
use crate::{
    BarycentricWeightScale, CmsError, DataColorSpace, InterpolationMethod, Layout,
    PointeeSizeExpressible, TransformExecutor, TransformOptions, Vector3f,
};
use num_traits::AsPrimitive;
use std::marker::PhantomData;

pub(crate) trait Vector3fCmykLerp {
    fn interpolate(a: Vector3f, b: Vector3f, t: f32, scale: f32) -> Vector3f;
}

#[allow(unused)]
#[derive(Copy, Clone, Default)]
struct DefaultVector3fLerp;

impl Vector3fCmykLerp for DefaultVector3fLerp {
    #[inline(always)]
    fn interpolate(a: Vector3f, b: Vector3f, t: f32, scale: f32) -> Vector3f {
        let t = Vector3f::from(t);
        let inter = a.neg_mla(a, t).mla(b, t);
        let mut new_vec = Vector3f::from(0.5).mla(inter, Vector3f::from(scale));
        new_vec.v[0] = m_clamp(new_vec.v[0], 0.0, scale);
        new_vec.v[1] = m_clamp(new_vec.v[1], 0.0, scale);
        new_vec.v[2] = m_clamp(new_vec.v[2], 0.0, scale);
        new_vec
    }
}

#[allow(unused)]
#[derive(Copy, Clone, Default)]
pub(crate) struct NonFiniteVector3fLerp;

impl Vector3fCmykLerp for NonFiniteVector3fLerp {
    #[inline(always)]
    fn interpolate(a: Vector3f, b: Vector3f, t: f32, _: f32) -> Vector3f {
        let t = Vector3f::from(t);
        a.neg_mla(a, t).mla(b, t)
    }
}

#[allow(unused)]
#[derive(Copy, Clone, Default)]
pub(crate) struct NonFiniteVector3fLerpUnbound;

impl Vector3fCmykLerp for NonFiniteVector3fLerpUnbound {
    #[inline(always)]
    fn interpolate(a: Vector3f, b: Vector3f, t: f32, _: f32) -> Vector3f {
        let t = Vector3f::from(t);
        a.neg_mla(a, t).mla(b, t)
    }
}

#[allow(unused)]
struct TransformLut4To3<
    T,
    U,
    const LAYOUT: u8,
    const GRID_SIZE: usize,
    const BIT_DEPTH: usize,
    const BINS: usize,
    const BARYCENTRIC_BINS: usize,
> {
    lut: Vec<f32>,
    _phantom: PhantomData<T>,
    _phantom1: PhantomData<U>,
    interpolation_method: InterpolationMethod,
    weights: Box<[BarycentricWeight<f32>; BINS]>,
    color_space: DataColorSpace,
    is_linear: bool,
}

#[allow(unused)]
impl<
    T: Copy + AsPrimitive<f32> + Default,
    U: AsPrimitive<usize>,
    const LAYOUT: u8,
    const GRID_SIZE: usize,
    const BIT_DEPTH: usize,
    const BINS: usize,
    const BARYCENTRIC_BINS: usize,
> TransformLut4To3<T, U, LAYOUT, GRID_SIZE, BIT_DEPTH, BINS, BARYCENTRIC_BINS>
where
    f32: AsPrimitive<T>,
    u32: AsPrimitive<T>,
    (): LutBarycentricReduction<T, U>,
{
    #[inline(always)]
    fn transform_chunk<
        'k,
        Tetrahedral: MultidimensionalInterpolation<'k, GRID_SIZE>,
        Interpolation: Vector3fCmykLerp,
    >(
        &'k self,
        src: &[T],
        dst: &mut [T],
    ) {
        let cn = Layout::from(LAYOUT);
        let channels = cn.channels();
        let grid_size = GRID_SIZE as i32;
        let grid_size3 = grid_size * grid_size * grid_size;

        let value_scale = ((1 << BIT_DEPTH) - 1) as f32;
        let max_value = ((1 << BIT_DEPTH) - 1u32).as_();

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
            let t: f32 = k_weights.w;

            let table1 = &self.lut[(w * grid_size3 * 3) as usize..];
            let table2 = &self.lut[(w_n * grid_size3 * 3) as usize..];

            let tetrahedral1 = Tetrahedral::new(table1);
            let tetrahedral2 = Tetrahedral::new(table2);
            let r1 = tetrahedral1.inter3(c, m, y, &self.weights);
            let r2 = tetrahedral2.inter3(c, m, y, &self.weights);
            let r = Interpolation::interpolate(r1, r2, t, value_scale);
            dst[cn.r_i()] = r.v[0].as_();
            dst[cn.g_i()] = r.v[1].as_();
            dst[cn.b_i()] = r.v[2].as_();
            if channels == 4 {
                dst[cn.a_i()] = max_value;
            }
        }
    }
}

#[allow(unused)]
impl<
    T: Copy + AsPrimitive<f32> + Default + PointeeSizeExpressible,
    U: AsPrimitive<usize>,
    const LAYOUT: u8,
    const GRID_SIZE: usize,
    const BIT_DEPTH: usize,
    const BINS: usize,
    const BARYCENTRIC_BINS: usize,
> TransformExecutor<T>
    for TransformLut4To3<T, U, LAYOUT, GRID_SIZE, BIT_DEPTH, BINS, BARYCENTRIC_BINS>
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

        if self.color_space == DataColorSpace::Lab
            || (self.is_linear && self.color_space == DataColorSpace::Rgb)
            || self.color_space == DataColorSpace::Xyz
        {
            if T::FINITE {
                self.transform_chunk::<Trilinear<GRID_SIZE>, DefaultVector3fLerp>(src, dst);
            } else {
                self.transform_chunk::<Trilinear<GRID_SIZE>, NonFiniteVector3fLerp>(src, dst);
            }
        } else {
            match self.interpolation_method {
                #[cfg(feature = "options")]
                InterpolationMethod::Tetrahedral => {
                    if T::FINITE {
                        self.transform_chunk::<Tetrahedral<GRID_SIZE>, DefaultVector3fLerp>(
                            src, dst,
                        );
                    } else {
                        self.transform_chunk::<Tetrahedral<GRID_SIZE>, NonFiniteVector3fLerp>(
                            src, dst,
                        );
                    }
                }
                #[cfg(feature = "options")]
                InterpolationMethod::Pyramid => {
                    if T::FINITE {
                        self.transform_chunk::<Pyramidal<GRID_SIZE>, DefaultVector3fLerp>(src, dst);
                    } else {
                        self.transform_chunk::<Pyramidal<GRID_SIZE>, NonFiniteVector3fLerp>(
                            src, dst,
                        );
                    }
                }
                #[cfg(feature = "options")]
                InterpolationMethod::Prism => {
                    if T::FINITE {
                        self.transform_chunk::<Prismatic<GRID_SIZE>, DefaultVector3fLerp>(src, dst);
                    } else {
                        self.transform_chunk::<Prismatic<GRID_SIZE>, NonFiniteVector3fLerp>(
                            src, dst,
                        );
                    }
                }
                InterpolationMethod::Linear => {
                    if T::FINITE {
                        self.transform_chunk::<Trilinear<GRID_SIZE>, DefaultVector3fLerp>(src, dst);
                    } else {
                        self.transform_chunk::<Trilinear<GRID_SIZE>, NonFiniteVector3fLerp>(
                            src, dst,
                        );
                    }
                }
            }
        }

        Ok(())
    }
}

#[allow(dead_code)]
pub(crate) struct DefaultLut4x3Factory {}

#[allow(dead_code)]
impl Lut4x3Factory for DefaultLut4x3Factory {
    fn make_transform_4x3<
        T: Copy + AsPrimitive<f32> + Default + PointeeSizeExpressible + 'static + Send + Sync,
        const LAYOUT: u8,
        const GRID_SIZE: usize,
        const BIT_DEPTH: usize,
    >(
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
        match options.barycentric_weight_scale {
            BarycentricWeightScale::Low => {
                Box::new(
                    TransformLut4To3::<T, u8, LAYOUT, GRID_SIZE, BIT_DEPTH, 256, 256> {
                        lut,
                        _phantom: PhantomData,
                        _phantom1: PhantomData,
                        interpolation_method: options.interpolation_method,
                        weights: BarycentricWeight::<f32>::create_ranged_256::<GRID_SIZE>(),
                        color_space,
                        is_linear,
                    },
                )
            }
            #[cfg(feature = "options")]
            BarycentricWeightScale::High => {
                Box::new(
                    TransformLut4To3::<T, u16, LAYOUT, GRID_SIZE, BIT_DEPTH, 65536, 65536> {
                        lut,
                        _phantom: PhantomData,
                        _phantom1: PhantomData,
                        interpolation_method: options.interpolation_method,
                        weights: BarycentricWeight::<f32>::create_binned::<GRID_SIZE, 65536>(),
                        color_space,
                        is_linear,
                    },
                )
            }
        }
    }
}
