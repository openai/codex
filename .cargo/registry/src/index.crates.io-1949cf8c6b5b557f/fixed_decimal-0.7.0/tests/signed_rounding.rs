// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use core::ops::RangeInclusive;
use fixed_decimal::Decimal;
use fixed_decimal::Sign;
use fixed_decimal::SignedRoundingMode as SRM;
use fixed_decimal::UnsignedRoundingMode as URM;
use writeable::Writeable;

#[test]
pub fn test_ecma402_table() {
    // Source: <https://tc39.es/ecma402/#table-intl-rounding-modes>
    let cases: [(_, _, _, _, _, _, _); 9] = [
        ("ceil", SRM::Ceil, -1, 1, 1, 1, 2),
        ("floor", SRM::Floor, -2, 0, 0, 0, 1),
        ("expand", SRM::Unsigned(URM::Expand), -2, 1, 1, 1, 2),
        ("trunc", SRM::Unsigned(URM::Trunc), -1, 0, 0, 0, 1),
        ("half_ceil", SRM::HalfCeil, -1, 0, 1, 1, 2),
        ("half_floor", SRM::HalfFloor, -2, 0, 0, 1, 1),
        (
            "half_expand",
            SRM::Unsigned(URM::HalfExpand),
            -2,
            0,
            1,
            1,
            2,
        ),
        ("half_trunc", SRM::Unsigned(URM::HalfTrunc), -1, 0, 0, 1, 1),
        ("half_even", SRM::Unsigned(URM::HalfEven), -2, 0, 0, 1, 2),
    ];
    for (name, mode, e1, e2, e3, e4, e5) in cases {
        let mut fd1: Decimal = "-1.5".parse().unwrap();
        let mut fd2: Decimal = "0.4".parse().unwrap();
        let mut fd3: Decimal = "0.5".parse().unwrap();
        let mut fd4: Decimal = "0.6".parse().unwrap();
        let mut fd5: Decimal = "1.5".parse().unwrap();
        fd1.round_with_mode(0, mode);
        fd2.round_with_mode(0, mode);
        fd3.round_with_mode(0, mode);
        fd4.round_with_mode(0, mode);
        fd5.round_with_mode(0, mode);
        assert_eq!(
            fd1.write_to_string(),
            e1.write_to_string(),
            "-1.5 failed for {name}"
        );
        assert_eq!(
            fd2.write_to_string(),
            e2.write_to_string(),
            "0.4 failed for {name}"
        );
        assert_eq!(
            fd3.write_to_string(),
            e3.write_to_string(),
            "0.5 failed for {name}"
        );
        assert_eq!(
            fd4.write_to_string(),
            e4.write_to_string(),
            "0.6 failed for {name}"
        );
        assert_eq!(
            fd5.write_to_string(),
            e5.write_to_string(),
            "1.5 failed for {name}"
        );
    }
}

#[test]
pub fn test_within_ranges() {
    struct TestCase {
        rounding_mode_name: &'static str,
        rounding_mode: SRM,
        range_n2000: RangeInclusive<i32>,
        range_n1000: RangeInclusive<i32>,
        range_0: RangeInclusive<i32>,
        range_1000: RangeInclusive<i32>,
        range_2000: RangeInclusive<i32>,
    }
    let cases: [TestCase; 9] = [
        TestCase {
            rounding_mode_name: "ceil",
            rounding_mode: SRM::Ceil,
            range_n2000: -2999..=-2000,
            range_n1000: -1999..=-1000,
            range_0: -999..=0,
            range_1000: 1..=1000,
            range_2000: 1001..=2000,
        },
        TestCase {
            rounding_mode_name: "floor",
            rounding_mode: SRM::Floor,
            range_n2000: -2000..=-1001,
            range_n1000: -1000..=-1,
            range_0: 0..=999,
            range_1000: 1000..=1999,
            range_2000: 2000..=2999,
        },
        TestCase {
            rounding_mode_name: "expand",
            rounding_mode: SRM::Unsigned(URM::Expand),
            range_n2000: -2000..=-1001,
            range_n1000: -1000..=-1,
            range_0: 0..=0,
            range_1000: 1..=1000,
            range_2000: 1001..=2000,
        },
        TestCase {
            rounding_mode_name: "trunc",
            rounding_mode: SRM::Unsigned(URM::Trunc),
            range_n2000: -2999..=-2000,
            range_n1000: -1999..=-1000,
            range_0: -999..=999,
            range_1000: 1000..=1999,
            range_2000: 2000..=2999,
        },
        TestCase {
            rounding_mode_name: "half_ceil",
            rounding_mode: SRM::HalfCeil,
            range_n2000: -2500..=-1501,
            range_n1000: -1500..=-501,
            range_0: -500..=449,
            range_1000: 500..=1449,
            range_2000: 1500..=2449,
        },
        TestCase {
            rounding_mode_name: "half_floor",
            rounding_mode: SRM::HalfFloor,
            range_n2000: -2449..=-1500,
            range_n1000: -1449..=-500,
            range_0: -449..=500,
            range_1000: 501..=1500,
            range_2000: 1501..=2500,
        },
        TestCase {
            rounding_mode_name: "half_expand",
            rounding_mode: SRM::Unsigned(URM::HalfExpand),
            range_n2000: -2449..=-1500,
            range_n1000: -1449..=-500,
            range_0: -449..=449,
            range_1000: 500..=1449,
            range_2000: 1500..=2449,
        },
        TestCase {
            rounding_mode_name: "half_trunc",
            rounding_mode: SRM::Unsigned(URM::HalfTrunc),
            range_n2000: -2500..=-1501,
            range_n1000: -1500..=-501,
            range_0: -500..=500,
            range_1000: 501..=1500,
            range_2000: 1501..=2500,
        },
        TestCase {
            rounding_mode_name: "half_even",
            rounding_mode: SRM::Unsigned(URM::HalfEven),
            range_n2000: -2500..=-1500,
            range_n1000: -1449..=-501,
            range_0: -500..=500,
            range_1000: 501..=1449,
            range_2000: 1500..=2500,
        },
    ];
    for TestCase {
        rounding_mode_name,
        rounding_mode,
        range_n2000,
        range_n1000,
        range_0,
        range_1000,
        range_2000,
    } in cases
    {
        for n in range_n2000 {
            let mut fd = Decimal::from(n);
            fd.round_with_mode(3, rounding_mode);
            assert_eq!(fd.write_to_string(), "-2000", "{rounding_mode_name}: {n}");
            let mut fd = Decimal::from(n - 1000000);
            fd.multiply_pow10(-5);
            fd.round_with_mode(-2, rounding_mode);
            assert_eq!(
                fd.write_to_string(),
                "-10.02",
                "{rounding_mode_name}: {n} ÷ 10^5 ± 10"
            );
        }
        for n in range_n1000 {
            let mut fd = Decimal::from(n);
            fd.round_with_mode(3, rounding_mode);
            assert_eq!(fd.write_to_string(), "-1000", "{rounding_mode_name}: {n}");
            let mut fd = Decimal::from(n - 1000000);
            fd.multiply_pow10(-5);
            fd.round_with_mode(-2, rounding_mode);
            assert_eq!(
                fd.write_to_string(),
                "-10.01",
                "{rounding_mode_name}: {n} ÷ 10^5 ± 10"
            );
        }
        for n in range_0 {
            let mut fd = Decimal::from(n);
            fd.round_with_mode(3, rounding_mode);
            fd.set_sign(Sign::None); // get rid of -0
            assert_eq!(fd.write_to_string(), "000", "{rounding_mode_name}: {n}");

            let (mut fd, expected) = if n < 0 {
                (
                    {
                        let mut fd = Decimal::from(n - 1000000);
                        fd.multiply_pow10(-5);
                        fd
                    },
                    "-10.00",
                )
            } else {
                (
                    {
                        let mut fd = Decimal::from(n + 1000000);
                        fd.multiply_pow10(-5);
                        fd
                    },
                    "10.00",
                )
            };
            fd.round_with_mode(-2, rounding_mode);
            assert_eq!(
                fd.write_to_string(),
                expected,
                "{rounding_mode_name}: {n} ÷ 10^5 ± 10"
            );
        }
        for n in range_1000 {
            let mut fd = Decimal::from(n);
            fd.round_with_mode(3, rounding_mode);
            assert_eq!(fd.write_to_string(), "1000", "{rounding_mode_name}: {n}");
            let mut fd = Decimal::from(n + 1000000);
            fd.multiply_pow10(-5);
            fd.round_with_mode(-2, rounding_mode);
            assert_eq!(
                fd.write_to_string(),
                "10.01",
                "{rounding_mode_name}: {n} ÷ 10^5 ± 10"
            );
        }
        for n in range_2000 {
            let mut fd = Decimal::from(n);
            fd.round_with_mode(3, rounding_mode);
            assert_eq!(fd.write_to_string(), "2000", "{rounding_mode_name}: {n}");
            let mut fd = Decimal::from(n + 1000000);
            fd.multiply_pow10(-5);
            fd.round_with_mode(-2, rounding_mode);
            assert_eq!(
                fd.write_to_string(),
                "10.02",
                "{rounding_mode_name}: {n} ÷ 10^5 ± 10"
            );
        }
    }
}

#[test]
pub fn extra_rounding_mode_cases() {
    struct TestCase {
        input: &'static str,
        position: i16,
        // ceil, floor, expand, trunc, half_ceil, half_floor, half_expand, half_trunc, half_even
        all_expected: [&'static str; 9],
    }
    let cases: [TestCase; 8] = [
        TestCase {
            input: "505.050",
            position: -3,
            all_expected: [
                "505.050", "505.050", "505.050", "505.050", "505.050", "505.050", "505.050",
                "505.050", "505.050",
            ],
        },
        TestCase {
            input: "505.050",
            position: -2,
            all_expected: [
                "505.05", "505.05", "505.05", "505.05", "505.05", "505.05", "505.05", "505.05",
                "505.05",
            ],
        },
        TestCase {
            input: "505.050",
            position: -1,
            all_expected: [
                "505.1", "505.0", "505.1", "505.0", "505.1", "505.0", "505.1", "505.0", "505.0",
            ],
        },
        TestCase {
            input: "505.050",
            position: 0,
            all_expected: [
                "506", "505", "506", "505", "505", "505", "505", "505", "505",
            ],
        },
        TestCase {
            input: "505.050",
            position: 1,
            all_expected: [
                "510", "500", "510", "500", "510", "510", "510", "510", "510",
            ],
        },
        TestCase {
            input: "505.050",
            position: 2,
            all_expected: [
                "600", "500", "600", "500", "500", "500", "500", "500", "500",
            ],
        },
        TestCase {
            input: "505.050",
            position: 3,
            all_expected: [
                "1000", "000", "1000", "000", "1000", "1000", "1000", "1000", "1000",
            ],
        },
        TestCase {
            input: "505.050",
            position: 4,
            all_expected: [
                "10000", "0000", "10000", "0000", "0000", "0000", "0000", "0000", "0000",
            ],
        },
    ];
    #[allow(clippy::type_complexity)] // most compact representation in code
    let rounding_modes: [(&'static str, SRM); 9] = [
        ("ceil", SRM::Ceil),
        ("floor", SRM::Floor),
        ("expand", SRM::Unsigned(URM::Expand)),
        ("trunc", SRM::Unsigned(URM::Trunc)),
        ("half_ceil", SRM::HalfCeil),
        ("half_floor", SRM::HalfFloor),
        ("half_expand", SRM::Unsigned(URM::HalfExpand)),
        ("half_trunc", SRM::Unsigned(URM::HalfTrunc)),
        ("half_even", SRM::Unsigned(URM::HalfEven)),
    ];
    for TestCase {
        input,
        position,
        all_expected,
    } in cases
    {
        for ((rounding_mode_name, rounding_mode), expected) in
            rounding_modes.iter().zip(all_expected.iter())
        {
            let mut fd: Decimal = input.parse().unwrap();
            fd.round_with_mode(position, *rounding_mode);
            assert_eq!(
                &*fd.write_to_string(),
                *expected,
                "{input}: {rounding_mode_name} @ {position}"
            )
        }
    }
}

#[test]
pub fn test_ecma402_table_with_increments() {
    use fixed_decimal::RoundingIncrement;

    #[rustfmt::skip] // Don't split everything on its own line. Makes it look a lot nicer.
    #[allow(clippy::type_complexity)]
    let cases: [(_, _, [(_, _, _, _, _, _, _); 9]); 3] = [
        ("two", RoundingIncrement::MultiplesOf2, [
            ("ceil", SRM::Ceil, "-1.4", "0.4", "0.6", "0.6", "1.6"),
            ("floor", SRM::Floor, "-1.6", "0.4", "0.4", "0.6", "1.4"),
            ("expand", SRM::Unsigned(URM::Expand), "-1.6", "0.4", "0.6", "0.6", "1.6"),
            ("trunc", SRM::Unsigned(URM::Trunc), "-1.4", "0.4", "0.4", "0.6", "1.4"),
            ("half_ceil", SRM::HalfCeil, "-1.4", "0.4", "0.6", "0.6", "1.6"),
            ("half_floor", SRM::HalfFloor, "-1.6", "0.4", "0.4", "0.6", "1.4"),
            ("half_expand", SRM::Unsigned(URM::HalfExpand), "-1.6", "0.4", "0.6", "0.6", "1.6"),
            ("half_trunc", SRM::Unsigned(URM::HalfTrunc), "-1.4", "0.4", "0.4", "0.6", "1.4"),
            ("half_even", SRM::Unsigned(URM::HalfEven), "-1.6", "0.4", "0.4", "0.6", "1.6"),
        ]),
        ("five", RoundingIncrement::MultiplesOf5, [
            ("ceil", SRM::Ceil, "-1.5", "0.5", "0.5", "1.0", "1.5"),
            ("floor", SRM::Floor, "-1.5", "0.0", "0.5", "0.5", "1.5"),
            ("expand", SRM::Unsigned(URM::Expand), "-1.5", "0.5", "0.5", "1.0", "1.5"),
            ("trunc", SRM::Unsigned(URM::Trunc), "-1.5", "0.0", "0.5", "0.5", "1.5"),
            ("half_ceil", SRM::HalfCeil, "-1.5", "0.5", "0.5", "0.5", "1.5"),
            ("half_floor", SRM::HalfFloor, "-1.5", "0.5", "0.5", "0.5", "1.5"),
            ("half_expand", SRM::Unsigned(URM::HalfExpand), "-1.5", "0.5", "0.5", "0.5", "1.5"),
            ("half_trunc", SRM::Unsigned(URM::HalfTrunc), "-1.5", "0.5", "0.5", "0.5", "1.5"),
            ("half_even", SRM::Unsigned(URM::HalfEven), "-1.5", "0.5", "0.5", "0.5", "1.5"),
        ]),
        ("twenty-five", RoundingIncrement::MultiplesOf25, [
            ("ceil", SRM::Ceil, "-0.0", "2.5", "2.5", "2.5", "2.5"),
            ("floor", SRM::Floor, "-2.5", "0.0", "0.0", "0.0", "0.0"),
            ("expand", SRM::Unsigned(URM::Expand), "-2.5", "2.5", "2.5", "2.5", "2.5"),
            ("trunc", SRM::Unsigned(URM::Trunc), "-0.0", "0.0", "0.0", "0.0", "0.0"),
            ("half_ceil", SRM::HalfCeil, "-2.5", "0.0", "0.0", "0.0", "2.5"),
            ("half_floor", SRM::HalfFloor, "-2.5", "0.0", "0.0", "0.0", "2.5"),
            ("half_expand", SRM::Unsigned(URM::HalfExpand), "-2.5", "0.0", "0.0", "0.0", "2.5"),
            ("half_trunc", SRM::Unsigned(URM::HalfTrunc), "-2.5", "0.0", "0.0", "0.0", "2.5"),
            ("half_even", SRM::Unsigned(URM::HalfEven), "-2.5", "0.0", "0.0", "0.0", "2.5"),
        ]),
    ];

    for (increment_str, increment, cases) in cases {
        for (rounding_mode_name, rounding_mode, e1, e2, e3, e4, e5) in cases {
            let mut fd1: Decimal = "-1.5".parse().unwrap();
            let mut fd2: Decimal = "0.4".parse().unwrap();
            let mut fd3: Decimal = "0.5".parse().unwrap();
            let mut fd4: Decimal = "0.6".parse().unwrap();
            let mut fd5: Decimal = "1.5".parse().unwrap();
            // The original ECMA-402 table tests rounding at magnitude 0.
            // However, testing rounding at magnitude -1 gives more
            // interesting test cases for increments.
            fd1.round_with_mode_and_increment(-1, rounding_mode, increment);
            fd2.round_with_mode_and_increment(-1, rounding_mode, increment);
            fd3.round_with_mode_and_increment(-1, rounding_mode, increment);
            fd4.round_with_mode_and_increment(-1, rounding_mode, increment);
            fd5.round_with_mode_and_increment(-1, rounding_mode, increment);
            assert_eq!(
                fd1.write_to_string(),
                e1,
                "-1.5 failed for {rounding_mode_name} with increments of {increment_str}"
            );
            assert_eq!(
                fd2.write_to_string(),
                e2,
                "0.4 failed for {rounding_mode_name} with increments of {increment_str}"
            );
            assert_eq!(
                fd3.write_to_string(),
                e3,
                "0.5 failed for {rounding_mode_name} with increments of {increment_str}"
            );
            assert_eq!(
                fd4.write_to_string(),
                e4,
                "0.6 failed for {rounding_mode_name} with increments of {increment_str}"
            );
            assert_eq!(
                fd5.write_to_string(),
                e5,
                "1.5 failed for {rounding_mode_name} with increments of {increment_str}"
            );
        }
    }
}
