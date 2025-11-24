/*
 * // Copyright (c) Radzivon Bartoshyk 2/2025. All rights reserved.
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
use crate::trc::ToneCurveEvaluator;
use crate::{CmsError, Layout, Matrix3f, PointeeSizeExpressible, Rgb, TransformExecutor};
use num_traits::AsPrimitive;
use std::marker::PhantomData;

pub(crate) struct TransformShaperRgbFloat<T: Clone, const BUCKET: usize> {
    pub(crate) r_linear: Box<[f32; BUCKET]>,
    pub(crate) g_linear: Box<[f32; BUCKET]>,
    pub(crate) b_linear: Box<[f32; BUCKET]>,
    pub(crate) gamma_evaluator: Box<dyn ToneCurveEvaluator + Send + Sync>,
    pub(crate) adaptation_matrix: Matrix3f,
    pub(crate) phantom_data: PhantomData<T>,
}

pub(crate) struct TransformShaperFloatInOut<T: Clone> {
    pub(crate) linear_evaluator: Box<dyn ToneCurveEvaluator + Send + Sync>,
    pub(crate) gamma_evaluator: Box<dyn ToneCurveEvaluator + Send + Sync>,
    pub(crate) adaptation_matrix: Matrix3f,
    pub(crate) phantom_data: PhantomData<T>,
}

struct TransformShaperFloatScalar<
    T: Clone,
    const SRC_LAYOUT: u8,
    const DST_LAYOUT: u8,
    const LINEAR_CAP: usize,
    const BIT_DEPTH: usize,
> {
    pub(crate) profile: TransformShaperRgbFloat<T, LINEAR_CAP>,
}

struct TransformShaperRgbFloatInOut<T: Clone, const SRC_LAYOUT: u8, const DST_LAYOUT: u8> {
    pub(crate) profile: TransformShaperFloatInOut<T>,
    pub(crate) bit_depth: usize,
}

pub(crate) fn make_rgb_xyz_rgb_transform_float<
    T: Clone + Send + Sync + PointeeSizeExpressible + 'static + Copy + Default,
    const LINEAR_CAP: usize,
    const BIT_DEPTH: usize,
>(
    src_layout: Layout,
    dst_layout: Layout,
    profile: TransformShaperRgbFloat<T, LINEAR_CAP>,
) -> Result<Box<dyn TransformExecutor<T> + Send + Sync>, CmsError>
where
    u32: AsPrimitive<T>,
    f32: AsPrimitive<T>,
{
    if (src_layout == Layout::Rgba) && (dst_layout == Layout::Rgba) {
        return Ok(Box::new(TransformShaperFloatScalar::<
            T,
            { Layout::Rgba as u8 },
            { Layout::Rgba as u8 },
            LINEAR_CAP,
            BIT_DEPTH,
        > {
            profile,
        }));
    } else if (src_layout == Layout::Rgb) && (dst_layout == Layout::Rgba) {
        return Ok(Box::new(TransformShaperFloatScalar::<
            T,
            { Layout::Rgb as u8 },
            { Layout::Rgba as u8 },
            LINEAR_CAP,
            BIT_DEPTH,
        > {
            profile,
        }));
    } else if (src_layout == Layout::Rgba) && (dst_layout == Layout::Rgb) {
        return Ok(Box::new(TransformShaperFloatScalar::<
            T,
            { Layout::Rgba as u8 },
            { Layout::Rgb as u8 },
            LINEAR_CAP,
            BIT_DEPTH,
        > {
            profile,
        }));
    } else if (src_layout == Layout::Rgb) && (dst_layout == Layout::Rgb) {
        return Ok(Box::new(TransformShaperFloatScalar::<
            T,
            { Layout::Rgb as u8 },
            { Layout::Rgb as u8 },
            LINEAR_CAP,
            BIT_DEPTH,
        > {
            profile,
        }));
    }
    Err(CmsError::UnsupportedProfileConnection)
}

pub(crate) fn make_rgb_xyz_rgb_transform_float_in_out<
    T: Clone + Send + Sync + PointeeSizeExpressible + 'static + Copy + Default + AsPrimitive<f32>,
    const BIT_DEPTH: usize,
>(
    src_layout: Layout,
    dst_layout: Layout,
    profile: TransformShaperFloatInOut<T>,
) -> Result<Box<dyn TransformExecutor<T> + Send + Sync>, CmsError>
where
    u32: AsPrimitive<T>,
    f32: AsPrimitive<T>,
{
    if (src_layout == Layout::Rgba) && (dst_layout == Layout::Rgba) {
        return Ok(Box::new(TransformShaperRgbFloatInOut::<
            T,
            { Layout::Rgba as u8 },
            { Layout::Rgba as u8 },
        > {
            profile,
            bit_depth: BIT_DEPTH,
        }));
    } else if (src_layout == Layout::Rgb) && (dst_layout == Layout::Rgba) {
        return Ok(Box::new(TransformShaperRgbFloatInOut::<
            T,
            { Layout::Rgb as u8 },
            { Layout::Rgba as u8 },
        > {
            profile,
            bit_depth: BIT_DEPTH,
        }));
    } else if (src_layout == Layout::Rgba) && (dst_layout == Layout::Rgb) {
        return Ok(Box::new(TransformShaperRgbFloatInOut::<
            T,
            { Layout::Rgba as u8 },
            { Layout::Rgb as u8 },
        > {
            profile,
            bit_depth: BIT_DEPTH,
        }));
    } else if (src_layout == Layout::Rgb) && (dst_layout == Layout::Rgb) {
        return Ok(Box::new(TransformShaperRgbFloatInOut::<
            T,
            { Layout::Rgb as u8 },
            { Layout::Rgb as u8 },
        > {
            profile,
            bit_depth: BIT_DEPTH,
        }));
    }
    Err(CmsError::UnsupportedProfileConnection)
}

impl<
    T: Clone + PointeeSizeExpressible + Copy + Default + 'static,
    const SRC_LAYOUT: u8,
    const DST_LAYOUT: u8,
    const LINEAR_CAP: usize,
    const BIT_DEPTH: usize,
> TransformExecutor<T>
    for TransformShaperFloatScalar<T, SRC_LAYOUT, DST_LAYOUT, LINEAR_CAP, BIT_DEPTH>
where
    u32: AsPrimitive<T>,
    f32: AsPrimitive<T>,
{
    fn transform(&self, src: &[T], dst: &mut [T]) -> Result<(), CmsError> {
        use crate::mlaf::mlaf;
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

        let transform = self.profile.adaptation_matrix;
        let max_colors: T = ((1 << BIT_DEPTH) - 1).as_();

        for (src, dst) in src
            .chunks_exact(src_channels)
            .zip(dst.chunks_exact_mut(dst_channels))
        {
            let r = self.profile.r_linear[src[src_cn.r_i()]._as_usize()];
            let g = self.profile.g_linear[src[src_cn.g_i()]._as_usize()];
            let b = self.profile.b_linear[src[src_cn.b_i()]._as_usize()];
            let a = if src_channels == 4 {
                src[src_cn.a_i()]
            } else {
                max_colors
            };

            let new_r = mlaf(
                mlaf(r * transform.v[0][0], g, transform.v[0][1]),
                b,
                transform.v[0][2],
            );

            let new_g = mlaf(
                mlaf(r * transform.v[1][0], g, transform.v[1][1]),
                b,
                transform.v[1][2],
            );

            let new_b = mlaf(
                mlaf(r * transform.v[2][0], g, transform.v[2][1]),
                b,
                transform.v[2][2],
            );

            let mut rgb = Rgb::new(new_r, new_g, new_b);
            rgb = self.profile.gamma_evaluator.evaluate_tristimulus(rgb);

            dst[dst_cn.r_i()] = rgb.r.as_();
            dst[dst_cn.g_i()] = rgb.g.as_();
            dst[dst_cn.b_i()] = rgb.b.as_();
            if dst_channels == 4 {
                dst[dst_cn.a_i()] = a;
            }
        }

        Ok(())
    }
}

impl<
    T: Clone + PointeeSizeExpressible + Copy + Default + 'static + AsPrimitive<f32>,
    const SRC_LAYOUT: u8,
    const DST_LAYOUT: u8,
> TransformExecutor<T> for TransformShaperRgbFloatInOut<T, SRC_LAYOUT, DST_LAYOUT>
where
    u32: AsPrimitive<T>,
    f32: AsPrimitive<T>,
{
    fn transform(&self, src: &[T], dst: &mut [T]) -> Result<(), CmsError> {
        use crate::mlaf::mlaf;
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

        let transform = self.profile.adaptation_matrix;
        let max_colors: T = ((1 << self.bit_depth) - 1).as_();

        for (src, dst) in src
            .chunks_exact(src_channels)
            .zip(dst.chunks_exact_mut(dst_channels))
        {
            let mut src_rgb = Rgb::new(
                src[src_cn.r_i()].as_(),
                src[src_cn.g_i()].as_(),
                src[src_cn.b_i()].as_(),
            );
            src_rgb = self.profile.linear_evaluator.evaluate_tristimulus(src_rgb);
            let r = src_rgb.r;
            let g = src_rgb.g;
            let b = src_rgb.b;
            let a = if src_channels == 4 {
                src[src_cn.a_i()]
            } else {
                max_colors
            };

            let new_r = mlaf(
                mlaf(r * transform.v[0][0], g, transform.v[0][1]),
                b,
                transform.v[0][2],
            );

            let new_g = mlaf(
                mlaf(r * transform.v[1][0], g, transform.v[1][1]),
                b,
                transform.v[1][2],
            );

            let new_b = mlaf(
                mlaf(r * transform.v[2][0], g, transform.v[2][1]),
                b,
                transform.v[2][2],
            );

            let mut rgb = Rgb::new(new_r, new_g, new_b);
            rgb = self.profile.gamma_evaluator.evaluate_tristimulus(rgb);

            dst[dst_cn.r_i()] = rgb.r.as_();
            dst[dst_cn.g_i()] = rgb.g.as_();
            dst[dst_cn.b_i()] = rgb.b.as_();

            if dst_channels == 4 {
                dst[dst_cn.a_i()] = a;
            }
        }

        Ok(())
    }
}
