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
use crate::bits::min_normal_f32;
use crate::common::*;
use crate::polyeval::f_polyeval3;
use std::hint::black_box;

#[repr(C, align(8))]
pub(crate) struct LogReductionF32Aligned(pub(crate) [u32; 128]);

pub(crate) static LOG_REDUCTION_F32: LogReductionF32Aligned = LogReductionF32Aligned([
    0x3f800000, 0x3f7e0000, 0x3f7c0000, 0x3f7a0000, 0x3f780000, 0x3f760000, 0x3f740000, 0x3f720000,
    0x3f700000, 0x3f6f0000, 0x3f6d0000, 0x3f6b0000, 0x3f6a0000, 0x3f680000, 0x3f660000, 0x3f650000,
    0x3f630000, 0x3f620000, 0x3f600000, 0x3f5f0000, 0x3f5d0000, 0x3f5c0000, 0x3f5a0000, 0x3f590000,
    0x3f570000, 0x3f560000, 0x3f540000, 0x3f530000, 0x3f520000, 0x3f500000, 0x3f4f0000, 0x3f4e0000,
    0x3f4c0000, 0x3f4b0000, 0x3f4a0000, 0x3f490000, 0x3f480000, 0x3f460000, 0x3f450000, 0x3f440000,
    0x3f430000, 0x3f420000, 0x3f400000, 0x3f3f0000, 0x3f3e0000, 0x3f3d0000, 0x3f3c0000, 0x3f3b0000,
    0x3f3a0000, 0x3f390000, 0x3f380000, 0x3f370000, 0x3f360000, 0x3f350000, 0x3f340000, 0x3f330000,
    0x3f320000, 0x3f310000, 0x3f300000, 0x3f2f0000, 0x3f2e0000, 0x3f2d0000, 0x3f2c0000, 0x3f2b0000,
    0x3f2a0000, 0x3f2a0000, 0x3f290000, 0x3f280000, 0x3f270000, 0x3f260000, 0x3f250000, 0x3f250000,
    0x3f240000, 0x3f230000, 0x3f220000, 0x3f210000, 0x3f200000, 0x3f200000, 0x3f1f0000, 0x3f1e0000,
    0x3f1d0000, 0x3f1d0000, 0x3f1c0000, 0x3f1b0000, 0x3f1a0000, 0x3f1a0000, 0x3f190000, 0x3f180000,
    0x3f180000, 0x3f170000, 0x3f160000, 0x3f160000, 0x3f150000, 0x3f140000, 0x3f140000, 0x3f130000,
    0x3f120000, 0x3f120000, 0x3f110000, 0x3f100000, 0x3f100000, 0x3f0f0000, 0x3f0e0000, 0x3f0e0000,
    0x3f0d0000, 0x3f0d0000, 0x3f0c0000, 0x3f0b0000, 0x3f0b0000, 0x3f0a0000, 0x3f0a0000, 0x3f090000,
    0x3f080000, 0x3f080000, 0x3f070000, 0x3f070000, 0x3f060000, 0x3f060000, 0x3f050000, 0x3f050000,
    0x3f040000, 0x3f040000, 0x3f030000, 0x3f030000, 0x3f020000, 0x3f020000, 0x3f010000, 0x3f000000,
]);

static LOG_R: [u64; 128] = [
    0x0000000000000000,
    0x3f8010157588de71,
    0x3f90205658935847,
    0x3f98492528c8cabf,
    0x3fa0415d89e74444,
    0x3fa466aed42de3ea,
    0x3fa894aa149fb343,
    0x3faccb73cdddb2cc,
    0x3fb08598b59e3a07,
    0x3fb1973bd1465567,
    0x3fb3bdf5a7d1ee64,
    0x3fb5e95a4d9791cb,
    0x3fb700d30aeac0e1,
    0x3fb9335e5d594989,
    0x3fbb6ac88dad5b1c,
    0x3fbc885801bc4b23,
    0x3fbec739830a1120,
    0x3fbfe89139dbd566,
    0x3fc1178e8227e47c,
    0x3fc1aa2b7e23f72a,
    0x3fc2d1610c86813a,
    0x3fc365fcb0159016,
    0x3fc4913d8333b561,
    0x3fc527e5e4a1b58d,
    0x3fc6574ebe8c133a,
    0x3fc6f0128b756abc,
    0x3fc823c16551a3c2,
    0x3fc8beafeb38fe8c,
    0x3fc95a5adcf7017f,
    0x3fca93ed3c8ad9e3,
    0x3fcb31d8575bce3d,
    0x3fcbd087383bd8ad,
    0x3fcd1037f2655e7b,
    0x3fcdb13db0d48940,
    0x3fce530effe71012,
    0x3fcef5ade4dcffe6,
    0x3fcf991c6cb3b379,
    0x3fd07138604d5862,
    0x3fd0c42d676162e3,
    0x3fd1178e8227e47c,
    0x3fd16b5ccbacfb73,
    0x3fd1bf99635a6b95,
    0x3fd269621134db92,
    0x3fd2bef07cdc9354,
    0x3fd314f1e1d35ce4,
    0x3fd36b6776be1117,
    0x3fd3c25277333184,
    0x3fd419b423d5e8c7,
    0x3fd4718dc271c41b,
    0x3fd4c9e09e172c3c,
    0x3fd522ae0738a3d8,
    0x3fd57bf753c8d1fb,
    0x3fd5d5bddf595f30,
    0x3fd630030b3aac49,
    0x3fd68ac83e9c6a14,
    0x3fd6e60ee6af1972,
    0x3fd741d876c67bb1,
    0x3fd79e26687cfb3e,
    0x3fd7fafa3bd8151c,
    0x3fd85855776dcbfb,
    0x3fd8b639a88b2df5,
    0x3fd914a8635bf68a,
    0x3fd973a3431356ae,
    0x3fd9d32bea15ed3b,
    0x3fda33440224fa79,
    0x3fda33440224fa79,
    0x3fda93ed3c8ad9e3,
    0x3fdaf5295248cdd0,
    0x3fdb56fa04462909,
    0x3fdbb9611b80e2fb,
    0x3fdc1c60693fa39e,
    0x3fdc1c60693fa39e,
    0x3fdc7ff9c74554c9,
    0x3fdce42f18064743,
    0x3fdd490246defa6b,
    0x3fddae75484c9616,
    0x3fde148a1a2726ce,
    0x3fde148a1a2726ce,
    0x3fde7b42c3ddad73,
    0x3fdee2a156b413e5,
    0x3fdf4aa7ee03192d,
    0x3fdf4aa7ee03192d,
    0x3fdfb358af7a4884,
    0x3fe00e5ae5b207ab,
    0x3fe04360be7603ad,
    0x3fe04360be7603ad,
    0x3fe078bf0533c568,
    0x3fe0ae76e2d054fa,
    0x3fe0ae76e2d054fa,
    0x3fe0e4898611cce1,
    0x3fe11af823c75aa8,
    0x3fe11af823c75aa8,
    0x3fe151c3f6f29612,
    0x3fe188ee40f23ca6,
    0x3fe188ee40f23ca6,
    0x3fe1c07849ae6007,
    0x3fe1f8635fc61659,
    0x3fe1f8635fc61659,
    0x3fe230b0d8bebc98,
    0x3fe269621134db92,
    0x3fe269621134db92,
    0x3fe2a2786d0ec107,
    0x3fe2dbf557b0df43,
    0x3fe2dbf557b0df43,
    0x3fe315da4434068b,
    0x3fe315da4434068b,
    0x3fe35028ad9d8c86,
    0x3fe38ae2171976e7,
    0x3fe38ae2171976e7,
    0x3fe3c6080c36bfb5,
    0x3fe3c6080c36bfb5,
    0x3fe4019c2125ca93,
    0x3fe43d9ff2f923c5,
    0x3fe43d9ff2f923c5,
    0x3fe47a1527e8a2d3,
    0x3fe47a1527e8a2d3,
    0x3fe4b6fd6f970c1f,
    0x3fe4b6fd6f970c1f,
    0x3fe4f45a835a4e19,
    0x3fe4f45a835a4e19,
    0x3fe5322e26867857,
    0x3fe5322e26867857,
    0x3fe5707a26bb8c66,
    0x3fe5707a26bb8c66,
    0x3fe5af405c3649e0,
    0x3fe5af405c3649e0,
    0x3fe5ee82aa241920,
    0x0000000000000000,
];

/// Natural logarithm
///
/// Max found ULP 0.4999988
#[inline]
pub fn f_logf(x: f32) -> f32 {
    let mut x_u = x.to_bits();

    const E_BIAS: u32 = (1u32 << (8 - 1u32)) - 1u32;

    let mut m = -(E_BIAS as i32);
    if x_u < 0x4c5d65a5u32 {
        if x_u == 0x3f7f4d6fu32 {
            return black_box(f64::from_bits(0xbf6659ec80000000) as f32) + min_normal_f32(true);
        } else if x_u == 0x41178febu32 {
            return black_box(f64::from_bits(0x4001fcbce0000000) as f32) + min_normal_f32(true);
        } else if x_u == 0x3f800000u32 {
            return 0.;
        } else if x_u == 0x1e88452du32 {
            return black_box(f64::from_bits(0xc046d7b180000000) as f32) + min_normal_f32(true);
        }
        if x_u < f32::MIN_POSITIVE.to_bits() {
            if x == 0.0 {
                return f32::NEG_INFINITY;
            }
            // Normalize denormal inputs.
            x_u = (x * f64::from_bits(0x4160000000000000) as f32).to_bits();
            m -= 23;
        }
    } else {
        if x_u == 0x4c5d65a5u32 {
            return black_box(f32::from_bits(0x418f034b)) + min_normal_f32(true);
        } else if x_u == 0x65d890d3u32 {
            return black_box(f32::from_bits(0x4254d1f9)) + min_normal_f32(true);
        } else if x_u == 0x6f31a8ecu32 {
            return black_box(f32::from_bits(0x42845a89)) + min_normal_f32(true);
        } else if x_u == 0x7a17f30au32 {
            return black_box(f32::from_bits(0x42a28a1b)) + min_normal_f32(true);
        } else if x_u == 0x500ffb03u32 {
            return black_box(f32::from_bits(0x41b7ee9a)) + min_normal_f32(true);
        } else if x_u == 0x5cd69e88u32 {
            return black_box(f32::from_bits(0x4222e0a3)) + min_normal_f32(true);
        } else if x_u == 0x5ee8984eu32 {
            return black_box(f32::from_bits(0x422e4a21)) + min_normal_f32(true);
        }

        if x_u > f32::MAX.to_bits() {
            if x_u == 0x80000000u32 {
                return f32::NEG_INFINITY;
            }
            if x.is_sign_negative() && !x.is_nan() {
                return f32::NAN + x;
            }
            // x is +inf or nan
            if x.is_nan() {
                return f32::NAN;
            }

            return x;
        }
    }

    let mant = x_u & 0x007F_FFFF;
    // Extract 7 leading fractional bits of the mantissa
    let index = mant.wrapping_shr(16);
    // Add unbiased exponent. Add an extra 1 if the 7 leading fractional bits are
    // all 1's.
    m = m.wrapping_add(x_u.wrapping_add(1 << 16).wrapping_shr(23) as i32);
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
    // Degree-5 polynomial approximation of log generated by Sollya with:
    // > P = fpminimax(log(1 + x)/x, 4, [|1, D...|], [-2^-8, 2^-7]);
    const COEFFS: [u64; 4] = [
        0xbfe000000000fe63,
        0x3fd555556e963c16,
        0xbfd000028dedf986,
        0x3fc966681bfda7f7,
    ];
    let v2 = v * v; // Exact
    let p2 = f_fmla(v, f64::from_bits(COEFFS[3]), f64::from_bits(COEFFS[2]));
    let p1 = f_fmla(v, f64::from_bits(COEFFS[1]), f64::from_bits(COEFFS[0]));
    let p0 = f64::from_bits(LOG_R[index as usize]) + v;
    const LOG_2: f64 = f64::from_bits(0x3fe62e42fefa39ef);
    let r = f_fmla(m as f64, LOG_2, f_polyeval3(v2, p0, p1, p2));
    r as f32
}

#[inline]
pub(crate) fn fast_logf(x: f32) -> f64 {
    let mut x_u = x.to_bits();
    const E_BIAS: u32 = (1u32 << (8 - 1u32)) - 1u32;
    let mut m = -(E_BIAS as i32);
    if x_u < f32::MIN_POSITIVE.to_bits() {
        // Normalize denormal inputs.
        x_u = (x * f64::from_bits(0x4160000000000000) as f32).to_bits();
        m -= 23;
    }

    let mant = x_u & 0x007F_FFFF;
    // Extract 7 leading fractional bits of the mantissa
    let index = mant.wrapping_shr(16);
    // Add unbiased exponent. Add an extra 1 if the 7 leading fractional bits are
    // all 1's.
    m = m.wrapping_add(x_u.wrapping_add(1 << 16).wrapping_shr(23) as i32);
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
    // Degree-5 polynomial approximation of log generated by Sollya with:
    // > P = fpminimax(log(1 + x)/x, 4, [|1, D...|], [-2^-8, 2^-7]);
    const COEFFS: [u64; 4] = [
        0xbfe000000000fe63,
        0x3fd555556e963c16,
        0xbfd000028dedf986,
        0x3fc966681bfda7f7,
    ];
    let v2 = v * v; // Exact
    let p2 = f_fmla(v, f64::from_bits(COEFFS[3]), f64::from_bits(COEFFS[2]));
    let p1 = f_fmla(v, f64::from_bits(COEFFS[1]), f64::from_bits(COEFFS[0]));
    let p0 = f64::from_bits(LOG_R[index as usize]) + v;
    const LOG_2: f64 = f64::from_bits(0x3fe62e42fefa39ef);
    f_fmla(m as f64, LOG_2, f_polyeval3(v2, p0, p1, p2))
}

/// Log for given value for const context.
/// This is simplified version just to make a good approximation on const context.
#[inline]
pub const fn logf(d: f32) -> f32 {
    let ux = d.to_bits();
    #[allow(clippy::collapsible_if)]
    if ux < (1 << 23) || ux >= 0x7f800000u32 {
        if ux == 0 || ux >= 0x7f800000u32 {
            if ux == 0x7f800000u32 {
                return d;
            }
            let ax = ux.wrapping_shl(1);
            if ax == 0u32 {
                // -0.0
                return f32::NEG_INFINITY;
            }
            if ax > 0xff000000u32 {
                return d + d;
            } // nan
            return f32::NAN;
        }
    }

    let mut ix = d.to_bits();
    /* reduce x into [sqrt(2)/2, sqrt(2)] */
    ix += 0x3f800000 - 0x3f3504f3;
    let n = (ix >> 23) as i32 - 0x7f;
    ix = (ix & 0x007fffff) + 0x3f3504f3;
    let a = f32::from_bits(ix) as f64;

    let x = (a - 1.) / (a + 1.);
    let x2 = x * x;
    let mut u = 0.2222220222147750310e+0;
    u = fmla(u, x2, 0.2857142871244668543e+0);
    u = fmla(u, x2, 0.3999999999950960318e+0);
    u = fmla(u, x2, 0.6666666666666734090e+0);
    u = fmla(u, x2, 2.);
    fmla(x, u, std::f64::consts::LN_2 * (n as f64)) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_logf() {
        assert!(
            (logf(1f32) - 0f32).abs() < 1e-6,
            "Invalid result {}",
            logf(1f32)
        );
        assert!(
            (logf(5f32) - 1.60943791243410037460f32).abs() < 1e-6,
            "Invalid result {}",
            logf(5f32)
        );
        assert_eq!(logf(0.), f32::NEG_INFINITY);
        assert!(logf(-1.).is_nan());
        assert!(logf(f32::NAN).is_nan());
        assert!(logf(f32::NEG_INFINITY).is_nan());
        assert_eq!(logf(f32::INFINITY), f32::INFINITY);
    }

    #[test]
    fn test_flogf() {
        assert!(
            (f_logf(1f32) - 0f32).abs() < 1e-6,
            "Invalid result {}",
            f_logf(1f32)
        );
        assert!(
            (f_logf(5f32) - 1.60943791243410037460f32).abs() < 1e-6,
            "Invalid result {}",
            f_logf(5f32)
        );
        assert_eq!(f_logf(0.), f32::NEG_INFINITY);
        assert!(f_logf(-1.).is_nan());
        assert!(f_logf(f32::NAN).is_nan());
        assert!(f_logf(f32::NEG_INFINITY).is_nan());
        assert_eq!(f_logf(f32::INFINITY), f32::INFINITY);
    }
}
