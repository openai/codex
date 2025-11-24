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
use crate::common::{f_fmla, f_fmlaf, set_exponent_f32};
use crate::polyeval::f_polyeval3;

pub(crate) static LOG2_R: [u64; 128] = [
    0x0000000000000000,
    0x3f872c7ba20f7327,
    0x3f9743ee861f3556,
    0x3fa184b8e4c56af8,
    0x3fa77394c9d958d5,
    0x3fad6ebd1f1febfe,
    0x3fb1bb32a600549d,
    0x3fb4c560fe68af88,
    0x3fb7d60496cfbb4c,
    0x3fb960caf9abb7ca,
    0x3fbc7b528b70f1c5,
    0x3fbf9c95dc1d1165,
    0x3fc097e38ce60649,
    0x3fc22dadc2ab3497,
    0x3fc3c6fb650cde51,
    0x3fc494f863b8df35,
    0x3fc633a8bf437ce1,
    0x3fc7046031c79f85,
    0x3fc8a8980abfbd32,
    0x3fc97c1cb13c7ec1,
    0x3fcb2602497d5346,
    0x3fcbfc67a7fff4cc,
    0x3fcdac22d3e441d3,
    0x3fce857d3d361368,
    0x3fd01d9bbcfa61d4,
    0x3fd08bce0d95fa38,
    0x3fd169c05363f158,
    0x3fd1d982c9d52708,
    0x3fd249cd2b13cd6c,
    0x3fd32bfee370ee68,
    0x3fd39de8e1559f6f,
    0x3fd4106017c3eca3,
    0x3fd4f6fbb2cec598,
    0x3fd56b22e6b578e5,
    0x3fd5dfdcf1eeae0e,
    0x3fd6552b49986277,
    0x3fd6cb0f6865c8ea,
    0x3fd7b89f02cf2aad,
    0x3fd8304d90c11fd3,
    0x3fd8a8980abfbd32,
    0x3fd921800924dd3b,
    0x3fd99b072a96c6b2,
    0x3fda8ff971810a5e,
    0x3fdb0b67f4f46810,
    0x3fdb877c57b1b070,
    0x3fdc043859e2fdb3,
    0x3fdc819dc2d45fe4,
    0x3fdcffae611ad12b,
    0x3fdd7e6c0abc3579,
    0x3fddfdd89d586e2b,
    0x3fde7df5fe538ab3,
    0x3fdefec61b011f85,
    0x3fdf804ae8d0cd02,
    0x3fe0014332be0033,
    0x3fe042bd4b9a7c99,
    0x3fe08494c66b8ef0,
    0x3fe0c6caaf0c5597,
    0x3fe1096015dee4da,
    0x3fe14c560fe68af9,
    0x3fe18fadb6e2d3c2,
    0x3fe1d368296b5255,
    0x3fe217868b0c37e8,
    0x3fe25c0a0463beb0,
    0x3fe2a0f3c340705c,
    0x3fe2e644fac04fd8,
    0x3fe2e644fac04fd8,
    0x3fe32bfee370ee68,
    0x3fe37222bb70747c,
    0x3fe3b8b1c68fa6ed,
    0x3fe3ffad4e74f1d6,
    0x3fe44716a2c08262,
    0x3fe44716a2c08262,
    0x3fe48eef19317991,
    0x3fe4d7380dcc422d,
    0x3fe51ff2e30214bc,
    0x3fe5692101d9b4a6,
    0x3fe5b2c3da19723b,
    0x3fe5b2c3da19723b,
    0x3fe5fcdce2727ddb,
    0x3fe6476d98ad990a,
    0x3fe6927781d932a8,
    0x3fe6927781d932a8,
    0x3fe6ddfc2a78fc63,
    0x3fe729fd26b707c8,
    0x3fe7767c12967a45,
    0x3fe7767c12967a45,
    0x3fe7c37a9227e7fb,
    0x3fe810fa51bf65fd,
    0x3fe810fa51bf65fd,
    0x3fe85efd062c656d,
    0x3fe8ad846cf369a4,
    0x3fe8ad846cf369a4,
    0x3fe8fc924c89ac84,
    0x3fe94c287492c4db,
    0x3fe94c287492c4db,
    0x3fe99c48be2063c8,
    0x3fe9ecf50bf43f13,
    0x3fe9ecf50bf43f13,
    0x3fea3e2f4ac43f60,
    0x3fea8ff971810a5e,
    0x3fea8ff971810a5e,
    0x3feae255819f022d,
    0x3feb35458761d479,
    0x3feb35458761d479,
    0x3feb88cb9a2ab521,
    0x3feb88cb9a2ab521,
    0x3febdce9dcc96187,
    0x3fec31a27dd00b4a,
    0x3fec31a27dd00b4a,
    0x3fec86f7b7ea4a89,
    0x3fec86f7b7ea4a89,
    0x3fecdcebd2373995,
    0x3fed338120a6dd9d,
    0x3fed338120a6dd9d,
    0x3fed8aba045b01c8,
    0x3fed8aba045b01c8,
    0x3fede298ec0bac0d,
    0x3fede298ec0bac0d,
    0x3fee3b20546f554a,
    0x3fee3b20546f554a,
    0x3fee9452c8a71028,
    0x3fee9452c8a71028,
    0x3feeee32e2aeccbf,
    0x3feeee32e2aeccbf,
    0x3fef48c34bd1e96f,
    0x3fef48c34bd1e96f,
    0x3fefa406bd2443df,
    0x3ff0000000000000,
];

/// Logarithm of base 2
///
/// Max found ULP 0.4999996
#[inline]
pub fn f_log2f(x: f32) -> f32 {
    let mut x_u = x.to_bits();

    const E_BIAS: u32 = (1u32 << (8 - 1u32)) - 1u32;

    let mut m = -(E_BIAS as i32);
    if x_u < f32::MIN_POSITIVE.to_bits() || x_u > f32::MAX.to_bits() {
        if x == 0.0 {
            return f32::NEG_INFINITY;
        }
        if x_u == 0x80000000u32 {
            return f32::NEG_INFINITY;
        }
        if x.is_sign_negative() && !x.is_nan() {
            return f32::NAN + x;
        }
        // x is +inf or nan
        if x.is_nan() || x.is_infinite() {
            return x + x;
        }
        // Normalize denormal inputs.
        x_u = (x * f64::from_bits(0x4160000000000000) as f32).to_bits();
        m -= 23;
    }

    m = m.wrapping_add(x_u.wrapping_shr(23) as i32);
    let mant = x_u & 0x007F_FFFF;
    let index = mant.wrapping_shr(16);

    x_u = set_exponent_f32(x_u, 0x7F);

    let v;
    let u = f32::from_bits(x_u);

    #[cfg(any(
        all(
            any(target_arch = "x86", target_arch = "x86_64"),
            target_feature = "fma"
        ),
        all(target_arch = "aarch64", target_feature = "neon")
    ))]
    {
        use crate::logs::logf::LOG_REDUCTION_F32;
        v = f_fmlaf(u, f32::from_bits(LOG_REDUCTION_F32.0[index as usize]), -1.0) as f64; // Exact.
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
        v = f_fmla(
            u as f64,
            f64::from_bits(LOG_RANGE_REDUCTION[index as usize]),
            -1.0,
        ); // Exact
    }
    // Degree-5 polynomial approximation of log2 generated by Sollya with:
    // > P = fpminimax(log2(1 + x)/x, 4, [|1, D...|], [-2^-8, 2^-7]);

    let extra_factor = m as f64 + f64::from_bits(LOG2_R[index as usize]);

    const COEFFS: [u64; 5] = [
        0x3ff71547652b8133,
        0xbfe71547652d1e33,
        0x3fdec70a098473de,
        0xbfd7154c5ccdf121,
        0x3fd2514fd90a130a,
    ];
    let v2 = v * v; // Exact
    let c0 = f_fmla(v, f64::from_bits(COEFFS[0]), extra_factor);
    let c1 = f_fmla(v, f64::from_bits(COEFFS[2]), f64::from_bits(COEFFS[1]));
    let c2 = f_fmla(v, f64::from_bits(COEFFS[4]), f64::from_bits(COEFFS[3]));

    let r = f_polyeval3(v2, c0, c1, c2);
    r as f32
}

/// Natural logarithm using FMA
///
/// Max found ULP 0.4999996
#[inline(always)]
#[allow(dead_code)]
pub(crate) fn f_log2fx(x: f32) -> f64 {
    let mut x_u = x.to_bits();

    const E_BIAS: u32 = (1u32 << (8 - 1u32)) - 1u32;

    let mut m = -(E_BIAS as i32);
    if x_u == 0x3f80_0000u32 {
        return 0.0;
    }

    if x_u < f32::MIN_POSITIVE.to_bits() || x_u > f32::MAX.to_bits() {
        if x == 0.0 {
            return f64::NEG_INFINITY;
        }
        if x_u == 0x80000000u32 {
            return f64::NEG_INFINITY;
        }
        if x.is_sign_negative() && !x.is_nan() {
            return f64::NAN + x as f64;
        }
        // x is +inf or nan
        if x.is_nan() || x.is_infinite() {
            return (x + x) as f64;
        }
        // Normalize denormal inputs.
        x_u = (x * f64::from_bits(0x4160000000000000) as f32).to_bits();
        m -= 23;
    }

    m = m.wrapping_add(x_u.wrapping_shr(23) as i32);
    let mant = x_u & 0x007F_FFFF;
    let index = mant.wrapping_shr(16);

    x_u = set_exponent_f32(x_u, 0x7F);

    let v;
    let u = f32::from_bits(x_u);

    #[cfg(any(
        all(
            any(target_arch = "x86", target_arch = "x86_64"),
            target_feature = "fma"
        ),
        all(target_arch = "aarch64", target_feature = "neon")
    ))]
    {
        use crate::logs::logf::LOG_REDUCTION_F32;
        v = f_fmlaf(u, f32::from_bits(LOG_REDUCTION_F32.0[index as usize]), -1.0) as f64; // Exact.
    }
    #[cfg(not(any(
        all(
            any(target_arch = "x86", target_arch = "x86_64"),
            target_feature = "fma"
        ),
        all(target_arch = "aarch64", target_feature = "neon")
    )))]
    {
        use crate::logs::log2::LOG_RANGE_REDUCTION;
        v = f_fmla(
            u as f64,
            f64::from_bits(LOG_RANGE_REDUCTION[index as usize]),
            -1.0,
        ); // Exact
    }
    // Degree-5 polynomial approximation of log2 generated by Sollya with:
    // > P = fpminimax(log2(1 + x)/x, 4, [|1, D...|], [-2^-8, 2^-7]);

    let extra_factor = m as f64 + f64::from_bits(LOG2_R[index as usize]);

    const COEFFS: [u64; 5] = [
        0x3ff71547652b8133,
        0xbfe71547652d1e33,
        0x3fdec70a098473de,
        0xbfd7154c5ccdf121,
        0x3fd2514fd90a130a,
    ];
    let v2 = v * v; // Exact
    let c0 = f_fmla(v, f64::from_bits(COEFFS[0]), extra_factor);
    let c1 = f_fmla(v, f64::from_bits(COEFFS[2]), f64::from_bits(COEFFS[1]));
    let c2 = f_fmla(v, f64::from_bits(COEFFS[4]), f64::from_bits(COEFFS[3]));

    f_polyeval3(v2, c0, c1, c2)
}

/// Dirty log2 using FMA
#[inline(always)]
#[allow(dead_code)]
pub(crate) fn dirty_log2f(d: f32) -> f32 {
    let mut ix = d.to_bits();
    /* reduce x into [sqrt(2)/2, sqrt(2)] */
    ix = ix.wrapping_add(0x3f800000 - 0x3f3504f3);
    let n = (ix >> 23) as i32 - 0x7f;
    ix = (ix & 0x007fffff).wrapping_add(0x3f3504f3);
    let a = f32::from_bits(ix);

    let x = (a - 1.) / (a + 1.);

    let x2 = x * x;
    let mut u = 0.4121985850084821691e+0;
    u = f_fmlaf(u, x2, 0.5770780163490337802e+0);
    u = f_fmlaf(u, x2, 0.9617966939259845749e+0);
    f_fmlaf(x2 * x, u, f_fmlaf(x, 0.2885390081777926802e+1, n as f32))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log2f() {
        assert!((f_log2f(0.35f32) - 0.35f32.log2()).abs() < 1e-5);
        assert!((f_log2f(0.9f32) - 0.9f32.log2()).abs() < 1e-5);
        assert_eq!(f_log2f(0.), f32::NEG_INFINITY);
        assert_eq!(f_log2f(1.0), 0.0);
        assert!(f_log2f(-1.).is_nan());
        assert!(f_log2f(f32::NAN).is_nan());
        assert!(f_log2f(f32::NEG_INFINITY).is_nan());
        assert_eq!(f_log2f(f32::INFINITY), f32::INFINITY);
    }

    #[test]
    fn test_dirty_log2f() {
        assert!((dirty_log2f(0.35f32) - 0.35f32.log2()).abs() < 1e-5);
        assert!((dirty_log2f(0.9f32) - 0.9f32.log2()).abs() < 1e-5);
    }
}
