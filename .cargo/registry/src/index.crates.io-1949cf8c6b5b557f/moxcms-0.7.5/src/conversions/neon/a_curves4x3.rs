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
use crate::conversions::neon::hypercube::HypercubeNeon;
use crate::conversions::neon::interpolator::NeonVector;
use crate::{CmsError, DataColorSpace, InterpolationMethod, Stage};
use std::arch::aarch64::vgetq_lane_f32;

pub(crate) struct ACurves4x3Neon<'a, const DEPTH: usize> {
    pub(crate) curve0: Box<[f32; 65536]>,
    pub(crate) curve1: Box<[f32; 65536]>,
    pub(crate) curve2: Box<[f32; 65536]>,
    pub(crate) curve3: Box<[f32; 65536]>,
    pub(crate) clut: &'a [f32],
    pub(crate) grid_size: [u8; 4],
    pub(crate) interpolation_method: InterpolationMethod,
    pub(crate) pcs: DataColorSpace,
}

pub(crate) struct ACurves4x3NeonOptimizedNeon<'a> {
    pub(crate) clut: &'a [f32],
    pub(crate) grid_size: [u8; 4],
    pub(crate) interpolation_method: InterpolationMethod,
    pub(crate) pcs: DataColorSpace,
}

impl<const DEPTH: usize> ACurves4x3Neon<'_, DEPTH> {
    fn transform_impl<Fetch: Fn(f32, f32, f32, f32) -> NeonVector>(
        &self,
        src: &[f32],
        dst: &mut [f32],
        fetch: Fetch,
    ) -> Result<(), CmsError> {
        let scale_value = (DEPTH - 1) as f32;

        assert_eq!(src.len() / 4, dst.len() / 3);

        for (src, dst) in src.chunks_exact(4).zip(dst.chunks_exact_mut(3)) {
            let a0 = (src[0] * scale_value).round().min(scale_value) as u16;
            let a1 = (src[1] * scale_value).round().min(scale_value) as u16;
            let a2 = (src[2] * scale_value).round().min(scale_value) as u16;
            let a3 = (src[3] * scale_value).round().min(scale_value) as u16;
            let c = self.curve0[a0 as usize];
            let m = self.curve1[a1 as usize];
            let y = self.curve2[a2 as usize];
            let k = self.curve3[a3 as usize];

            let v = fetch(c, m, y, k).v;
            unsafe {
                dst[0] = vgetq_lane_f32::<0>(v);
                dst[1] = vgetq_lane_f32::<1>(v);
                dst[2] = vgetq_lane_f32::<2>(v);
            }
        }
        Ok(())
    }
}

impl ACurves4x3NeonOptimizedNeon<'_> {
    fn transform_impl<Fetch: Fn(f32, f32, f32, f32) -> NeonVector>(
        &self,
        src: &[f32],
        dst: &mut [f32],
        fetch: Fetch,
    ) -> Result<(), CmsError> {
        assert_eq!(src.len() / 4, dst.len() / 3);

        for (src, dst) in src.chunks_exact(4).zip(dst.chunks_exact_mut(3)) {
            let c = src[0];
            let m = src[1];
            let y = src[2];
            let k = src[3];

            let v = fetch(c, m, y, k).v;
            unsafe {
                dst[0] = vgetq_lane_f32::<0>(v);
                dst[1] = vgetq_lane_f32::<1>(v);
                dst[2] = vgetq_lane_f32::<2>(v);
            }
        }
        Ok(())
    }
}

impl<const DEPTH: usize> Stage for ACurves4x3Neon<'_, DEPTH> {
    fn transform(&self, src: &[f32], dst: &mut [f32]) -> Result<(), CmsError> {
        let lut = HypercubeNeon::new(self.clut, self.grid_size, 3);

        // If PCS is LAB then linear interpolation should be used
        if self.pcs == DataColorSpace::Lab || self.pcs == DataColorSpace::Xyz {
            return self.transform_impl(src, dst, |x, y, z, w| lut.quadlinear_vec3(x, y, z, w));
        }

        match self.interpolation_method {
            #[cfg(feature = "options")]
            InterpolationMethod::Tetrahedral => {
                self.transform_impl(src, dst, |x, y, z, w| lut.tetra_vec3(x, y, z, w))?;
            }
            #[cfg(feature = "options")]
            InterpolationMethod::Pyramid => {
                self.transform_impl(src, dst, |x, y, z, w| lut.pyramid_vec3(x, y, z, w))?;
            }
            #[cfg(feature = "options")]
            InterpolationMethod::Prism => {
                self.transform_impl(src, dst, |x, y, z, w| lut.prism_vec3(x, y, z, w))?;
            }
            InterpolationMethod::Linear => {
                self.transform_impl(src, dst, |x, y, z, w| lut.quadlinear_vec3(x, y, z, w))?;
            }
        }
        Ok(())
    }
}

impl Stage for ACurves4x3NeonOptimizedNeon<'_> {
    fn transform(&self, src: &[f32], dst: &mut [f32]) -> Result<(), CmsError> {
        let lut = HypercubeNeon::new(self.clut, self.grid_size, 3);

        // If PCS is LAB then linear interpolation should be used
        if self.pcs == DataColorSpace::Lab || self.pcs == DataColorSpace::Xyz {
            return self.transform_impl(src, dst, |x, y, z, w| lut.quadlinear_vec3(x, y, z, w));
        }

        match self.interpolation_method {
            #[cfg(feature = "options")]
            InterpolationMethod::Tetrahedral => {
                self.transform_impl(src, dst, |x, y, z, w| lut.tetra_vec3(x, y, z, w))?;
            }
            #[cfg(feature = "options")]
            InterpolationMethod::Pyramid => {
                self.transform_impl(src, dst, |x, y, z, w| lut.pyramid_vec3(x, y, z, w))?;
            }
            #[cfg(feature = "options")]
            InterpolationMethod::Prism => {
                self.transform_impl(src, dst, |x, y, z, w| lut.prism_vec3(x, y, z, w))?;
            }
            InterpolationMethod::Linear => {
                self.transform_impl(src, dst, |x, y, z, w| lut.quadlinear_vec3(x, y, z, w))?;
            }
        }
        Ok(())
    }
}
