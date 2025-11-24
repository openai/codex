// This is a part of Chrono.
// See README.md and LICENSE.txt for details.

//! ISO 8601 calendar date without timezone.
//!
//! The implementation is optimized for determining year, month, day and day of week.
//!
//! Format of `NaiveDate`:
//! `YYYY_YYYY_YYYY_YYYY_YYYO_OOOO_OOOO_LWWW`
//! `Y`: Year
//! `O`: Ordinal
//! `L`: leap year flag (1 = common year, 0 is leap year)
//! `W`: weekday before the first day of the year
//! `LWWW`: will also be referred to as the year flags (`F`)

#[cfg(feature = "alloc")]
use core::borrow::Borrow;
use core::iter::FusedIterator;
use core::num::NonZeroI32;
use core::ops::{Add, AddAssign, Sub, SubAssign};
use core::{fmt, str};

#[cfg(any(feature = "rkyv", feature = "rkyv-16", feature = "rkyv-32", feature = "rkyv-64"))]
use rkyv::{Archive, Deserialize, Serialize};

/// L10n locales.
#[cfg(all(feature = "unstable-locales", feature = "alloc"))]
use pure_rust_locales::Locale;

use super::internals::{Mdf, YearFlags};
use crate::datetime::UNIX_EPOCH_DAY;
#[cfg(feature = "alloc")]
use crate::format::DelayedFormat;
use crate::format::{
    Item, Numeric, Pad, ParseError, ParseResult, Parsed, StrftimeItems, parse, parse_and_remainder,
    write_hundreds,
};
use crate::month::Months;
use crate::naive::{Days, IsoWeek, NaiveDateTime, NaiveTime, NaiveWeek};
use crate::{Datelike, TimeDelta, Weekday};
use crate::{expect, try_opt};

#[cfg(test)]
mod tests;

/// ISO 8601 calendar date without timezone.
/// Allows for every [proleptic Gregorian date] from Jan 1, 262145 BCE to Dec 31, 262143 CE.
/// Also supports the conversion from ISO 8601 ordinal and week date.
///
/// # Calendar Date
///
/// The ISO 8601 **calendar date** follows the proleptic Gregorian calendar.
/// It is like a normal civil calendar but note some slight differences:
///
/// * Dates before the Gregorian calendar's inception in 1582 are defined via the extrapolation.
///   Be careful, as historical dates are often noted in the Julian calendar and others
///   and the transition to Gregorian may differ across countries (as late as early 20C).
///
///   (Some example: Both Shakespeare from Britain and Cervantes from Spain seemingly died
///   on the same calendar date---April 23, 1616---but in the different calendar.
///   Britain used the Julian calendar at that time, so Shakespeare's death is later.)
///
/// * ISO 8601 calendars have the year 0, which is 1 BCE (a year before 1 CE).
///   If you need a typical BCE/BC and CE/AD notation for year numbers,
///   use the [`Datelike::year_ce`] method.
///
/// # Week Date
///
/// The ISO 8601 **week date** is a triple of year number, week number
/// and [day of the week](Weekday) with the following rules:
///
/// * A week consists of Monday through Sunday, and is always numbered within some year.
///   The week number ranges from 1 to 52 or 53 depending on the year.
///
/// * The week 1 of given year is defined as the first week containing January 4 of that year,
///   or equivalently, the first week containing four or more days in that year.
///
/// * The year number in the week date may *not* correspond to the actual Gregorian year.
///   For example, January 3, 2016 (Sunday) was on the last (53rd) week of 2015.
///
/// Chrono's date types default to the ISO 8601 [calendar date](#calendar-date), but
/// [`Datelike::iso_week`] and [`Datelike::weekday`] methods can be used to get the corresponding
/// week date.
///
/// # Ordinal Date
///
/// The ISO 8601 **ordinal date** is a pair of year number and day of the year ("ordinal").
/// The ordinal number ranges from 1 to 365 or 366 depending on the year.
/// The year number is the same as that of the [calendar date](#calendar-date).
///
/// This is currently the internal format of Chrono's date types.
///
/// [proleptic Gregorian date]: crate::NaiveDate#calendar-date
#[derive(PartialEq, Eq, Hash, PartialOrd, Ord, Copy, Clone)]
#[cfg_attr(
    any(feature = "rkyv", feature = "rkyv-16", feature = "rkyv-32", feature = "rkyv-64"),
    derive(Archive, Deserialize, Serialize),
    archive(compare(PartialEq, PartialOrd)),
    archive_attr(derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash))
)]
#[cfg_attr(feature = "rkyv-validation", archive(check_bytes))]
pub struct NaiveDate {
    yof: NonZeroI32, // (year << 13) | of
}

/// The minimum possible `NaiveDate` (January 1, 262145 BCE).
#[deprecated(since = "0.4.20", note = "Use NaiveDate::MIN instead")]
pub const MIN_DATE: NaiveDate = NaiveDate::MIN;
/// The maximum possible `NaiveDate` (December 31, 262143 CE).
#[deprecated(since = "0.4.20", note = "Use NaiveDate::MAX instead")]
pub const MAX_DATE: NaiveDate = NaiveDate::MAX;

#[cfg(all(feature = "arbitrary", feature = "std"))]
impl arbitrary::Arbitrary<'_> for NaiveDate {
    fn arbitrary(u: &mut arbitrary::Unstructured) -> arbitrary::Result<NaiveDate> {
        let year = u.int_in_range(MIN_YEAR..=MAX_YEAR)?;
        let max_days = YearFlags::from_year(year).ndays();
        let ord = u.int_in_range(1..=max_days)?;
        NaiveDate::from_yo_opt(year, ord).ok_or(arbitrary::Error::IncorrectFormat)
    }
}

impl NaiveDate {
    pub(crate) fn weeks_from(&self, day: Weekday) -> i32 {
        (self.ordinal() as i32 - self.weekday().days_since(day) as i32 + 6) / 7
    }

    /// Makes a new `NaiveDate` from year, ordinal and flags.
    /// Does not check whether the flags are correct for the provided year.
    const fn from_ordinal_and_flags(
        year: i32,
        ordinal: u32,
        flags: YearFlags,
    ) -> Option<NaiveDate> {
        if year < MIN_YEAR || year > MAX_YEAR {
            return None; // Out-of-range
        }
        if ordinal == 0 || ordinal > 366 {
            return None; // Invalid
        }
        debug_assert!(YearFlags::from_year(year).0 == flags.0);
        let yof = (year << 13) | (ordinal << 4) as i32 | flags.0 as i32;
        match yof & OL_MASK <= MAX_OL {
            true => Some(NaiveDate::from_yof(yof)),
            false => None, // Does not exist: Ordinal 366 in a common year.
        }
    }

    /// Makes a new `NaiveDate` from year and packed month-day-flags.
    /// Does not check whether the flags are correct for the provided year.
    const fn from_mdf(year: i32, mdf: Mdf) -> Option<NaiveDate> {
        if year < MIN_YEAR || year > MAX_YEAR {
            return None; // Out-of-range
        }
        Some(NaiveDate::from_yof((year << 13) | try_opt!(mdf.ordinal_and_flags())))
    }

    /// Makes a new `NaiveDate` from the [calendar date](#calendar-date)
    /// (year, month and day).
    ///
    /// # Panics
    ///
    /// Panics if the specified calendar day does not exist, on invalid values for `month` or `day`,
    /// or if `year` is out of range for `NaiveDate`.
    #[deprecated(since = "0.4.23", note = "use `from_ymd_opt()` instead")]
    #[must_use]
    pub const fn from_ymd(year: i32, month: u32, day: u32) -> NaiveDate {
        expect(NaiveDate::from_ymd_opt(year, month, day), "invalid or out-of-range date")
    }

    /// Makes a new `NaiveDate` from the [calendar date](#calendar-date)
    /// (year, month and day).
    ///
    /// # Errors
    ///
    /// Returns `None` if:
    /// - The specified calendar day does not exist (for example 2023-04-31).
    /// - The value for `month` or `day` is invalid.
    /// - `year` is out of range for `NaiveDate`.
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::NaiveDate;
    ///
    /// let from_ymd_opt = NaiveDate::from_ymd_opt;
    ///
    /// assert!(from_ymd_opt(2015, 3, 14).is_some());
    /// assert!(from_ymd_opt(2015, 0, 14).is_none());
    /// assert!(from_ymd_opt(2015, 2, 29).is_none());
    /// assert!(from_ymd_opt(-4, 2, 29).is_some()); // 5 BCE is a leap year
    /// assert!(from_ymd_opt(400000, 1, 1).is_none());
    /// assert!(from_ymd_opt(-400000, 1, 1).is_none());
    /// ```
    #[must_use]
    pub const fn from_ymd_opt(year: i32, month: u32, day: u32) -> Option<NaiveDate> {
        let flags = YearFlags::from_year(year);

        if let Some(mdf) = Mdf::new(month, day, flags) {
            NaiveDate::from_mdf(year, mdf)
        } else {
            None
        }
    }

    /// Makes a new `NaiveDate` from the [ordinal date](#ordinal-date)
    /// (year and day of the year).
    ///
    /// # Panics
    ///
    /// Panics if the specified ordinal day does not exist, on invalid values for `ordinal`, or if
    /// `year` is out of range for `NaiveDate`.
    #[deprecated(since = "0.4.23", note = "use `from_yo_opt()` instead")]
    #[must_use]
    pub const fn from_yo(year: i32, ordinal: u32) -> NaiveDate {
        expect(NaiveDate::from_yo_opt(year, ordinal), "invalid or out-of-range date")
    }

    /// Makes a new `NaiveDate` from the [ordinal date](#ordinal-date)
    /// (year and day of the year).
    ///
    /// # Errors
    ///
    /// Returns `None` if:
    /// - The specified ordinal day does not exist (for example 2023-366).
    /// - The value for `ordinal` is invalid (for example: `0`, `400`).
    /// - `year` is out of range for `NaiveDate`.
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::NaiveDate;
    ///
    /// let from_yo_opt = NaiveDate::from_yo_opt;
    ///
    /// assert!(from_yo_opt(2015, 100).is_some());
    /// assert!(from_yo_opt(2015, 0).is_none());
    /// assert!(from_yo_opt(2015, 365).is_some());
    /// assert!(from_yo_opt(2015, 366).is_none());
    /// assert!(from_yo_opt(-4, 366).is_some()); // 5 BCE is a leap year
    /// assert!(from_yo_opt(400000, 1).is_none());
    /// assert!(from_yo_opt(-400000, 1).is_none());
    /// ```
    #[must_use]
    pub const fn from_yo_opt(year: i32, ordinal: u32) -> Option<NaiveDate> {
        let flags = YearFlags::from_year(year);
        NaiveDate::from_ordinal_and_flags(year, ordinal, flags)
    }

    /// Makes a new `NaiveDate` from the [ISO week date](#week-date)
    /// (year, week number and day of the week).
    /// The resulting `NaiveDate` may have a different year from the input year.
    ///
    /// # Panics
    ///
    /// Panics if the specified week does not exist in that year, on invalid values for `week`, or
    /// if the resulting date is out of range for `NaiveDate`.
    #[deprecated(since = "0.4.23", note = "use `from_isoywd_opt()` instead")]
    #[must_use]
    pub const fn from_isoywd(year: i32, week: u32, weekday: Weekday) -> NaiveDate {
        expect(NaiveDate::from_isoywd_opt(year, week, weekday), "invalid or out-of-range date")
    }

    /// Makes a new `NaiveDate` from the [ISO week date](#week-date)
    /// (year, week number and day of the week).
    /// The resulting `NaiveDate` may have a different year from the input year.
    ///
    /// # Errors
    ///
    /// Returns `None` if:
    /// - The specified week does not exist in that year (for example 2023 week 53).
    /// - The value for `week` is invalid (for example: `0`, `60`).
    /// - If the resulting date is out of range for `NaiveDate`.
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::{NaiveDate, Weekday};
    ///
    /// let from_ymd = |y, m, d| NaiveDate::from_ymd_opt(y, m, d).unwrap();
    /// let from_isoywd_opt = NaiveDate::from_isoywd_opt;
    ///
    /// assert_eq!(from_isoywd_opt(2015, 0, Weekday::Sun), None);
    /// assert_eq!(from_isoywd_opt(2015, 10, Weekday::Sun), Some(from_ymd(2015, 3, 8)));
    /// assert_eq!(from_isoywd_opt(2015, 30, Weekday::Mon), Some(from_ymd(2015, 7, 20)));
    /// assert_eq!(from_isoywd_opt(2015, 60, Weekday::Mon), None);
    ///
    /// assert_eq!(from_isoywd_opt(400000, 10, Weekday::Fri), None);
    /// assert_eq!(from_isoywd_opt(-400000, 10, Weekday::Sat), None);
    /// ```
    ///
    /// The year number of ISO week date may differ from that of the calendar date.
    ///
    /// ```
    /// # use chrono::{NaiveDate, Weekday};
    /// # let from_ymd = |y, m, d| NaiveDate::from_ymd_opt(y, m, d).unwrap();
    /// # let from_isoywd_opt = NaiveDate::from_isoywd_opt;
    /// //           Mo Tu We Th Fr Sa Su
    /// // 2014-W52  22 23 24 25 26 27 28    has 4+ days of new year,
    /// // 2015-W01  29 30 31  1  2  3  4 <- so this is the first week
    /// assert_eq!(from_isoywd_opt(2014, 52, Weekday::Sun), Some(from_ymd(2014, 12, 28)));
    /// assert_eq!(from_isoywd_opt(2014, 53, Weekday::Mon), None);
    /// assert_eq!(from_isoywd_opt(2015, 1, Weekday::Mon), Some(from_ymd(2014, 12, 29)));
    ///
    /// // 2015-W52  21 22 23 24 25 26 27    has 4+ days of old year,
    /// // 2015-W53  28 29 30 31  1  2  3 <- so this is the last week
    /// // 2016-W01   4  5  6  7  8  9 10
    /// assert_eq!(from_isoywd_opt(2015, 52, Weekday::Sun), Some(from_ymd(2015, 12, 27)));
    /// assert_eq!(from_isoywd_opt(2015, 53, Weekday::Sun), Some(from_ymd(2016, 1, 3)));
    /// assert_eq!(from_isoywd_opt(2015, 54, Weekday::Mon), None);
    /// assert_eq!(from_isoywd_opt(2016, 1, Weekday::Mon), Some(from_ymd(2016, 1, 4)));
    /// ```
    #[must_use]
    pub const fn from_isoywd_opt(year: i32, week: u32, weekday: Weekday) -> Option<NaiveDate> {
        let flags = YearFlags::from_year(year);
        let nweeks = flags.nisoweeks();
        if week == 0 || week > nweeks {
            return None;
        }
        // ordinal = week ordinal - delta
        let weekord = week * 7 + weekday as u32;
        let delta = flags.isoweek_delta();
        let (year, ordinal, flags) = if weekord <= delta {
            // ordinal < 1, previous year
            let prevflags = YearFlags::from_year(year - 1);
            (year - 1, weekord + prevflags.ndays() - delta, prevflags)
        } else {
            let ordinal = weekord - delta;
            let ndays = flags.ndays();
            if ordinal <= ndays {
                // this year
                (year, ordinal, flags)
            } else {
                // ordinal > ndays, next year
                let nextflags = YearFlags::from_year(year + 1);
                (year + 1, ordinal - ndays, nextflags)
            }
        };
        NaiveDate::from_ordinal_and_flags(year, ordinal, flags)
    }

    /// Makes a new `NaiveDate` from a day's number in the proleptic Gregorian calendar, with
    /// January 1, 1 being day 1.
    ///
    /// # Panics
    ///
    /// Panics if the date is out of range.
    #[deprecated(since = "0.4.23", note = "use `from_num_days_from_ce_opt()` instead")]
    #[inline]
    #[must_use]
    pub const fn from_num_days_from_ce(days: i32) -> NaiveDate {
        expect(NaiveDate::from_num_days_from_ce_opt(days), "out-of-range date")
    }

    /// Makes a new `NaiveDate` from a day's number in the proleptic Gregorian calendar, with
    /// January 1, 1 being day 1.
    ///
    /// # Errors
    ///
    /// Returns `None` if the date is out of range.
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::NaiveDate;
    ///
    /// let from_ndays_opt = NaiveDate::from_num_days_from_ce_opt;
    /// let from_ymd = |y, m, d| NaiveDate::from_ymd_opt(y, m, d).unwrap();
    ///
    /// assert_eq!(from_ndays_opt(730_000), Some(from_ymd(1999, 9, 3)));
    /// assert_eq!(from_ndays_opt(1), Some(from_ymd(1, 1, 1)));
    /// assert_eq!(from_ndays_opt(0), Some(from_ymd(0, 12, 31)));
    /// assert_eq!(from_ndays_opt(-1), Some(from_ymd(0, 12, 30)));
    /// assert_eq!(from_ndays_opt(100_000_000), None);
    /// assert_eq!(from_ndays_opt(-100_000_000), None);
    /// ```
    #[must_use]
    pub const fn from_num_days_from_ce_opt(days: i32) -> Option<NaiveDate> {
        let days = try_opt!(days.checked_add(365)); // make December 31, 1 BCE equal to day 0
        let year_div_400 = days.div_euclid(146_097);
        let cycle = days.rem_euclid(146_097);
        let (year_mod_400, ordinal) = cycle_to_yo(cycle as u32);
        let flags = YearFlags::from_year_mod_400(year_mod_400 as i32);
        NaiveDate::from_ordinal_and_flags(year_div_400 * 400 + year_mod_400 as i32, ordinal, flags)
    }

    /// Makes a new `NaiveDate` from a day's number in the proleptic Gregorian calendar, with
    /// January 1, 1970 being day 0.
    ///
    /// # Errors
    ///
    /// Returns `None` if the date is out of range.
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::NaiveDate;
    ///
    /// let from_ndays_opt = NaiveDate::from_epoch_days;
    /// let from_ymd = |y, m, d| NaiveDate::from_ymd_opt(y, m, d).unwrap();
    ///
    /// assert_eq!(from_ndays_opt(-719_162), Some(from_ymd(1, 1, 1)));
    /// assert_eq!(from_ndays_opt(1), Some(from_ymd(1970, 1, 2)));
    /// assert_eq!(from_ndays_opt(0), Some(from_ymd(1970, 1, 1)));
    /// assert_eq!(from_ndays_opt(-1), Some(from_ymd(1969, 12, 31)));
    /// assert_eq!(from_ndays_opt(13036), Some(from_ymd(2005, 9, 10)));
    /// assert_eq!(from_ndays_opt(100_000_000), None);
    /// assert_eq!(from_ndays_opt(-100_000_000), None);
    /// ```
    #[must_use]
    pub const fn from_epoch_days(days: i32) -> Option<NaiveDate> {
        let ce_days = try_opt!(days.checked_add(UNIX_EPOCH_DAY as i32));
        NaiveDate::from_num_days_from_ce_opt(ce_days)
    }

    /// Makes a new `NaiveDate` by counting the number of occurrences of a particular day-of-week
    /// since the beginning of the given month. For instance, if you want the 2nd Friday of March
    /// 2017, you would use `NaiveDate::from_weekday_of_month(2017, 3, Weekday::Fri, 2)`.
    ///
    /// `n` is 1-indexed.
    ///
    /// # Panics
    ///
    /// Panics if the specified day does not exist in that month, on invalid values for `month` or
    /// `n`, or if `year` is out of range for `NaiveDate`.
    #[deprecated(since = "0.4.23", note = "use `from_weekday_of_month_opt()` instead")]
    #[must_use]
    pub const fn from_weekday_of_month(
        year: i32,
        month: u32,
        weekday: Weekday,
        n: u8,
    ) -> NaiveDate {
        expect(NaiveDate::from_weekday_of_month_opt(year, month, weekday, n), "out-of-range date")
    }

    /// Makes a new `NaiveDate` by counting the number of occurrences of a particular day-of-week
    /// since the beginning of the given month. For instance, if you want the 2nd Friday of March
    /// 2017, you would use `NaiveDate::from_weekday_of_month(2017, 3, Weekday::Fri, 2)`.
    ///
    /// `n` is 1-indexed.
    ///
    /// # Errors
    ///
    /// Returns `None` if:
    /// - The specified day does not exist in that month (for example the 5th Monday of Apr. 2023).
    /// - The value for `month` or `n` is invalid.
    /// - `year` is out of range for `NaiveDate`.
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::{NaiveDate, Weekday};
    /// assert_eq!(
    ///     NaiveDate::from_weekday_of_month_opt(2017, 3, Weekday::Fri, 2),
    ///     NaiveDate::from_ymd_opt(2017, 3, 10)
    /// )
    /// ```
    #[must_use]
    pub const fn from_weekday_of_month_opt(
        year: i32,
        month: u32,
        weekday: Weekday,
        n: u8,
    ) -> Option<NaiveDate> {
        if n == 0 {
            return None;
        }
        let first = try_opt!(NaiveDate::from_ymd_opt(year, month, 1)).weekday();
        let first_to_dow = (7 + weekday.number_from_monday() - first.number_from_monday()) % 7;
        let day = (n - 1) as u32 * 7 + first_to_dow + 1;
        NaiveDate::from_ymd_opt(year, month, day)
    }

    /// Parses a string with the specified format string and returns a new `NaiveDate`.
    /// See the [`format::strftime` module](crate::format::strftime)
    /// on the supported escape sequences.
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::NaiveDate;
    ///
    /// let parse_from_str = NaiveDate::parse_from_str;
    ///
    /// assert_eq!(
    ///     parse_from_str("2015-09-05", "%Y-%m-%d"),
    ///     Ok(NaiveDate::from_ymd_opt(2015, 9, 5).unwrap())
    /// );
    /// assert_eq!(
    ///     parse_from_str("5sep2015", "%d%b%Y"),
    ///     Ok(NaiveDate::from_ymd_opt(2015, 9, 5).unwrap())
    /// );
    /// ```
    ///
    /// Time and offset is ignored for the purpose of parsing.
    ///
    /// ```
    /// # use chrono::NaiveDate;
    /// # let parse_from_str = NaiveDate::parse_from_str;
    /// assert_eq!(
    ///     parse_from_str("2014-5-17T12:34:56+09:30", "%Y-%m-%dT%H:%M:%S%z"),
    ///     Ok(NaiveDate::from_ymd_opt(2014, 5, 17).unwrap())
    /// );
    /// ```
    ///
    /// Out-of-bound dates or insufficient fields are errors.
    ///
    /// ```
    /// # use chrono::NaiveDate;
    /// # let parse_from_str = NaiveDate::parse_from_str;
    /// assert!(parse_from_str("2015/9", "%Y/%m").is_err());
    /// assert!(parse_from_str("2015/9/31", "%Y/%m/%d").is_err());
    /// ```
    ///
    /// All parsed fields should be consistent to each other, otherwise it's an error.
    ///
    /// ```
    /// # use chrono::NaiveDate;
    /// # let parse_from_str = NaiveDate::parse_from_str;
    /// assert!(parse_from_str("Sat, 09 Aug 2013", "%a, %d %b %Y").is_err());
    /// ```
    pub fn parse_from_str(s: &str, fmt: &str) -> ParseResult<NaiveDate> {
        let mut parsed = Parsed::new();
        parse(&mut parsed, s, StrftimeItems::new(fmt))?;
        parsed.to_naive_date()
    }

    /// Parses a string from a user-specified format into a new `NaiveDate` value, and a slice with
    /// the remaining portion of the string.
    /// See the [`format::strftime` module](crate::format::strftime)
    /// on the supported escape sequences.
    ///
    /// Similar to [`parse_from_str`](#method.parse_from_str).
    ///
    /// # Example
    ///
    /// ```rust
    /// # use chrono::{NaiveDate};
    /// let (date, remainder) =
    ///     NaiveDate::parse_and_remainder("2015-02-18 trailing text", "%Y-%m-%d").unwrap();
    /// assert_eq!(date, NaiveDate::from_ymd_opt(2015, 2, 18).unwrap());
    /// assert_eq!(remainder, " trailing text");
    /// ```
    pub fn parse_and_remainder<'a>(s: &'a str, fmt: &str) -> ParseResult<(NaiveDate, &'a str)> {
        let mut parsed = Parsed::new();
        let remainder = parse_and_remainder(&mut parsed, s, StrftimeItems::new(fmt))?;
        parsed.to_naive_date().map(|d| (d, remainder))
    }

    /// Add a duration in [`Months`] to the date
    ///
    /// Uses the last day of the month if the day does not exist in the resulting month.
    ///
    /// # Errors
    ///
    /// Returns `None` if the resulting date would be out of range.
    ///
    /// # Example
    ///
    /// ```
    /// # use chrono::{NaiveDate, Months};
    /// assert_eq!(
    ///     NaiveDate::from_ymd_opt(2022, 2, 20).unwrap().checked_add_months(Months::new(6)),
    ///     Some(NaiveDate::from_ymd_opt(2022, 8, 20).unwrap())
    /// );
    /// assert_eq!(
    ///     NaiveDate::from_ymd_opt(2022, 7, 31).unwrap().checked_add_months(Months::new(2)),
    ///     Some(NaiveDate::from_ymd_opt(2022, 9, 30).unwrap())
    /// );
    /// ```
    #[must_use]
    pub const fn checked_add_months(self, months: Months) -> Option<Self> {
        if months.0 == 0 {
            return Some(self);
        }

        match months.0 <= i32::MAX as u32 {
            true => self.diff_months(months.0 as i32),
            false => None,
        }
    }

    /// Subtract a duration in [`Months`] from the date
    ///
    /// Uses the last day of the month if the day does not exist in the resulting month.
    ///
    /// # Errors
    ///
    /// Returns `None` if the resulting date would be out of range.
    ///
    /// # Example
    ///
    /// ```
    /// # use chrono::{NaiveDate, Months};
    /// assert_eq!(
    ///     NaiveDate::from_ymd_opt(2022, 2, 20).unwrap().checked_sub_months(Months::new(6)),
    ///     Some(NaiveDate::from_ymd_opt(2021, 8, 20).unwrap())
    /// );
    ///
    /// assert_eq!(
    ///     NaiveDate::from_ymd_opt(2014, 1, 1)
    ///         .unwrap()
    ///         .checked_sub_months(Months::new(core::i32::MAX as u32 + 1)),
    ///     None
    /// );
    /// ```
    #[must_use]
    pub const fn checked_sub_months(self, months: Months) -> Option<Self> {
        if months.0 == 0 {
            return Some(self);
        }

        match months.0 <= i32::MAX as u32 {
            true => self.diff_months(-(months.0 as i32)),
            false => None,
        }
    }

    const fn diff_months(self, months: i32) -> Option<Self> {
        let months = try_opt!((self.year() * 12 + self.month() as i32 - 1).checked_add(months));
        let year = months.div_euclid(12);
        let month = months.rem_euclid(12) as u32 + 1;

        // Clamp original day in case new month is shorter
        let flags = YearFlags::from_year(year);
        let feb_days = if flags.ndays() == 366 { 29 } else { 28 };
        let days = [31, feb_days, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
        let day_max = days[(month - 1) as usize];
        let mut day = self.day();
        if day > day_max {
            day = day_max;
        };

        NaiveDate::from_ymd_opt(year, month, day)
    }

    /// Add a duration in [`Days`] to the date
    ///
    /// # Errors
    ///
    /// Returns `None` if the resulting date would be out of range.
    ///
    /// # Example
    ///
    /// ```
    /// # use chrono::{NaiveDate, Days};
    /// assert_eq!(
    ///     NaiveDate::from_ymd_opt(2022, 2, 20).unwrap().checked_add_days(Days::new(9)),
    ///     Some(NaiveDate::from_ymd_opt(2022, 3, 1).unwrap())
    /// );
    /// assert_eq!(
    ///     NaiveDate::from_ymd_opt(2022, 7, 31).unwrap().checked_add_days(Days::new(2)),
    ///     Some(NaiveDate::from_ymd_opt(2022, 8, 2).unwrap())
    /// );
    /// assert_eq!(
    ///     NaiveDate::from_ymd_opt(2022, 7, 31).unwrap().checked_add_days(Days::new(1000000000000)),
    ///     None
    /// );
    /// ```
    #[must_use]
    pub const fn checked_add_days(self, days: Days) -> Option<Self> {
        match days.0 <= i32::MAX as u64 {
            true => self.add_days(days.0 as i32),
            false => None,
        }
    }

    /// Subtract a duration in [`Days`] from the date
    ///
    /// # Errors
    ///
    /// Returns `None` if the resulting date would be out of range.
    ///
    /// # Example
    ///
    /// ```
    /// # use chrono::{NaiveDate, Days};
    /// assert_eq!(
    ///     NaiveDate::from_ymd_opt(2022, 2, 20).unwrap().checked_sub_days(Days::new(6)),
    ///     Some(NaiveDate::from_ymd_opt(2022, 2, 14).unwrap())
    /// );
    /// assert_eq!(
    ///     NaiveDate::from_ymd_opt(2022, 2, 20).unwrap().checked_sub_days(Days::new(1000000000000)),
    ///     None
    /// );
    /// ```
    #[must_use]
    pub const fn checked_sub_days(self, days: Days) -> Option<Self> {
        match days.0 <= i32::MAX as u64 {
            true => self.add_days(-(days.0 as i32)),
            false => None,
        }
    }

    /// Add a duration of `i32` days to the date.
    pub(crate) const fn add_days(self, days: i32) -> Option<Self> {
        // Fast path if the result is within the same year.
        // Also `DateTime::checked_(add|sub)_days` relies on this path, because if the value remains
        // within the year it doesn't do a check if the year is in range.
        // This way `DateTime:checked_(add|sub)_days(Days::new(0))` can be a no-op on dates were the
        // local datetime is beyond `NaiveDate::{MIN, MAX}.
        const ORDINAL_MASK: i32 = 0b1_1111_1111_0000;
        if let Some(ordinal) = ((self.yof() & ORDINAL_MASK) >> 4).checked_add(days) {
            if ordinal > 0 && ordinal <= (365 + self.leap_year() as i32) {
                let year_and_flags = self.yof() & !ORDINAL_MASK;
                return Some(NaiveDate::from_yof(year_and_flags | (ordinal << 4)));
            }
        }
        // do the full check
        let year = self.year();
        let (mut year_div_400, year_mod_400) = div_mod_floor(year, 400);
        let cycle = yo_to_cycle(year_mod_400 as u32, self.ordinal());
        let cycle = try_opt!((cycle as i32).checked_add(days));
        let (cycle_div_400y, cycle) = div_mod_floor(cycle, 146_097);
        year_div_400 += cycle_div_400y;

        let (year_mod_400, ordinal) = cycle_to_yo(cycle as u32);
        let flags = YearFlags::from_year_mod_400(year_mod_400 as i32);
        NaiveDate::from_ordinal_and_flags(year_div_400 * 400 + year_mod_400 as i32, ordinal, flags)
    }

    /// Makes a new `NaiveDateTime` from the current date and given `NaiveTime`.
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
    ///
    /// let d = NaiveDate::from_ymd_opt(2015, 6, 3).unwrap();
    /// let t = NaiveTime::from_hms_milli_opt(12, 34, 56, 789).unwrap();
    ///
    /// let dt: NaiveDateTime = d.and_time(t);
    /// assert_eq!(dt.date(), d);
    /// assert_eq!(dt.time(), t);
    /// ```
    #[inline]
    #[must_use]
    pub const fn and_time(&self, time: NaiveTime) -> NaiveDateTime {
        NaiveDateTime::new(*self, time)
    }

    /// Makes a new `NaiveDateTime` from the current date, hour, minute and second.
    ///
    /// No [leap second](./struct.NaiveTime.html#leap-second-handling) is allowed here;
    /// use `NaiveDate::and_hms_*` methods with a subsecond parameter instead.
    ///
    /// # Panics
    ///
    /// Panics on invalid hour, minute and/or second.
    #[deprecated(since = "0.4.23", note = "use `and_hms_opt()` instead")]
    #[inline]
    #[must_use]
    pub const fn and_hms(&self, hour: u32, min: u32, sec: u32) -> NaiveDateTime {
        expect(self.and_hms_opt(hour, min, sec), "invalid time")
    }

    /// Makes a new `NaiveDateTime` from the current date, hour, minute and second.
    ///
    /// No [leap second](./struct.NaiveTime.html#leap-second-handling) is allowed here;
    /// use `NaiveDate::and_hms_*_opt` methods with a subsecond parameter instead.
    ///
    /// # Errors
    ///
    /// Returns `None` on invalid hour, minute and/or second.
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::NaiveDate;
    ///
    /// let d = NaiveDate::from_ymd_opt(2015, 6, 3).unwrap();
    /// assert!(d.and_hms_opt(12, 34, 56).is_some());
    /// assert!(d.and_hms_opt(12, 34, 60).is_none()); // use `and_hms_milli_opt` instead
    /// assert!(d.and_hms_opt(12, 60, 56).is_none());
    /// assert!(d.and_hms_opt(24, 34, 56).is_none());
    /// ```
    #[inline]
    #[must_use]
    pub const fn and_hms_opt(&self, hour: u32, min: u32, sec: u32) -> Option<NaiveDateTime> {
        let time = try_opt!(NaiveTime::from_hms_opt(hour, min, sec));
        Some(self.and_time(time))
    }

    /// Makes a new `NaiveDateTime` from the current date, hour, minute, second and millisecond.
    ///
    /// The millisecond part is allowed to exceed 1,000 in order to represent a [leap second](
    /// ./struct.NaiveTime.html#leap-second-handling), but only when `sec == 59`.
    ///
    /// # Panics
    ///
    /// Panics on invalid hour, minute, second and/or millisecond.
    #[deprecated(since = "0.4.23", note = "use `and_hms_milli_opt()` instead")]
    #[inline]
    #[must_use]
    pub const fn and_hms_milli(&self, hour: u32, min: u32, sec: u32, milli: u32) -> NaiveDateTime {
        expect(self.and_hms_milli_opt(hour, min, sec, milli), "invalid time")
    }

    /// Makes a new `NaiveDateTime` from the current date, hour, minute, second and millisecond.
    ///
    /// The millisecond part is allowed to exceed 1,000 in order to represent a [leap second](
    /// ./struct.NaiveTime.html#leap-second-handling), but only when `sec == 59`.
    ///
    /// # Errors
    ///
    /// Returns `None` on invalid hour, minute, second and/or millisecond.
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::NaiveDate;
    ///
    /// let d = NaiveDate::from_ymd_opt(2015, 6, 3).unwrap();
    /// assert!(d.and_hms_milli_opt(12, 34, 56, 789).is_some());
    /// assert!(d.and_hms_milli_opt(12, 34, 59, 1_789).is_some()); // leap second
    /// assert!(d.and_hms_milli_opt(12, 34, 59, 2_789).is_none());
    /// assert!(d.and_hms_milli_opt(12, 34, 60, 789).is_none());
    /// assert!(d.and_hms_milli_opt(12, 60, 56, 789).is_none());
    /// assert!(d.and_hms_milli_opt(24, 34, 56, 789).is_none());
    /// ```
    #[inline]
    #[must_use]
    pub const fn and_hms_milli_opt(
        &self,
        hour: u32,
        min: u32,
        sec: u32,
        milli: u32,
    ) -> Option<NaiveDateTime> {
        let time = try_opt!(NaiveTime::from_hms_milli_opt(hour, min, sec, milli));
        Some(self.and_time(time))
    }

    /// Makes a new `NaiveDateTime` from the current date, hour, minute, second and microsecond.
    ///
    /// The microsecond part is allowed to exceed 1,000,000 in order to represent a [leap second](
    /// ./struct.NaiveTime.html#leap-second-handling), but only when `sec == 59`.
    ///
    /// # Panics
    ///
    /// Panics on invalid hour, minute, second and/or microsecond.
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::{Datelike, NaiveDate, NaiveDateTime, Timelike, Weekday};
    ///
    /// let d = NaiveDate::from_ymd_opt(2015, 6, 3).unwrap();
    ///
    /// let dt: NaiveDateTime = d.and_hms_micro_opt(12, 34, 56, 789_012).unwrap();
    /// assert_eq!(dt.year(), 2015);
    /// assert_eq!(dt.weekday(), Weekday::Wed);
    /// assert_eq!(dt.second(), 56);
    /// assert_eq!(dt.nanosecond(), 789_012_000);
    /// ```
    #[deprecated(since = "0.4.23", note = "use `and_hms_micro_opt()` instead")]
    #[inline]
    #[must_use]
    pub const fn and_hms_micro(&self, hour: u32, min: u32, sec: u32, micro: u32) -> NaiveDateTime {
        expect(self.and_hms_micro_opt(hour, min, sec, micro), "invalid time")
    }

    /// Makes a new `NaiveDateTime` from the current date, hour, minute, second and microsecond.
    ///
    /// The microsecond part is allowed to exceed 1,000,000 in order to represent a [leap second](
    /// ./struct.NaiveTime.html#leap-second-handling), but only when `sec == 59`.
    ///
    /// # Errors
    ///
    /// Returns `None` on invalid hour, minute, second and/or microsecond.
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::NaiveDate;
    ///
    /// let d = NaiveDate::from_ymd_opt(2015, 6, 3).unwrap();
    /// assert!(d.and_hms_micro_opt(12, 34, 56, 789_012).is_some());
    /// assert!(d.and_hms_micro_opt(12, 34, 59, 1_789_012).is_some()); // leap second
    /// assert!(d.and_hms_micro_opt(12, 34, 59, 2_789_012).is_none());
    /// assert!(d.and_hms_micro_opt(12, 34, 60, 789_012).is_none());
    /// assert!(d.and_hms_micro_opt(12, 60, 56, 789_012).is_none());
    /// assert!(d.and_hms_micro_opt(24, 34, 56, 789_012).is_none());
    /// ```
    #[inline]
    #[must_use]
    pub const fn and_hms_micro_opt(
        &self,
        hour: u32,
        min: u32,
        sec: u32,
        micro: u32,
    ) -> Option<NaiveDateTime> {
        let time = try_opt!(NaiveTime::from_hms_micro_opt(hour, min, sec, micro));
        Some(self.and_time(time))
    }

    /// Makes a new `NaiveDateTime` from the current date, hour, minute, second and nanosecond.
    ///
    /// The nanosecond part is allowed to exceed 1,000,000,000 in order to represent a [leap second](
    /// ./struct.NaiveTime.html#leap-second-handling), but only when `sec == 59`.
    ///
    /// # Panics
    ///
    /// Panics on invalid hour, minute, second and/or nanosecond.
    #[deprecated(since = "0.4.23", note = "use `and_hms_nano_opt()` instead")]
    #[inline]
    #[must_use]
    pub const fn and_hms_nano(&self, hour: u32, min: u32, sec: u32, nano: u32) -> NaiveDateTime {
        expect(self.and_hms_nano_opt(hour, min, sec, nano), "invalid time")
    }

    /// Makes a new `NaiveDateTime` from the current date, hour, minute, second and nanosecond.
    ///
    /// The nanosecond part is allowed to exceed 1,000,000,000 in order to represent a [leap second](
    /// ./struct.NaiveTime.html#leap-second-handling), but only when `sec == 59`.
    ///
    /// # Errors
    ///
    /// Returns `None` on invalid hour, minute, second and/or nanosecond.
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::NaiveDate;
    ///
    /// let d = NaiveDate::from_ymd_opt(2015, 6, 3).unwrap();
    /// assert!(d.and_hms_nano_opt(12, 34, 56, 789_012_345).is_some());
    /// assert!(d.and_hms_nano_opt(12, 34, 59, 1_789_012_345).is_some()); // leap second
    /// assert!(d.and_hms_nano_opt(12, 34, 59, 2_789_012_345).is_none());
    /// assert!(d.and_hms_nano_opt(12, 34, 60, 789_012_345).is_none());
    /// assert!(d.and_hms_nano_opt(12, 60, 56, 789_012_345).is_none());
    /// assert!(d.and_hms_nano_opt(24, 34, 56, 789_012_345).is_none());
    /// ```
    #[inline]
    #[must_use]
    pub const fn and_hms_nano_opt(
        &self,
        hour: u32,
        min: u32,
        sec: u32,
        nano: u32,
    ) -> Option<NaiveDateTime> {
        let time = try_opt!(NaiveTime::from_hms_nano_opt(hour, min, sec, nano));
        Some(self.and_time(time))
    }

    /// Returns the packed month-day-flags.
    #[inline]
    const fn mdf(&self) -> Mdf {
        Mdf::from_ol((self.yof() & OL_MASK) >> 3, self.year_flags())
    }

    /// Makes a new `NaiveDate` with the packed month-day-flags changed.
    ///
    /// Returns `None` when the resulting `NaiveDate` would be invalid.
    #[inline]
    const fn with_mdf(&self, mdf: Mdf) -> Option<NaiveDate> {
        debug_assert!(self.year_flags().0 == mdf.year_flags().0);
        match mdf.ordinal() {
            Some(ordinal) => {
                Some(NaiveDate::from_yof((self.yof() & !ORDINAL_MASK) | (ordinal << 4) as i32))
            }
            None => None, // Non-existing date
        }
    }

    /// Makes a new `NaiveDate` for the next calendar date.
    ///
    /// # Panics
    ///
    /// Panics when `self` is the last representable date.
    #[deprecated(since = "0.4.23", note = "use `succ_opt()` instead")]
    #[inline]
    #[must_use]
    pub const fn succ(&self) -> NaiveDate {
        expect(self.succ_opt(), "out of bound")
    }

    /// Makes a new `NaiveDate` for the next calendar date.
    ///
    /// # Errors
    ///
    /// Returns `None` when `self` is the last representable date.
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::NaiveDate;
    ///
    /// assert_eq!(
    ///     NaiveDate::from_ymd_opt(2015, 6, 3).unwrap().succ_opt(),
    ///     Some(NaiveDate::from_ymd_opt(2015, 6, 4).unwrap())
    /// );
    /// assert_eq!(NaiveDate::MAX.succ_opt(), None);
    /// ```
    #[inline]
    #[must_use]
    pub const fn succ_opt(&self) -> Option<NaiveDate> {
        let new_ol = (self.yof() & OL_MASK) + (1 << 4);
        match new_ol <= MAX_OL {
            true => Some(NaiveDate::from_yof(self.yof() & !OL_MASK | new_ol)),
            false => NaiveDate::from_yo_opt(self.year() + 1, 1),
        }
    }

    /// Makes a new `NaiveDate` for the previous calendar date.
    ///
    /// # Panics
    ///
    /// Panics when `self` is the first representable date.
    #[deprecated(since = "0.4.23", note = "use `pred_opt()` instead")]
    #[inline]
    #[must_use]
    pub const fn pred(&self) -> NaiveDate {
        expect(self.pred_opt(), "out of bound")
    }

    /// Makes a new `NaiveDate` for the previous calendar date.
    ///
    /// # Errors
    ///
    /// Returns `None` when `self` is the first representable date.
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::NaiveDate;
    ///
    /// assert_eq!(
    ///     NaiveDate::from_ymd_opt(2015, 6, 3).unwrap().pred_opt(),
    ///     Some(NaiveDate::from_ymd_opt(2015, 6, 2).unwrap())
    /// );
    /// assert_eq!(NaiveDate::MIN.pred_opt(), None);
    /// ```
    #[inline]
    #[must_use]
    pub const fn pred_opt(&self) -> Option<NaiveDate> {
        let new_shifted_ordinal = (self.yof() & ORDINAL_MASK) - (1 << 4);
        match new_shifted_ordinal > 0 {
            true => Some(NaiveDate::from_yof(self.yof() & !ORDINAL_MASK | new_shifted_ordinal)),
            false => NaiveDate::from_ymd_opt(self.year() - 1, 12, 31),
        }
    }

    /// Adds the number of whole days in the given `TimeDelta` to the current date.
    ///
    /// # Errors
    ///
    /// Returns `None` if the resulting date would be out of range.
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::{NaiveDate, TimeDelta};
    ///
    /// let d = NaiveDate::from_ymd_opt(2015, 9, 5).unwrap();
    /// assert_eq!(
    ///     d.checked_add_signed(TimeDelta::try_days(40).unwrap()),
    ///     Some(NaiveDate::from_ymd_opt(2015, 10, 15).unwrap())
    /// );
    /// assert_eq!(
    ///     d.checked_add_signed(TimeDelta::try_days(-40).unwrap()),
    ///     Some(NaiveDate::from_ymd_opt(2015, 7, 27).unwrap())
    /// );
    /// assert_eq!(d.checked_add_signed(TimeDelta::try_days(1_000_000_000).unwrap()), None);
    /// assert_eq!(d.checked_add_signed(TimeDelta::try_days(-1_000_000_000).unwrap()), None);
    /// assert_eq!(NaiveDate::MAX.checked_add_signed(TimeDelta::try_days(1).unwrap()), None);
    /// ```
    #[must_use]
    pub const fn checked_add_signed(self, rhs: TimeDelta) -> Option<NaiveDate> {
        let days = rhs.num_days();
        if days < i32::MIN as i64 || days > i32::MAX as i64 {
            return None;
        }
        self.add_days(days as i32)
    }

    /// Subtracts the number of whole days in the given `TimeDelta` from the current date.
    ///
    /// # Errors
    ///
    /// Returns `None` if the resulting date would be out of range.
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::{NaiveDate, TimeDelta};
    ///
    /// let d = NaiveDate::from_ymd_opt(2015, 9, 5).unwrap();
    /// assert_eq!(
    ///     d.checked_sub_signed(TimeDelta::try_days(40).unwrap()),
    ///     Some(NaiveDate::from_ymd_opt(2015, 7, 27).unwrap())
    /// );
    /// assert_eq!(
    ///     d.checked_sub_signed(TimeDelta::try_days(-40).unwrap()),
    ///     Some(NaiveDate::from_ymd_opt(2015, 10, 15).unwrap())
    /// );
    /// assert_eq!(d.checked_sub_signed(TimeDelta::try_days(1_000_000_000).unwrap()), None);
    /// assert_eq!(d.checked_sub_signed(TimeDelta::try_days(-1_000_000_000).unwrap()), None);
    /// assert_eq!(NaiveDate::MIN.checked_sub_signed(TimeDelta::try_days(1).unwrap()), None);
    /// ```
    #[must_use]
    pub const fn checked_sub_signed(self, rhs: TimeDelta) -> Option<NaiveDate> {
        let days = -rhs.num_days();
        if days < i32::MIN as i64 || days > i32::MAX as i64 {
            return None;
        }
        self.add_days(days as i32)
    }

    /// Subtracts another `NaiveDate` from the current date.
    /// Returns a `TimeDelta` of integral numbers.
    ///
    /// This does not overflow or underflow at all,
    /// as all possible output fits in the range of `TimeDelta`.
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::{NaiveDate, TimeDelta};
    ///
    /// let from_ymd = |y, m, d| NaiveDate::from_ymd_opt(y, m, d).unwrap();
    /// let since = NaiveDate::signed_duration_since;
    ///
    /// assert_eq!(since(from_ymd(2014, 1, 1), from_ymd(2014, 1, 1)), TimeDelta::zero());
    /// assert_eq!(
    ///     since(from_ymd(2014, 1, 1), from_ymd(2013, 12, 31)),
    ///     TimeDelta::try_days(1).unwrap()
    /// );
    /// assert_eq!(since(from_ymd(2014, 1, 1), from_ymd(2014, 1, 2)), TimeDelta::try_days(-1).unwrap());
    /// assert_eq!(
    ///     since(from_ymd(2014, 1, 1), from_ymd(2013, 9, 23)),
    ///     TimeDelta::try_days(100).unwrap()
    /// );
    /// assert_eq!(
    ///     since(from_ymd(2014, 1, 1), from_ymd(2013, 1, 1)),
    ///     TimeDelta::try_days(365).unwrap()
    /// );
    /// assert_eq!(
    ///     since(from_ymd(2014, 1, 1), from_ymd(2010, 1, 1)),
    ///     TimeDelta::try_days(365 * 4 + 1).unwrap()
    /// );
    /// assert_eq!(
    ///     since(from_ymd(2014, 1, 1), from_ymd(1614, 1, 1)),
    ///     TimeDelta::try_days(365 * 400 + 97).unwrap()
    /// );
    /// ```
    #[must_use]
    pub const fn signed_duration_since(self, rhs: NaiveDate) -> TimeDelta {
        let year1 = self.year();
        let year2 = rhs.year();
        let (year1_div_400, year1_mod_400) = div_mod_floor(year1, 400);
        let (year2_div_400, year2_mod_400) = div_mod_floor(year2, 400);
        let cycle1 = yo_to_cycle(year1_mod_400 as u32, self.ordinal()) as i64;
        let cycle2 = yo_to_cycle(year2_mod_400 as u32, rhs.ordinal()) as i64;
        let days = (year1_div_400 as i64 - year2_div_400 as i64) * 146_097 + (cycle1 - cycle2);
        // The range of `TimeDelta` is ca. 585 million years, the range of `NaiveDate` ca. 525.000
        // years.
        expect(TimeDelta::try_days(days), "always in range")
    }

    /// Returns the number of whole years from the given `base` until `self`.
    ///
    /// # Errors
    ///
    /// Returns `None` if `base > self`.
    #[must_use]
    pub const fn years_since(&self, base: Self) -> Option<u32> {
        let mut years = self.year() - base.year();
        // Comparing tuples is not (yet) possible in const context. Instead we combine month and
        // day into one `u32` for easy comparison.
        if ((self.month() << 5) | self.day()) < ((base.month() << 5) | base.day()) {
            years -= 1;
        }

        match years >= 0 {
            true => Some(years as u32),
            false => None,
        }
    }

    /// Formats the date with the specified formatting items.
    /// Otherwise it is the same as the ordinary `format` method.
    ///
    /// The `Iterator` of items should be `Clone`able,
    /// since the resulting `DelayedFormat` value may be formatted multiple times.
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::format::strftime::StrftimeItems;
    /// use chrono::NaiveDate;
    ///
    /// let fmt = StrftimeItems::new("%Y-%m-%d");
    /// let d = NaiveDate::from_ymd_opt(2015, 9, 5).unwrap();
    /// assert_eq!(d.format_with_items(fmt.clone()).to_string(), "2015-09-05");
    /// assert_eq!(d.format("%Y-%m-%d").to_string(), "2015-09-05");
    /// ```
    ///
    /// The resulting `DelayedFormat` can be formatted directly via the `Display` trait.
    ///
    /// ```
    /// # use chrono::NaiveDate;
    /// # use chrono::format::strftime::StrftimeItems;
    /// # let fmt = StrftimeItems::new("%Y-%m-%d").clone();
    /// # let d = NaiveDate::from_ymd_opt(2015, 9, 5).unwrap();
    /// assert_eq!(format!("{}", d.format_with_items(fmt)), "2015-09-05");
    /// ```
    #[cfg(feature = "alloc")]
    #[inline]
    #[must_use]
    pub fn format_with_items<'a, I, B>(&self, items: I) -> DelayedFormat<I>
    where
        I: Iterator<Item = B> + Clone,
        B: Borrow<Item<'a>>,
    {
        DelayedFormat::new(Some(*self), None, items)
    }

    /// Formats the date with the specified format string.
    /// See the [`format::strftime` module](crate::format::strftime)
    /// on the supported escape sequences.
    ///
    /// This returns a `DelayedFormat`,
    /// which gets converted to a string only when actual formatting happens.
    /// You may use the `to_string` method to get a `String`,
    /// or just feed it into `print!` and other formatting macros.
    /// (In this way it avoids the redundant memory allocation.)
    ///
    /// # Panics
    ///
    /// Converting or formatting the returned `DelayedFormat` panics if the format string is wrong.
    /// Because of this delayed failure, you are recommended to immediately use the `DelayedFormat`
    /// value.
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::NaiveDate;
    ///
    /// let d = NaiveDate::from_ymd_opt(2015, 9, 5).unwrap();
    /// assert_eq!(d.format("%Y-%m-%d").to_string(), "2015-09-05");
    /// assert_eq!(d.format("%A, %-d %B, %C%y").to_string(), "Saturday, 5 September, 2015");
    /// ```
    ///
    /// The resulting `DelayedFormat` can be formatted directly via the `Display` trait.
    ///
    /// ```
    /// # use chrono::NaiveDate;
    /// # let d = NaiveDate::from_ymd_opt(2015, 9, 5).unwrap();
    /// assert_eq!(format!("{}", d.format("%Y-%m-%d")), "2015-09-05");
    /// assert_eq!(format!("{}", d.format("%A, %-d %B, %C%y")), "Saturday, 5 September, 2015");
    /// ```
    #[cfg(feature = "alloc")]
    #[inline]
    #[must_use]
    pub fn format<'a>(&self, fmt: &'a str) -> DelayedFormat<StrftimeItems<'a>> {
        self.format_with_items(StrftimeItems::new(fmt))
    }

    /// Formats the date with the specified formatting items and locale.
    #[cfg(all(feature = "unstable-locales", feature = "alloc"))]
    #[inline]
    #[must_use]
    pub fn format_localized_with_items<'a, I, B>(
        &self,
        items: I,
        locale: Locale,
    ) -> DelayedFormat<I>
    where
        I: Iterator<Item = B> + Clone,
        B: Borrow<Item<'a>>,
    {
        DelayedFormat::new_with_locale(Some(*self), None, items, locale)
    }

    /// Formats the date with the specified format string and locale.
    ///
    /// See the [`crate::format::strftime`] module on the supported escape
    /// sequences.
    #[cfg(all(feature = "unstable-locales", feature = "alloc"))]
    #[inline]
    #[must_use]
    pub fn format_localized<'a>(
        &self,
        fmt: &'a str,
        locale: Locale,
    ) -> DelayedFormat<StrftimeItems<'a>> {
        self.format_localized_with_items(StrftimeItems::new_with_locale(fmt, locale), locale)
    }

    /// Returns an iterator that steps by days across all representable dates.
    ///
    /// # Example
    ///
    /// ```
    /// # use chrono::NaiveDate;
    ///
    /// let expected = [
    ///     NaiveDate::from_ymd_opt(2016, 2, 27).unwrap(),
    ///     NaiveDate::from_ymd_opt(2016, 2, 28).unwrap(),
    ///     NaiveDate::from_ymd_opt(2016, 2, 29).unwrap(),
    ///     NaiveDate::from_ymd_opt(2016, 3, 1).unwrap(),
    /// ];
    ///
    /// let mut count = 0;
    /// for (idx, d) in NaiveDate::from_ymd_opt(2016, 2, 27).unwrap().iter_days().take(4).enumerate() {
    ///     assert_eq!(d, expected[idx]);
    ///     count += 1;
    /// }
    /// assert_eq!(count, 4);
    ///
    /// for d in NaiveDate::from_ymd_opt(2016, 3, 1).unwrap().iter_days().rev().take(4) {
    ///     count -= 1;
    ///     assert_eq!(d, expected[count]);
    /// }
    /// ```
    #[inline]
    pub const fn iter_days(&self) -> NaiveDateDaysIterator {
        NaiveDateDaysIterator { value: *self }
    }

    /// Returns an iterator that steps by weeks across all representable dates.
    ///
    /// # Example
    ///
    /// ```
    /// # use chrono::NaiveDate;
    ///
    /// let expected = [
    ///     NaiveDate::from_ymd_opt(2016, 2, 27).unwrap(),
    ///     NaiveDate::from_ymd_opt(2016, 3, 5).unwrap(),
    ///     NaiveDate::from_ymd_opt(2016, 3, 12).unwrap(),
    ///     NaiveDate::from_ymd_opt(2016, 3, 19).unwrap(),
    /// ];
    ///
    /// let mut count = 0;
    /// for (idx, d) in NaiveDate::from_ymd_opt(2016, 2, 27).unwrap().iter_weeks().take(4).enumerate() {
    ///     assert_eq!(d, expected[idx]);
    ///     count += 1;
    /// }
    /// assert_eq!(count, 4);
    ///
    /// for d in NaiveDate::from_ymd_opt(2016, 3, 19).unwrap().iter_weeks().rev().take(4) {
    ///     count -= 1;
    ///     assert_eq!(d, expected[count]);
    /// }
    /// ```
    #[inline]
    pub const fn iter_weeks(&self) -> NaiveDateWeeksIterator {
        NaiveDateWeeksIterator { value: *self }
    }

    /// Returns the [`NaiveWeek`] that the date belongs to, starting with the [`Weekday`]
    /// specified.
    #[inline]
    pub const fn week(&self, start: Weekday) -> NaiveWeek {
        NaiveWeek::new(*self, start)
    }

    /// Returns `true` if this is a leap year.
    ///
    /// ```
    /// # use chrono::NaiveDate;
    /// assert_eq!(NaiveDate::from_ymd_opt(2000, 1, 1).unwrap().leap_year(), true);
    /// assert_eq!(NaiveDate::from_ymd_opt(2001, 1, 1).unwrap().leap_year(), false);
    /// assert_eq!(NaiveDate::from_ymd_opt(2002, 1, 1).unwrap().leap_year(), false);
    /// assert_eq!(NaiveDate::from_ymd_opt(2003, 1, 1).unwrap().leap_year(), false);
    /// assert_eq!(NaiveDate::from_ymd_opt(2004, 1, 1).unwrap().leap_year(), true);
    /// assert_eq!(NaiveDate::from_ymd_opt(2100, 1, 1).unwrap().leap_year(), false);
    /// ```
    pub const fn leap_year(&self) -> bool {
        self.yof() & (0b1000) == 0
    }

    // This duplicates `Datelike::year()`, because trait methods can't be const yet.
    #[inline]
    const fn year(&self) -> i32 {
        self.yof() >> 13
    }

    /// Returns the day of year starting from 1.
    // This duplicates `Datelike::ordinal()`, because trait methods can't be const yet.
    #[inline]
    const fn ordinal(&self) -> u32 {
        ((self.yof() & ORDINAL_MASK) >> 4) as u32
    }

    // This duplicates `Datelike::month()`, because trait methods can't be const yet.
    #[inline]
    const fn month(&self) -> u32 {
        self.mdf().month()
    }

    // This duplicates `Datelike::day()`, because trait methods can't be const yet.
    #[inline]
    const fn day(&self) -> u32 {
        self.mdf().day()
    }

    /// Returns the day of week.
    // This duplicates `Datelike::weekday()`, because trait methods can't be const yet.
    #[inline]
    pub(super) const fn weekday(&self) -> Weekday {
        match (((self.yof() & ORDINAL_MASK) >> 4) + (self.yof() & WEEKDAY_FLAGS_MASK)) % 7 {
            0 => Weekday::Mon,
            1 => Weekday::Tue,
            2 => Weekday::Wed,
            3 => Weekday::Thu,
            4 => Weekday::Fri,
            5 => Weekday::Sat,
            _ => Weekday::Sun,
        }
    }

    #[inline]
    const fn year_flags(&self) -> YearFlags {
        YearFlags((self.yof() & YEAR_FLAGS_MASK) as u8)
    }

    /// Counts the days in the proleptic Gregorian calendar, with January 1, Year 1 (CE) as day 1.
    // This duplicates `Datelike::num_days_from_ce()`, because trait methods can't be const yet.
    pub(crate) const fn num_days_from_ce(&self) -> i32 {
        // we know this wouldn't overflow since year is limited to 1/2^13 of i32's full range.
        let mut year = self.year() - 1;
        let mut ndays = 0;
        if year < 0 {
            let excess = 1 + (-year) / 400;
            year += excess * 400;
            ndays -= excess * 146_097;
        }
        let div_100 = year / 100;
        ndays += ((year * 1461) >> 2) - div_100 + (div_100 >> 2);
        ndays + self.ordinal() as i32
    }

    /// Counts the days in the proleptic Gregorian calendar, with January 1, Year 1970 as day 0.
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::NaiveDate;
    ///
    /// let from_ymd = |y, m, d| NaiveDate::from_ymd_opt(y, m, d).unwrap();
    ///
    /// assert_eq!(from_ymd(1, 1, 1).to_epoch_days(), -719162);
    /// assert_eq!(from_ymd(1970, 1, 1).to_epoch_days(), 0);
    /// assert_eq!(from_ymd(2005, 9, 10).to_epoch_days(), 13036);
    /// ```
    pub const fn to_epoch_days(&self) -> i32 {
        self.num_days_from_ce() - UNIX_EPOCH_DAY as i32
    }

    /// Create a new `NaiveDate` from a raw year-ordinal-flags `i32`.
    ///
    /// In a valid value an ordinal is never `0`, and neither are the year flags. This method
    /// doesn't do any validation in release builds.
    #[inline]
    const fn from_yof(yof: i32) -> NaiveDate {
        // The following are the invariants our ordinal and flags should uphold for a valid
        // `NaiveDate`.
        debug_assert!(((yof & OL_MASK) >> 3) > 1);
        debug_assert!(((yof & OL_MASK) >> 3) <= MAX_OL);
        debug_assert!((yof & 0b111) != 000);
        NaiveDate { yof: unsafe { NonZeroI32::new_unchecked(yof) } }
    }

    /// Get the raw year-ordinal-flags `i32`.
    #[inline]
    const fn yof(&self) -> i32 {
        self.yof.get()
    }

    /// The minimum possible `NaiveDate` (January 1, 262144 BCE).
    pub const MIN: NaiveDate = NaiveDate::from_yof((MIN_YEAR << 13) | (1 << 4) | 0o12 /* D */);
    /// The maximum possible `NaiveDate` (December 31, 262142 CE).
    pub const MAX: NaiveDate =
        NaiveDate::from_yof((MAX_YEAR << 13) | (365 << 4) | 0o16 /* G */);

    /// One day before the minimum possible `NaiveDate` (December 31, 262145 BCE).
    pub(crate) const BEFORE_MIN: NaiveDate =
        NaiveDate::from_yof(((MIN_YEAR - 1) << 13) | (366 << 4) | 0o07 /* FE */);
    /// One day after the maximum possible `NaiveDate` (January 1, 262143 CE).
    pub(crate) const AFTER_MAX: NaiveDate =
        NaiveDate::from_yof(((MAX_YEAR + 1) << 13) | (1 << 4) | 0o17 /* F */);
}

impl Datelike for NaiveDate {
    /// Returns the year number in the [calendar date](#calendar-date).
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::{Datelike, NaiveDate};
    ///
    /// assert_eq!(NaiveDate::from_ymd_opt(2015, 9, 8).unwrap().year(), 2015);
    /// assert_eq!(NaiveDate::from_ymd_opt(-308, 3, 14).unwrap().year(), -308); // 309 BCE
    /// ```
    #[inline]
    fn year(&self) -> i32 {
        self.year()
    }

    /// Returns the month number starting from 1.
    ///
    /// The return value ranges from 1 to 12.
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::{Datelike, NaiveDate};
    ///
    /// assert_eq!(NaiveDate::from_ymd_opt(2015, 9, 8).unwrap().month(), 9);
    /// assert_eq!(NaiveDate::from_ymd_opt(-308, 3, 14).unwrap().month(), 3);
    /// ```
    #[inline]
    fn month(&self) -> u32 {
        self.month()
    }

    /// Returns the month number starting from 0.
    ///
    /// The return value ranges from 0 to 11.
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::{Datelike, NaiveDate};
    ///
    /// assert_eq!(NaiveDate::from_ymd_opt(2015, 9, 8).unwrap().month0(), 8);
    /// assert_eq!(NaiveDate::from_ymd_opt(-308, 3, 14).unwrap().month0(), 2);
    /// ```
    #[inline]
    fn month0(&self) -> u32 {
        self.month() - 1
    }

    /// Returns the day of month starting from 1.
    ///
    /// The return value ranges from 1 to 31. (The last day of month differs by months.)
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::{Datelike, NaiveDate};
    ///
    /// assert_eq!(NaiveDate::from_ymd_opt(2015, 9, 8).unwrap().day(), 8);
    /// assert_eq!(NaiveDate::from_ymd_opt(-308, 3, 14).unwrap().day(), 14);
    /// ```
    ///
    /// Combined with [`NaiveDate::pred_opt`](#method.pred_opt),
    /// one can determine the number of days in a particular month.
    /// (Note that this panics when `year` is out of range.)
    ///
    /// ```
    /// use chrono::{Datelike, NaiveDate};
    ///
    /// fn ndays_in_month(year: i32, month: u32) -> u32 {
    ///     // the first day of the next month...
    ///     let (y, m) = if month == 12 { (year + 1, 1) } else { (year, month + 1) };
    ///     let d = NaiveDate::from_ymd_opt(y, m, 1).unwrap();
    ///
    ///     // ...is preceded by the last day of the original month
    ///     d.pred_opt().unwrap().day()
    /// }
    ///
    /// assert_eq!(ndays_in_month(2015, 8), 31);
    /// assert_eq!(ndays_in_month(2015, 9), 30);
    /// assert_eq!(ndays_in_month(2015, 12), 31);
    /// assert_eq!(ndays_in_month(2016, 2), 29);
    /// assert_eq!(ndays_in_month(2017, 2), 28);
    /// ```
    #[inline]
    fn day(&self) -> u32 {
        self.day()
    }

    /// Returns the day of month starting from 0.
    ///
    /// The return value ranges from 0 to 30. (The last day of month differs by months.)
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::{Datelike, NaiveDate};
    ///
    /// assert_eq!(NaiveDate::from_ymd_opt(2015, 9, 8).unwrap().day0(), 7);
    /// assert_eq!(NaiveDate::from_ymd_opt(-308, 3, 14).unwrap().day0(), 13);
    /// ```
    #[inline]
    fn day0(&self) -> u32 {
        self.mdf().day() - 1
    }

    /// Returns the day of year starting from 1.
    ///
    /// The return value ranges from 1 to 366. (The last day of year differs by years.)
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::{Datelike, NaiveDate};
    ///
    /// assert_eq!(NaiveDate::from_ymd_opt(2015, 9, 8).unwrap().ordinal(), 251);
    /// assert_eq!(NaiveDate::from_ymd_opt(-308, 3, 14).unwrap().ordinal(), 74);
    /// ```
    ///
    /// Combined with [`NaiveDate::pred_opt`](#method.pred_opt),
    /// one can determine the number of days in a particular year.
    /// (Note that this panics when `year` is out of range.)
    ///
    /// ```
    /// use chrono::{Datelike, NaiveDate};
    ///
    /// fn ndays_in_year(year: i32) -> u32 {
    ///     // the first day of the next year...
    ///     let d = NaiveDate::from_ymd_opt(year + 1, 1, 1).unwrap();
    ///
    ///     // ...is preceded by the last day of the original year
    ///     d.pred_opt().unwrap().ordinal()
    /// }
    ///
    /// assert_eq!(ndays_in_year(2015), 365);
    /// assert_eq!(ndays_in_year(2016), 366);
    /// assert_eq!(ndays_in_year(2017), 365);
    /// assert_eq!(ndays_in_year(2000), 366);
    /// assert_eq!(ndays_in_year(2100), 365);
    /// ```
    #[inline]
    fn ordinal(&self) -> u32 {
        ((self.yof() & ORDINAL_MASK) >> 4) as u32
    }

    /// Returns the day of year starting from 0.
    ///
    /// The return value ranges from 0 to 365. (The last day of year differs by years.)
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::{Datelike, NaiveDate};
    ///
    /// assert_eq!(NaiveDate::from_ymd_opt(2015, 9, 8).unwrap().ordinal0(), 250);
    /// assert_eq!(NaiveDate::from_ymd_opt(-308, 3, 14).unwrap().ordinal0(), 73);
    /// ```
    #[inline]
    fn ordinal0(&self) -> u32 {
        self.ordinal() - 1
    }

    /// Returns the day of week.
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::{Datelike, NaiveDate, Weekday};
    ///
    /// assert_eq!(NaiveDate::from_ymd_opt(2015, 9, 8).unwrap().weekday(), Weekday::Tue);
    /// assert_eq!(NaiveDate::from_ymd_opt(-308, 3, 14).unwrap().weekday(), Weekday::Fri);
    /// ```
    #[inline]
    fn weekday(&self) -> Weekday {
        self.weekday()
    }

    #[inline]
    fn iso_week(&self) -> IsoWeek {
        IsoWeek::from_yof(self.year(), self.ordinal(), self.year_flags())
    }

    /// Makes a new `NaiveDate` with the year number changed, while keeping the same month and day.
    ///
    /// This method assumes you want to work on the date as a year-month-day value. Don't use it if
    /// you want the ordinal to stay the same after changing the year, of if you want the week and
    /// weekday values to stay the same.
    ///
    /// # Errors
    ///
    /// Returns `None` if:
    /// - The resulting date does not exist (February 29 in a non-leap year).
    /// - The year is out of range for a `NaiveDate`.
    ///
    /// # Examples
    ///
    /// ```
    /// use chrono::{Datelike, NaiveDate};
    ///
    /// assert_eq!(
    ///     NaiveDate::from_ymd_opt(2015, 9, 8).unwrap().with_year(2016),
    ///     Some(NaiveDate::from_ymd_opt(2016, 9, 8).unwrap())
    /// );
    /// assert_eq!(
    ///     NaiveDate::from_ymd_opt(2015, 9, 8).unwrap().with_year(-308),
    ///     Some(NaiveDate::from_ymd_opt(-308, 9, 8).unwrap())
    /// );
    /// ```
    ///
    /// A leap day (February 29) is a case where this method can return `None`.
    ///
    /// ```
    /// # use chrono::{NaiveDate, Datelike};
    /// assert!(NaiveDate::from_ymd_opt(2016, 2, 29).unwrap().with_year(2015).is_none());
    /// assert!(NaiveDate::from_ymd_opt(2016, 2, 29).unwrap().with_year(2020).is_some());
    /// ```
    ///
    /// Don't use `with_year` if you want the ordinal date to stay the same:
    ///
    /// ```
    /// # use chrono::{Datelike, NaiveDate};
    /// assert_ne!(
    ///     NaiveDate::from_yo_opt(2020, 100).unwrap().with_year(2023).unwrap(),
    ///     NaiveDate::from_yo_opt(2023, 100).unwrap() // result is 2023-101
    /// );
    /// ```
    #[inline]
    fn with_year(&self, year: i32) -> Option<NaiveDate> {
        // we need to operate with `mdf` since we should keep the month and day number as is
        let mdf = self.mdf();

        // adjust the flags as needed
        let flags = YearFlags::from_year(year);
        let mdf = mdf.with_flags(flags);

        NaiveDate::from_mdf(year, mdf)
    }

    /// Makes a new `NaiveDate` with the month number (starting from 1) changed.
    ///
    /// # Errors
    ///
    /// Returns `None` if:
    /// - The resulting date does not exist (for example `month(4)` when day of the month is 31).
    /// - The value for `month` is invalid.
    ///
    /// # Examples
    ///
    /// ```
    /// use chrono::{Datelike, NaiveDate};
    ///
    /// assert_eq!(
    ///     NaiveDate::from_ymd_opt(2015, 9, 8).unwrap().with_month(10),
    ///     Some(NaiveDate::from_ymd_opt(2015, 10, 8).unwrap())
    /// );
    /// assert_eq!(NaiveDate::from_ymd_opt(2015, 9, 8).unwrap().with_month(13), None); // No month 13
    /// assert_eq!(NaiveDate::from_ymd_opt(2015, 9, 30).unwrap().with_month(2), None); // No Feb 30
    /// ```
    ///
    /// Don't combine multiple `Datelike::with_*` methods. The intermediate value may not exist.
    ///
    /// ```
    /// use chrono::{Datelike, NaiveDate};
    ///
    /// fn with_year_month(date: NaiveDate, year: i32, month: u32) -> Option<NaiveDate> {
    ///     date.with_year(year)?.with_month(month)
    /// }
    /// let d = NaiveDate::from_ymd_opt(2020, 2, 29).unwrap();
    /// assert!(with_year_month(d, 2019, 1).is_none()); // fails because of invalid intermediate value
    ///
    /// // Correct version:
    /// fn with_year_month_fixed(date: NaiveDate, year: i32, month: u32) -> Option<NaiveDate> {
    ///     NaiveDate::from_ymd_opt(year, month, date.day())
    /// }
    /// let d = NaiveDate::from_ymd_opt(2020, 2, 29).unwrap();
    /// assert_eq!(with_year_month_fixed(d, 2019, 1), NaiveDate::from_ymd_opt(2019, 1, 29));
    /// ```
    #[inline]
    fn with_month(&self, month: u32) -> Option<NaiveDate> {
        self.with_mdf(self.mdf().with_month(month)?)
    }

    /// Makes a new `NaiveDate` with the month number (starting from 0) changed.
    ///
    /// # Errors
    ///
    /// Returns `None` if:
    /// - The resulting date does not exist (for example `month0(3)` when day of the month is 31).
    /// - The value for `month0` is invalid.
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::{Datelike, NaiveDate};
    ///
    /// assert_eq!(
    ///     NaiveDate::from_ymd_opt(2015, 9, 8).unwrap().with_month0(9),
    ///     Some(NaiveDate::from_ymd_opt(2015, 10, 8).unwrap())
    /// );
    /// assert_eq!(NaiveDate::from_ymd_opt(2015, 9, 8).unwrap().with_month0(12), None); // No month 12
    /// assert_eq!(NaiveDate::from_ymd_opt(2015, 9, 30).unwrap().with_month0(1), None); // No Feb 30
    /// ```
    #[inline]
    fn with_month0(&self, month0: u32) -> Option<NaiveDate> {
        let month = month0.checked_add(1)?;
        self.with_mdf(self.mdf().with_month(month)?)
    }

    /// Makes a new `NaiveDate` with the day of month (starting from 1) changed.
    ///
    /// # Errors
    ///
    /// Returns `None` if:
    /// - The resulting date does not exist (for example `day(31)` in April).
    /// - The value for `day` is invalid.
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::{Datelike, NaiveDate};
    ///
    /// assert_eq!(
    ///     NaiveDate::from_ymd_opt(2015, 9, 8).unwrap().with_day(30),
    ///     Some(NaiveDate::from_ymd_opt(2015, 9, 30).unwrap())
    /// );
    /// assert_eq!(NaiveDate::from_ymd_opt(2015, 9, 8).unwrap().with_day(31), None);
    /// // no September 31
    /// ```
    #[inline]
    fn with_day(&self, day: u32) -> Option<NaiveDate> {
        self.with_mdf(self.mdf().with_day(day)?)
    }

    /// Makes a new `NaiveDate` with the day of month (starting from 0) changed.
    ///
    /// # Errors
    ///
    /// Returns `None` if:
    /// - The resulting date does not exist (for example `day(30)` in April).
    /// - The value for `day0` is invalid.
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::{Datelike, NaiveDate};
    ///
    /// assert_eq!(
    ///     NaiveDate::from_ymd_opt(2015, 9, 8).unwrap().with_day0(29),
    ///     Some(NaiveDate::from_ymd_opt(2015, 9, 30).unwrap())
    /// );
    /// assert_eq!(NaiveDate::from_ymd_opt(2015, 9, 8).unwrap().with_day0(30), None);
    /// // no September 31
    /// ```
    #[inline]
    fn with_day0(&self, day0: u32) -> Option<NaiveDate> {
        let day = day0.checked_add(1)?;
        self.with_mdf(self.mdf().with_day(day)?)
    }

    /// Makes a new `NaiveDate` with the day of year (starting from 1) changed.
    ///
    /// # Errors
    ///
    /// Returns `None` if:
    /// - The resulting date does not exist (`with_ordinal(366)` in a non-leap year).
    /// - The value for `ordinal` is invalid.
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::{NaiveDate, Datelike};
    ///
    /// assert_eq!(NaiveDate::from_ymd_opt(2015, 1, 1).unwrap().with_ordinal(60),
    ///            Some(NaiveDate::from_ymd_opt(2015, 3, 1).unwrap()));
    /// assert_eq!(NaiveDate::from_ymd_opt(2015, 1, 1).unwrap().with_ordinal(366),
    ///            None); // 2015 had only 365 days
    ///
    /// assert_eq!(NaiveDate::from_ymd_opt(2016, 1, 1).unwrap().with_ordinal(60),
    ///            Some(NaiveDate::from_ymd_opt(2016, 2, 29).unwrap()));
    /// assert_eq!(NaiveDate::from_ymd_opt(2016, 1, 1).unwrap().with_ordinal(366),
    ///            Some(NaiveDate::from_ymd_opt(2016, 12, 31).unwrap()));
    /// ```
    #[inline]
    fn with_ordinal(&self, ordinal: u32) -> Option<NaiveDate> {
        if ordinal == 0 || ordinal > 366 {
            return None;
        }
        let yof = (self.yof() & !ORDINAL_MASK) | (ordinal << 4) as i32;
        match yof & OL_MASK <= MAX_OL {
            true => Some(NaiveDate::from_yof(yof)),
            false => None, // Does not exist: Ordinal 366 in a common year.
        }
    }

    /// Makes a new `NaiveDate` with the day of year (starting from 0) changed.
    ///
    /// # Errors
    ///
    /// Returns `None` if:
    /// - The resulting date does not exist (`with_ordinal0(365)` in a non-leap year).
    /// - The value for `ordinal0` is invalid.
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::{NaiveDate, Datelike};
    ///
    /// assert_eq!(NaiveDate::from_ymd_opt(2015, 1, 1).unwrap().with_ordinal0(59),
    ///            Some(NaiveDate::from_ymd_opt(2015, 3, 1).unwrap()));
    /// assert_eq!(NaiveDate::from_ymd_opt(2015, 1, 1).unwrap().with_ordinal0(365),
    ///            None); // 2015 had only 365 days
    ///
    /// assert_eq!(NaiveDate::from_ymd_opt(2016, 1, 1).unwrap().with_ordinal0(59),
    ///            Some(NaiveDate::from_ymd_opt(2016, 2, 29).unwrap()));
    /// assert_eq!(NaiveDate::from_ymd_opt(2016, 1, 1).unwrap().with_ordinal0(365),
    ///            Some(NaiveDate::from_ymd_opt(2016, 12, 31).unwrap()));
    /// ```
    #[inline]
    fn with_ordinal0(&self, ordinal0: u32) -> Option<NaiveDate> {
        let ordinal = ordinal0.checked_add(1)?;
        self.with_ordinal(ordinal)
    }
}

/// Add `TimeDelta` to `NaiveDate`.
///
/// This discards the fractional days in `TimeDelta`, rounding to the closest integral number of
/// days towards `TimeDelta::zero()`.
///
/// # Panics
///
/// Panics if the resulting date would be out of range.
/// Consider using [`NaiveDate::checked_add_signed`] to get an `Option` instead.
///
/// # Example
///
/// ```
/// use chrono::{NaiveDate, TimeDelta};
///
/// let from_ymd = |y, m, d| NaiveDate::from_ymd_opt(y, m, d).unwrap();
///
/// assert_eq!(from_ymd(2014, 1, 1) + TimeDelta::zero(), from_ymd(2014, 1, 1));
/// assert_eq!(from_ymd(2014, 1, 1) + TimeDelta::try_seconds(86399).unwrap(), from_ymd(2014, 1, 1));
/// assert_eq!(
///     from_ymd(2014, 1, 1) + TimeDelta::try_seconds(-86399).unwrap(),
///     from_ymd(2014, 1, 1)
/// );
/// assert_eq!(from_ymd(2014, 1, 1) + TimeDelta::try_days(1).unwrap(), from_ymd(2014, 1, 2));
/// assert_eq!(from_ymd(2014, 1, 1) + TimeDelta::try_days(-1).unwrap(), from_ymd(2013, 12, 31));
/// assert_eq!(from_ymd(2014, 1, 1) + TimeDelta::try_days(364).unwrap(), from_ymd(2014, 12, 31));
/// assert_eq!(
///     from_ymd(2014, 1, 1) + TimeDelta::try_days(365 * 4 + 1).unwrap(),
///     from_ymd(2018, 1, 1)
/// );
/// assert_eq!(
///     from_ymd(2014, 1, 1) + TimeDelta::try_days(365 * 400 + 97).unwrap(),
///     from_ymd(2414, 1, 1)
/// );
/// ```
///
/// [`NaiveDate::checked_add_signed`]: crate::NaiveDate::checked_add_signed
impl Add<TimeDelta> for NaiveDate {
    type Output = NaiveDate;

    #[inline]
    fn add(self, rhs: TimeDelta) -> NaiveDate {
        self.checked_add_signed(rhs).expect("`NaiveDate + TimeDelta` overflowed")
    }
}

/// Add-assign of `TimeDelta` to `NaiveDate`.
///
/// This discards the fractional days in `TimeDelta`, rounding to the closest integral number of days
/// towards `TimeDelta::zero()`.
///
/// # Panics
///
/// Panics if the resulting date would be out of range.
/// Consider using [`NaiveDate::checked_add_signed`] to get an `Option` instead.
impl AddAssign<TimeDelta> for NaiveDate {
    #[inline]
    fn add_assign(&mut self, rhs: TimeDelta) {
        *self = self.add(rhs);
    }
}

/// Add `Months` to `NaiveDate`.
///
/// The result will be clamped to valid days in the resulting month, see `checked_add_months` for
/// details.
///
/// # Panics
///
/// Panics if the resulting date would be out of range.
/// Consider using `NaiveDate::checked_add_months` to get an `Option` instead.
///
/// # Example
///
/// ```
/// use chrono::{Months, NaiveDate};
///
/// let from_ymd = |y, m, d| NaiveDate::from_ymd_opt(y, m, d).unwrap();
///
/// assert_eq!(from_ymd(2014, 1, 1) + Months::new(1), from_ymd(2014, 2, 1));
/// assert_eq!(from_ymd(2014, 1, 1) + Months::new(11), from_ymd(2014, 12, 1));
/// assert_eq!(from_ymd(2014, 1, 1) + Months::new(12), from_ymd(2015, 1, 1));
/// assert_eq!(from_ymd(2014, 1, 1) + Months::new(13), from_ymd(2015, 2, 1));
/// assert_eq!(from_ymd(2014, 1, 31) + Months::new(1), from_ymd(2014, 2, 28));
/// assert_eq!(from_ymd(2020, 1, 31) + Months::new(1), from_ymd(2020, 2, 29));
/// ```
impl Add<Months> for NaiveDate {
    type Output = NaiveDate;

    fn add(self, months: Months) -> Self::Output {
        self.checked_add_months(months).expect("`NaiveDate + Months` out of range")
    }
}

/// Subtract `Months` from `NaiveDate`.
///
/// The result will be clamped to valid days in the resulting month, see `checked_sub_months` for
/// details.
///
/// # Panics
///
/// Panics if the resulting date would be out of range.
/// Consider using `NaiveDate::checked_sub_months` to get an `Option` instead.
///
/// # Example
///
/// ```
/// use chrono::{Months, NaiveDate};
///
/// let from_ymd = |y, m, d| NaiveDate::from_ymd_opt(y, m, d).unwrap();
///
/// assert_eq!(from_ymd(2014, 1, 1) - Months::new(11), from_ymd(2013, 2, 1));
/// assert_eq!(from_ymd(2014, 1, 1) - Months::new(12), from_ymd(2013, 1, 1));
/// assert_eq!(from_ymd(2014, 1, 1) - Months::new(13), from_ymd(2012, 12, 1));
/// ```
impl Sub<Months> for NaiveDate {
    type Output = NaiveDate;

    fn sub(self, months: Months) -> Self::Output {
        self.checked_sub_months(months).expect("`NaiveDate - Months` out of range")
    }
}

/// Add `Days` to `NaiveDate`.
///
/// # Panics
///
/// Panics if the resulting date would be out of range.
/// Consider using `NaiveDate::checked_add_days` to get an `Option` instead.
impl Add<Days> for NaiveDate {
    type Output = NaiveDate;

    fn add(self, days: Days) -> Self::Output {
        self.checked_add_days(days).expect("`NaiveDate + Days` out of range")
    }
}

/// Subtract `Days` from `NaiveDate`.
///
/// # Panics
///
/// Panics if the resulting date would be out of range.
/// Consider using `NaiveDate::checked_sub_days` to get an `Option` instead.
impl Sub<Days> for NaiveDate {
    type Output = NaiveDate;

    fn sub(self, days: Days) -> Self::Output {
        self.checked_sub_days(days).expect("`NaiveDate - Days` out of range")
    }
}

/// Subtract `TimeDelta` from `NaiveDate`.
///
/// This discards the fractional days in `TimeDelta`, rounding to the closest integral number of
/// days towards `TimeDelta::zero()`.
/// It is the same as the addition with a negated `TimeDelta`.
///
/// # Panics
///
/// Panics if the resulting date would be out of range.
/// Consider using [`NaiveDate::checked_sub_signed`] to get an `Option` instead.
///
/// # Example
///
/// ```
/// use chrono::{NaiveDate, TimeDelta};
///
/// let from_ymd = |y, m, d| NaiveDate::from_ymd_opt(y, m, d).unwrap();
///
/// assert_eq!(from_ymd(2014, 1, 1) - TimeDelta::zero(), from_ymd(2014, 1, 1));
/// assert_eq!(from_ymd(2014, 1, 1) - TimeDelta::try_seconds(86399).unwrap(), from_ymd(2014, 1, 1));
/// assert_eq!(
///     from_ymd(2014, 1, 1) - TimeDelta::try_seconds(-86399).unwrap(),
///     from_ymd(2014, 1, 1)
/// );
/// assert_eq!(from_ymd(2014, 1, 1) - TimeDelta::try_days(1).unwrap(), from_ymd(2013, 12, 31));
/// assert_eq!(from_ymd(2014, 1, 1) - TimeDelta::try_days(-1).unwrap(), from_ymd(2014, 1, 2));
/// assert_eq!(from_ymd(2014, 1, 1) - TimeDelta::try_days(364).unwrap(), from_ymd(2013, 1, 2));
/// assert_eq!(
///     from_ymd(2014, 1, 1) - TimeDelta::try_days(365 * 4 + 1).unwrap(),
///     from_ymd(2010, 1, 1)
/// );
/// assert_eq!(
///     from_ymd(2014, 1, 1) - TimeDelta::try_days(365 * 400 + 97).unwrap(),
///     from_ymd(1614, 1, 1)
/// );
/// ```
///
/// [`NaiveDate::checked_sub_signed`]: crate::NaiveDate::checked_sub_signed
impl Sub<TimeDelta> for NaiveDate {
    type Output = NaiveDate;

    #[inline]
    fn sub(self, rhs: TimeDelta) -> NaiveDate {
        self.checked_sub_signed(rhs).expect("`NaiveDate - TimeDelta` overflowed")
    }
}

/// Subtract-assign `TimeDelta` from `NaiveDate`.
///
/// This discards the fractional days in `TimeDelta`, rounding to the closest integral number of
/// days towards `TimeDelta::zero()`.
/// It is the same as the addition with a negated `TimeDelta`.
///
/// # Panics
///
/// Panics if the resulting date would be out of range.
/// Consider using [`NaiveDate::checked_sub_signed`] to get an `Option` instead.
impl SubAssign<TimeDelta> for NaiveDate {
    #[inline]
    fn sub_assign(&mut self, rhs: TimeDelta) {
        *self = self.sub(rhs);
    }
}

/// Subtracts another `NaiveDate` from the current date.
/// Returns a `TimeDelta` of integral numbers.
///
/// This does not overflow or underflow at all,
/// as all possible output fits in the range of `TimeDelta`.
///
/// The implementation is a wrapper around
/// [`NaiveDate::signed_duration_since`](#method.signed_duration_since).
///
/// # Example
///
/// ```
/// use chrono::{NaiveDate, TimeDelta};
///
/// let from_ymd = |y, m, d| NaiveDate::from_ymd_opt(y, m, d).unwrap();
///
/// assert_eq!(from_ymd(2014, 1, 1) - from_ymd(2014, 1, 1), TimeDelta::zero());
/// assert_eq!(from_ymd(2014, 1, 1) - from_ymd(2013, 12, 31), TimeDelta::try_days(1).unwrap());
/// assert_eq!(from_ymd(2014, 1, 1) - from_ymd(2014, 1, 2), TimeDelta::try_days(-1).unwrap());
/// assert_eq!(from_ymd(2014, 1, 1) - from_ymd(2013, 9, 23), TimeDelta::try_days(100).unwrap());
/// assert_eq!(from_ymd(2014, 1, 1) - from_ymd(2013, 1, 1), TimeDelta::try_days(365).unwrap());
/// assert_eq!(
///     from_ymd(2014, 1, 1) - from_ymd(2010, 1, 1),
///     TimeDelta::try_days(365 * 4 + 1).unwrap()
/// );
/// assert_eq!(
///     from_ymd(2014, 1, 1) - from_ymd(1614, 1, 1),
///     TimeDelta::try_days(365 * 400 + 97).unwrap()
/// );
/// ```
impl Sub<NaiveDate> for NaiveDate {
    type Output = TimeDelta;

    #[inline]
    fn sub(self, rhs: NaiveDate) -> TimeDelta {
        self.signed_duration_since(rhs)
    }
}

impl From<NaiveDateTime> for NaiveDate {
    fn from(naive_datetime: NaiveDateTime) -> Self {
        naive_datetime.date()
    }
}

/// Iterator over `NaiveDate` with a step size of one day.
#[derive(Debug, Copy, Clone, Hash, PartialEq, PartialOrd, Eq, Ord)]
pub struct NaiveDateDaysIterator {
    value: NaiveDate,
}

impl Iterator for NaiveDateDaysIterator {
    type Item = NaiveDate;

    fn next(&mut self) -> Option<Self::Item> {
        // We return the current value, and have no way to return `NaiveDate::MAX`.
        let current = self.value;
        // This can't panic because current is < NaiveDate::MAX:
        self.value = current.succ_opt()?;
        Some(current)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let exact_size = NaiveDate::MAX.signed_duration_since(self.value).num_days();
        (exact_size as usize, Some(exact_size as usize))
    }
}

impl ExactSizeIterator for NaiveDateDaysIterator {}

impl DoubleEndedIterator for NaiveDateDaysIterator {
    fn next_back(&mut self) -> Option<Self::Item> {
        // We return the current value, and have no way to return `NaiveDate::MIN`.
        let current = self.value;
        self.value = current.pred_opt()?;
        Some(current)
    }
}

impl FusedIterator for NaiveDateDaysIterator {}

/// Iterator over `NaiveDate` with a step size of one week.
#[derive(Debug, Copy, Clone, Hash, PartialEq, PartialOrd, Eq, Ord)]
pub struct NaiveDateWeeksIterator {
    value: NaiveDate,
}

impl Iterator for NaiveDateWeeksIterator {
    type Item = NaiveDate;

    fn next(&mut self) -> Option<Self::Item> {
        let current = self.value;
        self.value = current.checked_add_days(Days::new(7))?;
        Some(current)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let exact_size = NaiveDate::MAX.signed_duration_since(self.value).num_weeks();
        (exact_size as usize, Some(exact_size as usize))
    }
}

impl ExactSizeIterator for NaiveDateWeeksIterator {}

impl DoubleEndedIterator for NaiveDateWeeksIterator {
    fn next_back(&mut self) -> Option<Self::Item> {
        let current = self.value;
        self.value = current.checked_sub_days(Days::new(7))?;
        Some(current)
    }
}

impl FusedIterator for NaiveDateWeeksIterator {}

/// The `Debug` output of the naive date `d` is the same as
/// [`d.format("%Y-%m-%d")`](crate::format::strftime).
///
/// The string printed can be readily parsed via the `parse` method on `str`.
///
/// # Example
///
/// ```
/// use chrono::NaiveDate;
///
/// assert_eq!(format!("{:?}", NaiveDate::from_ymd_opt(2015, 9, 5).unwrap()), "2015-09-05");
/// assert_eq!(format!("{:?}", NaiveDate::from_ymd_opt(0, 1, 1).unwrap()), "0000-01-01");
/// assert_eq!(format!("{:?}", NaiveDate::from_ymd_opt(9999, 12, 31).unwrap()), "9999-12-31");
/// ```
///
/// ISO 8601 requires an explicit sign for years before 1 BCE or after 9999 CE.
///
/// ```
/// # use chrono::NaiveDate;
/// assert_eq!(format!("{:?}", NaiveDate::from_ymd_opt(-1, 1, 1).unwrap()), "-0001-01-01");
/// assert_eq!(format!("{:?}", NaiveDate::from_ymd_opt(10000, 12, 31).unwrap()), "+10000-12-31");
/// ```
impl fmt::Debug for NaiveDate {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use core::fmt::Write;

        let year = self.year();
        let mdf = self.mdf();
        if (0..=9999).contains(&year) {
            write_hundreds(f, (year / 100) as u8)?;
            write_hundreds(f, (year % 100) as u8)?;
        } else {
            // ISO 8601 requires the explicit sign for out-of-range years
            write!(f, "{year:+05}")?;
        }

        f.write_char('-')?;
        write_hundreds(f, mdf.month() as u8)?;
        f.write_char('-')?;
        write_hundreds(f, mdf.day() as u8)
    }
}

/// The `Display` output of the naive date `d` is the same as
/// [`d.format("%Y-%m-%d")`](crate::format::strftime).
///
/// The string printed can be readily parsed via the `parse` method on `str`.
///
/// # Example
///
/// ```
/// use chrono::NaiveDate;
///
/// assert_eq!(format!("{}", NaiveDate::from_ymd_opt(2015, 9, 5).unwrap()), "2015-09-05");
/// assert_eq!(format!("{}", NaiveDate::from_ymd_opt(0, 1, 1).unwrap()), "0000-01-01");
/// assert_eq!(format!("{}", NaiveDate::from_ymd_opt(9999, 12, 31).unwrap()), "9999-12-31");
/// ```
///
/// ISO 8601 requires an explicit sign for years before 1 BCE or after 9999 CE.
///
/// ```
/// # use chrono::NaiveDate;
/// assert_eq!(format!("{}", NaiveDate::from_ymd_opt(-1, 1, 1).unwrap()), "-0001-01-01");
/// assert_eq!(format!("{}", NaiveDate::from_ymd_opt(10000, 12, 31).unwrap()), "+10000-12-31");
/// ```
impl fmt::Display for NaiveDate {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

/// Parsing a `str` into a `NaiveDate` uses the same format,
/// [`%Y-%m-%d`](crate::format::strftime), as in `Debug` and `Display`.
///
/// # Example
///
/// ```
/// use chrono::NaiveDate;
///
/// let d = NaiveDate::from_ymd_opt(2015, 9, 18).unwrap();
/// assert_eq!("2015-09-18".parse::<NaiveDate>(), Ok(d));
///
/// let d = NaiveDate::from_ymd_opt(12345, 6, 7).unwrap();
/// assert_eq!("+12345-6-7".parse::<NaiveDate>(), Ok(d));
///
/// assert!("foo".parse::<NaiveDate>().is_err());
/// ```
impl str::FromStr for NaiveDate {
    type Err = ParseError;

    fn from_str(s: &str) -> ParseResult<NaiveDate> {
        const ITEMS: &[Item<'static>] = &[
            Item::Numeric(Numeric::Year, Pad::Zero),
            Item::Space(""),
            Item::Literal("-"),
            Item::Numeric(Numeric::Month, Pad::Zero),
            Item::Space(""),
            Item::Literal("-"),
            Item::Numeric(Numeric::Day, Pad::Zero),
            Item::Space(""),
        ];

        let mut parsed = Parsed::new();
        parse(&mut parsed, s, ITEMS.iter())?;
        parsed.to_naive_date()
    }
}

/// The default value for a NaiveDate is 1st of January 1970.
///
/// # Example
///
/// ```rust
/// use chrono::NaiveDate;
///
/// let default_date = NaiveDate::default();
/// assert_eq!(default_date, NaiveDate::from_ymd_opt(1970, 1, 1).unwrap());
/// ```
impl Default for NaiveDate {
    fn default() -> Self {
        NaiveDate::from_ymd_opt(1970, 1, 1).unwrap()
    }
}

const fn cycle_to_yo(cycle: u32) -> (u32, u32) {
    let mut year_mod_400 = cycle / 365;
    let mut ordinal0 = cycle % 365;
    let delta = YEAR_DELTAS[year_mod_400 as usize] as u32;
    if ordinal0 < delta {
        year_mod_400 -= 1;
        ordinal0 += 365 - YEAR_DELTAS[year_mod_400 as usize] as u32;
    } else {
        ordinal0 -= delta;
    }
    (year_mod_400, ordinal0 + 1)
}

const fn yo_to_cycle(year_mod_400: u32, ordinal: u32) -> u32 {
    year_mod_400 * 365 + YEAR_DELTAS[year_mod_400 as usize] as u32 + ordinal - 1
}

const fn div_mod_floor(val: i32, div: i32) -> (i32, i32) {
    (val.div_euclid(div), val.rem_euclid(div))
}

/// MAX_YEAR is one year less than the type is capable of representing. Internally we may sometimes
/// use the headroom, notably to handle cases where the offset of a `DateTime` constructed with
/// `NaiveDate::MAX` pushes it beyond the valid, representable range.
pub(super) const MAX_YEAR: i32 = (i32::MAX >> 13) - 1;

/// MIN_YEAR is one year more than the type is capable of representing. Internally we may sometimes
/// use the headroom, notably to handle cases where the offset of a `DateTime` constructed with
/// `NaiveDate::MIN` pushes it beyond the valid, representable range.
pub(super) const MIN_YEAR: i32 = (i32::MIN >> 13) + 1;

const ORDINAL_MASK: i32 = 0b1_1111_1111_0000;

const LEAP_YEAR_MASK: i32 = 0b1000;

// OL: ordinal and leap year flag.
// With only these parts of the date an ordinal 366 in a common year would be encoded as
// `((366 << 1) | 1) << 3`, and in a leap year as `((366 << 1) | 0) << 3`, which is less.
// This allows for efficiently checking the ordinal exists depending on whether this is a leap year.
const OL_MASK: i32 = ORDINAL_MASK | LEAP_YEAR_MASK;
const MAX_OL: i32 = 366 << 4;

// Weekday of the last day in the preceding year.
// Allows for quick day of week calculation from the 1-based ordinal.
const WEEKDAY_FLAGS_MASK: i32 = 0b111;

const YEAR_FLAGS_MASK: i32 = LEAP_YEAR_MASK | WEEKDAY_FLAGS_MASK;

const YEAR_DELTAS: &[u8; 401] = &[
    0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 6, 6, 6, 6, 7, 7, 7, 7, 8, 8, 8,
    8, 9, 9, 9, 9, 10, 10, 10, 10, 11, 11, 11, 11, 12, 12, 12, 12, 13, 13, 13, 13, 14, 14, 14, 14,
    15, 15, 15, 15, 16, 16, 16, 16, 17, 17, 17, 17, 18, 18, 18, 18, 19, 19, 19, 19, 20, 20, 20, 20,
    21, 21, 21, 21, 22, 22, 22, 22, 23, 23, 23, 23, 24, 24, 24, 24, 25, 25, 25, // 100
    25, 25, 25, 25, 25, 26, 26, 26, 26, 27, 27, 27, 27, 28, 28, 28, 28, 29, 29, 29, 29, 30, 30, 30,
    30, 31, 31, 31, 31, 32, 32, 32, 32, 33, 33, 33, 33, 34, 34, 34, 34, 35, 35, 35, 35, 36, 36, 36,
    36, 37, 37, 37, 37, 38, 38, 38, 38, 39, 39, 39, 39, 40, 40, 40, 40, 41, 41, 41, 41, 42, 42, 42,
    42, 43, 43, 43, 43, 44, 44, 44, 44, 45, 45, 45, 45, 46, 46, 46, 46, 47, 47, 47, 47, 48, 48, 48,
    48, 49, 49, 49, // 200
    49, 49, 49, 49, 49, 50, 50, 50, 50, 51, 51, 51, 51, 52, 52, 52, 52, 53, 53, 53, 53, 54, 54, 54,
    54, 55, 55, 55, 55, 56, 56, 56, 56, 57, 57, 57, 57, 58, 58, 58, 58, 59, 59, 59, 59, 60, 60, 60,
    60, 61, 61, 61, 61, 62, 62, 62, 62, 63, 63, 63, 63, 64, 64, 64, 64, 65, 65, 65, 65, 66, 66, 66,
    66, 67, 67, 67, 67, 68, 68, 68, 68, 69, 69, 69, 69, 70, 70, 70, 70, 71, 71, 71, 71, 72, 72, 72,
    72, 73, 73, 73, // 300
    73, 73, 73, 73, 73, 74, 74, 74, 74, 75, 75, 75, 75, 76, 76, 76, 76, 77, 77, 77, 77, 78, 78, 78,
    78, 79, 79, 79, 79, 80, 80, 80, 80, 81, 81, 81, 81, 82, 82, 82, 82, 83, 83, 83, 83, 84, 84, 84,
    84, 85, 85, 85, 85, 86, 86, 86, 86, 87, 87, 87, 87, 88, 88, 88, 88, 89, 89, 89, 89, 90, 90, 90,
    90, 91, 91, 91, 91, 92, 92, 92, 92, 93, 93, 93, 93, 94, 94, 94, 94, 95, 95, 95, 95, 96, 96, 96,
    96, 97, 97, 97, 97, // 400+1
];

#[cfg(feature = "serde")]
mod serde {
    use super::NaiveDate;
    use core::fmt;
    use serde::{de, ser};

    // TODO not very optimized for space (binary formats would want something better)

    impl ser::Serialize for NaiveDate {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: ser::Serializer,
        {
            struct FormatWrapped<'a, D: 'a> {
                inner: &'a D,
            }

            impl<D: fmt::Debug> fmt::Display for FormatWrapped<'_, D> {
                fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                    self.inner.fmt(f)
                }
            }

            serializer.collect_str(&FormatWrapped { inner: &self })
        }
    }

    struct NaiveDateVisitor;

    impl de::Visitor<'_> for NaiveDateVisitor {
        type Value = NaiveDate;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a formatted date string")
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            value.parse().map_err(E::custom)
        }
    }

    impl<'de> de::Deserialize<'de> for NaiveDate {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: de::Deserializer<'de>,
        {
            deserializer.deserialize_str(NaiveDateVisitor)
        }
    }

    #[cfg(test)]
    mod tests {
        use crate::NaiveDate;

        #[test]
        fn test_serde_serialize() {
            assert_eq!(
                serde_json::to_string(&NaiveDate::from_ymd_opt(2014, 7, 24).unwrap()).ok(),
                Some(r#""2014-07-24""#.into())
            );
            assert_eq!(
                serde_json::to_string(&NaiveDate::from_ymd_opt(0, 1, 1).unwrap()).ok(),
                Some(r#""0000-01-01""#.into())
            );
            assert_eq!(
                serde_json::to_string(&NaiveDate::from_ymd_opt(-1, 12, 31).unwrap()).ok(),
                Some(r#""-0001-12-31""#.into())
            );
            assert_eq!(
                serde_json::to_string(&NaiveDate::MIN).ok(),
                Some(r#""-262143-01-01""#.into())
            );
            assert_eq!(
                serde_json::to_string(&NaiveDate::MAX).ok(),
                Some(r#""+262142-12-31""#.into())
            );
        }

        #[test]
        fn test_serde_deserialize() {
            let from_str = serde_json::from_str::<NaiveDate>;

            assert_eq!(
                from_str(r#""2016-07-08""#).ok(),
                Some(NaiveDate::from_ymd_opt(2016, 7, 8).unwrap())
            );
            assert_eq!(
                from_str(r#""2016-7-8""#).ok(),
                Some(NaiveDate::from_ymd_opt(2016, 7, 8).unwrap())
            );
            assert_eq!(from_str(r#""+002016-07-08""#).ok(), NaiveDate::from_ymd_opt(2016, 7, 8));
            assert_eq!(
                from_str(r#""0000-01-01""#).ok(),
                Some(NaiveDate::from_ymd_opt(0, 1, 1).unwrap())
            );
            assert_eq!(
                from_str(r#""0-1-1""#).ok(),
                Some(NaiveDate::from_ymd_opt(0, 1, 1).unwrap())
            );
            assert_eq!(
                from_str(r#""-0001-12-31""#).ok(),
                Some(NaiveDate::from_ymd_opt(-1, 12, 31).unwrap())
            );
            assert_eq!(from_str(r#""-262143-01-01""#).ok(), Some(NaiveDate::MIN));
            assert_eq!(from_str(r#""+262142-12-31""#).ok(), Some(NaiveDate::MAX));

            // bad formats
            assert!(from_str(r#""""#).is_err());
            assert!(from_str(r#""20001231""#).is_err());
            assert!(from_str(r#""2000-00-00""#).is_err());
            assert!(from_str(r#""2000-02-30""#).is_err());
            assert!(from_str(r#""2001-02-29""#).is_err());
            assert!(from_str(r#""2002-002-28""#).is_err());
            assert!(from_str(r#""yyyy-mm-dd""#).is_err());
            assert!(from_str(r#"0"#).is_err());
            assert!(from_str(r#"20.01"#).is_err());
            let min = i32::MIN.to_string();
            assert!(from_str(&min).is_err());
            let max = i32::MAX.to_string();
            assert!(from_str(&max).is_err());
            let min = i64::MIN.to_string();
            assert!(from_str(&min).is_err());
            let max = i64::MAX.to_string();
            assert!(from_str(&max).is_err());
            assert!(from_str(r#"{}"#).is_err());
        }

        #[test]
        fn test_serde_bincode() {
            // Bincode is relevant to test separately from JSON because
            // it is not self-describing.
            use bincode::{deserialize, serialize};

            let d = NaiveDate::from_ymd_opt(2014, 7, 24).unwrap();
            let encoded = serialize(&d).unwrap();
            let decoded: NaiveDate = deserialize(&encoded).unwrap();
            assert_eq!(d, decoded);
        }
    }
}
