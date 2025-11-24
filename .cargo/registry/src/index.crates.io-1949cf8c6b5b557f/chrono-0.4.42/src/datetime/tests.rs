use super::DateTime;
use crate::naive::{NaiveDate, NaiveTime};
#[cfg(feature = "clock")]
use crate::offset::Local;
use crate::offset::{FixedOffset, Offset, TimeZone, Utc};
use crate::{Datelike, Days, MappedLocalTime, Months, NaiveDateTime, TimeDelta, Timelike, Weekday};

#[derive(Clone)]
struct DstTester;

impl DstTester {
    fn winter_offset() -> FixedOffset {
        FixedOffset::east_opt(8 * 60 * 60).unwrap()
    }
    fn summer_offset() -> FixedOffset {
        FixedOffset::east_opt(9 * 60 * 60).unwrap()
    }

    const TO_WINTER_MONTH_DAY: (u32, u32) = (4, 15);
    const TO_SUMMER_MONTH_DAY: (u32, u32) = (9, 15);

    fn transition_start_local() -> NaiveTime {
        NaiveTime::from_hms_opt(2, 0, 0).unwrap()
    }
}

impl TimeZone for DstTester {
    type Offset = FixedOffset;

    fn from_offset(_: &Self::Offset) -> Self {
        DstTester
    }

    fn offset_from_local_date(&self, _: &NaiveDate) -> crate::MappedLocalTime<Self::Offset> {
        unimplemented!()
    }

    fn offset_from_local_datetime(
        &self,
        local: &NaiveDateTime,
    ) -> crate::MappedLocalTime<Self::Offset> {
        let local_to_winter_transition_start = NaiveDate::from_ymd_opt(
            local.year(),
            DstTester::TO_WINTER_MONTH_DAY.0,
            DstTester::TO_WINTER_MONTH_DAY.1,
        )
        .unwrap()
        .and_time(DstTester::transition_start_local());

        let local_to_winter_transition_end = NaiveDate::from_ymd_opt(
            local.year(),
            DstTester::TO_WINTER_MONTH_DAY.0,
            DstTester::TO_WINTER_MONTH_DAY.1,
        )
        .unwrap()
        .and_time(DstTester::transition_start_local() - TimeDelta::try_hours(1).unwrap());

        let local_to_summer_transition_start = NaiveDate::from_ymd_opt(
            local.year(),
            DstTester::TO_SUMMER_MONTH_DAY.0,
            DstTester::TO_SUMMER_MONTH_DAY.1,
        )
        .unwrap()
        .and_time(DstTester::transition_start_local());

        let local_to_summer_transition_end = NaiveDate::from_ymd_opt(
            local.year(),
            DstTester::TO_SUMMER_MONTH_DAY.0,
            DstTester::TO_SUMMER_MONTH_DAY.1,
        )
        .unwrap()
        .and_time(DstTester::transition_start_local() + TimeDelta::try_hours(1).unwrap());

        if *local < local_to_winter_transition_end || *local >= local_to_summer_transition_end {
            MappedLocalTime::Single(DstTester::summer_offset())
        } else if *local >= local_to_winter_transition_start
            && *local < local_to_summer_transition_start
        {
            MappedLocalTime::Single(DstTester::winter_offset())
        } else if *local >= local_to_winter_transition_end
            && *local < local_to_winter_transition_start
        {
            MappedLocalTime::Ambiguous(DstTester::winter_offset(), DstTester::summer_offset())
        } else if *local >= local_to_summer_transition_start
            && *local < local_to_summer_transition_end
        {
            MappedLocalTime::None
        } else {
            panic!("Unexpected local time {local}")
        }
    }

    fn offset_from_utc_date(&self, _: &NaiveDate) -> Self::Offset {
        unimplemented!()
    }

    fn offset_from_utc_datetime(&self, utc: &NaiveDateTime) -> Self::Offset {
        let utc_to_winter_transition = NaiveDate::from_ymd_opt(
            utc.year(),
            DstTester::TO_WINTER_MONTH_DAY.0,
            DstTester::TO_WINTER_MONTH_DAY.1,
        )
        .unwrap()
        .and_time(DstTester::transition_start_local())
            - DstTester::summer_offset();

        let utc_to_summer_transition = NaiveDate::from_ymd_opt(
            utc.year(),
            DstTester::TO_SUMMER_MONTH_DAY.0,
            DstTester::TO_SUMMER_MONTH_DAY.1,
        )
        .unwrap()
        .and_time(DstTester::transition_start_local())
            - DstTester::winter_offset();

        if *utc < utc_to_winter_transition || *utc >= utc_to_summer_transition {
            DstTester::summer_offset()
        } else if *utc >= utc_to_winter_transition && *utc < utc_to_summer_transition {
            DstTester::winter_offset()
        } else {
            panic!("Unexpected utc time {utc}")
        }
    }
}

#[test]
fn test_datetime_from_timestamp_millis() {
    let valid_map = [
        (1662921288000, "2022-09-11 18:34:48.000000000"),
        (1662921288123, "2022-09-11 18:34:48.123000000"),
        (1662921287890, "2022-09-11 18:34:47.890000000"),
        (-2208936075000, "1900-01-01 14:38:45.000000000"),
        (0, "1970-01-01 00:00:00.000000000"),
        (119731017000, "1973-10-17 18:36:57.000000000"),
        (1234567890000, "2009-02-13 23:31:30.000000000"),
        (2034061609000, "2034-06-16 09:06:49.000000000"),
    ];

    for (timestamp_millis, _formatted) in valid_map.iter().copied() {
        let datetime = DateTime::from_timestamp_millis(timestamp_millis).unwrap();
        assert_eq!(timestamp_millis, datetime.timestamp_millis());
        #[cfg(feature = "alloc")]
        assert_eq!(datetime.format("%F %T%.9f").to_string(), _formatted);
    }

    let invalid = [i64::MAX, i64::MIN];

    for timestamp_millis in invalid.iter().copied() {
        let datetime = DateTime::from_timestamp_millis(timestamp_millis);
        assert!(datetime.is_none());
    }

    // Test that the result of `from_timestamp_millis` compares equal to
    // that of `from_timestamp_opt`.
    let secs_test = [0, 1, 2, 1000, 1234, 12345678, -1, -2, -1000, -12345678];
    for secs in secs_test.iter().cloned() {
        assert_eq!(
            DateTime::from_timestamp_millis(secs * 1000),
            DateTime::from_timestamp_secs(secs)
        );
    }
}

#[test]
fn test_datetime_from_timestamp_micros() {
    let valid_map = [
        (1662921288000000, "2022-09-11 18:34:48.000000000"),
        (1662921288123456, "2022-09-11 18:34:48.123456000"),
        (1662921287890000, "2022-09-11 18:34:47.890000000"),
        (-2208936075000000, "1900-01-01 14:38:45.000000000"),
        (0, "1970-01-01 00:00:00.000000000"),
        (119731017000000, "1973-10-17 18:36:57.000000000"),
        (1234567890000000, "2009-02-13 23:31:30.000000000"),
        (2034061609000000, "2034-06-16 09:06:49.000000000"),
    ];

    for (timestamp_micros, _formatted) in valid_map.iter().copied() {
        let datetime = DateTime::from_timestamp_micros(timestamp_micros).unwrap();
        assert_eq!(timestamp_micros, datetime.timestamp_micros());
        #[cfg(feature = "alloc")]
        assert_eq!(datetime.format("%F %T%.9f").to_string(), _formatted);
    }

    let invalid = [i64::MAX, i64::MIN];

    for timestamp_micros in invalid.iter().copied() {
        let datetime = DateTime::from_timestamp_micros(timestamp_micros);
        assert!(datetime.is_none());
    }

    // Test that the result of `TimeZone::timestamp_micros` compares equal to
    // that of `TimeZone::timestamp_opt`.
    let secs_test = [0, 1, 2, 1000, 1234, 12345678, -1, -2, -1000, -12345678];
    for secs in secs_test.iter().copied() {
        assert_eq!(
            DateTime::from_timestamp_micros(secs * 1_000_000),
            DateTime::from_timestamp_secs(secs)
        );
    }
}

#[test]
fn test_datetime_from_timestamp_nanos() {
    let valid_map = [
        (1662921288000000000, "2022-09-11 18:34:48.000000000"),
        (1662921288123456000, "2022-09-11 18:34:48.123456000"),
        (1662921288123456789, "2022-09-11 18:34:48.123456789"),
        (1662921287890000000, "2022-09-11 18:34:47.890000000"),
        (-2208936075000000000, "1900-01-01 14:38:45.000000000"),
        (-5337182663000000000, "1800-11-15 01:15:37.000000000"),
        (0, "1970-01-01 00:00:00.000000000"),
        (119731017000000000, "1973-10-17 18:36:57.000000000"),
        (1234567890000000000, "2009-02-13 23:31:30.000000000"),
        (2034061609000000000, "2034-06-16 09:06:49.000000000"),
    ];

    for (timestamp_nanos, _formatted) in valid_map.iter().copied() {
        let datetime = DateTime::from_timestamp_nanos(timestamp_nanos);
        assert_eq!(timestamp_nanos, datetime.timestamp_nanos_opt().unwrap());
        #[cfg(feature = "alloc")]
        assert_eq!(datetime.format("%F %T%.9f").to_string(), _formatted);
    }

    const A_BILLION: i64 = 1_000_000_000;
    // Maximum datetime in nanoseconds
    let maximum = "2262-04-11T23:47:16.854775804UTC";
    let parsed: DateTime<Utc> = maximum.parse().unwrap();
    let nanos = parsed.timestamp_nanos_opt().unwrap();
    assert_eq!(
        Some(DateTime::from_timestamp_nanos(nanos)),
        DateTime::from_timestamp(nanos / A_BILLION, (nanos % A_BILLION) as u32)
    );
    // Minimum datetime in nanoseconds
    let minimum = "1677-09-21T00:12:44.000000000UTC";
    let parsed: DateTime<Utc> = minimum.parse().unwrap();
    let nanos = parsed.timestamp_nanos_opt().unwrap();
    assert_eq!(
        Some(DateTime::from_timestamp_nanos(nanos)),
        DateTime::from_timestamp(nanos / A_BILLION, (nanos % A_BILLION) as u32)
    );

    // Test that the result of `TimeZone::timestamp_nanos` compares equal to
    // that of `TimeZone::timestamp_opt`.
    let secs_test = [0, 1, 2, 1000, 1234, 12345678, -1, -2, -1000, -12345678];
    for secs in secs_test.iter().copied() {
        assert_eq!(
            Some(DateTime::from_timestamp_nanos(secs * 1_000_000_000)),
            DateTime::from_timestamp_secs(secs)
        );
    }
}

#[test]
fn test_datetime_from_timestamp_secs() {
    let valid = [-2208936075, 0, 119731017, 1234567890, 2034061609];

    for timestamp_secs in valid.iter().copied() {
        let datetime = DateTime::from_timestamp_secs(timestamp_secs).unwrap();
        assert_eq!(timestamp_secs, datetime.timestamp());
        assert_eq!(DateTime::from_timestamp(timestamp_secs, 0).unwrap(), datetime);
    }
}

#[test]
fn test_datetime_from_timestamp() {
    let ymdhms = |y, m, d, h, n, s| {
        NaiveDate::from_ymd_opt(y, m, d).unwrap().and_hms_opt(h, n, s).unwrap().and_utc()
    };
    assert_eq!(DateTime::from_timestamp_secs(-1), Some(ymdhms(1969, 12, 31, 23, 59, 59)));
    assert_eq!(DateTime::from_timestamp_secs(0), Some(ymdhms(1970, 1, 1, 0, 0, 0)));
    assert_eq!(DateTime::from_timestamp_secs(1), Some(ymdhms(1970, 1, 1, 0, 0, 1)));
    assert_eq!(DateTime::from_timestamp_secs(1_000_000_000), Some(ymdhms(2001, 9, 9, 1, 46, 40)));
    assert_eq!(DateTime::from_timestamp_secs(0x7fffffff), Some(ymdhms(2038, 1, 19, 3, 14, 7)));
    assert_eq!(DateTime::from_timestamp_secs(i64::MIN), None);
    assert_eq!(DateTime::from_timestamp_secs(i64::MAX), None);
}

#[test]
fn test_datetime_timestamp() {
    let to_timestamp = |y, m, d, h, n, s| {
        NaiveDate::from_ymd_opt(y, m, d)
            .unwrap()
            .and_hms_opt(h, n, s)
            .unwrap()
            .and_utc()
            .timestamp()
    };
    assert_eq!(to_timestamp(1969, 12, 31, 23, 59, 59), -1);
    assert_eq!(to_timestamp(1970, 1, 1, 0, 0, 0), 0);
    assert_eq!(to_timestamp(1970, 1, 1, 0, 0, 1), 1);
    assert_eq!(to_timestamp(2001, 9, 9, 1, 46, 40), 1_000_000_000);
    assert_eq!(to_timestamp(2038, 1, 19, 3, 14, 7), 0x7fffffff);
}

#[test]
fn test_nanosecond_range() {
    const A_BILLION: i64 = 1_000_000_000;
    let maximum = "2262-04-11T23:47:16.854775804UTC";
    let parsed: DateTime<Utc> = maximum.parse().unwrap();
    let nanos = parsed.timestamp_nanos_opt().unwrap();
    assert_eq!(
        parsed,
        DateTime::<Utc>::from_timestamp(nanos / A_BILLION, (nanos % A_BILLION) as u32).unwrap()
    );

    let minimum = "1677-09-21T00:12:44.000000000UTC";
    let parsed: DateTime<Utc> = minimum.parse().unwrap();
    let nanos = parsed.timestamp_nanos_opt().unwrap();
    assert_eq!(
        parsed,
        DateTime::<Utc>::from_timestamp(nanos / A_BILLION, (nanos % A_BILLION) as u32).unwrap()
    );

    // Just beyond range
    let maximum = "2262-04-11T23:47:16.854775804UTC";
    let parsed: DateTime<Utc> = maximum.parse().unwrap();
    let beyond_max = parsed + TimeDelta::try_milliseconds(300).unwrap();
    assert!(beyond_max.timestamp_nanos_opt().is_none());

    // Far beyond range
    let maximum = "2262-04-11T23:47:16.854775804UTC";
    let parsed: DateTime<Utc> = maximum.parse().unwrap();
    let beyond_max = parsed + Days::new(365);
    assert!(beyond_max.timestamp_nanos_opt().is_none());
}

#[test]
fn test_datetime_add_days() {
    let est = FixedOffset::west_opt(5 * 60 * 60).unwrap();
    let kst = FixedOffset::east_opt(9 * 60 * 60).unwrap();

    assert_eq!(
        format!("{}", est.with_ymd_and_hms(2014, 5, 6, 7, 8, 9).unwrap() + Days::new(5)),
        "2014-05-11 07:08:09 -05:00"
    );
    assert_eq!(
        format!("{}", kst.with_ymd_and_hms(2014, 5, 6, 7, 8, 9).unwrap() + Days::new(5)),
        "2014-05-11 07:08:09 +09:00"
    );

    assert_eq!(
        format!("{}", est.with_ymd_and_hms(2014, 5, 6, 7, 8, 9).unwrap() + Days::new(35)),
        "2014-06-10 07:08:09 -05:00"
    );
    assert_eq!(
        format!("{}", kst.with_ymd_and_hms(2014, 5, 6, 7, 8, 9).unwrap() + Days::new(35)),
        "2014-06-10 07:08:09 +09:00"
    );

    assert_eq!(
        format!("{}", DstTester.with_ymd_and_hms(2014, 4, 6, 7, 8, 9).unwrap() + Days::new(5)),
        "2014-04-11 07:08:09 +09:00"
    );
    assert_eq!(
        format!("{}", DstTester.with_ymd_and_hms(2014, 4, 6, 7, 8, 9).unwrap() + Days::new(10)),
        "2014-04-16 07:08:09 +08:00"
    );

    assert_eq!(
        format!("{}", DstTester.with_ymd_and_hms(2014, 9, 6, 7, 8, 9).unwrap() + Days::new(5)),
        "2014-09-11 07:08:09 +08:00"
    );
    assert_eq!(
        format!("{}", DstTester.with_ymd_and_hms(2014, 9, 6, 7, 8, 9).unwrap() + Days::new(10)),
        "2014-09-16 07:08:09 +09:00"
    );
}

#[test]
fn test_datetime_sub_days() {
    let est = FixedOffset::west_opt(5 * 60 * 60).unwrap();
    let kst = FixedOffset::east_opt(9 * 60 * 60).unwrap();

    assert_eq!(
        format!("{}", est.with_ymd_and_hms(2014, 5, 6, 7, 8, 9).unwrap() - Days::new(5)),
        "2014-05-01 07:08:09 -05:00"
    );
    assert_eq!(
        format!("{}", kst.with_ymd_and_hms(2014, 5, 6, 7, 8, 9).unwrap() - Days::new(5)),
        "2014-05-01 07:08:09 +09:00"
    );

    assert_eq!(
        format!("{}", est.with_ymd_and_hms(2014, 5, 6, 7, 8, 9).unwrap() - Days::new(35)),
        "2014-04-01 07:08:09 -05:00"
    );
    assert_eq!(
        format!("{}", kst.with_ymd_and_hms(2014, 5, 6, 7, 8, 9).unwrap() - Days::new(35)),
        "2014-04-01 07:08:09 +09:00"
    );
}

#[test]
fn test_datetime_add_months() {
    let est = FixedOffset::west_opt(5 * 60 * 60).unwrap();
    let kst = FixedOffset::east_opt(9 * 60 * 60).unwrap();

    assert_eq!(
        format!("{}", est.with_ymd_and_hms(2014, 5, 6, 7, 8, 9).unwrap() + Months::new(1)),
        "2014-06-06 07:08:09 -05:00"
    );
    assert_eq!(
        format!("{}", kst.with_ymd_and_hms(2014, 5, 6, 7, 8, 9).unwrap() + Months::new(1)),
        "2014-06-06 07:08:09 +09:00"
    );

    assert_eq!(
        format!("{}", est.with_ymd_and_hms(2014, 5, 6, 7, 8, 9).unwrap() + Months::new(5)),
        "2014-10-06 07:08:09 -05:00"
    );
    assert_eq!(
        format!("{}", kst.with_ymd_and_hms(2014, 5, 6, 7, 8, 9).unwrap() + Months::new(5)),
        "2014-10-06 07:08:09 +09:00"
    );
}

#[test]
fn test_datetime_sub_months() {
    let est = FixedOffset::west_opt(5 * 60 * 60).unwrap();
    let kst = FixedOffset::east_opt(9 * 60 * 60).unwrap();

    assert_eq!(
        format!("{}", est.with_ymd_and_hms(2014, 5, 6, 7, 8, 9).unwrap() - Months::new(1)),
        "2014-04-06 07:08:09 -05:00"
    );
    assert_eq!(
        format!("{}", kst.with_ymd_and_hms(2014, 5, 6, 7, 8, 9).unwrap() - Months::new(1)),
        "2014-04-06 07:08:09 +09:00"
    );

    assert_eq!(
        format!("{}", est.with_ymd_and_hms(2014, 5, 6, 7, 8, 9).unwrap() - Months::new(5)),
        "2013-12-06 07:08:09 -05:00"
    );
    assert_eq!(
        format!("{}", kst.with_ymd_and_hms(2014, 5, 6, 7, 8, 9).unwrap() - Months::new(5)),
        "2013-12-06 07:08:09 +09:00"
    );
}

// local helper function to easily create a DateTime<FixedOffset>
#[allow(clippy::too_many_arguments)]
fn ymdhms(
    fixedoffset: &FixedOffset,
    year: i32,
    month: u32,
    day: u32,
    hour: u32,
    min: u32,
    sec: u32,
) -> DateTime<FixedOffset> {
    fixedoffset.with_ymd_and_hms(year, month, day, hour, min, sec).unwrap()
}

// local helper function to easily create a DateTime<FixedOffset>
#[allow(clippy::too_many_arguments)]
fn ymdhms_milli(
    fixedoffset: &FixedOffset,
    year: i32,
    month: u32,
    day: u32,
    hour: u32,
    min: u32,
    sec: u32,
    milli: u32,
) -> DateTime<FixedOffset> {
    fixedoffset
        .with_ymd_and_hms(year, month, day, hour, min, sec)
        .unwrap()
        .with_nanosecond(milli * 1_000_000)
        .unwrap()
}

// local helper function to easily create a DateTime<FixedOffset>
#[allow(clippy::too_many_arguments)]
#[cfg(feature = "alloc")]
fn ymdhms_micro(
    fixedoffset: &FixedOffset,
    year: i32,
    month: u32,
    day: u32,
    hour: u32,
    min: u32,
    sec: u32,
    micro: u32,
) -> DateTime<FixedOffset> {
    fixedoffset
        .with_ymd_and_hms(year, month, day, hour, min, sec)
        .unwrap()
        .with_nanosecond(micro * 1000)
        .unwrap()
}

// local helper function to easily create a DateTime<FixedOffset>
#[allow(clippy::too_many_arguments)]
#[cfg(feature = "alloc")]
fn ymdhms_nano(
    fixedoffset: &FixedOffset,
    year: i32,
    month: u32,
    day: u32,
    hour: u32,
    min: u32,
    sec: u32,
    nano: u32,
) -> DateTime<FixedOffset> {
    fixedoffset
        .with_ymd_and_hms(year, month, day, hour, min, sec)
        .unwrap()
        .with_nanosecond(nano)
        .unwrap()
}

// local helper function to easily create a DateTime<Utc>
#[cfg(feature = "alloc")]
fn ymdhms_utc(year: i32, month: u32, day: u32, hour: u32, min: u32, sec: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(year, month, day, hour, min, sec).unwrap()
}

// local helper function to easily create a DateTime<Utc>
fn ymdhms_milli_utc(
    year: i32,
    month: u32,
    day: u32,
    hour: u32,
    min: u32,
    sec: u32,
    milli: u32,
) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(year, month, day, hour, min, sec)
        .unwrap()
        .with_nanosecond(milli * 1_000_000)
        .unwrap()
}

#[test]
fn test_datetime_offset() {
    let est = FixedOffset::west_opt(5 * 60 * 60).unwrap();
    let edt = FixedOffset::west_opt(4 * 60 * 60).unwrap();
    let kst = FixedOffset::east_opt(9 * 60 * 60).unwrap();

    assert_eq!(
        format!("{}", Utc.with_ymd_and_hms(2014, 5, 6, 7, 8, 9).unwrap()),
        "2014-05-06 07:08:09 UTC"
    );
    assert_eq!(
        format!("{}", edt.with_ymd_and_hms(2014, 5, 6, 7, 8, 9).unwrap()),
        "2014-05-06 07:08:09 -04:00"
    );
    assert_eq!(
        format!("{}", kst.with_ymd_and_hms(2014, 5, 6, 7, 8, 9).unwrap()),
        "2014-05-06 07:08:09 +09:00"
    );
    assert_eq!(
        format!("{:?}", Utc.with_ymd_and_hms(2014, 5, 6, 7, 8, 9).unwrap()),
        "2014-05-06T07:08:09Z"
    );
    assert_eq!(
        format!("{:?}", edt.with_ymd_and_hms(2014, 5, 6, 7, 8, 9).unwrap()),
        "2014-05-06T07:08:09-04:00"
    );
    assert_eq!(
        format!("{:?}", kst.with_ymd_and_hms(2014, 5, 6, 7, 8, 9).unwrap()),
        "2014-05-06T07:08:09+09:00"
    );

    // edge cases
    assert_eq!(
        format!("{:?}", Utc.with_ymd_and_hms(2014, 5, 6, 0, 0, 0).unwrap()),
        "2014-05-06T00:00:00Z"
    );
    assert_eq!(
        format!("{:?}", edt.with_ymd_and_hms(2014, 5, 6, 0, 0, 0).unwrap()),
        "2014-05-06T00:00:00-04:00"
    );
    assert_eq!(
        format!("{:?}", kst.with_ymd_and_hms(2014, 5, 6, 0, 0, 0).unwrap()),
        "2014-05-06T00:00:00+09:00"
    );
    assert_eq!(
        format!("{:?}", Utc.with_ymd_and_hms(2014, 5, 6, 23, 59, 59).unwrap()),
        "2014-05-06T23:59:59Z"
    );
    assert_eq!(
        format!("{:?}", edt.with_ymd_and_hms(2014, 5, 6, 23, 59, 59).unwrap()),
        "2014-05-06T23:59:59-04:00"
    );
    assert_eq!(
        format!("{:?}", kst.with_ymd_and_hms(2014, 5, 6, 23, 59, 59).unwrap()),
        "2014-05-06T23:59:59+09:00"
    );

    let dt = Utc.with_ymd_and_hms(2014, 5, 6, 7, 8, 9).unwrap();
    assert_eq!(dt, edt.with_ymd_and_hms(2014, 5, 6, 3, 8, 9).unwrap());
    assert_eq!(
        dt + TimeDelta::try_seconds(3600 + 60 + 1).unwrap(),
        Utc.with_ymd_and_hms(2014, 5, 6, 8, 9, 10).unwrap()
    );
    assert_eq!(
        dt.signed_duration_since(edt.with_ymd_and_hms(2014, 5, 6, 10, 11, 12).unwrap()),
        TimeDelta::try_seconds(-7 * 3600 - 3 * 60 - 3).unwrap()
    );

    assert_eq!(*Utc.with_ymd_and_hms(2014, 5, 6, 7, 8, 9).unwrap().offset(), Utc);
    assert_eq!(*edt.with_ymd_and_hms(2014, 5, 6, 7, 8, 9).unwrap().offset(), edt);
    assert!(*edt.with_ymd_and_hms(2014, 5, 6, 7, 8, 9).unwrap().offset() != est);
}

#[test]
#[allow(clippy::needless_borrow, clippy::op_ref)]
fn signed_duration_since_autoref() {
    let dt1 = Utc.with_ymd_and_hms(2014, 5, 6, 7, 8, 9).unwrap();
    let dt2 = Utc.with_ymd_and_hms(2014, 3, 4, 5, 6, 7).unwrap();
    let diff1 = dt1.signed_duration_since(dt2); // Copy/consume
    #[allow(clippy::needless_borrows_for_generic_args)]
    let diff2 = dt2.signed_duration_since(&dt1); // Take by reference
    assert_eq!(diff1, -diff2);

    let diff1 = dt1 - &dt2; // We can choose to subtract rhs by reference
    let diff2 = dt2 - dt1; // Or consume rhs
    assert_eq!(diff1, -diff2);
}

#[test]
fn test_datetime_date_and_time() {
    let tz = FixedOffset::east_opt(5 * 60 * 60).unwrap();
    let d = tz.with_ymd_and_hms(2014, 5, 6, 7, 8, 9).unwrap();
    assert_eq!(d.time(), NaiveTime::from_hms_opt(7, 8, 9).unwrap());
    assert_eq!(d.date_naive(), NaiveDate::from_ymd_opt(2014, 5, 6).unwrap());

    let tz = FixedOffset::east_opt(4 * 60 * 60).unwrap();
    let d = tz.with_ymd_and_hms(2016, 5, 4, 3, 2, 1).unwrap();
    assert_eq!(d.time(), NaiveTime::from_hms_opt(3, 2, 1).unwrap());
    assert_eq!(d.date_naive(), NaiveDate::from_ymd_opt(2016, 5, 4).unwrap());

    let tz = FixedOffset::west_opt(13 * 60 * 60).unwrap();
    let d = tz.with_ymd_and_hms(2017, 8, 9, 12, 34, 56).unwrap();
    assert_eq!(d.time(), NaiveTime::from_hms_opt(12, 34, 56).unwrap());
    assert_eq!(d.date_naive(), NaiveDate::from_ymd_opt(2017, 8, 9).unwrap());

    let utc_d = Utc.with_ymd_and_hms(2017, 8, 9, 12, 34, 56).unwrap();
    assert!(utc_d < d);
}

#[test]
#[cfg(feature = "clock")]
fn test_datetime_with_timezone() {
    let local_now = Local::now();
    let utc_now = local_now.with_timezone(&Utc);
    let local_now2 = utc_now.with_timezone(&Local);
    assert_eq!(local_now, local_now2);
}

#[test]
#[cfg(feature = "alloc")]
fn test_datetime_rfc2822() {
    let edt = FixedOffset::east_opt(5 * 60 * 60).unwrap();

    // timezone 0
    assert_eq!(
        Utc.with_ymd_and_hms(2015, 2, 18, 23, 16, 9).unwrap().to_rfc2822(),
        "Wed, 18 Feb 2015 23:16:09 +0000"
    );
    assert_eq!(
        Utc.with_ymd_and_hms(2015, 2, 1, 23, 16, 9).unwrap().to_rfc2822(),
        "Sun, 1 Feb 2015 23:16:09 +0000"
    );
    // timezone +05
    assert_eq!(
        edt.from_local_datetime(
            &NaiveDate::from_ymd_opt(2015, 2, 18)
                .unwrap()
                .and_hms_milli_opt(23, 16, 9, 150)
                .unwrap()
        )
        .unwrap()
        .to_rfc2822(),
        "Wed, 18 Feb 2015 23:16:09 +0500"
    );
    assert_eq!(
        DateTime::parse_from_rfc2822("Wed, 18 Feb 2015 23:59:60 +0500"),
        Ok(edt
            .from_local_datetime(
                &NaiveDate::from_ymd_opt(2015, 2, 18)
                    .unwrap()
                    .and_hms_milli_opt(23, 59, 59, 1_000)
                    .unwrap()
            )
            .unwrap())
    );
    assert!(DateTime::parse_from_rfc2822("31 DEC 262143 23:59 -2359").is_err());
    assert_eq!(
        DateTime::parse_from_rfc3339("2015-02-18T23:59:60.234567+05:00"),
        Ok(edt
            .from_local_datetime(
                &NaiveDate::from_ymd_opt(2015, 2, 18)
                    .unwrap()
                    .and_hms_micro_opt(23, 59, 59, 1_234_567)
                    .unwrap()
            )
            .unwrap())
    );
    // seconds 60
    assert_eq!(
        edt.from_local_datetime(
            &NaiveDate::from_ymd_opt(2015, 2, 18)
                .unwrap()
                .and_hms_micro_opt(23, 59, 59, 1_234_567)
                .unwrap()
        )
        .unwrap()
        .to_rfc2822(),
        "Wed, 18 Feb 2015 23:59:60 +0500"
    );

    assert_eq!(
        DateTime::parse_from_rfc2822("Wed, 18 Feb 2015 23:16:09 +0000"),
        Ok(FixedOffset::east_opt(0).unwrap().with_ymd_and_hms(2015, 2, 18, 23, 16, 9).unwrap())
    );
    assert_eq!(
        DateTime::parse_from_rfc2822("Wed, 18 Feb 2015 23:16:09 -0000"),
        Ok(FixedOffset::east_opt(0).unwrap().with_ymd_and_hms(2015, 2, 18, 23, 16, 9).unwrap())
    );
    assert_eq!(
        ymdhms_micro(&edt, 2015, 2, 18, 23, 59, 59, 1_234_567).to_rfc2822(),
        "Wed, 18 Feb 2015 23:59:60 +0500"
    );
    assert_eq!(
        DateTime::parse_from_rfc2822("Wed, 18 Feb 2015 23:59:58 +0500"),
        Ok(ymdhms(&edt, 2015, 2, 18, 23, 59, 58))
    );
    assert_ne!(
        DateTime::parse_from_rfc2822("Wed, 18 Feb 2015 23:59:58 +0500"),
        Ok(ymdhms_milli(&edt, 2015, 2, 18, 23, 59, 58, 500))
    );

    // many varying whitespace intermixed
    assert_eq!(
        DateTime::parse_from_rfc2822(
            "\t\t\tWed,\n\t\t18 \r\n\t\tFeb \u{3000} 2015\r\n\t\t\t23:59:58    \t+0500"
        ),
        Ok(ymdhms(&edt, 2015, 2, 18, 23, 59, 58))
    );
    // example from RFC 2822 Appendix A.5.
    assert_eq!(
        DateTime::parse_from_rfc2822(
            "Thu,\n\t13\n      Feb\n        1969\n    23:32\n             -0330 (Newfoundland Time)"
        ),
        Ok(
            ymdhms(&FixedOffset::east_opt(-3 * 60 * 60 - 30 * 60).unwrap(), 1969, 2, 13, 23, 32, 0,)
        )
    );
    // example from RFC 2822 Appendix A.5. without trailing " (Newfoundland Time)"
    assert_eq!(
        DateTime::parse_from_rfc2822(
            "Thu,\n\t13\n      Feb\n        1969\n    23:32\n             -0330"
        ),
        Ok(
            ymdhms(&FixedOffset::east_opt(-3 * 60 * 60 - 30 * 60).unwrap(), 1969, 2, 13, 23, 32, 0,)
        )
    );

    // bad year
    assert!(DateTime::parse_from_rfc2822("31 DEC 262143 23:59 -2359").is_err());
    // wrong format
    assert!(DateTime::parse_from_rfc2822("Wed, 18 Feb 2015 23:16:09 +00:00").is_err());
    // full name day of week
    assert!(DateTime::parse_from_rfc2822("Wednesday, 18 Feb 2015 23:16:09 +0000").is_err());
    // full name day of week
    assert!(DateTime::parse_from_rfc2822("Wednesday 18 Feb 2015 23:16:09 +0000").is_err());
    // wrong day of week separator '.'
    assert!(DateTime::parse_from_rfc2822("Wed. 18 Feb 2015 23:16:09 +0000").is_err());
    // *trailing* space causes failure
    assert!(DateTime::parse_from_rfc2822("Wed, 18 Feb 2015 23:16:09 +0000   ").is_err());
}

#[test]
#[cfg(feature = "alloc")]
fn test_datetime_rfc3339() {
    let edt5 = FixedOffset::east_opt(5 * 60 * 60).unwrap();
    let edt0 = FixedOffset::east_opt(0).unwrap();

    // timezone 0
    assert_eq!(
        Utc.with_ymd_and_hms(2015, 2, 18, 23, 16, 9).unwrap().to_rfc3339(),
        "2015-02-18T23:16:09+00:00"
    );
    // timezone +05
    assert_eq!(
        edt5.from_local_datetime(
            &NaiveDate::from_ymd_opt(2015, 2, 18)
                .unwrap()
                .and_hms_milli_opt(23, 16, 9, 150)
                .unwrap()
        )
        .unwrap()
        .to_rfc3339(),
        "2015-02-18T23:16:09.150+05:00"
    );

    assert_eq!(ymdhms_utc(2015, 2, 18, 23, 16, 9).to_rfc3339(), "2015-02-18T23:16:09+00:00");
    assert_eq!(
        ymdhms_milli(&edt5, 2015, 2, 18, 23, 16, 9, 150).to_rfc3339(),
        "2015-02-18T23:16:09.150+05:00"
    );
    assert_eq!(
        ymdhms_micro(&edt5, 2015, 2, 18, 23, 59, 59, 1_234_567).to_rfc3339(),
        "2015-02-18T23:59:60.234567+05:00"
    );
    assert_eq!(
        DateTime::parse_from_rfc3339("2015-02-18T23:59:59.123+05:00"),
        Ok(ymdhms_micro(&edt5, 2015, 2, 18, 23, 59, 59, 123_000))
    );
    assert_eq!(
        DateTime::parse_from_rfc3339("2015-02-18T23:59:59.123456+05:00"),
        Ok(ymdhms_micro(&edt5, 2015, 2, 18, 23, 59, 59, 123_456))
    );
    assert_eq!(
        DateTime::parse_from_rfc3339("2015-02-18T23:59:59.123456789+05:00"),
        Ok(ymdhms_nano(&edt5, 2015, 2, 18, 23, 59, 59, 123_456_789))
    );
    assert_eq!(
        DateTime::parse_from_rfc3339("2015-02-18T23:16:09Z"),
        Ok(ymdhms(&edt0, 2015, 2, 18, 23, 16, 9))
    );

    assert_eq!(
        ymdhms_micro(&edt5, 2015, 2, 18, 23, 59, 59, 1_234_567).to_rfc3339(),
        "2015-02-18T23:59:60.234567+05:00"
    );
    assert_eq!(
        ymdhms_milli(&edt5, 2015, 2, 18, 23, 16, 9, 150).to_rfc3339(),
        "2015-02-18T23:16:09.150+05:00"
    );
    assert_eq!(
        DateTime::parse_from_rfc3339("2015-02-18T00:00:00.234567+05:00"),
        Ok(ymdhms_micro(&edt5, 2015, 2, 18, 0, 0, 0, 234_567))
    );
    assert_eq!(
        DateTime::parse_from_rfc3339("2015-02-18T23:16:09Z"),
        Ok(ymdhms(&edt0, 2015, 2, 18, 23, 16, 9))
    );
    assert_eq!(
        DateTime::parse_from_rfc3339("2015-02-18 23:59:60.234567+05:00"),
        Ok(ymdhms_micro(&edt5, 2015, 2, 18, 23, 59, 59, 1_234_567))
    );
    assert_eq!(ymdhms_utc(2015, 2, 18, 23, 16, 9).to_rfc3339(), "2015-02-18T23:16:09+00:00");

    assert!(DateTime::parse_from_rfc3339("2015-02-18T23:59:60.234567 +05:00").is_err());
    assert!(DateTime::parse_from_rfc3339("2015-02-18T23:059:60.234567+05:00").is_err());
    assert!(DateTime::parse_from_rfc3339("2015-02-18T23:59:60.234567+05:00PST").is_err());
    assert!(DateTime::parse_from_rfc3339("2015-02-18T23:59:60.234567+PST").is_err());
    assert!(DateTime::parse_from_rfc3339("2015-02-18T23:59:60.234567PST").is_err());
    assert!(DateTime::parse_from_rfc3339("2015-02-18T23:59:60.234567+0500").is_err());
    assert!(DateTime::parse_from_rfc3339("2015-02-18T23:59:60.234567+05:00:00").is_err());
    assert!(DateTime::parse_from_rfc3339("2015-02-18T23:59:60.234567:+05:00").is_err());
    assert!(DateTime::parse_from_rfc3339("2015-02-18T23:59:60.234567+05:00 ").is_err());
    assert!(DateTime::parse_from_rfc3339(" 2015-02-18T23:59:60.234567+05:00").is_err());
    assert!(DateTime::parse_from_rfc3339("2015- 02-18T23:59:60.234567+05:00").is_err());
    assert!(DateTime::parse_from_rfc3339("2015-02-18T23:59:60.234567A+05:00").is_err());
}

#[test]
#[cfg(feature = "alloc")]
fn test_rfc3339_opts() {
    use crate::SecondsFormat::*;
    let pst = FixedOffset::east_opt(8 * 60 * 60).unwrap();
    let dt = pst
        .from_local_datetime(
            &NaiveDate::from_ymd_opt(2018, 1, 11)
                .unwrap()
                .and_hms_nano_opt(10, 5, 13, 84_660_000)
                .unwrap(),
        )
        .unwrap();
    assert_eq!(dt.to_rfc3339_opts(Secs, false), "2018-01-11T10:05:13+08:00");
    assert_eq!(dt.to_rfc3339_opts(Secs, true), "2018-01-11T10:05:13+08:00");
    assert_eq!(dt.to_rfc3339_opts(Millis, false), "2018-01-11T10:05:13.084+08:00");
    assert_eq!(dt.to_rfc3339_opts(Micros, false), "2018-01-11T10:05:13.084660+08:00");
    assert_eq!(dt.to_rfc3339_opts(Nanos, false), "2018-01-11T10:05:13.084660000+08:00");
    assert_eq!(dt.to_rfc3339_opts(AutoSi, false), "2018-01-11T10:05:13.084660+08:00");

    let ut = dt.naive_utc().and_utc();
    assert_eq!(ut.to_rfc3339_opts(Secs, false), "2018-01-11T02:05:13+00:00");
    assert_eq!(ut.to_rfc3339_opts(Secs, true), "2018-01-11T02:05:13Z");
    assert_eq!(ut.to_rfc3339_opts(Millis, false), "2018-01-11T02:05:13.084+00:00");
    assert_eq!(ut.to_rfc3339_opts(Millis, true), "2018-01-11T02:05:13.084Z");
    assert_eq!(ut.to_rfc3339_opts(Micros, true), "2018-01-11T02:05:13.084660Z");
    assert_eq!(ut.to_rfc3339_opts(Nanos, true), "2018-01-11T02:05:13.084660000Z");
    assert_eq!(ut.to_rfc3339_opts(AutoSi, true), "2018-01-11T02:05:13.084660Z");
}

#[test]
#[should_panic]
#[cfg(feature = "alloc")]
fn test_rfc3339_opts_nonexhaustive() {
    use crate::SecondsFormat;
    let dt = Utc.with_ymd_and_hms(1999, 10, 9, 1, 2, 3).unwrap();
    let _ = dt.to_rfc3339_opts(SecondsFormat::__NonExhaustive, true);
}

#[test]
fn test_datetime_from_str() {
    assert_eq!(
        "2015-02-18T23:16:9.15Z".parse::<DateTime<FixedOffset>>(),
        Ok(FixedOffset::east_opt(0)
            .unwrap()
            .from_local_datetime(
                &NaiveDate::from_ymd_opt(2015, 2, 18)
                    .unwrap()
                    .and_hms_milli_opt(23, 16, 9, 150)
                    .unwrap()
            )
            .unwrap())
    );
    assert_eq!(
        "2015-02-18T23:16:9.15Z".parse::<DateTime<Utc>>(),
        Ok(Utc
            .from_local_datetime(
                &NaiveDate::from_ymd_opt(2015, 2, 18)
                    .unwrap()
                    .and_hms_milli_opt(23, 16, 9, 150)
                    .unwrap()
            )
            .unwrap())
    );
    assert_eq!(
        "2015-02-18T23:16:9.15 UTC".parse::<DateTime<Utc>>(),
        Ok(Utc
            .from_local_datetime(
                &NaiveDate::from_ymd_opt(2015, 2, 18)
                    .unwrap()
                    .and_hms_milli_opt(23, 16, 9, 150)
                    .unwrap()
            )
            .unwrap())
    );
    assert_eq!(
        "2015-02-18T23:16:9.15UTC".parse::<DateTime<Utc>>(),
        Ok(Utc
            .from_local_datetime(
                &NaiveDate::from_ymd_opt(2015, 2, 18)
                    .unwrap()
                    .and_hms_milli_opt(23, 16, 9, 150)
                    .unwrap()
            )
            .unwrap())
    );
    assert_eq!(
        "2015-02-18T23:16:9.15Utc".parse::<DateTime<Utc>>(),
        Ok(Utc
            .from_local_datetime(
                &NaiveDate::from_ymd_opt(2015, 2, 18)
                    .unwrap()
                    .and_hms_milli_opt(23, 16, 9, 150)
                    .unwrap()
            )
            .unwrap())
    );

    assert_eq!(
        "2015-2-18T23:16:9.15Z".parse::<DateTime<FixedOffset>>(),
        Ok(FixedOffset::east_opt(0)
            .unwrap()
            .from_local_datetime(
                &NaiveDate::from_ymd_opt(2015, 2, 18)
                    .unwrap()
                    .and_hms_milli_opt(23, 16, 9, 150)
                    .unwrap()
            )
            .unwrap())
    );
    assert_eq!(
        "2015-2-18T13:16:9.15-10:00".parse::<DateTime<FixedOffset>>(),
        Ok(FixedOffset::west_opt(10 * 3600)
            .unwrap()
            .from_local_datetime(
                &NaiveDate::from_ymd_opt(2015, 2, 18)
                    .unwrap()
                    .and_hms_milli_opt(13, 16, 9, 150)
                    .unwrap()
            )
            .unwrap())
    );
    assert!("2015-2-18T23:16:9.15".parse::<DateTime<FixedOffset>>().is_err());

    assert_eq!(
        "2015-2-18T23:16:9.15Z".parse::<DateTime<Utc>>(),
        Ok(Utc
            .from_local_datetime(
                &NaiveDate::from_ymd_opt(2015, 2, 18)
                    .unwrap()
                    .and_hms_milli_opt(23, 16, 9, 150)
                    .unwrap()
            )
            .unwrap())
    );
    assert_eq!(
        "2015-2-18T13:16:9.15-10:00".parse::<DateTime<Utc>>(),
        Ok(Utc
            .from_local_datetime(
                &NaiveDate::from_ymd_opt(2015, 2, 18)
                    .unwrap()
                    .and_hms_milli_opt(23, 16, 9, 150)
                    .unwrap()
            )
            .unwrap())
    );
    assert!("2015-2-18T23:16:9.15".parse::<DateTime<Utc>>().is_err());
    assert!("2015-02-18T23:16:9.15øøø".parse::<DateTime<Utc>>().is_err());

    // no test for `DateTime<Local>`, we cannot verify that much.
}

#[test]
fn test_parse_datetime_utc() {
    // valid cases
    let valid = [
        "2001-02-03T04:05:06Z",
        "2001-02-03T04:05:06+0000",
        "2001-02-03T04:05:06-00:00",
        "2001-02-03T04:05:06-01:00",
        "2012-12-12 12:12:12Z",
        "2012-12-12t12:12:12Z",
        "2012-12-12T12:12:12Z",
        "2012 -12-12T12:12:12Z",
        "2012  -12-12T12:12:12Z",
        "2012- 12-12T12:12:12Z",
        "2012-  12-12T12:12:12Z",
        "2012-12-12T 12:12:12Z",
        "2012-12-12T12 :12:12Z",
        "2012-12-12T12  :12:12Z",
        "2012-12-12T12: 12:12Z",
        "2012-12-12T12:  12:12Z",
        "2012-12-12T12 : 12:12Z",
        "2012-12-12T12:12:12Z ",
        " 2012-12-12T12:12:12Z",
        "2015-02-18T23:16:09.153Z",
        "2015-2-18T23:16:09.153Z",
        "+2015-2-18T23:16:09.153Z",
        "-77-02-18T23:16:09Z",
        "+82701-05-6T15:9:60.898989898989Z",
    ];
    for &s in &valid {
        eprintln!("test_parse_datetime_utc valid {s:?}");
        let d = match s.parse::<DateTime<Utc>>() {
            Ok(d) => d,
            Err(e) => panic!("parsing `{s}` has failed: {e}"),
        };
        let s_ = format!("{d:?}");
        // `s` and `s_` may differ, but `s.parse()` and `s_.parse()` must be same
        let d_ = match s_.parse::<DateTime<Utc>>() {
            Ok(d) => d,
            Err(e) => {
                panic!("`{s}` is parsed into `{d:?}`, but reparsing that has failed: {e}")
            }
        };
        assert!(
            d == d_,
            "`{s}` is parsed into `{d:?}`, but reparsed result `{d_:?}` does not match"
        );
    }

    // some invalid cases
    // since `ParseErrorKind` is private, all we can do is to check if there was an error
    let invalid = [
        "",                                                          // empty
        "Z",                                                         // missing data
        "15Z",                                                       // missing data
        "15:8:9Z",                                                   // missing date
        "15-8-9Z",                                                   // missing time or date
        "Fri, 09 Aug 2013 23:54:35 GMT",                             // valid datetime, wrong format
        "Sat Jun 30 23:59:60 2012",                                  // valid datetime, wrong format
        "1441497364.649",                                            // valid datetime, wrong format
        "+1441497364.649",                                           // valid datetime, wrong format
        "+1441497364",                                               // valid datetime, wrong format
        "+1441497364Z",                                              // valid datetime, wrong format
        "2014/02/03 04:05:06Z",                                      // valid datetime, wrong format
        "2001-02-03T04:05:0600:00", // valid datetime, timezone too close
        "2015-15-15T15:15:15Z",     // invalid datetime
        "2012-12-12T12:12:12x",     // invalid timezone
        "2012-123-12T12:12:12Z",    // invalid month
        "2012-12-77T12:12:12Z",     // invalid day
        "2012-12-12T26:12:12Z",     // invalid hour
        "2012-12-12T12:61:12Z",     // invalid minute
        "2012-12-12T12:12:62Z",     // invalid second
        "2012-12-12 T12:12:12Z",    // space after date
        "2012-12-12T12:12:12ZZ",    // trailing literal 'Z'
        "+802701-12-12T12:12:12Z",  // invalid year (out of bounds)
        "+ 2012-12-12T12:12:12Z",   // invalid space before year
        "  +82701  -  05  -  6  T  15  :  9  : 60.898989898989   Z", // valid datetime, wrong format
    ];
    for &s in &invalid {
        eprintln!("test_parse_datetime_utc invalid {s:?}");
        assert!(s.parse::<DateTime<Utc>>().is_err());
    }
}

#[test]
fn test_parse_from_str() {
    let edt = FixedOffset::east_opt(570 * 60).unwrap();
    let edt0 = FixedOffset::east_opt(0).unwrap();
    let wdt = FixedOffset::west_opt(10 * 3600).unwrap();
    assert_eq!(
        DateTime::parse_from_str("2014-5-7T12:34:56+09:30", "%Y-%m-%dT%H:%M:%S%z"),
        Ok(ymdhms(&edt, 2014, 5, 7, 12, 34, 56))
    ); // ignore offset
    assert!(DateTime::parse_from_str("20140507000000", "%Y%m%d%H%M%S").is_err()); // no offset
    assert!(
        DateTime::parse_from_str("Fri, 09 Aug 2013 23:54:35 GMT", "%a, %d %b %Y %H:%M:%S GMT")
            .is_err()
    );
    assert_eq!(
        DateTime::parse_from_str("0", "%s").unwrap(),
        DateTime::from_timestamp(0, 0).unwrap().fixed_offset()
    );

    assert_eq!(
        "2015-02-18T23:16:9.15Z".parse::<DateTime<FixedOffset>>(),
        Ok(ymdhms_milli(&edt0, 2015, 2, 18, 23, 16, 9, 150))
    );
    assert_eq!(
        "2015-02-18T23:16:9.15Z".parse::<DateTime<Utc>>(),
        Ok(ymdhms_milli_utc(2015, 2, 18, 23, 16, 9, 150)),
    );
    assert_eq!(
        "2015-02-18T23:16:9.15 UTC".parse::<DateTime<Utc>>(),
        Ok(ymdhms_milli_utc(2015, 2, 18, 23, 16, 9, 150))
    );
    assert_eq!(
        "2015-02-18T23:16:9.15UTC".parse::<DateTime<Utc>>(),
        Ok(ymdhms_milli_utc(2015, 2, 18, 23, 16, 9, 150))
    );

    assert_eq!(
        "2015-2-18T23:16:9.15Z".parse::<DateTime<FixedOffset>>(),
        Ok(ymdhms_milli(&edt0, 2015, 2, 18, 23, 16, 9, 150))
    );
    assert_eq!(
        "2015-2-18T13:16:9.15-10:00".parse::<DateTime<FixedOffset>>(),
        Ok(ymdhms_milli(&wdt, 2015, 2, 18, 13, 16, 9, 150))
    );
    assert!("2015-2-18T23:16:9.15".parse::<DateTime<FixedOffset>>().is_err());

    assert_eq!(
        "2015-2-18T23:16:9.15Z".parse::<DateTime<Utc>>(),
        Ok(ymdhms_milli_utc(2015, 2, 18, 23, 16, 9, 150))
    );
    assert_eq!(
        "2015-2-18T13:16:9.15-10:00".parse::<DateTime<Utc>>(),
        Ok(ymdhms_milli_utc(2015, 2, 18, 23, 16, 9, 150))
    );
    assert!("2015-2-18T23:16:9.15".parse::<DateTime<Utc>>().is_err());

    // no test for `DateTime<Local>`, we cannot verify that much.
}

#[test]
fn test_datetime_parse_from_str() {
    let dt = ymdhms(&FixedOffset::east_opt(-9 * 60 * 60).unwrap(), 2013, 8, 9, 23, 54, 35);
    let parse = DateTime::parse_from_str;

    // timezone variations

    //
    // %Z
    //
    // wrong timezone format
    assert!(parse("Aug 09 2013 23:54:35 -0900", "%b %d %Y %H:%M:%S %Z").is_err());
    // bad timezone data?
    assert!(parse("Aug 09 2013 23:54:35 PST", "%b %d %Y %H:%M:%S %Z").is_err());
    // bad timezone data
    assert!(parse("Aug 09 2013 23:54:35 XXXXX", "%b %d %Y %H:%M:%S %Z").is_err());

    //
    // %z
    //
    assert_eq!(parse("Aug 09 2013 23:54:35 -0900", "%b %d %Y %H:%M:%S %z"), Ok(dt));
    assert_eq!(parse("Aug 09 2013 23:54:35 -09 00", "%b %d %Y %H:%M:%S %z"), Ok(dt));
    assert_eq!(parse("Aug 09 2013 23:54:35 -09:00", "%b %d %Y %H:%M:%S %z"), Ok(dt));
    assert_eq!(parse("Aug 09 2013 23:54:35 -09 : 00", "%b %d %Y %H:%M:%S %z"), Ok(dt));
    assert_eq!(parse("Aug 09 2013 23:54:35 --0900", "%b %d %Y %H:%M:%S -%z"), Ok(dt));
    assert_eq!(parse("Aug 09 2013 23:54:35 +-0900", "%b %d %Y %H:%M:%S +%z"), Ok(dt));
    assert_eq!(parse("Aug 09 2013 23:54:35 -09:00 ", "%b %d %Y %H:%M:%S %z "), Ok(dt));
    // trailing newline after timezone
    assert!(parse("Aug 09 2013 23:54:35 -09:00\n", "%b %d %Y %H:%M:%S %z").is_err());
    assert_eq!(parse("Aug 09 2013 23:54:35 -09:00\n", "%b %d %Y %H:%M:%S %z "), Ok(dt));
    // trailing colon
    assert!(parse("Aug 09 2013 23:54:35 -09:00:", "%b %d %Y %H:%M:%S %z").is_err());
    // trailing colon with space
    assert!(parse("Aug 09 2013 23:54:35 -09:00: ", "%b %d %Y %H:%M:%S %z ").is_err());
    // trailing colon, mismatch space
    assert!(parse("Aug 09 2013 23:54:35 -09:00:", "%b %d %Y %H:%M:%S %z ").is_err());
    // wrong timezone data
    assert!(parse("Aug 09 2013 23:54:35 -09", "%b %d %Y %H:%M:%S %z").is_err());
    assert_eq!(parse("Aug 09 2013 23:54:35 -09::00", "%b %d %Y %H:%M:%S %z"), Ok(dt));
    assert_eq!(parse("Aug 09 2013 23:54:35 -0900::", "%b %d %Y %H:%M:%S %z::"), Ok(dt));
    assert_eq!(parse("Aug 09 2013 23:54:35 -09:00:00", "%b %d %Y %H:%M:%S %z:00"), Ok(dt));
    assert_eq!(parse("Aug 09 2013 23:54:35 -09:00:00 ", "%b %d %Y %H:%M:%S %z:00 "), Ok(dt));

    //
    // %:z
    //
    assert_eq!(parse("Aug 09 2013 23:54:35  -09:00", "%b %d %Y %H:%M:%S  %:z"), Ok(dt));
    assert_eq!(parse("Aug 09 2013 23:54:35 -0900", "%b %d %Y %H:%M:%S %:z"), Ok(dt));
    assert_eq!(parse("Aug 09 2013 23:54:35 -09 00", "%b %d %Y %H:%M:%S %:z"), Ok(dt));
    assert_eq!(parse("Aug 09 2013 23:54:35 -09 : 00", "%b %d %Y %H:%M:%S %:z"), Ok(dt));
    assert_eq!(parse("Aug 09 2013 23:54:35 -09 : 00:", "%b %d %Y %H:%M:%S %:z:"), Ok(dt));
    // wrong timezone data
    assert!(parse("Aug 09 2013 23:54:35 -09", "%b %d %Y %H:%M:%S %:z").is_err());
    assert_eq!(parse("Aug 09 2013 23:54:35 -09::00", "%b %d %Y %H:%M:%S %:z"), Ok(dt));
    // timezone data hs too many colons
    assert!(parse("Aug 09 2013 23:54:35 -09:00:", "%b %d %Y %H:%M:%S %:z").is_err());
    // timezone data hs too many colons
    assert!(parse("Aug 09 2013 23:54:35 -09:00::", "%b %d %Y %H:%M:%S %:z").is_err());
    assert_eq!(parse("Aug 09 2013 23:54:35 -09:00::", "%b %d %Y %H:%M:%S %:z::"), Ok(dt));

    //
    // %::z
    //
    assert_eq!(parse("Aug 09 2013 23:54:35 -0900", "%b %d %Y %H:%M:%S %::z"), Ok(dt));
    assert_eq!(parse("Aug 09 2013 23:54:35 -09:00", "%b %d %Y %H:%M:%S %::z"), Ok(dt));
    assert_eq!(parse("Aug 09 2013 23:54:35 -09 : 00", "%b %d %Y %H:%M:%S %::z"), Ok(dt));
    // mismatching colon expectations
    assert!(parse("Aug 09 2013 23:54:35 -09:00:00", "%b %d %Y %H:%M:%S %::z").is_err());
    assert_eq!(parse("Aug 09 2013 23:54:35 -09::00", "%b %d %Y %H:%M:%S %::z"), Ok(dt));
    assert_eq!(parse("Aug 09 2013 23:54:35 -09::00", "%b %d %Y %H:%M:%S %:z"), Ok(dt));
    // wrong timezone data
    assert!(parse("Aug 09 2013 23:54:35 -09", "%b %d %Y %H:%M:%S %::z").is_err());
    assert_eq!(parse("Aug 09 2013 23:54:35 -09001234", "%b %d %Y %H:%M:%S %::z1234"), Ok(dt));
    assert_eq!(parse("Aug 09 2013 23:54:35 -09:001234", "%b %d %Y %H:%M:%S %::z1234"), Ok(dt));
    assert_eq!(parse("Aug 09 2013 23:54:35 -0900 ", "%b %d %Y %H:%M:%S %::z "), Ok(dt));
    assert_eq!(parse("Aug 09 2013 23:54:35 -0900\t\n", "%b %d %Y %H:%M:%S %::z\t\n"), Ok(dt));
    assert_eq!(parse("Aug 09 2013 23:54:35 -0900:", "%b %d %Y %H:%M:%S %::z:"), Ok(dt));
    assert_eq!(parse("Aug 09 2013 23:54:35 :-0900:0", "%b %d %Y %H:%M:%S :%::z:0"), Ok(dt));
    // mismatching colons and spaces
    assert!(parse("Aug 09 2013 23:54:35 :-0900: ", "%b %d %Y %H:%M:%S :%::z::").is_err());
    // mismatching colons expectations
    assert!(parse("Aug 09 2013 23:54:35 -09:00:00", "%b %d %Y %H:%M:%S %::z").is_err());
    assert_eq!(parse("Aug 09 2013 -0900: 23:54:35", "%b %d %Y %::z: %H:%M:%S"), Ok(dt));
    assert_eq!(parse("Aug 09 2013 :-0900:0 23:54:35", "%b %d %Y :%::z:0 %H:%M:%S"), Ok(dt));
    // mismatching colons expectations mid-string
    assert!(parse("Aug 09 2013 :-0900: 23:54:35", "%b %d %Y :%::z  %H:%M:%S").is_err());
    // mismatching colons expectations, before end
    assert!(parse("Aug 09 2013 23:54:35 -09:00:00 ", "%b %d %Y %H:%M:%S %::z ").is_err());

    //
    // %:::z
    //
    assert_eq!(parse("Aug 09 2013 23:54:35 -09:00", "%b %d %Y %H:%M:%S %:::z"), Ok(dt));
    assert_eq!(parse("Aug 09 2013 23:54:35 -0900", "%b %d %Y %H:%M:%S %:::z"), Ok(dt));
    assert_eq!(parse("Aug 09 2013 23:54:35 -0900  ", "%b %d %Y %H:%M:%S %:::z  "), Ok(dt));
    // wrong timezone data
    assert!(parse("Aug 09 2013 23:54:35 -09", "%b %d %Y %H:%M:%S %:::z").is_err());

    //
    // %::::z
    //
    // too many colons
    assert!(parse("Aug 09 2013 23:54:35 -0900", "%b %d %Y %H:%M:%S %::::z").is_err());
    // too many colons
    assert!(parse("Aug 09 2013 23:54:35 -09:00", "%b %d %Y %H:%M:%S %::::z").is_err());
    // too many colons
    assert!(parse("Aug 09 2013 23:54:35 -09:00:", "%b %d %Y %H:%M:%S %::::z").is_err());
    // too many colons
    assert!(parse("Aug 09 2013 23:54:35 -09:00:00", "%b %d %Y %H:%M:%S %::::z").is_err());

    //
    // %#z
    //
    assert_eq!(parse("Aug 09 2013 23:54:35 -09:00", "%b %d %Y %H:%M:%S %#z"), Ok(dt));
    assert_eq!(parse("Aug 09 2013 23:54:35 -0900", "%b %d %Y %H:%M:%S %#z"), Ok(dt));
    assert_eq!(parse("Aug 09 2013 23:54:35  -09:00  ", "%b %d %Y %H:%M:%S  %#z  "), Ok(dt));
    assert_eq!(parse("Aug 09 2013 23:54:35  -0900  ", "%b %d %Y %H:%M:%S  %#z  "), Ok(dt));
    assert_eq!(parse("Aug 09 2013 23:54:35 -09", "%b %d %Y %H:%M:%S %#z"), Ok(dt));
    assert_eq!(parse("Aug 09 2013 23:54:35 -0900", "%b %d %Y %H:%M:%S %#z"), Ok(dt));
    assert_eq!(parse("Aug 09 2013 23:54:35 -09:", "%b %d %Y %H:%M:%S %#z"), Ok(dt));
    assert_eq!(parse("Aug 09 2013 23:54:35 -09: ", "%b %d %Y %H:%M:%S %#z "), Ok(dt));
    assert_eq!(parse("Aug 09 2013 23:54:35+-09", "%b %d %Y %H:%M:%S+%#z"), Ok(dt));
    assert_eq!(parse("Aug 09 2013 23:54:35--09", "%b %d %Y %H:%M:%S-%#z"), Ok(dt));
    assert_eq!(parse("Aug 09 2013 -09:00 23:54:35", "%b %d %Y %#z%H:%M:%S"), Ok(dt));
    assert_eq!(parse("Aug 09 2013 -0900 23:54:35", "%b %d %Y %#z%H:%M:%S"), Ok(dt));
    assert_eq!(parse("Aug 09 2013 -090023:54:35", "%b %d %Y %#z%H:%M:%S"), Ok(dt));
    assert_eq!(parse("Aug 09 2013 -09:0023:54:35", "%b %d %Y %#z%H:%M:%S"), Ok(dt));
    // timezone with partial minutes adjacent hours
    assert_ne!(parse("Aug 09 2013 -09023:54:35", "%b %d %Y %#z%H:%M:%S"), Ok(dt));
    // bad timezone data
    assert!(parse("Aug 09 2013 23:54:35 -09:00:00", "%b %d %Y %H:%M:%S %#z").is_err());
    // bad timezone data (partial minutes)
    assert!(parse("Aug 09 2013 23:54:35 -090", "%b %d %Y %H:%M:%S %#z").is_err());
    // bad timezone data (partial minutes) with trailing space
    assert!(parse("Aug 09 2013 23:54:35 -090 ", "%b %d %Y %H:%M:%S %#z ").is_err());
    // bad timezone data (partial minutes) mid-string
    assert!(parse("Aug 09 2013 -090 23:54:35", "%b %d %Y %#z %H:%M:%S").is_err());
    // bad timezone data
    assert!(parse("Aug 09 2013 -09:00:00 23:54:35", "%b %d %Y %#z %H:%M:%S").is_err());
    // timezone data ambiguous with hours
    assert!(parse("Aug 09 2013 -09:00:23:54:35", "%b %d %Y %#z%H:%M:%S").is_err());
}

#[test]
fn test_to_string_round_trip() {
    let dt = Utc.with_ymd_and_hms(2000, 1, 1, 0, 0, 0).unwrap();
    let _dt: DateTime<Utc> = dt.to_string().parse().unwrap();

    let ndt_fixed = dt.with_timezone(&FixedOffset::east_opt(3600).unwrap());
    let _dt: DateTime<FixedOffset> = ndt_fixed.to_string().parse().unwrap();

    let ndt_fixed = dt.with_timezone(&FixedOffset::east_opt(0).unwrap());
    let _dt: DateTime<FixedOffset> = ndt_fixed.to_string().parse().unwrap();
}

#[test]
#[cfg(feature = "clock")]
fn test_to_string_round_trip_with_local() {
    let ndt = Local::now();
    let _dt: DateTime<FixedOffset> = ndt.to_string().parse().unwrap();
}

#[test]
#[cfg(feature = "clock")]
fn test_datetime_format_with_local() {
    // if we are not around the year boundary, local and UTC date should have the same year
    let dt = Local::now().with_month(5).unwrap();
    assert_eq!(dt.format("%Y").to_string(), dt.with_timezone(&Utc).format("%Y").to_string());
}

#[test]
fn test_datetime_is_send_and_copy() {
    #[derive(Clone)]
    struct Tz {
        _not_send: *const i32,
    }
    impl TimeZone for Tz {
        type Offset = Off;

        fn from_offset(_: &Self::Offset) -> Self {
            unimplemented!()
        }
        fn offset_from_local_date(&self, _: &NaiveDate) -> crate::MappedLocalTime<Self::Offset> {
            unimplemented!()
        }
        fn offset_from_local_datetime(
            &self,
            _: &NaiveDateTime,
        ) -> crate::MappedLocalTime<Self::Offset> {
            unimplemented!()
        }
        fn offset_from_utc_date(&self, _: &NaiveDate) -> Self::Offset {
            unimplemented!()
        }
        fn offset_from_utc_datetime(&self, _: &NaiveDateTime) -> Self::Offset {
            unimplemented!()
        }
    }

    #[derive(Copy, Clone, Debug)]
    struct Off(());
    impl Offset for Off {
        fn fix(&self) -> FixedOffset {
            unimplemented!()
        }
    }

    fn _assert_send_copy<T: Send + Copy>() {}
    // `DateTime` is `Send + Copy` if the offset is.
    _assert_send_copy::<DateTime<Tz>>();
}

#[test]
fn test_subsecond_part() {
    let datetime = Utc
        .from_local_datetime(
            &NaiveDate::from_ymd_opt(2014, 7, 8)
                .unwrap()
                .and_hms_nano_opt(9, 10, 11, 1234567)
                .unwrap(),
        )
        .unwrap();

    assert_eq!(1, datetime.timestamp_subsec_millis());
    assert_eq!(1234, datetime.timestamp_subsec_micros());
    assert_eq!(1234567, datetime.timestamp_subsec_nanos());
}

// Some targets, such as `wasm32-wasi`, have a problematic definition of `SystemTime`, such as an
// `i32` (year 2035 problem), or an `u64` (no values before `UNIX-EPOCH`).
// See https://github.com/rust-lang/rust/issues/44394.
#[test]
#[cfg(all(feature = "std", not(all(target_arch = "wasm32", target_os = "wasi"))))]
fn test_from_system_time() {
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    let nanos = 999_999_000;

    let epoch = Utc.with_ymd_and_hms(1970, 1, 1, 0, 0, 0).unwrap();

    // SystemTime -> DateTime<Utc>
    assert_eq!(DateTime::<Utc>::from(UNIX_EPOCH), epoch);
    assert_eq!(
        DateTime::<Utc>::from(UNIX_EPOCH + Duration::new(999_999_999, nanos)),
        Utc.from_local_datetime(
            &NaiveDate::from_ymd_opt(2001, 9, 9)
                .unwrap()
                .and_hms_nano_opt(1, 46, 39, nanos)
                .unwrap()
        )
        .unwrap()
    );
    assert_eq!(
        DateTime::<Utc>::from(UNIX_EPOCH - Duration::new(999_999_999, nanos)),
        Utc.from_local_datetime(
            &NaiveDate::from_ymd_opt(1938, 4, 24)
                .unwrap()
                .and_hms_nano_opt(22, 13, 20, 1_000)
                .unwrap()
        )
        .unwrap()
    );

    // DateTime<Utc> -> SystemTime
    assert_eq!(SystemTime::from(epoch), UNIX_EPOCH);
    assert_eq!(
        SystemTime::from(
            Utc.from_local_datetime(
                &NaiveDate::from_ymd_opt(2001, 9, 9)
                    .unwrap()
                    .and_hms_nano_opt(1, 46, 39, nanos)
                    .unwrap()
            )
            .unwrap()
        ),
        UNIX_EPOCH + Duration::new(999_999_999, nanos)
    );
    assert_eq!(
        SystemTime::from(
            Utc.from_local_datetime(
                &NaiveDate::from_ymd_opt(1938, 4, 24)
                    .unwrap()
                    .and_hms_nano_opt(22, 13, 20, 1_000)
                    .unwrap()
            )
            .unwrap()
        ),
        UNIX_EPOCH - Duration::new(999_999_999, nanos)
    );

    // DateTime<any tz> -> SystemTime (via `with_timezone`)
    #[cfg(feature = "clock")]
    {
        assert_eq!(SystemTime::from(epoch.with_timezone(&Local)), UNIX_EPOCH);
    }
    assert_eq!(
        SystemTime::from(epoch.with_timezone(&FixedOffset::east_opt(32400).unwrap())),
        UNIX_EPOCH
    );
    assert_eq!(
        SystemTime::from(epoch.with_timezone(&FixedOffset::west_opt(28800).unwrap())),
        UNIX_EPOCH
    );
}

#[test]
#[allow(deprecated)]
fn test_datetime_from_local() {
    // 2000-01-12T02:00:00Z
    let naivedatetime_utc =
        NaiveDate::from_ymd_opt(2000, 1, 12).unwrap().and_hms_opt(2, 0, 0).unwrap();
    let datetime_utc = DateTime::<Utc>::from_utc(naivedatetime_utc, Utc);

    // 2000-01-12T10:00:00+8:00:00
    let timezone_east = FixedOffset::east_opt(8 * 60 * 60).unwrap();
    let naivedatetime_east =
        NaiveDate::from_ymd_opt(2000, 1, 12).unwrap().and_hms_opt(10, 0, 0).unwrap();
    let datetime_east = DateTime::<FixedOffset>::from_local(naivedatetime_east, timezone_east);

    // 2000-01-11T19:00:00-7:00:00
    let timezone_west = FixedOffset::west_opt(7 * 60 * 60).unwrap();
    let naivedatetime_west =
        NaiveDate::from_ymd_opt(2000, 1, 11).unwrap().and_hms_opt(19, 0, 0).unwrap();
    let datetime_west = DateTime::<FixedOffset>::from_local(naivedatetime_west, timezone_west);

    assert_eq!(datetime_east, datetime_utc.with_timezone(&timezone_east));
    assert_eq!(datetime_west, datetime_utc.with_timezone(&timezone_west));
}

#[test]
#[cfg(feature = "clock")]
fn test_datetime_before_windows_api_limits() {
    // dt corresponds to `FILETIME = 147221225472` from issue 651.
    // (https://github.com/chronotope/chrono/issues/651)
    // This used to fail on Windows for timezones with an offset of -5:00 or greater.
    // The API limits years to 1601..=30827.
    let dt = NaiveDate::from_ymd_opt(1601, 1, 1).unwrap().and_hms_milli_opt(4, 5, 22, 122).unwrap();
    let local_dt = Local.from_utc_datetime(&dt);
    dbg!(local_dt);
}

#[test]
#[cfg(feature = "clock")]
fn test_years_elapsed() {
    // A bit more than 1 year
    let one_year_ago = Utc::now().date_naive() - Days::new(400);
    // A bit more than 2 years
    let two_year_ago = Utc::now().date_naive() - Days::new(750);

    assert_eq!(Utc::now().date_naive().years_since(one_year_ago), Some(1));
    assert_eq!(Utc::now().date_naive().years_since(two_year_ago), Some(2));

    // If the given DateTime is later than now, the function will always return 0.
    let future = Utc::now().date_naive() + Days(100);
    assert_eq!(Utc::now().date_naive().years_since(future), None);
}

#[test]
fn test_datetime_add_assign() {
    let naivedatetime = NaiveDate::from_ymd_opt(2000, 1, 1).unwrap().and_hms_opt(0, 0, 0).unwrap();
    let datetime = naivedatetime.and_utc();
    let mut datetime_add = datetime;

    datetime_add += TimeDelta::try_seconds(60).unwrap();
    assert_eq!(datetime_add, datetime + TimeDelta::try_seconds(60).unwrap());

    let timezone = FixedOffset::east_opt(60 * 60).unwrap();
    let datetime = datetime.with_timezone(&timezone);
    let datetime_add = datetime_add.with_timezone(&timezone);

    assert_eq!(datetime_add, datetime + TimeDelta::try_seconds(60).unwrap());

    let timezone = FixedOffset::west_opt(2 * 60 * 60).unwrap();
    let datetime = datetime.with_timezone(&timezone);
    let datetime_add = datetime_add.with_timezone(&timezone);

    assert_eq!(datetime_add, datetime + TimeDelta::try_seconds(60).unwrap());
}

#[test]
#[cfg(feature = "clock")]
fn test_datetime_add_assign_local() {
    let naivedatetime = NaiveDate::from_ymd_opt(2022, 1, 1).unwrap().and_hms_opt(0, 0, 0).unwrap();

    let datetime = Local.from_utc_datetime(&naivedatetime);
    let mut datetime_add = Local.from_utc_datetime(&naivedatetime);

    // ensure we cross a DST transition
    for i in 1..=365 {
        datetime_add += TimeDelta::try_days(1).unwrap();
        assert_eq!(datetime_add, datetime + TimeDelta::try_days(i).unwrap())
    }
}

#[test]
fn test_datetime_sub_assign() {
    let naivedatetime = NaiveDate::from_ymd_opt(2000, 1, 1).unwrap().and_hms_opt(12, 0, 0).unwrap();
    let datetime = naivedatetime.and_utc();
    let mut datetime_sub = datetime;

    datetime_sub -= TimeDelta::try_minutes(90).unwrap();
    assert_eq!(datetime_sub, datetime - TimeDelta::try_minutes(90).unwrap());

    let timezone = FixedOffset::east_opt(60 * 60).unwrap();
    let datetime = datetime.with_timezone(&timezone);
    let datetime_sub = datetime_sub.with_timezone(&timezone);

    assert_eq!(datetime_sub, datetime - TimeDelta::try_minutes(90).unwrap());

    let timezone = FixedOffset::west_opt(2 * 60 * 60).unwrap();
    let datetime = datetime.with_timezone(&timezone);
    let datetime_sub = datetime_sub.with_timezone(&timezone);

    assert_eq!(datetime_sub, datetime - TimeDelta::try_minutes(90).unwrap());
}

#[test]
fn test_min_max_getters() {
    let offset_min = FixedOffset::west_opt(2 * 60 * 60).unwrap();
    let beyond_min = offset_min.from_utc_datetime(&NaiveDateTime::MIN);
    let offset_max = FixedOffset::east_opt(2 * 60 * 60).unwrap();
    let beyond_max = offset_max.from_utc_datetime(&NaiveDateTime::MAX);

    assert_eq!(format!("{beyond_min:?}"), "-262144-12-31T22:00:00-02:00");
    // RFC 2822 doesn't support years with more than 4 digits.
    // assert_eq!(beyond_min.to_rfc2822(), "");
    #[cfg(feature = "alloc")]
    assert_eq!(beyond_min.to_rfc3339(), "-262144-12-31T22:00:00-02:00");
    #[cfg(feature = "alloc")]
    assert_eq!(
        beyond_min.format("%Y-%m-%dT%H:%M:%S%:z").to_string(),
        "-262144-12-31T22:00:00-02:00"
    );
    assert_eq!(beyond_min.year(), -262144);
    assert_eq!(beyond_min.month(), 12);
    assert_eq!(beyond_min.month0(), 11);
    assert_eq!(beyond_min.day(), 31);
    assert_eq!(beyond_min.day0(), 30);
    assert_eq!(beyond_min.ordinal(), 366);
    assert_eq!(beyond_min.ordinal0(), 365);
    assert_eq!(beyond_min.weekday(), Weekday::Wed);
    assert_eq!(beyond_min.iso_week().year(), -262143);
    assert_eq!(beyond_min.iso_week().week(), 1);
    assert_eq!(beyond_min.hour(), 22);
    assert_eq!(beyond_min.minute(), 0);
    assert_eq!(beyond_min.second(), 0);
    assert_eq!(beyond_min.nanosecond(), 0);

    assert_eq!(format!("{beyond_max:?}"), "+262143-01-01T01:59:59.999999999+02:00");
    // RFC 2822 doesn't support years with more than 4 digits.
    // assert_eq!(beyond_max.to_rfc2822(), "");
    #[cfg(feature = "alloc")]
    assert_eq!(beyond_max.to_rfc3339(), "+262143-01-01T01:59:59.999999999+02:00");
    #[cfg(feature = "alloc")]
    assert_eq!(
        beyond_max.format("%Y-%m-%dT%H:%M:%S%.9f%:z").to_string(),
        "+262143-01-01T01:59:59.999999999+02:00"
    );
    assert_eq!(beyond_max.year(), 262143);
    assert_eq!(beyond_max.month(), 1);
    assert_eq!(beyond_max.month0(), 0);
    assert_eq!(beyond_max.day(), 1);
    assert_eq!(beyond_max.day0(), 0);
    assert_eq!(beyond_max.ordinal(), 1);
    assert_eq!(beyond_max.ordinal0(), 0);
    assert_eq!(beyond_max.weekday(), Weekday::Tue);
    assert_eq!(beyond_max.iso_week().year(), 262143);
    assert_eq!(beyond_max.iso_week().week(), 1);
    assert_eq!(beyond_max.hour(), 1);
    assert_eq!(beyond_max.minute(), 59);
    assert_eq!(beyond_max.second(), 59);
    assert_eq!(beyond_max.nanosecond(), 999_999_999);
}

#[test]
fn test_min_max_setters() {
    let offset_min = FixedOffset::west_opt(2 * 60 * 60).unwrap();
    let beyond_min = offset_min.from_utc_datetime(&NaiveDateTime::MIN);
    let offset_max = FixedOffset::east_opt(2 * 60 * 60).unwrap();
    let beyond_max = offset_max.from_utc_datetime(&NaiveDateTime::MAX);

    assert_eq!(beyond_min.with_year(2020).unwrap().year(), 2020);
    assert_eq!(beyond_min.with_year(beyond_min.year()), Some(beyond_min));
    assert_eq!(beyond_min.with_month(beyond_min.month()), Some(beyond_min));
    assert_eq!(beyond_min.with_month(3), None);
    assert_eq!(beyond_min.with_month0(beyond_min.month0()), Some(beyond_min));
    assert_eq!(beyond_min.with_month0(3), None);
    assert_eq!(beyond_min.with_day(beyond_min.day()), Some(beyond_min));
    assert_eq!(beyond_min.with_day(15), None);
    assert_eq!(beyond_min.with_day0(beyond_min.day0()), Some(beyond_min));
    assert_eq!(beyond_min.with_day0(15), None);
    assert_eq!(beyond_min.with_ordinal(beyond_min.ordinal()), Some(beyond_min));
    assert_eq!(beyond_min.with_ordinal(200), None);
    assert_eq!(beyond_min.with_ordinal0(beyond_min.ordinal0()), Some(beyond_min));
    assert_eq!(beyond_min.with_ordinal0(200), None);
    assert_eq!(beyond_min.with_hour(beyond_min.hour()), Some(beyond_min));
    assert_eq!(
        beyond_min.with_hour(23),
        beyond_min.checked_add_signed(TimeDelta::try_hours(1).unwrap())
    );
    assert_eq!(beyond_min.with_hour(5), None);
    assert_eq!(beyond_min.with_minute(0), Some(beyond_min));
    assert_eq!(beyond_min.with_second(0), Some(beyond_min));
    assert_eq!(beyond_min.with_nanosecond(0), Some(beyond_min));

    assert_eq!(beyond_max.with_year(2020).unwrap().year(), 2020);
    assert_eq!(beyond_max.with_year(beyond_max.year()), Some(beyond_max));
    assert_eq!(beyond_max.with_month(beyond_max.month()), Some(beyond_max));
    assert_eq!(beyond_max.with_month(3), None);
    assert_eq!(beyond_max.with_month0(beyond_max.month0()), Some(beyond_max));
    assert_eq!(beyond_max.with_month0(3), None);
    assert_eq!(beyond_max.with_day(beyond_max.day()), Some(beyond_max));
    assert_eq!(beyond_max.with_day(15), None);
    assert_eq!(beyond_max.with_day0(beyond_max.day0()), Some(beyond_max));
    assert_eq!(beyond_max.with_day0(15), None);
    assert_eq!(beyond_max.with_ordinal(beyond_max.ordinal()), Some(beyond_max));
    assert_eq!(beyond_max.with_ordinal(200), None);
    assert_eq!(beyond_max.with_ordinal0(beyond_max.ordinal0()), Some(beyond_max));
    assert_eq!(beyond_max.with_ordinal0(200), None);
    assert_eq!(beyond_max.with_hour(beyond_max.hour()), Some(beyond_max));
    assert_eq!(
        beyond_max.with_hour(0),
        beyond_max.checked_sub_signed(TimeDelta::try_hours(1).unwrap())
    );
    assert_eq!(beyond_max.with_hour(5), None);
    assert_eq!(beyond_max.with_minute(beyond_max.minute()), Some(beyond_max));
    assert_eq!(beyond_max.with_second(beyond_max.second()), Some(beyond_max));
    assert_eq!(beyond_max.with_nanosecond(beyond_max.nanosecond()), Some(beyond_max));
}

#[test]
fn test_min_max_add_days() {
    let offset_min = FixedOffset::west_opt(2 * 60 * 60).unwrap();
    let beyond_min = offset_min.from_utc_datetime(&NaiveDateTime::MIN);
    let offset_max = FixedOffset::east_opt(2 * 60 * 60).unwrap();
    let beyond_max = offset_max.from_utc_datetime(&NaiveDateTime::MAX);
    let max_time = NaiveTime::from_hms_nano_opt(23, 59, 59, 999_999_999).unwrap();

    assert_eq!(beyond_min.checked_add_days(Days::new(0)), Some(beyond_min));
    assert_eq!(
        beyond_min.checked_add_days(Days::new(1)),
        Some(offset_min.from_utc_datetime(&(NaiveDate::MIN + Days(1)).and_time(NaiveTime::MIN)))
    );
    assert_eq!(beyond_min.checked_sub_days(Days::new(0)), Some(beyond_min));
    assert_eq!(beyond_min.checked_sub_days(Days::new(1)), None);

    assert_eq!(beyond_max.checked_add_days(Days::new(0)), Some(beyond_max));
    assert_eq!(beyond_max.checked_add_days(Days::new(1)), None);
    assert_eq!(beyond_max.checked_sub_days(Days::new(0)), Some(beyond_max));
    assert_eq!(
        beyond_max.checked_sub_days(Days::new(1)),
        Some(offset_max.from_utc_datetime(&(NaiveDate::MAX - Days(1)).and_time(max_time)))
    );
}

#[test]
fn test_min_max_add_months() {
    let offset_min = FixedOffset::west_opt(2 * 60 * 60).unwrap();
    let beyond_min = offset_min.from_utc_datetime(&NaiveDateTime::MIN);
    let offset_max = FixedOffset::east_opt(2 * 60 * 60).unwrap();
    let beyond_max = offset_max.from_utc_datetime(&NaiveDateTime::MAX);
    let max_time = NaiveTime::from_hms_nano_opt(23, 59, 59, 999_999_999).unwrap();

    assert_eq!(beyond_min.checked_add_months(Months::new(0)), Some(beyond_min));
    assert_eq!(
        beyond_min.checked_add_months(Months::new(1)),
        Some(offset_min.from_utc_datetime(&(NaiveDate::MIN + Months(1)).and_time(NaiveTime::MIN)))
    );
    assert_eq!(beyond_min.checked_sub_months(Months::new(0)), Some(beyond_min));
    assert_eq!(beyond_min.checked_sub_months(Months::new(1)), None);

    assert_eq!(beyond_max.checked_add_months(Months::new(0)), Some(beyond_max));
    assert_eq!(beyond_max.checked_add_months(Months::new(1)), None);
    assert_eq!(beyond_max.checked_sub_months(Months::new(0)), Some(beyond_max));
    assert_eq!(
        beyond_max.checked_sub_months(Months::new(1)),
        Some(offset_max.from_utc_datetime(&(NaiveDate::MAX - Months(1)).and_time(max_time)))
    );
}

#[test]
#[should_panic]
fn test_local_beyond_min_datetime() {
    let min = FixedOffset::west_opt(2 * 60 * 60).unwrap().from_utc_datetime(&NaiveDateTime::MIN);
    let _ = min.naive_local();
}

#[test]
#[should_panic]
fn test_local_beyond_max_datetime() {
    let max = FixedOffset::east_opt(2 * 60 * 60).unwrap().from_utc_datetime(&NaiveDateTime::MAX);
    let _ = max.naive_local();
}

#[test]
#[cfg(feature = "clock")]
fn test_datetime_sub_assign_local() {
    let naivedatetime = NaiveDate::from_ymd_opt(2022, 1, 1).unwrap().and_hms_opt(0, 0, 0).unwrap();

    let datetime = Local.from_utc_datetime(&naivedatetime);
    let mut datetime_sub = Local.from_utc_datetime(&naivedatetime);

    // ensure we cross a DST transition
    for i in 1..=365 {
        datetime_sub -= TimeDelta::try_days(1).unwrap();
        assert_eq!(datetime_sub, datetime - TimeDelta::try_days(i).unwrap())
    }
}

#[test]
fn test_core_duration_ops() {
    use core::time::Duration;

    let mut utc_dt = Utc.with_ymd_and_hms(2023, 8, 29, 11, 34, 12).unwrap();
    let same = utc_dt + Duration::ZERO;
    assert_eq!(utc_dt, same);

    utc_dt += Duration::new(3600, 0);
    assert_eq!(utc_dt, Utc.with_ymd_and_hms(2023, 8, 29, 12, 34, 12).unwrap());
}

#[test]
#[should_panic]
fn test_core_duration_max() {
    use core::time::Duration;

    let mut utc_dt = Utc.with_ymd_and_hms(2023, 8, 29, 11, 34, 12).unwrap();
    utc_dt += Duration::MAX;
}

#[test]
#[cfg(feature = "clock")]
fn test_datetime_local_from_preserves_offset() {
    let naivedatetime = NaiveDate::from_ymd_opt(2023, 1, 1).unwrap().and_hms_opt(0, 0, 0).unwrap();

    let datetime = Local.from_utc_datetime(&naivedatetime);
    let offset = datetime.offset().fix();

    let datetime_fixed: DateTime<FixedOffset> = datetime.into();
    assert_eq!(&offset, datetime_fixed.offset());
    assert_eq!(datetime.fixed_offset(), datetime_fixed);
}

#[test]
fn test_datetime_fixed_offset() {
    let naivedatetime = NaiveDate::from_ymd_opt(2023, 1, 1).unwrap().and_hms_opt(0, 0, 0).unwrap();

    let datetime = Utc.from_utc_datetime(&naivedatetime);
    let fixed_utc = FixedOffset::east_opt(0).unwrap();
    assert_eq!(datetime.fixed_offset(), fixed_utc.from_local_datetime(&naivedatetime).unwrap());

    let fixed_offset = FixedOffset::east_opt(3600).unwrap();
    let datetime_fixed = fixed_offset.from_local_datetime(&naivedatetime).unwrap();
    assert_eq!(datetime_fixed.fixed_offset(), datetime_fixed);
}

#[test]
fn test_datetime_to_utc() {
    let dt =
        FixedOffset::east_opt(3600).unwrap().with_ymd_and_hms(2020, 2, 22, 23, 24, 25).unwrap();
    let dt_utc: DateTime<Utc> = dt.to_utc();
    assert_eq!(dt, dt_utc);
}

#[test]
fn test_add_sub_months() {
    let utc_dt = Utc.with_ymd_and_hms(2018, 9, 5, 23, 58, 0).unwrap();
    assert_eq!(utc_dt + Months::new(15), Utc.with_ymd_and_hms(2019, 12, 5, 23, 58, 0).unwrap());

    let utc_dt = Utc.with_ymd_and_hms(2020, 1, 31, 23, 58, 0).unwrap();
    assert_eq!(utc_dt + Months::new(1), Utc.with_ymd_and_hms(2020, 2, 29, 23, 58, 0).unwrap());
    assert_eq!(utc_dt + Months::new(2), Utc.with_ymd_and_hms(2020, 3, 31, 23, 58, 0).unwrap());

    let utc_dt = Utc.with_ymd_and_hms(2018, 9, 5, 23, 58, 0).unwrap();
    assert_eq!(utc_dt - Months::new(15), Utc.with_ymd_and_hms(2017, 6, 5, 23, 58, 0).unwrap());

    let utc_dt = Utc.with_ymd_and_hms(2020, 3, 31, 23, 58, 0).unwrap();
    assert_eq!(utc_dt - Months::new(1), Utc.with_ymd_and_hms(2020, 2, 29, 23, 58, 0).unwrap());
    assert_eq!(utc_dt - Months::new(2), Utc.with_ymd_and_hms(2020, 1, 31, 23, 58, 0).unwrap());
}

#[test]
fn test_auto_conversion() {
    let utc_dt = Utc.with_ymd_and_hms(2018, 9, 5, 23, 58, 0).unwrap();
    let cdt_dt = FixedOffset::west_opt(5 * 60 * 60)
        .unwrap()
        .with_ymd_and_hms(2018, 9, 5, 18, 58, 0)
        .unwrap();
    let utc_dt2: DateTime<Utc> = cdt_dt.into();
    assert_eq!(utc_dt, utc_dt2);
}

#[test]
#[cfg(feature = "clock")]
#[allow(deprecated)]
fn test_test_deprecated_from_offset() {
    let now = Local::now();
    let naive = now.naive_local();
    let utc = now.naive_utc();
    let offset: FixedOffset = *now.offset();

    assert_eq!(DateTime::<Local>::from_local(naive, offset), now);
    assert_eq!(DateTime::<Local>::from_utc(utc, offset), now);
}

#[test]
#[cfg(all(feature = "unstable-locales", feature = "alloc"))]
fn locale_decimal_point() {
    use crate::Locale::{ar_SY, nl_NL};
    let dt =
        Utc.with_ymd_and_hms(2018, 9, 5, 18, 58, 0).unwrap().with_nanosecond(123456780).unwrap();

    assert_eq!(dt.format_localized("%T%.f", nl_NL).to_string(), "18:58:00,123456780");
    assert_eq!(dt.format_localized("%T%.3f", nl_NL).to_string(), "18:58:00,123");
    assert_eq!(dt.format_localized("%T%.6f", nl_NL).to_string(), "18:58:00,123456");
    assert_eq!(dt.format_localized("%T%.9f", nl_NL).to_string(), "18:58:00,123456780");

    assert_eq!(dt.format_localized("%T%.f", ar_SY).to_string(), "18:58:00.123456780");
    assert_eq!(dt.format_localized("%T%.3f", ar_SY).to_string(), "18:58:00.123");
    assert_eq!(dt.format_localized("%T%.6f", ar_SY).to_string(), "18:58:00.123456");
    assert_eq!(dt.format_localized("%T%.9f", ar_SY).to_string(), "18:58:00.123456780");
}

/// This is an extended test for <https://github.com/chronotope/chrono/issues/1289>.
#[test]
fn nano_roundrip() {
    const BILLION: i64 = 1_000_000_000;

    for nanos in [
        i64::MIN,
        i64::MIN + 1,
        i64::MIN + 2,
        i64::MIN + BILLION - 1,
        i64::MIN + BILLION,
        i64::MIN + BILLION + 1,
        -BILLION - 1,
        -BILLION,
        -BILLION + 1,
        0,
        BILLION - 1,
        BILLION,
        BILLION + 1,
        i64::MAX - BILLION - 1,
        i64::MAX - BILLION,
        i64::MAX - BILLION + 1,
        i64::MAX - 2,
        i64::MAX - 1,
        i64::MAX,
    ] {
        println!("nanos: {nanos}");
        let dt = Utc.timestamp_nanos(nanos);
        let nanos2 = dt.timestamp_nanos_opt().expect("value roundtrips");
        assert_eq!(nanos, nanos2);
    }
}
