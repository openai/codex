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
use crate::conversions::avx::cube::CubeAvxFma;
use crate::conversions::avx::interpolator::AvxVectorSse;
use crate::{CmsError, DataColorSpace, InPlaceStage, InterpolationMethod};
use std::arch::x86_64::*;

pub(crate) struct ACurves3AvxFma<'a, const DEPTH: usize> {
    pub(crate) curve0: Box<[f32; 65536]>,
    pub(crate) curve1: Box<[f32; 65536]>,
    pub(crate) curve2: Box<[f32; 65536]>,
    pub(crate) clut: &'a [f32],
    pub(crate) grid_size: [u8; 3],
    pub(crate) interpolation_method: InterpolationMethod,
    pub(crate) pcs: DataColorSpace,
}

pub(crate) struct ACurves3OptimizedAvxFma<'a> {
    pub(crate) clut: &'a [f32],
    pub(crate) grid_size: [u8; 3],
    pub(crate) interpolation_method: InterpolationMethod,
    pub(crate) pcs: DataColorSpace,
}

pub(crate) struct ACurves3InverseAvxFma<'a, const DEPTH: usize> {
    pub(crate) curve0: Box<[f32; 65536]>,
    pub(crate) curve1: Box<[f32; 65536]>,
    pub(crate) curve2: Box<[f32; 65536]>,
    pub(crate) clut: &'a [f32],
    pub(crate) grid_size: [u8; 3],
    pub(crate) interpolation_method: InterpolationMethod,
    pub(crate) pcs: DataColorSpace,
}

impl<const DEPTH: usize> ACurves3AvxFma<'_, DEPTH> {
    #[allow(unused_unsafe)]
    #[target_feature(enable = "avx2", enable = "fma")]
    unsafe fn transform_impl<Fetch: Fn(f32, f32, f32) -> AvxVectorSse>(
        &self,
        dst: &mut [f32],
        fetch: Fetch,
    ) -> Result<(), CmsError> {
        unsafe {
            let scale_value = (DEPTH - 1) as f32;

            for dst in dst.chunks_exact_mut(3) {
                let a0 = (dst[0] * scale_value).round().min(scale_value) as u16;
                let a1 = (dst[1] * scale_value).round().min(scale_value) as u16;
                let a2 = (dst[2] * scale_value).round().min(scale_value) as u16;
                let b0 = self.curve0[a0 as usize];
                let b1 = self.curve1[a1 as usize];
                let b2 = self.curve2[a2 as usize];
                let v = fetch(b0, b1, b2).v;
                dst[0] = f32::from_bits(_mm_extract_ps::<0>(v) as u32);
                dst[1] = f32::from_bits(_mm_extract_ps::<1>(v) as u32);
                dst[2] = f32::from_bits(_mm_extract_ps::<2>(v) as u32);
            }
        }
        Ok(())
    }
}

impl ACurves3OptimizedAvxFma<'_> {
    #[allow(unused_unsafe)]
    #[target_feature(enable = "avx2", enable = "fma")]
    unsafe fn transform_impl<Fetch: Fn(f32, f32, f32) -> AvxVectorSse>(
        &self,
        dst: &mut [f32],
        fetch: Fetch,
    ) -> Result<(), CmsError> {
        unsafe {
            for dst in dst.chunks_exact_mut(3) {
                let a0 = dst[0];
                let a1 = dst[1];
                let a2 = dst[2];
                let v = fetch(a0, a1, a2).v;
                dst[0] = f32::from_bits(_mm_extract_ps::<0>(v) as u32);
                dst[1] = f32::from_bits(_mm_extract_ps::<1>(v) as u32);
                dst[2] = f32::from_bits(_mm_extract_ps::<2>(v) as u32);
            }
        }
        Ok(())
    }
}

impl<const DEPTH: usize> InPlaceStage for ACurves3AvxFma<'_, DEPTH> {
    fn transform(&self, dst: &mut [f32]) -> Result<(), CmsError> {
        let lut = CubeAvxFma::new(self.clut, self.grid_size, 3);

        unsafe {
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
        }
        Ok(())
    }
}

impl InPlaceStage for ACurves3OptimizedAvxFma<'_> {
    fn transform(&self, dst: &mut [f32]) -> Result<(), CmsError> {
        let lut = CubeAvxFma::new(self.clut, self.grid_size, 3);

        unsafe {
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
        }
        Ok(())
    }
}

impl<const DEPTH: usize> ACurves3InverseAvxFma<'_, DEPTH> {
    #[allow(unused_unsafe)]
    #[target_feature(enable = "avx2", enable = "fma")]
    unsafe fn transform_impl<Fetch: Fn(f32, f32, f32) -> AvxVectorSse>(
        &self,
        dst: &mut [f32],
        fetch: Fetch,
    ) -> Result<(), CmsError> {
        unsafe {
            let v_scale_value = _mm_set1_ps((DEPTH as u32 - 1u32) as f32);
            for dst in dst.chunks_exact_mut(3) {
                let mut v = fetch(dst[0], dst[1], dst[2]).v;
                v = _mm_mul_ps(v, v_scale_value);
                v = _mm_min_ps(v, v_scale_value);
                v = _mm_max_ps(v, _mm_setzero_ps());
                let c = _mm_cvtps_epi32(v);
                let a0 = _mm_extract_epi32::<0>(c) as u16;
                let a1 = _mm_extract_epi32::<1>(c) as u16;
                let a2 = _mm_extract_epi32::<2>(c) as u16;
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

impl<const DEPTH: usize> InPlaceStage for ACurves3InverseAvxFma<'_, DEPTH> {
    fn transform(&self, dst: &mut [f32]) -> Result<(), CmsError> {
        let lut = CubeAvxFma::new(self.clut, self.grid_size, 3);

        unsafe {
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
        }
        Ok(())
    }
}
