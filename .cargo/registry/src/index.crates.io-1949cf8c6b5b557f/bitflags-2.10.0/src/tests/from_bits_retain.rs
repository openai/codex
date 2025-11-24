use super::*;

use crate::Flags;

#[test]
fn cases() {
    case(0, TestFlags::from_bits_retain);
    case(1, TestFlags::from_bits_retain);
    case(1 | 1 << 1 | 1 << 2, TestFlags::from_bits_retain);

    case(1 << 3, TestFlags::from_bits_retain);
    case(1 | 1 << 3, TestFlags::from_bits_retain);

    case(1 | 1 << 1, TestOverlapping::from_bits_retain);

    case(1 << 1, TestOverlapping::from_bits_retain);

    case(1 << 5, TestExternal::from_bits_retain);
}

#[track_caller]
fn case<T: Flags>(input: T::Bits, inherent: impl FnOnce(T::Bits) -> T)
where
    <T as Flags>::Bits: std::fmt::Debug + PartialEq,
{
    assert_eq!(
        input,
        inherent(input).bits(),
        "T::from_bits_retain({:?})",
        input
    );
    assert_eq!(
        input,
        T::from_bits_retain(input).bits(),
        "Flags::from_bits_retain({:?})",
        input
    );
}
