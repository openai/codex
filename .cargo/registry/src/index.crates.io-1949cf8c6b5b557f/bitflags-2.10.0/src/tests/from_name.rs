use super::*;

use crate::Flags;

#[test]
fn cases() {
    case(Some(1), "A", TestFlags::from_name);
    case(Some(1 << 1), "B", TestFlags::from_name);
    case(Some(1 | 1 << 1 | 1 << 2), "ABC", TestFlags::from_name);

    case(None, "", TestFlags::from_name);
    case(None, "a", TestFlags::from_name);
    case(None, "0x1", TestFlags::from_name);
    case(None, "A | B", TestFlags::from_name);

    case(Some(0), "ZERO", TestZero::from_name);

    case(Some(2), "二", TestUnicode::from_name);

    case(None, "_", TestExternal::from_name);

    case(None, "", TestExternal::from_name);
}

#[track_caller]
fn case<T: Flags>(expected: Option<T::Bits>, input: &str, inherent: impl FnOnce(&str) -> Option<T>)
where
    <T as Flags>::Bits: std::fmt::Debug + PartialEq,
{
    assert_eq!(
        expected,
        inherent(input).map(|f| f.bits()),
        "T::from_name({:?})",
        input
    );
    assert_eq!(
        expected,
        T::from_name(input).map(|f| f.bits()),
        "Flags::from_name({:?})",
        input
    );
}
