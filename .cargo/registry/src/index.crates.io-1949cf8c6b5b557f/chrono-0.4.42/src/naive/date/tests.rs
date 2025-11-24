use super::{Days, MAX_YEAR, MIN_YEAR, Months, NaiveDate};
use crate::naive::internals::{A, AG, B, BA, C, CB, D, DC, E, ED, F, FE, G, GF, YearFlags};
use crate::{Datelike, TimeDelta, Weekday};

// as it is hard to verify year flags in `NaiveDate::MIN` and `NaiveDate::MAX`,
// we use a separate run-time test.
#[test]
fn test_date_bounds() {
    let calculated_min = NaiveDate::from_ymd_opt(MIN_YEAR, 1, 1).unwrap();
    let calculated_max = NaiveDate::from_ymd_opt(MAX_YEAR, 12, 31).unwrap();
    assert!(
        NaiveDate::MIN == calculated_min,
        "`NaiveDate::MIN` should have year flag {:?}",
        calculated_min.year_flags()
    );
    assert!(
        NaiveDate::MAX == calculated_max,
        "`NaiveDate::MAX` should have year flag {:?} and ordinal {}",
        calculated_max.year_flags(),
        calculated_max.ordinal()
    );

    // let's also check that the entire range do not exceed 2^44 seconds
    // (sometimes used for bounding `TimeDelta` against overflow)
    let maxsecs = NaiveDate::MAX.signed_duration_since(NaiveDate::MIN).num_seconds();
    let maxsecs = maxsecs + 86401; // also take care of DateTime
    assert!(
        maxsecs < (1 << MAX_BITS),
        "The entire `NaiveDate` range somehow exceeds 2^{MAX_BITS} seconds"
    );

    const BEFORE_MIN: NaiveDate = NaiveDate::BEFORE_MIN;
    assert_eq!(BEFORE_MIN.year_flags(), YearFlags::from_year(BEFORE_MIN.year()));
    assert_eq!((BEFORE_MIN.month(), BEFORE_MIN.day()), (12, 31));

    const AFTER_MAX: NaiveDate = NaiveDate::AFTER_MAX;
    assert_eq!(AFTER_MAX.year_flags(), YearFlags::from_year(AFTER_MAX.year()));
    assert_eq!((AFTER_MAX.month(), AFTER_MAX.day()), (1, 1));
}

#[test]
fn diff_months() {
    // identity
    assert_eq!(
        NaiveDate::from_ymd_opt(2022, 8, 3).unwrap().checked_add_months(Months::new(0)),
        Some(NaiveDate::from_ymd_opt(2022, 8, 3).unwrap())
    );

    // add with months exceeding `i32::MAX`
    assert_eq!(
        NaiveDate::from_ymd_opt(2022, 8, 3)
            .unwrap()
            .checked_add_months(Months::new(i32::MAX as u32 + 1)),
        None
    );

    // sub with months exceeding `i32::MIN`
    assert_eq!(
        NaiveDate::from_ymd_opt(2022, 8, 3)
            .unwrap()
            .checked_sub_months(Months::new(i32::MIN.unsigned_abs() + 1)),
        None
    );

    // add overflowing year
    assert_eq!(NaiveDate::MAX.checked_add_months(Months::new(1)), None);

    // add underflowing year
    assert_eq!(NaiveDate::MIN.checked_sub_months(Months::new(1)), None);

    // sub crossing year 0 boundary
    assert_eq!(
        NaiveDate::from_ymd_opt(2022, 8, 3).unwrap().checked_sub_months(Months::new(2050 * 12)),
        Some(NaiveDate::from_ymd_opt(-28, 8, 3).unwrap())
    );

    // add crossing year boundary
    assert_eq!(
        NaiveDate::from_ymd_opt(2022, 8, 3).unwrap().checked_add_months(Months::new(6)),
        Some(NaiveDate::from_ymd_opt(2023, 2, 3).unwrap())
    );

    // sub crossing year boundary
    assert_eq!(
        NaiveDate::from_ymd_opt(2022, 8, 3).unwrap().checked_sub_months(Months::new(10)),
        Some(NaiveDate::from_ymd_opt(2021, 10, 3).unwrap())
    );

    // add clamping day, non-leap year
    assert_eq!(
        NaiveDate::from_ymd_opt(2022, 1, 29).unwrap().checked_add_months(Months::new(1)),
        Some(NaiveDate::from_ymd_opt(2022, 2, 28).unwrap())
    );

    // add to leap day
    assert_eq!(
        NaiveDate::from_ymd_opt(2022, 10, 29).unwrap().checked_add_months(Months::new(16)),
        Some(NaiveDate::from_ymd_opt(2024, 2, 29).unwrap())
    );

    // add into december
    assert_eq!(
        NaiveDate::from_ymd_opt(2022, 10, 31).unwrap().checked_add_months(Months::new(2)),
        Some(NaiveDate::from_ymd_opt(2022, 12, 31).unwrap())
    );

    // sub into december
    assert_eq!(
        NaiveDate::from_ymd_opt(2022, 10, 31).unwrap().checked_sub_months(Months::new(10)),
        Some(NaiveDate::from_ymd_opt(2021, 12, 31).unwrap())
    );

    // add into january
    assert_eq!(
        NaiveDate::from_ymd_opt(2022, 8, 3).unwrap().checked_add_months(Months::new(5)),
        Some(NaiveDate::from_ymd_opt(2023, 1, 3).unwrap())
    );

    // sub into january
    assert_eq!(
        NaiveDate::from_ymd_opt(2022, 8, 3).unwrap().checked_sub_months(Months::new(7)),
        Some(NaiveDate::from_ymd_opt(2022, 1, 3).unwrap())
    );
}

#[test]
fn test_readme_doomsday() {
    for y in NaiveDate::MIN.year()..=NaiveDate::MAX.year() {
        // even months
        let d4 = NaiveDate::from_ymd_opt(y, 4, 4).unwrap();
        let d6 = NaiveDate::from_ymd_opt(y, 6, 6).unwrap();
        let d8 = NaiveDate::from_ymd_opt(y, 8, 8).unwrap();
        let d10 = NaiveDate::from_ymd_opt(y, 10, 10).unwrap();
        let d12 = NaiveDate::from_ymd_opt(y, 12, 12).unwrap();

        // nine to five, seven-eleven
        let d59 = NaiveDate::from_ymd_opt(y, 5, 9).unwrap();
        let d95 = NaiveDate::from_ymd_opt(y, 9, 5).unwrap();
        let d711 = NaiveDate::from_ymd_opt(y, 7, 11).unwrap();
        let d117 = NaiveDate::from_ymd_opt(y, 11, 7).unwrap();

        // "March 0"
        let d30 = NaiveDate::from_ymd_opt(y, 3, 1).unwrap().pred_opt().unwrap();

        let weekday = d30.weekday();
        let other_dates = [d4, d6, d8, d10, d12, d59, d95, d711, d117];
        assert!(other_dates.iter().all(|d| d.weekday() == weekday));
    }
}

#[test]
fn test_date_from_ymd() {
    let from_ymd = NaiveDate::from_ymd_opt;

    assert!(from_ymd(2012, 0, 1).is_none());
    assert!(from_ymd(2012, 1, 1).is_some());
    assert!(from_ymd(2012, 2, 29).is_some());
    assert!(from_ymd(2014, 2, 29).is_none());
    assert!(from_ymd(2014, 3, 0).is_none());
    assert!(from_ymd(2014, 3, 1).is_some());
    assert!(from_ymd(2014, 3, 31).is_some());
    assert!(from_ymd(2014, 3, 32).is_none());
    assert!(from_ymd(2014, 12, 31).is_some());
    assert!(from_ymd(2014, 13, 1).is_none());
}

#[test]
fn test_date_from_yo() {
    let from_yo = NaiveDate::from_yo_opt;
    let ymd = |y, m, d| NaiveDate::from_ymd_opt(y, m, d).unwrap();

    assert_eq!(from_yo(2012, 0), None);
    assert_eq!(from_yo(2012, 1), Some(ymd(2012, 1, 1)));
    assert_eq!(from_yo(2012, 2), Some(ymd(2012, 1, 2)));
    assert_eq!(from_yo(2012, 32), Some(ymd(2012, 2, 1)));
    assert_eq!(from_yo(2012, 60), Some(ymd(2012, 2, 29)));
    assert_eq!(from_yo(2012, 61), Some(ymd(2012, 3, 1)));
    assert_eq!(from_yo(2012, 100), Some(ymd(2012, 4, 9)));
    assert_eq!(from_yo(2012, 200), Some(ymd(2012, 7, 18)));
    assert_eq!(from_yo(2012, 300), Some(ymd(2012, 10, 26)));
    assert_eq!(from_yo(2012, 366), Some(ymd(2012, 12, 31)));
    assert_eq!(from_yo(2012, 367), None);
    assert_eq!(from_yo(2012, (1 << 28) | 60), None);

    assert_eq!(from_yo(2014, 0), None);
    assert_eq!(from_yo(2014, 1), Some(ymd(2014, 1, 1)));
    assert_eq!(from_yo(2014, 2), Some(ymd(2014, 1, 2)));
    assert_eq!(from_yo(2014, 32), Some(ymd(2014, 2, 1)));
    assert_eq!(from_yo(2014, 59), Some(ymd(2014, 2, 28)));
    assert_eq!(from_yo(2014, 60), Some(ymd(2014, 3, 1)));
    assert_eq!(from_yo(2014, 100), Some(ymd(2014, 4, 10)));
    assert_eq!(from_yo(2014, 200), Some(ymd(2014, 7, 19)));
    assert_eq!(from_yo(2014, 300), Some(ymd(2014, 10, 27)));
    assert_eq!(from_yo(2014, 365), Some(ymd(2014, 12, 31)));
    assert_eq!(from_yo(2014, 366), None);
}

#[test]
fn test_date_from_isoywd() {
    let from_isoywd = NaiveDate::from_isoywd_opt;
    let ymd = |y, m, d| NaiveDate::from_ymd_opt(y, m, d).unwrap();

    assert_eq!(from_isoywd(2004, 0, Weekday::Sun), None);
    assert_eq!(from_isoywd(2004, 1, Weekday::Mon), Some(ymd(2003, 12, 29)));
    assert_eq!(from_isoywd(2004, 1, Weekday::Sun), Some(ymd(2004, 1, 4)));
    assert_eq!(from_isoywd(2004, 2, Weekday::Mon), Some(ymd(2004, 1, 5)));
    assert_eq!(from_isoywd(2004, 2, Weekday::Sun), Some(ymd(2004, 1, 11)));
    assert_eq!(from_isoywd(2004, 52, Weekday::Mon), Some(ymd(2004, 12, 20)));
    assert_eq!(from_isoywd(2004, 52, Weekday::Sun), Some(ymd(2004, 12, 26)));
    assert_eq!(from_isoywd(2004, 53, Weekday::Mon), Some(ymd(2004, 12, 27)));
    assert_eq!(from_isoywd(2004, 53, Weekday::Sun), Some(ymd(2005, 1, 2)));
    assert_eq!(from_isoywd(2004, 54, Weekday::Mon), None);

    assert_eq!(from_isoywd(2011, 0, Weekday::Sun), None);
    assert_eq!(from_isoywd(2011, 1, Weekday::Mon), Some(ymd(2011, 1, 3)));
    assert_eq!(from_isoywd(2011, 1, Weekday::Sun), Some(ymd(2011, 1, 9)));
    assert_eq!(from_isoywd(2011, 2, Weekday::Mon), Some(ymd(2011, 1, 10)));
    assert_eq!(from_isoywd(2011, 2, Weekday::Sun), Some(ymd(2011, 1, 16)));

    assert_eq!(from_isoywd(2018, 51, Weekday::Mon), Some(ymd(2018, 12, 17)));
    assert_eq!(from_isoywd(2018, 51, Weekday::Sun), Some(ymd(2018, 12, 23)));
    assert_eq!(from_isoywd(2018, 52, Weekday::Mon), Some(ymd(2018, 12, 24)));
    assert_eq!(from_isoywd(2018, 52, Weekday::Sun), Some(ymd(2018, 12, 30)));
    assert_eq!(from_isoywd(2018, 53, Weekday::Mon), None);
}

#[test]
fn test_date_from_isoywd_and_iso_week() {
    for year in 2000..2401 {
        for week in 1..54 {
            for &weekday in [
                Weekday::Mon,
                Weekday::Tue,
                Weekday::Wed,
                Weekday::Thu,
                Weekday::Fri,
                Weekday::Sat,
                Weekday::Sun,
            ]
            .iter()
            {
                let d = NaiveDate::from_isoywd_opt(year, week, weekday);
                if let Some(d) = d {
                    assert_eq!(d.weekday(), weekday);
                    let w = d.iso_week();
                    assert_eq!(w.year(), year);
                    assert_eq!(w.week(), week);
                }
            }
        }
    }

    for year in 2000..2401 {
        for month in 1..13 {
            for day in 1..32 {
                let d = NaiveDate::from_ymd_opt(year, month, day);
                if let Some(d) = d {
                    let w = d.iso_week();
                    let d_ = NaiveDate::from_isoywd_opt(w.year(), w.week(), d.weekday());
                    assert_eq!(d, d_.unwrap());
                }
            }
        }
    }
}

#[test]
fn test_date_from_num_days_from_ce() {
    let from_ndays_from_ce = NaiveDate::from_num_days_from_ce_opt;
    assert_eq!(from_ndays_from_ce(1), Some(NaiveDate::from_ymd_opt(1, 1, 1).unwrap()));
    assert_eq!(from_ndays_from_ce(2), Some(NaiveDate::from_ymd_opt(1, 1, 2).unwrap()));
    assert_eq!(from_ndays_from_ce(31), Some(NaiveDate::from_ymd_opt(1, 1, 31).unwrap()));
    assert_eq!(from_ndays_from_ce(32), Some(NaiveDate::from_ymd_opt(1, 2, 1).unwrap()));
    assert_eq!(from_ndays_from_ce(59), Some(NaiveDate::from_ymd_opt(1, 2, 28).unwrap()));
    assert_eq!(from_ndays_from_ce(60), Some(NaiveDate::from_ymd_opt(1, 3, 1).unwrap()));
    assert_eq!(from_ndays_from_ce(365), Some(NaiveDate::from_ymd_opt(1, 12, 31).unwrap()));
    assert_eq!(from_ndays_from_ce(365 + 1), Some(NaiveDate::from_ymd_opt(2, 1, 1).unwrap()));
    assert_eq!(from_ndays_from_ce(365 * 2 + 1), Some(NaiveDate::from_ymd_opt(3, 1, 1).unwrap()));
    assert_eq!(from_ndays_from_ce(365 * 3 + 1), Some(NaiveDate::from_ymd_opt(4, 1, 1).unwrap()));
    assert_eq!(from_ndays_from_ce(365 * 4 + 2), Some(NaiveDate::from_ymd_opt(5, 1, 1).unwrap()));
    assert_eq!(from_ndays_from_ce(146097 + 1), Some(NaiveDate::from_ymd_opt(401, 1, 1).unwrap()));
    assert_eq!(
        from_ndays_from_ce(146097 * 5 + 1),
        Some(NaiveDate::from_ymd_opt(2001, 1, 1).unwrap())
    );
    assert_eq!(from_ndays_from_ce(719163), Some(NaiveDate::from_ymd_opt(1970, 1, 1).unwrap()));
    assert_eq!(from_ndays_from_ce(0), Some(NaiveDate::from_ymd_opt(0, 12, 31).unwrap())); // 1 BCE
    assert_eq!(from_ndays_from_ce(-365), Some(NaiveDate::from_ymd_opt(0, 1, 1).unwrap()));
    assert_eq!(from_ndays_from_ce(-366), Some(NaiveDate::from_ymd_opt(-1, 12, 31).unwrap())); // 2 BCE

    for days in (-9999..10001).map(|x| x * 100) {
        assert_eq!(from_ndays_from_ce(days).map(|d| d.num_days_from_ce()), Some(days));
    }

    assert_eq!(from_ndays_from_ce(NaiveDate::MIN.num_days_from_ce()), Some(NaiveDate::MIN));
    assert_eq!(from_ndays_from_ce(NaiveDate::MIN.num_days_from_ce() - 1), None);
    assert_eq!(from_ndays_from_ce(NaiveDate::MAX.num_days_from_ce()), Some(NaiveDate::MAX));
    assert_eq!(from_ndays_from_ce(NaiveDate::MAX.num_days_from_ce() + 1), None);

    assert_eq!(from_ndays_from_ce(i32::MIN), None);
    assert_eq!(from_ndays_from_ce(i32::MAX), None);
}

#[test]
fn test_date_from_epoch_days() {
    let from_epoch_days = NaiveDate::from_epoch_days;
    assert_eq!(from_epoch_days(-719_162), Some(NaiveDate::from_ymd_opt(1, 1, 1).unwrap()));
    assert_eq!(from_epoch_days(0), Some(NaiveDate::from_ymd_opt(1970, 1, 1).unwrap()));
    assert_eq!(from_epoch_days(1), Some(NaiveDate::from_ymd_opt(1970, 1, 2).unwrap()));
    assert_eq!(from_epoch_days(2), Some(NaiveDate::from_ymd_opt(1970, 1, 3).unwrap()));
    assert_eq!(from_epoch_days(30), Some(NaiveDate::from_ymd_opt(1970, 1, 31).unwrap()));
    assert_eq!(from_epoch_days(31), Some(NaiveDate::from_ymd_opt(1970, 2, 1).unwrap()));
    assert_eq!(from_epoch_days(58), Some(NaiveDate::from_ymd_opt(1970, 2, 28).unwrap()));
    assert_eq!(from_epoch_days(59), Some(NaiveDate::from_ymd_opt(1970, 3, 1).unwrap()));
    assert_eq!(from_epoch_days(364), Some(NaiveDate::from_ymd_opt(1970, 12, 31).unwrap()));
    assert_eq!(from_epoch_days(365), Some(NaiveDate::from_ymd_opt(1971, 1, 1).unwrap()));
    assert_eq!(from_epoch_days(365 * 2), Some(NaiveDate::from_ymd_opt(1972, 1, 1).unwrap()));
    assert_eq!(from_epoch_days(365 * 3 + 1), Some(NaiveDate::from_ymd_opt(1973, 1, 1).unwrap()));
    assert_eq!(from_epoch_days(365 * 4 + 1), Some(NaiveDate::from_ymd_opt(1974, 1, 1).unwrap()));
    assert_eq!(from_epoch_days(13036), Some(NaiveDate::from_ymd_opt(2005, 9, 10).unwrap()));
    assert_eq!(from_epoch_days(-365), Some(NaiveDate::from_ymd_opt(1969, 1, 1).unwrap()));
    assert_eq!(from_epoch_days(-366), Some(NaiveDate::from_ymd_opt(1968, 12, 31).unwrap()));

    for days in (-9999..10001).map(|x| x * 100) {
        assert_eq!(from_epoch_days(days).map(|d| d.to_epoch_days()), Some(days));
    }

    assert_eq!(from_epoch_days(NaiveDate::MIN.to_epoch_days()), Some(NaiveDate::MIN));
    assert_eq!(from_epoch_days(NaiveDate::MIN.to_epoch_days() - 1), None);
    assert_eq!(from_epoch_days(NaiveDate::MAX.to_epoch_days()), Some(NaiveDate::MAX));
    assert_eq!(from_epoch_days(NaiveDate::MAX.to_epoch_days() + 1), None);

    assert_eq!(from_epoch_days(i32::MIN), None);
    assert_eq!(from_epoch_days(i32::MAX), None);
}

#[test]
fn test_date_from_weekday_of_month_opt() {
    let ymwd = NaiveDate::from_weekday_of_month_opt;
    assert_eq!(ymwd(2018, 8, Weekday::Tue, 0), None);
    assert_eq!(ymwd(2018, 8, Weekday::Wed, 1), Some(NaiveDate::from_ymd_opt(2018, 8, 1).unwrap()));
    assert_eq!(ymwd(2018, 8, Weekday::Thu, 1), Some(NaiveDate::from_ymd_opt(2018, 8, 2).unwrap()));
    assert_eq!(ymwd(2018, 8, Weekday::Sun, 1), Some(NaiveDate::from_ymd_opt(2018, 8, 5).unwrap()));
    assert_eq!(ymwd(2018, 8, Weekday::Mon, 1), Some(NaiveDate::from_ymd_opt(2018, 8, 6).unwrap()));
    assert_eq!(ymwd(2018, 8, Weekday::Tue, 1), Some(NaiveDate::from_ymd_opt(2018, 8, 7).unwrap()));
    assert_eq!(ymwd(2018, 8, Weekday::Wed, 2), Some(NaiveDate::from_ymd_opt(2018, 8, 8).unwrap()));
    assert_eq!(ymwd(2018, 8, Weekday::Sun, 2), Some(NaiveDate::from_ymd_opt(2018, 8, 12).unwrap()));
    assert_eq!(ymwd(2018, 8, Weekday::Thu, 3), Some(NaiveDate::from_ymd_opt(2018, 8, 16).unwrap()));
    assert_eq!(ymwd(2018, 8, Weekday::Thu, 4), Some(NaiveDate::from_ymd_opt(2018, 8, 23).unwrap()));
    assert_eq!(ymwd(2018, 8, Weekday::Thu, 5), Some(NaiveDate::from_ymd_opt(2018, 8, 30).unwrap()));
    assert_eq!(ymwd(2018, 8, Weekday::Fri, 5), Some(NaiveDate::from_ymd_opt(2018, 8, 31).unwrap()));
    assert_eq!(ymwd(2018, 8, Weekday::Sat, 5), None);
}

#[test]
fn test_date_fields() {
    fn check(year: i32, month: u32, day: u32, ordinal: u32) {
        let d1 = NaiveDate::from_ymd_opt(year, month, day).unwrap();
        assert_eq!(d1.year(), year);
        assert_eq!(d1.month(), month);
        assert_eq!(d1.day(), day);
        assert_eq!(d1.ordinal(), ordinal);

        let d2 = NaiveDate::from_yo_opt(year, ordinal).unwrap();
        assert_eq!(d2.year(), year);
        assert_eq!(d2.month(), month);
        assert_eq!(d2.day(), day);
        assert_eq!(d2.ordinal(), ordinal);

        assert_eq!(d1, d2);
    }

    check(2012, 1, 1, 1);
    check(2012, 1, 2, 2);
    check(2012, 2, 1, 32);
    check(2012, 2, 29, 60);
    check(2012, 3, 1, 61);
    check(2012, 4, 9, 100);
    check(2012, 7, 18, 200);
    check(2012, 10, 26, 300);
    check(2012, 12, 31, 366);

    check(2014, 1, 1, 1);
    check(2014, 1, 2, 2);
    check(2014, 2, 1, 32);
    check(2014, 2, 28, 59);
    check(2014, 3, 1, 60);
    check(2014, 4, 10, 100);
    check(2014, 7, 19, 200);
    check(2014, 10, 27, 300);
    check(2014, 12, 31, 365);
}

#[test]
fn test_date_weekday() {
    assert_eq!(NaiveDate::from_ymd_opt(1582, 10, 15).unwrap().weekday(), Weekday::Fri);
    // May 20, 1875 = ISO 8601 reference date
    assert_eq!(NaiveDate::from_ymd_opt(1875, 5, 20).unwrap().weekday(), Weekday::Thu);
    assert_eq!(NaiveDate::from_ymd_opt(2000, 1, 1).unwrap().weekday(), Weekday::Sat);
}

#[test]
fn test_date_with_fields() {
    let d = NaiveDate::from_ymd_opt(2000, 2, 29).unwrap();
    assert_eq!(d.with_year(-400), Some(NaiveDate::from_ymd_opt(-400, 2, 29).unwrap()));
    assert_eq!(d.with_year(-100), None);
    assert_eq!(d.with_year(1600), Some(NaiveDate::from_ymd_opt(1600, 2, 29).unwrap()));
    assert_eq!(d.with_year(1900), None);
    assert_eq!(d.with_year(2000), Some(NaiveDate::from_ymd_opt(2000, 2, 29).unwrap()));
    assert_eq!(d.with_year(2001), None);
    assert_eq!(d.with_year(2004), Some(NaiveDate::from_ymd_opt(2004, 2, 29).unwrap()));
    assert_eq!(d.with_year(i32::MAX), None);

    let d = NaiveDate::from_ymd_opt(2000, 4, 30).unwrap();
    assert_eq!(d.with_month(0), None);
    assert_eq!(d.with_month(1), Some(NaiveDate::from_ymd_opt(2000, 1, 30).unwrap()));
    assert_eq!(d.with_month(2), None);
    assert_eq!(d.with_month(3), Some(NaiveDate::from_ymd_opt(2000, 3, 30).unwrap()));
    assert_eq!(d.with_month(4), Some(NaiveDate::from_ymd_opt(2000, 4, 30).unwrap()));
    assert_eq!(d.with_month(12), Some(NaiveDate::from_ymd_opt(2000, 12, 30).unwrap()));
    assert_eq!(d.with_month(13), None);
    assert_eq!(d.with_month(u32::MAX), None);

    let d = NaiveDate::from_ymd_opt(2000, 2, 8).unwrap();
    assert_eq!(d.with_day(0), None);
    assert_eq!(d.with_day(1), Some(NaiveDate::from_ymd_opt(2000, 2, 1).unwrap()));
    assert_eq!(d.with_day(29), Some(NaiveDate::from_ymd_opt(2000, 2, 29).unwrap()));
    assert_eq!(d.with_day(30), None);
    assert_eq!(d.with_day(u32::MAX), None);
}

#[test]
fn test_date_with_ordinal() {
    let d = NaiveDate::from_ymd_opt(2000, 5, 5).unwrap();
    assert_eq!(d.with_ordinal(0), None);
    assert_eq!(d.with_ordinal(1), Some(NaiveDate::from_ymd_opt(2000, 1, 1).unwrap()));
    assert_eq!(d.with_ordinal(60), Some(NaiveDate::from_ymd_opt(2000, 2, 29).unwrap()));
    assert_eq!(d.with_ordinal(61), Some(NaiveDate::from_ymd_opt(2000, 3, 1).unwrap()));
    assert_eq!(d.with_ordinal(366), Some(NaiveDate::from_ymd_opt(2000, 12, 31).unwrap()));
    assert_eq!(d.with_ordinal(367), None);
    assert_eq!(d.with_ordinal((1 << 28) | 60), None);
    let d = NaiveDate::from_ymd_opt(1999, 5, 5).unwrap();
    assert_eq!(d.with_ordinal(366), None);
    assert_eq!(d.with_ordinal(u32::MAX), None);
}

#[test]
fn test_date_num_days_from_ce() {
    assert_eq!(NaiveDate::from_ymd_opt(1, 1, 1).unwrap().num_days_from_ce(), 1);

    for year in -9999..10001 {
        assert_eq!(
            NaiveDate::from_ymd_opt(year, 1, 1).unwrap().num_days_from_ce(),
            NaiveDate::from_ymd_opt(year - 1, 12, 31).unwrap().num_days_from_ce() + 1
        );
    }
}

#[test]
fn test_date_to_epoch_days() {
    assert_eq!(NaiveDate::from_ymd_opt(1970, 1, 1).unwrap().to_epoch_days(), 0);

    for year in -9999..10001 {
        assert_eq!(
            NaiveDate::from_ymd_opt(year, 1, 1).unwrap().to_epoch_days(),
            NaiveDate::from_ymd_opt(year - 1, 12, 31).unwrap().to_epoch_days() + 1
        );
    }
}

#[test]
fn test_date_succ() {
    let ymd = |y, m, d| NaiveDate::from_ymd_opt(y, m, d).unwrap();
    assert_eq!(ymd(2014, 5, 6).succ_opt(), Some(ymd(2014, 5, 7)));
    assert_eq!(ymd(2014, 5, 31).succ_opt(), Some(ymd(2014, 6, 1)));
    assert_eq!(ymd(2014, 12, 31).succ_opt(), Some(ymd(2015, 1, 1)));
    assert_eq!(ymd(2016, 2, 28).succ_opt(), Some(ymd(2016, 2, 29)));
    assert_eq!(ymd(NaiveDate::MAX.year(), 12, 31).succ_opt(), None);
}

#[test]
fn test_date_pred() {
    let ymd = |y, m, d| NaiveDate::from_ymd_opt(y, m, d).unwrap();
    assert_eq!(ymd(2016, 3, 1).pred_opt(), Some(ymd(2016, 2, 29)));
    assert_eq!(ymd(2015, 1, 1).pred_opt(), Some(ymd(2014, 12, 31)));
    assert_eq!(ymd(2014, 6, 1).pred_opt(), Some(ymd(2014, 5, 31)));
    assert_eq!(ymd(2014, 5, 7).pred_opt(), Some(ymd(2014, 5, 6)));
    assert_eq!(ymd(NaiveDate::MIN.year(), 1, 1).pred_opt(), None);
}

#[test]
fn test_date_checked_add_signed() {
    fn check(lhs: Option<NaiveDate>, delta: TimeDelta, rhs: Option<NaiveDate>) {
        assert_eq!(lhs.unwrap().checked_add_signed(delta), rhs);
        assert_eq!(lhs.unwrap().checked_sub_signed(-delta), rhs);
    }
    let ymd = NaiveDate::from_ymd_opt;

    check(ymd(2014, 1, 1), TimeDelta::zero(), ymd(2014, 1, 1));
    check(ymd(2014, 1, 1), TimeDelta::try_seconds(86399).unwrap(), ymd(2014, 1, 1));
    // always round towards zero
    check(ymd(2014, 1, 1), TimeDelta::try_seconds(-86399).unwrap(), ymd(2014, 1, 1));
    check(ymd(2014, 1, 1), TimeDelta::try_days(1).unwrap(), ymd(2014, 1, 2));
    check(ymd(2014, 1, 1), TimeDelta::try_days(-1).unwrap(), ymd(2013, 12, 31));
    check(ymd(2014, 1, 1), TimeDelta::try_days(364).unwrap(), ymd(2014, 12, 31));
    check(ymd(2014, 1, 1), TimeDelta::try_days(365 * 4 + 1).unwrap(), ymd(2018, 1, 1));
    check(ymd(2014, 1, 1), TimeDelta::try_days(365 * 400 + 97).unwrap(), ymd(2414, 1, 1));

    check(ymd(-7, 1, 1), TimeDelta::try_days(365 * 12 + 3).unwrap(), ymd(5, 1, 1));

    // overflow check
    check(
        ymd(0, 1, 1),
        TimeDelta::try_days(MAX_DAYS_FROM_YEAR_0 as i64).unwrap(),
        ymd(MAX_YEAR, 12, 31),
    );
    check(ymd(0, 1, 1), TimeDelta::try_days(MAX_DAYS_FROM_YEAR_0 as i64 + 1).unwrap(), None);
    check(ymd(0, 1, 1), TimeDelta::MAX, None);
    check(
        ymd(0, 1, 1),
        TimeDelta::try_days(MIN_DAYS_FROM_YEAR_0 as i64).unwrap(),
        ymd(MIN_YEAR, 1, 1),
    );
    check(ymd(0, 1, 1), TimeDelta::try_days(MIN_DAYS_FROM_YEAR_0 as i64 - 1).unwrap(), None);
    check(ymd(0, 1, 1), TimeDelta::MIN, None);
}

#[test]
fn test_date_signed_duration_since() {
    fn check(lhs: Option<NaiveDate>, rhs: Option<NaiveDate>, delta: TimeDelta) {
        assert_eq!(lhs.unwrap().signed_duration_since(rhs.unwrap()), delta);
        assert_eq!(rhs.unwrap().signed_duration_since(lhs.unwrap()), -delta);
    }
    let ymd = NaiveDate::from_ymd_opt;

    check(ymd(2014, 1, 1), ymd(2014, 1, 1), TimeDelta::zero());
    check(ymd(2014, 1, 2), ymd(2014, 1, 1), TimeDelta::try_days(1).unwrap());
    check(ymd(2014, 12, 31), ymd(2014, 1, 1), TimeDelta::try_days(364).unwrap());
    check(ymd(2015, 1, 3), ymd(2014, 1, 1), TimeDelta::try_days(365 + 2).unwrap());
    check(ymd(2018, 1, 1), ymd(2014, 1, 1), TimeDelta::try_days(365 * 4 + 1).unwrap());
    check(ymd(2414, 1, 1), ymd(2014, 1, 1), TimeDelta::try_days(365 * 400 + 97).unwrap());

    check(
        ymd(MAX_YEAR, 12, 31),
        ymd(0, 1, 1),
        TimeDelta::try_days(MAX_DAYS_FROM_YEAR_0 as i64).unwrap(),
    );
    check(
        ymd(MIN_YEAR, 1, 1),
        ymd(0, 1, 1),
        TimeDelta::try_days(MIN_DAYS_FROM_YEAR_0 as i64).unwrap(),
    );
}

#[test]
fn test_date_add_days() {
    fn check(lhs: Option<NaiveDate>, days: Days, rhs: Option<NaiveDate>) {
        assert_eq!(lhs.unwrap().checked_add_days(days), rhs);
    }
    let ymd = NaiveDate::from_ymd_opt;

    check(ymd(2014, 1, 1), Days::new(0), ymd(2014, 1, 1));
    // always round towards zero
    check(ymd(2014, 1, 1), Days::new(1), ymd(2014, 1, 2));
    check(ymd(2014, 1, 1), Days::new(364), ymd(2014, 12, 31));
    check(ymd(2014, 1, 1), Days::new(365 * 4 + 1), ymd(2018, 1, 1));
    check(ymd(2014, 1, 1), Days::new(365 * 400 + 97), ymd(2414, 1, 1));

    check(ymd(-7, 1, 1), Days::new(365 * 12 + 3), ymd(5, 1, 1));

    // overflow check
    check(ymd(0, 1, 1), Days::new(MAX_DAYS_FROM_YEAR_0.try_into().unwrap()), ymd(MAX_YEAR, 12, 31));
    check(ymd(0, 1, 1), Days::new(u64::try_from(MAX_DAYS_FROM_YEAR_0).unwrap() + 1), None);
}

#[test]
fn test_date_sub_days() {
    fn check(lhs: Option<NaiveDate>, days: Days, rhs: Option<NaiveDate>) {
        assert_eq!(lhs.unwrap().checked_sub_days(days), rhs);
    }
    let ymd = NaiveDate::from_ymd_opt;

    check(ymd(2014, 1, 1), Days::new(0), ymd(2014, 1, 1));
    check(ymd(2014, 1, 2), Days::new(1), ymd(2014, 1, 1));
    check(ymd(2014, 12, 31), Days::new(364), ymd(2014, 1, 1));
    check(ymd(2015, 1, 3), Days::new(365 + 2), ymd(2014, 1, 1));
    check(ymd(2018, 1, 1), Days::new(365 * 4 + 1), ymd(2014, 1, 1));
    check(ymd(2414, 1, 1), Days::new(365 * 400 + 97), ymd(2014, 1, 1));

    check(ymd(MAX_YEAR, 12, 31), Days::new(MAX_DAYS_FROM_YEAR_0.try_into().unwrap()), ymd(0, 1, 1));
    check(
        ymd(0, 1, 1),
        Days::new((-MIN_DAYS_FROM_YEAR_0).try_into().unwrap()),
        ymd(MIN_YEAR, 1, 1),
    );
}

#[test]
fn test_date_addassignment() {
    let ymd = |y, m, d| NaiveDate::from_ymd_opt(y, m, d).unwrap();
    let mut date = ymd(2016, 10, 1);
    date += TimeDelta::try_days(10).unwrap();
    assert_eq!(date, ymd(2016, 10, 11));
    date += TimeDelta::try_days(30).unwrap();
    assert_eq!(date, ymd(2016, 11, 10));
}

#[test]
fn test_date_subassignment() {
    let ymd = |y, m, d| NaiveDate::from_ymd_opt(y, m, d).unwrap();
    let mut date = ymd(2016, 10, 11);
    date -= TimeDelta::try_days(10).unwrap();
    assert_eq!(date, ymd(2016, 10, 1));
    date -= TimeDelta::try_days(2).unwrap();
    assert_eq!(date, ymd(2016, 9, 29));
}

#[test]
fn test_date_fmt() {
    assert_eq!(format!("{:?}", NaiveDate::from_ymd_opt(2012, 3, 4).unwrap()), "2012-03-04");
    assert_eq!(format!("{:?}", NaiveDate::from_ymd_opt(0, 3, 4).unwrap()), "0000-03-04");
    assert_eq!(format!("{:?}", NaiveDate::from_ymd_opt(-307, 3, 4).unwrap()), "-0307-03-04");
    assert_eq!(format!("{:?}", NaiveDate::from_ymd_opt(12345, 3, 4).unwrap()), "+12345-03-04");

    assert_eq!(NaiveDate::from_ymd_opt(2012, 3, 4).unwrap().to_string(), "2012-03-04");
    assert_eq!(NaiveDate::from_ymd_opt(0, 3, 4).unwrap().to_string(), "0000-03-04");
    assert_eq!(NaiveDate::from_ymd_opt(-307, 3, 4).unwrap().to_string(), "-0307-03-04");
    assert_eq!(NaiveDate::from_ymd_opt(12345, 3, 4).unwrap().to_string(), "+12345-03-04");

    // the format specifier should have no effect on `NaiveTime`
    assert_eq!(format!("{:+30?}", NaiveDate::from_ymd_opt(1234, 5, 6).unwrap()), "1234-05-06");
    assert_eq!(format!("{:30?}", NaiveDate::from_ymd_opt(12345, 6, 7).unwrap()), "+12345-06-07");
}

#[test]
fn test_date_from_str() {
    // valid cases
    let valid = [
        "-0000000123456-1-2",
        "    -123456 - 1 - 2    ",
        "-12345-1-2",
        "-1234-12-31",
        "-7-6-5",
        "350-2-28",
        "360-02-29",
        "0360-02-29",
        "2015-2 -18",
        "2015-02-18",
        "+70-2-18",
        "+70000-2-18",
        "+00007-2-18",
    ];
    for &s in &valid {
        eprintln!("test_date_from_str valid {s:?}");
        let d = match s.parse::<NaiveDate>() {
            Ok(d) => d,
            Err(e) => panic!("parsing `{s}` has failed: {e}"),
        };
        eprintln!("d {d:?} (NaiveDate)");
        let s_ = format!("{d:?}");
        eprintln!("s_ {s_:?}");
        // `s` and `s_` may differ, but `s.parse()` and `s_.parse()` must be same
        let d_ = match s_.parse::<NaiveDate>() {
            Ok(d) => d,
            Err(e) => {
                panic!("`{s}` is parsed into `{d:?}`, but reparsing that has failed: {e}")
            }
        };
        eprintln!("d_ {d_:?} (NaiveDate)");
        assert!(
            d == d_,
            "`{s}` is parsed into `{d:?}`, but reparsed result \
                            `{d_:?}` does not match"
        );
    }

    // some invalid cases
    // since `ParseErrorKind` is private, all we can do is to check if there was an error
    let invalid = [
        "",                     // empty
        "x",                    // invalid
        "Fri, 09 Aug 2013 GMT", // valid date, wrong format
        "Sat Jun 30 2012",      // valid date, wrong format
        "1441497364.649",       // valid datetime, wrong format
        "+1441497364.649",      // valid datetime, wrong format
        "+1441497364",          // valid datetime, wrong format
        "2014/02/03",           // valid date, wrong format
        "2014",                 // datetime missing data
        "2014-01",              // datetime missing data
        "2014-01-00",           // invalid day
        "2014-11-32",           // invalid day
        "2014-13-01",           // invalid month
        "2014-13-57",           // invalid month, day
        "9999999-9-9",          // invalid year (out of bounds)
    ];
    for &s in &invalid {
        eprintln!("test_date_from_str invalid {s:?}");
        assert!(s.parse::<NaiveDate>().is_err());
    }
}

#[test]
fn test_date_parse_from_str() {
    let ymd = |y, m, d| NaiveDate::from_ymd_opt(y, m, d).unwrap();
    assert_eq!(
        NaiveDate::parse_from_str("2014-5-7T12:34:56+09:30", "%Y-%m-%dT%H:%M:%S%z"),
        Ok(ymd(2014, 5, 7))
    ); // ignore time and offset
    assert_eq!(
        NaiveDate::parse_from_str("2015-W06-1=2015-033 Q1", "%G-W%V-%u = %Y-%j Q%q"),
        Ok(ymd(2015, 2, 2))
    );
    assert_eq!(NaiveDate::parse_from_str("Fri, 09 Aug 13", "%a, %d %b %y"), Ok(ymd(2013, 8, 9)));
    assert!(NaiveDate::parse_from_str("Sat, 09 Aug 2013", "%a, %d %b %Y").is_err());
    assert!(NaiveDate::parse_from_str("2014-57", "%Y-%m-%d").is_err());
    assert!(NaiveDate::parse_from_str("2014", "%Y").is_err()); // insufficient

    assert!(NaiveDate::parse_from_str("2014-5-7 Q3", "%Y-%m-%d Q%q").is_err()); // mismatched quarter

    assert_eq!(
        NaiveDate::parse_from_str("2020-01-0", "%Y-%W-%w").ok(),
        NaiveDate::from_ymd_opt(2020, 1, 12),
    );

    assert_eq!(
        NaiveDate::parse_from_str("2019-01-0", "%Y-%W-%w").ok(),
        NaiveDate::from_ymd_opt(2019, 1, 13),
    );
}

#[test]
fn test_day_iterator_limit() {
    assert_eq!(NaiveDate::from_ymd_opt(MAX_YEAR, 12, 29).unwrap().iter_days().take(4).count(), 2);
    assert_eq!(
        NaiveDate::from_ymd_opt(MIN_YEAR, 1, 3).unwrap().iter_days().rev().take(4).count(),
        2
    );
}

#[test]
fn test_week_iterator_limit() {
    assert_eq!(NaiveDate::from_ymd_opt(MAX_YEAR, 12, 12).unwrap().iter_weeks().take(4).count(), 2);
    assert_eq!(
        NaiveDate::from_ymd_opt(MIN_YEAR, 1, 15).unwrap().iter_weeks().rev().take(4).count(),
        2
    );
}

#[test]
fn test_weeks_from() {
    // tests per: https://github.com/chronotope/chrono/issues/961
    // these internally use `weeks_from` via the parsing infrastructure
    assert_eq!(
        NaiveDate::parse_from_str("2020-01-0", "%Y-%W-%w").ok(),
        NaiveDate::from_ymd_opt(2020, 1, 12),
    );
    assert_eq!(
        NaiveDate::parse_from_str("2019-01-0", "%Y-%W-%w").ok(),
        NaiveDate::from_ymd_opt(2019, 1, 13),
    );

    // direct tests
    for (y, starts_on) in &[
        (2019, Weekday::Tue),
        (2020, Weekday::Wed),
        (2021, Weekday::Fri),
        (2022, Weekday::Sat),
        (2023, Weekday::Sun),
        (2024, Weekday::Mon),
        (2025, Weekday::Wed),
        (2026, Weekday::Thu),
    ] {
        for day in &[
            Weekday::Mon,
            Weekday::Tue,
            Weekday::Wed,
            Weekday::Thu,
            Weekday::Fri,
            Weekday::Sat,
            Weekday::Sun,
        ] {
            assert_eq!(
                NaiveDate::from_ymd_opt(*y, 1, 1).map(|d| d.weeks_from(*day)),
                Some(if day == starts_on { 1 } else { 0 })
            );

            // last day must always be in week 52 or 53
            assert!(
                [52, 53].contains(&NaiveDate::from_ymd_opt(*y, 12, 31).unwrap().weeks_from(*day)),
            );
        }
    }

    let base = NaiveDate::from_ymd_opt(2019, 1, 1).unwrap();

    // 400 years covers all year types
    for day in &[
        Weekday::Mon,
        Weekday::Tue,
        Weekday::Wed,
        Weekday::Thu,
        Weekday::Fri,
        Weekday::Sat,
        Weekday::Sun,
    ] {
        // must always be below 54
        for dplus in 1..(400 * 366) {
            assert!((base + Days::new(dplus)).weeks_from(*day) < 54)
        }
    }
}

#[test]
fn test_with_0_overflow() {
    let dt = NaiveDate::from_ymd_opt(2023, 4, 18).unwrap();
    assert!(dt.with_month0(4294967295).is_none());
    assert!(dt.with_day0(4294967295).is_none());
    assert!(dt.with_ordinal0(4294967295).is_none());
}

#[test]
fn test_leap_year() {
    for year in 0..=MAX_YEAR {
        let date = NaiveDate::from_ymd_opt(year, 1, 1).unwrap();
        let is_leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
        assert_eq!(date.leap_year(), is_leap);
        assert_eq!(date.leap_year(), date.with_ordinal(366).is_some());
    }
}

#[test]
fn test_date_yearflags() {
    for (year, year_flags, _) in YEAR_FLAGS {
        assert_eq!(NaiveDate::from_yo_opt(year, 1).unwrap().year_flags(), year_flags);
    }
}

#[test]
fn test_weekday_with_yearflags() {
    for (year, year_flags, first_weekday) in YEAR_FLAGS {
        let first_day_of_year = NaiveDate::from_yo_opt(year, 1).unwrap();
        dbg!(year);
        assert_eq!(first_day_of_year.year_flags(), year_flags);
        assert_eq!(first_day_of_year.weekday(), first_weekday);

        let mut prev = first_day_of_year.weekday();
        for ordinal in 2u32..=year_flags.ndays() {
            let date = NaiveDate::from_yo_opt(year, ordinal).unwrap();
            let expected = prev.succ();
            assert_eq!(date.weekday(), expected);
            prev = expected;
        }
    }
}

#[test]
fn test_isoweekdate_with_yearflags() {
    for (year, year_flags, _) in YEAR_FLAGS {
        // January 4 should be in the first week
        let jan4 = NaiveDate::from_ymd_opt(year, 1, 4).unwrap();
        let iso_week = jan4.iso_week();
        assert_eq!(jan4.year_flags(), year_flags);
        assert_eq!(iso_week.week(), 1);
    }
}

#[test]
fn test_date_to_mdf_to_date() {
    for (year, year_flags, _) in YEAR_FLAGS {
        for ordinal in 1..=year_flags.ndays() {
            let date = NaiveDate::from_yo_opt(year, ordinal).unwrap();
            assert_eq!(date, NaiveDate::from_mdf(date.year(), date.mdf()).unwrap());
        }
    }
}

// Used for testing some methods with all combinations of `YearFlags`.
// (year, flags, first weekday of year)
const YEAR_FLAGS: [(i32, YearFlags, Weekday); 14] = [
    (2006, A, Weekday::Sun),
    (2005, B, Weekday::Sat),
    (2010, C, Weekday::Fri),
    (2009, D, Weekday::Thu),
    (2003, E, Weekday::Wed),
    (2002, F, Weekday::Tue),
    (2001, G, Weekday::Mon),
    (2012, AG, Weekday::Sun),
    (2000, BA, Weekday::Sat),
    (2016, CB, Weekday::Fri),
    (2004, DC, Weekday::Thu),
    (2020, ED, Weekday::Wed),
    (2008, FE, Weekday::Tue),
    (2024, GF, Weekday::Mon),
];

#[test]
#[cfg(feature = "rkyv-validation")]
fn test_rkyv_validation() {
    let date_min = NaiveDate::MIN;
    let bytes = rkyv::to_bytes::<_, 4>(&date_min).unwrap();
    assert_eq!(rkyv::from_bytes::<NaiveDate>(&bytes).unwrap(), date_min);

    let date_max = NaiveDate::MAX;
    let bytes = rkyv::to_bytes::<_, 4>(&date_max).unwrap();
    assert_eq!(rkyv::from_bytes::<NaiveDate>(&bytes).unwrap(), date_max);
}

//   MAX_YEAR-12-31 minus 0000-01-01
// = (MAX_YEAR-12-31 minus 0000-12-31) + (0000-12-31 - 0000-01-01)
// = MAX_YEAR * 365 + (# of leap years from 0001 to MAX_YEAR) + 365
// = (MAX_YEAR + 1) * 365 + (# of leap years from 0001 to MAX_YEAR)
const MAX_DAYS_FROM_YEAR_0: i32 =
    (MAX_YEAR + 1) * 365 + MAX_YEAR / 4 - MAX_YEAR / 100 + MAX_YEAR / 400;

//   MIN_YEAR-01-01 minus 0000-01-01
// = MIN_YEAR * 365 + (# of leap years from MIN_YEAR to 0000)
const MIN_DAYS_FROM_YEAR_0: i32 = MIN_YEAR * 365 + MIN_YEAR / 4 - MIN_YEAR / 100 + MIN_YEAR / 400;

// only used for testing, but duplicated in naive::datetime
const MAX_BITS: usize = 44;
