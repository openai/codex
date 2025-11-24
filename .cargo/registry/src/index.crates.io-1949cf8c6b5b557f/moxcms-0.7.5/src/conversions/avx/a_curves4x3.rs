// /*
//  * // Copyright (c) Radzivon Bartoshyk 3/2025. All rights reserved.
//  * //
//  * // Redistribution and use in source and binary forms, with or without modification,
//  * // are permitted provided that the following conditions are met:
//  * //
//  * // 1.  Redistributions of source code must retain the above copyright notice, this
//  * // list of conditions and the following disclaimer.
//  * //
//  * // 2.  Redistributions in binary form must reproduce the above copyright notice,
//  * // this list of conditions and the following disclaimer in the documentation
//  * // and/or other materials provided with the distribution.
//  * //
//  * // 3.  Neither the name of the copyright holder nor the names of its
//  * // contributors may be used to endorse or promote products derived from
//  * // this software without specific prior written permission.
//  * //
//  * // THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
//  * // AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
//  * // IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
//  * // DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE
//  * // FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
//  * // DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
//  * // SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
//  * // CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
//  * // OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
//  * // OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
//  */
use crate::conversions::avx::hypercube::HypercubeAvx;
use crate::conversions::avx::interpolator::AvxVectorSse;
use crate::{CmsError, DataColorSpace, InterpolationMethod, Stage};
use std::arch::x86_64::*;

pub(crate) struct ACurves4x3AvxFma<'a, const DEPTH: usize> {
    pub(crate) curve0: Box<[f32; 65536]>,
    pub(crate) curve1: Box<[f32; 65536]>,
    pub(crate) curve2: Box<[f32; 65536]>,
    pub(crate) curve3: Box<[f32; 65536]>,
    pub(crate) clut: &'a [f32],
    pub(crate) grid_size: [u8; 4],
    pub(crate) interpolation_method: InterpolationMethod,
    pub(crate) pcs: DataColorSpace,
}

pub(crate) struct ACurves4x3AvxFmaOptimized<'a> {
    pub(crate) clut: &'a [f32],
    pub(crate) grid_size: [u8; 4],
    pub(crate) interpolation_method: InterpolationMethod,
    pub(crate) pcs: DataColorSpace,
}

impl<const DEPTH: usize> ACurves4x3AvxFma<'_, DEPTH> {
    #[allow(unused_unsafe)]
    #[target_feature(enable = "avx2", enable = "fma")]
    unsafe fn transform_impl<Fetch: Fn(f32, f32, f32, f32) -> AvxVectorSse>(
        &self,
        src: &[f32],
        dst: &mut [f32],
        fetch: Fetch,
    ) -> Result<(), CmsError> {
        let scale_value = (DEPTH - 1) as f32;

        assert_eq!(src.len() / 4, dst.len() / 3);

        unsafe {
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
                dst[0] = f32::from_bits(_mm_extract_ps::<0>(v) as u32);
                dst[1] = f32::from_bits(_mm_extract_ps::<1>(v) as u32);
                dst[2] = f32::from_bits(_mm_extract_ps::<2>(v) as u32);
            }
        }
        Ok(())
    }
}

impl ACurves4x3AvxFmaOptimized<'_> {
    #[allow(unused_unsafe)]
    #[target_feature(enable = "avx2", enable = "fma")]
    unsafe fn transform_impl<Fetch: Fn(f32, f32, f32, f32) -> AvxVectorSse>(
        &self,
        src: &[f32],
        dst: &mut [f32],
        fetch: Fetch,
    ) -> Result<(), CmsError> {
        assert_eq!(src.len() / 4, dst.len() / 3);
        unsafe {
            for (src, dst) in src.chunks_exact(4).zip(dst.chunks_exact_mut(3)) {
                let c = src[0];
                let m = src[1];
                let y = src[2];
                let k = src[3];

                let v = fetch(c, m, y, k).v;
                dst[0] = f32::from_bits(_mm_extract_ps::<0>(v) as u32);
                dst[1] = f32::from_bits(_mm_extract_ps::<1>(v) as u32);
                dst[2] = f32::from_bits(_mm_extract_ps::<2>(v) as u32);
            }
        }
        Ok(())
    }
}

impl<const DEPTH: usize> Stage for ACurves4x3AvxFma<'_, DEPTH> {
    fn transform(&self, src: &[f32], dst: &mut [f32]) -> Result<(), CmsError> {
        let lut = HypercubeAvx::new(self.clut, self.grid_size, 3);

        assert!(std::arch::is_x86_feature_detected!("avx2"));
        assert!(std::arch::is_x86_feature_detected!("fma"));

        unsafe {
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
        }

        Ok(())
    }
}

impl Stage for ACurves4x3AvxFmaOptimized<'_> {
    fn transform(&self, src: &[f32], dst: &mut [f32]) -> Result<(), CmsError> {
        let lut = HypercubeAvx::new(self.clut, self.grid_size, 3);

        assert!(std::arch::is_x86_feature_detected!("avx2"));
        assert!(std::arch::is_x86_feature_detected!("fma"));

        unsafe {
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
        }
        Ok(())
    }
}
