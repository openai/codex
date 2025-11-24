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
use crate::common::f_fmla;
use crate::logs::{LOG_R_DD, LOG_RANGE_REDUCTION};
use crate::polyeval::{f_estrin_polyeval8, f_polyeval6};

#[cold]
pub(crate) fn special_logf(x: f32) -> f32 {
    let t = x.to_bits();
    if t == 0xbf800000u32 {
        // +0.0
        return f32::NEG_INFINITY;
    }
    if t == 0x7f800000u32 {
        return x;
    } // +inf
    let ax: u32 = t.wrapping_shl(1);
    if ax > 0xff000000u32 {
        return x + x;
    } // nan
    f32::NAN
}

#[inline]
pub(crate) fn core_logf(x: f64) -> f64 {
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

    let r1 = hi;
    // Polynomial for log(1+x)/x generated in Sollya:
    // d = [-2^-8, 2^-7];
    // f_log = log(1 + x)/x;
    // Q = fpminimax(f_log, 5, [|D...|], d);
    // See ./notes/log1pf_core.sollya
    let p = f_polyeval6(
        u,
        f64::from_bits(0x3fefffffffffffff),
        f64::from_bits(0xbfdffffffffff3e6),
        f64::from_bits(0x3fd5555555626b74),
        f64::from_bits(0xbfd0000026aeecc8),
        f64::from_bits(0x3fc9999114d16c06),
        f64::from_bits(0xbfc51e433a85278a),
    );
    f_fmla(p, u, r1) + lo
}

/// Computes log(x+1)
///
/// Max ULP 0.5
#[inline]
pub fn f_log1pf(x: f32) -> f32 {
    let ax = x.to_bits() & 0x7fff_ffffu32;
    let xd = x as f64;

    if !x.is_normal() {
        if x.is_nan() {
            return x + x;
        }
        if x.is_infinite() {
            return if x.is_sign_positive() {
                f32::INFINITY
            } else {
                f32::NAN
            };
        }
        if x == 0. {
            return x;
        }
    }

    // Use log1p(x) = log(1 + x) for |x| > 2^-6;
    if ax > 0x3c80_0000u32 {
        if x == -1. {
            return f32::NEG_INFINITY;
        }
        let x1p = xd + 1.;
        if x1p <= 0. {
            if x1p == 0. {
                return f32::NEG_INFINITY;
            }
            return f32::NAN;
        }
        return core_logf(x1p) as f32;
    }

    // log(1+x) is expected to be used near zero
    // Polynomial generated by Sollya:
    // d = [-2^-6; 2^-6];
    // f_log1pf = log(1+x)/x;
    // Q = fpminimax(f_log1pf, 7, [|0, D...|], d);
    // See ./notes/log1pf.sollya

    let p = f_estrin_polyeval8(
        xd,
        f64::from_bits(0x3ff0000000000000),
        f64::from_bits(0xbfe0000000000000),
        f64::from_bits(0x3fd5555555556aad),
        f64::from_bits(0xbfd000000000181a),
        f64::from_bits(0x3fc999998998124e),
        f64::from_bits(0xbfc55555452e2a2b),
        f64::from_bits(0x3fc24adb8cde4aa7),
        f64::from_bits(0xbfc0019db915ef6f),
    ) * xd;
    p as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log1pf_works() {
        assert!(f_log1pf(f32::from_bits(0xffefb9a7)).is_nan());
        assert!(f_log1pf(f32::NAN).is_nan());
        assert_eq!(f_log1pf(f32::from_bits(0x41078feb)), 2.2484074);
        assert_eq!(f_log1pf(-0.0000014305108), -0.0000014305118);
        assert_eq!(f_log1pf(0.0), 0.0);
        assert_eq!(f_log1pf(2.0), 1.0986123);
        assert_eq!(f_log1pf(-0.7), -1.2039728);
        assert_eq!(f_log1pf(-0.0000000000043243), -4.3243e-12);
    }
}
