//! Test Cases

extern crate alloc;

mod utils;

use crate::utils::{
    check_deserialization, check_error_deserialization, check_serialization, is_equal,
};
use alloc::collections::BTreeMap;
use chrono_0_4::{DateTime, Duration, Local, NaiveDateTime, Utc};
use core::str::FromStr;
use expect_test::expect;
use serde::{Deserialize, Serialize};
use serde_with::{
    formats::Flexible, serde_as, DurationMicroSeconds, DurationMicroSecondsWithFrac,
    DurationMilliSeconds, DurationMilliSecondsWithFrac, DurationNanoSeconds,
    DurationNanoSecondsWithFrac, DurationSeconds, DurationSecondsWithFrac, TimestampMicroSeconds,
    TimestampMicroSecondsWithFrac, TimestampMilliSeconds, TimestampMilliSecondsWithFrac,
    TimestampNanoSeconds, TimestampNanoSecondsWithFrac, TimestampSeconds, TimestampSecondsWithFrac,
};

fn new_datetime(secs: i64, nsecs: u32) -> DateTime<Utc> {
    DateTime::from_timestamp(secs, nsecs).unwrap()
}

fn unix_epoch_local() -> DateTime<Local> {
    DateTime::from_timestamp(0, 0)
        .unwrap()
        .with_timezone(&Local)
}

fn unix_epoch_naive() -> NaiveDateTime {
    DateTime::from_timestamp(0, 0).unwrap().naive_utc()
}

#[test]
fn json_datetime_from_any_to_string_deserialization() {
    #[derive(Debug, PartialEq, Deserialize)]
    struct S(
        #[serde(with = "serde_with::chrono_0_4::datetime_utc_ts_seconds_from_any")] DateTime<Utc>,
    );

    // just integers
    check_deserialization(
        vec![
            S(new_datetime(1_478_563_200, 0)),
            S(new_datetime(0, 0)),
            S(new_datetime(-86000, 0)),
        ],
        r#"[
            1478563200,
            0,
            -86000
        ]"#,
    );

    // floats, shows precision errors in sub-second part
    check_deserialization(
        vec![
            S(new_datetime(1_478_563_200, 122_999_906)),
            S(new_datetime(0, 0)),
            S(new_datetime(-86000, 998_999_999)),
        ],
        r#"[
            1478563200.123,
            0.000,
            -86000.999
        ]"#,
    );

    // string representation of floats
    check_deserialization(
        vec![
            S(new_datetime(1_478_563_200, 123_000_000)),
            S(new_datetime(0, 0)),
            S(new_datetime(-86000, 999_000_000)),
        ],
        r#"[
            "1478563200.123",
            "0.000",
            "-86000.999"
        ]"#,
    );
}

#[test]
fn test_chrono_naive_date_time() {
    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    pub struct S(#[serde_as(as = "DateTime<Utc>")] NaiveDateTime);

    is_equal(
        S(NaiveDateTime::from_str("1994-11-05T08:15:30").unwrap()),
        expect![[r#""1994-11-05T08:15:30Z""#]],
    );
}

#[test]
fn test_chrono_option_naive_date_time() {
    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    pub struct S(#[serde_as(as = "Option<DateTime<Utc>>")] Option<NaiveDateTime>);

    is_equal(
        S(NaiveDateTime::from_str("1994-11-05T08:15:30").ok()),
        expect![[r#""1994-11-05T08:15:30Z""#]],
    );
}

#[test]
fn test_chrono_vec_option_naive_date_time() {
    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    pub struct S(#[serde_as(as = "Vec<Option<DateTime<Utc>>>")] Vec<Option<NaiveDateTime>>);

    is_equal(
        S(vec![
            NaiveDateTime::from_str("1994-11-05T08:15:30").ok(),
            NaiveDateTime::from_str("1994-11-05T08:15:31").ok(),
        ]),
        expect![[r#"
            [
              "1994-11-05T08:15:30Z",
              "1994-11-05T08:15:31Z"
            ]"#]],
    );
}

#[test]
fn test_chrono_btreemap_naive_date_time() {
    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    pub struct S(#[serde_as(as = "BTreeMap<_, DateTime<Utc>>")] BTreeMap<i32, NaiveDateTime>);

    is_equal(
        S(BTreeMap::from_iter(vec![
            (1, NaiveDateTime::from_str("1994-11-05T08:15:30").unwrap()),
            (2, NaiveDateTime::from_str("1994-11-05T08:15:31").unwrap()),
        ])),
        expect![[r#"
            {
              "1": "1994-11-05T08:15:30Z",
              "2": "1994-11-05T08:15:31Z"
            }"#]],
    );
}

#[test]
fn test_chrono_duration_seconds() {
    let zero = Duration::zero();
    let one_second = Duration::seconds(1);
    let half_second = Duration::nanoseconds(500_000_000);
    let minus_one_second = zero - one_second;
    let minus_half_second = zero - half_second;

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct StructIntStrict(#[serde_as(as = "DurationSeconds<i64>")] Duration);

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
        r#"9223372036854775808"#,
        expect![[
            r#"invalid value: integer `9223372036854775808`, expected i64 at line 1 column 19"#
        ]],
    );

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct StructIntFlexible(#[serde_as(as = "DurationSeconds<i64, Flexible>")] Duration);

    is_equal(StructIntFlexible(zero), expect![[r#"0"#]]);
    is_equal(StructIntFlexible(one_second), expect![[r#"1"#]]);
    check_serialization(StructIntFlexible(half_second), expect![[r#"1"#]]);
    check_serialization(StructIntFlexible(minus_half_second), expect![[r#"-1"#]]);
    check_deserialization(StructIntFlexible(half_second), r#""0.5""#);
    check_deserialization(StructIntFlexible(minus_half_second), r#""-0.5""#);
    check_deserialization(StructIntFlexible(one_second), r#""1""#);
    check_deserialization(StructIntFlexible(minus_one_second), r#""-1""#);
    check_deserialization(StructIntFlexible(zero), r#""0""#);
    check_error_deserialization::<StructIntFlexible>(
        r#""a""#,
        expect![[
            r#"invalid value: string "a", expected an integer, a float, or a string containing a number at line 1 column 3"#
        ]],
    );

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct Structf64Strict(#[serde_as(as = "DurationSeconds<f64>")] Duration);

    is_equal(Structf64Strict(zero), expect![[r#"0.0"#]]);
    is_equal(Structf64Strict(one_second), expect![[r#"1.0"#]]);
    is_equal(Structf64Strict(minus_one_second), expect![[r#"-1.0"#]]);
    check_serialization(Structf64Strict(half_second), expect![[r#"1.0"#]]);
    check_serialization(Structf64Strict(minus_half_second), expect![[r#"-1.0"#]]);
    check_deserialization(Structf64Strict(one_second), r#"0.5"#);
    check_deserialization(Structf64Strict(minus_one_second), r#"-0.5"#);
    check_error_deserialization::<Structf64Strict>(
        r#""1""#,
        expect![[r#"invalid type: string "1", expected f64 at line 1 column 3"#]],
    );

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct Structf64Flexible(#[serde_as(as = "DurationSeconds<f64, Flexible>")] Duration);

    is_equal(Structf64Flexible(zero), expect![[r#"0.0"#]]);
    is_equal(Structf64Flexible(one_second), expect![[r#"1.0"#]]);
    is_equal(Structf64Flexible(minus_one_second), expect![[r#"-1.0"#]]);
    check_serialization(Structf64Flexible(half_second), expect![[r#"1.0"#]]);
    check_serialization(Structf64Flexible(minus_half_second), expect![[r#"-1.0"#]]);
    check_deserialization(Structf64Flexible(half_second), r#""0.5""#);
    check_deserialization(Structf64Flexible(minus_half_second), r#""-0.5""#);
    check_deserialization(Structf64Flexible(one_second), r#""1""#);
    check_deserialization(Structf64Flexible(minus_one_second), r#""-1""#);
    check_deserialization(Structf64Flexible(zero), r#""0""#);
    check_error_deserialization::<Structf64Flexible>(
        r#""a""#,
        expect![[
            r#"invalid value: string "a", expected an integer, a float, or a string containing a number at line 1 column 3"#
        ]],
    );

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct StructStringStrict(#[serde_as(as = "DurationSeconds<String>")] Duration);

    is_equal(StructStringStrict(zero), expect![[r#""0""#]]);
    is_equal(StructStringStrict(one_second), expect![[r#""1""#]]);
    is_equal(StructStringStrict(minus_one_second), expect![[r#""-1""#]]);
    check_serialization(StructStringStrict(half_second), expect![[r#""1""#]]);
    check_serialization(StructStringStrict(minus_half_second), expect![[r#""-1""#]]);
    check_error_deserialization::<StructStringStrict>(
        r#"1"#,
        expect![[
            r#"invalid type: integer `1`, expected a string containing a number at line 1 column 1"#
        ]],
    );
    check_error_deserialization::<StructStringStrict>(
        r#"-1"#,
        expect![[
            r#"invalid type: integer `-1`, expected a string containing a number at line 1 column 2"#
        ]],
    );

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct StructStringFlexible(#[serde_as(as = "DurationSeconds<String, Flexible>")] Duration);

    is_equal(StructStringFlexible(zero), expect![[r#""0""#]]);
    is_equal(StructStringFlexible(one_second), expect![[r#""1""#]]);
    is_equal(StructStringFlexible(minus_one_second), expect![[r#""-1""#]]);
    check_serialization(StructStringFlexible(half_second), expect![[r#""1""#]]);
    check_deserialization(StructStringFlexible(half_second), r#""0.5""#);
    check_deserialization(StructStringFlexible(one_second), r#""1""#);
    check_deserialization(StructStringFlexible(zero), r#""0""#);
    check_error_deserialization::<StructStringFlexible>(
        r#""a""#,
        expect![[
            r#"invalid value: string "a", expected an integer, a float, or a string containing a number at line 1 column 3"#
        ]],
    );
}

#[test]
fn test_chrono_duration_seconds_with_frac() {
    let zero = Duration::zero();
    let one_second = Duration::seconds(1);
    let half_second = Duration::nanoseconds(500_000_000);
    let minus_one_second = zero - one_second;
    let minus_half_second = zero - half_second;

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct Structf64Strict(#[serde_as(as = "DurationSecondsWithFrac<f64>")] Duration);

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
    struct Structf64Flexible(#[serde_as(as = "DurationSecondsWithFrac<f64, Flexible>")] Duration);

    is_equal(Structf64Flexible(zero), expect![[r#"0.0"#]]);
    is_equal(Structf64Flexible(one_second), expect![[r#"1.0"#]]);
    is_equal(Structf64Flexible(minus_one_second), expect![[r#"-1.0"#]]);
    is_equal(Structf64Flexible(minus_half_second), expect![[r#"-0.5"#]]);
    check_deserialization(Structf64Flexible(one_second), r#""1""#);
    check_deserialization(Structf64Flexible(minus_one_second), r#""-1""#);
    check_deserialization(Structf64Flexible(half_second), r#""0.5""#);
    check_deserialization(Structf64Flexible(zero), r#""0""#);
    check_error_deserialization::<Structf64Flexible>(
        r#""a""#,
        expect![[
            r#"invalid value: string "a", expected an integer, a float, or a string containing a number at line 1 column 3"#
        ]],
    );

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct StructStringStrict(#[serde_as(as = "DurationSecondsWithFrac<String>")] Duration);

    is_equal(StructStringStrict(zero), expect![[r#""0""#]]);
    is_equal(StructStringStrict(one_second), expect![[r#""1""#]]);
    is_equal(StructStringStrict(minus_one_second), expect![[r#""-1""#]]);
    is_equal(StructStringStrict(half_second), expect![[r#""0.5""#]]);
    is_equal(
        StructStringStrict(minus_half_second),
        expect![[r#""-0.5""#]],
    );
    is_equal(
        StructStringStrict(minus_half_second),
        expect![[r#""-0.5""#]],
    );
    check_error_deserialization::<StructStringStrict>(
        r#"1"#,
        expect![[r#"invalid type: integer `1`, expected a string at line 1 column 1"#]],
    );
    check_error_deserialization::<StructStringStrict>(
        r#"-1"#,
        expect![[r#"invalid type: integer `-1`, expected a string at line 1 column 2"#]],
    );

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct StructStringFlexible(
        #[serde_as(as = "DurationSecondsWithFrac<String, Flexible>")] Duration,
    );

    is_equal(StructStringFlexible(zero), expect![[r#""0""#]]);
    is_equal(StructStringFlexible(one_second), expect![[r#""1""#]]);
    is_equal(StructStringFlexible(minus_one_second), expect![[r#""-1""#]]);
    is_equal(StructStringFlexible(half_second), expect![[r#""0.5""#]]);
    is_equal(
        StructStringFlexible(minus_half_second),
        expect![[r#""-0.5""#]],
    );
    check_deserialization(StructStringFlexible(one_second), r#""1""#);
    check_deserialization(StructStringFlexible(zero), r#""0""#);
    check_error_deserialization::<StructStringFlexible>(
        r#""a""#,
        expect![[
            r#"invalid value: string "a", expected an integer, a float, or a string containing a number at line 1 column 3"#
        ]],
    );
}

#[test]
fn test_chrono_timestamp_seconds() {
    let zero = new_datetime(0, 0);
    let one_second = zero + Duration::seconds(1);
    let half_second = zero + Duration::nanoseconds(500_000_000);
    let minus_one_second = zero - Duration::seconds(1);
    let minus_half_second = zero - Duration::nanoseconds(500_000_000);

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct StructIntStrict(#[serde_as(as = "TimestampSeconds")] DateTime<Utc>);

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
    struct StructIntFlexible(#[serde_as(as = "TimestampSeconds<i64, Flexible>")] DateTime<Utc>);

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
    struct Structf64Strict(#[serde_as(as = "TimestampSeconds<f64>")] DateTime<Utc>);

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
    struct Structf64Flexible(#[serde_as(as = "TimestampSeconds<f64, Flexible>")] DateTime<Utc>);

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
    struct StructStringStrict(#[serde_as(as = "TimestampSeconds<String>")] DateTime<Utc>);

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
    struct StructStringFlexible(
        #[serde_as(as = "TimestampSeconds<String, Flexible>")] DateTime<Utc>,
    );

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
fn test_chrono_timestamp_seconds_with_frac() {
    let zero = new_datetime(0, 0);
    let one_second = zero + Duration::seconds(1);
    let half_second = zero + Duration::nanoseconds(500_000_000);
    let minus_one_second = zero - Duration::seconds(1);
    let minus_half_second = zero - Duration::nanoseconds(500_000_000);

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct Structf64Strict(#[serde_as(as = "TimestampSecondsWithFrac<f64>")] DateTime<Utc>);

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
        #[serde_as(as = "TimestampSecondsWithFrac<f64, Flexible>")] DateTime<Utc>,
    );

    is_equal(Structf64Flexible(zero), expect![[r#"0.0"#]]);
    is_equal(Structf64Flexible(one_second), expect![[r#"1.0"#]]);
    is_equal(Structf64Flexible(minus_one_second), expect![[r#"-1.0"#]]);
    is_equal(Structf64Flexible(half_second), expect![[r#"0.5"#]]);
    is_equal(Structf64Flexible(minus_half_second), expect![[r#"-0.5"#]]);
    check_deserialization(Structf64Flexible(one_second), r#""1""#);
    check_deserialization(Structf64Flexible(minus_half_second), r#""-0.5""#);
    check_error_deserialization::<Structf64Flexible>(
        r#""a""#,
        expect![[
            r#"invalid value: string "a", expected an integer, a float, or a string containing a number at line 1 column 3"#
        ]],
    );

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct StructStringStrict(#[serde_as(as = "TimestampSecondsWithFrac<String>")] DateTime<Utc>);

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
        #[serde_as(as = "TimestampSecondsWithFrac<String, Flexible>")] DateTime<Utc>,
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
    ($($valuety:ty, $adapter:literal, $value:expr, $expect:tt;)*) => {
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
    let zero = Duration::seconds(0);
    let one_second = Duration::seconds(1);

    smoketest! {
        Duration, "DurationSeconds<i64>", one_second, {expect![[r#"1"#]]};
        Duration, "DurationSeconds<f64>", one_second, {expect![[r#"1.0"#]]};
        Duration, "DurationMilliSeconds<i64>", one_second, {expect![[r#"1000"#]]};
        Duration, "DurationMilliSeconds<f64>", one_second, {expect![[r#"1000.0"#]]};
        Duration, "DurationMicroSeconds<i64>", one_second, {expect![[r#"1000000"#]]};
        Duration, "DurationMicroSeconds<f64>", one_second, {expect![[r#"1000000.0"#]]};
        Duration, "DurationNanoSeconds<i64>", one_second, {expect![[r#"1000000000"#]]};
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

    smoketest! {
        Duration, "DurationSecondsWithFrac", zero, {expect![[r#"0.0"#]]};
        Duration, "DurationSecondsWithFrac", zero + Duration::nanoseconds(500_000_000), {expect![[r#"0.5"#]]};
        Duration, "DurationSecondsWithFrac", zero + Duration::seconds(1), {expect![[r#"1.0"#]]};
        Duration, "DurationSecondsWithFrac", zero - Duration::nanoseconds(500_000_000), {expect![[r#"-0.5"#]]};
        Duration, "DurationSecondsWithFrac", zero - Duration::seconds(1), {expect![[r#"-1.0"#]]};
    };
}

#[test]
fn test_datetime_utc_smoketest() {
    let zero = new_datetime(0, 0);
    let one_second = zero + Duration::seconds(1);

    smoketest! {
        DateTime<Utc>, "TimestampSeconds<i64>", one_second, {expect![[r#"1"#]]};
        DateTime<Utc>, "TimestampSeconds<f64>", one_second, {expect![[r#"1.0"#]]};
        DateTime<Utc>, "TimestampMilliSeconds<i64>", one_second, {expect![[r#"1000"#]]};
        DateTime<Utc>, "TimestampMilliSeconds<f64>", one_second, {expect![[r#"1000.0"#]]};
        DateTime<Utc>, "TimestampMicroSeconds<i64>", one_second, {expect![[r#"1000000"#]]};
        DateTime<Utc>, "TimestampMicroSeconds<f64>", one_second, {expect![[r#"1000000.0"#]]};
        DateTime<Utc>, "TimestampNanoSeconds<i64>", one_second, {expect![[r#"1000000000"#]]};
        DateTime<Utc>, "TimestampNanoSeconds<f64>", one_second, {expect![[r#"1000000000.0"#]]};
    };

    smoketest! {
        DateTime<Utc>, "TimestampSecondsWithFrac", one_second, {expect![[r#"1.0"#]]};
        DateTime<Utc>, "TimestampSecondsWithFrac<String>", one_second, {expect![[r#""1""#]]};
        DateTime<Utc>, "TimestampMilliSecondsWithFrac", one_second, {expect![[r#"1000.0"#]]};
        DateTime<Utc>, "TimestampMilliSecondsWithFrac<String>", one_second, {expect![[r#""1000""#]]};
        DateTime<Utc>, "TimestampMicroSecondsWithFrac", one_second, {expect![[r#"1000000.0"#]]};
        DateTime<Utc>, "TimestampMicroSecondsWithFrac<String>", one_second, {expect![[r#""1000000""#]]};
        DateTime<Utc>, "TimestampNanoSecondsWithFrac", one_second, {expect![[r#"1000000000.0"#]]};
        DateTime<Utc>, "TimestampNanoSecondsWithFrac<String>", one_second, {expect![[r#""1000000000""#]]};
    };

    smoketest! {
        DateTime<Utc>, "TimestampSecondsWithFrac", zero, {expect![[r#"0.0"#]]};
        DateTime<Utc>, "TimestampSecondsWithFrac", zero + Duration::nanoseconds(500_000_000), {expect![[r#"0.5"#]]};
        DateTime<Utc>, "TimestampSecondsWithFrac", zero + Duration::seconds(1), {expect![[r#"1.0"#]]};
        DateTime<Utc>, "TimestampSecondsWithFrac", zero - Duration::nanoseconds(500_000_000), {expect![[r#"-0.5"#]]};
        DateTime<Utc>, "TimestampSecondsWithFrac", zero - Duration::seconds(1), {expect![[r#"-1.0"#]]};
    };
}

#[test]
fn test_datetime_local_smoketest() {
    let zero = unix_epoch_local();
    let one_second = zero + Duration::seconds(1);

    smoketest! {
        DateTime<Local>, "TimestampSeconds<i64>", one_second, {expect![[r#"1"#]]};
        DateTime<Local>, "TimestampSeconds<f64>", one_second, {expect![[r#"1.0"#]]};
        DateTime<Local>, "TimestampMilliSeconds<i64>", one_second, {expect![[r#"1000"#]]};
        DateTime<Local>, "TimestampMilliSeconds<f64>", one_second, {expect![[r#"1000.0"#]]};
        DateTime<Local>, "TimestampMicroSeconds<i64>", one_second, {expect![[r#"1000000"#]]};
        DateTime<Local>, "TimestampMicroSeconds<f64>", one_second, {expect![[r#"1000000.0"#]]};
        DateTime<Local>, "TimestampNanoSeconds<i64>", one_second, {expect![[r#"1000000000"#]]};
        DateTime<Local>, "TimestampNanoSeconds<f64>", one_second, {expect![[r#"1000000000.0"#]]};
    };

    smoketest! {
        DateTime<Local>, "TimestampSecondsWithFrac", one_second, {expect![[r#"1.0"#]]};
        DateTime<Local>, "TimestampSecondsWithFrac<String>", one_second, {expect![[r#""1""#]]};
        DateTime<Local>, "TimestampMilliSecondsWithFrac", one_second, {expect![[r#"1000.0"#]]};
        DateTime<Local>, "TimestampMilliSecondsWithFrac<String>", one_second, {expect![[r#""1000""#]]};
        DateTime<Local>, "TimestampMicroSecondsWithFrac", one_second, {expect![[r#"1000000.0"#]]};
        DateTime<Local>, "TimestampMicroSecondsWithFrac<String>", one_second, {expect![[r#""1000000""#]]};
        DateTime<Local>, "TimestampNanoSecondsWithFrac", one_second, {expect![[r#"1000000000.0"#]]};
        DateTime<Local>, "TimestampNanoSecondsWithFrac<String>", one_second, {expect![[r#""1000000000""#]]};
    };

    smoketest! {
        DateTime<Local>, "TimestampSecondsWithFrac", zero, {expect![[r#"0.0"#]]};
        DateTime<Local>, "TimestampSecondsWithFrac", zero + Duration::nanoseconds(500_000_000), {expect![[r#"0.5"#]]};
        DateTime<Local>, "TimestampSecondsWithFrac", zero + Duration::seconds(1), {expect![[r#"1.0"#]]};
        DateTime<Local>, "TimestampSecondsWithFrac", zero - Duration::nanoseconds(500_000_000), {expect![[r#"-0.5"#]]};
        DateTime<Local>, "TimestampSecondsWithFrac", zero - Duration::seconds(1), {expect![[r#"-1.0"#]]};
    };
}

#[test]
fn test_naive_datetime_smoketest() {
    let zero = unix_epoch_naive();
    let one_second = zero + Duration::seconds(1);

    smoketest! {
        NaiveDateTime, "TimestampSeconds<i64>", one_second, {expect![[r#"1"#]]};
        NaiveDateTime, "TimestampSeconds<f64>", one_second, {expect![[r#"1.0"#]]};
        NaiveDateTime, "TimestampMilliSeconds<i64>", one_second, {expect![[r#"1000"#]]};
        NaiveDateTime, "TimestampMilliSeconds<f64>", one_second, {expect![[r#"1000.0"#]]};
        NaiveDateTime, "TimestampMicroSeconds<i64>", one_second, {expect![[r#"1000000"#]]};
        NaiveDateTime, "TimestampMicroSeconds<f64>", one_second, {expect![[r#"1000000.0"#]]};
        NaiveDateTime, "TimestampNanoSeconds<i64>", one_second, {expect![[r#"1000000000"#]]};
        NaiveDateTime, "TimestampNanoSeconds<f64>", one_second, {expect![[r#"1000000000.0"#]]};
    };

    smoketest! {
        NaiveDateTime, "TimestampSecondsWithFrac", one_second, {expect![[r#"1.0"#]]};
        NaiveDateTime, "TimestampSecondsWithFrac<String>", one_second, {expect![[r#""1""#]]};
        NaiveDateTime, "TimestampMilliSecondsWithFrac", one_second, {expect![[r#"1000.0"#]]};
        NaiveDateTime, "TimestampMilliSecondsWithFrac<String>", one_second, {expect![[r#""1000""#]]};
        NaiveDateTime, "TimestampMicroSecondsWithFrac", one_second, {expect![[r#"1000000.0"#]]};
        NaiveDateTime, "TimestampMicroSecondsWithFrac<String>", one_second, {expect![[r#""1000000""#]]};
        NaiveDateTime, "TimestampNanoSecondsWithFrac", one_second, {expect![[r#"1000000000.0"#]]};
        NaiveDateTime, "TimestampNanoSecondsWithFrac<String>", one_second, {expect![[r#""1000000000""#]]};
    };

    smoketest! {
        NaiveDateTime, "TimestampSecondsWithFrac", zero, {expect![[r#"0.0"#]]};
        NaiveDateTime, "TimestampSecondsWithFrac", zero + Duration::nanoseconds(500_000_000), {expect![[r#"0.5"#]]};
        NaiveDateTime, "TimestampSecondsWithFrac", zero + Duration::seconds(1), {expect![[r#"1.0"#]]};
        NaiveDateTime, "TimestampSecondsWithFrac", zero - Duration::nanoseconds(500_000_000), {expect![[r#"-0.5"#]]};
        NaiveDateTime, "TimestampSecondsWithFrac", zero - Duration::seconds(1), {expect![[r#"-1.0"#]]};
    };
}
