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
mod a_curves3;
mod a_curves4x3;
mod cube;
mod hypercube;
mod interpolator;
mod interpolator_q0_15;
mod lut4_to_3;
mod lut4_to_3_q0_15;
mod preheat_lut4x3;
mod rgb_xyz;
mod rgb_xyz_opt;
mod rgb_xyz_q1_30_opt;
mod rgb_xyz_q2_13;
mod rgb_xyz_q2_13_opt;
mod t_lut3_to_3;
mod t_lut3_to_3_q0_15;

pub(crate) use a_curves3::{ACurves3InverseNeon, ACurves3Neon, ACurves3OptimizedNeon};
pub(crate) use a_curves4x3::{ACurves4x3Neon, ACurves4x3NeonOptimizedNeon};
pub(crate) use lut4_to_3::NeonLut4x3Factory;
pub(crate) use preheat_lut4x3::Lut4x3Neon;
pub(crate) use rgb_xyz::TransformShaperRgbNeon;
pub(crate) use rgb_xyz_opt::TransformShaperRgbOptNeon;
pub(crate) use rgb_xyz_q1_30_opt::TransformShaperQ1_30NeonOpt;
pub(crate) use rgb_xyz_q2_13::TransformShaperQ2_13Neon;
pub(crate) use rgb_xyz_q2_13_opt::TransformShaperQ2_13NeonOpt;
pub(crate) use t_lut3_to_3::NeonLut3x3Factory;
