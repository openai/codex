use super::*;

use crate::Flags;

#[test]
fn cases() {
    case(1 | 1 << 1 | 1 << 2, TestFlags::all);

    case(0, TestZero::all);

    case(0, TestEmpty::all);

    case(!0, TestExternal::all);
}

#[track_caller]
fn case<T: Flags>(expected: T::Bits, inherent: impl FnOnce() -> T)
where
    <T as Flags>::Bits: std::fmt::Debug + PartialEq,
{
    assert_eq!(expected, inherent().bits(), "T::all()");
    assert_eq!(expected, T::all().bits(), "Flags::all()");
}
