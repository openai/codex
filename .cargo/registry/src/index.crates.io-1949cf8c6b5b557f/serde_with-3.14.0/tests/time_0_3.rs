//! Test Cases

mod utils;

use crate::utils::{check_deserialization, check_error_deserialization, is_equal};
use expect_test::expect;
use serde::{Deserialize, Serialize};
use serde_with::{
    serde_as, DurationMicroSeconds, DurationMicroSecondsWithFrac, DurationMilliSeconds,
    DurationMilliSecondsWithFrac, DurationNanoSeconds, DurationNanoSecondsWithFrac,
    DurationSeconds, DurationSecondsWithFrac, TimestampMicroSeconds, TimestampMicroSecondsWithFrac,
    TimestampMilliSeconds, TimestampMilliSecondsWithFrac, TimestampNanoSeconds,
    TimestampNanoSecondsWithFrac, TimestampSeconds, TimestampSecondsWithFrac,
};
use time_0_3::{Duration, OffsetDateTime, PrimitiveDateTime, UtcOffset};

/// Create a [`PrimitiveDateTime`] for the Unix Epoch
fn unix_epoch_primitive() -> PrimitiveDateTime {
    PrimitiveDateTime::new(
        time_0_3::Date::from_ordinal_date(1970, 1).unwrap(),
        time_0_3::Time::from_hms_nano(0, 0, 0, 0).unwrap(),
    )
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
    let zero = OffsetDateTime::UNIX_EPOCH;
    let one_second = zero + Duration::seconds(1);

    smoketest! {
        OffsetDateTime, "TimestampSeconds<i64>", one_second, {expect![[r#"1"#]]};
        OffsetDateTime, "TimestampSeconds<f64>", one_second, {expect![[r#"1.0"#]]};
        OffsetDateTime, "TimestampMilliSeconds<i64>", one_second, {expect![[r#"1000"#]]};
        OffsetDateTime, "TimestampMilliSeconds<f64>", one_second, {expect![[r#"1000.0"#]]};
        OffsetDateTime, "TimestampMicroSeconds<i64>", one_second, {expect![[r#"1000000"#]]};
        OffsetDateTime, "TimestampMicroSeconds<f64>", one_second, {expect![[r#"1000000.0"#]]};
        OffsetDateTime, "TimestampNanoSeconds<i64>", one_second, {expect![[r#"1000000000"#]]};
        OffsetDateTime, "TimestampNanoSeconds<f64>", one_second, {expect![[r#"1000000000.0"#]]};
    };

    smoketest! {
        OffsetDateTime, "TimestampSecondsWithFrac", one_second, {expect![[r#"1.0"#]]};
        OffsetDateTime, "TimestampSecondsWithFrac<String>", one_second, {expect![[r#""1""#]]};
        OffsetDateTime, "TimestampMilliSecondsWithFrac", one_second, {expect![[r#"1000.0"#]]};
        OffsetDateTime, "TimestampMilliSecondsWithFrac<String>", one_second, {expect![[r#""1000""#]]};
        OffsetDateTime, "TimestampMicroSecondsWithFrac", one_second, {expect![[r#"1000000.0"#]]};
        OffsetDateTime, "TimestampMicroSecondsWithFrac<String>", one_second, {expect![[r#""1000000""#]]};
        OffsetDateTime, "TimestampNanoSecondsWithFrac", one_second, {expect![[r#"1000000000.0"#]]};
        OffsetDateTime, "TimestampNanoSecondsWithFrac<String>", one_second, {expect![[r#""1000000000""#]]};
    };

    smoketest! {
        OffsetDateTime, "TimestampSecondsWithFrac", zero, {expect![[r#"0.0"#]]};
        OffsetDateTime, "TimestampSecondsWithFrac", zero + Duration::nanoseconds(500_000_000), {expect![[r#"0.5"#]]};
        OffsetDateTime, "TimestampSecondsWithFrac", zero + Duration::seconds(1), {expect![[r#"1.0"#]]};
        OffsetDateTime, "TimestampSecondsWithFrac", zero - Duration::nanoseconds(500_000_000), {expect![[r#"-0.5"#]]};
        OffsetDateTime, "TimestampSecondsWithFrac", zero - Duration::seconds(1), {expect![[r#"-1.0"#]]};
    };
}

#[test]
fn test_naive_datetime_smoketest() {
    let zero = unix_epoch_primitive();
    let one_second = zero + Duration::seconds(1);

    smoketest! {
        PrimitiveDateTime, "TimestampSeconds<i64>", one_second, {expect![[r#"1"#]]};
        PrimitiveDateTime, "TimestampSeconds<f64>", one_second, {expect![[r#"1.0"#]]};
        PrimitiveDateTime, "TimestampMilliSeconds<i64>", one_second, {expect![[r#"1000"#]]};
        PrimitiveDateTime, "TimestampMilliSeconds<f64>", one_second, {expect![[r#"1000.0"#]]};
        PrimitiveDateTime, "TimestampMicroSeconds<i64>", one_second, {expect![[r#"1000000"#]]};
        PrimitiveDateTime, "TimestampMicroSeconds<f64>", one_second, {expect![[r#"1000000.0"#]]};
        PrimitiveDateTime, "TimestampNanoSeconds<i64>", one_second, {expect![[r#"1000000000"#]]};
        PrimitiveDateTime, "TimestampNanoSeconds<f64>", one_second, {expect![[r#"1000000000.0"#]]};
    };

    smoketest! {
        PrimitiveDateTime, "TimestampSecondsWithFrac", one_second, {expect![[r#"1.0"#]]};
        PrimitiveDateTime, "TimestampSecondsWithFrac<String>", one_second, {expect![[r#""1""#]]};
        PrimitiveDateTime, "TimestampMilliSecondsWithFrac", one_second, {expect![[r#"1000.0"#]]};
        PrimitiveDateTime, "TimestampMilliSecondsWithFrac<String>", one_second, {expect![[r#""1000""#]]};
        PrimitiveDateTime, "TimestampMicroSecondsWithFrac", one_second, {expect![[r#"1000000.0"#]]};
        PrimitiveDateTime, "TimestampMicroSecondsWithFrac<String>", one_second, {expect![[r#""1000000""#]]};
        PrimitiveDateTime, "TimestampNanoSecondsWithFrac", one_second, {expect![[r#"1000000000.0"#]]};
        PrimitiveDateTime, "TimestampNanoSecondsWithFrac<String>", one_second, {expect![[r#""1000000000""#]]};
    };

    smoketest! {
        PrimitiveDateTime, "TimestampSecondsWithFrac", zero, {expect![[r#"0.0"#]]};
        PrimitiveDateTime, "TimestampSecondsWithFrac", zero + Duration::nanoseconds(500_000_000), {expect![[r#"0.5"#]]};
        PrimitiveDateTime, "TimestampSecondsWithFrac", zero + Duration::seconds(1), {expect![[r#"1.0"#]]};
        PrimitiveDateTime, "TimestampSecondsWithFrac", zero - Duration::nanoseconds(500_000_000), {expect![[r#"-0.5"#]]};
        PrimitiveDateTime, "TimestampSecondsWithFrac", zero - Duration::seconds(1), {expect![[r#"-1.0"#]]};
    };
}

#[test]
fn test_offset_datetime_rfc2822() {
    #[serde_as]
    #[derive(Debug, PartialEq, Deserialize, Serialize)]
    struct S(#[serde_as(as = "time_0_3::format_description::well_known::Rfc2822")] OffsetDateTime);

    is_equal(
        S(OffsetDateTime::UNIX_EPOCH),
        expect![[r#""Thu, 01 Jan 1970 00:00:00 +0000""#]],
    );

    check_error_deserialization::<S>(
        r#""Foobar""#,
        expect!["the 'day' component could not be parsed at line 1 column 8"],
    );
    check_error_deserialization::<S>(
        r#""Fri, 2000""#,
        expect![[r#"a character literal was not valid at line 1 column 11"#]],
    );
}

#[test]
fn test_offset_datetime_rfc3339() {
    #[serde_as]
    #[derive(Debug, PartialEq, Deserialize, Serialize)]
    struct S(#[serde_as(as = "time_0_3::format_description::well_known::Rfc3339")] OffsetDateTime);

    is_equal(
        S(OffsetDateTime::UNIX_EPOCH),
        expect![[r#""1970-01-01T00:00:00Z""#]],
    );
    check_deserialization::<S>(
        S(
            OffsetDateTime::from_unix_timestamp_nanos(482_196_050_520_000_000)
                .unwrap()
                .to_offset(UtcOffset::from_hms(0, 0, 0).unwrap()),
        ),
        r#""1985-04-12T23:20:50.52Z""#,
    );
    check_deserialization::<S>(
        S(OffsetDateTime::from_unix_timestamp(851_042_397)
            .unwrap()
            .to_offset(UtcOffset::from_hms(-8, 0, 0).unwrap())),
        r#""1996-12-19T16:39:57-08:00""#,
    );
    check_deserialization::<S>(
        S(
            OffsetDateTime::from_unix_timestamp_nanos(662_687_999_999_999_999)
                .unwrap()
                .to_offset(UtcOffset::from_hms(0, 0, 0).unwrap()),
        ),
        r#""1990-12-31T23:59:60Z""#,
    );
    check_deserialization::<S>(
        S(
            OffsetDateTime::from_unix_timestamp_nanos(662_687_999_999_999_999)
                .unwrap()
                .to_offset(UtcOffset::from_hms(-8, 0, 0).unwrap()),
        ),
        r#""1990-12-31T15:59:60-08:00""#,
    );
    check_deserialization::<S>(
        S(
            OffsetDateTime::from_unix_timestamp_nanos(-1_041_337_172_130_000_000)
                .unwrap()
                .to_offset(UtcOffset::from_hms(0, 20, 0).unwrap()),
        ),
        r#""1937-01-01T12:00:27.87+00:20""#,
    );

    check_error_deserialization::<S>(
        r#""Foobar""#,
        expect![[r#"the 'year' component could not be parsed at line 1 column 8"#]],
    );
    check_error_deserialization::<S>(
        r#""2000-AA""#,
        expect![[r#"the 'month' component could not be parsed at line 1 column 9"#]],
    );
}

#[test]
fn test_offset_datetime_iso8601() {
    /// The default configuration for [`Iso8601`].
    const DEFAULT_CONFIG: time_0_3::format_description::well_known::iso8601::EncodedConfig =
        time_0_3::format_description::well_known::iso8601::Config::DEFAULT.encode();

    #[serde_as]
    #[derive(Debug, PartialEq, Deserialize, Serialize)]
    struct S(
        #[serde_as(as = "time_0_3::format_description::well_known::Iso8601<DEFAULT_CONFIG>")]
        OffsetDateTime,
    );

    is_equal(
        S(OffsetDateTime::UNIX_EPOCH),
        expect![[r#""1970-01-01T00:00:00.000000000Z""#]],
    );
    check_deserialization::<S>(
        S(
            OffsetDateTime::from_unix_timestamp_nanos(482_196_050_520_000_000)
                .unwrap()
                .to_offset(UtcOffset::from_hms(0, 0, 0).unwrap()),
        ),
        r#""1985-04-12T23:20:50.52Z""#,
    );
    check_deserialization::<S>(
        S(OffsetDateTime::from_unix_timestamp(851_042_397)
            .unwrap()
            .to_offset(UtcOffset::from_hms(-8, 0, 0).unwrap())),
        r#""1996-12-19T16:39:57-08:00""#,
    );
    check_deserialization::<S>(
        S(
            OffsetDateTime::from_unix_timestamp_nanos(-1_041_337_172_130_000_000)
                .unwrap()
                .to_offset(UtcOffset::from_hms(0, 20, 0).unwrap()),
        ),
        r#""1937-01-01T12:00:27.87+00:20""#,
    );

    check_error_deserialization::<S>(
        r#""Foobar""#,
        expect![[r#"the 'year' component could not be parsed at line 1 column 8"#]],
    );
    check_error_deserialization::<S>(
        r#""2000-AA""#,
        expect!["unexpected trailing characters; the end of input was expected at line 1 column 9"],
    );
}
