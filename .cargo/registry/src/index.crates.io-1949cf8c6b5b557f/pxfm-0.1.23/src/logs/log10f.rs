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
use crate::common::*;
use crate::polyeval::f_polyeval3;

static LOG10_R: [u64; 128] = [
    0x0000000000000000,
    0x3f6be76bd77b4fc3,
    0x3f7c03a80ae5e054,
    0x3f851824c7587eb0,
    0x3f8c3d0837784c41,
    0x3f91b85d6044e9ae,
    0x3f9559bd2406c3ba,
    0x3f9902c31d62a843,
    0x3f9cb38fccd8bfdb,
    0x3f9e8eeb09f2f6cb,
    0x3fa125d0432ea20e,
    0x3fa30838cdc2fbfd,
    0x3fa3faf7c663060e,
    0x3fa5e3966b7e9295,
    0x3fa7d070145f4fd7,
    0x3fa8c878eeb05074,
    0x3faabbcebd84fca0,
    0x3fabb7209d1e24e5,
    0x3fadb11ed766abf4,
    0x3faeafd05035bd3b,
    0x3fb0585283764178,
    0x3fb0d966cc6500fa,
    0x3fb1dd5460c8b16f,
    0x3fb2603072a25f82,
    0x3fb367ba3aaa1883,
    0x3fb3ec6ad5407868,
    0x3fb4f7aad9bbcbaf,
    0x3fb57e3d47c3af7b,
    0x3fb605735ee985f1,
    0x3fb715d0ce367afc,
    0x3fb79efb57b0f803,
    0x3fb828cfed29a215,
    0x3fb93e7de0fc3e80,
    0x3fb9ca5aa1729f45,
    0x3fba56e8325f5c87,
    0x3fbae4285509950b,
    0x3fbb721cd17157e3,
    0x3fbc902a19e65111,
    0x3fbd204698cb42bd,
    0x3fbdb11ed766abf4,
    0x3fbe42b4c16caaf3,
    0x3fbed50a4a26eafc,
    0x3fbffbfc2bbc7803,
    0x3fc0484e4942aa43,
    0x3fc093025a19976c,
    0x3fc0de1b56356b04,
    0x3fc1299a4fb3e306,
    0x3fc175805d1587c1,
    0x3fc1c1ce9955c0c6,
    0x3fc20e8624038fed,
    0x3fc25ba8215af7fc,
    0x3fc2a935ba5f1479,
    0x3fc2f7301cf4e87b,
    0x3fc345987bfeea91,
    0x3fc394700f7953fd,
    0x3fc3e3b8149739d4,
    0x3fc43371cde076c2,
    0x3fc4839e83506c87,
    0x3fc4d43f8275a483,
    0x3fc525561e9256ee,
    0x3fc576e3b0bde0a7,
    0x3fc5c8e998072fe2,
    0x3fc61b6939983048,
    0x3fc66e6400da3f77,
    0x3fc6c1db5f9bb336,
    0x3fc6c1db5f9bb336,
    0x3fc715d0ce367afc,
    0x3fc76a45cbb7e6ff,
    0x3fc7bf3bde099f30,
    0x3fc814b4921bd52b,
    0x3fc86ab17c10bc7f,
    0x3fc86ab17c10bc7f,
    0x3fc8c13437695532,
    0x3fc9183e673394fa,
    0x3fc96fd1b639fc09,
    0x3fc9c7efd734a2f9,
    0x3fca209a84fbcff8,
    0x3fca209a84fbcff8,
    0x3fca79d382bc21d9,
    0x3fcad39c9c2c6080,
    0x3fcb2df7a5c50299,
    0x3fcb2df7a5c50299,
    0x3fcb88e67cf97980,
    0x3fcbe46b087354bc,
    0x3fcc4087384f4f80,
    0x3fcc4087384f4f80,
    0x3fcc9d3d065c5b42,
    0x3fccfa8e765cbb72,
    0x3fccfa8e765cbb72,
    0x3fcd587d96494759,
    0x3fcdb70c7e96e7f3,
    0x3fcdb70c7e96e7f3,
    0x3fce163d527e68cf,
    0x3fce76124046b3f3,
    0x3fce76124046b3f3,
    0x3fced68d819191fc,
    0x3fcf37b15bab08d1,
    0x3fcf37b15bab08d1,
    0x3fcf99801fdb749d,
    0x3fcffbfc2bbc7803,
    0x3fcffbfc2bbc7803,
    0x3fd02f93f4c87101,
    0x3fd06182e84fd4ac,
    0x3fd06182e84fd4ac,
    0x3fd093cc32c90f84,
    0x3fd093cc32c90f84,
    0x3fd0c6711d6abd7a,
    0x3fd0f972f87ff3d6,
    0x3fd0f972f87ff3d6,
    0x3fd12cd31b9c99ff,
    0x3fd12cd31b9c99ff,
    0x3fd16092e5d3a9a6,
    0x3fd194b3bdef6b9e,
    0x3fd194b3bdef6b9e,
    0x3fd1c93712abc7ff,
    0x3fd1c93712abc7ff,
    0x3fd1fe1e5af2c141,
    0x3fd1fe1e5af2c141,
    0x3fd2336b161b3337,
    0x3fd2336b161b3337,
    0x3fd2691ecc29f042,
    0x3fd2691ecc29f042,
    0x3fd29f3b0e15584b,
    0x3fd29f3b0e15584b,
    0x3fd2d5c1760b86bb,
    0x3fd2d5c1760b86bb,
    0x3fd30cb3a7bb3625,
    0x3fd34413509f79ff,
];

/// Logarithm of base 10
///
/// Max found ULP 0.49999943
#[inline]
pub fn f_log10f(x: f32) -> f32 {
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
    let index = (x_u >> 16) & 0x7F;
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
        v = f_fmlaf(
            u,
            f32::from_bits(crate::logs::logf::LOG_REDUCTION_F32.0[index as usize]),
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
        v = f_fmla(
            u as f64,
            f32::from_bits(crate::logs::logf::LOG_REDUCTION_F32.0[index as usize]) as f64,
            -1.0,
        ); // Exact
    }
    // Degree-5 polynomial approximation of log10 generated by:
    // > P = fpminimax(log10(1 + x)/x, 4, [|D...|], [-2^-8, 2^-7]);
    const COEFFS: [u64; 5] = [
        0x3fdbcb7b1526e2e5,
        0xbfcbcb7b1528d43d,
        0x3fc287a77eb4ca0d,
        0xbfbbcb8110a181b5,
        0x3fb60e7e3e747129,
    ];
    let v2 = v * v; // Exact
    let p2 = f_fmla(v, f64::from_bits(COEFFS[4]), f64::from_bits(COEFFS[3]));
    let p1 = f_fmla(v, f64::from_bits(COEFFS[2]), f64::from_bits(COEFFS[1]));
    let p0 = f_fmla(
        v,
        f64::from_bits(COEFFS[0]),
        f64::from_bits(LOG10_R[index as usize]),
    );
    const LOG_10_2: f64 = f64::from_bits(0x3fd34413509f79ff);
    let r = f_fmla(m as f64, LOG_10_2, f_polyeval3(v2, p0, p1, p2));
    r as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log10f() {
        assert_eq!(f_log10f(0.35), -0.45593196f32);
        assert_eq!(f_log10f(0.9), -4.5757502e-2);
        assert_eq!(f_log10f(10.), 1.);
        assert_eq!(f_log10f(100.), 2.);
        assert_eq!(f_log10f(0.), f32::NEG_INFINITY);
        assert!(f_log10f(-1.).is_nan());
        assert!(f_log10f(f32::NAN).is_nan());
        assert!(f_log10f(f32::NEG_INFINITY).is_nan());
        assert_eq!(f_log10f(f32::INFINITY), f32::INFINITY);
    }
}
