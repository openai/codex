// This is a part of Chrono.
// See README.md and LICENSE.txt for details.

//! ISO 8601 date and time without timezone.

#[cfg(feature = "alloc")]
use core::borrow::Borrow;
use core::fmt::Write;
use core::ops::{Add, AddAssign, Sub, SubAssign};
use core::time::Duration;
use core::{fmt, str};

#[cfg(any(feature = "rkyv", feature = "rkyv-16", feature = "rkyv-32", feature = "rkyv-64"))]
use rkyv::{Archive, Deserialize, Serialize};

#[cfg(feature = "alloc")]
use crate::format::DelayedFormat;
use crate::format::{Fixed, Item, Numeric, Pad};
use crate::format::{ParseError, ParseResult, Parsed, StrftimeItems, parse, parse_and_remainder};
use crate::naive::{Days, IsoWeek, NaiveDate, NaiveTime};
use crate::offset::Utc;
use crate::time_delta::NANOS_PER_SEC;
use crate::{
    DateTime, Datelike, FixedOffset, MappedLocalTime, Months, TimeDelta, TimeZone, Timelike,
    Weekday, expect, try_opt,
};

/// Tools to help serializing/deserializing `NaiveDateTime`s
#[cfg(feature = "serde")]
pub(crate) mod serde;

#[cfg(test)]
mod tests;

/// The minimum possible `NaiveDateTime`.
#[deprecated(since = "0.4.20", note = "Use NaiveDateTime::MIN instead")]
pub const MIN_DATETIME: NaiveDateTime = NaiveDateTime::MIN;
/// The maximum possible `NaiveDateTime`.
#[deprecated(since = "0.4.20", note = "Use NaiveDateTime::MAX instead")]
pub const MAX_DATETIME: NaiveDateTime = NaiveDateTime::MAX;

/// ISO 8601 combined date and time without timezone.
///
/// # Example
///
/// `NaiveDateTime` is commonly created from [`NaiveDate`].
///
/// ```
/// use chrono::{NaiveDate, NaiveDateTime};
///
/// let dt: NaiveDateTime =
///     NaiveDate::from_ymd_opt(2016, 7, 8).unwrap().and_hms_opt(9, 10, 11).unwrap();
/// # let _ = dt;
/// ```
///
/// You can use typical [date-like](Datelike) and [time-like](Timelike) methods,
/// provided that relevant traits are in the scope.
///
/// ```
/// # use chrono::{NaiveDate, NaiveDateTime};
/// # let dt: NaiveDateTime = NaiveDate::from_ymd_opt(2016, 7, 8).unwrap().and_hms_opt(9, 10, 11).unwrap();
/// use chrono::{Datelike, Timelike, Weekday};
///
/// assert_eq!(dt.weekday(), Weekday::Fri);
/// assert_eq!(dt.num_seconds_from_midnight(), 33011);
/// ```
#[derive(PartialEq, Eq, Hash, PartialOrd, Ord, Copy, Clone)]
#[cfg_attr(
    any(feature = "rkyv", feature = "rkyv-16", feature = "rkyv-32", feature = "rkyv-64"),
    derive(Archive, Deserialize, Serialize),
    archive(compare(PartialEq, PartialOrd)),
    archive_attr(derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash))
)]
#[cfg_attr(feature = "rkyv-validation", archive(check_bytes))]
#[cfg_attr(all(feature = "arbitrary", feature = "std"), derive(arbitrary::Arbitrary))]
pub struct NaiveDateTime {
    date: NaiveDate,
    time: NaiveTime,
}

impl NaiveDateTime {
    /// Makes a new `NaiveDateTime` from date and time components.
    /// Equivalent to [`date.and_time(time)`](./struct.NaiveDate.html#method.and_time)
    /// and many other helper constructors on `NaiveDate`.
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
    ///
    /// let d = NaiveDate::from_ymd_opt(2015, 6, 3).unwrap();
    /// let t = NaiveTime::from_hms_milli_opt(12, 34, 56, 789).unwrap();
    ///
    /// let dt = NaiveDateTime::new(d, t);
    /// assert_eq!(dt.date(), d);
    /// assert_eq!(dt.time(), t);
    /// ```
    #[inline]
    pub const fn new(date: NaiveDate, time: NaiveTime) -> NaiveDateTime {
        NaiveDateTime { date, time }
    }

    /// Makes a new `NaiveDateTime` corresponding to a UTC date and time,
    /// from the number of non-leap seconds
    /// since the midnight UTC on January 1, 1970 (aka "UNIX timestamp")
    /// and the number of nanoseconds since the last whole non-leap second.
    ///
    /// For a non-naive version of this function see [`TimeZone::timestamp`].
    ///
    /// The nanosecond part can exceed 1,000,000,000 in order to represent a
    /// [leap second](NaiveTime#leap-second-handling), but only when `secs % 60 == 59`.
    /// (The true "UNIX timestamp" cannot represent a leap second unambiguously.)
    ///
    /// # Panics
    ///
    /// Panics if the number of seconds would be out of range for a `NaiveDateTime` (more than
    /// ca. 262,000 years away from common era), and panics on an invalid nanosecond (2 seconds or
    /// more).
    #[deprecated(since = "0.4.23", note = "use `DateTime::from_timestamp` instead")]
    #[inline]
    #[must_use]
    pub const fn from_timestamp(secs: i64, nsecs: u32) -> NaiveDateTime {
        let datetime =
            expect(DateTime::from_timestamp(secs, nsecs), "invalid or out-of-range datetime");
        datetime.naive_utc()
    }

    /// Creates a new [NaiveDateTime] from milliseconds since the UNIX epoch.
    ///
    /// The UNIX epoch starts on midnight, January 1, 1970, UTC.
    ///
    /// # Errors
    ///
    /// Returns `None` if the number of milliseconds would be out of range for a `NaiveDateTime`
    /// (more than ca. 262,000 years away from common era)
    #[deprecated(since = "0.4.35", note = "use `DateTime::from_timestamp_millis` instead")]
    #[inline]
    #[must_use]
    pub const fn from_timestamp_millis(millis: i64) -> Option<NaiveDateTime> {
        Some(try_opt!(DateTime::from_timestamp_millis(millis)).naive_utc())
    }

    /// Creates a new [NaiveDateTime] from microseconds since the UNIX epoch.
    ///
    /// The UNIX epoch starts on midnight, January 1, 1970, UTC.
    ///
    /// # Errors
    ///
    /// Returns `None` if the number of microseconds would be out of range for a `NaiveDateTime`
    /// (more than ca. 262,000 years away from common era)
    #[deprecated(since = "0.4.35", note = "use `DateTime::from_timestamp_micros` instead")]
    #[inline]
    #[must_use]
    pub const fn from_timestamp_micros(micros: i64) -> Option<NaiveDateTime> {
        let secs = micros.div_euclid(1_000_000);
        let nsecs = micros.rem_euclid(1_000_000) as u32 * 1000;
        Some(try_opt!(DateTime::<Utc>::from_timestamp(secs, nsecs)).naive_utc())
    }

    /// Creates a new [NaiveDateTime] from nanoseconds since the UNIX epoch.
    ///
    /// The UNIX epoch starts on midnight, January 1, 1970, UTC.
    ///
    /// # Errors
    ///
    /// Returns `None` if the number of nanoseconds would be out of range for a `NaiveDateTime`
    /// (more than ca. 262,000 years away from common era)
    #[deprecated(since = "0.4.35", note = "use `DateTime::from_timestamp_nanos` instead")]
    #[inline]
    #[must_use]
    pub const fn from_timestamp_nanos(nanos: i64) -> Option<NaiveDateTime> {
        let secs = nanos.div_euclid(NANOS_PER_SEC as i64);
        let nsecs = nanos.rem_euclid(NANOS_PER_SEC as i64) as u32;
        Some(try_opt!(DateTime::from_timestamp(secs, nsecs)).naive_utc())
    }

    /// Makes a new `NaiveDateTime` corresponding to a UTC date and time,
    /// from the number of non-leap seconds
    /// since the midnight UTC on January 1, 1970 (aka "UNIX timestamp")
    /// and the number of nanoseconds since the last whole non-leap second.
    ///
    /// The nanosecond part can exceed 1,000,000,000 in order to represent a
    /// [leap second](NaiveTime#leap-second-handling), but only when `secs % 60 == 59`.
    /// (The true "UNIX timestamp" cannot represent a leap second unambiguously.)
    ///
    /// # Errors
    ///
    /// Returns `None` if the number of seconds would be out of range for a `NaiveDateTime` (more
    /// than ca. 262,000 years away from common era), and panics on an invalid nanosecond
    /// (2 seconds or more).
    #[deprecated(since = "0.4.35", note = "use `DateTime::from_timestamp` instead")]
    #[inline]
    #[must_use]
    pub const fn from_timestamp_opt(secs: i64, nsecs: u32) -> Option<NaiveDateTime> {
        Some(try_opt!(DateTime::from_timestamp(secs, nsecs)).naive_utc())
    }

    /// Parses a string with the specified format string and returns a new `NaiveDateTime`.
    /// See the [`format::strftime` module](crate::format::strftime)
    /// on the supported escape sequences.
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::{NaiveDate, NaiveDateTime};
    ///
    /// let parse_from_str = NaiveDateTime::parse_from_str;
    ///
    /// assert_eq!(
    ///     parse_from_str("2015-09-05 23:56:04", "%Y-%m-%d %H:%M:%S"),
    ///     Ok(NaiveDate::from_ymd_opt(2015, 9, 5).unwrap().and_hms_opt(23, 56, 4).unwrap())
    /// );
    /// assert_eq!(
    ///     parse_from_str("5sep2015pm012345.6789", "%d%b%Y%p%I%M%S%.f"),
    ///     Ok(NaiveDate::from_ymd_opt(2015, 9, 5)
    ///         .unwrap()
    ///         .and_hms_micro_opt(13, 23, 45, 678_900)
    ///         .unwrap())
    /// );
    /// ```
    ///
    /// Offset is ignored for the purpose of parsing.
    ///
    /// ```
    /// # use chrono::{NaiveDateTime, NaiveDate};
    /// # let parse_from_str = NaiveDateTime::parse_from_str;
    /// assert_eq!(
    ///     parse_from_str("2014-5-17T12:34:56+09:30", "%Y-%m-%dT%H:%M:%S%z"),
    ///     Ok(NaiveDate::from_ymd_opt(2014, 5, 17).unwrap().and_hms_opt(12, 34, 56).unwrap())
    /// );
    /// ```
    ///
    /// [Leap seconds](./struct.NaiveTime.html#leap-second-handling) are correctly handled by
    /// treating any time of the form `hh:mm:60` as a leap second.
    /// (This equally applies to the formatting, so the round trip is possible.)
    ///
    /// ```
    /// # use chrono::{NaiveDateTime, NaiveDate};
    /// # let parse_from_str = NaiveDateTime::parse_from_str;
    /// assert_eq!(
    ///     parse_from_str("2015-07-01 08:59:60.123", "%Y-%m-%d %H:%M:%S%.f"),
    ///     Ok(NaiveDate::from_ymd_opt(2015, 7, 1)
    ///         .unwrap()
    ///         .and_hms_milli_opt(8, 59, 59, 1_123)
    ///         .unwrap())
    /// );
    /// ```
    ///
    /// Missing seconds are assumed to be zero,
    /// but out-of-bound times or insufficient fields are errors otherwise.
    ///
    /// ```
    /// # use chrono::{NaiveDateTime, NaiveDate};
    /// # let parse_from_str = NaiveDateTime::parse_from_str;
    /// assert_eq!(
    ///     parse_from_str("94/9/4 7:15", "%y/%m/%d %H:%M"),
    ///     Ok(NaiveDate::from_ymd_opt(1994, 9, 4).unwrap().and_hms_opt(7, 15, 0).unwrap())
    /// );
    ///
    /// assert!(parse_from_str("04m33s", "%Mm%Ss").is_err());
    /// assert!(parse_from_str("94/9/4 12", "%y/%m/%d %H").is_err());
    /// assert!(parse_from_str("94/9/4 17:60", "%y/%m/%d %H:%M").is_err());
    /// assert!(parse_from_str("94/9/4 24:00:00", "%y/%m/%d %H:%M:%S").is_err());
    /// ```
    ///
    /// All parsed fields should be consistent to each other, otherwise it's an error.
    ///
    /// ```
    /// # use chrono::NaiveDateTime;
    /// # let parse_from_str = NaiveDateTime::parse_from_str;
    /// let fmt = "%Y-%m-%d %H:%M:%S = UNIX timestamp %s";
    /// assert!(parse_from_str("2001-09-09 01:46:39 = UNIX timestamp 999999999", fmt).is_ok());
    /// assert!(parse_from_str("1970-01-01 00:00:00 = UNIX timestamp 1", fmt).is_err());
    /// ```
    ///
    /// Years before 1 BCE or after 9999 CE, require an initial sign
    ///
    ///```
    /// # use chrono::NaiveDateTime;
    /// # let parse_from_str = NaiveDateTime::parse_from_str;
    /// let fmt = "%Y-%m-%d %H:%M:%S";
    /// assert!(parse_from_str("10000-09-09 01:46:39", fmt).is_err());
    /// assert!(parse_from_str("+10000-09-09 01:46:39", fmt).is_ok());
    /// ```
    pub fn parse_from_str(s: &str, fmt: &str) -> ParseResult<NaiveDateTime> {
        let mut parsed = Parsed::new();
        parse(&mut parsed, s, StrftimeItems::new(fmt))?;
        parsed.to_naive_datetime_with_offset(0) // no offset adjustment
    }

    /// Parses a string with the specified format string and returns a new `NaiveDateTime`, and a
    /// slice with the remaining portion of the string.
    /// See the [`format::strftime` module](crate::format::strftime)
    /// on the supported escape sequences.
    ///
    /// Similar to [`parse_from_str`](#method.parse_from_str).
    ///
    /// # Example
    ///
    /// ```rust
    /// # use chrono::{NaiveDate, NaiveDateTime};
    /// let (datetime, remainder) = NaiveDateTime::parse_and_remainder(
    ///     "2015-02-18 23:16:09 trailing text",
    ///     "%Y-%m-%d %H:%M:%S",
    /// )
    /// .unwrap();
    /// assert_eq!(
    ///     datetime,
    ///     NaiveDate::from_ymd_opt(2015, 2, 18).unwrap().and_hms_opt(23, 16, 9).unwrap()
    /// );
    /// assert_eq!(remainder, " trailing text");
    /// ```
    pub fn parse_and_remainder<'a>(s: &'a str, fmt: &str) -> ParseResult<(NaiveDateTime, &'a str)> {
        let mut parsed = Parsed::new();
        let remainder = parse_and_remainder(&mut parsed, s, StrftimeItems::new(fmt))?;
        parsed.to_naive_datetime_with_offset(0).map(|d| (d, remainder)) // no offset adjustment
    }

    /// Retrieves a date component.
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::NaiveDate;
    ///
    /// let dt = NaiveDate::from_ymd_opt(2016, 7, 8).unwrap().and_hms_opt(9, 10, 11).unwrap();
    /// assert_eq!(dt.date(), NaiveDate::from_ymd_opt(2016, 7, 8).unwrap());
    /// ```
    #[inline]
    pub const fn date(&self) -> NaiveDate {
        self.date
    }

    /// Retrieves a time component.
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::{NaiveDate, NaiveTime};
    ///
    /// let dt = NaiveDate::from_ymd_opt(2016, 7, 8).unwrap().and_hms_opt(9, 10, 11).unwrap();
    /// assert_eq!(dt.time(), NaiveTime::from_hms_opt(9, 10, 11).unwrap());
    /// ```
    #[inline]
    pub const fn time(&self) -> NaiveTime {
        self.time
    }

    /// Returns the number of non-leap seconds since the midnight on January 1, 1970.
    ///
    /// Note that this does *not* account for the timezone!
    /// The true "UNIX timestamp" would count seconds since the midnight *UTC* on the epoch.
    #[deprecated(since = "0.4.35", note = "use `.and_utc().timestamp()` instead")]
    #[inline]
    #[must_use]
    pub const fn timestamp(&self) -> i64 {
        self.and_utc().timestamp()
    }

    /// Returns the number of non-leap *milliseconds* since midnight on January 1, 1970.
    ///
    /// Note that this does *not* account for the timezone!
    /// The true "UNIX timestamp" would count seconds since the midnight *UTC* on the epoch.
    #[deprecated(since = "0.4.35", note = "use `.and_utc().timestamp_millis()` instead")]
    #[inline]
    #[must_use]
    pub const fn timestamp_millis(&self) -> i64 {
        self.and_utc().timestamp_millis()
    }

    /// Returns the number of non-leap *microseconds* since midnight on January 1, 1970.
    ///
    /// Note that this does *not* account for the timezone!
    /// The true "UNIX timestamp" would count seconds since the midnight *UTC* on the epoch.
    #[deprecated(since = "0.4.35", note = "use `.and_utc().timestamp_micros()` instead")]
    #[inline]
    #[must_use]
    pub const fn timestamp_micros(&self) -> i64 {
        self.and_utc().timestamp_micros()
    }

    /// Returns the number of non-leap *nanoseconds* since midnight on January 1, 1970.
    ///
    /// Note that this does *not* account for the timezone!
    /// The true "UNIX timestamp" would count seconds since the midnight *UTC* on the epoch.
    ///
    /// # Panics
    ///
    /// An `i64` with nanosecond precision can span a range of ~584 years. This function panics on
    /// an out of range `NaiveDateTime`.
    ///
    /// The dates that can be represented as nanoseconds are between 1677-09-21T00:12:43.145224192
    /// and 2262-04-11T23:47:16.854775807.
    #[deprecated(since = "0.4.31", note = "use `.and_utc().timestamp_nanos_opt()` instead")]
    #[inline]
    #[must_use]
    #[allow(deprecated)]
    pub const fn timestamp_nanos(&self) -> i64 {
        self.and_utc().timestamp_nanos()
    }

    /// Returns the number of non-leap *nanoseconds* since midnight on January 1, 1970.
    ///
    /// Note that this does *not* account for the timezone!
    /// The true "UNIX timestamp" would count seconds since the midnight *UTC* on the epoch.
    ///
    /// # Errors
    ///
    /// An `i64` with nanosecond precision can span a range of ~584 years. This function returns
    /// `None` on an out of range `NaiveDateTime`.
    ///
    /// The dates that can be represented as nanoseconds are between 1677-09-21T00:12:43.145224192
    /// and 2262-04-11T23:47:16.854775807.
    #[deprecated(since = "0.4.35", note = "use `.and_utc().timestamp_nanos_opt()` instead")]
    #[inline]
    #[must_use]
    pub const fn timestamp_nanos_opt(&self) -> Option<i64> {
        self.and_utc().timestamp_nanos_opt()
    }

    /// Returns the number of milliseconds since the last whole non-leap second.
    ///
    /// The return value ranges from 0 to 999,
    /// or for [leap seconds](./struct.NaiveTime.html#leap-second-handling), to 1,999.
    #[deprecated(since = "0.4.35", note = "use `.and_utc().timestamp_subsec_millis()` instead")]
    #[inline]
    #[must_use]
    pub const fn timestamp_subsec_millis(&self) -> u32 {
        self.and_utc().timestamp_subsec_millis()
    }

    /// Returns the number of microseconds since the last whole non-leap second.
    ///
    /// The return value ranges from 0 to 999,999,
    /// or for [leap seconds](./struct.NaiveTime.html#leap-second-handling), to 1,999,999.
    #[deprecated(since = "0.4.35", note = "use `.and_utc().timestamp_subsec_micros()` instead")]
    #[inline]
    #[must_use]
    pub const fn timestamp_subsec_micros(&self) -> u32 {
        self.and_utc().timestamp_subsec_micros()
    }

    /// Returns the number of nanoseconds since the last whole non-leap second.
    ///
    /// The return value ranges from 0 to 999,999,999,
    /// or for [leap seconds](./struct.NaiveTime.html#leap-second-handling), to 1,999,999,999.
    #[deprecated(since = "0.4.36", note = "use `.and_utc().timestamp_subsec_nanos()` instead")]
    pub const fn timestamp_subsec_nanos(&self) -> u32 {
        self.and_utc().timestamp_subsec_nanos()
    }

    /// Adds given `TimeDelta` to the current date and time.
    ///
    /// As a part of Chrono's [leap second handling](./struct.NaiveTime.html#leap-second-handling),
    /// the addition assumes that **there is no leap second ever**,
    /// except when the `NaiveDateTime` itself represents a leap second
    /// in which case the assumption becomes that **there is exactly a single leap second ever**.
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
    /// let from_ymd = |y, m, d| NaiveDate::from_ymd_opt(y, m, d).unwrap();
    ///
    /// let d = from_ymd(2016, 7, 8);
    /// let hms = |h, m, s| d.and_hms_opt(h, m, s).unwrap();
    /// assert_eq!(hms(3, 5, 7).checked_add_signed(TimeDelta::zero()), Some(hms(3, 5, 7)));
    /// assert_eq!(
    ///     hms(3, 5, 7).checked_add_signed(TimeDelta::try_seconds(1).unwrap()),
    ///     Some(hms(3, 5, 8))
    /// );
    /// assert_eq!(
    ///     hms(3, 5, 7).checked_add_signed(TimeDelta::try_seconds(-1).unwrap()),
    ///     Some(hms(3, 5, 6))
    /// );
    /// assert_eq!(
    ///     hms(3, 5, 7).checked_add_signed(TimeDelta::try_seconds(3600 + 60).unwrap()),
    ///     Some(hms(4, 6, 7))
    /// );
    /// assert_eq!(
    ///     hms(3, 5, 7).checked_add_signed(TimeDelta::try_seconds(86_400).unwrap()),
    ///     Some(from_ymd(2016, 7, 9).and_hms_opt(3, 5, 7).unwrap())
    /// );
    ///
    /// let hmsm = |h, m, s, milli| d.and_hms_milli_opt(h, m, s, milli).unwrap();
    /// assert_eq!(
    ///     hmsm(3, 5, 7, 980).checked_add_signed(TimeDelta::try_milliseconds(450).unwrap()),
    ///     Some(hmsm(3, 5, 8, 430))
    /// );
    /// ```
    ///
    /// Overflow returns `None`.
    ///
    /// ```
    /// # use chrono::{TimeDelta, NaiveDate};
    /// # let hms = |h, m, s| NaiveDate::from_ymd_opt(2016, 7, 8).unwrap().and_hms_opt(h, m, s).unwrap();
    /// assert_eq!(hms(3, 5, 7).checked_add_signed(TimeDelta::try_days(1_000_000_000).unwrap()), None);
    /// ```
    ///
    /// Leap seconds are handled,
    /// but the addition assumes that it is the only leap second happened.
    ///
    /// ```
    /// # use chrono::{TimeDelta, NaiveDate};
    /// # let from_ymd = |y, m, d| NaiveDate::from_ymd_opt(y, m, d).unwrap();
    /// # let hmsm = |h, m, s, milli| from_ymd(2016, 7, 8).and_hms_milli_opt(h, m, s, milli).unwrap();
    /// let leap = hmsm(3, 5, 59, 1_300);
    /// assert_eq!(leap.checked_add_signed(TimeDelta::zero()),
    ///            Some(hmsm(3, 5, 59, 1_300)));
    /// assert_eq!(leap.checked_add_signed(TimeDelta::try_milliseconds(-500).unwrap()),
    ///            Some(hmsm(3, 5, 59, 800)));
    /// assert_eq!(leap.checked_add_signed(TimeDelta::try_milliseconds(500).unwrap()),
    ///            Some(hmsm(3, 5, 59, 1_800)));
    /// assert_eq!(leap.checked_add_signed(TimeDelta::try_milliseconds(800).unwrap()),
    ///            Some(hmsm(3, 6, 0, 100)));
    /// assert_eq!(leap.checked_add_signed(TimeDelta::try_seconds(10).unwrap()),
    ///            Some(hmsm(3, 6, 9, 300)));
    /// assert_eq!(leap.checked_add_signed(TimeDelta::try_seconds(-10).unwrap()),
    ///            Some(hmsm(3, 5, 50, 300)));
    /// assert_eq!(leap.checked_add_signed(TimeDelta::try_days(1).unwrap()),
    ///            Some(from_ymd(2016, 7, 9).and_hms_milli_opt(3, 5, 59, 300).unwrap()));
    /// ```
    #[must_use]
    pub const fn checked_add_signed(self, rhs: TimeDelta) -> Option<NaiveDateTime> {
        let (time, remainder) = self.time.overflowing_add_signed(rhs);
        let remainder = try_opt!(TimeDelta::try_seconds(remainder));
        let date = try_opt!(self.date.checked_add_signed(remainder));
        Some(NaiveDateTime { date, time })
    }

    /// Adds given `Months` to the current date and time.
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
    /// use chrono::{Months, NaiveDate};
    ///
    /// assert_eq!(
    ///     NaiveDate::from_ymd_opt(2014, 1, 1)
    ///         .unwrap()
    ///         .and_hms_opt(1, 0, 0)
    ///         .unwrap()
    ///         .checked_add_months(Months::new(1)),
    ///     Some(NaiveDate::from_ymd_opt(2014, 2, 1).unwrap().and_hms_opt(1, 0, 0).unwrap())
    /// );
    ///
    /// assert_eq!(
    ///     NaiveDate::from_ymd_opt(2014, 1, 1)
    ///         .unwrap()
    ///         .and_hms_opt(1, 0, 0)
    ///         .unwrap()
    ///         .checked_add_months(Months::new(core::i32::MAX as u32 + 1)),
    ///     None
    /// );
    /// ```
    #[must_use]
    pub const fn checked_add_months(self, rhs: Months) -> Option<NaiveDateTime> {
        Some(Self { date: try_opt!(self.date.checked_add_months(rhs)), time: self.time })
    }

    /// Adds given `FixedOffset` to the current datetime.
    /// Returns `None` if the result would be outside the valid range for [`NaiveDateTime`].
    ///
    /// This method is similar to [`checked_add_signed`](#method.checked_add_offset), but preserves
    /// leap seconds.
    #[must_use]
    pub const fn checked_add_offset(self, rhs: FixedOffset) -> Option<NaiveDateTime> {
        let (time, days) = self.time.overflowing_add_offset(rhs);
        let date = match days {
            -1 => try_opt!(self.date.pred_opt()),
            1 => try_opt!(self.date.succ_opt()),
            _ => self.date,
        };
        Some(NaiveDateTime { date, time })
    }

    /// Subtracts given `FixedOffset` from the current datetime.
    /// Returns `None` if the result would be outside the valid range for [`NaiveDateTime`].
    ///
    /// This method is similar to [`checked_sub_signed`](#method.checked_sub_signed), but preserves
    /// leap seconds.
    pub const fn checked_sub_offset(self, rhs: FixedOffset) -> Option<NaiveDateTime> {
        let (time, days) = self.time.overflowing_sub_offset(rhs);
        let date = match days {
            -1 => try_opt!(self.date.pred_opt()),
            1 => try_opt!(self.date.succ_opt()),
            _ => self.date,
        };
        Some(NaiveDateTime { date, time })
    }

    /// Adds given `FixedOffset` to the current datetime.
    /// The resulting value may be outside the valid range of [`NaiveDateTime`].
    ///
    /// This can be useful for intermediate values, but the resulting out-of-range `NaiveDate`
    /// should not be exposed to library users.
    #[must_use]
    pub(crate) fn overflowing_add_offset(self, rhs: FixedOffset) -> NaiveDateTime {
        let (time, days) = self.time.overflowing_add_offset(rhs);
        let date = match days {
            -1 => self.date.pred_opt().unwrap_or(NaiveDate::BEFORE_MIN),
            1 => self.date.succ_opt().unwrap_or(NaiveDate::AFTER_MAX),
            _ => self.date,
        };
        NaiveDateTime { date, time }
    }

    /// Subtracts given `FixedOffset` from the current datetime.
    /// The resulting value may be outside the valid range of [`NaiveDateTime`].
    ///
    /// This can be useful for intermediate values, but the resulting out-of-range `NaiveDate`
    /// should not be exposed to library users.
    #[must_use]
    #[allow(unused)] // currently only used in `Local` but not on all platforms
    pub(crate) fn overflowing_sub_offset(self, rhs: FixedOffset) -> NaiveDateTime {
        let (time, days) = self.time.overflowing_sub_offset(rhs);
        let date = match days {
            -1 => self.date.pred_opt().unwrap_or(NaiveDate::BEFORE_MIN),
            1 => self.date.succ_opt().unwrap_or(NaiveDate::AFTER_MAX),
            _ => self.date,
        };
        NaiveDateTime { date, time }
    }

    /// Subtracts given `TimeDelta` from the current date and time.
    ///
    /// As a part of Chrono's [leap second handling](./struct.NaiveTime.html#leap-second-handling),
    /// the subtraction assumes that **there is no leap second ever**,
    /// except when the `NaiveDateTime` itself represents a leap second
    /// in which case the assumption becomes that **there is exactly a single leap second ever**.
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
    /// let from_ymd = |y, m, d| NaiveDate::from_ymd_opt(y, m, d).unwrap();
    ///
    /// let d = from_ymd(2016, 7, 8);
    /// let hms = |h, m, s| d.and_hms_opt(h, m, s).unwrap();
    /// assert_eq!(hms(3, 5, 7).checked_sub_signed(TimeDelta::zero()), Some(hms(3, 5, 7)));
    /// assert_eq!(
    ///     hms(3, 5, 7).checked_sub_signed(TimeDelta::try_seconds(1).unwrap()),
    ///     Some(hms(3, 5, 6))
    /// );
    /// assert_eq!(
    ///     hms(3, 5, 7).checked_sub_signed(TimeDelta::try_seconds(-1).unwrap()),
    ///     Some(hms(3, 5, 8))
    /// );
    /// assert_eq!(
    ///     hms(3, 5, 7).checked_sub_signed(TimeDelta::try_seconds(3600 + 60).unwrap()),
    ///     Some(hms(2, 4, 7))
    /// );
    /// assert_eq!(
    ///     hms(3, 5, 7).checked_sub_signed(TimeDelta::try_seconds(86_400).unwrap()),
    ///     Some(from_ymd(2016, 7, 7).and_hms_opt(3, 5, 7).unwrap())
    /// );
    ///
    /// let hmsm = |h, m, s, milli| d.and_hms_milli_opt(h, m, s, milli).unwrap();
    /// assert_eq!(
    ///     hmsm(3, 5, 7, 450).checked_sub_signed(TimeDelta::try_milliseconds(670).unwrap()),
    ///     Some(hmsm(3, 5, 6, 780))
    /// );
    /// ```
    ///
    /// Overflow returns `None`.
    ///
    /// ```
    /// # use chrono::{TimeDelta, NaiveDate};
    /// # let hms = |h, m, s| NaiveDate::from_ymd_opt(2016, 7, 8).unwrap().and_hms_opt(h, m, s).unwrap();
    /// assert_eq!(hms(3, 5, 7).checked_sub_signed(TimeDelta::try_days(1_000_000_000).unwrap()), None);
    /// ```
    ///
    /// Leap seconds are handled,
    /// but the subtraction assumes that it is the only leap second happened.
    ///
    /// ```
    /// # use chrono::{TimeDelta, NaiveDate};
    /// # let from_ymd = |y, m, d| NaiveDate::from_ymd_opt(y, m, d).unwrap();
    /// # let hmsm = |h, m, s, milli| from_ymd(2016, 7, 8).and_hms_milli_opt(h, m, s, milli).unwrap();
    /// let leap = hmsm(3, 5, 59, 1_300);
    /// assert_eq!(leap.checked_sub_signed(TimeDelta::zero()),
    ///            Some(hmsm(3, 5, 59, 1_300)));
    /// assert_eq!(leap.checked_sub_signed(TimeDelta::try_milliseconds(200).unwrap()),
    ///            Some(hmsm(3, 5, 59, 1_100)));
    /// assert_eq!(leap.checked_sub_signed(TimeDelta::try_milliseconds(500).unwrap()),
    ///            Some(hmsm(3, 5, 59, 800)));
    /// assert_eq!(leap.checked_sub_signed(TimeDelta::try_seconds(60).unwrap()),
    ///            Some(hmsm(3, 5, 0, 300)));
    /// assert_eq!(leap.checked_sub_signed(TimeDelta::try_days(1).unwrap()),
    ///            Some(from_ymd(2016, 7, 7).and_hms_milli_opt(3, 6, 0, 300).unwrap()));
    /// ```
    #[must_use]
    pub const fn checked_sub_signed(self, rhs: TimeDelta) -> Option<NaiveDateTime> {
        let (time, remainder) = self.time.overflowing_sub_signed(rhs);
        let remainder = try_opt!(TimeDelta::try_seconds(remainder));
        let date = try_opt!(self.date.checked_sub_signed(remainder));
        Some(NaiveDateTime { date, time })
    }

    /// Subtracts given `Months` from the current date and time.
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
    /// use chrono::{Months, NaiveDate};
    ///
    /// assert_eq!(
    ///     NaiveDate::from_ymd_opt(2014, 1, 1)
    ///         .unwrap()
    ///         .and_hms_opt(1, 0, 0)
    ///         .unwrap()
    ///         .checked_sub_months(Months::new(1)),
    ///     Some(NaiveDate::from_ymd_opt(2013, 12, 1).unwrap().and_hms_opt(1, 0, 0).unwrap())
    /// );
    ///
    /// assert_eq!(
    ///     NaiveDate::from_ymd_opt(2014, 1, 1)
    ///         .unwrap()
    ///         .and_hms_opt(1, 0, 0)
    ///         .unwrap()
    ///         .checked_sub_months(Months::new(core::i32::MAX as u32 + 1)),
    ///     None
    /// );
    /// ```
    #[must_use]
    pub const fn checked_sub_months(self, rhs: Months) -> Option<NaiveDateTime> {
        Some(Self { date: try_opt!(self.date.checked_sub_months(rhs)), time: self.time })
    }

    /// Add a duration in [`Days`] to the date part of the `NaiveDateTime`
    ///
    /// Returns `None` if the resulting date would be out of range.
    #[must_use]
    pub const fn checked_add_days(self, days: Days) -> Option<Self> {
        Some(Self { date: try_opt!(self.date.checked_add_days(days)), ..self })
    }

    /// Subtract a duration in [`Days`] from the date part of the `NaiveDateTime`
    ///
    /// Returns `None` if the resulting date would be out of range.
    #[must_use]
    pub const fn checked_sub_days(self, days: Days) -> Option<Self> {
        Some(Self { date: try_opt!(self.date.checked_sub_days(days)), ..self })
    }

    /// Subtracts another `NaiveDateTime` from the current date and time.
    /// This does not overflow or underflow at all.
    ///
    /// As a part of Chrono's [leap second handling](./struct.NaiveTime.html#leap-second-handling),
    /// the subtraction assumes that **there is no leap second ever**,
    /// except when any of the `NaiveDateTime`s themselves represents a leap second
    /// in which case the assumption becomes that
    /// **there are exactly one (or two) leap second(s) ever**.
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::{NaiveDate, TimeDelta};
    ///
    /// let from_ymd = |y, m, d| NaiveDate::from_ymd_opt(y, m, d).unwrap();
    ///
    /// let d = from_ymd(2016, 7, 8);
    /// assert_eq!(
    ///     d.and_hms_opt(3, 5, 7).unwrap().signed_duration_since(d.and_hms_opt(2, 4, 6).unwrap()),
    ///     TimeDelta::try_seconds(3600 + 60 + 1).unwrap()
    /// );
    ///
    /// // July 8 is 190th day in the year 2016
    /// let d0 = from_ymd(2016, 1, 1);
    /// assert_eq!(
    ///     d.and_hms_milli_opt(0, 7, 6, 500)
    ///         .unwrap()
    ///         .signed_duration_since(d0.and_hms_opt(0, 0, 0).unwrap()),
    ///     TimeDelta::try_seconds(189 * 86_400 + 7 * 60 + 6).unwrap()
    ///         + TimeDelta::try_milliseconds(500).unwrap()
    /// );
    /// ```
    ///
    /// Leap seconds are handled, but the subtraction assumes that
    /// there were no other leap seconds happened.
    ///
    /// ```
    /// # use chrono::{TimeDelta, NaiveDate};
    /// # let from_ymd = |y, m, d| NaiveDate::from_ymd_opt(y, m, d).unwrap();
    /// let leap = from_ymd(2015, 6, 30).and_hms_milli_opt(23, 59, 59, 1_500).unwrap();
    /// assert_eq!(
    ///     leap.signed_duration_since(from_ymd(2015, 6, 30).and_hms_opt(23, 0, 0).unwrap()),
    ///     TimeDelta::try_seconds(3600).unwrap() + TimeDelta::try_milliseconds(500).unwrap()
    /// );
    /// assert_eq!(
    ///     from_ymd(2015, 7, 1).and_hms_opt(1, 0, 0).unwrap().signed_duration_since(leap),
    ///     TimeDelta::try_seconds(3600).unwrap() - TimeDelta::try_milliseconds(500).unwrap()
    /// );
    /// ```
    #[must_use]
    pub const fn signed_duration_since(self, rhs: NaiveDateTime) -> TimeDelta {
        expect(
            self.date
                .signed_duration_since(rhs.date)
                .checked_add(&self.time.signed_duration_since(rhs.time)),
            "always in range",
        )
    }

    /// Formats the combined date and time with the specified formatting items.
    /// Otherwise it is the same as the ordinary [`format`](#method.format) method.
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
    /// let fmt = StrftimeItems::new("%Y-%m-%d %H:%M:%S");
    /// let dt = NaiveDate::from_ymd_opt(2015, 9, 5).unwrap().and_hms_opt(23, 56, 4).unwrap();
    /// assert_eq!(dt.format_with_items(fmt.clone()).to_string(), "2015-09-05 23:56:04");
    /// assert_eq!(dt.format("%Y-%m-%d %H:%M:%S").to_string(), "2015-09-05 23:56:04");
    /// ```
    ///
    /// The resulting `DelayedFormat` can be formatted directly via the `Display` trait.
    ///
    /// ```
    /// # use chrono::NaiveDate;
    /// # use chrono::format::strftime::StrftimeItems;
    /// # let fmt = StrftimeItems::new("%Y-%m-%d %H:%M:%S").clone();
    /// # let dt = NaiveDate::from_ymd_opt(2015, 9, 5).unwrap().and_hms_opt(23, 56, 4).unwrap();
    /// assert_eq!(format!("{}", dt.format_with_items(fmt)), "2015-09-05 23:56:04");
    /// ```
    #[cfg(feature = "alloc")]
    #[inline]
    #[must_use]
    pub fn format_with_items<'a, I, B>(&self, items: I) -> DelayedFormat<I>
    where
        I: Iterator<Item = B> + Clone,
        B: Borrow<Item<'a>>,
    {
        DelayedFormat::new(Some(self.date), Some(self.time), items)
    }

    /// Formats the combined date and time with the specified format string.
    /// See the [`format::strftime` module](crate::format::strftime)
    /// on the supported escape sequences.
    ///
    /// This returns a `DelayedFormat`,
    /// which gets converted to a string only when actual formatting happens.
    /// You may use the `to_string` method to get a `String`,
    /// or just feed it into `print!` and other formatting macros.
    /// (In this way it avoids the redundant memory allocation.)
    ///
    /// A wrong format string does *not* issue an error immediately.
    /// Rather, converting or formatting the `DelayedFormat` fails.
    /// You are recommended to immediately use `DelayedFormat` for this reason.
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::NaiveDate;
    ///
    /// let dt = NaiveDate::from_ymd_opt(2015, 9, 5).unwrap().and_hms_opt(23, 56, 4).unwrap();
    /// assert_eq!(dt.format("%Y-%m-%d %H:%M:%S").to_string(), "2015-09-05 23:56:04");
    /// assert_eq!(dt.format("around %l %p on %b %-d").to_string(), "around 11 PM on Sep 5");
    /// ```
    ///
    /// The resulting `DelayedFormat` can be formatted directly via the `Display` trait.
    ///
    /// ```
    /// # use chrono::NaiveDate;
    /// # let dt = NaiveDate::from_ymd_opt(2015, 9, 5).unwrap().and_hms_opt(23, 56, 4).unwrap();
    /// assert_eq!(format!("{}", dt.format("%Y-%m-%d %H:%M:%S")), "2015-09-05 23:56:04");
    /// assert_eq!(format!("{}", dt.format("around %l %p on %b %-d")), "around 11 PM on Sep 5");
    /// ```
    #[cfg(feature = "alloc")]
    #[inline]
    #[must_use]
    pub fn format<'a>(&self, fmt: &'a str) -> DelayedFormat<StrftimeItems<'a>> {
        self.format_with_items(StrftimeItems::new(fmt))
    }

    /// Converts the `NaiveDateTime` into a timezone-aware `DateTime<Tz>` with the provided
    /// time zone.
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::{FixedOffset, NaiveDate};
    /// let hour = 3600;
    /// let tz = FixedOffset::east_opt(5 * hour).unwrap();
    /// let dt = NaiveDate::from_ymd_opt(2015, 9, 5)
    ///     .unwrap()
    ///     .and_hms_opt(23, 56, 4)
    ///     .unwrap()
    ///     .and_local_timezone(tz)
    ///     .unwrap();
    /// assert_eq!(dt.timezone(), tz);
    /// ```
    #[must_use]
    pub fn and_local_timezone<Tz: TimeZone>(&self, tz: Tz) -> MappedLocalTime<DateTime<Tz>> {
        tz.from_local_datetime(self)
    }

    /// Converts the `NaiveDateTime` into the timezone-aware `DateTime<Utc>`.
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::{NaiveDate, Utc};
    /// let dt =
    ///     NaiveDate::from_ymd_opt(2023, 1, 30).unwrap().and_hms_opt(19, 32, 33).unwrap().and_utc();
    /// assert_eq!(dt.timezone(), Utc);
    /// ```
    #[must_use]
    pub const fn and_utc(&self) -> DateTime<Utc> {
        DateTime::from_naive_utc_and_offset(*self, Utc)
    }

    /// The minimum possible `NaiveDateTime`.
    pub const MIN: Self = Self { date: NaiveDate::MIN, time: NaiveTime::MIN };

    /// The maximum possible `NaiveDateTime`.
    pub const MAX: Self = Self { date: NaiveDate::MAX, time: NaiveTime::MAX };

    /// The datetime of the Unix Epoch, 1970-01-01 00:00:00.
    ///
    /// Note that while this may look like the UNIX epoch, it is missing the
    /// time zone. The actual UNIX epoch cannot be expressed by this type,
    /// however it is available as [`DateTime::UNIX_EPOCH`].
    #[deprecated(since = "0.4.41", note = "use `DateTime::UNIX_EPOCH` instead")]
    pub const UNIX_EPOCH: Self = DateTime::UNIX_EPOCH.naive_utc();
}

impl From<NaiveDate> for NaiveDateTime {
    /// Converts a `NaiveDate` to a `NaiveDateTime` of the same date but at midnight.
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::{NaiveDate, NaiveDateTime};
    ///
    /// let nd = NaiveDate::from_ymd_opt(2016, 5, 28).unwrap();
    /// let ndt = NaiveDate::from_ymd_opt(2016, 5, 28).unwrap().and_hms_opt(0, 0, 0).unwrap();
    /// assert_eq!(ndt, NaiveDateTime::from(nd));
    fn from(date: NaiveDate) -> Self {
        date.and_hms_opt(0, 0, 0).unwrap()
    }
}

impl Datelike for NaiveDateTime {
    /// Returns the year number in the [calendar date](./struct.NaiveDate.html#calendar-date).
    ///
    /// See also the [`NaiveDate::year`](./struct.NaiveDate.html#method.year) method.
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::{Datelike, NaiveDate, NaiveDateTime};
    ///
    /// let dt: NaiveDateTime =
    ///     NaiveDate::from_ymd_opt(2015, 9, 25).unwrap().and_hms_opt(12, 34, 56).unwrap();
    /// assert_eq!(dt.year(), 2015);
    /// ```
    #[inline]
    fn year(&self) -> i32 {
        self.date.year()
    }

    /// Returns the month number starting from 1.
    ///
    /// The return value ranges from 1 to 12.
    ///
    /// See also the [`NaiveDate::month`](./struct.NaiveDate.html#method.month) method.
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::{Datelike, NaiveDate, NaiveDateTime};
    ///
    /// let dt: NaiveDateTime =
    ///     NaiveDate::from_ymd_opt(2015, 9, 25).unwrap().and_hms_opt(12, 34, 56).unwrap();
    /// assert_eq!(dt.month(), 9);
    /// ```
    #[inline]
    fn month(&self) -> u32 {
        self.date.month()
    }

    /// Returns the month number starting from 0.
    ///
    /// The return value ranges from 0 to 11.
    ///
    /// See also the [`NaiveDate::month0`] method.
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::{Datelike, NaiveDate, NaiveDateTime};
    ///
    /// let dt: NaiveDateTime =
    ///     NaiveDate::from_ymd_opt(2015, 9, 25).unwrap().and_hms_opt(12, 34, 56).unwrap();
    /// assert_eq!(dt.month0(), 8);
    /// ```
    #[inline]
    fn month0(&self) -> u32 {
        self.date.month0()
    }

    /// Returns the day of month starting from 1.
    ///
    /// The return value ranges from 1 to 31. (The last day of month differs by months.)
    ///
    /// See also the [`NaiveDate::day`](./struct.NaiveDate.html#method.day) method.
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::{Datelike, NaiveDate, NaiveDateTime};
    ///
    /// let dt: NaiveDateTime =
    ///     NaiveDate::from_ymd_opt(2015, 9, 25).unwrap().and_hms_opt(12, 34, 56).unwrap();
    /// assert_eq!(dt.day(), 25);
    /// ```
    #[inline]
    fn day(&self) -> u32 {
        self.date.day()
    }

    /// Returns the day of month starting from 0.
    ///
    /// The return value ranges from 0 to 30. (The last day of month differs by months.)
    ///
    /// See also the [`NaiveDate::day0`] method.
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::{Datelike, NaiveDate, NaiveDateTime};
    ///
    /// let dt: NaiveDateTime =
    ///     NaiveDate::from_ymd_opt(2015, 9, 25).unwrap().and_hms_opt(12, 34, 56).unwrap();
    /// assert_eq!(dt.day0(), 24);
    /// ```
    #[inline]
    fn day0(&self) -> u32 {
        self.date.day0()
    }

    /// Returns the day of year starting from 1.
    ///
    /// The return value ranges from 1 to 366. (The last day of year differs by years.)
    ///
    /// See also the [`NaiveDate::ordinal`](./struct.NaiveDate.html#method.ordinal) method.
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::{Datelike, NaiveDate, NaiveDateTime};
    ///
    /// let dt: NaiveDateTime =
    ///     NaiveDate::from_ymd_opt(2015, 9, 25).unwrap().and_hms_opt(12, 34, 56).unwrap();
    /// assert_eq!(dt.ordinal(), 268);
    /// ```
    #[inline]
    fn ordinal(&self) -> u32 {
        self.date.ordinal()
    }

    /// Returns the day of year starting from 0.
    ///
    /// The return value ranges from 0 to 365. (The last day of year differs by years.)
    ///
    /// See also the [`NaiveDate::ordinal0`] method.
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::{Datelike, NaiveDate, NaiveDateTime};
    ///
    /// let dt: NaiveDateTime =
    ///     NaiveDate::from_ymd_opt(2015, 9, 25).unwrap().and_hms_opt(12, 34, 56).unwrap();
    /// assert_eq!(dt.ordinal0(), 267);
    /// ```
    #[inline]
    fn ordinal0(&self) -> u32 {
        self.date.ordinal0()
    }

    /// Returns the day of week.
    ///
    /// See also the [`NaiveDate::weekday`](./struct.NaiveDate.html#method.weekday) method.
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::{Datelike, NaiveDate, NaiveDateTime, Weekday};
    ///
    /// let dt: NaiveDateTime =
    ///     NaiveDate::from_ymd_opt(2015, 9, 25).unwrap().and_hms_opt(12, 34, 56).unwrap();
    /// assert_eq!(dt.weekday(), Weekday::Fri);
    /// ```
    #[inline]
    fn weekday(&self) -> Weekday {
        self.date.weekday()
    }

    #[inline]
    fn iso_week(&self) -> IsoWeek {
        self.date.iso_week()
    }

    /// Makes a new `NaiveDateTime` with the year number changed, while keeping the same month and
    /// day.
    ///
    /// See also the [`NaiveDate::with_year`] method.
    ///
    /// # Errors
    ///
    /// Returns `None` if:
    /// - The resulting date does not exist (February 29 in a non-leap year).
    /// - The year is out of range for a `NaiveDate`.
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::{Datelike, NaiveDate, NaiveDateTime};
    ///
    /// let dt: NaiveDateTime =
    ///     NaiveDate::from_ymd_opt(2015, 9, 25).unwrap().and_hms_opt(12, 34, 56).unwrap();
    /// assert_eq!(
    ///     dt.with_year(2016),
    ///     Some(NaiveDate::from_ymd_opt(2016, 9, 25).unwrap().and_hms_opt(12, 34, 56).unwrap())
    /// );
    /// assert_eq!(
    ///     dt.with_year(-308),
    ///     Some(NaiveDate::from_ymd_opt(-308, 9, 25).unwrap().and_hms_opt(12, 34, 56).unwrap())
    /// );
    /// ```
    #[inline]
    fn with_year(&self, year: i32) -> Option<NaiveDateTime> {
        self.date.with_year(year).map(|d| NaiveDateTime { date: d, ..*self })
    }

    /// Makes a new `NaiveDateTime` with the month number (starting from 1) changed.
    ///
    /// Don't combine multiple `Datelike::with_*` methods. The intermediate value may not exist.
    ///
    /// See also the [`NaiveDate::with_month`] method.
    ///
    /// # Errors
    ///
    /// Returns `None` if:
    /// - The resulting date does not exist (for example `month(4)` when day of the month is 31).
    /// - The value for `month` is invalid.
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::{Datelike, NaiveDate, NaiveDateTime};
    ///
    /// let dt: NaiveDateTime =
    ///     NaiveDate::from_ymd_opt(2015, 9, 30).unwrap().and_hms_opt(12, 34, 56).unwrap();
    /// assert_eq!(
    ///     dt.with_month(10),
    ///     Some(NaiveDate::from_ymd_opt(2015, 10, 30).unwrap().and_hms_opt(12, 34, 56).unwrap())
    /// );
    /// assert_eq!(dt.with_month(13), None); // No month 13
    /// assert_eq!(dt.with_month(2), None); // No February 30
    /// ```
    #[inline]
    fn with_month(&self, month: u32) -> Option<NaiveDateTime> {
        self.date.with_month(month).map(|d| NaiveDateTime { date: d, ..*self })
    }

    /// Makes a new `NaiveDateTime` with the month number (starting from 0) changed.
    ///
    /// See also the [`NaiveDate::with_month0`] method.
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
    /// use chrono::{Datelike, NaiveDate, NaiveDateTime};
    ///
    /// let dt: NaiveDateTime =
    ///     NaiveDate::from_ymd_opt(2015, 9, 30).unwrap().and_hms_opt(12, 34, 56).unwrap();
    /// assert_eq!(
    ///     dt.with_month0(9),
    ///     Some(NaiveDate::from_ymd_opt(2015, 10, 30).unwrap().and_hms_opt(12, 34, 56).unwrap())
    /// );
    /// assert_eq!(dt.with_month0(12), None); // No month 13
    /// assert_eq!(dt.with_month0(1), None); // No February 30
    /// ```
    #[inline]
    fn with_month0(&self, month0: u32) -> Option<NaiveDateTime> {
        self.date.with_month0(month0).map(|d| NaiveDateTime { date: d, ..*self })
    }

    /// Makes a new `NaiveDateTime` with the day of month (starting from 1) changed.
    ///
    /// See also the [`NaiveDate::with_day`] method.
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
    /// use chrono::{Datelike, NaiveDate, NaiveDateTime};
    ///
    /// let dt: NaiveDateTime =
    ///     NaiveDate::from_ymd_opt(2015, 9, 8).unwrap().and_hms_opt(12, 34, 56).unwrap();
    /// assert_eq!(
    ///     dt.with_day(30),
    ///     Some(NaiveDate::from_ymd_opt(2015, 9, 30).unwrap().and_hms_opt(12, 34, 56).unwrap())
    /// );
    /// assert_eq!(dt.with_day(31), None); // no September 31
    /// ```
    #[inline]
    fn with_day(&self, day: u32) -> Option<NaiveDateTime> {
        self.date.with_day(day).map(|d| NaiveDateTime { date: d, ..*self })
    }

    /// Makes a new `NaiveDateTime` with the day of month (starting from 0) changed.
    ///
    /// See also the [`NaiveDate::with_day0`] method.
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
    /// use chrono::{Datelike, NaiveDate, NaiveDateTime};
    ///
    /// let dt: NaiveDateTime =
    ///     NaiveDate::from_ymd_opt(2015, 9, 8).unwrap().and_hms_opt(12, 34, 56).unwrap();
    /// assert_eq!(
    ///     dt.with_day0(29),
    ///     Some(NaiveDate::from_ymd_opt(2015, 9, 30).unwrap().and_hms_opt(12, 34, 56).unwrap())
    /// );
    /// assert_eq!(dt.with_day0(30), None); // no September 31
    /// ```
    #[inline]
    fn with_day0(&self, day0: u32) -> Option<NaiveDateTime> {
        self.date.with_day0(day0).map(|d| NaiveDateTime { date: d, ..*self })
    }

    /// Makes a new `NaiveDateTime` with the day of year (starting from 1) changed.
    ///
    /// See also the [`NaiveDate::with_ordinal`] method.
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
    /// use chrono::{Datelike, NaiveDate, NaiveDateTime};
    ///
    /// let dt: NaiveDateTime =
    ///     NaiveDate::from_ymd_opt(2015, 9, 8).unwrap().and_hms_opt(12, 34, 56).unwrap();
    /// assert_eq!(
    ///     dt.with_ordinal(60),
    ///     Some(NaiveDate::from_ymd_opt(2015, 3, 1).unwrap().and_hms_opt(12, 34, 56).unwrap())
    /// );
    /// assert_eq!(dt.with_ordinal(366), None); // 2015 had only 365 days
    ///
    /// let dt: NaiveDateTime =
    ///     NaiveDate::from_ymd_opt(2016, 9, 8).unwrap().and_hms_opt(12, 34, 56).unwrap();
    /// assert_eq!(
    ///     dt.with_ordinal(60),
    ///     Some(NaiveDate::from_ymd_opt(2016, 2, 29).unwrap().and_hms_opt(12, 34, 56).unwrap())
    /// );
    /// assert_eq!(
    ///     dt.with_ordinal(366),
    ///     Some(NaiveDate::from_ymd_opt(2016, 12, 31).unwrap().and_hms_opt(12, 34, 56).unwrap())
    /// );
    /// ```
    #[inline]
    fn with_ordinal(&self, ordinal: u32) -> Option<NaiveDateTime> {
        self.date.with_ordinal(ordinal).map(|d| NaiveDateTime { date: d, ..*self })
    }

    /// Makes a new `NaiveDateTime` with the day of year (starting from 0) changed.
    ///
    /// See also the [`NaiveDate::with_ordinal0`] method.
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
    /// use chrono::{Datelike, NaiveDate, NaiveDateTime};
    ///
    /// let dt: NaiveDateTime =
    ///     NaiveDate::from_ymd_opt(2015, 9, 8).unwrap().and_hms_opt(12, 34, 56).unwrap();
    /// assert_eq!(
    ///     dt.with_ordinal0(59),
    ///     Some(NaiveDate::from_ymd_opt(2015, 3, 1).unwrap().and_hms_opt(12, 34, 56).unwrap())
    /// );
    /// assert_eq!(dt.with_ordinal0(365), None); // 2015 had only 365 days
    ///
    /// let dt: NaiveDateTime =
    ///     NaiveDate::from_ymd_opt(2016, 9, 8).unwrap().and_hms_opt(12, 34, 56).unwrap();
    /// assert_eq!(
    ///     dt.with_ordinal0(59),
    ///     Some(NaiveDate::from_ymd_opt(2016, 2, 29).unwrap().and_hms_opt(12, 34, 56).unwrap())
    /// );
    /// assert_eq!(
    ///     dt.with_ordinal0(365),
    ///     Some(NaiveDate::from_ymd_opt(2016, 12, 31).unwrap().and_hms_opt(12, 34, 56).unwrap())
    /// );
    /// ```
    #[inline]
    fn with_ordinal0(&self, ordinal0: u32) -> Option<NaiveDateTime> {
        self.date.with_ordinal0(ordinal0).map(|d| NaiveDateTime { date: d, ..*self })
    }
}

impl Timelike for NaiveDateTime {
    /// Returns the hour number from 0 to 23.
    ///
    /// See also the [`NaiveTime::hour`] method.
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::{NaiveDate, NaiveDateTime, Timelike};
    ///
    /// let dt: NaiveDateTime =
    ///     NaiveDate::from_ymd_opt(2015, 9, 8).unwrap().and_hms_milli_opt(12, 34, 56, 789).unwrap();
    /// assert_eq!(dt.hour(), 12);
    /// ```
    #[inline]
    fn hour(&self) -> u32 {
        self.time.hour()
    }

    /// Returns the minute number from 0 to 59.
    ///
    /// See also the [`NaiveTime::minute`] method.
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::{NaiveDate, NaiveDateTime, Timelike};
    ///
    /// let dt: NaiveDateTime =
    ///     NaiveDate::from_ymd_opt(2015, 9, 8).unwrap().and_hms_milli_opt(12, 34, 56, 789).unwrap();
    /// assert_eq!(dt.minute(), 34);
    /// ```
    #[inline]
    fn minute(&self) -> u32 {
        self.time.minute()
    }

    /// Returns the second number from 0 to 59.
    ///
    /// See also the [`NaiveTime::second`] method.
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::{NaiveDate, NaiveDateTime, Timelike};
    ///
    /// let dt: NaiveDateTime =
    ///     NaiveDate::from_ymd_opt(2015, 9, 8).unwrap().and_hms_milli_opt(12, 34, 56, 789).unwrap();
    /// assert_eq!(dt.second(), 56);
    /// ```
    #[inline]
    fn second(&self) -> u32 {
        self.time.second()
    }

    /// Returns the number of nanoseconds since the whole non-leap second.
    /// The range from 1,000,000,000 to 1,999,999,999 represents
    /// the [leap second](./struct.NaiveTime.html#leap-second-handling).
    ///
    /// See also the [`NaiveTime#method.nanosecond`] method.
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::{NaiveDate, NaiveDateTime, Timelike};
    ///
    /// let dt: NaiveDateTime =
    ///     NaiveDate::from_ymd_opt(2015, 9, 8).unwrap().and_hms_milli_opt(12, 34, 56, 789).unwrap();
    /// assert_eq!(dt.nanosecond(), 789_000_000);
    /// ```
    #[inline]
    fn nanosecond(&self) -> u32 {
        self.time.nanosecond()
    }

    /// Makes a new `NaiveDateTime` with the hour number changed.
    ///
    /// See also the [`NaiveTime::with_hour`] method.
    ///
    /// # Errors
    ///
    /// Returns `None` if the value for `hour` is invalid.
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::{NaiveDate, NaiveDateTime, Timelike};
    ///
    /// let dt: NaiveDateTime =
    ///     NaiveDate::from_ymd_opt(2015, 9, 8).unwrap().and_hms_milli_opt(12, 34, 56, 789).unwrap();
    /// assert_eq!(
    ///     dt.with_hour(7),
    ///     Some(
    ///         NaiveDate::from_ymd_opt(2015, 9, 8).unwrap().and_hms_milli_opt(7, 34, 56, 789).unwrap()
    ///     )
    /// );
    /// assert_eq!(dt.with_hour(24), None);
    /// ```
    #[inline]
    fn with_hour(&self, hour: u32) -> Option<NaiveDateTime> {
        self.time.with_hour(hour).map(|t| NaiveDateTime { time: t, ..*self })
    }

    /// Makes a new `NaiveDateTime` with the minute number changed.
    ///
    /// See also the [`NaiveTime::with_minute`] method.
    ///
    /// # Errors
    ///
    /// Returns `None` if the value for `minute` is invalid.
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::{NaiveDate, NaiveDateTime, Timelike};
    ///
    /// let dt: NaiveDateTime =
    ///     NaiveDate::from_ymd_opt(2015, 9, 8).unwrap().and_hms_milli_opt(12, 34, 56, 789).unwrap();
    /// assert_eq!(
    ///     dt.with_minute(45),
    ///     Some(
    ///         NaiveDate::from_ymd_opt(2015, 9, 8)
    ///             .unwrap()
    ///             .and_hms_milli_opt(12, 45, 56, 789)
    ///             .unwrap()
    ///     )
    /// );
    /// assert_eq!(dt.with_minute(60), None);
    /// ```
    #[inline]
    fn with_minute(&self, min: u32) -> Option<NaiveDateTime> {
        self.time.with_minute(min).map(|t| NaiveDateTime { time: t, ..*self })
    }

    /// Makes a new `NaiveDateTime` with the second number changed.
    ///
    /// As with the [`second`](#method.second) method,
    /// the input range is restricted to 0 through 59.
    ///
    /// See also the [`NaiveTime::with_second`] method.
    ///
    /// # Errors
    ///
    /// Returns `None` if the value for `second` is invalid.
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::{NaiveDate, NaiveDateTime, Timelike};
    ///
    /// let dt: NaiveDateTime =
    ///     NaiveDate::from_ymd_opt(2015, 9, 8).unwrap().and_hms_milli_opt(12, 34, 56, 789).unwrap();
    /// assert_eq!(
    ///     dt.with_second(17),
    ///     Some(
    ///         NaiveDate::from_ymd_opt(2015, 9, 8)
    ///             .unwrap()
    ///             .and_hms_milli_opt(12, 34, 17, 789)
    ///             .unwrap()
    ///     )
    /// );
    /// assert_eq!(dt.with_second(60), None);
    /// ```
    #[inline]
    fn with_second(&self, sec: u32) -> Option<NaiveDateTime> {
        self.time.with_second(sec).map(|t| NaiveDateTime { time: t, ..*self })
    }

    /// Makes a new `NaiveDateTime` with nanoseconds since the whole non-leap second changed.
    ///
    /// Returns `None` when the resulting `NaiveDateTime` would be invalid.
    /// As with the [`NaiveDateTime::nanosecond`] method,
    /// the input range can exceed 1,000,000,000 for leap seconds.
    ///
    /// See also the [`NaiveTime::with_nanosecond`] method.
    ///
    /// # Errors
    ///
    /// Returns `None` if `nanosecond >= 2,000,000,000`.
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::{NaiveDate, NaiveDateTime, Timelike};
    ///
    /// let dt: NaiveDateTime =
    ///     NaiveDate::from_ymd_opt(2015, 9, 8).unwrap().and_hms_milli_opt(12, 34, 59, 789).unwrap();
    /// assert_eq!(
    ///     dt.with_nanosecond(333_333_333),
    ///     Some(
    ///         NaiveDate::from_ymd_opt(2015, 9, 8)
    ///             .unwrap()
    ///             .and_hms_nano_opt(12, 34, 59, 333_333_333)
    ///             .unwrap()
    ///     )
    /// );
    /// assert_eq!(
    ///     dt.with_nanosecond(1_333_333_333), // leap second
    ///     Some(
    ///         NaiveDate::from_ymd_opt(2015, 9, 8)
    ///             .unwrap()
    ///             .and_hms_nano_opt(12, 34, 59, 1_333_333_333)
    ///             .unwrap()
    ///     )
    /// );
    /// assert_eq!(dt.with_nanosecond(2_000_000_000), None);
    /// ```
    #[inline]
    fn with_nanosecond(&self, nano: u32) -> Option<NaiveDateTime> {
        self.time.with_nanosecond(nano).map(|t| NaiveDateTime { time: t, ..*self })
    }
}

/// Add `TimeDelta` to `NaiveDateTime`.
///
/// As a part of Chrono's [leap second handling], the addition assumes that **there is no leap
/// second ever**, except when the `NaiveDateTime` itself represents a leap  second in which case
/// the assumption becomes that **there is exactly a single leap second ever**.
///
/// # Panics
///
/// Panics if the resulting date would be out of range.
/// Consider using [`NaiveDateTime::checked_add_signed`] to get an `Option` instead.
///
/// # Example
///
/// ```
/// use chrono::{NaiveDate, TimeDelta};
///
/// let from_ymd = |y, m, d| NaiveDate::from_ymd_opt(y, m, d).unwrap();
///
/// let d = from_ymd(2016, 7, 8);
/// let hms = |h, m, s| d.and_hms_opt(h, m, s).unwrap();
/// assert_eq!(hms(3, 5, 7) + TimeDelta::zero(), hms(3, 5, 7));
/// assert_eq!(hms(3, 5, 7) + TimeDelta::try_seconds(1).unwrap(), hms(3, 5, 8));
/// assert_eq!(hms(3, 5, 7) + TimeDelta::try_seconds(-1).unwrap(), hms(3, 5, 6));
/// assert_eq!(hms(3, 5, 7) + TimeDelta::try_seconds(3600 + 60).unwrap(), hms(4, 6, 7));
/// assert_eq!(
///     hms(3, 5, 7) + TimeDelta::try_seconds(86_400).unwrap(),
///     from_ymd(2016, 7, 9).and_hms_opt(3, 5, 7).unwrap()
/// );
/// assert_eq!(
///     hms(3, 5, 7) + TimeDelta::try_days(365).unwrap(),
///     from_ymd(2017, 7, 8).and_hms_opt(3, 5, 7).unwrap()
/// );
///
/// let hmsm = |h, m, s, milli| d.and_hms_milli_opt(h, m, s, milli).unwrap();
/// assert_eq!(hmsm(3, 5, 7, 980) + TimeDelta::try_milliseconds(450).unwrap(), hmsm(3, 5, 8, 430));
/// ```
///
/// Leap seconds are handled,
/// but the addition assumes that it is the only leap second happened.
///
/// ```
/// # use chrono::{TimeDelta, NaiveDate};
/// # let from_ymd = |y, m, d| NaiveDate::from_ymd_opt(y, m, d).unwrap();
/// # let hmsm = |h, m, s, milli| from_ymd(2016, 7, 8).and_hms_milli_opt(h, m, s, milli).unwrap();
/// let leap = hmsm(3, 5, 59, 1_300);
/// assert_eq!(leap + TimeDelta::zero(), hmsm(3, 5, 59, 1_300));
/// assert_eq!(leap + TimeDelta::try_milliseconds(-500).unwrap(), hmsm(3, 5, 59, 800));
/// assert_eq!(leap + TimeDelta::try_milliseconds(500).unwrap(), hmsm(3, 5, 59, 1_800));
/// assert_eq!(leap + TimeDelta::try_milliseconds(800).unwrap(), hmsm(3, 6, 0, 100));
/// assert_eq!(leap + TimeDelta::try_seconds(10).unwrap(), hmsm(3, 6, 9, 300));
/// assert_eq!(leap + TimeDelta::try_seconds(-10).unwrap(), hmsm(3, 5, 50, 300));
/// assert_eq!(leap + TimeDelta::try_days(1).unwrap(),
///            from_ymd(2016, 7, 9).and_hms_milli_opt(3, 5, 59, 300).unwrap());
/// ```
///
/// [leap second handling]: crate::NaiveTime#leap-second-handling
impl Add<TimeDelta> for NaiveDateTime {
    type Output = NaiveDateTime;

    #[inline]
    fn add(self, rhs: TimeDelta) -> NaiveDateTime {
        self.checked_add_signed(rhs).expect("`NaiveDateTime + TimeDelta` overflowed")
    }
}

/// Add `std::time::Duration` to `NaiveDateTime`.
///
/// As a part of Chrono's [leap second handling], the addition assumes that **there is no leap
/// second ever**, except when the `NaiveDateTime` itself represents a leap  second in which case
/// the assumption becomes that **there is exactly a single leap second ever**.
///
/// # Panics
///
/// Panics if the resulting date would be out of range.
/// Consider using [`NaiveDateTime::checked_add_signed`] to get an `Option` instead.
impl Add<Duration> for NaiveDateTime {
    type Output = NaiveDateTime;

    #[inline]
    fn add(self, rhs: Duration) -> NaiveDateTime {
        let rhs = TimeDelta::from_std(rhs)
            .expect("overflow converting from core::time::Duration to TimeDelta");
        self.checked_add_signed(rhs).expect("`NaiveDateTime + TimeDelta` overflowed")
    }
}

/// Add-assign `TimeDelta` to `NaiveDateTime`.
///
/// As a part of Chrono's [leap second handling], the addition assumes that **there is no leap
/// second ever**, except when the `NaiveDateTime` itself represents a leap  second in which case
/// the assumption becomes that **there is exactly a single leap second ever**.
///
/// # Panics
///
/// Panics if the resulting date would be out of range.
/// Consider using [`NaiveDateTime::checked_add_signed`] to get an `Option` instead.
impl AddAssign<TimeDelta> for NaiveDateTime {
    #[inline]
    fn add_assign(&mut self, rhs: TimeDelta) {
        *self = self.add(rhs);
    }
}

/// Add-assign `std::time::Duration` to `NaiveDateTime`.
///
/// As a part of Chrono's [leap second handling], the addition assumes that **there is no leap
/// second ever**, except when the `NaiveDateTime` itself represents a leap  second in which case
/// the assumption becomes that **there is exactly a single leap second ever**.
///
/// # Panics
///
/// Panics if the resulting date would be out of range.
/// Consider using [`NaiveDateTime::checked_add_signed`] to get an `Option` instead.
impl AddAssign<Duration> for NaiveDateTime {
    #[inline]
    fn add_assign(&mut self, rhs: Duration) {
        *self = self.add(rhs);
    }
}

/// Add `FixedOffset` to `NaiveDateTime`.
///
/// # Panics
///
/// Panics if the resulting date would be out of range.
/// Consider using `checked_add_offset` to get an `Option` instead.
impl Add<FixedOffset> for NaiveDateTime {
    type Output = NaiveDateTime;

    #[inline]
    fn add(self, rhs: FixedOffset) -> NaiveDateTime {
        self.checked_add_offset(rhs).expect("`NaiveDateTime + FixedOffset` out of range")
    }
}

/// Add `Months` to `NaiveDateTime`.
///
/// The result will be clamped to valid days in the resulting month, see `checked_add_months` for
/// details.
///
/// # Panics
///
/// Panics if the resulting date would be out of range.
/// Consider using `checked_add_months` to get an `Option` instead.
///
/// # Example
///
/// ```
/// use chrono::{Months, NaiveDate};
///
/// assert_eq!(
///     NaiveDate::from_ymd_opt(2014, 1, 1).unwrap().and_hms_opt(1, 0, 0).unwrap() + Months::new(1),
///     NaiveDate::from_ymd_opt(2014, 2, 1).unwrap().and_hms_opt(1, 0, 0).unwrap()
/// );
/// assert_eq!(
///     NaiveDate::from_ymd_opt(2014, 1, 1).unwrap().and_hms_opt(0, 2, 0).unwrap()
///         + Months::new(11),
///     NaiveDate::from_ymd_opt(2014, 12, 1).unwrap().and_hms_opt(0, 2, 0).unwrap()
/// );
/// assert_eq!(
///     NaiveDate::from_ymd_opt(2014, 1, 1).unwrap().and_hms_opt(0, 0, 3).unwrap()
///         + Months::new(12),
///     NaiveDate::from_ymd_opt(2015, 1, 1).unwrap().and_hms_opt(0, 0, 3).unwrap()
/// );
/// assert_eq!(
///     NaiveDate::from_ymd_opt(2014, 1, 1).unwrap().and_hms_opt(0, 0, 4).unwrap()
///         + Months::new(13),
///     NaiveDate::from_ymd_opt(2015, 2, 1).unwrap().and_hms_opt(0, 0, 4).unwrap()
/// );
/// assert_eq!(
///     NaiveDate::from_ymd_opt(2014, 1, 31).unwrap().and_hms_opt(0, 5, 0).unwrap()
///         + Months::new(1),
///     NaiveDate::from_ymd_opt(2014, 2, 28).unwrap().and_hms_opt(0, 5, 0).unwrap()
/// );
/// assert_eq!(
///     NaiveDate::from_ymd_opt(2020, 1, 31).unwrap().and_hms_opt(6, 0, 0).unwrap()
///         + Months::new(1),
///     NaiveDate::from_ymd_opt(2020, 2, 29).unwrap().and_hms_opt(6, 0, 0).unwrap()
/// );
/// ```
impl Add<Months> for NaiveDateTime {
    type Output = NaiveDateTime;

    fn add(self, rhs: Months) -> Self::Output {
        self.checked_add_months(rhs).expect("`NaiveDateTime + Months` out of range")
    }
}

/// Subtract `TimeDelta` from `NaiveDateTime`.
///
/// This is the same as the addition with a negated `TimeDelta`.
///
/// As a part of Chrono's [leap second handling] the subtraction assumes that **there is no leap
/// second ever**, except when the `NaiveDateTime` itself represents a leap second in which case
/// the assumption becomes that **there is exactly a single leap second ever**.
///
/// # Panics
///
/// Panics if the resulting date would be out of range.
/// Consider using [`NaiveDateTime::checked_sub_signed`] to get an `Option` instead.
///
/// # Example
///
/// ```
/// use chrono::{NaiveDate, TimeDelta};
///
/// let from_ymd = |y, m, d| NaiveDate::from_ymd_opt(y, m, d).unwrap();
///
/// let d = from_ymd(2016, 7, 8);
/// let hms = |h, m, s| d.and_hms_opt(h, m, s).unwrap();
/// assert_eq!(hms(3, 5, 7) - TimeDelta::zero(), hms(3, 5, 7));
/// assert_eq!(hms(3, 5, 7) - TimeDelta::try_seconds(1).unwrap(), hms(3, 5, 6));
/// assert_eq!(hms(3, 5, 7) - TimeDelta::try_seconds(-1).unwrap(), hms(3, 5, 8));
/// assert_eq!(hms(3, 5, 7) - TimeDelta::try_seconds(3600 + 60).unwrap(), hms(2, 4, 7));
/// assert_eq!(
///     hms(3, 5, 7) - TimeDelta::try_seconds(86_400).unwrap(),
///     from_ymd(2016, 7, 7).and_hms_opt(3, 5, 7).unwrap()
/// );
/// assert_eq!(
///     hms(3, 5, 7) - TimeDelta::try_days(365).unwrap(),
///     from_ymd(2015, 7, 9).and_hms_opt(3, 5, 7).unwrap()
/// );
///
/// let hmsm = |h, m, s, milli| d.and_hms_milli_opt(h, m, s, milli).unwrap();
/// assert_eq!(hmsm(3, 5, 7, 450) - TimeDelta::try_milliseconds(670).unwrap(), hmsm(3, 5, 6, 780));
/// ```
///
/// Leap seconds are handled,
/// but the subtraction assumes that it is the only leap second happened.
///
/// ```
/// # use chrono::{TimeDelta, NaiveDate};
/// # let from_ymd = |y, m, d| NaiveDate::from_ymd_opt(y, m, d).unwrap();
/// # let hmsm = |h, m, s, milli| from_ymd(2016, 7, 8).and_hms_milli_opt(h, m, s, milli).unwrap();
/// let leap = hmsm(3, 5, 59, 1_300);
/// assert_eq!(leap - TimeDelta::zero(), hmsm(3, 5, 59, 1_300));
/// assert_eq!(leap - TimeDelta::try_milliseconds(200).unwrap(), hmsm(3, 5, 59, 1_100));
/// assert_eq!(leap - TimeDelta::try_milliseconds(500).unwrap(), hmsm(3, 5, 59, 800));
/// assert_eq!(leap - TimeDelta::try_seconds(60).unwrap(), hmsm(3, 5, 0, 300));
/// assert_eq!(leap - TimeDelta::try_days(1).unwrap(),
///            from_ymd(2016, 7, 7).and_hms_milli_opt(3, 6, 0, 300).unwrap());
/// ```
///
/// [leap second handling]: crate::NaiveTime#leap-second-handling
impl Sub<TimeDelta> for NaiveDateTime {
    type Output = NaiveDateTime;

    #[inline]
    fn sub(self, rhs: TimeDelta) -> NaiveDateTime {
        self.checked_sub_signed(rhs).expect("`NaiveDateTime - TimeDelta` overflowed")
    }
}

/// Subtract `std::time::Duration` from `NaiveDateTime`.
///
/// As a part of Chrono's [leap second handling] the subtraction assumes that **there is no leap
/// second ever**, except when the `NaiveDateTime` itself represents a leap second in which case
/// the assumption becomes that **there is exactly a single leap second ever**.
///
/// # Panics
///
/// Panics if the resulting date would be out of range.
/// Consider using [`NaiveDateTime::checked_sub_signed`] to get an `Option` instead.
impl Sub<Duration> for NaiveDateTime {
    type Output = NaiveDateTime;

    #[inline]
    fn sub(self, rhs: Duration) -> NaiveDateTime {
        let rhs = TimeDelta::from_std(rhs)
            .expect("overflow converting from core::time::Duration to TimeDelta");
        self.checked_sub_signed(rhs).expect("`NaiveDateTime - TimeDelta` overflowed")
    }
}

/// Subtract-assign `TimeDelta` from `NaiveDateTime`.
///
/// This is the same as the addition with a negated `TimeDelta`.
///
/// As a part of Chrono's [leap second handling], the addition assumes that **there is no leap
/// second ever**, except when the `NaiveDateTime` itself represents a leap  second in which case
/// the assumption becomes that **there is exactly a single leap second ever**.
///
/// # Panics
///
/// Panics if the resulting date would be out of range.
/// Consider using [`NaiveDateTime::checked_sub_signed`] to get an `Option` instead.
impl SubAssign<TimeDelta> for NaiveDateTime {
    #[inline]
    fn sub_assign(&mut self, rhs: TimeDelta) {
        *self = self.sub(rhs);
    }
}

/// Subtract-assign `std::time::Duration` from `NaiveDateTime`.
///
/// As a part of Chrono's [leap second handling], the addition assumes that **there is no leap
/// second ever**, except when the `NaiveDateTime` itself represents a leap  second in which case
/// the assumption becomes that **there is exactly a single leap second ever**.
///
/// # Panics
///
/// Panics if the resulting date would be out of range.
/// Consider using [`NaiveDateTime::checked_sub_signed`] to get an `Option` instead.
impl SubAssign<Duration> for NaiveDateTime {
    #[inline]
    fn sub_assign(&mut self, rhs: Duration) {
        *self = self.sub(rhs);
    }
}

/// Subtract `FixedOffset` from `NaiveDateTime`.
///
/// # Panics
///
/// Panics if the resulting date would be out of range.
/// Consider using `checked_sub_offset` to get an `Option` instead.
impl Sub<FixedOffset> for NaiveDateTime {
    type Output = NaiveDateTime;

    #[inline]
    fn sub(self, rhs: FixedOffset) -> NaiveDateTime {
        self.checked_sub_offset(rhs).expect("`NaiveDateTime - FixedOffset` out of range")
    }
}

/// Subtract `Months` from `NaiveDateTime`.
///
/// The result will be clamped to valid days in the resulting month, see
/// [`NaiveDateTime::checked_sub_months`] for details.
///
/// # Panics
///
/// Panics if the resulting date would be out of range.
/// Consider using [`NaiveDateTime::checked_sub_months`] to get an `Option` instead.
///
/// # Example
///
/// ```
/// use chrono::{Months, NaiveDate};
///
/// assert_eq!(
///     NaiveDate::from_ymd_opt(2014, 01, 01).unwrap().and_hms_opt(01, 00, 00).unwrap()
///         - Months::new(11),
///     NaiveDate::from_ymd_opt(2013, 02, 01).unwrap().and_hms_opt(01, 00, 00).unwrap()
/// );
/// assert_eq!(
///     NaiveDate::from_ymd_opt(2014, 01, 01).unwrap().and_hms_opt(00, 02, 00).unwrap()
///         - Months::new(12),
///     NaiveDate::from_ymd_opt(2013, 01, 01).unwrap().and_hms_opt(00, 02, 00).unwrap()
/// );
/// assert_eq!(
///     NaiveDate::from_ymd_opt(2014, 01, 01).unwrap().and_hms_opt(00, 00, 03).unwrap()
///         - Months::new(13),
///     NaiveDate::from_ymd_opt(2012, 12, 01).unwrap().and_hms_opt(00, 00, 03).unwrap()
/// );
/// ```
impl Sub<Months> for NaiveDateTime {
    type Output = NaiveDateTime;

    fn sub(self, rhs: Months) -> Self::Output {
        self.checked_sub_months(rhs).expect("`NaiveDateTime - Months` out of range")
    }
}

/// Subtracts another `NaiveDateTime` from the current date and time.
/// This does not overflow or underflow at all.
///
/// As a part of Chrono's [leap second handling](./struct.NaiveTime.html#leap-second-handling),
/// the subtraction assumes that **there is no leap second ever**,
/// except when any of the `NaiveDateTime`s themselves represents a leap second
/// in which case the assumption becomes that
/// **there are exactly one (or two) leap second(s) ever**.
///
/// The implementation is a wrapper around [`NaiveDateTime::signed_duration_since`].
///
/// # Example
///
/// ```
/// use chrono::{NaiveDate, TimeDelta};
///
/// let from_ymd = |y, m, d| NaiveDate::from_ymd_opt(y, m, d).unwrap();
///
/// let d = from_ymd(2016, 7, 8);
/// assert_eq!(
///     d.and_hms_opt(3, 5, 7).unwrap() - d.and_hms_opt(2, 4, 6).unwrap(),
///     TimeDelta::try_seconds(3600 + 60 + 1).unwrap()
/// );
///
/// // July 8 is 190th day in the year 2016
/// let d0 = from_ymd(2016, 1, 1);
/// assert_eq!(
///     d.and_hms_milli_opt(0, 7, 6, 500).unwrap() - d0.and_hms_opt(0, 0, 0).unwrap(),
///     TimeDelta::try_seconds(189 * 86_400 + 7 * 60 + 6).unwrap()
///         + TimeDelta::try_milliseconds(500).unwrap()
/// );
/// ```
///
/// Leap seconds are handled, but the subtraction assumes that no other leap
/// seconds happened.
///
/// ```
/// # use chrono::{TimeDelta, NaiveDate};
/// # let from_ymd = |y, m, d| NaiveDate::from_ymd_opt(y, m, d).unwrap();
/// let leap = from_ymd(2015, 6, 30).and_hms_milli_opt(23, 59, 59, 1_500).unwrap();
/// assert_eq!(
///     leap - from_ymd(2015, 6, 30).and_hms_opt(23, 0, 0).unwrap(),
///     TimeDelta::try_seconds(3600).unwrap() + TimeDelta::try_milliseconds(500).unwrap()
/// );
/// assert_eq!(
///     from_ymd(2015, 7, 1).and_hms_opt(1, 0, 0).unwrap() - leap,
///     TimeDelta::try_seconds(3600).unwrap() - TimeDelta::try_milliseconds(500).unwrap()
/// );
/// ```
impl Sub<NaiveDateTime> for NaiveDateTime {
    type Output = TimeDelta;

    #[inline]
    fn sub(self, rhs: NaiveDateTime) -> TimeDelta {
        self.signed_duration_since(rhs)
    }
}

/// Add `Days` to `NaiveDateTime`.
///
/// # Panics
///
/// Panics if the resulting date would be out of range.
/// Consider using `checked_add_days` to get an `Option` instead.
impl Add<Days> for NaiveDateTime {
    type Output = NaiveDateTime;

    fn add(self, days: Days) -> Self::Output {
        self.checked_add_days(days).expect("`NaiveDateTime + Days` out of range")
    }
}

/// Subtract `Days` from `NaiveDateTime`.
///
/// # Panics
///
/// Panics if the resulting date would be out of range.
/// Consider using `checked_sub_days` to get an `Option` instead.
impl Sub<Days> for NaiveDateTime {
    type Output = NaiveDateTime;

    fn sub(self, days: Days) -> Self::Output {
        self.checked_sub_days(days).expect("`NaiveDateTime - Days` out of range")
    }
}

/// The `Debug` output of the naive date and time `dt` is the same as
/// [`dt.format("%Y-%m-%dT%H:%M:%S%.f")`](crate::format::strftime).
///
/// The string printed can be readily parsed via the `parse` method on `str`.
///
/// It should be noted that, for leap seconds not on the minute boundary,
/// it may print a representation not distinguishable from non-leap seconds.
/// This doesn't matter in practice, since such leap seconds never happened.
/// (By the time of the first leap second on 1972-06-30,
/// every time zone offset around the world has standardized to the 5-minute alignment.)
///
/// # Example
///
/// ```
/// use chrono::NaiveDate;
///
/// let dt = NaiveDate::from_ymd_opt(2016, 11, 15).unwrap().and_hms_opt(7, 39, 24).unwrap();
/// assert_eq!(format!("{:?}", dt), "2016-11-15T07:39:24");
/// ```
///
/// Leap seconds may also be used.
///
/// ```
/// # use chrono::NaiveDate;
/// let dt =
///     NaiveDate::from_ymd_opt(2015, 6, 30).unwrap().and_hms_milli_opt(23, 59, 59, 1_500).unwrap();
/// assert_eq!(format!("{:?}", dt), "2015-06-30T23:59:60.500");
/// ```
impl fmt::Debug for NaiveDateTime {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.date.fmt(f)?;
        f.write_char('T')?;
        self.time.fmt(f)
    }
}

/// The `Display` output of the naive date and time `dt` is the same as
/// [`dt.format("%Y-%m-%d %H:%M:%S%.f")`](crate::format::strftime).
///
/// It should be noted that, for leap seconds not on the minute boundary,
/// it may print a representation not distinguishable from non-leap seconds.
/// This doesn't matter in practice, since such leap seconds never happened.
/// (By the time of the first leap second on 1972-06-30,
/// every time zone offset around the world has standardized to the 5-minute alignment.)
///
/// # Example
///
/// ```
/// use chrono::NaiveDate;
///
/// let dt = NaiveDate::from_ymd_opt(2016, 11, 15).unwrap().and_hms_opt(7, 39, 24).unwrap();
/// assert_eq!(format!("{}", dt), "2016-11-15 07:39:24");
/// ```
///
/// Leap seconds may also be used.
///
/// ```
/// # use chrono::NaiveDate;
/// let dt =
///     NaiveDate::from_ymd_opt(2015, 6, 30).unwrap().and_hms_milli_opt(23, 59, 59, 1_500).unwrap();
/// assert_eq!(format!("{}", dt), "2015-06-30 23:59:60.500");
/// ```
impl fmt::Display for NaiveDateTime {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.date.fmt(f)?;
        f.write_char(' ')?;
        self.time.fmt(f)
    }
}

/// Parsing a `str` into a `NaiveDateTime` uses the same format,
/// [`%Y-%m-%dT%H:%M:%S%.f`](crate::format::strftime), as in `Debug`.
///
/// # Example
///
/// ```
/// use chrono::{NaiveDateTime, NaiveDate};
///
/// let dt = NaiveDate::from_ymd_opt(2015, 9, 18).unwrap().and_hms_opt(23, 56, 4).unwrap();
/// assert_eq!("2015-09-18T23:56:04".parse::<NaiveDateTime>(), Ok(dt));
///
/// let dt = NaiveDate::from_ymd_opt(12345, 6, 7).unwrap().and_hms_milli_opt(7, 59, 59, 1_500).unwrap(); // leap second
/// assert_eq!("+12345-6-7T7:59:60.5".parse::<NaiveDateTime>(), Ok(dt));
///
/// assert!("foo".parse::<NaiveDateTime>().is_err());
/// ```
impl str::FromStr for NaiveDateTime {
    type Err = ParseError;

    fn from_str(s: &str) -> ParseResult<NaiveDateTime> {
        const ITEMS: &[Item<'static>] = &[
            Item::Numeric(Numeric::Year, Pad::Zero),
            Item::Space(""),
            Item::Literal("-"),
            Item::Numeric(Numeric::Month, Pad::Zero),
            Item::Space(""),
            Item::Literal("-"),
            Item::Numeric(Numeric::Day, Pad::Zero),
            Item::Space(""),
            Item::Literal("T"), // XXX shouldn't this be case-insensitive?
            Item::Numeric(Numeric::Hour, Pad::Zero),
            Item::Space(""),
            Item::Literal(":"),
            Item::Numeric(Numeric::Minute, Pad::Zero),
            Item::Space(""),
            Item::Literal(":"),
            Item::Numeric(Numeric::Second, Pad::Zero),
            Item::Fixed(Fixed::Nanosecond),
            Item::Space(""),
        ];

        let mut parsed = Parsed::new();
        parse(&mut parsed, s, ITEMS.iter())?;
        parsed.to_naive_datetime_with_offset(0)
    }
}

/// The default value for a NaiveDateTime is 1st of January 1970 at 00:00:00.
///
/// Note that while this may look like the UNIX epoch, it is missing the
/// time zone. The actual UNIX epoch cannot be expressed by this type,
/// however it is available as [`DateTime::UNIX_EPOCH`].
impl Default for NaiveDateTime {
    fn default() -> Self {
        DateTime::UNIX_EPOCH.naive_local()
    }
}
