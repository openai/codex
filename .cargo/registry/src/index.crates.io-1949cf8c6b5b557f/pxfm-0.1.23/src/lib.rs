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
// #![forbid(unsafe_code)]
#![deny(unreachable_pub)]
#![allow(
    clippy::excessive_precision,
    clippy::approx_constant,
    clippy::manual_range_contains
)]
#![deny(
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::print_literal,
    clippy::print_in_format_impl
)]
mod acos;
mod acosf;
mod acospi;
mod acospif;
mod asin;
mod asin_eval_dyadic;
mod asinf;
mod asinpi;
mod asinpif;
mod bessel;
mod bits;
mod ceil;
mod common;
mod compound;
mod cosm1;
mod csc;
mod cube_roots;
mod double_double;
mod dyadic_float;
mod dyadic_float256;
mod err;
mod exponents;
mod floor;
mod gamma;
mod hyperbolic;
mod logs;
mod polyeval;
mod pow;
mod pow_exec;
mod pow_tables;
mod powf;
mod powf_tables;
mod round;
mod round_ties_even;
mod sec;
mod shared_eval;
mod sin;
mod sin_cosf;
mod sin_helper;
mod sin_table;
mod sinc;
mod sincos;
mod sincos_dyadic;
mod sincos_reduce;
mod sincos_reduce_tables;
mod sincospi;
mod sincospi_tables;
mod sincpi;
mod sinmx;
mod square_root;
mod tangent;
mod triangle;
mod triple_double;
mod trunc;
mod u256;

pub use acos::f_acos;
pub use acosf::f_acosf;
pub use acospi::f_acospi;
pub use acospif::f_acospif;
pub use asin::f_asin;
pub use asinf::f_asinf;
pub use asinpi::f_asinpi;
pub use asinpif::f_asinpif;
pub use bessel::{
    f_i0, f_i0f, f_i1, f_i1f, f_i2, f_i2f, f_j0, f_j0f, f_j1, f_j1f, f_jincpi, f_jincpif, f_k0,
    f_k0f, f_k1, f_k1f, f_k2f, f_y0, f_y0f, f_y1, f_y1f,
};
pub use ceil::{ceil, ceilf};
pub use common::{copysignfk, copysignk};
pub use compound::{f_compound, f_compound_m1, f_compound_m1f, f_compoundf, f_powm1, f_powm1f};
pub use cosm1::f_cosm1;
pub use csc::f_csc;
pub use cube_roots::{cbrtf, f_cbrt, f_cbrtf, f_rcbrt, f_rcbrtf};
pub use err::{
    f_erf, f_erfc, f_erfcf, f_erfcinv, f_erfcinvf, f_erfcx, f_erfcxf, f_erff, f_erfinv, f_erfinvf,
    f_rerf, f_rerff,
};
pub use exponents::{
    exp, expf, f_exp, f_exp2, f_exp2f, f_exp2m1, f_exp2m1f, f_exp10, f_exp10f, f_exp10m1,
    f_exp10m1f, f_expf, f_expm1, f_expm1f,
};
pub use floor::{floor, floorf};
pub use gamma::{
    f_digamma, f_digammaf, f_lgamma, f_lgammaf, f_tgamma, f_tgammaf, f_trigamma, f_trigammaf,
};
pub use hyperbolic::{
    f_acosh, f_acoshf, f_asinh, f_asinhf, f_atanh, f_atanhf, f_cosh, f_coshf, f_sinh, f_sinhf,
    f_tanh, f_tanhf,
};
pub use logs::{
    f_log, f_log1p, f_log1pf, f_log1pmx, f_log1pmxf, f_log2, f_log2f, f_log2p1, f_log2p1f, f_log10,
    f_log10f, f_log10p1, f_log10p1f, f_logf, log, logf,
};
pub use pow::{f_pow, pow};
pub use powf::{dirty_powf, f_powf, powf};
pub use round::{round, roundf};
pub use round_ties_even::{round_ties_even, roundf_ties_even};
pub use sec::f_sec;
pub use sin::{f_cos, f_sin};
pub use sin_cosf::{
    f_cosf, f_cosm1f, f_cospif, f_cscf, f_secf, f_sincf, f_sincosf, f_sincospif, f_sincpif, f_sinf,
    f_sinmxf, f_sinpif,
};
pub use sinc::f_sinc;
pub use sincos::f_sincos;
pub use sincospi::{f_cospi, f_sincospi, f_sinpi};
pub use sincpi::f_sincpi;
pub use sinmx::f_sinmx;
pub use square_root::{f_rsqrt, f_rsqrtf, sqrtf};
pub use tangent::{
    f_atan, f_atan2, f_atan2f, f_atan2pi, f_atan2pif, f_atanf, f_atanpi, f_atanpif, f_cot, f_cotf,
    f_cotpi, f_cotpif, f_tan, f_tanf, f_tanpi, f_tanpif,
};
pub use triangle::{f_cathetus, f_cathetusf, f_hypot, f_hypot3f, f_hypotf};
pub use trunc::{trunc, truncf};
