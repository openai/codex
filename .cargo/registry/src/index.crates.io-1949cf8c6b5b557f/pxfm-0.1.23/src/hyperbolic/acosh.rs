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
use crate::double_double::DoubleDouble;

pub(crate) static ACOSH_ASINH_LL: [[(u64, u64, u64); 17]; 4] = [
    [
        (0x0000000000000000, 0x0000000000000000, 0x0000000000000000),
        (0x3f962e432b240000, 0xbd5745af34bb54b8, 0xb9e17e3ec05cde70),
        (0x3fa62e42e4a80000, 0x3d3111a4eadf3120, 0x3a2cff3027abb119),
        (0x3fb0a2b233f10000, 0xbd588ac4ec78af80, 0x3a24fa087ca75dfd),
        (0x3fb62e43056c0000, 0x3d16bd65e8b0b700, 0xba0b18e160362c24),
        (0x3fbbb9d3cbd60000, 0x3d5de14aa55ec2b0, 0xba1c6ac3f1862a6b),
        (0x3fc0a2b244da0000, 0x3d594def487fea70, 0xba1dead1a4581acf),
        (0x3fc3687aa9b78000, 0x3d49cec9a50db220, 0x3a234a70684f8e0e),
        (0x3fc62e42faba0000, 0xbd3d69047a3aeb00, 0xba04e061f79144e2),
        (0x3fc8f40b56d28000, 0x3d5de7d755fd2e20, 0x3a1bdc7ecf001489),
        (0x3fcbb9d3b61f0000, 0x3d1c14f1445b1200, 0x3a2a1d78cbdc5b58),
        (0x3fce7f9c11f08000, 0xbd46e3e0000dae70, 0x3a16a4559fadde98),
        (0x3fd0a2b242ec4000, 0x3d5bb7cf852a5fe8, 0x3a2a6aef11ee43bd),
        (0x3fd205966c764000, 0x3d2ad3a5f2142940, 0x3a25cc344fa10652),
        (0x3fd3687a98aac000, 0x3d21623671842f00, 0xba10b428fe1f9e43),
        (0x3fd4cb5ec93f4000, 0x3d53d50980ea5130, 0x3a267f0ea083b1c4),
        (0x3fd62e42fefa4000, 0xbd38432a1b0e2640, 0x3a2803f2f6af40f3),
    ],
    [
        (0x0000000000000000, 0x0000000000000000, 0x0000000000000000),
        (0x3f562e462b400000, 0x3d5061d003b97318, 0x3a2d7faee66a2e1e),
        (0x3f662e44c9200000, 0x3d595a7bff5e2390, 0xba0f7e788a871350),
        (0x3f70a2b1e3300000, 0x3d42a3a1a65aa3a0, 0xba254599c9605442),
        (0x3f762e4367c00000, 0xbd24a995b6d9ddc0, 0xb9b56bb79b254f33),
        (0x3f7bb9d449a00000, 0x3d58a119c42e9bc0, 0xba28ecf7d8d661f1),
        (0x3f80a2b1f1900000, 0x3d58863771bd10a8, 0x3a1e9731de7f0155),
        (0x3f83687ad1100000, 0x3d5e026a347ca1c8, 0x39efadc62522444d),
        (0x3f862e436f280000, 0x3d525b84f71b70b8, 0xb9ffcb3f98612d27),
        (0x3f88f40b7b380000, 0xbd462a0a4fd47580, 0x3a23cb3c35d9f6a1),
        (0x3f8bb9d3abb00000, 0xbd50ec48f94d7860, 0xba26b47d410e4cc7),
        (0x3f8e7f9bb2300000, 0x3d4e4415cbc97a00, 0xba23729fdb677231),
        (0x3f90a2b224780000, 0xbd5cb73f4505b030, 0xba21b3b3a3bc370a),
        (0x3f92059691e80000, 0xbd4abcc3412f2640, 0xba0fe6e998e48673),
        (0x3f93687a76800000, 0xbd543901e5c97a90, 0x39fb54cdd52a5d88),
        (0x3f94cb5eb5d80000, 0xbd58f106f00f13b8, 0xba28f793f5fce148),
        (0x3f962e432b240000, 0xbd5745af34bb54b8, 0xb9e17e3ec05cde70),
    ],
    [
        (0x0000000000000000, 0x0000000000000000, 0x0000000000000000),
        (0x3f162e7b00000000, 0xbd3868625640a680, 0xba234bf0db910f65),
        (0x3f262e35f6000000, 0xbd42ee3d96b696a0, 0x3a1a2948cd558655),
        (0x3f30a2b4b2000000, 0x3d053edbcf116500, 0xb9ecfc26ccf6d0e4),
        (0x3f362e4be1000000, 0x3cb783e334614000, 0xba204b96da30e63a),
        (0x3f3bb9e085000000, 0xbd460785f20acb20, 0xb9ff33369bf7dff1),
        (0x3f40a2b94d000000, 0x3d5fd4b3a2733530, 0xb9f685a35575eff1),
        (0x3f4368810f800000, 0x3d07ded26dc81300, 0xb9f4c4d1abca79bf),
        (0x3f462e4787800000, 0x3d57d2bee9a1f630, 0x3a2860233b7ad130),
        (0x3f48f40cb4800000, 0xbd5af034eaf471c0, 0x3a1ae748822d57b7),
        (0x3f4bb9d094000000, 0xbd57a223013a20f0, 0xba21e499087075b6),
        (0x3f4e7fa32c800000, 0xbd4b2e67b1b59bd0, 0xba254a41eda30fa6),
        (0x3f50a2b237000000, 0xbd37ad97ff4ac7a0, 0x3a2f932da91371dd),
        (0x3f52059a33800000, 0xbd396422d90df400, 0xba190800fbbf2ed3),
        (0x3f53687982400000, 0x3d30f90540018120, 0x3a29567e01e48f9a),
        (0x3f54cb602c000000, 0xbd40d709a5ec0b50, 0x3a1253dfd44635d2),
        (0x3f562e462b400000, 0x3d5061d003b97318, 0x3a2d7faee66a2e1e),
    ],
    [
        (0x0000000000000000, 0x0000000000000000, 0x0000000000000000),
        (0x3ed63007c0000000, 0xbd4db0e38e5aaaa0, 0x3a2259a7b94815b9),
        (0x3ee6300f60000000, 0x3d32b1c755804380, 0x3a278cabba01e3e4),
        (0x3ef0a21150000000, 0xbd55ff2237307590, 0x3a08074feacfe49d),
        (0x3ef62e1ec0000000, 0xbd285d6f6487ce40, 0x3a205485074b9276),
        (0x3efbba3010000000, 0xbd4af5d58a7c9210, 0xba230a8c0fd2ff5f),
        (0x3f00a32298000000, 0x3d4590faa0883bd0, 0x3a295e9bda999947),
        (0x3f03682f10000000, 0x3d5f0224376efaf8, 0xba25843c0db50d10),
        (0x3f062e3d80000000, 0xbd4142c13daed4a0, 0x3a2c68a61183ce87),
        (0x3f08f44dd8000000, 0xbd4aa489f3999310, 0x3a111c5c376854ea),
        (0x3f0bb96010000000, 0x3d59904d8b6a3638, 0x3a28c89554493c8f),
        (0x3f0e7f7440000000, 0x3d55785ddbe7cba8, 0x3a1e7ff3cde7d70c),
        (0x3f10a2c530000000, 0xbd46d9e8780d0d50, 0x3a1ad9c178106693),
        (0x3f1205d134000000, 0xbd4214a2e893fcc0, 0x3a2548a9500c9822),
        (0x3f13685e28000000, 0x3d4e235886461030, 0x3a12a97b26da2d88),
        (0x3f14cb6c18000000, 0x3d52b7cfcea9e0d8, 0xba25095048a6b824),
        (0x3f162e7b00000000, 0xbd3868625640a680, 0xba234bf0db910f65),
    ],
];

pub(crate) static ACOSH_SINH_REFINE_T1: [u64; 17] = [
    0x3ff0000000000000,
    0x3feea4afa0000000,
    0x3fed5818e0000000,
    0x3fec199be0000000,
    0x3feae89f98000000,
    0x3fe9c49180000000,
    0x3fe8ace540000000,
    0x3fe7a11470000000,
    0x3fe6a09e68000000,
    0x3fe5ab07e0000000,
    0x3fe4bfdad8000000,
    0x3fe3dea650000000,
    0x3fe306fe08000000,
    0x3fe2387a70000000,
    0x3fe172b840000000,
    0x3fe0b55870000000,
    0x3fe0000000000000,
];

pub(crate) static ACOSH_ASINH_REFINE_T2: [u64; 16] = [
    0x3ff0000000000000,
    0x3fefe9d968000000,
    0x3fefd3c228000000,
    0x3fefbdba38000000,
    0x3fefa7c180000000,
    0x3fef91d800000000,
    0x3fef7bfdb0000000,
    0x3fef663278000000,
    0x3fef507658000000,
    0x3fef3ac948000000,
    0x3fef252b38000000,
    0x3fef0f9c20000000,
    0x3feefa1bf0000000,
    0x3feee4aaa0000000,
    0x3feecf4830000000,
    0x3feeb9f488000000,
];

pub(crate) static ACOSH_SINH_REFINE_T3: [u64; 16] = [
    0x3ff0000000000000,
    0x3feffe9d20000000,
    0x3feffd3a58000000,
    0x3feffbd798000000,
    0x3feffa74e8000000,
    0x3feff91248000000,
    0x3feff7afb8000000,
    0x3feff64d38000000,
    0x3feff4eac8000000,
    0x3feff38868000000,
    0x3feff22618000000,
    0x3feff0c3d0000000,
    0x3fefef61a0000000,
    0x3fefedff78000000,
    0x3fefec9d68000000,
    0x3fefeb3b60000000,
];

pub(crate) static ACOSH_ASINH_REFINE_T4: [u64; 16] = [
    0x3ff0000000000000,
    0x3fefffe9d0000000,
    0x3fefffd3a0000000,
    0x3fefffbd78000000,
    0x3fefffa748000000,
    0x3fefff9118000000,
    0x3fefff7ae8000000,
    0x3fefff64c0000000,
    0x3fefff4e90000000,
    0x3fefff3860000000,
    0x3fefff2238000000,
    0x3fefff0c08000000,
    0x3feffef5d8000000,
    0x3feffedfa8000000,
    0x3feffec980000000,
    0x3feffeb350000000,
];

#[cold]
fn acosh_refine(x: f64, a: f64) -> f64 {
    let ix = x.to_bits();

    let z: DoubleDouble = if ix < 0x4190000000000000u64 {
        let dx2 = DoubleDouble::from_exact_mult(x, x);
        let w = DoubleDouble::from_exact_add(dx2.hi - 1., dx2.lo);
        let sh = w.hi.sqrt();
        let ish = 0.5 / w.hi;
        let sl = (ish * sh) * (w.lo - dd_fmla(sh, sh, -w.hi));
        let mut p = DoubleDouble::from_exact_add(x, sh);
        p.lo += sl;
        DoubleDouble::from_exact_add(p.hi, p.lo)
    } else if ix < 0x4330000000000000 {
        DoubleDouble::new(-0.5 / x, 2. * x)
    } else {
        DoubleDouble::new(0., x)
    };

    let mut t = z.hi.to_bits();
    let ex: i32 = (t >> 52) as i32;
    let e = ex.wrapping_sub(0x3ff) + if z.lo == 0.0 { 1i32 } else { 0i32 };
    t &= 0x000fffffffffffff;
    t |= 0x3ffu64 << 52;
    let ed = e as f64;
    let v = (a - ed + f64::from_bits(0x3ff0000800000000)).to_bits();
    let i = (v.wrapping_sub(0x3ffu64 << 52)) >> (52 - 16);
    let i1 = (i >> 12) & 0x1f;
    let i2 = (i >> 8) & 0xf;
    let i3 = (i >> 4) & 0xf;
    let i4 = i & 0xf;
    const L20: f64 = f64::from_bits(0x3fd62e42fefa3800);
    const L21: f64 = f64::from_bits(0x3d1ef35793c76800);
    const L22: f64 = f64::from_bits(0xba49ff0342542fc3);
    let el2 = L22 * ed;
    let el1 = L21 * ed;
    let el0 = L20 * ed;
    let mut dl0: f64;

    let ll0i1 = ACOSH_ASINH_LL[0][i1 as usize];
    let ll1i2 = ACOSH_ASINH_LL[1][i2 as usize];
    let ll2i3 = ACOSH_ASINH_LL[2][i3 as usize];
    let ll3i4 = ACOSH_ASINH_LL[3][i4 as usize];

    dl0 = f64::from_bits(ll0i1.0)
        + f64::from_bits(ll1i2.0)
        + (f64::from_bits(ll2i3.0) + f64::from_bits(ll3i4.0));
    let dl1 = f64::from_bits(ll0i1.1)
        + f64::from_bits(ll1i2.1)
        + (f64::from_bits(ll2i3.1) + f64::from_bits(ll3i4.1));
    let dl2 = f64::from_bits(ll0i1.2)
        + f64::from_bits(ll1i2.2)
        + (f64::from_bits(ll2i3.2) + f64::from_bits(ll3i4.2));
    dl0 += el0;
    let t12 = f64::from_bits(ACOSH_SINH_REFINE_T1[i1 as usize])
        * f64::from_bits(ACOSH_ASINH_REFINE_T2[i2 as usize]);
    let t34 = f64::from_bits(ACOSH_SINH_REFINE_T3[i3 as usize])
        * f64::from_bits(ACOSH_ASINH_REFINE_T4[i4 as usize]);
    let th = t12 * t34;
    let tl = dd_fmla(t12, t34, -th);
    let dh = th * f64::from_bits(t);
    let dl = dd_fmla(th, f64::from_bits(t), -dh);
    let sh = tl * f64::from_bits(t);
    let sl = dd_fmla(tl, f64::from_bits(t), -sh);

    let mut dx = DoubleDouble::from_exact_add(dh - 1., dl);
    if z.lo != 0.0 {
        t = z.lo.to_bits();
        t = t.wrapping_sub((e as i64).wrapping_shl(52) as u64);
        dx.lo += th * f64::from_bits(t);
    }
    dx = DoubleDouble::add(dx, DoubleDouble::new(sl, sh));
    const CL: [u64; 3] = [0xbfc0000000000000, 0x3fb9999999a0754f, 0xbfb55555555c3157];
    let sl = dx.hi
        * (f64::from_bits(CL[0]) + dx.hi * (f64::from_bits(CL[1]) + dx.hi * f64::from_bits(CL[2])));
    const CH: [(u64, u64); 3] = [
        (0x39024b67ee516e3b, 0x3fe0000000000000),
        (0xb91932ce43199a8d, 0xbfd0000000000000),
        (0x3c655540c15cf91f, 0x3fc5555555555555),
    ];
    let mut s = lpoly_xd_generic(dx, CH, sl);
    s = DoubleDouble::quick_mult(dx, s);
    s = DoubleDouble::add(s, DoubleDouble::new(el2, el1));
    s = DoubleDouble::add(s, DoubleDouble::new(dl2, dl1));
    let mut v02 = DoubleDouble::from_exact_add(dl0, s.hi);
    let mut v12 = DoubleDouble::from_exact_add(v02.lo, s.lo);
    v02.hi *= 2.;
    v12.hi *= 2.;
    v12.lo *= 2.;
    t = v12.hi.to_bits();
    if (t & 0x000fffffffffffff) == 0 {
        let w = v12.lo.to_bits();
        if ((w ^ t) >> 63) != 0 {
            t = t.wrapping_sub(1);
        } else {
            t = t.wrapping_add(1);
        }
        v12.hi = f64::from_bits(t);
    }
    v02.hi + v12.hi
}

pub(crate) static ACOSH_ASINH_B: [[i32; 2]; 32] = [
    [301, 27565],
    [7189, 24786],
    [13383, 22167],
    [18923, 19696],
    [23845, 17361],
    [28184, 15150],
    [31969, 13054],
    [35231, 11064],
    [37996, 9173],
    [40288, 7372],
    [42129, 5657],
    [43542, 4020],
    [44546, 2457],
    [45160, 962],
    [45399, -468],
    [45281, -1838],
    [44821, -3151],
    [44032, -4412],
    [42929, -5622],
    [41522, -6786],
    [39825, -7905],
    [37848, -8982],
    [35602, -10020],
    [33097, -11020],
    [30341, -11985],
    [27345, -12916],
    [24115, -13816],
    [20661, -14685],
    [16989, -15526],
    [13107, -16339],
    [9022, -17126],
    [4740, -17889],
];

pub(crate) static ACOSH_ASINH_R1: [u64; 33] = [
    0x3ff0000000000000,
    0x3fef507600000000,
    0x3feea4b000000000,
    0x3fedfc9800000000,
    0x3fed581800000000,
    0x3fecb72000000000,
    0x3fec199c00000000,
    0x3feb7f7600000000,
    0x3feae8a000000000,
    0x3fea550400000000,
    0x3fe9c49200000000,
    0x3fe9373800000000,
    0x3fe8ace600000000,
    0x3fe8258a00000000,
    0x3fe7a11400000000,
    0x3fe71f7600000000,
    0x3fe6a09e00000000,
    0x3fe6247e00000000,
    0x3fe5ab0800000000,
    0x3fe5342c00000000,
    0x3fe4bfda00000000,
    0x3fe44e0800000000,
    0x3fe3dea600000000,
    0x3fe371a800000000,
    0x3fe306fe00000000,
    0x3fe29e9e00000000,
    0x3fe2387a00000000,
    0x3fe1d48800000000,
    0x3fe172b800000000,
    0x3fe1130200000000,
    0x3fe0b55800000000,
    0x3fe059b000000000,
    0x3fe0000000000000,
];

pub(crate) static ACOSH_ASINH_R2: [u64; 33] = [
    0x3ff0000000000000,
    0x3feffa7400000000,
    0x3feff4ea00000000,
    0x3fefef6200000000,
    0x3fefe9da00000000,
    0x3fefe45200000000,
    0x3fefdecc00000000,
    0x3fefd94600000000,
    0x3fefd3c200000000,
    0x3fefce3e00000000,
    0x3fefc8bc00000000,
    0x3fefc33a00000000,
    0x3fefbdba00000000,
    0x3fefb83a00000000,
    0x3fefb2bc00000000,
    0x3fefad3e00000000,
    0x3fefa7c200000000,
    0x3fefa24600000000,
    0x3fef9cca00000000,
    0x3fef975000000000,
    0x3fef91d800000000,
    0x3fef8c6000000000,
    0x3fef86e800000000,
    0x3fef817200000000,
    0x3fef7bfe00000000,
    0x3fef768a00000000,
    0x3fef711600000000,
    0x3fef6ba400000000,
    0x3fef663200000000,
    0x3fef60c200000000,
    0x3fef5b5200000000,
    0x3fef55e400000000,
    0x3fef507600000000,
];

pub(crate) static ACOSH_ASINH_L1: [(u64, u64); 33] = [
    (0x0000000000000000, 0x0000000000000000),
    (0xbd1269e2038315b3, 0x3f962e4eacd40000),
    (0xbd23f2558bddfc47, 0x3fa62e3ce7218000),
    (0x3d207ea13c34efb5, 0x3fb0a2ab6d3ec000),
    (0x3d38f3e77084d3ba, 0x3fb62e4a86d8c000),
    (0xbd18d92a005f1a7e, 0x3fbbb9db7062c000),
    (0x3d358239e799bfe5, 0x3fc0a2b1a22cc000),
    (0xbd3a93fcf5f593b7, 0x3fc3687f0a298000),
    (0xbd1db4cac32fd2b5, 0x3fc62e4116b64000),
    (0xbd10e65a92ee0f3b, 0x3fc8f409e4df6000),
    (0xbd38261383d475f1, 0x3fcbb9d15001c000),
    (0xbd3359886207513b, 0x3fce7f9a8c940000),
    (0x3d3811f87496ceb7, 0x3fd0a2b052ddb000),
    (0x3d34991ec6cb435c, 0x3fd205955ef73000),
    (0xbd34581abfeb8927, 0x3fd3687bd9121000),
    (0x3d3cab48f6942703, 0x3fd4cb5e8f2b5000),
    (0xbd0df2c452fde132, 0x3fd62e4420e20000),
    (0x3d26109f4fdb74bd, 0x3fd791292c46a000),
    (0xbd36b95fbdac7696, 0x3fd8f40af84e7000),
    (0x3d17394fa880cbda, 0x3fda56ed8f865000),
    (0xbd150b06a94eccab, 0x3fdbb9d6505b4000),
    (0xbd3be2abf0b38989, 0x3fdd1cb91e728000),
    (0xbd37d6bf1e34da04, 0x3fde7f9d139e2000),
    (0xbd3423c1e14de6ed, 0x3fdfe27db9b0e000),
    (0x3d3c46f1a0efbbc2, 0x3fe0a2b25060a800),
    (0x3d2834fe4e3e6018, 0x3fe154244482a000),
    (0x3d16a03d0f02b650, 0x3fe2059731298800),
    (0x3d3d437056526f30, 0x3fe2b707145de000),
    (0xbd2a0233728405c5, 0x3fe3687b0e0b2800),
    (0xbd24dbdda10d2bf1, 0x3fe419ec5d3f6800),
    (0x3d3f7d0a25d154f2, 0x3fe4cb5f9fc02000),
    (0x3d315ede4d803b18, 0x3fe57cd28421a800),
    (0x3d2ef35793c76730, 0x3fe62e42fefa3800),
];

pub(crate) static ACOSH_ASINH_L2: [(u64, u64); 33] = [
    (0x0000000000000000, 0x0000000000000000),
    (0x3d35abdac3638e99, 0x3f4631ec81e00000),
    (0xbd216b8be9bbe239, 0x3f562fd812700000),
    (0xbd3364c6315542eb, 0x3f60a25205080000),
    (0x3d2734abe459c900, 0x3f662dadc1d00000),
    (0x3d30cf8a761431bf, 0x3f6bb9ff94d00000),
    (0x3d2da2718eb78708, 0x3f70a2a2def80000),
    (0x3d334ada62c59b93, 0x3f7368c0fae40000),
    (0x3d3d09ab376682d4, 0x3f762e58e4f80000),
    (0xbd23cb7b94329211, 0x3f78f46bd28c0000),
    (0xbd2eec5c297c41d0, 0x3f7bb9f831200000),
    (0xbd36411b9395d150, 0x3f7e7fff8f300000),
    (0xbd31c0e59a43053c, 0x3f80a2c0006e0000),
    (0x3d16506596e077b6, 0x3f8205bdb6f00000),
    (0x3d3e256bce6faa27, 0x3f836877c86e0000),
    (0x3ccbd42467b0c8d1, 0x3f84cb6f55780000),
    (0xbd3c4f92132ff0f0, 0x3f862e230e8c0000),
    (0xbd380be08bfab390, 0x3f87911440f60000),
    (0xbd3f0b1319ceb1f7, 0x3f88f443020a0000),
    (0x3d2a65fcfb8de99b, 0x3f8a572dbef40000),
    (0x3d14233885d3779c, 0x3f8bb9d449a60000),
    (0x3d3f46a59e646edb, 0x3f8d1cb8491c0000),
    (0xbd3c3d2f11c11446, 0x3f8e7fd9d2aa0000),
    (0x3d27763f78a1e0cc, 0x3f8fe2b6f9780000),
    (0x3d3b4c37fc60c043, 0x3f90a2a7c7a50000),
    (0xbd15b8a822859be3, 0x3f915412ca860000),
    (0xbd3f2d8c9fc06400, 0x3f92059c90050000),
    (0xbd3e80e79c20378d, 0x3f92b703f49b0000),
    (0x3d368256e4329bdb, 0x3f93688a1a8d0000),
    (0x3d37e9741da248c3, 0x3f9419edc7ba0000),
    (0x3d2e330dccce602b, 0x3f94cb7034fa0000),
    (0x3ce2f32b5d18eefb, 0x3f957cd011870000),
    (0xbd1269e2038315b3, 0x3f962e4eacd40000),
];

#[inline]
pub(crate) fn lpoly_xd_generic<const N: usize>(
    x: DoubleDouble,
    poly: [(u64, u64); N],
    l: f64,
) -> DoubleDouble {
    let zch = poly.last().unwrap();

    let tch = f64::from_bits(zch.1) + l;

    let mut ch = DoubleDouble::new(
        ((f64::from_bits(zch.1) - tch) + l) + f64::from_bits(zch.0),
        tch,
    );

    for zch in poly.iter().rev().skip(1) {
        ch = DoubleDouble::mult(ch, x);
        let th = ch.hi + f64::from_bits(zch.1);
        let tl = (f64::from_bits(zch.1) - th) + ch.hi;
        ch.hi = th;
        ch.lo += tl + f64::from_bits(zch.0);
    }

    ch
}

#[cold]
fn as_acosh_one(x: f64, sh: f64, sl: f64) -> f64 {
    static CH: [(u64, u64); 10] = [
        (0xbc55555555554af1, 0xbfb5555555555555),
        (0x3c29999998933f0e, 0x3f93333333333333),
        (0x3c024929b16ec6b7, 0xbf76db6db6db6db7),
        (0x3bdc56d45e265e2c, 0x3f5f1c71c71c71c7),
        (0x3be6d50ce7188d3d, 0xbf46e8ba2e8ba2e9),
        (0x3bdc6791d1cf399a, 0x3f31c4ec4ec4ec43),
        (0x3bbee0d9408a2e2a, 0xbf1c99999999914f),
        (0xbba1cea281e08012, 0x3f07a878787648e2),
        (0x3b70335101403d9d, 0xbef3fde50d0cb4b9),
        (0x3aff9c6b51787043, 0x3ee12ef3bf8a0a74),
    ];

    const CL: [u64; 6] = [
        0xbecdf3b9d1296ea9,
        0x3eba681d7d2298eb,
        0xbea77ead7b1ca449,
        0x3e94edd2ddb3721f,
        0xbe81bf173531ee23,
        0x3e6613229230e255,
    ];

    let yw0 = f_fmla(x, f64::from_bits(CL[5]), f64::from_bits(CL[4]));
    let yw1 = f_fmla(x, yw0, f64::from_bits(CL[3]));
    let yw2 = f_fmla(x, yw1, f64::from_bits(CL[2]));
    let yw3 = f_fmla(x, yw2, f64::from_bits(CL[1]));

    let y2 = x * f_fmla(x, yw3, f64::from_bits(CL[0]));
    let mut y1 = lpoly_xd_generic(DoubleDouble::new(0., x), CH, y2);
    y1 = DoubleDouble::mult_f64(y1, x);
    let y0 = DoubleDouble::from_exact_add(1., y1.hi);
    let yl = y0.lo + y1.lo;
    let p = DoubleDouble::quick_mult(DoubleDouble::new(yl, y0.hi), DoubleDouble::new(sl, sh));
    p.to_f64()
}

/// Huperbolic acos
///
/// Max ULP 0.5
pub fn f_acosh(x: f64) -> f64 {
    let ix = x.to_bits();
    if ix >= 0x7ff0000000000000u64 {
        let aix = ix.wrapping_shl(1);
        if ix == 0x7ff0000000000000u64 || aix > (0x7ffu64 << 53) {
            return x + x;
        } // +inf or nan

        return f64::NAN;
    }

    if ix <= 0x3ff0000000000000u64 {
        if ix == 0x3ff0000000000000u64 {
            return 0.;
        }
        return f64::NAN;
    }
    let mut off: i32 = 0x3fe;
    let mut t = ix;
    let g = if ix < 0x3ff1e83e425aee63u64 {
        let z = x - 1.;
        let iz = (-0.25) / z;
        let zt = 2. * z;
        let sh = zt.sqrt();
        let sl = dd_fmla(sh, sh, -zt) * (sh * iz);
        const CL: [u64; 9] = [
            0xbfb5555555555555,
            0x3f93333333332f95,
            0xbf76db6db6d5534c,
            0x3f5f1c71c1e04356,
            0xbf46e8b8e3e40d58,
            0x3f31c4ba825ac4fe,
            0xbf1c9045534e6d9e,
            0x3f071fedae26a76b,
            0xbeef1f4f8cc65342,
        ];
        let z2 = z * z;
        let z4 = z2 * z2;

        let ds0 = f_fmla(z, f64::from_bits(CL[8]), f64::from_bits(CL[7]));
        let ds1 = f_fmla(z, f64::from_bits(CL[6]), f64::from_bits(CL[5]));
        let ds2 = f_fmla(z, f64::from_bits(CL[4]), f64::from_bits(CL[3]));
        let ds3 = f_fmla(z, f64::from_bits(CL[2]), f64::from_bits(CL[1]));

        let dsw0 = f_fmla(z2, ds0, ds1);
        let dsw1 = f_fmla(z2, ds2, ds3);
        let dsw2 = f_fmla(z4, dsw0, dsw1);

        let mut ds = (sh * z) * f_fmla(z, dsw2, f64::from_bits(CL[0]));

        let eps = ds * f64::from_bits(0x3ccfc00000000000) - f64::from_bits(0x3970000000000000) * sh;
        ds += sl;
        let lb = sh + (ds - eps);
        let ub = sh + (ds + eps);
        if lb == ub {
            return lb;
        }
        return as_acosh_one(z, sh, sl);
    } else if ix < 0x405bf00000000000u64 {
        off = 0x3ff;
        let x2h = x * x;
        let wh = x2h - 1.;
        let wl = dd_fmla(x, x, -x2h);
        let sh = wh.sqrt();
        let ish = 0.5 / wh;
        let sl = (wl - dd_fmla(sh, sh, -wh)) * (sh * ish);
        let mut pt = DoubleDouble::from_exact_add(x, sh);
        pt.lo += sl;
        t = pt.hi.to_bits();
        pt.lo / pt.hi
    } else if ix < 0x4087100000000000u64 {
        const CL: [u64; 4] = [
            0x3bd5c4b6148816e2,
            0xbfd000000000005c,
            0xbfb7fffffebf3e6c,
            0xbfaaab6691f2bae7,
        ];
        let z = 1. / (x * x);
        let zw0 = f_fmla(z, f64::from_bits(CL[3]), f64::from_bits(CL[2]));
        let zw1 = f_fmla(z, zw0, f64::from_bits(CL[1]));
        f_fmla(z, zw1, f64::from_bits(CL[0]))
    } else if ix < 0x40e0100000000000u64 {
        const CL: [u64; 3] = [0xbbc7f77c8429c6c6, 0xbfcffffffffff214, 0xbfb8000268641bfe];
        let z = 1. / (x * x);
        let zw0 = f_fmla(z, f64::from_bits(CL[2]), f64::from_bits(CL[1]));
        f_fmla(z, zw0, f64::from_bits(CL[0]))
    } else if ix < 0x41ea000000000000u64 {
        const CL: [u64; 2] = [0x3bc7a0ed2effdd10, 0xbfd000000017d048];
        let z = 1. / (x * x);
        f_fmla(z, f64::from_bits(CL[1]), f64::from_bits(CL[0]))
    } else {
        0.
    };
    let ex: i32 = (t >> 52) as i32;
    let e = ex - off;
    t &= 0x000fffffffffffff;
    let ed = e;
    let i: u64 = t >> (52 - 5);
    let d: i64 = (t & 0x00007fffffffffff) as i64;
    let b_i = ACOSH_ASINH_B[i as usize];
    let j: u64 = t
        .wrapping_add((b_i[0] as u64).wrapping_shl(33))
        .wrapping_add((b_i[1] as i64).wrapping_mul(d >> 16) as u64)
        >> (52 - 10);
    t |= 0x3ffu64 << 52;
    let i1: i32 = (j >> 5) as i32;
    let i2 = j & 0x1f;
    let r =
        f64::from_bits(ACOSH_ASINH_R1[i1 as usize]) * f64::from_bits(ACOSH_ASINH_R2[i2 as usize]);
    let dx = dd_fmla(r, f64::from_bits(t), -1.);
    let dx2 = dx * dx;

    const C: [u64; 5] = [
        0xbfe0000000000000,
        0x3fd5555555555530,
        0xbfcfffffffffffa0,
        0x3fc99999e33a6366,
        0xbfc555559ef9525f,
    ];

    let fw0 = f_fmla(dx, f64::from_bits(C[3]), f64::from_bits(C[2]));
    let fw1 = f_fmla(dx, f64::from_bits(C[1]), f64::from_bits(C[0]));
    let fw2 = f_fmla(dx2, f64::from_bits(C[4]), fw0);

    let f = dx2 * f_fmla(dx2, fw2, fw1);
    const L2H: f64 = f64::from_bits(0x3fe62e42fefa3800);
    const L2L: f64 = f64::from_bits(0x3d2ef35793c76730);
    let l1r = ACOSH_ASINH_L1[i1 as usize];
    let l2r = ACOSH_ASINH_L2[i2 as usize];
    let lh = f_fmla(
        L2H,
        ed as f64,
        f64::from_bits(l1r.1) + f64::from_bits(l2r.1),
    );
    let mut ll = f_fmla(L2L, ed as f64, dx);
    ll += g;
    ll += f64::from_bits(l1r.0) + f64::from_bits(l2r.0);
    ll += f;
    let eps = 2.8e-19;
    let lb = lh + (ll - eps);
    let ub = lh + (ll + eps);
    if lb == ub {
        return lb;
    }
    acosh_refine(x, f64::from_bits(0x3ff71547652b82fe) * lb)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() {
        assert_eq!(f_acosh(1.52), 0.9801016289951905);
        assert_eq!(f_acosh(1.86), 1.2320677765479648);
        assert_eq!(f_acosh(4.52), 2.189191592518765);
    }
}
