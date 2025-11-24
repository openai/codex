use super::*;

use crate::Flags;

#[test]
#[cfg(not(miri))] // Very slow in miri
fn roundtrip() {
    for a in 0u8..=255 {
        for b in 0u8..=255 {
            let f = TestFlags::from_bits_retain(a | b);

            assert_eq!(f, f.iter().collect::<TestFlags>());
            assert_eq!(
                TestFlags::from_bits_truncate(f.bits()),
                f.iter_names().map(|(_, f)| f).collect::<TestFlags>()
            );

            let f = TestExternal::from_bits_retain(a | b);

            assert_eq!(f, f.iter().collect::<TestExternal>());
        }
    }
}

mod collect {
    use super::*;

    #[test]
    fn cases() {
        assert_eq!(0, [].into_iter().collect::<TestFlags>().bits());

        assert_eq!(1, [TestFlags::A,].into_iter().collect::<TestFlags>().bits());

        assert_eq!(
            1 | 1 << 1 | 1 << 2,
            [TestFlags::A, TestFlags::B | TestFlags::C,]
                .into_iter()
                .collect::<TestFlags>()
                .bits()
        );

        assert_eq!(
            1 | 1 << 3,
            [
                TestFlags::from_bits_retain(1 << 3),
                TestFlags::empty(),
                TestFlags::A,
            ]
            .into_iter()
            .collect::<TestFlags>()
            .bits()
        );

        assert_eq!(
            1 << 5 | 1 << 7,
            [
                TestExternal::empty(),
                TestExternal::from_bits_retain(1 << 5),
                TestExternal::from_bits_retain(1 << 7),
            ]
            .into_iter()
            .collect::<TestExternal>()
            .bits()
        );
    }
}

mod iter {
    use super::*;

    #[test]
    fn cases() {
        case(&[], TestFlags::empty(), TestFlags::iter);

        case(&[1], TestFlags::A, TestFlags::iter);
        case(&[1, 1 << 1], TestFlags::A | TestFlags::B, TestFlags::iter);
        case(
            &[1, 1 << 1, 1 << 3],
            TestFlags::A | TestFlags::B | TestFlags::from_bits_retain(1 << 3),
            TestFlags::iter,
        );

        case(&[1, 1 << 1, 1 << 2], TestFlags::ABC, TestFlags::iter);
        case(
            &[1, 1 << 1, 1 << 2, 1 << 3],
            TestFlags::ABC | TestFlags::from_bits_retain(1 << 3),
            TestFlags::iter,
        );

        case(
            &[1 | 1 << 1 | 1 << 2],
            TestFlagsInvert::ABC,
            TestFlagsInvert::iter,
        );

        case(&[], TestZero::ZERO, TestZero::iter);

        case(
            &[1, 1 << 1, 1 << 2, 0b1111_1000],
            TestExternal::all(),
            TestExternal::iter,
        );
    }

    #[track_caller]
    fn case<T: Flags + std::fmt::Debug + IntoIterator<Item = T> + Copy>(
        expected: &[T::Bits],
        value: T,
        inherent: impl FnOnce(&T) -> crate::iter::Iter<T>,
    ) where
        T::Bits: std::fmt::Debug + PartialEq,
    {
        assert_eq!(
            expected,
            inherent(&value).map(|f| f.bits()).collect::<Vec<_>>(),
            "{:?}.iter()",
            value
        );
        assert_eq!(
            expected,
            Flags::iter(&value).map(|f| f.bits()).collect::<Vec<_>>(),
            "Flags::iter({:?})",
            value
        );
        assert_eq!(
            expected,
            value.into_iter().map(|f| f.bits()).collect::<Vec<_>>(),
            "{:?}.into_iter()",
            value
        );
    }
}

mod iter_names {
    use super::*;

    #[test]
    fn cases() {
        case(&[], TestFlags::empty(), TestFlags::iter_names);

        case(&[("A", 1)], TestFlags::A, TestFlags::iter_names);
        case(
            &[("A", 1), ("B", 1 << 1)],
            TestFlags::A | TestFlags::B,
            TestFlags::iter_names,
        );
        case(
            &[("A", 1), ("B", 1 << 1)],
            TestFlags::A | TestFlags::B | TestFlags::from_bits_retain(1 << 3),
            TestFlags::iter_names,
        );

        case(
            &[("A", 1), ("B", 1 << 1), ("C", 1 << 2)],
            TestFlags::ABC,
            TestFlags::iter_names,
        );
        case(
            &[("A", 1), ("B", 1 << 1), ("C", 1 << 2)],
            TestFlags::ABC | TestFlags::from_bits_retain(1 << 3),
            TestFlags::iter_names,
        );

        case(
            &[("ABC", 1 | 1 << 1 | 1 << 2)],
            TestFlagsInvert::ABC,
            TestFlagsInvert::iter_names,
        );

        case(&[], TestZero::ZERO, TestZero::iter_names);

        case(
            &[("A", 1)],
            TestOverlappingFull::A,
            TestOverlappingFull::iter_names,
        );
        case(
            &[("A", 1), ("D", 1 << 1)],
            TestOverlappingFull::A | TestOverlappingFull::D,
            TestOverlappingFull::iter_names,
        );
    }

    #[track_caller]
    fn case<T: Flags + std::fmt::Debug>(
        expected: &[(&'static str, T::Bits)],
        value: T,
        inherent: impl FnOnce(&T) -> crate::iter::IterNames<T>,
    ) where
        T::Bits: std::fmt::Debug + PartialEq,
    {
        assert_eq!(
            expected,
            inherent(&value)
                .map(|(n, f)| (n, f.bits()))
                .collect::<Vec<_>>(),
            "{:?}.iter_names()",
            value
        );
        assert_eq!(
            expected,
            Flags::iter_names(&value)
                .map(|(n, f)| (n, f.bits()))
                .collect::<Vec<_>>(),
            "Flags::iter_names({:?})",
            value
        );
    }
}

mod iter_defined_names {
    use crate::Flags;

    #[test]
    fn test_defined_names() {
        bitflags! {
            #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
            struct TestFlags: u32 {
                const A = 0b00000001;
                const ZERO = 0;
                const B = 0b00000010;
                const C = 0b00000100;
                const CC = Self::C.bits();
                const D = 0b10000100;
                const ABC = Self::A.bits() | Self::B.bits() | Self::C.bits();
                const AB = Self::A.bits() | Self::B.bits();
                const AC = Self::A.bits() | Self::C.bits();
                const CB = Self::B.bits() | Self::C.bits();
            }
        }

        // Test all named flags produced by the iterator
        let all_named: Vec<(&'static str, TestFlags)> = TestFlags::iter_defined_names().collect();

        // Verify all named flags are included
        let expected_flags = vec![
            ("A", TestFlags::A),
            ("ZERO", TestFlags::ZERO),
            ("B", TestFlags::B),
            ("C", TestFlags::C),
            // Note: CC and C have the same bit value, but both are named flags
            ("CC", TestFlags::CC),
            ("D", TestFlags::D),
            ("ABC", TestFlags::ABC),
            ("AB", TestFlags::AB),
            ("AC", TestFlags::AC),
            ("CB", TestFlags::CB),
        ];

        assert_eq!(
            all_named.len(),
            expected_flags.len(),
            "Should have 10 named flags"
        );

        // Verify each expected flag is in the result
        for expected_flag in &expected_flags {
            assert!(
                all_named.contains(expected_flag),
                "Missing flag: {:?}",
                expected_flag
            );
        }

        // Test if iterator order is consistent with definition order
        let flags_in_order: Vec<(&'static str, TestFlags)> =
            TestFlags::iter_defined_names().collect();
        assert_eq!(
            flags_in_order, expected_flags,
            "Flag order should match definition order"
        );

        // Test that iterator can be used multiple times
        let first_iteration: Vec<(&'static str, TestFlags)> =
            TestFlags::iter_defined_names().collect();
        let second_iteration: Vec<(&'static str, TestFlags)> =
            TestFlags::iter_defined_names().collect();
        assert_eq!(
            first_iteration, second_iteration,
            "Multiple iterations should produce the same result"
        );

        // Test consistency with FLAGS constant
        let flags_from_iter: std::collections::HashSet<u32> = TestFlags::iter_defined_names()
            .map(|(_, f)| f.bits())
            .collect();

        let flags_from_const: std::collections::HashSet<u32> = TestFlags::FLAGS
            .iter()
            .filter(|f| f.is_named())
            .map(|f| f.value().bits())
            .collect();

        assert_eq!(
            flags_from_iter, flags_from_const,
            "iter_defined_names() should be consistent with named flags in FLAGS"
        );
    }
}
