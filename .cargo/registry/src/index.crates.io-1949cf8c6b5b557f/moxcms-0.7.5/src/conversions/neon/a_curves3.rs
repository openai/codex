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
use crate::conversions::neon::cube::CubeNeon;
use crate::conversions::neon::interpolator::NeonVector;
use crate::{CmsError, DataColorSpace, InPlaceStage, InterpolationMethod};
use std::arch::aarch64::*;

pub(crate) struct ACurves3Neon<'a, const DEPTH: usize> {
    pub(crate) curve0: Box<[f32; 65536]>,
    pub(crate) curve1: Box<[f32; 65536]>,
    pub(crate) curve2: Box<[f32; 65536]>,
    pub(crate) clut: &'a [f32],
    pub(crate) grid_size: [u8; 3],
    pub(crate) interpolation_method: InterpolationMethod,
    pub(crate) pcs: DataColorSpace,
}

pub(crate) struct ACurves3OptimizedNeon<'a> {
    pub(crate) clut: &'a [f32],
    pub(crate) grid_size: [u8; 3],
    pub(crate) interpolation_method: InterpolationMethod,
    pub(crate) pcs: DataColorSpace,
}

pub(crate) struct ACurves3InverseNeon<'a, const DEPTH: usize> {
    pub(crate) curve0: Box<[f32; 65536]>,
    pub(crate) curve1: Box<[f32; 65536]>,
    pub(crate) curve2: Box<[f32; 65536]>,
    pub(crate) clut: &'a [f32],
    pub(crate) grid_size: [u8; 3],
    pub(crate) interpolation_method: InterpolationMethod,
    pub(crate) pcs: DataColorSpace,
}

impl<const DEPTH: usize> ACurves3Neon<'_, DEPTH> {
    fn transform_impl<Fetch: Fn(f32, f32, f32) -> NeonVector>(
        &self,
        dst: &mut [f32],
        fetch: Fetch,
    ) -> Result<(), CmsError> {
        let scale_value = (DEPTH - 1) as f32;

        for dst in dst.chunks_exact_mut(3) {
            let a0 = (dst[0] * scale_value).round().min(scale_value) as u16;
            let a1 = (dst[1] * scale_value).round().min(scale_value) as u16;
            let a2 = (dst[2] * scale_value).round().min(scale_value) as u16;
            let b0 = self.curve0[a0 as usize];
            let b1 = self.curve1[a1 as usize];
            let b2 = self.curve2[a2 as usize];
            let v = fetch(b0, b1, b2).v;
            unsafe {
                dst[0] = vgetq_lane_f32::<0>(v);
                dst[1] = vgetq_lane_f32::<1>(v);
                dst[2] = vgetq_lane_f32::<2>(v);
            }
        }
        Ok(())
    }
}

impl ACurves3OptimizedNeon<'_> {
    fn transform_impl<Fetch: Fn(f32, f32, f32) -> NeonVector>(
        &self,
        dst: &mut [f32],
        fetch: Fetch,
    ) -> Result<(), CmsError> {
        for dst in dst.chunks_exact_mut(3) {
            let a0 = dst[0];
            let a1 = dst[1];
            let a2 = dst[2];
            let v = fetch(a0, a1, a2).v;
            unsafe {
                dst[0] = vgetq_lane_f32::<0>(v);
                dst[1] = vgetq_lane_f32::<1>(v);
                dst[2] = vgetq_lane_f32::<2>(v);
            }
        }
        Ok(())
    }
}

impl<const DEPTH: usize> InPlaceStage for ACurves3Neon<'_, DEPTH> {
    fn transform(&self, dst: &mut [f32]) -> Result<(), CmsError> {
        let lut = CubeNeon::new(self.clut, self.grid_size, 3);

        // If PCS is LAB then linear interpolation should be used
        if self.pcs == DataColorSpace::Lab || self.pcs == DataColorSpace::Xyz {
            return self.transform_impl(dst, |x, y, z| lut.trilinear_vec3(x, y, z));
        }

        match self.interpolation_method {
            #[cfg(feature = "options")]
            InterpolationMethod::Tetrahedral => {
                self.transform_impl(dst, |x, y, z| lut.tetra_vec3(x, y, z))?;
            }
            #[cfg(feature = "options")]
            InterpolationMethod::Pyramid => {
                self.transform_impl(dst, |x, y, z| lut.pyramid_vec3(x, y, z))?;
            }
            #[cfg(feature = "options")]
            InterpolationMethod::Prism => {
                self.transform_impl(dst, |x, y, z| lut.prism_vec3(x, y, z))?;
            }
            InterpolationMethod::Linear => {
                self.transform_impl(dst, |x, y, z| lut.trilinear_vec3(x, y, z))?;
            }
        }
        Ok(())
    }
}

impl InPlaceStage for ACurves3OptimizedNeon<'_> {
    fn transform(&self, dst: &mut [f32]) -> Result<(), CmsError> {
        let lut = CubeNeon::new(self.clut, self.grid_size, 3);

        // If PCS is LAB then linear interpolation should be used
        if self.pcs == DataColorSpace::Lab {
            return self.transform_impl(dst, |x, y, z| lut.trilinear_vec3(x, y, z));
        }

        match self.interpolation_method {
            #[cfg(feature = "options")]
            InterpolationMethod::Tetrahedral => {
                self.transform_impl(dst, |x, y, z| lut.tetra_vec3(x, y, z))?;
            }
            #[cfg(feature = "options")]
            InterpolationMethod::Pyramid => {
                self.transform_impl(dst, |x, y, z| lut.pyramid_vec3(x, y, z))?;
            }
            #[cfg(feature = "options")]
            InterpolationMethod::Prism => {
                self.transform_impl(dst, |x, y, z| lut.prism_vec3(x, y, z))?;
            }
            InterpolationMethod::Linear => {
                self.transform_impl(dst, |x, y, z| lut.trilinear_vec3(x, y, z))?;
            }
        }
        Ok(())
    }
}

impl<const DEPTH: usize> ACurves3InverseNeon<'_, DEPTH> {
    fn transform_impl<Fetch: Fn(f32, f32, f32) -> NeonVector>(
        &self,
        dst: &mut [f32],
        fetch: Fetch,
    ) -> Result<(), CmsError> {
        let v_scale_value = unsafe { vdupq_n_f32((DEPTH as u32 - 1u32) as f32) };

        unsafe {
            for dst in dst.chunks_exact_mut(3) {
                let mut v = fetch(dst[0], dst[1], dst[2]).v;
                v = vmulq_f32(v, v_scale_value);
                v = vminq_f32(v, v_scale_value);
                let c = vcvtaq_u32_f32(v);
                let a0 = vgetq_lane_u32::<0>(c) as u16;
                let a1 = vgetq_lane_u32::<1>(c) as u16;
                let a2 = vgetq_lane_u32::<2>(c) as u16;
                let b0 = self.curve0[a0 as usize];
                let b1 = self.curve1[a1 as usize];
                let b2 = self.curve2[a2 as usize];
                dst[0] = b0;
                dst[1] = b1;
                dst[2] = b2;
            }
        }
        Ok(())
    }
}

impl<const DEPTH: usize> InPlaceStage for ACurves3InverseNeon<'_, DEPTH> {
    fn transform(&self, dst: &mut [f32]) -> Result<(), CmsError> {
        let lut = CubeNeon::new(self.clut, self.grid_size, 3);

        // If PCS is LAB then linear interpolation should be used
        if self.pcs == DataColorSpace::Lab || self.pcs == DataColorSpace::Xyz {
            return self.transform_impl(dst, |x, y, z| lut.trilinear_vec3(x, y, z));
        }

        match self.interpolation_method {
            #[cfg(feature = "options")]
            InterpolationMethod::Tetrahedral => {
                self.transform_impl(dst, |x, y, z| lut.tetra_vec3(x, y, z))?;
            }
            #[cfg(feature = "options")]
            InterpolationMethod::Pyramid => {
                self.transform_impl(dst, |x, y, z| lut.pyramid_vec3(x, y, z))?;
            }
            #[cfg(feature = "options")]
            InterpolationMethod::Prism => {
                self.transform_impl(dst, |x, y, z| lut.prism_vec3(x, y, z))?;
            }
            InterpolationMethod::Linear => {
                self.transform_impl(dst, |x, y, z| lut.trilinear_vec3(x, y, z))?;
            }
        }
        Ok(())
    }
}
