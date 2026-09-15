//! Credit formatting preserves tiny adjustments and integer precision.

use super::data;
use pretty_assertions::assert_eq;

#[test]
fn analytics_credit_display_retains_integer_precision() {
    assert_eq!(
        [753.71, 411.12, 47.4, 0.0, 1_212.224, -0.004, -0.000004].map(data::credit_amount),
        [
            "753.71",
            "411.12",
            "47.40",
            "0.00",
            "1,212.22",
            "-0.004",
            "-0.000004"
        ]
        .map(str::to_string)
    );
    assert_eq!(
        [0, -4, 999_999_999, 19_350_041_555, i64::MIN, i64::MAX].map(data::credits),
        [
            "0.00",
            "-0.000004",
            "1,000.00",
            "19,350.04",
            "-9,223,372,036,854.78",
            "9,223,372,036,854.78"
        ]
        .map(str::to_string)
    );
    assert_eq!(
        [
            12_746.441206,
            -1_000.125,
            999.999,
            0.0,
            -0.000004,
            0.009,
            7.5
        ]
        .map(data::amount),
        [
            "12,746.44",
            "-1,000.12",
            "1,000",
            "0",
            "-0.000004",
            "0.009",
            "7.5"
        ]
        .map(str::to_string)
    );
}
