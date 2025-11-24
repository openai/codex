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
use crate::conversions::avx::interpolator::*;
use crate::conversions::avx::interpolator_q0_15::AvxAlignedI16;
use crate::conversions::avx::lut4_to_3_q0_15::TransformLut4To3AvxQ0_15;
use crate::conversions::interpolator::BarycentricWeight;
use crate::conversions::lut_transforms::Lut4x3Factory;
use crate::transform::PointeeSizeExpressible;
use crate::{
    BarycentricWeightScale, CmsError, DataColorSpace, InterpolationMethod, Layout,
    TransformExecutor, TransformOptions,
};
use num_traits::AsPrimitive;
use std::arch::x86_64::*;
use std::marker::PhantomData;

struct TransformLut4To3Avx<
    T,
    U,
    const LAYOUT: u8,
    const GRID_SIZE: usize,
    const BIT_DEPTH: usize,
    const BINS: usize,
    const BARYCENTRIC_BINS: usize,
> {
    lut: Vec<SseAlignedF32>,
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
    const LAYOUT: u8,
    const GRID_SIZE: usize,
    const BIT_DEPTH: usize,
    const BINS: usize,
    const BARYCENTRIC_BINS: usize,
> TransformLut4To3Avx<T, U, LAYOUT, GRID_SIZE, BIT_DEPTH, BINS, BARYCENTRIC_BINS>
where
    f32: AsPrimitive<T>,
    u32: AsPrimitive<T>,
    (): LutBarycentricReduction<T, U>,
{
    #[allow(unused_unsafe)]
    #[target_feature(enable = "avx2", enable = "fma")]
    unsafe fn transform_chunk<'b, Interpolator: AvxMdInterpolationDouble<'b, GRID_SIZE>>(
        &'b self,
        src: &[T],
        dst: &mut [T],
    ) {
        let cn = Layout::from(LAYOUT);
        let channels = cn.channels();
        let grid_size = GRID_SIZE as i32;
        let grid_size3 = grid_size * grid_size * grid_size;

        let value_scale = unsafe { _mm_set1_ps(((1 << BIT_DEPTH) - 1) as f32) };
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

            let table1 = &self.lut[(w * grid_size3) as usize..];
            let table2 = &self.lut[(w_n * grid_size3) as usize..];

            let interpolator = Interpolator::new(table1, table2);
            let v = interpolator.inter3_sse(c, m, y, &self.weights);
            let (a0, b0) = (v.0.v, v.1.v);

            if T::FINITE {
                unsafe {
                    let t0 = _mm_set1_ps(t);
                    let hp = _mm_fnmadd_ps(a0, t0, a0);
                    let mut v = _mm_fmadd_ps(b0, t0, hp);
                    v = _mm_max_ps(v, _mm_setzero_ps());
                    v = _mm_mul_ps(v, value_scale);
                    v = _mm_min_ps(v, value_scale);
                    let jvz = _mm_cvtps_epi32(v);

                    let x = _mm_extract_epi32::<0>(jvz);
                    let y = _mm_extract_epi32::<1>(jvz);
                    let z = _mm_extract_epi32::<2>(jvz);

                    dst[cn.r_i()] = (x as u32).as_();
                    dst[cn.g_i()] = (y as u32).as_();
                    dst[cn.b_i()] = (z as u32).as_();
                }
            } else {
                unsafe {
                    let t0 = _mm_set1_ps(t);
                    let hp = _mm_fnmadd_ps(a0, t0, a0);
                    let v = _mm_fmadd_ps(b0, t0, hp);
                    dst[cn.r_i()] = f32::from_bits(_mm_extract_ps::<0>(v) as u32).as_();
                    dst[cn.g_i()] = f32::from_bits(_mm_extract_ps::<1>(v) as u32).as_();
                    dst[cn.b_i()] = f32::from_bits(_mm_extract_ps::<2>(v) as u32).as_();
                }
            }
            if channels == 4 {
                dst[cn.a_i()] = max_value;
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
    for TransformLut4To3Avx<T, U, LAYOUT, GRID_SIZE, BIT_DEPTH, BINS, BARYCENTRIC_BINS>
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
                self.transform_chunk::<TrilinearAvxFmaDouble<GRID_SIZE>>(src, dst);
            } else {
                match self.interpolation_method {
                    #[cfg(feature = "options")]
                    InterpolationMethod::Tetrahedral => {
                        self.transform_chunk::<TetrahedralAvxFmaDouble<GRID_SIZE>>(src, dst);
                    }
                    #[cfg(feature = "options")]
                    InterpolationMethod::Pyramid => {
                        self.transform_chunk::<PyramidAvxFmaDouble<GRID_SIZE>>(src, dst);
                    }
                    #[cfg(feature = "options")]
                    InterpolationMethod::Prism => {
                        self.transform_chunk::<PrismaticAvxFmaDouble<GRID_SIZE>>(src, dst);
                    }
                    InterpolationMethod::Linear => {
                        self.transform_chunk::<TrilinearAvxFmaDouble<GRID_SIZE>>(src, dst);
                    }
                }
            }
        }

        Ok(())
    }
}

pub(crate) struct AvxLut4x3Factory {}

impl Lut4x3Factory for AvxLut4x3Factory {
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
    ) -> Box<dyn TransformExecutor<T> + Send + Sync>
    where
        f32: AsPrimitive<T>,
        u32: AsPrimitive<T>,
        (): LutBarycentricReduction<T, u8>,
        (): LutBarycentricReduction<T, u16>,
    {
        if options.prefer_fixed_point && BIT_DEPTH < 16 {
            let q: f32 = if T::FINITE {
                ((1i32 << BIT_DEPTH as i32) - 1) as f32
            } else {
                ((1i32 << 14i32) - 1) as f32
            };
            let lut = lut
                .chunks_exact(3)
                .map(|x| {
                    AvxAlignedI16([
                        (x[0] * q).round() as i16,
                        (x[1] * q).round() as i16,
                        (x[2] * q).round() as i16,
                        0,
                    ])
                })
                .collect::<Vec<_>>();
            return match options.barycentric_weight_scale {
                BarycentricWeightScale::Low => Box::new(TransformLut4To3AvxQ0_15::<
                    T,
                    u8,
                    LAYOUT,
                    GRID_SIZE,
                    BIT_DEPTH,
                    256,
                    256,
                > {
                    lut,
                    interpolation_method: options.interpolation_method,
                    weights: BarycentricWeight::<i16>::create_ranged_256::<GRID_SIZE>(),
                    _phantom: PhantomData,
                    _phantom1: PhantomData,
                    color_space,
                    is_linear,
                }),
                #[cfg(feature = "options")]
                BarycentricWeightScale::High => Box::new(TransformLut4To3AvxQ0_15::<
                    T,
                    u16,
                    LAYOUT,
                    GRID_SIZE,
                    BIT_DEPTH,
                    65536,
                    65536,
                > {
                    lut,
                    interpolation_method: options.interpolation_method,
                    weights: BarycentricWeight::<i16>::create_binned::<GRID_SIZE, 65536>(),
                    _phantom: PhantomData,
                    _phantom1: PhantomData,
                    color_space,
                    is_linear,
                }),
            };
        }
        assert!(
            std::arch::is_x86_feature_detected!("fma"),
            "Internal configuration error, this might not be called without `fma` feature"
        );
        let lut = lut
            .chunks_exact(3)
            .map(|x| SseAlignedF32([x[0], x[1], x[2], 0f32]))
            .collect::<Vec<_>>();
        match options.barycentric_weight_scale {
            BarycentricWeightScale::Low => {
                Box::new(
                    TransformLut4To3Avx::<T, u8, LAYOUT, GRID_SIZE, BIT_DEPTH, 256, 256> {
                        lut,
                        interpolation_method: options.interpolation_method,
                        weights: BarycentricWeight::<f32>::create_ranged_256::<GRID_SIZE>(),
                        _phantom: PhantomData,
                        _phantom1: PhantomData,
                        color_space,
                        is_linear,
                    },
                )
            }
            #[cfg(feature = "options")]
            BarycentricWeightScale::High => {
                Box::new(
                    TransformLut4To3Avx::<T, u16, LAYOUT, GRID_SIZE, BIT_DEPTH, 65536, 65536> {
                        lut,
                        interpolation_method: options.interpolation_method,
                        weights: BarycentricWeight::<f32>::create_binned::<GRID_SIZE, 65536>(),
                        _phantom: PhantomData,
                        _phantom1: PhantomData,
                        color_space,
                        is_linear,
                    },
                )
            }
        }
    }
}
