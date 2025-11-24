/*
 * // Copyright (c) Radzivon Bartoshyk 8/2025. All rights reserved.
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
mod argument_reduction;
mod argument_reduction_pi;
mod cosf;
mod cosm1f;
mod cospif;
mod cscf;
mod secf;
mod sincf;
mod sincosf;
mod sincosf_eval;
mod sincospif;
mod sincpif;
mod sinf;
mod sinmxf;
mod sinpif;

pub(crate) use argument_reduction_pi::ArgumentReducerPi;
pub use cosf::f_cosf;
pub use cosm1f::f_cosm1f;
pub use cospif::f_cospif;
pub use cscf::f_cscf;
pub use secf::f_secf;
pub use sincf::f_sincf;
pub use sincosf::f_sincosf;
pub(crate) use sincosf_eval::{tanf_eval, tanpif_eval};
pub use sincospif::f_sincospif;
pub use sincpif::f_sincpif;
pub use sinf::f_sinf;
pub use sinmxf::f_sinmxf;
pub use sinpif::f_sinpif;
pub(crate) use sinpif::fast_sinpif;
