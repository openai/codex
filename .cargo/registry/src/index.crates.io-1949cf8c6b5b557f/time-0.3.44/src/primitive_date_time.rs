//! The [`PrimitiveDateTime`] struct and its associated `impl`s.

#[cfg(feature = "formatting")]
use alloc::string::String;
use core::cmp::Ordering;
use core::fmt;
use core::hash::{Hash, Hasher};
use core::ops::{Add, AddAssign, Sub, SubAssign};
use core::time::Duration as StdDuration;
#[cfg(feature = "formatting")]
use std::io;

use powerfmt::ext::FormatterExt as _;
use powerfmt::smart_display::{self, FormatterOptions, Metadata, SmartDisplay};

#[cfg(feature = "formatting")]
use crate::formatting::Formattable;
use crate::internal_macros::{const_try, const_try_opt};
#[cfg(feature = "parsing")]
use crate::parsing::Parsable;
use crate::{
    error, util, Date, Duration, Month, OffsetDateTime, Time, UtcDateTime, UtcOffset, Weekday,
};

/// Combined date and time.
#[derive(Clone, Copy, Eq)]
#[cfg_attr(not(docsrs), repr(C))]
pub struct PrimitiveDateTime {
    // The order of this struct's fields matter! Do not reorder them.

    // Little endian version
    #[cfg(target_endian = "little")]
    time: Time,
    #[cfg(target_endian = "little")]
    date: Date,

    // Big endian version
    #[cfg(target_endian = "big")]
    date: Date,
    #[cfg(target_endian = "big")]
    time: Time,
}

impl Hash for PrimitiveDateTime {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_i128().hash(state);
    }
}

impl PartialEq for PrimitiveDateTime {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.as_i128().eq(&other.as_i128())
    }
}

impl PartialOrd for PrimitiveDateTime {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PrimitiveDateTime {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        self.as_i128().cmp(&other.as_i128())
    }
}

impl PrimitiveDateTime {
    /// Provide a representation of `PrimitiveDateTime` as a `i128`. This value can be used for
    /// equality, hashing, and ordering.
    ///
    /// **Note**: This value is explicitly signed, so do not cast this to or treat this as an
    /// unsigned integer. Doing so will lead to incorrect results for values with differing
    /// signs.
    #[inline]
    const fn as_i128(self) -> i128 {
        let time = self.time.as_u64() as i128;
        let date = self.date.as_i32() as i128;
        (date << 64) | time
    }

    /// The smallest value that can be represented by `PrimitiveDateTime`.
    ///
    /// Depending on `large-dates` feature flag, value of this constant may vary.
    ///
    /// 1. With `large-dates` disabled it is equal to `-9999-01-01 00:00:00.0`
    /// 2. With `large-dates` enabled it is equal to `-999999-01-01 00:00:00.0`
    ///
    /// ```rust
    /// # use time::PrimitiveDateTime;
    /// # use time_macros::datetime;
    #[cfg_attr(
        feature = "large-dates",
        doc = "// Assuming `large-dates` feature is enabled."
    )]
    #[cfg_attr(
        feature = "large-dates",
        doc = "assert_eq!(PrimitiveDateTime::MIN, datetime!(-999999-01-01 0:00));"
    )]
    #[cfg_attr(
        not(feature = "large-dates"),
        doc = "// Assuming `large-dates` feature is disabled."
    )]
    #[cfg_attr(
        not(feature = "large-dates"),
        doc = "assert_eq!(PrimitiveDateTime::MIN, datetime!(-9999-01-01 0:00));"
    )]
    /// ```
    pub const MIN: Self = Self {
        date: Date::MIN,
        time: Time::MIDNIGHT,
    };

    /// The largest value that can be represented by `PrimitiveDateTime`.
    ///
    /// Depending on `large-dates` feature flag, value of this constant may vary.
    ///
    /// 1. With `large-dates` disabled it is equal to `9999-12-31 23:59:59.999_999_999`
    /// 2. With `large-dates` enabled it is equal to `999999-12-31 23:59:59.999_999_999`
    ///
    /// ```rust
    /// # use time::PrimitiveDateTime;
    /// # use time_macros::datetime;
    #[cfg_attr(
        feature = "large-dates",
        doc = "// Assuming `large-dates` feature is enabled."
    )]
    #[cfg_attr(
        feature = "large-dates",
        doc = "assert_eq!(PrimitiveDateTime::MAX, datetime!(+999999-12-31 23:59:59.999_999_999));"
    )]
    #[cfg_attr(
        not(feature = "large-dates"),
        doc = "// Assuming `large-dates` feature is disabled."
    )]
    #[cfg_attr(
        not(feature = "large-dates"),
        doc = "assert_eq!(PrimitiveDateTime::MAX, datetime!(+9999-12-31 23:59:59.999_999_999));"
    )]
    /// ```
    pub const MAX: Self = Self {
        date: Date::MAX,
        time: Time::MAX,
    };

    /// Create a new `PrimitiveDateTime` from the provided [`Date`] and [`Time`].
    ///
    /// ```rust
    /// # use time::PrimitiveDateTime;
    /// # use time_macros::{date, datetime, time};
    /// assert_eq!(
    ///     PrimitiveDateTime::new(date!(2019-01-01), time!(0:00)),
    ///     datetime!(2019-01-01 0:00),
    /// );
    /// ```
    #[inline]
    pub const fn new(date: Date, time: Time) -> Self {
        Self { date, time }
    }

    /// Get the [`Date`] component of the `PrimitiveDateTime`.
    ///
    /// ```rust
    /// # use time_macros::{date, datetime};
    /// assert_eq!(datetime!(2019-01-01 0:00).date(), date!(2019-01-01));
    /// ```
    #[inline]
    pub const fn date(self) -> Date {
        self.date
    }

    /// Get the [`Time`] component of the `PrimitiveDateTime`.
    ///
    /// ```rust
    /// # use time_macros::{datetime, time};
    /// assert_eq!(datetime!(2019-01-01 0:00).time(), time!(0:00));
    /// ```
    #[inline]
    pub const fn time(self) -> Time {
        self.time
    }

    /// Get the year of the date.
    ///
    /// ```rust
    /// # use time_macros::datetime;
    /// assert_eq!(datetime!(2019-01-01 0:00).year(), 2019);
    /// assert_eq!(datetime!(2019-12-31 0:00).year(), 2019);
    /// assert_eq!(datetime!(2020-01-01 0:00).year(), 2020);
    /// ```
    #[inline]
    pub const fn year(self) -> i32 {
        self.date().year()
    }

    /// Get the month of the date.
    ///
    /// ```rust
    /// # use time::Month;
    /// # use time_macros::datetime;
    /// assert_eq!(datetime!(2019-01-01 0:00).month(), Month::January);
    /// assert_eq!(datetime!(2019-12-31 0:00).month(), Month::December);
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
    /// # use time_macros::datetime;
    /// assert_eq!(datetime!(2019-01-01 0:00).day(), 1);
    /// assert_eq!(datetime!(2019-12-31 0:00).day(), 31);
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
    /// # use time_macros::datetime;
    /// assert_eq!(datetime!(2019-01-01 0:00).ordinal(), 1);
    /// assert_eq!(datetime!(2019-12-31 0:00).ordinal(), 365);
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
    /// # use time_macros::datetime;
    /// assert_eq!(datetime!(2019-01-01 0:00).iso_week(), 1);
    /// assert_eq!(datetime!(2019-10-04 0:00).iso_week(), 40);
    /// assert_eq!(datetime!(2020-01-01 0:00).iso_week(), 1);
    /// assert_eq!(datetime!(2020-12-31 0:00).iso_week(), 53);
    /// assert_eq!(datetime!(2021-01-01 0:00).iso_week(), 53);
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
    /// # use time_macros::datetime;
    /// assert_eq!(datetime!(2019-01-01 0:00).sunday_based_week(), 0);
    /// assert_eq!(datetime!(2020-01-01 0:00).sunday_based_week(), 0);
    /// assert_eq!(datetime!(2020-12-31 0:00).sunday_based_week(), 52);
    /// assert_eq!(datetime!(2021-01-01 0:00).sunday_based_week(), 0);
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
    /// # use time_macros::datetime;
    /// assert_eq!(datetime!(2019-01-01 0:00).monday_based_week(), 0);
    /// assert_eq!(datetime!(2020-01-01 0:00).monday_based_week(), 0);
    /// assert_eq!(datetime!(2020-12-31 0:00).monday_based_week(), 52);
    /// assert_eq!(datetime!(2021-01-01 0:00).monday_based_week(), 0);
    /// ```
    #[inline]
    pub const fn monday_based_week(self) -> u8 {
        self.date().monday_based_week()
    }

    /// Get the year, month, and day.
    ///
    /// ```rust
    /// # use time::Month;
    /// # use time_macros::datetime;
    /// assert_eq!(
    ///     datetime!(2019-01-01 0:00).to_calendar_date(),
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
    /// # use time_macros::datetime;
    /// assert_eq!(datetime!(2019-01-01 0:00).to_ordinal_date(), (2019, 1));
    /// ```
    #[inline]
    pub const fn to_ordinal_date(self) -> (i32, u16) {
        self.date().to_ordinal_date()
    }

    /// Get the ISO 8601 year, week number, and weekday.
    ///
    /// ```rust
    /// # use time::Weekday::*;
    /// # use time_macros::datetime;
    /// assert_eq!(
    ///     datetime!(2019-01-01 0:00).to_iso_week_date(),
    ///     (2019, 1, Tuesday)
    /// );
    /// assert_eq!(
    ///     datetime!(2019-10-04 0:00).to_iso_week_date(),
    ///     (2019, 40, Friday)
    /// );
    /// assert_eq!(
    ///     datetime!(2020-01-01 0:00).to_iso_week_date(),
    ///     (2020, 1, Wednesday)
    /// );
    /// assert_eq!(
    ///     datetime!(2020-12-31 0:00).to_iso_week_date(),
    ///     (2020, 53, Thursday)
    /// );
    /// assert_eq!(
    ///     datetime!(2021-01-01 0:00).to_iso_week_date(),
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
    /// # use time_macros::datetime;
    /// assert_eq!(datetime!(2019-01-01 0:00).weekday(), Tuesday);
    /// assert_eq!(datetime!(2019-02-01 0:00).weekday(), Friday);
    /// assert_eq!(datetime!(2019-03-01 0:00).weekday(), Friday);
    /// assert_eq!(datetime!(2019-04-01 0:00).weekday(), Monday);
    /// assert_eq!(datetime!(2019-05-01 0:00).weekday(), Wednesday);
    /// assert_eq!(datetime!(2019-06-01 0:00).weekday(), Saturday);
    /// assert_eq!(datetime!(2019-07-01 0:00).weekday(), Monday);
    /// assert_eq!(datetime!(2019-08-01 0:00).weekday(), Thursday);
    /// assert_eq!(datetime!(2019-09-01 0:00).weekday(), Sunday);
    /// assert_eq!(datetime!(2019-10-01 0:00).weekday(), Tuesday);
    /// assert_eq!(datetime!(2019-11-01 0:00).weekday(), Friday);
    /// assert_eq!(datetime!(2019-12-01 0:00).weekday(), Sunday);
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
    /// # use time_macros::datetime;
    /// assert_eq!(datetime!(-4713-11-24 0:00).to_julian_day(), 0);
    /// assert_eq!(datetime!(2000-01-01 0:00).to_julian_day(), 2_451_545);
    /// assert_eq!(datetime!(2019-01-01 0:00).to_julian_day(), 2_458_485);
    /// assert_eq!(datetime!(2019-12-31 0:00).to_julian_day(), 2_458_849);
    /// ```
    #[inline]
    pub const fn to_julian_day(self) -> i32 {
        self.date().to_julian_day()
    }

    /// Get the clock hour, minute, and second.
    ///
    /// ```rust
    /// # use time_macros::datetime;
    /// assert_eq!(datetime!(2020-01-01 0:00:00).as_hms(), (0, 0, 0));
    /// assert_eq!(datetime!(2020-01-01 23:59:59).as_hms(), (23, 59, 59));
    /// ```
    #[inline]
    pub const fn as_hms(self) -> (u8, u8, u8) {
        self.time().as_hms()
    }

    /// Get the clock hour, minute, second, and millisecond.
    ///
    /// ```rust
    /// # use time_macros::datetime;
    /// assert_eq!(datetime!(2020-01-01 0:00:00).as_hms_milli(), (0, 0, 0, 0));
    /// assert_eq!(
    ///     datetime!(2020-01-01 23:59:59.999).as_hms_milli(),
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
    /// # use time_macros::datetime;
    /// assert_eq!(datetime!(2020-01-01 0:00:00).as_hms_micro(), (0, 0, 0, 0));
    /// assert_eq!(
    ///     datetime!(2020-01-01 23:59:59.999_999).as_hms_micro(),
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
    /// # use time_macros::datetime;
    /// assert_eq!(datetime!(2020-01-01 0:00:00).as_hms_nano(), (0, 0, 0, 0));
    /// assert_eq!(
    ///     datetime!(2020-01-01 23:59:59.999_999_999).as_hms_nano(),
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
    /// # use time_macros::datetime;
    /// assert_eq!(datetime!(2019-01-01 0:00).hour(), 0);
    /// assert_eq!(datetime!(2019-01-01 23:59:59).hour(), 23);
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
    /// # use time_macros::datetime;
    /// assert_eq!(datetime!(2019-01-01 0:00).minute(), 0);
    /// assert_eq!(datetime!(2019-01-01 23:59:59).minute(), 59);
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
    /// # use time_macros::datetime;
    /// assert_eq!(datetime!(2019-01-01 0:00).second(), 0);
    /// assert_eq!(datetime!(2019-01-01 23:59:59).second(), 59);
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
    /// # use time_macros::datetime;
    /// assert_eq!(datetime!(2019-01-01 0:00).millisecond(), 0);
    /// assert_eq!(datetime!(2019-01-01 23:59:59.999).millisecond(), 999);
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
    /// # use time_macros::datetime;
    /// assert_eq!(datetime!(2019-01-01 0:00).microsecond(), 0);
    /// assert_eq!(
    ///     datetime!(2019-01-01 23:59:59.999_999).microsecond(),
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
    /// # use time_macros::datetime;
    /// assert_eq!(datetime!(2019-01-01 0:00).nanosecond(), 0);
    /// assert_eq!(
    ///     datetime!(2019-01-01 23:59:59.999_999_999).nanosecond(),
    ///     999_999_999,
    /// );
    /// ```
    #[inline]
    pub const fn nanosecond(self) -> u32 {
        self.time().nanosecond()
    }

    /// Assuming that the existing `PrimitiveDateTime` represents a moment in the provided
    /// [`UtcOffset`], return an [`OffsetDateTime`].
    ///
    /// ```rust
    /// # use time_macros::{datetime, offset};
    /// assert_eq!(
    ///     datetime!(2019-01-01 0:00)
    ///         .assume_offset(offset!(UTC))
    ///         .unix_timestamp(),
    ///     1_546_300_800,
    /// );
    /// assert_eq!(
    ///     datetime!(2019-01-01 0:00)
    ///         .assume_offset(offset!(-1))
    ///         .unix_timestamp(),
    ///     1_546_304_400,
    /// );
    /// ```
    #[inline]
    pub const fn assume_offset(self, offset: UtcOffset) -> OffsetDateTime {
        OffsetDateTime::new_in_offset(self.date, self.time, offset)
    }

    /// Assuming that the existing `PrimitiveDateTime` represents a moment in UTC, return an
    /// [`OffsetDateTime`].
    ///
    /// ```rust
    /// # use time_macros::datetime;
    /// assert_eq!(
    ///     datetime!(2019-01-01 0:00).assume_utc().unix_timestamp(),
    ///     1_546_300_800,
    /// );
    /// ```
    ///
    /// **Note**: You may want a [`UtcDateTime`] instead, which can be obtained with the
    /// [`PrimitiveDateTime::as_utc`] method.
    #[inline]
    pub const fn assume_utc(self) -> OffsetDateTime {
        self.assume_offset(UtcOffset::UTC)
    }

    /// Assuming that the existing `PrimitiveDateTime` represents a moment in UTC, return a
    /// [`UtcDateTime`].
    ///
    /// ```rust
    /// # use time_macros::datetime;
    /// assert_eq!(
    ///     datetime!(2019-01-01 0:00).as_utc().unix_timestamp(),
    ///     1_546_300_800,
    /// );
    /// ```
    #[inline]
    pub const fn as_utc(self) -> UtcDateTime {
        UtcDateTime::from_primitive(self)
    }

    /// Computes `self + duration`, returning `None` if an overflow occurred.
    ///
    /// ```
    /// # use time::{Date, ext::NumericalDuration};
    /// # use time_macros::datetime;
    /// let datetime = Date::MIN.midnight();
    /// assert_eq!(datetime.checked_add((-2).days()), None);
    ///
    /// let datetime = Date::MAX.midnight();
    /// assert_eq!(datetime.checked_add(1.days()), None);
    ///
    /// assert_eq!(
    ///     datetime!(2019-11-25 15:30).checked_add(27.hours()),
    ///     Some(datetime!(2019-11-26 18:30))
    /// );
    /// ```
    #[inline]
    pub const fn checked_add(self, duration: Duration) -> Option<Self> {
        let (date_adjustment, time) = self.time.adjusting_add(duration);
        let date = const_try_opt!(self.date.checked_add(duration));

        Some(Self {
            date: match date_adjustment {
                util::DateAdjustment::Previous => const_try_opt!(date.previous_day()),
                util::DateAdjustment::Next => const_try_opt!(date.next_day()),
                util::DateAdjustment::None => date,
            },
            time,
        })
    }

    /// Computes `self - duration`, returning `None` if an overflow occurred.
    ///
    /// ```
    /// # use time::{Date, ext::NumericalDuration};
    /// # use time_macros::datetime;
    /// let datetime = Date::MIN.midnight();
    /// assert_eq!(datetime.checked_sub(2.days()), None);
    ///
    /// let datetime = Date::MAX.midnight();
    /// assert_eq!(datetime.checked_sub((-1).days()), None);
    ///
    /// assert_eq!(
    ///     datetime!(2019-11-25 15:30).checked_sub(27.hours()),
    ///     Some(datetime!(2019-11-24 12:30))
    /// );
    /// ```
    #[inline]
    pub const fn checked_sub(self, duration: Duration) -> Option<Self> {
        let (date_adjustment, time) = self.time.adjusting_sub(duration);
        let date = const_try_opt!(self.date.checked_sub(duration));

        Some(Self {
            date: match date_adjustment {
                util::DateAdjustment::Previous => const_try_opt!(date.previous_day()),
                util::DateAdjustment::Next => const_try_opt!(date.next_day()),
                util::DateAdjustment::None => date,
            },
            time,
        })
    }

    /// Computes `self + duration`, saturating value on overflow.
    ///
    /// ```
    /// # use time::{PrimitiveDateTime, ext::NumericalDuration};
    /// # use time_macros::datetime;
    /// assert_eq!(
    ///     PrimitiveDateTime::MIN.saturating_add((-2).days()),
    ///     PrimitiveDateTime::MIN
    /// );
    ///
    /// assert_eq!(
    ///     PrimitiveDateTime::MAX.saturating_add(2.days()),
    ///     PrimitiveDateTime::MAX
    /// );
    ///
    /// assert_eq!(
    ///     datetime!(2019-11-25 15:30).saturating_add(27.hours()),
    ///     datetime!(2019-11-26 18:30)
    /// );
    /// ```
    #[inline]
    pub const fn saturating_add(self, duration: Duration) -> Self {
        if let Some(datetime) = self.checked_add(duration) {
            datetime
        } else if duration.is_negative() {
            Self::MIN
        } else {
            Self::MAX
        }
    }

    /// Computes `self - duration`, saturating value on overflow.
    ///
    /// ```
    /// # use time::{PrimitiveDateTime, ext::NumericalDuration};
    /// # use time_macros::datetime;
    /// assert_eq!(
    ///     PrimitiveDateTime::MIN.saturating_sub(2.days()),
    ///     PrimitiveDateTime::MIN
    /// );
    ///
    /// assert_eq!(
    ///     PrimitiveDateTime::MAX.saturating_sub((-2).days()),
    ///     PrimitiveDateTime::MAX
    /// );
    ///
    /// assert_eq!(
    ///     datetime!(2019-11-25 15:30).saturating_sub(27.hours()),
    ///     datetime!(2019-11-24 12:30)
    /// );
    /// ```
    #[inline]
    pub const fn saturating_sub(self, duration: Duration) -> Self {
        if let Some(datetime) = self.checked_sub(duration) {
            datetime
        } else if duration.is_negative() {
            Self::MAX
        } else {
            Self::MIN
        }
    }
}

/// Methods that replace part of the `PrimitiveDateTime`.
impl PrimitiveDateTime {
    /// Replace the time, preserving the date.
    ///
    /// ```rust
    /// # use time_macros::{datetime, time};
    /// assert_eq!(
    ///     datetime!(2020-01-01 17:00).replace_time(time!(5:00)),
    ///     datetime!(2020-01-01 5:00)
    /// );
    /// ```
    #[must_use = "This method does not mutate the original `PrimitiveDateTime`."]
    #[inline]
    pub const fn replace_time(self, time: Time) -> Self {
        Self {
            date: self.date,
            time,
        }
    }

    /// Replace the date, preserving the time.
    ///
    /// ```rust
    /// # use time_macros::{datetime, date};
    /// assert_eq!(
    ///     datetime!(2020-01-01 12:00).replace_date(date!(2020-01-30)),
    ///     datetime!(2020-01-30 12:00)
    /// );
    /// ```
    #[must_use = "This method does not mutate the original `PrimitiveDateTime`."]
    #[inline]
    pub const fn replace_date(self, date: Date) -> Self {
        Self {
            date,
            time: self.time,
        }
    }

    /// Replace the year. The month and day will be unchanged.
    ///
    /// ```rust
    /// # use time_macros::datetime;
    /// assert_eq!(
    ///     datetime!(2022-02-18 12:00).replace_year(2019),
    ///     Ok(datetime!(2019-02-18 12:00))
    /// );
    /// assert!(datetime!(2022-02-18 12:00).replace_year(-1_000_000_000).is_err()); // -1_000_000_000 isn't a valid year
    /// assert!(datetime!(2022-02-18 12:00).replace_year(1_000_000_000).is_err()); // 1_000_000_000 isn't a valid year
    /// ```
    #[must_use = "This method does not mutate the original `PrimitiveDateTime`."]
    #[inline]
    pub const fn replace_year(self, year: i32) -> Result<Self, error::ComponentRange> {
        Ok(Self {
            date: const_try!(self.date.replace_year(year)),
            time: self.time,
        })
    }

    /// Replace the month of the year.
    ///
    /// ```rust
    /// # use time_macros::datetime;
    /// # use time::Month;
    /// assert_eq!(
    ///     datetime!(2022-02-18 12:00).replace_month(Month::January),
    ///     Ok(datetime!(2022-01-18 12:00))
    /// );
    /// assert!(datetime!(2022-01-30 12:00).replace_month(Month::February).is_err()); // 30 isn't a valid day in February
    /// ```
    #[must_use = "This method does not mutate the original `PrimitiveDateTime`."]
    #[inline]
    pub const fn replace_month(self, month: Month) -> Result<Self, error::ComponentRange> {
        Ok(Self {
            date: const_try!(self.date.replace_month(month)),
            time: self.time,
        })
    }

    /// Replace the day of the month.
    ///
    /// ```rust
    /// # use time_macros::datetime;
    /// assert_eq!(
    ///     datetime!(2022-02-18 12:00).replace_day(1),
    ///     Ok(datetime!(2022-02-01 12:00))
    /// );
    /// assert!(datetime!(2022-02-18 12:00).replace_day(0).is_err()); // 00 isn't a valid day
    /// assert!(datetime!(2022-02-18 12:00).replace_day(30).is_err()); // 30 isn't a valid day in February
    /// ```
    #[must_use = "This method does not mutate the original `PrimitiveDateTime`."]
    #[inline]
    pub const fn replace_day(self, day: u8) -> Result<Self, error::ComponentRange> {
        Ok(Self {
            date: const_try!(self.date.replace_day(day)),
            time: self.time,
        })
    }

    /// Replace the day of the year.
    ///
    /// ```rust
    /// # use time_macros::datetime;
    /// assert_eq!(datetime!(2022-049 12:00).replace_ordinal(1), Ok(datetime!(2022-001 12:00)));
    /// assert!(datetime!(2022-049 12:00).replace_ordinal(0).is_err()); // 0 isn't a valid ordinal
    /// assert!(datetime!(2022-049 12:00).replace_ordinal(366).is_err()); // 2022 isn't a leap year
    /// ````
    #[must_use = "This method does not mutate the original `PrimitiveDateTime`."]
    #[inline]
    pub const fn replace_ordinal(self, ordinal: u16) -> Result<Self, error::ComponentRange> {
        Ok(Self {
            date: const_try!(self.date.replace_ordinal(ordinal)),
            time: self.time,
        })
    }

    /// Replace the clock hour.
    ///
    /// ```rust
    /// # use time_macros::datetime;
    /// assert_eq!(
    ///     datetime!(2022-02-18 01:02:03.004_005_006).replace_hour(7),
    ///     Ok(datetime!(2022-02-18 07:02:03.004_005_006))
    /// );
    /// assert!(datetime!(2022-02-18 01:02:03.004_005_006).replace_hour(24).is_err()); // 24 isn't a valid hour
    /// ```
    #[must_use = "This method does not mutate the original `PrimitiveDateTime`."]
    #[inline]
    pub const fn replace_hour(self, hour: u8) -> Result<Self, error::ComponentRange> {
        Ok(Self {
            date: self.date,
            time: const_try!(self.time.replace_hour(hour)),
        })
    }

    /// Replace the minutes within the hour.
    ///
    /// ```rust
    /// # use time_macros::datetime;
    /// assert_eq!(
    ///     datetime!(2022-02-18 01:02:03.004_005_006).replace_minute(7),
    ///     Ok(datetime!(2022-02-18 01:07:03.004_005_006))
    /// );
    /// assert!(datetime!(2022-02-18 01:02:03.004_005_006).replace_minute(60).is_err()); // 60 isn't a valid minute
    /// ```
    #[must_use = "This method does not mutate the original `PrimitiveDateTime`."]
    #[inline]
    pub const fn replace_minute(self, minute: u8) -> Result<Self, error::ComponentRange> {
        Ok(Self {
            date: self.date,
            time: const_try!(self.time.replace_minute(minute)),
        })
    }

    /// Replace the seconds within the minute.
    ///
    /// ```rust
    /// # use time_macros::datetime;
    /// assert_eq!(
    ///     datetime!(2022-02-18 01:02:03.004_005_006).replace_second(7),
    ///     Ok(datetime!(2022-02-18 01:02:07.004_005_006))
    /// );
    /// assert!(datetime!(2022-02-18 01:02:03.004_005_006).replace_second(60).is_err()); // 60 isn't a valid second
    /// ```
    #[must_use = "This method does not mutate the original `PrimitiveDateTime`."]
    #[inline]
    pub const fn replace_second(self, second: u8) -> Result<Self, error::ComponentRange> {
        Ok(Self {
            date: self.date,
            time: const_try!(self.time.replace_second(second)),
        })
    }

    /// Replace the milliseconds within the second.
    ///
    /// ```rust
    /// # use time_macros::datetime;
    /// assert_eq!(
    ///     datetime!(2022-02-18 01:02:03.004_005_006).replace_millisecond(7),
    ///     Ok(datetime!(2022-02-18 01:02:03.007))
    /// );
    /// assert!(datetime!(2022-02-18 01:02:03.004_005_006).replace_millisecond(1_000).is_err()); // 1_000 isn't a valid millisecond
    /// ```
    #[must_use = "This method does not mutate the original `PrimitiveDateTime`."]
    #[inline]
    pub const fn replace_millisecond(
        self,
        millisecond: u16,
    ) -> Result<Self, error::ComponentRange> {
        Ok(Self {
            date: self.date,
            time: const_try!(self.time.replace_millisecond(millisecond)),
        })
    }

    /// Replace the microseconds within the second.
    ///
    /// ```rust
    /// # use time_macros::datetime;
    /// assert_eq!(
    ///     datetime!(2022-02-18 01:02:03.004_005_006).replace_microsecond(7_008),
    ///     Ok(datetime!(2022-02-18 01:02:03.007_008))
    /// );
    /// assert!(datetime!(2022-02-18 01:02:03.004_005_006).replace_microsecond(1_000_000).is_err()); // 1_000_000 isn't a valid microsecond
    /// ```
    #[must_use = "This method does not mutate the original `PrimitiveDateTime`."]
    #[inline]
    pub const fn replace_microsecond(
        self,
        microsecond: u32,
    ) -> Result<Self, error::ComponentRange> {
        Ok(Self {
            date: self.date,
            time: const_try!(self.time.replace_microsecond(microsecond)),
        })
    }

    /// Replace the nanoseconds within the second.
    ///
    /// ```rust
    /// # use time_macros::datetime;
    /// assert_eq!(
    ///     datetime!(2022-02-18 01:02:03.004_005_006).replace_nanosecond(7_008_009),
    ///     Ok(datetime!(2022-02-18 01:02:03.007_008_009))
    /// );
    /// assert!(datetime!(2022-02-18 01:02:03.004_005_006).replace_nanosecond(1_000_000_000).is_err()); // 1_000_000_000 isn't a valid nanosecond
    /// ```
    #[must_use = "This method does not mutate the original `PrimitiveDateTime`."]
    #[inline]
    pub const fn replace_nanosecond(self, nanosecond: u32) -> Result<Self, error::ComponentRange> {
        Ok(Self {
            date: self.date,
            time: const_try!(self.time.replace_nanosecond(nanosecond)),
        })
    }
}

#[cfg(feature = "formatting")]
impl PrimitiveDateTime {
    /// Format the `PrimitiveDateTime` using the provided [format
    /// description](crate::format_description).
    #[inline]
    pub fn format_into(
        self,
        output: &mut (impl io::Write + ?Sized),
        format: &(impl Formattable + ?Sized),
    ) -> Result<usize, error::Format> {
        format.format_into(output, Some(self.date), Some(self.time), None)
    }

    /// Format the `PrimitiveDateTime` using the provided [format
    /// description](crate::format_description).
    ///
    /// ```rust
    /// # use time::format_description;
    /// # use time_macros::datetime;
    /// let format = format_description::parse("[year]-[month]-[day] [hour]:[minute]:[second]")?;
    /// assert_eq!(
    ///     datetime!(2020-01-02 03:04:05).format(&format)?,
    ///     "2020-01-02 03:04:05"
    /// );
    /// # Ok::<_, time::Error>(())
    /// ```
    #[inline]
    pub fn format(self, format: &(impl Formattable + ?Sized)) -> Result<String, error::Format> {
        format.format(Some(self.date), Some(self.time), None)
    }
}

#[cfg(feature = "parsing")]
impl PrimitiveDateTime {
    /// Parse a `PrimitiveDateTime` from the input using the provided [format
    /// description](crate::format_description).
    ///
    /// ```rust
    /// # use time::PrimitiveDateTime;
    /// # use time_macros::{datetime, format_description};
    /// let format = format_description!("[year]-[month]-[day] [hour]:[minute]:[second]");
    /// assert_eq!(
    ///     PrimitiveDateTime::parse("2020-01-02 03:04:05", &format)?,
    ///     datetime!(2020-01-02 03:04:05)
    /// );
    /// # Ok::<_, time::Error>(())
    /// ```
    #[inline]
    pub fn parse(
        input: &str,
        description: &(impl Parsable + ?Sized),
    ) -> Result<Self, error::Parse> {
        description.parse_primitive_date_time(input.as_bytes())
    }
}

impl SmartDisplay for PrimitiveDateTime {
    type Metadata = ();

    #[inline]
    fn metadata(&self, _: FormatterOptions) -> Metadata<'_, Self> {
        let width = smart_display::padded_width_of!(self.date, " ", self.time);
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
            format_args!("{} {}", self.date, self.time),
        )
    }
}

impl fmt::Display for PrimitiveDateTime {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        SmartDisplay::fmt(self, f)
    }
}

impl fmt::Debug for PrimitiveDateTime {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl Add<Duration> for PrimitiveDateTime {
    type Output = Self;

    /// # Panics
    ///
    /// This may panic if an overflow occurs.
    #[inline]
    #[track_caller]
    fn add(self, duration: Duration) -> Self::Output {
        self.checked_add(duration)
            .expect("resulting value is out of range")
    }
}

impl Add<StdDuration> for PrimitiveDateTime {
    type Output = Self;

    /// # Panics
    ///
    /// This may panic if an overflow occurs.
    #[inline]
    #[track_caller]
    fn add(self, duration: StdDuration) -> Self::Output {
        let (is_next_day, time) = self.time.adjusting_add_std(duration);

        Self {
            date: if is_next_day {
                (self.date + duration)
                    .next_day()
                    .expect("resulting value is out of range")
            } else {
                self.date + duration
            },
            time,
        }
    }
}

impl AddAssign<Duration> for PrimitiveDateTime {
    /// # Panics
    ///
    /// This may panic if an overflow occurs.
    #[inline]
    #[track_caller]
    fn add_assign(&mut self, duration: Duration) {
        *self = *self + duration;
    }
}

impl AddAssign<StdDuration> for PrimitiveDateTime {
    /// # Panics
    ///
    /// This may panic if an overflow occurs.
    #[inline]
    #[track_caller]
    fn add_assign(&mut self, duration: StdDuration) {
        *self = *self + duration;
    }
}

impl Sub<Duration> for PrimitiveDateTime {
    type Output = Self;

    /// # Panics
    ///
    /// This may panic if an overflow occurs.
    #[inline]
    #[track_caller]
    fn sub(self, duration: Duration) -> Self::Output {
        self.checked_sub(duration)
            .expect("resulting value is out of range")
    }
}

impl Sub<StdDuration> for PrimitiveDateTime {
    type Output = Self;

    /// # Panics
    ///
    /// This may panic if an overflow occurs.
    #[inline]
    #[track_caller]
    fn sub(self, duration: StdDuration) -> Self::Output {
        let (is_previous_day, time) = self.time.adjusting_sub_std(duration);

        Self {
            date: if is_previous_day {
                (self.date - duration)
                    .previous_day()
                    .expect("resulting value is out of range")
            } else {
                self.date - duration
            },
            time,
        }
    }
}

impl SubAssign<Duration> for PrimitiveDateTime {
    /// # Panics
    ///
    /// This may panic if an overflow occurs.
    #[inline]
    #[track_caller]
    fn sub_assign(&mut self, duration: Duration) {
        *self = *self - duration;
    }
}

impl SubAssign<StdDuration> for PrimitiveDateTime {
    /// # Panics
    ///
    /// This may panic if an overflow occurs.
    #[inline]
    #[track_caller]
    fn sub_assign(&mut self, duration: StdDuration) {
        *self = *self - duration;
    }
}

impl Sub for PrimitiveDateTime {
    type Output = Duration;

    /// # Panics
    ///
    /// This may panic if an overflow occurs.
    #[inline]
    #[track_caller]
    fn sub(self, rhs: Self) -> Self::Output {
        (self.date - rhs.date) + (self.time - rhs.time)
    }
}
