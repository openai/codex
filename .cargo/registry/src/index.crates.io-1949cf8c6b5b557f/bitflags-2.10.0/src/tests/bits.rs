use super::*;

use crate::Flags;

#[test]
fn cases() {
    case(0, TestFlags::empty(), TestFlags::bits);

    case(1, TestFlags::A, TestFlags::bits);
    case(1 | 1 << 1 | 1 << 2, TestFlags::ABC, TestFlags::bits);

    case(!0, TestFlags::from_bits_retain(u8::MAX), TestFlags::bits);
    case(1 << 3, TestFlags::from_bits_retain(1 << 3), TestFlags::bits);

    case(1 << 3, TestZero::from_bits_retain(1 << 3), TestZero::bits);

    case(1 << 3, TestEmpty::from_bits_retain(1 << 3), TestEmpty::bits);

    case(
        1 << 4 | 1 << 6,
        TestExternal::from_bits_retain(1 << 4 | 1 << 6),
        TestExternal::bits,
    );
}

#[track_caller]
fn case<T: Flags + std::fmt::Debug>(
    expected: T::Bits,
    value: T,
    inherent: impl FnOnce(&T) -> T::Bits,
) where
    T::Bits: std::fmt::Debug + PartialEq,
{
    assert_eq!(expected, inherent(&value), "{:?}.bits()", value);
    assert_eq!(expected, Flags::bits(&value), "Flags::bits({:?})", value);
}
