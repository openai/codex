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
use crate::conversions::katana::md_nx3::interpolate_out_function;
use crate::conversions::katana::{KatanaFinalStage, KatanaInitialStage};
use crate::conversions::md_lut::{MultidimensionalLut, tetra_3i_to_any_vec};
use crate::profile::LutDataType;
use crate::safe_math::{SafeMul, SafePowi};
use crate::trc::lut_interp_linear_float;
use crate::{
    CmsError, DataColorSpace, Layout, MalformedSize, PointeeSizeExpressible, TransformOptions,
};
use num_traits::AsPrimitive;
use std::array::from_fn;
use std::marker::PhantomData;

#[derive(Default)]
struct KatanaLutNx3<T> {
    linearization: Vec<Vec<f32>>,
    clut: Vec<f32>,
    grid_size: u8,
    input_inks: usize,
    output: [Vec<f32>; 3],
    _phantom: PhantomData<T>,
    bit_depth: usize,
}

struct KatanaLut3xN<T> {
    linearization: [Vec<f32>; 3],
    clut: Vec<f32>,
    grid_size: u8,
    output_inks: usize,
    output: Vec<Vec<f32>>,
    dst_layout: Layout,
    target_color_space: DataColorSpace,
    _phantom: PhantomData<T>,
    bit_depth: usize,
}

impl<T: Copy + PointeeSizeExpressible + AsPrimitive<f32>> KatanaLutNx3<T> {
    fn to_pcs_impl(&self, input: &[T]) -> Result<Vec<f32>, CmsError> {
        if input.len() % self.input_inks != 0 {
            return Err(CmsError::LaneMultipleOfChannels);
        }
        let norm_value = if T::FINITE {
            1.0 / ((1u32 << self.bit_depth) - 1) as f32
        } else {
            1.0
        };

        let grid_sizes: [u8; 16] = from_fn(|i| {
            if i < self.input_inks {
                self.grid_size
            } else {
                0
            }
        });

        let md_lut = MultidimensionalLut::new(grid_sizes, self.input_inks, 3);

        let layout = Layout::from_inks(self.input_inks);

        let mut inks = vec![0.; self.input_inks];

        let mut dst = vec![0.; (input.len() / layout.channels()) * 3];

        let fetcher = interpolate_out_function(layout);

        for (dest, src) in dst
            .chunks_exact_mut(3)
            .zip(input.chunks_exact(layout.channels()))
        {
            for ((ink, src_ink), curve) in inks.iter_mut().zip(src).zip(self.linearization.iter()) {
                *ink = lut_interp_linear_float(src_ink.as_() * norm_value, curve);
            }

            let clut = fetcher(&md_lut, &self.clut, &inks);

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
    for KatanaLutNx3<T>
{
    fn to_pcs(&self, input: &[T]) -> Result<Vec<f32>, CmsError> {
        if input.len() % self.input_inks != 0 {
            return Err(CmsError::LaneMultipleOfChannels);
        }

        self.to_pcs_impl(input)
    }
}

impl<T: Copy + PointeeSizeExpressible + AsPrimitive<f32>> KatanaFinalStage<f32, T>
    for KatanaLut3xN<T>
where
    f32: AsPrimitive<T>,
{
    fn to_output(&self, src: &mut [f32], dst: &mut [T]) -> Result<(), CmsError> {
        if src.len() % 3 != 0 {
            return Err(CmsError::LaneMultipleOfChannels);
        }

        let grid_sizes: [u8; 16] = from_fn(|i| {
            if i < self.output_inks {
                self.grid_size
            } else {
                0
            }
        });

        let md_lut = MultidimensionalLut::new(grid_sizes, 3, self.output_inks);

        let scale_value = if T::FINITE {
            ((1u32 << self.bit_depth) - 1) as f32
        } else {
            1.0
        };

        let mut working = vec![0.; self.output_inks];

        for (dest, src) in dst
            .chunks_exact_mut(self.dst_layout.channels())
            .zip(src.chunks_exact(3))
        {
            let x = lut_interp_linear_float(src[0], &self.linearization[0]);
            let y = lut_interp_linear_float(src[1], &self.linearization[1]);
            let z = lut_interp_linear_float(src[2], &self.linearization[2]);

            tetra_3i_to_any_vec(&md_lut, &self.clut, x, y, z, &mut working, self.output_inks);

            for (ink, curve) in working.iter_mut().zip(self.output.iter()) {
                *ink = lut_interp_linear_float(*ink, curve);
            }

            if T::FINITE {
                for (dst, ink) in dest.iter_mut().zip(working.iter()) {
                    *dst = (*ink * scale_value).round().max(0.).min(scale_value).as_();
                }
            } else {
                for (dst, ink) in dest.iter_mut().zip(working.iter()) {
                    *dst = (*ink * scale_value).as_();
                }
            }
        }

        if self.dst_layout == Layout::Rgba && self.target_color_space == DataColorSpace::Rgb {
            for dst in dst.chunks_exact_mut(self.dst_layout.channels()) {
                dst[3] = scale_value.as_();
            }
        }

        Ok(())
    }
}

fn katana_make_lut_nx3<T: Copy + PointeeSizeExpressible + AsPrimitive<f32>>(
    inks: usize,
    lut: &LutDataType,
    _: TransformOptions,
    _: DataColorSpace,
    bit_depth: usize,
) -> Result<KatanaLutNx3<T>, CmsError> {
    if inks != lut.num_input_channels as usize {
        return Err(CmsError::UnsupportedProfileConnection);
    }
    if lut.num_output_channels != 3 {
        return Err(CmsError::UnsupportedProfileConnection);
    }
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

    if linearization_table.len() < lut.num_input_table_entries as usize * inks {
        return Err(CmsError::MalformedCurveLutTable(MalformedSize {
            size: linearization_table.len(),
            expected: lut.num_input_table_entries as usize * inks,
        }));
    }

    let linearization = (0..inks)
        .map(|x| {
            linearization_table[x * lut.num_input_table_entries as usize
                ..(x + 1) * lut.num_input_table_entries as usize]
                .to_vec()
        })
        .collect::<_>();

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

    let transform = KatanaLutNx3::<T> {
        linearization,
        clut: clut_table,
        grid_size: lut.num_clut_grid_points,
        output: [gamma_curve0, gamma_curve1, gamma_curve2],
        input_inks: inks,
        _phantom: PhantomData,
        bit_depth,
    };
    Ok(transform)
}

fn katana_make_lut_3xn<T: Copy + PointeeSizeExpressible + AsPrimitive<f32>>(
    inks: usize,
    dst_layout: Layout,
    lut: &LutDataType,
    _: TransformOptions,
    target_color_space: DataColorSpace,
    bit_depth: usize,
) -> Result<KatanaLut3xN<T>, CmsError> {
    if lut.num_input_channels as usize != 3 {
        return Err(CmsError::UnsupportedProfileConnection);
    }
    if target_color_space == DataColorSpace::Rgb {
        if lut.num_output_channels != 3 || lut.num_output_channels != 4 {
            return Err(CmsError::InvalidInksCountForProfile);
        }
        if dst_layout != Layout::Rgb || dst_layout != Layout::Rgba {
            return Err(CmsError::InvalidInksCountForProfile);
        }
    } else if lut.num_output_channels as usize != dst_layout.channels() {
        return Err(CmsError::InvalidInksCountForProfile);
    }
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

    if linearization_table.len() < lut.num_input_table_entries as usize * 3 {
        return Err(CmsError::MalformedCurveLutTable(MalformedSize {
            size: linearization_table.len(),
            expected: lut.num_input_table_entries as usize * 3,
        }));
    }

    let linear_curve0 = linearization_table[..lut.num_input_table_entries as usize].to_vec();
    let linear_curve1 = linearization_table
        [lut.num_input_table_entries as usize..lut.num_input_table_entries as usize * 2]
        .to_vec();
    let linear_curve2 = linearization_table
        [lut.num_input_table_entries as usize * 2..lut.num_input_table_entries as usize * 3]
        .to_vec();

    let gamma_table = lut.output_table.to_clut_f32();

    if gamma_table.len() < lut.num_output_table_entries as usize * inks {
        return Err(CmsError::MalformedCurveLutTable(MalformedSize {
            size: gamma_table.len(),
            expected: lut.num_output_table_entries as usize * inks,
        }));
    }

    let gamma = (0..inks)
        .map(|x| {
            gamma_table[x * lut.num_output_table_entries as usize
                ..(x + 1) * lut.num_output_table_entries as usize]
                .to_vec()
        })
        .collect::<_>();

    let transform = KatanaLut3xN::<T> {
        linearization: [linear_curve0, linear_curve1, linear_curve2],
        clut: clut_table,
        grid_size: lut.num_clut_grid_points,
        output: gamma,
        output_inks: inks,
        _phantom: PhantomData,
        target_color_space,
        dst_layout,
        bit_depth,
    };
    Ok(transform)
}

pub(crate) fn katana_input_make_lut_nx3<
    T: Copy + PointeeSizeExpressible + AsPrimitive<f32> + Send + Sync,
>(
    src_layout: Layout,
    inks: usize,
    lut: &LutDataType,
    options: TransformOptions,
    pcs: DataColorSpace,
    bit_depth: usize,
) -> Result<Box<dyn KatanaInitialStage<f32, T> + Send + Sync>, CmsError> {
    if pcs == DataColorSpace::Rgb {
        if lut.num_input_channels != 3 {
            return Err(CmsError::InvalidAtoBLut);
        }
        if src_layout != Layout::Rgba && src_layout != Layout::Rgb {
            return Err(CmsError::InvalidInksCountForProfile);
        }
    } else if lut.num_input_channels != src_layout.channels() as u8 {
        return Err(CmsError::InvalidInksCountForProfile);
    }
    let z0 = katana_make_lut_nx3::<T>(inks, lut, options, pcs, bit_depth)?;
    Ok(Box::new(z0))
}

pub(crate) fn katana_output_make_lut_3xn<
    T: Copy + PointeeSizeExpressible + AsPrimitive<f32> + Send + Sync,
>(
    dst_layout: Layout,
    lut: &LutDataType,
    options: TransformOptions,
    target_color_space: DataColorSpace,
    bit_depth: usize,
) -> Result<Box<dyn KatanaFinalStage<f32, T> + Send + Sync>, CmsError>
where
    f32: AsPrimitive<T>,
{
    let real_inks = if target_color_space == DataColorSpace::Rgb {
        3
    } else {
        dst_layout.channels()
    };
    let z0 = katana_make_lut_3xn::<T>(
        real_inks,
        dst_layout,
        lut,
        options,
        target_color_space,
        bit_depth,
    )?;
    Ok(Box::new(z0))
}
