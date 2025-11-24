use super::*;

use crate::Flags;

#[test]
fn cases() {
    case(0, TestFlags::empty);

    case(0, TestZero::empty);

    case(0, TestEmpty::empty);

    case(0, TestExternal::empty);
}

#[track_caller]
fn case<T: Flags>(expected: T::Bits, inherent: impl FnOnce() -> T)
where
    <T as Flags>::Bits: std::fmt::Debug + PartialEq,
{
    assert_eq!(expected, inherent().bits(), "T::empty()");
    assert_eq!(expected, T::empty().bits(), "Flags::empty()");
}
