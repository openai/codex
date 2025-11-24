use super::*;

use crate::Flags;

#[test]
fn cases() {
    case(
        TestFlags::empty(),
        &[
            (TestFlags::empty(), 0),
            (TestFlags::all(), 1 | 1 << 1 | 1 << 2),
            (TestFlags::from_bits_retain(1 << 3), 1 << 3),
        ],
        TestFlags::symmetric_difference,
        TestFlags::toggle,
    );

    case(
        TestFlags::A,
        &[
            (TestFlags::empty(), 1),
            (TestFlags::A, 0),
            (TestFlags::all(), 1 << 1 | 1 << 2),
        ],
        TestFlags::symmetric_difference,
        TestFlags::toggle,
    );

    case(
        TestFlags::A | TestFlags::B | TestFlags::from_bits_retain(1 << 3),
        &[
            (TestFlags::ABC, 1 << 2 | 1 << 3),
            (TestFlags::from_bits_retain(1 << 3), 1 | 1 << 1),
        ],
        TestFlags::symmetric_difference,
        TestFlags::toggle,
    );
}

#[track_caller]
fn case<T: Flags + std::fmt::Debug + std::ops::BitXor<Output = T> + std::ops::BitXorAssign + Copy>(
    value: T,
    inputs: &[(T, T::Bits)],
    mut inherent_sym_diff: impl FnMut(T, T) -> T,
    mut inherent_toggle: impl FnMut(&mut T, T),
) where
    T::Bits: std::fmt::Debug + PartialEq + Copy,
{
    for (input, expected) in inputs {
        assert_eq!(
            *expected,
            inherent_sym_diff(value, *input).bits(),
            "{:?}.symmetric_difference({:?})",
            value,
            input
        );
        assert_eq!(
            *expected,
            Flags::symmetric_difference(value, *input).bits(),
            "Flags::symmetric_difference({:?}, {:?})",
            value,
            input
        );
        assert_eq!(
            *expected,
            (value ^ *input).bits(),
            "{:?} ^ {:?}",
            value,
            input
        );
        assert_eq!(
            *expected,
            {
                let mut value = value;
                value ^= *input;
                value
            }
            .bits(),
            "{:?} ^= {:?}",
            value,
            input,
        );

        assert_eq!(
            *expected,
            {
                let mut value = value;
                inherent_toggle(&mut value, *input);
                value
            }
            .bits(),
            "{:?}.toggle({:?})",
            value,
            input,
        );

        assert_eq!(
            *expected,
            {
                let mut value = value;
                Flags::toggle(&mut value, *input);
                value
            }
            .bits(),
            "{:?}.toggle({:?})",
            value,
            input,
        );
    }
}
