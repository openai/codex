/*
 * // Copyright (c) Radzivon Bartoshyk 6/2025. All rights reserved.
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
use crate::common::f_fmla;
use std::hint::black_box;

static IX: [u64; 129] = [
    0x3ff0000000000000,
    0x3fefc07f01fc0000,
    0x3fef81f81f820000,
    0x3fef44659e4a0000,
    0x3fef07c1f07c0000,
    0x3feecc07b3020000,
    0x3fee9131abf00000,
    0x3fee573ac9020000,
    0x3fee1e1e1e1e0000,
    0x3fede5d6e3f80000,
    0x3fedae6076ba0000,
    0x3fed77b654b80000,
    0x3fed41d41d420000,
    0x3fed0cb58f6e0000,
    0x3fecd85689040000,
    0x3feca4b3055e0000,
    0x3fec71c71c720000,
    0x3fec3f8f01c40000,
    0x3fec0e0703820000,
    0x3febdd2b89940000,
    0x3febacf914c20000,
    0x3feb7d6c3dda0000,
    0x3feb4e81b4e80000,
    0x3feb2036406c0000,
    0x3feaf286bca20000,
    0x3feac5701ac60000,
    0x3fea98ef606a0000,
    0x3fea6d01a6d00000,
    0x3fea41a41a420000,
    0x3fea16d3f97a0000,
    0x3fe9ec8e95100000,
    0x3fe9c2d14ee40000,
    0x3fe99999999a0000,
    0x3fe970e4f80c0000,
    0x3fe948b0fcd60000,
    0x3fe920fb49d00000,
    0x3fe8f9c18f9c0000,
    0x3fe8d3018d300000,
    0x3fe8acb90f6c0000,
    0x3fe886e5f0ac0000,
    0x3fe8618618620000,
    0x3fe83c977ab20000,
    0x3fe8181818180000,
    0x3fe7f405fd020000,
    0x3fe7d05f417e0000,
    0x3fe7ad2208e00000,
    0x3fe78a4c81780000,
    0x3fe767dce4340000,
    0x3fe745d1745e0000,
    0x3fe724287f460000,
    0x3fe702e05c0c0000,
    0x3fe6e1f76b440000,
    0x3fe6c16c16c20000,
    0x3fe6a13cd1540000,
    0x3fe6816816820000,
    0x3fe661ec6a520000,
    0x3fe642c8590c0000,
    0x3fe623fa77020000,
    0x3fe6058160580000,
    0x3fe5e75bb8d00000,
    0x3fe5c9882b940000,
    0x3fe5ac056b020000,
    0x3fe58ed230820000,
    0x3fe571ed3c500000,
    0x3fe5555555560000,
    0x3fe5390948f40000,
    0x3fe51d07eae20000,
    0x3fe5015015020000,
    0x3fe4e5e0a7300000,
    0x3fe4cab887260000,
    0x3fe4afd6a0520000,
    0x3fe49539e3b20000,
    0x3fe47ae147ae0000,
    0x3fe460cbc7f60000,
    0x3fe446f865620000,
    0x3fe42d6625d60000,
    0x3fe4141414140000,
    0x3fe3fb013fb00000,
    0x3fe3e22cbce40000,
    0x3fe3c995a47c0000,
    0x3fe3b13b13b20000,
    0x3fe3991c2c180000,
    0x3fe3813813820000,
    0x3fe3698df3de0000,
    0x3fe3521cfb2c0000,
    0x3fe33ae45b580000,
    0x3fe323e34a2c0000,
    0x3fe30d1901300000,
    0x3fe2f684bda20000,
    0x3fe2e025c04c0000,
    0x3fe2c9fb4d820000,
    0x3fe2b404ad020000,
    0x3fe29e4129e40000,
    0x3fe288b012880000,
    0x3fe27350b8820000,
    0x3fe25e2270800000,
    0x3fe24924924a0000,
    0x3fe23456789a0000,
    0x3fe21fb781220000,
    0x3fe20b470c680000,
    0x3fe1f7047dc20000,
    0x3fe1e2ef3b400000,
    0x3fe1cf06ada20000,
    0x3fe1bb4a40460000,
    0x3fe1a7b9611a0000,
    0x3fe19453808c0000,
    0x3fe1811811820000,
    0x3fe16e0689420000,
    0x3fe15b1e5f760000,
    0x3fe1485f0e0a0000,
    0x3fe135c811360000,
    0x3fe12358e75e0000,
    0x3fe1111111120000,
    0x3fe0fef010fe0000,
    0x3fe0ecf56be60000,
    0x3fe0db20a8900000,
    0x3fe0c9714fbc0000,
    0x3fe0b7e6ec260000,
    0x3fe0a6810a680000,
    0x3fe0953f39020000,
    0x3fe0842108420000,
    0x3fe073260a480000,
    0x3fe0624dd2f20000,
    0x3fe05197f7d80000,
    0x3fe0410410420000,
    0x3fe03091b5200000,
    0x3fe0204081020000,
    0x3fe0101010100000,
    0x3fe0000000000000,
];
static LIX: [u64; 129] = [
    0x0000000000000000,
    0xbf86fe50b6f1eafa,
    0xbf96e79685c160d5,
    0xbfa11cd1d51955ba,
    0xbfa6bad37591e030,
    0xbfac4dfab908ddb5,
    0xbfb0eb389fab4795,
    0xbfb3aa2fdd26ae99,
    0xbfb663f6faca846b,
    0xbfb918a16e4cb157,
    0xbfbbc84240a78a13,
    0xbfbe72ec1181cfb1,
    0xbfc08c588cd964e4,
    0xbfc1dcd19759f2e3,
    0xbfc32ae9e27627c6,
    0xbfc476a9f989a58a,
    0xbfc5c01a39fa6533,
    0xbfc70742d4eed455,
    0xbfc84c2bd02d6434,
    0xbfc98edd077e9f0a,
    0xbfcacf5e2db31eea,
    0xbfcc0db6cddaa82d,
    0xbfcd49ee4c33121a,
    0xbfce840be751d775,
    0xbfcfbc16b9003e0b,
    0xbfd0790adbae3fc0,
    0xbfd11307dad465b5,
    0xbfd1ac05b2924cc5,
    0xbfd24407ab0cc410,
    0xbfd2db10fc4ea424,
    0xbfd37124cea58697,
    0xbfd406463b1d455d,
    0xbfd49a784bcbaa37,
    0xbfd52dbdfc4f341d,
    0xbfd5c01a39ff2c9b,
    0xbfd6518fe46abaa5,
    0xbfd6e221cd9d6933,
    0xbfd771d2ba7f5791,
    0xbfd800a56315ee2a,
    0xbfd88e9c72df8611,
    0xbfd91bba891d495f,
    0xbfd9a8023920fa4d,
    0xbfda33760a7fbca6,
    0xbfdabe18797d2eff,
    0xbfdb47ebf734b923,
    0xbfdbd0f2e9eb2b84,
    0xbfdc592fad2be1aa,
    0xbfdce0a4923cf5e6,
    0xbfdd6753e02f4ebc,
    0xbfdded3fd445af00,
    0xbfde726aa1e558fe,
    0xbfdef6d67325ba38,
    0xbfdf7a8568c8aea6,
    0xbfdffd799a81be87,
    0x3fdf804ae8d33c40,
    0x3fdefec61b04af4e,
    0x3fde7df5fe572606,
    0x3fddfdd89d5b0009,
    0x3fdd7e6c0abbd924,
    0x3fdcffae611a74d6,
    0x3fdc819dc2d8578c,
    0x3fdc043859e5bdbc,
    0x3fdb877c57b47c04,
    0x3fdb0b67f4f29a66,
    0x3fda8ff97183ed07,
    0x3fda152f14293c74,
    0x3fd99b072a9289ca,
    0x3fd921800927e284,
    0x3fd8a8980ac41130,
    0x3fd8304d90c2859d,
    0x3fd7b89f02cbd49a,
    0x3fd7418aceb84ab1,
    0x3fd6cb0f68656c95,
    0x3fd6552b49993dc2,
    0x3fd5dfdcf1eacd7b,
    0x3fd56b22e6b97c18,
    0x3fd4f6fbb2ce6943,
    0x3fd48365e6957b42,
    0x3fd4106017c0dbcf,
    0x3fd39de8e15727d9,
    0x3fd32bfee37489bc,
    0x3fd2baa0c34989c3,
    0x3fd249cd2b177fd5,
    0x3fd1d982c9d50468,
    0x3fd169c0536677ac,
    0x3fd0fa848045f67b,
    0x3fd08bce0d9a7c60,
    0x3fd01d9bbcf66a2c,
    0x3fcf5fd8a90e2d85,
    0x3fce857d3d3af1e5,
    0x3fcdac22d3ec5f4e,
    0x3fccd3c712db459a,
    0x3fcbfc67a7ff3c22,
    0x3fcb2602497678f4,
    0x3fca5094b555a1f8,
    0x3fc97c1cb136b96f,
    0x3fc8a8980ac8652d,
    0x3fc7d60496c83f66,
    0x3fc7046031c7cdaf,
    0x3fc633a8bf460335,
    0x3fc563dc2a08b102,
    0x3fc494f863bbc1de,
    0x3fc3c6fb6507a37e,
    0x3fc2f9e32d5257ec,
    0x3fc22dadc2a627ef,
    0x3fc1625931802e49,
    0x3fc097e38cef9519,
    0x3fbf9c95dc138295,
    0x3fbe0b1ae90505f6,
    0x3fbc7b528b5fcffa,
    0x3fbaed391abb17a1,
    0x3fb960caf9bd35ea,
    0x3fb7d60496e3edeb,
    0x3fb64ce26bf2108e,
    0x3fb4c560fe5b573b,
    0x3fb33f7cde24adfb,
    0x3fb1bb32a5ed9353,
    0x3fb0387efbd3006e,
    0x3fad6ebd1f1d0955,
    0x3faa6f9c37a8beab,
    0x3fa77394c9d6762c,
    0x3fa47aa07358e1a4,
    0x3fa184b8e4d490ef,
    0x3f9d23afc4d95c78,
    0x3f9743ee8678a7cb,
    0x3f916a21e243bf78,
    0x3f872c7ba20c907e,
    0x3f7720d9c0536e17,
    0x0000000000000000,
];

// |x| < 1.3862943452718848
#[cold]
fn log2p1f_small(z: f64, ax: u32, ux: u32) -> f32 {
    let z2 = z * z;
    let z4 = z2 * z2;
    if ax < 0x3b9d9d34u32 {
        // |x| < 0x1.3b3a68p-8
        if ax < 0x39638a7eu32 {
            // |x| < 0x1.c714fcp-13
            if ax < 0x329c5639u32 {
                // |x| < 0x1.38ac72p-26
                const C: [u64; 2] = [0x3ff71547652b82fe, 0xbfe71547652b82ff];
                let res = z * f_fmla(z, f64::from_bits(C[1]), f64::from_bits(C[0]));
                res as f32
            } else {
                if ux == 0x32ff7045u32 {
                    return black_box(f32::from_bits(0x3338428d))
                        - black_box(f32::from_bits(0x17c00000));
                }
                if ux == 0xb395efbbu32 {
                    return black_box(f32::from_bits(0xb3d85005))
                        + black_box(f32::from_bits(0x19800000));
                }
                if ux == 0x35a14df7u32 {
                    return black_box(f32::from_bits(0x35e8b690))
                        + black_box(f32::from_bits(0x1b800000));
                }
                if ux == 0x3841cb81u32 {
                    return black_box(f32::from_bits(0x388bca4f))
                        + black_box(f32::from_bits(0x1e000000));
                }
                const C: [u64; 4] = [
                    0x3ff71547652b82fe,
                    0xbfe71547652b82fd,
                    0x3fdec709ead0c9a7,
                    0xbfd7154773c1cb29,
                ];

                let zxf0 = f_fmla(z, f64::from_bits(C[3]), f64::from_bits(C[2]));
                let zxf1 = f_fmla(z, f64::from_bits(C[1]), f64::from_bits(C[0]));

                let zxf = z * f_fmla(z2, zxf0, zxf1);
                zxf as f32
            }
        } else {
            if ux == 0xbac9363du32 {
                return black_box(f32::from_bits(0xbb114155))
                    + black_box(f32::from_bits(0x21000000));
            }
            const C: [u64; 6] = [
                0x3ff71547652b82fe,
                0xbfe71547652b8300,
                0x3fdec709dc28f51b,
                0xbfd7154765157748,
                0x3fd2778a510a3682,
                0xbfcec745df1551fc,
            ];

            let zxf0 = f_fmla(z, f64::from_bits(C[5]), f64::from_bits(C[4]));
            let zxf1 = f_fmla(z, f64::from_bits(C[3]), f64::from_bits(C[2]));
            let zxf2 = f_fmla(z, f64::from_bits(C[1]), f64::from_bits(C[0]));

            let zw0 = f_fmla(z2, zxf1, zxf2);

            let zxf = z * f_fmla(z4, zxf0, zw0);
            zxf as f32
        }
    } else {
        const C: [u64; 8] = [
            0x3ff71547652b82fe,
            0xbfe71547652b82fb,
            0x3fdec709dc3b6a73,
            0xbfd71547652dc090,
            0x3fd2776c1a889010,
            0xbfcec7095bd4d208,
            0x3fca66bec7fc8f70,
            0xbfc71a900fc3f3f9,
        ];

        let zxf0 = f_fmla(z, f64::from_bits(C[7]), f64::from_bits(C[6]));
        let zxf1 = f_fmla(z, f64::from_bits(C[5]), f64::from_bits(C[4]));
        let zxf2 = f_fmla(z, f64::from_bits(C[3]), f64::from_bits(C[2]));
        let zxf3 = f_fmla(z, f64::from_bits(C[1]), f64::from_bits(C[0]));

        let zw0 = f_fmla(z2, zxf0, zxf1);
        let zw1 = f_fmla(z2, zxf2, zxf3);

        let zxf = z * f_fmla(z4, zw0, zw1);
        zxf as f32
    }
}

/// Computes log2(x+1)
///
/// Max ULP 0.5
#[inline]
pub fn f_log2p1f(x: f32) -> f32 {
    let mut z = x as f64;
    let t = x.to_bits();
    let ux = t;
    let ax = ux & 0x7fff_ffff;
    if ux >= 0x17fu32 << 23 {
        // x <= -1
        if ux == (0x17fu32 << 23) {
            return f32::NEG_INFINITY; // -1.0
        }
        if ux > (0x1ffu32 << 23) {
            return x + x;
        } // nan
        f32::NAN // x < -1
    } else if ax >= (0xff << 23) {
        // +inf, nan
        if ax > (0xff << 23) {
            return x + x;
        } // nan
        f32::INFINITY
    } else if ax < 0x3cb7aa26u32 {
        // |x| < 1.3862943452718848
        log2p1f_small(z, ax, ux)
    } else {
        // |x| >= 0x1.6f544cp-6
        if ux == 0x4ebd09e3u32 {
            let h = f32::from_bits(0x41f48013);
            let l = f32::from_bits(0x35780000);
            return black_box(h) + black_box(l);
        }
        let tp = (z + 1.0).to_bits();
        let m = tp & 0x000fffffffffffff;
        let mut e: i32 = (tp >> 52).wrapping_sub(0x3ff) as i32;
        let j: i32 = ((m as i64).wrapping_add(1i64 << (52 - 8)) >> (52 - 7)) as i32;
        let k = if j > 53 { 1 } else { 0 };
        e += k;
        let xd = m | 0x3ffu64 << 52;
        z = f_fmla(f64::from_bits(xd), f64::from_bits(IX[j as usize]), -1.0);
        const C: [u64; 6] = [
            0x3ff71547652b82fe,
            0xbfe71547652b82ff,
            0x3fdec709dc32988b,
            0xbfd715476521ec2b,
            0x3fd277801a1ad904,
            0xbfcec731704d6a88,
        ];
        let z2 = z * z;
        let mut c0 = f_fmla(z, f64::from_bits(C[1]), f64::from_bits(C[0]));
        let c2 = f_fmla(z, f64::from_bits(C[3]), f64::from_bits(C[2]));
        let c4 = f_fmla(z, f64::from_bits(C[5]), f64::from_bits(C[4]));

        let zv0 = f_fmla(z2, c4, c2);

        c0 = f_fmla(z2, zv0, c0);
        let res = f_fmla(z, c0, -f64::from_bits(LIX[j as usize])) + e as f64;
        res as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log2p1f() {
        assert_eq!(f_log2p1f(0.0), 0.0);
        assert_eq!(f_log2p1f(1.0), 1.0);
        assert_eq!(f_log2p1f(-0.0432432), -0.063775845);
        assert_eq!(f_log2p1f(-0.009874634), -0.01431689);
        assert_eq!(f_log2p1f(1.2443), 1.1662655);
        assert!(f_log2p1f(-1.0432432).is_nan());
    }
}
