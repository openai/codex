// This is a part of Chrono.
// See README.md and LICENSE.txt for details.

//! A collection of parsed date and time items.
//! They can be constructed incrementally while being checked for consistency.

use super::{IMPOSSIBLE, NOT_ENOUGH, OUT_OF_RANGE, ParseResult};
use crate::naive::{NaiveDate, NaiveDateTime, NaiveTime};
use crate::offset::{FixedOffset, MappedLocalTime, Offset, TimeZone};
use crate::{DateTime, Datelike, TimeDelta, Timelike, Weekday};

/// A type to hold parsed fields of date and time that can check all fields are consistent.
///
/// There are three classes of methods:
///
/// - `set_*` methods to set fields you have available. They do a basic range check, and if the
///   same field is set more than once it is checked for consistency.
///
/// - `to_*` methods try to make a concrete date and time value out of set fields.
///   They fully check that all fields are consistent and whether the date/datetime exists.
///
/// - Methods to inspect the parsed fields.
///
/// `Parsed` is used internally by all parsing functions in chrono. It is a public type so that it
/// can be used to write custom parsers that reuse the resolving algorithm, or to inspect the
/// results of a string parsed with chrono without converting it to concrete types.
///
/// # Resolving algorithm
///
/// Resolving date/time parts is littered with lots of corner cases, which is why common date/time
/// parsers do not implement it correctly.
///
/// Chrono provides a complete resolution algorithm that checks all fields for consistency via the
/// `Parsed` type.
///
/// As an easy example, consider RFC 2822. The [RFC 2822 date and time format] has a day of the week
/// part, which should be consistent with the other date parts. But a `strptime`-based parse would
/// happily accept inconsistent input:
///
/// ```python
/// >>> import time
/// >>> time.strptime('Wed, 31 Dec 2014 04:26:40 +0000',
///                   '%a, %d %b %Y %H:%M:%S +0000')
/// time.struct_time(tm_year=2014, tm_mon=12, tm_mday=31,
///                  tm_hour=4, tm_min=26, tm_sec=40,
///                  tm_wday=2, tm_yday=365, tm_isdst=-1)
/// >>> time.strptime('Thu, 31 Dec 2014 04:26:40 +0000',
///                   '%a, %d %b %Y %H:%M:%S +0000')
/// time.struct_time(tm_year=2014, tm_mon=12, tm_mday=31,
///                  tm_hour=4, tm_min=26, tm_sec=40,
///                  tm_wday=3, tm_yday=365, tm_isdst=-1)
/// ```
///
/// [RFC 2822 date and time format]: https://tools.ietf.org/html/rfc2822#section-3.3
///
/// # Example
///
/// Let's see how `Parsed` correctly detects the second RFC 2822 string from before is inconsistent.
///
/// ```
/// # #[cfg(feature = "alloc")] {
/// use chrono::format::{ParseErrorKind, Parsed};
/// use chrono::Weekday;
///
/// let mut parsed = Parsed::new();
/// parsed.set_weekday(Weekday::Wed)?;
/// parsed.set_day(31)?;
/// parsed.set_month(12)?;
/// parsed.set_year(2014)?;
/// parsed.set_hour(4)?;
/// parsed.set_minute(26)?;
/// parsed.set_second(40)?;
/// parsed.set_offset(0)?;
/// let dt = parsed.to_datetime()?;
/// assert_eq!(dt.to_rfc2822(), "Wed, 31 Dec 2014 04:26:40 +0000");
///
/// let mut parsed = Parsed::new();
/// parsed.set_weekday(Weekday::Thu)?; // changed to the wrong day
/// parsed.set_day(31)?;
/// parsed.set_month(12)?;
/// parsed.set_year(2014)?;
/// parsed.set_hour(4)?;
/// parsed.set_minute(26)?;
/// parsed.set_second(40)?;
/// parsed.set_offset(0)?;
/// let result = parsed.to_datetime();
///
/// assert!(result.is_err());
/// if let Err(error) = result {
///     assert_eq!(error.kind(), ParseErrorKind::Impossible);
/// }
/// # }
/// # Ok::<(), chrono::ParseError>(())
/// ```
///
/// The same using chrono's built-in parser for RFC 2822 (the [RFC2822 formatting item]) and
/// [`format::parse()`] showing how to inspect a field on failure.
///
/// [RFC2822 formatting item]: crate::format::Fixed::RFC2822
/// [`format::parse()`]: crate::format::parse()
///
/// ```
/// # #[cfg(feature = "alloc")] {
/// use chrono::format::{parse, Fixed, Item, Parsed};
/// use chrono::Weekday;
///
/// let rfc_2822 = [Item::Fixed(Fixed::RFC2822)];
///
/// let mut parsed = Parsed::new();
/// parse(&mut parsed, "Wed, 31 Dec 2014 04:26:40 +0000", rfc_2822.iter())?;
/// let dt = parsed.to_datetime()?;
///
/// assert_eq!(dt.to_rfc2822(), "Wed, 31 Dec 2014 04:26:40 +0000");
///
/// let mut parsed = Parsed::new();
/// parse(&mut parsed, "Thu, 31 Dec 2014 04:26:40 +0000", rfc_2822.iter())?;
/// let result = parsed.to_datetime();
///
/// assert!(result.is_err());
/// if result.is_err() {
///     // What is the weekday?
///     assert_eq!(parsed.weekday(), Some(Weekday::Thu));
/// }
/// # }
/// # Ok::<(), chrono::ParseError>(())
/// ```
#[allow(clippy::manual_non_exhaustive)]
#[derive(Clone, PartialEq, Eq, Debug, Default, Hash)]
pub struct Parsed {
    #[doc(hidden)]
    pub year: Option<i32>,
    #[doc(hidden)]
    pub year_div_100: Option<i32>,
    #[doc(hidden)]
    pub year_mod_100: Option<i32>,
    #[doc(hidden)]
    pub isoyear: Option<i32>,
    #[doc(hidden)]
    pub isoyear_div_100: Option<i32>,
    #[doc(hidden)]
    pub isoyear_mod_100: Option<i32>,
    #[doc(hidden)]
    pub quarter: Option<u32>,
    #[doc(hidden)]
    pub month: Option<u32>,
    #[doc(hidden)]
    pub week_from_sun: Option<u32>,
    #[doc(hidden)]
    pub week_from_mon: Option<u32>,
    #[doc(hidden)]
    pub isoweek: Option<u32>,
    #[doc(hidden)]
    pub weekday: Option<Weekday>,
    #[doc(hidden)]
    pub ordinal: Option<u32>,
    #[doc(hidden)]
    pub day: Option<u32>,
    #[doc(hidden)]
    pub hour_div_12: Option<u32>,
    #[doc(hidden)]
    pub hour_mod_12: Option<u32>,
    #[doc(hidden)]
    pub minute: Option<u32>,
    #[doc(hidden)]
    pub second: Option<u32>,
    #[doc(hidden)]
    pub nanosecond: Option<u32>,
    #[doc(hidden)]
    pub timestamp: Option<i64>,
    #[doc(hidden)]
    pub offset: Option<i32>,
    #[doc(hidden)]
    _dummy: (),
}

/// Checks if `old` is either empty or has the same value as `new` (i.e. "consistent"),
/// and if it is empty, set `old` to `new` as well.
#[inline]
fn set_if_consistent<T: PartialEq>(old: &mut Option<T>, new: T) -> ParseResult<()> {
    match old {
        Some(old) if *old != new => Err(IMPOSSIBLE),
        _ => {
            *old = Some(new);
            Ok(())
        }
    }
}

impl Parsed {
    /// Returns the initial value of parsed parts.
    #[must_use]
    pub fn new() -> Parsed {
        Parsed::default()
    }

    /// Set the [`year`](Parsed::year) field to the given value.
    ///
    /// The value can be negative, unlike the [`year_div_100`](Parsed::year_div_100) and
    /// [`year_mod_100`](Parsed::year_mod_100) fields.
    ///
    /// # Errors
    ///
    /// Returns `OUT_OF_RANGE` if `value` is outside the range of an `i32`.
    ///
    /// Returns `IMPOSSIBLE` if this field was already set to a different value.
    #[inline]
    pub fn set_year(&mut self, value: i64) -> ParseResult<()> {
        set_if_consistent(&mut self.year, i32::try_from(value).map_err(|_| OUT_OF_RANGE)?)
    }

    /// Set the [`year_div_100`](Parsed::year_div_100) field to the given value.
    ///
    /// # Errors
    ///
    /// Returns `OUT_OF_RANGE` if `value` is negative or if it is greater than `i32::MAX`.
    ///
    /// Returns `IMPOSSIBLE` if this field was already set to a different value.
    #[inline]
    pub fn set_year_div_100(&mut self, value: i64) -> ParseResult<()> {
        if !(0..=i32::MAX as i64).contains(&value) {
            return Err(OUT_OF_RANGE);
        }
        set_if_consistent(&mut self.year_div_100, value as i32)
    }

    /// Set the [`year_mod_100`](Parsed::year_mod_100) field to the given value.
    ///
    /// When set it implies that the year is not negative.
    ///
    /// If this field is set while the [`year_div_100`](Parsed::year_div_100) field is missing (and
    /// the full [`year`](Parsed::year) field is also not set), it assumes a default value for the
    /// [`year_div_100`](Parsed::year_div_100) field.
    /// The default is 19 when `year_mod_100 >= 70` and 20 otherwise.
    ///
    /// # Errors
    ///
    /// Returns `OUT_OF_RANGE` if `value` is negative or if it is greater than 99.
    ///
    /// Returns `IMPOSSIBLE` if this field was already set to a different value.
    #[inline]
    pub fn set_year_mod_100(&mut self, value: i64) -> ParseResult<()> {
        if !(0..100).contains(&value) {
            return Err(OUT_OF_RANGE);
        }
        set_if_consistent(&mut self.year_mod_100, value as i32)
    }

    /// Set the [`isoyear`](Parsed::isoyear) field, that is part of an [ISO 8601 week date], to the
    /// given value.
    ///
    /// The value can be negative, unlike the [`isoyear_div_100`](Parsed::isoyear_div_100) and
    /// [`isoyear_mod_100`](Parsed::isoyear_mod_100) fields.
    ///
    /// [ISO 8601 week date]: crate::NaiveDate#week-date
    ///
    /// # Errors
    ///
    /// Returns `OUT_OF_RANGE` if `value` is outside the range of an `i32`.
    ///
    /// Returns `IMPOSSIBLE` if this field was already set to a different value.
    #[inline]
    pub fn set_isoyear(&mut self, value: i64) -> ParseResult<()> {
        set_if_consistent(&mut self.isoyear, i32::try_from(value).map_err(|_| OUT_OF_RANGE)?)
    }

    /// Set the [`isoyear_div_100`](Parsed::isoyear_div_100) field, that is part of an
    /// [ISO 8601 week date], to the given value.
    ///
    /// [ISO 8601 week date]: crate::NaiveDate#week-date
    ///
    /// # Errors
    ///
    /// Returns `OUT_OF_RANGE` if `value` is negative or if it is greater than `i32::MAX`.
    ///
    /// Returns `IMPOSSIBLE` if this field was already set to a different value.
    #[inline]
    pub fn set_isoyear_div_100(&mut self, value: i64) -> ParseResult<()> {
        if !(0..=i32::MAX as i64).contains(&value) {
            return Err(OUT_OF_RANGE);
        }
        set_if_consistent(&mut self.isoyear_div_100, value as i32)
    }

    /// Set the [`isoyear_mod_100`](Parsed::isoyear_mod_100) field, that is part of an
    /// [ISO 8601 week date], to the given value.
    ///
    /// When set it implies that the year is not negative.
    ///
    /// If this field is set while the [`isoyear_div_100`](Parsed::isoyear_div_100) field is missing
    /// (and the full [`isoyear`](Parsed::isoyear) field is also not set), it assumes a default
    /// value for the [`isoyear_div_100`](Parsed::isoyear_div_100) field.
    /// The default is 19 when `year_mod_100 >= 70` and 20 otherwise.
    ///
    /// [ISO 8601 week date]: crate::NaiveDate#week-date
    ///
    /// # Errors
    ///
    /// Returns `OUT_OF_RANGE` if `value` is negative or if it is greater than 99.
    ///
    /// Returns `IMPOSSIBLE` if this field was already set to a different value.
    #[inline]
    pub fn set_isoyear_mod_100(&mut self, value: i64) -> ParseResult<()> {
        if !(0..100).contains(&value) {
            return Err(OUT_OF_RANGE);
        }
        set_if_consistent(&mut self.isoyear_mod_100, value as i32)
    }

    /// Set the [`quarter`](Parsed::quarter) field to the given value.
    ///
    /// Quarter 1 starts in January.
    ///
    /// # Errors
    ///
    /// Returns `OUT_OF_RANGE` if `value` is not in the range 1-4.
    ///
    /// Returns `IMPOSSIBLE` if this field was already set to a different value.
    #[inline]
    pub fn set_quarter(&mut self, value: i64) -> ParseResult<()> {
        if !(1..=4).contains(&value) {
            return Err(OUT_OF_RANGE);
        }
        set_if_consistent(&mut self.quarter, value as u32)
    }

    /// Set the [`month`](Parsed::month) field to the given value.
    ///
    /// # Errors
    ///
    /// Returns `OUT_OF_RANGE` if `value` is not in the range 1-12.
    ///
    /// Returns `IMPOSSIBLE` if this field was already set to a different value.
    #[inline]
    pub fn set_month(&mut self, value: i64) -> ParseResult<()> {
        if !(1..=12).contains(&value) {
            return Err(OUT_OF_RANGE);
        }
        set_if_consistent(&mut self.month, value as u32)
    }

    /// Set the [`week_from_sun`](Parsed::week_from_sun) week number field to the given value.
    ///
    /// Week 1 starts at the first Sunday of January.
    ///
    /// # Errors
    ///
    /// Returns `OUT_OF_RANGE` if `value` is not in the range 0-53.
    ///
    /// Returns `IMPOSSIBLE` if this field was already set to a different value.
    #[inline]
    pub fn set_week_from_sun(&mut self, value: i64) -> ParseResult<()> {
        if !(0..=53).contains(&value) {
            return Err(OUT_OF_RANGE);
        }
        set_if_consistent(&mut self.week_from_sun, value as u32)
    }

    /// Set the [`week_from_mon`](Parsed::week_from_mon) week number field to the given value.
    /// Set the 'week number starting with Monday' field to the given value.
    ///
    /// Week 1 starts at the first Monday of January.
    ///
    /// # Errors
    ///
    /// Returns `OUT_OF_RANGE` if `value` is not in the range 0-53.
    ///
    /// Returns `IMPOSSIBLE` if this field was already set to a different value.
    #[inline]
    pub fn set_week_from_mon(&mut self, value: i64) -> ParseResult<()> {
        if !(0..=53).contains(&value) {
            return Err(OUT_OF_RANGE);
        }
        set_if_consistent(&mut self.week_from_mon, value as u32)
    }

    /// Set the [`isoweek`](Parsed::isoweek) field for an [ISO 8601 week date] to the given value.
    ///
    /// [ISO 8601 week date]: crate::NaiveDate#week-date
    ///
    /// # Errors
    ///
    /// Returns `OUT_OF_RANGE` if `value` is not in the range 1-53.
    ///
    /// Returns `IMPOSSIBLE` if this field was already set to a different value.
    #[inline]
    pub fn set_isoweek(&mut self, value: i64) -> ParseResult<()> {
        if !(1..=53).contains(&value) {
            return Err(OUT_OF_RANGE);
        }
        set_if_consistent(&mut self.isoweek, value as u32)
    }

    /// Set the [`weekday`](Parsed::weekday) field to the given value.
    ///
    /// # Errors
    ///
    /// Returns `IMPOSSIBLE` if this field was already set to a different value.
    #[inline]
    pub fn set_weekday(&mut self, value: Weekday) -> ParseResult<()> {
        set_if_consistent(&mut self.weekday, value)
    }

    /// Set the [`ordinal`](Parsed::ordinal) (day of the year) field to the given value.
    ///
    /// # Errors
    ///
    /// Returns `OUT_OF_RANGE` if `value` is not in the range 1-366.
    ///
    /// Returns `IMPOSSIBLE` if this field was already set to a different value.
    #[inline]
    pub fn set_ordinal(&mut self, value: i64) -> ParseResult<()> {
        if !(1..=366).contains(&value) {
            return Err(OUT_OF_RANGE);
        }
        set_if_consistent(&mut self.ordinal, value as u32)
    }

    /// Set the [`day`](Parsed::day) of the month field to the given value.
    ///
    /// # Errors
    ///
    /// Returns `OUT_OF_RANGE` if `value` is not in the range 1-31.
    ///
    /// Returns `IMPOSSIBLE` if this field was already set to a different value.
    #[inline]
    pub fn set_day(&mut self, value: i64) -> ParseResult<()> {
        if !(1..=31).contains(&value) {
            return Err(OUT_OF_RANGE);
        }
        set_if_consistent(&mut self.day, value as u32)
    }

    /// Set the [`hour_div_12`](Parsed::hour_div_12) am/pm field to the given value.
    ///
    /// `false` indicates AM and `true` indicates PM.
    ///
    /// # Errors
    ///
    /// Returns `IMPOSSIBLE` if this field was already set to a different value.
    #[inline]
    pub fn set_ampm(&mut self, value: bool) -> ParseResult<()> {
        set_if_consistent(&mut self.hour_div_12, value as u32)
    }

    /// Set the [`hour_mod_12`](Parsed::hour_mod_12) field, for the hour number in 12-hour clocks,
    /// to the given value.
    ///
    /// Value must be in the canonical range of 1-12.
    /// It will internally be stored as 0-11 (`value % 12`).
    ///
    /// # Errors
    ///
    /// Returns `OUT_OF_RANGE` if `value` is not in the range 1-12.
    ///
    /// Returns `IMPOSSIBLE` if this field was already set to a different value.
    #[inline]
    pub fn set_hour12(&mut self, mut value: i64) -> ParseResult<()> {
        if !(1..=12).contains(&value) {
            return Err(OUT_OF_RANGE);
        }
        if value == 12 {
            value = 0
        }
        set_if_consistent(&mut self.hour_mod_12, value as u32)
    }

    /// Set the [`hour_div_12`](Parsed::hour_div_12) and [`hour_mod_12`](Parsed::hour_mod_12)
    /// fields to the given value for a 24-hour clock.
    ///
    /// # Errors
    ///
    /// May return `OUT_OF_RANGE` if `value` is not in the range 0-23.
    /// Currently only checks the value is not out of range for a `u32`.
    ///
    /// Returns `IMPOSSIBLE` one of the fields was already set to a different value.
    #[inline]
    pub fn set_hour(&mut self, value: i64) -> ParseResult<()> {
        let (hour_div_12, hour_mod_12) = match value {
            hour @ 0..=11 => (0, hour as u32),
            hour @ 12..=23 => (1, hour as u32 - 12),
            _ => return Err(OUT_OF_RANGE),
        };
        set_if_consistent(&mut self.hour_div_12, hour_div_12)?;
        set_if_consistent(&mut self.hour_mod_12, hour_mod_12)
    }

    /// Set the [`minute`](Parsed::minute) field to the given value.
    ///
    /// # Errors
    ///
    /// Returns `OUT_OF_RANGE` if `value` is not in the range 0-59.
    ///
    /// Returns `IMPOSSIBLE` if this field was already set to a different value.
    #[inline]
    pub fn set_minute(&mut self, value: i64) -> ParseResult<()> {
        if !(0..=59).contains(&value) {
            return Err(OUT_OF_RANGE);
        }
        set_if_consistent(&mut self.minute, value as u32)
    }

    /// Set the [`second`](Parsed::second) field to the given value.
    ///
    /// The value can be 60 in the case of a leap second.
    ///
    /// # Errors
    ///
    /// Returns `OUT_OF_RANGE` if `value` is not in the range 0-60.
    ///
    /// Returns `IMPOSSIBLE` if this field was already set to a different value.
    #[inline]
    pub fn set_second(&mut self, value: i64) -> ParseResult<()> {
        if !(0..=60).contains(&value) {
            return Err(OUT_OF_RANGE);
        }
        set_if_consistent(&mut self.second, value as u32)
    }

    /// Set the [`nanosecond`](Parsed::nanosecond) field to the given value.
    ///
    /// This is the number of nanoseconds since the whole second.
    ///
    /// # Errors
    ///
    /// Returns `OUT_OF_RANGE` if `value` is not in the range 0-999,999,999.
    ///
    /// Returns `IMPOSSIBLE` if this field was already set to a different value.
    #[inline]
    pub fn set_nanosecond(&mut self, value: i64) -> ParseResult<()> {
        if !(0..=999_999_999).contains(&value) {
            return Err(OUT_OF_RANGE);
        }
        set_if_consistent(&mut self.nanosecond, value as u32)
    }

    /// Set the [`timestamp`](Parsed::timestamp) field to the given value.
    ///
    /// A Unix timestamp is defined as the number of non-leap seconds since midnight UTC on
    /// January 1, 1970.
    ///
    /// # Errors
    ///
    /// Returns `IMPOSSIBLE` if this field was already set to a different value.
    #[inline]
    pub fn set_timestamp(&mut self, value: i64) -> ParseResult<()> {
        set_if_consistent(&mut self.timestamp, value)
    }

    /// Set the [`offset`](Parsed::offset) field to the given value.
    ///
    /// The offset is in seconds from local time to UTC.
    ///
    /// # Errors
    ///
    /// Returns `OUT_OF_RANGE` if `value` is outside the range of an `i32`.
    ///
    /// Returns `IMPOSSIBLE` if this field was already set to a different value.
    #[inline]
    pub fn set_offset(&mut self, value: i64) -> ParseResult<()> {
        set_if_consistent(&mut self.offset, i32::try_from(value).map_err(|_| OUT_OF_RANGE)?)
    }

    /// Returns a parsed naive date out of given fields.
    ///
    /// This method is able to determine the date from given subset of fields:
    ///
    /// - Year, month, day.
    /// - Year, day of the year (ordinal).
    /// - Year, week number counted from Sunday or Monday, day of the week.
    /// - ISO week date.
    ///
    /// Gregorian year and ISO week date year can have their century number (`*_div_100`) omitted,
    /// the two-digit year is used to guess the century number then.
    ///
    /// It checks all given date fields are consistent with each other.
    ///
    /// # Errors
    ///
    /// This method returns:
    /// - `IMPOSSIBLE` if any of the date fields conflict.
    /// - `NOT_ENOUGH` if there are not enough fields set in `Parsed` for a complete date.
    /// - `OUT_OF_RANGE`
    ///   - if any of the date fields of `Parsed` are set to a value beyond their acceptable range.
    ///   - if the value would be outside the range of a [`NaiveDate`].
    ///   - if the date does not exist.
    pub fn to_naive_date(&self) -> ParseResult<NaiveDate> {
        fn resolve_year(
            y: Option<i32>,
            q: Option<i32>,
            r: Option<i32>,
        ) -> ParseResult<Option<i32>> {
            match (y, q, r) {
                // if there is no further information, simply return the given full year.
                // this is a common case, so let's avoid division here.
                (y, None, None) => Ok(y),

                // if there is a full year *and* also quotient and/or modulo,
                // check if present quotient and/or modulo is consistent to the full year.
                // since the presence of those fields means a positive full year,
                // we should filter a negative full year first.
                (Some(y), q, r @ Some(0..=99)) | (Some(y), q, r @ None) => {
                    if y < 0 {
                        return Err(IMPOSSIBLE);
                    }
                    let q_ = y / 100;
                    let r_ = y % 100;
                    if q.unwrap_or(q_) == q_ && r.unwrap_or(r_) == r_ {
                        Ok(Some(y))
                    } else {
                        Err(IMPOSSIBLE)
                    }
                }

                // the full year is missing but we have quotient and modulo.
                // reconstruct the full year. make sure that the result is always positive.
                (None, Some(q), Some(r @ 0..=99)) => {
                    if q < 0 {
                        return Err(IMPOSSIBLE);
                    }
                    let y = q.checked_mul(100).and_then(|v| v.checked_add(r));
                    Ok(Some(y.ok_or(OUT_OF_RANGE)?))
                }

                // we only have modulo. try to interpret a modulo as a conventional two-digit year.
                // note: we are affected by Rust issue #18060. avoid multiple range patterns.
                (None, None, Some(r @ 0..=99)) => Ok(Some(r + if r < 70 { 2000 } else { 1900 })),

                // otherwise it is an out-of-bound or insufficient condition.
                (None, Some(_), None) => Err(NOT_ENOUGH),
                (_, _, Some(_)) => Err(OUT_OF_RANGE),
            }
        }

        let given_year = resolve_year(self.year, self.year_div_100, self.year_mod_100)?;
        let given_isoyear = resolve_year(self.isoyear, self.isoyear_div_100, self.isoyear_mod_100)?;

        // verify the normal year-month-day date.
        let verify_ymd = |date: NaiveDate| {
            let year = date.year();
            let (year_div_100, year_mod_100) = if year >= 0 {
                (Some(year / 100), Some(year % 100))
            } else {
                (None, None) // they should be empty to be consistent
            };
            let month = date.month();
            let day = date.day();
            self.year.unwrap_or(year) == year
                && self.year_div_100.or(year_div_100) == year_div_100
                && self.year_mod_100.or(year_mod_100) == year_mod_100
                && self.month.unwrap_or(month) == month
                && self.day.unwrap_or(day) == day
        };

        // verify the ISO week date.
        let verify_isoweekdate = |date: NaiveDate| {
            let week = date.iso_week();
            let isoyear = week.year();
            let isoweek = week.week();
            let weekday = date.weekday();
            let (isoyear_div_100, isoyear_mod_100) = if isoyear >= 0 {
                (Some(isoyear / 100), Some(isoyear % 100))
            } else {
                (None, None) // they should be empty to be consistent
            };
            self.isoyear.unwrap_or(isoyear) == isoyear
                && self.isoyear_div_100.or(isoyear_div_100) == isoyear_div_100
                && self.isoyear_mod_100.or(isoyear_mod_100) == isoyear_mod_100
                && self.isoweek.unwrap_or(isoweek) == isoweek
                && self.weekday.unwrap_or(weekday) == weekday
        };

        // verify the ordinal and other (non-ISO) week dates.
        let verify_ordinal = |date: NaiveDate| {
            let ordinal = date.ordinal();
            let week_from_sun = date.weeks_from(Weekday::Sun);
            let week_from_mon = date.weeks_from(Weekday::Mon);
            self.ordinal.unwrap_or(ordinal) == ordinal
                && self.week_from_sun.map_or(week_from_sun, |v| v as i32) == week_from_sun
                && self.week_from_mon.map_or(week_from_mon, |v| v as i32) == week_from_mon
        };

        // test several possibilities.
        // tries to construct a full `NaiveDate` as much as possible, then verifies that
        // it is consistent with other given fields.
        let (verified, parsed_date) = match (given_year, given_isoyear, self) {
            (Some(year), _, &Parsed { month: Some(month), day: Some(day), .. }) => {
                // year, month, day
                let date = NaiveDate::from_ymd_opt(year, month, day).ok_or(OUT_OF_RANGE)?;
                (verify_isoweekdate(date) && verify_ordinal(date), date)
            }

            (Some(year), _, &Parsed { ordinal: Some(ordinal), .. }) => {
                // year, day of the year
                let date = NaiveDate::from_yo_opt(year, ordinal).ok_or(OUT_OF_RANGE)?;
                (verify_ymd(date) && verify_isoweekdate(date) && verify_ordinal(date), date)
            }

            (Some(year), _, &Parsed { week_from_sun: Some(week), weekday: Some(weekday), .. }) => {
                // year, week (starting at 1st Sunday), day of the week
                let date = resolve_week_date(year, week, weekday, Weekday::Sun)?;
                (verify_ymd(date) && verify_isoweekdate(date) && verify_ordinal(date), date)
            }

            (Some(year), _, &Parsed { week_from_mon: Some(week), weekday: Some(weekday), .. }) => {
                // year, week (starting at 1st Monday), day of the week
                let date = resolve_week_date(year, week, weekday, Weekday::Mon)?;
                (verify_ymd(date) && verify_isoweekdate(date) && verify_ordinal(date), date)
            }

            (_, Some(isoyear), &Parsed { isoweek: Some(isoweek), weekday: Some(weekday), .. }) => {
                // ISO year, week, day of the week
                let date = NaiveDate::from_isoywd_opt(isoyear, isoweek, weekday);
                let date = date.ok_or(OUT_OF_RANGE)?;
                (verify_ymd(date) && verify_ordinal(date), date)
            }

            (_, _, _) => return Err(NOT_ENOUGH),
        };

        if !verified {
            return Err(IMPOSSIBLE);
        } else if let Some(parsed) = self.quarter {
            if parsed != parsed_date.quarter() {
                return Err(IMPOSSIBLE);
            }
        }

        Ok(parsed_date)
    }

    /// Returns a parsed naive time out of given fields.
    ///
    /// This method is able to determine the time from given subset of fields:
    ///
    /// - Hour, minute. (second and nanosecond assumed to be 0)
    /// - Hour, minute, second. (nanosecond assumed to be 0)
    /// - Hour, minute, second, nanosecond.
    ///
    /// It is able to handle leap seconds when given second is 60.
    ///
    /// # Errors
    ///
    /// This method returns:
    /// - `OUT_OF_RANGE` if any of the time fields of `Parsed` are set to a value beyond
    ///   their acceptable range.
    /// - `NOT_ENOUGH` if an hour field is missing, if AM/PM is missing in a 12-hour clock,
    ///   if minutes are missing, or if seconds are missing while the nanosecond field is present.
    pub fn to_naive_time(&self) -> ParseResult<NaiveTime> {
        let hour_div_12 = match self.hour_div_12 {
            Some(v @ 0..=1) => v,
            Some(_) => return Err(OUT_OF_RANGE),
            None => return Err(NOT_ENOUGH),
        };
        let hour_mod_12 = match self.hour_mod_12 {
            Some(v @ 0..=11) => v,
            Some(_) => return Err(OUT_OF_RANGE),
            None => return Err(NOT_ENOUGH),
        };
        let hour = hour_div_12 * 12 + hour_mod_12;

        let minute = match self.minute {
            Some(v @ 0..=59) => v,
            Some(_) => return Err(OUT_OF_RANGE),
            None => return Err(NOT_ENOUGH),
        };

        // we allow omitting seconds or nanoseconds, but they should be in the range.
        let (second, mut nano) = match self.second.unwrap_or(0) {
            v @ 0..=59 => (v, 0),
            60 => (59, 1_000_000_000),
            _ => return Err(OUT_OF_RANGE),
        };
        nano += match self.nanosecond {
            Some(v @ 0..=999_999_999) if self.second.is_some() => v,
            Some(0..=999_999_999) => return Err(NOT_ENOUGH), // second is missing
            Some(_) => return Err(OUT_OF_RANGE),
            None => 0,
        };

        NaiveTime::from_hms_nano_opt(hour, minute, second, nano).ok_or(OUT_OF_RANGE)
    }

    /// Returns a parsed naive date and time out of given fields, except for the offset field.
    ///
    /// The offset is assumed to have a given value. It is not compared against the offset field set
    /// in the `Parsed` type, so it is allowed to be inconsistent.
    ///
    /// This method is able to determine the combined date and time from date and time fields or
    /// from a single timestamp field. It checks all fields are consistent with each other.
    ///
    /// # Errors
    ///
    /// This method returns:
    /// - `IMPOSSIBLE`  if any of the date fields conflict, or if a timestamp conflicts with any of
    ///   the other fields.
    /// - `NOT_ENOUGH` if there are not enough fields set in `Parsed` for a complete datetime.
    /// - `OUT_OF_RANGE`
    ///   - if any of the date or time fields of `Parsed` are set to a value beyond their acceptable
    ///     range.
    ///   - if the value would be outside the range of a [`NaiveDateTime`].
    ///   - if the date does not exist.
    pub fn to_naive_datetime_with_offset(&self, offset: i32) -> ParseResult<NaiveDateTime> {
        let date = self.to_naive_date();
        let time = self.to_naive_time();
        if let (Ok(date), Ok(time)) = (date, time) {
            let datetime = date.and_time(time);

            // verify the timestamp field if any
            // the following is safe, `timestamp` is very limited in range
            let timestamp = datetime.and_utc().timestamp() - i64::from(offset);
            if let Some(given_timestamp) = self.timestamp {
                // if `datetime` represents a leap second, it might be off by one second.
                if given_timestamp != timestamp
                    && !(datetime.nanosecond() >= 1_000_000_000 && given_timestamp == timestamp + 1)
                {
                    return Err(IMPOSSIBLE);
                }
            }

            Ok(datetime)
        } else if let Some(timestamp) = self.timestamp {
            use super::ParseError as PE;
            use super::ParseErrorKind::{Impossible, OutOfRange};

            // if date and time is problematic already, there is no point proceeding.
            // we at least try to give a correct error though.
            match (date, time) {
                (Err(PE(OutOfRange)), _) | (_, Err(PE(OutOfRange))) => return Err(OUT_OF_RANGE),
                (Err(PE(Impossible)), _) | (_, Err(PE(Impossible))) => return Err(IMPOSSIBLE),
                (_, _) => {} // one of them is insufficient
            }

            // reconstruct date and time fields from timestamp
            let ts = timestamp.checked_add(i64::from(offset)).ok_or(OUT_OF_RANGE)?;
            let mut datetime = DateTime::from_timestamp_secs(ts).ok_or(OUT_OF_RANGE)?.naive_utc();

            // fill year, ordinal, hour, minute and second fields from timestamp.
            // if existing fields are consistent, this will allow the full date/time reconstruction.
            let mut parsed = self.clone();
            if parsed.second == Some(60) {
                // `datetime.second()` cannot be 60, so this is the only case for a leap second.
                match datetime.second() {
                    // it's okay, just do not try to overwrite the existing field.
                    59 => {}
                    // `datetime` is known to be off by one second.
                    0 => {
                        datetime -= TimeDelta::try_seconds(1).unwrap();
                    }
                    // otherwise it is impossible.
                    _ => return Err(IMPOSSIBLE),
                }
            // ...and we have the correct candidates for other fields.
            } else {
                parsed.set_second(i64::from(datetime.second()))?;
            }
            parsed.set_year(i64::from(datetime.year()))?;
            parsed.set_ordinal(i64::from(datetime.ordinal()))?; // more efficient than ymd
            parsed.set_hour(i64::from(datetime.hour()))?;
            parsed.set_minute(i64::from(datetime.minute()))?;

            // validate other fields (e.g. week) and return
            let date = parsed.to_naive_date()?;
            let time = parsed.to_naive_time()?;
            Ok(date.and_time(time))
        } else {
            // reproduce the previous error(s)
            date?;
            time?;
            unreachable!()
        }
    }

    /// Returns a parsed fixed time zone offset out of given fields.
    ///
    /// # Errors
    ///
    /// This method returns:
    /// - `OUT_OF_RANGE` if the offset is out of range for a `FixedOffset`.
    /// - `NOT_ENOUGH` if the offset field is not set.
    pub fn to_fixed_offset(&self) -> ParseResult<FixedOffset> {
        FixedOffset::east_opt(self.offset.ok_or(NOT_ENOUGH)?).ok_or(OUT_OF_RANGE)
    }

    /// Returns a parsed timezone-aware date and time out of given fields.
    ///
    /// This method is able to determine the combined date and time from date, time and offset
    /// fields, and/or from a single timestamp field. It checks all fields are consistent with each
    /// other.
    ///
    /// # Errors
    ///
    /// This method returns:
    /// - `IMPOSSIBLE`  if any of the date fields conflict, or if a timestamp conflicts with any of
    ///   the other fields.
    /// - `NOT_ENOUGH` if there are not enough fields set in `Parsed` for a complete datetime
    ///   including offset from UTC.
    /// - `OUT_OF_RANGE`
    ///   - if any of the fields of `Parsed` are set to a value beyond their acceptable
    ///     range.
    ///   - if the value would be outside the range of a [`NaiveDateTime`] or [`FixedOffset`].
    ///   - if the date does not exist.
    pub fn to_datetime(&self) -> ParseResult<DateTime<FixedOffset>> {
        // If there is no explicit offset, consider a timestamp value as indication of a UTC value.
        let offset = match (self.offset, self.timestamp) {
            (Some(off), _) => off,
            (None, Some(_)) => 0, // UNIX timestamp may assume 0 offset
            (None, None) => return Err(NOT_ENOUGH),
        };
        let datetime = self.to_naive_datetime_with_offset(offset)?;
        let offset = FixedOffset::east_opt(offset).ok_or(OUT_OF_RANGE)?;

        match offset.from_local_datetime(&datetime) {
            MappedLocalTime::None => Err(IMPOSSIBLE),
            MappedLocalTime::Single(t) => Ok(t),
            MappedLocalTime::Ambiguous(..) => Err(NOT_ENOUGH),
        }
    }

    /// Returns a parsed timezone-aware date and time out of given fields,
    /// with an additional [`TimeZone`] used to interpret and validate the local date.
    ///
    /// This method is able to determine the combined date and time from date and time, and/or from
    /// a single timestamp field. It checks all fields are consistent with each other.
    ///
    /// If the parsed fields include an UTC offset, it also has to be consistent with the offset in
    /// the provided `tz` time zone for that datetime.
    ///
    /// # Errors
    ///
    /// This method returns:
    /// - `IMPOSSIBLE`
    ///   - if any of the date fields conflict, if a timestamp conflicts with any of the other
    ///     fields, or if the offset field is set but differs from the offset at that time in the
    ///     `tz` time zone.
    ///   - if the local datetime does not exists in the provided time zone (because it falls in a
    ///     transition due to for example DST).
    /// - `NOT_ENOUGH` if there are not enough fields set in `Parsed` for a complete datetime, or if
    ///   the local time in the provided time zone is ambiguous (because it falls in a transition
    ///   due to for example DST) while there is no offset field or timestamp field set.
    /// - `OUT_OF_RANGE`
    ///   - if the value would be outside the range of a [`NaiveDateTime`] or [`FixedOffset`].
    ///   - if any of the fields of `Parsed` are set to a value beyond their acceptable range.
    ///   - if the date does not exist.
    pub fn to_datetime_with_timezone<Tz: TimeZone>(&self, tz: &Tz) -> ParseResult<DateTime<Tz>> {
        // if we have `timestamp` specified, guess an offset from that.
        let mut guessed_offset = 0;
        if let Some(timestamp) = self.timestamp {
            // make a naive `DateTime` from given timestamp and (if any) nanosecond.
            // an empty `nanosecond` is always equal to zero, so missing nanosecond is fine.
            let nanosecond = self.nanosecond.unwrap_or(0);
            let dt =
                DateTime::from_timestamp(timestamp, nanosecond).ok_or(OUT_OF_RANGE)?.naive_utc();
            guessed_offset = tz.offset_from_utc_datetime(&dt).fix().local_minus_utc();
        }

        // checks if the given `DateTime` has a consistent `Offset` with given `self.offset`.
        let check_offset = |dt: &DateTime<Tz>| {
            if let Some(offset) = self.offset {
                dt.offset().fix().local_minus_utc() == offset
            } else {
                true
            }
        };

        // `guessed_offset` should be correct when `self.timestamp` is given.
        // it will be 0 otherwise, but this is fine as the algorithm ignores offset for that case.
        let datetime = self.to_naive_datetime_with_offset(guessed_offset)?;
        match tz.from_local_datetime(&datetime) {
            MappedLocalTime::None => Err(IMPOSSIBLE),
            MappedLocalTime::Single(t) => {
                if check_offset(&t) {
                    Ok(t)
                } else {
                    Err(IMPOSSIBLE)
                }
            }
            MappedLocalTime::Ambiguous(min, max) => {
                // try to disambiguate two possible local dates by offset.
                match (check_offset(&min), check_offset(&max)) {
                    (false, false) => Err(IMPOSSIBLE),
                    (false, true) => Ok(max),
                    (true, false) => Ok(min),
                    (true, true) => Err(NOT_ENOUGH),
                }
            }
        }
    }

    /// Get the `year` field if set.
    ///
    /// See also [`set_year()`](Parsed::set_year).
    #[inline]
    pub fn year(&self) -> Option<i32> {
        self.year
    }

    /// Get the `year_div_100` field if set.
    ///
    /// See also [`set_year_div_100()`](Parsed::set_year_div_100).
    #[inline]
    pub fn year_div_100(&self) -> Option<i32> {
        self.year_div_100
    }

    /// Get the `year_mod_100` field if set.
    ///
    /// See also [`set_year_mod_100()`](Parsed::set_year_mod_100).
    #[inline]
    pub fn year_mod_100(&self) -> Option<i32> {
        self.year_mod_100
    }

    /// Get the `isoyear` field that is part of an [ISO 8601 week date] if set.
    ///
    /// See also [`set_isoyear()`](Parsed::set_isoyear).
    ///
    /// [ISO 8601 week date]: crate::NaiveDate#week-date
    #[inline]
    pub fn isoyear(&self) -> Option<i32> {
        self.isoyear
    }

    /// Get the `isoyear_div_100` field that is part of an [ISO 8601 week date] if set.
    ///
    /// See also [`set_isoyear_div_100()`](Parsed::set_isoyear_div_100).
    ///
    /// [ISO 8601 week date]: crate::NaiveDate#week-date
    #[inline]
    pub fn isoyear_div_100(&self) -> Option<i32> {
        self.isoyear_div_100
    }

    /// Get the `isoyear_mod_100` field that is part of an [ISO 8601 week date] if set.
    ///
    /// See also [`set_isoyear_mod_100()`](Parsed::set_isoyear_mod_100).
    ///
    /// [ISO 8601 week date]: crate::NaiveDate#week-date
    #[inline]
    pub fn isoyear_mod_100(&self) -> Option<i32> {
        self.isoyear_mod_100
    }

    /// Get the `quarter` field if set.
    ///
    /// See also [`set_quarter()`](Parsed::set_quarter).
    #[inline]
    pub fn quarter(&self) -> Option<u32> {
        self.quarter
    }

    /// Get the `month` field if set.
    ///
    /// See also [`set_month()`](Parsed::set_month).
    #[inline]
    pub fn month(&self) -> Option<u32> {
        self.month
    }

    /// Get the `week_from_sun` field if set.
    ///
    /// See also [`set_week_from_sun()`](Parsed::set_week_from_sun).
    #[inline]
    pub fn week_from_sun(&self) -> Option<u32> {
        self.week_from_sun
    }

    /// Get the `week_from_mon` field if set.
    ///
    /// See also [`set_week_from_mon()`](Parsed::set_week_from_mon).
    #[inline]
    pub fn week_from_mon(&self) -> Option<u32> {
        self.week_from_mon
    }

    /// Get the `isoweek` field that is part of an [ISO 8601 week date] if set.
    ///
    /// See also [`set_isoweek()`](Parsed::set_isoweek).
    ///
    /// [ISO 8601 week date]: crate::NaiveDate#week-date
    #[inline]
    pub fn isoweek(&self) -> Option<u32> {
        self.isoweek
    }

    /// Get the `weekday` field if set.
    ///
    /// See also [`set_weekday()`](Parsed::set_weekday).
    #[inline]
    pub fn weekday(&self) -> Option<Weekday> {
        self.weekday
    }

    /// Get the `ordinal` (day of the year) field if set.
    ///
    /// See also [`set_ordinal()`](Parsed::set_ordinal).
    #[inline]
    pub fn ordinal(&self) -> Option<u32> {
        self.ordinal
    }

    /// Get the `day` of the month field if set.
    ///
    /// See also [`set_day()`](Parsed::set_day).
    #[inline]
    pub fn day(&self) -> Option<u32> {
        self.day
    }

    /// Get the `hour_div_12` field (am/pm) if set.
    ///
    /// 0 indicates AM and 1 indicates PM.
    ///
    /// See also [`set_ampm()`](Parsed::set_ampm) and [`set_hour()`](Parsed::set_hour).
    #[inline]
    pub fn hour_div_12(&self) -> Option<u32> {
        self.hour_div_12
    }

    /// Get the `hour_mod_12` field if set.
    ///
    /// See also [`set_hour12()`](Parsed::set_hour12) and [`set_hour()`](Parsed::set_hour).
    pub fn hour_mod_12(&self) -> Option<u32> {
        self.hour_mod_12
    }

    /// Get the `minute` field if set.
    ///
    /// See also [`set_minute()`](Parsed::set_minute).
    #[inline]
    pub fn minute(&self) -> Option<u32> {
        self.minute
    }

    /// Get the `second` field if set.
    ///
    /// See also [`set_second()`](Parsed::set_second).
    #[inline]
    pub fn second(&self) -> Option<u32> {
        self.second
    }

    /// Get the `nanosecond` field if set.
    ///
    /// See also [`set_nanosecond()`](Parsed::set_nanosecond).
    #[inline]
    pub fn nanosecond(&self) -> Option<u32> {
        self.nanosecond
    }

    /// Get the `timestamp` field if set.
    ///
    /// See also [`set_timestamp()`](Parsed::set_timestamp).
    #[inline]
    pub fn timestamp(&self) -> Option<i64> {
        self.timestamp
    }

    /// Get the `offset` field if set.
    ///
    /// See also [`set_offset()`](Parsed::set_offset).
    #[inline]
    pub fn offset(&self) -> Option<i32> {
        self.offset
    }
}

/// Create a `NaiveDate` when given a year, week, weekday, and the definition at which day of the
/// week a week starts.
///
/// Returns `IMPOSSIBLE` if `week` is `0` or `53` and the `weekday` falls outside the year.
fn resolve_week_date(
    year: i32,
    week: u32,
    weekday: Weekday,
    week_start_day: Weekday,
) -> ParseResult<NaiveDate> {
    if week > 53 {
        return Err(OUT_OF_RANGE);
    }

    let first_day_of_year = NaiveDate::from_yo_opt(year, 1).ok_or(OUT_OF_RANGE)?;
    // Ordinal of the day at which week 1 starts.
    let first_week_start = 1 + week_start_day.days_since(first_day_of_year.weekday()) as i32;
    // Number of the `weekday`, which is 0 for the first day of the week.
    let weekday = weekday.days_since(week_start_day) as i32;
    let ordinal = first_week_start + (week as i32 - 1) * 7 + weekday;
    if ordinal <= 0 {
        return Err(IMPOSSIBLE);
    }
    first_day_of_year.with_ordinal(ordinal as u32).ok_or(IMPOSSIBLE)
}

#[cfg(test)]
mod tests {
    use super::super::{IMPOSSIBLE, NOT_ENOUGH, OUT_OF_RANGE};
    use super::Parsed;
    use crate::Datelike;
    use crate::Weekday::*;
    use crate::naive::{NaiveDate, NaiveTime};
    use crate::offset::{FixedOffset, TimeZone, Utc};

    #[test]
    fn test_parsed_set_fields() {
        // year*, isoyear*
        let mut p = Parsed::new();
        assert_eq!(p.set_year(1987), Ok(()));
        assert_eq!(p.set_year(1986), Err(IMPOSSIBLE));
        assert_eq!(p.set_year(1988), Err(IMPOSSIBLE));
        assert_eq!(p.set_year(1987), Ok(()));
        assert_eq!(p.set_year_div_100(20), Ok(())); // independent to `year`
        assert_eq!(p.set_year_div_100(21), Err(IMPOSSIBLE));
        assert_eq!(p.set_year_div_100(19), Err(IMPOSSIBLE));
        assert_eq!(p.set_year_mod_100(37), Ok(())); // ditto
        assert_eq!(p.set_year_mod_100(38), Err(IMPOSSIBLE));
        assert_eq!(p.set_year_mod_100(36), Err(IMPOSSIBLE));

        let mut p = Parsed::new();
        assert_eq!(p.set_year(0), Ok(()));
        assert_eq!(p.set_year_div_100(0), Ok(()));
        assert_eq!(p.set_year_mod_100(0), Ok(()));

        let mut p = Parsed::new();
        assert_eq!(p.set_year_div_100(-1), Err(OUT_OF_RANGE));
        assert_eq!(p.set_year_mod_100(-1), Err(OUT_OF_RANGE));
        assert_eq!(p.set_year(-1), Ok(()));
        assert_eq!(p.set_year(-2), Err(IMPOSSIBLE));
        assert_eq!(p.set_year(0), Err(IMPOSSIBLE));

        let mut p = Parsed::new();
        assert_eq!(p.set_year_div_100(0x1_0000_0008), Err(OUT_OF_RANGE));
        assert_eq!(p.set_year_div_100(8), Ok(()));
        assert_eq!(p.set_year_div_100(0x1_0000_0008), Err(OUT_OF_RANGE));

        // month, week*, isoweek, ordinal, day, minute, second, nanosecond, offset
        let mut p = Parsed::new();
        assert_eq!(p.set_month(7), Ok(()));
        assert_eq!(p.set_month(1), Err(IMPOSSIBLE));
        assert_eq!(p.set_month(6), Err(IMPOSSIBLE));
        assert_eq!(p.set_month(8), Err(IMPOSSIBLE));
        assert_eq!(p.set_month(12), Err(IMPOSSIBLE));

        let mut p = Parsed::new();
        assert_eq!(p.set_month(8), Ok(()));
        assert_eq!(p.set_month(0x1_0000_0008), Err(OUT_OF_RANGE));

        // hour
        let mut p = Parsed::new();
        assert_eq!(p.set_hour(12), Ok(()));
        assert_eq!(p.set_hour(11), Err(IMPOSSIBLE));
        assert_eq!(p.set_hour(13), Err(IMPOSSIBLE));
        assert_eq!(p.set_hour(12), Ok(()));
        assert_eq!(p.set_ampm(false), Err(IMPOSSIBLE));
        assert_eq!(p.set_ampm(true), Ok(()));
        assert_eq!(p.set_hour12(12), Ok(()));
        assert_eq!(p.set_hour12(0), Err(OUT_OF_RANGE)); // requires canonical representation
        assert_eq!(p.set_hour12(1), Err(IMPOSSIBLE));
        assert_eq!(p.set_hour12(11), Err(IMPOSSIBLE));

        let mut p = Parsed::new();
        assert_eq!(p.set_ampm(true), Ok(()));
        assert_eq!(p.set_hour12(7), Ok(()));
        assert_eq!(p.set_hour(7), Err(IMPOSSIBLE));
        assert_eq!(p.set_hour(18), Err(IMPOSSIBLE));
        assert_eq!(p.set_hour(19), Ok(()));

        // timestamp
        let mut p = Parsed::new();
        assert_eq!(p.set_timestamp(1_234_567_890), Ok(()));
        assert_eq!(p.set_timestamp(1_234_567_889), Err(IMPOSSIBLE));
        assert_eq!(p.set_timestamp(1_234_567_891), Err(IMPOSSIBLE));
    }

    #[test]
    fn test_parsed_set_range() {
        assert_eq!(Parsed::new().set_year(i32::MIN as i64 - 1), Err(OUT_OF_RANGE));
        assert!(Parsed::new().set_year(i32::MIN as i64).is_ok());
        assert!(Parsed::new().set_year(i32::MAX as i64).is_ok());
        assert_eq!(Parsed::new().set_year(i32::MAX as i64 + 1), Err(OUT_OF_RANGE));

        assert_eq!(Parsed::new().set_year_div_100(-1), Err(OUT_OF_RANGE));
        assert!(Parsed::new().set_year_div_100(0).is_ok());
        assert!(Parsed::new().set_year_div_100(i32::MAX as i64).is_ok());
        assert_eq!(Parsed::new().set_year_div_100(i32::MAX as i64 + 1), Err(OUT_OF_RANGE));

        assert_eq!(Parsed::new().set_year_mod_100(-1), Err(OUT_OF_RANGE));
        assert!(Parsed::new().set_year_mod_100(0).is_ok());
        assert!(Parsed::new().set_year_mod_100(99).is_ok());
        assert_eq!(Parsed::new().set_year_mod_100(100), Err(OUT_OF_RANGE));

        assert_eq!(Parsed::new().set_isoyear(i32::MIN as i64 - 1), Err(OUT_OF_RANGE));
        assert!(Parsed::new().set_isoyear(i32::MIN as i64).is_ok());
        assert!(Parsed::new().set_isoyear(i32::MAX as i64).is_ok());
        assert_eq!(Parsed::new().set_isoyear(i32::MAX as i64 + 1), Err(OUT_OF_RANGE));

        assert_eq!(Parsed::new().set_isoyear_div_100(-1), Err(OUT_OF_RANGE));
        assert!(Parsed::new().set_isoyear_div_100(0).is_ok());
        assert!(Parsed::new().set_isoyear_div_100(99).is_ok());
        assert_eq!(Parsed::new().set_isoyear_div_100(i32::MAX as i64 + 1), Err(OUT_OF_RANGE));

        assert_eq!(Parsed::new().set_isoyear_mod_100(-1), Err(OUT_OF_RANGE));
        assert!(Parsed::new().set_isoyear_mod_100(0).is_ok());
        assert!(Parsed::new().set_isoyear_mod_100(99).is_ok());
        assert_eq!(Parsed::new().set_isoyear_mod_100(100), Err(OUT_OF_RANGE));

        assert_eq!(Parsed::new().set_quarter(0), Err(OUT_OF_RANGE));
        assert!(Parsed::new().set_quarter(1).is_ok());
        assert!(Parsed::new().set_quarter(4).is_ok());
        assert_eq!(Parsed::new().set_quarter(5), Err(OUT_OF_RANGE));

        assert_eq!(Parsed::new().set_month(0), Err(OUT_OF_RANGE));
        assert!(Parsed::new().set_month(1).is_ok());
        assert!(Parsed::new().set_month(12).is_ok());
        assert_eq!(Parsed::new().set_month(13), Err(OUT_OF_RANGE));

        assert_eq!(Parsed::new().set_week_from_sun(-1), Err(OUT_OF_RANGE));
        assert!(Parsed::new().set_week_from_sun(0).is_ok());
        assert!(Parsed::new().set_week_from_sun(53).is_ok());
        assert_eq!(Parsed::new().set_week_from_sun(54), Err(OUT_OF_RANGE));

        assert_eq!(Parsed::new().set_week_from_mon(-1), Err(OUT_OF_RANGE));
        assert!(Parsed::new().set_week_from_mon(0).is_ok());
        assert!(Parsed::new().set_week_from_mon(53).is_ok());
        assert_eq!(Parsed::new().set_week_from_mon(54), Err(OUT_OF_RANGE));

        assert_eq!(Parsed::new().set_isoweek(0), Err(OUT_OF_RANGE));
        assert!(Parsed::new().set_isoweek(1).is_ok());
        assert!(Parsed::new().set_isoweek(53).is_ok());
        assert_eq!(Parsed::new().set_isoweek(54), Err(OUT_OF_RANGE));

        assert_eq!(Parsed::new().set_ordinal(0), Err(OUT_OF_RANGE));
        assert!(Parsed::new().set_ordinal(1).is_ok());
        assert!(Parsed::new().set_ordinal(366).is_ok());
        assert_eq!(Parsed::new().set_ordinal(367), Err(OUT_OF_RANGE));

        assert_eq!(Parsed::new().set_day(0), Err(OUT_OF_RANGE));
        assert!(Parsed::new().set_day(1).is_ok());
        assert!(Parsed::new().set_day(31).is_ok());
        assert_eq!(Parsed::new().set_day(32), Err(OUT_OF_RANGE));

        assert_eq!(Parsed::new().set_hour12(0), Err(OUT_OF_RANGE));
        assert!(Parsed::new().set_hour12(1).is_ok());
        assert!(Parsed::new().set_hour12(12).is_ok());
        assert_eq!(Parsed::new().set_hour12(13), Err(OUT_OF_RANGE));

        assert_eq!(Parsed::new().set_hour(-1), Err(OUT_OF_RANGE));
        assert!(Parsed::new().set_hour(0).is_ok());
        assert!(Parsed::new().set_hour(23).is_ok());
        assert_eq!(Parsed::new().set_hour(24), Err(OUT_OF_RANGE));

        assert_eq!(Parsed::new().set_minute(-1), Err(OUT_OF_RANGE));
        assert!(Parsed::new().set_minute(0).is_ok());
        assert!(Parsed::new().set_minute(59).is_ok());
        assert_eq!(Parsed::new().set_minute(60), Err(OUT_OF_RANGE));

        assert_eq!(Parsed::new().set_second(-1), Err(OUT_OF_RANGE));
        assert!(Parsed::new().set_second(0).is_ok());
        assert!(Parsed::new().set_second(60).is_ok());
        assert_eq!(Parsed::new().set_second(61), Err(OUT_OF_RANGE));

        assert_eq!(Parsed::new().set_nanosecond(-1), Err(OUT_OF_RANGE));
        assert!(Parsed::new().set_nanosecond(0).is_ok());
        assert!(Parsed::new().set_nanosecond(999_999_999).is_ok());
        assert_eq!(Parsed::new().set_nanosecond(1_000_000_000), Err(OUT_OF_RANGE));

        assert!(Parsed::new().set_timestamp(i64::MIN).is_ok());
        assert!(Parsed::new().set_timestamp(i64::MAX).is_ok());

        assert_eq!(Parsed::new().set_offset(i32::MIN as i64 - 1), Err(OUT_OF_RANGE));
        assert!(Parsed::new().set_offset(i32::MIN as i64).is_ok());
        assert!(Parsed::new().set_offset(i32::MAX as i64).is_ok());
        assert_eq!(Parsed::new().set_offset(i32::MAX as i64 + 1), Err(OUT_OF_RANGE));
    }

    #[test]
    fn test_parsed_to_naive_date() {
        macro_rules! parse {
            ($($k:ident: $v:expr),*) => (
                Parsed { $($k: Some($v),)* ..Parsed::new() }.to_naive_date()
            )
        }

        let ymd = |y, m, d| Ok(NaiveDate::from_ymd_opt(y, m, d).unwrap());

        // ymd: omission of fields
        assert_eq!(parse!(), Err(NOT_ENOUGH));
        assert_eq!(parse!(year: 1984), Err(NOT_ENOUGH));
        assert_eq!(parse!(year: 1984, month: 1), Err(NOT_ENOUGH));
        assert_eq!(parse!(year: 1984, month: 1, day: 2), ymd(1984, 1, 2));
        assert_eq!(parse!(year: 1984, day: 2), Err(NOT_ENOUGH));
        assert_eq!(parse!(year_div_100: 19), Err(NOT_ENOUGH));
        assert_eq!(parse!(year_div_100: 19, year_mod_100: 84), Err(NOT_ENOUGH));
        assert_eq!(parse!(year_div_100: 19, year_mod_100: 84, month: 1), Err(NOT_ENOUGH));
        assert_eq!(parse!(year_div_100: 19, year_mod_100: 84, month: 1, day: 2), ymd(1984, 1, 2));
        assert_eq!(parse!(year_div_100: 19, year_mod_100: 84, day: 2), Err(NOT_ENOUGH));
        assert_eq!(parse!(year_div_100: 19, month: 1, day: 2), Err(NOT_ENOUGH));
        assert_eq!(parse!(year_mod_100: 70, month: 1, day: 2), ymd(1970, 1, 2));
        assert_eq!(parse!(year_mod_100: 69, month: 1, day: 2), ymd(2069, 1, 2));

        // ymd: out-of-range conditions
        assert_eq!(parse!(year_div_100: 19, year_mod_100: 84, month: 2, day: 29), ymd(1984, 2, 29));
        assert_eq!(
            parse!(year_div_100: 19, year_mod_100: 83, month: 2, day: 29),
            Err(OUT_OF_RANGE)
        );
        assert_eq!(
            parse!(year_div_100: 19, year_mod_100: 83, month: 13, day: 1),
            Err(OUT_OF_RANGE)
        );
        assert_eq!(
            parse!(year_div_100: 19, year_mod_100: 83, month: 12, day: 31),
            ymd(1983, 12, 31)
        );
        assert_eq!(
            parse!(year_div_100: 19, year_mod_100: 83, month: 12, day: 32),
            Err(OUT_OF_RANGE)
        );
        assert_eq!(
            parse!(year_div_100: 19, year_mod_100: 83, month: 12, day: 0),
            Err(OUT_OF_RANGE)
        );
        assert_eq!(
            parse!(year_div_100: 19, year_mod_100: 100, month: 1, day: 1),
            Err(OUT_OF_RANGE)
        );
        assert_eq!(parse!(year_div_100: 19, year_mod_100: -1, month: 1, day: 1), Err(OUT_OF_RANGE));
        assert_eq!(parse!(year_div_100: 0, year_mod_100: 0, month: 1, day: 1), ymd(0, 1, 1));
        assert_eq!(parse!(year_div_100: -1, year_mod_100: 42, month: 1, day: 1), Err(IMPOSSIBLE));
        let max_year = NaiveDate::MAX.year();
        assert_eq!(
            parse!(year_div_100: max_year / 100,
                          year_mod_100: max_year % 100, month: 1, day: 1),
            ymd(max_year, 1, 1)
        );
        assert_eq!(
            parse!(year_div_100: (max_year + 1) / 100,
                          year_mod_100: (max_year + 1) % 100, month: 1, day: 1),
            Err(OUT_OF_RANGE)
        );

        // ymd: conflicting inputs
        assert_eq!(parse!(year: 1984, year_div_100: 19, month: 1, day: 1), ymd(1984, 1, 1));
        assert_eq!(parse!(year: 1984, year_div_100: 20, month: 1, day: 1), Err(IMPOSSIBLE));
        assert_eq!(parse!(year: 1984, year_mod_100: 84, month: 1, day: 1), ymd(1984, 1, 1));
        assert_eq!(parse!(year: 1984, year_mod_100: 83, month: 1, day: 1), Err(IMPOSSIBLE));
        assert_eq!(
            parse!(year: 1984, year_div_100: 19, year_mod_100: 84, month: 1, day: 1),
            ymd(1984, 1, 1)
        );
        assert_eq!(
            parse!(year: 1984, year_div_100: 18, year_mod_100: 94, month: 1, day: 1),
            Err(IMPOSSIBLE)
        );
        assert_eq!(
            parse!(year: 1984, year_div_100: 18, year_mod_100: 184, month: 1, day: 1),
            Err(OUT_OF_RANGE)
        );
        assert_eq!(
            parse!(year: -1, year_div_100: 0, year_mod_100: -1, month: 1, day: 1),
            Err(OUT_OF_RANGE)
        );
        assert_eq!(
            parse!(year: -1, year_div_100: -1, year_mod_100: 99, month: 1, day: 1),
            Err(IMPOSSIBLE)
        );
        assert_eq!(parse!(year: -1, year_div_100: 0, month: 1, day: 1), Err(IMPOSSIBLE));
        assert_eq!(parse!(year: -1, year_mod_100: 99, month: 1, day: 1), Err(IMPOSSIBLE));

        // quarters
        assert_eq!(parse!(year: 2000, quarter: 1), Err(NOT_ENOUGH));
        assert_eq!(parse!(year: 2000, quarter: 1, month: 1, day: 1), ymd(2000, 1, 1));
        assert_eq!(parse!(year: 2000, quarter: 2, month: 4, day: 1), ymd(2000, 4, 1));
        assert_eq!(parse!(year: 2000, quarter: 3, month: 7, day: 1), ymd(2000, 7, 1));
        assert_eq!(parse!(year: 2000, quarter: 4, month: 10, day: 1), ymd(2000, 10, 1));

        // quarter: conflicting inputs
        assert_eq!(parse!(year: 2000, quarter: 2, month: 3, day: 31), Err(IMPOSSIBLE));
        assert_eq!(parse!(year: 2000, quarter: 4, month: 3, day: 31), Err(IMPOSSIBLE));

        // weekdates
        assert_eq!(parse!(year: 2000, week_from_mon: 0), Err(NOT_ENOUGH));
        assert_eq!(parse!(year: 2000, week_from_sun: 0), Err(NOT_ENOUGH));
        assert_eq!(parse!(year: 2000, weekday: Sun), Err(NOT_ENOUGH));
        assert_eq!(parse!(year: 2000, week_from_mon: 0, weekday: Fri), Err(IMPOSSIBLE));
        assert_eq!(parse!(year: 2000, week_from_sun: 0, weekday: Fri), Err(IMPOSSIBLE));
        assert_eq!(parse!(year: 2000, week_from_mon: 0, weekday: Sat), ymd(2000, 1, 1));
        assert_eq!(parse!(year: 2000, week_from_sun: 0, weekday: Sat), ymd(2000, 1, 1));
        assert_eq!(parse!(year: 2000, week_from_mon: 0, weekday: Sun), ymd(2000, 1, 2));
        assert_eq!(parse!(year: 2000, week_from_sun: 1, weekday: Sun), ymd(2000, 1, 2));
        assert_eq!(parse!(year: 2000, week_from_mon: 1, weekday: Mon), ymd(2000, 1, 3));
        assert_eq!(parse!(year: 2000, week_from_sun: 1, weekday: Mon), ymd(2000, 1, 3));
        assert_eq!(parse!(year: 2000, week_from_mon: 1, weekday: Sat), ymd(2000, 1, 8));
        assert_eq!(parse!(year: 2000, week_from_sun: 1, weekday: Sat), ymd(2000, 1, 8));
        assert_eq!(parse!(year: 2000, week_from_mon: 1, weekday: Sun), ymd(2000, 1, 9));
        assert_eq!(parse!(year: 2000, week_from_sun: 2, weekday: Sun), ymd(2000, 1, 9));
        assert_eq!(parse!(year: 2000, week_from_mon: 2, weekday: Mon), ymd(2000, 1, 10));
        assert_eq!(parse!(year: 2000, week_from_sun: 52, weekday: Sat), ymd(2000, 12, 30));
        assert_eq!(parse!(year: 2000, week_from_sun: 53, weekday: Sun), ymd(2000, 12, 31));
        assert_eq!(parse!(year: 2000, week_from_sun: 53, weekday: Mon), Err(IMPOSSIBLE));
        assert_eq!(parse!(year: 2000, week_from_sun: 0xffffffff, weekday: Mon), Err(OUT_OF_RANGE));
        assert_eq!(parse!(year: 2006, week_from_sun: 0, weekday: Sat), Err(IMPOSSIBLE));
        assert_eq!(parse!(year: 2006, week_from_sun: 1, weekday: Sun), ymd(2006, 1, 1));

        // weekdates: conflicting inputs
        assert_eq!(
            parse!(year: 2000, week_from_mon: 1, week_from_sun: 1, weekday: Sat),
            ymd(2000, 1, 8)
        );
        assert_eq!(
            parse!(year: 2000, week_from_mon: 1, week_from_sun: 2, weekday: Sun),
            ymd(2000, 1, 9)
        );
        assert_eq!(
            parse!(year: 2000, week_from_mon: 1, week_from_sun: 1, weekday: Sun),
            Err(IMPOSSIBLE)
        );
        assert_eq!(
            parse!(year: 2000, week_from_mon: 2, week_from_sun: 2, weekday: Sun),
            Err(IMPOSSIBLE)
        );

        // ISO weekdates
        assert_eq!(parse!(isoyear: 2004, isoweek: 53), Err(NOT_ENOUGH));
        assert_eq!(parse!(isoyear: 2004, isoweek: 53, weekday: Fri), ymd(2004, 12, 31));
        assert_eq!(parse!(isoyear: 2004, isoweek: 53, weekday: Sat), ymd(2005, 1, 1));
        assert_eq!(parse!(isoyear: 2004, isoweek: 0xffffffff, weekday: Sat), Err(OUT_OF_RANGE));
        assert_eq!(parse!(isoyear: 2005, isoweek: 0, weekday: Thu), Err(OUT_OF_RANGE));
        assert_eq!(parse!(isoyear: 2005, isoweek: 5, weekday: Thu), ymd(2005, 2, 3));
        assert_eq!(parse!(isoyear: 2005, weekday: Thu), Err(NOT_ENOUGH));

        // year and ordinal
        assert_eq!(parse!(ordinal: 123), Err(NOT_ENOUGH));
        assert_eq!(parse!(year: 2000, ordinal: 0), Err(OUT_OF_RANGE));
        assert_eq!(parse!(year: 2000, ordinal: 1), ymd(2000, 1, 1));
        assert_eq!(parse!(year: 2000, ordinal: 60), ymd(2000, 2, 29));
        assert_eq!(parse!(year: 2000, ordinal: 61), ymd(2000, 3, 1));
        assert_eq!(parse!(year: 2000, ordinal: 366), ymd(2000, 12, 31));
        assert_eq!(parse!(year: 2000, ordinal: 367), Err(OUT_OF_RANGE));
        assert_eq!(parse!(year: 2000, ordinal: 0xffffffff), Err(OUT_OF_RANGE));
        assert_eq!(parse!(year: 2100, ordinal: 0), Err(OUT_OF_RANGE));
        assert_eq!(parse!(year: 2100, ordinal: 1), ymd(2100, 1, 1));
        assert_eq!(parse!(year: 2100, ordinal: 59), ymd(2100, 2, 28));
        assert_eq!(parse!(year: 2100, ordinal: 60), ymd(2100, 3, 1));
        assert_eq!(parse!(year: 2100, ordinal: 365), ymd(2100, 12, 31));
        assert_eq!(parse!(year: 2100, ordinal: 366), Err(OUT_OF_RANGE));
        assert_eq!(parse!(year: 2100, ordinal: 0xffffffff), Err(OUT_OF_RANGE));

        // more complex cases
        assert_eq!(
            parse!(year: 2014, month: 12, day: 31, ordinal: 365, isoyear: 2015, isoweek: 1,
                          week_from_sun: 52, week_from_mon: 52, weekday: Wed),
            ymd(2014, 12, 31)
        );
        assert_eq!(
            parse!(year: 2014, month: 12, ordinal: 365, isoyear: 2015, isoweek: 1,
                          week_from_sun: 52, week_from_mon: 52),
            ymd(2014, 12, 31)
        );
        assert_eq!(
            parse!(year: 2014, month: 12, day: 31, ordinal: 365, isoyear: 2014, isoweek: 53,
                          week_from_sun: 52, week_from_mon: 52, weekday: Wed),
            Err(IMPOSSIBLE)
        ); // no ISO week date 2014-W53-3
        assert_eq!(
            parse!(year: 2012, isoyear: 2015, isoweek: 1,
                          week_from_sun: 52, week_from_mon: 52),
            Err(NOT_ENOUGH)
        ); // ambiguous (2014-12-29, 2014-12-30, 2014-12-31)
        assert_eq!(parse!(year_div_100: 20, isoyear_mod_100: 15, ordinal: 366), Err(NOT_ENOUGH));
        // technically unique (2014-12-31) but Chrono gives up
    }

    #[test]
    fn test_parsed_to_naive_time() {
        macro_rules! parse {
            ($($k:ident: $v:expr),*) => (
                Parsed { $($k: Some($v),)* ..Parsed::new() }.to_naive_time()
            )
        }

        let hms = |h, m, s| Ok(NaiveTime::from_hms_opt(h, m, s).unwrap());
        let hmsn = |h, m, s, n| Ok(NaiveTime::from_hms_nano_opt(h, m, s, n).unwrap());

        // omission of fields
        assert_eq!(parse!(), Err(NOT_ENOUGH));
        assert_eq!(parse!(hour_div_12: 0), Err(NOT_ENOUGH));
        assert_eq!(parse!(hour_div_12: 0, hour_mod_12: 1), Err(NOT_ENOUGH));
        assert_eq!(parse!(hour_div_12: 0, hour_mod_12: 1, minute: 23), hms(1, 23, 0));
        assert_eq!(parse!(hour_div_12: 0, hour_mod_12: 1, minute: 23, second: 45), hms(1, 23, 45));
        assert_eq!(
            parse!(hour_div_12: 0, hour_mod_12: 1, minute: 23, second: 45,
                          nanosecond: 678_901_234),
            hmsn(1, 23, 45, 678_901_234)
        );
        assert_eq!(parse!(hour_div_12: 1, hour_mod_12: 11, minute: 45, second: 6), hms(23, 45, 6));
        assert_eq!(parse!(hour_mod_12: 1, minute: 23), Err(NOT_ENOUGH));
        assert_eq!(
            parse!(hour_div_12: 0, hour_mod_12: 1, minute: 23, nanosecond: 456_789_012),
            Err(NOT_ENOUGH)
        );

        // out-of-range conditions
        assert_eq!(parse!(hour_div_12: 2, hour_mod_12: 0, minute: 0), Err(OUT_OF_RANGE));
        assert_eq!(parse!(hour_div_12: 1, hour_mod_12: 12, minute: 0), Err(OUT_OF_RANGE));
        assert_eq!(parse!(hour_div_12: 0, hour_mod_12: 1, minute: 60), Err(OUT_OF_RANGE));
        assert_eq!(
            parse!(hour_div_12: 0, hour_mod_12: 1, minute: 23, second: 61),
            Err(OUT_OF_RANGE)
        );
        assert_eq!(
            parse!(hour_div_12: 0, hour_mod_12: 1, minute: 23, second: 34,
                          nanosecond: 1_000_000_000),
            Err(OUT_OF_RANGE)
        );

        // leap seconds
        assert_eq!(
            parse!(hour_div_12: 0, hour_mod_12: 1, minute: 23, second: 60),
            hmsn(1, 23, 59, 1_000_000_000)
        );
        assert_eq!(
            parse!(hour_div_12: 0, hour_mod_12: 1, minute: 23, second: 60,
                          nanosecond: 999_999_999),
            hmsn(1, 23, 59, 1_999_999_999)
        );
    }

    #[test]
    fn test_parsed_to_naive_datetime_with_offset() {
        macro_rules! parse {
            (offset = $offset:expr; $($k:ident: $v:expr),*) => (
                Parsed { $($k: Some($v),)* ..Parsed::new() }.to_naive_datetime_with_offset($offset)
            );
            ($($k:ident: $v:expr),*) => (parse!(offset = 0; $($k: $v),*))
        }

        let ymdhms = |y, m, d, h, n, s| {
            Ok(NaiveDate::from_ymd_opt(y, m, d).unwrap().and_hms_opt(h, n, s).unwrap())
        };
        let ymdhmsn = |y, m, d, h, n, s, nano| {
            Ok(NaiveDate::from_ymd_opt(y, m, d).unwrap().and_hms_nano_opt(h, n, s, nano).unwrap())
        };

        // omission of fields
        assert_eq!(parse!(), Err(NOT_ENOUGH));
        assert_eq!(
            parse!(year: 2015, month: 1, day: 30,
                          hour_div_12: 1, hour_mod_12: 2, minute: 38),
            ymdhms(2015, 1, 30, 14, 38, 0)
        );
        assert_eq!(
            parse!(year: 1997, month: 1, day: 30,
                          hour_div_12: 1, hour_mod_12: 2, minute: 38, second: 5),
            ymdhms(1997, 1, 30, 14, 38, 5)
        );
        assert_eq!(
            parse!(year: 2012, ordinal: 34, hour_div_12: 0, hour_mod_12: 5,
                          minute: 6, second: 7, nanosecond: 890_123_456),
            ymdhmsn(2012, 2, 3, 5, 6, 7, 890_123_456)
        );
        assert_eq!(parse!(timestamp: 0), ymdhms(1970, 1, 1, 0, 0, 0));
        assert_eq!(parse!(timestamp: 1, nanosecond: 0), ymdhms(1970, 1, 1, 0, 0, 1));
        assert_eq!(parse!(timestamp: 1, nanosecond: 1), ymdhmsn(1970, 1, 1, 0, 0, 1, 1));
        assert_eq!(parse!(timestamp: 1_420_000_000), ymdhms(2014, 12, 31, 4, 26, 40));
        assert_eq!(parse!(timestamp: -0x1_0000_0000), ymdhms(1833, 11, 24, 17, 31, 44));

        // full fields
        assert_eq!(
            parse!(year: 2014, year_div_100: 20, year_mod_100: 14, month: 12, day: 31,
                          ordinal: 365, isoyear: 2015, isoyear_div_100: 20, isoyear_mod_100: 15,
                          isoweek: 1, week_from_sun: 52, week_from_mon: 52, weekday: Wed,
                          hour_div_12: 0, hour_mod_12: 4, minute: 26, second: 40,
                          nanosecond: 12_345_678, timestamp: 1_420_000_000),
            ymdhmsn(2014, 12, 31, 4, 26, 40, 12_345_678)
        );
        assert_eq!(
            parse!(year: 2014, year_div_100: 20, year_mod_100: 14, month: 12, day: 31,
                          ordinal: 365, isoyear: 2015, isoyear_div_100: 20, isoyear_mod_100: 15,
                          isoweek: 1, week_from_sun: 52, week_from_mon: 52, weekday: Wed,
                          hour_div_12: 0, hour_mod_12: 4, minute: 26, second: 40,
                          nanosecond: 12_345_678, timestamp: 1_419_999_999),
            Err(IMPOSSIBLE)
        );
        assert_eq!(
            parse!(offset = 32400;
                          year: 2014, year_div_100: 20, year_mod_100: 14, month: 12, day: 31,
                          ordinal: 365, isoyear: 2015, isoyear_div_100: 20, isoyear_mod_100: 15,
                          isoweek: 1, week_from_sun: 52, week_from_mon: 52, weekday: Wed,
                          hour_div_12: 0, hour_mod_12: 4, minute: 26, second: 40,
                          nanosecond: 12_345_678, timestamp: 1_419_967_600),
            ymdhmsn(2014, 12, 31, 4, 26, 40, 12_345_678)
        );

        // more timestamps
        let max_days_from_year_1970 =
            NaiveDate::MAX.signed_duration_since(NaiveDate::from_ymd_opt(1970, 1, 1).unwrap());
        let year_0_from_year_1970 = NaiveDate::from_ymd_opt(0, 1, 1)
            .unwrap()
            .signed_duration_since(NaiveDate::from_ymd_opt(1970, 1, 1).unwrap());
        let min_days_from_year_1970 =
            NaiveDate::MIN.signed_duration_since(NaiveDate::from_ymd_opt(1970, 1, 1).unwrap());
        assert_eq!(
            parse!(timestamp: min_days_from_year_1970.num_seconds()),
            ymdhms(NaiveDate::MIN.year(), 1, 1, 0, 0, 0)
        );
        assert_eq!(
            parse!(timestamp: year_0_from_year_1970.num_seconds()),
            ymdhms(0, 1, 1, 0, 0, 0)
        );
        assert_eq!(
            parse!(timestamp: max_days_from_year_1970.num_seconds() + 86399),
            ymdhms(NaiveDate::MAX.year(), 12, 31, 23, 59, 59)
        );

        // leap seconds #1: partial fields
        assert_eq!(parse!(second: 59, timestamp: 1_341_100_798), Err(IMPOSSIBLE));
        assert_eq!(parse!(second: 59, timestamp: 1_341_100_799), ymdhms(2012, 6, 30, 23, 59, 59));
        assert_eq!(parse!(second: 59, timestamp: 1_341_100_800), Err(IMPOSSIBLE));
        assert_eq!(
            parse!(second: 60, timestamp: 1_341_100_799),
            ymdhmsn(2012, 6, 30, 23, 59, 59, 1_000_000_000)
        );
        assert_eq!(
            parse!(second: 60, timestamp: 1_341_100_800),
            ymdhmsn(2012, 6, 30, 23, 59, 59, 1_000_000_000)
        );
        assert_eq!(parse!(second: 0, timestamp: 1_341_100_800), ymdhms(2012, 7, 1, 0, 0, 0));
        assert_eq!(parse!(second: 1, timestamp: 1_341_100_800), Err(IMPOSSIBLE));
        assert_eq!(parse!(second: 60, timestamp: 1_341_100_801), Err(IMPOSSIBLE));

        // leap seconds #2: full fields
        // we need to have separate tests for them since it uses another control flow.
        assert_eq!(
            parse!(year: 2012, ordinal: 182, hour_div_12: 1, hour_mod_12: 11,
                          minute: 59, second: 59, timestamp: 1_341_100_798),
            Err(IMPOSSIBLE)
        );
        assert_eq!(
            parse!(year: 2012, ordinal: 182, hour_div_12: 1, hour_mod_12: 11,
                          minute: 59, second: 59, timestamp: 1_341_100_799),
            ymdhms(2012, 6, 30, 23, 59, 59)
        );
        assert_eq!(
            parse!(year: 2012, ordinal: 182, hour_div_12: 1, hour_mod_12: 11,
                          minute: 59, second: 59, timestamp: 1_341_100_800),
            Err(IMPOSSIBLE)
        );
        assert_eq!(
            parse!(year: 2012, ordinal: 182, hour_div_12: 1, hour_mod_12: 11,
                          minute: 59, second: 60, timestamp: 1_341_100_799),
            ymdhmsn(2012, 6, 30, 23, 59, 59, 1_000_000_000)
        );
        assert_eq!(
            parse!(year: 2012, ordinal: 182, hour_div_12: 1, hour_mod_12: 11,
                          minute: 59, second: 60, timestamp: 1_341_100_800),
            ymdhmsn(2012, 6, 30, 23, 59, 59, 1_000_000_000)
        );
        assert_eq!(
            parse!(year: 2012, ordinal: 183, hour_div_12: 0, hour_mod_12: 0,
                          minute: 0, second: 0, timestamp: 1_341_100_800),
            ymdhms(2012, 7, 1, 0, 0, 0)
        );
        assert_eq!(
            parse!(year: 2012, ordinal: 183, hour_div_12: 0, hour_mod_12: 0,
                          minute: 0, second: 1, timestamp: 1_341_100_800),
            Err(IMPOSSIBLE)
        );
        assert_eq!(
            parse!(year: 2012, ordinal: 182, hour_div_12: 1, hour_mod_12: 11,
                          minute: 59, second: 60, timestamp: 1_341_100_801),
            Err(IMPOSSIBLE)
        );

        // error codes
        assert_eq!(
            parse!(year: 2015, month: 1, day: 20, weekday: Tue,
                          hour_div_12: 2, hour_mod_12: 1, minute: 35, second: 20),
            Err(OUT_OF_RANGE)
        ); // `hour_div_12` is out of range
    }

    #[test]
    fn test_parsed_to_datetime() {
        macro_rules! parse {
            ($($k:ident: $v:expr),*) => (
                Parsed { $($k: Some($v),)* ..Parsed::new() }.to_datetime()
            )
        }

        let ymdhmsn = |y, m, d, h, n, s, nano, off| {
            Ok(FixedOffset::east_opt(off)
                .unwrap()
                .from_local_datetime(
                    &NaiveDate::from_ymd_opt(y, m, d)
                        .unwrap()
                        .and_hms_nano_opt(h, n, s, nano)
                        .unwrap(),
                )
                .unwrap())
        };

        assert_eq!(parse!(offset: 0), Err(NOT_ENOUGH));
        assert_eq!(
            parse!(year: 2014, ordinal: 365, hour_div_12: 0, hour_mod_12: 4,
                          minute: 26, second: 40, nanosecond: 12_345_678),
            Err(NOT_ENOUGH)
        );
        assert_eq!(
            parse!(year: 2014, ordinal: 365, hour_div_12: 0, hour_mod_12: 4,
                          minute: 26, second: 40, nanosecond: 12_345_678, offset: 0),
            ymdhmsn(2014, 12, 31, 4, 26, 40, 12_345_678, 0)
        );
        assert_eq!(
            parse!(year: 2014, ordinal: 365, hour_div_12: 1, hour_mod_12: 1,
                          minute: 26, second: 40, nanosecond: 12_345_678, offset: 32400),
            ymdhmsn(2014, 12, 31, 13, 26, 40, 12_345_678, 32400)
        );
        assert_eq!(
            parse!(year: 2014, ordinal: 365, hour_div_12: 0, hour_mod_12: 1,
                          minute: 42, second: 4, nanosecond: 12_345_678, offset: -9876),
            ymdhmsn(2014, 12, 31, 1, 42, 4, 12_345_678, -9876)
        );
        assert_eq!(
            parse!(year: 2015, ordinal: 1, hour_div_12: 0, hour_mod_12: 4,
                          minute: 26, second: 40, nanosecond: 12_345_678, offset: 86_400),
            Err(OUT_OF_RANGE)
        ); // `FixedOffset` does not support such huge offset
    }

    #[test]
    fn test_parsed_to_datetime_with_timezone() {
        macro_rules! parse {
            ($tz:expr; $($k:ident: $v:expr),*) => (
                Parsed { $($k: Some($v),)* ..Parsed::new() }.to_datetime_with_timezone(&$tz)
            )
        }

        // single result from ymdhms
        assert_eq!(
            parse!(Utc;
                          year: 2014, ordinal: 365, hour_div_12: 0, hour_mod_12: 4,
                          minute: 26, second: 40, nanosecond: 12_345_678, offset: 0),
            Ok(Utc
                .from_local_datetime(
                    &NaiveDate::from_ymd_opt(2014, 12, 31)
                        .unwrap()
                        .and_hms_nano_opt(4, 26, 40, 12_345_678)
                        .unwrap()
                )
                .unwrap())
        );
        assert_eq!(
            parse!(Utc;
                          year: 2014, ordinal: 365, hour_div_12: 1, hour_mod_12: 1,
                          minute: 26, second: 40, nanosecond: 12_345_678, offset: 32400),
            Err(IMPOSSIBLE)
        );
        assert_eq!(
            parse!(FixedOffset::east_opt(32400).unwrap();
                          year: 2014, ordinal: 365, hour_div_12: 0, hour_mod_12: 4,
                          minute: 26, second: 40, nanosecond: 12_345_678, offset: 0),
            Err(IMPOSSIBLE)
        );
        assert_eq!(
            parse!(FixedOffset::east_opt(32400).unwrap();
                          year: 2014, ordinal: 365, hour_div_12: 1, hour_mod_12: 1,
                          minute: 26, second: 40, nanosecond: 12_345_678, offset: 32400),
            Ok(FixedOffset::east_opt(32400)
                .unwrap()
                .from_local_datetime(
                    &NaiveDate::from_ymd_opt(2014, 12, 31)
                        .unwrap()
                        .and_hms_nano_opt(13, 26, 40, 12_345_678)
                        .unwrap()
                )
                .unwrap())
        );

        // single result from timestamp
        assert_eq!(
            parse!(Utc; timestamp: 1_420_000_000, offset: 0),
            Ok(Utc.with_ymd_and_hms(2014, 12, 31, 4, 26, 40).unwrap())
        );
        assert_eq!(parse!(Utc; timestamp: 1_420_000_000, offset: 32400), Err(IMPOSSIBLE));
        assert_eq!(
            parse!(FixedOffset::east_opt(32400).unwrap(); timestamp: 1_420_000_000, offset: 0),
            Err(IMPOSSIBLE)
        );
        assert_eq!(
            parse!(FixedOffset::east_opt(32400).unwrap(); timestamp: 1_420_000_000, offset: 32400),
            Ok(FixedOffset::east_opt(32400)
                .unwrap()
                .with_ymd_and_hms(2014, 12, 31, 13, 26, 40)
                .unwrap())
        );

        // TODO test with a variable time zone (for None and Ambiguous cases)
    }

    #[test]
    fn issue_551() {
        use crate::Weekday;
        let mut parsed = Parsed::new();

        parsed.year = Some(2002);
        parsed.week_from_mon = Some(22);
        parsed.weekday = Some(Weekday::Mon);
        assert_eq!(NaiveDate::from_ymd_opt(2002, 6, 3).unwrap(), parsed.to_naive_date().unwrap());

        parsed.year = Some(2001);
        assert_eq!(NaiveDate::from_ymd_opt(2001, 5, 28).unwrap(), parsed.to_naive_date().unwrap());
    }
}
