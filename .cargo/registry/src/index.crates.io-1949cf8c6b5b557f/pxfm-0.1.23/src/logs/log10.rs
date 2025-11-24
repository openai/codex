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
use crate::common::{f_fmla, min_normal_f64};
use crate::double_double::DoubleDouble;
use crate::logs::log2::LOG_COEFFS;
use crate::logs::log10dd::log10_dd;
use crate::logs::log10td::log10_td;
use crate::polyeval::f_polyeval4;

pub(crate) static LOG_R_DD: [(u64, u64); 128] = [
    (0x0000000000000000, 0x0000000000000000),
    (0xbd10c76b999d2be8, 0x3f80101575890000),
    (0xbd23dc5b06e2f7d2, 0x3f90205658938000),
    (0xbd2aa0ba325a0c34, 0x3f98492528c90000),
    (0x3d0111c05cf1d753, 0x3fa0415d89e74000),
    (0xbd2c167375bdfd28, 0x3fa466aed42e0000),
    (0xbd197995d05a267d, 0x3fa894aa149fc000),
    (0xbd1a68f247d82807, 0x3faccb73cdddc000),
    (0xbd17e5dd7009902c, 0x3fb08598b59e4000),
    (0xbd25325d560d9e9b, 0x3fb1973bd1466000),
    (0x3d2cc85ea5db4ed7, 0x3fb3bdf5a7d1e000),
    (0xbd2c69063c5d1d1e, 0x3fb5e95a4d97a000),
    (0x3cec1e8da99ded32, 0x3fb700d30aeac000),
    (0x3d23115c3abd47da, 0x3fb9335e5d594000),
    (0xbd1390802bf768e5, 0x3fbb6ac88dad6000),
    (0x3d2646d1c65aacd3, 0x3fbc885801bc4000),
    (0xbd2dc068afe645e0, 0x3fbec739830a2000),
    (0xbd2534d64fa10afd, 0x3fbfe89139dbe000),
    (0x3d21ef78ce2d07f2, 0x3fc1178e8227e000),
    (0x3d2ca78e44389934, 0x3fc1aa2b7e23f000),
    (0x3d039d6ccb81b4a1, 0x3fc2d1610c868000),
    (0x3cc62fa8234b7289, 0x3fc365fcb0159000),
    (0x3d25837954fdb678, 0x3fc4913d8333b000),
    (0x3d2633e8e5697dc7, 0x3fc527e5e4a1b000),
    (0x3d19cf8b2c3c2e78, 0x3fc6574ebe8c1000),
    (0xbd25118de59c21e1, 0x3fc6f0128b757000),
    (0x3d1e0ddb9a631e83, 0x3fc823c16551a000),
    (0xbd073d54aae92cd1, 0x3fc8beafeb390000),
    (0x3d07f22858a0ff6f, 0x3fc95a5adcf70000),
    (0xbd28724350562169, 0x3fca93ed3c8ae000),
    (0xbd0c358d4eace1aa, 0x3fcb31d8575bd000),
    (0xbd2d4bc4595412b6, 0x3fcbd087383be000),
    (0xbd084a7e75b6f6e4, 0x3fcd1037f2656000),
    (0xbd2aff2af715b035, 0x3fcdb13db0d49000),
    (0x3cc212276041f430, 0x3fce530effe71000),
    (0xbcca211565bb8e11, 0x3fcef5ade4dd0000),
    (0x3d1bcbecca0cdf30, 0x3fcf991c6cb3b000),
    (0x3cf89cdb16ed4e91, 0x3fd07138604d5800),
    (0x3d27188b163ceae9, 0x3fd0c42d67616000),
    (0xbd2c210e63a5f01c, 0x3fd1178e8227e800),
    (0x3d2b9acdf7a51681, 0x3fd16b5ccbacf800),
    (0x3d2ca6ed5147bdb7, 0x3fd1bf99635a6800),
    (0x3d2c93c1df5bb3b6, 0x3fd269621134d800),
    (0x3d2a9cfa4a5004f4, 0x3fd2bef07cdc9000),
    (0xbd28e27ad3213cb8, 0x3fd314f1e1d36000),
    (0x3d116ecdb0f177c8, 0x3fd36b6776be1000),
    (0x3d183b54b606bd5c, 0x3fd3c25277333000),
    (0x3d08e436ec90e09d, 0x3fd419b423d5e800),
    (0xbd2f27ce0967d675, 0x3fd4718dc271c800),
    (0xbd2e20891b0ad8a4, 0x3fd4c9e09e173000),
    (0x3d2ebe708164c759, 0x3fd522ae0738a000),
    (0x3d1fadedee5d40ef, 0x3fd57bf753c8d000),
    (0xbd0a0b2a08a465dc, 0x3fd5d5bddf596000),
    (0xbd2db623e731ae00, 0x3fd630030b3ab000),
    (0x3d20a0d32756eba0, 0x3fd68ac83e9c6800),
    (0x3d1721657c222d87, 0x3fd6e60ee6af1800),
    (0x3d2d8b0949dc60b3, 0x3fd741d876c67800),
    (0x3d29ec7d2efd1778, 0x3fd79e26687cf800),
    (0xbd272090c812566a, 0x3fd7fafa3bd81800),
    (0x3d2fd56f3333778a, 0x3fd85855776dc800),
    (0xbd205ae1e5e70470, 0x3fd8b639a88b3000),
    (0xbd1766b52ee6307d, 0x3fd914a8635bf800),
    (0xbd152313a502d9f0, 0x3fd973a343135800),
    (0xbd26279e10d0c0b0, 0x3fd9d32bea15f000),
    (0x3d23c6457f9d79f5, 0x3fda33440224f800),
    (0x3d23c6457f9d79f5, 0x3fda33440224f800),
    (0x3d1e36f2bea77a5d, 0x3fda93ed3c8ad800),
    (0xbd217cc552774458, 0x3fdaf5295248d000),
    (0x3d1095252d841995, 0x3fdb56fa04462800),
    (0x3d27d85bf40a666d, 0x3fdbb9611b80e000),
    (0x3d2cec807fe8e180, 0x3fdc1c60693fa000),
    (0x3d2cec807fe8e180, 0x3fdc1c60693fa000),
    (0xbd29b6ddc15249ae, 0x3fdc7ff9c7455800),
    (0xbd0797c33ec7a6b0, 0x3fdce42f18064800),
    (0x3d235bafe9a767a8, 0x3fdd490246def800),
    (0xbd1ea42d60dc616a, 0x3fddae75484c9800),
    (0xbd1326b207322938, 0x3fde148a1a272800),
    (0xbd1326b207322938, 0x3fde148a1a272800),
    (0xbd2465505372bd08, 0x3fde7b42c3ddb000),
    (0x3d2f27f45a470251, 0x3fdee2a156b41000),
    (0x3d12cde56f014a8b, 0x3fdf4aa7ee031800),
    (0x3d12cde56f014a8b, 0x3fdf4aa7ee031800),
    (0x3d0085fa3c164935, 0x3fdfb358af7a4800),
    (0xbd053ba3b1727b1c, 0x3fe00e5ae5b20800),
    (0xbd04c45fe79539e0, 0x3fe04360be760400),
    (0xbd04c45fe79539e0, 0x3fe04360be760400),
    (0x3d26812241edf5fd, 0x3fe078bf0533c400),
    (0x3d1f486b887e7e27, 0x3fe0ae76e2d05400),
    (0x3d1f486b887e7e27, 0x3fe0ae76e2d05400),
    (0x3d1c299807801742, 0x3fe0e4898611cc00),
    (0xbd258647bb9ddcb2, 0x3fe11af823c75c00),
    (0xbd258647bb9ddcb2, 0x3fe11af823c75c00),
    (0xbd2edd97a293ae49, 0x3fe151c3f6f29800),
    (0x3d14cc4ef8ab4650, 0x3fe188ee40f23c00),
    (0x3d14cc4ef8ab4650, 0x3fe188ee40f23c00),
    (0x3cccacdeed70e667, 0x3fe1c07849ae6000),
    (0xbd2a7242c9fe81d3, 0x3fe1f8635fc61800),
    (0xbd2a7242c9fe81d3, 0x3fe1f8635fc61800),
    (0x3d12fc066e48667b, 0x3fe230b0d8bebc00),
    (0xbd0b61f105226250, 0x3fe269621134dc00),
    (0xbd0b61f105226250, 0x3fe269621134dc00),
    (0x3d206d2be797882d, 0x3fe2a2786d0ec000),
    (0xbd17a6e507b9dc11, 0x3fe2dbf557b0e000),
    (0xbd17a6e507b9dc11, 0x3fe2dbf557b0e000),
    (0xbd274e93c5a0ed9c, 0x3fe315da44340800),
    (0xbd274e93c5a0ed9c, 0x3fe315da44340800),
    (0x3d10b83f9527e6ac, 0x3fe35028ad9d8c00),
    (0xbd218b7abb5569a4, 0x3fe38ae217197800),
    (0xbd218b7abb5569a4, 0x3fe38ae217197800),
    (0xbd02b7367cfe13c2, 0x3fe3c6080c36c000),
    (0xbd02b7367cfe13c2, 0x3fe3c6080c36c000),
    (0xbd26ce7930f0c74c, 0x3fe4019c2125cc00),
    (0xbcfd984f481051f7, 0x3fe43d9ff2f92400),
    (0xbcfd984f481051f7, 0x3fe43d9ff2f92400),
    (0xbd22cb6af94d60aa, 0x3fe47a1527e8a400),
    (0xbd22cb6af94d60aa, 0x3fe47a1527e8a400),
    (0x3cef7115ed4c541c, 0x3fe4b6fd6f970c00),
    (0x3cef7115ed4c541c, 0x3fe4b6fd6f970c00),
    (0xbd2e6c516d93b8fb, 0x3fe4f45a835a5000),
    (0xbd2e6c516d93b8fb, 0x3fe4f45a835a5000),
    (0x3d05ccc45d257531, 0x3fe5322e26867800),
    (0x3d05ccc45d257531, 0x3fe5322e26867800),
    (0x3d09980bff3303dd, 0x3fe5707a26bb8c00),
    (0x3d09980bff3303dd, 0x3fe5707a26bb8c00),
    (0x3d2dfa63ac10c9fb, 0x3fe5af405c364800),
    (0x3d2dfa63ac10c9fb, 0x3fe5af405c364800),
    (0x3d2202380cda46be, 0x3fe5ee82aa241800),
    (0x0000000000000000, 0x0000000000000000),
];

/// Logarithm of base 10
///
/// Max found ULP 0.5
pub fn f_log10(x: f64) -> f64 {
    let mut x_u = x.to_bits();

    const E_BIAS: u64 = (1u64 << (11 - 1u64)) - 1u64;

    let mut x_e: i64 = -(E_BIAS as i64);

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
    let shifted = (x_u >> 45) as i64;
    let index = shifted & 0x7F;
    let r = f64::from_bits(crate::logs::log2::LOG_RANGE_REDUCTION[index as usize]);

    // Add unbiased exponent. Add an extra 1 if the 8 leading fractional bits are
    // all 1's.
    x_e = x_e.wrapping_add(x_u.wrapping_add(1u64 << 45).wrapping_shr(52) as i64);
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
    let p = f_polyeval4(u_sq, lo, p0, p1, p2);

    // Exact sum:
    //   r1.hi + r1.lo = e_x * log(2)_hi - log(r)_hi + u
    let mut r1 = DoubleDouble::from_exact_add(hi, u);
    r1.lo += p;

    // Quick double-double multiplication:
    //   r2.hi + r2.lo ~ r1 * log10(e),
    // with error bounded by:
    //   4*ulp( ulp(r2.hi) )
    const LOG10_E: DoubleDouble = DoubleDouble::new(
        f64::from_bits(0x3c695355baaafad3),
        f64::from_bits(0x3fdbcb7b1526e50e),
    );
    let r2 = DoubleDouble::quick_mult(r1, LOG10_E);

    const HI_ERR: f64 = f64::from_bits(0x3aa0000000000000);

    // Extra errors from P is from using x^2 to reduce evaluation latency.
    const P_ERR: f64 = f64::from_bits(0x3cc0000000000000);

    // Technicallly error of r1.lo is bounded by:
    //    |hi|*ulp(log(2)_lo) + C*ulp(u^2)
    // To simplify the error computation a bit, we replace |hi|*ulp(log(2)_lo)
    // with the upper bound: 2^11 * ulp(log(2)_lo) = 2^-85.
    // Total error is bounded by ~ C * ulp(u^2) + 2^-85.
    let err = f_fmla(u_sq, P_ERR, HI_ERR);

    // Lower bound from the result
    let left = r2.hi + (r2.lo - err);
    // Upper bound from the result
    let right = r2.hi + (r2.lo + err);

    // Ziv's test if fast pass is accurate enough.
    if left == right {
        return left;
    }

    log10_dd_accurate(x)
}

#[cold]
#[inline(never)]
fn log10_dd_accurate(x: f64) -> f64 {
    let r = log10_dd(x);
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
    log10_dd_accurate_slow(x)
}

#[cold]
#[inline(never)]
fn log10_dd_accurate_slow(x: f64) -> f64 {
    log10_td(x).to_f64()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log10d() {
        assert_eq!(f_log10(0.35), -0.4559319556497244);
        assert_eq!(f_log10(0.9), -0.045757490560675115);
        assert_eq!(f_log10(10.), 1.);
        assert_eq!(f_log10(0.), f64::NEG_INFINITY);
        assert!(f_log10(-1.).is_nan());
        assert!(f_log10(f64::NAN).is_nan());
        assert!(f_log10(f64::NEG_INFINITY).is_nan());
        assert_eq!(f_log10(f64::INFINITY), f64::INFINITY);
    }
}
