use super::*;

use crate::Flags;

#[test]
fn cases() {
    case(
        TestFlags::empty(),
        &[
            (TestFlags::A, 1),
            (TestFlags::A | TestFlags::B, 1 | 1 << 1),
            (TestFlags::empty(), 0),
            (TestFlags::from_bits_retain(1 << 3), 1 << 3),
        ],
        TestFlags::insert,
        TestFlags::set,
    );

    case(
        TestFlags::A,
        &[
            (TestFlags::A, 1),
            (TestFlags::empty(), 1),
            (TestFlags::B, 1 | 1 << 1),
        ],
        TestFlags::insert,
        TestFlags::set,
    );
}

#[track_caller]
fn case<T: Flags + std::fmt::Debug + Copy>(
    value: T,
    inputs: &[(T, T::Bits)],
    mut inherent_insert: impl FnMut(&mut T, T),
    mut inherent_set: impl FnMut(&mut T, T, bool),
) where
    T::Bits: std::fmt::Debug + PartialEq + Copy,
{
    for (input, expected) in inputs {
        assert_eq!(
            *expected,
            {
                let mut value = value;
                inherent_insert(&mut value, *input);
                value
            }
            .bits(),
            "{:?}.insert({:?})",
            value,
            input
        );
        assert_eq!(
            *expected,
            {
                let mut value = value;
                Flags::insert(&mut value, *input);
                value
            }
            .bits(),
            "Flags::insert({:?}, {:?})",
            value,
            input
        );

        assert_eq!(
            *expected,
            {
                let mut value = value;
                inherent_set(&mut value, *input, true);
                value
            }
            .bits(),
            "{:?}.set({:?}, true)",
            value,
            input
        );
        assert_eq!(
            *expected,
            {
                let mut value = value;
                Flags::set(&mut value, *input, true);
                value
            }
            .bits(),
            "Flags::set({:?}, {:?}, true)",
            value,
            input
        );
    }
}
