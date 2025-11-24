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
use crate::trc::{lut_interp_linear_float, lut_interp_linear_float_clamped};
use crate::{CmsError, DataColorSpace, InterpolationMethod, Stage};
use std::arch::aarch64::{vdupq_n_f32, vgetq_lane_f32, vmaxq_f32, vminq_f32};

#[derive(Default)]
pub(crate) struct Lut4x3Neon {
    pub(crate) linearization: [Vec<f32>; 4],
    pub(crate) clut: Vec<f32>,
    pub(crate) grid_size: u8,
    pub(crate) output: [Vec<f32>; 3],
    pub(crate) interpolation_method: InterpolationMethod,
    pub(crate) pcs: DataColorSpace,
}

impl Lut4x3Neon {
    fn transform_impl<Fetch: Fn(f32, f32, f32, f32) -> NeonVector>(
        &self,
        src: &[f32],
        dst: &mut [f32],
        fetch: Fetch,
    ) -> Result<(), CmsError> {
        let linearization_0 = &self.linearization[0];
        let linearization_1 = &self.linearization[1];
        let linearization_2 = &self.linearization[2];
        let linearization_3 = &self.linearization[3];
        for (dest, src) in dst.chunks_exact_mut(3).zip(src.chunks_exact(4)) {
            debug_assert!(self.grid_size as i32 >= 1);
            let linear_x = lut_interp_linear_float(src[0], linearization_0);
            let linear_y = lut_interp_linear_float(src[1], linearization_1);
            let linear_z = lut_interp_linear_float(src[2], linearization_2);
            let linear_w = lut_interp_linear_float(src[3], linearization_3);

            unsafe {
                let mut v = fetch(linear_x, linear_y, linear_z, linear_w).v;
                v = vmaxq_f32(v, vdupq_n_f32(0.));
                v = vminq_f32(v, vdupq_n_f32(1.));

                let pcs_x =
                    lut_interp_linear_float_clamped(vgetq_lane_f32::<0>(v), &self.output[0]);
                let pcs_y =
                    lut_interp_linear_float_clamped(vgetq_lane_f32::<1>(v), &self.output[1]);
                let pcs_z =
                    lut_interp_linear_float_clamped(vgetq_lane_f32::<2>(v), &self.output[2]);
                dest[0] = pcs_x;
                dest[1] = pcs_y;
                dest[2] = pcs_z;
            }
        }
        Ok(())
    }
}

macro_rules! dispatch_preheat {
    ($heater: ident) => {
        impl Stage for $heater {
            fn transform(&self, src: &[f32], dst: &mut [f32]) -> Result<(), CmsError> {
                let l_tbl = HypercubeNeon::new(
                    &self.clut,
                    [
                        self.grid_size,
                        self.grid_size,
                        self.grid_size,
                        self.grid_size,
                    ],
                    3,
                );

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
                        self.transform_impl(src, dst, |x, y, z, w| {
                            l_tbl.quadlinear_vec3(x, y, z, w)
                        })?
                    }
                }
                Ok(())
            }
        }
    };
}

dispatch_preheat!(Lut4x3Neon);
