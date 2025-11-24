// Adapted from https://github.com/Alexhuszagh/rust-lexical.

use crate::lexical::float::ExtendedFloat;
use crate::lexical::rounding::round_nearest_tie_even;
use std::{f32, f64};

// NORMALIZE

fn check_normalize(mant: u64, exp: i32, shift: u32, r_mant: u64, r_exp: i32) {
    let mut x = ExtendedFloat { mant, exp };
    assert_eq!(x.normalize(), shift);
    assert_eq!(
        x,
        ExtendedFloat {
            mant: r_mant,
            exp: r_exp
        }
    );
}

#[test]
fn normalize_test() {
    // F32
    // 0
    check_normalize(0, 0, 0, 0, 0);

    // min value
    check_normalize(1, -149, 63, 9223372036854775808, -212);

    // 1.0e-40
    check_normalize(71362, -149, 47, 10043308644012916736, -196);

    // 1.0e-20
    check_normalize(12379400, -90, 40, 13611294244890214400, -130);

    // 1.0
    check_normalize(8388608, -23, 40, 9223372036854775808, -63);

    // 1e20
    check_normalize(11368684, 43, 40, 12500000250510966784, 3);

    // max value
    check_normalize(16777213, 104, 40, 18446740775174668288, 64);

    // F64

    // min value
    check_normalize(1, -1074, 63, 9223372036854775808, -1137);

    // 1.0e-250
    check_normalize(6448907850777164, -883, 11, 13207363278391631872, -894);

    // 1.0e-150
    check_normalize(7371020360979573, -551, 11, 15095849699286165504, -562);

    // 1.0e-45
    check_normalize(6427752177035961, -202, 11, 13164036458569648128, -213);

    // 1.0e-40
    check_normalize(4903985730770844, -185, 11, 10043362776618688512, -196);

    // 1.0e-20
    check_normalize(6646139978924579, -119, 11, 13611294676837537792, -130);

    // 1.0
    check_normalize(4503599627370496, -52, 11, 9223372036854775808, -63);

    // 1e20
    check_normalize(6103515625000000, 14, 11, 12500000000000000000, 3);

    // 1e40
    check_normalize(8271806125530277, 80, 11, 16940658945086007296, 69);

    // 1e150
    check_normalize(5503284107318959, 446, 11, 11270725851789228032, 435);

    // 1e250
    check_normalize(6290184345309700, 778, 11, 12882297539194265600, 767);

    // max value
    check_normalize(9007199254740991, 971, 11, 18446744073709549568, 960);
}

// ROUND

fn check_round_to_f32(mant: u64, exp: i32, r_mant: u64, r_exp: i32) {
    let mut x = ExtendedFloat { mant, exp };
    x.round_to_native::<f32, _>(round_nearest_tie_even);
    assert_eq!(
        x,
        ExtendedFloat {
            mant: r_mant,
            exp: r_exp
        }
    );
}

#[test]
fn round_to_f32_test() {
    // This is lossy, so some of these values are **slightly** rounded.

    // underflow
    check_round_to_f32(9223372036854775808, -213, 0, -149);

    // min value
    check_round_to_f32(9223372036854775808, -212, 1, -149);

    // 1.0e-40
    check_round_to_f32(10043308644012916736, -196, 71362, -149);

    // 1.0e-20
    check_round_to_f32(13611294244890214400, -130, 12379400, -90);

    // 1.0
    check_round_to_f32(9223372036854775808, -63, 8388608, -23);

    // 1e20
    check_round_to_f32(12500000250510966784, 3, 11368684, 43);

    // max value
    check_round_to_f32(18446740775174668288, 64, 16777213, 104);

    // overflow
    check_round_to_f32(18446740775174668288, 65, 16777213, 105);
}

fn check_round_to_f64(mant: u64, exp: i32, r_mant: u64, r_exp: i32) {
    let mut x = ExtendedFloat { mant, exp };
    x.round_to_native::<f64, _>(round_nearest_tie_even);
    assert_eq!(
        x,
        ExtendedFloat {
            mant: r_mant,
            exp: r_exp
        }
    );
}

#[test]
fn round_to_f64_test() {
    // This is lossy, so some of these values are **slightly** rounded.

    // underflow
    check_round_to_f64(9223372036854775808, -1138, 0, -1074);

    // min value
    check_round_to_f64(9223372036854775808, -1137, 1, -1074);

    // 1.0e-250
    check_round_to_f64(15095849699286165504, -562, 7371020360979573, -551);

    // 1.0e-150
    check_round_to_f64(15095849699286165504, -562, 7371020360979573, -551);

    // 1.0e-45
    check_round_to_f64(13164036458569648128, -213, 6427752177035961, -202);

    // 1.0e-40
    check_round_to_f64(10043362776618688512, -196, 4903985730770844, -185);

    // 1.0e-20
    check_round_to_f64(13611294676837537792, -130, 6646139978924579, -119);

    // 1.0
    check_round_to_f64(9223372036854775808, -63, 4503599627370496, -52);

    // 1e20
    check_round_to_f64(12500000000000000000, 3, 6103515625000000, 14);

    // 1e40
    check_round_to_f64(16940658945086007296, 69, 8271806125530277, 80);

    // 1e150
    check_round_to_f64(11270725851789228032, 435, 5503284107318959, 446);

    // 1e250
    check_round_to_f64(12882297539194265600, 767, 6290184345309700, 778);

    // max value
    check_round_to_f64(18446744073709549568, 960, 9007199254740991, 971);

    // Bug fixes
    // 1.2345e-308
    check_round_to_f64(10234494226754558294, -1086, 2498655817078750, -1074);
}

fn assert_normalized_eq(mut x: ExtendedFloat, mut y: ExtendedFloat) {
    x.normalize();
    y.normalize();
    assert_eq!(x, y);
}

#[test]
fn from_float() {
    let values: [f32; 26] = [
        1e-40, 2e-40, 1e-35, 2e-35, 1e-30, 2e-30, 1e-25, 2e-25, 1e-20, 2e-20, 1e-15, 2e-15, 1e-10,
        2e-10, 1e-5, 2e-5, 1.0, 2.0, 1e5, 2e5, 1e10, 2e10, 1e15, 2e15, 1e20, 2e20,
    ];
    for value in &values {
        assert_normalized_eq(
            ExtendedFloat::from_float(*value),
            ExtendedFloat::from_float(*value as f64),
        );
    }
}

// TO

// Sample of interesting numbers to check during standard test builds.
const INTEGERS: [u64; 32] = [
    0,                    // 0x0
    1,                    // 0x1
    7,                    // 0x7
    15,                   // 0xF
    112,                  // 0x70
    119,                  // 0x77
    127,                  // 0x7F
    240,                  // 0xF0
    247,                  // 0xF7
    255,                  // 0xFF
    2032,                 // 0x7F0
    2039,                 // 0x7F7
    2047,                 // 0x7FF
    4080,                 // 0xFF0
    4087,                 // 0xFF7
    4095,                 // 0xFFF
    65520,                // 0xFFF0
    65527,                // 0xFFF7
    65535,                // 0xFFFF
    1048560,              // 0xFFFF0
    1048567,              // 0xFFFF7
    1048575,              // 0xFFFFF
    16777200,             // 0xFFFFF0
    16777207,             // 0xFFFFF7
    16777215,             // 0xFFFFFF
    268435440,            // 0xFFFFFF0
    268435447,            // 0xFFFFFF7
    268435455,            // 0xFFFFFFF
    4294967280,           // 0xFFFFFFF0
    4294967287,           // 0xFFFFFFF7
    4294967295,           // 0xFFFFFFFF
    18446744073709551615, // 0xFFFFFFFFFFFFFFFF
];

#[test]
fn to_f32_test() {
    // underflow
    let x = ExtendedFloat {
        mant: 9223372036854775808,
        exp: -213,
    };
    assert_eq!(x.into_float::<f32>(), 0.0);

    // min value
    let x = ExtendedFloat {
        mant: 9223372036854775808,
        exp: -212,
    };
    assert_eq!(x.into_float::<f32>(), 1e-45);

    // 1.0e-40
    let x = ExtendedFloat {
        mant: 10043308644012916736,
        exp: -196,
    };
    assert_eq!(x.into_float::<f32>(), 1e-40);

    // 1.0e-20
    let x = ExtendedFloat {
        mant: 13611294244890214400,
        exp: -130,
    };
    assert_eq!(x.into_float::<f32>(), 1e-20);

    // 1.0
    let x = ExtendedFloat {
        mant: 9223372036854775808,
        exp: -63,
    };
    assert_eq!(x.into_float::<f32>(), 1.0);

    // 1e20
    let x = ExtendedFloat {
        mant: 12500000250510966784,
        exp: 3,
    };
    assert_eq!(x.into_float::<f32>(), 1e20);

    // max value
    let x = ExtendedFloat {
        mant: 18446740775174668288,
        exp: 64,
    };
    assert_eq!(x.into_float::<f32>(), 3.402823e38);

    // almost max, high exp
    let x = ExtendedFloat {
        mant: 1048575,
        exp: 108,
    };
    assert_eq!(x.into_float::<f32>(), 3.4028204e38);

    // max value + 1
    let x = ExtendedFloat {
        mant: 16777216,
        exp: 104,
    };
    assert_eq!(x.into_float::<f32>(), f32::INFINITY);

    // max value + 1
    let x = ExtendedFloat {
        mant: 1048576,
        exp: 108,
    };
    assert_eq!(x.into_float::<f32>(), f32::INFINITY);

    // 1e40
    let x = ExtendedFloat {
        mant: 16940658945086007296,
        exp: 69,
    };
    assert_eq!(x.into_float::<f32>(), f32::INFINITY);

    // Integers.
    for int in &INTEGERS {
        let fp = ExtendedFloat { mant: *int, exp: 0 };
        assert_eq!(fp.into_float::<f32>(), *int as f32, "{:?} as f32", *int);
    }
}

#[test]
fn to_f64_test() {
    // underflow
    let x = ExtendedFloat {
        mant: 9223372036854775808,
        exp: -1138,
    };
    assert_eq!(x.into_float::<f64>(), 0.0);

    // min value
    let x = ExtendedFloat {
        mant: 9223372036854775808,
        exp: -1137,
    };
    assert_eq!(x.into_float::<f64>(), 5e-324);

    // 1.0e-250
    let x = ExtendedFloat {
        mant: 13207363278391631872,
        exp: -894,
    };
    assert_eq!(x.into_float::<f64>(), 1e-250);

    // 1.0e-150
    let x = ExtendedFloat {
        mant: 15095849699286165504,
        exp: -562,
    };
    assert_eq!(x.into_float::<f64>(), 1e-150);

    // 1.0e-45
    let x = ExtendedFloat {
        mant: 13164036458569648128,
        exp: -213,
    };
    assert_eq!(x.into_float::<f64>(), 1e-45);

    // 1.0e-40
    let x = ExtendedFloat {
        mant: 10043362776618688512,
        exp: -196,
    };
    assert_eq!(x.into_float::<f64>(), 1e-40);

    // 1.0e-20
    let x = ExtendedFloat {
        mant: 13611294676837537792,
        exp: -130,
    };
    assert_eq!(x.into_float::<f64>(), 1e-20);

    // 1.0
    let x = ExtendedFloat {
        mant: 9223372036854775808,
        exp: -63,
    };
    assert_eq!(x.into_float::<f64>(), 1.0);

    // 1e20
    let x = ExtendedFloat {
        mant: 12500000000000000000,
        exp: 3,
    };
    assert_eq!(x.into_float::<f64>(), 1e20);

    // 1e40
    let x = ExtendedFloat {
        mant: 16940658945086007296,
        exp: 69,
    };
    assert_eq!(x.into_float::<f64>(), 1e40);

    // 1e150
    let x = ExtendedFloat {
        mant: 11270725851789228032,
        exp: 435,
    };
    assert_eq!(x.into_float::<f64>(), 1e150);

    // 1e250
    let x = ExtendedFloat {
        mant: 12882297539194265600,
        exp: 767,
    };
    assert_eq!(x.into_float::<f64>(), 1e250);

    // max value
    let x = ExtendedFloat {
        mant: 9007199254740991,
        exp: 971,
    };
    assert_eq!(x.into_float::<f64>(), 1.7976931348623157e308);

    // max value
    let x = ExtendedFloat {
        mant: 18446744073709549568,
        exp: 960,
    };
    assert_eq!(x.into_float::<f64>(), 1.7976931348623157e308);

    // overflow
    let x = ExtendedFloat {
        mant: 9007199254740992,
        exp: 971,
    };
    assert_eq!(x.into_float::<f64>(), f64::INFINITY);

    // overflow
    let x = ExtendedFloat {
        mant: 18446744073709549568,
        exp: 961,
    };
    assert_eq!(x.into_float::<f64>(), f64::INFINITY);

    // Underflow
    // Adapted from failures in strtod.
    let x = ExtendedFloat {
        exp: -1139,
        mant: 18446744073709550712,
    };
    assert_eq!(x.into_float::<f64>(), 0.0);

    let x = ExtendedFloat {
        exp: -1139,
        mant: 18446744073709551460,
    };
    assert_eq!(x.into_float::<f64>(), 0.0);

    let x = ExtendedFloat {
        exp: -1138,
        mant: 9223372036854776103,
    };
    assert_eq!(x.into_float::<f64>(), 5e-324);

    // Integers.
    for int in &INTEGERS {
        let fp = ExtendedFloat { mant: *int, exp: 0 };
        assert_eq!(fp.into_float::<f64>(), *int as f64, "{:?} as f64", *int);
    }
}

// OPERATIONS

fn check_mul(a: ExtendedFloat, b: ExtendedFloat, c: ExtendedFloat) {
    let r = a.mul(&b);
    assert_eq!(r, c);
}

#[test]
fn mul_test() {
    // Normalized (64-bit mantissa)
    let a = ExtendedFloat {
        mant: 13164036458569648128,
        exp: -213,
    };
    let b = ExtendedFloat {
        mant: 9223372036854775808,
        exp: -62,
    };
    let c = ExtendedFloat {
        mant: 6582018229284824064,
        exp: -211,
    };
    check_mul(a, b, c);

    // Check with integers
    // 64-bit mantissa
    let mut a = ExtendedFloat { mant: 10, exp: 0 };
    let mut b = ExtendedFloat { mant: 10, exp: 0 };
    a.normalize();
    b.normalize();
    assert_eq!(a.mul(&b).into_float::<f64>(), 100.0);

    // Check both values need high bits set.
    let a = ExtendedFloat {
        mant: 1 << 32,
        exp: -31,
    };
    let b = ExtendedFloat {
        mant: 1 << 32,
        exp: -31,
    };
    assert_eq!(a.mul(&b).into_float::<f64>(), 4.0);

    // Check both values need high bits set.
    let a = ExtendedFloat {
        mant: 10 << 31,
        exp: -31,
    };
    let b = ExtendedFloat {
        mant: 10 << 31,
        exp: -31,
    };
    assert_eq!(a.mul(&b).into_float::<f64>(), 100.0);
}

fn check_imul(mut a: ExtendedFloat, b: ExtendedFloat, c: ExtendedFloat) {
    a.imul(&b);
    assert_eq!(a, c);
}

#[test]
fn imul_test() {
    // Normalized (64-bit mantissa)
    let a = ExtendedFloat {
        mant: 13164036458569648128,
        exp: -213,
    };
    let b = ExtendedFloat {
        mant: 9223372036854775808,
        exp: -62,
    };
    let c = ExtendedFloat {
        mant: 6582018229284824064,
        exp: -211,
    };
    check_imul(a, b, c);

    // Check with integers
    // 64-bit mantissa
    let mut a = ExtendedFloat { mant: 10, exp: 0 };
    let mut b = ExtendedFloat { mant: 10, exp: 0 };
    a.normalize();
    b.normalize();
    a.imul(&b);
    assert_eq!(a.into_float::<f64>(), 100.0);

    // Check both values need high bits set.
    let mut a = ExtendedFloat {
        mant: 1 << 32,
        exp: -31,
    };
    let b = ExtendedFloat {
        mant: 1 << 32,
        exp: -31,
    };
    a.imul(&b);
    assert_eq!(a.into_float::<f64>(), 4.0);

    // Check both values need high bits set.
    let mut a = ExtendedFloat {
        mant: 10 << 31,
        exp: -31,
    };
    let b = ExtendedFloat {
        mant: 10 << 31,
        exp: -31,
    };
    a.imul(&b);
    assert_eq!(a.into_float::<f64>(), 100.0);
}
