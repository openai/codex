use super::*;

use crate::Flags;

#[test]
fn cases() {
    case(Some(0), 0, TestFlags::from_bits);
    case(Some(1), 1, TestFlags::from_bits);
    case(
        Some(1 | 1 << 1 | 1 << 2),
        1 | 1 << 1 | 1 << 2,
        TestFlags::from_bits,
    );

    case(None, 1 << 3, TestFlags::from_bits);
    case(None, 1 | 1 << 3, TestFlags::from_bits);

    case(Some(1 | 1 << 1), 1 | 1 << 1, TestOverlapping::from_bits);

    case(Some(1 << 1), 1 << 1, TestOverlapping::from_bits);

    case(Some(1 << 5), 1 << 5, TestExternal::from_bits);
}

#[track_caller]
fn case<T: Flags>(
    expected: Option<T::Bits>,
    input: T::Bits,
    inherent: impl FnOnce(T::Bits) -> Option<T>,
) where
    <T as Flags>::Bits: std::fmt::Debug + PartialEq,
{
    assert_eq!(
        expected,
        inherent(input).map(|f| f.bits()),
        "T::from_bits({:?})",
        input
    );
    assert_eq!(
        expected,
        T::from_bits(input).map(|f| f.bits()),
        "Flags::from_bits({:?})",
        input
    );
}
