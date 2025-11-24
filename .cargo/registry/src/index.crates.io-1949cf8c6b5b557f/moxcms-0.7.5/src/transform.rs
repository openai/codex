/*
 * // Copyright (c) Radzivon Bartoshyk 2/2025. All rights reserved.
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
use crate::conversions::{
    LutBarycentricReduction, RgbXyzFactory, RgbXyzFactoryOpt, ToneReproductionRgbToGray,
    TransformMatrixShaper, make_gray_to_unfused, make_gray_to_x, make_lut_transform,
    make_rgb_to_gray,
};
use crate::err::CmsError;
use crate::trc::GammaLutInterpolate;
use crate::{ColorProfile, DataColorSpace, LutWarehouse, RenderingIntent, Vector3f, Xyzd};
use num_traits::AsPrimitive;
use std::marker::PhantomData;

/// Transformation executor itself
pub trait TransformExecutor<V: Copy + Default> {
    /// Count of samples always must match.
    /// If there is N samples of *Cmyk* source then N samples of *Rgb* is expected as an output.
    fn transform(&self, src: &[V], dst: &mut [V]) -> Result<(), CmsError>;
}

/// Helper for intermediate transformation stages
pub trait Stage {
    fn transform(&self, src: &[f32], dst: &mut [f32]) -> Result<(), CmsError>;
}

/// Helper for intermediate transformation stages
pub trait InPlaceStage {
    fn transform(&self, dst: &mut [f32]) -> Result<(), CmsError>;
}

/// Barycentric interpolation weights size.
///
/// Bigger weights increases precision.
#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash, Default)]
pub enum BarycentricWeightScale {
    #[default]
    /// Low scale weights is enough for common case.
    ///
    /// However, it might crush dark zones and gradients.
    /// Weights increasing costs 5% performance.
    Low,
    #[cfg(feature = "options")]
    High,
}

/// Declares additional transformation options
#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct TransformOptions {
    pub rendering_intent: RenderingIntent,
    /// If set it will try to use Transfer Characteristics from CICP
    /// on transform. This might be more precise and faster.
    pub allow_use_cicp_transfer: bool,
    /// Prefers fixed point where implemented as default.
    /// Most of the applications actually do not need floating point.
    ///
    /// Do not change it if you're not sure that extreme precision is required,
    /// in most cases it is a simple way to spend energy to warming up environment
    /// a little.
    ///
    /// Q2.13 for RGB->XYZ->RGB is used.
    /// LUT interpolation use Q0.15.
    pub prefer_fixed_point: bool,
    /// Interpolation method for 3D LUT
    ///
    /// This parameter has no effect on LAB/XYZ interpolation and scene linear RGB.
    ///
    /// Technically, it should be assumed to perform cube dividing interpolation:
    /// - Source colorspace is gamma-encoded (discards scene linear RGB and XYZ).
    /// - Colorspace is uniform.
    /// - Colorspace has linear scaling (discards LAB).
    /// - Interpolation doesn't shift hues (discards LAB).
    ///
    /// For LAB, XYZ and scene linear RGB `trilinear/quadlinear` always in force.
    pub interpolation_method: InterpolationMethod,
    /// Barycentric weights scale.
    ///
    /// This value controls LUT weights precision.
    pub barycentric_weight_scale: BarycentricWeightScale,
    /// For floating points transform, it will try to detect gamma function on *Matrix Shaper* profiles.
    /// If gamma function is found, then it will be used instead of LUT table.
    /// This allows to work with excellent precision with extended range,
    /// at a cost of execution time.
    pub allow_extended_range_rgb_xyz: bool,
    // pub black_point_compensation: bool,
}

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash, Default)]
/// Defines the interpolation method.
///
/// All methods produce very close results that almost not possible to separate without
/// some automation tools.
///
/// This implementation chooses the fastest method as default.
pub enum InterpolationMethod {
    /// General Tetrahedron interpolation.
    /// This is used in lcms2 and others CMS.
    #[cfg(feature = "options")]
    Tetrahedral,
    /// Divides cube into a pyramids and interpolate then in the pyramid.
    #[cfg(feature = "options")]
    Pyramid,
    /// Interpolation by dividing cube into prisms.
    #[cfg(feature = "options")]
    Prism,
    /// Trilinear/Quadlinear interpolation
    #[default]
    Linear,
}

impl Default for TransformOptions {
    fn default() -> Self {
        Self {
            rendering_intent: RenderingIntent::default(),
            allow_use_cicp_transfer: true,
            prefer_fixed_point: true,
            interpolation_method: InterpolationMethod::default(),
            barycentric_weight_scale: BarycentricWeightScale::default(),
            allow_extended_range_rgb_xyz: false,
            // black_point_compensation: false,
        }
    }
}

pub type Transform8BitExecutor = dyn TransformExecutor<u8> + Send + Sync;
pub type Transform16BitExecutor = dyn TransformExecutor<u16> + Send + Sync;
pub type TransformF32BitExecutor = dyn TransformExecutor<f32> + Send + Sync;
pub type TransformF64BitExecutor = dyn TransformExecutor<f64> + Send + Sync;

/// Layout declares a data layout.
/// For RGB it shows also the channel order.
/// To handle different data bit-depth appropriate executor must be used.
/// Cmyk8 uses the same layout as Rgba8.
#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub enum Layout {
    Rgb = 0,
    Rgba = 1,
    Gray = 2,
    GrayAlpha = 3,
    Inks5 = 4,
    Inks6 = 5,
    Inks7 = 6,
    Inks8 = 7,
    Inks9 = 8,
    Inks10 = 9,
    Inks11 = 10,
    Inks12 = 11,
    Inks13 = 12,
    Inks14 = 13,
    Inks15 = 14,
}

impl Layout {
    /// Returns Red channel index
    #[inline(always)]
    pub const fn r_i(self) -> usize {
        match self {
            Layout::Rgb => 0,
            Layout::Rgba => 0,
            Layout::Gray => unimplemented!(),
            Layout::GrayAlpha => unimplemented!(),
            _ => unimplemented!(),
        }
    }

    /// Returns Green channel index
    #[inline(always)]
    pub const fn g_i(self) -> usize {
        match self {
            Layout::Rgb => 1,
            Layout::Rgba => 1,
            Layout::Gray => unimplemented!(),
            Layout::GrayAlpha => unimplemented!(),
            _ => unimplemented!(),
        }
    }

    /// Returns Blue channel index
    #[inline(always)]
    pub const fn b_i(self) -> usize {
        match self {
            Layout::Rgb => 2,
            Layout::Rgba => 2,
            Layout::Gray => unimplemented!(),
            Layout::GrayAlpha => unimplemented!(),
            _ => unimplemented!(),
        }
    }

    #[inline(always)]
    pub const fn a_i(self) -> usize {
        match self {
            Layout::Rgb => unimplemented!(),
            Layout::Rgba => 3,
            Layout::Gray => unimplemented!(),
            Layout::GrayAlpha => 1,
            _ => unimplemented!(),
        }
    }

    #[inline(always)]
    pub const fn has_alpha(self) -> bool {
        match self {
            Layout::Rgb => false,
            Layout::Rgba => true,
            Layout::Gray => false,
            Layout::GrayAlpha => true,
            _ => false,
        }
    }

    #[inline]
    pub const fn channels(self) -> usize {
        match self {
            Layout::Rgb => 3,
            Layout::Rgba => 4,
            Layout::Gray => 1,
            Layout::GrayAlpha => 2,
            Layout::Inks5 => 5,
            Layout::Inks6 => 6,
            Layout::Inks7 => 7,
            Layout::Inks8 => 8,
            Layout::Inks9 => 9,
            Layout::Inks10 => 10,
            Layout::Inks11 => 11,
            Layout::Inks12 => 12,
            Layout::Inks13 => 13,
            Layout::Inks14 => 14,
            Layout::Inks15 => 15,
        }
    }

    pub(crate) fn from_inks(inks: usize) -> Self {
        match inks {
            1 => Layout::Gray,
            2 => Layout::GrayAlpha,
            3 => Layout::Rgb,
            4 => Layout::Rgba,
            5 => Layout::Inks5,
            6 => Layout::Inks6,
            7 => Layout::Inks7,
            8 => Layout::Inks8,
            9 => Layout::Inks9,
            10 => Layout::Inks10,
            11 => Layout::Inks11,
            12 => Layout::Inks12,
            13 => Layout::Inks13,
            14 => Layout::Inks14,
            15 => Layout::Inks15,
            _ => unreachable!("Impossible amount of inks"),
        }
    }
}

impl From<u8> for Layout {
    fn from(value: u8) -> Self {
        match value {
            0 => Layout::Rgb,
            1 => Layout::Rgba,
            2 => Layout::Gray,
            3 => Layout::GrayAlpha,
            _ => unimplemented!(),
        }
    }
}

impl Layout {
    #[inline(always)]
    pub const fn resolve(value: u8) -> Self {
        match value {
            0 => Layout::Rgb,
            1 => Layout::Rgba,
            2 => Layout::Gray,
            3 => Layout::GrayAlpha,
            4 => Layout::Inks5,
            5 => Layout::Inks6,
            6 => Layout::Inks7,
            7 => Layout::Inks8,
            8 => Layout::Inks9,
            9 => Layout::Inks10,
            10 => Layout::Inks11,
            11 => Layout::Inks12,
            12 => Layout::Inks13,
            13 => Layout::Inks14,
            14 => Layout::Inks15,
            _ => unimplemented!(),
        }
    }
}

#[doc(hidden)]
pub trait PointeeSizeExpressible {
    fn _as_usize(self) -> usize;
    const FINITE: bool;
    const NOT_FINITE_GAMMA_TABLE_SIZE: usize;
    const NOT_FINITE_LINEAR_TABLE_SIZE: usize;
    const IS_U8: bool;
    const IS_U16: bool;
}

impl PointeeSizeExpressible for u8 {
    #[inline(always)]
    fn _as_usize(self) -> usize {
        self as usize
    }

    const FINITE: bool = true;
    const NOT_FINITE_GAMMA_TABLE_SIZE: usize = 1;
    const NOT_FINITE_LINEAR_TABLE_SIZE: usize = 1;
    const IS_U8: bool = true;
    const IS_U16: bool = false;
}

impl PointeeSizeExpressible for u16 {
    #[inline(always)]
    fn _as_usize(self) -> usize {
        self as usize
    }

    const FINITE: bool = true;

    const NOT_FINITE_GAMMA_TABLE_SIZE: usize = 1;
    const NOT_FINITE_LINEAR_TABLE_SIZE: usize = 1;

    const IS_U8: bool = false;
    const IS_U16: bool = true;
}

impl PointeeSizeExpressible for f32 {
    #[inline(always)]
    fn _as_usize(self) -> usize {
        const MAX_14_BIT: f32 = ((1 << 14u32) - 1) as f32;
        ((self * MAX_14_BIT).max(0f32).min(MAX_14_BIT) as u16) as usize
    }

    const FINITE: bool = false;

    const NOT_FINITE_GAMMA_TABLE_SIZE: usize = 32768;
    const NOT_FINITE_LINEAR_TABLE_SIZE: usize = 1 << 14u32;
    const IS_U8: bool = false;
    const IS_U16: bool = false;
}

impl PointeeSizeExpressible for f64 {
    #[inline(always)]
    fn _as_usize(self) -> usize {
        const MAX_16_BIT: f64 = ((1 << 16u32) - 1) as f64;
        ((self * MAX_16_BIT).max(0.).min(MAX_16_BIT) as u16) as usize
    }

    const FINITE: bool = false;

    const NOT_FINITE_GAMMA_TABLE_SIZE: usize = 65536;
    const NOT_FINITE_LINEAR_TABLE_SIZE: usize = 1 << 16;
    const IS_U8: bool = false;
    const IS_U16: bool = false;
}

impl ColorProfile {
    /// Checks if profile is valid *Matrix Shaper* profile
    pub fn is_matrix_shaper(&self) -> bool {
        self.color_space == DataColorSpace::Rgb
            && self.red_colorant != Xyzd::default()
            && self.green_colorant != Xyzd::default()
            && self.blue_colorant != Xyzd::default()
            && self.red_trc.is_some()
            && self.green_trc.is_some()
            && self.blue_trc.is_some()
    }

    /// Creates transform between source and destination profile
    /// Use for 16 bit-depth data bit-depth only.
    pub fn create_transform_16bit(
        &self,
        src_layout: Layout,
        dst_pr: &ColorProfile,
        dst_layout: Layout,
        options: TransformOptions,
    ) -> Result<Box<Transform16BitExecutor>, CmsError> {
        self.create_transform_nbit::<u16, 16, 65536, 65536>(src_layout, dst_pr, dst_layout, options)
    }

    /// Creates transform between source and destination profile
    /// Use for 12 bit-depth data bit-depth only.
    pub fn create_transform_12bit(
        &self,
        src_layout: Layout,
        dst_pr: &ColorProfile,
        dst_layout: Layout,
        options: TransformOptions,
    ) -> Result<Box<Transform16BitExecutor>, CmsError> {
        self.create_transform_nbit::<u16, 12, 65536, 16384>(src_layout, dst_pr, dst_layout, options)
    }

    /// Creates transform between source and destination profile
    /// Use for 10 bit-depth data bit-depth only.
    pub fn create_transform_10bit(
        &self,
        src_layout: Layout,
        dst_pr: &ColorProfile,
        dst_layout: Layout,
        options: TransformOptions,
    ) -> Result<Box<Transform16BitExecutor>, CmsError> {
        self.create_transform_nbit::<u16, 10, 65536, 8192>(src_layout, dst_pr, dst_layout, options)
    }

    /// Creates transform between source and destination profile
    /// Data has to be normalized into [0, 1] range.
    /// ICC profiles and LUT tables do not exist in infinite precision.
    /// Thus, this implementation considers `f32` as 14-bit values.
    /// Floating point transformer works in extended mode, that means returned data might be negative
    /// or more than 1.
    pub fn create_transform_f32(
        &self,
        src_layout: Layout,
        dst_pr: &ColorProfile,
        dst_layout: Layout,
        options: TransformOptions,
    ) -> Result<Box<TransformF32BitExecutor>, CmsError> {
        self.create_transform_nbit::<f32, 1, 65536, 32768>(src_layout, dst_pr, dst_layout, options)
    }

    /// Creates transform between source and destination profile
    /// Data has to be normalized into [0, 1] range.
    /// ICC profiles and LUT tables do not exist in infinite precision.
    /// Thus, this implementation considers `f64` as 16-bit values.
    /// Floating point transformer works in extended mode, that means returned data might be negative
    /// or more than 1.
    pub fn create_transform_f64(
        &self,
        src_layout: Layout,
        dst_pr: &ColorProfile,
        dst_layout: Layout,
        options: TransformOptions,
    ) -> Result<Box<TransformF64BitExecutor>, CmsError> {
        self.create_transform_nbit::<f64, 1, 65536, 65536>(src_layout, dst_pr, dst_layout, options)
    }

    fn create_transform_nbit<
        T: Copy
            + Default
            + AsPrimitive<usize>
            + PointeeSizeExpressible
            + Send
            + Sync
            + AsPrimitive<f32>
            + RgbXyzFactory<T>
            + RgbXyzFactoryOpt<T>
            + GammaLutInterpolate,
        const BIT_DEPTH: usize,
        const LINEAR_CAP: usize,
        const GAMMA_CAP: usize,
    >(
        &self,
        src_layout: Layout,
        dst_pr: &ColorProfile,
        dst_layout: Layout,
        options: TransformOptions,
    ) -> Result<Box<dyn TransformExecutor<T> + Send + Sync>, CmsError>
    where
        f32: AsPrimitive<T>,
        u32: AsPrimitive<T>,
        (): LutBarycentricReduction<T, u8>,
        (): LutBarycentricReduction<T, u16>,
    {
        if self.color_space == DataColorSpace::Rgb
            && dst_pr.pcs == DataColorSpace::Xyz
            && dst_pr.color_space == DataColorSpace::Rgb
            && self.pcs == DataColorSpace::Xyz
            && self.is_matrix_shaper()
            && dst_pr.is_matrix_shaper()
        {
            if src_layout == Layout::Gray || src_layout == Layout::GrayAlpha {
                return Err(CmsError::InvalidLayout);
            }
            if dst_layout == Layout::Gray || dst_layout == Layout::GrayAlpha {
                return Err(CmsError::InvalidLayout);
            }

            if self.has_device_to_pcs_lut() || dst_pr.has_pcs_to_device_lut() {
                return make_lut_transform::<T, BIT_DEPTH, LINEAR_CAP, GAMMA_CAP>(
                    src_layout, self, dst_layout, dst_pr, options,
                );
            }

            let transform = self.transform_matrix(dst_pr);

            if !T::FINITE && options.allow_extended_range_rgb_xyz {
                if let Some(gamma_evaluator) = dst_pr.try_extended_gamma_evaluator() {
                    if let Some(linear_evaluator) = self.try_extended_linearizing_evaluator() {
                        use crate::conversions::{
                            TransformShaperFloatInOut, make_rgb_xyz_rgb_transform_float_in_out,
                        };
                        let p = TransformShaperFloatInOut {
                            linear_evaluator,
                            gamma_evaluator,
                            adaptation_matrix: transform.to_f32(),
                            phantom_data: PhantomData,
                        };
                        return make_rgb_xyz_rgb_transform_float_in_out::<T, BIT_DEPTH>(
                            src_layout, dst_layout, p,
                        );
                    }

                    let lin_r = self.build_r_linearize_table::<T, LINEAR_CAP, BIT_DEPTH>(
                        options.allow_use_cicp_transfer,
                    )?;
                    let lin_g = self.build_g_linearize_table::<T, LINEAR_CAP, BIT_DEPTH>(
                        options.allow_use_cicp_transfer,
                    )?;
                    let lin_b = self.build_b_linearize_table::<T, LINEAR_CAP, BIT_DEPTH>(
                        options.allow_use_cicp_transfer,
                    )?;

                    use crate::conversions::{
                        TransformShaperRgbFloat, make_rgb_xyz_rgb_transform_float,
                    };
                    let p = TransformShaperRgbFloat {
                        r_linear: lin_r,
                        g_linear: lin_g,
                        b_linear: lin_b,
                        gamma_evaluator,
                        adaptation_matrix: transform.to_f32(),
                        phantom_data: PhantomData,
                    };
                    return make_rgb_xyz_rgb_transform_float::<T, LINEAR_CAP, BIT_DEPTH>(
                        src_layout, dst_layout, p,
                    );
                }
            }

            if self.are_all_trc_the_same() && dst_pr.are_all_trc_the_same() {
                let linear = self.build_r_linearize_table::<T, LINEAR_CAP, BIT_DEPTH>(
                    options.allow_use_cicp_transfer,
                )?;

                let gamma = dst_pr.build_gamma_table::<T, 65536, GAMMA_CAP, BIT_DEPTH>(
                    &dst_pr.red_trc,
                    options.allow_use_cicp_transfer,
                )?;

                let profile_transform = crate::conversions::TransformMatrixShaperOptimized {
                    linear,
                    gamma,
                    adaptation_matrix: transform.to_f32(),
                };

                return T::make_optimized_transform::<LINEAR_CAP, GAMMA_CAP, BIT_DEPTH>(
                    src_layout,
                    dst_layout,
                    profile_transform,
                    options,
                );
            }

            let lin_r = self.build_r_linearize_table::<T, LINEAR_CAP, BIT_DEPTH>(
                options.allow_use_cicp_transfer,
            )?;
            let lin_g = self.build_g_linearize_table::<T, LINEAR_CAP, BIT_DEPTH>(
                options.allow_use_cicp_transfer,
            )?;
            let lin_b = self.build_b_linearize_table::<T, LINEAR_CAP, BIT_DEPTH>(
                options.allow_use_cicp_transfer,
            )?;

            let gamma_r = dst_pr.build_gamma_table::<T, 65536, GAMMA_CAP, BIT_DEPTH>(
                &dst_pr.red_trc,
                options.allow_use_cicp_transfer,
            )?;
            let gamma_g = dst_pr.build_gamma_table::<T, 65536, GAMMA_CAP, BIT_DEPTH>(
                &dst_pr.green_trc,
                options.allow_use_cicp_transfer,
            )?;
            let gamma_b = dst_pr.build_gamma_table::<T, 65536, GAMMA_CAP, BIT_DEPTH>(
                &dst_pr.blue_trc,
                options.allow_use_cicp_transfer,
            )?;

            let profile_transform = TransformMatrixShaper {
                r_linear: lin_r,
                g_linear: lin_g,
                b_linear: lin_b,
                r_gamma: gamma_r,
                g_gamma: gamma_g,
                b_gamma: gamma_b,
                adaptation_matrix: transform.to_f32(),
            };

            T::make_transform::<LINEAR_CAP, GAMMA_CAP, BIT_DEPTH>(
                src_layout,
                dst_layout,
                profile_transform,
                options,
            )
        } else if (self.color_space == DataColorSpace::Gray && self.gray_trc.is_some())
            && (dst_pr.color_space == DataColorSpace::Rgb
                || (dst_pr.color_space == DataColorSpace::Gray && dst_pr.gray_trc.is_some()))
            && self.pcs == DataColorSpace::Xyz
            && dst_pr.pcs == DataColorSpace::Xyz
        {
            if src_layout != Layout::GrayAlpha && src_layout != Layout::Gray {
                return Err(CmsError::InvalidLayout);
            }

            if self.has_device_to_pcs_lut() || dst_pr.has_pcs_to_device_lut() {
                return make_lut_transform::<T, BIT_DEPTH, LINEAR_CAP, GAMMA_CAP>(
                    src_layout, self, dst_layout, dst_pr, options,
                );
            }

            let gray_linear = self.build_gray_linearize_table::<T, LINEAR_CAP, BIT_DEPTH>()?;

            if dst_pr.color_space == DataColorSpace::Gray {
                if !T::FINITE && options.allow_extended_range_rgb_xyz {
                    if let Some(gamma_evaluator) = dst_pr.try_extended_gamma_evaluator() {
                        if let Some(linear_evaluator) = self.try_extended_linearizing_evaluator() {
                            // Gray -> Gray case extended range
                            use crate::conversions::make_gray_to_one_trc_extended;
                            return make_gray_to_one_trc_extended::<T>(
                                src_layout,
                                dst_layout,
                                linear_evaluator,
                                gamma_evaluator,
                                BIT_DEPTH,
                            );
                        }
                    }
                }

                // Gray -> Gray case
                let gray_gamma = dst_pr.build_gamma_table::<T, 65536, GAMMA_CAP, BIT_DEPTH>(
                    &dst_pr.gray_trc,
                    options.allow_use_cicp_transfer,
                )?;

                make_gray_to_x::<T, LINEAR_CAP>(
                    src_layout,
                    dst_layout,
                    &gray_linear,
                    &gray_gamma,
                    BIT_DEPTH,
                    GAMMA_CAP,
                )
            } else {
                #[allow(clippy::collapsible_if)]
                if dst_pr.are_all_trc_the_same() {
                    if !T::FINITE && options.allow_extended_range_rgb_xyz {
                        if let Some(gamma_evaluator) = dst_pr.try_extended_gamma_evaluator() {
                            if let Some(linear_evaluator) =
                                self.try_extended_linearizing_evaluator()
                            {
                                // Gray -> RGB where all TRC is the same with extended range
                                use crate::conversions::make_gray_to_one_trc_extended;
                                return make_gray_to_one_trc_extended::<T>(
                                    src_layout,
                                    dst_layout,
                                    linear_evaluator,
                                    gamma_evaluator,
                                    BIT_DEPTH,
                                );
                            }
                        }
                    }

                    // Gray -> RGB where all TRC is the same
                    let rgb_gamma = dst_pr.build_gamma_table::<T, 65536, GAMMA_CAP, BIT_DEPTH>(
                        &dst_pr.red_trc,
                        options.allow_use_cicp_transfer,
                    )?;

                    make_gray_to_x::<T, LINEAR_CAP>(
                        src_layout,
                        dst_layout,
                        &gray_linear,
                        &rgb_gamma,
                        BIT_DEPTH,
                        GAMMA_CAP,
                    )
                } else {
                    // Gray -> RGB where all TRC is NOT the same
                    if !T::FINITE && options.allow_extended_range_rgb_xyz {
                        if let Some(gamma_evaluator) = dst_pr.try_extended_gamma_evaluator() {
                            if let Some(linear_evaluator) =
                                self.try_extended_linearizing_evaluator()
                            {
                                // Gray -> RGB where all TRC is NOT the same with extended range

                                use crate::conversions::make_gray_to_rgb_extended;
                                return make_gray_to_rgb_extended::<T>(
                                    src_layout,
                                    dst_layout,
                                    linear_evaluator,
                                    gamma_evaluator,
                                    BIT_DEPTH,
                                );
                            }
                        }
                    }

                    let red_gamma = dst_pr.build_gamma_table::<T, 65536, GAMMA_CAP, BIT_DEPTH>(
                        &dst_pr.red_trc,
                        options.allow_use_cicp_transfer,
                    )?;
                    let green_gamma = dst_pr.build_gamma_table::<T, 65536, GAMMA_CAP, BIT_DEPTH>(
                        &dst_pr.green_trc,
                        options.allow_use_cicp_transfer,
                    )?;
                    let blue_gamma = dst_pr.build_gamma_table::<T, 65536, GAMMA_CAP, BIT_DEPTH>(
                        &dst_pr.blue_trc,
                        options.allow_use_cicp_transfer,
                    )?;

                    make_gray_to_unfused::<T, LINEAR_CAP>(
                        src_layout,
                        dst_layout,
                        gray_linear,
                        red_gamma,
                        green_gamma,
                        blue_gamma,
                        BIT_DEPTH,
                        GAMMA_CAP,
                    )
                }
            }
        } else if self.color_space == DataColorSpace::Rgb
            && (dst_pr.color_space == DataColorSpace::Gray && dst_pr.gray_trc.is_some())
            && dst_pr.pcs == DataColorSpace::Xyz
            && self.pcs == DataColorSpace::Xyz
        {
            if src_layout == Layout::Gray || src_layout == Layout::GrayAlpha {
                return Err(CmsError::InvalidLayout);
            }
            if dst_layout != Layout::Gray && dst_layout != Layout::GrayAlpha {
                return Err(CmsError::InvalidLayout);
            }

            if self.has_device_to_pcs_lut() || dst_pr.has_pcs_to_device_lut() {
                return make_lut_transform::<T, BIT_DEPTH, LINEAR_CAP, GAMMA_CAP>(
                    src_layout, self, dst_layout, dst_pr, options,
                );
            }

            let transform = self.transform_matrix(dst_pr).to_f32();

            let vector = Vector3f {
                v: [transform.v[1][0], transform.v[1][1], transform.v[1][2]],
            };

            if !T::FINITE && options.allow_extended_range_rgb_xyz {
                if let Some(gamma_evaluator) = dst_pr.try_extended_gamma_evaluator() {
                    if let Some(linear_evaluator) = self.try_extended_linearizing_evaluator() {
                        use crate::conversions::make_rgb_to_gray_extended;
                        return Ok(make_rgb_to_gray_extended::<T>(
                            src_layout,
                            dst_layout,
                            linear_evaluator,
                            gamma_evaluator,
                            vector,
                            BIT_DEPTH,
                        ));
                    }
                }
            }

            let lin_r = self.build_r_linearize_table::<T, LINEAR_CAP, BIT_DEPTH>(
                options.allow_use_cicp_transfer,
            )?;
            let lin_g = self.build_g_linearize_table::<T, LINEAR_CAP, BIT_DEPTH>(
                options.allow_use_cicp_transfer,
            )?;
            let lin_b = self.build_b_linearize_table::<T, LINEAR_CAP, BIT_DEPTH>(
                options.allow_use_cicp_transfer,
            )?;
            let gray_linear = dst_pr.build_gamma_table::<T, 65536, GAMMA_CAP, BIT_DEPTH>(
                &dst_pr.gray_trc,
                options.allow_use_cicp_transfer,
            )?;

            let trc_box = ToneReproductionRgbToGray::<T, LINEAR_CAP> {
                r_linear: lin_r,
                g_linear: lin_g,
                b_linear: lin_b,
                gray_gamma: gray_linear,
            };

            Ok(make_rgb_to_gray::<T, LINEAR_CAP, BIT_DEPTH, GAMMA_CAP>(
                src_layout, dst_layout, trc_box, vector,
            ))
        } else if (self.color_space.is_three_channels()
            || self.color_space == DataColorSpace::Cmyk
            || self.color_space == DataColorSpace::Color4)
            && (dst_pr.color_space.is_three_channels()
                || dst_pr.color_space == DataColorSpace::Cmyk
                || dst_pr.color_space == DataColorSpace::Color4)
            && (dst_pr.pcs == DataColorSpace::Xyz || dst_pr.pcs == DataColorSpace::Lab)
            && (self.pcs == DataColorSpace::Xyz || self.pcs == DataColorSpace::Lab)
        {
            if src_layout == Layout::Gray || src_layout == Layout::GrayAlpha {
                return Err(CmsError::InvalidLayout);
            }
            if dst_layout == Layout::Gray || dst_layout == Layout::GrayAlpha {
                return Err(CmsError::InvalidLayout);
            }
            make_lut_transform::<T, BIT_DEPTH, LINEAR_CAP, GAMMA_CAP>(
                src_layout, self, dst_layout, dst_pr, options,
            )
        } else {
            make_lut_transform::<T, BIT_DEPTH, LINEAR_CAP, GAMMA_CAP>(
                src_layout, self, dst_layout, dst_pr, options,
            )
        }
    }

    /// Creates transform between source and destination profile
    /// Only 8 bit is supported.
    pub fn create_transform_8bit(
        &self,
        src_layout: Layout,
        dst_pr: &ColorProfile,
        dst_layout: Layout,
        options: TransformOptions,
    ) -> Result<Box<Transform8BitExecutor>, CmsError> {
        self.create_transform_nbit::<u8, 8, 256, 4096>(src_layout, dst_pr, dst_layout, options)
    }

    pub(crate) fn get_device_to_pcs(&self, intent: RenderingIntent) -> Option<&LutWarehouse> {
        match intent {
            RenderingIntent::AbsoluteColorimetric => self.lut_a_to_b_colorimetric.as_ref(),
            RenderingIntent::Saturation => self.lut_a_to_b_saturation.as_ref(),
            RenderingIntent::RelativeColorimetric => self.lut_a_to_b_colorimetric.as_ref(),
            RenderingIntent::Perceptual => self.lut_a_to_b_perceptual.as_ref(),
        }
    }

    pub(crate) fn get_pcs_to_device(&self, intent: RenderingIntent) -> Option<&LutWarehouse> {
        match intent {
            RenderingIntent::AbsoluteColorimetric => self.lut_b_to_a_colorimetric.as_ref(),
            RenderingIntent::Saturation => self.lut_b_to_a_saturation.as_ref(),
            RenderingIntent::RelativeColorimetric => self.lut_b_to_a_colorimetric.as_ref(),
            RenderingIntent::Perceptual => self.lut_b_to_a_perceptual.as_ref(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{ColorProfile, DataColorSpace, Layout, RenderingIntent, TransformOptions};
    use rand::Rng;

    #[test]
    fn test_transform_rgb8() {
        let mut srgb_profile = ColorProfile::new_srgb();
        let bt2020_profile = ColorProfile::new_bt2020();
        let random_point_x = rand::rng().random_range(0..255);
        let transform = bt2020_profile
            .create_transform_8bit(
                Layout::Rgb,
                &srgb_profile,
                Layout::Rgb,
                TransformOptions::default(),
            )
            .unwrap();
        let src = vec![random_point_x; 256 * 256 * 3];
        let mut dst = vec![random_point_x; 256 * 256 * 3];
        transform.transform(&src, &mut dst).unwrap();

        let transform = bt2020_profile
            .create_transform_8bit(
                Layout::Rgb,
                &srgb_profile,
                Layout::Rgb,
                TransformOptions {
                    ..TransformOptions::default()
                },
            )
            .unwrap();
        transform.transform(&src, &mut dst).unwrap();
        srgb_profile.rendering_intent = RenderingIntent::RelativeColorimetric;
        let transform = bt2020_profile
            .create_transform_8bit(
                Layout::Rgb,
                &srgb_profile,
                Layout::Rgb,
                TransformOptions {
                    ..TransformOptions::default()
                },
            )
            .unwrap();
        transform.transform(&src, &mut dst).unwrap();
        srgb_profile.rendering_intent = RenderingIntent::Saturation;
        let transform = bt2020_profile
            .create_transform_8bit(
                Layout::Rgb,
                &srgb_profile,
                Layout::Rgb,
                TransformOptions {
                    ..TransformOptions::default()
                },
            )
            .unwrap();
        transform.transform(&src, &mut dst).unwrap();
    }

    #[test]
    fn test_transform_rgba8() {
        let srgb_profile = ColorProfile::new_srgb();
        let bt2020_profile = ColorProfile::new_bt2020();
        let random_point_x = rand::rng().random_range(0..255);
        let transform = bt2020_profile
            .create_transform_8bit(
                Layout::Rgba,
                &srgb_profile,
                Layout::Rgba,
                TransformOptions::default(),
            )
            .unwrap();
        let src = vec![random_point_x; 256 * 256 * 4];
        let mut dst = vec![random_point_x; 256 * 256 * 4];
        transform.transform(&src, &mut dst).unwrap();
    }

    #[test]
    fn test_transform_gray_to_rgb8() {
        let gray_profile = ColorProfile::new_gray_with_gamma(2.2f32);
        let bt2020_profile = ColorProfile::new_bt2020();
        let random_point_x = rand::rng().random_range(0..255);
        let transform = gray_profile
            .create_transform_8bit(
                Layout::Gray,
                &bt2020_profile,
                Layout::Rgb,
                TransformOptions::default(),
            )
            .unwrap();
        let src = vec![random_point_x; 256 * 256];
        let mut dst = vec![random_point_x; 256 * 256 * 3];
        transform.transform(&src, &mut dst).unwrap();
    }

    #[test]
    fn test_transform_gray_to_rgba8() {
        let srgb_profile = ColorProfile::new_gray_with_gamma(2.2f32);
        let bt2020_profile = ColorProfile::new_bt2020();
        let random_point_x = rand::rng().random_range(0..255);
        let transform = srgb_profile
            .create_transform_8bit(
                Layout::Gray,
                &bt2020_profile,
                Layout::Rgba,
                TransformOptions::default(),
            )
            .unwrap();
        let src = vec![random_point_x; 256 * 256];
        let mut dst = vec![random_point_x; 256 * 256 * 4];
        transform.transform(&src, &mut dst).unwrap();
    }

    #[test]
    fn test_transform_gray_to_gray_alpha8() {
        let srgb_profile = ColorProfile::new_gray_with_gamma(2.2f32);
        let bt2020_profile = ColorProfile::new_bt2020();
        let random_point_x = rand::rng().random_range(0..255);
        let transform = srgb_profile
            .create_transform_8bit(
                Layout::Gray,
                &bt2020_profile,
                Layout::GrayAlpha,
                TransformOptions::default(),
            )
            .unwrap();
        let src = vec![random_point_x; 256 * 256];
        let mut dst = vec![random_point_x; 256 * 256 * 2];
        transform.transform(&src, &mut dst).unwrap();
    }

    #[test]
    fn test_transform_rgb10() {
        let srgb_profile = ColorProfile::new_srgb();
        let bt2020_profile = ColorProfile::new_bt2020();
        let random_point_x = rand::rng().random_range(0..((1 << 10) - 1));
        let transform = bt2020_profile
            .create_transform_10bit(
                Layout::Rgb,
                &srgb_profile,
                Layout::Rgb,
                TransformOptions::default(),
            )
            .unwrap();
        let src = vec![random_point_x; 256 * 256 * 3];
        let mut dst = vec![random_point_x; 256 * 256 * 3];
        transform.transform(&src, &mut dst).unwrap();
    }

    #[test]
    fn test_transform_rgb12() {
        let srgb_profile = ColorProfile::new_srgb();
        let bt2020_profile = ColorProfile::new_bt2020();
        let random_point_x = rand::rng().random_range(0..((1 << 12) - 1));
        let transform = bt2020_profile
            .create_transform_12bit(
                Layout::Rgb,
                &srgb_profile,
                Layout::Rgb,
                TransformOptions::default(),
            )
            .unwrap();
        let src = vec![random_point_x; 256 * 256 * 3];
        let mut dst = vec![random_point_x; 256 * 256 * 3];
        transform.transform(&src, &mut dst).unwrap();
    }

    #[test]
    fn test_transform_rgb16() {
        let srgb_profile = ColorProfile::new_srgb();
        let bt2020_profile = ColorProfile::new_bt2020();
        let random_point_x = rand::rng().random_range(0..((1u32 << 16u32) - 1u32)) as u16;
        let transform = bt2020_profile
            .create_transform_16bit(
                Layout::Rgb,
                &srgb_profile,
                Layout::Rgb,
                TransformOptions::default(),
            )
            .unwrap();
        let src = vec![random_point_x; 256 * 256 * 3];
        let mut dst = vec![random_point_x; 256 * 256 * 3];
        transform.transform(&src, &mut dst).unwrap();
    }

    #[test]
    fn test_transform_round_trip_rgb8() {
        let srgb_profile = ColorProfile::new_srgb();
        let bt2020_profile = ColorProfile::new_bt2020();
        let transform = srgb_profile
            .create_transform_8bit(
                Layout::Rgb,
                &bt2020_profile,
                Layout::Rgb,
                TransformOptions::default(),
            )
            .unwrap();
        let mut src = vec![0u8; 256 * 256 * 3];
        for dst in src.chunks_exact_mut(3) {
            dst[0] = 175;
            dst[1] = 75;
            dst[2] = 13;
        }
        let mut dst = vec![0u8; 256 * 256 * 3];
        transform.transform(&src, &mut dst).unwrap();

        let transform_inverse = bt2020_profile
            .create_transform_8bit(
                Layout::Rgb,
                &srgb_profile,
                Layout::Rgb,
                TransformOptions::default(),
            )
            .unwrap();

        transform_inverse.transform(&dst, &mut src).unwrap();

        for src in src.chunks_exact_mut(3) {
            let diff0 = (src[0] as i32 - 175).abs();
            let diff1 = (src[1] as i32 - 75).abs();
            let diff2 = (src[2] as i32 - 13).abs();
            assert!(
                diff0 < 3,
                "On channel 0 difference should be less than 3, but it was {diff0}"
            );
            assert!(
                diff1 < 3,
                "On channel 1 difference should be less than 3, but it was {diff1}"
            );
            assert!(
                diff2 < 3,
                "On channel 2 difference should be less than 3, but it was {diff2}"
            );
        }
    }

    #[test]
    fn test_transform_round_trip_rgb10() {
        let srgb_profile = ColorProfile::new_srgb();
        let bt2020_profile = ColorProfile::new_bt2020();
        let transform = srgb_profile
            .create_transform_10bit(
                Layout::Rgb,
                &bt2020_profile,
                Layout::Rgb,
                TransformOptions::default(),
            )
            .unwrap();
        let mut src = vec![0u16; 256 * 256 * 3];
        for dst in src.chunks_exact_mut(3) {
            dst[0] = 175;
            dst[1] = 256;
            dst[2] = 512;
        }
        let mut dst = vec![0u16; 256 * 256 * 3];
        transform.transform(&src, &mut dst).unwrap();

        let transform_inverse = bt2020_profile
            .create_transform_10bit(
                Layout::Rgb,
                &srgb_profile,
                Layout::Rgb,
                TransformOptions::default(),
            )
            .unwrap();

        transform_inverse.transform(&dst, &mut src).unwrap();

        for src in src.chunks_exact_mut(3) {
            let diff0 = (src[0] as i32 - 175).abs();
            let diff1 = (src[1] as i32 - 256).abs();
            let diff2 = (src[2] as i32 - 512).abs();
            assert!(
                diff0 < 15,
                "On channel 0 difference should be less than 15, but it was {diff0}"
            );
            assert!(
                diff1 < 15,
                "On channel 1 difference should be less than 15, but it was {diff1}"
            );
            assert!(
                diff2 < 15,
                "On channel 2 difference should be less than 15, but it was {diff2}"
            );
        }
    }

    #[test]
    fn test_transform_round_trip_rgb12() {
        let srgb_profile = ColorProfile::new_srgb();
        let bt2020_profile = ColorProfile::new_bt2020();
        let transform = srgb_profile
            .create_transform_12bit(
                Layout::Rgb,
                &bt2020_profile,
                Layout::Rgb,
                TransformOptions::default(),
            )
            .unwrap();
        let mut src = vec![0u16; 256 * 256 * 3];
        for dst in src.chunks_exact_mut(3) {
            dst[0] = 1750;
            dst[1] = 2560;
            dst[2] = 3143;
        }
        let mut dst = vec![0u16; 256 * 256 * 3];
        transform.transform(&src, &mut dst).unwrap();

        let transform_inverse = bt2020_profile
            .create_transform_12bit(
                Layout::Rgb,
                &srgb_profile,
                Layout::Rgb,
                TransformOptions::default(),
            )
            .unwrap();

        transform_inverse.transform(&dst, &mut src).unwrap();

        for src in src.chunks_exact_mut(3) {
            let diff0 = (src[0] as i32 - 1750).abs();
            let diff1 = (src[1] as i32 - 2560).abs();
            let diff2 = (src[2] as i32 - 3143).abs();
            assert!(
                diff0 < 25,
                "On channel 0 difference should be less than 25, but it was {diff0}"
            );
            assert!(
                diff1 < 25,
                "On channel 1 difference should be less than 25, but it was {diff1}"
            );
            assert!(
                diff2 < 25,
                "On channel 2 difference should be less than 25, but it was {diff2}"
            );
        }
    }

    #[test]
    fn test_transform_round_trip_rgb16() {
        let srgb_profile = ColorProfile::new_srgb();
        let bt2020_profile = ColorProfile::new_bt2020();
        let transform = srgb_profile
            .create_transform_16bit(
                Layout::Rgb,
                &bt2020_profile,
                Layout::Rgb,
                TransformOptions::default(),
            )
            .unwrap();
        let mut src = vec![0u16; 256 * 256 * 3];
        for dst in src.chunks_exact_mut(3) {
            dst[0] = 1760;
            dst[1] = 2560;
            dst[2] = 5120;
        }
        let mut dst = vec![0u16; 256 * 256 * 3];
        transform.transform(&src, &mut dst).unwrap();

        let transform_inverse = bt2020_profile
            .create_transform_16bit(
                Layout::Rgb,
                &srgb_profile,
                Layout::Rgb,
                TransformOptions::default(),
            )
            .unwrap();

        transform_inverse.transform(&dst, &mut src).unwrap();

        for src in src.chunks_exact_mut(3) {
            let diff0 = (src[0] as i32 - 1760).abs();
            let diff1 = (src[1] as i32 - 2560).abs();
            let diff2 = (src[2] as i32 - 5120).abs();
            assert!(
                diff0 < 35,
                "On channel 0 difference should be less than 35, but it was {diff0}"
            );
            assert!(
                diff1 < 35,
                "On channel 1 difference should be less than 35, but it was {diff1}"
            );
            assert!(
                diff2 < 35,
                "On channel 2 difference should be less than 35, but it was {diff2}"
            );
        }
    }

    #[test]
    fn test_transform_rgb_to_gray_extended() {
        let srgb = ColorProfile::new_srgb();
        let mut gray_profile = ColorProfile::new_gray_with_gamma(1.0);
        gray_profile.color_space = DataColorSpace::Gray;
        gray_profile.gray_trc = srgb.red_trc.clone();
        let mut test_profile = vec![0.; 4];
        test_profile[2] = 1.;
        let mut dst = vec![0.; 1];

        let mut inverse = vec![0.; 4];

        let cvt0 = srgb
            .create_transform_f32(
                Layout::Rgba,
                &gray_profile,
                Layout::Gray,
                TransformOptions {
                    allow_extended_range_rgb_xyz: true,
                    ..Default::default()
                },
            )
            .unwrap();
        cvt0.transform(&test_profile, &mut dst).unwrap();
        assert!((dst[0] - 0.273046) < 1e-4);

        let cvt_inverse = gray_profile
            .create_transform_f32(
                Layout::Gray,
                &srgb,
                Layout::Rgba,
                TransformOptions {
                    allow_extended_range_rgb_xyz: false,
                    ..Default::default()
                },
            )
            .unwrap();
        cvt_inverse.transform(&dst, &mut inverse).unwrap();
        assert!((inverse[0] - 0.273002833) < 1e-4);

        let cvt1 = srgb
            .create_transform_f32(
                Layout::Rgba,
                &gray_profile,
                Layout::Gray,
                TransformOptions {
                    allow_extended_range_rgb_xyz: false,
                    ..Default::default()
                },
            )
            .unwrap();
        cvt1.transform(&test_profile, &mut dst).unwrap();
        assert!((dst[0] - 0.27307168) < 1e-5);

        inverse.fill(0.);

        let cvt_inverse = gray_profile
            .create_transform_f32(
                Layout::Gray,
                &srgb,
                Layout::Rgba,
                TransformOptions {
                    allow_extended_range_rgb_xyz: true,
                    ..Default::default()
                },
            )
            .unwrap();
        cvt_inverse.transform(&dst, &mut inverse).unwrap();
        assert!((inverse[0] - 0.273002833) < 1e-4);
    }
}
