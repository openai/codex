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

#![allow(clippy::excessive_precision)]

use crate::bessel::alpha1::{
    bessel_1_asympt_alpha, bessel_1_asympt_alpha_fast, bessel_1_asympt_alpha_hard,
};
use crate::bessel::beta1::{
    bessel_1_asympt_beta, bessel_1_asympt_beta_fast, bessel_1_asympt_beta_hard,
};
use crate::bessel::i0::bessel_rsqrt_hard;
use crate::bessel::j1_coeffs::{J1_COEFFS, J1_ZEROS, J1_ZEROS_VALUE};
use crate::bessel::j1_coeffs_taylor::J1_COEFFS_TAYLOR;
use crate::common::f_fmla;
use crate::double_double::DoubleDouble;
use crate::dyadic_float::{DyadicFloat128, DyadicSign};
use crate::polyeval::{f_polyeval8, f_polyeval9, f_polyeval12, f_polyeval19};
use crate::sin_helper::{sin_dd_small, sin_dd_small_fast, sin_f128_small};
use crate::sincos_reduce::{AngleReduced, rem2pi_any, rem2pi_f128};

/// Bessel J 1st order in f64
///
/// Note about accuracy:
/// - Close to zero Bessel have tiny values such that testing against MPFR must be done exactly
///   in the same precision, since any nearest representable number have ULP > 0.5,
///   for example `J1(0.000000000000000000000000000000000000023509886)` in single precision
///   have 0.7 ULP for any number with extended precision that would be represented in f32
///   Same applies to J1(4.4501477170144018E-309) in double precision and some others subnormal numbers
pub fn f_j1(x: f64) -> f64 {
    if !x.is_normal() {
        if x == 0. {
            return x;
        }
        if x.is_infinite() {
            return 0.;
        }
        if x.is_nan() {
            return x + x;
        }
    }

    let ax: u64 = x.to_bits() & 0x7fff_ffff_ffff_ffff;

    if ax < 0x4052a6784230fcf8u64 {
        // 74.60109
        if ax < 0x3feccccccccccccd {
            // 0.9
            return j1_maclaurin_series_fast(x);
        }
        return j1_small_argument_fast(x);
    }

    j1_asympt_fast(x)
}

/*
   Evaluates:
   J1 = sqrt(2/(PI*x)) * beta(x) * cos(x - 3*PI/4 - alpha(x))
   discarding 1*PI/2 using identities gives:
   J1 = sqrt(2/(PI*x)) * beta(x) * sin(x - PI/4 - alpha(x))

   to avoid squashing small (-PI/4 - alpha(x)) into a large x actual expansion is:

   J1 = sqrt(2/(PI*x)) * beta(x) * sin((x mod 2*PI) - PI/4 - alpha(x))
*/
#[inline]
fn j1_asympt_fast(x: f64) -> f64 {
    let origin_x = x;
    static SGN: [f64; 2] = [1., -1.];
    let sign_scale = SGN[x.is_sign_negative() as usize];
    let x = x.abs();

    const SQRT_2_OVER_PI: DoubleDouble = DoubleDouble::new(
        f64::from_bits(0xbc8cbc0d30ebfd15),
        f64::from_bits(0x3fe9884533d43651),
    );
    const MPI_OVER_4: DoubleDouble = DoubleDouble::new(
        f64::from_bits(0xbc81a62633145c07),
        f64::from_bits(0xbfe921fb54442d18),
    );

    let recip = if x.to_bits() > 0x7fd000000000000u64 {
        DoubleDouble::quick_mult_f64(DoubleDouble::from_exact_safe_div(4.0, x), 0.25)
    } else {
        DoubleDouble::from_recip(x)
    };

    let alpha = bessel_1_asympt_alpha_fast(recip);
    let beta = bessel_1_asympt_beta_fast(recip);

    let AngleReduced { angle } = rem2pi_any(x);

    // Without full subtraction cancellation happens sometimes
    let x0pi34 = DoubleDouble::full_dd_sub(MPI_OVER_4, alpha);
    let r0 = DoubleDouble::full_dd_add(angle, x0pi34);

    let m_sin = sin_dd_small_fast(r0);
    let z0 = DoubleDouble::quick_mult(beta, m_sin);
    let r_sqrt = DoubleDouble::from_rsqrt_fast(x);
    let scale = DoubleDouble::quick_mult(SQRT_2_OVER_PI, r_sqrt);
    let p = DoubleDouble::quick_mult(scale, z0);

    let err = f_fmla(
        p.hi,
        f64::from_bits(0x3be0000000000000), // 2^-65
        f64::from_bits(0x3a60000000000000), // 2^-89
    );
    let ub = p.hi + (p.lo + err);
    let lb = p.hi + (p.lo - err);

    if ub == lb {
        return p.to_f64() * sign_scale;
    }

    j1_asympt(origin_x, recip, r_sqrt, angle)
}

/*
   Evaluates:
   J1 = sqrt(2/(PI*x)) * beta(x) * cos(x - 3*PI/4 - alpha(x))
   discarding 1*PI/2 using identities gives:
   J1 = sqrt(2/(PI*x)) * beta(x) * sin(x - PI/4 - alpha(x))

   to avoid squashing small (-PI/4 - alpha(x)) into a large x actual expansion is:

   J1 = sqrt(2/(PI*x)) * beta(x) * sin((x mod 2*PI) - PI/4 - alpha(x))
*/
fn j1_asympt(x: f64, recip: DoubleDouble, r_sqrt: DoubleDouble, angle: DoubleDouble) -> f64 {
    let origin_x = x;
    static SGN: [f64; 2] = [1., -1.];
    let sign_scale = SGN[x.is_sign_negative() as usize];

    const SQRT_2_OVER_PI: DoubleDouble = DoubleDouble::new(
        f64::from_bits(0xbc8cbc0d30ebfd15),
        f64::from_bits(0x3fe9884533d43651),
    );
    const MPI_OVER_4: DoubleDouble = DoubleDouble::new(
        f64::from_bits(0xbc81a62633145c07),
        f64::from_bits(0xbfe921fb54442d18),
    );

    let alpha = bessel_1_asympt_alpha(recip);
    let beta = bessel_1_asympt_beta(recip);

    // Without full subtraction cancellation happens sometimes
    let x0pi34 = DoubleDouble::full_dd_sub(MPI_OVER_4, alpha);
    let r0 = DoubleDouble::full_dd_add(angle, x0pi34);

    let m_sin = sin_dd_small(r0);
    let z0 = DoubleDouble::quick_mult(beta, m_sin);
    let scale = DoubleDouble::quick_mult(SQRT_2_OVER_PI, r_sqrt);
    let r = DoubleDouble::quick_mult(scale, z0);

    let p = DoubleDouble::from_exact_add(r.hi, r.lo);

    let err = f_fmla(
        p.hi,
        f64::from_bits(0x3bc0000000000000), // 2^-67
        f64::from_bits(0x39c0000000000000), // 2^-99
    );

    let ub = p.hi + (p.lo + err);
    let lb = p.hi + (p.lo - err);

    if ub == lb {
        return p.to_f64() * sign_scale;
    }

    j1_asympt_hard(origin_x)
}

/**
Generated in Sollya:
```text
pretty = proc(u) {
  return ~(floor(u*1000)/1000);
};

bessel_j1 = library("./cmake-build-release/libbessel_sollya.dylib");

f = bessel_j1(x)/x;
d = [0, 0.921];
w = 1;
pf = fpminimax(f, [|0,2,4,6,8,10,12,14,16,18,20,22,24|], [|107, 107, 107, 107, 107, D...|], d, absolute, floating);

w = 1;
or_f = bessel_j1(x);
pf1 = pf * x;
err_p = -log2(dirtyinfnorm(pf1*w-or_f, d));
print ("relative error:", pretty(err_p));

for i from 0 to degree(pf) by 2 do {
    print("'", coeff(pf, i), "',");
};
```
See ./notes/bessel_sollya/bessel_j1_at_zero_fast.sollya
**/
#[inline]
pub(crate) fn j1_maclaurin_series_fast(x: f64) -> f64 {
    const C0: DoubleDouble = DoubleDouble::from_bit_pair((0x3b30e9e087200000, 0x3fe0000000000000));
    let x2 = DoubleDouble::from_exact_mult(x, x);
    let p = f_polyeval12(
        x2.hi,
        f64::from_bits(0xbfb0000000000000),
        f64::from_bits(0x3f65555555555555),
        f64::from_bits(0xbf0c71c71c71c45e),
        f64::from_bits(0x3ea6c16c16b82b02),
        f64::from_bits(0xbe3845c87ec0cbef),
        f64::from_bits(0x3dc27e0313e8534c),
        f64::from_bits(0xbd4443dd2d0305d0),
        f64::from_bits(0xbd0985a435fe9aa1),
        f64::from_bits(0x3d10c82d92c46d30),
        f64::from_bits(0xbd0aa3684321f219),
        f64::from_bits(0x3cf8351f29ac345a),
        f64::from_bits(0xbcd333fe6cd52c9f),
    );
    let mut z = DoubleDouble::mul_f64_add(x2, p, C0);
    z = DoubleDouble::quick_mult_f64(z, x);

    // squaring error (2^-56) + poly error 2^-75
    let err = f_fmla(
        x2.hi,
        f64::from_bits(0x3c70000000000000), // 2^-56
        f64::from_bits(0x3b40000000000000), // 2^-75
    );
    let ub = z.hi + (z.lo + err);
    let lb = z.hi + (z.lo - err);

    if ub == lb {
        return z.to_f64();
    }
    j1_maclaurin_series(x)
}

/**
Generated in Sollya:
```text
pretty = proc(u) {
  return ~(floor(u*1000)/1000);
};

bessel_j1 = library("./cmake-build-release/libbessel_sollya.dylib");

f = bessel_j1(x)/x;
d = [0, 0.921];
w = 1;
pf = fpminimax(f, [|0,2,4,6,8,10,12,14,16,18,20,22,24|], [|107, 107, 107, 107, 107, D...|], d, absolute, floating);

w = 1;
or_f = bessel_j1(x);
pf1 = pf * x;
err_p = -log2(dirtyinfnorm(pf1*w-or_f, d));
print ("relative error:", pretty(err_p));

for i from 0 to degree(pf) by 2 do {
    print("'", coeff(pf, i), "',");
};
```
See ./notes/bessel_sollya/bessel_j1_at_zero.sollya
**/
pub(crate) fn j1_maclaurin_series(x: f64) -> f64 {
    let origin_x = x;
    static SGN: [f64; 2] = [1., -1.];
    let sign_scale = SGN[x.is_sign_negative() as usize];
    let x = x.abs();

    const CL: [(u64, u64); 5] = [
        (0xb930000000000000, 0x3fe0000000000000),
        (0x39c8e80000000000, 0xbfb0000000000000),
        (0x3c05555554f3add7, 0x3f65555555555555),
        (0xbbac71c4eb0f8c94, 0xbf0c71c71c71c71c),
        (0xbb3f56b7a43206d4, 0x3ea6c16c16c16c17),
    ];

    let dx2 = DoubleDouble::from_exact_mult(x, x);

    let p = f_polyeval8(
        dx2.hi,
        f64::from_bits(0xbe3845c8a0ce5129),
        f64::from_bits(0x3dc27e4fb7789ea2),
        f64::from_bits(0xbd4522a43f633af1),
        f64::from_bits(0x3cc2c97589d53f97),
        f64::from_bits(0xbc3ab8151dca7912),
        f64::from_bits(0x3baf08732286d1d4),
        f64::from_bits(0xbb10ac65637413f4),
        f64::from_bits(0xbae4d8336e4f779c),
    );

    let mut p_e = DoubleDouble::mul_f64_add(dx2, p, DoubleDouble::from_bit_pair(CL[4]));
    p_e = DoubleDouble::mul_add(dx2, p_e, DoubleDouble::from_bit_pair(CL[3]));
    p_e = DoubleDouble::mul_add(dx2, p_e, DoubleDouble::from_bit_pair(CL[2]));
    p_e = DoubleDouble::mul_add(dx2, p_e, DoubleDouble::from_bit_pair(CL[1]));
    p_e = DoubleDouble::mul_add(dx2, p_e, DoubleDouble::from_bit_pair(CL[0]));

    let p = DoubleDouble::quick_mult_f64(p_e, x);

    let err = f_fmla(
        p.hi,
        f64::from_bits(0x3bd0000000000000), // 2^-66
        f64::from_bits(0x3a00000000000000), // 2^-95
    );
    let ub = p.hi + (p.lo + err);
    let lb = p.hi + (p.lo - err);
    if ub != lb {
        return j1_maclaurin_series_hard(origin_x);
    }

    p.to_f64() * sign_scale
}

/**
Taylor expansion at 0

Generated by SageMath:
```python
def print_expansion_at_0():
    print(f"static C: [DyadicFloat128; 13] = ")
    from mpmath import mp, j1, taylor, expm1
    poly = taylor(lambda val: j1(val), 0, 26)
    real_i = 0
    print("[")
    for i in range(1, len(poly), 2):
        print_dyadic(poly[i])
        real_i = real_i + 1
    print("],")

    print("];")

mp.prec = 180

print_expansion_at_0()
```
**/
#[cold]
#[inline(never)]
fn j1_maclaurin_series_hard(x: f64) -> f64 {
    static SGN: [f64; 2] = [1., -1.];
    let sign_scale = SGN[x.is_sign_negative() as usize];
    let x = x.abs();
    static C: [DyadicFloat128; 13] = [
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -128,
            mantissa: 0x80000000_00000000_00000000_00000000_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Neg,
            exponent: -131,
            mantissa: 0x80000000_00000000_00000000_00000000_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -136,
            mantissa: 0xaaaaaaaa_aaaaaaaa_aaaaaaaa_aaaaaaab_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Neg,
            exponent: -142,
            mantissa: 0xe38e38e3_8e38e38e_38e38e38_e38e38e4_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -148,
            mantissa: 0xb60b60b6_0b60b60b_60b60b60_b60b60b6_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Neg,
            exponent: -155,
            mantissa: 0xc22e4506_72894ab6_cd8efb11_d33f5618_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -162,
            mantissa: 0x93f27dbb_c4fae397_780b69f5_333c725b_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Neg,
            exponent: -170,
            mantissa: 0xa91521fb_2a434d3f_649f5485_f169a743_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -178,
            mantissa: 0x964bac6d_7ae67d8d_aec68405_485dea03_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Neg,
            exponent: -187,
            mantissa: 0xd5c0f53a_fe6fa17f_8c7b0b68_39691f4e_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -196,
            mantissa: 0xf8bb4be7_8e7896b0_58fee362_01a4370c_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Neg,
            exponent: -205,
            mantissa: 0xf131bdf7_cff8d02e_e1ef6820_f9d58ab6_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -214,
            mantissa: 0xc5e72c48_0d1aec75_3caa2e0d_edd008ca_u128,
        },
    ];

    let rx = DyadicFloat128::new_from_f64(x);
    let dx = rx * rx;

    let mut p = C[12];
    for i in (0..12).rev() {
        p = dx * p + C[i];
    }

    (p * rx).fast_as_f64() * sign_scale
}

/// This method on small range searches for nearest zero or extremum.
/// Then picks stored series expansion at the point end evaluates the poly at the point.
#[inline]
pub(crate) fn j1_small_argument_fast(x: f64) -> f64 {
    static SIGN: [f64; 2] = [1., -1.];
    let sign_scale = SIGN[x.is_sign_negative() as usize];
    let x_abs = f64::from_bits(x.to_bits() & 0x7fff_ffff_ffff_ffff);

    // let avg_step = 74.60109 / 47.0;
    // let inv_step = 1.0 / avg_step;

    const INV_STEP: f64 = 0.6300176043004198;

    let fx = x_abs * INV_STEP;
    const J1_ZEROS_COUNT: f64 = (J1_ZEROS.len() - 1) as f64;
    let idx0 = fx.min(J1_ZEROS_COUNT) as usize;
    let idx1 = fx.ceil().min(J1_ZEROS_COUNT) as usize;

    let found_zero0 = DoubleDouble::from_bit_pair(J1_ZEROS[idx0]);
    let found_zero1 = DoubleDouble::from_bit_pair(J1_ZEROS[idx1]);

    let dist0 = (found_zero0.hi - x_abs).abs();
    let dist1 = (found_zero1.hi - x_abs).abs();

    let (found_zero, idx, dist) = if dist0 < dist1 {
        (found_zero0, idx0, dist0)
    } else {
        (found_zero1, idx1, dist1)
    };

    if idx == 0 {
        return j1_maclaurin_series_fast(x);
    }

    let r = DoubleDouble::full_add_f64(-found_zero, x_abs);

    // We hit exact zero, value, better to return it directly
    if dist == 0. {
        return f64::from_bits(J1_ZEROS_VALUE[idx]) * sign_scale;
    }

    let is_zero_too_close = dist.abs() < 1e-3;

    let c = if is_zero_too_close {
        &J1_COEFFS_TAYLOR[idx - 1]
    } else {
        &J1_COEFFS[idx - 1]
    };

    let p = f_polyeval19(
        r.hi,
        f64::from_bits(c[5].1),
        f64::from_bits(c[6].1),
        f64::from_bits(c[7].1),
        f64::from_bits(c[8].1),
        f64::from_bits(c[9].1),
        f64::from_bits(c[10].1),
        f64::from_bits(c[11].1),
        f64::from_bits(c[12].1),
        f64::from_bits(c[13].1),
        f64::from_bits(c[14].1),
        f64::from_bits(c[15].1),
        f64::from_bits(c[16].1),
        f64::from_bits(c[17].1),
        f64::from_bits(c[18].1),
        f64::from_bits(c[19].1),
        f64::from_bits(c[20].1),
        f64::from_bits(c[21].1),
        f64::from_bits(c[22].1),
        f64::from_bits(c[23].1),
    );

    let mut z = DoubleDouble::mul_f64_add(r, p, DoubleDouble::from_bit_pair(c[4]));
    z = DoubleDouble::mul_add(z, r, DoubleDouble::from_bit_pair(c[3]));
    z = DoubleDouble::mul_add(z, r, DoubleDouble::from_bit_pair(c[2]));
    z = DoubleDouble::mul_add(z, r, DoubleDouble::from_bit_pair(c[1]));
    z = DoubleDouble::mul_add(z, r, DoubleDouble::from_bit_pair(c[0]));
    let err = f_fmla(
        z.hi,
        f64::from_bits(0x3c70000000000000), // 2^-56
        f64::from_bits(0x3bf0000000000000), // 2^-64
    );
    let ub = z.hi + (z.lo + err);
    let lb = z.hi + (z.lo - err);

    if ub == lb {
        return z.to_f64() * sign_scale;
    }

    j1_small_argument_dd(sign_scale, r, c)
}

fn j1_small_argument_dd(sign_scale: f64, r: DoubleDouble, c0: &[(u64, u64); 24]) -> f64 {
    let c = &c0[15..];

    let p0 = f_polyeval9(
        r.to_f64(),
        f64::from_bits(c[0].1),
        f64::from_bits(c[1].1),
        f64::from_bits(c[2].1),
        f64::from_bits(c[3].1),
        f64::from_bits(c[4].1),
        f64::from_bits(c[5].1),
        f64::from_bits(c[6].1),
        f64::from_bits(c[7].1),
        f64::from_bits(c[8].1),
    );

    let c = c0;

    let mut p_e = DoubleDouble::mul_f64_add(r, p0, DoubleDouble::from_bit_pair(c[14]));
    p_e = DoubleDouble::mul_add(p_e, r, DoubleDouble::from_bit_pair(c[13]));
    p_e = DoubleDouble::mul_add(p_e, r, DoubleDouble::from_bit_pair(c[12]));
    p_e = DoubleDouble::mul_add(p_e, r, DoubleDouble::from_bit_pair(c[11]));
    p_e = DoubleDouble::mul_add(p_e, r, DoubleDouble::from_bit_pair(c[10]));
    p_e = DoubleDouble::mul_add(p_e, r, DoubleDouble::from_bit_pair(c[9]));
    p_e = DoubleDouble::mul_add(p_e, r, DoubleDouble::from_bit_pair(c[8]));
    p_e = DoubleDouble::mul_add(p_e, r, DoubleDouble::from_bit_pair(c[7]));
    p_e = DoubleDouble::mul_add(p_e, r, DoubleDouble::from_bit_pair(c[6]));
    p_e = DoubleDouble::mul_add(p_e, r, DoubleDouble::from_bit_pair(c[5]));
    p_e = DoubleDouble::mul_add(p_e, r, DoubleDouble::from_bit_pair(c[4]));
    p_e = DoubleDouble::mul_add(p_e, r, DoubleDouble::from_bit_pair(c[3]));
    p_e = DoubleDouble::mul_add(p_e, r, DoubleDouble::from_bit_pair(c[2]));
    p_e = DoubleDouble::mul_add(p_e, r, DoubleDouble::from_bit_pair(c[1]));
    p_e = DoubleDouble::mul_add(p_e, r, DoubleDouble::from_bit_pair(c[0]));

    let p = DoubleDouble::from_exact_add(p_e.hi, p_e.lo);
    let err = f_fmla(
        p.hi,
        f64::from_bits(0x3c10000000000000), // 2^-62
        f64::from_bits(0x3a00000000000000), // 2^-95
    );
    let ub = p.hi + (p.lo + err);
    let lb = p.hi + (p.lo - err);
    if ub != lb {
        return j1_small_argument_path_hard(sign_scale, r, c);
    }
    p.to_f64() * sign_scale
}

#[cold]
#[inline(never)]
fn j1_small_argument_path_hard(sign_scale: f64, r: DoubleDouble, c: &[(u64, u64); 24]) -> f64 {
    let mut p = DoubleDouble::from_bit_pair(c[23]);
    for i in (0..23).rev() {
        p = DoubleDouble::mul_add(r, p, DoubleDouble::from_bit_pair(c[i]));
        p = DoubleDouble::from_exact_add(p.hi, p.lo);
    }
    p.to_f64() * sign_scale
}

/*
   Evaluates:
   J1 = sqrt(2/(PI*x)) * beta(x) * cos(x - 3*PI/4 - alpha(x))
   discarding 1*PI/2 using identities gives:
   J1 = sqrt(2/(PI*x)) * beta(x) * sin(x - PI/4 - alpha(x))

   to avoid squashing small (-PI/4 - alpha(x)) into a large x actual expansion is:

   J1 = sqrt(2/(PI*x)) * beta(x) * sin((x mod 2*PI) - PI/4 - alpha(x))

   This method is required for situations where x*x or 1/(x*x) will overflow
*/
#[cold]
#[inline(never)]
fn j1_asympt_hard(x: f64) -> f64 {
    static SGN: [f64; 2] = [1., -1.];
    let sign_scale = SGN[x.is_sign_negative() as usize];
    let x = x.abs();

    const SQRT_2_OVER_PI: DyadicFloat128 = DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xcc42299e_a1b28468_7e59e280_5d5c7180_u128,
    };

    const MPI_OVER_4: DyadicFloat128 = DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -128,
        mantissa: 0xc90fdaa2_2168c234_c4c6628b_80dc1cd1_u128,
    };

    let x_dyadic = DyadicFloat128::new_from_f64(x);
    let recip = DyadicFloat128::accurate_reciprocal(x);

    let alpha = bessel_1_asympt_alpha_hard(recip);
    let beta = bessel_1_asympt_beta_hard(recip);

    let angle = rem2pi_f128(x_dyadic);

    let x0pi34 = MPI_OVER_4 - alpha;
    let r0 = angle + x0pi34;

    let m_sin = sin_f128_small(r0);

    let z0 = beta * m_sin;
    let r_sqrt = bessel_rsqrt_hard(x, recip);
    let scale = SQRT_2_OVER_PI * r_sqrt;
    let p = scale * z0;
    p.fast_as_f64() * sign_scale
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_j1() {
        assert_eq!(f_j1(73.81695991658546), -0.06531447184607607);
        assert_eq!(f_j1(0.01), 0.004999937500260416);
        assert_eq!(f_j1(0.9), 0.4059495460788057);
        assert_eq!(
            f_j1(162605674999778540000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000.),
            0.000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000008686943178258183
        );
        assert_eq!(f_j1(3.831705970207517), -1.8501090915423025e-15);
        assert_eq!(f_j1(-3.831705970207517), 1.8501090915423025e-15);
        assert_eq!(f_j1(-6.1795701510782757E+307), 8.130935041593236e-155);
        assert_eq!(
            f_j1(0.000000000000000000000000000000000000008827127),
            0.0000000000000000000000000000000000000044135635
        );
        assert_eq!(
            f_j1(-0.000000000000000000000000000000000000008827127),
            -0.0000000000000000000000000000000000000044135635
        );
        assert_eq!(f_j1(5.4), -0.3453447907795863);
        assert_eq!(
            f_j1(77.743162408196766932633181568235159),
            0.09049267898021947
        );
        assert_eq!(
            f_j1(84.027189586293545175976760219782591),
            0.0870430264022591
        );
        assert_eq!(f_j1(f64::NEG_INFINITY), 0.0);
        assert_eq!(f_j1(f64::INFINITY), 0.0);
        assert!(f_j1(f64::NAN).is_nan());
    }
}
