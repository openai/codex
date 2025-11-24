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
use crate::bits::{EXP_MASK, get_exponent_f64};
use crate::common::{dyad_fmla, f_fmla};
use crate::double_double::DoubleDouble;
use crate::dyadic_float::DyadicFloat128;
use crate::logs::log1p_dd::log1p_dd;
use crate::logs::log1p_dyadic::log1p_accurate;
use crate::polyeval::f_polyeval4;

// R1[i] = 2^-8 * nearestint( 2^8 / (1 + i * 2^-7) )
pub(crate) static R1: [u64; 129] = [
    0x3ff0000000000000,
    0x3fefc00000000000,
    0x3fef800000000000,
    0x3fef400000000000,
    0x3fef000000000000,
    0x3feec00000000000,
    0x3feea00000000000,
    0x3fee600000000000,
    0x3fee200000000000,
    0x3fede00000000000,
    0x3feda00000000000,
    0x3fed800000000000,
    0x3fed400000000000,
    0x3fed000000000000,
    0x3fece00000000000,
    0x3feca00000000000,
    0x3fec800000000000,
    0x3fec400000000000,
    0x3fec000000000000,
    0x3febe00000000000,
    0x3feba00000000000,
    0x3feb800000000000,
    0x3feb400000000000,
    0x3feb200000000000,
    0x3feb000000000000,
    0x3feac00000000000,
    0x3feaa00000000000,
    0x3fea600000000000,
    0x3fea400000000000,
    0x3fea200000000000,
    0x3fe9e00000000000,
    0x3fe9c00000000000,
    0x3fe9a00000000000,
    0x3fe9800000000000,
    0x3fe9400000000000,
    0x3fe9200000000000,
    0x3fe9000000000000,
    0x3fe8e00000000000,
    0x3fe8a00000000000,
    0x3fe8800000000000,
    0x3fe8600000000000,
    0x3fe8400000000000,
    0x3fe8200000000000,
    0x3fe8000000000000,
    0x3fe7e00000000000,
    0x3fe7a00000000000,
    0x3fe7800000000000,
    0x3fe7600000000000,
    0x3fe7400000000000,
    0x3fe7200000000000,
    0x3fe7000000000000,
    0x3fe6e00000000000,
    0x3fe6c00000000000,
    0x3fe6a00000000000,
    0x3fe6800000000000,
    0x3fe6600000000000,
    0x3fe6400000000000,
    0x3fe6200000000000,
    0x3fe6000000000000,
    0x3fe5e00000000000,
    0x3fe5c00000000000,
    0x3fe5a00000000000,
    0x3fe5800000000000,
    0x3fe5800000000000,
    0x3fe5600000000000,
    0x3fe5400000000000,
    0x3fe5200000000000,
    0x3fe5000000000000,
    0x3fe4e00000000000,
    0x3fe4c00000000000,
    0x3fe4a00000000000,
    0x3fe4a00000000000,
    0x3fe4800000000000,
    0x3fe4600000000000,
    0x3fe4400000000000,
    0x3fe4200000000000,
    0x3fe4200000000000,
    0x3fe4000000000000,
    0x3fe3e00000000000,
    0x3fe3c00000000000,
    0x3fe3c00000000000,
    0x3fe3a00000000000,
    0x3fe3800000000000,
    0x3fe3600000000000,
    0x3fe3600000000000,
    0x3fe3400000000000,
    0x3fe3200000000000,
    0x3fe3000000000000,
    0x3fe3000000000000,
    0x3fe2e00000000000,
    0x3fe2c00000000000,
    0x3fe2c00000000000,
    0x3fe2a00000000000,
    0x3fe2800000000000,
    0x3fe2800000000000,
    0x3fe2600000000000,
    0x3fe2400000000000,
    0x3fe2400000000000,
    0x3fe2200000000000,
    0x3fe2000000000000,
    0x3fe2000000000000,
    0x3fe1e00000000000,
    0x3fe1c00000000000,
    0x3fe1c00000000000,
    0x3fe1a00000000000,
    0x3fe1a00000000000,
    0x3fe1800000000000,
    0x3fe1600000000000,
    0x3fe1600000000000,
    0x3fe1400000000000,
    0x3fe1400000000000,
    0x3fe1200000000000,
    0x3fe1200000000000,
    0x3fe1000000000000,
    0x3fe0e00000000000,
    0x3fe0e00000000000,
    0x3fe0c00000000000,
    0x3fe0c00000000000,
    0x3fe0a00000000000,
    0x3fe0a00000000000,
    0x3fe0800000000000,
    0x3fe0800000000000,
    0x3fe0600000000000,
    0x3fe0600000000000,
    0x3fe0400000000000,
    0x3fe0400000000000,
    0x3fe0200000000000,
    0x3fe0200000000000,
    0x3fe0000000000000,
];

// Extra constants for exact range reduction when FMA instructions are not
// available:
// r * c - 1 for r = 2^-8 * nearestint( 2^8 / (1 + i * 2^-7))
//           and c = 1 + i * 2^-7
//           with i = 0..128.
#[cfg(not(any(
    all(
        any(target_arch = "x86", target_arch = "x86_64"),
        target_feature = "fma"
    ),
    all(target_arch = "aarch64", target_feature = "neon")
)))]
pub(crate) static RCM1: [u64; 129] = [
    0x0000000000000000,
    0xbf10000000000000,
    0xbf30000000000000,
    0xbf42000000000000,
    0xbf50000000000000,
    0xbf59000000000000,
    0x3f5f000000000000,
    0x3f52800000000000,
    0x3f30000000000000,
    0xbf49000000000000,
    0xbf5f000000000000,
    0x3f52000000000000,
    0xbf30000000000000,
    0xbf5c000000000000,
    0x3f51000000000000,
    0xbf45000000000000,
    0x3f60000000000000,
    0x3f10000000000000,
    0xbf60000000000000,
    0x3f3a000000000000,
    0xbf5e000000000000,
    0x3f38000000000000,
    0xbf61000000000000,
    0xbf00000000000000,
    0x3f60000000000000,
    0xbf4a000000000000,
    0x3f51000000000000,
    0xbf5f800000000000,
    0xbf30000000000000,
    0x3f56800000000000,
    0xbf5f000000000000,
    0xbf3c000000000000,
    0x3f50000000000000,
    0x3f63000000000000,
    0xbf56000000000000,
    0xbf24000000000000,
    0x3f50000000000000,
    0x3f60c00000000000,
    0xbf60800000000000,
    0xbf52000000000000,
    0xbf30000000000000,
    0x3f42000000000000,
    0x3f55000000000000,
    0x3f60000000000000,
    0x3f65000000000000,
    0xbf61c00000000000,
    0xbf5c000000000000,
    0xbf55800000000000,
    0xbf50000000000000,
    0xbf47000000000000,
    0xbf40000000000000,
    0xbf36000000000000,
    0xbf30000000000000,
    0xbf2c000000000000,
    0xbf30000000000000,
    0xbf36000000000000,
    0xbf40000000000000,
    0xbf47000000000000,
    0xbf50000000000000,
    0xbf55800000000000,
    0xbf5c000000000000,
    0xbf61c00000000000,
    0xbf66000000000000,
    0x3f65000000000000,
    0x3f60000000000000,
    0x3f55000000000000,
    0x3f42000000000000,
    0xbf30000000000000,
    0xbf52000000000000,
    0xbf60800000000000,
    0xbf68800000000000,
    0x3f60c00000000000,
    0x3f50000000000000,
    0xbf24000000000000,
    0xbf56000000000000,
    0xbf65400000000000,
    0x3f63000000000000,
    0x3f50000000000000,
    0xbf3c000000000000,
    0xbf5f000000000000,
    0x3f68000000000000,
    0x3f56800000000000,
    0xbf30000000000000,
    0xbf5f800000000000,
    0x3f67000000000000,
    0x3f51000000000000,
    0xbf4a000000000000,
    0xbf66000000000000,
    0x3f60000000000000,
    0xbf00000000000000,
    0xbf61000000000000,
    0x3f64800000000000,
    0x3f38000000000000,
    0xbf5e000000000000,
    0x3f66000000000000,
    0x3f3a000000000000,
    0xbf60000000000000,
    0x3f64800000000000,
    0x3f10000000000000,
    0xbf64000000000000,
    0x3f60000000000000,
    0xbf45000000000000,
    0xbf6b000000000000,
    0x3f51000000000000,
    0xbf5c000000000000,
    0x3f65400000000000,
    0xbf30000000000000,
    0xbf69c00000000000,
    0x3f52000000000000,
    0xbf5f000000000000,
    0x3f63000000000000,
    0xbf49000000000000,
    0x3f6c000000000000,
    0x3f30000000000000,
    0xbf68800000000000,
    0x3f52800000000000,
    0xbf62000000000000,
    0x3f5f000000000000,
    0xbf59000000000000,
    0x3f64c00000000000,
    0xbf50000000000000,
    0x3f69000000000000,
    0xbf42000000000000,
    0x3f6c400000000000,
    0xbf30000000000000,
    0x3f6e800000000000,
    0xbf10000000000000,
    0x3f6fc00000000000,
    0x0000000000000000,
];

pub(crate) static LOG_R1_DD: [(u64, u64); 129] = [
    (0x0000000000000000, 0x0000000000000000),
    (0xbd10c76b999d2be8, 0x3f80101575890000),
    (0xbd23dc5b06e2f7d2, 0x3f90205658938000),
    (0xbd2aa0ba325a0c34, 0x3f98492528c90000),
    (0x3d0111c05cf1d753, 0x3fa0415d89e74000),
    (0xbd2c167375bdfd28, 0x3fa466aed42e0000),
    (0xbd029efbec19afa2, 0x3fa67c94f2d4c000),
    (0x3d20fc1a353bb42e, 0x3faaaef2d0fb0000),
    (0xbd0e113e4fc93b7b, 0x3faeea31c006c000),
    (0xbd25325d560d9e9b, 0x3fb1973bd1466000),
    (0x3d2cc85ea5db4ed7, 0x3fb3bdf5a7d1e000),
    (0xbcf53a2582f4e1ef, 0x3fb4d3115d208000),
    (0x3cec1e8da99ded32, 0x3fb700d30aeac000),
    (0x3d23115c3abd47da, 0x3fb9335e5d594000),
    (0xbd0e42b6b94407c8, 0x3fba4e7640b1c000),
    (0x3d2646d1c65aacd3, 0x3fbc885801bc4000),
    (0x3d1a89401fa71733, 0x3fbda72763844000),
    (0xbd2534d64fa10afd, 0x3fbfe89139dbe000),
    (0x3d21ef78ce2d07f2, 0x3fc1178e8227e000),
    (0x3d2ca78e44389934, 0x3fc1aa2b7e23f000),
    (0x3d039d6ccb81b4a1, 0x3fc2d1610c868000),
    (0x3cc62fa8234b7289, 0x3fc365fcb0159000),
    (0x3d25837954fdb678, 0x3fc4913d8333b000),
    (0x3d2633e8e5697dc7, 0x3fc527e5e4a1b000),
    (0xbd127023eb68981c, 0x3fc5bf406b544000),
    (0xbd25118de59c21e1, 0x3fc6f0128b757000),
    (0xbd1c661070914305, 0x3fc7898d85445000),
    (0xbd073d54aae92cd1, 0x3fc8beafeb390000),
    (0x3d07f22858a0ff6f, 0x3fc95a5adcf70000),
    (0x3d29904d6865817a, 0x3fc9f6c407089000),
    (0xbd0c358d4eace1aa, 0x3fcb31d8575bd000),
    (0xbd2d4bc4595412b6, 0x3fcbd087383be000),
    (0xbcf1ec72c5962bd2, 0x3fcc6ffbc6f01000),
    (0xbd084a7e75b6f6e4, 0x3fcd1037f2656000),
    (0x3cc212276041f430, 0x3fce530effe71000),
    (0xbcca211565bb8e11, 0x3fcef5ade4dd0000),
    (0x3d1bcbecca0cdf30, 0x3fcf991c6cb3b000),
    (0xbd16f08c1485e94a, 0x3fd01eae5626c800),
    (0x3d27188b163ceae9, 0x3fd0c42d67616000),
    (0xbd2c210e63a5f01c, 0x3fd1178e8227e800),
    (0x3d2b9acdf7a51681, 0x3fd16b5ccbacf800),
    (0x3d2ca6ed5147bdb7, 0x3fd1bf99635a6800),
    (0x3d0a87deba46baea, 0x3fd214456d0eb800),
    (0x3d2c93c1df5bb3b6, 0x3fd269621134d800),
    (0x3d2a9cfa4a5004f4, 0x3fd2bef07cdc9000),
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
    (0xbd152313a502d9f0, 0x3fd973a343135800),
    (0xbd26279e10d0c0b0, 0x3fd9d32bea15f000),
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
    (0xbd1ea42d60dc616a, 0x3fddae75484c9800),
    (0xbd1326b207322938, 0x3fde148a1a272800),
    (0xbd2465505372bd08, 0x3fde7b42c3ddb000),
    (0x3d2f27f45a470251, 0x3fdee2a156b41000),
    (0x3d2f27f45a470251, 0x3fdee2a156b41000),
    (0x3d12cde56f014a8b, 0x3fdf4aa7ee031800),
    (0x3d0085fa3c164935, 0x3fdfb358af7a4800),
    (0xbd053ba3b1727b1c, 0x3fe00e5ae5b20800),
    (0xbd053ba3b1727b1c, 0x3fe00e5ae5b20800),
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
    (0xbd26ce7930f0c74c, 0x3fe4019c2125cc00),
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
    (0x3d2202380cda46be, 0x3fe5ee82aa241800),
    (0x0000000000000000, 0x0000000000000000),
];

#[inline]
pub(crate) fn log1p_f64_dyadic(x: f64) -> DyadicFloat128 {
    let mut x_u = x.to_bits();

    let mut x_dd = DoubleDouble::default();

    let x_exp: u16 = ((x_u >> 52) & 0x7ff) as u16;

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

    const EXP_BIAS: u16 = (1u16 << (11 - 1u16)) - 1u16;

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

    log1p_accurate(x_e, idx as usize, v_dd)
}

/// Computes log(x+1)
///
/// Max ULP 0.5
pub fn f_log1p(x: f64) -> f64 {
    let mut x_u = x.to_bits();

    let mut x_dd = DoubleDouble::default();

    let x_exp: u16 = ((x_u >> 52) & 0x7ff) as u16;

    const EXP_BIAS: u16 = (1u16 << (11 - 1u16)) - 1u16;

    let e = (((x_u >> 52) & 0x7ff) as i32).wrapping_sub(0x3ff);

    if e == 0x400 || x == 0. || x <= -1.0 {
        /* case NaN/Inf, +/-0 or x <= -1 */
        if e == 0x400 && x.to_bits() != 0xfffu64 << 52 {
            /* NaN or + Inf*/
            return x + x;
        }
        if x <= -1.0
        /* we use the fact that NaN < -1 is false */
        {
            /* log2p(x<-1) is NaN, log2p(-1) is -Inf and raises DivByZero */
            return if x < -1.0 {
                f64::NAN
            } else {
                // x=-1
                f64::NEG_INFINITY
            };
        }
        return x + x; /* +/-0 */
    }

    let ax = x_u.wrapping_shl(1);

    if ax < 0x7f60000000000000u64 {
        // |x| < 0.0625
        // check case x tiny first to avoid spurious underflow in x*x
        if ax < 0x7940000000000000u64 {
            // |x| < 0x1p-53
            if ax == 0 {
                return x;
            }
            /* we have underflow when |x| < 2^-1022, or when |x| = 2^-1022 and
            the result is smaller than 2^-1022 in absolute value */
            let res = dyad_fmla(x.abs(), f64::from_bits(0xbc90000000000000), x);
            return res;
        }
    }

    if x_exp >= EXP_BIAS {
        // |x| >= 1
        if x_u >= 0x4650_0000_0000_0000u64 {
            x_dd.hi = x;
        } else {
            x_dd = DoubleDouble::from_exact_add(x, 1.0);
        }
    } else {
        // |x| < 1
        if x_exp < EXP_BIAS - 52 - 1 {
            // Quick return when |x| < 2^-53.
            // Since log(1 + x) = x - x^2/2 + x^3/3 - ...,
            // for |x| < 2^-53,
            //   x > log(1 + x) > x - x^2 > x(1 - 2^-54) > x - ulp(x)/2
            // Thus,
            //   log(1 + x) = nextafter(x, -inf) for FE_DOWNWARD, or
            //                                       FE_TOWARDZERO and x > 0,
            //              = x                  otherwise.
            if x == 0.0 {
                return x + x;
            }

            let tp = 1.0f32;
            let tn = -1.0f32;
            let rdp = tp - f32::from_bits(0x3d594caf) != tp;
            let rdn = tn - f32::from_bits(0x33800000) != tn;

            if x > 0. && rdp {
                return f64::from_bits(x_u - 1);
            }

            if x < 0. && rdn {
                return f64::from_bits(x_u + 1);
            }

            return if x + x == 0.0 { x + x } else { x };
        }
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
    let r1 = DoubleDouble::from_exact_add(hi, v_dd.hi);

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

    let left = r1.hi + (p - err);
    let right = r1.hi + (p + err);
    // Ziv's test to see if fast pass is accurate enough.
    if left == right {
        return left;
    }
    log1p_accurate_dd(x)
}

#[cold]
fn log1p_accurate_dd(x: f64) -> f64 {
    log1p_dd(x).to_f64()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log1p() {
        assert_eq!(
            f_log1p(-0.0000000000000003834186599935256),
            -0.00000000000000038341865999352564
        );
        assert_eq!(f_log1p(0.000032417476177515677), 0.000032416950742490306);
        assert_eq!(f_log1p(-0.0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001866527236137164),
                   -0.0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001866527236137164);
        assert_eq!(f_log1p(-0.0016481876264151651), -0.0016495473819346394);
        assert_eq!(f_log1p(-0.55), -0.7985076962177717);
        assert_eq!(f_log1p(-0.65), -1.0498221244986778);
        assert_eq!(f_log1p(1.65), 0.9745596399981308);
        assert!(f_log1p(-2.).is_nan());
    }
}
