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
use crate::conversions::mab::{BCurves3, MCurves3};
use crate::safe_math::SafeMul;
use crate::{
    CmsError, Cube, DataColorSpace, InPlaceStage, InterpolationMethod, LutMultidimensionalType,
    MalformedSize, Matrix3d, Stage, TransformOptions, Vector3d, Vector4f,
};

struct ACurves3x4Inverse<'a, const DEPTH: usize> {
    curve0: Box<[f32; 65536]>,
    curve1: Box<[f32; 65536]>,
    curve2: Box<[f32; 65536]>,
    curve3: Box<[f32; 65536]>,
    clut: &'a [f32],
    grid_size: [u8; 3],
    interpolation_method: InterpolationMethod,
    pcs: DataColorSpace,
}

struct ACurves3x4InverseOptimized<'a> {
    clut: &'a [f32],
    grid_size: [u8; 3],
    interpolation_method: InterpolationMethod,
    pcs: DataColorSpace,
}

impl<const DEPTH: usize> ACurves3x4Inverse<'_, DEPTH> {
    fn transform_impl<Fetch: Fn(f32, f32, f32) -> Vector4f>(
        &self,
        src: &[f32],
        dst: &mut [f32],
        fetch: Fetch,
    ) -> Result<(), CmsError> {
        let scale_value = (DEPTH as u32 - 1u32) as f32;

        assert_eq!(src.len() / 3, dst.len() / 4);

        for (src, dst) in src.chunks_exact(3).zip(dst.chunks_exact_mut(4)) {
            let interpolated = fetch(src[0], src[1], src[2]);
            let a0 = (interpolated.v[0] * scale_value).round().min(scale_value) as u16;
            let a1 = (interpolated.v[1] * scale_value).round().min(scale_value) as u16;
            let a2 = (interpolated.v[2] * scale_value).round().min(scale_value) as u16;
            let a3 = (interpolated.v[3] * scale_value).round().min(scale_value) as u16;
            let b0 = self.curve0[a0 as usize];
            let b1 = self.curve1[a1 as usize];
            let b2 = self.curve2[a2 as usize];
            let b3 = self.curve3[a3 as usize];
            dst[0] = b0;
            dst[1] = b1;
            dst[2] = b2;
            dst[3] = b3;
        }
        Ok(())
    }
}

impl ACurves3x4InverseOptimized<'_> {
    fn transform_impl<Fetch: Fn(f32, f32, f32) -> Vector4f>(
        &self,
        src: &[f32],
        dst: &mut [f32],
        fetch: Fetch,
    ) -> Result<(), CmsError> {
        assert_eq!(src.len() / 3, dst.len() / 4);

        for (src, dst) in src.chunks_exact(3).zip(dst.chunks_exact_mut(4)) {
            let interpolated = fetch(src[0], src[1], src[2]);
            let b0 = interpolated.v[0];
            let b1 = interpolated.v[1];
            let b2 = interpolated.v[2];
            let b3 = interpolated.v[3];
            dst[0] = b0;
            dst[1] = b1;
            dst[2] = b2;
            dst[3] = b3;
        }
        Ok(())
    }
}

impl<const DEPTH: usize> Stage for ACurves3x4Inverse<'_, DEPTH> {
    fn transform(&self, src: &[f32], dst: &mut [f32]) -> Result<(), CmsError> {
        let lut = Cube::new_cube(self.clut, self.grid_size);

        // If PCS is LAB then linear interpolation should be used
        if self.pcs == DataColorSpace::Lab || self.pcs == DataColorSpace::Xyz {
            return self.transform_impl(src, dst, |x, y, z| lut.trilinear_vec4(x, y, z));
        }

        match self.interpolation_method {
            #[cfg(feature = "options")]
            InterpolationMethod::Tetrahedral => {
                self.transform_impl(src, dst, |x, y, z| lut.tetra_vec4(x, y, z))?;
            }
            #[cfg(feature = "options")]
            InterpolationMethod::Pyramid => {
                self.transform_impl(src, dst, |x, y, z| lut.pyramid_vec4(x, y, z))?;
            }
            #[cfg(feature = "options")]
            InterpolationMethod::Prism => {
                self.transform_impl(src, dst, |x, y, z| lut.prism_vec4(x, y, z))?;
            }
            InterpolationMethod::Linear => {
                self.transform_impl(src, dst, |x, y, z| lut.trilinear_vec4(x, y, z))?;
            }
        }
        Ok(())
    }
}

impl Stage for ACurves3x4InverseOptimized<'_> {
    fn transform(&self, src: &[f32], dst: &mut [f32]) -> Result<(), CmsError> {
        let lut = Cube::new_cube(self.clut, self.grid_size);

        // If PCS is LAB then linear interpolation should be used
        if self.pcs == DataColorSpace::Lab || self.pcs == DataColorSpace::Xyz {
            return self.transform_impl(src, dst, |x, y, z| lut.trilinear_vec4(x, y, z));
        }

        match self.interpolation_method {
            #[cfg(feature = "options")]
            InterpolationMethod::Tetrahedral => {
                self.transform_impl(src, dst, |x, y, z| lut.tetra_vec4(x, y, z))?;
            }
            #[cfg(feature = "options")]
            InterpolationMethod::Pyramid => {
                self.transform_impl(src, dst, |x, y, z| lut.pyramid_vec4(x, y, z))?;
            }
            #[cfg(feature = "options")]
            InterpolationMethod::Prism => {
                self.transform_impl(src, dst, |x, y, z| lut.prism_vec4(x, y, z))?;
            }
            InterpolationMethod::Linear => {
                self.transform_impl(src, dst, |x, y, z| lut.trilinear_vec4(x, y, z))?;
            }
        }
        Ok(())
    }
}

pub(crate) fn prepare_mba_3x4(
    mab: &LutMultidimensionalType,
    lut: &mut [f32],
    options: TransformOptions,
    pcs: DataColorSpace,
) -> Result<Vec<f32>, CmsError> {
    if mab.num_input_channels != 3 && mab.num_output_channels != 4 {
        return Err(CmsError::UnsupportedProfileConnection);
    }

    const LERP_DEPTH: usize = 65536;
    const BP: usize = 13;
    const DEPTH: usize = 8192;

    if mab.b_curves.len() == 3 {
        let all_curves_linear = mab.b_curves.iter().all(|curve| curve.is_linear());

        if !all_curves_linear {
            let curves: Result<Vec<_>, _> = mab
                .b_curves
                .iter()
                .map(|c| {
                    c.build_linearize_table::<u16, LERP_DEPTH, BP>()
                        .ok_or(CmsError::InvalidTrcCurve)
                })
                .collect();

            let [curve0, curve1, curve2] =
                curves?.try_into().map_err(|_| CmsError::InvalidTrcCurve)?;
            let b_curves = BCurves3::<DEPTH> {
                curve0,
                curve1,
                curve2,
            };
            b_curves.transform(lut)?;
        }
    } else {
        return Err(CmsError::InvalidAtoBLut);
    }

    if mab.m_curves.len() == 3 {
        let all_curves_linear = mab.m_curves.iter().all(|curve| curve.is_linear());
        if !all_curves_linear
            || !mab.matrix.test_equality(Matrix3d::IDENTITY)
            || mab.bias.ne(&Vector3d::default())
        {
            let curves: Result<Vec<_>, _> = mab
                .m_curves
                .iter()
                .map(|c| {
                    c.build_linearize_table::<u16, LERP_DEPTH, BP>()
                        .ok_or(CmsError::InvalidTrcCurve)
                })
                .collect();

            let [curve0, curve1, curve2] =
                curves?.try_into().map_err(|_| CmsError::InvalidTrcCurve)?;

            let matrix = mab.matrix.to_f32();
            let bias = mab.bias.cast();
            let m_curves = MCurves3::<DEPTH> {
                curve0,
                curve1,
                curve2,
                matrix,
                bias,
                inverse: true,
            };
            m_curves.transform(lut)?;
        }
    }

    let mut new_lut = vec![0f32; (lut.len() / 3) * 4];

    if mab.a_curves.len() == 4 && mab.clut.is_some() {
        let clut = &mab.clut.as_ref().map(|x| x.to_clut_f32()).unwrap();

        let lut_grid = (mab.grid_points[0] as usize)
            .safe_mul(mab.grid_points[1] as usize)?
            .safe_mul(mab.grid_points[2] as usize)?
            .safe_mul(mab.num_output_channels as usize)?;
        if clut.len() != lut_grid {
            return Err(CmsError::MalformedClut(MalformedSize {
                size: clut.len(),
                expected: lut_grid,
            }));
        }

        let grid_size = [mab.grid_points[0], mab.grid_points[1], mab.grid_points[2]];

        let all_curves_linear = mab.a_curves.iter().all(|curve| curve.is_linear());

        if all_curves_linear {
            let a_curves = ACurves3x4InverseOptimized {
                clut,
                grid_size: [mab.grid_points[0], mab.grid_points[1], mab.grid_points[2]],
                interpolation_method: options.interpolation_method,
                pcs,
            };
            a_curves.transform(lut, &mut new_lut)?;
        } else {
            let curves: Result<Vec<_>, _> = mab
                .a_curves
                .iter()
                .map(|c| {
                    c.build_linearize_table::<u16, LERP_DEPTH, BP>()
                        .ok_or(CmsError::InvalidTrcCurve)
                })
                .collect();

            let [curve0, curve1, curve2, curve3] =
                curves?.try_into().map_err(|_| CmsError::InvalidTrcCurve)?;

            let a_curves = ACurves3x4Inverse::<DEPTH> {
                curve0,
                curve1,
                curve2,
                curve3,
                clut,
                grid_size,
                interpolation_method: options.interpolation_method,
                pcs,
            };
            a_curves.transform(lut, &mut new_lut)?;
        }
    } else {
        return Err(CmsError::UnsupportedProfileConnection);
    }

    Ok(new_lut)
}
