/*
 * // Copyright (c) Radzivon Bartoshyk 4/2025. All rights reserved.
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
use crate::conversions::lut3x4::create_lut3_samples;
use crate::mlaf::mlaf;
use crate::trc::ToneCurveEvaluator;
use crate::{
    CmsError, ColorProfile, GammaLutInterpolate, InPlaceStage, Matrix3f, PointeeSizeExpressible,
    RenderingIntent, Rgb, TransformOptions, filmlike_clip,
};
use num_traits::AsPrimitive;
use std::marker::PhantomData;

pub(crate) struct XyzToRgbStage<T: Clone> {
    pub(crate) r_gamma: Box<[T; 65536]>,
    pub(crate) g_gamma: Box<[T; 65536]>,
    pub(crate) b_gamma: Box<[T; 65536]>,
    pub(crate) matrices: Vec<Matrix3f>,
    pub(crate) intent: RenderingIntent,
    pub(crate) bit_depth: usize,
    pub(crate) gamma_lut: usize,
}

impl<T: Clone + AsPrimitive<f32>> InPlaceStage for XyzToRgbStage<T> {
    fn transform(&self, dst: &mut [f32]) -> Result<(), CmsError> {
        assert!(self.bit_depth > 0);
        if !self.matrices.is_empty() {
            let m = self.matrices[0];
            for dst in dst.chunks_exact_mut(3) {
                let x = dst[0];
                let y = dst[1];
                let z = dst[2];
                dst[0] = mlaf(mlaf(x * m.v[0][0], y, m.v[0][1]), z, m.v[0][2]);
                dst[1] = mlaf(mlaf(x * m.v[1][0], y, m.v[1][1]), z, m.v[1][2]);
                dst[2] = mlaf(mlaf(x * m.v[2][0], y, m.v[2][1]), z, m.v[2][2]);
            }
        }

        for m in self.matrices.iter().skip(1) {
            for dst in dst.chunks_exact_mut(3) {
                let x = dst[0];
                let y = dst[1];
                let z = dst[2];
                dst[0] = mlaf(mlaf(x * m.v[0][0], y, m.v[0][1]), z, m.v[0][2]);
                dst[1] = mlaf(mlaf(x * m.v[1][0], y, m.v[1][1]), z, m.v[1][2]);
                dst[2] = mlaf(mlaf(x * m.v[2][0], y, m.v[2][1]), z, m.v[2][2]);
            }
        }

        let max_colors = (1 << self.bit_depth) - 1;
        let color_scale = 1f32 / max_colors as f32;
        let lut_cap = (self.gamma_lut - 1) as f32;

        if self.intent != RenderingIntent::AbsoluteColorimetric {
            for dst in dst.chunks_exact_mut(3) {
                let mut rgb = Rgb::new(dst[0], dst[1], dst[2]);
                if rgb.is_out_of_gamut() {
                    rgb = filmlike_clip(rgb);
                }
                let r = mlaf(0.5f32, rgb.r, lut_cap).min(lut_cap).max(0f32) as u16;
                let g = mlaf(0.5f32, rgb.g, lut_cap).min(lut_cap).max(0f32) as u16;
                let b = mlaf(0.5f32, rgb.b, lut_cap).min(lut_cap).max(0f32) as u16;

                dst[0] = self.r_gamma[r as usize].as_() * color_scale;
                dst[1] = self.g_gamma[g as usize].as_() * color_scale;
                dst[2] = self.b_gamma[b as usize].as_() * color_scale;
            }
        } else {
            for dst in dst.chunks_exact_mut(3) {
                let rgb = Rgb::new(dst[0], dst[1], dst[2]);
                let r = mlaf(0.5f32, rgb.r, lut_cap).min(lut_cap).max(0f32) as u16;
                let g = mlaf(0.5f32, rgb.g, lut_cap).min(lut_cap).max(0f32) as u16;
                let b = mlaf(0.5f32, rgb.b, lut_cap).min(lut_cap).max(0f32) as u16;

                dst[0] = self.r_gamma[r as usize].as_() * color_scale;
                dst[1] = self.g_gamma[g as usize].as_() * color_scale;
                dst[2] = self.b_gamma[b as usize].as_() * color_scale;
            }
        }

        Ok(())
    }
}

pub(crate) struct XyzToRgbStageExtended<T: Clone> {
    pub(crate) gamma_evaluator: Box<dyn ToneCurveEvaluator>,
    pub(crate) matrices: Vec<Matrix3f>,
    pub(crate) phantom_data: PhantomData<T>,
}

impl<T: Clone + AsPrimitive<f32>> InPlaceStage for XyzToRgbStageExtended<T> {
    fn transform(&self, dst: &mut [f32]) -> Result<(), CmsError> {
        if !self.matrices.is_empty() {
            let m = self.matrices[0];
            for dst in dst.chunks_exact_mut(3) {
                let x = dst[0];
                let y = dst[1];
                let z = dst[2];
                dst[0] = mlaf(mlaf(x * m.v[0][0], y, m.v[0][1]), z, m.v[0][2]);
                dst[1] = mlaf(mlaf(x * m.v[1][0], y, m.v[1][1]), z, m.v[1][2]);
                dst[2] = mlaf(mlaf(x * m.v[2][0], y, m.v[2][1]), z, m.v[2][2]);
            }
        }

        for m in self.matrices.iter().skip(1) {
            for dst in dst.chunks_exact_mut(3) {
                let x = dst[0];
                let y = dst[1];
                let z = dst[2];
                dst[0] = mlaf(mlaf(x * m.v[0][0], y, m.v[0][1]), z, m.v[0][2]);
                dst[1] = mlaf(mlaf(x * m.v[1][0], y, m.v[1][1]), z, m.v[1][2]);
                dst[2] = mlaf(mlaf(x * m.v[2][0], y, m.v[2][1]), z, m.v[2][2]);
            }
        }

        for dst in dst.chunks_exact_mut(3) {
            let mut rgb = Rgb::new(dst[0], dst[1], dst[2]);
            rgb = self.gamma_evaluator.evaluate_tristimulus(rgb);
            dst[0] = rgb.r.as_();
            dst[1] = rgb.g.as_();
            dst[2] = rgb.b.as_();
        }

        Ok(())
    }
}

struct RgbLinearizationStage<T: Clone, const LINEAR_CAP: usize, const SAMPLES: usize> {
    r_lin: Box<[f32; LINEAR_CAP]>,
    g_lin: Box<[f32; LINEAR_CAP]>,
    b_lin: Box<[f32; LINEAR_CAP]>,
    _phantom: PhantomData<T>,
    bit_depth: usize,
}

impl<
    T: Clone + AsPrimitive<usize> + PointeeSizeExpressible,
    const LINEAR_CAP: usize,
    const SAMPLES: usize,
> RgbLinearizationStage<T, LINEAR_CAP, SAMPLES>
{
    fn transform(&self, src: &[T], dst: &mut [f32]) -> Result<(), CmsError> {
        if src.len() % 3 != 0 {
            return Err(CmsError::LaneMultipleOfChannels);
        }
        if dst.len() % 3 != 0 {
            return Err(CmsError::LaneMultipleOfChannels);
        }

        let scale = if T::FINITE {
            ((1 << self.bit_depth) - 1) as f32 / (SAMPLES as f32 - 1f32)
        } else {
            (T::NOT_FINITE_LINEAR_TABLE_SIZE - 1) as f32 / (SAMPLES as f32 - 1f32)
        };

        let capped_value = if T::FINITE {
            (1 << self.bit_depth) - 1
        } else {
            T::NOT_FINITE_LINEAR_TABLE_SIZE - 1
        };

        for (src, dst) in src.chunks_exact(3).zip(dst.chunks_exact_mut(3)) {
            let j_r = src[0].as_() as f32 * scale;
            let j_g = src[1].as_() as f32 * scale;
            let j_b = src[2].as_() as f32 * scale;
            dst[0] = self.r_lin[(j_r.round().max(0.0).min(capped_value as f32) as u16) as usize];
            dst[1] = self.g_lin[(j_g.round().max(0.0).min(capped_value as f32) as u16) as usize];
            dst[2] = self.b_lin[(j_b.round().max(0.0).min(capped_value as f32) as u16) as usize];
        }
        Ok(())
    }
}

pub(crate) fn create_rgb_lin_lut<
    T: Copy + Default + AsPrimitive<f32> + Send + Sync + AsPrimitive<usize> + PointeeSizeExpressible,
    const BIT_DEPTH: usize,
    const LINEAR_CAP: usize,
    const GRID_SIZE: usize,
>(
    source: &ColorProfile,
    opts: TransformOptions,
) -> Result<Vec<f32>, CmsError>
where
    u32: AsPrimitive<T>,
    f32: AsPrimitive<T>,
{
    let lut_origins = create_lut3_samples::<T, GRID_SIZE>();

    let lin_r =
        source.build_r_linearize_table::<T, LINEAR_CAP, BIT_DEPTH>(opts.allow_use_cicp_transfer)?;
    let lin_g =
        source.build_g_linearize_table::<T, LINEAR_CAP, BIT_DEPTH>(opts.allow_use_cicp_transfer)?;
    let lin_b =
        source.build_b_linearize_table::<T, LINEAR_CAP, BIT_DEPTH>(opts.allow_use_cicp_transfer)?;

    let lin_stage = RgbLinearizationStage::<T, LINEAR_CAP, GRID_SIZE> {
        r_lin: lin_r,
        g_lin: lin_g,
        b_lin: lin_b,
        _phantom: PhantomData,
        bit_depth: BIT_DEPTH,
    };

    let mut lut = vec![0f32; lut_origins.len()];
    lin_stage.transform(&lut_origins, &mut lut)?;

    let xyz_to_rgb = source.rgb_to_xyz_matrix();

    let matrices = vec![
        xyz_to_rgb.to_f32(),
        Matrix3f {
            v: [
                [32768.0 / 65535.0, 0.0, 0.0],
                [0.0, 32768.0 / 65535.0, 0.0],
                [0.0, 0.0, 32768.0 / 65535.0],
            ],
        },
    ];

    let matrix_stage = crate::conversions::lut_transforms::MatrixStage { matrices };
    matrix_stage.transform(&mut lut)?;
    Ok(lut)
}

pub(crate) fn prepare_inverse_lut_rgb_xyz<
    T: Copy
        + Default
        + AsPrimitive<f32>
        + Send
        + Sync
        + AsPrimitive<usize>
        + PointeeSizeExpressible
        + GammaLutInterpolate,
    const BIT_DEPTH: usize,
    const GAMMA_LUT: usize,
>(
    dest: &ColorProfile,
    lut: &mut [f32],
    options: TransformOptions,
) -> Result<(), CmsError>
where
    f32: AsPrimitive<T>,
    u32: AsPrimitive<T>,
{
    if !T::FINITE {
        if let Some(extended_gamma) = dest.try_extended_gamma_evaluator() {
            let xyz_to_rgb = dest.rgb_to_xyz_matrix().inverse();

            let mut matrices = vec![Matrix3f {
                v: [
                    [65535.0 / 32768.0, 0.0, 0.0],
                    [0.0, 65535.0 / 32768.0, 0.0],
                    [0.0, 0.0, 65535.0 / 32768.0],
                ],
            }];

            matrices.push(xyz_to_rgb.to_f32());
            let xyz_to_rgb_stage = XyzToRgbStageExtended::<T> {
                gamma_evaluator: extended_gamma,
                matrices,
                phantom_data: PhantomData,
            };
            xyz_to_rgb_stage.transform(lut)?;
            return Ok(());
        }
    }
    let gamma_map_r = dest.build_gamma_table::<T, 65536, GAMMA_LUT, BIT_DEPTH>(
        &dest.red_trc,
        options.allow_use_cicp_transfer,
    )?;
    let gamma_map_g = dest.build_gamma_table::<T, 65536, GAMMA_LUT, BIT_DEPTH>(
        &dest.green_trc,
        options.allow_use_cicp_transfer,
    )?;
    let gamma_map_b = dest.build_gamma_table::<T, 65536, GAMMA_LUT, BIT_DEPTH>(
        &dest.blue_trc,
        options.allow_use_cicp_transfer,
    )?;

    let xyz_to_rgb = dest.rgb_to_xyz_matrix().inverse();

    let mut matrices = vec![Matrix3f {
        v: [
            [65535.0 / 32768.0, 0.0, 0.0],
            [0.0, 65535.0 / 32768.0, 0.0],
            [0.0, 0.0, 65535.0 / 32768.0],
        ],
    }];

    matrices.push(xyz_to_rgb.to_f32());
    let xyz_to_rgb_stage = XyzToRgbStage::<T> {
        r_gamma: gamma_map_r,
        g_gamma: gamma_map_g,
        b_gamma: gamma_map_b,
        matrices,
        intent: options.rendering_intent,
        gamma_lut: GAMMA_LUT,
        bit_depth: BIT_DEPTH,
    };
    xyz_to_rgb_stage.transform(lut)?;
    Ok(())
}
