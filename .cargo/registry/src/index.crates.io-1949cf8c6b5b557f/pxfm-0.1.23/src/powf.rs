/*
 * // Copyright (c) Radzivon Bartoshyk 4/2025. All rights reserved.
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
use crate::bits::biased_exponent_f64;
use crate::common::*;
use crate::double_double::DoubleDouble;
use crate::exponents::expf;
use crate::logf;
use crate::logs::LOG2_R;
use crate::polyeval::{f_polyeval3, f_polyeval6, f_polyeval10};
use crate::pow_tables::EXP2_MID1;
use crate::powf_tables::{LOG2_R_TD, LOG2_R2_DD, POWF_R2};

/// Power function for given value for const context.
/// This is simplified version just to make a good approximation on const context.
#[inline]
pub const fn powf(d: f32, n: f32) -> f32 {
    let value = d.abs();
    let c = expf(n * logf(value));
    if n == 1. {
        return d;
    }
    if d < 0.0 {
        let y = n as i32;
        if y % 2 == 0 { c } else { -c }
    } else {
        c
    }
}

#[inline]
pub(crate) const fn is_integer(x: f32) -> bool {
    let x_u = x.to_bits();
    let x_e = (x_u & EXP_MASK_F32) >> 23;
    let lsb = (x_u | EXP_MASK_F32).trailing_zeros();
    const E_BIAS: u32 = (1u32 << (8 - 1u32)) - 1u32;
    const UNIT_EXPONENT: u32 = E_BIAS + 23;
    x_e + lsb >= UNIT_EXPONENT
}

#[inline]
pub(crate) fn is_odd_integer(x: f32) -> bool {
    let x_u = x.to_bits();
    let x_e = (x_u & EXP_MASK_F32) >> 23;
    let lsb = (x_u | EXP_MASK_F32).trailing_zeros();
    const E_BIAS: u32 = (1u32 << (8 - 1u32)) - 1u32;

    const UNIT_EXPONENT: u32 = E_BIAS + 23;
    x_e + lsb == UNIT_EXPONENT
}

#[inline]
const fn larger_exponent(a: f64, b: f64) -> bool {
    biased_exponent_f64(a) >= biased_exponent_f64(b)
}

// Calculate 2^(y * log2(x)) in double-double precision.
// At this point we can reuse the following values:
//   idx_x: index for extra precision of log2 for the middle part of log2(x).
//   dx: the reduced argument for log2(x)
//   y6: 2^6 * y.
//   lo6_hi: the high part of 2^6 * (y - (hi + mid))
//   exp2_hi_mid: high part of 2^(hi + mid)
#[cold]
#[inline(never)]
fn powf_dd(idx_x: i32, dx: f64, y6: f64, lo6_hi: f64, exp2_hi_mid: DoubleDouble) -> f64 {
    // Perform a second range reduction step:
    //   idx2 = round(2^14 * (dx  + 2^-8)) = round ( dx * 2^14 + 2^6)
    //   dx2 = (1 + dx) * r2 - 1
    // Output range:
    //   -0x1.3ffcp-15 <= dx2 <= 0x1.3e3dp-15
    let idx2 = f_fmla(
        dx,
        f64::from_bits(0x40d0000000000000),
        f64::from_bits(0x4050000000000000),
    )
    .round() as usize;
    let dx2 = f_fmla(1.0 + dx, f64::from_bits(POWF_R2[idx2]), -1.0); // Exact

    const COEFFS: [(u64, u64); 6] = [
        (0x3c7777d0ffda25e0, 0x3ff71547652b82fe),
        (0xbc6777d101cf0a84, 0xbfe71547652b82fe),
        (0x3c7ce04b5140d867, 0x3fdec709dc3a03fd),
        (0x3c7137b47e635be5, 0xbfd71547652b82fb),
        (0xbc5b5a30b3bdb318, 0x3fd2776c516a92a2),
        (0x3c62d2fbd081e657, 0xbfcec70af1929ca6),
    ];
    let dx_dd = DoubleDouble::new(0.0, dx2);
    let p = f_polyeval6(
        dx_dd,
        DoubleDouble::from_bit_pair(COEFFS[0]),
        DoubleDouble::from_bit_pair(COEFFS[1]),
        DoubleDouble::from_bit_pair(COEFFS[2]),
        DoubleDouble::from_bit_pair(COEFFS[3]),
        DoubleDouble::from_bit_pair(COEFFS[4]),
        DoubleDouble::from_bit_pair(COEFFS[5]),
    );
    // log2(1 + dx2) ~ dx2 * P(dx2)
    let log2_x_lo = DoubleDouble::quick_mult_f64(p, dx2);
    // Lower parts of (e_x - log2(r1)) of the first range reduction constant
    let log2_r_td = LOG2_R_TD[idx_x as usize];
    let log2_x_mid = DoubleDouble::new(f64::from_bits(log2_r_td.0), f64::from_bits(log2_r_td.1));
    // -log2(r2) + lower part of (e_x - log2(r1))
    let log2_x_m = DoubleDouble::add(DoubleDouble::from_bit_pair(LOG2_R2_DD[idx2]), log2_x_mid);
    // log2(1 + dx2) - log2(r2) + lower part of (e_x - log2(r1))
    // Since we don't know which one has larger exponent to apply Fast2Sum
    // algorithm, we need to check them before calling double-double addition.
    let log2_x = if larger_exponent(log2_x_m.hi, log2_x_lo.hi) {
        DoubleDouble::add(log2_x_m, log2_x_lo)
    } else {
        DoubleDouble::add(log2_x_lo, log2_x_m)
    };
    let lo6_hi_dd = DoubleDouble::new(0.0, lo6_hi);
    // 2^6 * y * (log2(1 + dx2) - log2(r2) + lower part of (e_x - log2(r1)))
    let prod = DoubleDouble::quick_mult_f64(log2_x, y6);
    // 2^6 * (y * log2(x) - (hi + mid)) = 2^6 * lo
    let lo6 = if larger_exponent(prod.hi, lo6_hi) {
        DoubleDouble::add(prod, lo6_hi_dd)
    } else {
        DoubleDouble::add(lo6_hi_dd, prod)
    };

    const EXP2_COEFFS: [(u64, u64); 10] = [
        (0x0000000000000000, 0x3ff0000000000000),
        (0x3c1abc9e3b398024, 0x3f862e42fefa39ef),
        (0xbba5e43a5429bddb, 0x3f0ebfbdff82c58f),
        (0xbb2d33162491268f, 0x3e8c6b08d704a0c0),
        (0x3a94fb32d240a14e, 0x3e03b2ab6fba4e77),
        (0x39ee84e916be83e0, 0x3d75d87fe78a6731),
        (0xb989a447bfddc5e6, 0x3ce430912f86bfb8),
        (0xb8e31a55719de47f, 0x3c4ffcbfc588ded9),
        (0xb850ba57164eb36b, 0x3bb62c034beb8339),
        (0xb7b8483eabd9642d, 0x3b1b5251ff97bee1),
    ];

    let pp = f_polyeval10(
        lo6,
        DoubleDouble::from_bit_pair(EXP2_COEFFS[0]),
        DoubleDouble::from_bit_pair(EXP2_COEFFS[1]),
        DoubleDouble::from_bit_pair(EXP2_COEFFS[2]),
        DoubleDouble::from_bit_pair(EXP2_COEFFS[3]),
        DoubleDouble::from_bit_pair(EXP2_COEFFS[4]),
        DoubleDouble::from_bit_pair(EXP2_COEFFS[5]),
        DoubleDouble::from_bit_pair(EXP2_COEFFS[6]),
        DoubleDouble::from_bit_pair(EXP2_COEFFS[7]),
        DoubleDouble::from_bit_pair(EXP2_COEFFS[8]),
        DoubleDouble::from_bit_pair(EXP2_COEFFS[9]),
    );
    let rr = DoubleDouble::quick_mult(exp2_hi_mid, pp);

    // Make sure the sum is normalized:
    let r = DoubleDouble::from_exact_add(rr.hi, rr.lo);

    const FRACTION_MASK: u64 = (1u64 << 52) - 1;

    let mut r_bits = r.hi.to_bits();
    if ((r_bits & 0xfff_ffff) == 0) && (r.lo != 0.0) {
        let hi_sign = r.hi.to_bits() >> 63;
        let lo_sign = r.lo.to_bits() >> 63;
        if hi_sign == lo_sign {
            r_bits = r_bits.wrapping_add(1);
        } else if (r_bits & FRACTION_MASK) > 0 {
            r_bits = r_bits.wrapping_sub(1);
        }
    }

    f64::from_bits(r_bits)
}

/// Power function
///
/// Max found ULP 0.5
#[inline]
pub fn f_powf(x: f32, y: f32) -> f32 {
    let mut x_u = x.to_bits();
    let x_abs = x_u & 0x7fff_ffff;
    let mut y = y;
    let y_u = y.to_bits();
    let y_abs = y_u & 0x7fff_ffff;
    let mut x = x;

    if ((y_abs & 0x0007_ffff) == 0) || (y_abs > 0x4f170000) {
        // y is signaling NaN
        if x.is_nan() || y.is_nan() {
            if y.abs() == 0. {
                return 1.;
            }
            if x == 1. {
                return 1.;
            }
            return f32::NAN;
        }

        // Exceptional exponents.
        if y == 0.0 {
            return 1.0;
        }

        match y_abs {
            0x7f80_0000 => {
                if x_abs > 0x7f80_0000 {
                    // pow(NaN, +-Inf) = NaN
                    return x;
                }
                if x_abs == 0x3f80_0000 {
                    // pow(+-1, +-Inf) = 1.0f
                    return 1.0;
                }
                if x == 0.0 && y_u == 0xff80_0000 {
                    // pow(+-0, -Inf) = +inf and raise FE_DIVBYZERO
                    return f32::INFINITY;
                }
                // pow (|x| < 1, -inf) = +inf
                // pow (|x| < 1, +inf) = 0.0f
                // pow (|x| > 1, -inf) = 0.0f
                // pow (|x| > 1, +inf) = +inf
                return if (x_abs < 0x3f80_0000) == (y_u == 0xff80_0000) {
                    f32::INFINITY
                } else {
                    0.
                };
            }
            _ => {
                match y_u {
                    0x3f00_0000 => {
                        // pow(x, 1/2) = sqrt(x)
                        if x == 0.0 || x_u == 0xff80_0000 {
                            // pow(-0, 1/2) = +0
                            // pow(-inf, 1/2) = +inf
                            // Make sure it is correct for FTZ/DAZ.
                            return x * x;
                        }
                        let r = x.sqrt();
                        return if r.to_bits() != 0x8000_0000 { r } else { 0.0 };
                    }
                    0x3f80_0000 => {
                        return x;
                    } // y = 1.0f
                    0x4000_0000 => return x * x, // y = 2.0f
                    _ => {
                        let is_int = is_integer(y);
                        if is_int && (y_u > 0x4000_0000) && (y_u <= 0x41c0_0000) {
                            // Check for exact cases when 2 < y < 25 and y is an integer.
                            let mut msb: i32 = if x_abs == 0 {
                                32 - 2
                            } else {
                                x_abs.leading_zeros() as i32
                            };
                            msb = if msb > 8 { msb } else { 8 };
                            let mut lsb: i32 = if x_abs == 0 {
                                0
                            } else {
                                x_abs.trailing_zeros() as i32
                            };
                            lsb = if lsb > 23 { 23 } else { lsb };
                            let extra_bits: i32 = 32 - 2 - lsb - msb;
                            let iter = y as i32;

                            if extra_bits * iter <= 23 + 2 {
                                // The result is either exact or exactly half-way.
                                // But it is exactly representable in double precision.
                                let x_d = x as f64;
                                let mut result = x_d;
                                for _ in 1..iter {
                                    result *= x_d;
                                }
                                return result as f32;
                            }
                        }

                        if y_abs > 0x4f17_0000 {
                            // if y is NaN
                            if y_abs > 0x7f80_0000 {
                                if x_u == 0x3f80_0000 {
                                    // x = 1.0f
                                    // pow(1, NaN) = 1
                                    return 1.0;
                                }
                                // pow(x, NaN) = NaN
                                return y;
                            }
                            // x^y will be overflow / underflow in single precision.  Set y to a
                            // large enough exponent but not too large, so that the computations
                            // won't be overflow in double precision.
                            y = f32::from_bits((y_u & 0x8000_0000).wrapping_add(0x4f800000u32));
                        }
                    }
                }
            }
        }
    }

    const E_BIAS: u32 = (1u32 << (8 - 1u32)) - 1u32;
    let mut ex = -(E_BIAS as i32);
    let mut sign: u64 = 0;

    if ((x_u & 0x801f_ffffu32) == 0) || x_u >= 0x7f80_0000u32 || x_u < 0x0080_0000u32 {
        if x.is_nan() {
            return f32::NAN;
        }

        if x_u == 0x3f80_0000 {
            return 1.;
        }

        let x_is_neg = x.to_bits() > 0x8000_0000;

        if x == 0.0 {
            let out_is_neg = x_is_neg && is_odd_integer(f32::from_bits(y_u));
            if y_u > 0x8000_0000u32 {
                // pow(0, negative number) = inf
                return if x_is_neg {
                    f32::NEG_INFINITY
                } else {
                    f32::INFINITY
                };
            }
            // pow(0, positive number) = 0
            return if out_is_neg { -0.0 } else { 0.0 };
        }

        if x_abs == 0x7f80_0000u32 {
            // x = +-Inf
            let out_is_neg = x_is_neg && is_odd_integer(f32::from_bits(y_u));
            if y_u >= 0x7fff_ffff {
                return if out_is_neg { -0.0 } else { 0.0 };
            }
            return if out_is_neg {
                f32::NEG_INFINITY
            } else {
                f32::INFINITY
            };
        }

        if x_abs > 0x7f80_0000 {
            // x is NaN.
            // pow (aNaN, 0) is already taken care above.
            return x;
        }

        // Normalize denormal inputs.
        if x_abs < 0x0080_0000u32 {
            ex = ex.wrapping_sub(64);
            x *= f32::from_bits(0x5f800000);
        }

        // x is finite and negative, and y is a finite integer.
        if x.is_sign_negative() {
            if is_integer(y) {
                x = -x;
                if is_odd_integer(y) {
                    sign = 0x8000_0000_0000_0000u64;
                }
            } else {
                // pow( negative, non-integer ) = NaN
                return f32::NAN;
            }
        }
    }

    // x^y = 2^( y * log2(x) )
    //     = 2^( y * ( e_x + log2(m_x) ) )
    // First we compute log2(x) = e_x + log2(m_x)
    x_u = x.to_bits();

    // Extract exponent field of x.
    ex = ex.wrapping_add((x_u >> 23) as i32);
    let e_x = ex as f64;
    // Use the highest 7 fractional bits of m_x as the index for look up tables.
    let x_mant = x_u & ((1u32 << 23) - 1);
    let idx_x = (x_mant >> (23 - 7)) as i32;
    // Add the hidden bit to the mantissa.
    // 1 <= m_x < 2
    let m_x = f32::from_bits(x_mant | 0x3f800000);

    // Reduced argument for log2(m_x):
    //   dx = r * m_x - 1.
    // The computation is exact, and -2^-8 <= dx < 2^-7.
    // Then m_x = (1 + dx) / r, and
    //   log2(m_x) = log2( (1 + dx) / r )
    //             = log2(1 + dx) - log2(r).

    let dx;
    #[cfg(any(
        all(
            any(target_arch = "x86", target_arch = "x86_64"),
            target_feature = "fma"
        ),
        all(target_arch = "aarch64", target_feature = "neon")
    ))]
    {
        use crate::logs::LOG_REDUCTION_F32;
        dx = f_fmlaf(
            m_x,
            f32::from_bits(LOG_REDUCTION_F32.0[idx_x as usize]),
            -1.0,
        ) as f64; // Exact.
    }
    #[cfg(not(any(
        all(
            any(target_arch = "x86", target_arch = "x86_64"),
            target_feature = "fma"
        ),
        all(target_arch = "aarch64", target_feature = "neon")
    )))]
    {
        use crate::logs::LOG_RANGE_REDUCTION;
        dx = f_fmla(
            m_x as f64,
            f64::from_bits(LOG_RANGE_REDUCTION[idx_x as usize]),
            -1.0,
        ); // Exact
    }

    // Degree-5 polynomial approximation:
    //   dx * P(dx) ~ log2(1 + dx)
    // Generated by Sollya with:
    // > P = fpminimax(log2(1 + x)/x, 5, [|D...|], [-2^-8, 2^-7]);
    // > dirtyinfnorm(log2(1 + x)/x - P, [-2^-8, 2^-7]);
    //   0x1.653...p-52
    const COEFFS: [u64; 6] = [
        0x3ff71547652b82fe,
        0xbfe71547652b7a07,
        0x3fdec709dc458db1,
        0xbfd715479c2266c9,
        0x3fd2776ae1ddf8f0,
        0xbfce7b2178870157,
    ];

    let dx2 = dx * dx; // Exact
    let c0 = f_fmla(dx, f64::from_bits(COEFFS[1]), f64::from_bits(COEFFS[0]));
    let c1 = f_fmla(dx, f64::from_bits(COEFFS[3]), f64::from_bits(COEFFS[2]));
    let c2 = f_fmla(dx, f64::from_bits(COEFFS[5]), f64::from_bits(COEFFS[4]));

    let p = f_polyeval3(dx2, c0, c1, c2);

    // s = e_x - log2(r) + dx * P(dx)
    // Approximation errors:
    //   |log2(x) - s| < ulp(e_x) + (bounds on dx) * (error bounds of P(dx))
    //                 = ulp(e_x) + 2^-7 * 2^-51
    //                 < 2^8 * 2^-52 + 2^-7 * 2^-43
    //                 ~ 2^-44 + 2^-50
    let s = f_fmla(dx, p, f64::from_bits(LOG2_R[idx_x as usize]) + e_x);

    // To compute 2^(y * log2(x)), we break the exponent into 3 parts:
    //   y * log(2) = hi + mid + lo, where
    //   hi is an integer
    //   mid * 2^6 is an integer
    //   |lo| <= 2^-7
    // Then:
    //   x^y = 2^(y * log2(x)) = 2^hi * 2^mid * 2^lo,
    // In which 2^mid is obtained from a look-up table of size 2^6 = 64 elements,
    // and 2^lo ~ 1 + lo * P(lo).
    // Thus, we have:
    //   hi + mid = 2^-6 * round( 2^6 * y * log2(x) )
    // If we restrict the output such that |hi| < 150, (hi + mid) uses (8 + 6)
    // bits, hence, if we use double precision to perform
    //   round( 2^6 * y * log2(x))
    // the lo part is bounded by 2^-7 + 2^(-(52 - 14)) = 2^-7 + 2^-38

    // In the following computations:
    //   y6  = 2^6 * y
    //   hm  = 2^6 * (hi + mid) = round(2^6 * y * log2(x)) ~ round(y6 * s)
    //   lo6 = 2^6 * lo = 2^6 * (y - (hi + mid)) = y6 * log2(x) - hm.
    let y6 = (y * f32::from_bits(0x42800000)) as f64; // Exact.
    let hm = (s * y6).round();

    // let log2_rr = LOG2_R2_DD[idx_x as usize];

    // // lo6 = 2^6 * lo.
    // let lo6_hi = f_fmla(y6, e_x + f64::from_bits(log2_rr.1), -hm); // Exact
    // // Error bounds:
    // //   | (y*log2(x) - hm * 2^-6 - lo) / y| < err(dx * p) + err(LOG2_R_DD.lo)
    // //                                       < 2^-51 + 2^-75
    // let lo6 = f_fmla(y6, f_fmla(dx, p, f64::from_bits(log2_rr.0)), lo6_hi);

    // lo6 = 2^6 * lo.
    let lo6_hi = f_fmla(y6, e_x + f64::from_bits(LOG2_R_TD[idx_x as usize].2), -hm); // Exact
    // Error bounds:
    //   | (y*log2(x) - hm * 2^-6 - lo) / y| < err(dx * p) + err(LOG2_R_DD.lo)
    //                                       < 2^-51 + 2^-75
    let lo6 = f_fmla(
        y6,
        f_fmla(dx, p, f64::from_bits(LOG2_R_TD[idx_x as usize].1)),
        lo6_hi,
    );

    // |2^(hi + mid) - exp2_hi_mid| <= ulp(exp2_hi_mid) / 2
    // Clamp the exponent part into smaller range that fits double precision.
    // For those exponents that are out of range, the final conversion will round
    // them correctly to inf/max float or 0/min float accordingly.
    let mut hm_i = hm as i64;
    hm_i = if hm_i > (1i64 << 15) {
        1 << 15
    } else if hm_i < (-(1i64 << 15)) {
        -(1 << 15)
    } else {
        hm_i
    };

    let idx_y = hm_i & 0x3f;

    // 2^hi
    let exp_hi_i = (hm_i >> 6).wrapping_shl(52);
    // 2^mid
    let exp_mid_i = EXP2_MID1[idx_y as usize].1;
    // (-1)^sign * 2^hi * 2^mid
    // Error <= 2^hi * 2^-53
    let exp2_hi_mid_i = (exp_hi_i.wrapping_add(exp_mid_i as i64) as u64).wrapping_add(sign);
    let exp2_hi_mid = f64::from_bits(exp2_hi_mid_i);

    // Degree-5 polynomial approximation P(lo6) ~ 2^(lo6 / 2^6) = 2^(lo).
    // Generated by Sollya with:
    // > P = fpminimax(2^(x/64), 5, [|1, D...|], [-2^-1, 2^-1]);
    // > dirtyinfnorm(2^(x/64) - P, [-0.5, 0.5]);
    // 0x1.a2b77e618f5c4c176fd11b7659016cde5de83cb72p-60
    const EXP2_COEFFS: [u64; 6] = [
        0x3ff0000000000000,
        0x3f862e42fefa39ef,
        0x3f0ebfbdff82a23a,
        0x3e8c6b08d7076268,
        0x3e03b2ad33f8b48b,
        0x3d75d870c4d84445,
    ];

    let lo6_sqr = lo6 * lo6;
    let d0 = f_fmla(
        lo6,
        f64::from_bits(EXP2_COEFFS[1]),
        f64::from_bits(EXP2_COEFFS[0]),
    );
    let d1 = f_fmla(
        lo6,
        f64::from_bits(EXP2_COEFFS[3]),
        f64::from_bits(EXP2_COEFFS[2]),
    );
    let d2 = f_fmla(
        lo6,
        f64::from_bits(EXP2_COEFFS[5]),
        f64::from_bits(EXP2_COEFFS[4]),
    );
    let pp = f_polyeval3(lo6_sqr, d0, d1, d2);

    let r = pp * exp2_hi_mid;
    let r_u = r.to_bits();

    #[cfg(any(
        all(
            any(target_arch = "x86", target_arch = "x86_64"),
            target_feature = "fma"
        ),
        all(target_arch = "aarch64", target_feature = "neon")
    ))]
    const ERR: u64 = 64;
    #[cfg(not(any(
        all(
            any(target_arch = "x86", target_arch = "x86_64"),
            target_feature = "fma"
        ),
        all(target_arch = "aarch64", target_feature = "neon")
    )))]
    const ERR: u64 = 128;

    let r_upper = f64::from_bits(r_u + ERR) as f32;
    let r_lower = f64::from_bits(r_u - ERR) as f32;
    if r_upper == r_lower {
        return r_upper;
    }

    // Scale lower part of 2^(hi + mid)
    let exp2_hi_mid_dd = DoubleDouble {
        lo: if idx_y != 0 {
            f64::from_bits((exp_hi_i as u64).wrapping_add(EXP2_MID1[idx_y as usize].0))
        } else {
            0.
        },
        hi: exp2_hi_mid,
    };

    let r_dd = powf_dd(idx_x, dx, y6, lo6_hi, exp2_hi_mid_dd);
    r_dd as f32
}

/// Dirty fast pow
#[inline]
pub fn dirty_powf(d: f32, n: f32) -> f32 {
    use crate::exponents::dirty_exp2f;
    use crate::logs::dirty_log2f;
    let value = d.abs();
    let lg = dirty_log2f(value);
    let c = dirty_exp2f(n * lg);
    if d < 0.0 {
        let y = n as i32;
        if y % 2 == 0 { c } else { -c }
    } else {
        c
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn powf_test() {
        assert!(
            (powf(2f32, 3f32) - 8f32).abs() < 1e-6,
            "Invalid result {}",
            powf(2f32, 3f32)
        );
        assert!(
            (powf(0.5f32, 2f32) - 0.25f32).abs() < 1e-6,
            "Invalid result {}",
            powf(0.5f32, 2f32)
        );
    }

    #[test]
    fn f_powf_test() {
        assert!(
            (f_powf(2f32, 3f32) - 8f32).abs() < 1e-6,
            "Invalid result {}",
            f_powf(2f32, 3f32)
        );
        assert!(
            (f_powf(0.5f32, 2f32) - 0.25f32).abs() < 1e-6,
            "Invalid result {}",
            f_powf(0.5f32, 2f32)
        );
        assert_eq!(f_powf(0.5f32, 1.5432f32), 0.34312353);
        assert_eq!(
            f_powf(f32::INFINITY, 0.00000000000000000000000000000000038518824),
            f32::INFINITY
        );
        assert_eq!(f_powf(f32::NAN, 0.0), 1.);
        assert_eq!(f_powf(1., f32::NAN), 1.);
    }

    #[test]
    fn dirty_powf_test() {
        assert!(
            (dirty_powf(2f32, 3f32) - 8f32).abs() < 1e-6,
            "Invalid result {}",
            dirty_powf(2f32, 3f32)
        );
        assert!(
            (dirty_powf(0.5f32, 2f32) - 0.25f32).abs() < 1e-6,
            "Invalid result {}",
            dirty_powf(0.5f32, 2f32)
        );
    }
}
