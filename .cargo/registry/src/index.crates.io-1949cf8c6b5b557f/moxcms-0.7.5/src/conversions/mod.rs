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
#[cfg(all(target_arch = "x86_64", feature = "avx"))]
mod avx;
#[cfg(all(target_arch = "x86_64", feature = "avx512"))]
mod avx512;
mod bpc;
mod gray2rgb;
mod gray2rgb_extended;
mod interpolator;
mod katana;
mod lut3x3;
mod lut3x4;
mod lut4;
mod lut_transforms;
mod mab;
mod mab4x3;
mod mba3x4;
mod md_lut;
mod md_luts_factory;
#[cfg(all(target_arch = "aarch64", target_feature = "neon", feature = "neon"))]
mod neon;
mod prelude_lut_xyz_rgb;
mod rgb2gray;
mod rgb2gray_extended;
mod rgb_xyz_factory;
mod rgbxyz;
mod rgbxyz_fixed;
mod rgbxyz_float;
#[cfg(all(any(target_arch = "x86", target_arch = "x86_64"), feature = "sse"))]
mod sse;
mod transform_lut3_to_3;
mod transform_lut3_to_4;
mod transform_lut4_to_3;
mod xyz_lab;

pub(crate) use gray2rgb::{make_gray_to_unfused, make_gray_to_x};
pub(crate) use gray2rgb_extended::{make_gray_to_one_trc_extended, make_gray_to_rgb_extended};
pub(crate) use interpolator::LutBarycentricReduction;
pub(crate) use lut_transforms::make_lut_transform;
pub(crate) use rgb_xyz_factory::{RgbXyzFactory, RgbXyzFactoryOpt};
pub(crate) use rgb2gray::{ToneReproductionRgbToGray, make_rgb_to_gray};
pub(crate) use rgb2gray_extended::make_rgb_to_gray_extended;
pub(crate) use rgbxyz::{TransformMatrixShaper, TransformMatrixShaperOptimized};
pub(crate) use rgbxyz_float::{
    TransformShaperFloatInOut, TransformShaperRgbFloat, make_rgb_xyz_rgb_transform_float,
    make_rgb_xyz_rgb_transform_float_in_out,
};
