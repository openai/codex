//! These tests are adapted from the Rust core library's unittests.

#![cfg(not(feature = "compact"))]

use minimal_lexical::lemire;
use minimal_lexical::num::Float;

fn compute_error32(q: i32, w: u64) -> (i32, u64) {
    let fp = lemire::compute_error::<f32>(q, w);
    (fp.exp, fp.mant)
}

fn compute_error64(q: i32, w: u64) -> (i32, u64) {
    let fp = lemire::compute_error::<f64>(q, w);
    (fp.exp, fp.mant)
}

fn compute_error_scaled32(q: i32, w: u64, lz: i32) -> (i32, u64) {
    let fp = lemire::compute_error_scaled::<f32>(q, w, lz);
    (fp.exp, fp.mant)
}

fn compute_error_scaled64(q: i32, w: u64, lz: i32) -> (i32, u64) {
    let fp = lemire::compute_error_scaled::<f64>(q, w, lz);
    (fp.exp, fp.mant)
}

fn compute_float32(q: i32, w: u64) -> (i32, u64) {
    let fp = lemire::compute_float::<f32>(q, w);
    (fp.exp, fp.mant)
}

fn compute_float64(q: i32, w: u64) -> (i32, u64) {
    let fp = lemire::compute_float::<f64>(q, w);
    (fp.exp, fp.mant)
}

#[test]
fn compute_error32_test() {
    // These test near-halfway cases for single-precision floats.
    assert_eq!(compute_error32(0, 16777216), (111 + f32::INVALID_FP, 9223372036854775808));
    assert_eq!(compute_error32(0, 16777217), (111 + f32::INVALID_FP, 9223372586610589696));
    assert_eq!(compute_error32(0, 16777218), (111 + f32::INVALID_FP, 9223373136366403584));
    assert_eq!(compute_error32(0, 16777219), (111 + f32::INVALID_FP, 9223373686122217472));
    assert_eq!(compute_error32(0, 16777220), (111 + f32::INVALID_FP, 9223374235878031360));

    // These are examples of the above tests, with
    // digits from the exponent shifted to the mantissa.
    assert_eq!(
        compute_error32(-10, 167772160000000000),
        (111 + f32::INVALID_FP, 9223372036854775808)
    );
    assert_eq!(
        compute_error32(-10, 167772170000000000),
        (111 + f32::INVALID_FP, 9223372586610589696)
    );
    assert_eq!(
        compute_error32(-10, 167772180000000000),
        (111 + f32::INVALID_FP, 9223373136366403584)
    );
    // Let's check the lines to see if anything is different in table...
    assert_eq!(
        compute_error32(-10, 167772190000000000),
        (111 + f32::INVALID_FP, 9223373686122217472)
    );
    assert_eq!(
        compute_error32(-10, 167772200000000000),
        (111 + f32::INVALID_FP, 9223374235878031360)
    );
}

#[test]
fn compute_error64_test() {
    // These test near-halfway cases for double-precision floats.
    assert_eq!(compute_error64(0, 9007199254740992), (1065 + f64::INVALID_FP, 9223372036854775808));
    assert_eq!(compute_error64(0, 9007199254740993), (1065 + f64::INVALID_FP, 9223372036854776832));
    assert_eq!(compute_error64(0, 9007199254740994), (1065 + f64::INVALID_FP, 9223372036854777856));
    assert_eq!(compute_error64(0, 9007199254740995), (1065 + f64::INVALID_FP, 9223372036854778880));
    assert_eq!(compute_error64(0, 9007199254740996), (1065 + f64::INVALID_FP, 9223372036854779904));
    assert_eq!(
        compute_error64(0, 18014398509481984),
        (1066 + f64::INVALID_FP, 9223372036854775808)
    );
    assert_eq!(
        compute_error64(0, 18014398509481986),
        (1066 + f64::INVALID_FP, 9223372036854776832)
    );
    assert_eq!(
        compute_error64(0, 18014398509481988),
        (1066 + f64::INVALID_FP, 9223372036854777856)
    );
    assert_eq!(
        compute_error64(0, 18014398509481990),
        (1066 + f64::INVALID_FP, 9223372036854778880)
    );
    assert_eq!(
        compute_error64(0, 18014398509481992),
        (1066 + f64::INVALID_FP, 9223372036854779904)
    );

    // Test a much closer set of examples.
    assert_eq!(
        compute_error64(0, 9007199254740991),
        (1064 + f64::INVALID_FP, 18446744073709549568)
    );
    assert_eq!(
        compute_error64(0, 9223372036854776831),
        (1075 + f64::INVALID_FP, 9223372036854776830)
    );
    assert_eq!(
        compute_error64(0, 9223372036854776832),
        (1075 + f64::INVALID_FP, 9223372036854776832)
    );
    assert_eq!(
        compute_error64(0, 9223372036854776833),
        (1075 + f64::INVALID_FP, 9223372036854776832)
    );
    assert_eq!(
        compute_error64(-42, 9123456727292927),
        (925 + f64::INVALID_FP, 13021432563531497894)
    );
    assert_eq!(
        compute_error64(-43, 91234567272929275),
        (925 + f64::INVALID_FP, 13021432563531498606)
    );
    assert_eq!(
        compute_error64(-42, 9123456727292928),
        (925 + f64::INVALID_FP, 13021432563531499320)
    );

    // These are examples of the above tests, with
    // digits from the exponent shifted to the mantissa.
    assert_eq!(
        compute_error64(-3, 9007199254740992000),
        (1065 + f64::INVALID_FP, 9223372036854775808)
    );
    assert_eq!(
        compute_error64(-3, 9007199254740993000),
        (1065 + f64::INVALID_FP, 9223372036854776832)
    );
    assert_eq!(
        compute_error64(-3, 9007199254740994000),
        (1065 + f64::INVALID_FP, 9223372036854777856)
    );
    assert_eq!(
        compute_error64(-3, 9007199254740995000),
        (1065 + f64::INVALID_FP, 9223372036854778880)
    );
    assert_eq!(
        compute_error64(-3, 9007199254740996000),
        (1065 + f64::INVALID_FP, 9223372036854779904)
    );

    // Test from errors in atof.
    assert_eq!(
        compute_error64(-18, 1000000178813934326),
        (1012 + f64::INVALID_FP, 9223373686122217470)
    );

    // Check edge-cases from previous errors.
    assert_eq!(
        compute_error64(-342, 2470328229206232720),
        (-64 + f64::INVALID_FP, 18446744073709551608)
    );
}

#[test]
fn compute_error_scaled32_test() {
    // These are the same examples above, just using pre-computed scaled values.

    // These test near-halfway cases for single-precision floats.
    assert_eq!(
        compute_error_scaled32(0, 4611686018427387904, 39),
        (111 + f32::INVALID_FP, 9223372036854775808)
    );
    assert_eq!(
        compute_error_scaled32(0, 4611686293305294848, 39),
        (111 + f32::INVALID_FP, 9223372586610589696)
    );
    assert_eq!(
        compute_error_scaled32(0, 4611686568183201792, 39),
        (111 + f32::INVALID_FP, 9223373136366403584)
    );
    assert_eq!(
        compute_error_scaled32(0, 4611686843061108736, 39),
        (111 + f32::INVALID_FP, 9223373686122217472)
    );
    assert_eq!(
        compute_error_scaled32(0, 4611687117939015680, 39),
        (111 + f32::INVALID_FP, 9223374235878031360)
    );

    assert_eq!(
        compute_error_scaled32(-10, 9223372036854775808, 6),
        (111 + f32::INVALID_FP, 9223372036854775808)
    );
    assert_eq!(
        compute_error_scaled32(-10, 9223372586610589696, 6),
        (111 + f32::INVALID_FP, 9223372586610589696)
    );
    assert_eq!(
        compute_error_scaled32(-10, 9223373136366403584, 6),
        (111 + f32::INVALID_FP, 9223373136366403584)
    );
    assert_eq!(
        compute_error_scaled32(-10, 9223373686122217472, 6),
        (111 + f32::INVALID_FP, 9223373686122217472)
    );
    assert_eq!(
        compute_error_scaled32(-10, 9223374235878031360, 6),
        (111 + f32::INVALID_FP, 9223374235878031360)
    );
}

#[test]
fn compute_error_scaled64_test() {
    // These are the same examples above, just using pre-computed scaled values.

    // These test near-halfway cases for double-precision floats.
    assert_eq!(
        compute_error_scaled64(0, 4611686018427387904, 10),
        (1065 + f64::INVALID_FP, 9223372036854775808)
    );
    assert_eq!(
        compute_error_scaled64(0, 4611686018427388416, 10),
        (1065 + f64::INVALID_FP, 9223372036854776832)
    );
    assert_eq!(
        compute_error_scaled64(0, 4611686018427388928, 10),
        (1065 + f64::INVALID_FP, 9223372036854777856)
    );
    assert_eq!(
        compute_error_scaled64(0, 4611686018427389440, 10),
        (1065 + f64::INVALID_FP, 9223372036854778880)
    );
    assert_eq!(
        compute_error_scaled64(0, 4611686018427389952, 10),
        (1065 + f64::INVALID_FP, 9223372036854779904)
    );
    assert_eq!(
        compute_error_scaled64(0, 4611686018427387904, 9),
        (1066 + f64::INVALID_FP, 9223372036854775808)
    );
    assert_eq!(
        compute_error_scaled64(0, 4611686018427388416, 9),
        (1066 + f64::INVALID_FP, 9223372036854776832)
    );
    assert_eq!(
        compute_error_scaled64(0, 4611686018427388928, 9),
        (1066 + f64::INVALID_FP, 9223372036854777856)
    );
    assert_eq!(
        compute_error_scaled64(0, 4611686018427389440, 9),
        (1066 + f64::INVALID_FP, 9223372036854778880)
    );
    assert_eq!(
        compute_error_scaled64(0, 4611686018427389952, 9),
        (1066 + f64::INVALID_FP, 9223372036854779904)
    );

    // Test a much closer set of examples.
    assert_eq!(
        compute_error_scaled64(0, 9223372036854774784, 11),
        (1064 + f64::INVALID_FP, 18446744073709549568)
    );
    assert_eq!(
        compute_error_scaled64(0, 4611686018427388415, 0),
        (1075 + f64::INVALID_FP, 9223372036854776830)
    );
    assert_eq!(
        compute_error_scaled64(0, 4611686018427388416, 0),
        (1075 + f64::INVALID_FP, 9223372036854776832)
    );
    assert_eq!(
        compute_error_scaled64(0, 4611686018427388416, 0),
        (1075 + f64::INVALID_FP, 9223372036854776832)
    );
    assert_eq!(
        compute_error_scaled64(-42, 6510716281765748947, 10),
        (925 + f64::INVALID_FP, 13021432563531497894)
    );
    assert_eq!(
        compute_error_scaled64(-43, 6510716281765749303, 7),
        (925 + f64::INVALID_FP, 13021432563531498606)
    );
    assert_eq!(
        compute_error_scaled64(-42, 6510716281765749660, 10),
        (925 + f64::INVALID_FP, 13021432563531499320)
    );

    // These are examples of the above tests, with
    // digits from the exponent shifted to the mantissa.
    assert_eq!(
        compute_error_scaled64(-3, 9223372036854775808, 1),
        (1065 + f64::INVALID_FP, 9223372036854775808)
    );
    assert_eq!(
        compute_error_scaled64(-3, 9223372036854776832, 1),
        (1065 + f64::INVALID_FP, 9223372036854776832)
    );
    assert_eq!(
        compute_error_scaled64(-3, 9223372036854777856, 1),
        (1065 + f64::INVALID_FP, 9223372036854777856)
    );
    assert_eq!(
        compute_error_scaled64(-3, 9223372036854778880, 1),
        (1065 + f64::INVALID_FP, 9223372036854778880)
    );
    assert_eq!(
        compute_error_scaled64(-3, 9223372036854779904, 1),
        (1065 + f64::INVALID_FP, 9223372036854779904)
    );

    // Test from errors in atof.
    assert_eq!(
        compute_error_scaled64(-18, 9223373686122217470, 4),
        (1012 + f64::INVALID_FP, 9223373686122217470)
    );

    // Check edge-cases from previous errors.
    assert_eq!(
        compute_error_scaled64(-342, 9223372036854775804, 2),
        (-64 + f64::INVALID_FP, 18446744073709551608)
    );
}

#[test]
fn compute_float_f32_rounding() {
    // These test near-halfway cases for single-precision floats.
    assert_eq!(compute_float32(0, 16777216), (151, 0));
    assert_eq!(compute_float32(0, 16777217), (151, 0));
    assert_eq!(compute_float32(0, 16777218), (151, 1));
    assert_eq!(compute_float32(0, 16777219), (151, 2));
    assert_eq!(compute_float32(0, 16777220), (151, 2));

    // These are examples of the above tests, with
    // digits from the exponent shifted to the mantissa.
    assert_eq!(compute_float32(-10, 167772160000000000), (151, 0));
    assert_eq!(compute_float32(-10, 167772170000000000), (151, 0));
    assert_eq!(compute_float32(-10, 167772180000000000), (151, 1));
    // Let's check the lines to see if anything is different in table...
    assert_eq!(compute_float32(-10, 167772190000000000), (151, 2));
    assert_eq!(compute_float32(-10, 167772200000000000), (151, 2));
}

#[test]
fn compute_float_f64_rounding() {
    // Also need to check halfway cases **inside** that exponent range.

    // These test near-halfway cases for double-precision floats.
    assert_eq!(compute_float64(0, 9007199254740992), (1076, 0));
    assert_eq!(compute_float64(0, 9007199254740993), (1076, 0));
    assert_eq!(compute_float64(0, 9007199254740994), (1076, 1));
    assert_eq!(compute_float64(0, 9007199254740995), (1076, 2));
    assert_eq!(compute_float64(0, 9007199254740996), (1076, 2));
    assert_eq!(compute_float64(0, 18014398509481984), (1077, 0));
    assert_eq!(compute_float64(0, 18014398509481986), (1077, 0));
    assert_eq!(compute_float64(0, 18014398509481988), (1077, 1));
    assert_eq!(compute_float64(0, 18014398509481990), (1077, 2));
    assert_eq!(compute_float64(0, 18014398509481992), (1077, 2));

    // Test a much closer set of examples.
    assert_eq!(compute_float64(0, 9007199254740991), (1075, 4503599627370495));
    assert_eq!(compute_float64(0, 9223372036854776831), (1086, 0));
    assert_eq!(compute_float64(0, 9223372036854776832), (1086, 0));
    assert_eq!(compute_float64(0, 9223372036854776833), (1086, 1));
    assert_eq!(compute_float64(-42, 9123456727292927), (936, 1854521741541368));
    assert_eq!(compute_float64(-43, 91234567272929275), (936, 1854521741541369));
    assert_eq!(compute_float64(-42, 9123456727292928), (936, 1854521741541369));

    // These are examples of the above tests, with
    // digits from the exponent shifted to the mantissa.
    assert_eq!(compute_float64(-3, 9007199254740992000), (1076, 0));
    assert_eq!(compute_float64(-3, 9007199254740993000), (1076, 0));
    assert_eq!(compute_float64(-3, 9007199254740994000), (1076, 1));
    assert_eq!(compute_float64(-3, 9007199254740995000), (1076, 2));
    assert_eq!(compute_float64(-3, 9007199254740996000), (1076, 2));
}
