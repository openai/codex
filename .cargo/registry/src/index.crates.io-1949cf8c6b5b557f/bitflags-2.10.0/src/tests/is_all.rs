use super::*;

use crate::Flags;

#[test]
fn cases() {
    case(false, TestFlags::empty(), TestFlags::is_all);
    case(false, TestFlags::A, TestFlags::is_all);

    case(true, TestFlags::ABC, TestFlags::is_all);

    case(
        true,
        TestFlags::ABC | TestFlags::from_bits_retain(1 << 3),
        TestFlags::is_all,
    );

    case(true, TestZero::empty(), TestZero::is_all);

    case(true, TestEmpty::empty(), TestEmpty::is_all);
}

#[track_caller]
fn case<T: Flags + std::fmt::Debug>(expected: bool, value: T, inherent: impl FnOnce(&T) -> bool) {
    assert_eq!(expected, inherent(&value), "{:?}.is_all()", value);
    assert_eq!(
        expected,
        Flags::is_all(&value),
        "Flags::is_all({:?})",
        value
    );
}
