use super::*;

use crate::Flags;

#[test]
fn cases() {
    case(false, TestFlags::empty(), TestFlags::contains_unknown_bits);
    case(false, TestFlags::A, TestFlags::contains_unknown_bits);

    case(
        true,
        TestFlags::ABC | TestFlags::from_bits_retain(1 << 3),
        TestFlags::contains_unknown_bits,
    );

    case(
        true,
        TestFlags::empty() | TestFlags::from_bits_retain(1 << 3),
        TestFlags::contains_unknown_bits,
    );

    case(false, TestFlags::all(), TestFlags::contains_unknown_bits);

    case(false, TestZero::empty(), TestZero::contains_unknown_bits);
}
#[track_caller]
fn case<T: Flags + std::fmt::Debug>(expected: bool, value: T, inherent: impl FnOnce(&T) -> bool) {
    assert_eq!(
        expected,
        inherent(&value),
        "{:?}.contains_unknown_bits()",
        value
    );
    assert_eq!(
        expected,
        Flags::contains_unknown_bits(&value),
        "Flags::contains_unknown_bits({:?})",
        value
    );
}
