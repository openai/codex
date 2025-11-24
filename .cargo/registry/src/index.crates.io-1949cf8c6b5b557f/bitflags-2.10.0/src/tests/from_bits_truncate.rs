use super::*;

use crate::Flags;

#[test]
fn cases() {
    case(0, 0, TestFlags::from_bits_truncate);
    case(1, 1, TestFlags::from_bits_truncate);
    case(
        1 | 1 << 1 | 1 << 2,
        1 | 1 << 1 | 1 << 2,
        TestFlags::from_bits_truncate,
    );

    case(0, 1 << 3, TestFlags::from_bits_truncate);
    case(1, 1 | 1 << 3, TestFlags::from_bits_truncate);

    case(1 | 1 << 1, 1 | 1 << 1, TestOverlapping::from_bits_truncate);

    case(1 << 1, 1 << 1, TestOverlapping::from_bits_truncate);

    case(1 << 5, 1 << 5, TestExternal::from_bits_truncate);
}

#[track_caller]
fn case<T: Flags>(expected: T::Bits, input: T::Bits, inherent: impl FnOnce(T::Bits) -> T)
where
    <T as Flags>::Bits: std::fmt::Debug + PartialEq,
{
    assert_eq!(
        expected,
        inherent(input).bits(),
        "T::from_bits_truncate({:?})",
        input
    );
    assert_eq!(
        expected,
        T::from_bits_truncate(input).bits(),
        "Flags::from_bits_truncate({:?})",
        input
    );
}
