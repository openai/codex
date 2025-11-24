/*
 * // Copyright (c) Radzivon Bartoshyk 7/2025. All rights reserved.
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
use crate::transform::PointeeSizeExpressible;
use crate::trc::ToneCurveEvaluator;
use crate::{CmsError, Layout, Rgb, TransformExecutor};
use num_traits::AsPrimitive;
use std::marker::PhantomData;

struct TransformGrayOneToOneExecutor<T, const SRC_LAYOUT: u8, const DEST_LAYOUT: u8> {
    linear_eval: Box<dyn ToneCurveEvaluator + Send + Sync>,
    gamma_eval: Box<dyn ToneCurveEvaluator + Send + Sync>,
    _phantom: PhantomData<T>,
    bit_depth: usize,
}

pub(crate) fn make_gray_to_one_trc_extended<
    T: Copy + Default + PointeeSizeExpressible + 'static + Send + Sync + AsPrimitive<f32>,
>(
    src_layout: Layout,
    dst_layout: Layout,
    linear_eval: Box<dyn ToneCurveEvaluator + Send + Sync>,
    gamma_eval: Box<dyn ToneCurveEvaluator + Send + Sync>,
    bit_depth: usize,
) -> Result<Box<dyn TransformExecutor<T> + Sync + Send>, CmsError>
where
    u32: AsPrimitive<T>,
    f32: AsPrimitive<T>,
{
    if src_layout != Layout::Gray && src_layout != Layout::GrayAlpha {
        return Err(CmsError::UnsupportedProfileConnection);
    }

    match src_layout {
        Layout::Gray => match dst_layout {
            Layout::Rgb => Ok(Box::new(TransformGrayOneToOneExecutor::<
                T,
                { Layout::Gray as u8 },
                { Layout::Rgb as u8 },
            > {
                linear_eval,
                gamma_eval,
                _phantom: PhantomData,
                bit_depth,
            })),
            Layout::Rgba => Ok(Box::new(TransformGrayOneToOneExecutor::<
                T,
                { Layout::Gray as u8 },
                { Layout::Rgba as u8 },
            > {
                linear_eval,
                gamma_eval,
                _phantom: PhantomData,
                bit_depth,
            })),
            Layout::Gray => Ok(Box::new(TransformGrayOneToOneExecutor::<
                T,
                { Layout::Gray as u8 },
                { Layout::Gray as u8 },
            > {
                linear_eval,
                gamma_eval,
                _phantom: PhantomData,
                bit_depth,
            })),
            Layout::GrayAlpha => Ok(Box::new(TransformGrayOneToOneExecutor::<
                T,
                { Layout::Gray as u8 },
                { Layout::GrayAlpha as u8 },
            > {
                linear_eval,
                gamma_eval,
                _phantom: PhantomData,
                bit_depth,
            })),
            _ => unreachable!(),
        },
        Layout::GrayAlpha => match dst_layout {
            Layout::Rgb => Ok(Box::new(TransformGrayOneToOneExecutor::<
                T,
                { Layout::Gray as u8 },
                { Layout::GrayAlpha as u8 },
            > {
                linear_eval,
                gamma_eval,
                _phantom: PhantomData,
                bit_depth,
            })),
            Layout::Rgba => Ok(Box::new(TransformGrayOneToOneExecutor::<
                T,
                { Layout::Gray as u8 },
                { Layout::Rgba as u8 },
            > {
                linear_eval,
                gamma_eval,
                _phantom: PhantomData,
                bit_depth,
            })),
            Layout::Gray => Ok(Box::new(TransformGrayOneToOneExecutor::<
                T,
                { Layout::Gray as u8 },
                { Layout::Gray as u8 },
            > {
                linear_eval,
                gamma_eval,
                _phantom: PhantomData,
                bit_depth,
            })),
            Layout::GrayAlpha => Ok(Box::new(TransformGrayOneToOneExecutor::<
                T,
                { Layout::GrayAlpha as u8 },
                { Layout::GrayAlpha as u8 },
            > {
                linear_eval,
                gamma_eval,
                _phantom: PhantomData,
                bit_depth,
            })),
            _ => unreachable!(),
        },
        _ => Err(CmsError::UnsupportedProfileConnection),
    }
}

impl<
    T: Copy + Default + PointeeSizeExpressible + 'static + AsPrimitive<f32>,
    const SRC_LAYOUT: u8,
    const DST_LAYOUT: u8,
> TransformExecutor<T> for TransformGrayOneToOneExecutor<T, SRC_LAYOUT, DST_LAYOUT>
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

        let is_gray_alpha = src_cn == Layout::GrayAlpha;

        let max_value: T = ((1u32 << self.bit_depth as u32) - 1u32).as_();

        for (src, dst) in src
            .chunks_exact(src_channels)
            .zip(dst.chunks_exact_mut(dst_channels))
        {
            let linear_value = self.linear_eval.evaluate_value(src[0].as_());
            let g = self.gamma_eval.evaluate_value(linear_value).as_();
            let a = if is_gray_alpha { src[1] } else { max_value };

            dst[0] = g;
            if dst_cn == Layout::GrayAlpha {
                dst[1] = a;
            } else if dst_cn == Layout::Rgb {
                dst[1] = g;
                dst[2] = g;
            } else if dst_cn == Layout::Rgba {
                dst[1] = g;
                dst[2] = g;
                dst[3] = a;
            }
        }

        Ok(())
    }
}

struct TransformGrayToRgbExtendedExecutor<T, const SRC_LAYOUT: u8, const DEST_LAYOUT: u8> {
    linear_eval: Box<dyn ToneCurveEvaluator + Send + Sync>,
    gamma_eval: Box<dyn ToneCurveEvaluator + Send + Sync>,
    _phantom: PhantomData<T>,
    bit_depth: usize,
}

pub(crate) fn make_gray_to_rgb_extended<
    T: Copy + Default + PointeeSizeExpressible + 'static + Send + Sync + AsPrimitive<f32>,
>(
    src_layout: Layout,
    dst_layout: Layout,
    linear_eval: Box<dyn ToneCurveEvaluator + Send + Sync>,
    gamma_eval: Box<dyn ToneCurveEvaluator + Send + Sync>,
    bit_depth: usize,
) -> Result<Box<dyn TransformExecutor<T> + Sync + Send>, CmsError>
where
    u32: AsPrimitive<T>,
    f32: AsPrimitive<T>,
{
    if src_layout != Layout::Gray && src_layout != Layout::GrayAlpha {
        return Err(CmsError::UnsupportedProfileConnection);
    }
    if dst_layout != Layout::Rgb && dst_layout != Layout::Rgba {
        return Err(CmsError::UnsupportedProfileConnection);
    }
    match src_layout {
        Layout::Gray => match dst_layout {
            Layout::Rgb => Ok(Box::new(TransformGrayToRgbExtendedExecutor::<
                T,
                { Layout::Gray as u8 },
                { Layout::Rgb as u8 },
            > {
                linear_eval,
                gamma_eval,
                _phantom: PhantomData,
                bit_depth,
            })),
            Layout::Rgba => Ok(Box::new(TransformGrayToRgbExtendedExecutor::<
                T,
                { Layout::Gray as u8 },
                { Layout::Rgba as u8 },
            > {
                linear_eval,
                gamma_eval,
                _phantom: PhantomData,
                bit_depth,
            })),
            Layout::Gray => Ok(Box::new(TransformGrayToRgbExtendedExecutor::<
                T,
                { Layout::Gray as u8 },
                { Layout::Gray as u8 },
            > {
                linear_eval,
                gamma_eval,
                _phantom: PhantomData,
                bit_depth,
            })),
            Layout::GrayAlpha => Ok(Box::new(TransformGrayToRgbExtendedExecutor::<
                T,
                { Layout::Gray as u8 },
                { Layout::GrayAlpha as u8 },
            > {
                linear_eval,
                gamma_eval,
                _phantom: PhantomData,
                bit_depth,
            })),
            _ => Err(CmsError::UnsupportedProfileConnection),
        },
        Layout::GrayAlpha => match dst_layout {
            Layout::Rgb => Ok(Box::new(TransformGrayToRgbExtendedExecutor::<
                T,
                { Layout::Gray as u8 },
                { Layout::GrayAlpha as u8 },
            > {
                linear_eval,
                gamma_eval,
                _phantom: PhantomData,
                bit_depth,
            })),
            Layout::Rgba => Ok(Box::new(TransformGrayToRgbExtendedExecutor::<
                T,
                { Layout::Gray as u8 },
                { Layout::Rgba as u8 },
            > {
                linear_eval,
                gamma_eval,
                _phantom: PhantomData,
                bit_depth,
            })),
            Layout::Gray => Ok(Box::new(TransformGrayToRgbExtendedExecutor::<
                T,
                { Layout::Gray as u8 },
                { Layout::Gray as u8 },
            > {
                linear_eval,
                gamma_eval,
                _phantom: PhantomData,
                bit_depth,
            })),
            Layout::GrayAlpha => Ok(Box::new(TransformGrayToRgbExtendedExecutor::<
                T,
                { Layout::GrayAlpha as u8 },
                { Layout::GrayAlpha as u8 },
            > {
                linear_eval,
                gamma_eval,
                _phantom: PhantomData,
                bit_depth,
            })),
            _ => Err(CmsError::UnsupportedProfileConnection),
        },
        _ => Err(CmsError::UnsupportedProfileConnection),
    }
}

impl<
    T: Copy + Default + PointeeSizeExpressible + 'static + AsPrimitive<f32>,
    const SRC_LAYOUT: u8,
    const DST_LAYOUT: u8,
> TransformExecutor<T> for TransformGrayToRgbExtendedExecutor<T, SRC_LAYOUT, DST_LAYOUT>
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

        let is_gray_alpha = src_cn == Layout::GrayAlpha;

        let max_value: T = ((1u32 << self.bit_depth as u32) - 1u32).as_();

        for (src, dst) in src
            .chunks_exact(src_channels)
            .zip(dst.chunks_exact_mut(dst_channels))
        {
            let linear_value = self.linear_eval.evaluate_value(src[0].as_());
            let a = if is_gray_alpha { src[1] } else { max_value };

            let tristimulus = self.gamma_eval.evaluate_tristimulus(Rgb::new(
                linear_value,
                linear_value,
                linear_value,
            ));

            let red_value = tristimulus.r.as_();
            let green_value = tristimulus.g.as_();
            let blue_value = tristimulus.b.as_();

            if dst_cn == Layout::Rgb {
                dst[0] = red_value;
                dst[1] = green_value;
                dst[2] = blue_value;
            } else if dst_cn == Layout::Rgba {
                dst[0] = red_value;
                dst[1] = green_value;
                dst[2] = blue_value;
                dst[3] = a;
            } else {
                return Err(CmsError::UnsupportedProfileConnection);
            }
        }

        Ok(())
    }
}
