// Translated from C to Rust. The original C code can be found at
// https://github.com/ulfjack/ryu and carries the following license:
//
// Copyright 2018 Ulf Adams
//
// The contents of this file may be used under the terms of the Apache License,
// Version 2.0.
//
//    (See accompanying file LICENSE-Apache or copy at
//     http://www.apache.org/licenses/LICENSE-2.0)
//
// Alternatively, the contents of this file may be used under the terms of
// the Boost Software License, Version 1.0.
//    (See accompanying file LICENSE-Boost or copy at
//     https://www.boost.org/LICENSE_1_0.txt)
//
// Unless required by applicable law or agreed to in writing, this software
// is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied.

use crate::common::pow5bits;

pub static DOUBLE_POW5_INV_SPLIT2: [(u64, u64); 15] = [
    (1, 2305843009213693952),
    (5955668970331000884, 1784059615882449851),
    (8982663654677661702, 1380349269358112757),
    (7286864317269821294, 2135987035920910082),
    (7005857020398200553, 1652639921975621497),
    (17965325103354776697, 1278668206209430417),
    (8928596168509315048, 1978643211784836272),
    (10075671573058298858, 1530901034580419511),
    (597001226353042382, 1184477304306571148),
    (1527430471115325346, 1832889850782397517),
    (12533209867169019542, 1418129833677084982),
    (5577825024675947042, 2194449627517475473),
    (11006974540203867551, 1697873161311732311),
    (10313493231639821582, 1313665730009899186),
    (12701016819766672773, 2032799256770390445),
];

pub static POW5_INV_OFFSETS: [u32; 19] = [
    0x54544554, 0x04055545, 0x10041000, 0x00400414, 0x40010000, 0x41155555, 0x00000454, 0x00010044,
    0x40000000, 0x44000041, 0x50454450, 0x55550054, 0x51655554, 0x40004000, 0x01000001, 0x00010500,
    0x51515411, 0x05555554, 0x00000000,
];

pub static DOUBLE_POW5_SPLIT2: [(u64, u64); 13] = [
    (0, 1152921504606846976),
    (0, 1490116119384765625),
    (1032610780636961552, 1925929944387235853),
    (7910200175544436838, 1244603055572228341),
    (16941905809032713930, 1608611746708759036),
    (13024893955298202172, 2079081953128979843),
    (6607496772837067824, 1343575221513417750),
    (17332926989895652603, 1736530273035216783),
    (13037379183483547984, 2244412773384604712),
    (1605989338741628675, 1450417759929778918),
    (9630225068416591280, 1874621017369538693),
    (665883850346957067, 1211445438634777304),
    (14931890668723713708, 1565756531257009982),
];

pub static POW5_OFFSETS: [u32; 21] = [
    0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x40000000, 0x59695995, 0x55545555, 0x56555515,
    0x41150504, 0x40555410, 0x44555145, 0x44504540, 0x45555550, 0x40004000, 0x96440440, 0x55565565,
    0x54454045, 0x40154151, 0x55559155, 0x51405555, 0x00000105,
];

pub static DOUBLE_POW5_TABLE: [u64; 26] = [
    1,
    5,
    25,
    125,
    625,
    3125,
    15625,
    78125,
    390625,
    1953125,
    9765625,
    48828125,
    244140625,
    1220703125,
    6103515625,
    30517578125,
    152587890625,
    762939453125,
    3814697265625,
    19073486328125,
    95367431640625,
    476837158203125,
    2384185791015625,
    11920928955078125,
    59604644775390625,
    298023223876953125,
];

// Computes 5^i in the form required by Ryū.
#[cfg_attr(feature = "no-panic", inline)]
pub unsafe fn compute_pow5(i: u32) -> (u64, u64) {
    let base = i / DOUBLE_POW5_TABLE.len() as u32;
    let base2 = base * DOUBLE_POW5_TABLE.len() as u32;
    let offset = i - base2;
    debug_assert!(base < DOUBLE_POW5_SPLIT2.len() as u32);
    let mul = *DOUBLE_POW5_SPLIT2.get_unchecked(base as usize);
    if offset == 0 {
        return mul;
    }
    debug_assert!(offset < DOUBLE_POW5_TABLE.len() as u32);
    let m = *DOUBLE_POW5_TABLE.get_unchecked(offset as usize);
    let b0 = m as u128 * mul.0 as u128;
    let b2 = m as u128 * mul.1 as u128;
    let delta = pow5bits(i as i32) - pow5bits(base2 as i32);
    debug_assert!(i / 16 < POW5_OFFSETS.len() as u32);
    let shifted_sum = (b0 >> delta)
        + (b2 << (64 - delta))
        + ((*POW5_OFFSETS.get_unchecked((i / 16) as usize) >> ((i % 16) << 1)) & 3) as u128;
    (shifted_sum as u64, (shifted_sum >> 64) as u64)
}

// Computes 5^-i in the form required by Ryū.
#[cfg_attr(feature = "no-panic", inline)]
pub unsafe fn compute_inv_pow5(i: u32) -> (u64, u64) {
    let base = (i + DOUBLE_POW5_TABLE.len() as u32 - 1) / DOUBLE_POW5_TABLE.len() as u32;
    let base2 = base * DOUBLE_POW5_TABLE.len() as u32;
    let offset = base2 - i;
    debug_assert!(base < DOUBLE_POW5_INV_SPLIT2.len() as u32);
    let mul = *DOUBLE_POW5_INV_SPLIT2.get_unchecked(base as usize); // 1/5^base2
    if offset == 0 {
        return mul;
    }
    debug_assert!(offset < DOUBLE_POW5_TABLE.len() as u32);
    let m = *DOUBLE_POW5_TABLE.get_unchecked(offset as usize); // 5^offset
    let b0 = m as u128 * (mul.0 - 1) as u128;
    let b2 = m as u128 * mul.1 as u128; // 1/5^base2 * 5^offset = 1/5^(base2-offset) = 1/5^i
    let delta = pow5bits(base2 as i32) - pow5bits(i as i32);
    debug_assert!(base < POW5_INV_OFFSETS.len() as u32);
    let shifted_sum = ((b0 >> delta) + (b2 << (64 - delta)))
        + 1
        + ((*POW5_INV_OFFSETS.get_unchecked((i / 16) as usize) >> ((i % 16) << 1)) & 3) as u128;
    (shifted_sum as u64, (shifted_sum >> 64) as u64)
}
