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
use crate::transform::PointeeSizeExpressible;
use crate::{CmsError, Layout, TransformExecutor};
use num_traits::AsPrimitive;

#[derive(Clone)]
struct TransformGray2RgbFusedExecutor<
    T,
    const SRC_LAYOUT: u8,
    const DEST_LAYOUT: u8,
    const BUCKET: usize,
> {
    fused_gamma: Box<[T; BUCKET]>,
    bit_depth: usize,
}

pub(crate) fn make_gray_to_x<
    T: Copy + Default + PointeeSizeExpressible + 'static + Send + Sync,
    const BUCKET: usize,
>(
    src_layout: Layout,
    dst_layout: Layout,
    gray_linear: &[f32; BUCKET],
    gray_gamma: &[T; 65536],
    bit_depth: usize,
    gamma_lut: usize,
) -> Result<Box<dyn TransformExecutor<T> + Sync + Send>, CmsError>
where
    u32: AsPrimitive<T>,
{
    if src_layout != Layout::Gray && src_layout != Layout::GrayAlpha {
        return Err(CmsError::UnsupportedProfileConnection);
    }

    let mut fused_gamma = Box::new([T::default(); BUCKET]);
    let max_lut_size = (gamma_lut - 1) as f32;
    for (&src, dst) in gray_linear.iter().zip(fused_gamma.iter_mut()) {
        let possible_value = ((src * max_lut_size).round() as u32).min(max_lut_size as u32) as u16;
        *dst = gray_gamma[possible_value as usize];
    }

    match src_layout {
        Layout::Gray => match dst_layout {
            Layout::Rgb => Ok(Box::new(TransformGray2RgbFusedExecutor::<
                T,
                { Layout::Gray as u8 },
                { Layout::Rgb as u8 },
                BUCKET,
            > {
                fused_gamma,
                bit_depth,
            })),
            Layout::Rgba => Ok(Box::new(TransformGray2RgbFusedExecutor::<
                T,
                { Layout::Gray as u8 },
                { Layout::Rgba as u8 },
                BUCKET,
            > {
                fused_gamma,
                bit_depth,
            })),
            Layout::Gray => Ok(Box::new(TransformGray2RgbFusedExecutor::<
                T,
                { Layout::Gray as u8 },
                { Layout::Gray as u8 },
                BUCKET,
            > {
                fused_gamma,
                bit_depth,
            })),
            Layout::GrayAlpha => Ok(Box::new(TransformGray2RgbFusedExecutor::<
                T,
                { Layout::Gray as u8 },
                { Layout::GrayAlpha as u8 },
                BUCKET,
            > {
                fused_gamma,
                bit_depth,
            })),
            _ => unreachable!(),
        },
        Layout::GrayAlpha => match dst_layout {
            Layout::Rgb => Ok(Box::new(TransformGray2RgbFusedExecutor::<
                T,
                { Layout::Gray as u8 },
                { Layout::GrayAlpha as u8 },
                BUCKET,
            > {
                fused_gamma,
                bit_depth,
            })),
            Layout::Rgba => Ok(Box::new(TransformGray2RgbFusedExecutor::<
                T,
                { Layout::Gray as u8 },
                { Layout::Rgba as u8 },
                BUCKET,
            > {
                fused_gamma,
                bit_depth,
            })),
            Layout::Gray => Ok(Box::new(TransformGray2RgbFusedExecutor::<
                T,
                { Layout::Gray as u8 },
                { Layout::Gray as u8 },
                BUCKET,
            > {
                fused_gamma,
                bit_depth,
            })),
            Layout::GrayAlpha => Ok(Box::new(TransformGray2RgbFusedExecutor::<
                T,
                { Layout::GrayAlpha as u8 },
                { Layout::GrayAlpha as u8 },
                BUCKET,
            > {
                fused_gamma,
                bit_depth,
            })),
            _ => unreachable!(),
        },
        _ => Err(CmsError::UnsupportedProfileConnection),
    }
}

impl<
    T: Copy + Default + PointeeSizeExpressible + 'static,
    const SRC_LAYOUT: u8,
    const DST_LAYOUT: u8,
    const BUCKET: usize,
> TransformExecutor<T> for TransformGray2RgbFusedExecutor<T, SRC_LAYOUT, DST_LAYOUT, BUCKET>
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

        let is_gray_alpha = src_cn == Layout::GrayAlpha;

        let max_value: T = ((1u32 << self.bit_depth as u32) - 1u32).as_();

        for (src, dst) in src
            .chunks_exact(src_channels)
            .zip(dst.chunks_exact_mut(dst_channels))
        {
            let g = self.fused_gamma[src[0]._as_usize()];
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

#[derive(Clone)]
struct TransformGrayToRgbExecutor<
    T,
    const SRC_LAYOUT: u8,
    const DEST_LAYOUT: u8,
    const BUCKET: usize,
> {
    gray_linear: Box<[f32; BUCKET]>,
    red_gamma: Box<[T; 65536]>,
    green_gamma: Box<[T; 65536]>,
    blue_gamma: Box<[T; 65536]>,
    bit_depth: usize,
    gamma_lut: usize,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn make_gray_to_unfused<
    T: Copy + Default + PointeeSizeExpressible + 'static + Send + Sync,
    const BUCKET: usize,
>(
    src_layout: Layout,
    dst_layout: Layout,
    gray_linear: Box<[f32; BUCKET]>,
    red_gamma: Box<[T; 65536]>,
    green_gamma: Box<[T; 65536]>,
    blue_gamma: Box<[T; 65536]>,
    bit_depth: usize,
    gamma_lut: usize,
) -> Result<Box<dyn TransformExecutor<T> + Sync + Send>, CmsError>
where
    u32: AsPrimitive<T>,
{
    if src_layout != Layout::Gray && src_layout != Layout::GrayAlpha {
        return Err(CmsError::UnsupportedProfileConnection);
    }
    if dst_layout != Layout::Rgb && dst_layout != Layout::Rgba {
        return Err(CmsError::UnsupportedProfileConnection);
    }
    match src_layout {
        Layout::Gray => match dst_layout {
            Layout::Rgb => Ok(Box::new(TransformGrayToRgbExecutor::<
                T,
                { Layout::Gray as u8 },
                { Layout::Rgb as u8 },
                BUCKET,
            > {
                gray_linear,
                red_gamma,
                green_gamma,
                blue_gamma,
                bit_depth,
                gamma_lut,
            })),
            Layout::Rgba => Ok(Box::new(TransformGrayToRgbExecutor::<
                T,
                { Layout::Gray as u8 },
                { Layout::Rgba as u8 },
                BUCKET,
            > {
                gray_linear,
                red_gamma,
                green_gamma,
                blue_gamma,
                bit_depth,
                gamma_lut,
            })),
            Layout::Gray => Ok(Box::new(TransformGrayToRgbExecutor::<
                T,
                { Layout::Gray as u8 },
                { Layout::Gray as u8 },
                BUCKET,
            > {
                gray_linear,
                red_gamma,
                green_gamma,
                blue_gamma,
                bit_depth,
                gamma_lut,
            })),
            Layout::GrayAlpha => Ok(Box::new(TransformGrayToRgbExecutor::<
                T,
                { Layout::Gray as u8 },
                { Layout::GrayAlpha as u8 },
                BUCKET,
            > {
                gray_linear,
                red_gamma,
                green_gamma,
                blue_gamma,
                bit_depth,
                gamma_lut,
            })),
            _ => Err(CmsError::UnsupportedProfileConnection),
        },
        Layout::GrayAlpha => match dst_layout {
            Layout::Rgb => Ok(Box::new(TransformGrayToRgbExecutor::<
                T,
                { Layout::Gray as u8 },
                { Layout::GrayAlpha as u8 },
                BUCKET,
            > {
                gray_linear,
                red_gamma,
                green_gamma,
                blue_gamma,
                bit_depth,
                gamma_lut,
            })),
            Layout::Rgba => Ok(Box::new(TransformGrayToRgbExecutor::<
                T,
                { Layout::Gray as u8 },
                { Layout::Rgba as u8 },
                BUCKET,
            > {
                gray_linear,
                red_gamma,
                green_gamma,
                blue_gamma,
                bit_depth,
                gamma_lut,
            })),
            Layout::Gray => Ok(Box::new(TransformGrayToRgbExecutor::<
                T,
                { Layout::Gray as u8 },
                { Layout::Gray as u8 },
                BUCKET,
            > {
                gray_linear,
                red_gamma,
                green_gamma,
                blue_gamma,
                bit_depth,
                gamma_lut,
            })),
            Layout::GrayAlpha => Ok(Box::new(TransformGrayToRgbExecutor::<
                T,
                { Layout::GrayAlpha as u8 },
                { Layout::GrayAlpha as u8 },
                BUCKET,
            > {
                gray_linear,
                red_gamma,
                green_gamma,
                blue_gamma,
                bit_depth,
                gamma_lut,
            })),
            _ => Err(CmsError::UnsupportedProfileConnection),
        },
        _ => Err(CmsError::UnsupportedProfileConnection),
    }
}

impl<
    T: Copy + Default + PointeeSizeExpressible + 'static,
    const SRC_LAYOUT: u8,
    const DST_LAYOUT: u8,
    const BUCKET: usize,
> TransformExecutor<T> for TransformGrayToRgbExecutor<T, SRC_LAYOUT, DST_LAYOUT, BUCKET>
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

        let is_gray_alpha = src_cn == Layout::GrayAlpha;

        let max_value: T = ((1u32 << self.bit_depth as u32) - 1u32).as_();
        let max_lut_size = (self.gamma_lut - 1) as f32;

        for (src, dst) in src
            .chunks_exact(src_channels)
            .zip(dst.chunks_exact_mut(dst_channels))
        {
            let g = self.gray_linear[src[0]._as_usize()];
            let a = if is_gray_alpha { src[1] } else { max_value };

            let possible_value = ((g * max_lut_size).round() as u16) as usize;
            let red_value = self.red_gamma[possible_value];
            let green_value = self.green_gamma[possible_value];
            let blue_value = self.blue_gamma[possible_value];

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
