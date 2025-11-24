use super::*;

use crate::Flags;

#[test]
fn cases() {
    case(
        TestFlags::empty(),
        &[
            (TestFlags::A, 1),
            (TestFlags::all(), 1 | 1 << 1 | 1 << 2),
            (TestFlags::empty(), 0),
            (TestFlags::from_bits_retain(1 << 3), 1 << 3),
        ],
        TestFlags::union,
    );

    case(
        TestFlags::A | TestFlags::C,
        &[
            (TestFlags::A | TestFlags::B, 1 | 1 << 1 | 1 << 2),
            (TestFlags::A, 1 | 1 << 2),
        ],
        TestFlags::union,
    );
}

#[track_caller]
fn case<T: Flags + std::fmt::Debug + std::ops::BitOr<Output = T> + std::ops::BitOrAssign + Copy>(
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
            "{:?}.union({:?})",
            value,
            input
        );
        assert_eq!(
            *expected,
            Flags::union(value, *input).bits(),
            "Flags::union({:?}, {:?})",
            value,
            input
        );
        assert_eq!(
            *expected,
            (value | *input).bits(),
            "{:?} | {:?}",
            value,
            input
        );
        assert_eq!(
            *expected,
            {
                let mut value = value;
                value |= *input;
                value
            }
            .bits(),
            "{:?} |= {:?}",
            value,
            input,
        );
    }
}
