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
use crate::common::dd_fmla;
use crate::double_double::DoubleDouble;
use crate::err::erf::{Erf, erf_accurate, erf_fast};
use crate::exponents::{EXP_REDUCE_T0, EXP_REDUCE_T1, ldexp};
use std::hint::black_box;

static ASYMPTOTIC_POLY: [[u64; 13]; 6] = [
    [
        0x3fe20dd750429b6d,
        0x3c61a1feb75a48a8,
        0xbfd20dd750429b6c,
        0x3fdb14c2f863e403,
        0xbff0ecf9db3af35d,
        0x400d9eb53ca6eeed,
        0xc030a945830d95c8,
        0x4056e8a963e2f1f5,
        0xc0829b7ccc8f396f,
        0x40b15e716e83c27e,
        0xc0e1cfdcfbcaf22a,
        0x4111986cc7a7e8fe,
        0xc1371f7540590a91,
    ],
    [
        0x3fe20dd750429ae7,
        0x3c863da89e801fd4,
        0xbfd20dd750400795,
        0x3fdb14c2f57c490c,
        0xbff0ecf95c8c9014,
        0x400d9e981f2321ef,
        0xc030a81482de1506,
        0x4056d662420a604b,
        0xc08233c96fff7772,
        0x40af5d62018d3e37,
        0xc0d9ae55e9554450,
        0x410052901e10d139,
        0xc1166465df1385f0,
    ],
    [
        0x3fe20dd75041e3fc,
        0xbc7c9b491c4920fc,
        0xbfd20dd74e5f1526,
        0x3fdb14c1d35a40e0,
        0xbff0ecdecd30e86b,
        0x400d9b4e7f725263,
        0xc030958b5ca8fb39,
        0x40563e3179bf609c,
        0xc0806bbd1cd2d0fd,
        0x40a7b66eb6d1d2f2,
        0xc0cce5a4b1afab75,
        0x40e8b5c6ae6f773c,
        0xc0f5475860326f86,
    ],
    [
        0x3fe20dd75025cfe9,
        0x3c55a92eef32fb20,
        0xbfd20dd71eb9d4e7,
        0x3fdb14af4c25db28,
        0xbff0ebc78a22b3d8,
        0x400d85287a0b3399,
        0xc03045f751e5ca1d,
        0x4054a0d87ddea589,
        0xc07ac6a0981d1eee,
        0x409f44822f567956,
        0xc0bcba372de71349,
        0x40d1a4a19f550ca4,
        0xc0d52a580455ed79,
    ],
    [
        0x3fe20dd74eb31d84,
        0xbc439c4054b7c090,
        0xbfd20dd561af98c4,
        0x3fdb1435165d9df1,
        0xbff0e6b60308e940,
        0x400d3ce30c140882,
        0xc02f2083e404c299,
        0x40520f113d89b42a,
        0xc0741433ebd89f19,
        0x4092f35b6a3154f6,
        0xc0ab020a4313cf3b,
        0x40b90f07e92da7ee,
        0xc0b6565e1d7665c3,
    ],
    [
        0x3fe20dd744b3517b,
        0xbc6f77ab25e01ab4,
        0xbfd20dcc62ec4024,
        0x3fdb125bfa4f66c1,
        0xbff0d80e65381970,
        0x400ca11fbcfa65b2,
        0xc02cd9eaffb88315,
        0x404e010db42e0da7,
        0xc06c5c85250ef6a3,
        0x4085e118d9c1eeaf,
        0xc098d74be13d3d30,
        0x40a211b1b2b5ac83,
        0xc09900be759fc663,
    ],
];

static ASYMPTOTIC_POLY_ACCURATE: [[u64; 30]; 10] = [
    [
        0x3fe20dd750429b6d,
        0x3c61ae3a912b08f0,
        0xbfd20dd750429b6d,
        0xbc51ae34c0606d68,
        0x3fdb14c2f863e924,
        0xbc796c0f4c848fc8,
        0xbff0ecf9db3e71b6,
        0x3c645d756bd288b0,
        0x400d9eb53fad4672,
        0xbcac61629de9adf2,
        0xc030a945f3d147ea,
        0x3cb8fec5ad7ece20,
        0x4056e8c02f27ca6d,
        0xc0829d1c21c363e0,
        0x40b17349b70be627,
        0xc0e28a6bb4686182,
        0x411602d1662523ca,
        0xc14ccae7625c4111,
        0x4184237d064f6e0d,
        0xc1bb1e5466ca3a2f,
        0x41e90ae06a0f6cc1,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
    ],
    [
        0x3fe20dd750429b6d,
        0x3c61adaa62435c10,
        0xbfd20dd750429b6d,
        0xbc441516126827c8,
        0x3fdb14c2f863e90b,
        0x3c7a535780ba5ed4,
        0xbff0ecf9db3e65d6,
        0xbc9089edde27ad07,
        0x400d9eb53fa52f20,
        0xbcabc9737e9464ac,
        0xc030a945f2cd7621,
        0xbcc589f28b700332,
        0x4056e8bffd7e194e,
        0xc0829d18716876e2,
        0x40b17312abe18250,
        0xc0e287e73592805c,
        0x4115ebf7394a39c1,
        0xc14c2f14d46d0cf9,
        0x4182af3d256f955e,
        0xc1b7041659ebd7aa,
        0x41e6039c232e2f71,
        0xc2070ca15c5a07cb,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
    ],
    [
        0x3fe20dd750429b6d,
        0x3c5d3c35b5d37410,
        0xbfd20dd750429b56,
        0xbc7c028415f6f81b,
        0x3fdb14c2f863c1cf,
        0x3c51bb0de6470dbc,
        0xbff0ecf9db33c363,
        0x3c80f8068459eb16,
        0x400d9eb53b9ce57b,
        0x3ca20cce33e7d84a,
        0xc030a945aa2ec4fa,
        0xbcdf6e0fcd7c6030,
        0x4056e8b824d2bfaa,
        0xc0829cc372a6d0b0,
        0x40b1703a99ddd429,
        0xc0e2749f9a267cc6,
        0x4115856a17271849,
        0xc14a8bcb4ba9753f,
        0x418035dcce882940,
        0xc1b1e5d8c5e6e043,
        0x41dfe3b4f365386e,
        0xc20398fdef2b98fe,
        0x42184234d4f4ea12,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
    ],
    [
        0x3fe20dd750429b6a,
        0x3c8ae622b765e9fd,
        0xbfd20dd750428f0e,
        0x3c703c6c67d69513,
        0x3fdb14c2f8563e8e,
        0x3c6766a6bd7aa89c,
        0xbff0ecf9d8dedd48,
        0x3c90af52e90336e3,
        0x400d9eb4aad086fe,
        0x3ca640d371d54a19,
        0xc030a93f1d01cfe0,
        0xbcc68dbd8d9c522c,
        0x4056e842e9fd5898,
        0xc08299886ef1fb80,
        0x40b15e0f0162c9a0,
        0xc0e222dbc6b04cd8,
        0x411460268db1ebdf,
        0xc1474f53ce065fb3,
        0x417961ca8553f870,
        0xc1a8788395d13798,
        0x41d35e37b25d0e81,
        0xc1f707b7457c8f5e,
        0x4211ff852df1c023,
        0xc21b75d0ec56e2cd,
        0,
        0,
        0,
        0,
        0,
        0,
    ],
    [
        0x3fe20dd750429a8f,
        0xbc766d8dda59bcea,
        0xbfd20dd7503fdbab,
        0x3c6707bdffc2b3fe,
        0x3fdb14c2f6526025,
        0xbc27fa4bb9541140,
        0xbff0ecf99c417d45,
        0xbc9748645ef7af94,
        0x400d9eaa9c712a7d,
        0x3ca79e478994ebb4,
        0xc030a8ef11fbf141,
        0x3cbb5c72d69f8954,
        0x4056e4653e0455b1,
        0xc08286909448e6cf,
        0x40b113424ce76821,
        0xc0e1346d859e76de,
        0x4111f9f6cf2293bf,
        0xc14258e6e3b337db,
        0x41714029ecd465fb,
        0xc19c530df5337a6f,
        0x41c34bc4bbccd336,
        0xc1e4a37c52641688,
        0x420019707cec2974,
        0xc21031fe736ea169,
        0x420f6b3003de3ddf,
        0,
        0,
        0,
        0,
        0,
    ],
    [
        0x3fe20dd75042756b,
        0x3c84ad9178b56910,
        0xbfd20dd74feda9e8,
        0xbc78141c70bbc8d6,
        0x3fdb14c2cb128467,
        0xbc709aebaa106821,
        0xbff0ecf603921a0b,
        0x3c97d3cb5bceaf0b,
        0x400d9e3e1751ca59,
        0x3c76622ae5642670,
        0xc030a686af57f547,
        0x3cc083b320aff6b6,
        0x4056cf0b6c027326,
        0xc0823afcb69443d3,
        0x40b03ab450d9f1b9,
        0xc0de74cdb76bcab4,
        0x410c671b60e607f1,
        0xc138f1376d324ce4,
        0x4163b64276234676,
        0xc18aff0ce13c5a8e,
        0x41aef20247251e87,
        0xc1cc9f5662f721f6,
        0x41e4687858e185e1,
        0xc1f4fa507be073c2,
        0x41fb99ac35ee4acc,
        0xc1f16cb585ee3fa9,
        0,
        0,
        0,
        0,
    ],
    [
        0x3fe20dd7503e730d,
        0x3c84e524a098a467,
        0xbfd20dd7498fa6b2,
        0x3c260a4e27751c80,
        0x3fdb14c061bd2a0c,
        0x3c695a8f847d2fc2,
        0xbff0ecd0f11b8c7d,
        0xbc94126deea76061,
        0x400d9b1344463548,
        0x3cafe09a4eca9b0e,
        0xc030996ea52a87ed,
        0xbca924f920db26c0,
        0x40567a2264b556b0,
        0xc0815dfc2c86b6b5,
        0x40accc291b62efe4,
        0xc0d81375a78e746a,
        0x41033a6f15546329,
        0xc12c1e9dc1216010,
        0x4152397ea3d43fda,
        0xc174661e5b2ea512,
        0x4193412367ca5d45,
        0xc1ade56b9d7f37c4,
        0x41c2851d9722146d,
        0xc1d19027baf0c3fe,
        0x41d7e7b8b6ab58ac,
        0xc1d4c446d56aaf22,
        0x41c1492190400505,
        0,
        0,
        0,
    ],
    [
        0x3fe20dd74ff10852,
        0x3c8a32f26deff875,
        0xbfd20dd6f06c491c,
        0x3c770c16e1793358,
        0x3fdb14a7d5e7fd4a,
        0x3c7479998b54db5b,
        0xbff0ebbdb3889c5f,
        0xbc759b853e11369c,
        0x400d89dd249d7ef8,
        0xbc84b5edf0c8c314,
        0xc0306526fb386114,
        0xbc840d04eed7c7e0,
        0x40557ff657e429ce,
        0xc07ef63e90d38630,
        0x40a6d4f34c4ea3da,
        0xc0d04542b9e36a54,
        0x40f577bf19097738,
        0xc119702fe47c736d,
        0x413a7ae12b54fdc6,
        0xc157ca3f0f7c4fa9,
        0x417225d983963cbf,
        0xc1871a6eac612f9e,
        0x4198086324225e1e,
        0xc1a3de68670a7716,
        0x41a91674de4dcbe9,
        0xc1a6b44cc15b76c2,
        0x419a36dae0f30d80,
        0xc17cffc1747ea3dc,
        0,
        0,
    ],
    [
        0x3fe20dd74ba8f300,
        0xbc59dd256871d210,
        0xbfd20dd3593675bc,
        0x3c7ec0e7ffa91ad9,
        0x3fdb13eef86a077a,
        0xbc74fb5d78d411b8,
        0xbff0e5cf52a11f3a,
        0xbc851f36c779dc8c,
        0x400d4417a08b39d5,
        0x3c91be9fb5956638,
        0xc02f91b9f6ce80c3,
        0xbccc9c99dd42829c,
        0x405356439f45bb43,
        0xc078c0ca12819b48,
        0x409efcad2ecd6671,
        0xc0c21b0af6fc1039,
        0x40e327d215ee30c9,
        0xc101fabda96167b0,
        0x411d82e4373b315d,
        0xc134ed9e2ff591e9,
        0x41495c85dcd8eab5,
        0xc159f016f0a3d62a,
        0x41660e89d918b96f,
        0xc16e97be202cba64,
        0x4170d8a081619793,
        0xc16c5422b4fcfc65,
        0x4161131a9dc6aed1,
        0xc14a457d9dced257,
        0x4123605e980e8b86,
        0,
    ],
    [
        0x3fe20dd7319d4d25,
        0x3c82b02992c3b7ab,
        0xbfd20dc29c13ab1b,
        0xbc7d78d79b4ad767,
        0x3fdb115a57b5ab13,
        0xbc6aa8c45be0aa2e,
        0xbff0d58ec437efd7,
        0xbc5994f00a15e850,
        0x400cb1742e229f23,
        0xbca8000471d54399,
        0xc02d99a5edf7b946,
        0xbcbaf76ed7e35cde,
        0x4050a8b71058eb28,
        0xc072d88289da5bfc,
        0x40943ddf24168edb,
        0xc0b3e9dfc38b6d1a,
        0x40d18d4df97ab3df,
        0xc0eb550fc62dcab5,
        0x41029cb71f116ed1,
        0xc115fc9cc4e854e3,
        0x41265915fd0567b1,
        0xc1335eb5fca0e46d,
        0x413c5261ecc0d789,
        0xc14138932dc4eafc,
        0x414117d4eb18facd,
        0xc13af96163e35eca,
        0x4130454a3a63c766,
        0xc11c2ebc1d39b44a,
        0x40ff3327698e0e6b,
        0xc0d094febc3dff35,
    ],
];

// Approximation for the fast path of exp(z) for z=zh+zl,
// with |z| < 0.000130273 < 2^-12.88 and |zl| < 2^-42.6
// (assuming x^y does not overflow or underflow)
#[inline]
fn q_1(z_dd: DoubleDouble) -> DoubleDouble {
    const C: [u64; 5] = [
        0x3ff0000000000000,
        0x3ff0000000000000,
        0x3fe0000000000000,
        0x3fc5555555995d37,
        0x3fa55555558489dc,
    ];
    let z = z_dd.to_f64();
    let mut q = dd_fmla(f64::from_bits(C[4]), z_dd.hi, f64::from_bits(C[3]));

    q = dd_fmla(q, z, f64::from_bits(C[2]));

    let mut v = DoubleDouble::from_exact_add(f64::from_bits(C[1]), q * z);
    v = DoubleDouble::quick_mult(z_dd, v);
    DoubleDouble::f64_add(f64::from_bits(C[0]), v)
}

#[inline]
fn exp_1(x: DoubleDouble) -> DoubleDouble {
    const INVLOG2: f64 = f64::from_bits(0x40b71547652b82fe); /* |INVLOG2-2^12/log(2)| < 2^-43.4 */
    let k = (x.hi * INVLOG2).round_ties_even();

    const LOG2_DD: DoubleDouble = DoubleDouble::new(
        f64::from_bits(0x3bbabc9e3b39803f),
        f64::from_bits(0x3f262e42fefa39ef),
    );
    let k_dd = DoubleDouble::quick_f64_mult(k, LOG2_DD);
    let mut y_dd = DoubleDouble::from_exact_add(x.hi - k_dd.hi, x.lo);
    y_dd.lo -= k_dd.lo;

    let ki: i64 = k as i64; /* Note: k is an integer, this is just a conversion. */
    let mi = (ki >> 12).wrapping_add(0x3ff);
    let i2: i64 = (ki >> 6) & 0x3f;
    let i1: i64 = ki & 0x3f;
    let t1 = DoubleDouble::new(
        f64::from_bits(EXP_REDUCE_T0[i2 as usize].0),
        f64::from_bits(EXP_REDUCE_T0[i2 as usize].1),
    );
    let t2 = DoubleDouble::new(
        f64::from_bits(EXP_REDUCE_T1[i1 as usize].0),
        f64::from_bits(EXP_REDUCE_T1[i1 as usize].1),
    );
    let mut v = DoubleDouble::quick_mult(t2, t1);
    let q = q_1(y_dd);
    v = DoubleDouble::quick_mult(v, q);

    let scale = f64::from_bits((mi as u64) << 52);
    v.hi *= scale;
    v.lo *= scale;
    v
}

struct Exp {
    e: i32,
    result: DoubleDouble,
}

fn exp_accurate(x_dd: DoubleDouble) -> Exp {
    static E2: [u64; 28] = [
        0x3ff0000000000000,
        0xb960000000000000,
        0x3ff0000000000000,
        0xb9be200000000000,
        0x3fe0000000000000,
        0x3a03c00000000000,
        0x3fc5555555555555,
        0x3c655555555c78d9,
        0x3fa5555555555555,
        0x3c455555545616e2,
        0x3f81111111111111,
        0x3c011110121fc314,
        0x3f56c16c16c16c17,
        0xbbef49e06ee3a56e,
        0x3f2a01a01a01a01a,
        0x3b6b053e1eeab9c0,
        0x3efa01a01a01a01a,
        0x3ec71de3a556c733,
        0x3e927e4fb7789f66,
        0x3e5ae64567f54abe,
        0x3e21eed8eff8958b,
        0x3de6124613837216,
        0x3da93974aaf26a57,
        0x3d6ae7f4fd6d0bd9,
        0x3d2ae7e982620b25,
        0x3ce94e4ca59460d8,
        0x3ca69a2a4b7ef36d,
        0x3c6abfe1602308c9,
    ];
    const LOG2INV: f64 = f64::from_bits(0x3ff71547652b82fe);
    let k: i32 = (x_dd.hi * LOG2INV).round_ties_even() as i32;

    const LOG2_H: f64 = f64::from_bits(0x3fe62e42fefa39ef);
    /* we approximate LOG2Lacc ~ log(2) - LOG2H with 38 bits, so that
    k*LOG2Lacc is exact (k has at most 11 bits) */
    const LOG2_L: f64 = f64::from_bits(0x3c7abc9e3b398000);
    const LOG2_TINY: f64 = f64::from_bits(0x398f97b57a079a19);
    let yh = dd_fmla(-k as f64, LOG2_H, x_dd.hi);
    /* since |xh+xl| >= 2.92 we have |k| >= 4;
    (|k|-1/2)*log(2) <= |x| <= (|k|+1/2)*log(2) thus
    1-1/(2|k|) <= |x/(k*log(2))| <= 1+1/(2|k|) thus by Sterbenz theorem
    yh is exact too */
    let mut t = DoubleDouble::from_full_exact_add(-k as f64 * LOG2_L, x_dd.lo);
    let mut y_dd = DoubleDouble::from_exact_add(yh, t.hi);
    y_dd.lo = dd_fmla(-k as f64, LOG2_TINY, y_dd.lo + t.lo);
    /* now yh+yl approximates xh + xl - k*log(2), and we approximate p(yh+yl)
    in h + l */
    /* Since |xh| <= 742, we assume |xl| <= ulp(742) = 2^-43. Then since
       |k| <= round(742/log(2)) = 1070, |yl| <= 1070*LOG2L + 2^-42 < 2^-42.7.
       Since |yh| <= log(2)/2, the contribution of yl is negligible as long
       as |i*p[i]*yh^(i-1)*yl| < 2^-104, which holds for i >= 16.
       Thus for coefficients of degree 16 or more, we don't take yl into account.
    */
    let mut h = f64::from_bits(E2[19 + 8]); // degree 19
    for a in (16..=18).rev() {
        h = dd_fmla(h, y_dd.hi, f64::from_bits(E2[a + 8])); // degree i
    }
    /* degree 15: h*(yh+yl)+E2[15 + 8] */
    t = DoubleDouble::from_exact_mult(h, y_dd.hi);
    t.lo = dd_fmla(h, y_dd.lo, t.lo);
    let mut v = DoubleDouble::from_exact_add(f64::from_bits(E2[15 + 8]), t.hi);
    v.lo += t.lo;
    for a in (8..=14).rev() {
        /* degree i: (h+l)*(yh+yl)+E2[i+8] */
        t = DoubleDouble::quick_mult(v, y_dd);
        v = DoubleDouble::from_exact_add(f64::from_bits(E2[a + 8]), t.hi);
        v.lo += t.lo;
    }
    for a in (0..=7).rev() {
        /* degree i: (h+l)*(yh+yl)+E2[2i]+E2[2i+1] */
        t = DoubleDouble::quick_mult(v, y_dd);
        v = DoubleDouble::from_exact_add(f64::from_bits(E2[2 * a]), t.hi);
        v.lo += t.lo + f64::from_bits(E2[2 * a + 1]);
    }

    Exp { e: k, result: v }
}

#[cold]
fn erfc_asympt_accurate(x: f64) -> f64 {
    /* subnormal exceptions */
    if x == f64::from_bits(0x403a8f7bfbd15495) {
        return dd_fmla(
            f64::from_bits(0x0000000000000001),
            -0.25,
            f64::from_bits(0x000667bd620fd95b),
        );
    }
    let u_dd = DoubleDouble::from_exact_mult(x, x);
    let exp_result = exp_accurate(DoubleDouble::new(-u_dd.lo, -u_dd.hi));

    /* compute 1/x as double-double */
    let yh = 1.0 / x;
    /* Newton's iteration for 1/x is y -> y + y*(1-x*y) */
    let yl = yh * dd_fmla(-x, yh, 1.0);
    // yh+yl approximates 1/x
    static THRESHOLD: [u64; 10] = [
        0x3fb4500000000000,
        0x3fbe000000000000,
        0x3fc3f00000000000,
        0x3fc9500000000000,
        0x3fcf500000000000,
        0x3fd3100000000000,
        0x3fd7100000000000,
        0x3fdbc00000000000,
        0x3fe0b00000000000,
        0x3fe3000000000000,
    ];
    let mut i = 0usize;
    while i < THRESHOLD.len() && yh > f64::from_bits(THRESHOLD[i]) {
        i += 1;
    }
    let p = ASYMPTOTIC_POLY_ACCURATE[i];
    let mut u_dd = DoubleDouble::from_exact_mult(yh, yh);
    u_dd.lo = dd_fmla(2.0 * yh, yl, u_dd.lo);
    /* the polynomial p has degree 29+2i, and its coefficient of largest
    degree is p[14+6+i] */
    let mut z_dd = DoubleDouble::new(0., f64::from_bits(p[14 + 6 + i]));
    for a in (13..=27 + 2 * i).rev().step_by(2) {
        /* degree j: (zh+zl)*(uh+ul)+p[(j-1)/2+6]] */
        let v = DoubleDouble::quick_mult(z_dd, u_dd);
        z_dd = DoubleDouble::from_full_exact_add(f64::from_bits(p[(a - 1) / 2 + 6]), v.hi);
        z_dd.lo += v.lo;
    }
    for a in (1..=11).rev().step_by(2) {
        let v = DoubleDouble::quick_mult(z_dd, u_dd);
        z_dd = DoubleDouble::from_full_exact_add(f64::from_bits(p[a - 1]), v.hi);
        z_dd.lo += v.lo + f64::from_bits(p[a]);
    }

    /* multiply by yh+yl */
    u_dd = DoubleDouble::quick_mult(z_dd, DoubleDouble::new(yl, yh));
    /* now uh+ul approximates p(1/x), i.e., erfc(x)*exp(x^2) */
    /* now multiply (uh+ul)*(eh+el), after normalizing uh+ul to reduce the
    number of exceptional cases */
    u_dd = DoubleDouble::from_exact_add(u_dd.hi, u_dd.lo);
    let v = DoubleDouble::quick_mult(u_dd, exp_result.result);
    /* multiply by 2^e */
    /* multiply by 2^e */
    let mut res = ldexp(v.to_f64(), exp_result.e);
    if res < f64::from_bits(0x0010000000000000) {
        /* for erfc(x) in the subnormal range, we have to perform a special
        rounding */
        let mut corr = v.hi - ldexp(res, -exp_result.e);
        corr += v.lo;
        /* add corr*2^e */
        res += ldexp(corr, exp_result.e);
    }
    res
}

#[cold]
fn erfc_accurate(x: f64) -> f64 {
    if x < 0. {
        let mut v_dd = erf_accurate(-x);
        let t = DoubleDouble::from_exact_add(1.0, v_dd.hi);
        v_dd.hi = t.hi;
        v_dd.lo += t.lo;
        return v_dd.to_f64();
    } else if x <= f64::from_bits(0x3ffb59ffb450828c) {
        // erfc(x) >= 2^-6
        let mut v_dd = erf_accurate(x);
        let t = DoubleDouble::from_exact_add(1.0, -v_dd.hi);
        v_dd.hi = t.hi;
        v_dd.lo = t.lo - v_dd.lo;
        return v_dd.to_f64();
    }
    // now 0x1.b59ffb450828cp+0 < x < 0x1.b39dc41e48bfdp+4
    erfc_asympt_accurate(x)
}

/* Fast path for 0x1.713786d9c7c09p+1 < x < 0x1.b39dc41e48bfdp+4,
using the asymptotic formula erfc(x) = exp(-x^2) * p(1/x)*/
fn erfc_asympt_fast(x: f64) -> Erf {
    /* for x >= 0x1.9db1bb14e15cap+4, erfc(x) < 2^-970, and we might encounter
    underflow issues in the computation of l, thus we delegate this case
    to the accurate path */
    if x >= f64::from_bits(0x4039db1bb14e15ca) {
        return Erf {
            err: 1.0,
            result: DoubleDouble::default(),
        };
    }

    let mut u = DoubleDouble::from_exact_mult(x, x);
    let e_dd = exp_1(DoubleDouble::new(-u.lo, -u.hi));

    /* the assumptions from exp_1 are satisfied:
    * a_mul ensures |ul| <= ulp(uh), thus |ul/uh| <= 2^-52
    * since |x| < 0x1.9db1bb14e15cap+4 we have
      |ul| < ulp(0x1.9db1bb14e15cap+4^2) = 2^-43 */
    /* eh+el approximates exp(-x^2) with maximal relative error 2^-74.139 */

    /* compute 1/x as double-double */
    let yh = 1.0 / x;
    /* Assume 1 <= x < 2, then 0.5 <= yh <= 1,
    and yh = 1/x + eps with |eps| <= 2^-53. */
    /* Newton's iteration for 1/x is y -> y + y*(1-x*y) */
    let yl = yh * dd_fmla(-x, yh, 1.0);
    /* x*yh-1 = x*(1/x+eps)-1 = x*eps
       with |x*eps| <= 2^-52, thus the error on the FMA is bounded by
       ulp(2^-52.1) = 2^-105.
       Now |yl| <= |yh| * 2^-52 <= 2^-52, thus the rounding error on
       yh * __builtin_fma (-x, yh, 1.0) is bounded also by ulp(2^-52.1) = 2^-105.
       From [6], Lemma 3.7, if yl was computed exactly, then yh+yl would differ
       from 1/x by at most yh^2/theta^3*(1/x-yh)^2 for some theta in [yh,1/x]
       or [1/x,yh].
       Since yh, 1/x <= 1, this gives eps^2 <= 2^-106.
       Adding the rounding errors, we have:
       |yh + yl - 1/x| <= 2^-105 + 2^-105 + 2^-106 < 2^-103.67.
       For the relative error, since |yh| >= 1/2, this gives:
       |yh + yl - 1/x| < 2^-102.67 * |yh+yl|
    */
    const THRESHOLD: [u64; 6] = [
        0x3fbd500000000000,
        0x3fc59da6ca291ba6,
        0x3fcbc00000000000,
        0x3fd0c00000000000,
        0x3fd3800000000000,
        0x3fd6300000000000,
    ];
    let mut i = 0usize;
    while i < THRESHOLD.len() && yh > f64::from_bits(THRESHOLD[i]) {
        i += 1;
    }
    let p = ASYMPTOTIC_POLY[i];
    u = DoubleDouble::from_exact_mult(yh, yh);
    /* Since |yh| <= 1, we have |uh| <= 1 and |ul| <= 2^-53. */
    u.lo = dd_fmla(2.0 * yh, yl, u.lo);
    /* uh+ul approximates (yh+yl)^2, with absolute error bounded by
       ulp(ul) + yl^2, where ulp(ul) is the maximal rounding error in
       the FMA, and yl^2 is the neglected term.
       Since |ul| <= 2^-53, ulp(ul) <= 2^-105, and since |yl| <= 2^-52,
       this yields |uh + ul - yh^2| <= 2^-105 + 2^-104 < 2^-103.41.
       For the relative error, since |(yh+yl)^2| >= 1/4:
       |uh + ul - yh^2| < 2^-101.41 * |uh+ul|.
       And relatively to 1/x^2:
       yh + yl = 1/x * (1 + eps1)       with |eps1| < 2^-102.67
       uh + ul = (yh+yl)^2 * (1 + eps2) with |eps2| < 2^-101.41
       This yields:
       |uh + ul - 1/x| < 2^-100.90 * |uh+ul|.
    */

    /* evaluate p(uh+ul) */
    let mut zh: f64 = f64::from_bits(p[12]); // degree 23
    zh = dd_fmla(zh, u.hi, f64::from_bits(p[11])); // degree 21
    zh = dd_fmla(zh, u.hi, f64::from_bits(p[10])); // degree 19

    /* degree 17: zh*(uh+ul)+p[i] */
    let mut v = DoubleDouble::quick_f64_mult(zh, u);
    let mut z_dd = DoubleDouble::from_exact_add(f64::from_bits(p[9]), v.hi);
    z_dd.lo += v.lo;

    for a in (3..=15).rev().step_by(2) {
        v = DoubleDouble::quick_mult(z_dd, u);
        z_dd = DoubleDouble::from_exact_add(f64::from_bits(p[((a + 1) / 2) as usize]), v.hi);
        z_dd.lo += v.lo;
    }

    /* degree 1: (zh+zl)*(uh+ul)+p[0]+p[1] */
    v = DoubleDouble::quick_mult(z_dd, u);
    z_dd = DoubleDouble::from_exact_add(f64::from_bits(p[0]), v.hi);
    z_dd.lo += v.lo + f64::from_bits(p[1]);
    /* multiply by yh+yl */
    u = DoubleDouble::quick_mult(z_dd, DoubleDouble::new(yl, yh));
    /* now uh+ul approximates p(1/x) */
    /* now multiply (uh+ul)*(eh+el) */
    v = DoubleDouble::quick_mult(u, e_dd);

    /* Write y = 1/x.  We have the following errors:
       * the maximal mathematical error is:
         |erfc(x)*exp(x^2) - p(y)| < 2^-71.804 * |p(y)| (for i=3) thus
         |erfc(x) - exp(-x^2)*p(y)| < 2^-71.804 * |exp(-x^2)*p(y)|
       * the error in approximating exp(-x^2) by eh+el:
         |eh + el - exp(-x^2)| < 2^-74.139 * |eh + el|
       * the fact that we evaluate p on yh+yl instead of 1/x
         this error is bounded by |p'| * |yh+yl - 1/x|, where
         |yh+yl - 1/x| < 2^-102.67 * |yh+yl|, and the relative
         error is bounded by |p'/p| * |yh+yl - 1/x|.
         Since the maximal value of |p'/p| is bounded by 27.2 (for i=0),
         this yields 27.2 * 2^-102.67 < 2^-97.9
       * the rounding errors when evaluating p on yh+yl: this error is bounded
         (relatively) by 2^-67.184 (for i=5), see analyze_erfc_asympt_fast()
         in erfc.sage
       * the rounding error in (uh+ul)*(eh+el): we assume this error is bounded
         by 2^-80 (relatively)
       This yields a global relative bound of:
       (1+2^-71.804)*(1+2^-74.139)*(1+2^-97.9)*(1+2^-67.184)*(1+2^-80)-1
       < 2^-67.115
    */
    if v.hi >= f64::from_bits(0x044151b9a3fdd5c9) {
        Erf {
            err: f64::from_bits(0x3bbd900000000000) * v.hi,
            result: v,
        } /* 2^-67.115 < 0x1.d9p-68 */
    } else {
        Erf {
            result: v,
            err: f64::from_bits(0x0010000000000000),
        } // this overestimates 0x1.d9p-68 * h
    }
}

#[inline]
fn erfc_fast(x: f64) -> Erf {
    if x < 0.
    // erfc(x) = 1 - erf(x) = 1 + erf(-x)
    {
        let res = erf_fast(-x);
        /* h+l approximates erf(-x), with relative error bounded by err,
        where err <= 0x1.78p-69 */
        let err = res.err * res.result.hi; /* convert into absolute error */
        let mut t = DoubleDouble::from_exact_add(1.0, res.result.hi);
        t.lo += res.result.lo;
        // since h <= 2, the fast_two_sum() error is bounded by 2^-105*h <= 2^-104
        /* After the fast_two_sum() call, we have |t| <= ulp(h) <= ulp(2) = 2^-51
        thus assuming |l| <= 2^-51 after the cr_erf_fast() call,
        we have |t| <= 2^-50 here, thus the rounding
        error on t -= *l is bounded by ulp(2^-50) = 2^-102.
        The absolute error is thus bounded by err + 2^-104 + 2^-102
        = err + 0x1.4p-102.
        The maximal value of err here is for |x| < 0.0625, where cr_erf_fast()
        returns 0x1.78p-69, and h=1/2, yielding err = 0x1.78p-70 here.
        Adding 0x1.4p-102 is thus exact. */
        return Erf {
            err: err + f64::from_bits(0x3994000000000000),
            result: t,
        };
    } else if x <= f64::from_bits(0x400713786d9c7c09) {
        let res = erf_fast(x);
        /* h+l approximates erf(x), with relative error bounded by err,
        where err <= 0x1.78p-69 */
        let err = res.err * res.result.hi; /* convert into absolute error */
        let mut t = DoubleDouble::from_exact_add(1.0, -res.result.hi);
        t.lo -= res.result.lo;
        /* for x >= 0x1.e861fbb24c00ap-2, erf(x) >= 1/2, thus 1-h is exact
        by Sterbenz theorem, thus t = 0 in fast_two_sum(), and we have t = -l
        here, thus the absolute error is err */
        if x >= f64::from_bits(0x3fde861fbb24c00a) {
            return Erf { err, result: t };
        }
        /* for x < 0x1.e861fbb24c00ap-2, the error in fast_two_sum() is bounded
        by 2^-105*h, and since h <= 1/2, this yields 2^-106.
        After the fast_two_sum() call, we have |t| <= ulp(h) <= ulp(1/2) = 2^-53
        thus assuming |l| <= 2^-53 after the cr_erf_fast() call,
        we have |t| <= 2^-52 here, thus the rounding
        error on t -= *l is bounded by ulp(2^-52) = 2^-104.
        The absolute error is thus bounded by err + 2^-106 + 2^-104
        The maximal value of err here is for x < 0.0625, where cr_erf_fast()
        returns 0x1.78p-69, and h=1/2, yielding err = 0x1.78p-70 here.
        Adding 0x1.4p-104 is thus exact. */
        return Erf {
            err: err + f64::from_bits(0x3974000000000000),
            result: t,
        };
    }
    /* Now THRESHOLD1 < x < 0x1.b39dc41e48bfdp+4 thus erfc(x) < 0.000046. */
    /* on a i7-8700 with gcc 12.2.0, for x in [THRESHOLD1,+5.0],
    the average reciprocal throughput is about 111 cycles
    (among which 20 cycles for exp_1) */
    erfc_asympt_fast(x)
}

/// Complementary error function
///
/// Max ulp 0.5
pub fn f_erfc(x: f64) -> f64 {
    let t: u64 = x.to_bits();
    let at: u64 = t & 0x7fff_ffff_ffff_ffff;

    if t >= 0x8000000000000000u64
    // x = -NaN or x <= 0 (excluding +0)
    {
        // for x <= -0x1.7744f8f74e94bp2, erfc(x) rounds to 2 (to nearest)
        if t >= 0xc017744f8f74e94bu64
        // x = NaN or x <= -0x1.7744f8f74e94bp2
        {
            if t >= 0xfff0000000000000u64 {
                // -Inf or NaN
                if t == 0xfff0000000000000u64 {
                    return 2.0;
                } // -Inf
                return x + x; // NaN
            }
            return black_box(2.0) - black_box(f64::from_bits(0x3c90000000000000)); // rounds to 2 or below(2)
        }

        // for -9.8390953768041405e-17 <= x <= 0, erfc(x) rounds to 1 (to nearest)
        if f64::from_bits(0xbc9c5bf891b4ef6a) <= x {
            return dd_fmla(-x, f64::from_bits(0x3c90000000000000), 1.0);
        }
    } else
    // x = +NaN or x >= 0 (excluding -0)
    {
        // for x >= 0x1.b39dc41e48bfdp+4, erfc(x) < 2^-1075: rounds to 0 or 2^-1074
        if at >= 0x403b39dc41e48bfdu64
        // x = NaN or x >= 0x1.b39dc41e48bfdp+4
        {
            if at >= 0x7ff0000000000000u64 {
                // +Inf or NaN
                if at == 0x7ff0000000000000u64 {
                    return 0.0;
                } // +Inf
                return x + x; // NaN
            }
            return black_box(f64::from_bits(0x0000000000000001)) * black_box(0.25); // 0 or 2^-1074 wrt rounding
        }

        // for 0 <= x <= 0x1.c5bf891b4ef6ap-55, erfc(x) rounds to 1 (to nearest)
        if x <= f64::from_bits(0x3c8c5bf891b4ef6a) {
            return dd_fmla(-x, f64::from_bits(0x3c90000000000000), 1.0);
        }
    }

    /* now -0x1.7744f8f74e94bp+2 < x < -0x1.c5bf891b4ef6ap-54
    or 0x1.c5bf891b4ef6ap-55 < x < 0x1.b39dc41e48bfdp+4 */

    let result = erfc_fast(x);

    let left = result.result.hi + (result.result.lo - result.err);
    let right = result.result.hi + (result.result.lo + result.err);
    if left == right {
        return left;
    }
    erfc_accurate(x)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_erfc() {
        assert_eq!(f_erfc(1.0), 0.15729920705028513);
        assert_eq!(f_erfc(0.5), 0.4795001221869535);
        assert_eq!(f_erfc(0.000000005), 0.9999999943581042);
        assert_eq!(f_erfc(-0.00000000000065465465423305), 1.0000000000007387);
    }
}
