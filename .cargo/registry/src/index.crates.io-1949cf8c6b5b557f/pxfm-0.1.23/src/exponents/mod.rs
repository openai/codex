/*
 * // Copyright (c) Radzivon Bartoshyk 7/2025. All rights reserved.
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
#![deny(unreachable_pub)]
mod auxiliary;
mod exp;
mod exp10;
mod exp10f;
mod exp10m1;
mod exp10m1f;
mod exp2;
mod exp2f;
mod exp2m1;
mod exp2m1f;
mod exp_f128;
mod expf;
mod expm1;
mod expm1f;

pub(crate) use auxiliary::{fast_ldexp, ldexp};
pub(crate) use exp::{EXP_REDUCE_T0, EXP_REDUCE_T1};
pub use exp::{exp, f_exp};
pub(crate) use exp_f128::rational128_exp;
pub use exp2::f_exp2;
pub(crate) use exp2f::dirty_exp2f;
pub use exp2f::f_exp2f;
pub(crate) use exp2m1::exp2m1_accurate_tiny;
pub use exp2m1::f_exp2m1;
pub use exp2m1f::f_exp2m1f;
pub use exp10::f_exp10;
pub use exp10f::f_exp10f;
pub use exp10m1::f_exp10m1;
pub use exp10m1f::f_exp10m1f;
pub(crate) use expf::{core_expdf, core_expf};
pub use expf::{expf, f_expf};
pub use expm1::f_expm1;
pub(crate) use expm1::{EXPM1_T0, EXPM1_T1};
pub use expm1f::f_expm1f;
