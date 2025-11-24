use super::*;
use core::time::Duration;
use serde_with::{
    DurationMicroSeconds, DurationMicroSecondsWithFrac, DurationMilliSeconds,
    DurationMilliSecondsWithFrac, DurationNanoSeconds, DurationNanoSecondsWithFrac,
    DurationSeconds, DurationSecondsWithFrac, TimestampMicroSeconds, TimestampMicroSecondsWithFrac,
    TimestampMilliSeconds, TimestampMilliSecondsWithFrac, TimestampNanoSeconds,
    TimestampNanoSecondsWithFrac, TimestampSeconds, TimestampSecondsWithFrac,
};
use std::time::SystemTime;

#[test]
fn test_duration_seconds() {
    let zero = Duration::new(0, 0);
    let one_second = Duration::new(1, 0);
    let half_second = Duration::new(0, 500_000_000);

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct IntStrict(#[serde_as(as = "DurationSeconds")] Duration);

    is_equal(IntStrict(zero), expect![[r#"0"#]]);
    is_equal(IntStrict(one_second), expect![[r#"1"#]]);
    check_serialization(IntStrict(half_second), expect![[r#"1"#]]);
    check_error_deserialization::<IntStrict>(
        r#""1""#,
        expect![[r#"invalid type: string "1", expected u64 at line 1 column 3"#]],
    );
    check_error_deserialization::<IntStrict>(
        r#"-1"#,
        expect![[r#"invalid value: integer `-1`, expected u64 at line 1 column 2"#]],
    );

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct IntFlexible(#[serde_as(as = "DurationSeconds<u64, Flexible>")] Duration);

    is_equal(IntFlexible(zero), expect![[r#"0"#]]);
    is_equal(IntFlexible(one_second), expect![[r#"1"#]]);
    check_serialization(IntFlexible(half_second), expect![[r#"1"#]]);
    check_deserialization(IntFlexible(half_second), r#""0.5""#);
    check_deserialization(IntFlexible(one_second), r#""1""#);
    check_deserialization(IntFlexible(zero), r#""0""#);
    check_error_deserialization::<IntFlexible>(
        r#""a""#,
        expect![[
            r#"invalid value: string "a", expected an integer, a float, or a string containing a number at line 1 column 3"#
        ]],
    );
    check_error_deserialization::<IntFlexible>(
        r#"-1"#,
        expect![[r#"std::time::Duration cannot be negative"#]],
    );

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct F64Strict(#[serde_as(as = "DurationSeconds<f64>")] Duration);

    is_equal(F64Strict(zero), expect![[r#"0.0"#]]);
    is_equal(F64Strict(one_second), expect![[r#"1.0"#]]);
    check_serialization(F64Strict(half_second), expect![[r#"1.0"#]]);
    check_deserialization(F64Strict(one_second), r#"0.5"#);
    check_error_deserialization::<F64Strict>(
        r#""1""#,
        expect![[r#"invalid type: string "1", expected f64 at line 1 column 3"#]],
    );
    check_error_deserialization::<F64Strict>(
        r#"-1.0"#,
        expect![[r#"std::time::Duration cannot be negative"#]],
    );

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct F64Flexible(#[serde_as(as = "DurationSeconds<f64, Flexible>")] Duration);

    is_equal(F64Flexible(zero), expect![[r#"0.0"#]]);
    is_equal(F64Flexible(one_second), expect![[r#"1.0"#]]);
    check_serialization(F64Flexible(half_second), expect![[r#"1.0"#]]);
    check_deserialization(F64Flexible(half_second), r#""0.5""#);
    check_deserialization(F64Flexible(one_second), r#""1""#);
    check_deserialization(F64Flexible(zero), r#""0""#);
    check_error_deserialization::<F64Flexible>(
        r#""a""#,
        expect![[
            r#"invalid value: string "a", expected an integer, a float, or a string containing a number at line 1 column 3"#
        ]],
    );
    check_error_deserialization::<F64Flexible>(
        r#"-1"#,
        expect![[r#"std::time::Duration cannot be negative"#]],
    );

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct StringStrict(#[serde_as(as = "DurationSeconds<String>")] Duration);

    is_equal(StringStrict(zero), expect![[r#""0""#]]);
    is_equal(StringStrict(one_second), expect![[r#""1""#]]);
    check_serialization(StringStrict(half_second), expect![[r#""1""#]]);
    check_error_deserialization::<StringStrict>(
        r#"1"#,
        expect![[
            r#"invalid type: integer `1`, expected a string containing a number at line 1 column 1"#
        ]],
    );
    check_error_deserialization::<StringStrict>(
        r#"-1"#,
        expect![[
            r#"invalid type: integer `-1`, expected a string containing a number at line 1 column 2"#
        ]],
    );

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct StringFlexible(#[serde_as(as = "DurationSeconds<String, Flexible>")] Duration);

    is_equal(StringFlexible(zero), expect![[r#""0""#]]);
    is_equal(StringFlexible(one_second), expect![[r#""1""#]]);
    check_serialization(StringFlexible(half_second), expect![[r#""1""#]]);
    check_deserialization(StringFlexible(half_second), r#""0.5""#);
    check_deserialization(StringFlexible(one_second), r#""1""#);
    check_deserialization(StringFlexible(zero), r#""0""#);
    check_error_deserialization::<StringFlexible>(
        r#""a""#,
        expect![[
            r#"invalid value: string "a", expected an integer, a float, or a string containing a number at line 1 column 3"#
        ]],
    );
    check_error_deserialization::<StringFlexible>(
        r#"-1"#,
        expect![[r#"std::time::Duration cannot be negative"#]],
    );
}

#[test]
fn test_duration_seconds_with_frac() {
    let zero = Duration::new(0, 0);
    let one_second = Duration::new(1, 0);
    let half_second = Duration::new(0, 500_000_000);

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct F64Strict(#[serde_as(as = "DurationSecondsWithFrac<f64>")] Duration);

    is_equal(F64Strict(zero), expect![[r#"0.0"#]]);
    is_equal(F64Strict(one_second), expect![[r#"1.0"#]]);
    is_equal(F64Strict(half_second), expect![[r#"0.5"#]]);
    check_error_deserialization::<F64Strict>(
        r#""1""#,
        expect![[r#"invalid type: string "1", expected f64 at line 1 column 3"#]],
    );
    check_error_deserialization::<F64Strict>(
        r#"-1.0"#,
        expect![[r#"std::time::Duration cannot be negative"#]],
    );

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct F64Flexible(#[serde_as(as = "DurationSecondsWithFrac<f64, Flexible>")] Duration);

    is_equal(F64Flexible(zero), expect![[r#"0.0"#]]);
    is_equal(F64Flexible(one_second), expect![[r#"1.0"#]]);
    is_equal(F64Flexible(half_second), expect![[r#"0.5"#]]);
    check_deserialization(F64Flexible(one_second), r#""1""#);
    check_deserialization(F64Flexible(zero), r#""0""#);
    check_error_deserialization::<F64Flexible>(
        r#""a""#,
        expect![[
            r#"invalid value: string "a", expected an integer, a float, or a string containing a number at line 1 column 3"#
        ]],
    );
    check_error_deserialization::<F64Flexible>(
        r#"-1"#,
        expect![[r#"std::time::Duration cannot be negative"#]],
    );

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct StringStrict(#[serde_as(as = "DurationSecondsWithFrac<String>")] Duration);

    is_equal(StringStrict(zero), expect![[r#""0""#]]);
    is_equal(StringStrict(one_second), expect![[r#""1""#]]);
    is_equal(StringStrict(half_second), expect![[r#""0.5""#]]);
    check_error_deserialization::<StringStrict>(
        r#"1"#,
        expect![[r#"invalid type: integer `1`, expected a string at line 1 column 1"#]],
    );
    check_error_deserialization::<StringStrict>(
        r#"-1"#,
        expect![[r#"invalid type: integer `-1`, expected a string at line 1 column 2"#]],
    );

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct StringFlexible(#[serde_as(as = "DurationSecondsWithFrac<String, Flexible>")] Duration);

    is_equal(StringFlexible(zero), expect![[r#""0""#]]);
    is_equal(StringFlexible(one_second), expect![[r#""1""#]]);
    is_equal(StringFlexible(half_second), expect![[r#""0.5""#]]);
    check_deserialization(StringFlexible(zero), r#""0""#);
    check_error_deserialization::<StringFlexible>(
        r#""a""#,
        expect![[
            r#"invalid value: string "a", expected an integer, a float, or a string containing a number at line 1 column 3"#
        ]],
    );
    check_error_deserialization::<StringFlexible>(
        r#"-1"#,
        expect![[r#"std::time::Duration cannot be negative"#]],
    );
}

#[test]
fn test_timestamp_seconds_systemtime() {
    let zero = SystemTime::UNIX_EPOCH;
    let one_second = SystemTime::UNIX_EPOCH
        .checked_add(Duration::new(1, 0))
        .unwrap();
    let half_second = SystemTime::UNIX_EPOCH
        .checked_add(Duration::new(0, 500_000_000))
        .unwrap();
    let minus_one_second = SystemTime::UNIX_EPOCH
        .checked_sub(Duration::new(1, 0))
        .unwrap();
    let minus_half_second = SystemTime::UNIX_EPOCH
        .checked_sub(Duration::new(0, 500_000_000))
        .unwrap();

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct StructIntStrict(#[serde_as(as = "TimestampSeconds")] SystemTime);

    is_equal(StructIntStrict(zero), expect![[r#"0"#]]);
    is_equal(StructIntStrict(one_second), expect![[r#"1"#]]);
    is_equal(StructIntStrict(minus_one_second), expect![[r#"-1"#]]);
    check_serialization(StructIntStrict(half_second), expect![[r#"1"#]]);
    check_serialization(StructIntStrict(minus_half_second), expect![[r#"-1"#]]);
    check_error_deserialization::<StructIntStrict>(
        r#""1""#,
        expect![[r#"invalid type: string "1", expected i64 at line 1 column 3"#]],
    );
    check_error_deserialization::<StructIntStrict>(
        r#"0.123"#,
        expect![[r#"invalid type: floating point `0.123`, expected i64 at line 1 column 5"#]],
    );

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct StructIntFlexible(#[serde_as(as = "TimestampSeconds<i64, Flexible>")] SystemTime);

    is_equal(StructIntFlexible(zero), expect![[r#"0"#]]);
    is_equal(StructIntFlexible(one_second), expect![[r#"1"#]]);
    is_equal(StructIntFlexible(minus_one_second), expect![[r#"-1"#]]);
    check_serialization(StructIntFlexible(half_second), expect![[r#"1"#]]);
    check_serialization(StructIntFlexible(minus_half_second), expect![[r#"-1"#]]);
    check_deserialization(StructIntFlexible(one_second), r#""1""#);
    check_deserialization(StructIntFlexible(one_second), r#"1.0"#);
    check_deserialization(StructIntFlexible(minus_half_second), r#""-0.5""#);
    check_deserialization(StructIntFlexible(half_second), r#"0.5"#);
    check_error_deserialization::<StructIntFlexible>(
        r#""a""#,
        expect![[
            r#"invalid value: string "a", expected an integer, a float, or a string containing a number at line 1 column 3"#
        ]],
    );

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct Structf64Strict(#[serde_as(as = "TimestampSeconds<f64>")] SystemTime);

    is_equal(Structf64Strict(zero), expect![[r#"0.0"#]]);
    is_equal(Structf64Strict(one_second), expect![[r#"1.0"#]]);
    is_equal(Structf64Strict(minus_one_second), expect![[r#"-1.0"#]]);
    check_serialization(Structf64Strict(half_second), expect![[r#"1.0"#]]);
    check_serialization(Structf64Strict(minus_half_second), expect![[r#"-1.0"#]]);
    check_deserialization(Structf64Strict(one_second), r#"0.5"#);
    check_error_deserialization::<Structf64Strict>(
        r#""1""#,
        expect![[r#"invalid type: string "1", expected f64 at line 1 column 3"#]],
    );

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct Structf64Flexible(#[serde_as(as = "TimestampSeconds<f64, Flexible>")] SystemTime);

    is_equal(Structf64Flexible(zero), expect![[r#"0.0"#]]);
    is_equal(Structf64Flexible(one_second), expect![[r#"1.0"#]]);
    is_equal(Structf64Flexible(minus_one_second), expect![[r#"-1.0"#]]);
    check_serialization(Structf64Flexible(half_second), expect![[r#"1.0"#]]);
    check_serialization(Structf64Flexible(minus_half_second), expect![[r#"-1.0"#]]);
    check_deserialization(Structf64Flexible(one_second), r#""1""#);
    check_deserialization(Structf64Flexible(one_second), r#"1.0"#);
    check_deserialization(Structf64Flexible(minus_half_second), r#""-0.5""#);
    check_deserialization(Structf64Flexible(half_second), r#"0.5"#);
    check_error_deserialization::<Structf64Flexible>(
        r#""a""#,
        expect![[
            r#"invalid value: string "a", expected an integer, a float, or a string containing a number at line 1 column 3"#
        ]],
    );

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct StructStringStrict(#[serde_as(as = "TimestampSeconds<String>")] SystemTime);

    is_equal(StructStringStrict(zero), expect![[r#""0""#]]);
    is_equal(StructStringStrict(one_second), expect![[r#""1""#]]);
    is_equal(StructStringStrict(minus_one_second), expect![[r#""-1""#]]);
    check_serialization(StructStringStrict(half_second), expect![[r#""1""#]]);
    check_serialization(StructStringStrict(minus_half_second), expect![[r#""-1""#]]);
    check_deserialization(StructStringStrict(one_second), r#""1""#);
    check_error_deserialization::<StructStringStrict>(
        r#""0.5""#,
        expect![[r#"invalid digit found in string at line 1 column 5"#]],
    );
    check_error_deserialization::<StructStringStrict>(
        r#""-0.5""#,
        expect![[r#"invalid digit found in string at line 1 column 6"#]],
    );
    check_error_deserialization::<StructStringStrict>(
        r#"1"#,
        expect![[
            r#"invalid type: integer `1`, expected a string containing a number at line 1 column 1"#
        ]],
    );
    check_error_deserialization::<StructStringStrict>(
        r#"0.1"#,
        expect![[
            r#"invalid type: floating point `0.1`, expected a string containing a number at line 1 column 3"#
        ]],
    );

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct StructStringFlexible(#[serde_as(as = "TimestampSeconds<String, Flexible>")] SystemTime);

    is_equal(StructStringFlexible(zero), expect![[r#""0""#]]);
    is_equal(StructStringFlexible(one_second), expect![[r#""1""#]]);
    is_equal(StructStringFlexible(minus_one_second), expect![[r#""-1""#]]);
    check_serialization(StructStringFlexible(half_second), expect![[r#""1""#]]);
    check_serialization(
        StructStringFlexible(minus_half_second),
        expect![[r#""-1""#]],
    );
    check_deserialization(StructStringFlexible(one_second), r#"1"#);
    check_deserialization(StructStringFlexible(one_second), r#"1.0"#);
    check_deserialization(StructStringFlexible(minus_half_second), r#""-0.5""#);
    check_deserialization(StructStringFlexible(half_second), r#"0.5"#);
    check_error_deserialization::<StructStringFlexible>(
        r#""a""#,
        expect![[
            r#"invalid value: string "a", expected an integer, a float, or a string containing a number at line 1 column 3"#
        ]],
    );
}

#[test]
fn test_timestamp_seconds_with_frac_systemtime() {
    let zero = SystemTime::UNIX_EPOCH;
    let one_second = SystemTime::UNIX_EPOCH
        .checked_add(Duration::new(1, 0))
        .unwrap();
    let half_second = SystemTime::UNIX_EPOCH
        .checked_add(Duration::new(0, 500_000_000))
        .unwrap();
    let minus_one_second = SystemTime::UNIX_EPOCH
        .checked_sub(Duration::new(1, 0))
        .unwrap();
    let minus_half_second = SystemTime::UNIX_EPOCH
        .checked_sub(Duration::new(0, 500_000_000))
        .unwrap();

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct Structf64Strict(#[serde_as(as = "TimestampSecondsWithFrac<f64>")] SystemTime);

    is_equal(Structf64Strict(zero), expect![[r#"0.0"#]]);
    is_equal(Structf64Strict(one_second), expect![[r#"1.0"#]]);
    is_equal(Structf64Strict(minus_one_second), expect![[r#"-1.0"#]]);
    is_equal(Structf64Strict(half_second), expect![[r#"0.5"#]]);
    is_equal(Structf64Strict(minus_half_second), expect![[r#"-0.5"#]]);
    check_error_deserialization::<Structf64Strict>(
        r#""1""#,
        expect![[r#"invalid type: string "1", expected f64 at line 1 column 3"#]],
    );

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct Structf64Flexible(
        #[serde_as(as = "TimestampSecondsWithFrac<f64, Flexible>")] SystemTime,
    );

    is_equal(Structf64Flexible(zero), expect![[r#"0.0"#]]);
    is_equal(Structf64Flexible(one_second), expect![[r#"1.0"#]]);
    is_equal(Structf64Flexible(minus_one_second), expect![[r#"-1.0"#]]);
    is_equal(Structf64Flexible(half_second), expect![[r#"0.5"#]]);
    is_equal(Structf64Flexible(minus_half_second), expect![[r#"-0.5"#]]);
    check_deserialization(Structf64Flexible(one_second), r#""1""#);
    check_deserialization(Structf64Flexible(one_second), r#"1.0"#);
    check_deserialization(Structf64Flexible(minus_half_second), r#""-0.5""#);
    check_deserialization(Structf64Flexible(half_second), r#"0.5"#);
    check_error_deserialization::<Structf64Flexible>(
        r#""a""#,
        expect![[
            r#"invalid value: string "a", expected an integer, a float, or a string containing a number at line 1 column 3"#
        ]],
    );

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct StructStringStrict(#[serde_as(as = "TimestampSecondsWithFrac<String>")] SystemTime);

    is_equal(StructStringStrict(zero), expect![[r#""0""#]]);
    is_equal(StructStringStrict(one_second), expect![[r#""1""#]]);
    is_equal(StructStringStrict(minus_one_second), expect![[r#""-1""#]]);
    is_equal(StructStringStrict(half_second), expect![[r#""0.5""#]]);
    is_equal(
        StructStringStrict(minus_half_second),
        expect![[r#""-0.5""#]],
    );
    check_error_deserialization::<StructStringStrict>(
        r#"1"#,
        expect![[r#"invalid type: integer `1`, expected a string at line 1 column 1"#]],
    );
    check_error_deserialization::<StructStringStrict>(
        r#"0.1"#,
        expect![[r#"invalid type: floating point `0.1`, expected a string at line 1 column 3"#]],
    );

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct StructStringFlexible(
        #[serde_as(as = "TimestampSecondsWithFrac<String, Flexible>")] SystemTime,
    );

    is_equal(StructStringFlexible(zero), expect![[r#""0""#]]);
    is_equal(StructStringFlexible(one_second), expect![[r#""1""#]]);
    is_equal(StructStringFlexible(minus_one_second), expect![[r#""-1""#]]);
    is_equal(StructStringFlexible(half_second), expect![[r#""0.5""#]]);
    is_equal(
        StructStringFlexible(minus_half_second),
        expect![[r#""-0.5""#]],
    );
    check_deserialization(StructStringFlexible(one_second), r#"1"#);
    check_deserialization(StructStringFlexible(one_second), r#"1.0"#);
    check_deserialization(StructStringFlexible(half_second), r#"0.5"#);
    check_error_deserialization::<StructStringFlexible>(
        r#""a""#,
        expect![[
            r#"invalid value: string "a", expected an integer, a float, or a string containing a number at line 1 column 3"#
        ]],
    );
}

macro_rules! smoketest {
    ($($valuety:ty, $adapter:literal, $value:ident, $expect:tt;)*) => {
        $({
            #[serde_as]
            #[derive(Debug, Serialize, Deserialize, PartialEq)]
            struct S(#[serde_as(as = $adapter)] $valuety);
            #[allow(unused_braces)]
            is_equal(S($value), $expect);
        })*
    };
}

#[test]
fn test_duration_smoketest() {
    let one_second = Duration::new(1, 0);

    smoketest! {
        Duration, "DurationSeconds<u64>", one_second, {expect![[r#"1"#]]};
        Duration, "DurationSeconds<f64>", one_second, {expect![[r#"1.0"#]]};
        Duration, "DurationMilliSeconds<u64>", one_second, {expect![[r#"1000"#]]};
        Duration, "DurationMilliSeconds<f64>", one_second, {expect![[r#"1000.0"#]]};
        Duration, "DurationMicroSeconds<u64>", one_second, {expect![[r#"1000000"#]]};
        Duration, "DurationMicroSeconds<f64>", one_second, {expect![[r#"1000000.0"#]]};
        Duration, "DurationNanoSeconds<u64>", one_second, {expect![[r#"1000000000"#]]};
        Duration, "DurationNanoSeconds<f64>", one_second, {expect![[r#"1000000000.0"#]]};
    };

    smoketest! {
        Duration, "DurationSecondsWithFrac", one_second, {expect![[r#"1.0"#]]};
        Duration, "DurationSecondsWithFrac<String>", one_second, {expect![[r#""1""#]]};
        Duration, "DurationMilliSecondsWithFrac", one_second, {expect![[r#"1000.0"#]]};
        Duration, "DurationMilliSecondsWithFrac<String>", one_second, {expect![[r#""1000""#]]};
        Duration, "DurationMicroSecondsWithFrac", one_second, {expect![[r#"1000000.0"#]]};
        Duration, "DurationMicroSecondsWithFrac<String>", one_second, {expect![[r#""1000000""#]]};
        Duration, "DurationNanoSecondsWithFrac", one_second, {expect![[r#"1000000000.0"#]]};
        Duration, "DurationNanoSecondsWithFrac<String>", one_second, {expect![[r#""1000000000""#]]};
    };
}

#[test]
fn test_timestamp_systemtime_smoketest() {
    let one_second = SystemTime::UNIX_EPOCH
        .checked_add(Duration::new(1, 0))
        .unwrap();

    smoketest! {
        SystemTime, "TimestampSeconds<i64>", one_second, {expect![[r#"1"#]]};
        SystemTime, "TimestampSeconds<f64>", one_second, {expect![[r#"1.0"#]]};
        SystemTime, "TimestampMilliSeconds<i64>", one_second, {expect![[r#"1000"#]]};
        SystemTime, "TimestampMilliSeconds<f64>", one_second, {expect![[r#"1000.0"#]]};
        SystemTime, "TimestampMicroSeconds<i64>", one_second, {expect![[r#"1000000"#]]};
        SystemTime, "TimestampMicroSeconds<f64>", one_second, {expect![[r#"1000000.0"#]]};
        SystemTime, "TimestampNanoSeconds<i64>", one_second, {expect![[r#"1000000000"#]]};
        SystemTime, "TimestampNanoSeconds<f64>", one_second, {expect![[r#"1000000000.0"#]]};
    };

    smoketest! {
        SystemTime, "TimestampSecondsWithFrac", one_second, {expect![[r#"1.0"#]]};
        SystemTime, "TimestampSecondsWithFrac<String>", one_second, {expect![[r#""1""#]]};
        SystemTime, "TimestampMilliSecondsWithFrac", one_second, {expect![[r#"1000.0"#]]};
        SystemTime, "TimestampMilliSecondsWithFrac<String>", one_second, {expect![[r#""1000""#]]};
        SystemTime, "TimestampMicroSecondsWithFrac", one_second, {expect![[r#"1000000.0"#]]};
        SystemTime, "TimestampMicroSecondsWithFrac<String>", one_second, {expect![[r#""1000000""#]]};
        SystemTime, "TimestampNanoSecondsWithFrac", one_second, {expect![[r#"1000000000.0"#]]};
        SystemTime, "TimestampNanoSecondsWithFrac<String>", one_second, {expect![[r#""1000000000""#]]};
    };
}
