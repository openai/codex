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
use crate::common::f_fmla;
use crate::double_double::DoubleDouble;
use crate::logs::{LOG_COEFFS, LOG_R_DD, LOG_RANGE_REDUCTION};
use crate::polyeval::f_polyeval4;

#[inline]
pub(crate) fn simple_fast_log(x: f64) -> f64 {
    let x_u = x.to_bits();

    const E_BIAS: u64 = (1u64 << (11 - 1u64)) - 1u64;

    let mut x_e: i32 = -(E_BIAS as i32);

    // log2(x) = log2(2^x_e * x_m)
    //         = x_e + log2(x_m)
    // Range reduction for log2(x_m):
    // For each x_m, we would like to find r such that:
    //   -2^-8 <= r * x_m - 1 < 2^-7
    let shifted = (x_u >> 45) as i32;
    let index = shifted & 0x7F;
    let r = f64::from_bits(LOG_RANGE_REDUCTION[index as usize]);

    // Add unbiased exponent. Add an extra 1 if the 8 leading fractional bits are
    // all 1's.
    x_e = x_e.wrapping_add(x_u.wrapping_add(1u64 << 45).wrapping_shr(52) as i32);
    let e_x = x_e as f64;

    const LOG_2_HI: f64 = f64::from_bits(0x3fe62e42fefa3800);
    const LOG_2_LO: f64 = f64::from_bits(0x3d2ef35793c76730);

    let log_r_dd = LOG_R_DD[index as usize];

    // hi is exact
    let hi = f_fmla(e_x, LOG_2_HI, f64::from_bits(log_r_dd.1));
    // lo errors ~ e_x * LSB(LOG_2_LO) + LSB(LOG_R[index].lo) + rounding err
    //           <= 2 * (e_x * LSB(LOG_2_LO) + LSB(LOG_R[index].lo))
    let lo = f_fmla(e_x, LOG_2_LO, f64::from_bits(log_r_dd.0));

    // Set m = 1.mantissa.
    let x_m = (x_u & 0x000F_FFFF_FFFF_FFFFu64) | 0x3FF0_0000_0000_0000u64;
    let m = f64::from_bits(x_m);

    let u;
    #[cfg(any(
        all(
            any(target_arch = "x86", target_arch = "x86_64"),
            target_feature = "fma"
        ),
        all(target_arch = "aarch64", target_feature = "neon")
    ))]
    {
        u = f_fmla(r, m, -1.0); // exact
    }
    #[cfg(not(any(
        all(
            any(target_arch = "x86", target_arch = "x86_64"),
            target_feature = "fma"
        ),
        all(target_arch = "aarch64", target_feature = "neon")
    )))]
    {
        use crate::logs::LOG_CD;
        let c_m = x_m & 0x3FFF_E000_0000_0000u64;
        let c = f64::from_bits(c_m);
        u = f_fmla(r, m - c, f64::from_bits(LOG_CD[index as usize])); // exact
    }

    let r1 = DoubleDouble::from_exact_add(hi, u);

    let u_sq = u * u;
    // Degree-7 minimax polynomial
    let p0 = f_fmla(
        u,
        f64::from_bits(LOG_COEFFS[1]),
        f64::from_bits(LOG_COEFFS[0]),
    );
    let p1 = f_fmla(
        u,
        f64::from_bits(LOG_COEFFS[3]),
        f64::from_bits(LOG_COEFFS[2]),
    );
    let p2 = f_fmla(
        u,
        f64::from_bits(LOG_COEFFS[5]),
        f64::from_bits(LOG_COEFFS[4]),
    );
    let p = f_polyeval4(u_sq, lo + r1.lo, p0, p1, p2);
    r1.hi + p
}
