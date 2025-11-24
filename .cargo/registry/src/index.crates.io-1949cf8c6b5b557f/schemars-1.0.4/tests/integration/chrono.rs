use crate::prelude::*;
use chrono04::{prelude::*, TimeDelta};

#[derive(JsonSchema, Serialize, Deserialize)]
struct ChronoTypes {
    weekday: Weekday,
    date_time: DateTime<Utc>,
    naive_date: NaiveDate,
    naive_date_time: NaiveDateTime,
    naive_time: NaiveTime,
    time_delta: TimeDelta,
}

#[test]
fn chrono() {
    test!(ChronoTypes).assert_snapshot();

    test!(Weekday)
        .assert_allows_ser_roundtrip([Weekday::Mon])
        .assert_matches_de_roundtrip(arbitrary_values());

    test!(DateTime<Utc>)
        .assert_allows_ser_roundtrip_default()
        // JSON Schema only allows dates with 4-digit years
        // .assert_allows_ser_roundtrip([DateTime::<Utc>::MIN_UTC, DateTime::<Utc>::MAX_UTC])
        .assert_matches_de_roundtrip(arbitrary_values());

    test!(NaiveDate)
        .assert_allows_ser_roundtrip_default()
        // JSON Schema only allows dates with 4-digit years
        // .assert_allows_ser_roundtrip([NaiveDate::MIN, NaiveDate::MAX])
        .assert_matches_de_roundtrip(arbitrary_values());

    test!(NaiveDateTime)
        .assert_allows_ser_roundtrip_default()
        // JSON Schema only allows dates with 4-digit years
        // .assert_allows_ser_roundtrip([NaiveDateTime::MIN, NaiveDateTime::MAX])
        .assert_matches_de_roundtrip(arbitrary_values_except(
            Value::is_string,
            "Custom format 'partial-date-time', so arbitrary strings technically allowed by schema",
        ));

    test!(NaiveTime)
        .assert_allows_ser_roundtrip_default()
        .assert_allows_ser_roundtrip([NaiveTime::MIN, NaiveDateTime::MAX.time()])
        .assert_matches_de_roundtrip(arbitrary_values_except(
            Value::is_string,
            "Custom format 'date-time', so arbitrary strings technically allowed by schema",
        ));

    test!(TimeDelta)
        .assert_allows_ser_roundtrip_default()
        .assert_allows_ser_roundtrip([TimeDelta::MIN, TimeDelta::MAX])
        .assert_rejects_de([json!([0, -1]), json!([0, 1_000_000_000])])
        .assert_matches_de_roundtrip(arbitrary_values());
}
