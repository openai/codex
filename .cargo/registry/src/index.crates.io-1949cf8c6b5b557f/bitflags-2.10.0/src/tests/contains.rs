use super::*;

use crate::Flags;

#[test]
fn cases() {
    case(
        TestFlags::empty(),
        &[
            (TestFlags::empty(), true),
            (TestFlags::A, false),
            (TestFlags::B, false),
            (TestFlags::C, false),
            (TestFlags::from_bits_retain(1 << 3), false),
        ],
        TestFlags::contains,
    );

    case(
        TestFlags::A,
        &[
            (TestFlags::empty(), true),
            (TestFlags::A, true),
            (TestFlags::B, false),
            (TestFlags::C, false),
            (TestFlags::ABC, false),
            (TestFlags::from_bits_retain(1 << 3), false),
            (TestFlags::from_bits_retain(1 | (1 << 3)), false),
        ],
        TestFlags::contains,
    );

    case(
        TestFlags::ABC,
        &[
            (TestFlags::empty(), true),
            (TestFlags::A, true),
            (TestFlags::B, true),
            (TestFlags::C, true),
            (TestFlags::ABC, true),
            (TestFlags::from_bits_retain(1 << 3), false),
        ],
        TestFlags::contains,
    );

    case(
        TestFlags::from_bits_retain(1 << 3),
        &[
            (TestFlags::empty(), true),
            (TestFlags::A, false),
            (TestFlags::B, false),
            (TestFlags::C, false),
            (TestFlags::from_bits_retain(1 << 3), true),
        ],
        TestFlags::contains,
    );

    case(
        TestZero::ZERO,
        &[(TestZero::ZERO, true)],
        TestZero::contains,
    );

    case(
        TestOverlapping::AB,
        &[
            (TestOverlapping::AB, true),
            (TestOverlapping::BC, false),
            (TestOverlapping::from_bits_retain(1 << 1), true),
        ],
        TestOverlapping::contains,
    );

    case(
        TestExternal::all(),
        &[
            (TestExternal::A, true),
            (TestExternal::B, true),
            (TestExternal::C, true),
            (TestExternal::from_bits_retain(1 << 5 | 1 << 7), true),
        ],
        TestExternal::contains,
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
            "{:?}.contains({:?})",
            value,
            input
        );
        assert_eq!(
            *expected,
            Flags::contains(&value, *input),
            "Flags::contains({:?}, {:?})",
            value,
            input
        );
    }
}
