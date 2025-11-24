// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

//! Example data useful for testing ZeroVec.

// This module is included directly in tests and can trigger the dead_code
// warning since not all samples are used in each test
#![allow(dead_code)]

#[repr(align(8))]
struct Aligned<T>(pub T);

// This is aligned so that we can test unaligned behavior at odd offsets
const ALIGNED_TEST_BUFFER_LE: Aligned<[u8; 80]> = Aligned([
    0x00, 0x01, 0x02, 0x00, 0x04, 0x05, 0x06, 0x00, 0x08, 0x09, 0x0a, 0x00, 0x0c, 0x0d, 0x0e, 0x00,
    0x10, 0x11, 0x12, 0x00, 0x14, 0x15, 0x16, 0x00, 0x18, 0x19, 0x1a, 0x00, 0x1c, 0x1d, 0x1e, 0x00,
    0x20, 0x21, 0x22, 0x00, 0x24, 0x25, 0x26, 0x00, 0x28, 0x29, 0x2a, 0x00, 0x2c, 0x2d, 0x2e, 0x00,
    0x30, 0x31, 0x32, 0x00, 0x34, 0x35, 0x36, 0x00, 0x38, 0x39, 0x3a, 0x00, 0x3c, 0x3d, 0x3e, 0x00,
    0x40, 0x41, 0x42, 0x00, 0x44, 0x45, 0x46, 0x00, 0x48, 0x49, 0x4a, 0x00, 0x4c, 0x4d, 0x4e, 0x00,
]);

/// An example byte array intended to be used in `ZeroVec<u32>`.
pub const TEST_BUFFER_LE: &[u8] = &ALIGNED_TEST_BUFFER_LE.0;

/// u32 numbers corresponding to the above byte array.
pub const TEST_SLICE: &[u32] = &[
    0x020100, 0x060504, 0x0a0908, 0x0e0d0c, 0x121110, 0x161514, 0x1a1918, 0x1e1d1c, 0x222120,
    0x262524, 0x2a2928, 0x2e2d2c, 0x323130, 0x363534, 0x3a3938, 0x3e3d3c, 0x424140, 0x464544,
    0x4a4948, 0x4e4d4c,
];

/// The sum of the numbers in TEST_SLICE.
pub const TEST_SUM: u32 = 52629240;

/// Representation of TEST_SLICE in JSON.
pub const JSON_STR: &str = "[131328,394500,657672,920844,1184016,1447188,1710360,1973532,2236704,2499876,2763048,3026220,3289392,3552564,3815736,4078908,4342080,4605252,4868424,5131596]";

/// Representation of TEST_SLICE in Bincode.
pub const BINCODE_BUF: &[u8] = &[
    80, 0, 0, 0, 0, 0, 0, 0, 0, 1, 2, 0, 4, 5, 6, 0, 8, 9, 10, 0, 12, 13, 14, 0, 16, 17, 18, 0, 20,
    21, 22, 0, 24, 25, 26, 0, 28, 29, 30, 0, 32, 33, 34, 0, 36, 37, 38, 0, 40, 41, 42, 0, 44, 45,
    46, 0, 48, 49, 50, 0, 52, 53, 54, 0, 56, 57, 58, 0, 60, 61, 62, 0, 64, 65, 66, 0, 68, 69, 70,
    0, 72, 73, 74, 0, 76, 77, 78, 0,
];

/// Representation of a VarZeroVec<str> with contents ["w", "ω", "文", "𑄃"]
pub const TEST_VARZEROSLICE_BYTES: &[u8] = &[
    4, 0, 0, 0, 0, 0, 1, 0, 3, 0, 6, 0, 119, 207, 137, 230, 150, 135, 240, 145, 132, 131,
];

#[test]
fn validate() {
    use crate::{VarZeroVec, ZeroVec};

    assert_eq!(
        ZeroVec::<u32>::parse_bytes(TEST_BUFFER_LE).unwrap(),
        ZeroVec::alloc_from_slice(TEST_SLICE)
    );

    assert_eq!(TEST_SLICE.iter().sum::<u32>(), TEST_SUM);

    assert_eq!(
        serde_json::from_str::<ZeroVec::<u32>>(JSON_STR).unwrap(),
        ZeroVec::alloc_from_slice(TEST_SLICE)
    );

    assert_eq!(
        bincode::deserialize::<ZeroVec::<u32>>(BINCODE_BUF).unwrap(),
        ZeroVec::alloc_from_slice(TEST_SLICE)
    );

    VarZeroVec::<str>::parse_bytes(TEST_VARZEROSLICE_BYTES).unwrap();
}
