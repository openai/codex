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

use crate::dyadic_float::{DyadicFloat128, DyadicSign};
use crate::polyeval::f_polyeval16;

// > procedure PRINTF128(a) {
//   write("{");
//   if (a < 0)
//     then write("Sign::NEG, ") else write("Sign::Pos, ");
//   a_exp = floor(log2(a)) + 1;
//   write((a + 2 ^ a_exp) * 2 ^ -128);
//   print("},");
// };
// > verbosity = 0;
// > procedure ASIN_APPROX(N, Deg) {
//     abs_error = 0;
//     rel_error = 0;
//     for i from 1 to N / 4 do {
//       Q = fpminimax(asin(sqrt(i / N + x)) / sqrt(i / N + x), Deg,
//                     [| 128... | ], [ -1 / (2 * N), 1 / (2 * N) ]);
//       abs_err = dirtyinfnorm(asin(sqrt(i / N + x)) - sqrt(i / N + x) * Q,
//                              [ -1 / (2 * N), 1 / (2 * N) ]);
//       rel_err = dirtyinfnorm(asin(sqrt(i / N + x)) / sqrt(i / N + x) - Q,
//                              [ -1 / (2 * N), 1 / (2 * N) ]);
//       if (abs_err > abs_error) then abs_error = abs_err;
//       if (rel_err > rel_error) then rel_error = rel_err;
//       write("{");
//       for j from 0 to Deg do PRINTF128(coeff(Q, j));
//       print("},");
//     };
//     print("Absolute Errors:", abs_error);
//     print("Relative Errors:", rel_error);
//   };
// > ASIN_APPROX(64, 15);
// ...
// Absolute Errors: 0x1.0b3...p-129
// Relative Errors: 0x1.1db...p-128
//
// For k = 0, we use Taylor polynomial of asin(x)/x around x = 0.
//   asin(x)/x ~ 1 + x^2/6 + (3 x^4)/40 + (5 x^6)/112 + (35 x^8)/1152 +
//               + (63 x^10)/2816 + (231 x^12)/13312 + (143 x^14)/10240 +
//               + (6435 x^16)/557056 + (12155 x^18)/1245184 +
//               + (46189 x^20)/5505024 + (88179 x^22)/12058624 +
//               + (676039 x^24)/104857600 + (1300075 x^26)/226492416 +
//               + (5014575 x^28)/973078528 + (9694845 x^30)/2080374784.
static ASIN_COEFFS_F128: [[DyadicFloat128; 16]; 17] = [
    [
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -127,
            mantissa: 0x80000000_00000000_00000000_00000000_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -130,
            mantissa: 0xaaaaaaaa_aaaaaaaa_aaaaaaaa_aaaaaaab_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -131,
            mantissa: 0x99999999_99999999_99999999_9999999a_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -132,
            mantissa: 0xb6db6db6_db6db6db_6db6db6d_b6db6db7_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -133,
            mantissa: 0xf8e38e38_e38e38e3_8e38e38e_38e38e39_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -133,
            mantissa: 0xb745d174_5d1745d1_745d1745_d1745d17_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -133,
            mantissa: 0x8e276276_27627627_62762762_76276276_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -134,
            mantissa: 0xe4cccccc_cccccccc_cccccccc_cccccccd_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -134,
            mantissa: 0xbd43c3c3_c3c3c3c3_c3c3c3c3_c3c3c3c4_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -134,
            mantissa: 0x9fef286b_ca1af286_bca1af28_6bca1af3_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -134,
            mantissa: 0x89779e79_e79e79e7_9e79e79e_79e79e7a_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -135,
            mantissa: 0xef9de9bd_37a6f4de_9bd37a6f_4de9bd38_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -135,
            mantissa: 0xd3431eb8_51eb851e_b851eb85_1eb851ec_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -135,
            mantissa: 0xbc16ed09_7b425ed0_97b425ed_097b425f_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -135,
            mantissa: 0xa8dd1846_9ee58469_ee58469e_e58469ee_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -135,
            mantissa: 0x98b41def_7bdef7bd_ef7bdef7_bdef7bdf_u128,
        },
    ],
    [
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -127,
            mantissa: 0x8055f060_94f0f05f_3ac3b927_50a701d9_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -130,
            mantissa: 0xad19c2ea_e3dd2429_8d04f71d_b965ee1b_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -131,
            mantissa: 0x9dfa882b_7b31af17_f9f19d33_0c45d24b_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -132,
            mantissa: 0xbedd3b58_c9e605ef_1404e1f0_4ba57940_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -132,
            mantissa: 0x83df2581_cb4fea82_b406f201_2fde6d5c_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -133,
            mantissa: 0xc534fe61_9b82dd16_ed5d8a43_f7710526_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -133,
            mantissa: 0x9b56fa62_88295ddf_ce8425fe_a04d733e_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -134,
            mantissa: 0xfdeddb19_4a030da7_27158080_d24caf46_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -134,
            mantissa: 0xd55827db_ff416ea8_042c4d8c_07cddeeb_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -134,
            mantissa: 0xb71d73a9_f2ba0688_5eaeeae9_413a0f5f_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -134,
            mantissa: 0x9fde87e2_ace91274_38f82666_d619c1ba_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -134,
            mantissa: 0x8d876557_5e4626a1_1b621336_93587847_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -135,
            mantissa: 0xfd801840_c8710595_6880fe13_a9657f8f_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -135,
            mantissa: 0xe54245a9_4c8c2ebb_30488494_64b0e34d_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -135,
            mantissa: 0xd11eb46f_4095a661_8890d123_15c96482_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -135,
            mantissa: 0xc01a4201_467fbc0b_960618d5_ec2adaa8_u128,
        },
    ],
    [
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -127,
            mantissa: 0x80ad1cbe_7878de11_4293301c_11ce9d49_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -130,
            mantissa: 0xaf9ac0df_3d845544_0fe5e31b_9051d03e_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -131,
            mantissa: 0xa28ceef8_d7297e05_f94773ad_f4a695c6_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -132,
            mantissa: 0xc75a5b77_58b4b11d_396c68ad_6733022b_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -132,
            mantissa: 0x8bde42a1_084a6674_50c5bceb_005d4b62_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -133,
            mantissa: 0xd471cdae_e2f35a96_bd4bc513_e0ccdf2c_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -133,
            mantissa: 0xa9fc6fd5_d204a4e3_e609940c_6b991b67_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -133,
            mantissa: 0x8d242d97_ba12b492_e25c7e7c_0c3fcf60_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -134,
            mantissa: 0xf0f1ba74_b149afc3_2f0bbab5_a20c6199_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -134,
            mantissa: 0xd21b42fb_d8e9098d_19612692_9a043332_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -134,
            mantissa: 0xba5e5492_7896a3e7_193a74d5_78631587_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -134,
            mantissa: 0xa7a17ae7_fc707f45_910e7a5d_c95251f4_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -134,
            mantissa: 0x98889a6a_b0370464_50c950d3_61d79ed7_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -134,
            mantissa: 0x8c29330e_4318fd29_25c5b528_84e39e7c_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -134,
            mantissa: 0x81e7bf48_b25bc7c0_b9204a4f_d4f5fa8b_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -135,
            mantissa: 0xf2801b09_11bf0768_773996dd_5224d852_u128,
        },
    ],
    [
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -127,
            mantissa: 0x81058e3e_f82ba622_ab81cd63_e1a91d57_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -130,
            mantissa: 0xb22e7055_c80dd354_8a2f2e8e_860d3f33_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -131,
            mantissa: 0xa753ce1a_7e3d1f57_247b37e6_03f93624_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -132,
            mantissa: 0xd05c5604_8eca8d18_dcdd76b7_f4b1f185_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -132,
            mantissa: 0x947cdd5e_f1d64df0_84f78df1_e2ecb854_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -133,
            mantissa: 0xe5218370_2ebbf6e8_3727a755_57843b93_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -133,
            mantissa: 0xba482553_383b92eb_186f78f1_8c35d6af_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -133,
            mantissa: 0x9d2b034a_7266c6a1_54b78a98_1a547429_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -133,
            mantissa: 0x8852f723_feea6046_e125f5a9_64e168e6_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -134,
            mantissa: 0xf19c9891_6c896c99_732052fe_5c54e992_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -134,
            mantissa: 0xd9cc81a5_c5ddf0f0_d651011e_a8ecd936_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -134,
            mantissa: 0xc7173169_dcb6095f_a6160847_b595aaff_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -134,
            mantissa: 0xb81cd3f6_4a422ebe_07aeb734_e4dcf3a1_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -134,
            mantissa: 0xabf01b1c_d15932aa_698d4382_512318a9_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -134,
            mantissa: 0xa1f1cf1b_d889a1ac_7120ca2f_bbbc1745_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -134,
            mantissa: 0x99a1b838_e38fbf11_429a4350_76b7d191_u128,
        },
    ],
    [
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -127,
            mantissa: 0x815f4e70_5c3e68f2_e84ed170_78211dfd_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -130,
            mantissa: 0xb4d5a992_de1ac4da_16fe6024_3a6cc371_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -131,
            mantissa: 0xac526184_bd558c65_66642dce_edc4b04a_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -132,
            mantissa: 0xd9ed9b03_46ec0bab_429ea221_4774bbc1_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -132,
            mantissa: 0x9dca410c_1efaeb74_87956685_dd5fe848_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -133,
            mantissa: 0xf76e411b_a926fc02_7f942265_9c39a882_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -133,
            mantissa: 0xcc71b004_eeb60c0f_1d387f76_44b46bf8_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -133,
            mantissa: 0xaf527a40_6f1084fb_5019904e_d12d384d_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -133,
            mantissa: 0x9a9304b0_d8a9de19_e1803691_269be22c_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -133,
            mantissa: 0x8b3d37c0_dbde09ef_342ddf4f_e80dd3fb_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -134,
            mantissa: 0xff2e9111_3a961c78_92297bab_cc257804_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -134,
            mantissa: 0xed1fb643_f2ca31c1_b0a1553a_e077285a_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -134,
            mantissa: 0xdeeb0f5e_81ad5e30_78d79ae3_83be1c18_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -134,
            mantissa: 0xd3a13ba6_8ce9abfc_a66eb1fd_c0c760fd_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -134,
            mantissa: 0xcaa8c381_d44bb44f_0ab25126_9a5fae10_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -134,
            mantissa: 0xc36fb2c4_244401cf_10dd8a39_78ccbf7f_u128,
        },
    ],
    [
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -127,
            mantissa: 0x81ba6750_6064f4dd_08015b7c_713688f0_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -130,
            mantissa: 0xb791524b_d975fdd1_584037b7_103b42ca_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -131,
            mantissa: 0xb18c26c5_3ced9856_db5bc672_cc95a64f_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -132,
            mantissa: 0xe4199ce5_d25be89b_4a0ad208_da77022d_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -132,
            mantissa: 0xa7d77999_0f80e3e9_7e97e9d1_0e337550_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -132,
            mantissa: 0x85c3e039_8959c95b_e6e1e87f_7e6636b1_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -133,
            mantissa: 0xe0b90ecd_95f7e6eb_a675bae0_628bd214_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -133,
            mantissa: 0xc3edb6b4_ed0a684c_c7a3ee4d_f1dcd3f9_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -133,
            mantissa: 0xafa274d2_e66e1f61_9e8ab3c7_7221214e_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -133,
            mantissa: 0xa0dd903d_e110b71a_8a1fc9df_cc080308_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -133,
            mantissa: 0x95e2f38c_60441961_72b90625_e3a37573_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -133,
            mantissa: 0x8d9fe38f_2c705139_029f857c_9f628b2b_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -133,
            mantissa: 0x8762410a_4967a974_6b609e83_7c025a39_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -133,
            mantissa: 0x82b220be_d9ec0e5a_9ce9af7c_c65c94b9_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -134,
            mantissa: 0xfe866073_2312c056_4265d82a_3afea10c_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -134,
            mantissa: 0xf99b667c_5f8ef6a6_11fafa4d_5c76ebb3_u128,
        },
    ],
    [
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -127,
            mantissa: 0x8216e353_2ffdf638_15d72316_a2f327f2_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -130,
            mantissa: 0xba625eba_097ce944_7024c0a3_c873729b_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -131,
            mantissa: 0xb704e369_5b95ce44_cde30106_90e92cc3_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -132,
            mantissa: 0xeeecee6d_7298b8a3_075da5d7_456bdcde_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -132,
            mantissa: 0xb2b78fb1_fcfdc273_1d1ac11c_e29c16f1_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -132,
            mantissa: 0x90d21722_148fdaf5_0d566a01_0bb8784b_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -133,
            mantissa: 0xf7681c54_9771ebb6_17686858_eb5e1caf_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -133,
            mantissa: 0xdb5e45c0_52ec0c1c_ff28765e_d4c44bfb_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -133,
            mantissa: 0xc7ff0dd7_a34ee29b_7cb689af_fe887bf5_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -133,
            mantissa: 0xba4e6f37_a98a3e3f_f1175427_20f45c82_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -133,
            mantissa: 0xb08f6e11_688e4174_b3d48abe_c0a6d5cd_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -133,
            mantissa: 0xa9af6a33_14aabe45_26da1218_05bbb52e_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -133,
            mantissa: 0xa4fd22fa_1b4f0d7f_1456af96_cbd0cde6_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -133,
            mantissa: 0xa20229b4_7e9c2e39_22c49987_66a05c5a_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -133,
            mantissa: 0xa0775ca8_4409c735_351d01f1_34467927_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -133,
            mantissa: 0xa010d2d9_08428a53_53603f20_66c8b8ba_u128,
        },
    ],
    [
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -127,
            mantissa: 0x8274cd6a_f25e642d_0b1a02fb_03f53f3e_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -130,
            mantissa: 0xbd49d2c8_b9005b2a_ee795b17_92181a48_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -131,
            mantissa: 0xbcc0ac23_98e00fd7_c40811f5_486aca6a_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -132,
            mantissa: 0xfa756493_b381b917_6cdea268_e44dd2fd_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -132,
            mantissa: 0xbe7fce1e_462b43c6_0537d6f7_138c87ac_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -132,
            mantissa: 0x9d00958b_edc83095_b4cc907c_a92c30f1_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -132,
            mantissa: 0x886a2440_ed93d825_333c19c2_6de36d73_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -133,
            mantissa: 0xf616ebc0_4f576462_d9312544_e8fbe0fd_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -133,
            mantissa: 0xe43f4c9d_ebb5d685_00903a00_7bd6ad39_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -133,
            mantissa: 0xd8516eab_32337672_569b4e19_a44e795c_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -133,
            mantissa: 0xd091fa04_954666ee_cc4da283_82e977c0_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -133,
            mantissa: 0xcbf13442_c4c0f859_0449c2c4_2fc046fe_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -133,
            mantissa: 0xc9c1d1b4_dea4c76c_d101e562_dc3af77f_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -133,
            mantissa: 0xc9924d2a_b8ec37d9_80af1780_0fb63e4e_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -133,
            mantissa: 0xcb24b252_1ff37e4a_41f35260_2b9ace95_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -133,
            mantissa: 0xce2d87ac_194a6304_1658ed0e_4cdb8161_u128,
        },
    ],
    [
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -127,
            mantissa: 0x82d4310f_f58b570d_266275fc_1d085c87_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -130,
            mantissa: 0xc048c361_72bee7b0_8d2ca7e5_afe4f335_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -131,
            mantissa: 0xc2c3ecca_216e290e_b99c5c53_5d48595a_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -131,
            mantissa: 0x83611e8f_3adf2217_be3c342a_dfb1c562_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -132,
            mantissa: 0xcb481202_8b0ba9aa_e586f73d_faea68e4_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -132,
            mantissa: 0xaa727c9a_4caba65d_c8dc13ef_8bed52e4_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -132,
            mantissa: 0x96b05462_efac126e_db6871d0_0be1eff9_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -132,
            mantissa: 0x8a4f8752_9b3c9232_63eb1596_a2c83eb4_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -132,
            mantissa: 0x828be6f4_1b14e6e6_8efc1012_2afe425a_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -133,
            mantissa: 0xfbd2f055_9d699ea9_b572008e_1fb08088_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -133,
            mantissa: 0xf71b3c70_dc4610e6_bc1e581c_817b88bd_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -133,
            mantissa: 0xf5e8ebf6_3b0aef3f_97ba4c8f_e49b6f0a_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -133,
            mantissa: 0xf7986238_1eb8bd7a_73577ed0_c05e4abf_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -133,
            mantissa: 0xfbc3832a_a903cd65_a46ee523_f342c621_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -132,
            mantissa: 0x811ea5f3_7409245e_1777fdd1_59b29f80_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -132,
            mantissa: 0x85619588_b83c90ef_67740d6a_d2f372a8_u128,
        },
    ],
    [
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -127,
            mantissa: 0x83351a49_8764656f_e1774024_a5e751a6_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -130,
            mantissa: 0xc36057da_23d39c2b_336474e0_3a893914_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -131,
            mantissa: 0xc913714c_a46cc0bf_3bdd68ba_53a309d4_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -131,
            mantissa: 0x89f2254d_f1469d60_e1324bac_95db6742_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -132,
            mantissa: 0xd92b27f6_38df6911_5842365c_c120cc63_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -132,
            mantissa: 0xb94ff079_7848d391_486efffa_a6fbc37f_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -132,
            mantissa: 0xa6c03919_862e8437_70f86a73_43da3a6e_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -132,
            mantissa: 0x9bcb70c9_a378e97f_a59f25f3_ba202e33_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -132,
            mantissa: 0x95b103b0_62aa9f64_ee2d6146_76020bc5_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -132,
            mantissa: 0x92fa4a1c_7d7fd161_8f25aa4e_f65ca52f_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -132,
            mantissa: 0x92d387a2_c5dd771d_4015ca29_e3eda1d9_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -132,
            mantissa: 0x94c13c5c_997615c3_8a2f63c8_c314226f_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -132,
            mantissa: 0x987b8c8f_5e9e7a5f_e8497909_d60d1194_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -132,
            mantissa: 0x9ddb0978_da99e6ad_83d5eca2_9d079ef7_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -132,
            mantissa: 0xa4d9aeee_4b512ed4_5ec95cd1_37ce3f22_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -132,
            mantissa: 0xad602af3_1e14d681_8a267da2_57c030de_u128,
        },
    ],
    [
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -127,
            mantissa: 0x839795b7_8f3005a4_689f57cc_d201f7dc_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -130,
            mantissa: 0xc691cb89_3d75d3d5_a1892f2a_bf54ec45_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -131,
            mantissa: 0xcfb46fc4_6d28c32c_9ae5ad3d_a7749dc8_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -131,
            mantissa: 0x90f71352_c806c830_20edb8b2_7594386b_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -132,
            mantissa: 0xe8473840_d511dc77_d63def5d_7f4de9c0_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -132,
            mantissa: 0xc9c6eb30_aaf2b63d_ec20f671_8689534a_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -132,
            mantissa: 0xb8dcfa84_eb6cab93_3023ddcc_b8f68a2f_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -132,
            mantissa: 0xafde4094_c1a14390_9609a3ea_847225a9_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -132,
            mantissa: 0xac1254e7_5852a836_b2aca5e5_0cfc484f_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -132,
            mantissa: 0xac0d3ffa_d6171016_b1a12557_858663c1_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -132,
            mantissa: 0xaf0877f9_0ca5c52f_fc54b5af_b5cbc350_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -132,
            mantissa: 0xb498574f_af349a2b_f391ff83_b3570919_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -132,
            mantissa: 0xbc87c7bb_34182440_280647cd_976affb0_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -132,
            mantissa: 0xc6c5688f_58a42593_4569de36_0855c393_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -132,
            mantissa: 0xd368b088_5bb9496a_dd7c92df_8798aaf7_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -132,
            mantissa: 0xe272168a_c8dbe668_381542bf_fc24c266_u128,
        },
    ],
    [
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -127,
            mantissa: 0x83fbb09c_fbb0ebf4_208c9037_70373f79_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -130,
            mantissa: 0xc9de6f84_8e652b0b_3b2a2bb9_f7ce3de8_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -131,
            mantissa: 0xd6ac93c7_6e215233_f184fdcc_e5872970_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -131,
            mantissa: 0x987a35b9_87c02522_1927dee9_70fc6b18_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -132,
            mantissa: 0xf8be450d_266409a9_2e534ffd_905f4424_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -132,
            mantissa: 0xdc0c36d7_34415e3b_c5121c4d_4e28c17d_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -132,
            mantissa: 0xcd551b98_81d982a8_1399d9ba_ddf55821_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -132,
            mantissa: 0xc6f91e3f_428d6be3_646f3147_20445145_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -132,
            mantissa: 0xc64f100c_85e1e8f1_6f501d1e_2155f872_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -132,
            mantissa: 0xc9fe25ae_295f1f24_5924cf9a_036a31f2_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -132,
            mantissa: 0xd157410e_fcc10fbb_fceb318a_b4990bd7_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -132,
            mantissa: 0xdc0aeb56_ca679f92_3b3c44d8_99b1add7_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -132,
            mantissa: 0xea05b383_bc339550_e5c5c34b_bfa416a1_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -132,
            mantissa: 0xfb5e3897_5a5c8f62_280a90dc_9ebe9107_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -131,
            mantissa: 0x88301d81_b38f225d_2226ab7e_df342d90_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -131,
            mantissa: 0x949e3465_e4a8aef7_46311182_5fc3fde8_u128,
        },
    ],
    [
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -127,
            mantissa: 0x846178eb_1c7260da_3e0aca9a_51e68d84_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -130,
            mantissa: 0xcd47ac90_3c311c2b_98dd7493_4656d210_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -131,
            mantissa: 0xde020b2d_abd5628c_b88634e5_73f312fc_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -131,
            mantissa: 0xa086fafa_c220fb73_9939cae3_2d69683f_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -131,
            mantissa: 0x855b5efa_f6963d73_e4664cb1_d43f03a9_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -132,
            mantissa: 0xf05c9774_fe0de25c_ccf1c1df_d2ed9941_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -132,
            mantissa: 0xe484a941_19639229_f06ae955_f8edc7d1_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -132,
            mantissa: 0xe1a32bb2_52ca122c_bf2f0904_cfc476cb_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -132,
            mantissa: 0xe528e091_7bb8a01a_9218ce3e_1e85af60_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -132,
            mantissa: 0xeddd556a_faa2d46f_e91c61fa_adf12aec_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -132,
            mantissa: 0xfb390fa3_15e9d55f_5683c0c4_c7719f81_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -131,
            mantissa: 0x868e5fa4_15597c8f_7c42a262_8f2d6332_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -131,
            mantissa: 0x91d79767_a3d037f9_cd84ead5_c0714310_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -131,
            mantissa: 0x9fa6a035_915bc052_377a8abb_faf4e3c6_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -131,
            mantissa: 0xb04edefd_6ac2a93e_ec33e6f6_3d53e7c2_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -131,
            mantissa: 0xc416980d_dc5c186b_7bdcded6_97ea5844_u128,
        },
    ],
    [
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -127,
            mantissa: 0x84c8fd4d_ffdf9fc6_bdd7ebca_88183d7b_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -130,
            mantissa: 0xd0cf0544_11dbf845_cb6eeae5_bc980e2f_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -131,
            mantissa: 0xe5bb9480_7ce0eaca_74300a46_8398e944_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -131,
            mantissa: 0xa92a18f8_d611860b_5f2ef8c6_8e8ca002_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -131,
            mantissa: 0x8f2e1684_17eb4e6c_1ec44b9b_e4b1c3e5_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -131,
            mantissa: 0x837f1764_0ee8f416_8694b4a1_c647af0c_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -132,
            mantissa: 0xfed7e2a9_05a5190e_b7d70a61_a24ad801_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -131,
            mantissa: 0x803f29ff_dc6fd2bc_3c3c4b50_a9dc860c_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -131,
            mantissa: 0x84c61e09_b8aa35e4_96239f9c_b1d00b3c_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -131,
            mantissa: 0x8c7ed311_f77980d6_842ddf90_6a68a0bc_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -131,
            mantissa: 0x9746077b_d397c2d1_038a4744_a76f5fb5_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -131,
            mantissa: 0xa5341277_c4185ace_54f26328_322158e8_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -131,
            mantissa: 0xb68d78f5_0972f6de_9189aa23_d3ecefc2_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -131,
            mantissa: 0xcbbcefc2_15bade4e_f1d36947_c8b6e460_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -131,
            mantissa: 0xe564a459_c851390d_d45a4748_f29f182b_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -130,
            mantissa: 0x820ea28b_c89662c3_2a64ccdc_efb2b259_u128,
        },
    ],
    [
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -127,
            mantissa: 0x85324d39_f30f9174_ac0d817e_9c744b0b_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -130,
            mantissa: 0xd476186e_49c47f3a_a71f8886_7f9f21c4_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -131,
            mantissa: 0xede08f54_a830e87b_07881700_65e57b6c_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -131,
            mantissa: 0xb271b8eb_309963ee_89187c73_0b92f7d5_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -131,
            mantissa: 0x99f0011d_95d3a6dd_282bd00a_db808151_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -131,
            mantissa: 0x9021134e_02b479e7_3aabf9bb_b7ab6cf3_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -131,
            mantissa: 0x8e673bf2_f11db54a_909c4c72_6389499f_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -131,
            mantissa: 0x9226a371_88dd55f7_bfe21777_4a42a7ae_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -131,
            mantissa: 0x9a4d78fc_9df79d9a_44609c02_a625808a_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -131,
            mantissa: 0xa68335fb_41d2d91c_e7bbd2a3_31a1d17b_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -131,
            mantissa: 0xb6d89c39_28d0cb26_809d4df6_e55cba1a_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -131,
            mantissa: 0xcba71468_9177fc2d_7f23df2f_37226488_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -131,
            mantissa: 0xe5846de8_44833ae9_34416c87_0315eb9e_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -130,
            mantissa: 0x82a07032_64e6226b_200d94a1_66fc7951_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -130,
            mantissa: 0x9602695c_b6fa8886_68ca0cba_b59ea683_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -130,
            mantissa: 0xad7d185a_ab3d14dd_d908a7b1_c57352bb_u128,
        },
    ],
    [
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -127,
            mantissa: 0x859d78fa_4405d8fa_287dbc69_95d0975e_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -130,
            mantissa: 0xd83ea3bc_131d6baa_67c51d88_4c4dae01_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -131,
            mantissa: 0xf6790edb_df07342b_aad85870_167af128_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -131,
            mantissa: 0xbc6daa33_12be0f85_bc7fa753_52b10a83_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -131,
            mantissa: 0xa5bd41bc_9c986b13_1af2542e_92aacb59_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -131,
            mantissa: 0x9e4358bc_24e04364_b4539b76_e444b790_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -131,
            mantissa: 0x9f7fc21b_dca1f2b5_f3f6d44b_c5a37626_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -131,
            mantissa: 0xa6fd793c_0b9c44c1_30a518cc_66b5e511_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -131,
            mantissa: 0xb3dccfac_cd1592b3_bcd6b7c0_9749993d_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -131,
            mantissa: 0xc6056c3a_4a5f329a_48f1429d_27f930fc_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -131,
            mantissa: 0xddd9e529_858a4502_6e7f3d1c_1e7dcb89_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -131,
            mantissa: 0xfc1bccee_dc8d2567_1721c468_6f7f53ec_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -130,
            mantissa: 0x90f2bb21_5cdbe7e2_f9ef8e12_059cc66a_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -130,
            mantissa: 0xa857d5df_5b4da940_15ce4e95_7201fc79_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -130,
            mantissa: 0xc54119c0_10c02bf4_d87ece17_1ef85c5f_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -130,
            mantissa: 0xe8c50ebc_880356de_2c1f4c42_9ee9748f_u128,
        },
    ],
    [
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -127,
            mantissa: 0x860a91c1_6b9b2c23_2dd99707_ab3d688b_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -130,
            mantissa: 0xdc2a86b1_5fdb645d_ea2781dd_25555f49_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -131,
            mantissa: 0xff8def07_d1e514d7_b2e8ebb6_5c3afe5e_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -131,
            mantissa: 0xc72f9d5b_4fb559e3_20db92e3_a5ae3f73_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -131,
            mantissa: 0xb2b5f45b_1d26f4dd_0b210309_fb68914f_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -131,
            mantissa: 0xae1cbaae_c7b55465_4da858f5_47e62a37_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -131,
            mantissa: 0xb30f3998_10202a0d_a52ec085_a7d63289_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -131,
            mantissa: 0xbf51f27f_b7aff89d_dc24e2aa_208d2054_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -131,
            mantissa: 0xd250735e_87d0b527_6f99bcc9_bd6fc717_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -131,
            mantissa: 0xec543ec2_bddb2efb_36d9ce81_a7c84336_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -130,
            mantissa: 0x871f73e3_298ef45c_eed83998_2bc731b9_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -130,
            mantissa: 0x9cbb5447_af8574f1_21fa4cda_93d82b7e_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -130,
            mantissa: 0xb7f5a6c0_430a347f_11b22cde_91de0885_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -130,
            mantissa: 0xda153cc4_14abdb96_840df7c2_3299fec0_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -129,
            mantissa: 0x826c129b_3e4a2612_b2cd11f1_4d2ba60c_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -129,
            mantissa: 0x9d19c289_fc0e8aa4_f351418b_b760ce90_u128,
        },
    ],
];

#[cold]
pub(crate) fn asin_eval_dyadic(u: DyadicFloat128, idx: usize) -> DyadicFloat128 {
    let coeffs = ASIN_COEFFS_F128[idx];
    f_polyeval16(
        u, coeffs[0], coeffs[1], coeffs[2], coeffs[3], coeffs[4], coeffs[5], coeffs[6], coeffs[7],
        coeffs[8], coeffs[9], coeffs[10], coeffs[11], coeffs[12], coeffs[13], coeffs[14],
        coeffs[15],
    )
}
