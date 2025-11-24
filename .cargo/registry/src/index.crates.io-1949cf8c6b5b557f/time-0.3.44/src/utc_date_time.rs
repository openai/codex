//! The [`UtcDateTime`] struct and associated `impl`s.

#[cfg(feature = "formatting")]
use alloc::string::String;
use core::fmt;
use core::ops::{Add, AddAssign, Sub, SubAssign};
use core::time::Duration as StdDuration;
#[cfg(feature = "formatting")]
use std::io;

use deranged::RangedI64;
use powerfmt::ext::FormatterExt as _;
use powerfmt::smart_display::{self, FormatterOptions, Metadata, SmartDisplay};

use crate::convert::*;
use crate::date::{MAX_YEAR, MIN_YEAR};
#[cfg(feature = "formatting")]
use crate::formatting::Formattable;
use crate::internal_macros::{
    carry, cascade, const_try, const_try_opt, div_floor, ensure_ranged, expect_opt,
};
#[cfg(feature = "parsing")]
use crate::parsing::Parsable;
use crate::{
    error, util, Date, Duration, Month, OffsetDateTime, PrimitiveDateTime, Time, UtcOffset, Weekday,
};

/// The Julian day of the Unix epoch.
const UNIX_EPOCH_JULIAN_DAY: i32 = UtcDateTime::UNIX_EPOCH.to_julian_day();

/// A [`PrimitiveDateTime`] that is known to be UTC.
///
/// `UtcDateTime` is guaranteed to be ABI-compatible with [`PrimitiveDateTime`], meaning that
/// transmuting from one to the other will not result in undefined behavior.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UtcDateTime {
    inner: PrimitiveDateTime,
}

impl UtcDateTime {
    /// Midnight, 1 January, 1970.
    ///
    /// ```rust
    /// # use time::UtcDateTime;
    /// # use time_macros::utc_datetime;
    /// assert_eq!(UtcDateTime::UNIX_EPOCH, utc_datetime!(1970-01-01 0:00));
    /// ```
    pub const UNIX_EPOCH: Self = Self::new(Date::UNIX_EPOCH, Time::MIDNIGHT);

    /// The smallest value that can be represented by `UtcDateTime`.
    ///
    /// Depending on `large-dates` feature flag, value of this constant may vary.
    ///
    /// 1. With `large-dates` disabled it is equal to `-9999-01-01 00:00:00.0`
    /// 2. With `large-dates` enabled it is equal to `-999999-01-01 00:00:00.0`
    ///
    /// ```rust
    /// # use time::UtcDateTime;
    /// # use time_macros::utc_datetime;
    #[cfg_attr(
        feature = "large-dates",
        doc = "// Assuming `large-dates` feature is enabled."
    )]
    #[cfg_attr(
        feature = "large-dates",
        doc = "assert_eq!(UtcDateTime::MIN, utc_datetime!(-999999-01-01 0:00));"
    )]
    #[cfg_attr(
        not(feature = "large-dates"),
        doc = "// Assuming `large-dates` feature is disabled."
    )]
    #[cfg_attr(
        not(feature = "large-dates"),
        doc = "assert_eq!(UtcDateTime::MIN, utc_datetime!(-9999-01-01 0:00));"
    )]
    /// ```
    pub const MIN: Self = Self::new(Date::MIN, Time::MIDNIGHT);

    /// The largest value that can be represented by `UtcDateTime`.
    ///
    /// Depending on `large-dates` feature flag, value of this constant may vary.
    ///
    /// 1. With `large-dates` disabled it is equal to `9999-12-31 23:59:59.999_999_999`
    /// 2. With `large-dates` enabled it is equal to `999999-12-31 23:59:59.999_999_999`
    ///
    /// ```rust
    /// # use time::UtcDateTime;
    /// # use time_macros::utc_datetime;
    #[cfg_attr(
        feature = "large-dates",
        doc = "// Assuming `large-dates` feature is enabled."
    )]
    #[cfg_attr(
        feature = "large-dates",
        doc = "assert_eq!(UtcDateTime::MAX, utc_datetime!(+999999-12-31 23:59:59.999_999_999));"
    )]
    #[cfg_attr(
        not(feature = "large-dates"),
        doc = "// Assuming `large-dates` feature is disabled."
    )]
    #[cfg_attr(
        not(feature = "large-dates"),
        doc = "assert_eq!(UtcDateTime::MAX, utc_datetime!(+9999-12-31 23:59:59.999_999_999));"
    )]
    /// ```
    pub const MAX: Self = Self::new(Date::MAX, Time::MAX);

    /// Create a new `UtcDateTime` with the current date and time.
    ///
    /// ```rust
    /// # use time::UtcDateTime;
    /// assert!(UtcDateTime::now().year() >= 2019);
    /// ```
    #[cfg(feature = "std")]
    #[inline]
    pub fn now() -> Self {
        #[cfg(all(
            target_family = "wasm",
            not(any(target_os = "emscripten", target_os = "wasi")),
            feature = "wasm-bindgen"
        ))]
        {
            js_sys::Date::new_0().into()
        }

        #[cfg(not(all(
            target_family = "wasm",
            not(any(target_os = "emscripten", target_os = "wasi")),
            feature = "wasm-bindgen"
        )))]
        std::time::SystemTime::now().into()
    }

    /// Create a new `UtcDateTime` from the provided [`Date`] and [`Time`].
    ///
    /// ```rust
    /// # use time::UtcDateTime;
    /// # use time_macros::{date, utc_datetime, time};
    /// assert_eq!(
    ///     UtcDateTime::new(date!(2019-01-01), time!(0:00)),
    ///     utc_datetime!(2019-01-01 0:00),
    /// );
    /// ```
    #[inline]
    pub const fn new(date: Date, time: Time) -> Self {
        Self {
            inner: PrimitiveDateTime::new(date, time),
        }
    }

    /// Create a new `UtcDateTime` from the [`PrimitiveDateTime`], assuming that the latter is UTC.
    #[inline]
    pub(crate) const fn from_primitive(date_time: PrimitiveDateTime) -> Self {
        Self { inner: date_time }
    }

    /// Obtain the [`PrimitiveDateTime`] that this `UtcDateTime` represents. The no-longer-attached
    /// [`UtcOffset`] is assumed to be UTC.
    #[inline]
    pub(crate) const fn as_primitive(self) -> PrimitiveDateTime {
        self.inner
    }

    /// Create a `UtcDateTime` from the provided Unix timestamp.
    ///
    /// ```rust
    /// # use time::UtcDateTime;
    /// # use time_macros::utc_datetime;
    /// assert_eq!(
    ///     UtcDateTime::from_unix_timestamp(0),
    ///     Ok(UtcDateTime::UNIX_EPOCH),
    /// );
    /// assert_eq!(
    ///     UtcDateTime::from_unix_timestamp(1_546_300_800),
    ///     Ok(utc_datetime!(2019-01-01 0:00)),
    /// );
    /// ```
    ///
    /// If you have a timestamp-nanosecond pair, you can use something along the lines of the
    /// following:
    ///
    /// ```rust
    /// # use time::{Duration, UtcDateTime, ext::NumericalDuration};
    /// let (timestamp, nanos) = (1, 500_000_000);
    /// assert_eq!(
    ///     UtcDateTime::from_unix_timestamp(timestamp)? + Duration::nanoseconds(nanos),
    ///     UtcDateTime::UNIX_EPOCH + 1.5.seconds()
    /// );
    /// # Ok::<_, time::Error>(())
    /// ```
    #[inline]
    pub const fn from_unix_timestamp(timestamp: i64) -> Result<Self, error::ComponentRange> {
        type Timestamp =
            RangedI64<{ UtcDateTime::MIN.unix_timestamp() }, { UtcDateTime::MAX.unix_timestamp() }>;
        ensure_ranged!(Timestamp: timestamp);

        // Use the unchecked method here, as the input validity has already been verified.
        // Safety: The Julian day number is in range.
        let date = unsafe {
            Date::from_julian_day_unchecked(
                UNIX_EPOCH_JULIAN_DAY + div_floor!(timestamp, Second::per_t::<i64>(Day)) as i32,
            )
        };

        let seconds_within_day = timestamp.rem_euclid(Second::per_t::<i64>(Day));
        // Safety: All values are in range.
        let time = unsafe {
            Time::__from_hms_nanos_unchecked(
                (seconds_within_day / Second::per_t::<i64>(Hour)) as u8,
                ((seconds_within_day % Second::per_t::<i64>(Hour)) / Minute::per_t::<i64>(Hour))
                    as u8,
                (seconds_within_day % Second::per_t::<i64>(Minute)) as u8,
                0,
            )
        };

        Ok(Self::new(date, time))
    }

    /// Construct an `UtcDateTime` from the provided Unix timestamp (in nanoseconds).
    ///
    /// ```rust
    /// # use time::UtcDateTime;
    /// # use time_macros::utc_datetime;
    /// assert_eq!(
    ///     UtcDateTime::from_unix_timestamp_nanos(0),
    ///     Ok(UtcDateTime::UNIX_EPOCH),
    /// );
    /// assert_eq!(
    ///     UtcDateTime::from_unix_timestamp_nanos(1_546_300_800_000_000_000),
    ///     Ok(utc_datetime!(2019-01-01 0:00)),
    /// );
    /// ```
    #[inline]
    pub const fn from_unix_timestamp_nanos(timestamp: i128) -> Result<Self, error::ComponentRange> {
        let datetime = const_try!(Self::from_unix_timestamp(div_floor!(
            timestamp,
            Nanosecond::per_t::<i128>(Second)
        ) as i64));

        Ok(Self::new(
            datetime.date(),
            // Safety: `nanosecond` is in range due to `rem_euclid`.
            unsafe {
                Time::__from_hms_nanos_unchecked(
                    datetime.hour(),
                    datetime.minute(),
                    datetime.second(),
                    timestamp.rem_euclid(Nanosecond::per_t(Second)) as u32,
                )
            },
        ))
    }

    /// Convert the `UtcDateTime` from UTC to the provided [`UtcOffset`], returning an
    /// [`OffsetDateTime`].
    ///
    /// ```rust
    /// # use time_macros::{utc_datetime, offset};
    /// assert_eq!(
    ///     utc_datetime!(2000-01-01 0:00)
    ///         .to_offset(offset!(-1))
    ///         .year(),
    ///     1999,
    /// );
    ///
    /// // Construct midnight on new year's, UTC.
    /// let utc = utc_datetime!(2000-01-01 0:00);
    /// let new_york = utc.to_offset(offset!(-5));
    /// let los_angeles = utc.to_offset(offset!(-8));
    /// assert_eq!(utc.hour(), 0);
    /// assert_eq!(new_york.hour(), 19);
    /// assert_eq!(los_angeles.hour(), 16);
    /// ```
    ///
    /// # Panics
    ///
    /// This method panics if the local date-time in the new offset is outside the supported range.
    #[inline]
    #[track_caller]
    pub const fn to_offset(self, offset: UtcOffset) -> OffsetDateTime {
        expect_opt!(
            self.checked_to_offset(offset),
            "local datetime out of valid range"
        )
    }

    /// Convert the `UtcDateTime` from UTC to the provided [`UtcOffset`], returning an
    /// [`OffsetDateTime`]. `None` is returned if the date-time in the resulting offset is
    /// invalid.
    ///
    /// ```rust
    /// # use time::UtcDateTime;
    /// # use time_macros::{utc_datetime, offset};
    /// assert_eq!(
    ///     utc_datetime!(2000-01-01 0:00)
    ///         .checked_to_offset(offset!(-1))
    ///         .unwrap()
    ///         .year(),
    ///     1999,
    /// );
    /// assert_eq!(
    ///     UtcDateTime::MAX.checked_to_offset(offset!(+1)),
    ///     None,
    /// );
    /// ```
    #[inline]
    pub const fn checked_to_offset(self, offset: UtcOffset) -> Option<OffsetDateTime> {
        // Fast path for when no conversion is necessary.
        if offset.is_utc() {
            return Some(self.inner.assume_utc());
        }

        let (year, ordinal, time) = self.to_offset_raw(offset);

        if year > MAX_YEAR || year < MIN_YEAR {
            return None;
        }

        Some(OffsetDateTime::new_in_offset(
            // Safety: `ordinal` is not zero.
            unsafe { Date::__from_ordinal_date_unchecked(year, ordinal) },
            time,
            offset,
        ))
    }

    /// Equivalent to `.to_offset(offset)`, but returning the year, ordinal, and time. This avoids
    /// constructing an invalid [`Date`] if the new value is out of range.
    #[inline]
    pub(crate) const fn to_offset_raw(self, offset: UtcOffset) -> (i32, u16, Time) {
        let (second, carry) = carry!(@most_once
            self.second() as i8 + offset.seconds_past_minute(),
            0..Second::per_t(Minute)
        );
        let (minute, carry) = carry!(@most_once
            self.minute() as i8 + offset.minutes_past_hour() + carry,
            0..Minute::per_t(Hour)
        );
        let (hour, carry) = carry!(@most_twice
            self.hour() as i8 + offset.whole_hours() + carry,
            0..Hour::per_t(Day)
        );
        let (mut year, ordinal) = self.to_ordinal_date();
        let mut ordinal = ordinal as i16 + carry;
        cascade!(ordinal => year);

        debug_assert!(ordinal > 0);
        debug_assert!(ordinal <= util::days_in_year(year) as i16);

        (
            year,
            ordinal as u16,
            // Safety: The cascades above ensure the values are in range.
            unsafe {
                Time::__from_hms_nanos_unchecked(
                    hour as u8,
                    minute as u8,
                    second as u8,
                    self.nanosecond(),
                )
            },
        )
    }

    /// Get the [Unix timestamp](https://en.wikipedia.org/wiki/Unix_time).
    ///
    /// ```rust
    /// # use time_macros::utc_datetime;
    /// assert_eq!(utc_datetime!(1970-01-01 0:00).unix_timestamp(), 0);
    /// assert_eq!(utc_datetime!(1970-01-01 1:00).unix_timestamp(), 3_600);
    /// ```
    #[inline]
    pub const fn unix_timestamp(self) -> i64 {
        let days = (self.to_julian_day() as i64 - UNIX_EPOCH_JULIAN_DAY as i64)
            * Second::per_t::<i64>(Day);
        let hours = self.hour() as i64 * Second::per_t::<i64>(Hour);
        let minutes = self.minute() as i64 * Second::per_t::<i64>(Minute);
        let seconds = self.second() as i64;
        days + hours + minutes + seconds
    }

    /// Get the Unix timestamp in nanoseconds.
    ///
    /// ```rust
    /// use time_macros::utc_datetime;
    /// assert_eq!(utc_datetime!(1970-01-01 0:00).unix_timestamp_nanos(), 0);
    /// assert_eq!(
    ///     utc_datetime!(1970-01-01 1:00).unix_timestamp_nanos(),
    ///     3_600_000_000_000,
    /// );
    /// ```
    #[inline]
    pub const fn unix_timestamp_nanos(self) -> i128 {
        self.unix_timestamp() as i128 * Nanosecond::per_t::<i128>(Second)
            + self.nanosecond() as i128
    }

    /// Get the [`Date`] component of the `UtcDateTime`.
    ///
    /// ```rust
    /// # use time_macros::{date, utc_datetime};
    /// assert_eq!(utc_datetime!(2019-01-01 0:00).date(), date!(2019-01-01));
    /// ```
    #[inline]
    pub const fn date(self) -> Date {
        self.inner.date()
    }

    /// Get the [`Time`] component of the `UtcDateTime`.
    ///
    /// ```rust
    /// # use time_macros::{utc_datetime, time};
    /// assert_eq!(utc_datetime!(2019-01-01 0:00).time(), time!(0:00));
    /// ```
    #[inline]
    pub const fn time(self) -> Time {
        self.inner.time()
    }

    /// Get the year of the date.
    ///
    /// ```rust
    /// # use time_macros::utc_datetime;
    /// assert_eq!(utc_datetime!(2019-01-01 0:00).year(), 2019);
    /// assert_eq!(utc_datetime!(2019-12-31 0:00).year(), 2019);
    /// assert_eq!(utc_datetime!(2020-01-01 0:00).year(), 2020);
    /// ```
    #[inline]
    pub const fn year(self) -> i32 {
        self.date().year()
    }

    /// Get the month of the date.
    ///
    /// ```rust
    /// # use time::Month;
    /// # use time_macros::utc_datetime;
    /// assert_eq!(utc_datetime!(2019-01-01 0:00).month(), Month::January);
    /// assert_eq!(utc_datetime!(2019-12-31 0:00).month(), Month::December);
    /// ```
    #[inline]
    pub const fn month(self) -> Month {
        self.date().month()
    }

    /// Get the day of the date.
    ///
    /// The returned value will always be in the range `1..=31`.
    ///
    /// ```rust
    /// # use time_macros::utc_datetime;
    /// assert_eq!(utc_datetime!(2019-01-01 0:00).day(), 1);
    /// assert_eq!(utc_datetime!(2019-12-31 0:00).day(), 31);
    /// ```
    #[inline]
    pub const fn day(self) -> u8 {
        self.date().day()
    }

    /// Get the day of the year.
    ///
    /// The returned value will always be in the range `1..=366` (`1..=365` for common years).
    ///
    /// ```rust
    /// # use time_macros::utc_datetime;
    /// assert_eq!(utc_datetime!(2019-01-01 0:00).ordinal(), 1);
    /// assert_eq!(utc_datetime!(2019-12-31 0:00).ordinal(), 365);
    /// ```
    #[inline]
    pub const fn ordinal(self) -> u16 {
        self.date().ordinal()
    }

    /// Get the ISO week number.
    ///
    /// The returned value will always be in the range `1..=53`.
    ///
    /// ```rust
    /// # use time_macros::utc_datetime;
    /// assert_eq!(utc_datetime!(2019-01-01 0:00).iso_week(), 1);
    /// assert_eq!(utc_datetime!(2019-10-04 0:00).iso_week(), 40);
    /// assert_eq!(utc_datetime!(2020-01-01 0:00).iso_week(), 1);
    /// assert_eq!(utc_datetime!(2020-12-31 0:00).iso_week(), 53);
    /// assert_eq!(utc_datetime!(2021-01-01 0:00).iso_week(), 53);
    /// ```
    #[inline]
    pub const fn iso_week(self) -> u8 {
        self.date().iso_week()
    }

    /// Get the week number where week 1 begins on the first Sunday.
    ///
    /// The returned value will always be in the range `0..=53`.
    ///
    /// ```rust
    /// # use time_macros::utc_datetime;
    /// assert_eq!(utc_datetime!(2019-01-01 0:00).sunday_based_week(), 0);
    /// assert_eq!(utc_datetime!(2020-01-01 0:00).sunday_based_week(), 0);
    /// assert_eq!(utc_datetime!(2020-12-31 0:00).sunday_based_week(), 52);
    /// assert_eq!(utc_datetime!(2021-01-01 0:00).sunday_based_week(), 0);
    /// ```
    #[inline]
    pub const fn sunday_based_week(self) -> u8 {
        self.date().sunday_based_week()
    }

    /// Get the week number where week 1 begins on the first Monday.
    ///
    /// The returned value will always be in the range `0..=53`.
    ///
    /// ```rust
    /// # use time_macros::utc_datetime;
    /// assert_eq!(utc_datetime!(2019-01-01 0:00).monday_based_week(), 0);
    /// assert_eq!(utc_datetime!(2020-01-01 0:00).monday_based_week(), 0);
    /// assert_eq!(utc_datetime!(2020-12-31 0:00).monday_based_week(), 52);
    /// assert_eq!(utc_datetime!(2021-01-01 0:00).monday_based_week(), 0);
    /// ```
    #[inline]
    pub const fn monday_based_week(self) -> u8 {
        self.date().monday_based_week()
    }

    /// Get the year, month, and day.
    ///
    /// ```rust
    /// # use time::Month;
    /// # use time_macros::utc_datetime;
    /// assert_eq!(
    ///     utc_datetime!(2019-01-01 0:00).to_calendar_date(),
    ///     (2019, Month::January, 1)
    /// );
    /// ```
    #[inline]
    pub const fn to_calendar_date(self) -> (i32, Month, u8) {
        self.date().to_calendar_date()
    }

    /// Get the year and ordinal day number.
    ///
    /// ```rust
    /// # use time_macros::utc_datetime;
    /// assert_eq!(utc_datetime!(2019-01-01 0:00).to_ordinal_date(), (2019, 1));
    /// ```
    #[inline]
    pub const fn to_ordinal_date(self) -> (i32, u16) {
        self.date().to_ordinal_date()
    }

    /// Get the ISO 8601 year, week number, and weekday.
    ///
    /// ```rust
    /// # use time::Weekday::*;
    /// # use time_macros::utc_datetime;
    /// assert_eq!(
    ///     utc_datetime!(2019-01-01 0:00).to_iso_week_date(),
    ///     (2019, 1, Tuesday)
    /// );
    /// assert_eq!(
    ///     utc_datetime!(2019-10-04 0:00).to_iso_week_date(),
    ///     (2019, 40, Friday)
    /// );
    /// assert_eq!(
    ///     utc_datetime!(2020-01-01 0:00).to_iso_week_date(),
    ///     (2020, 1, Wednesday)
    /// );
    /// assert_eq!(
    ///     utc_datetime!(2020-12-31 0:00).to_iso_week_date(),
    ///     (2020, 53, Thursday)
    /// );
    /// assert_eq!(
    ///     utc_datetime!(2021-01-01 0:00).to_iso_week_date(),
    ///     (2020, 53, Friday)
    /// );
    /// ```
    #[inline]
    pub const fn to_iso_week_date(self) -> (i32, u8, Weekday) {
        self.date().to_iso_week_date()
    }

    /// Get the weekday.
    ///
    /// ```rust
    /// # use time::Weekday::*;
    /// # use time_macros::utc_datetime;
    /// assert_eq!(utc_datetime!(2019-01-01 0:00).weekday(), Tuesday);
    /// assert_eq!(utc_datetime!(2019-02-01 0:00).weekday(), Friday);
    /// assert_eq!(utc_datetime!(2019-03-01 0:00).weekday(), Friday);
    /// assert_eq!(utc_datetime!(2019-04-01 0:00).weekday(), Monday);
    /// assert_eq!(utc_datetime!(2019-05-01 0:00).weekday(), Wednesday);
    /// assert_eq!(utc_datetime!(2019-06-01 0:00).weekday(), Saturday);
    /// assert_eq!(utc_datetime!(2019-07-01 0:00).weekday(), Monday);
    /// assert_eq!(utc_datetime!(2019-08-01 0:00).weekday(), Thursday);
    /// assert_eq!(utc_datetime!(2019-09-01 0:00).weekday(), Sunday);
    /// assert_eq!(utc_datetime!(2019-10-01 0:00).weekday(), Tuesday);
    /// assert_eq!(utc_datetime!(2019-11-01 0:00).weekday(), Friday);
    /// assert_eq!(utc_datetime!(2019-12-01 0:00).weekday(), Sunday);
    /// ```
    #[inline]
    pub const fn weekday(self) -> Weekday {
        self.date().weekday()
    }

    /// Get the Julian day for the date. The time is not taken into account for this calculation.
    ///
    /// The algorithm to perform this conversion is derived from one provided by Peter Baum; it is
    /// freely available [here](https://www.researchgate.net/publication/316558298_Date_Algorithms).
    ///
    /// ```rust
    /// # use time_macros::utc_datetime;
    /// assert_eq!(utc_datetime!(-4713-11-24 0:00).to_julian_day(), 0);
    /// assert_eq!(utc_datetime!(2000-01-01 0:00).to_julian_day(), 2_451_545);
    /// assert_eq!(utc_datetime!(2019-01-01 0:00).to_julian_day(), 2_458_485);
    /// assert_eq!(utc_datetime!(2019-12-31 0:00).to_julian_day(), 2_458_849);
    /// ```
    #[inline]
    pub const fn to_julian_day(self) -> i32 {
        self.date().to_julian_day()
    }

    /// Get the clock hour, minute, and second.
    ///
    /// ```rust
    /// # use time_macros::utc_datetime;
    /// assert_eq!(utc_datetime!(2020-01-01 0:00:00).as_hms(), (0, 0, 0));
    /// assert_eq!(utc_datetime!(2020-01-01 23:59:59).as_hms(), (23, 59, 59));
    /// ```
    #[inline]
    pub const fn as_hms(self) -> (u8, u8, u8) {
        self.time().as_hms()
    }

    /// Get the clock hour, minute, second, and millisecond.
    ///
    /// ```rust
    /// # use time_macros::utc_datetime;
    /// assert_eq!(utc_datetime!(2020-01-01 0:00:00).as_hms_milli(), (0, 0, 0, 0));
    /// assert_eq!(
    ///     utc_datetime!(2020-01-01 23:59:59.999).as_hms_milli(),
    ///     (23, 59, 59, 999)
    /// );
    /// ```
    #[inline]
    pub const fn as_hms_milli(self) -> (u8, u8, u8, u16) {
        self.time().as_hms_milli()
    }

    /// Get the clock hour, minute, second, and microsecond.
    ///
    /// ```rust
    /// # use time_macros::utc_datetime;
    /// assert_eq!(utc_datetime!(2020-01-01 0:00:00).as_hms_micro(), (0, 0, 0, 0));
    /// assert_eq!(
    ///     utc_datetime!(2020-01-01 23:59:59.999_999).as_hms_micro(),
    ///     (23, 59, 59, 999_999)
    /// );
    /// ```
    #[inline]
    pub const fn as_hms_micro(self) -> (u8, u8, u8, u32) {
        self.time().as_hms_micro()
    }

    /// Get the clock hour, minute, second, and nanosecond.
    ///
    /// ```rust
    /// # use time_macros::utc_datetime;
    /// assert_eq!(utc_datetime!(2020-01-01 0:00:00).as_hms_nano(), (0, 0, 0, 0));
    /// assert_eq!(
    ///     utc_datetime!(2020-01-01 23:59:59.999_999_999).as_hms_nano(),
    ///     (23, 59, 59, 999_999_999)
    /// );
    /// ```
    #[inline]
    pub const fn as_hms_nano(self) -> (u8, u8, u8, u32) {
        self.time().as_hms_nano()
    }

    /// Get the clock hour.
    ///
    /// The returned value will always be in the range `0..24`.
    ///
    /// ```rust
    /// # use time_macros::utc_datetime;
    /// assert_eq!(utc_datetime!(2019-01-01 0:00).hour(), 0);
    /// assert_eq!(utc_datetime!(2019-01-01 23:59:59).hour(), 23);
    /// ```
    #[inline]
    pub const fn hour(self) -> u8 {
        self.time().hour()
    }

    /// Get the minute within the hour.
    ///
    /// The returned value will always be in the range `0..60`.
    ///
    /// ```rust
    /// # use time_macros::utc_datetime;
    /// assert_eq!(utc_datetime!(2019-01-01 0:00).minute(), 0);
    /// assert_eq!(utc_datetime!(2019-01-01 23:59:59).minute(), 59);
    /// ```
    #[inline]
    pub const fn minute(self) -> u8 {
        self.time().minute()
    }

    /// Get the second within the minute.
    ///
    /// The returned value will always be in the range `0..60`.
    ///
    /// ```rust
    /// # use time_macros::utc_datetime;
    /// assert_eq!(utc_datetime!(2019-01-01 0:00).second(), 0);
    /// assert_eq!(utc_datetime!(2019-01-01 23:59:59).second(), 59);
    /// ```
    #[inline]
    pub const fn second(self) -> u8 {
        self.time().second()
    }

    /// Get the milliseconds within the second.
    ///
    /// The returned value will always be in the range `0..1_000`.
    ///
    /// ```rust
    /// # use time_macros::utc_datetime;
    /// assert_eq!(utc_datetime!(2019-01-01 0:00).millisecond(), 0);
    /// assert_eq!(utc_datetime!(2019-01-01 23:59:59.999).millisecond(), 999);
    /// ```
    #[inline]
    pub const fn millisecond(self) -> u16 {
        self.time().millisecond()
    }

    /// Get the microseconds within the second.
    ///
    /// The returned value will always be in the range `0..1_000_000`.
    ///
    /// ```rust
    /// # use time_macros::utc_datetime;
    /// assert_eq!(utc_datetime!(2019-01-01 0:00).microsecond(), 0);
    /// assert_eq!(
    ///     utc_datetime!(2019-01-01 23:59:59.999_999).microsecond(),
    ///     999_999
    /// );
    /// ```
    #[inline]
    pub const fn microsecond(self) -> u32 {
        self.time().microsecond()
    }

    /// Get the nanoseconds within the second.
    ///
    /// The returned value will always be in the range `0..1_000_000_000`.
    ///
    /// ```rust
    /// # use time_macros::utc_datetime;
    /// assert_eq!(utc_datetime!(2019-01-01 0:00).nanosecond(), 0);
    /// assert_eq!(
    ///     utc_datetime!(2019-01-01 23:59:59.999_999_999).nanosecond(),
    ///     999_999_999,
    /// );
    /// ```
    #[inline]
    pub const fn nanosecond(self) -> u32 {
        self.time().nanosecond()
    }

    /// Computes `self + duration`, returning `None` if an overflow occurred.
    ///
    /// ```rust
    /// # use time::{UtcDateTime, ext::NumericalDuration};
    /// # use time_macros::utc_datetime;
    /// assert_eq!(UtcDateTime::MIN.checked_add((-2).days()), None);
    /// assert_eq!(UtcDateTime::MAX.checked_add(1.days()), None);
    /// assert_eq!(
    ///     utc_datetime!(2019 - 11 - 25 15:30).checked_add(27.hours()),
    ///     Some(utc_datetime!(2019 - 11 - 26 18:30))
    /// );
    /// ```
    #[inline]
    pub const fn checked_add(self, duration: Duration) -> Option<Self> {
        Some(Self::from_primitive(const_try_opt!(self
            .inner
            .checked_add(duration))))
    }

    /// Computes `self - duration`, returning `None` if an overflow occurred.
    ///
    /// ```rust
    /// # use time::{UtcDateTime, ext::NumericalDuration};
    /// # use time_macros::utc_datetime;
    /// assert_eq!(UtcDateTime::MIN.checked_sub(2.days()), None);
    /// assert_eq!(UtcDateTime::MAX.checked_sub((-1).days()), None);
    /// assert_eq!(
    ///     utc_datetime!(2019 - 11 - 25 15:30).checked_sub(27.hours()),
    ///     Some(utc_datetime!(2019 - 11 - 24 12:30))
    /// );
    /// ```
    #[inline]
    pub const fn checked_sub(self, duration: Duration) -> Option<Self> {
        Some(Self::from_primitive(const_try_opt!(self
            .inner
            .checked_sub(duration))))
    }

    /// Computes `self + duration`, saturating value on overflow.
    ///
    /// ```rust
    /// # use time::{UtcDateTime, ext::NumericalDuration};
    /// # use time_macros::utc_datetime;
    /// assert_eq!(
    ///     UtcDateTime::MIN.saturating_add((-2).days()),
    ///     UtcDateTime::MIN
    /// );
    /// assert_eq!(
    ///     UtcDateTime::MAX.saturating_add(2.days()),
    ///     UtcDateTime::MAX
    /// );
    /// assert_eq!(
    ///     utc_datetime!(2019 - 11 - 25 15:30).saturating_add(27.hours()),
    ///     utc_datetime!(2019 - 11 - 26 18:30)
    /// );
    /// ```
    #[inline]
    pub const fn saturating_add(self, duration: Duration) -> Self {
        Self::from_primitive(self.inner.saturating_add(duration))
    }

    /// Computes `self - duration`, saturating value on overflow.
    ///
    /// ```rust
    /// # use time::{UtcDateTime, ext::NumericalDuration};
    /// # use time_macros::utc_datetime;
    /// assert_eq!(
    ///     UtcDateTime::MIN.saturating_sub(2.days()),
    ///     UtcDateTime::MIN
    /// );
    /// assert_eq!(
    ///     UtcDateTime::MAX.saturating_sub((-2).days()),
    ///     UtcDateTime::MAX
    /// );
    /// assert_eq!(
    ///     utc_datetime!(2019 - 11 - 25 15:30).saturating_sub(27.hours()),
    ///     utc_datetime!(2019 - 11 - 24 12:30)
    /// );
    /// ```
    #[inline]
    pub const fn saturating_sub(self, duration: Duration) -> Self {
        Self::from_primitive(self.inner.saturating_sub(duration))
    }
}

/// Methods that replace part of the `UtcDateTime`.
impl UtcDateTime {
    /// Replace the time, preserving the date.
    ///
    /// ```rust
    /// # use time_macros::{utc_datetime, time};
    /// assert_eq!(
    ///     utc_datetime!(2020-01-01 17:00).replace_time(time!(5:00)),
    ///     utc_datetime!(2020-01-01 5:00)
    /// );
    /// ```
    #[must_use = "This method does not mutate the original `UtcDateTime`."]
    #[inline]
    pub const fn replace_time(self, time: Time) -> Self {
        Self::from_primitive(self.inner.replace_time(time))
    }

    /// Replace the date, preserving the time.
    ///
    /// ```rust
    /// # use time_macros::{utc_datetime, date};
    /// assert_eq!(
    ///     utc_datetime!(2020-01-01 12:00).replace_date(date!(2020-01-30)),
    ///     utc_datetime!(2020-01-30 12:00)
    /// );
    /// ```
    #[must_use = "This method does not mutate the original `UtcDateTime`."]
    #[inline]
    pub const fn replace_date(self, date: Date) -> Self {
        Self::from_primitive(self.inner.replace_date(date))
    }

    /// Replace the year. The month and day will be unchanged.
    ///
    /// ```rust
    /// # use time_macros::utc_datetime;
    /// assert_eq!(
    ///     utc_datetime!(2022 - 02 - 18 12:00).replace_year(2019),
    ///     Ok(utc_datetime!(2019 - 02 - 18 12:00))
    /// );
    /// assert!(utc_datetime!(2022 - 02 - 18 12:00).replace_year(-1_000_000_000).is_err()); // -1_000_000_000 isn't a valid year
    /// assert!(utc_datetime!(2022 - 02 - 18 12:00).replace_year(1_000_000_000).is_err()); // 1_000_000_000 isn't a valid year
    /// ```
    #[must_use = "This method does not mutate the original `UtcDateTime`."]
    #[inline]
    pub const fn replace_year(self, year: i32) -> Result<Self, error::ComponentRange> {
        Ok(Self::from_primitive(const_try!(self
            .inner
            .replace_year(year))))
    }

    /// Replace the month of the year.
    ///
    /// ```rust
    /// # use time_macros::utc_datetime;
    /// # use time::Month;
    /// assert_eq!(
    ///     utc_datetime!(2022 - 02 - 18 12:00).replace_month(Month::January),
    ///     Ok(utc_datetime!(2022 - 01 - 18 12:00))
    /// );
    /// assert!(utc_datetime!(2022 - 01 - 30 12:00).replace_month(Month::February).is_err()); // 30 isn't a valid day in February
    /// ```
    #[must_use = "This method does not mutate the original `UtcDateTime`."]
    #[inline]
    pub const fn replace_month(self, month: Month) -> Result<Self, error::ComponentRange> {
        Ok(Self::from_primitive(const_try!(self
            .inner
            .replace_month(month))))
    }

    /// Replace the day of the month.
    ///
    /// ```rust
    /// # use time_macros::utc_datetime;
    /// assert_eq!(
    ///     utc_datetime!(2022 - 02 - 18 12:00).replace_day(1),
    ///     Ok(utc_datetime!(2022 - 02 - 01 12:00))
    /// );
    /// assert!(utc_datetime!(2022 - 02 - 18 12:00).replace_day(0).is_err()); // 00 isn't a valid day
    /// assert!(utc_datetime!(2022 - 02 - 18 12:00).replace_day(30).is_err()); // 30 isn't a valid day in February
    /// ```
    #[must_use = "This method does not mutate the original `UtcDateTime`."]
    #[inline]
    pub const fn replace_day(self, day: u8) -> Result<Self, error::ComponentRange> {
        Ok(Self::from_primitive(const_try!(self
            .inner
            .replace_day(day))))
    }

    /// Replace the day of the year.
    ///
    /// ```rust
    /// # use time_macros::utc_datetime;
    /// assert_eq!(utc_datetime!(2022-049 12:00).replace_ordinal(1), Ok(utc_datetime!(2022-001 12:00)));
    /// assert!(utc_datetime!(2022-049 12:00).replace_ordinal(0).is_err()); // 0 isn't a valid ordinal
    /// assert!(utc_datetime!(2022-049 12:00).replace_ordinal(366).is_err()); // 2022 isn't a leap year
    /// ```
    #[must_use = "This method does not mutate the original `UtcDateTime`."]
    #[inline]
    pub const fn replace_ordinal(self, ordinal: u16) -> Result<Self, error::ComponentRange> {
        Ok(Self::from_primitive(const_try!(self
            .inner
            .replace_ordinal(ordinal))))
    }

    /// Replace the clock hour.
    ///
    /// ```rust
    /// # use time_macros::utc_datetime;
    /// assert_eq!(
    ///     utc_datetime!(2022 - 02 - 18 01:02:03.004_005_006).replace_hour(7),
    ///     Ok(utc_datetime!(2022 - 02 - 18 07:02:03.004_005_006))
    /// );
    /// assert!(utc_datetime!(2022 - 02 - 18 01:02:03.004_005_006).replace_hour(24).is_err()); // 24 isn't a valid hour
    /// ```
    #[must_use = "This method does not mutate the original `UtcDateTime`."]
    #[inline]
    pub const fn replace_hour(self, hour: u8) -> Result<Self, error::ComponentRange> {
        Ok(Self::from_primitive(const_try!(self
            .inner
            .replace_hour(hour))))
    }

    /// Replace the minutes within the hour.
    ///
    /// ```rust
    /// # use time_macros::utc_datetime;
    /// assert_eq!(
    ///     utc_datetime!(2022 - 02 - 18 01:02:03.004_005_006).replace_minute(7),
    ///     Ok(utc_datetime!(2022 - 02 - 18 01:07:03.004_005_006))
    /// );
    /// assert!(utc_datetime!(2022 - 02 - 18 01:02:03.004_005_006).replace_minute(60).is_err()); // 60 isn't a valid minute
    /// ```
    #[must_use = "This method does not mutate the original `UtcDateTime`."]
    #[inline]
    pub const fn replace_minute(
        self,
        sunday_based_week: u8,
    ) -> Result<Self, error::ComponentRange> {
        Ok(Self::from_primitive(const_try!(self
            .inner
            .replace_minute(sunday_based_week))))
    }

    /// Replace the seconds within the minute.
    ///
    /// ```rust
    /// # use time_macros::utc_datetime;
    /// assert_eq!(
    ///     utc_datetime!(2022 - 02 - 18 01:02:03.004_005_006).replace_second(7),
    ///     Ok(utc_datetime!(2022 - 02 - 18 01:02:07.004_005_006))
    /// );
    /// assert!(utc_datetime!(2022 - 02 - 18 01:02:03.004_005_006).replace_second(60).is_err()); // 60 isn't a valid second
    /// ```
    #[must_use = "This method does not mutate the original `UtcDateTime`."]
    #[inline]
    pub const fn replace_second(
        self,
        monday_based_week: u8,
    ) -> Result<Self, error::ComponentRange> {
        Ok(Self::from_primitive(const_try!(self
            .inner
            .replace_second(monday_based_week))))
    }

    /// Replace the milliseconds within the second.
    ///
    /// ```rust
    /// # use time_macros::utc_datetime;
    /// assert_eq!(
    ///     utc_datetime!(2022 - 02 - 18 01:02:03.004_005_006).replace_millisecond(7),
    ///     Ok(utc_datetime!(2022 - 02 - 18 01:02:03.007))
    /// );
    /// assert!(utc_datetime!(2022 - 02 - 18 01:02:03.004_005_006).replace_millisecond(1_000).is_err()); // 1_000 isn't a valid millisecond
    /// ```
    #[must_use = "This method does not mutate the original `UtcDateTime`."]
    #[inline]
    pub const fn replace_millisecond(
        self,
        millisecond: u16,
    ) -> Result<Self, error::ComponentRange> {
        Ok(Self::from_primitive(const_try!(self
            .inner
            .replace_millisecond(millisecond))))
    }

    /// Replace the microseconds within the second.
    ///
    /// ```rust
    /// # use time_macros::utc_datetime;
    /// assert_eq!(
    ///     utc_datetime!(2022 - 02 - 18 01:02:03.004_005_006).replace_microsecond(7_008),
    ///     Ok(utc_datetime!(2022 - 02 - 18 01:02:03.007_008))
    /// );
    /// assert!(utc_datetime!(2022 - 02 - 18 01:02:03.004_005_006).replace_microsecond(1_000_000).is_err()); // 1_000_000 isn't a valid microsecond
    /// ```
    #[must_use = "This method does not mutate the original `UtcDateTime`."]
    #[inline]
    pub const fn replace_microsecond(
        self,
        microsecond: u32,
    ) -> Result<Self, error::ComponentRange> {
        Ok(Self::from_primitive(const_try!(self
            .inner
            .replace_microsecond(microsecond))))
    }

    /// Replace the nanoseconds within the second.
    ///
    /// ```rust
    /// # use time_macros::utc_datetime;
    /// assert_eq!(
    ///     utc_datetime!(2022 - 02 - 18 01:02:03.004_005_006).replace_nanosecond(7_008_009),
    ///     Ok(utc_datetime!(2022 - 02 - 18 01:02:03.007_008_009))
    /// );
    /// assert!(utc_datetime!(2022 - 02 - 18 01:02:03.004_005_006).replace_nanosecond(1_000_000_000).is_err()); // 1_000_000_000 isn't a valid nanosecond
    /// ```
    #[must_use = "This method does not mutate the original `UtcDateTime`."]
    #[inline]
    pub const fn replace_nanosecond(self, nanosecond: u32) -> Result<Self, error::ComponentRange> {
        Ok(Self::from_primitive(const_try!(self
            .inner
            .replace_nanosecond(nanosecond))))
    }
}

#[cfg(feature = "formatting")]
impl UtcDateTime {
    /// Format the `UtcDateTime` using the provided [format
    /// description](crate::format_description).
    #[inline]
    pub fn format_into(
        self,
        output: &mut (impl io::Write + ?Sized),
        format: &(impl Formattable + ?Sized),
    ) -> Result<usize, error::Format> {
        format.format_into(
            output,
            Some(self.date()),
            Some(self.time()),
            Some(UtcOffset::UTC),
        )
    }

    /// Format the `UtcDateTime` using the provided [format
    /// description](crate::format_description).
    ///
    /// ```rust
    /// # use time::format_description;
    /// # use time_macros::utc_datetime;
    /// let format = format_description::parse(
    ///     "[year]-[month]-[day] [hour]:[minute]:[second] [offset_hour \
    ///          sign:mandatory]:[offset_minute]:[offset_second]",
    /// )?;
    /// assert_eq!(
    ///     utc_datetime!(2020-01-02 03:04:05).format(&format)?,
    ///     "2020-01-02 03:04:05 +00:00:00"
    /// );
    /// # Ok::<_, time::Error>(())
    /// ```
    #[inline]
    pub fn format(self, format: &(impl Formattable + ?Sized)) -> Result<String, error::Format> {
        format.format(Some(self.date()), Some(self.time()), Some(UtcOffset::UTC))
    }
}

#[cfg(feature = "parsing")]
impl UtcDateTime {
    /// Parse an `UtcDateTime` from the input using the provided [format
    /// description](crate::format_description). A [`UtcOffset`] is permitted, but not required to
    /// be present. If present, the value will be converted to UTC.
    ///
    /// ```rust
    /// # use time::UtcDateTime;
    /// # use time_macros::{utc_datetime, format_description};
    /// let format = format_description!("[year]-[month]-[day] [hour]:[minute]:[second]");
    /// assert_eq!(
    ///     UtcDateTime::parse("2020-01-02 03:04:05", &format)?,
    ///     utc_datetime!(2020-01-02 03:04:05)
    /// );
    /// # Ok::<_, time::Error>(())
    /// ```
    #[inline]
    pub fn parse(
        input: &str,
        description: &(impl Parsable + ?Sized),
    ) -> Result<Self, error::Parse> {
        description.parse_utc_date_time(input.as_bytes())
    }

    /// A helper method to check if the `UtcDateTime` is a valid representation of a leap second.
    /// Leap seconds, when parsed, are represented as the preceding nanosecond. However, leap
    /// seconds can only occur as the last second of a month UTC.
    #[cfg(feature = "parsing")]
    #[inline]
    pub(crate) const fn is_valid_leap_second_stand_in(self) -> bool {
        let dt = self.inner;

        dt.hour() == 23
            && dt.minute() == 59
            && dt.second() == 59
            && dt.nanosecond() == 999_999_999
            && dt.day() == dt.month().length(dt.year())
    }
}

impl SmartDisplay for UtcDateTime {
    type Metadata = ();

    #[inline]
    fn metadata(&self, _: FormatterOptions) -> Metadata<'_, Self> {
        let width = smart_display::padded_width_of!(self.date(), " ", self.time(), " +00");
        Metadata::new(width, self, ())
    }

    #[inline]
    fn fmt_with_metadata(
        &self,
        f: &mut fmt::Formatter<'_>,
        metadata: Metadata<Self>,
    ) -> fmt::Result {
        f.pad_with_width(
            metadata.unpadded_width(),
            format_args!("{} {} +00", self.date(), self.time()),
        )
    }
}

impl fmt::Display for UtcDateTime {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        SmartDisplay::fmt(self, f)
    }
}

impl fmt::Debug for UtcDateTime {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl Add<Duration> for UtcDateTime {
    type Output = Self;

    /// # Panics
    ///
    /// This may panic if an overflow occurs.
    #[inline]
    #[track_caller]
    fn add(self, duration: Duration) -> Self::Output {
        self.inner.add(duration).as_utc()
    }
}

impl Add<StdDuration> for UtcDateTime {
    type Output = Self;

    /// # Panics
    ///
    /// This may panic if an overflow occurs.
    #[inline]
    #[track_caller]
    fn add(self, duration: StdDuration) -> Self::Output {
        self.inner.add(duration).as_utc()
    }
}

impl AddAssign<Duration> for UtcDateTime {
    /// # Panics
    ///
    /// This may panic if an overflow occurs.
    #[inline]
    #[track_caller]
    fn add_assign(&mut self, rhs: Duration) {
        self.inner.add_assign(rhs);
    }
}

impl AddAssign<StdDuration> for UtcDateTime {
    /// # Panics
    ///
    /// This may panic if an overflow occurs.
    #[inline]
    #[track_caller]
    fn add_assign(&mut self, rhs: StdDuration) {
        self.inner.add_assign(rhs);
    }
}

impl Sub<Duration> for UtcDateTime {
    type Output = Self;

    /// # Panics
    ///
    /// This may panic if an overflow occurs.
    #[inline]
    #[track_caller]
    fn sub(self, rhs: Duration) -> Self::Output {
        self.checked_sub(rhs)
            .expect("resulting value is out of range")
    }
}

impl Sub<StdDuration> for UtcDateTime {
    type Output = Self;

    /// # Panics
    ///
    /// This may panic if an overflow occurs.
    #[inline]
    #[track_caller]
    fn sub(self, duration: StdDuration) -> Self::Output {
        Self::from_primitive(self.inner.sub(duration))
    }
}

impl SubAssign<Duration> for UtcDateTime {
    /// # Panics
    ///
    /// This may panic if an overflow occurs.
    #[inline]
    #[track_caller]
    fn sub_assign(&mut self, rhs: Duration) {
        self.inner.sub_assign(rhs);
    }
}

impl SubAssign<StdDuration> for UtcDateTime {
    /// # Panics
    ///
    /// This may panic if an overflow occurs.
    #[inline]
    #[track_caller]
    fn sub_assign(&mut self, rhs: StdDuration) {
        self.inner.sub_assign(rhs);
    }
}

impl Sub for UtcDateTime {
    type Output = Duration;

    /// # Panics
    ///
    /// This may panic if an overflow occurs.
    #[inline]
    #[track_caller]
    fn sub(self, rhs: Self) -> Self::Output {
        self.inner.sub(rhs.inner)
    }
}
