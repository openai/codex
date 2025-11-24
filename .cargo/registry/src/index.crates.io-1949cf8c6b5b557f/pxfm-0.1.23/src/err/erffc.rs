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
use crate::common::{dd_fmla, f_fmla};
use std::hint::black_box;

static ERR0: [u64; 128] = [
    0x3ff0000000000000,
    0x3ff0163da9fb3335,
    0x3ff02c9a3e778061,
    0x3ff04315e86e7f85,
    0x3ff059b0d3158574,
    0x3ff0706b29ddf6de,
    0x3ff0874518759bc8,
    0x3ff09e3ecac6f383,
    0x3ff0b5586cf9890f,
    0x3ff0cc922b7247f7,
    0x3ff0e3ec32d3d1a2,
    0x3ff0fb66affed31b,
    0x3ff11301d0125b51,
    0x3ff12abdc06c31cc,
    0x3ff1429aaea92de0,
    0x3ff15a98c8a58e51,
    0x3ff172b83c7d517b,
    0x3ff18af9388c8dea,
    0x3ff1a35beb6fcb75,
    0x3ff1bbe084045cd4,
    0x3ff1d4873168b9aa,
    0x3ff1ed5022fcd91d,
    0x3ff2063b88628cd6,
    0x3ff21f49917ddc96,
    0x3ff2387a6e756238,
    0x3ff251ce4fb2a63f,
    0x3ff26b4565e27cdd,
    0x3ff284dfe1f56381,
    0x3ff29e9df51fdee1,
    0x3ff2b87fd0dad990,
    0x3ff2d285a6e4030b,
    0x3ff2ecafa93e2f56,
    0x3ff306fe0a31b715,
    0x3ff32170fc4cd831,
    0x3ff33c08b26416ff,
    0x3ff356c55f929ff1,
    0x3ff371a7373aa9cb,
    0x3ff38cae6d05d866,
    0x3ff3a7db34e59ff7,
    0x3ff3c32dc313a8e5,
    0x3ff3dea64c123422,
    0x3ff3fa4504ac801c,
    0x3ff4160a21f72e2a,
    0x3ff431f5d950a897,
    0x3ff44e086061892d,
    0x3ff46a41ed1d0057,
    0x3ff486a2b5c13cd0,
    0x3ff4a32af0d7d3de,
    0x3ff4bfdad5362a27,
    0x3ff4dcb299fddd0d,
    0x3ff4f9b2769d2ca7,
    0x3ff516daa2cf6642,
    0x3ff5342b569d4f82,
    0x3ff551a4ca5d920f,
    0x3ff56f4736b527da,
    0x3ff58d12d497c7fd,
    0x3ff5ab07dd485429,
    0x3ff5c9268a5946b7,
    0x3ff5e76f15ad2148,
    0x3ff605e1b976dc09,
    0x3ff6247eb03a5585,
    0x3ff6434634ccc320,
    0x3ff6623882552225,
    0x3ff68155d44ca973,
    0x3ff6a09e667f3bcd,
    0x3ff6c012750bdabf,
    0x3ff6dfb23c651a2f,
    0x3ff6ff7df9519484,
    0x3ff71f75e8ec5f74,
    0x3ff73f9a48a58174,
    0x3ff75feb564267c9,
    0x3ff780694fde5d3f,
    0x3ff7a11473eb0187,
    0x3ff7c1ed0130c132,
    0x3ff7e2f336cf4e62,
    0x3ff80427543e1a12,
    0x3ff82589994cce13,
    0x3ff8471a4623c7ad,
    0x3ff868d99b4492ed,
    0x3ff88ac7d98a6699,
    0x3ff8ace5422aa0db,
    0x3ff8cf3216b5448c,
    0x3ff8f1ae99157736,
    0x3ff9145b0b91ffc6,
    0x3ff93737b0cdc5e5,
    0x3ff95a44cbc8520f,
    0x3ff97d829fde4e50,
    0x3ff9a0f170ca07ba,
    0x3ff9c49182a3f090,
    0x3ff9e86319e32323,
    0x3ffa0c667b5de565,
    0x3ffa309bec4a2d33,
    0x3ffa5503b23e255d,
    0x3ffa799e1330b358,
    0x3ffa9e6b5579fdbf,
    0x3ffac36bbfd3f37a,
    0x3ffae89f995ad3ad,
    0x3ffb0e07298db666,
    0x3ffb33a2b84f15fb,
    0x3ffb59728de5593a,
    0x3ffb7f76f2fb5e47,
    0x3ffba5b030a1064a,
    0x3ffbcc1e904bc1d2,
    0x3ffbf2c25bd71e09,
    0x3ffc199bdd85529c,
    0x3ffc40ab5fffd07a,
    0x3ffc67f12e57d14b,
    0x3ffc8f6d9406e7b5,
    0x3ffcb720dcef9069,
    0x3ffcdf0b555dc3fa,
    0x3ffd072d4a07897c,
    0x3ffd2f87080d89f2,
    0x3ffd5818dcfba487,
    0x3ffd80e316c98398,
    0x3ffda9e603db3285,
    0x3ffdd321f301b460,
    0x3ffdfc97337b9b5f,
    0x3ffe264614f5a129,
    0x3ffe502ee78b3ff6,
    0x3ffe7a51fbc74c83,
    0x3ffea4afa2a490da,
    0x3ffecf482d8e67f1,
    0x3ffefa1bee615a27,
    0x3fff252b376bba97,
    0x3fff50765b6e4540,
    0x3fff7bfdad9cbe14,
    0x3fffa7c1819e90d8,
    0x3fffd3c22b8f71f1,
];

static ERFC_COEFFS: [[u64; 16]; 2] = [
    [
        0x3fec162355429b28,
        0x400d99999999999a,
        0x3fdda951cece2b85,
        0xbff70ef6cff4bcc4,
        0x4003d7f7b3d617de,
        0xc009d0aa47537c51,
        0x4009754ea9a3fcb1,
        0xc0027a5453fcc015,
        0x3ff1ef2e0531aeba,
        0xbfceca090f5a1c06,
        0xbfb7a3cd173a063c,
        0x3fb30fa68a68fddd,
        0x3f555ad9a326993a,
        0xbf907e7b0bb39fbf,
        0x3f52328706c0e950,
        0x3f6d6aa0b7b19cfe,
    ],
    [
        0x401137c8983f8516,
        0x400799999999999a,
        0x3fc05b53aa241333,
        0xbfca3f53872bf870,
        0x3fbde4c30742c9d5,
        0xbfacb24bfa591986,
        0x3f9666aec059ca5f,
        0xbf7a61250eb26b0b,
        0x3f52b28b7924b34d,
        0x3f041b13a9d45013,
        0xbf16dd5e8a273613,
        0x3ef09ce8ea5e8da5,
        0x3ed33923b4102981,
        0xbec1dfd161e3f984,
        0xbe8c87618fcae3b3,
        0x3e8e8a6ffa0ba2c7,
    ],
];

/// Complementary error function
///
/// Max ULP 0.5
#[inline]
pub fn f_erfcf(x: f32) -> f32 {
    let ax = f32::from_bits(x.to_bits() & 0x7fff_ffff);
    let axd = ax as f64;
    let x2 = axd * axd;
    let t = x.to_bits();
    let at = t & 0x7fff_ffff;
    let sgn = t >> 31;
    let i: i64 = (at > 0x40051000) as i64;
    /* for x < -0x1.ea8f94p+1, erfc(x) rounds to 2 (to nearest) */
    if t > 0xc07547cau32 {
        // x < -0x1.ea8f94p+1
        if t >= 0xff800000u32 {
            // -Inf or NaN
            if t == 0xff800000u32 {
                return 2.0;
            } // -Inf
            return x + x; // NaN
        }
        return black_box(2.0) - black_box(f32::from_bits(0x33000000)); // rounds to 2 or nextbelow(2)
    }
    /* at is the absolute value of x
    for x >= 0x1.41bbf8p+3, erfc(x) < 2^-150, thus rounds to 0 or to 2^-149
    depending on the rounding mode */
    if at >= 0x4120ddfcu32 {
        // |x| >= 0x1.41bbf8p+3
        if at >= 0x7f800000u32 {
            // +Inf or NaN
            if at == 0x7f800000u32 {
                return 0.0;
            } // +Inf
            return x + x; // NaN
        }
        // 0x1p-149f * 0.25f rounds to 0 or 2^-149 depending on rounding
        return black_box(f32::from_bits(0x00000001)) * black_box(0.25);
    }
    if at <= 0x3db80000u32 {
        // |x| <= 0x1.7p-4
        if t == 0xb76c9f62u32 {
            // x = -0x1.d93ec4p-17
            return black_box(f32::from_bits(0x3f800085)) + black_box(f32::from_bits(0x33000000)); // exceptional case
        }
        /* for |x| <= 0x1.c5bf88p-26. erfc(x) rounds to 1 (to nearest) */
        if at <= 0x32e2dfc4u32 {
            // |x| <= 0x1.c5bf88p-26
            if at == 0 {
                return 1.0;
            }
            static D: [f32; 2] = [f32::from_bits(0xb2800000), f32::from_bits(0x33000000)];
            return 1.0 + D[sgn as usize];
        }
        /* around 0, erfc(x) behaves as 1 - (odd polynomial) */
        const C: [u64; 5] = [
            0x3ff20dd750429b6d,
            0xbfd812746b03610b,
            0x3fbce2f218831d2f,
            0xbf9b82c609607dcb,
            0x3f7553af09b8008e,
        ];

        let fw0 = f_fmla(x2, f64::from_bits(C[4]), f64::from_bits(C[3]));
        let fw1 = f_fmla(x2, fw0, f64::from_bits(C[2]));
        let fw2 = f_fmla(x2, fw1, f64::from_bits(C[1]));

        let f0 = x as f64 * f_fmla(x2, fw2, f64::from_bits(C[0]));
        return (1.0 - f0) as f32;
    }

    /* now -0x1.ea8f94p+1 <= x <= 0x1.41bbf8p+3, with |x| > 0x1.7p-4 */
    const ILN2: f64 = f64::from_bits(0x3ff71547652b82fe);
    const LN2H: f64 = f64::from_bits(0x3f762e42fefa0000);
    const LN2L: f64 = f64::from_bits(0x3d0cf79abd6f5dc8);

    let jt = dd_fmla(x2, ILN2, -(1024. + f64::from_bits(0x3f70000000000000))).to_bits();
    let j: i64 = ((jt << 12) as i64) >> 48;
    let sf = ((j >> 7) as u64)
        .wrapping_add(0x3ffu64 | (sgn as u64) << 11)
        .wrapping_shl(52);

    const CH: [u64; 4] = [
        0xbfdffffffffff333,
        0x3fc5555555556a14,
        0xbfa55556666659b4,
        0x3f81111074cc7b22,
    ];
    let d = f_fmla(LN2L, j as f64, f_fmla(LN2H, j as f64, x2));
    let d2 = d * d;
    let e0 = f64::from_bits(ERR0[(j & 127) as usize]);

    let fw0 = f_fmla(d, f64::from_bits(CH[3]), f64::from_bits(CH[2]));
    let fw1 = f_fmla(d, f64::from_bits(CH[1]), f64::from_bits(CH[0]));
    let fw2 = f_fmla(d2, fw0, fw1);

    let f = f_fmla(d2, fw2, d);

    let ct = ERFC_COEFFS[i as usize];
    let z = (axd - f64::from_bits(ct[0])) / (axd + f64::from_bits(ct[1]));
    let z2 = z * z;
    let z4 = z2 * z2;
    let z8 = z4 * z4;
    let c = &ct[3..];

    let sw0 = f_fmla(z, f64::from_bits(c[1]), f64::from_bits(c[0]));
    let sw1 = f_fmla(z, f64::from_bits(c[3]), f64::from_bits(c[2]));
    let sw2 = f_fmla(z, f64::from_bits(c[5]), f64::from_bits(c[4]));
    let sw3 = f_fmla(z, f64::from_bits(c[7]), f64::from_bits(c[6]));

    let zw0 = f_fmla(z2, sw1, sw0);
    let zw1 = f_fmla(z2, sw3, sw2);

    let sw4 = f_fmla(z, f64::from_bits(c[9]), f64::from_bits(c[8]));
    let sw5 = f_fmla(z, f64::from_bits(c[11]), f64::from_bits(c[10]));

    let zw2 = f_fmla(z4, zw1, zw0);
    let zw3 = f_fmla(z2, sw5, sw4);
    let zw4 = f_fmla(z4, f64::from_bits(c[12]), zw3);

    let mut s = f_fmla(z8, zw4, zw2);

    s = f_fmla(z, s, f64::from_bits(ct[2]));

    static OFF: [f64; 2] = [0., 2.];

    let r = (f64::from_bits(sf) * f_fmla(-f, e0, e0)) * s;
    let y = OFF[sgn as usize] + r;
    y as f32
}

#[cfg(test)]
mod tests {
    use crate::f_erfcf;

    #[test]
    fn test_erfc() {
        assert_eq!(f_erfcf(0.0), 1.0);
        assert_eq!(f_erfcf(0.5), 0.47950011);
        assert_eq!(f_erfcf(1.0), 0.1572992);
    }
}
