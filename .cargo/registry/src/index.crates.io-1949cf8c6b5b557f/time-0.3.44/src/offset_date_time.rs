//! The [`OffsetDateTime`] struct and its associated `impl`s.

#[cfg(feature = "formatting")]
use alloc::string::String;
use core::cmp::Ordering;
use core::fmt;
use core::hash::Hash;
use core::ops::{Add, AddAssign, Sub, SubAssign};
use core::time::Duration as StdDuration;
#[cfg(feature = "formatting")]
use std::io;

use deranged::RangedI64;
use num_conv::prelude::*;
use powerfmt::ext::FormatterExt as _;
use powerfmt::smart_display::{self, FormatterOptions, Metadata, SmartDisplay};
use time_core::convert::*;

use crate::date::{MAX_YEAR, MIN_YEAR};
#[cfg(feature = "formatting")]
use crate::formatting::Formattable;
use crate::internal_macros::{
    carry, cascade, const_try, const_try_opt, div_floor, ensure_ranged, expect_opt,
};
#[cfg(feature = "parsing")]
use crate::parsing::Parsable;
use crate::{
    error, util, Date, Duration, Month, PrimitiveDateTime, Time, UtcDateTime, UtcOffset, Weekday,
};

/// The Julian day of the Unix epoch.
const UNIX_EPOCH_JULIAN_DAY: i32 = OffsetDateTime::UNIX_EPOCH.to_julian_day();

/// A [`PrimitiveDateTime`] with a [`UtcOffset`].
#[derive(Clone, Copy, Eq)]
pub struct OffsetDateTime {
    local_date_time: PrimitiveDateTime,
    offset: UtcOffset,
}

impl PartialEq for OffsetDateTime {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        raw_to_bits((self.year(), self.ordinal(), self.time()))
            == raw_to_bits(other.to_offset_raw(self.offset()))
    }
}

impl PartialOrd for OffsetDateTime {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for OffsetDateTime {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        raw_to_bits((self.year(), self.ordinal(), self.time()))
            .cmp(&raw_to_bits(other.to_offset_raw(self.offset())))
    }
}

impl Hash for OffsetDateTime {
    #[inline]
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        raw_to_bits(self.to_utc_raw()).hash(state);
    }
}

/// **Note**: This value is explicitly signed, so do not cast this to or treat this as an
/// unsigned integer. Doing so will lead to incorrect results for values with differing
/// signs.
#[inline]
const fn raw_to_bits((year, ordinal, time): (i32, u16, Time)) -> i128 {
    ((year as i128) << 74) | ((ordinal as i128) << 64) | (time.as_u64() as i128)
}

impl OffsetDateTime {
    /// Midnight, 1 January, 1970 (UTC).
    ///
    /// ```rust
    /// # use time::OffsetDateTime;
    /// # use time_macros::datetime;
    /// assert_eq!(OffsetDateTime::UNIX_EPOCH, datetime!(1970-01-01 0:00 UTC));
    /// ```
    pub const UNIX_EPOCH: Self =
        Self::new_in_offset(Date::UNIX_EPOCH, Time::MIDNIGHT, UtcOffset::UTC);

    /// Create a new `OffsetDateTime` with the current date and time in UTC.
    ///
    /// ```rust
    /// # use time::OffsetDateTime;
    /// # use time_macros::offset;
    /// assert!(OffsetDateTime::now_utc().year() >= 2019);
    /// assert_eq!(OffsetDateTime::now_utc().offset(), offset!(UTC));
    /// ```
    #[cfg(feature = "std")]
    #[inline]
    pub fn now_utc() -> Self {
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

    /// Attempt to create a new `OffsetDateTime` with the current date and time in the local offset.
    /// If the offset cannot be determined, an error is returned.
    ///
    /// ```rust
    /// # use time::OffsetDateTime;
    /// # if false {
    /// assert!(OffsetDateTime::now_local().is_ok());
    /// # }
    /// ```
    #[cfg(feature = "local-offset")]
    #[inline]
    pub fn now_local() -> Result<Self, error::IndeterminateOffset> {
        let t = Self::now_utc();
        Ok(t.to_offset(UtcOffset::local_offset_at(t)?))
    }

    /// Create a new `OffsetDateTime` with the given [`Date`], [`Time`], and [`UtcOffset`].
    ///
    /// ```
    /// # use time::{Date, Month, OffsetDateTime, Time, UtcOffset};
    /// # use time_macros::datetime;
    /// let dt = OffsetDateTime::new_in_offset(
    ///     Date::from_calendar_date(2024, Month::January, 1)?,
    ///     Time::from_hms_nano(12, 59, 59, 500_000_000)?,
    ///     UtcOffset::from_hms(-5, 0, 0)?,
    /// );
    /// assert_eq!(dt, datetime!(2024-01-01 12:59:59.5 -5));
    /// # Ok::<_, time::error::Error>(())
    /// ```
    #[inline]
    pub const fn new_in_offset(date: Date, time: Time, offset: UtcOffset) -> Self {
        Self {
            local_date_time: date.with_time(time),
            offset,
        }
    }

    /// Create a new `OffsetDateTime` with the given [`Date`] and [`Time`] in the UTC timezone.
    ///
    /// ```
    /// # use time::{Date, Month, OffsetDateTime, Time};
    /// # use time_macros::datetime;
    /// let dt = OffsetDateTime::new_utc(
    ///     Date::from_calendar_date(2024, Month::January, 1)?,
    ///     Time::from_hms_nano(12, 59, 59, 500_000_000)?,
    /// );
    /// assert_eq!(dt, datetime!(2024-01-01 12:59:59.5 UTC));
    /// # Ok::<_, time::error::Error>(())
    /// ```
    #[inline]
    pub const fn new_utc(date: Date, time: Time) -> Self {
        PrimitiveDateTime::new(date, time).assume_utc()
    }

    /// Convert the `OffsetDateTime` from the current [`UtcOffset`] to the provided [`UtcOffset`].
    ///
    /// ```rust
    /// # use time_macros::{datetime, offset};
    /// assert_eq!(
    ///     datetime!(2000-01-01 0:00 UTC)
    ///         .to_offset(offset!(-1))
    ///         .year(),
    ///     1999,
    /// );
    ///
    /// // Let's see what time Sydney's new year's celebration is in New York and Los Angeles.
    ///
    /// // Construct midnight on new year's in Sydney.
    /// let sydney = datetime!(2000-01-01 0:00 +11);
    /// let new_york = sydney.to_offset(offset!(-5));
    /// let los_angeles = sydney.to_offset(offset!(-8));
    /// assert_eq!(sydney.hour(), 0);
    /// assert_eq!(new_york.hour(), 8);
    /// assert_eq!(los_angeles.hour(), 5);
    /// ```
    ///
    /// # Panics
    ///
    /// This method panics if the local date-time in the new offset is outside the supported range.
    #[inline]
    #[track_caller]
    pub const fn to_offset(self, offset: UtcOffset) -> Self {
        expect_opt!(
            self.checked_to_offset(offset),
            "local datetime out of valid range"
        )
    }

    /// Convert the `OffsetDateTime` from the current [`UtcOffset`] to the provided [`UtcOffset`],
    /// returning `None` if the date-time in the resulting offset is invalid.
    ///
    /// ```rust
    /// # use time::PrimitiveDateTime;
    /// # use time_macros::{datetime, offset};
    /// assert_eq!(
    ///     datetime!(2000-01-01 0:00 UTC)
    ///         .checked_to_offset(offset!(-1))
    ///         .unwrap()
    ///         .year(),
    ///     1999,
    /// );
    /// assert_eq!(
    ///     PrimitiveDateTime::MAX
    ///         .assume_utc()
    ///         .checked_to_offset(offset!(+1)),
    ///     None,
    /// );
    /// ```
    #[inline]
    pub const fn checked_to_offset(self, offset: UtcOffset) -> Option<Self> {
        if self.offset.as_u32() == offset.as_u32() {
            return Some(self);
        }

        let (year, ordinal, time) = self.to_offset_raw(offset);

        if year > MAX_YEAR || year < MIN_YEAR {
            return None;
        }

        Some(Self::new_in_offset(
            // Safety: `ordinal` is not zero.
            unsafe { Date::__from_ordinal_date_unchecked(year, ordinal) },
            time,
            offset,
        ))
    }

    /// Convert the `OffsetDateTime` from the current [`UtcOffset`] to UTC, returning a
    /// [`UtcDateTime`].
    ///
    /// ```rust
    /// # use time_macros::datetime;
    /// assert_eq!(
    ///     datetime!(2000-01-01 0:00 +1)
    ///         .to_utc()
    ///         .year(),
    ///     1999,
    /// );
    /// ```
    ///
    /// # Panics
    ///
    /// This method panics if the UTC date-time is outside the supported range.
    #[inline]
    #[track_caller]
    pub const fn to_utc(self) -> UtcDateTime {
        expect_opt!(self.checked_to_utc(), "local datetime out of valid range")
    }

    /// Convert the `OffsetDateTime` from the current [`UtcOffset`] to UTC, returning `None` if the
    /// UTC date-time is invalid. Returns a [`UtcDateTime`].
    ///
    /// ```rust
    /// # use time_macros::datetime;
    /// assert_eq!(
    ///     datetime!(2000-01-01 0:00 +1)
    ///         .checked_to_utc()
    ///         .unwrap()
    ///         .year(),
    ///     1999,
    /// );
    /// assert_eq!(
    #[cfg_attr(
        feature = "large-dates",
        doc = "    datetime!(+999999-12-31 23:59:59 -1).checked_to_utc(),"
    )]
    #[cfg_attr(
        not(feature = "large-dates"),
        doc = "    datetime!(9999-12-31 23:59:59 -1).checked_to_utc(),"
    )]
    ///     None,
    /// );
    /// ```
    #[inline]
    pub const fn checked_to_utc(self) -> Option<UtcDateTime> {
        if self.offset.is_utc() {
            return Some(self.local_date_time.as_utc());
        }

        let (year, ordinal, time) = self.to_utc_raw();

        if year > MAX_YEAR || year < MIN_YEAR {
            return None;
        }

        Some(UtcDateTime::new(
            // Safety: `ordinal` is not zero.
            unsafe { Date::__from_ordinal_date_unchecked(year, ordinal) },
            time,
        ))
    }

    /// Equivalent to `.to_utc()`, but returning the year, ordinal, and time. This avoids
    /// constructing an invalid [`Date`] if the new value is out of range.
    #[inline]
    pub(crate) const fn to_utc_raw(self) -> (i32, u16, Time) {
        let from = self.offset;

        // Fast path for when no conversion is necessary.
        if from.is_utc() {
            return (self.year(), self.ordinal(), self.time());
        }

        let (second, carry) = carry!(@most_once
            self.second() as i8 - from.seconds_past_minute(),
            0..Second::per_t(Minute)
        );
        let (minute, carry) = carry!(@most_once
            self.minute() as i8 - from.minutes_past_hour() + carry,
            0..Minute::per_t(Hour)
        );
        let (hour, carry) = carry!(@most_twice
            self.hour() as i8 - from.whole_hours() + carry,
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

    /// Equivalent to `.to_offset(offset)`, but returning the year, ordinal, and time. This avoids
    /// constructing an invalid [`Date`] if the new value is out of range.
    #[inline]
    pub(crate) const fn to_offset_raw(self, offset: UtcOffset) -> (i32, u16, Time) {
        let from = self.offset;
        let to = offset;

        // Fast path for when no conversion is necessary.
        if from.as_u32() == to.as_u32() {
            return (self.year(), self.ordinal(), self.time());
        }

        let (second, carry) = carry!(@most_twice
            self.second() as i16 - from.seconds_past_minute() as i16
                + to.seconds_past_minute() as i16,
            0..Second::per_t(Minute)
        );
        let (minute, carry) = carry!(@most_twice
            self.minute() as i16 - from.minutes_past_hour() as i16
                + to.minutes_past_hour() as i16
                + carry,
            0..Minute::per_t(Hour)
        );
        let (hour, carry) = carry!(@most_thrice
            self.hour() as i8 - from.whole_hours() + to.whole_hours() + carry,
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

    /// Create an `OffsetDateTime` from the provided Unix timestamp. Calling `.offset()` on the
    /// resulting value is guaranteed to return UTC.
    ///
    /// ```rust
    /// # use time::OffsetDateTime;
    /// # use time_macros::datetime;
    /// assert_eq!(
    ///     OffsetDateTime::from_unix_timestamp(0),
    ///     Ok(OffsetDateTime::UNIX_EPOCH),
    /// );
    /// assert_eq!(
    ///     OffsetDateTime::from_unix_timestamp(1_546_300_800),
    ///     Ok(datetime!(2019-01-01 0:00 UTC)),
    /// );
    /// ```
    ///
    /// If you have a timestamp-nanosecond pair, you can use something along the lines of the
    /// following:
    ///
    /// ```rust
    /// # use time::{Duration, OffsetDateTime, ext::NumericalDuration};
    /// let (timestamp, nanos) = (1, 500_000_000);
    /// assert_eq!(
    ///     OffsetDateTime::from_unix_timestamp(timestamp)? + Duration::nanoseconds(nanos),
    ///     OffsetDateTime::UNIX_EPOCH + 1.5.seconds()
    /// );
    /// # Ok::<_, time::Error>(())
    /// ```
    #[inline]
    pub const fn from_unix_timestamp(timestamp: i64) -> Result<Self, error::ComponentRange> {
        type Timestamp = RangedI64<
            {
                OffsetDateTime::new_in_offset(Date::MIN, Time::MIDNIGHT, UtcOffset::UTC)
                    .unix_timestamp()
            },
            {
                OffsetDateTime::new_in_offset(Date::MAX, Time::MAX, UtcOffset::UTC).unix_timestamp()
            },
        >;
        ensure_ranged!(Timestamp: timestamp);

        // Use the unchecked method here, as the input validity has already been verified.
        // Safety: The Julian day number is in range.
        let date = unsafe {
            Date::from_julian_day_unchecked(
                UNIX_EPOCH_JULIAN_DAY + div_floor!(timestamp, Second::per_t::<i64>(Day)) as i32,
            )
        };

        let seconds_within_day = timestamp.rem_euclid(Second::per_t(Day));
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

        Ok(Self::new_in_offset(date, time, UtcOffset::UTC))
    }

    /// Construct an `OffsetDateTime` from the provided Unix timestamp (in nanoseconds). Calling
    /// `.offset()` on the resulting value is guaranteed to return UTC.
    ///
    /// ```rust
    /// # use time::OffsetDateTime;
    /// # use time_macros::datetime;
    /// assert_eq!(
    ///     OffsetDateTime::from_unix_timestamp_nanos(0),
    ///     Ok(OffsetDateTime::UNIX_EPOCH),
    /// );
    /// assert_eq!(
    ///     OffsetDateTime::from_unix_timestamp_nanos(1_546_300_800_000_000_000),
    ///     Ok(datetime!(2019-01-01 0:00 UTC)),
    /// );
    /// ```
    #[inline]
    pub const fn from_unix_timestamp_nanos(timestamp: i128) -> Result<Self, error::ComponentRange> {
        let datetime = const_try!(Self::from_unix_timestamp(div_floor!(
            timestamp,
            Nanosecond::per_t::<i128>(Second)
        ) as i64));

        Ok(Self::new_in_offset(
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
            UtcOffset::UTC,
        ))
    }

    /// Get the [`UtcOffset`].
    ///
    /// ```rust
    /// # use time_macros::{datetime, offset};
    /// assert_eq!(datetime!(2019-01-01 0:00 UTC).offset(), offset!(UTC));
    /// assert_eq!(datetime!(2019-01-01 0:00 +1).offset(), offset!(+1));
    /// ```
    #[inline]
    pub const fn offset(self) -> UtcOffset {
        self.offset
    }

    /// Get the [Unix timestamp](https://en.wikipedia.org/wiki/Unix_time).
    ///
    /// ```rust
    /// # use time_macros::datetime;
    /// assert_eq!(datetime!(1970-01-01 0:00 UTC).unix_timestamp(), 0);
    /// assert_eq!(datetime!(1970-01-01 0:00 -1).unix_timestamp(), 3_600);
    /// ```
    #[inline]
    pub const fn unix_timestamp(self) -> i64 {
        let days = (self.to_julian_day() as i64 - UNIX_EPOCH_JULIAN_DAY as i64)
            * Second::per_t::<i64>(Day);
        let hours = self.hour() as i64 * Second::per_t::<i64>(Hour);
        let minutes = self.minute() as i64 * Second::per_t::<i64>(Minute);
        let seconds = self.second() as i64;
        let offset_seconds = self.offset.whole_seconds() as i64;
        days + hours + minutes + seconds - offset_seconds
    }

    /// Get the Unix timestamp in nanoseconds.
    ///
    /// ```rust
    /// use time_macros::datetime;
    /// assert_eq!(datetime!(1970-01-01 0:00 UTC).unix_timestamp_nanos(), 0);
    /// assert_eq!(
    ///     datetime!(1970-01-01 0:00 -1).unix_timestamp_nanos(),
    ///     3_600_000_000_000,
    /// );
    /// ```
    #[inline]
    pub const fn unix_timestamp_nanos(self) -> i128 {
        self.unix_timestamp() as i128 * Nanosecond::per_t::<i128>(Second)
            + self.nanosecond() as i128
    }

    /// Get the [`PrimitiveDateTime`] in the stored offset.
    #[inline]
    pub(crate) const fn date_time(self) -> PrimitiveDateTime {
        self.local_date_time
    }

    /// Get the [`Date`] in the stored offset.
    ///
    /// ```rust
    /// # use time_macros::{date, datetime, offset};
    /// assert_eq!(datetime!(2019-01-01 0:00 UTC).date(), date!(2019-01-01));
    /// assert_eq!(
    ///     datetime!(2019-01-01 0:00 UTC)
    ///         .to_offset(offset!(-1))
    ///         .date(),
    ///     date!(2018-12-31),
    /// );
    /// ```
    #[inline]
    pub const fn date(self) -> Date {
        self.date_time().date()
    }

    /// Get the [`Time`] in the stored offset.
    ///
    /// ```rust
    /// # use time_macros::{datetime, offset, time};
    /// assert_eq!(datetime!(2019-01-01 0:00 UTC).time(), time!(0:00));
    /// assert_eq!(
    ///     datetime!(2019-01-01 0:00 UTC)
    ///         .to_offset(offset!(-1))
    ///         .time(),
    ///     time!(23:00)
    /// );
    /// ```
    #[inline]
    pub const fn time(self) -> Time {
        self.date_time().time()
    }

    /// Get the year of the date in the stored offset.
    ///
    /// ```rust
    /// # use time_macros::{datetime, offset};
    /// assert_eq!(datetime!(2019-01-01 0:00 UTC).year(), 2019);
    /// assert_eq!(
    ///     datetime!(2019-12-31 23:00 UTC)
    ///         .to_offset(offset!(+1))
    ///         .year(),
    ///     2020,
    /// );
    /// assert_eq!(datetime!(2020-01-01 0:00 UTC).year(), 2020);
    /// ```
    #[inline]
    pub const fn year(self) -> i32 {
        self.date().year()
    }

    /// Get the month of the date in the stored offset.
    ///
    /// ```rust
    /// # use time::Month;
    /// # use time_macros::{datetime, offset};
    /// assert_eq!(datetime!(2019-01-01 0:00 UTC).month(), Month::January);
    /// assert_eq!(
    ///     datetime!(2019-12-31 23:00 UTC)
    ///         .to_offset(offset!(+1))
    ///         .month(),
    ///     Month::January,
    /// );
    /// ```
    #[inline]
    pub const fn month(self) -> Month {
        self.date().month()
    }

    /// Get the day of the date in the stored offset.
    ///
    /// The returned value will always be in the range `1..=31`.
    ///
    /// ```rust
    /// # use time_macros::{datetime, offset};
    /// assert_eq!(datetime!(2019-01-01 0:00 UTC).day(), 1);
    /// assert_eq!(
    ///     datetime!(2019-12-31 23:00 UTC)
    ///         .to_offset(offset!(+1))
    ///         .day(),
    ///     1,
    /// );
    /// ```
    #[inline]
    pub const fn day(self) -> u8 {
        self.date().day()
    }

    /// Get the day of the year of the date in the stored offset.
    ///
    /// The returned value will always be in the range `1..=366`.
    ///
    /// ```rust
    /// # use time_macros::{datetime, offset};
    /// assert_eq!(datetime!(2019-01-01 0:00 UTC).ordinal(), 1);
    /// assert_eq!(
    ///     datetime!(2019-12-31 23:00 UTC)
    ///         .to_offset(offset!(+1))
    ///         .ordinal(),
    ///     1,
    /// );
    /// ```
    #[inline]
    pub const fn ordinal(self) -> u16 {
        self.date().ordinal()
    }

    /// Get the ISO week number of the date in the stored offset.
    ///
    /// The returned value will always be in the range `1..=53`.
    ///
    /// ```rust
    /// # use time_macros::datetime;
    /// assert_eq!(datetime!(2019-01-01 0:00 UTC).iso_week(), 1);
    /// assert_eq!(datetime!(2020-01-01 0:00 UTC).iso_week(), 1);
    /// assert_eq!(datetime!(2020-12-31 0:00 UTC).iso_week(), 53);
    /// assert_eq!(datetime!(2021-01-01 0:00 UTC).iso_week(), 53);
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
    /// assert_eq!(datetime!(2019-01-01 0:00 UTC).sunday_based_week(), 0);
    /// assert_eq!(datetime!(2020-01-01 0:00 UTC).sunday_based_week(), 0);
    /// assert_eq!(datetime!(2020-12-31 0:00 UTC).sunday_based_week(), 52);
    /// assert_eq!(datetime!(2021-01-01 0:00 UTC).sunday_based_week(), 0);
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
    /// assert_eq!(datetime!(2019-01-01 0:00 UTC).monday_based_week(), 0);
    /// assert_eq!(datetime!(2020-01-01 0:00 UTC).monday_based_week(), 0);
    /// assert_eq!(datetime!(2020-12-31 0:00 UTC).monday_based_week(), 52);
    /// assert_eq!(datetime!(2021-01-01 0:00 UTC).monday_based_week(), 0);
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
    ///     datetime!(2019-01-01 0:00 UTC).to_calendar_date(),
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
    /// assert_eq!(
    ///     datetime!(2019-01-01 0:00 UTC).to_ordinal_date(),
    ///     (2019, 1)
    /// );
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
    ///     datetime!(2019-01-01 0:00 UTC).to_iso_week_date(),
    ///     (2019, 1, Tuesday)
    /// );
    /// assert_eq!(
    ///     datetime!(2019-10-04 0:00 UTC).to_iso_week_date(),
    ///     (2019, 40, Friday)
    /// );
    /// assert_eq!(
    ///     datetime!(2020-01-01 0:00 UTC).to_iso_week_date(),
    ///     (2020, 1, Wednesday)
    /// );
    /// assert_eq!(
    ///     datetime!(2020-12-31 0:00 UTC).to_iso_week_date(),
    ///     (2020, 53, Thursday)
    /// );
    /// assert_eq!(
    ///     datetime!(2021-01-01 0:00 UTC).to_iso_week_date(),
    ///     (2020, 53, Friday)
    /// );
    /// ```
    #[inline]
    pub const fn to_iso_week_date(self) -> (i32, u8, Weekday) {
        self.date().to_iso_week_date()
    }

    /// Get the weekday of the date in the stored offset.
    ///
    /// ```rust
    /// # use time::Weekday::*;
    /// # use time_macros::datetime;
    /// assert_eq!(datetime!(2019-01-01 0:00 UTC).weekday(), Tuesday);
    /// assert_eq!(datetime!(2019-02-01 0:00 UTC).weekday(), Friday);
    /// assert_eq!(datetime!(2019-03-01 0:00 UTC).weekday(), Friday);
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
    /// assert_eq!(datetime!(-4713-11-24 0:00 UTC).to_julian_day(), 0);
    /// assert_eq!(datetime!(2000-01-01 0:00 UTC).to_julian_day(), 2_451_545);
    /// assert_eq!(datetime!(2019-01-01 0:00 UTC).to_julian_day(), 2_458_485);
    /// assert_eq!(datetime!(2019-12-31 0:00 UTC).to_julian_day(), 2_458_849);
    /// ```
    #[inline]
    pub const fn to_julian_day(self) -> i32 {
        self.date().to_julian_day()
    }

    /// Get the clock hour, minute, and second.
    ///
    /// ```rust
    /// # use time_macros::datetime;
    /// assert_eq!(datetime!(2020-01-01 0:00:00 UTC).to_hms(), (0, 0, 0));
    /// assert_eq!(datetime!(2020-01-01 23:59:59 UTC).to_hms(), (23, 59, 59));
    /// ```
    #[inline]
    pub const fn to_hms(self) -> (u8, u8, u8) {
        self.time().as_hms()
    }

    /// Get the clock hour, minute, second, and millisecond.
    ///
    /// ```rust
    /// # use time_macros::datetime;
    /// assert_eq!(
    ///     datetime!(2020-01-01 0:00:00 UTC).to_hms_milli(),
    ///     (0, 0, 0, 0)
    /// );
    /// assert_eq!(
    ///     datetime!(2020-01-01 23:59:59.999 UTC).to_hms_milli(),
    ///     (23, 59, 59, 999)
    /// );
    /// ```
    #[inline]
    pub const fn to_hms_milli(self) -> (u8, u8, u8, u16) {
        self.time().as_hms_milli()
    }

    /// Get the clock hour, minute, second, and microsecond.
    ///
    /// ```rust
    /// # use time_macros::datetime;
    /// assert_eq!(
    ///     datetime!(2020-01-01 0:00:00 UTC).to_hms_micro(),
    ///     (0, 0, 0, 0)
    /// );
    /// assert_eq!(
    ///     datetime!(2020-01-01 23:59:59.999_999 UTC).to_hms_micro(),
    ///     (23, 59, 59, 999_999)
    /// );
    /// ```
    #[inline]
    pub const fn to_hms_micro(self) -> (u8, u8, u8, u32) {
        self.time().as_hms_micro()
    }

    /// Get the clock hour, minute, second, and nanosecond.
    ///
    /// ```rust
    /// # use time_macros::datetime;
    /// assert_eq!(
    ///     datetime!(2020-01-01 0:00:00 UTC).to_hms_nano(),
    ///     (0, 0, 0, 0)
    /// );
    /// assert_eq!(
    ///     datetime!(2020-01-01 23:59:59.999_999_999 UTC).to_hms_nano(),
    ///     (23, 59, 59, 999_999_999)
    /// );
    /// ```
    #[inline]
    pub const fn to_hms_nano(self) -> (u8, u8, u8, u32) {
        self.time().as_hms_nano()
    }

    /// Get the clock hour in the stored offset.
    ///
    /// The returned value will always be in the range `0..24`.
    ///
    /// ```rust
    /// # use time_macros::{datetime, offset};
    /// assert_eq!(datetime!(2019-01-01 0:00 UTC).hour(), 0);
    /// assert_eq!(
    ///     datetime!(2019-01-01 23:59:59 UTC)
    ///         .to_offset(offset!(-2))
    ///         .hour(),
    ///     21,
    /// );
    /// ```
    #[inline]
    pub const fn hour(self) -> u8 {
        self.time().hour()
    }

    /// Get the minute within the hour in the stored offset.
    ///
    /// The returned value will always be in the range `0..60`.
    ///
    /// ```rust
    /// # use time_macros::{datetime, offset};
    /// assert_eq!(datetime!(2019-01-01 0:00 UTC).minute(), 0);
    /// assert_eq!(
    ///     datetime!(2019-01-01 23:59:59 UTC)
    ///         .to_offset(offset!(+0:30))
    ///         .minute(),
    ///     29,
    /// );
    /// ```
    #[inline]
    pub const fn minute(self) -> u8 {
        self.time().minute()
    }

    /// Get the second within the minute in the stored offset.
    ///
    /// The returned value will always be in the range `0..60`.
    ///
    /// ```rust
    /// # use time_macros::{datetime, offset};
    /// assert_eq!(datetime!(2019-01-01 0:00 UTC).second(), 0);
    /// assert_eq!(
    ///     datetime!(2019-01-01 23:59:59 UTC)
    ///         .to_offset(offset!(+0:00:30))
    ///         .second(),
    ///     29,
    /// );
    /// ```
    #[inline]
    pub const fn second(self) -> u8 {
        self.time().second()
    }

    // Because a `UtcOffset` is limited in resolution to one second, any subsecond value will not
    // change when adjusting for the offset.

    /// Get the milliseconds within the second in the stored offset.
    ///
    /// The returned value will always be in the range `0..1_000`.
    ///
    /// ```rust
    /// # use time_macros::datetime;
    /// assert_eq!(datetime!(2019-01-01 0:00 UTC).millisecond(), 0);
    /// assert_eq!(datetime!(2019-01-01 23:59:59.999 UTC).millisecond(), 999);
    /// ```
    #[inline]
    pub const fn millisecond(self) -> u16 {
        self.time().millisecond()
    }

    /// Get the microseconds within the second in the stored offset.
    ///
    /// The returned value will always be in the range `0..1_000_000`.
    ///
    /// ```rust
    /// # use time_macros::datetime;
    /// assert_eq!(datetime!(2019-01-01 0:00 UTC).microsecond(), 0);
    /// assert_eq!(
    ///     datetime!(2019-01-01 23:59:59.999_999 UTC).microsecond(),
    ///     999_999,
    /// );
    /// ```
    #[inline]
    pub const fn microsecond(self) -> u32 {
        self.time().microsecond()
    }

    /// Get the nanoseconds within the second in the stored offset.
    ///
    /// The returned value will always be in the range `0..1_000_000_000`.
    ///
    /// ```rust
    /// # use time_macros::datetime;
    /// assert_eq!(datetime!(2019-01-01 0:00 UTC).nanosecond(), 0);
    /// assert_eq!(
    ///     datetime!(2019-01-01 23:59:59.999_999_999 UTC).nanosecond(),
    ///     999_999_999,
    /// );
    /// ```
    #[inline]
    pub const fn nanosecond(self) -> u32 {
        self.time().nanosecond()
    }

    /// Computes `self + duration`, returning `None` if an overflow occurred.
    ///
    /// ```
    /// # use time::{Date, ext::NumericalDuration};
    /// # use time_macros::{datetime, offset};
    /// let datetime = Date::MIN.midnight().assume_offset(offset!(+10));
    /// assert_eq!(datetime.checked_add((-2).days()), None);
    ///
    /// let datetime = Date::MAX.midnight().assume_offset(offset!(+10));
    /// assert_eq!(datetime.checked_add(2.days()), None);
    ///
    /// assert_eq!(
    ///     datetime!(2019-11-25 15:30 +10).checked_add(27.hours()),
    ///     Some(datetime!(2019-11-26 18:30 +10))
    /// );
    /// ```
    #[inline]
    pub const fn checked_add(self, duration: Duration) -> Option<Self> {
        Some(const_try_opt!(self.date_time().checked_add(duration)).assume_offset(self.offset()))
    }

    /// Computes `self - duration`, returning `None` if an overflow occurred.
    ///
    /// ```
    /// # use time::{Date, ext::NumericalDuration};
    /// # use time_macros::{datetime, offset};
    /// let datetime = Date::MIN.midnight().assume_offset(offset!(+10));
    /// assert_eq!(datetime.checked_sub(2.days()), None);
    ///
    /// let datetime = Date::MAX.midnight().assume_offset(offset!(+10));
    /// assert_eq!(datetime.checked_sub((-2).days()), None);
    ///
    /// assert_eq!(
    ///     datetime!(2019-11-25 15:30 +10).checked_sub(27.hours()),
    ///     Some(datetime!(2019-11-24 12:30 +10))
    /// );
    /// ```
    #[inline]
    pub const fn checked_sub(self, duration: Duration) -> Option<Self> {
        Some(const_try_opt!(self.date_time().checked_sub(duration)).assume_offset(self.offset()))
    }

    /// Computes `self + duration`, saturating value on overflow.
    ///
    /// ```
    /// # use time::ext::NumericalDuration;
    /// # use time_macros::datetime;
    /// assert_eq!(
    #[cfg_attr(
        feature = "large-dates",
        doc = "    datetime!(-999999-01-01 0:00 +10).saturating_add((-2).days()),"
    )]
    #[cfg_attr(feature = "large-dates", doc = "    datetime!(-999999-01-01 0:00 +10)")]
    #[cfg_attr(
        not(feature = "large-dates"),
        doc = "    datetime!(-9999-01-01 0:00 +10).saturating_add((-2).days()),"
    )]
    #[cfg_attr(
        not(feature = "large-dates"),
        doc = "    datetime!(-9999-01-01 0:00 +10)"
    )]
    /// );
    ///
    /// assert_eq!(
    #[cfg_attr(
        feature = "large-dates",
        doc = "    datetime!(+999999-12-31 23:59:59.999_999_999 +10).saturating_add(2.days()),"
    )]
    #[cfg_attr(
        feature = "large-dates",
        doc = "    datetime!(+999999-12-31 23:59:59.999_999_999 +10)"
    )]
    #[cfg_attr(
        not(feature = "large-dates"),
        doc = "    datetime!(+9999-12-31 23:59:59.999_999_999 +10).saturating_add(2.days()),"
    )]
    #[cfg_attr(
        not(feature = "large-dates"),
        doc = "    datetime!(+9999-12-31 23:59:59.999_999_999 +10)"
    )]
    /// );
    ///
    /// assert_eq!(
    ///     datetime!(2019-11-25 15:30 +10).saturating_add(27.hours()),
    ///     datetime!(2019-11-26 18:30 +10)
    /// );
    /// ```
    #[inline]
    pub const fn saturating_add(self, duration: Duration) -> Self {
        if let Some(datetime) = self.checked_add(duration) {
            datetime
        } else if duration.is_negative() {
            PrimitiveDateTime::MIN.assume_offset(self.offset())
        } else {
            PrimitiveDateTime::MAX.assume_offset(self.offset())
        }
    }

    /// Computes `self - duration`, saturating value on overflow.
    ///
    /// ```
    /// # use time::ext::NumericalDuration;
    /// # use time_macros::datetime;
    /// assert_eq!(
    #[cfg_attr(
        feature = "large-dates",
        doc = "    datetime!(-999999-01-01 0:00 +10).saturating_sub(2.days()),"
    )]
    #[cfg_attr(feature = "large-dates", doc = "    datetime!(-999999-01-01 0:00 +10)")]
    #[cfg_attr(
        not(feature = "large-dates"),
        doc = "    datetime!(-9999-01-01 0:00 +10).saturating_sub(2.days()),"
    )]
    #[cfg_attr(
        not(feature = "large-dates"),
        doc = "    datetime!(-9999-01-01 0:00 +10)"
    )]
    /// );
    ///
    /// assert_eq!(
    #[cfg_attr(
        feature = "large-dates",
        doc = "    datetime!(+999999-12-31 23:59:59.999_999_999 +10).saturating_sub((-2).days()),"
    )]
    #[cfg_attr(
        feature = "large-dates",
        doc = "    datetime!(+999999-12-31 23:59:59.999_999_999 +10)"
    )]
    #[cfg_attr(
        not(feature = "large-dates"),
        doc = "    datetime!(+9999-12-31 23:59:59.999_999_999 +10).saturating_sub((-2).days()),"
    )]
    #[cfg_attr(
        not(feature = "large-dates"),
        doc = "    datetime!(+9999-12-31 23:59:59.999_999_999 +10)"
    )]
    /// );
    ///
    /// assert_eq!(
    ///     datetime!(2019-11-25 15:30 +10).saturating_sub(27.hours()),
    ///     datetime!(2019-11-24 12:30 +10)
    /// );
    /// ```
    #[inline]
    pub const fn saturating_sub(self, duration: Duration) -> Self {
        if let Some(datetime) = self.checked_sub(duration) {
            datetime
        } else if duration.is_negative() {
            PrimitiveDateTime::MAX.assume_offset(self.offset())
        } else {
            PrimitiveDateTime::MIN.assume_offset(self.offset())
        }
    }
}

/// Methods that replace part of the `OffsetDateTime`.
impl OffsetDateTime {
    /// Replace the time, which is assumed to be in the stored offset. The date and offset
    /// components are unchanged.
    ///
    /// ```rust
    /// # use time_macros::{datetime, time};
    /// assert_eq!(
    ///     datetime!(2020-01-01 5:00 UTC).replace_time(time!(12:00)),
    ///     datetime!(2020-01-01 12:00 UTC)
    /// );
    /// assert_eq!(
    ///     datetime!(2020-01-01 12:00 -5).replace_time(time!(7:00)),
    ///     datetime!(2020-01-01 7:00 -5)
    /// );
    /// assert_eq!(
    ///     datetime!(2020-01-01 0:00 +1).replace_time(time!(12:00)),
    ///     datetime!(2020-01-01 12:00 +1)
    /// );
    /// ```
    #[must_use = "This method does not mutate the original `OffsetDateTime`."]
    #[inline]
    pub const fn replace_time(self, time: Time) -> Self {
        Self::new_in_offset(self.date(), time, self.offset())
    }

    /// Replace the date, which is assumed to be in the stored offset. The time and offset
    /// components are unchanged.
    ///
    /// ```rust
    /// # use time_macros::{datetime, date};
    /// assert_eq!(
    ///     datetime!(2020-01-01 12:00 UTC).replace_date(date!(2020-01-30)),
    ///     datetime!(2020-01-30 12:00 UTC)
    /// );
    /// assert_eq!(
    ///     datetime!(2020-01-01 0:00 +1).replace_date(date!(2020-01-30)),
    ///     datetime!(2020-01-30 0:00 +1)
    /// );
    /// ```
    #[must_use = "This method does not mutate the original `OffsetDateTime`."]
    #[inline]
    pub const fn replace_date(self, date: Date) -> Self {
        Self::new_in_offset(date, self.time(), self.offset())
    }

    /// Replace the date and time, which are assumed to be in the stored offset. The offset
    /// component remains unchanged.
    ///
    /// ```rust
    /// # use time_macros::datetime;
    /// assert_eq!(
    ///     datetime!(2020-01-01 12:00 UTC).replace_date_time(datetime!(2020-01-30 16:00)),
    ///     datetime!(2020-01-30 16:00 UTC)
    /// );
    /// assert_eq!(
    ///     datetime!(2020-01-01 12:00 +1).replace_date_time(datetime!(2020-01-30 0:00)),
    ///     datetime!(2020-01-30 0:00 +1)
    /// );
    /// ```
    #[must_use = "This method does not mutate the original `OffsetDateTime`."]
    #[inline]
    pub const fn replace_date_time(self, date_time: PrimitiveDateTime) -> Self {
        date_time.assume_offset(self.offset())
    }

    /// Replace the offset. The date and time components remain unchanged.
    ///
    /// ```rust
    /// # use time_macros::{datetime, offset};
    /// assert_eq!(
    ///     datetime!(2020-01-01 0:00 UTC).replace_offset(offset!(-5)),
    ///     datetime!(2020-01-01 0:00 -5)
    /// );
    /// ```
    #[must_use = "This method does not mutate the original `OffsetDateTime`."]
    #[inline]
    pub const fn replace_offset(self, offset: UtcOffset) -> Self {
        self.date_time().assume_offset(offset)
    }

    /// Replace the year. The month and day will be unchanged.
    ///
    /// ```rust
    /// # use time_macros::datetime;
    /// assert_eq!(
    ///     datetime!(2022-02-18 12:00 +01).replace_year(2019),
    ///     Ok(datetime!(2019-02-18 12:00 +01))
    /// );
    /// assert!(datetime!(2022-02-18 12:00 +01).replace_year(-1_000_000_000).is_err()); // -1_000_000_000 isn't a valid year
    /// assert!(datetime!(2022-02-18 12:00 +01).replace_year(1_000_000_000).is_err()); // 1_000_000_000 isn't a valid year
    /// ```
    #[must_use = "This method does not mutate the original `OffsetDateTime`."]
    #[inline]
    pub const fn replace_year(self, year: i32) -> Result<Self, error::ComponentRange> {
        Ok(const_try!(self.date_time().replace_year(year)).assume_offset(self.offset()))
    }

    /// Replace the month of the year.
    ///
    /// ```rust
    /// # use time_macros::datetime;
    /// # use time::Month;
    /// assert_eq!(
    ///     datetime!(2022-02-18 12:00 +01).replace_month(Month::January),
    ///     Ok(datetime!(2022-01-18 12:00 +01))
    /// );
    /// assert!(datetime!(2022-01-30 12:00 +01).replace_month(Month::February).is_err()); // 30 isn't a valid day in February
    /// ```
    #[must_use = "This method does not mutate the original `OffsetDateTime`."]
    #[inline]
    pub const fn replace_month(self, month: Month) -> Result<Self, error::ComponentRange> {
        Ok(const_try!(self.date_time().replace_month(month)).assume_offset(self.offset()))
    }

    /// Replace the day of the month.
    ///
    /// ```rust
    /// # use time_macros::datetime;
    /// assert_eq!(
    ///     datetime!(2022-02-18 12:00 +01).replace_day(1),
    ///     Ok(datetime!(2022-02-01 12:00 +01))
    /// );
    /// assert!(datetime!(2022-02-18 12:00 +01).replace_day(0).is_err()); // 00 isn't a valid day
    /// assert!(datetime!(2022-02-18 12:00 +01).replace_day(30).is_err()); // 30 isn't a valid day in February
    /// ```
    #[must_use = "This method does not mutate the original `OffsetDateTime`."]
    #[inline]
    pub const fn replace_day(self, day: u8) -> Result<Self, error::ComponentRange> {
        Ok(const_try!(self.date_time().replace_day(day)).assume_offset(self.offset()))
    }

    /// Replace the day of the year.
    ///
    /// ```rust
    /// # use time_macros::datetime;
    /// assert_eq!(datetime!(2022-049 12:00 +01).replace_ordinal(1), Ok(datetime!(2022-001 12:00 +01)));
    /// assert!(datetime!(2022-049 12:00 +01).replace_ordinal(0).is_err()); // 0 isn't a valid ordinal
    /// assert!(datetime!(2022-049 12:00 +01).replace_ordinal(366).is_err()); // 2022 isn't a leap year
    /// ```
    #[must_use = "This method does not mutate the original `OffsetDateTime`."]
    #[inline]
    pub const fn replace_ordinal(self, ordinal: u16) -> Result<Self, error::ComponentRange> {
        Ok(const_try!(self.date_time().replace_ordinal(ordinal)).assume_offset(self.offset()))
    }

    /// Replace the clock hour.
    ///
    /// ```rust
    /// # use time_macros::datetime;
    /// assert_eq!(
    ///     datetime!(2022-02-18 01:02:03.004_005_006 +01).replace_hour(7),
    ///     Ok(datetime!(2022-02-18 07:02:03.004_005_006 +01))
    /// );
    /// assert!(datetime!(2022-02-18 01:02:03.004_005_006 +01).replace_hour(24).is_err()); // 24 isn't a valid hour
    /// ```
    #[must_use = "This method does not mutate the original `OffsetDateTime`."]
    #[inline]
    pub const fn replace_hour(self, hour: u8) -> Result<Self, error::ComponentRange> {
        Ok(const_try!(self.date_time().replace_hour(hour)).assume_offset(self.offset()))
    }

    /// Replace the minutes within the hour.
    ///
    /// ```rust
    /// # use time_macros::datetime;
    /// assert_eq!(
    ///     datetime!(2022-02-18 01:02:03.004_005_006 +01).replace_minute(7),
    ///     Ok(datetime!(2022-02-18 01:07:03.004_005_006 +01))
    /// );
    /// assert!(datetime!(2022-02-18 01:02:03.004_005_006 +01).replace_minute(60).is_err()); // 60 isn't a valid minute
    /// ```
    #[must_use = "This method does not mutate the original `OffsetDateTime`."]
    #[inline]
    pub const fn replace_minute(self, minute: u8) -> Result<Self, error::ComponentRange> {
        Ok(const_try!(self.date_time().replace_minute(minute)).assume_offset(self.offset()))
    }

    /// Replace the seconds within the minute.
    ///
    /// ```rust
    /// # use time_macros::datetime;
    /// assert_eq!(
    ///     datetime!(2022-02-18 01:02:03.004_005_006 +01).replace_second(7),
    ///     Ok(datetime!(2022-02-18 01:02:07.004_005_006 +01))
    /// );
    /// assert!(datetime!(2022-02-18 01:02:03.004_005_006 +01).replace_second(60).is_err()); // 60 isn't a valid second
    /// ```
    #[must_use = "This method does not mutate the original `OffsetDateTime`."]
    #[inline]
    pub const fn replace_second(self, second: u8) -> Result<Self, error::ComponentRange> {
        Ok(const_try!(self.date_time().replace_second(second)).assume_offset(self.offset()))
    }

    /// Replace the milliseconds within the second.
    ///
    /// ```rust
    /// # use time_macros::datetime;
    /// assert_eq!(
    ///     datetime!(2022-02-18 01:02:03.004_005_006 +01).replace_millisecond(7),
    ///     Ok(datetime!(2022-02-18 01:02:03.007 +01))
    /// );
    /// assert!(datetime!(2022-02-18 01:02:03.004_005_006 +01).replace_millisecond(1_000).is_err()); // 1_000 isn't a valid millisecond
    /// ```
    #[must_use = "This method does not mutate the original `OffsetDateTime`."]
    #[inline]
    pub const fn replace_millisecond(
        self,
        millisecond: u16,
    ) -> Result<Self, error::ComponentRange> {
        Ok(
            const_try!(self.date_time().replace_millisecond(millisecond))
                .assume_offset(self.offset()),
        )
    }

    /// Replace the microseconds within the second.
    ///
    /// ```rust
    /// # use time_macros::datetime;
    /// assert_eq!(
    ///     datetime!(2022-02-18 01:02:03.004_005_006 +01).replace_microsecond(7_008),
    ///     Ok(datetime!(2022-02-18 01:02:03.007_008 +01))
    /// );
    /// assert!(datetime!(2022-02-18 01:02:03.004_005_006 +01).replace_microsecond(1_000_000).is_err()); // 1_000_000 isn't a valid microsecond
    /// ```
    #[must_use = "This method does not mutate the original `OffsetDateTime`."]
    #[inline]
    pub const fn replace_microsecond(
        self,
        microsecond: u32,
    ) -> Result<Self, error::ComponentRange> {
        Ok(
            const_try!(self.date_time().replace_microsecond(microsecond))
                .assume_offset(self.offset()),
        )
    }

    /// Replace the nanoseconds within the second.
    ///
    /// ```rust
    /// # use time_macros::datetime;
    /// assert_eq!(
    ///     datetime!(2022-02-18 01:02:03.004_005_006 +01).replace_nanosecond(7_008_009),
    ///     Ok(datetime!(2022-02-18 01:02:03.007_008_009 +01))
    /// );
    /// assert!(datetime!(2022-02-18 01:02:03.004_005_006 +01).replace_nanosecond(1_000_000_000).is_err()); // 1_000_000_000 isn't a valid nanosecond
    /// ```
    #[must_use = "This method does not mutate the original `OffsetDateTime`."]
    #[inline]
    pub const fn replace_nanosecond(self, nanosecond: u32) -> Result<Self, error::ComponentRange> {
        Ok(
            const_try!(self.date_time().replace_nanosecond(nanosecond))
                .assume_offset(self.offset()),
        )
    }
}

#[cfg(feature = "formatting")]
impl OffsetDateTime {
    /// Format the `OffsetDateTime` using the provided [format
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
            Some(self.offset()),
        )
    }

    /// Format the `OffsetDateTime` using the provided [format
    /// description](crate::format_description).
    ///
    /// ```rust
    /// # use time::format_description;
    /// # use time_macros::datetime;
    /// let format = format_description::parse(
    ///     "[year]-[month]-[day] [hour]:[minute]:[second] [offset_hour \
    ///          sign:mandatory]:[offset_minute]:[offset_second]",
    /// )?;
    /// assert_eq!(
    ///     datetime!(2020-01-02 03:04:05 +06:07:08).format(&format)?,
    ///     "2020-01-02 03:04:05 +06:07:08"
    /// );
    /// # Ok::<_, time::Error>(())
    /// ```
    #[inline]
    pub fn format(self, format: &(impl Formattable + ?Sized)) -> Result<String, error::Format> {
        format.format(Some(self.date()), Some(self.time()), Some(self.offset()))
    }
}

#[cfg(feature = "parsing")]
impl OffsetDateTime {
    /// Parse an `OffsetDateTime` from the input using the provided [format
    /// description](crate::format_description).
    ///
    /// ```rust
    /// # use time::OffsetDateTime;
    /// # use time_macros::{datetime, format_description};
    /// let format = format_description!(
    ///     "[year]-[month]-[day] [hour]:[minute]:[second] [offset_hour \
    ///          sign:mandatory]:[offset_minute]:[offset_second]"
    /// );
    /// assert_eq!(
    ///     OffsetDateTime::parse("2020-01-02 03:04:05 +06:07:08", &format)?,
    ///     datetime!(2020-01-02 03:04:05 +06:07:08)
    /// );
    /// # Ok::<_, time::Error>(())
    /// ```
    #[inline]
    pub fn parse(
        input: &str,
        description: &(impl Parsable + ?Sized),
    ) -> Result<Self, error::Parse> {
        description.parse_offset_date_time(input.as_bytes())
    }

    /// A helper method to check if the `OffsetDateTime` is a valid representation of a leap second.
    /// Leap seconds, when parsed, are represented as the preceding nanosecond. However, leap
    /// seconds can only occur as the last second of a month UTC.
    #[cfg(feature = "parsing")]
    #[inline]
    pub(crate) const fn is_valid_leap_second_stand_in(self) -> bool {
        // This comparison doesn't need to be adjusted for the stored offset, so check it first for
        // speed.
        if self.nanosecond() != 999_999_999 {
            return false;
        }

        let (year, ordinal, time) = self.to_utc_raw();
        let Ok(date) = Date::from_ordinal_date(year, ordinal) else {
            return false;
        };

        time.hour() == 23
            && time.minute() == 59
            && time.second() == 59
            && date.day() == date.month().length(year)
    }
}

impl SmartDisplay for OffsetDateTime {
    type Metadata = ();

    #[inline]
    fn metadata(&self, _: FormatterOptions) -> Metadata<'_, Self> {
        let width =
            smart_display::padded_width_of!(self.date(), " ", self.time(), " ", self.offset());
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
            format_args!("{} {} {}", self.date(), self.time(), self.offset()),
        )
    }
}

impl fmt::Display for OffsetDateTime {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        SmartDisplay::fmt(self, f)
    }
}

impl fmt::Debug for OffsetDateTime {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl Add<Duration> for OffsetDateTime {
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

impl Add<StdDuration> for OffsetDateTime {
    type Output = Self;

    /// # Panics
    ///
    /// This may panic if an overflow occurs.
    #[inline]
    #[track_caller]
    fn add(self, duration: StdDuration) -> Self::Output {
        let (is_next_day, time) = self.time().adjusting_add_std(duration);

        Self::new_in_offset(
            if is_next_day {
                (self.date() + duration)
                    .next_day()
                    .expect("resulting value is out of range")
            } else {
                self.date() + duration
            },
            time,
            self.offset,
        )
    }
}

impl AddAssign<Duration> for OffsetDateTime {
    /// # Panics
    ///
    /// This may panic if an overflow occurs.
    #[inline]
    #[track_caller]
    fn add_assign(&mut self, rhs: Duration) {
        *self = *self + rhs;
    }
}

impl AddAssign<StdDuration> for OffsetDateTime {
    /// # Panics
    ///
    /// This may panic if an overflow occurs.
    #[inline]
    #[track_caller]
    fn add_assign(&mut self, rhs: StdDuration) {
        *self = *self + rhs;
    }
}

impl Sub<Duration> for OffsetDateTime {
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

impl Sub<StdDuration> for OffsetDateTime {
    type Output = Self;

    /// # Panics
    ///
    /// This may panic if an overflow occurs.
    #[inline]
    #[track_caller]
    fn sub(self, duration: StdDuration) -> Self::Output {
        let (is_previous_day, time) = self.time().adjusting_sub_std(duration);

        Self::new_in_offset(
            if is_previous_day {
                (self.date() - duration)
                    .previous_day()
                    .expect("resulting value is out of range")
            } else {
                self.date() - duration
            },
            time,
            self.offset,
        )
    }
}

impl SubAssign<Duration> for OffsetDateTime {
    /// # Panics
    ///
    /// This may panic if an overflow occurs.
    #[inline]
    #[track_caller]
    fn sub_assign(&mut self, rhs: Duration) {
        *self = *self - rhs;
    }
}

impl SubAssign<StdDuration> for OffsetDateTime {
    /// # Panics
    ///
    /// This may panic if an overflow occurs.
    #[inline]
    #[track_caller]
    fn sub_assign(&mut self, rhs: StdDuration) {
        *self = *self - rhs;
    }
}

impl Sub for OffsetDateTime {
    type Output = Duration;

    /// # Panics
    ///
    /// This may panic if an overflow occurs.
    #[inline]
    #[track_caller]
    fn sub(self, rhs: Self) -> Self::Output {
        let base = self.date_time() - rhs.date_time();
        let adjustment = Duration::seconds(
            (self.offset.whole_seconds() - rhs.offset.whole_seconds()).extend::<i64>(),
        );
        base - adjustment
    }
}
