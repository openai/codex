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
use crate::exponents::fast_ldexp;
use crate::shared_eval::poly_dekker_generic;
use std::hint::black_box;

static TZ: [(u64, u64); 65] = [
    (0xbc6797d4686c5393, 0xbfcc5041854df7d4),
    (0xbc8ea1cb9d163339, 0xbfcb881a23aebb48),
    (0x3c8f483a3e8cd60f, 0xbfcabe60e1f21838),
    (0x3c7dffd920f493db, 0xbfc9f3129931fab0),
    (0xbc851bfdbb129094, 0xbfc9262c1c3430a0),
    (0x3c8cd3e5225e2206, 0xbfc857aa375db4e4),
    (0x3c5e3a6bdaece8f9, 0xbfc78789b0a5e0c0),
    (0xbc8daf2ae0c2d3d4, 0xbfc6b5c7478983d8),
    (0xbc7fd36226fadd44, 0xbfc5e25fb4fde210),
    (0x3c7d887cd0341ab0, 0xbfc50d4fab639758),
    (0xbc8676a52a1a618b, 0xbfc43693d679612c),
    (0x3c79776b420ad283, 0xbfc35e28db4ecd9c),
    (0x3c73d5fd7d70a5ed, 0xbfc2840b5836cf68),
    (0x3c5a94ad2c8fa0bf, 0xbfc1a837e4ba3760),
    (0x3c26ad4c353465b0, 0xbfc0caab118a1278),
    (0xbc78bba170e59b65, 0xbfbfd6c2d0e3d910),
    (0xbc8e1e0a76cb0685, 0xbfbe14aed893eef0),
    (0x3c8fe131f55e75f8, 0xbfbc4f1331d22d40),
    (0xbc8b5beee8bcee31, 0xbfba85e8c62d9c10),
    (0xbc77fe9b02c25e9b, 0xbfb8b92870fa2b58),
    (0xbc832ae7bdaf1116, 0xbfb6e8caff341fe8),
    (0x3c7a6cfe58cbd73b, 0xbfb514c92f634788),
    (0x3c68798de3138a56, 0xbfb33d1bb17df2e8),
    (0xbc3589321a7ef10b, 0xbfb161bb26cbb590),
    (0xbc78d0e700fcfb65, 0xbfaf0540438fd5c0),
    (0x3c8473ef07d5dd3b, 0xbfab3f864c080000),
    (0xbc838e62149c16e2, 0xbfa7723950130400),
    (0xbc508bb6309bd394, 0xbfa39d4a1a77e050),
    (0xbc8bad3fd501a227, 0xbf9f8152aee94500),
    (0x3c63d27ac39ed253, 0xbf97b88f290230e0),
    (0xbc8b60bbd08aac55, 0xbf8fc055004416c0),
    (0xbc4a00d03b3359de, 0xbf7fe0154aaeed80),
    (0x0000000000000000, 0x0000000000000000),
    (0x3c8861931c15e39b, 0x3f80100ab00222c0),
    (0x3c77ab864b3e9045, 0x3f90202ad5778e40),
    (0x3c74e5659d75e95b, 0x3f984890d9043740),
    (0x3c78e0bd083aba81, 0x3fa040ac0224fd90),
    (0x3c345cc1cf959b1b, 0x3fa465509d383eb0),
    (0xbc8eb6980ce14da7, 0x3fa89246d053d180),
    (0x3c77324137d6c342, 0x3facc79f4f5613a0),
    (0xbc45272ff30eed1b, 0x3fb082b577d34ed8),
    (0xbc81280f19dace1c, 0x3fb2a5dd543ccc50),
    (0xbc8d550af31c8ec3, 0x3fb4cd4fc989cd68),
    (0x3c87923b72aa582d, 0x3fb6f91575870690),
    (0xbc776c2e732457f1, 0x3fb92937074e0cd8),
    (0x3c881f5c92a5200f, 0x3fbb5dbd3f681220),
    (0x3c8e8ac7a4d3206c, 0x3fbd96b0eff0e790),
    (0xbc712db6f4bbe33b, 0x3fbfd41afcba45e8),
    (0xbc58c4a5df1ec7e5, 0x3fc10b022db7ae68),
    (0xbc6bd4b1c37ea8a2, 0x3fc22e3b09dc54d8),
    (0x3c85aeb9860044d0, 0x3fc353bc9fb00b20),
    (0xbc64c26602c63fda, 0x3fc47b8b853aafec),
    (0xbc87f644c1f9d314, 0x3fc5a5ac59b963cc),
    (0x3c8f5aa8ec61fc2d, 0x3fc6d223c5b10638),
    (0x3c27ab912c69ffeb, 0x3fc800f67b00d7b8),
    (0xbc5b3564bc0ec9cd, 0x3fc9322934f54148),
    (0x3c86a7062465be33, 0x3fca65c0b85ac1a8),
    (0xbc885718d2ff1bf4, 0x3fcb9bc1d3910094),
    (0xbc8045cb0c685e08, 0x3fccd4315e9e0834),
    (0xbc16e7fb859d5055, 0x3fce0f143b41a554),
    (0x3c851bbdee020603, 0x3fcf4c6f5508ee5c),
    (0x3c6e17611afc42c5, 0x3fd04623d0b0f8c8),
    (0xbc71c5b2e8735a43, 0x3fd0e7510fd7c564),
    (0xbc825fe139c4cffd, 0x3fd189c1ecaeb084),
    (0xbc789843c4964554, 0x3fd22d78f0fa061a),
];

pub(crate) static EXPM1_T0: [(u64, u64); 64] = [
    (0x0000000000000000, 0x3ff0000000000000),
    (0xbc719083535b085e, 0x3ff02c9a3e778061),
    (0x3c8d73e2a475b466, 0x3ff059b0d3158574),
    (0x3c6186be4bb28500, 0x3ff0874518759bc8),
    (0x3c98a62e4adc610a, 0x3ff0b5586cf9890f),
    (0x3c403a1727c57b52, 0x3ff0e3ec32d3d1a2),
    (0xbc96c51039449b3a, 0x3ff11301d0125b51),
    (0xbc932fbf9af1369e, 0x3ff1429aaea92de0),
    (0xbc819041b9d78a76, 0x3ff172b83c7d517b),
    (0x3c8e5b4c7b4968e4, 0x3ff1a35beb6fcb75),
    (0x3c9e016e00a2643c, 0x3ff1d4873168b9aa),
    (0x3c8dc775814a8494, 0x3ff2063b88628cd6),
    (0x3c99b07eb6c70572, 0x3ff2387a6e756238),
    (0x3c82bd339940e9da, 0x3ff26b4565e27cdd),
    (0x3c8612e8afad1256, 0x3ff29e9df51fdee1),
    (0x3c90024754db41d4, 0x3ff2d285a6e4030b),
    (0x3c86f46ad23182e4, 0x3ff306fe0a31b715),
    (0x3c932721843659a6, 0x3ff33c08b26416ff),
    (0xbc963aeabf42eae2, 0x3ff371a7373aa9cb),
    (0xbc75e436d661f5e2, 0x3ff3a7db34e59ff7),
    (0x3c8ada0911f09ebc, 0x3ff3dea64c123422),
    (0xbc5ef3691c309278, 0x3ff4160a21f72e2a),
    (0x3c489b7a04ef80d0, 0x3ff44e086061892d),
    (0x3c73c1a3b69062f0, 0x3ff486a2b5c13cd0),
    (0x3c7d4397afec42e2, 0x3ff4bfdad5362a27),
    (0xbc94b309d25957e4, 0x3ff4f9b2769d2ca7),
    (0xbc807abe1db13cac, 0x3ff5342b569d4f82),
    (0x3c99bb2c011d93ac, 0x3ff56f4736b527da),
    (0x3c96324c054647ac, 0x3ff5ab07dd485429),
    (0x3c9ba6f93080e65e, 0x3ff5e76f15ad2148),
    (0xbc9383c17e40b496, 0x3ff6247eb03a5585),
    (0xbc9bb60987591c34, 0x3ff6623882552225),
    (0xbc9bdd3413b26456, 0x3ff6a09e667f3bcd),
    (0xbc6bbe3a683c88aa, 0x3ff6dfb23c651a2f),
    (0xbc816e4786887a9a, 0x3ff71f75e8ec5f74),
    (0xbc90245957316dd4, 0x3ff75feb564267c9),
    (0xbc841577ee049930, 0x3ff7a11473eb0187),
    (0x3c705d02ba15797e, 0x3ff7e2f336cf4e62),
    (0xbc9d4c1dd41532d8, 0x3ff82589994cce13),
    (0xbc9fc6f89bd4f6ba, 0x3ff868d99b4492ed),
    (0x3c96e9f156864b26, 0x3ff8ace5422aa0db),
    (0x3c85cc13a2e3976c, 0x3ff8f1ae99157736),
    (0xbc675fc781b57ebc, 0x3ff93737b0cdc5e5),
    (0xbc9d185b7c1b85d0, 0x3ff97d829fde4e50),
    (0x3c7c7c46b071f2be, 0x3ff9c49182a3f090),
    (0xbc9359495d1cd532, 0x3ffa0c667b5de565),
    (0xbc9d2f6edb8d41e2, 0x3ffa5503b23e255d),
    (0x3c90fac90ef7fd32, 0x3ffa9e6b5579fdbf),
    (0x3c97a1cd345dcc82, 0x3ffae89f995ad3ad),
    (0xbc62805e3084d708, 0x3ffb33a2b84f15fb),
    (0xbc75584f7e54ac3a, 0x3ffb7f76f2fb5e47),
    (0x3c823dd07a2d9e84, 0x3ffbcc1e904bc1d2),
    (0x3c811065895048de, 0x3ffc199bdd85529c),
    (0x3c92884dff483cac, 0x3ffc67f12e57d14b),
    (0x3c7503cbd1e949dc, 0x3ffcb720dcef9069),
    (0xbc9cbc3743797a9c, 0x3ffd072d4a07897c),
    (0x3c82ed02d75b3706, 0x3ffd5818dcfba487),
    (0x3c9c2300696db532, 0x3ffda9e603db3285),
    (0xbc91a5cd4f184b5c, 0x3ffdfc97337b9b5f),
    (0x3c839e8980a9cc90, 0x3ffe502ee78b3ff6),
    (0xbc9e9c23179c2894, 0x3ffea4afa2a490da),
    (0x3c9dc7f486a4b6b0, 0x3ffefa1bee615a27),
    (0x3c99d3e12dd8a18a, 0x3fff50765b6e4540),
    (0x3c874853f3a5931e, 0x3fffa7c1819e90d8),
];

pub(crate) static EXPM1_T1: [(u64, u64); 64] = [
    (0x0000000000000000, 0x3ff0000000000000),
    (0x3c9ae8e38c59c72a, 0x3ff000b175effdc7),
    (0xbc57b5d0d58ea8f4, 0x3ff00162f3904052),
    (0x3c94115cb6b16a8e, 0x3ff0021478e11ce6),
    (0xbc8d7c96f201bb2e, 0x3ff002c605e2e8cf),
    (0x3c984711d4c35ea0, 0x3ff003779a95f959),
    (0xbc80484245243778, 0x3ff0042936faa3d8),
    (0xbc94b237da2025fa, 0x3ff004dadb113da0),
    (0xbc75e00e62d6b30e, 0x3ff0058c86da1c0a),
    (0x3c9a1d6cedbb9480, 0x3ff0063e3a559473),
    (0xbc94acf197a00142, 0x3ff006eff583fc3d),
    (0xbc6eaf2ea42391a6, 0x3ff007a1b865a8ca),
    (0x3c7da93f90835f76, 0x3ff0085382faef83),
    (0xbc86a79084ab093c, 0x3ff00905554425d4),
    (0x3c986364f8fbe8f8, 0x3ff009b72f41a12b),
    (0xbc882e8e14e3110e, 0x3ff00a6910f3b6fd),
    (0xbc84f6b2a7609f72, 0x3ff00b1afa5abcbf),
    (0xbc7e1a258ea8f71a, 0x3ff00bcceb7707ec),
    (0x3c74362ca5bc26f2, 0x3ff00c7ee448ee02),
    (0x3c9095a56c919d02, 0x3ff00d30e4d0c483),
    (0xbc6406ac4e81a646, 0x3ff00de2ed0ee0f5),
    (0x3c9b5a6902767e08, 0x3ff00e94fd0398e0),
    (0xbc991b2060859320, 0x3ff00f4714af41d3),
    (0x3c8427068ab22306, 0x3ff00ff93412315c),
    (0x3c9c1d0660524e08, 0x3ff010ab5b2cbd11),
    (0xbc9e7bdfb3204be8, 0x3ff0115d89ff3a8b),
    (0x3c8843aa8b9cbbc6, 0x3ff0120fc089ff63),
    (0xbc734104ee7edae8, 0x3ff012c1fecd613b),
    (0xbc72b6aeb6176892, 0x3ff0137444c9b5b5),
    (0x3c7a8cd33b8a1bb2, 0x3ff01426927f5278),
    (0x3c72edc08e5da99a, 0x3ff014d8e7ee8d2f),
    (0x3c857ba2dc7e0c72, 0x3ff0158b4517bb88),
    (0x3c9b61299ab8cdb8, 0x3ff0163da9fb3335),
    (0xbc990565902c5f44, 0x3ff016f0169949ed),
    (0x3c870fc41c5c2d54, 0x3ff017a28af25567),
    (0x3c94b9a6e145d76c, 0x3ff018550706ab62),
    (0xbc7008eff5142bfa, 0x3ff019078ad6a19f),
    (0xbc977669f033c7de, 0x3ff019ba16628de2),
    (0xbc909bb78eeead0a, 0x3ff01a6ca9aac5f3),
    (0x3c9371231477ece6, 0x3ff01b1f44af9f9e),
    (0x3c75e7626621eb5a, 0x3ff01bd1e77170b4),
    (0xbc9bc72b100828a4, 0x3ff01c8491f08f08),
    (0xbc6ce39cbbab8bbe, 0x3ff01d37442d5070),
    (0x3c816996709da2e2, 0x3ff01de9fe280ac8),
    (0xbc8c11f5239bf536, 0x3ff01e9cbfe113ef),
    (0x3c8e1d4eb5edc6b4, 0x3ff01f4f8958c1c6),
    (0xbc9afb99946ee3f0, 0x3ff020025a8f6a35),
    (0xbc98f06d8a148a32, 0x3ff020b533856324),
    (0xbc82bf310fc54eb6, 0x3ff02168143b0281),
    (0xbc9c95a035eb4176, 0x3ff0221afcb09e3e),
    (0xbc9491793e46834c, 0x3ff022cdece68c4f),
    (0xbc73e8d0d9c49090, 0x3ff02380e4dd22ad),
    (0xbc9314aa16278aa4, 0x3ff02433e494b755),
    (0x3c848daf888e9650, 0x3ff024e6ec0da046),
    (0x3c856dc8046821f4, 0x3ff02599fb483385),
    (0x3c945b42356b9d46, 0x3ff0264d1244c719),
    (0xbc7082ef51b61d7e, 0x3ff027003103b10e),
    (0x3c72106ed0920a34, 0x3ff027b357854772),
    (0xbc9fd4cf26ea5d0e, 0x3ff0286685c9e059),
    (0xbc909f8775e78084, 0x3ff02919bbd1d1d8),
    (0x3c564cbba902ca28, 0x3ff029ccf99d720a),
    (0x3c94383ef231d206, 0x3ff02a803f2d170d),
    (0x3c94a47a505b3a46, 0x3ff02b338c811703),
    (0x3c9e471202234680, 0x3ff02be6e199c811),
];

static EXPM1_DD1: [(u64, u64); 11] = [
    (0x3c65555555555554, 0x3fc5555555555555),
    (0x3c45555555555123, 0x3fa5555555555555),
    (0x3c01111111118167, 0x3f81111111111111),
    (0xbbef49f49e220cea, 0x3f56c16c16c16c17),
    (0x3b6a019eff6f919c, 0x3f2a01a01a01a01a),
    (0x3b39fcff48a75b41, 0x3efa01a01a01a01a),
    (0xbb6c14f73758cd7f, 0x3ec71de3a556c734),
    (0x3b3dfce97931018f, 0x3e927e4fb7789f5c),
    (0x3afc513da9e4c9c5, 0x3e5ae64567f544e3),
    (0x3acca00af84f2b60, 0x3e21eed8eff8d831),
    (0x3a8f27ac6000898f, 0x3de6124613a86e8f),
];

static EXPM1_DD2: [(u64, u64); 7] = [
    (0x3ff0000000000000, 0x0000000000000000),
    (0x3fe0000000000000, 0x39c712f72ecec2cf),
    (0x3fc5555555555555, 0x3c65555555554d07),
    (0x3fa5555555555555, 0x3c455194d28275da),
    (0x3f81111111111111, 0x3c012faa0e1c0f7b),
    (0x3f56c16c16da6973, 0xbbf4ba45ab25d2a3),
    (0x3f2a01a019eb7f31, 0xbbc9091d845ecd36),
];

#[inline]
pub(crate) fn opoly_dd_generic<const N: usize>(
    x: DoubleDouble,
    poly: [(u64, u64); N],
) -> DoubleDouble {
    let zch = poly.last().unwrap();
    let p0 = DoubleDouble::from_exact_add(f64::from_bits(zch.0), x.lo);
    let ach = p0.hi;
    let acl = f64::from_bits(zch.1) + p0.lo;
    let mut ch = DoubleDouble::new(acl, ach);

    for zch in poly.iter().rev().skip(1) {
        ch = DoubleDouble::quick_mult_f64(ch, x.hi);
        let z0 = DoubleDouble::from_bit_pair(*zch);
        ch = DoubleDouble::add(z0, ch);
    }

    ch
}

#[cold]
fn as_expm1_accurate(x: f64) -> f64 {
    let mut ix;
    if x.abs() < 0.25 {
        const CL: [u64; 6] = [
            0x3da93974a8ca5354,
            0x3d6ae7f3e71e4908,
            0x3d2ae7f357341648,
            0x3ce952c7f96664cb,
            0x3ca686f8ce633aae,
            0x3c62f49b2fbfb5b6,
        ];

        let fl0 = f_fmla(x, f64::from_bits(CL[5]), f64::from_bits(CL[4]));
        let fl1 = f_fmla(x, fl0, f64::from_bits(CL[3]));
        let fl2 = f_fmla(x, fl1, f64::from_bits(CL[2]));
        let fl3 = f_fmla(x, fl2, f64::from_bits(CL[1]));

        let fl = x * f_fmla(x, fl3, f64::from_bits(CL[0]));
        let mut f = opoly_dd_generic(DoubleDouble::new(fl, x), EXPM1_DD1);
        f = DoubleDouble::quick_mult_f64(f, x);
        f = DoubleDouble::quick_mult_f64(f, x);
        f = DoubleDouble::quick_mult_f64(f, x);
        let hx = 0.5 * x;
        let dx2dd = DoubleDouble::from_exact_mult(x, hx);
        f = DoubleDouble::add(dx2dd, f);
        let v0 = DoubleDouble::from_exact_add(x, f.hi);
        let v1 = DoubleDouble::from_exact_add(v0.lo, f.lo);
        let v2 = DoubleDouble::from_exact_add(v0.hi, v1.hi);
        let mut v3 = DoubleDouble::from_exact_add(v2.lo, v1.lo);
        ix = v3.hi.to_bits();
        if (ix & 0x000fffffffffffff) == 0 {
            let v = v3.lo.to_bits();
            let d: i64 = ((((ix as i64) >> 63) ^ ((v as i64) >> 63)) as u64)
                .wrapping_shl(1)
                .wrapping_add(1) as i64;
            ix = ix.wrapping_add(d as u64);
            v3.hi = f64::from_bits(ix);
        }
        v3.hi + v2.hi
    } else {
        const S: f64 = f64::from_bits(0x40b71547652b82fe);
        let t = (x * S).round_ties_even();
        let jt: i64 = t as i64;
        let i0 = (jt >> 6) & 0x3f;
        let i1 = jt & 0x3f;
        let ie = jt >> 12;
        let t0 = DoubleDouble::from_bit_pair(EXPM1_T0[i0 as usize]);
        let t1 = DoubleDouble::from_bit_pair(EXPM1_T1[i1 as usize]);

        let bt = DoubleDouble::quick_mult(t0, t1);

        const L2H: f64 = f64::from_bits(0x3f262e42ff000000);
        const L2L: f64 = f64::from_bits(0x3d0718432a1b0e26);
        const L2LL: f64 = f64::from_bits(0x3999ff0342542fc3);

        let dx = f_fmla(-L2H, t, x);
        let dxl = L2L * t;
        let dxll = f_fmla(L2LL, t, dd_fmla(L2L, t, -dxl));
        let dxh = dx + dxl;
        let dxl = (dx - dxh) + dxl + dxll;
        let mut f = poly_dekker_generic(DoubleDouble::new(dxl, dxh), EXPM1_DD2);
        f = DoubleDouble::quick_mult(DoubleDouble::new(dxl, dxh), f);
        f = DoubleDouble::quick_mult(f, bt);
        f = DoubleDouble::add(bt, f);
        let off: u64 = (2048i64 + 1023i64).wrapping_sub(ie).wrapping_shl(52) as u64;
        let e: f64;
        if ie < 53 {
            let fhz = DoubleDouble::from_exact_add(f64::from_bits(off), f.hi);
            f.hi = fhz.hi;
            e = fhz.lo;
        } else if ie < 104 {
            let fhz = DoubleDouble::from_exact_add(f.hi, f64::from_bits(off));
            f.hi = fhz.hi;
            e = fhz.lo;
        } else {
            e = 0.;
        }
        f.lo += e;
        let dst = DoubleDouble::from_exact_add(f.hi, f.lo);
        fast_ldexp(dst.hi, ie as i32)
    }
}

/// Computes e^x - 1
///
/// Max found ULP 0.5
pub fn f_expm1(x: f64) -> f64 {
    let ix = x.to_bits();
    let aix: u64 = ix & 0x7fff_ffff_ffff_ffff;
    if aix < 0x3fd0000000000000u64 {
        if aix < 0x3ca0000000000000u64 {
            if aix == 0 {
                return x;
            }
            return dyad_fmla(f64::from_bits(0x3c90000000000000), x.abs(), x);
        }
        let sx = f64::from_bits(0x4060000000000000) * x;
        let fx = sx.round_ties_even();
        let z = sx - fx;
        let z2 = z * z;
        let i: i64 = fx as i64;
        let t = DoubleDouble::from_bit_pair(TZ[i.wrapping_add(32) as usize]);
        const C: [u64; 6] = [
            0x3f80000000000000,
            0x3f00000000000000,
            0x3e755555555551ad,
            0x3de555555555599c,
            0x3d511111ad1ad69d,
            0x3cb6c16c168b1fb5,
        ];
        let fh = z * f64::from_bits(C[0]);

        let fl0 = f_fmla(z, f64::from_bits(C[5]), f64::from_bits(C[4]));
        let fl1 = f_fmla(z, f64::from_bits(C[2]), f64::from_bits(C[1]));

        let fw0 = f_fmla(z, fl0, f64::from_bits(C[3]));

        let fl = z2 * f_fmla(z2, fw0, fl1);
        let mut f = DoubleDouble::new(fl, fh);
        let e0 = f64::from_bits(0x3bea000000000000);
        let eps = z2 * e0 + f64::from_bits(0x3970000000000000);
        let mut r = DoubleDouble::from_exact_add(t.hi, f.hi);
        r.lo += t.lo + f.lo;
        f = DoubleDouble::quick_mult(t, f);
        f = DoubleDouble::add(r, f);
        let ub = f.hi + (f.lo + eps);
        let lb = f.hi + (f.lo - eps);
        if ub != lb {
            return as_expm1_accurate(x);
        }
        lb
    } else {
        if aix >= 0x40862e42fefa39f0u64 {
            if aix > 0x7ff0000000000000u64 {
                return x + x;
            } // nan
            if aix == 0x7ff0000000000000u64 {
                return if (ix >> 63) != 0 { -1.0 } else { x };
            }
            if (ix >> 63) == 0 {
                const Z: f64 = f64::from_bits(0x7fe0000000000000);
                return black_box(Z) * black_box(Z);
            }
        }
        if ix >= 0xc0425e4f7b2737fau64 {
            if ix >= 0xc042b708872320e2u64 {
                return black_box(-1.0) + black_box(f64::from_bits(0x3c80000000000000));
            }
            return (f64::from_bits(0x40425e4f7b2737fa) + x + f64::from_bits(0x3cc8486612173c69))
                * f64::from_bits(0x3c971547652b82fe)
                - f64::from_bits(0x3fefffffffffffff);
        }

        const S: f64 = f64::from_bits(0x40b71547652b82fe);
        let t = (x * S).round_ties_even();
        let jt: i64 = t as i64;
        let i0 = (jt >> 6) & 0x3f;
        let i1 = jt & 0x3f;
        let ie = jt >> 12;
        let t0 = DoubleDouble::from_bit_pair(EXPM1_T0[i0 as usize]);
        let t1 = DoubleDouble::from_bit_pair(EXPM1_T1[i1 as usize]);

        let bt = DoubleDouble::quick_mult(t0, t1);

        const L2H: f64 = f64::from_bits(0x3f262e42ff000000);
        const L2L: f64 = f64::from_bits(0x3d0718432a1b0e26);
        let dx = dd_fmla(L2L, t, f_fmla(-L2H, t, x));
        let dx2 = dx * dx;

        const CH: [u64; 4] = [
            0x3ff0000000000000,
            0x3fe0000000000000,
            0x3fc55555557e54ff,
            0x3fa55555553a12f4,
        ];

        let p0 = f_fmla(dx, f64::from_bits(CH[3]), f64::from_bits(CH[2]));
        let p1 = f_fmla(dx, f64::from_bits(CH[1]), f64::from_bits(CH[0]));

        let p = f_fmla(dx2, p0, p1);
        let mut fh = bt.hi;
        let tx = bt.hi * dx;
        let mut fl = f_fmla(tx, p, bt.lo);
        let eps = f64::from_bits(0x3c0833beace2b6fe) * bt.hi;

        let off: u64 = ((2048i64 + 1023i64).wrapping_sub(ie) as u64).wrapping_shl(52);
        let e: f64;
        if ie < 53 {
            let flz = DoubleDouble::from_exact_add(f64::from_bits(off), fh);
            e = flz.lo;
            fh = flz.hi;
        } else if ie < 75 {
            let flz = DoubleDouble::from_exact_add(fh, f64::from_bits(off));
            e = flz.lo;
            fh = flz.hi;
        } else {
            e = 0.;
        }
        fl += e;
        let ub = fh + (fl + eps);
        let lb = fh + (fl - eps);
        if ub != lb {
            return as_expm1_accurate(x);
        }
        fast_ldexp(lb, ie as i32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expm1() {
        assert_eq!(f_expm1(0.0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000028321017343872864),
                   0.0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000028321017343872864 );
        assert_eq!(f_expm1(36.52188110363568), 7265264535836525.);
        assert_eq!(f_expm1(2.), 6.38905609893065);
        assert_eq!(f_expm1(0.4321321), 0.5405386068701713);
        assert_eq!(f_expm1(-0.0000004321321), -4.321320066309375e-7);
    }
}
