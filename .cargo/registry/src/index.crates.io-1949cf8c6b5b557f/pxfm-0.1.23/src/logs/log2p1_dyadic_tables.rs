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
use crate::dyadic_float::{DyadicFloat128, DyadicSign};

pub(crate) static LOG2P1_F128_POLY: [DyadicFloat128; 13] = [
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0x8000_0000_0000_0000_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -129,
        mantissa: 0xffff_ffff_ffff_ffff_ffff_ffff_ffff_ebd8_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -129,
        mantissa: 0xaaaa_aaaa_aaaa_aaaa_aaaa_aaaa_a5c4_8b54_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -130,
        mantissa: 0xffff_ffff_ffff_ffff_ffff_ff22_4582_3ae0_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -130,
        mantissa: 0xcccc_cccc_cccc_cccc_ccc2_ca18_b08f_e343_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -130,
        mantissa: 0xaaaa_aaaa_aaaa_aaaa_6637_fd4b_1974_3eec_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -130,
        mantissa: 0x9249_2492_4924_911d_862b_c3d3_3abb_3649_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -131,
        mantissa: 0xffff_ffff_fff9_24cc_05b3_08e3_9fa7_dfb5_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -131,
        mantissa: 0xe38e_38e3_807c_fa4b_c976_e6cb_d22e_203f_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -131,
        mantissa: 0xcccc_ccb9_ec01_7492_f934_e28d_924e_76d4_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -131,
        mantissa: 0xba2e_7a1e_af85_6174_70e5_c5a5_ebbe_0226_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -131,
        mantissa: 0xaaa0_2d43_f696_c3e4_4dbe_7546_67b6_bc48_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -131,
        mantissa: 0x99df_88a0_4308_13ca_a1cf_fb6e_966a_70f6_u128,
    },
];

pub(crate) static LOG2P1_INVERSE_2: [DyadicFloat128; 240] = [
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -126,
        mantissa: 0x8000_0000_0000_0000_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0xfe03_f80f_e03f_80ff_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0xfc0f_c0fc_0fc0_fc10_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0xfa23_2cf2_5213_8ac0_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0xf83e_0f83_e0f8_3e10_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0xf660_3d98_0f66_03da_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0xf489_8d5f_85bb_3951_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0xf2b9_d648_0f2b_9d65_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0xf0f0_f0f0_f0f0_f0f1_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0xef2e_b71f_c434_5239_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0xed73_03b5_cc0e_d731_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0xebbd_b2a5_c161_9c8c_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0xea0e_a0ea_0ea0_ea0f_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0xe865_ac7b_7603_a197_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0xe6c2_b448_1cd8_568a_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0xe525_982a_f70c_880f_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0xe38e_38e3_8e38_e38f_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0xe1fc_780e_1fc7_80e2_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0xe070_381c_0e07_0382_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0xdee9_5c4c_a037_ba58_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0xdd67_c8a6_0dd6_7c8b_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0xdbeb_61ee_d19c_5958_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0xda74_0da7_40da_740e_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0xd901_b203_6406_c80e_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0xd794_35e5_0d79_435f_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0xd62b_80d6_2b80_d62c_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0xd4c7_7b03_531d_ec0e_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0xd368_0d36_80d3_680e_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0xd20d_20d2_0d20_d20e_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0xd0b6_9fcb_d258_0d0c_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0xcf64_74a8_819e_c8ea_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0xce16_8a77_2508_0ce2_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0xcccc_cccc_cccc_cccd_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0xcb87_27c0_65c3_93e1_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0xca45_87e6_b74f_032a_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0xc907_da4e_8711_46ad_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0xc7ce_0c7c_e0c7_ce0d_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0xc698_0c69_80c6_980d_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0xc565_c87b_5f9d_4d1c_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0xc437_2f85_5d82_4ca6_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0xc30c_30c3_0c30_c30d_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0xc1e4_bbd5_95f6_e948_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0xc0c0_c0c0_c0c0_c0c1_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0xbfa0_2fe8_0bfa_02ff_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0xbe82_fa0b_e82f_a0bf_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0xbd69_1047_0766_1aa3_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0xbc52_640b_c526_40bd_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0xbb3e_e721_a54d_880c_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0xba2e_8ba2_e8ba_2e8c_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0xb921_43fa_36f5_e02f_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0xb817_02e0_5c0b_8171_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0xb70f_bb5a_19be_3659_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0xb60b_60b6_0b60_b60c_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0xb509_e68a_9b94_8220_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0xb40b_40b4_0b40_b40c_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0xb30f_6352_8917_c80c_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0xb216_42c8_590b_2165_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0xb11f_d3b8_0b11_fd3c_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0xb02c_0b02_c0b0_2c0c_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0xaf3a_ddc6_80af_3ade_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0xae4c_415c_9882_b932_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0xad60_2b58_0ad6_02b6_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0xac76_9184_0ac7_6919_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0xab8f_69e2_8359_cd12_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0xaaaa_aaaa_aaaa_aaab_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0xa9c8_4a47_a07f_5638_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0xa8e8_3f57_17c0_a8e9_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0xa80a_80a8_0a80_a80b_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0xa72f_0539_7829_cbc2_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0xa655_c439_2d7b_73a8_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0xa57e_b502_95fa_d40b_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0xa4a9_cf1d_9683_3752_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0xa3d7_0a3d_70a3_d70b_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0xa306_5e3f_ae7c_d0e1_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0xa237_c32b_16cf_d773_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0xa16b_312e_a8fc_377d_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0xa0a0_a0a0_a0a0_a0a1_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0x9fd8_09fd_809f_d80a_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0x9f11_65e7_2548_13e3_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0x9e4c_ad23_dd5f_3a21_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0x9d89_d89d_89d8_9d8a_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0x9cc8_e160_c3fb_19b9_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0x9c09_c09c_09c0_9c0a_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0x9b4c_6f9e_f03a_3caa_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0x9a90_e7d9_5bc6_09aa_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0x99d7_22da_bde5_8f07_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0x991f_1a51_5885_fb38_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0x9868_c809_868c_8099_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0x97b4_25ed_097b_425f_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0x9701_2e02_5c04_b80a_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0x964f_da6c_0964_fda7_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0x95a0_2568_095a_0257_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0x94f2_094f_2094_f20a_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0x9445_8094_4580_9446_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0x939a_85c4_0939_a85d_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0x92f1_1384_0497_889d_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0x9249_2492_4924_924a_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0x91a2_b3c4_d5e6_f80a_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0x90fd_bc09_0fdb_c091_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0x905a_3863_3e06_c43b_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0x8fb8_23ee_08fb_823f_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0x8f17_79d9_fdc3_a219_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0x8e78_356d_1408_e784_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0x8dda_5202_3769_4809_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0x8d3d_cb08_d3dc_b08e_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0x8ca2_9c04_6514_e024_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0x8c08_c08c_08c0_8c09_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0x8b70_344a_139b_c75b_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0x8ad8_f2fb_a938_6823_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0x8a42_f870_5669_db47_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0x89ae_4089_ae40_89af_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0x891a_c73a_e981_9b51_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0x8888_8888_8888_8889_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0x87f7_8087_f780_87f8_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0x8767_ab5f_34e4_7ef2_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0x86d9_0544_7a34_acc7_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0x864b_8a7d_e6d1_d609_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0x85bf_3761_2cee_3c9b_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0x8534_0853_4085_3409_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0x84a9_f9c8_084a_9f9d_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0x8421_0842_1084_2109_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0x8399_3052_3fbe_3368_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0x8312_6e97_8d4f_df3c_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0x828c_bfbe_b9a0_20a4_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0x8208_2082_0820_8209_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0x8184_8da8_faf0_d278_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0x8102_0408_1020_4082_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0x8000_0000_0000_0000_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0x8000_0000_0000_0000_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xff00_ff00_ff00_ff02_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xfe03_f80f_e03f_80ff_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xfd08_e550_0fd0_8e56_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xfc0f_c0fc_0fc0_fc11_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xfb18_8565_06dd_aba7_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xfa23_2cf2_5213_8ac1_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xf92f_b221_1855_a866_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xf83e_0f83_e0f8_3e11_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xf74e_3fc2_2c70_0f76_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xf660_3d98_0f66_03db_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xf574_03d5_d00f_5741_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xf489_8d5f_85bb_3951_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xf3a0_d52c_ba87_2337_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xf2b9_d648_0f2b_9d66_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xf1d4_8bce_e0d3_99fb_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xf0f0_f0f0_f0f0_f0f2_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xf00f_00f0_0f00_f010_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xef2e_b71f_c434_5239_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xee50_0ee5_00ee_5010_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xed73_03b5_cc0e_d731_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xec97_9118_f3fc_4da3_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xebbd_b2a5_c161_9c8d_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xeae5_6403_ab95_9010_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xea0e_a0ea_0ea0_ea10_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xe939_651f_e2d8_d35d_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xe865_ac7b_7603_a198_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xe793_72e2_25fe_30da_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xe6c2_b448_1cd8_568a_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xe5f3_6cb0_0e5f_36cc_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xe525_982a_f70c_880f_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xe459_32d7_dc52_100f_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xe38e_38e3_8e38_e38f_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xe2c4_a688_6a4c_2e11_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xe1fc_780e_1fc7_80e3_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xe135_a9c9_7500_e137_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xe070_381c_0e07_0383_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xdfac_1f74_346c_5760_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xdee9_5c4c_a037_ba58_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xde27_eb2c_41f3_d9d2_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xdd67_c8a6_0dd6_7c8b_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xdca8_f158_c7f9_1ab9_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xdbeb_61ee_d19c_5959_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xdb2f_171d_f770_291a_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xda74_0da7_40da_740f_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xd9ba_4256_c036_6e92_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xd901_b203_6406_c80f_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xd84a_598e_c915_1f44_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xd794_35e5_0d79_435f_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xd6df_43fc_a482_f00e_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xd62b_80d6_2b80_d62d_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xd578_e97c_3f5f_e552_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xd4c7_7b03_531d_ec0e_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xd417_3289_870a_c52f_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xd368_0d36_80d3_680e_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xd2ba_083b_4452_50ac_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xd20d_20d2_0d20_d20e_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xd161_543e_28e5_0275_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xd0b6_9fcb_d258_0d0c_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xd00d_00d0_0d00_d00e_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xcf64_74a8_819e_c8ea_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xcebc_f8bb_5b41_69cc_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xce16_8a77_2508_0ce2_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xcd71_2752_a886_d243_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xcccc_cccc_cccc_ccce_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xcc29_786c_7607_f9a0_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xcb87_27c0_65c3_93e1_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xcae5_d85f_1bbd_6c96_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xca45_87e6_b74f_032a_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xc9a6_33fc_d967_300e_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xc907_da4e_8711_46ae_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xc86a_7890_0c86_a78a_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xc7ce_0c7c_e0c7_ce0d_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xc732_93d7_89b9_f839_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xc698_0c69_80c6_980d_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xc5fe_7403_17f9_d00d_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xc565_c87b_5f9d_4d1d_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xc4ce_07b0_0c4c_e07c_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xc437_2f85_5d82_4ca7_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xc3a1_3de6_0495_c774_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xc30c_30c3_0c30_c30d_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xc278_0613_c030_9e03_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xc1e4_bbd5_95f6_e948_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xc152_500c_1525_00c2_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xc0c0_c0c0_c0c0_c0c2_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xc030_0c03_00c0_300d_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xbfa0_2fe8_0bfa_0300_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xbf11_2a8a_d278_e8de_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xbe82_fa0b_e82f_a0c0_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xbdf5_9c91_700b_df5b_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xbd69_1047_0766_1aa4_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xbcdd_535d_b1cc_5b7c_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xbc52_640b_c526_40bd_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xbbc8_408c_d630_69a2_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xbb3e_e721_a54d_880d_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xbab6_5610_0bab_6562_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xba2e_8ba2_e8ba_2e8d_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xb9a7_862a_0ff4_6589_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xb921_43fa_36f5_e02f_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xb89b_c36c_e3e0_453b_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xb817_02e0_5c0b_8171_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xb793_00b7_9300_b794_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xb70f_bb5a_19be_365a_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xb68d_3134_0e43_07d9_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xb60b_60b6_0b60_b60c_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xb58a_4855_18d1_e7e5_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xb509_e68a_9b94_8220_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xb48a_39d4_4685_fe98_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xb40b_40b4_0b40_b40c_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xb38c_f9b0_0b38_cf9c_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xb30f_6352_8917_c80c_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -128,
        mantissa: 0xb292_7c29_da55_19d0_0000_0000_0000_0000_u128,
    },
];

pub(crate) static LOG2P1_LOG_INV_2: [DyadicFloat128; 240] = [
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -128,
        mantissa: 0xb172_17f7_d1cf_79ab_c9e3_b398_03f2_f6af_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -128,
        mantissa: 0xaf74_1551_20c9_011d_046d_235e_e630_73dc_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -128,
        mantissa: 0xad7a_02e1_b24e_fd32_1608_64fd_949b_4bd3_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -128,
        mantissa: 0xab83_d135_dc63_3301_ffe6_607b_a902_ef3b_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -128,
        mantissa: 0xa991_7134_33c2_b999_0ba4_aea6_14d0_5700_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -128,
        mantissa: 0xa7a2_d41a_d270_c9d7_cd36_2382_a768_8479_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -128,
        mantissa: 0xa5b7_eb7c_b860_fb89_7b6a_62a0_dec6_e072_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -128,
        mantissa: 0xa3d0_a93f_4516_9a4b_0959_4fab_088c_0d64_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -128,
        mantissa: 0xa1ec_ff97_c91e_267b_1b7e_fae0_8e59_7e16_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -128,
        mantissa: 0xa00c_e109_2e54_98c4_6987_9c5a_30cd_1241_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -128,
        mantissa: 0x9e30_4061_b5fd_a91a_0460_3d87_b6df_81ac_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -128,
        mantissa: 0x9c57_10b8_cbb7_3a42_aa55_4b2d_d461_9e63_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -128,
        mantissa: 0x9a81_456c_ec64_2e10_4d49_f9aa_ea3c_b5e0_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -128,
        mantissa: 0x98ae_d221_a034_58b6_732f_8932_1647_b358_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -128,
        mantissa: 0x96df_aabd_86fa_1647_d611_88fb_c94e_2f14_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -128,
        mantissa: 0x9513_c368_7608_3696_b5cb_c416_a241_8011_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -128,
        mantissa: 0x934b_1089_a6dc_93c2_bf5b_b3b6_0554_e151_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -128,
        mantissa: 0x9185_86c5_f5e4_bf01_9f92_199e_d1a4_bab0_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -128,
        mantissa: 0x8fc3_1afe_30b2_c6de_e300_bf16_7e95_da66_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -128,
        mantissa: 0x8e03_c24d_7300_395a_cdda_e1cc_ce24_7837_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -128,
        mantissa: 0x8c47_7207_91e5_3314_762a_d194_15fe_25a5_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -128,
        mantissa: 0x8a8e_1fb7_94b0_9134_9eb6_28db_a173_c82d_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -128,
        mantissa: 0x88d7_c11e_3ad5_3cdc_8a31_11a7_07b6_de2c_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -128,
        mantissa: 0x8724_4c30_8e67_0a66_85e0_05d0_6dbf_a8f7_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -128,
        mantissa: 0x8573_b716_82a7_d21b_b21f_9f89_c1ab_80b2_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -128,
        mantissa: 0x83c5_f829_9e2b_4091_b8f6_fafe_8fbb_68b8_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -128,
        mantissa: 0x821b_05f3_b01d_6774_db0d_58c3_f7e2_ea1e_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -128,
        mantissa: 0x8072_d72d_903d_588c_7dd1_b09c_70c4_0109_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -129,
        mantissa: 0xfd9a_c57b_d244_2180_af05_924d_258c_14c4_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -129,
        mantissa: 0xfa55_3f70_18c9_66f4_2780_a545_a1b5_4dce_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -129,
        mantissa: 0xf715_0ab5_a09f_27f6_0a47_0250_d40e_be8e_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -129,
        mantissa: 0xf3da_161e_ed6b_9ab1_248d_42f7_8d3e_65d2_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -129,
        mantissa: 0xf0a4_50d1_3936_6ca7_7c66_eb64_08ff_6432_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -129,
        mantissa: 0xed73_aa42_64b0_adeb_5391_cf4b_33e4_2996_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -129,
        mantissa: 0xea48_1236_f7d3_5bb2_39a7_67a8_0d6d_97e6_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -129,
        mantissa: 0xe721_78c0_323a_1a0f_cc4e_1653_e71d_9973_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -129,
        mantissa: 0xe3ff_ce3a_2aa6_4923_8ead_b651_b49a_c539_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -129,
        mantissa: 0xe0e3_0349_fd1c_ec82_03e8_e180_2aba_24d5_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -129,
        mantissa: 0xddcb_08dc_0717_d85c_940a_666c_8784_2842_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -129,
        mantissa: 0xdab7_d022_3148_4a93_bec2_0cca_6efe_2ac4_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -129,
        mantissa: 0xd7a9_4a92_466e_833c_cd88_bba7_d0ce_e8df_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -129,
        mantissa: 0xd49f_69e4_56cf_1b7b_7f53_bd2e_406e_66e6_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -129,
        mantissa: 0xd19a_2011_27d3_c646_279d_79f5_1dcc_7301_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -129,
        mantissa: 0xce99_5f50_af69_d863_432f_3f4f_861a_d6a8_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -129,
        mantissa: 0xcb9d_1a18_9ab5_6e77_7d7e_9307_c70c_0667_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -129,
        mantissa: 0xc8a5_431a_dfb4_4ca6_048c_e7c1_a75e_341a_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -129,
        mantissa: 0xc5b1_cd44_596f_a51f_f218_fb8f_9f9e_f27f_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -129,
        mantissa: 0xc2c2_abbb_6e5f_d570_0333_7789_d592_e296_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -129,
        mantissa: 0xbfd7_d1de_c0a8_df70_37ed_a996_244b_ccaf_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -129,
        mantissa: 0xbcf1_3343_e7d9_ec7f_2afd_1778_1bb3_afea_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -129,
        mantissa: 0xba0e_c3b6_33dd_8b0b_91dc_60b2_b059_a609_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -129,
        mantissa: 0xb730_7735_78cb_90b3_aa11_16c3_466b_eb6c_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -129,
        mantissa: 0xb456_41f4_e350_a0d4_e756_eba0_0bc3_3976_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -129,
        mantissa: 0xb180_1859_d562_49de_98ce_51ff_f994_79cb_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -129,
        mantissa: 0xaead_eefa_caf9_7d37_9dd6_e688_ebb1_3b01_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -129,
        mantissa: 0xabdf_ba9e_468f_d6f9_472e_a077_49ce_6bd1_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -129,
        mantissa: 0xa915_7039_c51e_be72_e164_c759_686a_2207_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -129,
        mantissa: 0xa64f_04f0_b961_df78_54f5_275c_2d15_c21e_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -129,
        mantissa: 0xa38c_6e13_8e20_d834_d698_298a_dddd_7f30_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -129,
        mantissa: 0xa0cd_a11e_af46_390e_6324_3827_3918_db7d_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -129,
        mantissa: 0x9e12_93b9_998c_1dad_3b03_5eae_273a_855c_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -129,
        mantissa: 0x9b5b_3bb5_f088_b768_5078_bbe3_d392_be24_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -129,
        mantissa: 0x98a7_8f0e_9ae7_1d87_64de_c347_8470_7838_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -129,
        mantissa: 0x95f7_83e6_e49a_9cfc_0250_04f3_ef06_3312_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -129,
        mantissa: 0x934b_1089_a6dc_93c2_df5b_b3b6_0554_e151_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -129,
        mantissa: 0x90a2_2b68_75c6_a1f8_8e91_aeba_609c_8876_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -129,
        mantissa: 0x8dfc_cb1a_d35c_a6ef_9947_bdb6_ddca_f59a_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -129,
        mantissa: 0x8b5a_e65d_67db_9acf_7ba5_1681_26a5_8b99_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -129,
        mantissa: 0x88bc_7411_3f23_def3_bc5a_0fe3_96f4_0f1c_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -129,
        mantissa: 0x8621_6b3b_0b17_188c_363c_eae8_8f72_0f1d_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -129,
        mantissa: 0x8389_c302_6ac3_139d_6add_a9d2_270f_a1f3_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -129,
        mantissa: 0x80f5_72b1_3634_87bc_edbd_0b5b_3479_d5f2_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -130,
        mantissa: 0xfcc8_e365_9d9b_cbf1_8a0c_df30_1431_b60b_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -130,
        mantissa: 0xf7ad_6f26_e7ff_2efc_9cd2_238f_75f9_69ad_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -130,
        mantissa: 0xf298_77ff_3880_9097_2b02_0fa1_820c_948d_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -130,
        mantissa: 0xed89_ed86_a44a_01ab_09d4_9f96_cb88_317a_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -130,
        mantissa: 0xe881_bf93_2af3_dac3_2524_848e_3443_e03f_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -130,
        mantissa: 0xe37f_de37_807b_84e3_5e9a_750b_6b68_781c_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -130,
        mantissa: 0xde84_39c1_dec5_687c_9d57_da94_5b5d_0aa6_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -130,
        mantissa: 0xd98e_c2ba_de71_e53e_d0a9_8f2a_d65b_ee96_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -130,
        mantissa: 0xd49f_69e4_56cf_1b7a_5f53_bd2e_406e_66e7_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -130,
        mantissa: 0xcfb6_2038_44b3_209b_18cb_02f3_3f79_c16b_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -130,
        mantissa: 0xcad2_d6e7_b80b_f915_cc50_7fb7_a3d0_bf69_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -130,
        mantissa: 0xc5f5_7f59_c7f4_6156_9a8b_6997_a402_bf30_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -130,
        mantissa: 0xc11e_0b2a_8d1e_0de1_da63_1e83_0fd3_08fe_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -130,
        mantissa: 0xbc4c_6c2a_2263_99f6_276e_bcfb_2016_a433_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -130,
        mantissa: 0xb780_945b_ab55_dcea_b4c7_bc3d_3275_0fd9_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -130,
        mantissa: 0xb2ba_75f4_6099_cf8f_243c_2e77_904a_fa76_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -130,
        mantissa: 0xadfa_035a_a1ed_8fdd_5497_67e4_1031_6d2b_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -130,
        mantissa: 0xa93f_2f25_0dac_67d5_9ad2_fb8d_4805_4add_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -130,
        mantissa: 0xa489_ec19_9dab_06f4_59fb_6cf0_ecb4_11b7_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -130,
        mantissa: 0x9fda_2d2c_c946_5c52_6b2b_9565_f535_5180_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -130,
        mantissa: 0x9b2f_e580_ac80_b182_011a_5b94_4aca_8705_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -130,
        mantissa: 0x968b_0864_3409_ceb9_d5c0_da50_6a08_8482_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -130,
        mantissa: 0x91eb_8952_4e10_0d28_bfd3_df5c_52d6_7e77_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -130,
        mantissa: 0x8d51_5bf1_1fb9_4f22_a071_3268_840c_bcbb_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -130,
        mantissa: 0x88bc_7411_3f23_def7_9c5a_0fe3_96f4_0f19_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -130,
        mantissa: 0x842c_c5ac_f1d0_344b_6fec_dfa8_19b9_6092_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -131,
        mantissa: 0xff44_89ce_deab_2ca6_e17b_d40d_8d92_91ec_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -131,
        mantissa: 0xf639_cc18_5088_fe62_5066_e87f_2c0f_733d_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -131,
        mantissa: 0xed39_3b1c_2235_1281_ff4e_2e66_0317_d55f_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -131,
        mantissa: 0xe442_c00d_e259_1b4c_e96a_b34c_e0bc_cd10_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -131,
        mantissa: 0xdb56_446d_6ad8_df09_2811_2e35_a60e_636f_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -131,
        mantissa: 0xd273_b205_8de1_bd4b_36bb_f837_b4d3_20c6_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -131,
        mantissa: 0xc99a_f2ea_ca4c_457b_eaf5_1f66_6928_44b2_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -131,
        mantissa: 0xc0cb_f17a_071f_80e9_396f_fdf7_6a14_7cc2_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -131,
        mantissa: 0xb806_9857_5607_07a7_0a67_7b4c_8bec_22e0_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -131,
        mantissa: 0xaf4a_d26c_bc8e_5bef_9e8b_8b88_a14f_f0c9_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -131,
        mantissa: 0xa698_8ae9_03f5_62f1_7e85_8f08_597b_3a68_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -131,
        mantissa: 0x9def_ad3e_8f73_2186_476d_3b5b_45f6_ca02_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -131,
        mantissa: 0x9550_2522_38bd_2468_658e_5a0b_811c_596d_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -131,
        mantissa: 0x8cb9_de8a_32ab_3694_97c9_8595_30a4_514c_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -131,
        mantissa: 0x842c_c5ac_f1d0_344c_1fec_dfa8_19b9_6094_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -132,
        mantissa: 0xf751_8e00_35c3_dd92_606d_8909_3278_a931_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -132,
        mantissa: 0xe65b_9e6e_ed96_5c4f_609f_5fe2_058d_5ff2_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -132,
        mantissa: 0xd577_9687_d887_e0ee_49dd_a170_56e4_5ebb_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -132,
        mantissa: 0xc4a5_50a4_fd9a_19bb_3e97_660a_23cc_5402_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -132,
        mantissa: 0xb3e4_a796_a5da_c213_07cc_a0bc_c06c_2f8e_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -132,
        mantissa: 0xa335_76a1_6f1f_4c79_1210_16bd_904d_c95a_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -132,
        mantissa: 0x9297_997c_68c1_f4e6_610d_b3d4_dd42_3bc9_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -132,
        mantissa: 0x820a_ec4f_3a22_2397_b9e3_aea6_c444_eef6_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -133,
        mantissa: 0xe31e_9760_a557_8c6d_f9eb_2f28_4f31_c35a_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -133,
        mantissa: 0xc249_2946_4655_f482_da5f_3cc0_b325_1da6_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -133,
        mantissa: 0xa195_492c_c066_0519_4a18_dff7_cdb4_ae33_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -133,
        mantissa: 0x8102_b2c4_9ac2_3a86_91d0_82dc_e3dd_cd08_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -134,
        mantissa: 0xc122_451c_4515_5150_b161_37f0_9a00_2b0e_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Neg,
        exponent: -134,
        mantissa: 0x8080_abac_46f3_89c4_662d_417c_ed00_79c9_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: 0,
        mantissa: 0x0000_0000_0000_0000_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: 0,
        mantissa: 0x0000_0000_0000_0000_0000_0000_0000_0000_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -136,
        mantissa: 0xff80_5515_885e_014e_435a_b4da_6a5b_b50f_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -135,
        mantissa: 0xff01_5358_833c_4762_bb48_1c8e_e141_6999_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -134,
        mantissa: 0xbee2_3afc_0853_b6a8_a897_82c2_0df3_50c2_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -134,
        mantissa: 0xfe05_4587_e01f_1e2b_f6d3_a69b_d5ea_b72f_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -133,
        mantissa: 0x9e75_221a_352b_a751_452b_7ea6_2f21_98ea_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -133,
        mantissa: 0xbdc8_d83e_ad88_d518_7faa_638b_5e00_ee90_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -133,
        mantissa: 0xdcfe_013d_7c8c_bfc5_632d_bac4_6f30_d009_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -133,
        mantissa: 0xfc14_d873_c198_0236_c7e0_9e3d_e453_f5fc_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -132,
        mantissa: 0x8d86_cc49_1ecb_fe03_f177_6453_b7e8_2558_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -132,
        mantissa: 0x9cf4_3dcf_f5ea_fd2f_2ad9_0155_c8a7_236a_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -132,
        mantissa: 0xac52_dd7e_4726_a456_a47a_963a_91bb_3018_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -132,
        mantissa: 0xbba2_c7b1_96e7_e224_e795_0f72_52c1_63cf_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -132,
        mantissa: 0xcae4_1876_471f_5bde_91d0_0a41_7e33_0f8e_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -132,
        mantissa: 0xda16_eb88_cb8d_f5fb_28a6_3ecf_b66e_94c0_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -132,
        mantissa: 0xe93b_5c56_d85a_9083_ce29_92bf_ea38_e76b_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -132,
        mantissa: 0xf851_8600_8b15_32f9_e64b_8b77_5997_8998_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -131,
        mantissa: 0x83ac_c1ac_c723_8978_5a53_33c4_5b7f_442e_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -131,
        mantissa: 0x8b29_b775_1bd7_073b_02e0_b9ee_992f_2372_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -131,
        mantissa: 0x929f_b178_50a0_b7be_5b4d_3807_6605_16a4_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -131,
        mantissa: 0x9a0e_bcb0_de8e_848e_2c1b_b082_689b_a814_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -131,
        mantissa: 0xa176_e5f5_3237_81d2_dcf9_3599_6c92_e8d4_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -131,
        mantissa: 0xa8d8_39f8_30c1_fb40_4c73_4351_7c8a_c264_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -131,
        mantissa: 0xb032_c549_ba86_1d83_774e_27bc_92ce_3373_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -131,
        mantissa: 0xb786_9457_2b5a_5cd3_24cd_cf68_cdb2_067c_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -131,
        mantissa: 0xbed3_b36b_d896_6419_7c06_44d7_d9ed_08b4_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -131,
        mantissa: 0xc61a_2eb1_8cd9_07a1_e5a1_532f_6d5a_1ac1_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -131,
        mantissa: 0xcd5a_1231_019d_66d7_761e_3e7b_171e_44b2_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -131,
        mantissa: 0xd493_69d2_56ab_1b1f_9e91_54e1_d526_3cda_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -131,
        mantissa: 0xdbc6_415d_876d_0839_3e33_c0c9_f882_4f54_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -131,
        mantissa: 0xe2f2_a47a_de3a_18a8_a0bf_7c0b_0d8b_b4ef_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -131,
        mantissa: 0xea18_9eb3_659a_eaeb_93b2_a3b2_1f44_8259_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -131,
        mantissa: 0xf138_3b71_5797_2f48_543f_ff0f_f4f0_aaf1_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -131,
        mantissa: 0xf851_8600_8b15_3302_5e4b_8b77_5997_8993_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -131,
        mantissa: 0xff64_898e_df55_d548_428c_cfc9_9271_dffa_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -130,
        mantissa: 0x8338_a896_52cb_714a_b247_eb86_498c_2ce7_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -130,
        mantissa: 0x86bb_f3e6_8472_cb2f_0b8b_d206_1574_7126_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -130,
        mantissa: 0x8a3c_2c23_3a15_6341_9027_c74f_e0e6_f64f_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -130,
        mantissa: 0x8db9_56a9_7b3d_0143_f023_472c_d739_f9e1_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -130,
        mantissa: 0x9133_78c8_52d6_5be6_977e_3013_d10f_7525_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -130,
        mantissa: 0x94aa_97c0_ffa9_1a5d_4ee3_880f_b7d3_4429_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -130,
        mantissa: 0x981e_b8c7_23fe_97f2_1f1c_134f_b702_d433_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -130,
        mantissa: 0x9b8f_e100_f47b_a1d8_04b6_2af1_89fc_ba0d_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -130,
        mantissa: 0x9efe_1587_6631_4e4f_4d71_827e_fe89_2fc8_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -130,
        mantissa: 0xa269_5b66_5be8_f338_4eca_87c3_f0f0_6211_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -130,
        mantissa: 0xa5d1_b79c_d2af_2aca_8837_986c_eabf_bed6_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -130,
        mantissa: 0xa937_2f1d_0da1_bd10_580e_b71e_58cd_36e5_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -130,
        mantissa: 0xac99_c6cc_c104_2e94_3dd5_5752_8315_838d_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -130,
        mantissa: 0xaff9_8385_3c9e_9e40_5f10_5039_091d_d7f5_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -130,
        mantissa: 0xb356_6a13_956a_86f4_471b_1e15_74d9_fd55_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -130,
        mantissa: 0xb6b0_7f38_ce90_e463_7bb2_e265_d0de_37e1_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -130,
        mantissa: 0xba07_c7aa_01bd_2648_43f9_d57b_324b_d05f_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -130,
        mantissa: 0xbd5c_4810_86c8_48db_bb59_6b50_3040_3242_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -130,
        mantissa: 0xc0ae_050a_1abf_56ad_2f7f_8c5f_a9c5_0d76_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -130,
        mantissa: 0xc3fd_0329_0648_847d_3048_0bee_4cbb_d698_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -130,
        mantissa: 0xc749_46f4_436a_054e_f4f5_cb53_1201_c0d3_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -130,
        mantissa: 0xca92_d4e7_a2b5_a3ad_c983_a9c5_c4b3_b135_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -130,
        mantissa: 0xcdd9_b173_efdc_1aaa_8863_e007_c184_a1e7_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -130,
        mantissa: 0xd11d_e0ff_15ab_18c6_d88d_83d4_cc61_3f21_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -130,
        mantissa: 0xd45f_67e4_4178_c612_5486_e73c_6151_58b4_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -130,
        mantissa: 0xd79e_4a74_05ff_96c3_1300_c9be_67ae_5da0_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -130,
        mantissa: 0xdada_8cf4_7dad_236d_dffb_833c_3409_ee7e_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -130,
        mantissa: 0xde14_33a1_6c66_b14c_de74_4870_f54f_0f18_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -130,
        mantissa: 0xe14b_42ac_60c6_0512_4e38_eb80_92a0_1f06_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -130,
        mantissa: 0xe47f_be3c_d4d1_0d5b_2ec0_f797_fdcd_125c_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -130,
        mantissa: 0xe7b1_aa70_4e2e_e240_b40f_aab6_d2ad_0841_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -130,
        mantissa: 0xeae1_0b5a_7ddc_8ad8_806b_2fc9_a803_8790_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -130,
        mantissa: 0xee0d_e505_5f63_eb01_90a3_3316_df83_ba5a_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -130,
        mantissa: 0xf138_3b71_5797_2f4a_b43f_ff0f_f4f0_aaf1_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -130,
        mantissa: 0xf460_1295_52d2_ff41_e62e_3201_bb2b_bdce_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -130,
        mantissa: 0xf785_6e5e_e2c9_b28a_76f2_a1b8_4190_a7dc_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -130,
        mantissa: 0xfaa8_52b2_5bd9_b833_a6db_fa03_186e_0666_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -130,
        mantissa: 0xfdc8_c36a_f1f1_5468_0a33_61bc_a696_504a_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -129,
        mantissa: 0x8073_622d_6a80_e631_e897_0090_1531_6073_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -129,
        mantissa: 0x8201_2ca5_a682_06d5_8fde_85af_dd2b_c88a_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -129,
        mantissa: 0x838d_c2fe_6ac8_68e7_1a3f_cbde_f401_00cb_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -129,
        mantissa: 0x8519_2713_9c87_1af8_67bd_00c3_8061_c51f_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -129,
        mantissa: 0x86a3_5abc_d5ba_5901_5481_c3cb_d925_ccd2_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -129,
        mantissa: 0x882c_5fcd_7256_a8c1_3905_5a65_98e7_c29e_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -129,
        mantissa: 0x89b4_3814_9d45_82f5_3453_1dba_493e_b5a6_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -129,
        mantissa: 0x8b3a_e55d_5d30_701a_c63e_ab88_3717_0480_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -129,
        mantissa: 0x8cc0_696e_a11b_7b36_9436_1c9a_28d3_8a6a_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -129,
        mantissa: 0x8e44_c60b_4ccf_d7dc_1473_aa01_c777_8679_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -129,
        mantissa: 0x8fc7_fcf2_4517_946a_380c_be76_9f2c_6793_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -129,
        mantissa: 0x914a_0fde_7bcb_2d0e_c429_ed3a_ea19_7a60_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -129,
        mantissa: 0x92cb_0086_fbb1_cf75_a29d_47c5_0b11_82d0_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -129,
        mantissa: 0x944a_d09e_f435_1af1_a498_27e0_81cb_16ba_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -129,
        mantissa: 0x95c9_81d5_c4e9_24ea_4540_4f5a_a577_d6b4_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -129,
        mantissa: 0x9747_15d7_08e9_84dd_6648_d428_40d9_e6fb_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -129,
        mantissa: 0x98c3_8e4a_a20c_27d2_8467_67ec_990d_7333_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -129,
        mantissa: 0x9a3e_ecd4_c3ea_a6ae_db3a_7f6e_6087_b947_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -129,
        mantissa: 0x9bb9_3315_fec2_d790_7f58_9fba_0865_790f_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -129,
        mantissa: 0x9d32_62ab_4a2f_4e37_a1ae_6ba0_6846_fae0_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -129,
        mantissa: 0x9eaa_7d2e_0fb8_7c35_ff47_2bc6_ce64_8a7d_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -129,
        mantissa: 0xa021_8434_353f_1de4_d493_efa6_3253_0acc_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -129,
        mantissa: 0xa197_7950_2740_9daa_1dd1_d4a6_df96_0357_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -129,
        mantissa: 0xa30c_5e10_e2f6_13e4_9bd9_bd99_e39a_20b3_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -129,
        mantissa: 0xa480_3402_004e_865c_31cb_e0e8_8241_16cd_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -129,
        mantissa: 0xa5f2_fcab_bbc5_06d8_68ca_4fb7_ec32_3d74_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -129,
        mantissa: 0xa764_b993_0013_4d79_0d04_d104_7430_1862_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -129,
        mantissa: 0xa8d5_6c39_6fc1_684c_01eb_067d_578c_4756_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -129,
        mantissa: 0xaa45_161d_6e93_167b_9b08_1cf7_2249_f5b2_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -129,
        mantissa: 0xabb3_b8ba_2ad3_62a1_1db6_506c_c17a_01f5_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -129,
        mantissa: 0xad21_5587_a67f_0cdf_e890_422c_b86b_7cb1_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -129,
        mantissa: 0xae8d_edfa_c04e_5282_ac70_7b8f_fc22_b3e8_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -129,
        mantissa: 0xaff9_8385_3c9e_9e3f_c510_5039_091d_d7f8_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -129,
        mantissa: 0xb164_1795_ce3c_a978_faf9_1530_0e51_7393_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -129,
        mantissa: 0xb2cd_ab98_1f0f_940b_c857_c77d_c1df_600f_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -129,
        mantissa: 0xb436_40f4_d8a5_761f_f5f0_80a7_1c34_b25d_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -129,
        mantissa: 0xb59d_d911_aca1_ec48_1d26_64cf_09a0_c1bf_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -129,
        mantissa: 0xb704_7551_5d0f_1c5e_4c98_c6b8_be17_818d_u128,
    },
    DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -129,
        mantissa: 0xb86a_1713_c491_aeaa_d37e_e287_2a6f_1cd6_u128,
    },
];
