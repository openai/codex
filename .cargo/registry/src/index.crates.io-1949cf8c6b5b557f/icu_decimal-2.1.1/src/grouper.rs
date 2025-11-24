// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

//! Algorithms to determine where to position grouping separators.

use crate::options::GroupingStrategy;
use crate::provider::GroupingSizes;
use core::cmp;

/// Returns whether to display a grouping separator at the given magnitude.
///
/// `upper_magnitude` is the magnitude of the highest-power digit, used for resolving minimum
/// grouping digits.
pub fn check(
    upper_magnitude: i16,
    magnitude: i16,
    strategy: GroupingStrategy,
    sizes: GroupingSizes,
) -> bool {
    let primary = if sizes.primary == 0 {
        return false;
    } else {
        sizes.primary as i16
    };
    if magnitude < primary {
        return false;
    }
    let min_grouping = {
        use GroupingStrategy::*;
        match strategy {
            Never => return false,
            // Note: Auto and Always are the same for DecimalFormatter.
            // When currencies are implemented, this will change.
            Auto | Always => cmp::max(1, sizes.min_grouping) as i16,
            Min2 => cmp::max(2, sizes.min_grouping) as i16,
        }
    };
    if upper_magnitude < primary + min_grouping - 1 {
        return false;
    }
    let secondary = if sizes.secondary == 0 {
        primary
    } else {
        sizes.secondary as i16
    };
    let magnitude_prime = magnitude - primary;
    if magnitude_prime % secondary == 0 {
        return true;
    }
    false
}

#[test]
fn test_grouper() {
    use crate::input::Decimal;
    use crate::options;
    use crate::provider::*;
    use crate::DecimalFormatter;
    use icu_provider::prelude::*;
    use std::cell::RefCell;
    use writeable::assert_writeable_eq;

    let western_sizes = GroupingSizes {
        min_grouping: 1,
        primary: 3,
        secondary: 3,
    };
    let indic_sizes = GroupingSizes {
        min_grouping: 1,
        primary: 3,
        secondary: 2,
    };
    let western_sizes_min3 = GroupingSizes {
        min_grouping: 3,
        primary: 3,
        secondary: 3,
    };

    // primary=0 implies no grouping; the other fields are ignored
    let zero_test = GroupingSizes {
        min_grouping: 0,
        primary: 0,
        secondary: 0,
    };

    // secondary=0 implies that it inherits from primary
    let blank_secondary = GroupingSizes {
        min_grouping: 0,
        primary: 3,
        secondary: 0,
    };

    #[derive(Debug)]
    struct TestCase {
        strategy: GroupingStrategy,
        sizes: GroupingSizes,
        // Expected results for numbers with magnitude 3, 4, 5, and 6
        expected: [&'static str; 4],
    }
    let cases = [
        TestCase {
            strategy: GroupingStrategy::Auto,
            sizes: western_sizes,
            expected: ["1,000", "10,000", "100,000", "1,000,000"],
        },
        TestCase {
            strategy: GroupingStrategy::Min2,
            sizes: western_sizes,
            expected: ["1000", "10,000", "100,000", "1,000,000"],
        },
        TestCase {
            strategy: GroupingStrategy::Auto,
            sizes: indic_sizes,
            expected: ["1,000", "10,000", "1,00,000", "10,00,000"],
        },
        TestCase {
            strategy: GroupingStrategy::Min2,
            sizes: indic_sizes,
            expected: ["1000", "10,000", "1,00,000", "10,00,000"],
        },
        TestCase {
            strategy: GroupingStrategy::Auto,
            sizes: western_sizes_min3,
            expected: ["1000", "10000", "100,000", "1,000,000"],
        },
        TestCase {
            strategy: GroupingStrategy::Min2,
            sizes: western_sizes_min3,
            expected: ["1000", "10000", "100,000", "1,000,000"],
        },
        TestCase {
            strategy: GroupingStrategy::Auto,
            sizes: zero_test,
            expected: ["1000", "10000", "100000", "1000000"],
        },
        TestCase {
            strategy: GroupingStrategy::Min2,
            sizes: zero_test,
            expected: ["1000", "10000", "100000", "1000000"],
        },
        TestCase {
            strategy: GroupingStrategy::Auto,
            sizes: blank_secondary,
            expected: ["1,000", "10,000", "100,000", "1,000,000"],
        },
        TestCase {
            strategy: GroupingStrategy::Min2,
            sizes: blank_secondary,
            expected: ["1000", "10,000", "100,000", "1,000,000"],
        },
    ];
    for cas in &cases {
        for i in 0..4 {
            let dec = {
                let mut dec = Decimal::from(1);
                dec.multiply_pow10((i as i16) + 3);
                dec
            };
            let symbols = crate::provider::DecimalSymbols {
                grouping_sizes: cas.sizes,
                ..DecimalSymbols::new_en_for_testing()
            };
            let digits = ['0', '1', '2', '3', '4', '5', '6', '7', '8', '9'];
            struct Provider(RefCell<Option<DecimalSymbols<'static>>>, [char; 10]);
            impl DataProvider<DecimalSymbolsV1> for Provider {
                fn load(
                    &self,
                    _req: icu_provider::DataRequest,
                ) -> Result<icu_provider::DataResponse<DecimalSymbolsV1>, icu_provider::DataError>
                {
                    Ok(DataResponse {
                        metadata: Default::default(),
                        payload: DataPayload::from_owned(
                            self.0
                                .borrow_mut()
                                .take()
                                // We only have one payload
                                .ok_or(DataErrorKind::Custom.into_error())?,
                        ),
                    })
                }
            }
            impl DataProvider<DecimalDigitsV1> for Provider {
                fn load(
                    &self,
                    _req: icu_provider::DataRequest,
                ) -> Result<icu_provider::DataResponse<DecimalDigitsV1>, icu_provider::DataError>
                {
                    Ok(DataResponse {
                        metadata: Default::default(),
                        payload: DataPayload::from_owned(self.1),
                    })
                }
            }
            let provider = Provider(RefCell::new(Some(symbols)), digits);
            let options = options::DecimalFormatterOptions {
                grouping_strategy: Some(cas.strategy),
                ..Default::default()
            };
            let formatter =
                DecimalFormatter::try_new_unstable(&provider, Default::default(), options).unwrap();
            let actual = formatter.format(&dec);
            assert_writeable_eq!(actual, cas.expected[i], "{:?}", cas);
        }
    }
}
