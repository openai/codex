macro_rules! test_average {
    ($I:ident, $U:ident) => {
        mod $I {
            mod ceil {
                use num_integer::Average;

                #[test]
                fn same_sign() {
                    assert_eq!((14 as $I).average_ceil(&16), 15 as $I);
                    assert_eq!((14 as $I).average_ceil(&17), 16 as $I);

                    let max = std::$I::MAX;
                    assert_eq!((max - 3).average_ceil(&(max - 1)), max - 2);
                    assert_eq!((max - 3).average_ceil(&(max - 2)), max - 2);
                }

                #[test]
                fn different_sign() {
                    assert_eq!((14 as $I).average_ceil(&-4), 5 as $I);
                    assert_eq!((14 as $I).average_ceil(&-5), 5 as $I);

                    let min = std::$I::MIN;
                    let max = std::$I::MAX;
                    assert_eq!(min.average_ceil(&max), 0 as $I);
                }
            }

            mod floor {
                use num_integer::Average;

                #[test]
                fn same_sign() {
                    assert_eq!((14 as $I).average_floor(&16), 15 as $I);
                    assert_eq!((14 as $I).average_floor(&17), 15 as $I);

                    let max = std::$I::MAX;
                    assert_eq!((max - 3).average_floor(&(max - 1)), max - 2);
                    assert_eq!((max - 3).average_floor(&(max - 2)), max - 3);
                }

                #[test]
                fn different_sign() {
                    assert_eq!((14 as $I).average_floor(&-4), 5 as $I);
                    assert_eq!((14 as $I).average_floor(&-5), 4 as $I);

                    let min = std::$I::MIN;
                    let max = std::$I::MAX;
                    assert_eq!(min.average_floor(&max), -1 as $I);
                }
            }
        }

        mod $U {
            mod ceil {
                use num_integer::Average;

                #[test]
                fn bounded() {
                    assert_eq!((14 as $U).average_ceil(&16), 15 as $U);
                    assert_eq!((14 as $U).average_ceil(&17), 16 as $U);
                }

                #[test]
                fn overflow() {
                    let max = std::$U::MAX;
                    assert_eq!((max - 3).average_ceil(&(max - 1)), max - 2);
                    assert_eq!((max - 3).average_ceil(&(max - 2)), max - 2);
                }
            }

            mod floor {
                use num_integer::Average;

                #[test]
                fn bounded() {
                    assert_eq!((14 as $U).average_floor(&16), 15 as $U);
                    assert_eq!((14 as $U).average_floor(&17), 15 as $U);
                }

                #[test]
                fn overflow() {
                    let max = std::$U::MAX;
                    assert_eq!((max - 3).average_floor(&(max - 1)), max - 2);
                    assert_eq!((max - 3).average_floor(&(max - 2)), max - 3);
                }
            }
        }
    };
}

test_average!(i8, u8);
test_average!(i16, u16);
test_average!(i32, u32);
test_average!(i64, u64);
test_average!(i128, u128);
test_average!(isize, usize);
