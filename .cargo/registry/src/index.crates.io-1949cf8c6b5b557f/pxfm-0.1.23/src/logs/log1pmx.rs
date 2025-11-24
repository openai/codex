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
use crate::bits::{EXP_MASK, get_exponent_f64};
use crate::common::f_fmla;
use crate::double_double::DoubleDouble;
use crate::logs::log1p::{LOG_R1_DD, R1};
use crate::logs::log1p_dd;
use crate::polyeval::{f_estrin_polyeval8, f_polyeval4, f_polyeval5};

#[cold]
fn tiny_hard(x: f64) -> f64 {
    // d = [-2^-10; 2^-10];
    // f_log1pf = (log(1+x) - x)/x^2;
    // Q = fpminimax(f_log1pf, 7, [|0, 107...|], d);
    // See ./notes/log1pmx_at_zero_hard.sollya

    let x2 = DoubleDouble::from_exact_mult(x, x);

    const C: [(u64, u64); 7] = [
        (0x3c755555555129de, 0x3fd5555555555555),
        (0xbbd333352fe28400, 0xbfd0000000000000),
        (0xbc698cb3ef1ea2dd, 0x3fc999999999999a),
        (0x3c4269700b3f95d0, 0xbfc55555555546ef),
        (0x3c61290e9261823e, 0x3fc2492492491565),
        (0x3c6fb0a243c2a59c, 0xbfc000018af8cb7e),
        (0x3c3b271ceb5c60a0, 0x3fbc71ca10b30518),
    ];

    let mut r = DoubleDouble::mul_f64_add(
        DoubleDouble::from_bit_pair(C[6]),
        x,
        DoubleDouble::from_bit_pair(C[5]),
    );
    r = DoubleDouble::mul_f64_add(r, x, DoubleDouble::from_bit_pair(C[4]));
    r = DoubleDouble::mul_f64_add(r, x, DoubleDouble::from_bit_pair(C[3]));
    r = DoubleDouble::mul_f64_add(r, x, DoubleDouble::from_bit_pair(C[2]));
    r = DoubleDouble::mul_f64_add(r, x, DoubleDouble::from_bit_pair(C[1]));
    r = DoubleDouble::mul_f64_add(r, x, DoubleDouble::from_bit_pair(C[0]));
    r = DoubleDouble::mul_f64_add_f64(r, x, f64::from_bits(0xbfe0000000000000));
    r = DoubleDouble::quick_mult(r, x2);
    r.to_f64()
}

fn log1pmx_big(x: f64) -> f64 {
    let mut x_u = x.to_bits();

    let mut x_dd = DoubleDouble::default();

    let x_exp: u16 = ((x_u >> 52) & 0x7ff) as u16;

    const EXP_BIAS: u16 = (1u16 << (11 - 1u16)) - 1u16;

    if x_exp >= EXP_BIAS {
        // |x| >= 1
        if x_u >= 0x4650_0000_0000_0000u64 {
            x_dd.hi = x;
        } else {
            x_dd = DoubleDouble::from_exact_add(x, 1.0);
        }
    } else {
        // |x| < 1
        x_dd = DoubleDouble::from_exact_add(1.0, x);
    }

    // At this point, x_dd is the exact sum of 1 + x:
    //   x_dd.hi + x_dd.lo = x + 1.0 exactly.
    //   |x_dd.hi| >= 2^-54
    //   |x_dd.lo| < ulp(x_dd.hi)

    let xhi_bits = x_dd.hi.to_bits();
    let xhi_frac = xhi_bits & ((1u64 << 52) - 1);
    x_u = xhi_bits;
    // Range reduction:
    // Find k such that |x_hi - k * 2^-7| <= 2^-8.
    let idx: i32 = ((xhi_frac.wrapping_add(1u64 << (52 - 8))) >> (52 - 7)) as i32;
    let x_e = (get_exponent_f64(f64::from_bits(xhi_bits)) as i32).wrapping_add(idx >> 7);
    let e_x = x_e as f64;

    const LOG_2: DoubleDouble = DoubleDouble::new(
        f64::from_bits(0x3d2ef35793c76730),
        f64::from_bits(0x3fe62e42fefa3800),
    );

    // hi is exact
    // ulp(hi) = ulp(LOG_2_HI) = ulp(LOG_R1_DD[idx].hi) = 2^-43

    let r_dd = LOG_R1_DD[idx as usize];

    let hi = f_fmla(e_x, LOG_2.hi, f64::from_bits(r_dd.1));
    // lo errors < |e_x| * ulp(LOG_2_LO) + ulp(LOG_R1[idx].lo)
    //           <= 2^11 * 2^(-43-53) = 2^-85
    let lo = f_fmla(e_x, LOG_2.lo, f64::from_bits(r_dd.0));

    // Scale x_dd by 2^(-xh_bits.get_exponent()).
    let s_u: i64 = (x_u & EXP_MASK) as i64 - (EXP_BIAS as i64).wrapping_shl(52);
    // Normalize arguments:
    //   1 <= m_dd.hi < 2
    //   |m_dd.lo| < 2^-52.
    // This is exact.
    let m_hi = 1f64.to_bits() | xhi_frac;

    let m_lo = if x_dd.lo.abs() > x_dd.hi * f64::from_bits(0x3800000000000000) {
        (x_dd.lo.to_bits() as i64).wrapping_sub(s_u)
    } else {
        0
    };

    let m_dd = DoubleDouble::new(f64::from_bits(m_lo as u64), f64::from_bits(m_hi));

    // Perform range reduction:
    //   r * m - 1 = r * (m_dd.hi + m_dd.lo) - 1
    //             = (r * m_dd.hi - 1) + r * m_dd.lo
    //             = v_hi + (v_lo.hi + v_lo.lo)
    // where:
    //   v_hi = r * m_dd.hi - 1          (exact)
    //   v_lo.hi + v_lo.lo = r * m_dd.lo (exact)
    // Bounds on the values:
    //   -0x1.69000000000edp-8 < r * m - 1 < 0x1.7f00000000081p-8
    //   |v_lo.hi| <= |r| * |m_dd.lo| < 2^-52
    //   |v_lo.lo| < ulp(v_lo.hi) <= 2^(-52 - 53) = 2^(-105)
    let r = R1[idx as usize];
    let v_hi;
    let v_lo = DoubleDouble::from_exact_mult(m_dd.lo, f64::from_bits(r));

    #[cfg(any(
        all(
            any(target_arch = "x86", target_arch = "x86_64"),
            target_feature = "fma"
        ),
        all(target_arch = "aarch64", target_feature = "neon")
    ))]
    {
        v_hi = f_fmla(f64::from_bits(r), m_dd.hi, -1.0); // Exact.
    }

    #[cfg(not(any(
        all(
            any(target_arch = "x86", target_arch = "x86_64"),
            target_feature = "fma"
        ),
        all(target_arch = "aarch64", target_feature = "neon")
    )))]
    {
        use crate::logs::log1p::RCM1;
        let c = f64::from_bits(
            (idx as u64)
                .wrapping_shl(52 - 7)
                .wrapping_add(0x3FF0_0000_0000_0000u64),
        );
        v_hi = f_fmla(
            f64::from_bits(r),
            m_dd.hi - c,
            f64::from_bits(RCM1[idx as usize]),
        ); // Exact
    }

    // Range reduction output:
    //   -0x1.69000000000edp-8 < v_hi + v_lo < 0x1.7f00000000081p-8
    //   |v_dd.lo| < ulp(v_dd.hi) <= 2^(-7 - 53) = 2^-60
    let mut v_dd = DoubleDouble::from_exact_add(v_hi, v_lo.hi);
    v_dd.lo += v_lo.lo;

    // Exact sum:
    //   r1.hi + r1.lo = e_x * log(2)_hi - log(r)_hi + u
    let mut r1 = DoubleDouble::from_exact_add(hi, v_dd.hi);

    // Degree-7 minimax polynomial log(1 + v) ~ v - v^2 / 2 + ...
    // generated by Sollya with:
    // > P = fpminimax(log(1 + x)/x, 6, [|1, 1, D...|],
    //                 [-0x1.69000000000edp-8, 0x1.7f00000000081p-8]);
    const P_COEFFS: [u64; 6] = [
        0xbfe0000000000000,
        0x3fd5555555555166,
        0xbfcfffffffdb7746,
        0x3fc99999a8718a60,
        0xbfc555874ce8ce22,
        0x3fc24335555ddbe5,
    ];

    //   C * ulp(v_sq) + err_hi
    let v_sq = v_dd.hi * v_dd.hi;
    let p0 = f_fmla(
        v_dd.hi,
        f64::from_bits(P_COEFFS[1]),
        f64::from_bits(P_COEFFS[0]),
    );
    let p1 = f_fmla(
        v_dd.hi,
        f64::from_bits(P_COEFFS[3]),
        f64::from_bits(P_COEFFS[2]),
    );
    let p2 = f_fmla(
        v_dd.hi,
        f64::from_bits(P_COEFFS[5]),
        f64::from_bits(P_COEFFS[4]),
    );
    let p = f_polyeval4(v_sq, (v_dd.lo + r1.lo) + lo, p0, p1, p2);

    const ERR_HI: [f64; 2] = [f64::from_bits(0x3aa0000000000000), 0.0];
    let err_hi = ERR_HI[if hi == 0.0 { 1 } else { 0 }];

    let err = f_fmla(v_sq, f64::from_bits(0x3ce0000000000000), err_hi);

    r1.lo = p;
    r1 = DoubleDouble::from_exact_add(r1.hi, r1.lo);
    r1 = DoubleDouble::full_add_f64(r1, -x);

    let left = r1.hi + (r1.lo - err);
    let right = r1.hi + (r1.lo + err);
    // Ziv's test to see if fast pass is accurate enough.
    if left == right {
        return left;
    }
    log1pmx_accurate_dd(x)
}

#[cold]
fn log1pmx_accurate_dd(x: f64) -> f64 {
    let r = log1p_dd(x);
    DoubleDouble::full_add_f64(r, -x).to_f64()
}

/// Computes log(1+x) - x
///
/// ulp 0.5
pub fn f_log1pmx(x: f64) -> f64 {
    let ax = x.to_bits() & 0x7fff_ffff_ffff_ffffu64;

    let x_e = (x.to_bits() >> 52) & 0x7ff;

    if !x.is_normal() {
        if x.is_nan() {
            return x + x;
        }
        if x.is_infinite() {
            return f64::NAN;
        }
        if x == 0. {
            return x;
        }
    }

    if ax > 0x3f90000000000000u64 {
        // |x| > 2^-6
        if x <= -1. {
            if x == -1. {
                return f64::NEG_INFINITY;
            }
            return f64::NAN;
        }
        return log1pmx_big(x);
    }

    const E_BIAS: u64 = (1u64 << (11 - 1u64)) - 1u64;

    // log(1+x) is expected to be used near zero

    if x_e < E_BIAS - 10 {
        if x_e < E_BIAS - 100 {
            // |x| < 2^-100
            // taylor series log(1+x) - x ~ -x^2/2 + x^3/3
            let x2 = x * x;
            let dx2 = f_fmla(x, x, -x2);
            let rl = dx2 * -0.5;
            return f_fmla(x2, -0.5, rl);
        }

        // Polynomial generated by Sollya in form log(1+x) - x = x^2 * R(x):
        // d = [-2^-10; 2^-10];
        // f_log1pf = (log(1+x) - x)/x^2;
        // Q = fpminimax(f_log1pf, 7, [|0, 107, 107, D...|], d);
        // See ./notes/log1pmx_at_zero.sollya

        let p = f_polyeval5(
            x,
            f64::from_bits(0x3fc999999999999a),
            f64::from_bits(0xbfc55555555546ef),
            f64::from_bits(0x3fc24924923d3abf),
            f64::from_bits(0xbfc000018af7f637),
            f64::from_bits(0x3fbc72984db24b6a),
        );

        let x2 = DoubleDouble::from_exact_mult(x, x);

        let mut r = DoubleDouble::f64_mul_f64_add(
            x,
            p,
            DoubleDouble::from_bit_pair((0xbbd3330899095800, 0xbfd0000000000000)),
        );
        r = DoubleDouble::mul_f64_add(
            r,
            x,
            DoubleDouble::from_bit_pair((0x3c75555538509407, 0x3fd5555555555555)),
        );
        r = DoubleDouble::mul_f64_add_f64(r, x, f64::from_bits(0xbfe0000000000000));
        r = DoubleDouble::quick_mult(r, x2);
        const ERR: f64 = f64::from_bits(0x3af0000000000000); // 2^-80
        let ub = r.hi + (r.lo + ERR);
        let lb = r.hi + (r.lo - ERR);
        if lb == ub {
            return r.to_f64();
        }
        return tiny_hard(x);
    }

    // Polynomial generated by Sollya in form log(1+x) - x = x^2 * R(x):
    // d = [-2^-6; 2^-6];
    // f_log1pf = (log(1+x) - x)/x^2;
    // Q = fpminimax(f_log1pf, 10, [|0, 107, 107, D...|], d);
    // See ./notes/log1pmx.sollya

    let p = f_estrin_polyeval8(
        x,
        f64::from_bits(0x3fc9999999999997),
        f64::from_bits(0xbfc5555555555552),
        f64::from_bits(0x3fc249249249fb64),
        f64::from_bits(0xbfc000000000f450),
        f64::from_bits(0x3fbc71c6e2591149),
        f64::from_bits(0xbfb999995cf14d86),
        f64::from_bits(0x3fb7494eb6c2c544),
        f64::from_bits(0xbfb558bf05690e85),
    );

    let x2 = DoubleDouble::from_exact_mult(x, x);

    let mut r = DoubleDouble::f64_mul_f64_add(
        x,
        p,
        DoubleDouble::from_bit_pair((0xbb9d89dc15a38000, 0xbfd0000000000000)),
    );
    r = DoubleDouble::mul_f64_add(
        r,
        x,
        DoubleDouble::from_bit_pair((0x3c7555a15a94e505, 0x3fd5555555555555)),
    );
    r = DoubleDouble::mul_f64_add_f64(r, x, f64::from_bits(0xbfe0000000000000));
    r = DoubleDouble::quick_mult(r, x2);

    r.to_f64()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log1pmx() {
        assert_eq!(
            f_log1pmx(0.00000000000000005076835735015165),
            -0.0000000000000000000000000000000012887130540163486
        );
        assert_eq!(f_log1pmx(5.), -3.208240530771945);
        assert_eq!(f_log1pmx(-0.99), -3.6151701859880907);
        assert_eq!(f_log1pmx(-1.), f64::NEG_INFINITY);
        assert_eq!(
            f_log1pmx(1.0000000000008708e-13),
            -0.0000000000000000000000000050000000000083744
        );
        assert_eq!(
            f_log1pmx(1.0000000000008708e-26),
            -0.00000000000000000000000000000000000000000000000000005000000000008707
        );
        assert_eq!(f_log1pmx(0.00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000012890176556069385),
                   -0.00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000830783258233204);
    }
}
