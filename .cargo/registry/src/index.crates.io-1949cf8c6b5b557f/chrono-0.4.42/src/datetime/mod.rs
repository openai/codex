// This is a part of Chrono.
// See README.md and LICENSE.txt for details.

//! ISO 8601 date and time with time zone.

#[cfg(all(feature = "alloc", not(feature = "std"), not(test)))]
use alloc::string::String;
use core::borrow::Borrow;
use core::cmp::Ordering;
use core::fmt::Write;
use core::ops::{Add, AddAssign, Sub, SubAssign};
use core::time::Duration;
use core::{fmt, hash, str};
#[cfg(feature = "std")]
use std::time::{SystemTime, UNIX_EPOCH};

#[allow(deprecated)]
use crate::Date;
#[cfg(all(feature = "unstable-locales", feature = "alloc"))]
use crate::format::Locale;
#[cfg(feature = "alloc")]
use crate::format::{DelayedFormat, SecondsFormat, write_rfc2822, write_rfc3339};
use crate::format::{
    Fixed, Item, ParseError, ParseResult, Parsed, StrftimeItems, TOO_LONG, parse,
    parse_and_remainder, parse_rfc3339,
};
use crate::naive::{Days, IsoWeek, NaiveDate, NaiveDateTime, NaiveTime};
#[cfg(feature = "clock")]
use crate::offset::Local;
use crate::offset::{FixedOffset, LocalResult, Offset, TimeZone, Utc};
use crate::{Datelike, Months, TimeDelta, Timelike, Weekday};
use crate::{expect, try_opt};

#[cfg(any(feature = "rkyv", feature = "rkyv-16", feature = "rkyv-32", feature = "rkyv-64"))]
use rkyv::{Archive, Deserialize, Serialize};

/// documented at re-export site
#[cfg(feature = "serde")]
pub(super) mod serde;

#[cfg(test)]
mod tests;

/// ISO 8601 combined date and time with time zone.
///
/// There are some constructors implemented here (the `from_*` methods), but
/// the general-purpose constructors are all via the methods on the
/// [`TimeZone`](./offset/trait.TimeZone.html) implementations.
#[derive(Clone)]
#[cfg_attr(
    any(feature = "rkyv", feature = "rkyv-16", feature = "rkyv-32", feature = "rkyv-64"),
    derive(Archive, Deserialize, Serialize),
    archive(compare(PartialEq, PartialOrd))
)]
#[cfg_attr(feature = "rkyv-validation", archive(check_bytes))]
pub struct DateTime<Tz: TimeZone> {
    datetime: NaiveDateTime,
    offset: Tz::Offset,
}

/// The minimum possible `DateTime<Utc>`.
#[deprecated(since = "0.4.20", note = "Use DateTime::MIN_UTC instead")]
pub const MIN_DATETIME: DateTime<Utc> = DateTime::<Utc>::MIN_UTC;
/// The maximum possible `DateTime<Utc>`.
#[deprecated(since = "0.4.20", note = "Use DateTime::MAX_UTC instead")]
pub const MAX_DATETIME: DateTime<Utc> = DateTime::<Utc>::MAX_UTC;

impl<Tz: TimeZone> DateTime<Tz> {
    /// Makes a new `DateTime` from its components: a `NaiveDateTime` in UTC and an `Offset`.
    ///
    /// This is a low-level method, intended for use cases such as deserializing a `DateTime` or
    /// passing it through FFI.
    ///
    /// For regular use you will probably want to use a method such as
    /// [`TimeZone::from_local_datetime`] or [`NaiveDateTime::and_local_timezone`] instead.
    ///
    /// # Example
    ///
    /// ```
    /// # #[cfg(feature = "clock")] {
    /// use chrono::{DateTime, Local};
    ///
    /// let dt = Local::now();
    /// // Get components
    /// let naive_utc = dt.naive_utc();
    /// let offset = dt.offset().clone();
    /// // Serialize, pass through FFI... and recreate the `DateTime`:
    /// let dt_new = DateTime::<Local>::from_naive_utc_and_offset(naive_utc, offset);
    /// assert_eq!(dt, dt_new);
    /// # }
    /// ```
    #[inline]
    #[must_use]
    pub const fn from_naive_utc_and_offset(
        datetime: NaiveDateTime,
        offset: Tz::Offset,
    ) -> DateTime<Tz> {
        DateTime { datetime, offset }
    }

    /// Makes a new `DateTime` from its components: a `NaiveDateTime` in UTC and an `Offset`.
    #[inline]
    #[must_use]
    #[deprecated(
        since = "0.4.27",
        note = "Use TimeZone::from_utc_datetime() or DateTime::from_naive_utc_and_offset instead"
    )]
    pub fn from_utc(datetime: NaiveDateTime, offset: Tz::Offset) -> DateTime<Tz> {
        DateTime { datetime, offset }
    }

    /// Makes a new `DateTime` from a `NaiveDateTime` in *local* time and an `Offset`.
    ///
    /// # Panics
    ///
    /// Panics if the local datetime can't be converted to UTC because it would be out of range.
    ///
    /// This can happen if `datetime` is near the end of the representable range of `NaiveDateTime`,
    /// and the offset from UTC pushes it beyond that.
    #[inline]
    #[must_use]
    #[deprecated(
        since = "0.4.27",
        note = "Use TimeZone::from_local_datetime() or NaiveDateTime::and_local_timezone instead"
    )]
    pub fn from_local(datetime: NaiveDateTime, offset: Tz::Offset) -> DateTime<Tz> {
        let datetime_utc = datetime - offset.fix();

        DateTime { datetime: datetime_utc, offset }
    }

    /// Retrieves the date component with an associated timezone.
    ///
    /// Unless you are immediately planning on turning this into a `DateTime`
    /// with the same timezone you should use the [`date_naive`](DateTime::date_naive) method.
    ///
    /// [`NaiveDate`] is a more well-defined type, and has more traits implemented on it,
    /// so should be preferred to [`Date`] any time you truly want to operate on dates.
    ///
    /// # Panics
    ///
    /// [`DateTime`] internally stores the date and time in UTC with a [`NaiveDateTime`]. This
    /// method will panic if the offset from UTC would push the local date outside of the
    /// representable range of a [`Date`].
    #[inline]
    #[deprecated(since = "0.4.23", note = "Use `date_naive()` instead")]
    #[allow(deprecated)]
    #[must_use]
    pub fn date(&self) -> Date<Tz> {
        Date::from_utc(self.naive_local().date(), self.offset.clone())
    }

    /// Retrieves the date component.
    ///
    /// # Panics
    ///
    /// [`DateTime`] internally stores the date and time in UTC with a [`NaiveDateTime`]. This
    /// method will panic if the offset from UTC would push the local date outside of the
    /// representable range of a [`NaiveDate`].
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::prelude::*;
    ///
    /// let date: DateTime<Utc> = Utc.with_ymd_and_hms(2020, 1, 1, 0, 0, 0).unwrap();
    /// let other: DateTime<FixedOffset> =
    ///     FixedOffset::east_opt(23).unwrap().with_ymd_and_hms(2020, 1, 1, 0, 0, 0).unwrap();
    /// assert_eq!(date.date_naive(), other.date_naive());
    /// ```
    #[inline]
    #[must_use]
    pub fn date_naive(&self) -> NaiveDate {
        self.naive_local().date()
    }

    /// Retrieves the time component.
    #[inline]
    #[must_use]
    pub fn time(&self) -> NaiveTime {
        self.datetime.time() + self.offset.fix()
    }

    /// Returns the number of non-leap seconds since January 1, 1970 0:00:00 UTC
    /// (aka "UNIX timestamp").
    ///
    /// The reverse operation of creating a [`DateTime`] from a timestamp can be performed
    /// using [`from_timestamp`](DateTime::from_timestamp) or [`TimeZone::timestamp_opt`].
    ///
    /// ```
    /// use chrono::{DateTime, TimeZone, Utc};
    ///
    /// let dt: DateTime<Utc> = Utc.with_ymd_and_hms(2015, 5, 15, 0, 0, 0).unwrap();
    /// assert_eq!(dt.timestamp(), 1431648000);
    ///
    /// assert_eq!(DateTime::from_timestamp(dt.timestamp(), dt.timestamp_subsec_nanos()).unwrap(), dt);
    /// ```
    #[inline]
    #[must_use]
    pub const fn timestamp(&self) -> i64 {
        let gregorian_day = self.datetime.date().num_days_from_ce() as i64;
        let seconds_from_midnight = self.datetime.time().num_seconds_from_midnight() as i64;
        (gregorian_day - UNIX_EPOCH_DAY) * 86_400 + seconds_from_midnight
    }

    /// Returns the number of non-leap-milliseconds since January 1, 1970 UTC.
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::{NaiveDate, Utc};
    ///
    /// let dt = NaiveDate::from_ymd_opt(1970, 1, 1)
    ///     .unwrap()
    ///     .and_hms_milli_opt(0, 0, 1, 444)
    ///     .unwrap()
    ///     .and_local_timezone(Utc)
    ///     .unwrap();
    /// assert_eq!(dt.timestamp_millis(), 1_444);
    ///
    /// let dt = NaiveDate::from_ymd_opt(2001, 9, 9)
    ///     .unwrap()
    ///     .and_hms_milli_opt(1, 46, 40, 555)
    ///     .unwrap()
    ///     .and_local_timezone(Utc)
    ///     .unwrap();
    /// assert_eq!(dt.timestamp_millis(), 1_000_000_000_555);
    /// ```
    #[inline]
    #[must_use]
    pub const fn timestamp_millis(&self) -> i64 {
        let as_ms = self.timestamp() * 1000;
        as_ms + self.timestamp_subsec_millis() as i64
    }

    /// Returns the number of non-leap-microseconds since January 1, 1970 UTC.
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::{NaiveDate, Utc};
    ///
    /// let dt = NaiveDate::from_ymd_opt(1970, 1, 1)
    ///     .unwrap()
    ///     .and_hms_micro_opt(0, 0, 1, 444)
    ///     .unwrap()
    ///     .and_local_timezone(Utc)
    ///     .unwrap();
    /// assert_eq!(dt.timestamp_micros(), 1_000_444);
    ///
    /// let dt = NaiveDate::from_ymd_opt(2001, 9, 9)
    ///     .unwrap()
    ///     .and_hms_micro_opt(1, 46, 40, 555)
    ///     .unwrap()
    ///     .and_local_timezone(Utc)
    ///     .unwrap();
    /// assert_eq!(dt.timestamp_micros(), 1_000_000_000_000_555);
    /// ```
    #[inline]
    #[must_use]
    pub const fn timestamp_micros(&self) -> i64 {
        let as_us = self.timestamp() * 1_000_000;
        as_us + self.timestamp_subsec_micros() as i64
    }

    /// Returns the number of non-leap-nanoseconds since January 1, 1970 UTC.
    ///
    /// # Panics
    ///
    /// An `i64` with nanosecond precision can span a range of ~584 years. This function panics on
    /// an out of range `DateTime`.
    ///
    /// The dates that can be represented as nanoseconds are between 1677-09-21T00:12:43.145224192
    /// and 2262-04-11T23:47:16.854775807.
    #[deprecated(since = "0.4.31", note = "use `timestamp_nanos_opt()` instead")]
    #[inline]
    #[must_use]
    pub const fn timestamp_nanos(&self) -> i64 {
        expect(
            self.timestamp_nanos_opt(),
            "value can not be represented in a timestamp with nanosecond precision.",
        )
    }

    /// Returns the number of non-leap-nanoseconds since January 1, 1970 UTC.
    ///
    /// # Errors
    ///
    /// An `i64` with nanosecond precision can span a range of ~584 years. This function returns
    /// `None` on an out of range `DateTime`.
    ///
    /// The dates that can be represented as nanoseconds are between 1677-09-21T00:12:43.145224192
    /// and 2262-04-11T23:47:16.854775807.
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::{NaiveDate, Utc};
    ///
    /// let dt = NaiveDate::from_ymd_opt(1970, 1, 1)
    ///     .unwrap()
    ///     .and_hms_nano_opt(0, 0, 1, 444)
    ///     .unwrap()
    ///     .and_local_timezone(Utc)
    ///     .unwrap();
    /// assert_eq!(dt.timestamp_nanos_opt(), Some(1_000_000_444));
    ///
    /// let dt = NaiveDate::from_ymd_opt(2001, 9, 9)
    ///     .unwrap()
    ///     .and_hms_nano_opt(1, 46, 40, 555)
    ///     .unwrap()
    ///     .and_local_timezone(Utc)
    ///     .unwrap();
    /// assert_eq!(dt.timestamp_nanos_opt(), Some(1_000_000_000_000_000_555));
    ///
    /// let dt = NaiveDate::from_ymd_opt(1677, 9, 21)
    ///     .unwrap()
    ///     .and_hms_nano_opt(0, 12, 43, 145_224_192)
    ///     .unwrap()
    ///     .and_local_timezone(Utc)
    ///     .unwrap();
    /// assert_eq!(dt.timestamp_nanos_opt(), Some(-9_223_372_036_854_775_808));
    ///
    /// let dt = NaiveDate::from_ymd_opt(2262, 4, 11)
    ///     .unwrap()
    ///     .and_hms_nano_opt(23, 47, 16, 854_775_807)
    ///     .unwrap()
    ///     .and_local_timezone(Utc)
    ///     .unwrap();
    /// assert_eq!(dt.timestamp_nanos_opt(), Some(9_223_372_036_854_775_807));
    ///
    /// let dt = NaiveDate::from_ymd_opt(1677, 9, 21)
    ///     .unwrap()
    ///     .and_hms_nano_opt(0, 12, 43, 145_224_191)
    ///     .unwrap()
    ///     .and_local_timezone(Utc)
    ///     .unwrap();
    /// assert_eq!(dt.timestamp_nanos_opt(), None);
    ///
    /// let dt = NaiveDate::from_ymd_opt(2262, 4, 11)
    ///     .unwrap()
    ///     .and_hms_nano_opt(23, 47, 16, 854_775_808)
    ///     .unwrap()
    ///     .and_local_timezone(Utc)
    ///     .unwrap();
    /// assert_eq!(dt.timestamp_nanos_opt(), None);
    /// ```
    #[inline]
    #[must_use]
    pub const fn timestamp_nanos_opt(&self) -> Option<i64> {
        let mut timestamp = self.timestamp();
        let mut subsec_nanos = self.timestamp_subsec_nanos() as i64;
        // `(timestamp * 1_000_000_000) + subsec_nanos` may create a temporary that underflows while
        // the final value can be represented as an `i64`.
        // As workaround we converting the negative case to:
        // `((timestamp + 1) * 1_000_000_000) + (ns - 1_000_000_000)``
        //
        // Also see <https://github.com/chronotope/chrono/issues/1289>.
        if timestamp < 0 {
            subsec_nanos -= 1_000_000_000;
            timestamp += 1;
        }
        try_opt!(timestamp.checked_mul(1_000_000_000)).checked_add(subsec_nanos)
    }

    /// Returns the number of milliseconds since the last second boundary.
    ///
    /// In event of a leap second this may exceed 999.
    #[inline]
    #[must_use]
    pub const fn timestamp_subsec_millis(&self) -> u32 {
        self.timestamp_subsec_nanos() / 1_000_000
    }

    /// Returns the number of microseconds since the last second boundary.
    ///
    /// In event of a leap second this may exceed 999,999.
    #[inline]
    #[must_use]
    pub const fn timestamp_subsec_micros(&self) -> u32 {
        self.timestamp_subsec_nanos() / 1_000
    }

    /// Returns the number of nanoseconds since the last second boundary
    ///
    /// In event of a leap second this may exceed 999,999,999.
    #[inline]
    #[must_use]
    pub const fn timestamp_subsec_nanos(&self) -> u32 {
        self.datetime.time().nanosecond()
    }

    /// Retrieves an associated offset from UTC.
    #[inline]
    #[must_use]
    pub const fn offset(&self) -> &Tz::Offset {
        &self.offset
    }

    /// Retrieves an associated time zone.
    #[inline]
    #[must_use]
    pub fn timezone(&self) -> Tz {
        TimeZone::from_offset(&self.offset)
    }

    /// Changes the associated time zone.
    /// The returned `DateTime` references the same instant of time from the perspective of the
    /// provided time zone.
    #[inline]
    #[must_use]
    pub fn with_timezone<Tz2: TimeZone>(&self, tz: &Tz2) -> DateTime<Tz2> {
        tz.from_utc_datetime(&self.datetime)
    }

    /// Fix the offset from UTC to its current value, dropping the associated timezone information.
    /// This it useful for converting a generic `DateTime<Tz: Timezone>` to `DateTime<FixedOffset>`.
    #[inline]
    #[must_use]
    pub fn fixed_offset(&self) -> DateTime<FixedOffset> {
        self.with_timezone(&self.offset().fix())
    }

    /// Turn this `DateTime` into a `DateTime<Utc>`, dropping the offset and associated timezone
    /// information.
    #[inline]
    #[must_use]
    pub const fn to_utc(&self) -> DateTime<Utc> {
        DateTime { datetime: self.datetime, offset: Utc }
    }

    /// Adds given `TimeDelta` to the current date and time.
    ///
    /// # Errors
    ///
    /// Returns `None` if the resulting date would be out of range.
    #[inline]
    #[must_use]
    pub fn checked_add_signed(self, rhs: TimeDelta) -> Option<DateTime<Tz>> {
        let datetime = self.datetime.checked_add_signed(rhs)?;
        let tz = self.timezone();
        Some(tz.from_utc_datetime(&datetime))
    }

    /// Adds given `Months` to the current date and time.
    ///
    /// Uses the last day of the month if the day does not exist in the resulting month.
    ///
    /// See [`NaiveDate::checked_add_months`] for more details on behavior.
    ///
    /// # Errors
    ///
    /// Returns `None` if:
    /// - The local time at the resulting date does not exist or is ambiguous, for example during a
    ///   daylight saving time transition.
    /// - The resulting UTC datetime would be out of range.
    /// - The resulting local datetime would be out of range (unless `months` is zero).
    #[must_use]
    pub fn checked_add_months(self, months: Months) -> Option<DateTime<Tz>> {
        // `NaiveDate::checked_add_months` has a fast path for `Months(0)` that does not validate
        // the resulting date, with which we can return `Some` even for an out of range local
        // datetime.
        self.overflowing_naive_local()
            .checked_add_months(months)?
            .and_local_timezone(Tz::from_offset(&self.offset))
            .single()
    }

    /// Subtracts given `TimeDelta` from the current date and time.
    ///
    /// # Errors
    ///
    /// Returns `None` if the resulting date would be out of range.
    #[inline]
    #[must_use]
    pub fn checked_sub_signed(self, rhs: TimeDelta) -> Option<DateTime<Tz>> {
        let datetime = self.datetime.checked_sub_signed(rhs)?;
        let tz = self.timezone();
        Some(tz.from_utc_datetime(&datetime))
    }

    /// Subtracts given `Months` from the current date and time.
    ///
    /// Uses the last day of the month if the day does not exist in the resulting month.
    ///
    /// See [`NaiveDate::checked_sub_months`] for more details on behavior.
    ///
    /// # Errors
    ///
    /// Returns `None` if:
    /// - The local time at the resulting date does not exist or is ambiguous, for example during a
    ///   daylight saving time transition.
    /// - The resulting UTC datetime would be out of range.
    /// - The resulting local datetime would be out of range (unless `months` is zero).
    #[must_use]
    pub fn checked_sub_months(self, months: Months) -> Option<DateTime<Tz>> {
        // `NaiveDate::checked_sub_months` has a fast path for `Months(0)` that does not validate
        // the resulting date, with which we can return `Some` even for an out of range local
        // datetime.
        self.overflowing_naive_local()
            .checked_sub_months(months)?
            .and_local_timezone(Tz::from_offset(&self.offset))
            .single()
    }

    /// Add a duration in [`Days`] to the date part of the `DateTime`.
    ///
    /// # Errors
    ///
    /// Returns `None` if:
    /// - The local time at the resulting date does not exist or is ambiguous, for example during a
    ///   daylight saving time transition.
    /// - The resulting UTC datetime would be out of range.
    /// - The resulting local datetime would be out of range (unless `days` is zero).
    #[must_use]
    pub fn checked_add_days(self, days: Days) -> Option<Self> {
        if days == Days::new(0) {
            return Some(self);
        }
        // `NaiveDate::add_days` has a fast path if the result remains within the same year, that
        // does not validate the resulting date. This allows us to return `Some` even for an out of
        // range local datetime when adding `Days(0)`.
        self.overflowing_naive_local()
            .checked_add_days(days)
            .and_then(|dt| self.timezone().from_local_datetime(&dt).single())
            .filter(|dt| dt <= &DateTime::<Utc>::MAX_UTC)
    }

    /// Subtract a duration in [`Days`] from the date part of the `DateTime`.
    ///
    /// # Errors
    ///
    /// Returns `None` if:
    /// - The local time at the resulting date does not exist or is ambiguous, for example during a
    ///   daylight saving time transition.
    /// - The resulting UTC datetime would be out of range.
    /// - The resulting local datetime would be out of range (unless `days` is zero).
    #[must_use]
    pub fn checked_sub_days(self, days: Days) -> Option<Self> {
        // `NaiveDate::add_days` has a fast path if the result remains within the same year, that
        // does not validate the resulting date. This allows us to return `Some` even for an out of
        // range local datetime when adding `Days(0)`.
        self.overflowing_naive_local()
            .checked_sub_days(days)
            .and_then(|dt| self.timezone().from_local_datetime(&dt).single())
            .filter(|dt| dt >= &DateTime::<Utc>::MIN_UTC)
    }

    /// Subtracts another `DateTime` from the current date and time.
    /// This does not overflow or underflow at all.
    #[inline]
    #[must_use]
    pub fn signed_duration_since<Tz2: TimeZone>(
        self,
        rhs: impl Borrow<DateTime<Tz2>>,
    ) -> TimeDelta {
        self.datetime.signed_duration_since(rhs.borrow().datetime)
    }

    /// Returns a view to the naive UTC datetime.
    #[inline]
    #[must_use]
    pub const fn naive_utc(&self) -> NaiveDateTime {
        self.datetime
    }

    /// Returns a view to the naive local datetime.
    ///
    /// # Panics
    ///
    /// [`DateTime`] internally stores the date and time in UTC with a [`NaiveDateTime`]. This
    /// method will panic if the offset from UTC would push the local datetime outside of the
    /// representable range of a [`NaiveDateTime`].
    #[inline]
    #[must_use]
    pub fn naive_local(&self) -> NaiveDateTime {
        self.datetime
            .checked_add_offset(self.offset.fix())
            .expect("Local time out of range for `NaiveDateTime`")
    }

    /// Returns the naive local datetime.
    ///
    /// This makes use of the buffer space outside of the representable range of values of
    /// `NaiveDateTime`. The result can be used as intermediate value, but should never be exposed
    /// outside chrono.
    #[inline]
    #[must_use]
    pub(crate) fn overflowing_naive_local(&self) -> NaiveDateTime {
        self.datetime.overflowing_add_offset(self.offset.fix())
    }

    /// Retrieve the elapsed years from now to the given [`DateTime`].
    ///
    /// # Errors
    ///
    /// Returns `None` if `base > self`.
    #[must_use]
    pub fn years_since(&self, base: Self) -> Option<u32> {
        let mut years = self.year() - base.year();
        let earlier_time =
            (self.month(), self.day(), self.time()) < (base.month(), base.day(), base.time());

        years -= match earlier_time {
            true => 1,
            false => 0,
        };

        match years >= 0 {
            true => Some(years as u32),
            false => None,
        }
    }

    /// Returns an RFC 2822 date and time string such as `Tue, 1 Jul 2003 10:52:37 +0200`.
    ///
    /// # Panics
    ///
    /// Panics if the date can not be represented in this format: the year may not be negative and
    /// can not have more than 4 digits.
    #[cfg(feature = "alloc")]
    #[must_use]
    pub fn to_rfc2822(&self) -> String {
        let mut result = String::with_capacity(32);
        write_rfc2822(&mut result, self.overflowing_naive_local(), self.offset.fix())
            .expect("writing rfc2822 datetime to string should never fail");
        result
    }

    /// Returns an RFC 3339 and ISO 8601 date and time string such as `1996-12-19T16:39:57-08:00`.
    #[cfg(feature = "alloc")]
    #[must_use]
    pub fn to_rfc3339(&self) -> String {
        // For some reason a string with a capacity less than 32 is ca 20% slower when benchmarking.
        let mut result = String::with_capacity(32);
        let naive = self.overflowing_naive_local();
        let offset = self.offset.fix();
        write_rfc3339(&mut result, naive, offset, SecondsFormat::AutoSi, false)
            .expect("writing rfc3339 datetime to string should never fail");
        result
    }

    /// Return an RFC 3339 and ISO 8601 date and time string with subseconds
    /// formatted as per `SecondsFormat`.
    ///
    /// If `use_z` is true and the timezone is UTC (offset 0), uses `Z` as
    /// per [`Fixed::TimezoneOffsetColonZ`]. If `use_z` is false, uses
    /// [`Fixed::TimezoneOffsetColon`]
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use chrono::{FixedOffset, SecondsFormat, TimeZone, NaiveDate};
    /// let dt = NaiveDate::from_ymd_opt(2018, 1, 26)
    ///     .unwrap()
    ///     .and_hms_micro_opt(18, 30, 9, 453_829)
    ///     .unwrap()
    ///     .and_utc();
    /// assert_eq!(dt.to_rfc3339_opts(SecondsFormat::Millis, false), "2018-01-26T18:30:09.453+00:00");
    /// assert_eq!(dt.to_rfc3339_opts(SecondsFormat::Millis, true), "2018-01-26T18:30:09.453Z");
    /// assert_eq!(dt.to_rfc3339_opts(SecondsFormat::Secs, true), "2018-01-26T18:30:09Z");
    ///
    /// let pst = FixedOffset::east_opt(8 * 60 * 60).unwrap();
    /// let dt = pst
    ///     .from_local_datetime(
    ///         &NaiveDate::from_ymd_opt(2018, 1, 26)
    ///             .unwrap()
    ///             .and_hms_micro_opt(10, 30, 9, 453_829)
    ///             .unwrap(),
    ///     )
    ///     .unwrap();
    /// assert_eq!(dt.to_rfc3339_opts(SecondsFormat::Secs, true), "2018-01-26T10:30:09+08:00");
    /// ```
    #[cfg(feature = "alloc")]
    #[must_use]
    pub fn to_rfc3339_opts(&self, secform: SecondsFormat, use_z: bool) -> String {
        let mut result = String::with_capacity(38);
        write_rfc3339(&mut result, self.naive_local(), self.offset.fix(), secform, use_z)
            .expect("writing rfc3339 datetime to string should never fail");
        result
    }

    /// Set the time to a new fixed time on the existing date.
    ///
    /// # Errors
    ///
    /// Returns `LocalResult::None` if the datetime is at the edge of the representable range for a
    /// `DateTime`, and `with_time` would push the value in UTC out of range.
    ///
    /// # Example
    ///
    /// ```
    /// # #[cfg(feature = "clock")] {
    /// use chrono::{Local, NaiveTime};
    ///
    /// let noon = NaiveTime::from_hms_opt(12, 0, 0).unwrap();
    /// let today_noon = Local::now().with_time(noon);
    /// let today_midnight = Local::now().with_time(NaiveTime::MIN);
    ///
    /// assert_eq!(today_noon.single().unwrap().time(), noon);
    /// assert_eq!(today_midnight.single().unwrap().time(), NaiveTime::MIN);
    /// # }
    /// ```
    #[must_use]
    pub fn with_time(&self, time: NaiveTime) -> LocalResult<Self> {
        self.timezone().from_local_datetime(&self.overflowing_naive_local().date().and_time(time))
    }

    /// The minimum possible `DateTime<Utc>`.
    pub const MIN_UTC: DateTime<Utc> = DateTime { datetime: NaiveDateTime::MIN, offset: Utc };
    /// The maximum possible `DateTime<Utc>`.
    pub const MAX_UTC: DateTime<Utc> = DateTime { datetime: NaiveDateTime::MAX, offset: Utc };
}

impl DateTime<Utc> {
    /// Makes a new `DateTime<Utc>` from the number of non-leap seconds
    /// since January 1, 1970 0:00:00 UTC (aka "UNIX timestamp").
    ///
    /// This is a convenience wrapper around [`DateTime::from_timestamp`],
    /// which is useful in functions like [`Iterator::map`] to avoid a closure.
    ///
    /// This is guaranteed to round-trip with regard to [`timestamp`](DateTime::timestamp).
    ///
    /// If you need to create a `DateTime` with a [`TimeZone`] different from [`Utc`], use
    /// [`TimeZone::timestamp_opt`] or [`DateTime::with_timezone`]; if you need to create a
    /// `DateTime` with more precision, use [`DateTime::from_timestamp_micros`],
    /// [`DateTime::from_timestamp_millis`], or [`DateTime::from_timestamp_nanos`].
    ///
    /// # Errors
    ///
    /// Returns `None` on out-of-range number of seconds,
    /// otherwise returns `Some(DateTime {...})`.
    ///
    /// # Examples
    ///
    /// Using [`Option::and_then`]:
    ///
    /// ```
    /// # use chrono::DateTime;
    /// let maybe_timestamp: Option<i64> = Some(1431648000);
    /// let maybe_dt = maybe_timestamp.and_then(DateTime::from_timestamp_secs);
    ///
    /// assert!(maybe_dt.is_some());
    /// assert_eq!(maybe_dt.unwrap().to_string(), "2015-05-15 00:00:00 UTC");
    /// ```
    ///
    /// Using [`Iterator::map`]:
    ///
    /// ```
    /// # use chrono::{DateTime, Utc};
    /// let v = vec![i64::MIN, 1_000_000_000, 1_234_567_890, i64::MAX];
    /// let timestamps: Vec<Option<DateTime<Utc>>> = v
    ///     .into_iter()
    ///     .map(DateTime::from_timestamp_secs)
    ///     .collect();
    ///
    /// assert_eq!(vec![
    ///     None,
    ///     Some(DateTime::parse_from_rfc3339("2001-09-09 01:46:40Z").unwrap().to_utc()),
    ///     Some(DateTime::parse_from_rfc3339("2009-02-13 23:31:30Z").unwrap().to_utc()),
    ///     None,
    /// ], timestamps);
    /// ```
    ///
    #[inline]
    #[must_use]
    pub const fn from_timestamp_secs(secs: i64) -> Option<Self> {
        Self::from_timestamp(secs, 0)
    }

    /// Makes a new `DateTime<Utc>` from the number of non-leap seconds
    /// since January 1, 1970 0:00:00 UTC (aka "UNIX timestamp")
    /// and the number of nanoseconds since the last whole non-leap second.
    ///
    /// This is guaranteed to round-trip with regard to [`timestamp`](DateTime::timestamp) and
    /// [`timestamp_subsec_nanos`](DateTime::timestamp_subsec_nanos).
    ///
    /// If you need to create a `DateTime` with a [`TimeZone`] different from [`Utc`], use
    /// [`TimeZone::timestamp_opt`] or [`DateTime::with_timezone`].
    ///
    /// The nanosecond part can exceed 1,000,000,000 in order to represent a
    /// [leap second](NaiveTime#leap-second-handling), but only when `secs % 60 == 59`.
    /// (The true "UNIX timestamp" cannot represent a leap second unambiguously.)
    ///
    /// # Errors
    ///
    /// Returns `None` on out-of-range number of seconds and/or
    /// invalid nanosecond, otherwise returns `Some(DateTime {...})`.
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::DateTime;
    ///
    /// let dt = DateTime::from_timestamp(1431648000, 0).expect("invalid timestamp");
    ///
    /// assert_eq!(dt.to_string(), "2015-05-15 00:00:00 UTC");
    /// assert_eq!(DateTime::from_timestamp(dt.timestamp(), dt.timestamp_subsec_nanos()).unwrap(), dt);
    /// ```
    #[inline]
    #[must_use]
    pub const fn from_timestamp(secs: i64, nsecs: u32) -> Option<Self> {
        let days = secs.div_euclid(86_400) + UNIX_EPOCH_DAY;
        let secs = secs.rem_euclid(86_400);
        if days < i32::MIN as i64 || days > i32::MAX as i64 {
            return None;
        }
        let date = try_opt!(NaiveDate::from_num_days_from_ce_opt(days as i32));
        let time = try_opt!(NaiveTime::from_num_seconds_from_midnight_opt(secs as u32, nsecs));
        Some(date.and_time(time).and_utc())
    }

    /// Makes a new `DateTime<Utc>` from the number of non-leap milliseconds
    /// since January 1, 1970 0:00:00.000 UTC (aka "UNIX timestamp").
    ///
    /// This is guaranteed to round-trip with [`timestamp_millis`](DateTime::timestamp_millis).
    ///
    /// If you need to create a `DateTime` with a [`TimeZone`] different from [`Utc`], use
    /// [`TimeZone::timestamp_millis_opt`] or [`DateTime::with_timezone`].
    ///
    /// # Errors
    ///
    /// Returns `None` on out-of-range number of milliseconds, otherwise returns `Some(DateTime {...})`.
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::DateTime;
    ///
    /// let dt = DateTime::from_timestamp_millis(947638923004).expect("invalid timestamp");
    ///
    /// assert_eq!(dt.to_string(), "2000-01-12 01:02:03.004 UTC");
    /// assert_eq!(DateTime::from_timestamp_millis(dt.timestamp_millis()).unwrap(), dt);
    /// ```
    #[inline]
    #[must_use]
    pub const fn from_timestamp_millis(millis: i64) -> Option<Self> {
        let secs = millis.div_euclid(1000);
        let nsecs = millis.rem_euclid(1000) as u32 * 1_000_000;
        Self::from_timestamp(secs, nsecs)
    }

    /// Creates a new `DateTime<Utc>` from the number of non-leap microseconds
    /// since January 1, 1970 0:00:00.000 UTC (aka "UNIX timestamp").
    ///
    /// This is guaranteed to round-trip with [`timestamp_micros`](DateTime::timestamp_micros).
    ///
    /// If you need to create a `DateTime` with a [`TimeZone`] different from [`Utc`], use
    /// [`TimeZone::timestamp_micros`] or [`DateTime::with_timezone`].
    ///
    /// # Errors
    ///
    /// Returns `None` if the number of microseconds would be out of range for a `NaiveDateTime`
    /// (more than ca. 262,000 years away from common era)
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::DateTime;
    ///
    /// let timestamp_micros: i64 = 1662921288000000; // Sun, 11 Sep 2022 18:34:48 UTC
    /// let dt = DateTime::from_timestamp_micros(timestamp_micros);
    /// assert!(dt.is_some());
    /// assert_eq!(timestamp_micros, dt.expect("invalid timestamp").timestamp_micros());
    ///
    /// // Negative timestamps (before the UNIX epoch) are supported as well.
    /// let timestamp_micros: i64 = -2208936075000000; // Mon, 1 Jan 1900 14:38:45 UTC
    /// let dt = DateTime::from_timestamp_micros(timestamp_micros);
    /// assert!(dt.is_some());
    /// assert_eq!(timestamp_micros, dt.expect("invalid timestamp").timestamp_micros());
    /// ```
    #[inline]
    #[must_use]
    pub const fn from_timestamp_micros(micros: i64) -> Option<Self> {
        let secs = micros.div_euclid(1_000_000);
        let nsecs = micros.rem_euclid(1_000_000) as u32 * 1000;
        Self::from_timestamp(secs, nsecs)
    }

    /// Creates a new [`DateTime<Utc>`] from the number of non-leap nanoseconds
    /// since January 1, 1970 0:00:00.000 UTC (aka "UNIX timestamp").
    ///
    /// This is guaranteed to round-trip with [`timestamp_nanos`](DateTime::timestamp_nanos).
    ///
    /// If you need to create a `DateTime` with a [`TimeZone`] different from [`Utc`], use
    /// [`TimeZone::timestamp_nanos`] or [`DateTime::with_timezone`].
    ///
    /// The UNIX epoch starts on midnight, January 1, 1970, UTC.
    ///
    /// An `i64` with nanosecond precision can span a range of ~584 years. Because all values can
    /// be represented as a `DateTime` this method never fails.
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::DateTime;
    ///
    /// let timestamp_nanos: i64 = 1662921288_000_000_000; // Sun, 11 Sep 2022 18:34:48 UTC
    /// let dt = DateTime::from_timestamp_nanos(timestamp_nanos);
    /// assert_eq!(timestamp_nanos, dt.timestamp_nanos_opt().unwrap());
    ///
    /// // Negative timestamps (before the UNIX epoch) are supported as well.
    /// let timestamp_nanos: i64 = -2208936075_000_000_000; // Mon, 1 Jan 1900 14:38:45 UTC
    /// let dt = DateTime::from_timestamp_nanos(timestamp_nanos);
    /// assert_eq!(timestamp_nanos, dt.timestamp_nanos_opt().unwrap());
    /// ```
    #[inline]
    #[must_use]
    pub const fn from_timestamp_nanos(nanos: i64) -> Self {
        let secs = nanos.div_euclid(1_000_000_000);
        let nsecs = nanos.rem_euclid(1_000_000_000) as u32;
        expect(Self::from_timestamp(secs, nsecs), "timestamp in nanos is always in range")
    }

    /// The Unix Epoch, 1970-01-01 00:00:00 UTC.
    pub const UNIX_EPOCH: Self =
        expect(NaiveDate::from_ymd_opt(1970, 1, 1), "").and_time(NaiveTime::MIN).and_utc();
}

impl Default for DateTime<Utc> {
    fn default() -> Self {
        Utc.from_utc_datetime(&NaiveDateTime::default())
    }
}

#[cfg(feature = "clock")]
impl Default for DateTime<Local> {
    fn default() -> Self {
        Local.from_utc_datetime(&NaiveDateTime::default())
    }
}

impl Default for DateTime<FixedOffset> {
    fn default() -> Self {
        FixedOffset::west_opt(0).unwrap().from_utc_datetime(&NaiveDateTime::default())
    }
}

/// Convert a `DateTime<Utc>` instance into a `DateTime<FixedOffset>` instance.
impl From<DateTime<Utc>> for DateTime<FixedOffset> {
    /// Convert this `DateTime<Utc>` instance into a `DateTime<FixedOffset>` instance.
    ///
    /// Conversion is done via [`DateTime::with_timezone`]. Note that the converted value returned by
    /// this will be created with a fixed timezone offset of 0.
    fn from(src: DateTime<Utc>) -> Self {
        src.with_timezone(&FixedOffset::east_opt(0).unwrap())
    }
}

/// Convert a `DateTime<Utc>` instance into a `DateTime<Local>` instance.
#[cfg(feature = "clock")]
impl From<DateTime<Utc>> for DateTime<Local> {
    /// Convert this `DateTime<Utc>` instance into a `DateTime<Local>` instance.
    ///
    /// Conversion is performed via [`DateTime::with_timezone`], accounting for the difference in timezones.
    fn from(src: DateTime<Utc>) -> Self {
        src.with_timezone(&Local)
    }
}

/// Convert a `DateTime<FixedOffset>` instance into a `DateTime<Utc>` instance.
impl From<DateTime<FixedOffset>> for DateTime<Utc> {
    /// Convert this `DateTime<FixedOffset>` instance into a `DateTime<Utc>` instance.
    ///
    /// Conversion is performed via [`DateTime::with_timezone`], accounting for the timezone
    /// difference.
    fn from(src: DateTime<FixedOffset>) -> Self {
        src.with_timezone(&Utc)
    }
}

/// Convert a `DateTime<FixedOffset>` instance into a `DateTime<Local>` instance.
#[cfg(feature = "clock")]
impl From<DateTime<FixedOffset>> for DateTime<Local> {
    /// Convert this `DateTime<FixedOffset>` instance into a `DateTime<Local>` instance.
    ///
    /// Conversion is performed via [`DateTime::with_timezone`]. Returns the equivalent value in local
    /// time.
    fn from(src: DateTime<FixedOffset>) -> Self {
        src.with_timezone(&Local)
    }
}

/// Convert a `DateTime<Local>` instance into a `DateTime<Utc>` instance.
#[cfg(feature = "clock")]
impl From<DateTime<Local>> for DateTime<Utc> {
    /// Convert this `DateTime<Local>` instance into a `DateTime<Utc>` instance.
    ///
    /// Conversion is performed via [`DateTime::with_timezone`], accounting for the difference in
    /// timezones.
    fn from(src: DateTime<Local>) -> Self {
        src.with_timezone(&Utc)
    }
}

/// Convert a `DateTime<Local>` instance into a `DateTime<FixedOffset>` instance.
#[cfg(feature = "clock")]
impl From<DateTime<Local>> for DateTime<FixedOffset> {
    /// Convert this `DateTime<Local>` instance into a `DateTime<FixedOffset>` instance.
    ///
    /// Conversion is performed via [`DateTime::with_timezone`].
    fn from(src: DateTime<Local>) -> Self {
        src.with_timezone(&src.offset().fix())
    }
}

/// Maps the local datetime to other datetime with given conversion function.
fn map_local<Tz: TimeZone, F>(dt: &DateTime<Tz>, mut f: F) -> Option<DateTime<Tz>>
where
    F: FnMut(NaiveDateTime) -> Option<NaiveDateTime>,
{
    f(dt.overflowing_naive_local())
        .and_then(|datetime| dt.timezone().from_local_datetime(&datetime).single())
        .filter(|dt| dt >= &DateTime::<Utc>::MIN_UTC && dt <= &DateTime::<Utc>::MAX_UTC)
}

impl DateTime<FixedOffset> {
    /// Parses an RFC 2822 date-and-time string into a `DateTime<FixedOffset>` value.
    ///
    /// This parses valid RFC 2822 datetime strings (such as `Tue, 1 Jul 2003 10:52:37 +0200`)
    /// and returns a new [`DateTime`] instance with the parsed timezone as the [`FixedOffset`].
    ///
    /// RFC 2822 is the internet message standard that specifies the representation of times in HTTP
    /// and email headers. It is the 2001 revision of RFC 822, and is itself revised as RFC 5322 in
    /// 2008.
    ///
    /// # Support for the obsolete date format
    ///
    /// - A 2-digit year is interpreted to be a year in 1950-2049.
    /// - The standard allows comments and whitespace between many of the tokens. See [4.3] and
    ///   [Appendix A.5]
    /// - Single letter 'military' time zone names are parsed as a `-0000` offset.
    ///   They were defined with the wrong sign in RFC 822 and corrected in RFC 2822. But because
    ///   the meaning is now ambiguous, the standard says they should be considered as `-0000`
    ///   unless there is out-of-band information confirming their meaning.
    ///   The exception is `Z`, which remains identical to `+0000`.
    ///
    /// [4.3]: https://www.rfc-editor.org/rfc/rfc2822#section-4.3
    /// [Appendix A.5]: https://www.rfc-editor.org/rfc/rfc2822#appendix-A.5
    ///
    /// # Example
    ///
    /// ```
    /// # use chrono::{DateTime, FixedOffset, TimeZone};
    /// assert_eq!(
    ///     DateTime::parse_from_rfc2822("Wed, 18 Feb 2015 23:16:09 GMT").unwrap(),
    ///     FixedOffset::east_opt(0).unwrap().with_ymd_and_hms(2015, 2, 18, 23, 16, 9).unwrap()
    /// );
    /// ```
    pub fn parse_from_rfc2822(s: &str) -> ParseResult<DateTime<FixedOffset>> {
        const ITEMS: &[Item<'static>] = &[Item::Fixed(Fixed::RFC2822)];
        let mut parsed = Parsed::new();
        parse(&mut parsed, s, ITEMS.iter())?;
        parsed.to_datetime()
    }

    /// Parses an RFC 3339 date-and-time string into a `DateTime<FixedOffset>` value.
    ///
    /// Parses all valid RFC 3339 values (as well as the subset of valid ISO 8601 values that are
    /// also valid RFC 3339 date-and-time values) and returns a new [`DateTime`] with a
    /// [`FixedOffset`] corresponding to the parsed timezone. While RFC 3339 values come in a wide
    /// variety of shapes and sizes, `1996-12-19T16:39:57-08:00` is an example of the most commonly
    /// encountered variety of RFC 3339 formats.
    ///
    /// Why isn't this named `parse_from_iso8601`? That's because ISO 8601 allows representing
    /// values in a wide range of formats, only some of which represent actual date-and-time
    /// instances (rather than periods, ranges, dates, or times). Some valid ISO 8601 values are
    /// also simultaneously valid RFC 3339 values, but not all RFC 3339 values are valid ISO 8601
    /// values (or the other way around).
    pub fn parse_from_rfc3339(s: &str) -> ParseResult<DateTime<FixedOffset>> {
        let mut parsed = Parsed::new();
        let (s, _) = parse_rfc3339(&mut parsed, s)?;
        if !s.is_empty() {
            return Err(TOO_LONG);
        }
        parsed.to_datetime()
    }

    /// Parses a string from a user-specified format into a `DateTime<FixedOffset>` value.
    ///
    /// Note that this method *requires a timezone* in the input string. See
    /// [`NaiveDateTime::parse_from_str`](./naive/struct.NaiveDateTime.html#method.parse_from_str)
    /// for a version that does not require a timezone in the to-be-parsed str. The returned
    /// [`DateTime`] value will have a [`FixedOffset`] reflecting the parsed timezone.
    ///
    /// See the [`format::strftime` module](crate::format::strftime) for supported format
    /// sequences.
    ///
    /// # Example
    ///
    /// ```rust
    /// use chrono::{DateTime, FixedOffset, NaiveDate, TimeZone};
    ///
    /// let dt = DateTime::parse_from_str("1983 Apr 13 12:09:14.274 +0000", "%Y %b %d %H:%M:%S%.3f %z");
    /// assert_eq!(
    ///     dt,
    ///     Ok(FixedOffset::east_opt(0)
    ///         .unwrap()
    ///         .from_local_datetime(
    ///             &NaiveDate::from_ymd_opt(1983, 4, 13)
    ///                 .unwrap()
    ///                 .and_hms_milli_opt(12, 9, 14, 274)
    ///                 .unwrap()
    ///         )
    ///         .unwrap())
    /// );
    /// ```
    pub fn parse_from_str(s: &str, fmt: &str) -> ParseResult<DateTime<FixedOffset>> {
        let mut parsed = Parsed::new();
        parse(&mut parsed, s, StrftimeItems::new(fmt))?;
        parsed.to_datetime()
    }

    /// Parses a string from a user-specified format into a `DateTime<FixedOffset>` value, and a
    /// slice with the remaining portion of the string.
    ///
    /// Note that this method *requires a timezone* in the input string. See
    /// [`NaiveDateTime::parse_and_remainder`] for a version that does not
    /// require a timezone in `s`. The returned [`DateTime`] value will have a [`FixedOffset`]
    /// reflecting the parsed timezone.
    ///
    /// See the [`format::strftime` module](./format/strftime/index.html) for supported format
    /// sequences.
    ///
    /// Similar to [`parse_from_str`](#method.parse_from_str).
    ///
    /// # Example
    ///
    /// ```rust
    /// # use chrono::{DateTime, FixedOffset, TimeZone};
    /// let (datetime, remainder) = DateTime::parse_and_remainder(
    ///     "2015-02-18 23:16:09 +0200 trailing text",
    ///     "%Y-%m-%d %H:%M:%S %z",
    /// )
    /// .unwrap();
    /// assert_eq!(
    ///     datetime,
    ///     FixedOffset::east_opt(2 * 3600).unwrap().with_ymd_and_hms(2015, 2, 18, 23, 16, 9).unwrap()
    /// );
    /// assert_eq!(remainder, " trailing text");
    /// ```
    pub fn parse_and_remainder<'a>(
        s: &'a str,
        fmt: &str,
    ) -> ParseResult<(DateTime<FixedOffset>, &'a str)> {
        let mut parsed = Parsed::new();
        let remainder = parse_and_remainder(&mut parsed, s, StrftimeItems::new(fmt))?;
        parsed.to_datetime().map(|d| (d, remainder))
    }
}

impl<Tz: TimeZone> DateTime<Tz>
where
    Tz::Offset: fmt::Display,
{
    /// Formats the combined date and time with the specified formatting items.
    #[cfg(feature = "alloc")]
    #[inline]
    #[must_use]
    pub fn format_with_items<'a, I, B>(&self, items: I) -> DelayedFormat<I>
    where
        I: Iterator<Item = B> + Clone,
        B: Borrow<Item<'a>>,
    {
        let local = self.overflowing_naive_local();
        DelayedFormat::new_with_offset(Some(local.date()), Some(local.time()), &self.offset, items)
    }

    /// Formats the combined date and time per the specified format string.
    ///
    /// See the [`crate::format::strftime`] module for the supported escape sequences.
    ///
    /// # Example
    /// ```rust
    /// use chrono::prelude::*;
    ///
    /// let date_time: DateTime<Utc> = Utc.with_ymd_and_hms(2017, 04, 02, 12, 50, 32).unwrap();
    /// let formatted = format!("{}", date_time.format("%d/%m/%Y %H:%M"));
    /// assert_eq!(formatted, "02/04/2017 12:50");
    /// ```
    #[cfg(feature = "alloc")]
    #[inline]
    #[must_use]
    pub fn format<'a>(&self, fmt: &'a str) -> DelayedFormat<StrftimeItems<'a>> {
        self.format_with_items(StrftimeItems::new(fmt))
    }

    /// Formats the combined date and time with the specified formatting items and locale.
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
        let local = self.overflowing_naive_local();
        DelayedFormat::new_with_offset_and_locale(
            Some(local.date()),
            Some(local.time()),
            &self.offset,
            items,
            locale,
        )
    }

    /// Formats the combined date and time per the specified format string and
    /// locale.
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
}

impl<Tz: TimeZone> Datelike for DateTime<Tz> {
    #[inline]
    fn year(&self) -> i32 {
        self.overflowing_naive_local().year()
    }
    #[inline]
    fn month(&self) -> u32 {
        self.overflowing_naive_local().month()
    }
    #[inline]
    fn month0(&self) -> u32 {
        self.overflowing_naive_local().month0()
    }
    #[inline]
    fn day(&self) -> u32 {
        self.overflowing_naive_local().day()
    }
    #[inline]
    fn day0(&self) -> u32 {
        self.overflowing_naive_local().day0()
    }
    #[inline]
    fn ordinal(&self) -> u32 {
        self.overflowing_naive_local().ordinal()
    }
    #[inline]
    fn ordinal0(&self) -> u32 {
        self.overflowing_naive_local().ordinal0()
    }
    #[inline]
    fn weekday(&self) -> Weekday {
        self.overflowing_naive_local().weekday()
    }
    #[inline]
    fn iso_week(&self) -> IsoWeek {
        self.overflowing_naive_local().iso_week()
    }

    #[inline]
    /// Makes a new `DateTime` with the year number changed, while keeping the same month and day.
    ///
    /// See also the [`NaiveDate::with_year`] method.
    ///
    /// # Errors
    ///
    /// Returns `None` if:
    /// - The resulting date does not exist (February 29 in a non-leap year).
    /// - The local time at the resulting date does not exist or is ambiguous, for example during a
    ///   daylight saving time transition.
    /// - The resulting UTC datetime would be out of range.
    /// - The resulting local datetime would be out of range (unless the year remains the same).
    fn with_year(&self, year: i32) -> Option<DateTime<Tz>> {
        map_local(self, |dt| match dt.year() == year {
            true => Some(dt),
            false => dt.with_year(year),
        })
    }

    /// Makes a new `DateTime` with the month number (starting from 1) changed.
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
    /// - The local time at the resulting date does not exist or is ambiguous, for example during a
    ///   daylight saving time transition.
    #[inline]
    fn with_month(&self, month: u32) -> Option<DateTime<Tz>> {
        map_local(self, |datetime| datetime.with_month(month))
    }

    /// Makes a new `DateTime` with the month number (starting from 0) changed.
    ///
    /// See also the [`NaiveDate::with_month0`] method.
    ///
    /// # Errors
    ///
    /// Returns `None` if:
    /// - The resulting date does not exist (for example `month0(3)` when day of the month is 31).
    /// - The value for `month0` is invalid.
    /// - The local time at the resulting date does not exist or is ambiguous, for example during a
    ///   daylight saving time transition.
    #[inline]
    fn with_month0(&self, month0: u32) -> Option<DateTime<Tz>> {
        map_local(self, |datetime| datetime.with_month0(month0))
    }

    /// Makes a new `DateTime` with the day of month (starting from 1) changed.
    ///
    /// See also the [`NaiveDate::with_day`] method.
    ///
    /// # Errors
    ///
    /// Returns `None` if:
    /// - The resulting date does not exist (for example `day(31)` in April).
    /// - The value for `day` is invalid.
    /// - The local time at the resulting date does not exist or is ambiguous, for example during a
    ///   daylight saving time transition.
    #[inline]
    fn with_day(&self, day: u32) -> Option<DateTime<Tz>> {
        map_local(self, |datetime| datetime.with_day(day))
    }

    /// Makes a new `DateTime` with the day of month (starting from 0) changed.
    ///
    /// See also the [`NaiveDate::with_day0`] method.
    ///
    /// # Errors
    ///
    /// Returns `None` if:
    /// - The resulting date does not exist (for example `day(30)` in April).
    /// - The value for `day0` is invalid.
    /// - The local time at the resulting date does not exist or is ambiguous, for example during a
    ///   daylight saving time transition.
    #[inline]
    fn with_day0(&self, day0: u32) -> Option<DateTime<Tz>> {
        map_local(self, |datetime| datetime.with_day0(day0))
    }

    /// Makes a new `DateTime` with the day of year (starting from 1) changed.
    ///
    /// See also the [`NaiveDate::with_ordinal`] method.
    ///
    /// # Errors
    ///
    /// Returns `None` if:
    /// - The resulting date does not exist (`with_ordinal(366)` in a non-leap year).
    /// - The value for `ordinal` is invalid.
    /// - The local time at the resulting date does not exist or is ambiguous, for example during a
    ///   daylight saving time transition.
    #[inline]
    fn with_ordinal(&self, ordinal: u32) -> Option<DateTime<Tz>> {
        map_local(self, |datetime| datetime.with_ordinal(ordinal))
    }

    /// Makes a new `DateTime` with the day of year (starting from 0) changed.
    ///
    /// See also the [`NaiveDate::with_ordinal0`] method.
    ///
    /// # Errors
    ///
    /// Returns `None` if:
    /// - The resulting date does not exist (`with_ordinal0(365)` in a non-leap year).
    /// - The value for `ordinal0` is invalid.
    /// - The local time at the resulting date does not exist or is ambiguous, for example during a
    ///   daylight saving time transition.
    #[inline]
    fn with_ordinal0(&self, ordinal0: u32) -> Option<DateTime<Tz>> {
        map_local(self, |datetime| datetime.with_ordinal0(ordinal0))
    }
}

impl<Tz: TimeZone> Timelike for DateTime<Tz> {
    #[inline]
    fn hour(&self) -> u32 {
        self.overflowing_naive_local().hour()
    }
    #[inline]
    fn minute(&self) -> u32 {
        self.overflowing_naive_local().minute()
    }
    #[inline]
    fn second(&self) -> u32 {
        self.overflowing_naive_local().second()
    }
    #[inline]
    fn nanosecond(&self) -> u32 {
        self.overflowing_naive_local().nanosecond()
    }

    /// Makes a new `DateTime` with the hour number changed.
    ///
    /// See also the [`NaiveTime::with_hour`] method.
    ///
    /// # Errors
    ///
    /// Returns `None` if:
    /// - The value for `hour` is invalid.
    /// - The local time at the resulting date does not exist or is ambiguous, for example during a
    ///   daylight saving time transition.
    #[inline]
    fn with_hour(&self, hour: u32) -> Option<DateTime<Tz>> {
        map_local(self, |datetime| datetime.with_hour(hour))
    }

    /// Makes a new `DateTime` with the minute number changed.
    ///
    /// See also the [`NaiveTime::with_minute`] method.
    ///
    /// # Errors
    ///
    /// - The value for `minute` is invalid.
    /// - The local time at the resulting date does not exist or is ambiguous, for example during a
    ///   daylight saving time transition.
    #[inline]
    fn with_minute(&self, min: u32) -> Option<DateTime<Tz>> {
        map_local(self, |datetime| datetime.with_minute(min))
    }

    /// Makes a new `DateTime` with the second number changed.
    ///
    /// As with the [`second`](#method.second) method,
    /// the input range is restricted to 0 through 59.
    ///
    /// See also the [`NaiveTime::with_second`] method.
    ///
    /// # Errors
    ///
    /// Returns `None` if:
    /// - The value for `second` is invalid.
    /// - The local time at the resulting date does not exist or is ambiguous, for example during a
    ///   daylight saving time transition.
    #[inline]
    fn with_second(&self, sec: u32) -> Option<DateTime<Tz>> {
        map_local(self, |datetime| datetime.with_second(sec))
    }

    /// Makes a new `DateTime` with nanoseconds since the whole non-leap second changed.
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
    #[inline]
    fn with_nanosecond(&self, nano: u32) -> Option<DateTime<Tz>> {
        map_local(self, |datetime| datetime.with_nanosecond(nano))
    }
}

// We don't store a field with the `Tz` type, so it doesn't need to influence whether `DateTime` can
// be `Copy`. Implement it manually if the two types we do have are `Copy`.
impl<Tz: TimeZone> Copy for DateTime<Tz>
where
    <Tz as TimeZone>::Offset: Copy,
    NaiveDateTime: Copy,
{
}

impl<Tz: TimeZone, Tz2: TimeZone> PartialEq<DateTime<Tz2>> for DateTime<Tz> {
    fn eq(&self, other: &DateTime<Tz2>) -> bool {
        self.datetime == other.datetime
    }
}

impl<Tz: TimeZone> Eq for DateTime<Tz> {}

impl<Tz: TimeZone, Tz2: TimeZone> PartialOrd<DateTime<Tz2>> for DateTime<Tz> {
    /// Compare two DateTimes based on their true time, ignoring time zones
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::prelude::*;
    ///
    /// let earlier = Utc
    ///     .with_ymd_and_hms(2015, 5, 15, 2, 0, 0)
    ///     .unwrap()
    ///     .with_timezone(&FixedOffset::west_opt(1 * 3600).unwrap());
    /// let later = Utc
    ///     .with_ymd_and_hms(2015, 5, 15, 3, 0, 0)
    ///     .unwrap()
    ///     .with_timezone(&FixedOffset::west_opt(5 * 3600).unwrap());
    ///
    /// assert_eq!(earlier.to_string(), "2015-05-15 01:00:00 -01:00");
    /// assert_eq!(later.to_string(), "2015-05-14 22:00:00 -05:00");
    ///
    /// assert!(later > earlier);
    /// ```
    fn partial_cmp(&self, other: &DateTime<Tz2>) -> Option<Ordering> {
        self.datetime.partial_cmp(&other.datetime)
    }
}

impl<Tz: TimeZone> Ord for DateTime<Tz> {
    fn cmp(&self, other: &DateTime<Tz>) -> Ordering {
        self.datetime.cmp(&other.datetime)
    }
}

impl<Tz: TimeZone> hash::Hash for DateTime<Tz> {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.datetime.hash(state)
    }
}

/// Add `TimeDelta` to `DateTime`.
///
/// As a part of Chrono's [leap second handling], the addition assumes that **there is no leap
/// second ever**, except when the `NaiveDateTime` itself represents a leap  second in which case
/// the assumption becomes that **there is exactly a single leap second ever**.
///
/// # Panics
///
/// Panics if the resulting date would be out of range.
/// Consider using [`DateTime<Tz>::checked_add_signed`] to get an `Option` instead.
impl<Tz: TimeZone> Add<TimeDelta> for DateTime<Tz> {
    type Output = DateTime<Tz>;

    #[inline]
    fn add(self, rhs: TimeDelta) -> DateTime<Tz> {
        self.checked_add_signed(rhs).expect("`DateTime + TimeDelta` overflowed")
    }
}

/// Add `std::time::Duration` to `DateTime`.
///
/// As a part of Chrono's [leap second handling], the addition assumes that **there is no leap
/// second ever**, except when the `NaiveDateTime` itself represents a leap  second in which case
/// the assumption becomes that **there is exactly a single leap second ever**.
///
/// # Panics
///
/// Panics if the resulting date would be out of range.
/// Consider using [`DateTime<Tz>::checked_add_signed`] to get an `Option` instead.
impl<Tz: TimeZone> Add<Duration> for DateTime<Tz> {
    type Output = DateTime<Tz>;

    #[inline]
    fn add(self, rhs: Duration) -> DateTime<Tz> {
        let rhs = TimeDelta::from_std(rhs)
            .expect("overflow converting from core::time::Duration to TimeDelta");
        self.checked_add_signed(rhs).expect("`DateTime + TimeDelta` overflowed")
    }
}

/// Add-assign `chrono::Duration` to `DateTime`.
///
/// As a part of Chrono's [leap second handling], the addition assumes that **there is no leap
/// second ever**, except when the `NaiveDateTime` itself represents a leap  second in which case
/// the assumption becomes that **there is exactly a single leap second ever**.
///
/// # Panics
///
/// Panics if the resulting date would be out of range.
/// Consider using [`DateTime<Tz>::checked_add_signed`] to get an `Option` instead.
impl<Tz: TimeZone> AddAssign<TimeDelta> for DateTime<Tz> {
    #[inline]
    fn add_assign(&mut self, rhs: TimeDelta) {
        let datetime =
            self.datetime.checked_add_signed(rhs).expect("`DateTime + TimeDelta` overflowed");
        let tz = self.timezone();
        *self = tz.from_utc_datetime(&datetime);
    }
}

/// Add-assign `std::time::Duration` to `DateTime`.
///
/// As a part of Chrono's [leap second handling], the addition assumes that **there is no leap
/// second ever**, except when the `NaiveDateTime` itself represents a leap  second in which case
/// the assumption becomes that **there is exactly a single leap second ever**.
///
/// # Panics
///
/// Panics if the resulting date would be out of range.
/// Consider using [`DateTime<Tz>::checked_add_signed`] to get an `Option` instead.
impl<Tz: TimeZone> AddAssign<Duration> for DateTime<Tz> {
    #[inline]
    fn add_assign(&mut self, rhs: Duration) {
        let rhs = TimeDelta::from_std(rhs)
            .expect("overflow converting from core::time::Duration to TimeDelta");
        *self += rhs;
    }
}

/// Add `FixedOffset` to the datetime value of `DateTime` (offset remains unchanged).
///
/// # Panics
///
/// Panics if the resulting date would be out of range.
impl<Tz: TimeZone> Add<FixedOffset> for DateTime<Tz> {
    type Output = DateTime<Tz>;

    #[inline]
    fn add(mut self, rhs: FixedOffset) -> DateTime<Tz> {
        self.datetime =
            self.naive_utc().checked_add_offset(rhs).expect("`DateTime + FixedOffset` overflowed");
        self
    }
}

/// Add `Months` to `DateTime`.
///
/// The result will be clamped to valid days in the resulting month, see `checked_add_months` for
/// details.
///
/// # Panics
///
/// Panics if:
/// - The resulting date would be out of range.
/// - The local time at the resulting date does not exist or is ambiguous, for example during a
///   daylight saving time transition.
///
/// Strongly consider using [`DateTime<Tz>::checked_add_months`] to get an `Option` instead.
impl<Tz: TimeZone> Add<Months> for DateTime<Tz> {
    type Output = DateTime<Tz>;

    fn add(self, rhs: Months) -> Self::Output {
        self.checked_add_months(rhs).expect("`DateTime + Months` out of range")
    }
}

/// Subtract `TimeDelta` from `DateTime`.
///
/// This is the same as the addition with a negated `TimeDelta`.
///
/// As a part of Chrono's [leap second handling] the subtraction assumes that **there is no leap
/// second ever**, except when the `DateTime` itself represents a leap second in which case
/// the assumption becomes that **there is exactly a single leap second ever**.
///
/// # Panics
///
/// Panics if the resulting date would be out of range.
/// Consider using [`DateTime<Tz>::checked_sub_signed`] to get an `Option` instead.
impl<Tz: TimeZone> Sub<TimeDelta> for DateTime<Tz> {
    type Output = DateTime<Tz>;

    #[inline]
    fn sub(self, rhs: TimeDelta) -> DateTime<Tz> {
        self.checked_sub_signed(rhs).expect("`DateTime - TimeDelta` overflowed")
    }
}

/// Subtract `std::time::Duration` from `DateTime`.
///
/// As a part of Chrono's [leap second handling] the subtraction assumes that **there is no leap
/// second ever**, except when the `DateTime` itself represents a leap second in which case
/// the assumption becomes that **there is exactly a single leap second ever**.
///
/// # Panics
///
/// Panics if the resulting date would be out of range.
/// Consider using [`DateTime<Tz>::checked_sub_signed`] to get an `Option` instead.
impl<Tz: TimeZone> Sub<Duration> for DateTime<Tz> {
    type Output = DateTime<Tz>;

    #[inline]
    fn sub(self, rhs: Duration) -> DateTime<Tz> {
        let rhs = TimeDelta::from_std(rhs)
            .expect("overflow converting from core::time::Duration to TimeDelta");
        self.checked_sub_signed(rhs).expect("`DateTime - TimeDelta` overflowed")
    }
}

/// Subtract-assign `TimeDelta` from `DateTime`.
///
/// This is the same as the addition with a negated `TimeDelta`.
///
/// As a part of Chrono's [leap second handling], the addition assumes that **there is no leap
/// second ever**, except when the `DateTime` itself represents a leap  second in which case
/// the assumption becomes that **there is exactly a single leap second ever**.
///
/// # Panics
///
/// Panics if the resulting date would be out of range.
/// Consider using [`DateTime<Tz>::checked_sub_signed`] to get an `Option` instead.
impl<Tz: TimeZone> SubAssign<TimeDelta> for DateTime<Tz> {
    #[inline]
    fn sub_assign(&mut self, rhs: TimeDelta) {
        let datetime =
            self.datetime.checked_sub_signed(rhs).expect("`DateTime - TimeDelta` overflowed");
        let tz = self.timezone();
        *self = tz.from_utc_datetime(&datetime)
    }
}

/// Subtract-assign `std::time::Duration` from `DateTime`.
///
/// As a part of Chrono's [leap second handling], the addition assumes that **there is no leap
/// second ever**, except when the `DateTime` itself represents a leap  second in which case
/// the assumption becomes that **there is exactly a single leap second ever**.
///
/// # Panics
///
/// Panics if the resulting date would be out of range.
/// Consider using [`DateTime<Tz>::checked_sub_signed`] to get an `Option` instead.
impl<Tz: TimeZone> SubAssign<Duration> for DateTime<Tz> {
    #[inline]
    fn sub_assign(&mut self, rhs: Duration) {
        let rhs = TimeDelta::from_std(rhs)
            .expect("overflow converting from core::time::Duration to TimeDelta");
        *self -= rhs;
    }
}

/// Subtract `FixedOffset` from the datetime value of `DateTime` (offset remains unchanged).
///
/// # Panics
///
/// Panics if the resulting date would be out of range.
impl<Tz: TimeZone> Sub<FixedOffset> for DateTime<Tz> {
    type Output = DateTime<Tz>;

    #[inline]
    fn sub(mut self, rhs: FixedOffset) -> DateTime<Tz> {
        self.datetime =
            self.naive_utc().checked_sub_offset(rhs).expect("`DateTime - FixedOffset` overflowed");
        self
    }
}

/// Subtract `Months` from `DateTime`.
///
/// The result will be clamped to valid days in the resulting month, see
/// [`DateTime<Tz>::checked_sub_months`] for details.
///
/// # Panics
///
/// Panics if:
/// - The resulting date would be out of range.
/// - The local time at the resulting date does not exist or is ambiguous, for example during a
///   daylight saving time transition.
///
/// Strongly consider using [`DateTime<Tz>::checked_sub_months`] to get an `Option` instead.
impl<Tz: TimeZone> Sub<Months> for DateTime<Tz> {
    type Output = DateTime<Tz>;

    fn sub(self, rhs: Months) -> Self::Output {
        self.checked_sub_months(rhs).expect("`DateTime - Months` out of range")
    }
}

impl<Tz: TimeZone> Sub<DateTime<Tz>> for DateTime<Tz> {
    type Output = TimeDelta;

    #[inline]
    fn sub(self, rhs: DateTime<Tz>) -> TimeDelta {
        self.signed_duration_since(rhs)
    }
}

impl<Tz: TimeZone> Sub<&DateTime<Tz>> for DateTime<Tz> {
    type Output = TimeDelta;

    #[inline]
    fn sub(self, rhs: &DateTime<Tz>) -> TimeDelta {
        self.signed_duration_since(rhs)
    }
}

/// Add `Days` to `NaiveDateTime`.
///
/// # Panics
///
/// Panics if:
/// - The resulting date would be out of range.
/// - The local time at the resulting date does not exist or is ambiguous, for example during a
///   daylight saving time transition.
///
/// Strongly consider using `DateTime<Tz>::checked_add_days` to get an `Option` instead.
impl<Tz: TimeZone> Add<Days> for DateTime<Tz> {
    type Output = DateTime<Tz>;

    fn add(self, days: Days) -> Self::Output {
        self.checked_add_days(days).expect("`DateTime + Days` out of range")
    }
}

/// Subtract `Days` from `DateTime`.
///
/// # Panics
///
/// Panics if:
/// - The resulting date would be out of range.
/// - The local time at the resulting date does not exist or is ambiguous, for example during a
///   daylight saving time transition.
///
/// Strongly consider using `DateTime<Tz>::checked_sub_days` to get an `Option` instead.
impl<Tz: TimeZone> Sub<Days> for DateTime<Tz> {
    type Output = DateTime<Tz>;

    fn sub(self, days: Days) -> Self::Output {
        self.checked_sub_days(days).expect("`DateTime - Days` out of range")
    }
}

impl<Tz: TimeZone> fmt::Debug for DateTime<Tz> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.overflowing_naive_local().fmt(f)?;
        self.offset.fmt(f)
    }
}

// `fmt::Debug` is hand implemented for the `rkyv::Archive` variant of `DateTime` because
// deriving a trait recursively does not propagate trait defined associated types with their own
// constraints:
// In our case `<<Tz as offset::TimeZone>::Offset as Archive>::Archived`
// cannot be formatted using `{:?}` because it doesn't implement `Debug`.
// See below for further discussion:
// * https://github.com/rust-lang/rust/issues/26925
// * https://github.com/rkyv/rkyv/issues/333
// * https://github.com/dtolnay/syn/issues/370
#[cfg(feature = "rkyv-validation")]
impl<Tz: TimeZone> fmt::Debug for ArchivedDateTime<Tz>
where
    Tz: Archive,
    <Tz as Archive>::Archived: fmt::Debug,
    <<Tz as TimeZone>::Offset as Archive>::Archived: fmt::Debug,
    <Tz as TimeZone>::Offset: fmt::Debug + Archive,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("ArchivedDateTime")
            .field("datetime", &self.datetime)
            .field("offset", &self.offset)
            .finish()
    }
}

impl<Tz: TimeZone> fmt::Display for DateTime<Tz>
where
    Tz::Offset: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.overflowing_naive_local().fmt(f)?;
        f.write_char(' ')?;
        self.offset.fmt(f)
    }
}

/// Accepts a relaxed form of RFC3339.
/// A space or a 'T' are accepted as the separator between the date and time
/// parts.
///
/// All of these examples are equivalent:
/// ```
/// # use chrono::{DateTime, Utc};
/// "2012-12-12T12:12:12Z".parse::<DateTime<Utc>>()?;
/// "2012-12-12 12:12:12Z".parse::<DateTime<Utc>>()?;
/// "2012-12-12 12:12:12+0000".parse::<DateTime<Utc>>()?;
/// "2012-12-12 12:12:12+00:00".parse::<DateTime<Utc>>()?;
/// # Ok::<(), chrono::ParseError>(())
/// ```
impl str::FromStr for DateTime<Utc> {
    type Err = ParseError;

    fn from_str(s: &str) -> ParseResult<DateTime<Utc>> {
        s.parse::<DateTime<FixedOffset>>().map(|dt| dt.with_timezone(&Utc))
    }
}

/// Accepts a relaxed form of RFC3339.
/// A space or a 'T' are accepted as the separator between the date and time
/// parts.
///
/// All of these examples are equivalent:
/// ```
/// # use chrono::{DateTime, Local};
/// "2012-12-12T12:12:12Z".parse::<DateTime<Local>>()?;
/// "2012-12-12 12:12:12Z".parse::<DateTime<Local>>()?;
/// "2012-12-12 12:12:12+0000".parse::<DateTime<Local>>()?;
/// "2012-12-12 12:12:12+00:00".parse::<DateTime<Local>>()?;
/// # Ok::<(), chrono::ParseError>(())
/// ```
#[cfg(feature = "clock")]
impl str::FromStr for DateTime<Local> {
    type Err = ParseError;

    fn from_str(s: &str) -> ParseResult<DateTime<Local>> {
        s.parse::<DateTime<FixedOffset>>().map(|dt| dt.with_timezone(&Local))
    }
}

#[cfg(feature = "std")]
impl From<SystemTime> for DateTime<Utc> {
    fn from(t: SystemTime) -> DateTime<Utc> {
        let (sec, nsec) = match t.duration_since(UNIX_EPOCH) {
            Ok(dur) => (dur.as_secs() as i64, dur.subsec_nanos()),
            Err(e) => {
                // unlikely but should be handled
                let dur = e.duration();
                let (sec, nsec) = (dur.as_secs() as i64, dur.subsec_nanos());
                if nsec == 0 { (-sec, 0) } else { (-sec - 1, 1_000_000_000 - nsec) }
            }
        };
        Utc.timestamp_opt(sec, nsec).unwrap()
    }
}

#[cfg(feature = "clock")]
impl From<SystemTime> for DateTime<Local> {
    fn from(t: SystemTime) -> DateTime<Local> {
        DateTime::<Utc>::from(t).with_timezone(&Local)
    }
}

#[cfg(feature = "std")]
impl<Tz: TimeZone> From<DateTime<Tz>> for SystemTime {
    fn from(dt: DateTime<Tz>) -> SystemTime {
        let sec = dt.timestamp();
        let nsec = dt.timestamp_subsec_nanos();
        if sec < 0 {
            // unlikely but should be handled
            UNIX_EPOCH - Duration::new(-sec as u64, 0) + Duration::new(0, nsec)
        } else {
            UNIX_EPOCH + Duration::new(sec as u64, nsec)
        }
    }
}

#[cfg(all(
    target_arch = "wasm32",
    feature = "wasmbind",
    not(any(target_os = "emscripten", target_os = "wasi", target_os = "linux"))
))]
impl From<js_sys::Date> for DateTime<Utc> {
    fn from(date: js_sys::Date) -> DateTime<Utc> {
        DateTime::<Utc>::from(&date)
    }
}

#[cfg(all(
    target_arch = "wasm32",
    feature = "wasmbind",
    not(any(target_os = "emscripten", target_os = "wasi", target_os = "linux"))
))]
impl From<&js_sys::Date> for DateTime<Utc> {
    fn from(date: &js_sys::Date) -> DateTime<Utc> {
        Utc.timestamp_millis_opt(date.get_time() as i64).unwrap()
    }
}

#[cfg(all(
    target_arch = "wasm32",
    feature = "wasmbind",
    not(any(target_os = "emscripten", target_os = "wasi", target_os = "linux"))
))]
impl From<DateTime<Utc>> for js_sys::Date {
    /// Converts a `DateTime<Utc>` to a JS `Date`. The resulting value may be lossy,
    /// any values that have a millisecond timestamp value greater/less than ±8,640,000,000,000,000
    /// (April 20, 271821 BCE ~ September 13, 275760 CE) will become invalid dates in JS.
    fn from(date: DateTime<Utc>) -> js_sys::Date {
        let js_millis = wasm_bindgen::JsValue::from_f64(date.timestamp_millis() as f64);
        js_sys::Date::new(&js_millis)
    }
}

// Note that implementation of Arbitrary cannot be simply derived for DateTime<Tz>, due to
// the nontrivial bound <Tz as TimeZone>::Offset: Arbitrary.
#[cfg(all(feature = "arbitrary", feature = "std"))]
impl<'a, Tz> arbitrary::Arbitrary<'a> for DateTime<Tz>
where
    Tz: TimeZone,
    <Tz as TimeZone>::Offset: arbitrary::Arbitrary<'a>,
{
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<DateTime<Tz>> {
        let datetime = NaiveDateTime::arbitrary(u)?;
        let offset = <Tz as TimeZone>::Offset::arbitrary(u)?;
        Ok(DateTime::from_naive_utc_and_offset(datetime, offset))
    }
}

/// Number of days between January 1, 1970 and December 31, 1 BCE which we define to be day 0.
/// 4 full leap year cycles until December 31, 1600     4 * 146097 = 584388
/// 1 day until January 1, 1601                                           1
/// 369 years until January 1, 1970                      369 * 365 = 134685
/// of which floor(369 / 4) are leap years          floor(369 / 4) =     92
/// except for 1700, 1800 and 1900                                       -3 +
///                                                                  --------
///                                                                  719163
pub(crate) const UNIX_EPOCH_DAY: i64 = 719_163;
