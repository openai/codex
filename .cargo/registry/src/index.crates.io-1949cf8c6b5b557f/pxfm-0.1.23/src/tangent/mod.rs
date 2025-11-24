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
mod atan;
mod atan2;
mod atan2f;
mod atan2pi;
mod atan2pif;
mod atanf;
mod atanpi;
mod atanpif;
mod cot;
mod cotf;
mod cotpi;
mod cotpif;
mod tan;
mod tanf;
mod tanpi;
mod tanpif;

pub use atan::f_atan;
pub use atan2::f_atan2;
pub use atan2f::f_atan2f;
pub use atan2pi::f_atan2pi;
pub use atan2pif::f_atan2pif;
pub use atanf::f_atanf;
pub use atanpi::f_atanpi;
pub use atanpif::f_atanpif;
pub use cot::f_cot;
pub use cotf::f_cotf;
pub(crate) use cotpi::cotpi_core;
pub use cotpi::f_cotpi;
pub(crate) use cotpif::cotpif_core;
pub use cotpif::f_cotpif;
pub use tan::f_tan;
pub use tanf::f_tanf;
pub use tanpi::f_tanpi;
pub use tanpif::f_tanpif;
