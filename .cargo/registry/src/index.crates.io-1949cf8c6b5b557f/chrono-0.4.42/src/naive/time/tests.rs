use super::NaiveTime;
use crate::{FixedOffset, TimeDelta, Timelike};

#[test]
fn test_time_from_hms_milli() {
    assert_eq!(
        NaiveTime::from_hms_milli_opt(3, 5, 7, 0),
        Some(NaiveTime::from_hms_nano_opt(3, 5, 7, 0).unwrap())
    );
    assert_eq!(
        NaiveTime::from_hms_milli_opt(3, 5, 7, 777),
        Some(NaiveTime::from_hms_nano_opt(3, 5, 7, 777_000_000).unwrap())
    );
    assert_eq!(
        NaiveTime::from_hms_milli_opt(3, 5, 59, 1_999),
        Some(NaiveTime::from_hms_nano_opt(3, 5, 59, 1_999_000_000).unwrap())
    );
    assert_eq!(NaiveTime::from_hms_milli_opt(3, 5, 59, 2_000), None);
    assert_eq!(NaiveTime::from_hms_milli_opt(3, 5, 59, 5_000), None); // overflow check
    assert_eq!(NaiveTime::from_hms_milli_opt(3, 5, 59, u32::MAX), None);
}

#[test]
fn test_time_from_hms_micro() {
    assert_eq!(
        NaiveTime::from_hms_micro_opt(3, 5, 7, 0),
        Some(NaiveTime::from_hms_nano_opt(3, 5, 7, 0).unwrap())
    );
    assert_eq!(
        NaiveTime::from_hms_micro_opt(3, 5, 7, 333),
        Some(NaiveTime::from_hms_nano_opt(3, 5, 7, 333_000).unwrap())
    );
    assert_eq!(
        NaiveTime::from_hms_micro_opt(3, 5, 7, 777_777),
        Some(NaiveTime::from_hms_nano_opt(3, 5, 7, 777_777_000).unwrap())
    );
    assert_eq!(
        NaiveTime::from_hms_micro_opt(3, 5, 59, 1_999_999),
        Some(NaiveTime::from_hms_nano_opt(3, 5, 59, 1_999_999_000).unwrap())
    );
    assert_eq!(NaiveTime::from_hms_micro_opt(3, 5, 59, 2_000_000), None);
    assert_eq!(NaiveTime::from_hms_micro_opt(3, 5, 59, 5_000_000), None); // overflow check
    assert_eq!(NaiveTime::from_hms_micro_opt(3, 5, 59, u32::MAX), None);
}

#[test]
fn test_time_hms() {
    assert_eq!(NaiveTime::from_hms_opt(3, 5, 7).unwrap().hour(), 3);
    assert_eq!(
        NaiveTime::from_hms_opt(3, 5, 7).unwrap().with_hour(0),
        Some(NaiveTime::from_hms_opt(0, 5, 7).unwrap())
    );
    assert_eq!(
        NaiveTime::from_hms_opt(3, 5, 7).unwrap().with_hour(23),
        Some(NaiveTime::from_hms_opt(23, 5, 7).unwrap())
    );
    assert_eq!(NaiveTime::from_hms_opt(3, 5, 7).unwrap().with_hour(24), None);
    assert_eq!(NaiveTime::from_hms_opt(3, 5, 7).unwrap().with_hour(u32::MAX), None);

    assert_eq!(NaiveTime::from_hms_opt(3, 5, 7).unwrap().minute(), 5);
    assert_eq!(
        NaiveTime::from_hms_opt(3, 5, 7).unwrap().with_minute(0),
        Some(NaiveTime::from_hms_opt(3, 0, 7).unwrap())
    );
    assert_eq!(
        NaiveTime::from_hms_opt(3, 5, 7).unwrap().with_minute(59),
        Some(NaiveTime::from_hms_opt(3, 59, 7).unwrap())
    );
    assert_eq!(NaiveTime::from_hms_opt(3, 5, 7).unwrap().with_minute(60), None);
    assert_eq!(NaiveTime::from_hms_opt(3, 5, 7).unwrap().with_minute(u32::MAX), None);

    assert_eq!(NaiveTime::from_hms_opt(3, 5, 7).unwrap().second(), 7);
    assert_eq!(
        NaiveTime::from_hms_opt(3, 5, 7).unwrap().with_second(0),
        Some(NaiveTime::from_hms_opt(3, 5, 0).unwrap())
    );
    assert_eq!(
        NaiveTime::from_hms_opt(3, 5, 7).unwrap().with_second(59),
        Some(NaiveTime::from_hms_opt(3, 5, 59).unwrap())
    );
    assert_eq!(NaiveTime::from_hms_opt(3, 5, 7).unwrap().with_second(60), None);
    assert_eq!(NaiveTime::from_hms_opt(3, 5, 7).unwrap().with_second(u32::MAX), None);
}

#[test]
fn test_time_add() {
    macro_rules! check {
        ($lhs:expr, $rhs:expr, $sum:expr) => {{
            assert_eq!($lhs + $rhs, $sum);
            //assert_eq!($rhs + $lhs, $sum);
        }};
    }

    let hmsm = |h, m, s, ms| NaiveTime::from_hms_milli_opt(h, m, s, ms).unwrap();

    check!(hmsm(3, 5, 59, 900), TimeDelta::zero(), hmsm(3, 5, 59, 900));
    check!(hmsm(3, 5, 59, 900), TimeDelta::try_milliseconds(100).unwrap(), hmsm(3, 6, 0, 0));
    check!(hmsm(3, 5, 59, 1_300), TimeDelta::try_milliseconds(-1800).unwrap(), hmsm(3, 5, 58, 500));
    check!(hmsm(3, 5, 59, 1_300), TimeDelta::try_milliseconds(-800).unwrap(), hmsm(3, 5, 59, 500));
    check!(
        hmsm(3, 5, 59, 1_300),
        TimeDelta::try_milliseconds(-100).unwrap(),
        hmsm(3, 5, 59, 1_200)
    );
    check!(hmsm(3, 5, 59, 1_300), TimeDelta::try_milliseconds(100).unwrap(), hmsm(3, 5, 59, 1_400));
    check!(hmsm(3, 5, 59, 1_300), TimeDelta::try_milliseconds(800).unwrap(), hmsm(3, 6, 0, 100));
    check!(hmsm(3, 5, 59, 1_300), TimeDelta::try_milliseconds(1800).unwrap(), hmsm(3, 6, 1, 100));
    check!(hmsm(3, 5, 59, 900), TimeDelta::try_seconds(86399).unwrap(), hmsm(3, 5, 58, 900)); // overwrap
    check!(hmsm(3, 5, 59, 900), TimeDelta::try_seconds(-86399).unwrap(), hmsm(3, 6, 0, 900));
    check!(hmsm(3, 5, 59, 900), TimeDelta::try_days(12345).unwrap(), hmsm(3, 5, 59, 900));
    check!(hmsm(3, 5, 59, 1_300), TimeDelta::try_days(1).unwrap(), hmsm(3, 5, 59, 300));
    check!(hmsm(3, 5, 59, 1_300), TimeDelta::try_days(-1).unwrap(), hmsm(3, 6, 0, 300));

    // regression tests for #37
    check!(hmsm(0, 0, 0, 0), TimeDelta::try_milliseconds(-990).unwrap(), hmsm(23, 59, 59, 10));
    check!(hmsm(0, 0, 0, 0), TimeDelta::try_milliseconds(-9990).unwrap(), hmsm(23, 59, 50, 10));
}

#[test]
fn test_time_overflowing_add() {
    let hmsm = |h, m, s, ms| NaiveTime::from_hms_milli_opt(h, m, s, ms).unwrap();

    assert_eq!(
        hmsm(3, 4, 5, 678).overflowing_add_signed(TimeDelta::try_hours(11).unwrap()),
        (hmsm(14, 4, 5, 678), 0)
    );
    assert_eq!(
        hmsm(3, 4, 5, 678).overflowing_add_signed(TimeDelta::try_hours(23).unwrap()),
        (hmsm(2, 4, 5, 678), 86_400)
    );
    assert_eq!(
        hmsm(3, 4, 5, 678).overflowing_add_signed(TimeDelta::try_hours(-7).unwrap()),
        (hmsm(20, 4, 5, 678), -86_400)
    );

    // overflowing_add_signed with leap seconds may be counter-intuitive
    assert_eq!(
        hmsm(3, 4, 59, 1_678).overflowing_add_signed(TimeDelta::try_days(1).unwrap()),
        (hmsm(3, 4, 59, 678), 86_400)
    );
    assert_eq!(
        hmsm(3, 4, 59, 1_678).overflowing_add_signed(TimeDelta::try_days(-1).unwrap()),
        (hmsm(3, 5, 0, 678), -86_400)
    );
}

#[test]
fn test_time_addassignment() {
    let hms = |h, m, s| NaiveTime::from_hms_opt(h, m, s).unwrap();
    let mut time = hms(12, 12, 12);
    time += TimeDelta::try_hours(10).unwrap();
    assert_eq!(time, hms(22, 12, 12));
    time += TimeDelta::try_hours(10).unwrap();
    assert_eq!(time, hms(8, 12, 12));
}

#[test]
fn test_time_subassignment() {
    let hms = |h, m, s| NaiveTime::from_hms_opt(h, m, s).unwrap();
    let mut time = hms(12, 12, 12);
    time -= TimeDelta::try_hours(10).unwrap();
    assert_eq!(time, hms(2, 12, 12));
    time -= TimeDelta::try_hours(10).unwrap();
    assert_eq!(time, hms(16, 12, 12));
}

#[test]
fn test_time_sub() {
    macro_rules! check {
        ($lhs:expr, $rhs:expr, $diff:expr) => {{
            // `time1 - time2 = duration` is equivalent to `time2 - time1 = -duration`
            assert_eq!($lhs.signed_duration_since($rhs), $diff);
            assert_eq!($rhs.signed_duration_since($lhs), -$diff);
        }};
    }

    let hmsm = |h, m, s, ms| NaiveTime::from_hms_milli_opt(h, m, s, ms).unwrap();

    check!(hmsm(3, 5, 7, 900), hmsm(3, 5, 7, 900), TimeDelta::zero());
    check!(hmsm(3, 5, 7, 900), hmsm(3, 5, 7, 600), TimeDelta::try_milliseconds(300).unwrap());
    check!(hmsm(3, 5, 7, 200), hmsm(2, 4, 6, 200), TimeDelta::try_seconds(3600 + 60 + 1).unwrap());
    check!(
        hmsm(3, 5, 7, 200),
        hmsm(2, 4, 6, 300),
        TimeDelta::try_seconds(3600 + 60).unwrap() + TimeDelta::try_milliseconds(900).unwrap()
    );

    // treats the leap second as if it coincides with the prior non-leap second,
    // as required by `time1 - time2 = duration` and `time2 - time1 = -duration` equivalence.
    check!(hmsm(3, 6, 0, 200), hmsm(3, 5, 59, 1_800), TimeDelta::try_milliseconds(400).unwrap());
    //check!(hmsm(3, 5, 7, 1_200), hmsm(3, 5, 6, 1_800), TimeDelta::try_milliseconds(1400).unwrap());
    //check!(hmsm(3, 5, 7, 1_200), hmsm(3, 5, 6, 800), TimeDelta::try_milliseconds(1400).unwrap());

    // additional equality: `time1 + duration = time2` is equivalent to
    // `time2 - time1 = duration` IF AND ONLY IF `time2` represents a non-leap second.
    assert_eq!(hmsm(3, 5, 6, 800) + TimeDelta::try_milliseconds(400).unwrap(), hmsm(3, 5, 7, 200));
    //assert_eq!(hmsm(3, 5, 6, 1_800) + TimeDelta::try_milliseconds(400).unwrap(), hmsm(3, 5, 7, 200));
}

#[test]
fn test_core_duration_ops() {
    use core::time::Duration;

    let mut t = NaiveTime::from_hms_opt(11, 34, 23).unwrap();
    let same = t + Duration::ZERO;
    assert_eq!(t, same);

    t += Duration::new(3600, 0);
    assert_eq!(t, NaiveTime::from_hms_opt(12, 34, 23).unwrap());

    t -= Duration::new(7200, 0);
    assert_eq!(t, NaiveTime::from_hms_opt(10, 34, 23).unwrap());
}

#[test]
fn test_time_fmt() {
    assert_eq!(
        format!("{}", NaiveTime::from_hms_milli_opt(23, 59, 59, 999).unwrap()),
        "23:59:59.999"
    );
    assert_eq!(
        format!("{}", NaiveTime::from_hms_milli_opt(23, 59, 59, 1_000).unwrap()),
        "23:59:60"
    );
    assert_eq!(
        format!("{}", NaiveTime::from_hms_milli_opt(23, 59, 59, 1_001).unwrap()),
        "23:59:60.001"
    );
    assert_eq!(
        format!("{}", NaiveTime::from_hms_micro_opt(0, 0, 0, 43210).unwrap()),
        "00:00:00.043210"
    );
    assert_eq!(
        format!("{}", NaiveTime::from_hms_nano_opt(0, 0, 0, 6543210).unwrap()),
        "00:00:00.006543210"
    );

    // the format specifier should have no effect on `NaiveTime`
    assert_eq!(
        format!("{:30}", NaiveTime::from_hms_milli_opt(3, 5, 7, 9).unwrap()),
        "03:05:07.009"
    );
}

#[test]
fn test_time_from_str() {
    // valid cases
    let valid = [
        "0:0:0",
        "0:0:0.0000000",
        "0:0:0.0000003",
        " 4 : 3 : 2.1 ",
        " 09:08:07 ",
        " 09:08 ",
        " 9:8:07 ",
        "01:02:03",
        "4:3:2.1",
        "9:8:7",
        "09:8:7",
        "9:08:7",
        "9:8:07",
        "09:08:7",
        "09:8:07",
        "09:08:7",
        "9:08:07",
        "09:08:07",
        "9:8:07.123",
        "9:08:7.123",
        "09:8:7.123",
        "09:08:7.123",
        "9:08:07.123",
        "09:8:07.123",
        "09:08:07.123",
        "09:08:07.123",
        "09:08:07.1234",
        "09:08:07.12345",
        "09:08:07.123456",
        "09:08:07.1234567",
        "09:08:07.12345678",
        "09:08:07.123456789",
        "09:08:07.1234567891",
        "09:08:07.12345678912",
        "23:59:60.373929310237",
    ];
    for &s in &valid {
        eprintln!("test_time_parse_from_str valid {s:?}");
        let d = match s.parse::<NaiveTime>() {
            Ok(d) => d,
            Err(e) => panic!("parsing `{s}` has failed: {e}"),
        };
        let s_ = format!("{d:?}");
        // `s` and `s_` may differ, but `s.parse()` and `s_.parse()` must be same
        let d_ = match s_.parse::<NaiveTime>() {
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
        "",                  // empty
        "x",                 // invalid
        "15",                // missing data
        "15:8:",             // trailing colon
        "15:8:x",            // invalid data
        "15:8:9x",           // invalid data
        "23:59:61",          // invalid second (out of bounds)
        "23:54:35 GMT",      // invalid (timezone non-sensical for NaiveTime)
        "23:54:35 +0000",    // invalid (timezone non-sensical for NaiveTime)
        "1441497364.649",    // valid datetime, not a NaiveTime
        "+1441497364.649",   // valid datetime, not a NaiveTime
        "+1441497364",       // valid datetime, not a NaiveTime
        "001:02:03",         // invalid hour
        "01:002:03",         // invalid minute
        "01:02:003",         // invalid second
        "12:34:56.x",        // invalid fraction
        "12:34:56. 0",       // invalid fraction format
        "09:08:00000000007", // invalid second / invalid fraction format
    ];
    for &s in &invalid {
        eprintln!("test_time_parse_from_str invalid {s:?}");
        assert!(s.parse::<NaiveTime>().is_err());
    }
}

#[test]
fn test_time_parse_from_str() {
    let hms = |h, m, s| NaiveTime::from_hms_opt(h, m, s).unwrap();
    assert_eq!(
        NaiveTime::parse_from_str("2014-5-7T12:34:56+09:30", "%Y-%m-%dT%H:%M:%S%z"),
        Ok(hms(12, 34, 56))
    ); // ignore date and offset
    assert_eq!(NaiveTime::parse_from_str("PM 12:59", "%P %H:%M"), Ok(hms(12, 59, 0)));
    assert_eq!(NaiveTime::parse_from_str("12:59 \n\t PM", "%H:%M \n\t %P"), Ok(hms(12, 59, 0)));
    assert_eq!(NaiveTime::parse_from_str("\t\t12:59\tPM\t", "\t\t%H:%M\t%P\t"), Ok(hms(12, 59, 0)));
    assert_eq!(
        NaiveTime::parse_from_str("\t\t1259\t\tPM\t", "\t\t%H%M\t\t%P\t"),
        Ok(hms(12, 59, 0))
    );
    assert!(NaiveTime::parse_from_str("12:59 PM", "%H:%M\t%P").is_ok());
    assert!(NaiveTime::parse_from_str("\t\t12:59 PM\t", "\t\t%H:%M\t%P\t").is_ok());
    assert!(NaiveTime::parse_from_str("12:59  PM", "%H:%M %P").is_ok());
    assert!(NaiveTime::parse_from_str("12:3456", "%H:%M:%S").is_err());
}

#[test]
fn test_overflowing_offset() {
    let hmsm = |h, m, s, n| NaiveTime::from_hms_milli_opt(h, m, s, n).unwrap();

    let positive_offset = FixedOffset::east_opt(4 * 60 * 60).unwrap();
    // regular time
    let t = hmsm(5, 6, 7, 890);
    assert_eq!(t.overflowing_add_offset(positive_offset), (hmsm(9, 6, 7, 890), 0));
    assert_eq!(t.overflowing_sub_offset(positive_offset), (hmsm(1, 6, 7, 890), 0));
    // leap second is preserved, and wrap to next day
    let t = hmsm(23, 59, 59, 1_000);
    assert_eq!(t.overflowing_add_offset(positive_offset), (hmsm(3, 59, 59, 1_000), 1));
    assert_eq!(t.overflowing_sub_offset(positive_offset), (hmsm(19, 59, 59, 1_000), 0));
    // wrap to previous day
    let t = hmsm(1, 2, 3, 456);
    assert_eq!(t.overflowing_sub_offset(positive_offset), (hmsm(21, 2, 3, 456), -1));
    // an odd offset
    let negative_offset = FixedOffset::west_opt(((2 * 60) + 3) * 60 + 4).unwrap();
    let t = hmsm(5, 6, 7, 890);
    assert_eq!(t.overflowing_add_offset(negative_offset), (hmsm(3, 3, 3, 890), 0));
    assert_eq!(t.overflowing_sub_offset(negative_offset), (hmsm(7, 9, 11, 890), 0));

    assert_eq!(t.overflowing_add_offset(positive_offset).0, t + positive_offset);
    assert_eq!(t.overflowing_sub_offset(positive_offset).0, t - positive_offset);
}

#[test]
#[cfg(feature = "rkyv-validation")]
fn test_rkyv_validation() {
    let t_min = NaiveTime::MIN;
    let bytes = rkyv::to_bytes::<_, 8>(&t_min).unwrap();
    assert_eq!(rkyv::from_bytes::<NaiveTime>(&bytes).unwrap(), t_min);

    let t_max = NaiveTime::MAX;
    let bytes = rkyv::to_bytes::<_, 8>(&t_max).unwrap();
    assert_eq!(rkyv::from_bytes::<NaiveTime>(&bytes).unwrap(), t_max);
}
