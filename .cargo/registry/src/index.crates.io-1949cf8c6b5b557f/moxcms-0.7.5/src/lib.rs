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
#![allow(clippy::manual_clamp, clippy::excessive_precision)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![deny(unreachable_pub)]
#![deny(
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::print_literal,
    clippy::print_in_format_impl
)]
#![allow(stable_features)]
#![cfg_attr(
    not(any(feature = "avx", feature = "sse", feature = "avx512", feature = "neon")),
    forbid(unsafe_code)
)]
#![cfg_attr(all(feature = "avx512", target_arch = "x86_64"), feature(cfg_version))]
#![cfg_attr(
    all(feature = "avx512", target_arch = "x86_64"),
    feature(avx512_target_feature)
)]
#![cfg_attr(
    all(feature = "avx512", target_arch = "x86_64"),
    feature(stdarch_x86_avx512)
)]
mod chad;
mod cicp;
mod conversions;
mod dat;
mod defaults;
mod err;
mod gamma;
mod gamut;
mod ictcp;
mod jzazbz;
mod jzczhz;
mod lab;
mod luv;
/// One of main intent is to provide fast math available in const context
/// ULP most of the methods <= 0.5
mod math;
mod matrix;
mod mlaf;
mod nd_array;
mod oklab;
mod oklch;
mod profile;
mod reader;
mod rgb;
mod safe_math;
mod tag;
mod transform;
mod trc;
mod writer;
mod yrg;
// Simple math analysis module
mod chromaticity;
mod dt_ucs;
mod helpers;
mod lut_hint;
mod matan;
mod srlab2;
mod xyy;

pub use chad::{
    adapt_to_d50, adapt_to_d50_d, adapt_to_illuminant, adapt_to_illuminant_d,
    adapt_to_illuminant_xyz, adapt_to_illuminant_xyz_d, adaption_matrix, adaption_matrix_d,
};
pub use chromaticity::Chromaticity;
pub use cicp::{CicpColorPrimaries, ColorPrimaries, MatrixCoefficients, TransferCharacteristics};
pub use dat::ColorDateTime;
pub use defaults::{
    HLG_LUT_TABLE, PQ_LUT_TABLE, WHITE_POINT_D50, WHITE_POINT_D60, WHITE_POINT_D65,
    WHITE_POINT_DCI_P3,
};
pub use dt_ucs::{DtUchHcb, DtUchHsb, DtUchJch};
pub use err::{CmsError, MalformedSize};
pub use gamut::filmlike_clip;
pub use ictcp::ICtCp;
pub use jzazbz::Jzazbz;
pub use jzczhz::Jzczhz;
pub use lab::Lab;
pub use luv::{LCh, Luv};
pub use math::rounding_div_ceil;
pub use matrix::{
    BT2020_MATRIX, DISPLAY_P3_MATRIX, Matrix3, Matrix3d, Matrix3f, Matrix4f, SRGB_MATRIX, Vector3,
    Vector3d, Vector3f, Vector3i, Vector3u, Vector4, Vector4d, Vector4f, Vector4i, Xyz, Xyzd,
};
pub use nd_array::{Cube, Hypercube};
pub use oklab::Oklab;
pub use oklch::Oklch;
pub use profile::{
    CicpProfile, ColorProfile, DataColorSpace, DescriptionString, LocalizableString, LutDataType,
    LutMultidimensionalType, LutStore, LutType, LutWarehouse, Measurement, MeasurementGeometry,
    ParsingOptions, ProfileClass, ProfileSignature, ProfileText, ProfileVersion, RenderingIntent,
    StandardIlluminant, StandardObserver, TechnologySignatures, ViewingConditions,
};
pub use rgb::{FusedExp, FusedExp2, FusedExp10, FusedLog, FusedLog2, FusedLog10, FusedPow, Rgb};
pub use srlab2::Srlab2;
pub use transform::{
    BarycentricWeightScale, InPlaceStage, InterpolationMethod, Layout, PointeeSizeExpressible,
    Stage, Transform8BitExecutor, Transform16BitExecutor, TransformExecutor,
    TransformF32BitExecutor, TransformF64BitExecutor, TransformOptions,
};
pub use trc::{GammaLutInterpolate, ToneCurveEvaluator, ToneReprCurve, curve_from_gamma};
pub use xyy::{XyY, XyYRepresentable};
pub use yrg::{Ych, Yrg, cie_y_1931_to_cie_y_2006};
