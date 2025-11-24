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
use crate::common::{dyad_fmla, min_normal_f64};
use crate::double_double::DoubleDouble;
use crate::dyadic_float::DyadicFloat128;
use crate::sin::{SinCos, get_sin_k_rational, range_reduction_small, sincos_eval};
use crate::sin_table::SIN_K_PI_OVER_128;
use crate::sincos_dyadic::{range_reduction_small_f128_f128, sincos_eval_dyadic};

#[inline]
fn sin_eval_dd(z: DoubleDouble) -> DoubleDouble {
    const SIN_C: [(u64, u64); 5] = [
        (0x0000000000000000, 0x3ff0000000000000),
        (0xbc65555555555555, 0xbfc5555555555555),
        (0x3c01111111111111, 0x3f81111111111111),
        (0xbb6a01a01a01a01a, 0xbf2a01a01a01a01a),
        (0xbb6c154f8ddc6c00, 0x3ec71de3a556c734),
    ];
    let x2 = DoubleDouble::quick_mult(z, z);
    let mut p = DoubleDouble::mul_add(
        x2,
        DoubleDouble::from_bit_pair(SIN_C[4]),
        DoubleDouble::from_bit_pair(SIN_C[3]),
    );

    p = DoubleDouble::mul_add(x2, p, DoubleDouble::from_bit_pair(SIN_C[2]));
    p = DoubleDouble::mul_add(x2, p, DoubleDouble::from_bit_pair(SIN_C[1]));
    p = DoubleDouble::mul_add(x2, p, DoubleDouble::from_bit_pair(SIN_C[0]));
    DoubleDouble::quick_mult(p, z)
}

pub(crate) fn sin_dd_small(z: DoubleDouble) -> DoubleDouble {
    let x_e = (z.hi.to_bits() >> 52) & 0x7ff;
    const E_BIAS: u64 = (1u64 << (11 - 1u64)) - 1u64;

    if x_e < E_BIAS - 8 {
        return sin_eval_dd(z);
    }

    let (u_f128, k) = range_reduction_small_dd(z);

    let sin_cos = sincos_eval_dd(u_f128);

    // Fast look up version, but needs 256-entry table.
    // cos(k * pi/128) = sin(k * pi/128 + pi/2) = sin((k + 64) * pi/128).
    let sk = SIN_K_PI_OVER_128[(k & 255) as usize];
    let ck = SIN_K_PI_OVER_128[((k.wrapping_add(64)) & 255) as usize];

    let sin_k = DoubleDouble::from_bit_pair(sk);
    let cos_k = DoubleDouble::from_bit_pair(ck);

    let sin_k_cos_y = DoubleDouble::quick_mult(sin_cos.v_cos, sin_k);
    let cos_k_sin_y = DoubleDouble::quick_mult(sin_cos.v_sin, cos_k);

    let mut rr = DoubleDouble::from_exact_add(sin_k_cos_y.hi, cos_k_sin_y.hi);
    rr.lo += sin_k_cos_y.lo + cos_k_sin_y.lo;
    rr
}

pub(crate) fn sin_dd_small_fast(z: DoubleDouble) -> DoubleDouble {
    let x_e = (z.hi.to_bits() >> 52) & 0x7ff;
    const E_BIAS: u64 = (1u64 << (11 - 1u64)) - 1u64;

    if x_e < E_BIAS - 8 {
        return sin_eval_dd(z);
    }

    let (u_f128, k) = range_reduction_small_dd(z);

    let sin_cos = sincos_eval(u_f128);

    // Fast look up version, but needs 256-entry table.
    // cos(k * pi/128) = sin(k * pi/128 + pi/2) = sin((k + 64) * pi/128).
    let sk = SIN_K_PI_OVER_128[(k & 255) as usize];
    let ck = SIN_K_PI_OVER_128[((k.wrapping_add(64)) & 255) as usize];

    let sin_k = DoubleDouble::from_bit_pair(sk);
    let cos_k = DoubleDouble::from_bit_pair(ck);

    let sin_k_cos_y = DoubleDouble::quick_mult(sin_cos.v_cos, sin_k);
    let cos_k_sin_y = DoubleDouble::quick_mult(sin_cos.v_sin, cos_k);

    let mut rr = DoubleDouble::from_exact_add(sin_k_cos_y.hi, cos_k_sin_y.hi);
    rr.lo += sin_k_cos_y.lo + cos_k_sin_y.lo;
    rr
}

#[inline]
fn cos_eval_dd(z: DoubleDouble) -> DoubleDouble {
    let x2 = DoubleDouble::quick_mult(z, z);
    const COS_C: [(u64, u64); 5] = [
        (0x0000000000000000, 0x3ff0000000000000),
        (0x0000000000000000, 0xbfe0000000000000),
        (0x3c45555555555555, 0x3fa5555555555555),
        (0x3bef49f49f49f49f, 0xbf56c16c16c16c17),
        (0x3b3a01a01a01a01a, 0x3efa01a01a01a01a),
    ];

    let mut p = DoubleDouble::mul_add(
        x2,
        DoubleDouble::from_bit_pair(COS_C[4]),
        DoubleDouble::from_bit_pair(COS_C[3]),
    );

    p = DoubleDouble::mul_add(x2, p, DoubleDouble::from_bit_pair(COS_C[2]));
    p = DoubleDouble::mul_add(x2, p, DoubleDouble::from_bit_pair(COS_C[1]));
    p = DoubleDouble::mul_add(x2, p, DoubleDouble::from_bit_pair(COS_C[0]));

    p
}

pub(crate) fn cos_dd_small(z: DoubleDouble) -> DoubleDouble {
    let x_e = (z.hi.to_bits() >> 52) & 0x7ff;
    const E_BIAS: u64 = (1u64 << (11 - 1u64)) - 1u64;

    if x_e < E_BIAS - 8 {
        return cos_eval_dd(z);
    }

    let (u_f128, k) = range_reduction_small_dd(z);

    let sin_cos = sincos_eval_dd(u_f128);

    // cos(k * pi/128) = sin(k * pi/128 + pi/2) = sin((k + 64) * pi/128).
    let sk = SIN_K_PI_OVER_128[(k.wrapping_add(128) & 255) as usize];
    let ck = SIN_K_PI_OVER_128[((k.wrapping_add(64)) & 255) as usize];
    let msin_k = DoubleDouble::from_bit_pair(sk);
    let cos_k = DoubleDouble::from_bit_pair(ck);

    let sin_k_cos_y = DoubleDouble::quick_mult(sin_cos.v_cos, cos_k);
    let cos_k_sin_y = DoubleDouble::quick_mult(sin_cos.v_sin, msin_k);

    let mut rr = DoubleDouble::from_full_exact_add(sin_k_cos_y.hi, cos_k_sin_y.hi);
    rr.lo += sin_k_cos_y.lo + cos_k_sin_y.lo;

    rr
}

pub(crate) fn cos_dd_small_fast(z: DoubleDouble) -> DoubleDouble {
    let x_e = (z.hi.to_bits() >> 52) & 0x7ff;
    const E_BIAS: u64 = (1u64 << (11 - 1u64)) - 1u64;

    if x_e < E_BIAS - 8 {
        return cos_eval_dd(z);
    }

    let (u_f128, k) = range_reduction_small_dd(z);

    let sin_cos = sincos_eval(u_f128);

    // cos(k * pi/128) = sin(k * pi/128 + pi/2) = sin((k + 64) * pi/128).
    let sk = SIN_K_PI_OVER_128[(k.wrapping_add(128) & 255) as usize];
    let ck = SIN_K_PI_OVER_128[((k.wrapping_add(64)) & 255) as usize];
    let msin_k = DoubleDouble::from_bit_pair(sk);
    let cos_k = DoubleDouble::from_bit_pair(ck);

    let sin_k_cos_y = DoubleDouble::quick_mult(sin_cos.v_cos, cos_k);
    let cos_k_sin_y = DoubleDouble::quick_mult(sin_cos.v_sin, msin_k);

    let mut rr = DoubleDouble::from_full_exact_add(sin_k_cos_y.hi, cos_k_sin_y.hi);
    rr.lo += sin_k_cos_y.lo + cos_k_sin_y.lo;

    rr
}

pub(crate) fn sin_f128_small(z: DyadicFloat128) -> DyadicFloat128 {
    let (u_f128, k) = range_reduction_small_f128_f128(z);

    let sin_cos = sincos_eval_dyadic(&u_f128);
    // cos(k * pi/128) = sin(k * pi/128 + pi/2) = sin((k + 64) * pi/128).
    let sin_k_f128 = get_sin_k_rational(k as u64);
    let cos_k_f128 = get_sin_k_rational((k as u64).wrapping_add(64));

    // sin(x) = sin(k * pi/128 + u)
    //        = sin(u) * cos(k*pi/128) + cos(u) * sin(k*pi/128)
    (sin_k_f128 * sin_cos.v_cos) + (cos_k_f128 * sin_cos.v_sin)
}

pub(crate) fn cos_f128_small(z: DyadicFloat128) -> DyadicFloat128 {
    let (u_f128, k) = range_reduction_small_f128_f128(z);

    let sin_cos = sincos_eval_dyadic(&u_f128);
    // -sin(k * pi/128) = sin((k + 128) * pi/128)
    // cos(k * pi/128) = sin(k * pi/128 + pi/2) = sin((k + 64) * pi/128).
    let msin_k_f128 = get_sin_k_rational((k as u64).wrapping_add(128));
    let cos_k_f128 = get_sin_k_rational((k as u64).wrapping_add(64));

    // cos(x) = cos((k * pi/128 + u)
    //        = cos(u) * cos(k*pi/128) - sin(u) * sin(k*pi/128)
    (cos_k_f128 * sin_cos.v_cos) + (msin_k_f128 * sin_cos.v_sin)
}

#[inline]
pub(crate) fn sin_small(z: f64) -> f64 {
    let x_e = (z.to_bits() >> 52) & 0x7ff;
    const E_BIAS: u64 = (1u64 << (11 - 1u64)) - 1u64;

    if x_e < E_BIAS - 26 {
        return dyad_fmla(z, f64::from_bits(0xbc90000000000000), z);
    }

    let (angle_dd, k) = range_reduction_small(z);

    let sin_cos = sincos_eval(angle_dd);

    // Fast look up version, but needs 256-entry table.
    // cos(k * pi/128) = sin(k * pi/128 + pi/2) = sin((k + 64) * pi/128).
    let sk = SIN_K_PI_OVER_128[(k & 255) as usize];
    let ck = SIN_K_PI_OVER_128[((k.wrapping_add(64)) & 255) as usize];

    let sin_k = DoubleDouble::from_bit_pair(sk);
    let cos_k = DoubleDouble::from_bit_pair(ck);

    let sin_k_cos_y = DoubleDouble::quick_mult(sin_cos.v_cos, sin_k);
    let cos_k_sin_y = DoubleDouble::quick_mult(sin_cos.v_sin, cos_k);

    let mut rr = DoubleDouble::from_full_exact_add(sin_k_cos_y.hi, cos_k_sin_y.hi);
    rr.lo += sin_k_cos_y.lo + cos_k_sin_y.lo;
    rr.to_f64()
}

#[inline]
pub(crate) fn cos_small(z: f64) -> f64 {
    let x_e = (z.to_bits() >> 52) & 0x7ff;
    const E_BIAS: u64 = (1u64 << (11 - 1u64)) - 1u64;

    if x_e < E_BIAS - 27 {
        // Signed zeros.
        if z == 0.0 {
            return 1.0;
        }
        // For |x| < 2^-26, |sin(x) - x| < ulp(x)/2.
        return 1.0 - min_normal_f64();
    }

    let (u_f128, k) = range_reduction_small(z);

    let sin_cos = sincos_eval(u_f128);

    // cos(k * pi/128) = sin(k * pi/128 + pi/2) = sin((k + 64) * pi/128).
    let sk = SIN_K_PI_OVER_128[(k.wrapping_add(128) & 255) as usize];
    let ck = SIN_K_PI_OVER_128[((k.wrapping_add(64)) & 255) as usize];
    let msin_k = DoubleDouble::from_bit_pair(sk);
    let cos_k = DoubleDouble::from_bit_pair(ck);

    let sin_k_cos_y = DoubleDouble::quick_mult(sin_cos.v_cos, cos_k);
    let cos_k_sin_y = DoubleDouble::quick_mult(sin_cos.v_sin, msin_k);

    let mut rr = DoubleDouble::from_full_exact_add(sin_k_cos_y.hi, cos_k_sin_y.hi);
    rr.lo += sin_k_cos_y.lo + cos_k_sin_y.lo;
    rr.to_f64()
}

#[inline]
pub(crate) fn sincos_eval_dd(u: DoubleDouble) -> SinCos {
    const SIN_C: [(u64, u64); 5] = [
        (0x0000000000000000, 0x3ff0000000000000),
        (0xbc65555555555555, 0xbfc5555555555555),
        (0x3c01111111111111, 0x3f81111111111111),
        (0xbb6a01a01a01a01a, 0xbf2a01a01a01a01a),
        (0xbb6c154f8ddc6c00, 0x3ec71de3a556c734),
    ];
    let x2 = DoubleDouble::quick_mult(u, u);
    let mut p = DoubleDouble::quick_mul_add(
        x2,
        DoubleDouble::from_bit_pair(SIN_C[4]),
        DoubleDouble::from_bit_pair(SIN_C[3]),
    );

    p = DoubleDouble::quick_mul_add(x2, p, DoubleDouble::from_bit_pair(SIN_C[2]));
    p = DoubleDouble::quick_mul_add(x2, p, DoubleDouble::from_bit_pair(SIN_C[1]));
    p = DoubleDouble::quick_mul_add(x2, p, DoubleDouble::from_bit_pair(SIN_C[0]));
    let sin_u = DoubleDouble::quick_mult(p, u);

    const COS_C: [(u64, u64); 5] = [
        (0x0000000000000000, 0x3ff0000000000000),
        (0x0000000000000000, 0xbfe0000000000000),
        (0x3c45555555555555, 0x3fa5555555555555),
        (0x3bef49f49f49f49f, 0xbf56c16c16c16c17),
        (0x3b3a01a01a01a01a, 0x3efa01a01a01a01a),
    ];

    let mut p = DoubleDouble::quick_mul_add(
        x2,
        DoubleDouble::from_bit_pair(COS_C[4]),
        DoubleDouble::from_bit_pair(COS_C[3]),
    );

    p = DoubleDouble::quick_mul_add(x2, p, DoubleDouble::from_bit_pair(COS_C[2]));
    p = DoubleDouble::quick_mul_add(x2, p, DoubleDouble::from_bit_pair(COS_C[1]));
    p = DoubleDouble::quick_mul_add(x2, p, DoubleDouble::from_bit_pair(COS_C[0]));

    let cos_u = p;
    SinCos {
        v_sin: sin_u,
        v_cos: cos_u,
        err: 0.,
    }
}

#[inline]
pub(crate) fn range_reduction_small_dd(x: DoubleDouble) -> (DoubleDouble, i64) {
    const MPI_OVER_128: [u64; 5] = [
        0xbf9921fb54400000,
        0xbd70b4611a600000,
        0xbb43198a2e000000,
        0xb91b839a25200000,
        0xb6b2704453400000,
    ];
    const ONE_TWENTY_EIGHT_OVER_PI_D: f64 = f64::from_bits(0x40445f306dc9c883);
    let prod_hi = DoubleDouble::quick_mult_f64(x, ONE_TWENTY_EIGHT_OVER_PI_D);
    let kd = prod_hi.to_f64().round();

    let p_hi = f64::from_bits(MPI_OVER_128[0]); // hi
    let p_mid = f64::from_bits(MPI_OVER_128[1]); // mid
    let p_lo = f64::from_bits(MPI_OVER_128[2]); // lo
    let p_lo_lo = f64::from_bits(MPI_OVER_128[3]); // lo_lo

    let mut q = DoubleDouble::f64_mul_f64_add(kd, p_hi, x);
    q = DoubleDouble::f64_mul_f64_add(kd, p_mid, q);
    q = DoubleDouble::f64_mul_f64_add(kd, p_lo, q);
    q = DoubleDouble::f64_mul_f64_add(kd, p_lo_lo, q);

    (q, kd as i64)
}
