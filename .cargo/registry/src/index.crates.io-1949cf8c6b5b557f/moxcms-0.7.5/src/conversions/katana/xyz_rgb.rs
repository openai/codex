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
use crate::conversions::katana::{
    KatanaDefaultIntermediate, KatanaFinalStage, KatanaIntermediateStage,
};
use crate::mlaf::mlaf;
use crate::{
    CmsError, ColorProfile, GammaLutInterpolate, Layout, Matrix3f, PointeeSizeExpressible,
    RenderingIntent, Rgb, TransformOptions, filmlike_clip,
};
use num_traits::AsPrimitive;

pub(crate) struct KatanaXyzToRgbStage<T: Clone, const LAYOUT: u8> {
    pub(crate) r_gamma: Box<[T; 65536]>,
    pub(crate) g_gamma: Box<[T; 65536]>,
    pub(crate) b_gamma: Box<[T; 65536]>,
    pub(crate) intent: RenderingIntent,
    pub(crate) bit_depth: usize,
    pub(crate) gamma_lut: usize,
}

impl<T: Clone + AsPrimitive<f32> + PointeeSizeExpressible, const LAYOUT: u8>
    KatanaFinalStage<f32, T> for KatanaXyzToRgbStage<T, LAYOUT>
where
    u32: AsPrimitive<T>,
    f32: AsPrimitive<T>,
{
    fn to_output(&self, src: &mut [f32], dst: &mut [T]) -> Result<(), CmsError> {
        let dst_cn = Layout::from(LAYOUT);
        let dst_channels = dst_cn.channels();
        if src.len() % 3 != 0 {
            return Err(CmsError::LaneMultipleOfChannels);
        }
        if dst.len() % dst_channels != 0 {
            return Err(CmsError::LaneMultipleOfChannels);
        }
        let src_chunks = src.len() / 3;
        let dst_chunks = dst.len() / dst_channels;
        if src_chunks != dst_chunks {
            return Err(CmsError::LaneSizeMismatch);
        }

        let max_colors: T = (if T::FINITE {
            ((1u32 << self.bit_depth) - 1) as f32
        } else {
            1.0
        })
        .as_();
        let lut_cap = (self.gamma_lut - 1) as f32;

        if self.intent != RenderingIntent::AbsoluteColorimetric {
            for (src, dst) in src.chunks_exact(3).zip(dst.chunks_exact_mut(dst_channels)) {
                let mut rgb = Rgb::new(src[0], src[1], src[2]);
                if rgb.is_out_of_gamut() {
                    rgb = filmlike_clip(rgb);
                }
                let r = mlaf(0.5, rgb.r, lut_cap).min(lut_cap).max(0.) as u16;
                let g = mlaf(0.5, rgb.g, lut_cap).min(lut_cap).max(0.) as u16;
                let b = mlaf(0.5, rgb.b, lut_cap).min(lut_cap).max(0.) as u16;

                dst[0] = self.r_gamma[r as usize];
                dst[1] = self.g_gamma[g as usize];
                dst[2] = self.b_gamma[b as usize];
                if dst_cn == Layout::Rgba {
                    dst[3] = max_colors;
                }
            }
        } else {
            for (src, dst) in src.chunks_exact(3).zip(dst.chunks_exact_mut(dst_channels)) {
                let rgb = Rgb::new(src[0], src[1], src[2]);
                let r = mlaf(0.5, rgb.r, lut_cap).min(lut_cap).max(0.) as u16;
                let g = mlaf(0.5, rgb.g, lut_cap).min(lut_cap).max(0.) as u16;
                let b = mlaf(0.5, rgb.b, lut_cap).min(lut_cap).max(0.) as u16;

                dst[0] = self.r_gamma[r as usize];
                dst[1] = self.g_gamma[g as usize];
                dst[2] = self.b_gamma[b as usize];
                if dst_cn == Layout::Rgba {
                    dst[3] = max_colors;
                }
            }
        }

        Ok(())
    }
}

pub(crate) struct KatanaXyzRgbState<T> {
    pub(crate) stages: Vec<Box<dyn KatanaIntermediateStage<f32> + Send + Sync>>,
    pub(crate) final_stage: Box<dyn KatanaFinalStage<f32, T> + Send + Sync>,
}

pub(crate) fn katana_prepare_inverse_lut_rgb_xyz<
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
    dest_layout: Layout,
    options: TransformOptions,
) -> Result<KatanaXyzRgbState<T>, CmsError>
where
    f32: AsPrimitive<T>,
    u32: AsPrimitive<T>,
{
    // if !T::FINITE {
    // if let Some(extended_gamma) = dest.try_extended_gamma_evaluator() {
    //     let xyz_to_rgb = dest.rgb_to_xyz_matrix().inverse();
    //
    //     let mut matrices = vec![Matrix3f {
    //         v: [
    //             [65535.0 / 32768.0, 0.0, 0.0],
    //             [0.0, 65535.0 / 32768.0, 0.0],
    //             [0.0, 0.0, 65535.0 / 32768.0],
    //         ],
    //     }];
    //
    //     matrices.push(xyz_to_rgb.to_f32());
    //     let xyz_to_rgb_stage = XyzToRgbStageExtended::<T> {
    //         gamma_evaluator: extended_gamma,
    //         matrices,
    //         phantom_data: PhantomData,
    //     };
    //     xyz_to_rgb_stage.transform(lut)?;
    //     return Ok(());
    // }
    // }
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

    let mut matrices: Vec<Box<KatanaDefaultIntermediate>> =
        vec![Box::new(KatanaMatrixStage::new(Matrix3f {
            v: [
                [65535.0 / 32768.0, 0.0, 0.0],
                [0.0, 65535.0 / 32768.0, 0.0],
                [0.0, 0.0, 65535.0 / 32768.0],
            ],
        }))];

    matrices.push(Box::new(KatanaMatrixStage::new(xyz_to_rgb.to_f32())));
    match dest_layout {
        Layout::Rgb => {
            let xyz_to_rgb_stage = KatanaXyzToRgbStage::<T, { Layout::Rgb as u8 }> {
                r_gamma: gamma_map_r,
                g_gamma: gamma_map_g,
                b_gamma: gamma_map_b,
                intent: options.rendering_intent,
                bit_depth: BIT_DEPTH,
                gamma_lut: GAMMA_LUT,
            };
            Ok(KatanaXyzRgbState {
                stages: matrices,
                final_stage: Box::new(xyz_to_rgb_stage),
            })
        }
        Layout::Rgba => {
            let xyz_to_rgb_stage = KatanaXyzToRgbStage::<T, { Layout::Rgba as u8 }> {
                r_gamma: gamma_map_r,
                g_gamma: gamma_map_g,
                b_gamma: gamma_map_b,
                intent: options.rendering_intent,
                bit_depth: BIT_DEPTH,
                gamma_lut: GAMMA_LUT,
            };
            Ok(KatanaXyzRgbState {
                stages: matrices,
                final_stage: Box::new(xyz_to_rgb_stage),
            })
        }
        Layout::Gray => unreachable!("Gray layout must not be called on Rgb/Rgba path"),
        Layout::GrayAlpha => unreachable!("Gray layout must not be called on Rgb/Rgba path"),
        _ => unreachable!(
            "layout {:?} should not be called on xyz->rgb path",
            dest_layout
        ),
    }
}
