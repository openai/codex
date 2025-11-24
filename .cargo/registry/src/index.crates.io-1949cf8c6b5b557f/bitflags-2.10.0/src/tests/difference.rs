use super::*;

use crate::Flags;

#[test]
fn cases() {
    case(
        TestFlags::A | TestFlags::B,
        &[
            (TestFlags::A, 1 << 1),
            (TestFlags::B, 1),
            (TestFlags::from_bits_retain(1 << 3), 1 | 1 << 1),
        ],
        TestFlags::difference,
    );

    case(
        TestFlags::from_bits_retain(1 | 1 << 3),
        &[
            (TestFlags::A, 1 << 3),
            (TestFlags::from_bits_retain(1 << 3), 1),
        ],
        TestFlags::difference,
    );

    case(
        TestExternal::from_bits_retain(!0),
        &[(TestExternal::A, 0b1111_1110)],
        TestExternal::difference,
    );

    assert_eq!(
        0b1111_1110,
        (TestExternal::from_bits_retain(!0) & !TestExternal::A).bits()
    );

    assert_eq!(
        0b1111_1110,
        (TestFlags::from_bits_retain(!0).difference(TestFlags::A)).bits()
    );

    // The `!` operator unsets bits that don't correspond to known flags
    assert_eq!(
        1 << 1 | 1 << 2,
        (TestFlags::from_bits_retain(!0) & !TestFlags::A).bits()
    );
}

#[track_caller]
fn case<T: Flags + std::fmt::Debug + std::ops::Sub<Output = T> + std::ops::SubAssign + Copy>(
    value: T,
    inputs: &[(T, T::Bits)],
    mut inherent: impl FnMut(T, T) -> T,
) where
    T::Bits: std::fmt::Debug + PartialEq + Copy,
{
    for (input, expected) in inputs {
        assert_eq!(
            *expected,
            inherent(value, *input).bits(),
            "{:?}.difference({:?})",
            value,
            input
        );
        assert_eq!(
            *expected,
            Flags::difference(value, *input).bits(),
            "Flags::difference({:?}, {:?})",
            value,
            input
        );
        assert_eq!(
            *expected,
            (value - *input).bits(),
            "{:?} - {:?}",
            value,
            input
        );
        assert_eq!(
            *expected,
            {
                let mut value = value;
                value -= *input;
                value
            }
            .bits(),
            "{:?} -= {:?}",
            value,
            input,
        );
    }
}
