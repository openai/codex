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
use crate::conversions::katana::KatanaInitialStage;
use crate::profile::LutDataType;
use crate::safe_math::{SafeMul, SafePowi};
use crate::trc::lut_interp_linear_float;
use crate::{
    CmsError, DataColorSpace, Hypercube, InterpolationMethod, MalformedSize,
    PointeeSizeExpressible, Stage, TransformOptions, Vector3f,
};
use num_traits::AsPrimitive;
use std::marker::PhantomData;

#[allow(unused)]
#[derive(Default)]
struct Lut4x3 {
    linearization: [Vec<f32>; 4],
    clut: Vec<f32>,
    grid_size: u8,
    output: [Vec<f32>; 3],
    interpolation_method: InterpolationMethod,
    pcs: DataColorSpace,
}

#[allow(unused)]
#[derive(Default)]
struct KatanaLut4x3<T: Copy + PointeeSizeExpressible + AsPrimitive<f32>> {
    linearization: [Vec<f32>; 4],
    clut: Vec<f32>,
    grid_size: u8,
    output: [Vec<f32>; 3],
    interpolation_method: InterpolationMethod,
    pcs: DataColorSpace,
    _phantom: PhantomData<T>,
    bit_depth: usize,
}

#[allow(unused)]
impl Lut4x3 {
    fn transform_impl<Fetch: Fn(f32, f32, f32, f32) -> Vector3f>(
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

            let clut = fetch(linear_x, linear_y, linear_z, linear_w);

            let pcs_x = lut_interp_linear_float(clut.v[0], &self.output[0]);
            let pcs_y = lut_interp_linear_float(clut.v[1], &self.output[1]);
            let pcs_z = lut_interp_linear_float(clut.v[2], &self.output[2]);
            dest[0] = pcs_x;
            dest[1] = pcs_y;
            dest[2] = pcs_z;
        }
        Ok(())
    }
}

macro_rules! define_lut4_dispatch {
    ($dispatcher: ident) => {
        impl Stage for $dispatcher {
            fn transform(&self, src: &[f32], dst: &mut [f32]) -> Result<(), CmsError> {
                let l_tbl = Hypercube::new(&self.clut, self.grid_size as usize);

                // If Source PCS is LAB trilinear should be used
                if self.pcs == DataColorSpace::Lab || self.pcs == DataColorSpace::Xyz {
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

impl<T: Copy + PointeeSizeExpressible + AsPrimitive<f32>> KatanaLut4x3<T> {
    fn to_pcs_impl<Fetch: Fn(f32, f32, f32, f32) -> Vector3f>(
        &self,
        input: &[T],
        fetch: Fetch,
    ) -> Result<Vec<f32>, CmsError> {
        if input.len() % 4 != 0 {
            return Err(CmsError::LaneMultipleOfChannels);
        }
        let norm_value = if T::FINITE {
            1.0 / ((1u32 << self.bit_depth) - 1) as f32
        } else {
            1.0
        };
        let mut dst = vec![0.; (input.len() / 4) * 3];
        let linearization_0 = &self.linearization[0];
        let linearization_1 = &self.linearization[1];
        let linearization_2 = &self.linearization[2];
        let linearization_3 = &self.linearization[3];
        for (dest, src) in dst.chunks_exact_mut(3).zip(input.chunks_exact(4)) {
            let linear_x = lut_interp_linear_float(src[0].as_() * norm_value, linearization_0);
            let linear_y = lut_interp_linear_float(src[1].as_() * norm_value, linearization_1);
            let linear_z = lut_interp_linear_float(src[2].as_() * norm_value, linearization_2);
            let linear_w = lut_interp_linear_float(src[3].as_() * norm_value, linearization_3);

            let clut = fetch(linear_x, linear_y, linear_z, linear_w);

            let pcs_x = lut_interp_linear_float(clut.v[0], &self.output[0]);
            let pcs_y = lut_interp_linear_float(clut.v[1], &self.output[1]);
            let pcs_z = lut_interp_linear_float(clut.v[2], &self.output[2]);
            dest[0] = pcs_x;
            dest[1] = pcs_y;
            dest[2] = pcs_z;
        }
        Ok(dst)
    }
}

impl<T: Copy + PointeeSizeExpressible + AsPrimitive<f32>> KatanaInitialStage<f32, T>
    for KatanaLut4x3<T>
{
    fn to_pcs(&self, input: &[T]) -> Result<Vec<f32>, CmsError> {
        if input.len() % 4 != 0 {
            return Err(CmsError::LaneMultipleOfChannels);
        }
        let l_tbl = Hypercube::new(&self.clut, self.grid_size as usize);

        // If Source PCS is LAB trilinear should be used
        if self.pcs == DataColorSpace::Lab || self.pcs == DataColorSpace::Xyz {
            return self.to_pcs_impl(input, |x, y, z, w| l_tbl.quadlinear_vec3(x, y, z, w));
        }

        match self.interpolation_method {
            #[cfg(feature = "options")]
            InterpolationMethod::Tetrahedral => {
                self.to_pcs_impl(input, |x, y, z, w| l_tbl.tetra_vec3(x, y, z, w))
            }
            #[cfg(feature = "options")]
            InterpolationMethod::Pyramid => {
                self.to_pcs_impl(input, |x, y, z, w| l_tbl.pyramid_vec3(x, y, z, w))
            }
            #[cfg(feature = "options")]
            InterpolationMethod::Prism => {
                self.to_pcs_impl(input, |x, y, z, w| l_tbl.prism_vec3(x, y, z, w))
            }
            InterpolationMethod::Linear => {
                self.to_pcs_impl(input, |x, y, z, w| l_tbl.quadlinear_vec3(x, y, z, w))
            }
        }
    }
}

define_lut4_dispatch!(Lut4x3);

fn make_lut_4x3(
    lut: &LutDataType,
    options: TransformOptions,
    pcs: DataColorSpace,
) -> Result<Lut4x3, CmsError> {
    // There is 4 possible cases:
    // - All curves are non-linear
    // - Linearization curves are non-linear, but gamma is linear
    // - Gamma curves are non-linear, but linearization is linear
    // - All curves linear
    let clut_length: usize = (lut.num_clut_grid_points as usize)
        .safe_powi(lut.num_input_channels as u32)?
        .safe_mul(lut.num_output_channels as usize)?;

    let clut_table = lut.clut_table.to_clut_f32();
    if clut_table.len() != clut_length {
        return Err(CmsError::MalformedClut(MalformedSize {
            size: clut_table.len(),
            expected: clut_length,
        }));
    }

    let linearization_table = lut.input_table.to_clut_f32();

    if linearization_table.len() < lut.num_input_table_entries as usize * 4 {
        return Err(CmsError::MalformedCurveLutTable(MalformedSize {
            size: linearization_table.len(),
            expected: lut.num_input_table_entries as usize * 4,
        }));
    }

    let lin_curve0 = linearization_table[0..lut.num_input_table_entries as usize].to_vec();
    let lin_curve1 = linearization_table
        [lut.num_input_table_entries as usize..lut.num_input_table_entries as usize * 2]
        .to_vec();
    let lin_curve2 = linearization_table
        [lut.num_input_table_entries as usize * 2..lut.num_input_table_entries as usize * 3]
        .to_vec();
    let lin_curve3 = linearization_table
        [lut.num_input_table_entries as usize * 3..lut.num_input_table_entries as usize * 4]
        .to_vec();

    let gamma_table = lut.output_table.to_clut_f32();

    if gamma_table.len() < lut.num_output_table_entries as usize * 3 {
        return Err(CmsError::MalformedCurveLutTable(MalformedSize {
            size: gamma_table.len(),
            expected: lut.num_output_table_entries as usize * 3,
        }));
    }

    let gamma_curve0 = gamma_table[..lut.num_output_table_entries as usize].to_vec();
    let gamma_curve1 = gamma_table
        [lut.num_output_table_entries as usize..lut.num_output_table_entries as usize * 2]
        .to_vec();
    let gamma_curve2 = gamma_table
        [lut.num_output_table_entries as usize * 2..lut.num_output_table_entries as usize * 3]
        .to_vec();

    let transform = Lut4x3 {
        linearization: [lin_curve0, lin_curve1, lin_curve2, lin_curve3],
        interpolation_method: options.interpolation_method,
        pcs,
        clut: clut_table,
        grid_size: lut.num_clut_grid_points,
        output: [gamma_curve0, gamma_curve1, gamma_curve2],
    };
    Ok(transform)
}

fn stage_lut_4x3(
    lut: &LutDataType,
    options: TransformOptions,
    pcs: DataColorSpace,
) -> Result<Box<dyn Stage>, CmsError> {
    let lut = make_lut_4x3(lut, options, pcs)?;
    #[cfg(all(target_arch = "aarch64", target_feature = "neon", feature = "neon"))]
    {
        use crate::conversions::neon::Lut4x3Neon;
        let transform = Lut4x3Neon {
            linearization: lut.linearization,
            interpolation_method: lut.interpolation_method,
            pcs: lut.pcs,
            clut: lut.clut,
            grid_size: lut.grid_size,
            output: lut.output,
        };
        Ok(Box::new(transform))
    }
    #[cfg(not(all(target_arch = "aarch64", target_feature = "neon", feature = "neon")))]
    {
        #[cfg(all(target_arch = "x86_64", feature = "avx"))]
        {
            use crate::conversions::avx::Lut4x3AvxFma;
            if std::arch::is_x86_feature_detected!("avx2")
                && std::arch::is_x86_feature_detected!("fma")
            {
                let transform = Lut4x3AvxFma {
                    linearization: lut.linearization,
                    interpolation_method: lut.interpolation_method,
                    pcs: lut.pcs,
                    clut: lut.clut,
                    grid_size: lut.grid_size,
                    output: lut.output,
                };
                return Ok(Box::new(transform));
            }
        }
        let transform = Lut4x3 {
            linearization: lut.linearization,
            interpolation_method: lut.interpolation_method,
            pcs: lut.pcs,
            clut: lut.clut,
            grid_size: lut.grid_size,
            output: lut.output,
        };
        Ok(Box::new(transform))
    }
}

pub(crate) fn katana_input_stage_lut_4x3<
    T: Copy + PointeeSizeExpressible + AsPrimitive<f32> + Send + Sync,
>(
    lut: &LutDataType,
    options: TransformOptions,
    pcs: DataColorSpace,
    bit_depth: usize,
) -> Result<Box<dyn KatanaInitialStage<f32, T> + Send + Sync>, CmsError> {
    // There is 4 possible cases:
    // - All curves are non-linear
    // - Linearization curves are non-linear, but gamma is linear
    // - Gamma curves are non-linear, but linearization is linear
    // - All curves linear
    let lut = make_lut_4x3(lut, options, pcs)?;

    let transform = KatanaLut4x3::<T> {
        linearization: lut.linearization,
        interpolation_method: lut.interpolation_method,
        pcs: lut.pcs,
        clut: lut.clut,
        grid_size: lut.grid_size,
        output: lut.output,
        _phantom: PhantomData,
        bit_depth,
    };
    Ok(Box::new(transform))
}

pub(crate) fn create_lut4_norm_samples<const SAMPLES: usize>() -> Vec<f32> {
    let lut_size: u32 = (4 * SAMPLES * SAMPLES * SAMPLES * SAMPLES) as u32;

    let mut src = Vec::with_capacity(lut_size as usize);

    let recpeq = 1f32 / (SAMPLES - 1) as f32;
    for k in 0..SAMPLES {
        for c in 0..SAMPLES {
            for m in 0..SAMPLES {
                for y in 0..SAMPLES {
                    src.push(c as f32 * recpeq);
                    src.push(m as f32 * recpeq);
                    src.push(y as f32 * recpeq);
                    src.push(k as f32 * recpeq);
                }
            }
        }
    }
    src
}

pub(crate) fn create_lut4<const SAMPLES: usize>(
    lut: &LutDataType,
    options: TransformOptions,
    pcs: DataColorSpace,
) -> Result<Vec<f32>, CmsError> {
    if lut.num_input_channels != 4 {
        return Err(CmsError::UnsupportedProfileConnection);
    }
    let lut_size: u32 = (4 * SAMPLES * SAMPLES * SAMPLES * SAMPLES) as u32;

    let src = create_lut4_norm_samples::<SAMPLES>();
    let mut dest = vec![0.; (lut_size as usize) / 4 * 3];

    let lut_stage = stage_lut_4x3(lut, options, pcs)?;
    lut_stage.transform(&src, &mut dest)?;
    Ok(dest)
}
