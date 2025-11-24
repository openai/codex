/*
 * // Copyright (c) Radzivon Bartoshyk 6/2025. All rights reserved.
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
use crate::conversions::katana::KatanaInitialStage;
use crate::conversions::katana::md3x3::MultidimensionalDirection;
use crate::mlaf::mlaf;
use crate::safe_math::SafeMul;
use crate::trc::lut_interp_linear_float;
use crate::{
    CmsError, DataColorSpace, Hypercube, InterpolationMethod, LutMultidimensionalType,
    MalformedSize, Matrix3d, Matrix3f, PointeeSizeExpressible, TransformOptions, Vector3d,
    Vector3f,
};
use num_traits::AsPrimitive;
use std::marker::PhantomData;

pub(crate) fn execute_simple_curves3(dst: &mut [f32], curves: &[Vec<f32>; 3]) {
    let curve0 = &curves[0];
    let curve1 = &curves[1];
    let curve2 = &curves[2];

    for dst in dst.chunks_exact_mut(3) {
        let a0 = dst[0];
        let a1 = dst[1];
        let a2 = dst[2];
        let b0 = lut_interp_linear_float(a0, curve0);
        let b1 = lut_interp_linear_float(a1, curve1);
        let b2 = lut_interp_linear_float(a2, curve2);
        dst[0] = b0;
        dst[1] = b1;
        dst[2] = b2;
    }
}

pub(crate) fn execute_matrix_stage3(matrix: Matrix3f, bias: Vector3f, dst: &mut [f32]) {
    let m = matrix;
    let b = bias;

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

struct Multidimensional4x3<
    T: Copy + Default + AsPrimitive<f32> + PointeeSizeExpressible + Send + Sync,
> {
    a_curves: Option<Box<[Vec<f32>; 4]>>,
    m_curves: Option<Box<[Vec<f32>; 3]>>,
    b_curves: Option<Box<[Vec<f32>; 3]>>,
    clut: Option<Vec<f32>>,
    matrix: Matrix3f,
    bias: Vector3f,
    direction: MultidimensionalDirection,
    options: TransformOptions,
    pcs: DataColorSpace,
    grid_size: [u8; 4],
    _phantom: PhantomData<T>,
    bit_depth: usize,
}

impl<T: Copy + Default + AsPrimitive<f32> + PointeeSizeExpressible + Send + Sync>
    Multidimensional4x3<T>
{
    fn to_pcs_impl<Fetch: Fn(f32, f32, f32, f32) -> Vector3f>(
        &self,
        input: &[T],
        dst: &mut [f32],
        fetch: Fetch,
    ) -> Result<(), CmsError> {
        let norm_value = if T::FINITE {
            1.0 / ((1u32 << self.bit_depth) - 1) as f32
        } else {
            1.0
        };
        assert_eq!(
            self.direction,
            MultidimensionalDirection::DeviceToPcs,
            "PCS to device cannot be used on `to pcs` stage"
        );

        // A -> B
        // OR B - A A - curves stage

        if let (Some(a_curves), Some(clut)) = (self.a_curves.as_ref(), self.clut.as_ref()) {
            if !clut.is_empty() {
                let curve0 = &a_curves[0];
                let curve1 = &a_curves[1];
                let curve2 = &a_curves[2];
                let curve3 = &a_curves[3];
                for (src, dst) in input.chunks_exact(4).zip(dst.chunks_exact_mut(3)) {
                    let b0 = lut_interp_linear_float(src[0].as_() * norm_value, curve0);
                    let b1 = lut_interp_linear_float(src[1].as_() * norm_value, curve1);
                    let b2 = lut_interp_linear_float(src[2].as_() * norm_value, curve2);
                    let b3 = lut_interp_linear_float(src[3].as_() * norm_value, curve3);
                    let interpolated = fetch(b0, b1, b2, b3);
                    dst[0] = interpolated.v[0];
                    dst[1] = interpolated.v[1];
                    dst[2] = interpolated.v[2];
                }
            }
        } else {
            return Err(CmsError::InvalidAtoBLut);
        }

        // Matrix stage

        if let Some(m_curves) = self.m_curves.as_ref() {
            execute_simple_curves3(dst, m_curves);
            execute_matrix_stage3(self.matrix, self.bias, dst);
        }

        // B-curves is mandatory
        if let Some(b_curves) = &self.b_curves.as_ref() {
            execute_simple_curves3(dst, b_curves);
        }

        Ok(())
    }
}

impl<T: Copy + Default + AsPrimitive<f32> + PointeeSizeExpressible + Send + Sync>
    KatanaInitialStage<f32, T> for Multidimensional4x3<T>
{
    fn to_pcs(&self, input: &[T]) -> Result<Vec<f32>, CmsError> {
        if input.len() % 4 != 0 {
            return Err(CmsError::LaneMultipleOfChannels);
        }
        let fixed_new_clut = Vec::new();
        let new_clut = self.clut.as_ref().unwrap_or(&fixed_new_clut);
        let lut = Hypercube::new_hypercube(new_clut, self.grid_size);

        let mut new_dst = vec![0f32; (input.len() / 4) * 3];

        // If PCS is LAB then linear interpolation should be used
        if self.pcs == DataColorSpace::Lab || self.pcs == DataColorSpace::Xyz {
            self.to_pcs_impl(input, &mut new_dst, |x, y, z, w| {
                lut.quadlinear_vec3(x, y, z, w)
            })?;
            return Ok(new_dst);
        }

        match self.options.interpolation_method {
            #[cfg(feature = "options")]
            InterpolationMethod::Tetrahedral => {
                self.to_pcs_impl(input, &mut new_dst, |x, y, z, w| lut.tetra_vec3(x, y, z, w))?;
            }
            #[cfg(feature = "options")]
            InterpolationMethod::Pyramid => {
                self.to_pcs_impl(input, &mut new_dst, |x, y, z, w| {
                    lut.pyramid_vec3(x, y, z, w)
                })?;
            }
            #[cfg(feature = "options")]
            InterpolationMethod::Prism => {
                self.to_pcs_impl(input, &mut new_dst, |x, y, z, w| lut.prism_vec3(x, y, z, w))?;
            }
            InterpolationMethod::Linear => {
                self.to_pcs_impl(input, &mut new_dst, |x, y, z, w| {
                    lut.quadlinear_vec3(x, y, z, w)
                })?;
            }
        }
        Ok(new_dst)
    }
}

fn make_multidimensional_4x3<
    T: Copy + Default + AsPrimitive<f32> + PointeeSizeExpressible + Send + Sync,
>(
    mab: &LutMultidimensionalType,
    options: TransformOptions,
    pcs: DataColorSpace,
    direction: MultidimensionalDirection,
    bit_depth: usize,
) -> Result<Multidimensional4x3<T>, CmsError> {
    if mab.num_input_channels != 4 && mab.num_output_channels != 3 {
        return Err(CmsError::UnsupportedProfileConnection);
    }
    if mab.b_curves.is_empty() || mab.b_curves.len() != 3 {
        return Err(CmsError::InvalidAtoBLut);
    }

    let grid_size = [
        mab.grid_points[0],
        mab.grid_points[1],
        mab.grid_points[2],
        mab.grid_points[3],
    ];

    let clut: Option<Vec<f32>> = if mab.a_curves.len() == 4 && mab.clut.is_some() {
        let clut = mab.clut.as_ref().map(|x| x.to_clut_f32()).unwrap();
        let lut_grid = (mab.grid_points[0] as usize)
            .safe_mul(mab.grid_points[1] as usize)?
            .safe_mul(mab.grid_points[2] as usize)?
            .safe_mul(mab.grid_points[3] as usize)?
            .safe_mul(mab.num_output_channels as usize)?;
        if clut.len() != lut_grid {
            return Err(CmsError::MalformedCurveLutTable(MalformedSize {
                size: clut.len(),
                expected: lut_grid,
            }));
        }
        Some(clut)
    } else {
        return Err(CmsError::InvalidAtoBLut);
    };

    let a_curves: Option<Box<[Vec<f32>; 4]>> = if mab.a_curves.len() == 4 && mab.clut.is_some() {
        let mut arr = Box::<[Vec<f32>; 4]>::default();
        for (a_curve, dst) in mab.a_curves.iter().zip(arr.iter_mut()) {
            *dst = a_curve.to_clut()?;
        }
        Some(arr)
    } else {
        None
    };

    let b_curves: Option<Box<[Vec<f32>; 3]>> = if mab.b_curves.len() == 3 {
        let mut arr = Box::<[Vec<f32>; 3]>::default();
        let all_curves_linear = mab.b_curves.iter().all(|curve| curve.is_linear());
        if all_curves_linear {
            None
        } else {
            for (c_curve, dst) in mab.b_curves.iter().zip(arr.iter_mut()) {
                *dst = c_curve.to_clut()?;
            }
            Some(arr)
        }
    } else {
        return Err(CmsError::InvalidAtoBLut);
    };

    let matrix = mab.matrix.to_f32();

    let m_curves: Option<Box<[Vec<f32>; 3]>> = if mab.m_curves.len() == 3 {
        let all_curves_linear = mab.m_curves.iter().all(|curve| curve.is_linear());
        if !all_curves_linear
            || !mab.matrix.test_equality(Matrix3d::IDENTITY)
            || mab.bias.ne(&Vector3d::default())
        {
            let mut arr = Box::<[Vec<f32>; 3]>::default();
            for (curve, dst) in mab.m_curves.iter().zip(arr.iter_mut()) {
                *dst = curve.to_clut()?;
            }
            Some(arr)
        } else {
            None
        }
    } else {
        None
    };

    let bias = mab.bias.cast();

    let transform = Multidimensional4x3::<T> {
        a_curves,
        b_curves,
        m_curves,
        matrix,
        direction,
        options,
        clut,
        pcs,
        grid_size,
        bias,
        _phantom: PhantomData,
        bit_depth,
    };

    Ok(transform)
}

pub(crate) fn multi_dimensional_4x3_to_pcs<
    T: Copy + Default + AsPrimitive<f32> + PointeeSizeExpressible + Send + Sync,
>(
    mab: &LutMultidimensionalType,
    options: TransformOptions,
    pcs: DataColorSpace,
    bit_depth: usize,
) -> Result<Box<dyn KatanaInitialStage<f32, T> + Send + Sync>, CmsError> {
    let transform = make_multidimensional_4x3::<T>(
        mab,
        options,
        pcs,
        MultidimensionalDirection::DeviceToPcs,
        bit_depth,
    )?;
    Ok(Box::new(transform))
}
