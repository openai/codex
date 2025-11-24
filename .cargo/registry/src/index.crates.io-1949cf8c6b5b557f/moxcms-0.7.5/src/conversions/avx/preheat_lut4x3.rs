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
use crate::conversions::avx::hypercube::HypercubeAvx;
use crate::conversions::avx::interpolator::AvxVectorSse;
use crate::trc::{lut_interp_linear_float, lut_interp_linear_float_clamped};
use crate::{CmsError, DataColorSpace, InterpolationMethod, Stage};
use std::arch::x86_64::*;

#[derive(Default)]
pub(crate) struct Lut4x3AvxFma {
    pub(crate) linearization: [Vec<f32>; 4],
    pub(crate) clut: Vec<f32>,
    pub(crate) grid_size: u8,
    pub(crate) output: [Vec<f32>; 3],
    pub(crate) interpolation_method: InterpolationMethod,
    pub(crate) pcs: DataColorSpace,
}

impl Lut4x3AvxFma {
    #[allow(unused_unsafe)]
    #[target_feature(enable = "avx2", enable = "fma")]
    unsafe fn transform_impl<Fetch: Fn(f32, f32, f32, f32) -> AvxVectorSse>(
        &self,
        src: &[f32],
        dst: &mut [f32],
        fetch: Fetch,
    ) -> Result<(), CmsError> {
        let linearization_0 = &self.linearization[0];
        let linearization_1 = &self.linearization[1];
        let linearization_2 = &self.linearization[2];
        let linearization_3 = &self.linearization[3];
        unsafe {
            let ones = _mm_set1_ps(1.);
            for (dest, src) in dst.chunks_exact_mut(3).zip(src.chunks_exact(4)) {
                debug_assert!(self.grid_size as i32 >= 1);
                let linear_x = lut_interp_linear_float(src[0], linearization_0);
                let linear_y = lut_interp_linear_float(src[1], linearization_1);
                let linear_z = lut_interp_linear_float(src[2], linearization_2);
                let linear_w = lut_interp_linear_float(src[3], linearization_3);

                let mut v = fetch(linear_x, linear_y, linear_z, linear_w).v;
                v = _mm_max_ps(v, _mm_setzero_ps());
                v = _mm_min_ps(v, ones);

                let pcs_x = lut_interp_linear_float_clamped(
                    f32::from_bits(_mm_extract_ps::<0>(v) as u32),
                    &self.output[0],
                );
                let pcs_y = lut_interp_linear_float_clamped(
                    f32::from_bits(_mm_extract_ps::<1>(v) as u32),
                    &self.output[1],
                );
                let pcs_z = lut_interp_linear_float_clamped(
                    f32::from_bits(_mm_extract_ps::<2>(v) as u32),
                    &self.output[2],
                );
                dest[0] = pcs_x;
                dest[1] = pcs_y;
                dest[2] = pcs_z;
            }
        }
        Ok(())
    }
}

impl Stage for Lut4x3AvxFma {
    fn transform(&self, src: &[f32], dst: &mut [f32]) -> Result<(), CmsError> {
        let l_tbl = HypercubeAvx::new(
            &self.clut,
            [
                self.grid_size,
                self.grid_size,
                self.grid_size,
                self.grid_size,
            ],
            3,
        );

        assert!(std::arch::is_x86_feature_detected!("avx2"));
        assert!(std::arch::is_x86_feature_detected!("fma"));

        unsafe {
            // If Source PCS is LAB trilinear should be used
            if self.pcs == DataColorSpace::Lab {
                return self
                    .transform_impl(src, dst, |x, y, z, w| l_tbl.quadlinear_vec3(x, y, z, w));
            }

            match self.interpolation_method {
                #[cfg(feature = "options")]
                InterpolationMethod::Tetrahedral => {
                    self.transform_impl(src, dst, |x, y, z, w| l_tbl.tetra_vec3(x, y, z, w))?;
                }
                #[cfg(feature = "options")]
                InterpolationMethod::Pyramid => {
                    self.transform_impl(src, dst, |x, y, z, w| l_tbl.pyramid_vec3(x, y, z, w))?;
                }
                #[cfg(feature = "options")]
                InterpolationMethod::Prism => {
                    self.transform_impl(src, dst, |x, y, z, w| l_tbl.prism_vec3(x, y, z, w))?
                }
                InterpolationMethod::Linear => {
                    self.transform_impl(src, dst, |x, y, z, w| l_tbl.quadlinear_vec3(x, y, z, w))?
                }
            }
        }
        Ok(())
    }
}
