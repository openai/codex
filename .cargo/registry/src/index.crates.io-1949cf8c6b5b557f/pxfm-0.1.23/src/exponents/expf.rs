/*
 * // Copyright (c) Radzivon Bartoshyk 4/2025. All rights reserved.
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
use crate::common::{f_fmla, f_fmlaf, fmlaf, pow2if, rintfk};
use crate::polyeval::f_polyeval5;

const L2U_F: f32 = 0.693_145_751_953_125;
const L2L_F: f32 = 1.428_606_765_330_187_045_e-6;
const R_LN2_F: f32 = std::f32::consts::LOG2_E;

/// Exp for given value for const context.
/// This is simplified version just to make a good approximation on const context.
#[inline]
pub const fn expf(d: f32) -> f32 {
    const EXP_POLY_1_S: f32 = 2f32;
    const EXP_POLY_2_S: f32 = 0.16666707f32;
    const EXP_POLY_3_S: f32 = -0.002775669f32;
    let qf = rintfk(d * R_LN2_F);
    let q = qf as i32;
    let r = fmlaf(qf, -L2U_F, d);
    let r = fmlaf(qf, -L2L_F, r);

    let f = r * r;
    // Poly for u = r*(exp(r)+1)/(exp(r)-1)
    let mut u = EXP_POLY_3_S;
    u = fmlaf(u, f, EXP_POLY_2_S);
    u = fmlaf(u, f, EXP_POLY_1_S);
    let u = 1f32 + 2f32 * r / (u - r);
    let i2 = pow2if(q);
    u * i2
    // if d < -87f32 {
    //     r = 0f32;
    // }
    // if d > 88f32 {
    //     r = f32::INFINITY;
    // }
}

// Lookup table for exp(m) with m = -104, ..., 102.
//   -104 = floor(log(single precision's min denormal))
//    102 = ceil(log(single precision's max bessel K(n) that will be used))
// Table is generated with SageMath as follows:
// for r in range(-104, 102):
//     print(double_to_hex(RealField(180)(r).exp()) + ",")
static EXP_M1: [u64; 206] = [
    0x368f1e6b68529e33,
    0x36a525be4e4e601d,
    0x36bcbe0a45f75eb1,
    0x36d3884e838aea68,
    0x36ea8c1f14e2af5d,
    0x37020a717e64a9bd,
    0x3718851d84118908,
    0x3730a9bdfb02d240,
    0x3746a5bea046b42e,
    0x375ec7f3b269efa8,
    0x3774eafb87eab0f2,
    0x378c6e2d05bbc000,
    0x37a35208867c2683,
    0x37ba425b317eeacd,
    0x37d1d8508fa8246a,
    0x37e840fbc08fdc8a,
    0x38007b7112bc1ffe,
    0x381666d0dad2961d,
    0x382e726c3f64d0fe,
    0x3844b0dc07cabf98,
    0x385c1f2daf3b6a46,
    0x38731c5957a47de2,
    0x3889f96445648b9f,
    0x38a1a6baeadb4fd1,
    0x38b7fd974d372e45,
    0x38d04da4d1452919,
    0x38e62891f06b3450,
    0x38fe1dd273aa8a4a,
    0x3914775e0840bfdd,
    0x392bd109d9d94bda,
    0x3942e73f53fba844,
    0x3959b138170d6bfe,
    0x397175af0cf60ec5,
    0x3987baee1bffa80b,
    0x39a02057d1245ceb,
    0x39b5eafffb34ba31,
    0x39cdca23bae16424,
    0x39e43e7fc88b8056,
    0x39fb83bf23a9a9eb,
    0x3a12b2b8dd05b318,
    0x3a2969d47321e4cc,
    0x3a41452b7723aed2,
    0x3a5778fe2497184c,
    0x3a6fe7116182e9cc,
    0x3a85ae191a99585a,
    0x3a9d775d87da854d,
    0x3ab4063f8cc8bb98,
    0x3acb374b315f87c1,
    0x3ae27ec458c65e3c,
    0x3af923372c67a074,
    0x3b11152eaeb73c08,
    0x3b2737c5645114b5,
    0x3b3f8e6c24b5592e,
    0x3b5571db733a9d61,
    0x3b6d257d547e083f,
    0x3b83ce9b9de78f85,
    0x3b9aebabae3a41b5,
    0x3bb24b6031b49bda,
    0x3bc8dd5e1bb09d7e,
    0x3be0e5b73d1ff53d,
    0x3bf6f741de1748ec,
    0x3c0f36bd37f42f3e,
    0x3c2536452ee2f75c,
    0x3c3cd480a1b74820,
    0x3c539792499b1a24,
    0x3c6aa0de4bf35b38,
    0x3c82188ad6ae3303,
    0x3c9898471fca6055,
    0x3cb0b6c3afdde064,
    0x3cc6b7719a59f0e0,
    0x3cdee001eed62aa0,
    0x3cf4fb547c775da8,
    0x3d0c8464f7616468,
    0x3d236121e24d3bba,
    0x3d3a56e0c2ac7f75,
    0x3d51e642baeb84a0,
    0x3d6853f01d6d53ba,
    0x3d80885298767e9a,
    0x3d967852a7007e42,
    0x3dae8a37a45fc32e,
    0x3dc4c1078fe9228a,
    0x3ddc3527e433fab1,
    0x3df32b48bf117da2,
    0x3e0a0db0d0ddb3ec,
    0x3e21b48655f37267,
    0x3e381056ff2c5772,
    0x3e505a628c699fa1,
    0x3e6639e3175a689d,
    0x3e7e355bbaee85cb,
    0x3e94875ca227ec38,
    0x3eabe6c6fdb01612,
    0x3ec2f6053b981d98,
    0x3ed9c54c3b43bc8b,
    0x3ef18354238f6764,
    0x3f07cd79b5647c9b,
    0x3f202cf22526545a,
    0x3f35fc21041027ad,
    0x3f4de16b9c24a98f,
    0x3f644e51f113d4d6,
    0x3f7b993fe00d5376,
    0x3f92c155b8213cf4,
    0x3fa97db0ccceb0af,
    0x3fc152aaa3bf81cc,
    0x3fd78b56362cef38,
    0x3ff0000000000000,
    0x4005bf0a8b145769,
    0x401d8e64b8d4ddae,
    0x403415e5bf6fb106,
    0x404b4c902e273a58,
    0x40628d389970338f,
    0x407936dc5690c08f,
    0x409122885aaeddaa,
    0x40a749ea7d470c6e,
    0x40bfa7157c470f82,
    0x40d5829dcf950560,
    0x40ed3c4488ee4f7f,
    0x4103de1654d37c9a,
    0x411b00b5916ac955,
    0x413259ac48bf05d7,
    0x4148f0ccafad2a87,
    0x4160f2ebd0a80020,
    0x417709348c0ea4f9,
    0x418f4f22091940bd,
    0x41a546d8f9ed26e1,
    0x41bceb088b68e804,
    0x41d3a6e1fd9eecfd,
    0x41eab5adb9c43600,
    0x420226af33b1fdc1,
    0x4218ab7fb5475fb7,
    0x4230c3d3920962c9,
    0x4246c932696a6b5d,
    0x425ef822f7f6731d,
    0x42750bba3796379a,
    0x428c9aae4631c056,
    0x42a370470aec28ed,
    0x42ba6b765d8cdf6d,
    0x42d1f43fcc4b662c,
    0x42e866f34a725782,
    0x4300953e2f3a1ef7,
    0x431689e221bc8d5b,
    0x432ea215a1d20d76,
    0x4344d13fbb1a001a,
    0x435c4b334617cc67,
    0x43733a43d282a519,
    0x438a220d397972eb,
    0x43a1c25c88df6862,
    0x43b8232558201159,
    0x43d0672a3c9eb871,
    0x43e64b41c6d37832,
    0x43fe4cf766fe49be,
    0x44149767bc0483e3,
    0x442bfc951eb8bb76,
    0x444304d6aeca254b,
    0x4459d97010884251,
    0x44719103e4080b45,
    0x4487e013cd114461,
    0x44a03996528e074c,
    0x44b60d4f6fdac731,
    0x44cdf8c5af17ba3b,
    0x44e45e3076d61699,
    0x44fbaed16a6e0da7,
    0x4512cffdfebde1a1,
    0x4529919cabefcb69,
    0x454160345c9953e3,
    0x45579dbc9dc53c66,
    0x45700c810d464097,
    0x4585d009394c5c27,
    0x459da57de8f107a8,
    0x45b425982cf597cd,
    0x45cb61e5ca3a5e31,
    0x45e29bb825dfcf87,
    0x45f94a90db0d6fe2,
    0x46112fec759586fd,
    0x46275c1dc469e3af,
    0x463fbfd219c43b04,
    0x4655936d44e1a146,
    0x466d531d8a7ee79c,
    0x4683ed9d24a2d51b,
    0x469b15cfe5b6e17b,
    0x46b268038c2c0e00,
    0x46c9044a73545d48,
    0x46e1002ab6218b38,
    0x46f71b3540cbf921,
    0x470f6799ea9c414a,
    0x47255779b984f3eb,
    0x473d01a210c44aa4,
    0x4753b63da8e91210,
    0x476aca8d6b0116b8,
    0x478234de9e0c74e9,
    0x4798bec7503ca477,
    0x47b0d0eda9796b90,
    0x47c6db0118477245,
    0x47df1056dc7bf22d,
    0x47f51c2cc3433801,
    0x480cb108ffbec164,
    0x48237f780991b584,
    0x483a801c0ea8ac4d,
    0x48520247cc4c46c1,
    0x48687a0553328015,
    0x4880a233dee4f9bb,
    0x48969b7f55b808ba,
    0x48aeba064644060a,
    0x48c4e184933d9364,
    0x48dc614fe2531841,
    0x48f3494a9b171bf5,
    0x490a36798b9d969b,
];

// Lookup table for exp(m * 2^(-7)) with m = 0, ..., 127.
// Table is generated with Sollya as follows:
// > display = hexadecimal;
// > for i from 0 to 127 do { D(exp(i / 128)); };
static EXP_M2: [u64; 128] = [
    0x3ff0000000000000,
    0x3ff0202015600446,
    0x3ff04080ab55de39,
    0x3ff06122436410dd,
    0x3ff08205601127ed,
    0x3ff0a32a84e9c1f6,
    0x3ff0c49236829e8c,
    0x3ff0e63cfa7ab09d,
    0x3ff1082b577d34ed,
    0x3ff12a5dd543ccc5,
    0x3ff14cd4fc989cd6,
    0x3ff16f9157587069,
    0x3ff192937074e0cd,
    0x3ff1b5dbd3f68122,
    0x3ff1d96b0eff0e79,
    0x3ff1fd41afcba45e,
    0x3ff2216045b6f5cd,
    0x3ff245c7613b8a9b,
    0x3ff26a7793f60164,
    0x3ff28f7170a755fd,
    0x3ff2b4b58b372c79,
    0x3ff2da4478b620c7,
    0x3ff3001ecf601af7,
    0x3ff32645269ea829,
    0x3ff34cb8170b5835,
    0x3ff373783a722012,
    0x3ff39a862bd3c106,
    0x3ff3c1e2876834aa,
    0x3ff3e98deaa11dcc,
    0x3ff41188f42c3e32,
    0x3ff439d443f5f159,
    0x3ff462707b2bac21,
    0x3ff48b5e3c3e8186,
    0x3ff4b49e2ae5ac67,
    0x3ff4de30ec211e60,
    0x3ff50817263c13cd,
    0x3ff5325180cfacf7,
    0x3ff55ce0a4c58c7c,
    0x3ff587c53c5a7af0,
    0x3ff5b2fff3210fd9,
    0x3ff5de9176045ff5,
    0x3ff60a7a734ab0e8,
    0x3ff636bb9a983258,
    0x3ff663559cf1bc7c,
    0x3ff690492cbf9433,
    0x3ff6bd96fdd034a2,
    0x3ff6eb3fc55b1e76,
    0x3ff719443a03acb9,
    0x3ff747a513dbef6a,
    0x3ff776630c678bc1,
    0x3ff7a57ede9ea23e,
    0x3ff7d4f946f0ba8d,
    0x3ff804d30347b546,
    0x3ff8350cd30ac390,
    0x3ff865a7772164c5,
    0x3ff896a3b1f66a0e,
    0x3ff8c802477b0010,
    0x3ff8f9c3fd29beaf,
    0x3ff92be99a09bf00,
    0x3ff95e73e6b1b75e,
    0x3ff99163ad4b1dcc,
    0x3ff9c4b9b995509b,
    0x3ff9f876d8e8c566,
    0x3ffa2c9bda3a3e78,
    0x3ffa61298e1e069c,
    0x3ffa9620c6cb3374,
    0x3ffacb82581eee54,
    0x3ffb014f179fc3b8,
    0x3ffb3787dc80f95f,
    0x3ffb6e2d7fa5eb18,
    0x3ffba540dba56e56,
    0x3ffbdcc2cccd3c85,
    0x3ffc14b431256446,
    0x3ffc4d15e873c193,
    0x3ffc85e8d43f7cd0,
    0x3ffcbf2dd7d490f2,
    0x3ffcf8e5d84758a9,
    0x3ffd3311bc7822b4,
    0x3ffd6db26d16cd67,
    0x3ffda8c8d4a66969,
    0x3ffde455df80e3c0,
    0x3ffe205a7bdab73e,
    0x3ffe5cd799c6a54e,
    0x3ffe99ce2b397649,
    0x3ffed73f240dc142,
    0x3fff152b7a07bb76,
    0x3fff539424d90f5e,
    0x3fff927a1e24bb76,
    0x3fffd1de6182f8c9,
    0x400008e0f64294ab,
    0x40002912df5ce72a,
    0x400049856cd84339,
    0x40006a39207f0a09,
    0x40008b2e7d2035cf,
    0x4000ac6606916501,
    0x4000cde041b0e9ae,
    0x4000ef9db467dcf8,
    0x4001119ee5ac36b6,
    0x400133e45d82e952,
    0x4001566ea50201d7,
    0x4001793e4652cc50,
    0x40019c53ccb3fc6b,
    0x4001bfafc47bda73,
    0x4001e352bb1a74ad,
    0x4002073d3f1bd518,
    0x40022b6fe02a3b9c,
    0x40024feb2f105cb8,
    0x400274afbdbba4a6,
    0x400299be1f3e7f1c,
    0x4002bf16e7d2a38c,
    0x4002e4baacdb6614,
    0x40030aaa04e80d05,
    0x400330e587b62b28,
    0x4003576dce33fead,
    0x40037e437282d4ee,
    0x4003a5670ff972ed,
    0x4003ccd9432682b4,
    0x4003f49aa9d30590,
    0x40041cabe304cb34,
    0x4004450d8f00edd4,
    0x40046dc04f4e5338,
    0x400496c4c6b832da,
    0x4004c01b9950a111,
    0x4004e9c56c731f5d,
    0x400513c2e6c731d7,
    0x40053e14b042f9ca,
    0x400568bb722dd593,
    0x400593b7d72305bb,
];

/// Computes exp
///
/// Max found ULP 0.5
#[inline]
pub fn f_expf(x: f32) -> f32 {
    let x_u = x.to_bits();
    let x_abs = x_u & 0x7fff_ffffu32;
    if x_abs >= 0x42b2_0000u32 || x_abs <= 0x3280_0000u32 {
        let exp = ((x_u >> 23) & 0xFF) as i32;
        // |x| < 2^-25
        if exp <= 101i32 {
            return 1.0 + x;
        }

        // When x < log(2^-150) or nan
        if x_u >= 0xc2cf_f1b5u32 {
            // exp(-Inf) = 0
            if x.is_infinite() {
                return 0.0;
            }
            // exp(nan) = nan
            if x.is_nan() {
                return x;
            }
            return 0.0;
        }
        // x >= 89 or nan
        if x.is_sign_positive() && (x_u >= 0x42b2_0000) {
            // x is +inf or nan
            return x + f32::INFINITY;
        }
    }

    // For -104 < x < 89, to compute exp(x), we perform the following range
    // reduction: find hi, mid, lo such that:
    //   x = hi + mid + lo, in which
    //     hi is an integer,
    //     mid * 2^7 is an integer
    //     -2^(-8) <= lo < 2^-8.
    // In particular,
    //   hi + mid = round(x * 2^7) * 2^(-7).
    // Then,
    //   exp(x) = exp(hi + mid + lo) = exp(hi) * exp(mid) * exp(lo).
    // We store exp(hi) and exp(mid) in the lookup tables EXP_M1 and EXP_M2
    // respectively.  exp(lo) is computed using a degree-4 minimax polynomial
    // generated by Sollya.

    // x_hi = (hi + mid) * 2^7 = round(x * 2^7).
    let kf = (x * 128.).round();
    // Subtract (hi + mid) from x to get lo.
    let xd = f_fmlaf(kf, -0.0078125 /* - 1/128 */, x) as f64;
    let mut x_hi = kf as i32;
    x_hi += 104 << 7;
    // hi = x_hi >> 7
    let exp_hi = f64::from_bits(EXP_M1[(x_hi >> 7) as usize]);
    // mid * 2^7 = x_hi & 0x0000'007fU;
    let exp_mid = f64::from_bits(EXP_M2[(x_hi & 0x7f) as usize]);
    // Degree-4 minimax polynomial generated by Sollya with the following
    // commands:
    // d = [-2^-8, 2^-8];
    // f_exp = expm1(x)/x;
    // Q = fpminimax(f_exp, 3, [|D...|], [-2^-8, 2^-8]);
    let p = f_polyeval5(
        xd,
        1.,
        f64::from_bits(0x3feffffffffff777),
        f64::from_bits(0x3fe000000000071c),
        f64::from_bits(0x3fc555566668e5e7),
        f64::from_bits(0x3fa55555555ef243),
    );
    (p * exp_hi * exp_mid) as f32
}

#[inline]
pub(crate) fn core_expf(x: f32) -> f64 {
    // x_hi = (hi + mid) * 2^7 = round(x * 2^7).
    let kf = (x * 128.).round();
    // Subtract (hi + mid) from x to get lo.
    let xd = f_fmlaf(kf, -0.0078125 /* - 1/128 */, x) as f64;
    let mut x_hi = kf as i32;
    x_hi += 104 << 7;
    // hi = x_hi >> 7
    let exp_hi = f64::from_bits(EXP_M1[(x_hi >> 7) as usize]);
    // mid * 2^7 = x_hi & 0x0000'007fU;
    let exp_mid = f64::from_bits(EXP_M2[(x_hi & 0x7f) as usize]);
    // Degree-4 minimax polynomial generated by Sollya with the following
    // commands:
    // d = [-2^-8, 2^-8];
    // f_exp = expm1(x)/x;
    // Q = fpminimax(f_exp, 3, [|D...|], [-2^-8, 2^-8]);
    let p = f_polyeval5(
        xd,
        1.,
        f64::from_bits(0x3feffffffffff777),
        f64::from_bits(0x3fe000000000071c),
        f64::from_bits(0x3fc555566668e5e7),
        f64::from_bits(0x3fa55555555ef243),
    );
    p * exp_hi * exp_mid
}

#[inline]
pub(crate) fn core_expdf(x: f64) -> f64 {
    // x_hi = (hi + mid) * 2^7 = round(x * 2^7).
    let kf = (x * 128.).round();
    // Subtract (hi + mid) from x to get lo.
    let xd = f_fmla(kf, -0.0078125 /* - 1/128 */, x);
    let mut x_hi = kf as i32;
    x_hi += 104 << 7;
    // hi = x_hi >> 7
    let exp_hi = f64::from_bits(EXP_M1[(x_hi >> 7) as usize]);
    // mid * 2^7 = x_hi & 0x0000'007fU;
    let exp_mid = f64::from_bits(EXP_M2[(x_hi & 0x7f) as usize]);
    // Degree-4 minimax polynomial generated by Sollya with the following
    // commands:
    // d = [-2^-8, 2^-8];
    // f_exp = expm1(x)/x;
    // Q = fpminimax(f_exp, 3, [|D...|], [-2^-8, 2^-8]);
    let p = f_polyeval5(
        xd,
        1.,
        f64::from_bits(0x3feffffffffff777),
        f64::from_bits(0x3fe000000000071c),
        f64::from_bits(0x3fc555566668e5e7),
        f64::from_bits(0x3fa55555555ef243),
    );
    p * exp_hi * exp_mid
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expf_test() {
        assert!(
            (expf(0f32) - 1f32).abs() < 1e-6,
            "Invalid result {}",
            expf(0f32)
        );
        assert!(
            (expf(5f32) - 148.4131591025766f32).abs() < 1e-6,
            "Invalid result {}",
            expf(5f32)
        );
    }

    #[test]
    fn f_expf_test() {
        assert_eq!(f_expf(-103.971596), 1e-45);
        assert!(
            (f_expf(0f32) - 1f32).abs() < 1e-6,
            "Invalid result {}",
            f_expf(0f32)
        );
        assert!(
            (f_expf(5f32) - 148.4131591025766f32).abs() < 1e-6,
            "Invalid result {}",
            f_expf(5f32)
        );
        assert_eq!(f_expf(f32::INFINITY), f32::INFINITY);
        assert_eq!(f_expf(f32::NEG_INFINITY), 0.);
        assert!(f_expf(f32::NAN).is_nan());
    }
}
