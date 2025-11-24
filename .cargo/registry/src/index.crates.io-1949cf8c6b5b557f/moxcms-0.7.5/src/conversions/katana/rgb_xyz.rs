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
use crate::conversions::katana::pcs_stages::KatanaMatrixStage;
use crate::conversions::katana::{KatanaInitialStage, KatanaIntermediateStage};
use crate::{CmsError, ColorProfile, Layout, Matrix3f, PointeeSizeExpressible, TransformOptions};
use num_traits::AsPrimitive;
use std::marker::PhantomData;

struct KatanaRgbLinearizationStage<T: Clone, const LAYOUT: u8, const LINEAR_CAP: usize> {
    r_lin: Box<[f32; LINEAR_CAP]>,
    g_lin: Box<[f32; LINEAR_CAP]>,
    b_lin: Box<[f32; LINEAR_CAP]>,
    linear_cap: usize,
    bit_depth: usize,
    _phantom: PhantomData<T>,
}

impl<
    T: Clone + AsPrimitive<f32> + PointeeSizeExpressible,
    const LAYOUT: u8,
    const LINEAR_CAP: usize,
> KatanaInitialStage<f32, T> for KatanaRgbLinearizationStage<T, LAYOUT, LINEAR_CAP>
{
    fn to_pcs(&self, input: &[T]) -> Result<Vec<f32>, CmsError> {
        let src_layout = Layout::from(LAYOUT);
        if input.len() % src_layout.channels() != 0 {
            return Err(CmsError::LaneMultipleOfChannels);
        }
        let mut dst = vec![0.; input.len() / src_layout.channels() * 3];

        let scale = if T::FINITE {
            (self.linear_cap as f32 - 1.) / ((1 << self.bit_depth) - 1) as f32
        } else {
            (T::NOT_FINITE_LINEAR_TABLE_SIZE - 1) as f32
        };

        let cap_value = if T::FINITE {
            ((1 << self.bit_depth) - 1) as f32
        } else {
            (T::NOT_FINITE_LINEAR_TABLE_SIZE - 1) as f32
        };

        for (src, dst) in input
            .chunks_exact(src_layout.channels())
            .zip(dst.chunks_exact_mut(3))
        {
            let j_r = src[0].as_() * scale;
            let j_g = src[1].as_() * scale;
            let j_b = src[2].as_() * scale;
            dst[0] = self.r_lin[(j_r.round().min(cap_value).max(0.) as u16) as usize];
            dst[1] = self.g_lin[(j_g.round().min(cap_value).max(0.) as u16) as usize];
            dst[2] = self.b_lin[(j_b.round().min(cap_value).max(0.) as u16) as usize];
        }
        Ok(dst)
    }
}

pub(crate) struct KatanaRgbLinearizationState<T> {
    pub(crate) stages: Vec<Box<dyn KatanaIntermediateStage<f32> + Send + Sync>>,
    pub(crate) initial_stage: Box<dyn KatanaInitialStage<f32, T> + Send + Sync>,
}

pub(crate) fn katana_create_rgb_lin_lut<
    T: Copy + Default + AsPrimitive<f32> + Send + Sync + AsPrimitive<usize> + PointeeSizeExpressible,
    const BIT_DEPTH: usize,
    const LINEAR_CAP: usize,
>(
    layout: Layout,
    source: &ColorProfile,
    opts: TransformOptions,
) -> Result<KatanaRgbLinearizationState<T>, CmsError>
where
    u32: AsPrimitive<T>,
    f32: AsPrimitive<T>,
{
    let lin_r =
        source.build_r_linearize_table::<T, LINEAR_CAP, BIT_DEPTH>(opts.allow_use_cicp_transfer)?;
    let lin_g =
        source.build_g_linearize_table::<T, LINEAR_CAP, BIT_DEPTH>(opts.allow_use_cicp_transfer)?;
    let lin_b =
        source.build_b_linearize_table::<T, LINEAR_CAP, BIT_DEPTH>(opts.allow_use_cicp_transfer)?;

    let lin_stage: Box<dyn KatanaInitialStage<f32, T> + Send + Sync> = match layout {
        Layout::Rgb => {
            Box::new(
                KatanaRgbLinearizationStage::<T, { Layout::Rgb as u8 }, LINEAR_CAP> {
                    r_lin: lin_r,
                    g_lin: lin_g,
                    b_lin: lin_b,
                    bit_depth: BIT_DEPTH,
                    linear_cap: LINEAR_CAP,
                    _phantom: PhantomData,
                },
            )
        }
        Layout::Rgba => {
            Box::new(
                KatanaRgbLinearizationStage::<T, { Layout::Rgba as u8 }, LINEAR_CAP> {
                    r_lin: lin_r,
                    g_lin: lin_g,
                    b_lin: lin_b,
                    bit_depth: BIT_DEPTH,
                    linear_cap: LINEAR_CAP,
                    _phantom: PhantomData,
                },
            )
        }
        Layout::Gray => unimplemented!("Gray should not be called on Rgb/Rgba execution path"),
        Layout::GrayAlpha => {
            unimplemented!("GrayAlpha should not be called on Rgb/Rgba execution path")
        }
        _ => unreachable!(),
    };

    let xyz_to_rgb = source.rgb_to_xyz_matrix();

    let matrices: Vec<Box<dyn KatanaIntermediateStage<f32> + Send + Sync>> =
        vec![Box::new(KatanaMatrixStage {
            matrices: vec![
                xyz_to_rgb.to_f32(),
                Matrix3f {
                    v: [
                        [32768.0 / 65535.0, 0.0, 0.0],
                        [0.0, 32768.0 / 65535.0, 0.0],
                        [0.0, 0.0, 32768.0 / 65535.0],
                    ],
                },
            ],
        })];

    Ok(KatanaRgbLinearizationState {
        stages: matrices,
        initial_stage: lin_stage,
    })
}
