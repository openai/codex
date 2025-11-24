// Adapted from https://github.com/Alexhuszagh/rust-lexical.

use crate::lexical::float::ExtendedFloat;
use crate::lexical::num::Float;
use crate::lexical::rounding::*;

// MASKS

#[test]
fn lower_n_mask_test() {
    assert_eq!(lower_n_mask(0u64), 0b0);
    assert_eq!(lower_n_mask(1u64), 0b1);
    assert_eq!(lower_n_mask(2u64), 0b11);
    assert_eq!(lower_n_mask(10u64), 0b1111111111);
    assert_eq!(lower_n_mask(32u64), 0b11111111111111111111111111111111);
}

#[test]
fn lower_n_halfway_test() {
    assert_eq!(lower_n_halfway(0u64), 0b0);
    assert_eq!(lower_n_halfway(1u64), 0b1);
    assert_eq!(lower_n_halfway(2u64), 0b10);
    assert_eq!(lower_n_halfway(10u64), 0b1000000000);
    assert_eq!(lower_n_halfway(32u64), 0b10000000000000000000000000000000);
}

#[test]
fn nth_bit_test() {
    assert_eq!(nth_bit(0u64), 0b1);
    assert_eq!(nth_bit(1u64), 0b10);
    assert_eq!(nth_bit(2u64), 0b100);
    assert_eq!(nth_bit(10u64), 0b10000000000);
    assert_eq!(nth_bit(31u64), 0b10000000000000000000000000000000);
}

#[test]
fn internal_n_mask_test() {
    assert_eq!(internal_n_mask(1u64, 0u64), 0b0);
    assert_eq!(internal_n_mask(1u64, 1u64), 0b1);
    assert_eq!(internal_n_mask(2u64, 1u64), 0b10);
    assert_eq!(internal_n_mask(4u64, 2u64), 0b1100);
    assert_eq!(internal_n_mask(10u64, 2u64), 0b1100000000);
    assert_eq!(internal_n_mask(10u64, 4u64), 0b1111000000);
    assert_eq!(
        internal_n_mask(32u64, 4u64),
        0b11110000000000000000000000000000
    );
}

// NEAREST ROUNDING

#[test]
fn round_nearest_test() {
    // Check exactly halfway (b'1100000')
    let mut fp = ExtendedFloat { mant: 0x60, exp: 0 };
    let (above, halfway) = round_nearest(&mut fp, 6);
    assert!(!above);
    assert!(halfway);
    assert_eq!(fp.mant, 1);

    // Check above halfway (b'1100001')
    let mut fp = ExtendedFloat { mant: 0x61, exp: 0 };
    let (above, halfway) = round_nearest(&mut fp, 6);
    assert!(above);
    assert!(!halfway);
    assert_eq!(fp.mant, 1);

    // Check below halfway (b'1011111')
    let mut fp = ExtendedFloat { mant: 0x5F, exp: 0 };
    let (above, halfway) = round_nearest(&mut fp, 6);
    assert!(!above);
    assert!(!halfway);
    assert_eq!(fp.mant, 1);
}

// DIRECTED ROUNDING

#[test]
fn round_downward_test() {
    // b0000000
    let mut fp = ExtendedFloat { mant: 0x00, exp: 0 };
    round_downward(&mut fp, 6);
    assert_eq!(fp.mant, 0);

    // b1000000
    let mut fp = ExtendedFloat { mant: 0x40, exp: 0 };
    round_downward(&mut fp, 6);
    assert_eq!(fp.mant, 1);

    // b1100000
    let mut fp = ExtendedFloat { mant: 0x60, exp: 0 };
    round_downward(&mut fp, 6);
    assert_eq!(fp.mant, 1);

    // b1110000
    let mut fp = ExtendedFloat { mant: 0x70, exp: 0 };
    round_downward(&mut fp, 6);
    assert_eq!(fp.mant, 1);
}

#[test]
fn round_nearest_tie_even_test() {
    // Check round-up, halfway
    let mut fp = ExtendedFloat { mant: 0x60, exp: 0 };
    round_nearest_tie_even(&mut fp, 6);
    assert_eq!(fp.mant, 2);

    // Check round-down, halfway
    let mut fp = ExtendedFloat { mant: 0x20, exp: 0 };
    round_nearest_tie_even(&mut fp, 6);
    assert_eq!(fp.mant, 0);

    // Check round-up, above halfway
    let mut fp = ExtendedFloat { mant: 0x61, exp: 0 };
    round_nearest_tie_even(&mut fp, 6);
    assert_eq!(fp.mant, 2);

    let mut fp = ExtendedFloat { mant: 0x21, exp: 0 };
    round_nearest_tie_even(&mut fp, 6);
    assert_eq!(fp.mant, 1);

    // Check round-down, below halfway
    let mut fp = ExtendedFloat { mant: 0x5F, exp: 0 };
    round_nearest_tie_even(&mut fp, 6);
    assert_eq!(fp.mant, 1);

    let mut fp = ExtendedFloat { mant: 0x1F, exp: 0 };
    round_nearest_tie_even(&mut fp, 6);
    assert_eq!(fp.mant, 0);
}

// HIGH-LEVEL

#[test]
fn round_to_float_test() {
    // Denormal
    let mut fp = ExtendedFloat {
        mant: 1 << 63,
        exp: f64::DENORMAL_EXPONENT - 15,
    };
    round_to_float::<f64, _>(&mut fp, round_nearest_tie_even);
    assert_eq!(fp.mant, 1 << 48);
    assert_eq!(fp.exp, f64::DENORMAL_EXPONENT);

    // Halfway, round-down (b'1000000000000000000000000000000000000000000000000000010000000000')
    let mut fp = ExtendedFloat {
        mant: 0x8000000000000400,
        exp: -63,
    };
    round_to_float::<f64, _>(&mut fp, round_nearest_tie_even);
    assert_eq!(fp.mant, 1 << 52);
    assert_eq!(fp.exp, -52);

    // Halfway, round-up (b'1000000000000000000000000000000000000000000000000000110000000000')
    let mut fp = ExtendedFloat {
        mant: 0x8000000000000C00,
        exp: -63,
    };
    round_to_float::<f64, _>(&mut fp, round_nearest_tie_even);
    assert_eq!(fp.mant, (1 << 52) + 2);
    assert_eq!(fp.exp, -52);

    // Above halfway
    let mut fp = ExtendedFloat {
        mant: 0x8000000000000401,
        exp: -63,
    };
    round_to_float::<f64, _>(&mut fp, round_nearest_tie_even);
    assert_eq!(fp.mant, (1 << 52) + 1);
    assert_eq!(fp.exp, -52);

    let mut fp = ExtendedFloat {
        mant: 0x8000000000000C01,
        exp: -63,
    };
    round_to_float::<f64, _>(&mut fp, round_nearest_tie_even);
    assert_eq!(fp.mant, (1 << 52) + 2);
    assert_eq!(fp.exp, -52);

    // Below halfway
    let mut fp = ExtendedFloat {
        mant: 0x80000000000003FF,
        exp: -63,
    };
    round_to_float::<f64, _>(&mut fp, round_nearest_tie_even);
    assert_eq!(fp.mant, 1 << 52);
    assert_eq!(fp.exp, -52);

    let mut fp = ExtendedFloat {
        mant: 0x8000000000000BFF,
        exp: -63,
    };
    round_to_float::<f64, _>(&mut fp, round_nearest_tie_even);
    assert_eq!(fp.mant, (1 << 52) + 1);
    assert_eq!(fp.exp, -52);
}

#[test]
fn avoid_overflow_test() {
    // Avoid overflow, fails by 1
    let mut fp = ExtendedFloat {
        mant: 0xFFFFFFFFFFFF,
        exp: f64::MAX_EXPONENT + 5,
    };
    avoid_overflow::<f64>(&mut fp);
    assert_eq!(fp.mant, 0xFFFFFFFFFFFF);
    assert_eq!(fp.exp, f64::MAX_EXPONENT + 5);

    // Avoid overflow, succeeds
    let mut fp = ExtendedFloat {
        mant: 0xFFFFFFFFFFFF,
        exp: f64::MAX_EXPONENT + 4,
    };
    avoid_overflow::<f64>(&mut fp);
    assert_eq!(fp.mant, 0x1FFFFFFFFFFFE0);
    assert_eq!(fp.exp, f64::MAX_EXPONENT - 1);
}

#[test]
fn round_to_native_test() {
    // Overflow
    let mut fp = ExtendedFloat {
        mant: 0xFFFFFFFFFFFF,
        exp: f64::MAX_EXPONENT + 4,
    };
    round_to_native::<f64, _>(&mut fp, round_nearest_tie_even);
    assert_eq!(fp.mant, 0x1FFFFFFFFFFFE0);
    assert_eq!(fp.exp, f64::MAX_EXPONENT - 1);

    // Need denormal
    let mut fp = ExtendedFloat {
        mant: 1,
        exp: f64::DENORMAL_EXPONENT + 48,
    };
    round_to_native::<f64, _>(&mut fp, round_nearest_tie_even);
    assert_eq!(fp.mant, 1 << 48);
    assert_eq!(fp.exp, f64::DENORMAL_EXPONENT);

    // Halfway, round-down (b'10000000000000000000000000000000000000000000000000000100000')
    let mut fp = ExtendedFloat {
        mant: 0x400000000000020,
        exp: -58,
    };
    round_to_native::<f64, _>(&mut fp, round_nearest_tie_even);
    assert_eq!(fp.mant, 1 << 52);
    assert_eq!(fp.exp, -52);

    // Halfway, round-up (b'10000000000000000000000000000000000000000000000000001100000')
    let mut fp = ExtendedFloat {
        mant: 0x400000000000060,
        exp: -58,
    };
    round_to_native::<f64, _>(&mut fp, round_nearest_tie_even);
    assert_eq!(fp.mant, (1 << 52) + 2);
    assert_eq!(fp.exp, -52);

    // Above halfway
    let mut fp = ExtendedFloat {
        mant: 0x400000000000021,
        exp: -58,
    };
    round_to_native::<f64, _>(&mut fp, round_nearest_tie_even);
    assert_eq!(fp.mant, (1 << 52) + 1);
    assert_eq!(fp.exp, -52);

    let mut fp = ExtendedFloat {
        mant: 0x400000000000061,
        exp: -58,
    };
    round_to_native::<f64, _>(&mut fp, round_nearest_tie_even);
    assert_eq!(fp.mant, (1 << 52) + 2);
    assert_eq!(fp.exp, -52);

    // Below halfway
    let mut fp = ExtendedFloat {
        mant: 0x40000000000001F,
        exp: -58,
    };
    round_to_native::<f64, _>(&mut fp, round_nearest_tie_even);
    assert_eq!(fp.mant, 1 << 52);
    assert_eq!(fp.exp, -52);

    let mut fp = ExtendedFloat {
        mant: 0x40000000000005F,
        exp: -58,
    };
    round_to_native::<f64, _>(&mut fp, round_nearest_tie_even);
    assert_eq!(fp.mant, (1 << 52) + 1);
    assert_eq!(fp.exp, -52);

    // Underflow
    // Adapted from failures in strtod.
    let mut fp = ExtendedFloat {
        exp: -1139,
        mant: 18446744073709550712,
    };
    round_to_native::<f64, _>(&mut fp, round_nearest_tie_even);
    assert_eq!(fp.mant, 0);
    assert_eq!(fp.exp, 0);

    let mut fp = ExtendedFloat {
        exp: -1139,
        mant: 18446744073709551460,
    };
    round_to_native::<f64, _>(&mut fp, round_nearest_tie_even);
    assert_eq!(fp.mant, 0);
    assert_eq!(fp.exp, 0);

    let mut fp = ExtendedFloat {
        exp: -1138,
        mant: 9223372036854776103,
    };
    round_to_native::<f64, _>(&mut fp, round_nearest_tie_even);
    assert_eq!(fp.mant, 1);
    assert_eq!(fp.exp, -1074);
}
