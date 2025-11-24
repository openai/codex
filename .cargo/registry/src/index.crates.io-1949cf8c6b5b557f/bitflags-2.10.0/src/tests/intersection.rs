use super::*;

use crate::Flags;

#[test]
fn cases() {
    case(
        TestFlags::empty(),
        &[(TestFlags::empty(), 0), (TestFlags::all(), 0)],
        TestFlags::intersection,
    );

    case(
        TestFlags::all(),
        &[
            (TestFlags::all(), 1 | 1 << 1 | 1 << 2),
            (TestFlags::A, 1),
            (TestFlags::from_bits_retain(1 << 3), 0),
        ],
        TestFlags::intersection,
    );

    case(
        TestFlags::from_bits_retain(1 << 3),
        &[(TestFlags::from_bits_retain(1 << 3), 1 << 3)],
        TestFlags::intersection,
    );

    case(
        TestOverlapping::AB,
        &[(TestOverlapping::BC, 1 << 1)],
        TestOverlapping::intersection,
    );
}

#[track_caller]
fn case<T: Flags + std::fmt::Debug + std::ops::BitAnd<Output = T> + std::ops::BitAndAssign + Copy>(
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
            "{:?}.intersection({:?})",
            value,
            input
        );
        assert_eq!(
            *expected,
            Flags::intersection(value, *input).bits(),
            "Flags::intersection({:?}, {:?})",
            value,
            input
        );
        assert_eq!(
            *expected,
            (value & *input).bits(),
            "{:?} & {:?}",
            value,
            input
        );
        assert_eq!(
            *expected,
            {
                let mut value = value;
                value &= *input;
                value
            }
            .bits(),
            "{:?} &= {:?}",
            value,
            input,
        );
    }
}
