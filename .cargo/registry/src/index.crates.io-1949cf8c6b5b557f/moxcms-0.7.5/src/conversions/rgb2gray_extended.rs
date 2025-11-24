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
use crate::mlaf::mlaf;
use crate::transform::PointeeSizeExpressible;
use crate::trc::ToneCurveEvaluator;
use crate::{CmsError, Layout, Rgb, TransformExecutor, Vector3f};
use num_traits::AsPrimitive;
use std::marker::PhantomData;

struct TransformRgbToGrayExtendedExecutor<T, const SRC_LAYOUT: u8, const DST_LAYOUT: u8> {
    linear_eval: Box<dyn ToneCurveEvaluator + Send + Sync>,
    gamma_eval: Box<dyn ToneCurveEvaluator + Send + Sync>,
    weights: Vector3f,
    _phantom: PhantomData<T>,
    bit_depth: usize,
}

pub(crate) fn make_rgb_to_gray_extended<
    T: Copy + Default + PointeeSizeExpressible + Send + Sync + 'static + AsPrimitive<f32>,
>(
    src_layout: Layout,
    dst_layout: Layout,
    linear_eval: Box<dyn ToneCurveEvaluator + Send + Sync>,
    gamma_eval: Box<dyn ToneCurveEvaluator + Send + Sync>,
    weights: Vector3f,
    bit_depth: usize,
) -> Box<dyn TransformExecutor<T> + Send + Sync>
where
    u32: AsPrimitive<T>,
    f32: AsPrimitive<T>,
{
    match src_layout {
        Layout::Rgb => match dst_layout {
            Layout::Rgb => unreachable!(),
            Layout::Rgba => unreachable!(),
            Layout::Gray => Box::new(TransformRgbToGrayExtendedExecutor::<
                T,
                { Layout::Rgb as u8 },
                { Layout::Gray as u8 },
            > {
                linear_eval,
                gamma_eval,
                weights,
                _phantom: PhantomData,
                bit_depth,
            }),
            Layout::GrayAlpha => Box::new(TransformRgbToGrayExtendedExecutor::<
                T,
                { Layout::Rgb as u8 },
                { Layout::GrayAlpha as u8 },
            > {
                linear_eval,
                gamma_eval,
                weights,
                _phantom: PhantomData,
                bit_depth,
            }),
            _ => unreachable!(),
        },
        Layout::Rgba => match dst_layout {
            Layout::Rgb => unreachable!(),
            Layout::Rgba => unreachable!(),
            Layout::Gray => Box::new(TransformRgbToGrayExtendedExecutor::<
                T,
                { Layout::Rgba as u8 },
                { Layout::Gray as u8 },
            > {
                linear_eval,
                gamma_eval,
                weights,
                _phantom: PhantomData,
                bit_depth,
            }),
            Layout::GrayAlpha => Box::new(TransformRgbToGrayExtendedExecutor::<
                T,
                { Layout::Rgba as u8 },
                { Layout::GrayAlpha as u8 },
            > {
                linear_eval,
                gamma_eval,
                weights,
                _phantom: PhantomData,
                bit_depth,
            }),
            _ => unreachable!(),
        },
        Layout::Gray => unreachable!(),
        Layout::GrayAlpha => unreachable!(),
        _ => unreachable!(),
    }
}

impl<
    T: Copy + Default + PointeeSizeExpressible + 'static + AsPrimitive<f32>,
    const SRC_LAYOUT: u8,
    const DST_LAYOUT: u8,
> TransformExecutor<T> for TransformRgbToGrayExtendedExecutor<T, SRC_LAYOUT, DST_LAYOUT>
where
    u32: AsPrimitive<T>,
    f32: AsPrimitive<T>,
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

        let max_value = ((1u32 << self.bit_depth) - 1).as_();

        for (src, dst) in src
            .chunks_exact(src_channels)
            .zip(dst.chunks_exact_mut(dst_channels))
        {
            let in_tristimulus = Rgb::<f32>::new(
                src[src_cn.r_i()].as_(),
                src[src_cn.g_i()].as_(),
                src[src_cn.b_i()].as_(),
            );
            let lin_tristimulus = self.linear_eval.evaluate_tristimulus(in_tristimulus);
            let a = if src_channels == 4 {
                src[src_cn.a_i()]
            } else {
                max_value
            };
            let grey = mlaf(
                mlaf(
                    self.weights.v[0] * lin_tristimulus.r,
                    self.weights.v[1],
                    lin_tristimulus.g,
                ),
                self.weights.v[2],
                lin_tristimulus.b,
            )
            .min(1.)
            .max(0.);
            let gamma_value = self.gamma_eval.evaluate_value(grey);
            dst[0] = gamma_value.as_();
            if dst_channels == 2 {
                dst[1] = a;
            }
        }

        Ok(())
    }
}
