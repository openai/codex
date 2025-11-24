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
use crate::bits::{get_exponent_f64, set_exponent_f64};
use crate::common::f_fmla;
use crate::polyeval::{f_polyeval4, f_polyeval10};

// Lookup table for (1/f) where f = 1 + n*2^(-7), n = 0..127.
static ONE_OVER_F: [u64; 128] = [
    0x3ff0000000000000,
    0x3fefc07f01fc07f0,
    0x3fef81f81f81f820,
    0x3fef44659e4a4271,
    0x3fef07c1f07c1f08,
    0x3feecc07b301ecc0,
    0x3fee9131abf0b767,
    0x3fee573ac901e574,
    0x3fee1e1e1e1e1e1e,
    0x3fede5d6e3f8868a,
    0x3fedae6076b981db,
    0x3fed77b654b82c34,
    0x3fed41d41d41d41d,
    0x3fed0cb58f6ec074,
    0x3fecd85689039b0b,
    0x3feca4b3055ee191,
    0x3fec71c71c71c71c,
    0x3fec3f8f01c3f8f0,
    0x3fec0e070381c0e0,
    0x3febdd2b899406f7,
    0x3febacf914c1bad0,
    0x3feb7d6c3dda338b,
    0x3feb4e81b4e81b4f,
    0x3feb2036406c80d9,
    0x3feaf286bca1af28,
    0x3feac5701ac5701b,
    0x3fea98ef606a63be,
    0x3fea6d01a6d01a6d,
    0x3fea41a41a41a41a,
    0x3fea16d3f97a4b02,
    0x3fe9ec8e951033d9,
    0x3fe9c2d14ee4a102,
    0x3fe999999999999a,
    0x3fe970e4f80cb872,
    0x3fe948b0fcd6e9e0,
    0x3fe920fb49d0e229,
    0x3fe8f9c18f9c18fa,
    0x3fe8d3018d3018d3,
    0x3fe8acb90f6bf3aa,
    0x3fe886e5f0abb04a,
    0x3fe8618618618618,
    0x3fe83c977ab2bedd,
    0x3fe8181818181818,
    0x3fe7f405fd017f40,
    0x3fe7d05f417d05f4,
    0x3fe7ad2208e0ecc3,
    0x3fe78a4c8178a4c8,
    0x3fe767dce434a9b1,
    0x3fe745d1745d1746,
    0x3fe724287f46debc,
    0x3fe702e05c0b8170,
    0x3fe6e1f76b4337c7,
    0x3fe6c16c16c16c17,
    0x3fe6a13cd1537290,
    0x3fe6816816816817,
    0x3fe661ec6a5122f9,
    0x3fe642c8590b2164,
    0x3fe623fa77016240,
    0x3fe6058160581606,
    0x3fe5e75bb8d015e7,
    0x3fe5c9882b931057,
    0x3fe5ac056b015ac0,
    0x3fe58ed2308158ed,
    0x3fe571ed3c506b3a,
    0x3fe5555555555555,
    0x3fe5390948f40feb,
    0x3fe51d07eae2f815,
    0x3fe5015015015015,
    0x3fe4e5e0a72f0539,
    0x3fe4cab88725af6e,
    0x3fe4afd6a052bf5b,
    0x3fe49539e3b2d067,
    0x3fe47ae147ae147b,
    0x3fe460cbc7f5cf9a,
    0x3fe446f86562d9fb,
    0x3fe42d6625d51f87,
    0x3fe4141414141414,
    0x3fe3fb013fb013fb,
    0x3fe3e22cbce4a902,
    0x3fe3c995a47babe7,
    0x3fe3b13b13b13b14,
    0x3fe3991c2c187f63,
    0x3fe3813813813814,
    0x3fe3698df3de0748,
    0x3fe3521cfb2b78c1,
    0x3fe33ae45b57bcb2,
    0x3fe323e34a2b10bf,
    0x3fe30d190130d190,
    0x3fe2f684bda12f68,
    0x3fe2e025c04b8097,
    0x3fe2c9fb4d812ca0,
    0x3fe2b404ad012b40,
    0x3fe29e4129e4129e,
    0x3fe288b01288b013,
    0x3fe27350b8812735,
    0x3fe25e22708092f1,
    0x3fe2492492492492,
    0x3fe23456789abcdf,
    0x3fe21fb78121fb78,
    0x3fe20b470c67c0d9,
    0x3fe1f7047dc11f70,
    0x3fe1e2ef3b3fb874,
    0x3fe1cf06ada2811d,
    0x3fe1bb4a4046ed29,
    0x3fe1a7b9611a7b96,
    0x3fe19453808ca29c,
    0x3fe1811811811812,
    0x3fe16e0689427379,
    0x3fe15b1e5f75270d,
    0x3fe1485f0e0acd3b,
    0x3fe135c81135c811,
    0x3fe12358e75d3033,
    0x3fe1111111111111,
    0x3fe0fef010fef011,
    0x3fe0ecf56be69c90,
    0x3fe0db20a88f4696,
    0x3fe0c9714fbcda3b,
    0x3fe0b7e6ec259dc8,
    0x3fe0a6810a6810a7,
    0x3fe0953f39010954,
    0x3fe0842108421084,
    0x3fe073260a47f7c6,
    0x3fe0624dd2f1a9fc,
    0x3fe05197f7d73404,
    0x3fe0410410410410,
    0x3fe03091b51f5e1a,
    0x3fe0204081020408,
    0x3fe0101010101010,
];

// Lookup table for log(f) = log(1 + n*2^(-7)) where n = 0..127.
static LOG_F: [u64; 128] = [
    0x0000000000000000,
    0x3f7fe02a6b106788,
    0x3f8fc0a8b0fc03e3,
    0x3f97b91b07d5b11a,
    0x3f9f829b0e783300,
    0x3fa39e87b9febd5f,
    0x3fa77458f632dcfc,
    0x3fab42dd711971be,
    0x3faf0a30c01162a6,
    0x3fb16536eea37ae0,
    0x3fb341d7961bd1d0,
    0x3fb51b073f06183f,
    0x3fb6f0d28ae56b4b,
    0x3fb8c345d6319b20,
    0x3fba926d3a4ad563,
    0x3fbc5e548f5bc743,
    0x3fbe27076e2af2e5,
    0x3fbfec9131dbeaba,
    0x3fc0d77e7cd08e59,
    0x3fc1b72ad52f67a0,
    0x3fc29552f81ff523,
    0x3fc371fc201e8f74,
    0x3fc44d2b6ccb7d1e,
    0x3fc526e5e3a1b437,
    0x3fc5ff3070a793d3,
    0x3fc6d60fe719d21c,
    0x3fc7ab890210d909,
    0x3fc87fa06520c910,
    0x3fc9525a9cf456b4,
    0x3fca23bc1fe2b563,
    0x3fcaf3c94e80bff2,
    0x3fcbc286742d8cd6,
    0x3fcc8ff7c79a9a21,
    0x3fcd5c216b4fbb91,
    0x3fce27076e2af2e5,
    0x3fcef0adcbdc5936,
    0x3fcfb9186d5e3e2a,
    0x3fd0402594b4d040,
    0x3fd0a324e27390e3,
    0x3fd1058bf9ae4ad5,
    0x3fd1675cababa60e,
    0x3fd1c898c16999fa,
    0x3fd22941fbcf7965,
    0x3fd2895a13de86a3,
    0x3fd2e8e2bae11d30,
    0x3fd347dd9a987d54,
    0x3fd3a64c556945e9,
    0x3fd404308686a7e3,
    0x3fd4618bc21c5ec2,
    0x3fd4be5f957778a0,
    0x3fd51aad872df82d,
    0x3fd5767717455a6c,
    0x3fd5d1bdbf5809ca,
    0x3fd62c82f2b9c795,
    0x3fd686c81e9b14ae,
    0x3fd6e08eaa2ba1e3,
    0x3fd739d7f6bbd006,
    0x3fd792a55fdd47a2,
    0x3fd7eaf83b82afc3,
    0x3fd842d1da1e8b17,
    0x3fd89a3386c1425a,
    0x3fd8f11e873662c7,
    0x3fd947941c2116fa,
    0x3fd99d958117e08a,
    0x3fd9f323ecbf984b,
    0x3fda484090e5bb0a,
    0x3fda9cec9a9a0849,
    0x3fdaf1293247786b,
    0x3fdb44f77bcc8f62,
    0x3fdb9858969310fb,
    0x3fdbeb4d9da71b7b,
    0x3fdc3dd7a7cdad4d,
    0x3fdc8ff7c79a9a21,
    0x3fdce1af0b85f3eb,
    0x3fdd32fe7e00ebd5,
    0x3fdd83e7258a2f3e,
    0x3fddd46a04c1c4a0,
    0x3fde24881a7c6c26,
    0x3fde744261d68787,
    0x3fdec399d2468cc0,
    0x3fdf128f5faf06ec,
    0x3fdf6123fa7028ac,
    0x3fdfaf588f78f31e,
    0x3fdffd2e0857f498,
    0x3fe02552a5a5d0fe,
    0x3fe04bdf9da926d2,
    0x3fe0723e5c1cdf40,
    0x3fe0986f4f573520,
    0x3fe0be72e4252a82,
    0x3fe0e44985d1cc8b,
    0x3fe109f39e2d4c96,
    0x3fe12f719593efbc,
    0x3fe154c3d2f4d5e9,
    0x3fe179eabbd899a0,
    0x3fe19ee6b467c96e,
    0x3fe1c3b81f713c24,
    0x3fe1e85f5e7040d0,
    0x3fe20cdcd192ab6d,
    0x3fe23130d7bebf42,
    0x3fe2555bce98f7cb,
    0x3fe2795e1289b11a,
    0x3fe29d37fec2b08a,
    0x3fe2c0e9ed448e8b,
    0x3fe2e47436e40268,
    0x3fe307d7334f10be,
    0x3fe32b1339121d71,
    0x3fe34e289d9ce1d3,
    0x3fe37117b54747b5,
    0x3fe393e0d3562a19,
    0x3fe3b68449fffc22,
    0x3fe3d9026a7156fa,
    0x3fe3fb5b84d16f42,
    0x3fe41d8fe84672ae,
    0x3fe43f9fe2f9ce67,
    0x3fe4618bc21c5ec2,
    0x3fe48353d1ea88df,
    0x3fe4a4f85db03ebb,
    0x3fe4c679afccee39,
    0x3fe4e7d811b75bb0,
    0x3fe50913cc01686b,
    0x3fe52a2d265bc5aa,
    0x3fe54b2467999497,
    0x3fe56bf9d5b3f399,
    0x3fe58cadb5cd7989,
    0x3fe5ad404c359f2c,
    0x3fe5cdb1dc6c1764,
    0x3fe5ee02a9241675,
    0x3fe60e32f44788d8,
];

#[inline]
pub(crate) fn log_eval(x: f64) -> f64 {
    let ex = get_exponent_f64(x);

    // p1 is the leading 7 bits of mx, i.e.
    // p1 * 2^(-7) <= m_x < (p1 + 1) * 2^(-7).
    let p1 = ((x.to_bits() & ((1u64 << 52) - 1)) >> (52 - 7)) as i32;

    // Set bs to (1 + (mx - p1*2^(-7))
    let mut bs = x.to_bits() & (((1u64 << 52) - 1) >> 7);
    const EXP_BIAS: u64 = (1u64 << (11 - 1u64)) - 1u64;
    bs = set_exponent_f64(bs, EXP_BIAS);
    // dx = (mx - p1*2^(-7)) / (1 + p1*2^(-7)).
    let dx = (f64::from_bits(bs) - 1.0) * f64::from_bits(ONE_OVER_F[p1 as usize]);

    // Minimax polynomial of log(1 + dx) generated by Sollya with:
    // > P = fpminimax(log(1 + x)/x, 6, [|D...|], [0, 2^-7]);
    const COEFFS: [u64; 6] = [
        0xbfdffffffffffffc,
        0x3fd5555555552dde,
        0xbfcffffffefe562d,
        0x3fc9999817d3a50f,
        0xbfc554317b3f67a5,
        0x3fc1dc5c45e09c18,
    ];
    let dx2 = dx * dx;
    let c1 = f_fmla(dx, f64::from_bits(COEFFS[1]), f64::from_bits(COEFFS[0]));
    let c2 = f_fmla(dx, f64::from_bits(COEFFS[3]), f64::from_bits(COEFFS[2]));
    let c3 = f_fmla(dx, f64::from_bits(COEFFS[5]), f64::from_bits(COEFFS[4]));

    let p = f_polyeval4(dx2, dx, c1, c2, c3);
    f_fmla(
        ex as f64,
        /*log(2)*/ f64::from_bits(0x3fe62e42fefa39ef),
        f64::from_bits(LOG_F[p1 as usize]) + p,
    )
}

/// Hyperbolic arcsine function
///
/// Max ULP 0.5
#[inline]
pub fn f_asinhf(x: f32) -> f32 {
    let x_u = x.to_bits();
    let x_abs = x_u & 0x7fff_ffff;

    // |x| <= 2^-3
    if x_abs <= 0x3e80_0000u32 {
        // |x| <= 2^-26
        if x_abs <= 0x3280_0000u32 {
            return if x_abs == 0 {
                x
            } else {
                (x as f64 - f64::from_bits(0x3fc5555555555555) * x as f64 * x as f64 * x as f64)
                    as f32
            };
        }

        let x_d = x as f64;
        let x_sq = x_d * x_d;
        // Generated by Sollya with:
        // R = remez(asinh(x)/x, [|0, 2, 4, 6, 8, 10, 12, 14, 16, 18|], [0, 2^-3]);
        // P = fpminimax(asinh(x)/x, [|0, 2, 4, 6, 8, 10, 12, 14, 16, 18|], [|D...|], [0, 2^-3], absolute, floating, R);
        // See `./notes/asinhf.sollya
        let p = f_polyeval10(
            x_sq,
            0.0,
            f64::from_bits(0xbfc5555555555555),
            f64::from_bits(0x3fb3333333333333),
            f64::from_bits(0xbfa6db6db6db6d8e),
            f64::from_bits(0x3f9f1c71c71beb52),
            f64::from_bits(0xbf96e8ba2e0dde02),
            f64::from_bits(0x3f91c4ec071a2f97),
            f64::from_bits(0xbf8c9966fc6b6fda),
            f64::from_bits(0x3f879da45ad06ce8),
            f64::from_bits(0xbf82b3657f620c14),
        );
        return f_fmla(x_d, p, x_d) as f32;
    }

    static SIGN: [f64; 2] = [1.0, -1.0];
    let x_sign = SIGN[(x_u >> 31) as usize];
    let x_d = x as f64;

    if !x.is_finite() {
        return x;
    }

    // asinh(x) = log(x + sqrt(x^2 + 1))
    (x_sign * log_eval(f_fmla(x_d, x_sign, f_fmla(x_d, x_d, 1.0).sqrt()))) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_asinhf() {
        assert_eq!(f_asinhf(-0.24319594), -0.2408603);
        assert_eq!(f_asinhf(2.0), 1.4436355);
        assert_eq!(f_asinhf(-2.0), -1.4436355);
        assert_eq!(f_asinhf(1.054234), 0.9192077);
    }
}
