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
use crate::common::f_fmla;
use crate::logs::log1pf::special_logf;
use std::hint::black_box;

static TR: [u64; 65] = [
    0x3ff0000000000000,
    0x3fef81f820000000,
    0x3fef07c1f0000000,
    0x3fee9131ac000000,
    0x3fee1e1e1e000000,
    0x3fedae6077000000,
    0x3fed41d41d000000,
    0x3fecd85689000000,
    0x3fec71c71c000000,
    0x3fec0e0704000000,
    0x3febacf915000000,
    0x3feb4e81b5000000,
    0x3feaf286bd000000,
    0x3fea98ef60000000,
    0x3fea41a41a000000,
    0x3fe9ec8e95000000,
    0x3fe999999a000000,
    0x3fe948b0fd000000,
    0x3fe8f9c190000000,
    0x3fe8acb90f000000,
    0x3fe8618618000000,
    0x3fe8181818000000,
    0x3fe7d05f41000000,
    0x3fe78a4c81000000,
    0x3fe745d174000000,
    0x3fe702e05c000000,
    0x3fe6c16c17000000,
    0x3fe6816817000000,
    0x3fe642c859000000,
    0x3fe6058160000000,
    0x3fe5c9882c000000,
    0x3fe58ed231000000,
    0x3fe5555555000000,
    0x3fe51d07eb000000,
    0x3fe4e5e0a7000000,
    0x3fe4afd6a0000000,
    0x3fe47ae148000000,
    0x3fe446f865000000,
    0x3fe4141414000000,
    0x3fe3e22cbd000000,
    0x3fe3b13b14000000,
    0x3fe3813814000000,
    0x3fe3521cfb000000,
    0x3fe323e34a000000,
    0x3fe2f684be000000,
    0x3fe2c9fb4e000000,
    0x3fe29e412a000000,
    0x3fe27350b9000000,
    0x3fe2492492000000,
    0x3fe21fb781000000,
    0x3fe1f7047e000000,
    0x3fe1cf06ae000000,
    0x3fe1a7b961000000,
    0x3fe1811812000000,
    0x3fe15b1e5f000000,
    0x3fe135c811000000,
    0x3fe1111111000000,
    0x3fe0ecf56c000000,
    0x3fe0c97150000000,
    0x3fe0a6810a000000,
    0x3fe0842108000000,
    0x3fe0624dd3000000,
    0x3fe0410410000000,
    0x3fe0204081000000,
    0x3fe0000000000000,
];

static TL: [u64; 65] = [
    0xbd4562ec497ef351,
    0x3f7b9476892ea99c,
    0x3f8b5e909c959eec,
    0x3f945f4f59ec84f0,
    0x3f9af5f92cbcf2aa,
    0x3fa0ba01a6069052,
    0x3fa3ed119b99dd41,
    0x3fa714834298a088,
    0x3faa30a9d98309c1,
    0x3fad41d51266b9d9,
    0x3fb02428c0f62dfc,
    0x3fb1a23444eea521,
    0x3fb31b30543f2597,
    0x3fb48f3ed39bd5e7,
    0x3fb5fe8049a0bd06,
    0x3fb769140a6a78ea,
    0x3fb8cf1836c96595,
    0x3fba30a9d5551a84,
    0x3fbb8de4d1ee5b21,
    0x3fbce6e4202c7bc9,
    0x3fbe3bc1accaa6ea,
    0x3fbf8c9683b584b7,
    0x3fc06cbd68ca86e0,
    0x3fc11142f19de3a2,
    0x3fc1b3e71fa795f0,
    0x3fc254b4d37a3354,
    0x3fc2f3b6912cab79,
    0x3fc390f6831144f7,
    0x3fc42c7e7fffb21a,
    0x3fc4c65808c779ae,
    0x3fc55e8c507508c7,
    0x3fc5f52445deb049,
    0x3fc68a288c3efe72,
    0x3fc71da17bdef98b,
    0x3fc7af9736089c4b,
    0x3fc84011952a11eb,
    0x3fc8cf1837a7d6d1,
    0x3fc95cb2891e3048,
    0x3fc9e8e7b0f85651,
    0x3fca73beaa5d9dfe,
    0x3fcafd3e39454544,
    0x3fcb856cf060c662,
    0x3fcc0c5134de0c6d,
    0x3fcc91f1371bb611,
    0x3fcd1652ffcd2bc5,
    0x3fcd997c6f634ae6,
    0x3fce1b733ab8fbad,
    0x3fce9c3ceadab4c8,
    0x3fcf1bdeec438f77,
    0x3fcf9a5e7a5f906f,
    0x3fd00be05ac02564,
    0x3fd04a054d81990c,
    0x3fd087a083594e33,
    0x3fd0c4b457098b4f,
    0x3fd101431aa1f48a,
    0x3fd13d4f08b98411,
    0x3fd178da53edaecb,
    0x3fd1b3e71e9f9391,
    0x3fd1ee777defd526,
    0x3fd2288d7b48d874,
    0x3fd2622b0f52dad8,
    0x3fd29b522a4c594c,
    0x3fd2d404b0e305b9,
    0x3fd30c4478f3f21d,
    0x3fd34413509f6f4d,
];

#[cold]
fn log10p1f_accurate(ax: u32, ux: u32, v: f64, z: f64, l: f64, e: i32) -> f32 {
    if ax < 0x3d32743eu32 {
        // |x| < 0x1.64e87cp-5f
        if ux == 0xa6aba8afu32 {
            return black_box(f32::from_bits(0xa61519de)) + black_box(f32::from_bits(0x19800000));
        }
        if ux == 0xaf39b9a7u32 {
            return black_box(f32::from_bits(0xaea151a1)) + black_box(f32::from_bits(0x22000000));
        }
        if ux == 0x399a7c00u32 {
            return black_box(f32::from_bits(0x390629e5)) + black_box(f32::from_bits(0x2c800000));
        }
        let z = z / (2.0 + z);
        let z2 = z * z;
        let z4 = z2 * z2;

        const C: [u64; 4] = [
            0x3febcb7b1526e50f,
            0x3fd287a76370129d,
            0x3fc63c62378fa3db,
            0x3fbfca4139a42374,
        ];

        let r0 = f_fmla(z2, f64::from_bits(C[3]), f64::from_bits(C[2]));
        let r1 = f_fmla(z2, f64::from_bits(C[1]), f64::from_bits(C[0]));

        let r = z * f_fmla(z4, r0, r1);
        return r as f32;
    }
    if ux == 0x7956ba5eu32 {
        return black_box(f32::from_bits(0x420b5f5d)) + black_box(f32::from_bits(0x35800000));
    }
    if ux == 0xbd86ffb9u32 {
        return black_box(f32::from_bits(0xbcf29a9b)) + black_box(f32::from_bits(0x30000000));
    }
    const C: [u64; 7] = [
        0x3fdbcb7b1526e50e,
        0xbfcbcb7b1526e53d,
        0x3fc287a7636f3fa2,
        0xbfbbcb7b146a14b3,
        0x3fb63c627d5219cb,
        0xbfb2880736c8762d,
        0x3fafc1ecf913961a,
    ];

    let v2 = v * v;

    let xv0 = f_fmla(v, f64::from_bits(C[1]), f64::from_bits(C[0]));
    let xv1 = f_fmla(v, f64::from_bits(C[3]), f64::from_bits(C[2]));
    let xv2 = f_fmla(v, f64::from_bits(C[5]), f64::from_bits(C[4]));

    let xw0 = f_fmla(v2, f64::from_bits(C[6]), xv2);
    let xw1 = f_fmla(v2, xw0, xv1);

    let mut f = v * f_fmla(v2, xw1, xv0);
    f += l - f64::from_bits(TL[0]);
    let r = f_fmla(e as f64, f64::from_bits(0x3fd34413509f79ff), f);
    r as f32
}

/// Computes log10(x+1)
///
/// Max ULP 0.5
#[inline]
pub fn f_log10p1f(x: f32) -> f32 {
    let z = x as f64;
    let t = x.to_bits();
    let ux: u32 = t;
    if ux >= 0x17fu32 << 23 {
        // x <= -1
        return special_logf(x);
    }
    let ax = ux & 0x7fff_ffff;
    if ax == 0 {
        return f32::copysign(0., x);
    }
    if ax >= (0xff << 23) {
        // +inf, nan
        return special_logf(x);
    }

    let mut tz = (z + 1.0).to_bits();
    let m: u64 = tz & 0x000f_ffff_ffff_ffff;
    let e: i32 = (tz >> 52).wrapping_sub(1023) as i32;
    let j: i32 = ((m.wrapping_add((1i64 << 45) as u64)) >> 46) as i32;
    tz = m | (0x3ffu64 << 52);
    let ix = f64::from_bits(TR[j as usize]);
    let l = f64::from_bits(TL[j as usize]);
    let off = e as f64 * f64::from_bits(0x3fd34413509f79ff) + l;
    let v = f_fmla(f64::from_bits(tz), ix, -1.);

    const H: [u64; 4] = [
        0x3fdbcb7b150bf6d8,
        0xbfcbcb7b1738c07e,
        0x3fc287de19e795c5,
        0xbfbbca44edc44bc4,
    ];

    let v2 = v * v;

    let zwf0 = f_fmla(v, f64::from_bits(H[3]), f64::from_bits(H[2]));
    let zwf1 = f_fmla(v, f64::from_bits(H[1]), f64::from_bits(H[0]));

    let f = f_fmla(v2, zwf0, zwf1);
    let r = f_fmla(v, f, off);
    let ub: f32 = r as f32;
    let lb: f32 = (r + f64::from_bits(0x3d55c00000000000)) as f32;
    if ub != lb {
        return log10p1f_accurate(ax, ux, v, z, l, e);
    }
    ub
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log10p1f() {
        assert_eq!(f_log10p1f(0.0), 0.0);
        assert_eq!(f_log10p1f(1.0), 0.30103);
        assert_eq!(f_log10p1f(-0.0432432), -0.019198442);
        assert_eq!(f_log10p1f(-0.009874634), -0.0043098135);
        assert_eq!(f_log10p1f(1.2443), 0.35108092);
        assert!(f_log10p1f(-1.0432432).is_nan());
    }
}
