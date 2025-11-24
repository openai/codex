use super::*;

use crate::Flags;

#[test]
fn cases() {
    case(true, TestFlags::empty(), TestFlags::is_empty);

    case(false, TestFlags::A, TestFlags::is_empty);
    case(false, TestFlags::ABC, TestFlags::is_empty);
    case(
        false,
        TestFlags::from_bits_retain(1 << 3),
        TestFlags::is_empty,
    );

    case(true, TestZero::empty(), TestZero::is_empty);

    case(true, TestEmpty::empty(), TestEmpty::is_empty);
}

#[track_caller]
fn case<T: Flags + std::fmt::Debug>(expected: bool, value: T, inherent: impl FnOnce(&T) -> bool) {
    assert_eq!(expected, inherent(&value), "{:?}.is_empty()", value);
    assert_eq!(
        expected,
        Flags::is_empty(&value),
        "Flags::is_empty({:?})",
        value
    );
}
