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
use crate::conversions::katana::KatanaFinalStage;
use crate::conversions::katana::md3x3::MultidimensionalDirection;
use crate::conversions::katana::md4x3::{execute_matrix_stage3, execute_simple_curves3};
use crate::conversions::md_lut::{MultidimensionalLut, tetra_3i_to_any_vec};
use crate::safe_math::SafeMul;
use crate::trc::lut_interp_linear_float;
use crate::{
    CmsError, DataColorSpace, Layout, LutMultidimensionalType, MalformedSize, Matrix3d, Matrix3f,
    PointeeSizeExpressible, TransformOptions, Vector3d, Vector3f,
};
use num_traits::AsPrimitive;
use std::marker::PhantomData;

struct Multidimensional3xN<
    T: Copy + Default + AsPrimitive<f32> + PointeeSizeExpressible + Send + Sync,
> {
    a_curves: Option<Vec<Vec<f32>>>,
    m_curves: Option<Box<[Vec<f32>; 3]>>,
    b_curves: Option<Box<[Vec<f32>; 3]>>,
    clut: Option<Vec<f32>>,
    matrix: Matrix3f,
    bias: Vector3f,
    direction: MultidimensionalDirection,
    grid_size: [u8; 16],
    output_inks: usize,
    _phantom: PhantomData<T>,
    dst_layout: Layout,
    bit_depth: usize,
}

impl<T: Copy + Default + AsPrimitive<f32> + PointeeSizeExpressible + Send + Sync>
    Multidimensional3xN<T>
where
    f32: AsPrimitive<T>,
{
    fn to_output_impl(&self, src: &mut [f32], dst: &mut [T]) -> Result<(), CmsError> {
        let norm_value = if T::FINITE {
            ((1u32 << self.bit_depth) - 1) as f32
        } else {
            1.0
        };
        assert_eq!(
            self.direction,
            MultidimensionalDirection::PcsToDevice,
            "PCS to device cannot be used on `to pcs` stage"
        );

        // B-curves is mandatory
        if let Some(b_curves) = &self.b_curves.as_ref() {
            execute_simple_curves3(src, b_curves);
        }

        // Matrix stage

        if let Some(m_curves) = self.m_curves.as_ref() {
            execute_matrix_stage3(self.matrix, self.bias, src);
            execute_simple_curves3(src, m_curves);
        }

        if let (Some(a_curves), Some(clut)) = (self.a_curves.as_ref(), self.clut.as_ref()) {
            let mut inks = vec![0.; self.output_inks];

            if clut.is_empty() {
                return Err(CmsError::InvalidAtoBLut);
            }

            let md_lut = MultidimensionalLut::new(self.grid_size, 3, self.output_inks);

            for (src, dst) in src
                .chunks_exact(3)
                .zip(dst.chunks_exact_mut(self.dst_layout.channels()))
            {
                tetra_3i_to_any_vec(
                    &md_lut,
                    clut,
                    src[0],
                    src[1],
                    src[2],
                    &mut inks,
                    self.output_inks,
                );

                for (ink, curve) in inks.iter_mut().zip(a_curves.iter()) {
                    *ink = lut_interp_linear_float(*ink, curve);
                }

                if T::FINITE {
                    for (dst, ink) in dst.iter_mut().zip(inks.iter()) {
                        *dst = (*ink * norm_value).round().max(0.).min(norm_value).as_();
                    }
                } else {
                    for (dst, ink) in dst.iter_mut().zip(inks.iter()) {
                        *dst = (*ink * norm_value).as_();
                    }
                }
            }
        } else {
            return Err(CmsError::InvalidAtoBLut);
        }

        Ok(())
    }
}

impl<T: Copy + Default + AsPrimitive<f32> + PointeeSizeExpressible + Send + Sync>
    KatanaFinalStage<f32, T> for Multidimensional3xN<T>
where
    f32: AsPrimitive<T>,
{
    fn to_output(&self, src: &mut [f32], dst: &mut [T]) -> Result<(), CmsError> {
        if src.len() % 3 != 0 {
            return Err(CmsError::LaneMultipleOfChannels);
        }
        if dst.len() % self.output_inks != 0 {
            return Err(CmsError::LaneMultipleOfChannels);
        }

        self.to_output_impl(src, dst)?;
        Ok(())
    }
}

fn make_multidimensional_nx3<
    T: Copy + Default + AsPrimitive<f32> + PointeeSizeExpressible + Send + Sync,
>(
    dst_layout: Layout,
    mab: &LutMultidimensionalType,
    _: TransformOptions,
    pcs: DataColorSpace,
    direction: MultidimensionalDirection,
    bit_depth: usize,
) -> Result<Multidimensional3xN<T>, CmsError> {
    let real_inks = if pcs == DataColorSpace::Rgb {
        3
    } else {
        dst_layout.channels()
    };

    if mab.num_output_channels != real_inks as u8 {
        return Err(CmsError::UnsupportedProfileConnection);
    }

    if mab.b_curves.is_empty() || mab.b_curves.len() != 3 {
        return Err(CmsError::InvalidAtoBLut);
    }

    let clut: Option<Vec<f32>> =
        if mab.a_curves.len() == mab.num_output_channels as usize && mab.clut.is_some() {
            let clut = mab.clut.as_ref().map(|x| x.to_clut_f32()).unwrap();
            let mut lut_grid = 1usize;
            for grid in mab.grid_points.iter().take(mab.num_input_channels as usize) {
                lut_grid = lut_grid.safe_mul(*grid as usize)?;
            }
            let lut_grid = lut_grid.safe_mul(mab.num_output_channels as usize)?;
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

    let a_curves: Option<Vec<Vec<f32>>> =
        if mab.a_curves.len() == mab.num_output_channels as usize && mab.clut.is_some() {
            let mut arr = Vec::new();
            for a_curve in mab.a_curves.iter() {
                arr.push(a_curve.to_clut()?);
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

    let transform = Multidimensional3xN::<T> {
        a_curves,
        b_curves,
        m_curves,
        matrix,
        direction,
        clut,
        grid_size: mab.grid_points,
        bias,
        dst_layout,
        output_inks: real_inks,
        _phantom: PhantomData,
        bit_depth,
    };

    Ok(transform)
}

pub(crate) fn katana_multi_dimensional_3xn_to_device<
    T: Copy + Default + AsPrimitive<f32> + PointeeSizeExpressible + Send + Sync,
>(
    dst_layout: Layout,
    mab: &LutMultidimensionalType,
    options: TransformOptions,
    pcs: DataColorSpace,
    bit_depth: usize,
) -> Result<Box<dyn KatanaFinalStage<f32, T> + Send + Sync>, CmsError>
where
    f32: AsPrimitive<T>,
{
    if mab.num_input_channels == 0 {
        return Err(CmsError::UnsupportedProfileConnection);
    }
    let transform = make_multidimensional_nx3::<T>(
        dst_layout,
        mab,
        options,
        pcs,
        MultidimensionalDirection::PcsToDevice,
        bit_depth,
    )?;
    Ok(Box::new(transform))
}
