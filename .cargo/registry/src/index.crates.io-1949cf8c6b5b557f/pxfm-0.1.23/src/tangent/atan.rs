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
use crate::common::{dd_fmla, dyad_fmla, f_fmla};
use crate::double_double::DoubleDouble;
use crate::shared_eval::poly_dd_3;

pub(crate) static ATAN_CIRCLE: [[u16; 3]; 31] = [
    [419, 81, 0],
    [500, 81, 0],
    [582, 163, 0],
    [745, 163, 0],
    [908, 326, 0],
    [1234, 326, 0],
    [1559, 651, 0],
    [2210, 650, 1],
    [2860, 1299, 3],
    [4156, 1293, 4],
    [5444, 2569, 24],
    [7989, 2520, 32],
    [10476, 4917, 168],
    [15224, 4576, 200],
    [19601, 8341, 838],
    [27105, 6648, 731],
    [33036, 10210, 1998],
    [41266, 6292, 1117],
    [46469, 7926, 2048],
    [52375, 4038, 849],
    [55587, 4591, 1291],
    [58906, 2172, 479],
    [60612, 2390, 688],
    [62325, 1107, 247],
    [63192, 1207, 349],
    [64056, 556, 124],
    [64491, 605, 175],
    [64923, 278, 62],
    [65141, 303, 88],
    [65358, 139, 31],
    [65467, 151, 44],
];

pub(crate) static ATAN_REDUCE: [(u64, u64); 129] = [
    (0x0000000000000000, 0x0000000000000000),
    (0x3f89224e047e368e, 0x3c1a3ca6c727c59d),
    (0x3f992346247a91f0, 0x3bf138b0ef96a186),
    (0x3fa2dbaae9a05db0, 0x3c436e7f8a3f5e42),
    (0x3fa927278a3b1162, 0xbbfac986efb92662),
    (0x3faf7495ea3f3783, 0x3c406ec8011ee816),
    (0x3fb2e239ccff3831, 0xbc5858437d431332),
    (0x3fb60b9f7597fdec, 0xbc3cebd13eb7c513),
    (0x3fb936bb8c5b2da2, 0xbc5840cac0d81db5),
    (0x3fbc63ce377fc802, 0x3c5400b0fdaa109e),
    (0x3fbf93183a8db9e9, 0x3c40e04e06c86e72),
    (0x3fc1626d85a91e70, 0x3c4f7ad829163ca7),
    (0x3fc2fcac73a60640, 0xbc52680735ce2cd8),
    (0x3fc4986a74cf4e57, 0xbc690559690b42e4),
    (0x3fc635c990ce0d36, 0x3c591d29110b41aa),
    (0x3fc7d4ec54fb5968, 0xbc4ea90e27182780),
    (0x3fc975f5e0553158, 0xbc2dc82ac14e3e1c),
    (0x3fcb1909efd8b762, 0xbc573a10fd13daaf),
    (0x3fccbe4ceb4b4cf2, 0xbc63a7ffbeabda0b),
    (0x3fce65e3f27c9f2a, 0xbc6db6627a24d523),
    (0x3fd007fa758626ae, 0xbc645f97dd3099f6),
    (0x3fd0de53475f3b3c, 0xbc66293f68741816),
    (0x3fd1b6103d3597e9, 0xbc6ab240d40633e9),
    (0x3fd28f459ecad74d, 0xbc2de34d14e832e0),
    (0x3fd36a08355c63dc, 0x3c6af540d9fb4926),
    (0x3fd4466d542bac92, 0x3c6da60fdbc82ac4),
    (0x3fd5248ae1701b17, 0xbc792a601170138a),
    (0x3fd604775fbb27df, 0xbc67f1fca1d5d15b),
    (0x3fd6e649f7d78649, 0xbc64e223ea716c7b),
    (0x3fd7ca1a832d0f84, 0x3c7b24c824ac51fc),
    (0x3fd8b00196b3d022, 0x3c64314cd132ba43),
    (0x3fd998188e816bf0, 0xbc711f1e0817879a),
    (0x3fda827999fcef32, 0xbc6c3dea4dbad538),
    (0x3fdb6f3fc8c61e5b, 0x3c660d1b780ee3eb),
    (0x3fdc5e87185e67b6, 0xbc4ab5edb7dfa545),
    (0x3fdd506c82a2c800, 0xbc68e1437048b5bd),
    (0x3fde450e0d273e7a, 0xbc706951c97b050f),
    (0x3fdf3c8ad985d9ee, 0xbc414af9522ab518),
    (0x3fe01b819b5a7cf7, 0xbc7aba0d7d97d1f2),
    (0x3fe09a4c59bd0d4d, 0x3c4095bc4ebc2c42),
    (0x3fe11ab7190834ec, 0x3c8798826fa27774),
    (0x3fe19cd3fe8e405d, 0x3c8008f6258fc98f),
    (0x3fe220b5ef047825, 0xbc5462af7ceb7de6),
    (0x3fe2a6709a74f289, 0xbc71184dfd78b472),
    (0x3fe32e1889047ffd, 0x3c79141876dc40c5),
    (0x3fe3b7c3289ed6f3, 0x3c8481c20189726c),
    (0x3fe44386db9ce5db, 0x3c82e851bd025441),
    (0x3fe4d17b087b265d, 0x3c713ada9b8bc419),
    (0x3fe561b82ab7f990, 0xbc805b4c3c4cbee8),
    (0x3fe5f457e4f4812e, 0xbc85619249bd96f1),
    (0x3fe6897514751db6, 0xbc6b0a0fbcafc671),
    (0x3fe7212be621be6d, 0xbc819ff2dc66da45),
    (0x3fe7bb99ed2990cf, 0x3c81320449592d92),
    (0x3fe858de3b716571, 0xbc81fddcd2f3da8e),
    (0x3fe8f9197bf85eeb, 0x3c6d44a42e35cc97),
    (0x3fe99c6e0f634394, 0xbc7585a178b4a18d),
    (0x3fea43002ae42850, 0x3c6f95a531b3a970),
    (0x3feaecf5f9ba35a6, 0xbc396c2d43ca3392),
    (0x3feb9a77c18c1af2, 0xbc6a5bed94b05def),
    (0x3fec4bb009e77983, 0x3c454509d2bff511),
    (0x3fed00cbc7384d2e, 0xbc6b4c867cef300c),
    (0x3fedb9fa89953fcf, 0xbc1ddfac663d6bc6),
    (0x3fee776eafc91706, 0xbc7a510683ff7cb6),
    (0x3fef395d9f0e3c92, 0x3c44fdcd8e4e8710),
    (0x3ff0000000000000, 0x0000000000000000),
    (0x3ff065c900aaf2d8, 0xbc8deec7fc9042ad),
    (0x3ff0ce29d0883c99, 0xbc8395ae45e0657d),
    (0x3ff139447e6a86ee, 0x3c8332cf301a97f3),
    (0x3ff1a73d55278c4b, 0xbc86cc8c4b78213b),
    (0x3ff2183b0c4573ff, 0x3c870a90841da57a),
    (0x3ff28c66fdaf8f09, 0xbc6ba39bad450ee0),
    (0x3ff303ed61109e20, 0xbc88692946d9f93c),
    (0x3ff37efd8d87607e, 0x3c63b711bf765b58),
    (0x3ff3fdca42847507, 0x3c7c21387985b081),
    (0x3ff48089f8bf42cc, 0xbc87ddb19d3d0efc),
    (0x3ff507773c537ead, 0xbc7f5e354cf971f3),
    (0x3ff592d11142fa55, 0xbc700f0ad675330d),
    (0x3ff622db63c8ecc2, 0xbc82c93f50ab2c0e),
    (0x3ff6b7df86265200, 0x3c7bec391adc37d5),
    (0x3ff7522cbdd428a8, 0xbc69686ddc9ffcf5),
    (0x3ff7f218e25a7461, 0xbc78d16529514246),
    (0x3ff89801106cc709, 0xbc8092f51e9c2803),
    (0x3ff9444a7462122a, 0xbc807c06755404c4),
    (0x3ff9f7632fa9e871, 0x3c802e0d43abc92b),
    (0x3ffab1c35d8a74ea, 0x3c5d0184e48af6f7),
    (0x3ffb73ee3c3ef16a, 0x3c773be957380bc2),
    (0x3ffc3e738086bc0f, 0xbc702b6e26c84462),
    (0x3ffd11f0dae40609, 0x3c525c4f3ffa6e1f),
    (0x3ffdef13b73c1406, 0xbc5e302db3c6823f),
    (0x3ffed69b4153a45d, 0x3c73207830326c0e),
    (0x3fffc95abad6cf4a, 0xbc66308cee7927bf),
    (0x4000641e192ceab3, 0xbc70147ebf0df4c5),
    (0x4000ea21d716fbf7, 0xbc7168533cc41d8b),
    (0x40017749711a6679, 0xbc652a0b0333e9c5),
    (0x40020c36c6a7f38e, 0x3c68659eece35395),
    (0x4002a99f50fd4f4f, 0x3c820fcad18cb36f),
    (0x4003504f333f9de6, 0xbc752afdbd5a8c74),
    (0x4004012ce2586a17, 0xbc79747a792907d7),
    (0x4004bd3d87fe0650, 0x3c790c59393b52c8),
    (0x400585aa4e1530fa, 0x3c7af6934f13a3a8),
    (0x40065bc6cc825147, 0xbc48534dcab5ad3e),
    (0x40074118e4b6a7c8, 0xbc7555aa8bfca9a1),
    (0x400837626d70fdb8, 0xbc556b3fee9ca72b),
    (0x400940ad30abc792, 0x3c54b3fdd4fdc06c),
    (0x400a5f59e90600dd, 0x3c6285d367c55ddc),
    (0x400b9633283b6d14, 0xbc48712976f17a16),
    (0x400ce885653127e7, 0xbc3abe8ab65d49fc),
    (0x400e5a3de972a377, 0x3c5cd9be81ad764b),
    (0x400ff01305ecd8dc, 0x3c4742c2922656fa),
    (0x4010d7dc7cff4c9e, 0xbc77c842978bee09),
    (0x4011d0143e71565f, 0x3c67bc7dea7c3c03),
    (0x4012e4ff1626b949, 0x3c4aefbe25b404e9),
    (0x40141bfee2424771, 0xbc34bcfaaa95cb2c),
    (0x40157be4eaa5e11b, 0x3c50fe741e4ec679),
    (0x40170d751908c1b1, 0x3c5fe74a5b0ec709),
    (0x4018dc25c117782b, 0x3c50ca1c19f710ef),
    (0x401af73f4ca3310f, 0x3c52867b40ba77d6),
    (0x401d7398d15e70db, 0x3c60fd4e0d4b1547),
    (0x4020372fb36b87e2, 0x3c5c16c9ecc1621d),
    (0x402208dbdae055ef, 0x3c56b81a36e75e8c),
    (0x40244e6c595afdcc, 0xbc57c22045771848),
    (0x4027398c57f3f1ad, 0x3c5970503be105c0),
    (0x402b1d03c03d2f7f, 0xbc3f299d010aead2),
    (0x403046e9fe60a77e, 0x3c5d2b61deff33ec),
    (0x40345affed201b55, 0x3bf0e84d9567203a),
    (0x403b267195b1ffae, 0xbbfad44b44b92653),
    (0x40445e2455e4aaa7, 0xbc3296d577b5e21d),
    (0x40545eed6854ce99, 0x3c02db53886013ca),
    (0x0000000000000000, 0x0000000000000000),
];

#[cold]
fn atan_refine(x: f64, a: f64) -> f64 {
    const CH: [(u64, u64); 3] = [
        (0xbfd5555555555555, 0xbc75555555555555),
        (0x3fc999999999999a, 0xbc6999999999bcb8),
        (0xbfc2492492492492, 0xbc6249242093c016),
    ];
    const CL: [u64; 4] = [
        0x3fbc71c71c71c71c,
        0xbfb745d1745d1265,
        0x3fb3b13b115bcbc4,
        0xbfb1107c41ad3253,
    ];
    let phi = f_fmla(a.abs(), f64::from_bits(0x40545f306dc9c883), 256.5).to_bits();
    let i = ((phi >> (52 - 8)) & 0xff) as i64;
    let h: DoubleDouble = if i == 128 {
        DoubleDouble::from_quick_recip(-x)
    } else {
        let ta = f64::copysign(f64::from_bits(ATAN_REDUCE[i as usize].0), x);
        let dzt = DoubleDouble::from_exact_mult(x, ta);
        let zmta = x - ta;
        let v = 1.0 + dzt.hi;
        let d = 1.0 - v;
        let ev = (d + dzt.hi) - ((d + v) - 1.0) + dzt.lo;
        let r = 1.0 / v;
        let rl = f_fmla(-ev, r, dd_fmla(r, -v, 1.0)) * r;
        DoubleDouble::quick_mult_f64(DoubleDouble::new(rl, r), zmta)
    };
    let h2 = DoubleDouble::quick_mult(h, h);
    let h3 = DoubleDouble::quick_mult(h, h2);
    let h4 = h2.hi * h2.hi;

    let zw0 = f_fmla(h2.hi, f64::from_bits(CL[3]), f64::from_bits(CL[2]));
    let zw1 = f_fmla(h2.hi, f64::from_bits(CL[1]), f64::from_bits(CL[0]));
    let zfl = f_fmla(h2.hi, zw1, h4 * zw0);

    let mut f = poly_dd_3(h2, CH, zfl);
    f = DoubleDouble::quick_mult(h3, f);
    let (ah, mut az);
    if i == 0 {
        ah = h.hi;
        az = f;
    } else {
        let mut df = 0.;
        if i < 128 {
            df = f64::copysign(1.0, x) * f64::from_bits(ATAN_REDUCE[i as usize].1);
        }
        let id = f64::copysign(i as f64, x);
        ah = f64::from_bits(0x3f8921fb54442d00) * id;
        az = DoubleDouble::new(
            f64::from_bits(0xb97fc8f8cbb5bf80) * id,
            f64::from_bits(0x3c88469898cc5180) * id,
        );
        az = DoubleDouble::add(az, DoubleDouble::new(0., df));
        az = DoubleDouble::add(az, h);
        az = DoubleDouble::add(az, f);
    }
    let v0 = DoubleDouble::from_exact_add(ah, az.hi);
    let v1 = DoubleDouble::from_exact_add(v0.lo, az.lo);

    v1.hi + v0.hi
}

/// Computes atan in double precision
///
/// ULP 0.5
pub fn f_atan(x: f64) -> f64 {
    const CH: [u64; 4] = [
        0x3ff0000000000000,
        0xbfd555555555552b,
        0x3fc9999999069c20,
        0xbfc248d2c8444ac6,
    ];
    let t = x.to_bits();
    let at = t & 0x7fffffffffffffff; // at encodes |x|
    let mut i = (at.wrapping_shr(51) as i64).wrapping_sub(2030i64); // -2030 <= i <= 2065
    if at < 0x3f7b21c475e6362au64 {
        // |x| < 0x1.b21c475e6362ap-8
        if at == 0 {
            return x;
        } // atan(+/-0) = +/-0
        const CH2: [u64; 4] = [
            0xbfd5555555555555,
            0x3fc99999999998c1,
            0xbfc249249176aec0,
            0x3fbc711fd121ae80,
        ];
        if at < 0x3e40000000000000u64 {
            // |x| < 0x1p-27
            /* underflow when 0 < |x| < 2^-1022 or when |x| = 2^-1022
            and rounding towards zero. */
            return dyad_fmla(f64::from_bits(0xbc90000000000000), x, x);
        }
        let x2 = x * x;
        let x3 = x * x2;
        let x4 = x2 * x2;

        let w0 = f_fmla(x2, f64::from_bits(CH2[3]), f64::from_bits(CH2[2]));
        let w1 = f_fmla(x2, f64::from_bits(CH2[1]), f64::from_bits(CH2[0]));

        let f = x3 * f_fmla(x4, w0, w1);
        let ub = f_fmla(f, f64::from_bits(0x3cd2000000000000), f) + x;
        let lb = f_fmla(-f, f64::from_bits(0x3cc4000000000000), f) + x;
        // Ziv's accuracy test
        if ub != lb {
            return atan_refine(x, ub);
        }
        return ub;
    }
    let h;
    let ah;
    let mut al;
    if at > 0x4062ded8e34a9035u64 {
        // |x| > 0x4062ded8e34a9035
        ah = f64::copysign(f64::from_bits(0x3ff921fb54442d18), x);
        al = f64::copysign(f64::from_bits(0x3c91a62633145c07), x);
        if at >= 0x434d02967c31cdb5u64 {
            // |x| >= 1.63312e+16
            if at > (0x7ffu64 << 52) {
                return x + x;
            } // NaN
            return ah + al;
        }
        h = -1.0 / x;
    } else {
        // now 0.006624 <= |x| <= 150.964 thus 1<=i<=30
        let u: u64 = t & 0x0007ffffffffffff;
        let ut: u64 = u.wrapping_shr(51 - 16);
        let ut2: u64 = (ut * ut).wrapping_shr(16);
        let lc = ATAN_CIRCLE[i as usize];
        i = (((lc[0] as u64)
            .wrapping_shl(16)
            .wrapping_add(ut * lc[1] as u64)
            .wrapping_sub(ut2 * lc[2] as u64))
            >> (16 + 9)) as i64;
        let la = ATAN_REDUCE[i as usize];
        let ta = f64::copysign(1.0, x) * f64::from_bits(la.0);
        let id = f64::copysign(1.0, x) * i as f64;
        al = dd_fmla(
            f64::copysign(1.0, x),
            f64::from_bits(la.1),
            f64::from_bits(0x3c88469898cc5170) * id,
        );
        h = (x - ta) / f_fmla(x, ta, 1.0);
        ah = f64::from_bits(0x3f8921fb54442d00) * id;
    }
    let h2 = h * h;
    let h4 = h2 * h2;

    let f0 = f_fmla(h2, f64::from_bits(CH[3]), f64::from_bits(CH[2]));
    let f1 = f_fmla(h2, f64::from_bits(CH[1]), f64::from_bits(CH[0]));

    let f = f_fmla(h4, f0, f1);
    al = dd_fmla(h, f, al);
    let ub = f_fmla(h, f64::from_bits(0x3ccf800000000000), al) + ah;
    let lb = f_fmla(-h, f64::from_bits(0x3ccf800000000000), al) + ah;
    // Ziv's accuracy test
    if lb != ub {
        return atan_refine(x, ub);
    }
    ub
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atan_test() {
        assert_eq!(f_atan(7.7877585082074305E-308), 7.78775850820743e-308);
        assert_eq!(f_atan(0.0), 0.0);
        assert_eq!(f_atan(1.0), 0.7853981633974483096156608458198);
        assert_eq!(f_atan(35.9), 1.542948374599341097473183563168947);
        assert_eq!(f_atan(-35.9), -1.542948374599341097473183563168947);
        assert_eq!(f_atan(f64::INFINITY), 1.5707963267948966);
    }
}
