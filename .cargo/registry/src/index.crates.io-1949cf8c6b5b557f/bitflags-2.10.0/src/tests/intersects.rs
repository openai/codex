use super::*;

use crate::Flags;

#[test]
fn cases() {
    case(
        TestFlags::empty(),
        &[
            (TestFlags::empty(), false),
            (TestFlags::A, false),
            (TestFlags::B, false),
            (TestFlags::C, false),
            (TestFlags::from_bits_retain(1 << 3), false),
        ],
        TestFlags::intersects,
    );

    case(
        TestFlags::A,
        &[
            (TestFlags::empty(), false),
            (TestFlags::A, true),
            (TestFlags::B, false),
            (TestFlags::C, false),
            (TestFlags::ABC, true),
            (TestFlags::from_bits_retain(1 << 3), false),
            (TestFlags::from_bits_retain(1 | (1 << 3)), true),
        ],
        TestFlags::intersects,
    );

    case(
        TestFlags::ABC,
        &[
            (TestFlags::empty(), false),
            (TestFlags::A, true),
            (TestFlags::B, true),
            (TestFlags::C, true),
            (TestFlags::ABC, true),
            (TestFlags::from_bits_retain(1 << 3), false),
        ],
        TestFlags::intersects,
    );

    case(
        TestFlags::from_bits_retain(1 << 3),
        &[
            (TestFlags::empty(), false),
            (TestFlags::A, false),
            (TestFlags::B, false),
            (TestFlags::C, false),
            (TestFlags::from_bits_retain(1 << 3), true),
        ],
        TestFlags::intersects,
    );

    case(
        TestOverlapping::AB,
        &[
            (TestOverlapping::AB, true),
            (TestOverlapping::BC, true),
            (TestOverlapping::from_bits_retain(1 << 1), true),
        ],
        TestOverlapping::intersects,
    );
}

#[track_caller]
fn case<T: Flags + std::fmt::Debug + Copy>(
    value: T,
    inputs: &[(T, bool)],
    mut inherent: impl FnMut(&T, T) -> bool,
) {
    for (input, expected) in inputs {
        assert_eq!(
            *expected,
            inherent(&value, *input),
            "{:?}.intersects({:?})",
            value,
            input
        );
        assert_eq!(
            *expected,
            Flags::intersects(&value, *input),
            "Flags::intersects({:?}, {:?})",
            value,
            input
        );
    }
}
