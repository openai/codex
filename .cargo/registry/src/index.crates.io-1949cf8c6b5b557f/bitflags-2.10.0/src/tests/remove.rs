use super::*;

use crate::Flags;

#[test]
fn cases() {
    case(
        TestFlags::empty(),
        &[
            (TestFlags::A, 0),
            (TestFlags::empty(), 0),
            (TestFlags::from_bits_retain(1 << 3), 0),
        ],
        TestFlags::remove,
        TestFlags::set,
    );

    case(
        TestFlags::A,
        &[
            (TestFlags::A, 0),
            (TestFlags::empty(), 1),
            (TestFlags::B, 1),
        ],
        TestFlags::remove,
        TestFlags::set,
    );

    case(
        TestFlags::ABC,
        &[
            (TestFlags::A, 1 << 1 | 1 << 2),
            (TestFlags::A | TestFlags::C, 1 << 1),
        ],
        TestFlags::remove,
        TestFlags::set,
    );
}

#[track_caller]
fn case<T: Flags + std::fmt::Debug + Copy>(
    value: T,
    inputs: &[(T, T::Bits)],
    mut inherent_remove: impl FnMut(&mut T, T),
    mut inherent_set: impl FnMut(&mut T, T, bool),
) where
    T::Bits: std::fmt::Debug + PartialEq + Copy,
{
    for (input, expected) in inputs {
        assert_eq!(
            *expected,
            {
                let mut value = value;
                inherent_remove(&mut value, *input);
                value
            }
            .bits(),
            "{:?}.remove({:?})",
            value,
            input
        );
        assert_eq!(
            *expected,
            {
                let mut value = value;
                Flags::remove(&mut value, *input);
                value
            }
            .bits(),
            "Flags::remove({:?}, {:?})",
            value,
            input
        );

        assert_eq!(
            *expected,
            {
                let mut value = value;
                inherent_set(&mut value, *input, false);
                value
            }
            .bits(),
            "{:?}.set({:?}, false)",
            value,
            input
        );
        assert_eq!(
            *expected,
            {
                let mut value = value;
                Flags::set(&mut value, *input, false);
                value
            }
            .bits(),
            "Flags::set({:?}, {:?}, false)",
            value,
            input
        );
    }
}
