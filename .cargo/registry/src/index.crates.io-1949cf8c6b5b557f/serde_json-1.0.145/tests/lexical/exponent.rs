// Adapted from https://github.com/Alexhuszagh/rust-lexical.

use crate::lexical::exponent::*;

#[test]
fn scientific_exponent_test() {
    // 0 digits in the integer
    assert_eq!(scientific_exponent(0, 0, 5), -6);
    assert_eq!(scientific_exponent(10, 0, 5), 4);
    assert_eq!(scientific_exponent(-10, 0, 5), -16);

    // >0 digits in the integer
    assert_eq!(scientific_exponent(0, 1, 5), 0);
    assert_eq!(scientific_exponent(0, 2, 5), 1);
    assert_eq!(scientific_exponent(0, 2, 20), 1);
    assert_eq!(scientific_exponent(10, 2, 20), 11);
    assert_eq!(scientific_exponent(-10, 2, 20), -9);

    // Underflow
    assert_eq!(scientific_exponent(i32::MIN, 0, 0), i32::MIN);
    assert_eq!(scientific_exponent(i32::MIN, 0, 5), i32::MIN);

    // Overflow
    assert_eq!(scientific_exponent(i32::MAX, 0, 0), i32::MAX - 1);
    assert_eq!(scientific_exponent(i32::MAX, 5, 0), i32::MAX);
}

#[test]
fn mantissa_exponent_test() {
    assert_eq!(mantissa_exponent(10, 5, 0), 5);
    assert_eq!(mantissa_exponent(0, 5, 0), -5);
    assert_eq!(mantissa_exponent(i32::MAX, 5, 0), i32::MAX - 5);
    assert_eq!(mantissa_exponent(i32::MAX, 0, 5), i32::MAX);
    assert_eq!(mantissa_exponent(i32::MIN, 5, 0), i32::MIN);
    assert_eq!(mantissa_exponent(i32::MIN, 0, 5), i32::MIN + 5);
}
