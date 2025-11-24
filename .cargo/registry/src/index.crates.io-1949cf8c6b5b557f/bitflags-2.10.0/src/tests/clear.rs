use super::*;

use crate::Flags;

#[test]
fn cases() {
    case(TestFlags::from_bits_retain(0));

    case(TestFlags::from_bits_retain(1 << 3));

    case(TestFlags::ABC | TestFlags::from_bits_retain(1 << 3));

    case(TestZero::empty());

    case(TestZero::all());

    case(TestFlags::from_bits_retain(1 << 3) | TestFlags::all());
}

#[track_caller]
fn case<T: Flags + std::fmt::Debug>(mut flags: T)
where
    T: std::fmt::Debug + PartialEq + Copy,
{
    flags.clear();
    assert_eq!(flags, T::empty(), "{:?}.clear()", flags);
}
