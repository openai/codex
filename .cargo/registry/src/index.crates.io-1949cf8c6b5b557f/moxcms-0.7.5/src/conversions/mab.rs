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
use crate::mlaf::mlaf;
use crate::safe_math::SafeMul;
use crate::{
    CmsError, Cube, DataColorSpace, InPlaceStage, InterpolationMethod, LutMultidimensionalType,
    MalformedSize, Matrix3d, Matrix3f, TransformOptions, Vector3d, Vector3f,
};

#[allow(unused)]
struct ACurves3<'a, const DEPTH: usize> {
    curve0: Box<[f32; 65536]>,
    curve1: Box<[f32; 65536]>,
    curve2: Box<[f32; 65536]>,
    clut: &'a [f32],
    grid_size: [u8; 3],
    interpolation_method: InterpolationMethod,
    pcs: DataColorSpace,
}

#[allow(unused)]
struct ACurves3Optimized<'a> {
    clut: &'a [f32],
    grid_size: [u8; 3],
    interpolation_method: InterpolationMethod,
    pcs: DataColorSpace,
}

#[allow(unused)]
impl<const DEPTH: usize> ACurves3<'_, DEPTH> {
    fn transform_impl<Fetch: Fn(f32, f32, f32) -> Vector3f>(
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
            let interpolated = fetch(b0, b1, b2);
            dst[0] = interpolated.v[0];
            dst[1] = interpolated.v[1];
            dst[2] = interpolated.v[2];
        }
        Ok(())
    }
}

#[allow(unused)]
impl ACurves3Optimized<'_> {
    fn transform_impl<Fetch: Fn(f32, f32, f32) -> Vector3f>(
        &self,
        dst: &mut [f32],
        fetch: Fetch,
    ) -> Result<(), CmsError> {
        for dst in dst.chunks_exact_mut(3) {
            let a0 = dst[0];
            let a1 = dst[1];
            let a2 = dst[2];
            let interpolated = fetch(a0, a1, a2);
            dst[0] = interpolated.v[0];
            dst[1] = interpolated.v[1];
            dst[2] = interpolated.v[2];
        }
        Ok(())
    }
}

impl<const DEPTH: usize> InPlaceStage for ACurves3<'_, DEPTH> {
    fn transform(&self, dst: &mut [f32]) -> Result<(), CmsError> {
        let lut = Cube::new_cube(self.clut, self.grid_size);

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

impl InPlaceStage for ACurves3Optimized<'_> {
    fn transform(&self, dst: &mut [f32]) -> Result<(), CmsError> {
        let lut = Cube::new_cube(self.clut, self.grid_size);

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

#[allow(unused)]
struct ACurves3Inverse<'a, const DEPTH: usize> {
    curve0: Box<[f32; 65536]>,
    curve1: Box<[f32; 65536]>,
    curve2: Box<[f32; 65536]>,
    clut: &'a [f32],
    grid_size: [u8; 3],
    interpolation_method: InterpolationMethod,
    pcs: DataColorSpace,
}

#[allow(unused)]
impl<const DEPTH: usize> ACurves3Inverse<'_, DEPTH> {
    fn transform_impl<Fetch: Fn(f32, f32, f32) -> Vector3f>(
        &self,
        dst: &mut [f32],
        fetch: Fetch,
    ) -> Result<(), CmsError> {
        let scale_value = (DEPTH as u32 - 1u32) as f32;

        for dst in dst.chunks_exact_mut(3) {
            let interpolated = fetch(dst[0], dst[1], dst[2]);
            let a0 = (interpolated.v[0] * scale_value).round().min(scale_value) as u16;
            let a1 = (interpolated.v[1] * scale_value).round().min(scale_value) as u16;
            let a2 = (interpolated.v[2] * scale_value).round().min(scale_value) as u16;
            let b0 = self.curve0[a0 as usize];
            let b1 = self.curve1[a1 as usize];
            let b2 = self.curve2[a2 as usize];
            dst[0] = b0;
            dst[1] = b1;
            dst[2] = b2;
        }
        Ok(())
    }
}

impl<const DEPTH: usize> InPlaceStage for ACurves3Inverse<'_, DEPTH> {
    fn transform(&self, dst: &mut [f32]) -> Result<(), CmsError> {
        let lut = Cube::new_cube(self.clut, self.grid_size);

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

pub(crate) struct MCurves3<const DEPTH: usize> {
    pub(crate) curve0: Box<[f32; 65536]>,
    pub(crate) curve1: Box<[f32; 65536]>,
    pub(crate) curve2: Box<[f32; 65536]>,
    pub(crate) matrix: Matrix3f,
    pub(crate) bias: Vector3f,
    pub(crate) inverse: bool,
}

impl<const DEPTH: usize> MCurves3<DEPTH> {
    fn execute_matrix_stage(&self, dst: &mut [f32]) {
        let m = self.matrix;
        let b = self.bias;

        if !m.test_equality(Matrix3f::IDENTITY) || !b.eq(&Vector3f::default()) {
            for dst in dst.chunks_exact_mut(3) {
                let x = dst[0];
                let y = dst[1];
                let z = dst[2];
                dst[0] = mlaf(mlaf(mlaf(b.v[0], x, m.v[0][0]), y, m.v[0][1]), z, m.v[0][2]);
                dst[1] = mlaf(mlaf(mlaf(b.v[1], x, m.v[1][0]), y, m.v[1][1]), z, m.v[1][2]);
                dst[2] = mlaf(mlaf(mlaf(b.v[2], x, m.v[2][0]), y, m.v[2][1]), z, m.v[2][2]);
            }
        }
    }
}

impl<const DEPTH: usize> InPlaceStage for MCurves3<DEPTH> {
    fn transform(&self, dst: &mut [f32]) -> Result<(), CmsError> {
        let scale_value = (DEPTH - 1) as f32;

        if self.inverse {
            self.execute_matrix_stage(dst);
        }

        for dst in dst.chunks_exact_mut(3) {
            let a0 = (dst[0] * scale_value).round().min(scale_value) as u16;
            let a1 = (dst[1] * scale_value).round().min(scale_value) as u16;
            let a2 = (dst[2] * scale_value).round().min(scale_value) as u16;
            let b0 = self.curve0[a0 as usize];
            let b1 = self.curve1[a1 as usize];
            let b2 = self.curve2[a2 as usize];
            dst[0] = b0;
            dst[1] = b1;
            dst[2] = b2;
        }

        if !self.inverse {
            self.execute_matrix_stage(dst);
        }

        Ok(())
    }
}

pub(crate) struct BCurves3<const DEPTH: usize> {
    pub(crate) curve0: Box<[f32; 65536]>,
    pub(crate) curve1: Box<[f32; 65536]>,
    pub(crate) curve2: Box<[f32; 65536]>,
}

impl<const DEPTH: usize> InPlaceStage for BCurves3<DEPTH> {
    fn transform(&self, dst: &mut [f32]) -> Result<(), CmsError> {
        let scale_value = (DEPTH - 1) as f32;

        for dst in dst.chunks_exact_mut(3) {
            let a0 = (dst[0] * scale_value).round().min(scale_value) as u16;
            let a1 = (dst[1] * scale_value).round().min(scale_value) as u16;
            let a2 = (dst[2] * scale_value).round().min(scale_value) as u16;
            let b0 = self.curve0[a0 as usize];
            let b1 = self.curve1[a1 as usize];
            let b2 = self.curve2[a2 as usize];
            dst[0] = b0;
            dst[1] = b1;
            dst[2] = b2;
        }

        Ok(())
    }
}

pub(crate) fn prepare_mab_3x3(
    mab: &LutMultidimensionalType,
    lut: &mut [f32],
    options: TransformOptions,
    pcs: DataColorSpace,
) -> Result<(), CmsError> {
    const LERP_DEPTH: usize = 65536;
    const BP: usize = 13;
    const DEPTH: usize = 8192;

    if mab.num_input_channels != 3 && mab.num_output_channels != 3 {
        return Err(CmsError::UnsupportedProfileConnection);
    }
    if mab.a_curves.len() == 3 && mab.clut.is_some() {
        let clut = &mab.clut.as_ref().map(|x| x.to_clut_f32()).unwrap();
        let lut_grid = (mab.grid_points[0] as usize)
            .safe_mul(mab.grid_points[1] as usize)?
            .safe_mul(mab.grid_points[2] as usize)?
            .safe_mul(mab.num_output_channels as usize)?;
        if clut.len() != lut_grid {
            return Err(CmsError::MalformedCurveLutTable(MalformedSize {
                size: clut.len(),
                expected: lut_grid,
            }));
        }

        let all_curves_linear = mab.a_curves.iter().all(|curve| curve.is_linear());
        let grid_size = [mab.grid_points[0], mab.grid_points[1], mab.grid_points[2]];

        #[cfg(all(target_arch = "aarch64", target_feature = "neon", feature = "neon"))]
        if all_curves_linear {
            use crate::conversions::neon::ACurves3OptimizedNeon;
            let a_curves = ACurves3OptimizedNeon {
                clut,
                grid_size,
                interpolation_method: options.interpolation_method,
                pcs,
            };
            a_curves.transform(lut)?;
        } else {
            use crate::conversions::neon::ACurves3Neon;
            let curves: Result<Vec<_>, _> = mab
                .a_curves
                .iter()
                .map(|c| {
                    c.build_linearize_table::<u16, LERP_DEPTH, BP>()
                        .ok_or(CmsError::InvalidTrcCurve)
                })
                .collect();

            let [curve0, curve1, curve2] =
                curves?.try_into().map_err(|_| CmsError::InvalidTrcCurve)?;
            let a_curves = ACurves3Neon::<DEPTH> {
                curve0,
                curve1,
                curve2,
                clut,
                grid_size,
                interpolation_method: options.interpolation_method,
                pcs,
            };
            a_curves.transform(lut)?;
        }

        #[cfg(not(all(target_arch = "aarch64", target_feature = "neon", feature = "neon")))]
        {
            let mut execution_box: Option<Box<dyn InPlaceStage>> = None;

            if all_curves_linear {
                #[cfg(all(target_arch = "x86_64", feature = "avx"))]
                {
                    use crate::conversions::avx::ACurves3OptimizedAvxFma;
                    if std::arch::is_x86_feature_detected!("avx2")
                        && std::arch::is_x86_feature_detected!("fma")
                    {
                        execution_box = Some(Box::new(ACurves3OptimizedAvxFma {
                            clut,
                            grid_size,
                            interpolation_method: options.interpolation_method,
                            pcs,
                        }));
                    }
                }
                if execution_box.is_none() {
                    execution_box = Some(Box::new(ACurves3Optimized {
                        clut,
                        grid_size,
                        interpolation_method: options.interpolation_method,
                        pcs,
                    }));
                }
            } else {
                #[cfg(all(target_arch = "x86_64", feature = "avx"))]
                {
                    use crate::conversions::avx::ACurves3AvxFma;
                    if std::arch::is_x86_feature_detected!("avx2")
                        && std::arch::is_x86_feature_detected!("fma")
                    {
                        let curves: Result<Vec<_>, _> = mab
                            .a_curves
                            .iter()
                            .map(|c| {
                                c.build_linearize_table::<u16, LERP_DEPTH, BP>()
                                    .ok_or(CmsError::InvalidTrcCurve)
                            })
                            .collect();

                        let [curve0, curve1, curve2] =
                            curves?.try_into().map_err(|_| CmsError::InvalidTrcCurve)?;
                        execution_box = Some(Box::new(ACurves3AvxFma::<DEPTH> {
                            curve0,
                            curve1,
                            curve2,
                            clut,
                            grid_size,
                            interpolation_method: options.interpolation_method,
                            pcs,
                        }));
                    }
                }

                if execution_box.is_none() {
                    let curves: Result<Vec<_>, _> = mab
                        .a_curves
                        .iter()
                        .map(|c| {
                            c.build_linearize_table::<u16, LERP_DEPTH, BP>()
                                .ok_or(CmsError::InvalidTrcCurve)
                        })
                        .collect();

                    let [curve0, curve1, curve2] =
                        curves?.try_into().map_err(|_| CmsError::InvalidTrcCurve)?;
                    execution_box = Some(Box::new(ACurves3::<DEPTH> {
                        curve0,
                        curve1,
                        curve2,
                        clut,
                        grid_size,
                        interpolation_method: options.interpolation_method,
                        pcs,
                    }));
                }
            }

            execution_box
                .expect("LUT Sampler on Multidimensional 3x3 must be set")
                .transform(lut)?;
        }
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
            let bias: Vector3f = mab.bias.cast();
            let m_curves = MCurves3::<DEPTH> {
                curve0,
                curve1,
                curve2,
                matrix,
                bias,
                inverse: false,
            };
            m_curves.transform(lut)?;
        }
    }

    if mab.b_curves.len() == 3 {
        const LERP_DEPTH: usize = 65536;
        const BP: usize = 13;
        const DEPTH: usize = 8192;
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

    Ok(())
}

pub(crate) fn prepare_mba_3x3(
    mab: &LutMultidimensionalType,
    lut: &mut [f32],
    options: TransformOptions,
    pcs: DataColorSpace,
) -> Result<(), CmsError> {
    if mab.num_input_channels != 3 && mab.num_output_channels != 3 {
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
            let bias: Vector3f = mab.bias.cast();
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

    if mab.a_curves.len() == 3 && mab.clut.is_some() {
        let clut = &mab.clut.as_ref().map(|x| x.to_clut_f32()).unwrap();
        let lut_grid = (mab.grid_points[0] as usize)
            .safe_mul(mab.grid_points[1] as usize)?
            .safe_mul(mab.grid_points[2] as usize)?
            .safe_mul(mab.num_output_channels as usize)?;
        if clut.len() != lut_grid {
            return Err(CmsError::MalformedCurveLutTable(MalformedSize {
                size: clut.len(),
                expected: lut_grid,
            }));
        }

        let all_curves_linear = mab.a_curves.iter().all(|curve| curve.is_linear());
        let grid_size = [mab.grid_points[0], mab.grid_points[1], mab.grid_points[2]];

        #[cfg(all(target_arch = "aarch64", target_feature = "neon", feature = "neon"))]
        if all_curves_linear {
            use crate::conversions::neon::ACurves3OptimizedNeon;
            let a_curves = ACurves3OptimizedNeon {
                clut,
                grid_size,
                interpolation_method: options.interpolation_method,
                pcs,
            };
            a_curves.transform(lut)?;
        } else {
            use crate::conversions::neon::ACurves3InverseNeon;
            let curves: Result<Vec<_>, _> = mab
                .a_curves
                .iter()
                .map(|c| {
                    c.build_linearize_table::<u16, LERP_DEPTH, BP>()
                        .ok_or(CmsError::InvalidTrcCurve)
                })
                .collect();

            let [curve0, curve1, curve2] =
                curves?.try_into().map_err(|_| CmsError::InvalidTrcCurve)?;
            let a_curves = ACurves3InverseNeon::<DEPTH> {
                curve0,
                curve1,
                curve2,
                clut,
                grid_size,
                interpolation_method: options.interpolation_method,
                pcs,
            };
            a_curves.transform(lut)?;
        }
        #[cfg(not(all(target_arch = "aarch64", target_feature = "neon", feature = "neon")))]
        {
            let mut execution_box: Option<Box<dyn InPlaceStage>> = None;

            if all_curves_linear {
                #[cfg(all(target_arch = "x86_64", feature = "avx"))]
                {
                    use crate::conversions::avx::ACurves3OptimizedAvxFma;
                    if std::arch::is_x86_feature_detected!("avx2")
                        && std::arch::is_x86_feature_detected!("fma")
                    {
                        execution_box = Some(Box::new(ACurves3OptimizedAvxFma {
                            clut,
                            grid_size,
                            interpolation_method: options.interpolation_method,
                            pcs,
                        }));
                    }
                }

                if execution_box.is_none() {
                    execution_box = Some(Box::new(ACurves3Optimized {
                        clut,
                        grid_size,
                        interpolation_method: options.interpolation_method,
                        pcs,
                    }));
                }
            } else {
                #[cfg(all(target_arch = "x86_64", feature = "avx"))]
                {
                    use crate::conversions::avx::ACurves3InverseAvxFma;
                    if std::arch::is_x86_feature_detected!("avx2")
                        && std::arch::is_x86_feature_detected!("fma")
                    {
                        let curves: Result<Vec<_>, _> = mab
                            .a_curves
                            .iter()
                            .map(|c| {
                                c.build_linearize_table::<u16, LERP_DEPTH, BP>()
                                    .ok_or(CmsError::InvalidTrcCurve)
                            })
                            .collect();

                        let [curve0, curve1, curve2] =
                            curves?.try_into().map_err(|_| CmsError::InvalidTrcCurve)?;
                        execution_box = Some(Box::new(ACurves3InverseAvxFma::<DEPTH> {
                            curve0,
                            curve1,
                            curve2,
                            clut,
                            grid_size,
                            interpolation_method: options.interpolation_method,
                            pcs,
                        }));
                    }
                }

                if execution_box.is_none() {
                    let curves: Result<Vec<_>, _> = mab
                        .a_curves
                        .iter()
                        .map(|c| {
                            c.build_linearize_table::<u16, LERP_DEPTH, BP>()
                                .ok_or(CmsError::InvalidTrcCurve)
                        })
                        .collect();

                    let [curve0, curve1, curve2] =
                        curves?.try_into().map_err(|_| CmsError::InvalidTrcCurve)?;
                    execution_box = Some(Box::new(ACurves3Inverse::<DEPTH> {
                        curve0,
                        curve1,
                        curve2,
                        clut,
                        grid_size,
                        interpolation_method: options.interpolation_method,
                        pcs,
                    }));
                }
            }

            execution_box
                .expect("LUT Sampler on Multidimensional Inverse 3x3 must be set")
                .transform(lut)?;
        }
    }

    Ok(())
}
