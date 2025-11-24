use super::NaiveDateTime;
use crate::{Datelike, FixedOffset, MappedLocalTime, NaiveDate, TimeDelta, Utc};

#[test]
fn test_datetime_add() {
    fn check(
        (y, m, d, h, n, s): (i32, u32, u32, u32, u32, u32),
        rhs: TimeDelta,
        result: Option<(i32, u32, u32, u32, u32, u32)>,
    ) {
        let lhs = NaiveDate::from_ymd_opt(y, m, d).unwrap().and_hms_opt(h, n, s).unwrap();
        let sum = result.map(|(y, m, d, h, n, s)| {
            NaiveDate::from_ymd_opt(y, m, d).unwrap().and_hms_opt(h, n, s).unwrap()
        });
        assert_eq!(lhs.checked_add_signed(rhs), sum);
        assert_eq!(lhs.checked_sub_signed(-rhs), sum);
    }
    let seconds = |s| TimeDelta::try_seconds(s).unwrap();

    check((2014, 5, 6, 7, 8, 9), seconds(3600 + 60 + 1), Some((2014, 5, 6, 8, 9, 10)));
    check((2014, 5, 6, 7, 8, 9), seconds(-(3600 + 60 + 1)), Some((2014, 5, 6, 6, 7, 8)));
    check((2014, 5, 6, 7, 8, 9), seconds(86399), Some((2014, 5, 7, 7, 8, 8)));
    check((2014, 5, 6, 7, 8, 9), seconds(86_400 * 10), Some((2014, 5, 16, 7, 8, 9)));
    check((2014, 5, 6, 7, 8, 9), seconds(-86_400 * 10), Some((2014, 4, 26, 7, 8, 9)));
    check((2014, 5, 6, 7, 8, 9), seconds(86_400 * 10), Some((2014, 5, 16, 7, 8, 9)));

    // overflow check
    // assumes that we have correct values for MAX/MIN_DAYS_FROM_YEAR_0 from `naive::date`.
    // (they are private constants, but the equivalence is tested in that module.)
    let max_days_from_year_0 =
        NaiveDate::MAX.signed_duration_since(NaiveDate::from_ymd_opt(0, 1, 1).unwrap());
    check((0, 1, 1, 0, 0, 0), max_days_from_year_0, Some((NaiveDate::MAX.year(), 12, 31, 0, 0, 0)));
    check(
        (0, 1, 1, 0, 0, 0),
        max_days_from_year_0 + seconds(86399),
        Some((NaiveDate::MAX.year(), 12, 31, 23, 59, 59)),
    );
    check((0, 1, 1, 0, 0, 0), max_days_from_year_0 + seconds(86_400), None);
    check((0, 1, 1, 0, 0, 0), TimeDelta::MAX, None);

    let min_days_from_year_0 =
        NaiveDate::MIN.signed_duration_since(NaiveDate::from_ymd_opt(0, 1, 1).unwrap());
    check((0, 1, 1, 0, 0, 0), min_days_from_year_0, Some((NaiveDate::MIN.year(), 1, 1, 0, 0, 0)));
    check((0, 1, 1, 0, 0, 0), min_days_from_year_0 - seconds(1), None);
    check((0, 1, 1, 0, 0, 0), TimeDelta::MIN, None);
}

#[test]
fn test_datetime_sub() {
    let ymdhms =
        |y, m, d, h, n, s| NaiveDate::from_ymd_opt(y, m, d).unwrap().and_hms_opt(h, n, s).unwrap();
    let since = NaiveDateTime::signed_duration_since;
    assert_eq!(since(ymdhms(2014, 5, 6, 7, 8, 9), ymdhms(2014, 5, 6, 7, 8, 9)), TimeDelta::zero());
    assert_eq!(
        since(ymdhms(2014, 5, 6, 7, 8, 10), ymdhms(2014, 5, 6, 7, 8, 9)),
        TimeDelta::try_seconds(1).unwrap()
    );
    assert_eq!(
        since(ymdhms(2014, 5, 6, 7, 8, 9), ymdhms(2014, 5, 6, 7, 8, 10)),
        TimeDelta::try_seconds(-1).unwrap()
    );
    assert_eq!(
        since(ymdhms(2014, 5, 7, 7, 8, 9), ymdhms(2014, 5, 6, 7, 8, 10)),
        TimeDelta::try_seconds(86399).unwrap()
    );
    assert_eq!(
        since(ymdhms(2001, 9, 9, 1, 46, 39), ymdhms(1970, 1, 1, 0, 0, 0)),
        TimeDelta::try_seconds(999_999_999).unwrap()
    );
}

#[test]
fn test_datetime_addassignment() {
    let ymdhms =
        |y, m, d, h, n, s| NaiveDate::from_ymd_opt(y, m, d).unwrap().and_hms_opt(h, n, s).unwrap();
    let mut date = ymdhms(2016, 10, 1, 10, 10, 10);
    date += TimeDelta::try_minutes(10_000_000).unwrap();
    assert_eq!(date, ymdhms(2035, 10, 6, 20, 50, 10));
    date += TimeDelta::try_days(10).unwrap();
    assert_eq!(date, ymdhms(2035, 10, 16, 20, 50, 10));
}

#[test]
fn test_datetime_subassignment() {
    let ymdhms =
        |y, m, d, h, n, s| NaiveDate::from_ymd_opt(y, m, d).unwrap().and_hms_opt(h, n, s).unwrap();
    let mut date = ymdhms(2016, 10, 1, 10, 10, 10);
    date -= TimeDelta::try_minutes(10_000_000).unwrap();
    assert_eq!(date, ymdhms(1997, 9, 26, 23, 30, 10));
    date -= TimeDelta::try_days(10).unwrap();
    assert_eq!(date, ymdhms(1997, 9, 16, 23, 30, 10));
}

#[test]
fn test_core_duration_ops() {
    use core::time::Duration;

    let mut dt = NaiveDate::from_ymd_opt(2023, 8, 29).unwrap().and_hms_opt(11, 34, 12).unwrap();
    let same = dt + Duration::ZERO;
    assert_eq!(dt, same);

    dt += Duration::new(3600, 0);
    assert_eq!(dt, NaiveDate::from_ymd_opt(2023, 8, 29).unwrap().and_hms_opt(12, 34, 12).unwrap());
}

#[test]
#[should_panic]
fn test_core_duration_max() {
    use core::time::Duration;

    let mut utc_dt = NaiveDate::from_ymd_opt(2023, 8, 29).unwrap().and_hms_opt(11, 34, 12).unwrap();
    utc_dt += Duration::MAX;
}

#[test]
fn test_datetime_from_str() {
    // valid cases
    let valid = [
        "2001-02-03T04:05:06",
        "2012-12-12T12:12:12",
        "2015-02-18T23:16:09.153",
        "2015-2-18T23:16:09.153",
        "-77-02-18T23:16:09",
        "+82701-05-6T15:9:60.898989898989",
        "  +82701  -  05  -  6  T  15  :  9  : 60.898989898989   ",
    ];
    for &s in &valid {
        eprintln!("test_parse_naivedatetime valid {s:?}");
        let d = match s.parse::<NaiveDateTime>() {
            Ok(d) => d,
            Err(e) => panic!("parsing `{s}` has failed: {e}"),
        };
        let s_ = format!("{d:?}");
        // `s` and `s_` may differ, but `s.parse()` and `s_.parse()` must be same
        let d_ = match s_.parse::<NaiveDateTime>() {
            Ok(d) => d,
            Err(e) => {
                panic!("`{s}` is parsed into `{d:?}`, but reparsing that has failed: {e}")
            }
        };
        assert!(
            d == d_,
            "`{s}` is parsed into `{d:?}`, but reparsed result \
             `{d_:?}` does not match"
        );
    }

    // some invalid cases
    // since `ParseErrorKind` is private, all we can do is to check if there was an error
    let invalid = [
        "",                              // empty
        "x",                             // invalid / missing data
        "15",                            // missing data
        "15:8:9",                        // looks like a time (invalid date)
        "15-8-9",                        // looks like a date (invalid)
        "Fri, 09 Aug 2013 23:54:35 GMT", // valid date, wrong format
        "Sat Jun 30 23:59:60 2012",      // valid date, wrong format
        "1441497364.649",                // valid date, wrong format
        "+1441497364.649",               // valid date, wrong format
        "+1441497364",                   // valid date, wrong format
        "2014/02/03 04:05:06",           // valid date, wrong format
        "2015-15-15T15:15:15",           // invalid date
        "2012-12-12T12:12:12x",          // bad timezone / trailing literal
        "2012-12-12T12:12:12+00:00",     // unexpected timezone / trailing literal
        "2012-12-12T12:12:12 +00:00",    // unexpected timezone / trailing literal
        "2012-12-12T12:12:12 GMT",       // unexpected timezone / trailing literal
        "2012-123-12T12:12:12",          // invalid month
        "2012-12-12t12:12:12",           // bad divider 't'
        "2012-12-12 12:12:12",           // missing divider 'T'
        "2012-12-12T12:12:12Z",          // trailing char 'Z'
        "+ 82701-123-12T12:12:12",       // strange year, invalid month
        "+802701-123-12T12:12:12",       // out-of-bound year, invalid month
    ];
    for &s in &invalid {
        eprintln!("test_datetime_from_str invalid {s:?}");
        assert!(s.parse::<NaiveDateTime>().is_err());
    }
}

#[test]
fn test_datetime_parse_from_str() {
    let ymdhms =
        |y, m, d, h, n, s| NaiveDate::from_ymd_opt(y, m, d).unwrap().and_hms_opt(h, n, s).unwrap();
    let ymdhmsn = |y, m, d, h, n, s, nano| {
        NaiveDate::from_ymd_opt(y, m, d).unwrap().and_hms_nano_opt(h, n, s, nano).unwrap()
    };
    assert_eq!(
        NaiveDateTime::parse_from_str("2014-5-7T12:34:56+09:30", "%Y-%m-%dT%H:%M:%S%z"),
        Ok(ymdhms(2014, 5, 7, 12, 34, 56))
    ); // ignore offset
    assert_eq!(
        NaiveDateTime::parse_from_str("2015-W06-1 000000", "%G-W%V-%u%H%M%S"),
        Ok(ymdhms(2015, 2, 2, 0, 0, 0))
    );
    assert_eq!(
        NaiveDateTime::parse_from_str("Fri, 09 Aug 2013 23:54:35 GMT", "%a, %d %b %Y %H:%M:%S GMT"),
        Ok(ymdhms(2013, 8, 9, 23, 54, 35))
    );
    assert!(
        NaiveDateTime::parse_from_str("Sat, 09 Aug 2013 23:54:35 GMT", "%a, %d %b %Y %H:%M:%S GMT")
            .is_err()
    );
    assert!(NaiveDateTime::parse_from_str("2014-5-7 Q2 12:3456", "%Y-%m-%d Q%q %H:%M:%S").is_err());
    assert!(NaiveDateTime::parse_from_str("12:34:56", "%H:%M:%S").is_err()); // insufficient
    assert_eq!(
        NaiveDateTime::parse_from_str("1441497364", "%s"),
        Ok(ymdhms(2015, 9, 5, 23, 56, 4))
    );
    assert_eq!(
        NaiveDateTime::parse_from_str("1283929614.1234", "%s.%f"),
        Ok(ymdhmsn(2010, 9, 8, 7, 6, 54, 1234))
    );
    assert_eq!(
        NaiveDateTime::parse_from_str("1441497364.649", "%s%.3f"),
        Ok(ymdhmsn(2015, 9, 5, 23, 56, 4, 649000000))
    );
    assert_eq!(
        NaiveDateTime::parse_from_str("1497854303.087654", "%s%.6f"),
        Ok(ymdhmsn(2017, 6, 19, 6, 38, 23, 87654000))
    );
    assert_eq!(
        NaiveDateTime::parse_from_str("1437742189.918273645", "%s%.9f"),
        Ok(ymdhmsn(2015, 7, 24, 12, 49, 49, 918273645))
    );
}

#[test]
fn test_datetime_parse_from_str_with_spaces() {
    let parse_from_str = NaiveDateTime::parse_from_str;
    let dt = NaiveDate::from_ymd_opt(2013, 8, 9).unwrap().and_hms_opt(23, 54, 35).unwrap();
    // with varying spaces - should succeed
    assert_eq!(parse_from_str(" Aug 09 2013 23:54:35", " %b %d %Y %H:%M:%S"), Ok(dt));
    assert_eq!(parse_from_str("Aug 09 2013 23:54:35 ", "%b %d %Y %H:%M:%S "), Ok(dt));
    assert_eq!(parse_from_str(" Aug 09 2013  23:54:35 ", " %b %d %Y  %H:%M:%S "), Ok(dt));
    assert_eq!(parse_from_str("  Aug 09 2013 23:54:35", "  %b %d %Y %H:%M:%S"), Ok(dt));
    assert_eq!(parse_from_str("   Aug 09 2013 23:54:35", "   %b %d %Y %H:%M:%S"), Ok(dt));
    assert_eq!(parse_from_str("\n\tAug 09 2013 23:54:35  ", "\n\t%b %d %Y %H:%M:%S  "), Ok(dt));
    assert_eq!(parse_from_str("\tAug 09 2013 23:54:35\t", "\t%b %d %Y %H:%M:%S\t"), Ok(dt));
    assert_eq!(parse_from_str("Aug  09 2013 23:54:35", "%b  %d %Y %H:%M:%S"), Ok(dt));
    assert_eq!(parse_from_str("Aug    09 2013 23:54:35", "%b    %d %Y %H:%M:%S"), Ok(dt));
    assert_eq!(parse_from_str("Aug  09 2013\t23:54:35", "%b  %d %Y\t%H:%M:%S"), Ok(dt));
    assert_eq!(parse_from_str("Aug  09 2013\t\t23:54:35", "%b  %d %Y\t\t%H:%M:%S"), Ok(dt));
    assert_eq!(parse_from_str("Aug 09 2013 23:54:35 ", "%b %d %Y %H:%M:%S\n"), Ok(dt));
    assert_eq!(parse_from_str("Aug 09 2013 23:54:35", "%b %d %Y\t%H:%M:%S"), Ok(dt));
    assert_eq!(parse_from_str("Aug 09 2013 23:54:35", "%b %d %Y %H:%M:%S "), Ok(dt));
    assert_eq!(parse_from_str("Aug 09 2013 23:54:35", " %b %d %Y %H:%M:%S"), Ok(dt));
    assert_eq!(parse_from_str("Aug 09 2013 23:54:35", "%b %d %Y %H:%M:%S\n"), Ok(dt));
    // with varying spaces - should fail
    // leading space in data
    assert!(parse_from_str(" Aug 09 2013 23:54:35", "%b %d %Y %H:%M:%S").is_err());
    // trailing space in data
    assert!(parse_from_str("Aug 09 2013 23:54:35 ", "%b %d %Y %H:%M:%S").is_err());
    // trailing tab in data
    assert!(parse_from_str("Aug 09 2013 23:54:35\t", "%b %d %Y %H:%M:%S").is_err());
    // mismatched newlines
    assert!(parse_from_str("\nAug 09 2013 23:54:35", "%b %d %Y %H:%M:%S\n").is_err());
    // trailing literal in data
    assert!(parse_from_str("Aug 09 2013 23:54:35 !!!", "%b %d %Y %H:%M:%S ").is_err());
}

#[test]
fn test_datetime_add_sub_invariant() {
    // issue #37
    let base = NaiveDate::from_ymd_opt(2000, 1, 1).unwrap().and_hms_opt(0, 0, 0).unwrap();
    let t = -946684799990000;
    let time = base + TimeDelta::microseconds(t);
    assert_eq!(t, time.signed_duration_since(base).num_microseconds().unwrap());
}

#[test]
fn test_and_local_timezone() {
    let ndt = NaiveDate::from_ymd_opt(2022, 6, 15).unwrap().and_hms_opt(18, 59, 36).unwrap();
    let dt_utc = ndt.and_utc();
    assert_eq!(dt_utc.naive_local(), ndt);
    assert_eq!(dt_utc.timezone(), Utc);

    let offset_tz = FixedOffset::west_opt(4 * 3600).unwrap();
    let dt_offset = ndt.and_local_timezone(offset_tz).unwrap();
    assert_eq!(dt_offset.naive_local(), ndt);
    assert_eq!(dt_offset.timezone(), offset_tz);
}

#[test]
fn test_and_utc() {
    let ndt = NaiveDate::from_ymd_opt(2023, 1, 30).unwrap().and_hms_opt(19, 32, 33).unwrap();
    let dt_utc = ndt.and_utc();
    assert_eq!(dt_utc.naive_local(), ndt);
    assert_eq!(dt_utc.timezone(), Utc);
}

#[test]
fn test_checked_add_offset() {
    let ymdhmsm = |y, m, d, h, mn, s, mi| {
        NaiveDate::from_ymd_opt(y, m, d).unwrap().and_hms_milli_opt(h, mn, s, mi)
    };

    let positive_offset = FixedOffset::east_opt(2 * 60 * 60).unwrap();
    // regular date
    let dt = ymdhmsm(2023, 5, 5, 20, 10, 0, 0).unwrap();
    assert_eq!(dt.checked_add_offset(positive_offset), ymdhmsm(2023, 5, 5, 22, 10, 0, 0));
    // leap second is preserved
    let dt = ymdhmsm(2023, 6, 30, 23, 59, 59, 1_000).unwrap();
    assert_eq!(dt.checked_add_offset(positive_offset), ymdhmsm(2023, 7, 1, 1, 59, 59, 1_000));
    // out of range
    assert!(NaiveDateTime::MAX.checked_add_offset(positive_offset).is_none());

    let negative_offset = FixedOffset::west_opt(2 * 60 * 60).unwrap();
    // regular date
    let dt = ymdhmsm(2023, 5, 5, 20, 10, 0, 0).unwrap();
    assert_eq!(dt.checked_add_offset(negative_offset), ymdhmsm(2023, 5, 5, 18, 10, 0, 0));
    // leap second is preserved
    let dt = ymdhmsm(2023, 6, 30, 23, 59, 59, 1_000).unwrap();
    assert_eq!(dt.checked_add_offset(negative_offset), ymdhmsm(2023, 6, 30, 21, 59, 59, 1_000));
    // out of range
    assert!(NaiveDateTime::MIN.checked_add_offset(negative_offset).is_none());
}

#[test]
fn test_checked_sub_offset() {
    let ymdhmsm = |y, m, d, h, mn, s, mi| {
        NaiveDate::from_ymd_opt(y, m, d).unwrap().and_hms_milli_opt(h, mn, s, mi)
    };

    let positive_offset = FixedOffset::east_opt(2 * 60 * 60).unwrap();
    // regular date
    let dt = ymdhmsm(2023, 5, 5, 20, 10, 0, 0).unwrap();
    assert_eq!(dt.checked_sub_offset(positive_offset), ymdhmsm(2023, 5, 5, 18, 10, 0, 0));
    // leap second is preserved
    let dt = ymdhmsm(2023, 6, 30, 23, 59, 59, 1_000).unwrap();
    assert_eq!(dt.checked_sub_offset(positive_offset), ymdhmsm(2023, 6, 30, 21, 59, 59, 1_000));
    // out of range
    assert!(NaiveDateTime::MIN.checked_sub_offset(positive_offset).is_none());

    let negative_offset = FixedOffset::west_opt(2 * 60 * 60).unwrap();
    // regular date
    let dt = ymdhmsm(2023, 5, 5, 20, 10, 0, 0).unwrap();
    assert_eq!(dt.checked_sub_offset(negative_offset), ymdhmsm(2023, 5, 5, 22, 10, 0, 0));
    // leap second is preserved
    let dt = ymdhmsm(2023, 6, 30, 23, 59, 59, 1_000).unwrap();
    assert_eq!(dt.checked_sub_offset(negative_offset), ymdhmsm(2023, 7, 1, 1, 59, 59, 1_000));
    // out of range
    assert!(NaiveDateTime::MAX.checked_sub_offset(negative_offset).is_none());

    assert_eq!(dt.checked_add_offset(positive_offset), Some(dt + positive_offset));
    assert_eq!(dt.checked_sub_offset(positive_offset), Some(dt - positive_offset));
}

#[test]
fn test_overflowing_add_offset() {
    let ymdhmsm = |y, m, d, h, mn, s, mi| {
        NaiveDate::from_ymd_opt(y, m, d).unwrap().and_hms_milli_opt(h, mn, s, mi).unwrap()
    };
    let positive_offset = FixedOffset::east_opt(2 * 60 * 60).unwrap();
    // regular date
    let dt = ymdhmsm(2023, 5, 5, 20, 10, 0, 0);
    assert_eq!(dt.overflowing_add_offset(positive_offset), ymdhmsm(2023, 5, 5, 22, 10, 0, 0));
    // leap second is preserved
    let dt = ymdhmsm(2023, 6, 30, 23, 59, 59, 1_000);
    assert_eq!(dt.overflowing_add_offset(positive_offset), ymdhmsm(2023, 7, 1, 1, 59, 59, 1_000));
    // out of range
    assert!(NaiveDateTime::MAX.overflowing_add_offset(positive_offset) > NaiveDateTime::MAX);

    let negative_offset = FixedOffset::west_opt(2 * 60 * 60).unwrap();
    // regular date
    let dt = ymdhmsm(2023, 5, 5, 20, 10, 0, 0);
    assert_eq!(dt.overflowing_add_offset(negative_offset), ymdhmsm(2023, 5, 5, 18, 10, 0, 0));
    // leap second is preserved
    let dt = ymdhmsm(2023, 6, 30, 23, 59, 59, 1_000);
    assert_eq!(dt.overflowing_add_offset(negative_offset), ymdhmsm(2023, 6, 30, 21, 59, 59, 1_000));
    // out of range
    assert!(NaiveDateTime::MIN.overflowing_add_offset(negative_offset) < NaiveDateTime::MIN);
}

#[test]
fn test_and_timezone_min_max_dates() {
    for offset_hour in -23..=23 {
        dbg!(offset_hour);
        let offset = FixedOffset::east_opt(offset_hour * 60 * 60).unwrap();

        let local_max = NaiveDateTime::MAX.and_local_timezone(offset);
        if offset_hour >= 0 {
            assert_eq!(local_max.unwrap().naive_local(), NaiveDateTime::MAX);
        } else {
            assert_eq!(local_max, MappedLocalTime::None);
        }
        let local_min = NaiveDateTime::MIN.and_local_timezone(offset);
        if offset_hour <= 0 {
            assert_eq!(local_min.unwrap().naive_local(), NaiveDateTime::MIN);
        } else {
            assert_eq!(local_min, MappedLocalTime::None);
        }
    }
}

#[test]
#[cfg(feature = "rkyv-validation")]
fn test_rkyv_validation() {
    let dt_min = NaiveDateTime::MIN;
    let bytes = rkyv::to_bytes::<_, 12>(&dt_min).unwrap();
    assert_eq!(rkyv::from_bytes::<NaiveDateTime>(&bytes).unwrap(), dt_min);

    let dt_max = NaiveDateTime::MAX;
    let bytes = rkyv::to_bytes::<_, 12>(&dt_max).unwrap();
    assert_eq!(rkyv::from_bytes::<NaiveDateTime>(&bytes).unwrap(), dt_max);
}
