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
use crate::conversions::avx::interpolator_q0_15::*;
use crate::conversions::interpolator::BarycentricWeight;
use crate::transform::PointeeSizeExpressible;
use crate::{CmsError, DataColorSpace, InterpolationMethod, Layout, TransformExecutor};
use num_traits::AsPrimitive;
use std::arch::x86_64::*;
use std::marker::PhantomData;

pub(crate) struct TransformLut3x3AvxQ0_15<
    T,
    U,
    const SRC_LAYOUT: u8,
    const DST_LAYOUT: u8,
    const GRID_SIZE: usize,
    const BIT_DEPTH: usize,
    const BINS: usize,
    const BARYCENTRIC_BINS: usize,
> {
    pub(crate) lut: Vec<AvxAlignedI16>,
    pub(crate) _phantom: PhantomData<T>,
    pub(crate) _phantom2: PhantomData<U>,
    pub(crate) interpolation_method: InterpolationMethod,
    pub(crate) weights: Box<[BarycentricWeight<i16>; BINS]>,
    pub(crate) color_space: DataColorSpace,
    pub(crate) is_linear: bool,
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
>
    TransformLut3x3AvxQ0_15<
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
    #[allow(unused_unsafe)]
    #[target_feature(enable = "avx2")]
    unsafe fn transform_chunk<'b, Interpolator: AvxMdInterpolationQ0_15<'b, GRID_SIZE>>(
        &'b self,
        src: &[T],
        dst: &mut [T],
    ) {
        unsafe {
            let src_cn = Layout::from(SRC_LAYOUT);
            let src_channels = src_cn.channels();

            let dst_cn = Layout::from(DST_LAYOUT);
            let dst_channels = dst_cn.channels();

            let f_value_scale = _mm_set1_ps(1. / ((1 << 14i32) - 1) as f32);
            let max_value = ((1u32 << BIT_DEPTH) - 1).as_();
            let v_max_scale = if T::FINITE {
                _mm_set1_epi16(((1i32 << BIT_DEPTH) - 1) as i16)
            } else {
                _mm_set1_epi16(((1i32 << 14i32) - 1) as i16)
            };

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
                let v = tetrahedral.inter3_sse(x, y, z, &self.weights);
                if T::FINITE {
                    let mut o = _mm_max_epi16(v.v, _mm_setzero_si128());
                    o = _mm_min_epi16(o, v_max_scale);
                    let x = _mm_extract_epi16::<0>(o);
                    let y = _mm_extract_epi16::<1>(o);
                    let z = _mm_extract_epi16::<2>(o);

                    dst[dst_cn.r_i()] = (x as u32).as_();
                    dst[dst_cn.g_i()] = (y as u32).as_();
                    dst[dst_cn.b_i()] = (z as u32).as_();
                } else {
                    let mut r = _mm_cvtepi32_ps(_mm_cvtepi16_epi32(v.v));
                    r = _mm_mul_ps(r, f_value_scale);
                    dst[dst_cn.r_i()] = f32::from_bits(_mm_extract_ps::<0>(r) as u32).as_();
                    dst[dst_cn.g_i()] = f32::from_bits(_mm_extract_ps::<1>(r) as u32).as_();
                    dst[dst_cn.b_i()] = f32::from_bits(_mm_extract_ps::<2>(r) as u32).as_();
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
    for TransformLut3x3AvxQ0_15<
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

        unsafe {
            if self.color_space == DataColorSpace::Lab
                || (self.is_linear && self.color_space == DataColorSpace::Rgb)
                || self.color_space == DataColorSpace::Xyz
            {
                self.transform_chunk::<TrilinearAvxQ0_15<GRID_SIZE>>(src, dst);
            } else {
                match self.interpolation_method {
                    #[cfg(feature = "options")]
                    InterpolationMethod::Tetrahedral => {
                        self.transform_chunk::<TetrahedralAvxQ0_15<GRID_SIZE>>(src, dst);
                    }
                    #[cfg(feature = "options")]
                    InterpolationMethod::Pyramid => {
                        self.transform_chunk::<PyramidalAvxQ0_15<GRID_SIZE>>(src, dst);
                    }
                    #[cfg(feature = "options")]
                    InterpolationMethod::Prism => {
                        self.transform_chunk::<PrismaticAvxQ0_15<GRID_SIZE>>(src, dst);
                    }
                    InterpolationMethod::Linear => {
                        self.transform_chunk::<TrilinearAvxQ0_15<GRID_SIZE>>(src, dst);
                    }
                }
            }
        }
        Ok(())
    }
}
