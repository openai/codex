/*
 * // Copyright (c) Radzivon Bartoshyk 8/2025. All rights reserved.
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
use crate::conversions::katana::KatanaPostFinalizationStage;
use crate::{CmsError, DataColorSpace, Layout, PointeeSizeExpressible};
use num_traits::AsPrimitive;
use std::marker::PhantomData;

pub(crate) struct InjectAlphaStage<I> {
    pub(crate) dst_layout: Layout,
    pub(crate) target_color_space: DataColorSpace,
    pub(crate) _phantom: PhantomData<I>,
    pub(crate) bit_depth: usize,
}

pub(crate) struct CopyAlphaStage<I> {
    pub(crate) src_layout: Layout,
    pub(crate) dst_layout: Layout,
    pub(crate) target_color_space: DataColorSpace,
    pub(crate) _phantom: PhantomData<I>,
}

impl<T: Copy + Default + AsPrimitive<f32> + PointeeSizeExpressible + Send + Sync>
    KatanaPostFinalizationStage<T> for InjectAlphaStage<T>
where
    f32: AsPrimitive<T>,
{
    fn finalize(&self, _: &[T], dst: &mut [T]) -> Result<(), CmsError> {
        let norm_value: T = (if T::FINITE {
            ((1u32 << self.bit_depth) - 1) as f32
        } else {
            1.0
        })
        .as_();
        if self.dst_layout == Layout::Rgba && self.target_color_space == DataColorSpace::Rgb {
            for dst in dst.chunks_exact_mut(self.dst_layout.channels()) {
                dst[3] = norm_value;
            }
        } else if self.dst_layout == Layout::GrayAlpha
            && self.target_color_space == DataColorSpace::Gray
        {
            for dst in dst.chunks_exact_mut(self.dst_layout.channels()) {
                dst[1] = norm_value;
            }
        }
        Ok(())
    }
}

impl<T: Copy + Default + AsPrimitive<f32> + PointeeSizeExpressible + Send + Sync>
    KatanaPostFinalizationStage<T> for CopyAlphaStage<T>
where
    f32: AsPrimitive<T>,
{
    fn finalize(&self, src: &[T], dst: &mut [T]) -> Result<(), CmsError> {
        if self.dst_layout == Layout::Rgba && self.target_color_space == DataColorSpace::Rgb {
            if self.src_layout == Layout::Rgba {
                for (src, dst) in src
                    .chunks_exact(self.src_layout.channels())
                    .zip(dst.chunks_exact_mut(self.dst_layout.channels()))
                {
                    dst[3] = src[3];
                }
            } else if self.src_layout == Layout::GrayAlpha {
                for (src, dst) in src
                    .chunks_exact(self.src_layout.channels())
                    .zip(dst.chunks_exact_mut(self.dst_layout.channels()))
                {
                    dst[3] = src[1];
                }
            }
        } else if self.dst_layout == Layout::GrayAlpha
            && self.target_color_space == DataColorSpace::Gray
        {
            if self.src_layout == Layout::Rgba {
                for (src, dst) in src
                    .chunks_exact(self.src_layout.channels())
                    .zip(dst.chunks_exact_mut(self.dst_layout.channels()))
                {
                    dst[1] = src[3];
                }
            } else if self.src_layout == Layout::GrayAlpha {
                for (src, dst) in src
                    .chunks_exact(self.src_layout.channels())
                    .zip(dst.chunks_exact_mut(self.dst_layout.channels()))
                {
                    dst[1] = src[1];
                }
            }
        }
        Ok(())
    }
}
