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
use crate::common::{f_fmla, fmla, min_normal_f64};
use crate::double_double::DoubleDouble;
use crate::dyadic_float::{DyadicFloat128, DyadicSign};
use crate::logs::log_dyadic::{LOG_STEP_1, LOG_STEP_2, LOG_STEP_3, LOG_STEP_4};
use crate::logs::log_range_reduction::log_range_reduction;
use crate::logs::log_td::log_td;
use crate::logs::log2::LOG_RANGE_REDUCTION;
use crate::logs::log10::LOG_R_DD;
use crate::logs::{LOG_COEFFS, log_dd};
use crate::polyeval::f_polyeval4;

/// Assumes that NaN and infinities, negatives were filtered out
pub(crate) fn log_dyadic(x: f64) -> DyadicFloat128 {
    let mut x_u = x.to_bits();

    const E_BIAS: u64 = (1u64 << (11 - 1u64)) - 1u64;

    let mut x_e: i32 = -(E_BIAS as i32);

    const MAX_NORMAL: u64 = f64::to_bits(f64::MAX);

    if x_u == 1f64.to_bits() {
        // log2(1.0) = +0.0
        return DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: 0,
            mantissa: 0u128,
        };
    }
    if x_u < min_normal_f64().to_bits() || x_u > MAX_NORMAL {
        // Normalize denormal inputs.
        x_u = (x * f64::from_bits(0x4330000000000000)).to_bits();
        x_e -= 52;
    }

    // Range reduction for log2(x_m):
    // For each x_m, we would like to find r such that:
    //   -2^-8 <= r * x_m - 1 < 2^-7
    let shifted = (x_u >> 45) as i32;
    let index = shifted & 0x7F;
    let r = f64::from_bits(LOG_RANGE_REDUCTION[index as usize]);

    // Add unbiased exponent. Add an extra 1 if the 8 leading fractional bits are
    // all 1's.
    x_e = x_e.wrapping_add(x_u.wrapping_add(1u64 << 45).wrapping_shr(52) as i32);

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
        use crate::logs::log2::LOG_CD;
        let c_m = x_m & 0x3FFF_E000_0000_0000u64;
        let c = f64::from_bits(c_m);
        u = f_fmla(r, m - c, f64::from_bits(LOG_CD[index as usize])); // exact
    }
    log_accurate(x_e, index, u)
}

// Reuse the output of the fast pass range reduction.
// -2^-8 <= m_x < 2^-7
#[cold]
fn log_accurate(e_x: i32, index: i32, m_x: f64) -> DyadicFloat128 {
    // > P = fpminimax((log(1 + x) - x)/x^2, 2, [|1, 128...|],
    //                 [-0x1.0002143p-29 , 0x1p-29]);
    // > P;
    // > dirtyinfnorm(log(1 + x)/x - x*P, [-0x1.0002143p-29 , 0x1p-29]);
    // 0x1.99a3...p-121
    const BIG_COEFFS: [DyadicFloat128; 3] = [
        DyadicFloat128 {
            sign: DyadicSign::Neg,
            exponent: -129,
            mantissa: 0x8000_0000_0006_a710_b59c_58e5_554d_581c_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -129,
            mantissa: 0xaaaa_aaaa_aaaa_aabd_de05_c7c9_4ae9_cbae_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Neg,
            exponent: -128,
            mantissa: 0x8000_0000_0000_0000_0000_0000_0000_0000_u128,
        },
    ];

    const LOG_2: DyadicFloat128 = DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xb17217f7_d1cf79ab_c9e3b398_03f2f6af_u128,
    };

    let e_x_f128 = DyadicFloat128::new_from_f64(e_x as f64);
    let mut sum = LOG_2 * e_x_f128;
    sum = sum + LOG_STEP_1[index as usize];

    let (v_f128, mut sum) = log_range_reduction(
        m_x,
        &[&LOG_STEP_1, &LOG_STEP_2, &LOG_STEP_3, &LOG_STEP_4],
        sum,
    );

    sum = sum + v_f128;

    // Polynomial approximation
    let mut p = v_f128 * BIG_COEFFS[0];

    p = v_f128 * (p + BIG_COEFFS[1]);
    p = v_f128 * (p + BIG_COEFFS[2]);
    p = v_f128 * p;

    sum + p
}

/// Natural logarithm
///
/// Max found ULP 0.5
pub fn f_log(x: f64) -> f64 {
    let mut x_u = x.to_bits();

    const E_BIAS: u64 = (1u64 << (11 - 1u64)) - 1u64;

    let mut x_e: i32 = -(E_BIAS as i32);

    const MAX_NORMAL: u64 = f64::to_bits(f64::MAX);

    if x_u == 1f64.to_bits() {
        // log2(1.0) = +0.0
        return 0.0;
    }
    if x_u < min_normal_f64().to_bits() || x_u > MAX_NORMAL {
        if x == 0.0 {
            return f64::NEG_INFINITY;
        }
        if x < 0. || x.is_nan() {
            return f64::NAN;
        }
        if x.is_infinite() || x.is_nan() {
            return x + x;
        }
        // Normalize denormal inputs.
        x_u = (x * f64::from_bits(0x4330000000000000)).to_bits();
        x_e -= 52;
    }

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
        use crate::logs::log2::LOG_CD;
        let c_m = x_m & 0x3FFF_E000_0000_0000u64;
        let c = f64::from_bits(c_m);
        u = f_fmla(r, m - c, f64::from_bits(LOG_CD[index as usize])); // exact
    }

    let r1 = DoubleDouble::from_exact_add(hi, u);

    let u_sq = u * u;

    // Degree-7 minimax polynomial
    // Minimax polynomial for (log(1 + x) - x)/x^2, generated by sollya with:
    // > P = fpminimax((log(1 + x) - x)/x^2, 5, [|D...|], [-2^-8, 2^-7]);

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

    const HI_ERR: f64 = f64::from_bits(0x3aa0000000000000);

    // Extra errors from P is from using x^2 to reduce evaluation latency.
    const P_ERR: f64 = f64::from_bits(0x3cd0000000000000);

    // Technicallly error of r1.lo is bounded by:
    //    hi*ulp(log(2)_lo) + C*ulp(u^2)
    // To simplify the error computation a bit, we replace |hi|*ulp(log(2)_lo)
    // with the upper bound: 2^11 * ulp(log(2)_lo) = 2^-85.
    // Total error is bounded by ~ C * ulp(u^2) + 2^-85.
    let err = f_fmla(u_sq, P_ERR, HI_ERR);

    // Lower bound from the result
    let left = r1.hi + (p - err);
    // Upper bound from the result
    let right = r1.hi + (p + err);

    // Ziv's test if fast pass is accurate enough.
    if left == right {
        return left;
    }

    log_accurate_slow(x)
}

#[cold]
#[inline(never)]
fn log_accurate_slow(x: f64) -> f64 {
    let r = log_dd(x);
    let err = f_fmla(
        r.hi,
        f64::from_bits(0x3b50000000000000), // 2^-74
        f64::from_bits(0x3990000000000000), // 2^-102
    );
    let ub = r.hi + (r.lo + err);
    let lb = r.hi + (r.lo - err);
    if ub == lb {
        return r.to_f64();
    }
    log_accurate_slow_td(x)
}

#[cold]
#[inline(never)]
fn log_accurate_slow_td(x: f64) -> f64 {
    log_td(x).to_f64()
}

/// Log for given value for const context.
/// This is simplified version just to make a good approximation on const context.
#[inline]
pub const fn log(d: f64) -> f64 {
    const LN_POLY_2_D: f64 = 0.6666666666666762678e+0;
    const LN_POLY_3_D: f64 = 0.3999999999936908641e+0;
    const LN_POLY_4_D: f64 = 0.2857142874046159249e+0;
    const LN_POLY_5_D: f64 = 0.2222219947428228041e+0;
    const LN_POLY_6_D: f64 = 0.1818349302807168999e+0;
    const LN_POLY_7_D: f64 = 0.1531633000781658996e+0;
    const LN_POLY_8_D: f64 = 0.1476969208015536904e+0;

    let e = d.to_bits().wrapping_shr(52).wrapping_sub(0x3ff);
    if e >= 0x400 || e == 0x00000000fffffc01 {
        let minf = 0xfffu64 << 52;
        if e == 0x400 || (e == 0xc00 && d != f64::from_bits(minf)) {
            /* +Inf or NaN */
            return d + d;
        }
        if d <= 0. {
            return if d < 0. { f64::NAN } else { f64::NEG_INFINITY };
        }
    }

    // reduce into [sqrt(2)/2;sqrt(2)]
    let mut ui: u64 = d.to_bits();
    let mut hx = (ui >> 32) as u32;
    hx = hx.wrapping_add(0x3ff00000 - 0x3fe6a09e);
    let n = (hx >> 20) as i32 - 0x3ff;
    hx = (hx & 0x000fffff).wrapping_add(0x3fe6a09e);
    ui = (hx as u64) << 32 | (ui & 0xffffffff);
    let a = f64::from_bits(ui);

    let m = a - 1.;

    let x = m / (a + 1.);
    let x2 = x * x;
    let f = x2;

    const LN2_H: f64 = 0.6931471805599453;
    const LN2_L: f64 = 2.3190468138462996e-17;

    let mut u = LN_POLY_8_D;
    u = fmla(u, f, LN_POLY_7_D);
    u = fmla(u, f, LN_POLY_6_D);
    u = fmla(u, f, LN_POLY_5_D);
    u = fmla(u, f, LN_POLY_4_D);
    u = fmla(u, f, LN_POLY_3_D);
    u = fmla(u, f, LN_POLY_2_D);
    u *= f;

    let t = m * m * 0.5;
    let r = fmla(x, t, fmla(x, u, LN2_L * n as f64)) - t + m;
    fmla(LN2_H, n as f64, r)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_test() {
        assert!(
            (log(1f64) - 0f64).abs() < 1e-8,
            "Invalid result {}",
            log(1f64)
        );
        assert!(
            (log(5f64) - 1.60943791243410037460f64).abs() < 1e-8,
            "Invalid result {}",
            log(5f64)
        );
    }

    #[test]
    fn f_log_test() {
        assert_eq!(f_log(1.99999999779061), 0.693147179455250308807056);
        assert_eq!(f_log(0.9999999999999999), -1.1102230246251565e-16);
        assert!(
            (f_log(1f64) - 0f64).abs() < 1e-8,
            "Invalid result {}",
            f_log(1f64)
        );
        assert!(
            (f_log(5f64) - 5f64.ln()).abs() < 1e-8,
            "Invalid result {}, expected {}",
            f_log(5f64),
            5f64.ln()
        );
        assert_eq!(
            f_log(23f64),
            3.13549421592914969080675283181019611844238031484043574199863537748299324598,
            "Invalid result {}, expected {}",
            f_log(23f64),
            3.13549421592914969080675283181019611844238031484043574199863537748299324598,
        );
        assert_eq!(f_log(0.), f64::NEG_INFINITY);
        assert!(f_log(-1.).is_nan());
        assert!(f_log(f64::NAN).is_nan());
        assert!(f_log(f64::NEG_INFINITY).is_nan());
        assert_eq!(f_log(f64::INFINITY), f64::INFINITY);
    }

    #[test]
    fn log_control_values() {
        assert_eq!(
            f_log(f64::from_bits(0x3ff1211bef8f68e9)),
            0.06820362355801819
        );
        assert_eq!(
            f_log(f64::from_bits(0x3ff008000db2e8be)),
            0.0019512710640270448
        );
        assert_eq!(
            f_log(f64::from_bits(0x3ff10803885617a6)),
            0.062464334544603616
        );
        assert_eq!(
            f_log(f64::from_bits(0x3ff48ae5a67204f5)),
            0.24991043470757288
        );
        assert_eq!(
            f_log(f64::from_bits(0x3fedc0b586f2b260)),
            -0.07281366978582131
        );
        assert_eq!(
            f_log(f64::from_bits(0x3fe490af72a25a81)),
            -0.44213668842326787
        );
        assert_eq!(
            f_log(f64::from_bits(0x4015b6e7e4e96f86)),
            1.6916847703128641
        );
        assert_eq!(
            f_log(f64::from_bits(0x3ff0ffc349469a2f)),
            0.06057012512237759
        );
        assert_eq!(
            f_log(f64::from_bits(0x3fe69e7aa6da2df5)),
            -0.3469430325599064
        );
        assert_eq!(
            f_log(f64::from_bits(0x3fe5556123e8a2b0)),
            -0.4054566631657151
        );
    }
}
