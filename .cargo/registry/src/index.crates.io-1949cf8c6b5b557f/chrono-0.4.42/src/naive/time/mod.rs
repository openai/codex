// This is a part of Chrono.
// See README.md and LICENSE.txt for details.

//! ISO 8601 time without timezone.

#[cfg(feature = "alloc")]
use core::borrow::Borrow;
use core::ops::{Add, AddAssign, Sub, SubAssign};
use core::time::Duration;
use core::{fmt, str};

#[cfg(any(feature = "rkyv", feature = "rkyv-16", feature = "rkyv-32", feature = "rkyv-64"))]
use rkyv::{Archive, Deserialize, Serialize};

#[cfg(feature = "alloc")]
use crate::format::DelayedFormat;
use crate::format::{
    Fixed, Item, Numeric, Pad, ParseError, ParseResult, Parsed, StrftimeItems, parse,
    parse_and_remainder, write_hundreds,
};
use crate::{FixedOffset, TimeDelta, Timelike};
use crate::{expect, try_opt};

#[cfg(feature = "serde")]
mod serde;

#[cfg(test)]
mod tests;

/// ISO 8601 time without timezone.
/// Allows for the nanosecond precision and optional leap second representation.
///
/// # Leap Second Handling
///
/// Since 1960s, the manmade atomic clock has been so accurate that
/// it is much more accurate than Earth's own motion.
/// It became desirable to define the civil time in terms of the atomic clock,
/// but that risks the desynchronization of the civil time from Earth.
/// To account for this, the designers of the Coordinated Universal Time (UTC)
/// made that the UTC should be kept within 0.9 seconds of the observed Earth-bound time.
/// When the mean solar day is longer than the ideal (86,400 seconds),
/// the error slowly accumulates and it is necessary to add a **leap second**
/// to slow the UTC down a bit.
/// (We may also remove a second to speed the UTC up a bit, but it never happened.)
/// The leap second, if any, follows 23:59:59 of June 30 or December 31 in the UTC.
///
/// Fast forward to the 21st century,
/// we have seen 26 leap seconds from January 1972 to December 2015.
/// Yes, 26 seconds. Probably you can read this paragraph within 26 seconds.
/// But those 26 seconds, and possibly more in the future, are never predictable,
/// and whether to add a leap second or not is known only before 6 months.
/// Internet-based clocks (via NTP) do account for known leap seconds,
/// but the system API normally doesn't (and often can't, with no network connection)
/// and there is no reliable way to retrieve leap second information.
///
/// Chrono does not try to accurately implement leap seconds; it is impossible.
/// Rather, **it allows for leap seconds but behaves as if there are *no other* leap seconds.**
/// Various operations will ignore any possible leap second(s)
/// except when any of the operands were actually leap seconds.
///
/// If you cannot tolerate this behavior,
/// you must use a separate `TimeZone` for the International Atomic Time (TAI).
/// TAI is like UTC but has no leap seconds, and thus slightly differs from UTC.
/// Chrono does not yet provide such implementation, but it is planned.
///
/// ## Representing Leap Seconds
///
/// The leap second is indicated via fractional seconds more than 1 second.
/// This makes possible to treat a leap second as the prior non-leap second
/// if you don't care about sub-second accuracy.
/// You should use the proper formatting to get the raw leap second.
///
/// All methods accepting fractional seconds will accept such values.
///
/// ```
/// use chrono::{NaiveDate, NaiveTime};
///
/// let t = NaiveTime::from_hms_milli_opt(8, 59, 59, 1_000).unwrap();
///
/// let dt1 = NaiveDate::from_ymd_opt(2015, 7, 1)
///     .unwrap()
///     .and_hms_micro_opt(8, 59, 59, 1_000_000)
///     .unwrap();
///
/// let dt2 = NaiveDate::from_ymd_opt(2015, 6, 30)
///     .unwrap()
///     .and_hms_nano_opt(23, 59, 59, 1_000_000_000)
///     .unwrap()
///     .and_utc();
/// # let _ = (t, dt1, dt2);
/// ```
///
/// Note that the leap second can happen anytime given an appropriate time zone;
/// 2015-07-01 01:23:60 would be a proper leap second if UTC+01:24 had existed.
/// Practically speaking, though, by the time of the first leap second on 1972-06-30,
/// every time zone offset around the world has standardized to the 5-minute alignment.
///
/// ## Date And Time Arithmetic
///
/// As a concrete example, let's assume that `03:00:60` and `04:00:60` are leap seconds.
/// In reality, of course, leap seconds are separated by at least 6 months.
/// We will also use some intuitive concise notations for the explanation.
///
/// `Time + TimeDelta`
/// (short for [`NaiveTime::overflowing_add_signed`](#method.overflowing_add_signed)):
///
/// - `03:00:00 + 1s = 03:00:01`.
/// - `03:00:59 + 60s = 03:01:59`.
/// - `03:00:59 + 61s = 03:02:00`.
/// - `03:00:59 + 1s = 03:01:00`.
/// - `03:00:60 + 1s = 03:01:00`.
///   Note that the sum is identical to the previous.
/// - `03:00:60 + 60s = 03:01:59`.
/// - `03:00:60 + 61s = 03:02:00`.
/// - `03:00:60.1 + 0.8s = 03:00:60.9`.
///
/// `Time - TimeDelta`
/// (short for [`NaiveTime::overflowing_sub_signed`](#method.overflowing_sub_signed)):
///
/// - `03:00:00 - 1s = 02:59:59`.
/// - `03:01:00 - 1s = 03:00:59`.
/// - `03:01:00 - 60s = 03:00:00`.
/// - `03:00:60 - 60s = 03:00:00`.
///   Note that the result is identical to the previous.
/// - `03:00:60.7 - 0.4s = 03:00:60.3`.
/// - `03:00:60.7 - 0.9s = 03:00:59.8`.
///
/// `Time - Time`
/// (short for [`NaiveTime::signed_duration_since`](#method.signed_duration_since)):
///
/// - `04:00:00 - 03:00:00 = 3600s`.
/// - `03:01:00 - 03:00:00 = 60s`.
/// - `03:00:60 - 03:00:00 = 60s`.
///   Note that the difference is identical to the previous.
/// - `03:00:60.6 - 03:00:59.4 = 1.2s`.
/// - `03:01:00 - 03:00:59.8 = 0.2s`.
/// - `03:01:00 - 03:00:60.5 = 0.5s`.
///   Note that the difference is larger than the previous,
///   even though the leap second clearly follows the previous whole second.
/// - `04:00:60.9 - 03:00:60.1 =
///   (04:00:60.9 - 04:00:00) + (04:00:00 - 03:01:00) + (03:01:00 - 03:00:60.1) =
///   60.9s + 3540s + 0.9s = 3601.8s`.
///
/// In general,
///
/// - `Time + TimeDelta` unconditionally equals to `TimeDelta + Time`.
///
/// - `Time - TimeDelta` unconditionally equals to `Time + (-TimeDelta)`.
///
/// - `Time1 - Time2` unconditionally equals to `-(Time2 - Time1)`.
///
/// - Associativity does not generally hold, because
///   `(Time + TimeDelta1) - TimeDelta2` no longer equals to `Time + (TimeDelta1 - TimeDelta2)`
///   for two positive durations.
///
///     - As a special case, `(Time + TimeDelta) - TimeDelta` also does not equal to `Time`.
///
///     - If you can assume that all durations have the same sign, however,
///       then the associativity holds:
///       `(Time + TimeDelta1) + TimeDelta2` equals to `Time + (TimeDelta1 + TimeDelta2)`
///       for two positive durations.
///
/// ## Reading And Writing Leap Seconds
///
/// The "typical" leap seconds on the minute boundary are
/// correctly handled both in the formatting and parsing.
/// The leap second in the human-readable representation
/// will be represented as the second part being 60, as required by ISO 8601.
///
/// ```
/// use chrono::NaiveDate;
///
/// let dt = NaiveDate::from_ymd_opt(2015, 6, 30)
///     .unwrap()
///     .and_hms_milli_opt(23, 59, 59, 1_000)
///     .unwrap()
///     .and_utc();
/// assert_eq!(format!("{:?}", dt), "2015-06-30T23:59:60Z");
/// ```
///
/// There are hypothetical leap seconds not on the minute boundary nevertheless supported by Chrono.
/// They are allowed for the sake of completeness and consistency; there were several "exotic" time
/// zone offsets with fractional minutes prior to UTC after all.
/// For such cases the human-readable representation is ambiguous and would be read back to the next
/// non-leap second.
///
/// A `NaiveTime` with a leap second that is not on a minute boundary can only be created from a
/// [`DateTime`](crate::DateTime) with fractional minutes as offset, or using
/// [`Timelike::with_nanosecond()`].
///
/// ```
/// use chrono::{FixedOffset, NaiveDate, TimeZone};
///
/// let paramaribo_pre1945 = FixedOffset::east_opt(-13236).unwrap(); // -03:40:36
/// let leap_sec_2015 =
///     NaiveDate::from_ymd_opt(2015, 6, 30).unwrap().and_hms_milli_opt(23, 59, 59, 1_000).unwrap();
/// let dt1 = paramaribo_pre1945.from_utc_datetime(&leap_sec_2015);
/// assert_eq!(format!("{:?}", dt1), "2015-06-30T20:19:24-03:40:36");
/// assert_eq!(format!("{:?}", dt1.time()), "20:19:24");
///
/// let next_sec = NaiveDate::from_ymd_opt(2015, 7, 1).unwrap().and_hms_opt(0, 0, 0).unwrap();
/// let dt2 = paramaribo_pre1945.from_utc_datetime(&next_sec);
/// assert_eq!(format!("{:?}", dt2), "2015-06-30T20:19:24-03:40:36");
/// assert_eq!(format!("{:?}", dt2.time()), "20:19:24");
///
/// assert!(dt1.time() != dt2.time());
/// assert!(dt1.time().to_string() == dt2.time().to_string());
/// ```
///
/// Since Chrono alone cannot determine any existence of leap seconds,
/// **there is absolutely no guarantee that the leap second read has actually happened**.
#[derive(PartialEq, Eq, Hash, PartialOrd, Ord, Copy, Clone)]
#[cfg_attr(
    any(feature = "rkyv", feature = "rkyv-16", feature = "rkyv-32", feature = "rkyv-64"),
    derive(Archive, Deserialize, Serialize),
    archive(compare(PartialEq, PartialOrd)),
    archive_attr(derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash))
)]
#[cfg_attr(feature = "rkyv-validation", archive(check_bytes))]
pub struct NaiveTime {
    secs: u32,
    frac: u32,
}

#[cfg(feature = "arbitrary")]
impl arbitrary::Arbitrary<'_> for NaiveTime {
    fn arbitrary(u: &mut arbitrary::Unstructured) -> arbitrary::Result<NaiveTime> {
        let mins = u.int_in_range(0..=1439)?;
        let mut secs = u.int_in_range(0..=60)?;
        let mut nano = u.int_in_range(0..=999_999_999)?;
        if secs == 60 {
            secs = 59;
            nano += 1_000_000_000;
        }
        let time = NaiveTime::from_num_seconds_from_midnight_opt(mins * 60 + secs, nano)
            .expect("Could not generate a valid chrono::NaiveTime. It looks like implementation of Arbitrary for NaiveTime is erroneous.");
        Ok(time)
    }
}

impl NaiveTime {
    /// Makes a new `NaiveTime` from hour, minute and second.
    ///
    /// No [leap second](#leap-second-handling) is allowed here;
    /// use `NaiveTime::from_hms_*` methods with a subsecond parameter instead.
    ///
    /// # Panics
    ///
    /// Panics on invalid hour, minute and/or second.
    #[deprecated(since = "0.4.23", note = "use `from_hms_opt()` instead")]
    #[inline]
    #[must_use]
    pub const fn from_hms(hour: u32, min: u32, sec: u32) -> NaiveTime {
        expect(NaiveTime::from_hms_opt(hour, min, sec), "invalid time")
    }

    /// Makes a new `NaiveTime` from hour, minute and second.
    ///
    /// The millisecond part is allowed to exceed 1,000,000,000 in order to represent a
    /// [leap second](#leap-second-handling), but only when `sec == 59`.
    ///
    /// # Errors
    ///
    /// Returns `None` on invalid hour, minute and/or second.
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::NaiveTime;
    ///
    /// let from_hms_opt = NaiveTime::from_hms_opt;
    ///
    /// assert!(from_hms_opt(0, 0, 0).is_some());
    /// assert!(from_hms_opt(23, 59, 59).is_some());
    /// assert!(from_hms_opt(24, 0, 0).is_none());
    /// assert!(from_hms_opt(23, 60, 0).is_none());
    /// assert!(from_hms_opt(23, 59, 60).is_none());
    /// ```
    #[inline]
    #[must_use]
    pub const fn from_hms_opt(hour: u32, min: u32, sec: u32) -> Option<NaiveTime> {
        NaiveTime::from_hms_nano_opt(hour, min, sec, 0)
    }

    /// Makes a new `NaiveTime` from hour, minute, second and millisecond.
    ///
    /// The millisecond part can exceed 1,000
    /// in order to represent the [leap second](#leap-second-handling).
    ///
    /// # Panics
    ///
    /// Panics on invalid hour, minute, second and/or millisecond.
    #[deprecated(since = "0.4.23", note = "use `from_hms_milli_opt()` instead")]
    #[inline]
    #[must_use]
    pub const fn from_hms_milli(hour: u32, min: u32, sec: u32, milli: u32) -> NaiveTime {
        expect(NaiveTime::from_hms_milli_opt(hour, min, sec, milli), "invalid time")
    }

    /// Makes a new `NaiveTime` from hour, minute, second and millisecond.
    ///
    /// The millisecond part is allowed to exceed 1,000,000,000 in order to represent a
    /// [leap second](#leap-second-handling), but only when `sec == 59`.
    ///
    /// # Errors
    ///
    /// Returns `None` on invalid hour, minute, second and/or millisecond.
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::NaiveTime;
    ///
    /// let from_hmsm_opt = NaiveTime::from_hms_milli_opt;
    ///
    /// assert!(from_hmsm_opt(0, 0, 0, 0).is_some());
    /// assert!(from_hmsm_opt(23, 59, 59, 999).is_some());
    /// assert!(from_hmsm_opt(23, 59, 59, 1_999).is_some()); // a leap second after 23:59:59
    /// assert!(from_hmsm_opt(24, 0, 0, 0).is_none());
    /// assert!(from_hmsm_opt(23, 60, 0, 0).is_none());
    /// assert!(from_hmsm_opt(23, 59, 60, 0).is_none());
    /// assert!(from_hmsm_opt(23, 59, 59, 2_000).is_none());
    /// ```
    #[inline]
    #[must_use]
    pub const fn from_hms_milli_opt(
        hour: u32,
        min: u32,
        sec: u32,
        milli: u32,
    ) -> Option<NaiveTime> {
        let nano = try_opt!(milli.checked_mul(1_000_000));
        NaiveTime::from_hms_nano_opt(hour, min, sec, nano)
    }

    /// Makes a new `NaiveTime` from hour, minute, second and microsecond.
    ///
    /// The microsecond part is allowed to exceed 1,000,000,000 in order to represent a
    /// [leap second](#leap-second-handling), but only when `sec == 59`.
    ///
    /// # Panics
    ///
    /// Panics on invalid hour, minute, second and/or microsecond.
    #[deprecated(since = "0.4.23", note = "use `from_hms_micro_opt()` instead")]
    #[inline]
    #[must_use]
    pub const fn from_hms_micro(hour: u32, min: u32, sec: u32, micro: u32) -> NaiveTime {
        expect(NaiveTime::from_hms_micro_opt(hour, min, sec, micro), "invalid time")
    }

    /// Makes a new `NaiveTime` from hour, minute, second and microsecond.
    ///
    /// The microsecond part is allowed to exceed 1,000,000,000 in order to represent a
    /// [leap second](#leap-second-handling), but only when `sec == 59`.
    ///
    /// # Errors
    ///
    /// Returns `None` on invalid hour, minute, second and/or microsecond.
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::NaiveTime;
    ///
    /// let from_hmsu_opt = NaiveTime::from_hms_micro_opt;
    ///
    /// assert!(from_hmsu_opt(0, 0, 0, 0).is_some());
    /// assert!(from_hmsu_opt(23, 59, 59, 999_999).is_some());
    /// assert!(from_hmsu_opt(23, 59, 59, 1_999_999).is_some()); // a leap second after 23:59:59
    /// assert!(from_hmsu_opt(24, 0, 0, 0).is_none());
    /// assert!(from_hmsu_opt(23, 60, 0, 0).is_none());
    /// assert!(from_hmsu_opt(23, 59, 60, 0).is_none());
    /// assert!(from_hmsu_opt(23, 59, 59, 2_000_000).is_none());
    /// ```
    #[inline]
    #[must_use]
    pub const fn from_hms_micro_opt(
        hour: u32,
        min: u32,
        sec: u32,
        micro: u32,
    ) -> Option<NaiveTime> {
        let nano = try_opt!(micro.checked_mul(1_000));
        NaiveTime::from_hms_nano_opt(hour, min, sec, nano)
    }

    /// Makes a new `NaiveTime` from hour, minute, second and nanosecond.
    ///
    /// The nanosecond part is allowed to exceed 1,000,000,000 in order to represent a
    /// [leap second](#leap-second-handling), but only when `sec == 59`.
    ///
    /// # Panics
    ///
    /// Panics on invalid hour, minute, second and/or nanosecond.
    #[deprecated(since = "0.4.23", note = "use `from_hms_nano_opt()` instead")]
    #[inline]
    #[must_use]
    pub const fn from_hms_nano(hour: u32, min: u32, sec: u32, nano: u32) -> NaiveTime {
        expect(NaiveTime::from_hms_nano_opt(hour, min, sec, nano), "invalid time")
    }

    /// Makes a new `NaiveTime` from hour, minute, second and nanosecond.
    ///
    /// The nanosecond part is allowed to exceed 1,000,000,000 in order to represent a
    /// [leap second](#leap-second-handling), but only when `sec == 59`.
    ///
    /// # Errors
    ///
    /// Returns `None` on invalid hour, minute, second and/or nanosecond.
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::NaiveTime;
    ///
    /// let from_hmsn_opt = NaiveTime::from_hms_nano_opt;
    ///
    /// assert!(from_hmsn_opt(0, 0, 0, 0).is_some());
    /// assert!(from_hmsn_opt(23, 59, 59, 999_999_999).is_some());
    /// assert!(from_hmsn_opt(23, 59, 59, 1_999_999_999).is_some()); // a leap second after 23:59:59
    /// assert!(from_hmsn_opt(24, 0, 0, 0).is_none());
    /// assert!(from_hmsn_opt(23, 60, 0, 0).is_none());
    /// assert!(from_hmsn_opt(23, 59, 60, 0).is_none());
    /// assert!(from_hmsn_opt(23, 59, 59, 2_000_000_000).is_none());
    /// ```
    #[inline]
    #[must_use]
    pub const fn from_hms_nano_opt(hour: u32, min: u32, sec: u32, nano: u32) -> Option<NaiveTime> {
        if (hour >= 24 || min >= 60 || sec >= 60)
            || (nano >= 1_000_000_000 && sec != 59)
            || nano >= 2_000_000_000
        {
            return None;
        }
        let secs = hour * 3600 + min * 60 + sec;
        Some(NaiveTime { secs, frac: nano })
    }

    /// Makes a new `NaiveTime` from the number of seconds since midnight and nanosecond.
    ///
    /// The nanosecond part is allowed to exceed 1,000,000,000 in order to represent a
    /// [leap second](#leap-second-handling), but only when `secs % 60 == 59`.
    ///
    /// # Panics
    ///
    /// Panics on invalid number of seconds and/or nanosecond.
    #[deprecated(since = "0.4.23", note = "use `from_num_seconds_from_midnight_opt()` instead")]
    #[inline]
    #[must_use]
    pub const fn from_num_seconds_from_midnight(secs: u32, nano: u32) -> NaiveTime {
        expect(NaiveTime::from_num_seconds_from_midnight_opt(secs, nano), "invalid time")
    }

    /// Makes a new `NaiveTime` from the number of seconds since midnight and nanosecond.
    ///
    /// The nanosecond part is allowed to exceed 1,000,000,000 in order to represent a
    /// [leap second](#leap-second-handling), but only when `secs % 60 == 59`.
    ///
    /// # Errors
    ///
    /// Returns `None` on invalid number of seconds and/or nanosecond.
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::NaiveTime;
    ///
    /// let from_nsecs_opt = NaiveTime::from_num_seconds_from_midnight_opt;
    ///
    /// assert!(from_nsecs_opt(0, 0).is_some());
    /// assert!(from_nsecs_opt(86399, 999_999_999).is_some());
    /// assert!(from_nsecs_opt(86399, 1_999_999_999).is_some()); // a leap second after 23:59:59
    /// assert!(from_nsecs_opt(86_400, 0).is_none());
    /// assert!(from_nsecs_opt(86399, 2_000_000_000).is_none());
    /// ```
    #[inline]
    #[must_use]
    pub const fn from_num_seconds_from_midnight_opt(secs: u32, nano: u32) -> Option<NaiveTime> {
        if secs >= 86_400 || nano >= 2_000_000_000 || (nano >= 1_000_000_000 && secs % 60 != 59) {
            return None;
        }
        Some(NaiveTime { secs, frac: nano })
    }

    /// Parses a string with the specified format string and returns a new `NaiveTime`.
    /// See the [`format::strftime` module](crate::format::strftime)
    /// on the supported escape sequences.
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::NaiveTime;
    ///
    /// let parse_from_str = NaiveTime::parse_from_str;
    ///
    /// assert_eq!(
    ///     parse_from_str("23:56:04", "%H:%M:%S"),
    ///     Ok(NaiveTime::from_hms_opt(23, 56, 4).unwrap())
    /// );
    /// assert_eq!(
    ///     parse_from_str("pm012345.6789", "%p%I%M%S%.f"),
    ///     Ok(NaiveTime::from_hms_micro_opt(13, 23, 45, 678_900).unwrap())
    /// );
    /// ```
    ///
    /// Date and offset is ignored for the purpose of parsing.
    ///
    /// ```
    /// # use chrono::NaiveTime;
    /// # let parse_from_str = NaiveTime::parse_from_str;
    /// assert_eq!(
    ///     parse_from_str("2014-5-17T12:34:56+09:30", "%Y-%m-%dT%H:%M:%S%z"),
    ///     Ok(NaiveTime::from_hms_opt(12, 34, 56).unwrap())
    /// );
    /// ```
    ///
    /// [Leap seconds](#leap-second-handling) are correctly handled by
    /// treating any time of the form `hh:mm:60` as a leap second.
    /// (This equally applies to the formatting, so the round trip is possible.)
    ///
    /// ```
    /// # use chrono::NaiveTime;
    /// # let parse_from_str = NaiveTime::parse_from_str;
    /// assert_eq!(
    ///     parse_from_str("08:59:60.123", "%H:%M:%S%.f"),
    ///     Ok(NaiveTime::from_hms_milli_opt(8, 59, 59, 1_123).unwrap())
    /// );
    /// ```
    ///
    /// Missing seconds are assumed to be zero,
    /// but out-of-bound times or insufficient fields are errors otherwise.
    ///
    /// ```
    /// # use chrono::NaiveTime;
    /// # let parse_from_str = NaiveTime::parse_from_str;
    /// assert_eq!(parse_from_str("7:15", "%H:%M"), Ok(NaiveTime::from_hms_opt(7, 15, 0).unwrap()));
    ///
    /// assert!(parse_from_str("04m33s", "%Mm%Ss").is_err());
    /// assert!(parse_from_str("12", "%H").is_err());
    /// assert!(parse_from_str("17:60", "%H:%M").is_err());
    /// assert!(parse_from_str("24:00:00", "%H:%M:%S").is_err());
    /// ```
    ///
    /// All parsed fields should be consistent to each other, otherwise it's an error.
    /// Here `%H` is for 24-hour clocks, unlike `%I`,
    /// and thus can be independently determined without AM/PM.
    ///
    /// ```
    /// # use chrono::NaiveTime;
    /// # let parse_from_str = NaiveTime::parse_from_str;
    /// assert!(parse_from_str("13:07 AM", "%H:%M %p").is_err());
    /// ```
    pub fn parse_from_str(s: &str, fmt: &str) -> ParseResult<NaiveTime> {
        let mut parsed = Parsed::new();
        parse(&mut parsed, s, StrftimeItems::new(fmt))?;
        parsed.to_naive_time()
    }

    /// Parses a string from a user-specified format into a new `NaiveTime` value, and a slice with
    /// the remaining portion of the string.
    /// See the [`format::strftime` module](crate::format::strftime)
    /// on the supported escape sequences.
    ///
    /// Similar to [`parse_from_str`](#method.parse_from_str).
    ///
    /// # Example
    ///
    /// ```rust
    /// # use chrono::{NaiveTime};
    /// let (time, remainder) =
    ///     NaiveTime::parse_and_remainder("3h4m33s trailing text", "%-Hh%-Mm%-Ss").unwrap();
    /// assert_eq!(time, NaiveTime::from_hms_opt(3, 4, 33).unwrap());
    /// assert_eq!(remainder, " trailing text");
    /// ```
    pub fn parse_and_remainder<'a>(s: &'a str, fmt: &str) -> ParseResult<(NaiveTime, &'a str)> {
        let mut parsed = Parsed::new();
        let remainder = parse_and_remainder(&mut parsed, s, StrftimeItems::new(fmt))?;
        parsed.to_naive_time().map(|t| (t, remainder))
    }

    /// Adds given `TimeDelta` to the current time, and also returns the number of *seconds*
    /// in the integral number of days ignored from the addition.
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::{NaiveTime, TimeDelta};
    ///
    /// let from_hms = |h, m, s| NaiveTime::from_hms_opt(h, m, s).unwrap();
    ///
    /// assert_eq!(
    ///     from_hms(3, 4, 5).overflowing_add_signed(TimeDelta::try_hours(11).unwrap()),
    ///     (from_hms(14, 4, 5), 0)
    /// );
    /// assert_eq!(
    ///     from_hms(3, 4, 5).overflowing_add_signed(TimeDelta::try_hours(23).unwrap()),
    ///     (from_hms(2, 4, 5), 86_400)
    /// );
    /// assert_eq!(
    ///     from_hms(3, 4, 5).overflowing_add_signed(TimeDelta::try_hours(-7).unwrap()),
    ///     (from_hms(20, 4, 5), -86_400)
    /// );
    /// ```
    #[must_use]
    pub const fn overflowing_add_signed(&self, rhs: TimeDelta) -> (NaiveTime, i64) {
        let mut secs = self.secs as i64;
        let mut frac = self.frac as i32;
        let secs_to_add = rhs.num_seconds();
        let frac_to_add = rhs.subsec_nanos();

        // Check if `self` is a leap second and adding `rhs` would escape that leap second.
        // If that is the case, update `frac` and `secs` to involve no leap second.
        // If it stays within the leap second or the second before, and only adds a fractional
        // second, just do that and return (this way the rest of the code can ignore leap seconds).
        if frac >= 1_000_000_000 {
            // check below is adjusted to not overflow an i32: `frac + frac_to_add >= 2_000_000_000`
            if secs_to_add > 0 || (frac_to_add > 0 && frac >= 2_000_000_000 - frac_to_add) {
                frac -= 1_000_000_000;
            } else if secs_to_add < 0 {
                frac -= 1_000_000_000;
                secs += 1;
            } else {
                return (NaiveTime { secs: self.secs, frac: (frac + frac_to_add) as u32 }, 0);
            }
        }

        let mut secs = secs + secs_to_add;
        frac += frac_to_add;

        if frac < 0 {
            frac += 1_000_000_000;
            secs -= 1;
        } else if frac >= 1_000_000_000 {
            frac -= 1_000_000_000;
            secs += 1;
        }

        let secs_in_day = secs.rem_euclid(86_400);
        let remaining = secs - secs_in_day;
        (NaiveTime { secs: secs_in_day as u32, frac: frac as u32 }, remaining)
    }

    /// Subtracts given `TimeDelta` from the current time, and also returns the number of *seconds*
    /// in the integral number of days ignored from the subtraction.
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::{NaiveTime, TimeDelta};
    ///
    /// let from_hms = |h, m, s| NaiveTime::from_hms_opt(h, m, s).unwrap();
    ///
    /// assert_eq!(
    ///     from_hms(3, 4, 5).overflowing_sub_signed(TimeDelta::try_hours(2).unwrap()),
    ///     (from_hms(1, 4, 5), 0)
    /// );
    /// assert_eq!(
    ///     from_hms(3, 4, 5).overflowing_sub_signed(TimeDelta::try_hours(17).unwrap()),
    ///     (from_hms(10, 4, 5), 86_400)
    /// );
    /// assert_eq!(
    ///     from_hms(3, 4, 5).overflowing_sub_signed(TimeDelta::try_hours(-22).unwrap()),
    ///     (from_hms(1, 4, 5), -86_400)
    /// );
    /// ```
    #[inline]
    #[must_use]
    pub const fn overflowing_sub_signed(&self, rhs: TimeDelta) -> (NaiveTime, i64) {
        let (time, rhs) = self.overflowing_add_signed(rhs.neg());
        (time, -rhs) // safe to negate, rhs is within +/- (2^63 / 1000)
    }

    /// Subtracts another `NaiveTime` from the current time.
    /// Returns a `TimeDelta` within +/- 1 day.
    /// This does not overflow or underflow at all.
    ///
    /// As a part of Chrono's [leap second handling](#leap-second-handling),
    /// the subtraction assumes that **there is no leap second ever**,
    /// except when any of the `NaiveTime`s themselves represents a leap second
    /// in which case the assumption becomes that
    /// **there are exactly one (or two) leap second(s) ever**.
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::{NaiveTime, TimeDelta};
    ///
    /// let from_hmsm = |h, m, s, milli| NaiveTime::from_hms_milli_opt(h, m, s, milli).unwrap();
    /// let since = NaiveTime::signed_duration_since;
    ///
    /// assert_eq!(since(from_hmsm(3, 5, 7, 900), from_hmsm(3, 5, 7, 900)), TimeDelta::zero());
    /// assert_eq!(
    ///     since(from_hmsm(3, 5, 7, 900), from_hmsm(3, 5, 7, 875)),
    ///     TimeDelta::try_milliseconds(25).unwrap()
    /// );
    /// assert_eq!(
    ///     since(from_hmsm(3, 5, 7, 900), from_hmsm(3, 5, 6, 925)),
    ///     TimeDelta::try_milliseconds(975).unwrap()
    /// );
    /// assert_eq!(
    ///     since(from_hmsm(3, 5, 7, 900), from_hmsm(3, 5, 0, 900)),
    ///     TimeDelta::try_seconds(7).unwrap()
    /// );
    /// assert_eq!(
    ///     since(from_hmsm(3, 5, 7, 900), from_hmsm(3, 0, 7, 900)),
    ///     TimeDelta::try_seconds(5 * 60).unwrap()
    /// );
    /// assert_eq!(
    ///     since(from_hmsm(3, 5, 7, 900), from_hmsm(0, 5, 7, 900)),
    ///     TimeDelta::try_seconds(3 * 3600).unwrap()
    /// );
    /// assert_eq!(
    ///     since(from_hmsm(3, 5, 7, 900), from_hmsm(4, 5, 7, 900)),
    ///     TimeDelta::try_seconds(-3600).unwrap()
    /// );
    /// assert_eq!(
    ///     since(from_hmsm(3, 5, 7, 900), from_hmsm(2, 4, 6, 800)),
    ///     TimeDelta::try_seconds(3600 + 60 + 1).unwrap() + TimeDelta::try_milliseconds(100).unwrap()
    /// );
    /// ```
    ///
    /// Leap seconds are handled, but the subtraction assumes that
    /// there were no other leap seconds happened.
    ///
    /// ```
    /// # use chrono::{TimeDelta, NaiveTime};
    /// # let from_hmsm = |h, m, s, milli| { NaiveTime::from_hms_milli_opt(h, m, s, milli).unwrap() };
    /// # let since = NaiveTime::signed_duration_since;
    /// assert_eq!(since(from_hmsm(3, 0, 59, 1_000), from_hmsm(3, 0, 59, 0)),
    ///            TimeDelta::try_seconds(1).unwrap());
    /// assert_eq!(since(from_hmsm(3, 0, 59, 1_500), from_hmsm(3, 0, 59, 0)),
    ///            TimeDelta::try_milliseconds(1500).unwrap());
    /// assert_eq!(since(from_hmsm(3, 0, 59, 1_000), from_hmsm(3, 0, 0, 0)),
    ///            TimeDelta::try_seconds(60).unwrap());
    /// assert_eq!(since(from_hmsm(3, 0, 0, 0), from_hmsm(2, 59, 59, 1_000)),
    ///            TimeDelta::try_seconds(1).unwrap());
    /// assert_eq!(since(from_hmsm(3, 0, 59, 1_000), from_hmsm(2, 59, 59, 1_000)),
    ///            TimeDelta::try_seconds(61).unwrap());
    /// ```
    #[must_use]
    pub const fn signed_duration_since(self, rhs: NaiveTime) -> TimeDelta {
        //     |    |    :leap|    |    |    |    |    |    |    :leap|    |
        //     |    |    :    |    |    |    |    |    |    |    :    |    |
        // ----+----+-----*---+----+----+----+----+----+----+-------*-+----+----
        //          |   `rhs` |                             |    `self`
        //          |======================================>|       |
        //          |     |  `self.secs - rhs.secs`         |`self.frac`
        //          |====>|   |                             |======>|
        //      `rhs.frac`|========================================>|
        //          |     |   |        `self - rhs`         |       |

        let mut secs = self.secs as i64 - rhs.secs as i64;
        let frac = self.frac as i64 - rhs.frac as i64;

        // `secs` may contain a leap second yet to be counted
        if self.secs > rhs.secs && rhs.frac >= 1_000_000_000 {
            secs += 1;
        } else if self.secs < rhs.secs && self.frac >= 1_000_000_000 {
            secs -= 1;
        }

        let secs_from_frac = frac.div_euclid(1_000_000_000);
        let frac = frac.rem_euclid(1_000_000_000) as u32;

        expect(TimeDelta::new(secs + secs_from_frac, frac), "must be in range")
    }

    /// Adds given `FixedOffset` to the current time, and returns the number of days that should be
    /// added to a date as a result of the offset (either `-1`, `0`, or `1` because the offset is
    /// always less than 24h).
    ///
    /// This method is similar to [`overflowing_add_signed`](#method.overflowing_add_signed), but
    /// preserves leap seconds.
    pub(super) const fn overflowing_add_offset(&self, offset: FixedOffset) -> (NaiveTime, i32) {
        let secs = self.secs as i32 + offset.local_minus_utc();
        let days = secs.div_euclid(86_400);
        let secs = secs.rem_euclid(86_400);
        (NaiveTime { secs: secs as u32, frac: self.frac }, days)
    }

    /// Subtracts given `FixedOffset` from the current time, and returns the number of days that
    /// should be added to a date as a result of the offset (either `-1`, `0`, or `1` because the
    /// offset is always less than 24h).
    ///
    /// This method is similar to [`overflowing_sub_signed`](#method.overflowing_sub_signed), but
    /// preserves leap seconds.
    pub(super) const fn overflowing_sub_offset(&self, offset: FixedOffset) -> (NaiveTime, i32) {
        let secs = self.secs as i32 - offset.local_minus_utc();
        let days = secs.div_euclid(86_400);
        let secs = secs.rem_euclid(86_400);
        (NaiveTime { secs: secs as u32, frac: self.frac }, days)
    }

    /// Formats the time with the specified formatting items.
    /// Otherwise it is the same as the ordinary [`format`](#method.format) method.
    ///
    /// The `Iterator` of items should be `Clone`able,
    /// since the resulting `DelayedFormat` value may be formatted multiple times.
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::format::strftime::StrftimeItems;
    /// use chrono::NaiveTime;
    ///
    /// let fmt = StrftimeItems::new("%H:%M:%S");
    /// let t = NaiveTime::from_hms_opt(23, 56, 4).unwrap();
    /// assert_eq!(t.format_with_items(fmt.clone()).to_string(), "23:56:04");
    /// assert_eq!(t.format("%H:%M:%S").to_string(), "23:56:04");
    /// ```
    ///
    /// The resulting `DelayedFormat` can be formatted directly via the `Display` trait.
    ///
    /// ```
    /// # use chrono::NaiveTime;
    /// # use chrono::format::strftime::StrftimeItems;
    /// # let fmt = StrftimeItems::new("%H:%M:%S").clone();
    /// # let t = NaiveTime::from_hms_opt(23, 56, 4).unwrap();
    /// assert_eq!(format!("{}", t.format_with_items(fmt)), "23:56:04");
    /// ```
    #[cfg(feature = "alloc")]
    #[inline]
    #[must_use]
    pub fn format_with_items<'a, I, B>(&self, items: I) -> DelayedFormat<I>
    where
        I: Iterator<Item = B> + Clone,
        B: Borrow<Item<'a>>,
    {
        DelayedFormat::new(None, Some(*self), items)
    }

    /// Formats the time with the specified format string.
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
    /// use chrono::NaiveTime;
    ///
    /// let t = NaiveTime::from_hms_nano_opt(23, 56, 4, 12_345_678).unwrap();
    /// assert_eq!(t.format("%H:%M:%S").to_string(), "23:56:04");
    /// assert_eq!(t.format("%H:%M:%S%.6f").to_string(), "23:56:04.012345");
    /// assert_eq!(t.format("%-I:%M %p").to_string(), "11:56 PM");
    /// ```
    ///
    /// The resulting `DelayedFormat` can be formatted directly via the `Display` trait.
    ///
    /// ```
    /// # use chrono::NaiveTime;
    /// # let t = NaiveTime::from_hms_nano_opt(23, 56, 4, 12_345_678).unwrap();
    /// assert_eq!(format!("{}", t.format("%H:%M:%S")), "23:56:04");
    /// assert_eq!(format!("{}", t.format("%H:%M:%S%.6f")), "23:56:04.012345");
    /// assert_eq!(format!("{}", t.format("%-I:%M %p")), "11:56 PM");
    /// ```
    #[cfg(feature = "alloc")]
    #[inline]
    #[must_use]
    pub fn format<'a>(&self, fmt: &'a str) -> DelayedFormat<StrftimeItems<'a>> {
        self.format_with_items(StrftimeItems::new(fmt))
    }

    /// Returns a triple of the hour, minute and second numbers.
    pub(crate) fn hms(&self) -> (u32, u32, u32) {
        let sec = self.secs % 60;
        let mins = self.secs / 60;
        let min = mins % 60;
        let hour = mins / 60;
        (hour, min, sec)
    }

    /// Returns the number of non-leap seconds past the last midnight.
    // This duplicates `Timelike::num_seconds_from_midnight()`, because trait methods can't be const
    // yet.
    #[inline]
    pub(crate) const fn num_seconds_from_midnight(&self) -> u32 {
        self.secs
    }

    /// Returns the number of nanoseconds since the whole non-leap second.
    // This duplicates `Timelike::nanosecond()`, because trait methods can't be const yet.
    #[inline]
    pub(crate) const fn nanosecond(&self) -> u32 {
        self.frac
    }

    /// The earliest possible `NaiveTime`
    pub const MIN: Self = Self { secs: 0, frac: 0 };
    pub(super) const MAX: Self = Self { secs: 23 * 3600 + 59 * 60 + 59, frac: 999_999_999 };
}

impl Timelike for NaiveTime {
    /// Returns the hour number from 0 to 23.
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::{NaiveTime, Timelike};
    ///
    /// assert_eq!(NaiveTime::from_hms_opt(0, 0, 0).unwrap().hour(), 0);
    /// assert_eq!(NaiveTime::from_hms_nano_opt(23, 56, 4, 12_345_678).unwrap().hour(), 23);
    /// ```
    #[inline]
    fn hour(&self) -> u32 {
        self.hms().0
    }

    /// Returns the minute number from 0 to 59.
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::{NaiveTime, Timelike};
    ///
    /// assert_eq!(NaiveTime::from_hms_opt(0, 0, 0).unwrap().minute(), 0);
    /// assert_eq!(NaiveTime::from_hms_nano_opt(23, 56, 4, 12_345_678).unwrap().minute(), 56);
    /// ```
    #[inline]
    fn minute(&self) -> u32 {
        self.hms().1
    }

    /// Returns the second number from 0 to 59.
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::{NaiveTime, Timelike};
    ///
    /// assert_eq!(NaiveTime::from_hms_opt(0, 0, 0).unwrap().second(), 0);
    /// assert_eq!(NaiveTime::from_hms_nano_opt(23, 56, 4, 12_345_678).unwrap().second(), 4);
    /// ```
    ///
    /// This method never returns 60 even when it is a leap second.
    /// ([Why?](#leap-second-handling))
    /// Use the proper [formatting method](#method.format) to get a human-readable representation.
    ///
    /// ```
    /// # #[cfg(feature = "alloc")] {
    /// # use chrono::{NaiveTime, Timelike};
    /// let leap = NaiveTime::from_hms_milli_opt(23, 59, 59, 1_000).unwrap();
    /// assert_eq!(leap.second(), 59);
    /// assert_eq!(leap.format("%H:%M:%S").to_string(), "23:59:60");
    /// # }
    /// ```
    #[inline]
    fn second(&self) -> u32 {
        self.hms().2
    }

    /// Returns the number of nanoseconds since the whole non-leap second.
    /// The range from 1,000,000,000 to 1,999,999,999 represents
    /// the [leap second](#leap-second-handling).
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::{NaiveTime, Timelike};
    ///
    /// assert_eq!(NaiveTime::from_hms_opt(0, 0, 0).unwrap().nanosecond(), 0);
    /// assert_eq!(
    ///     NaiveTime::from_hms_nano_opt(23, 56, 4, 12_345_678).unwrap().nanosecond(),
    ///     12_345_678
    /// );
    /// ```
    ///
    /// Leap seconds may have seemingly out-of-range return values.
    /// You can reduce the range with `time.nanosecond() % 1_000_000_000`, or
    /// use the proper [formatting method](#method.format) to get a human-readable representation.
    ///
    /// ```
    /// # #[cfg(feature = "alloc")] {
    /// # use chrono::{NaiveTime, Timelike};
    /// let leap = NaiveTime::from_hms_milli_opt(23, 59, 59, 1_000).unwrap();
    /// assert_eq!(leap.nanosecond(), 1_000_000_000);
    /// assert_eq!(leap.format("%H:%M:%S%.9f").to_string(), "23:59:60.000000000");
    /// # }
    /// ```
    #[inline]
    fn nanosecond(&self) -> u32 {
        self.frac
    }

    /// Makes a new `NaiveTime` with the hour number changed.
    ///
    /// # Errors
    ///
    /// Returns `None` if the value for `hour` is invalid.
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::{NaiveTime, Timelike};
    ///
    /// let dt = NaiveTime::from_hms_nano_opt(23, 56, 4, 12_345_678).unwrap();
    /// assert_eq!(dt.with_hour(7), Some(NaiveTime::from_hms_nano_opt(7, 56, 4, 12_345_678).unwrap()));
    /// assert_eq!(dt.with_hour(24), None);
    /// ```
    #[inline]
    fn with_hour(&self, hour: u32) -> Option<NaiveTime> {
        if hour >= 24 {
            return None;
        }
        let secs = hour * 3600 + self.secs % 3600;
        Some(NaiveTime { secs, ..*self })
    }

    /// Makes a new `NaiveTime` with the minute number changed.
    ///
    /// # Errors
    ///
    /// Returns `None` if the value for `minute` is invalid.
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::{NaiveTime, Timelike};
    ///
    /// let dt = NaiveTime::from_hms_nano_opt(23, 56, 4, 12_345_678).unwrap();
    /// assert_eq!(
    ///     dt.with_minute(45),
    ///     Some(NaiveTime::from_hms_nano_opt(23, 45, 4, 12_345_678).unwrap())
    /// );
    /// assert_eq!(dt.with_minute(60), None);
    /// ```
    #[inline]
    fn with_minute(&self, min: u32) -> Option<NaiveTime> {
        if min >= 60 {
            return None;
        }
        let secs = self.secs / 3600 * 3600 + min * 60 + self.secs % 60;
        Some(NaiveTime { secs, ..*self })
    }

    /// Makes a new `NaiveTime` with the second number changed.
    ///
    /// As with the [`second`](#method.second) method,
    /// the input range is restricted to 0 through 59.
    ///
    /// # Errors
    ///
    /// Returns `None` if the value for `second` is invalid.
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::{NaiveTime, Timelike};
    ///
    /// let dt = NaiveTime::from_hms_nano_opt(23, 56, 4, 12_345_678).unwrap();
    /// assert_eq!(
    ///     dt.with_second(17),
    ///     Some(NaiveTime::from_hms_nano_opt(23, 56, 17, 12_345_678).unwrap())
    /// );
    /// assert_eq!(dt.with_second(60), None);
    /// ```
    #[inline]
    fn with_second(&self, sec: u32) -> Option<NaiveTime> {
        if sec >= 60 {
            return None;
        }
        let secs = self.secs / 60 * 60 + sec;
        Some(NaiveTime { secs, ..*self })
    }

    /// Makes a new `NaiveTime` with nanoseconds since the whole non-leap second changed.
    ///
    /// As with the [`nanosecond`](#method.nanosecond) method,
    /// the input range can exceed 1,000,000,000 for leap seconds.
    ///
    /// # Errors
    ///
    /// Returns `None` if `nanosecond >= 2,000,000,000`.
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::{NaiveTime, Timelike};
    ///
    /// let dt = NaiveTime::from_hms_nano_opt(23, 56, 4, 12_345_678).unwrap();
    /// assert_eq!(
    ///     dt.with_nanosecond(333_333_333),
    ///     Some(NaiveTime::from_hms_nano_opt(23, 56, 4, 333_333_333).unwrap())
    /// );
    /// assert_eq!(dt.with_nanosecond(2_000_000_000), None);
    /// ```
    ///
    /// Leap seconds can theoretically follow *any* whole second.
    /// The following would be a proper leap second at the time zone offset of UTC-00:03:57
    /// (there are several historical examples comparable to this "non-sense" offset),
    /// and therefore is allowed.
    ///
    /// ```
    /// # use chrono::{NaiveTime, Timelike};
    /// let dt = NaiveTime::from_hms_nano_opt(23, 56, 4, 12_345_678).unwrap();
    /// let strange_leap_second = dt.with_nanosecond(1_333_333_333).unwrap();
    /// assert_eq!(strange_leap_second.nanosecond(), 1_333_333_333);
    /// ```
    #[inline]
    fn with_nanosecond(&self, nano: u32) -> Option<NaiveTime> {
        if nano >= 2_000_000_000 {
            return None;
        }
        Some(NaiveTime { frac: nano, ..*self })
    }

    /// Returns the number of non-leap seconds past the last midnight.
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::{NaiveTime, Timelike};
    ///
    /// assert_eq!(NaiveTime::from_hms_opt(1, 2, 3).unwrap().num_seconds_from_midnight(), 3723);
    /// assert_eq!(
    ///     NaiveTime::from_hms_nano_opt(23, 56, 4, 12_345_678).unwrap().num_seconds_from_midnight(),
    ///     86164
    /// );
    /// assert_eq!(
    ///     NaiveTime::from_hms_milli_opt(23, 59, 59, 1_000).unwrap().num_seconds_from_midnight(),
    ///     86399
    /// );
    /// ```
    #[inline]
    fn num_seconds_from_midnight(&self) -> u32 {
        self.secs // do not repeat the calculation!
    }
}

/// Add `TimeDelta` to `NaiveTime`.
///
/// This wraps around and never overflows or underflows.
/// In particular the addition ignores integral number of days.
///
/// As a part of Chrono's [leap second handling], the addition assumes that **there is no leap
/// second ever**, except when the `NaiveTime` itself represents a leap second in which case the
/// assumption becomes that **there is exactly a single leap second ever**.
///
/// # Example
///
/// ```
/// use chrono::{NaiveTime, TimeDelta};
///
/// let from_hmsm = |h, m, s, milli| NaiveTime::from_hms_milli_opt(h, m, s, milli).unwrap();
///
/// assert_eq!(from_hmsm(3, 5, 7, 0) + TimeDelta::zero(), from_hmsm(3, 5, 7, 0));
/// assert_eq!(from_hmsm(3, 5, 7, 0) + TimeDelta::try_seconds(1).unwrap(), from_hmsm(3, 5, 8, 0));
/// assert_eq!(from_hmsm(3, 5, 7, 0) + TimeDelta::try_seconds(-1).unwrap(), from_hmsm(3, 5, 6, 0));
/// assert_eq!(
///     from_hmsm(3, 5, 7, 0) + TimeDelta::try_seconds(60 + 4).unwrap(),
///     from_hmsm(3, 6, 11, 0)
/// );
/// assert_eq!(
///     from_hmsm(3, 5, 7, 0) + TimeDelta::try_seconds(7 * 60 * 60 - 6 * 60).unwrap(),
///     from_hmsm(9, 59, 7, 0)
/// );
/// assert_eq!(
///     from_hmsm(3, 5, 7, 0) + TimeDelta::try_milliseconds(80).unwrap(),
///     from_hmsm(3, 5, 7, 80)
/// );
/// assert_eq!(
///     from_hmsm(3, 5, 7, 950) + TimeDelta::try_milliseconds(280).unwrap(),
///     from_hmsm(3, 5, 8, 230)
/// );
/// assert_eq!(
///     from_hmsm(3, 5, 7, 950) + TimeDelta::try_milliseconds(-980).unwrap(),
///     from_hmsm(3, 5, 6, 970)
/// );
/// ```
///
/// The addition wraps around.
///
/// ```
/// # use chrono::{TimeDelta, NaiveTime};
/// # let from_hmsm = |h, m, s, milli| { NaiveTime::from_hms_milli_opt(h, m, s, milli).unwrap() };
/// assert_eq!(from_hmsm(3, 5, 7, 0) + TimeDelta::try_seconds(22*60*60).unwrap(), from_hmsm(1, 5, 7, 0));
/// assert_eq!(from_hmsm(3, 5, 7, 0) + TimeDelta::try_seconds(-8*60*60).unwrap(), from_hmsm(19, 5, 7, 0));
/// assert_eq!(from_hmsm(3, 5, 7, 0) + TimeDelta::try_days(800).unwrap(), from_hmsm(3, 5, 7, 0));
/// ```
///
/// Leap seconds are handled, but the addition assumes that it is the only leap second happened.
///
/// ```
/// # use chrono::{TimeDelta, NaiveTime};
/// # let from_hmsm = |h, m, s, milli| { NaiveTime::from_hms_milli_opt(h, m, s, milli).unwrap() };
/// let leap = from_hmsm(3, 5, 59, 1_300);
/// assert_eq!(leap + TimeDelta::zero(), from_hmsm(3, 5, 59, 1_300));
/// assert_eq!(leap + TimeDelta::try_milliseconds(-500).unwrap(), from_hmsm(3, 5, 59, 800));
/// assert_eq!(leap + TimeDelta::try_milliseconds(500).unwrap(), from_hmsm(3, 5, 59, 1_800));
/// assert_eq!(leap + TimeDelta::try_milliseconds(800).unwrap(), from_hmsm(3, 6, 0, 100));
/// assert_eq!(leap + TimeDelta::try_seconds(10).unwrap(), from_hmsm(3, 6, 9, 300));
/// assert_eq!(leap + TimeDelta::try_seconds(-10).unwrap(), from_hmsm(3, 5, 50, 300));
/// assert_eq!(leap + TimeDelta::try_days(1).unwrap(), from_hmsm(3, 5, 59, 300));
/// ```
///
/// [leap second handling]: crate::NaiveTime#leap-second-handling
impl Add<TimeDelta> for NaiveTime {
    type Output = NaiveTime;

    #[inline]
    fn add(self, rhs: TimeDelta) -> NaiveTime {
        self.overflowing_add_signed(rhs).0
    }
}

/// Add-assign `TimeDelta` to `NaiveTime`.
///
/// This wraps around and never overflows or underflows.
/// In particular the addition ignores integral number of days.
impl AddAssign<TimeDelta> for NaiveTime {
    #[inline]
    fn add_assign(&mut self, rhs: TimeDelta) {
        *self = self.add(rhs);
    }
}

/// Add `std::time::Duration` to `NaiveTime`.
///
/// This wraps around and never overflows or underflows.
/// In particular the addition ignores integral number of days.
impl Add<Duration> for NaiveTime {
    type Output = NaiveTime;

    #[inline]
    fn add(self, rhs: Duration) -> NaiveTime {
        // We don't care about values beyond `24 * 60 * 60`, so we can take a modulus and avoid
        // overflow during the conversion to `TimeDelta`.
        // But we limit to double that just in case `self` is a leap-second.
        let secs = rhs.as_secs() % (2 * 24 * 60 * 60);
        let d = TimeDelta::new(secs as i64, rhs.subsec_nanos()).unwrap();
        self.overflowing_add_signed(d).0
    }
}

/// Add-assign `std::time::Duration` to `NaiveTime`.
///
/// This wraps around and never overflows or underflows.
/// In particular the addition ignores integral number of days.
impl AddAssign<Duration> for NaiveTime {
    #[inline]
    fn add_assign(&mut self, rhs: Duration) {
        *self = *self + rhs;
    }
}

/// Add `FixedOffset` to `NaiveTime`.
///
/// This wraps around and never overflows or underflows.
/// In particular the addition ignores integral number of days.
impl Add<FixedOffset> for NaiveTime {
    type Output = NaiveTime;

    #[inline]
    fn add(self, rhs: FixedOffset) -> NaiveTime {
        self.overflowing_add_offset(rhs).0
    }
}

/// Subtract `TimeDelta` from `NaiveTime`.
///
/// This wraps around and never overflows or underflows.
/// In particular the subtraction ignores integral number of days.
/// This is the same as addition with a negated `TimeDelta`.
///
/// As a part of Chrono's [leap second handling], the subtraction assumes that **there is no leap
/// second ever**, except when the `NaiveTime` itself represents a leap second in which case the
/// assumption becomes that **there is exactly a single leap second ever**.
///
/// # Example
///
/// ```
/// use chrono::{NaiveTime, TimeDelta};
///
/// let from_hmsm = |h, m, s, milli| NaiveTime::from_hms_milli_opt(h, m, s, milli).unwrap();
///
/// assert_eq!(from_hmsm(3, 5, 7, 0) - TimeDelta::zero(), from_hmsm(3, 5, 7, 0));
/// assert_eq!(from_hmsm(3, 5, 7, 0) - TimeDelta::try_seconds(1).unwrap(), from_hmsm(3, 5, 6, 0));
/// assert_eq!(
///     from_hmsm(3, 5, 7, 0) - TimeDelta::try_seconds(60 + 5).unwrap(),
///     from_hmsm(3, 4, 2, 0)
/// );
/// assert_eq!(
///     from_hmsm(3, 5, 7, 0) - TimeDelta::try_seconds(2 * 60 * 60 + 6 * 60).unwrap(),
///     from_hmsm(0, 59, 7, 0)
/// );
/// assert_eq!(
///     from_hmsm(3, 5, 7, 0) - TimeDelta::try_milliseconds(80).unwrap(),
///     from_hmsm(3, 5, 6, 920)
/// );
/// assert_eq!(
///     from_hmsm(3, 5, 7, 950) - TimeDelta::try_milliseconds(280).unwrap(),
///     from_hmsm(3, 5, 7, 670)
/// );
/// ```
///
/// The subtraction wraps around.
///
/// ```
/// # use chrono::{TimeDelta, NaiveTime};
/// # let from_hmsm = |h, m, s, milli| { NaiveTime::from_hms_milli_opt(h, m, s, milli).unwrap() };
/// assert_eq!(from_hmsm(3, 5, 7, 0) - TimeDelta::try_seconds(8*60*60).unwrap(), from_hmsm(19, 5, 7, 0));
/// assert_eq!(from_hmsm(3, 5, 7, 0) - TimeDelta::try_days(800).unwrap(), from_hmsm(3, 5, 7, 0));
/// ```
///
/// Leap seconds are handled, but the subtraction assumes that it is the only leap second happened.
///
/// ```
/// # use chrono::{TimeDelta, NaiveTime};
/// # let from_hmsm = |h, m, s, milli| { NaiveTime::from_hms_milli_opt(h, m, s, milli).unwrap() };
/// let leap = from_hmsm(3, 5, 59, 1_300);
/// assert_eq!(leap - TimeDelta::zero(), from_hmsm(3, 5, 59, 1_300));
/// assert_eq!(leap - TimeDelta::try_milliseconds(200).unwrap(), from_hmsm(3, 5, 59, 1_100));
/// assert_eq!(leap - TimeDelta::try_milliseconds(500).unwrap(), from_hmsm(3, 5, 59, 800));
/// assert_eq!(leap - TimeDelta::try_seconds(60).unwrap(), from_hmsm(3, 5, 0, 300));
/// assert_eq!(leap - TimeDelta::try_days(1).unwrap(), from_hmsm(3, 6, 0, 300));
/// ```
///
/// [leap second handling]: crate::NaiveTime#leap-second-handling
impl Sub<TimeDelta> for NaiveTime {
    type Output = NaiveTime;

    #[inline]
    fn sub(self, rhs: TimeDelta) -> NaiveTime {
        self.overflowing_sub_signed(rhs).0
    }
}

/// Subtract-assign `TimeDelta` from `NaiveTime`.
///
/// This wraps around and never overflows or underflows.
/// In particular the subtraction ignores integral number of days.
impl SubAssign<TimeDelta> for NaiveTime {
    #[inline]
    fn sub_assign(&mut self, rhs: TimeDelta) {
        *self = self.sub(rhs);
    }
}

/// Subtract `std::time::Duration` from `NaiveTime`.
///
/// This wraps around and never overflows or underflows.
/// In particular the subtraction ignores integral number of days.
impl Sub<Duration> for NaiveTime {
    type Output = NaiveTime;

    #[inline]
    fn sub(self, rhs: Duration) -> NaiveTime {
        // We don't care about values beyond `24 * 60 * 60`, so we can take a modulus and avoid
        // overflow during the conversion to `TimeDelta`.
        // But we limit to double that just in case `self` is a leap-second.
        let secs = rhs.as_secs() % (2 * 24 * 60 * 60);
        let d = TimeDelta::new(secs as i64, rhs.subsec_nanos()).unwrap();
        self.overflowing_sub_signed(d).0
    }
}

/// Subtract-assign `std::time::Duration` from `NaiveTime`.
///
/// This wraps around and never overflows or underflows.
/// In particular the subtraction ignores integral number of days.
impl SubAssign<Duration> for NaiveTime {
    #[inline]
    fn sub_assign(&mut self, rhs: Duration) {
        *self = *self - rhs;
    }
}

/// Subtract `FixedOffset` from `NaiveTime`.
///
/// This wraps around and never overflows or underflows.
/// In particular the subtraction ignores integral number of days.
impl Sub<FixedOffset> for NaiveTime {
    type Output = NaiveTime;

    #[inline]
    fn sub(self, rhs: FixedOffset) -> NaiveTime {
        self.overflowing_sub_offset(rhs).0
    }
}

/// Subtracts another `NaiveTime` from the current time.
/// Returns a `TimeDelta` within +/- 1 day.
/// This does not overflow or underflow at all.
///
/// As a part of Chrono's [leap second handling](#leap-second-handling),
/// the subtraction assumes that **there is no leap second ever**,
/// except when any of the `NaiveTime`s themselves represents a leap second
/// in which case the assumption becomes that
/// **there are exactly one (or two) leap second(s) ever**.
///
/// The implementation is a wrapper around
/// [`NaiveTime::signed_duration_since`](#method.signed_duration_since).
///
/// # Example
///
/// ```
/// use chrono::{NaiveTime, TimeDelta};
///
/// let from_hmsm = |h, m, s, milli| NaiveTime::from_hms_milli_opt(h, m, s, milli).unwrap();
///
/// assert_eq!(from_hmsm(3, 5, 7, 900) - from_hmsm(3, 5, 7, 900), TimeDelta::zero());
/// assert_eq!(
///     from_hmsm(3, 5, 7, 900) - from_hmsm(3, 5, 7, 875),
///     TimeDelta::try_milliseconds(25).unwrap()
/// );
/// assert_eq!(
///     from_hmsm(3, 5, 7, 900) - from_hmsm(3, 5, 6, 925),
///     TimeDelta::try_milliseconds(975).unwrap()
/// );
/// assert_eq!(
///     from_hmsm(3, 5, 7, 900) - from_hmsm(3, 5, 0, 900),
///     TimeDelta::try_seconds(7).unwrap()
/// );
/// assert_eq!(
///     from_hmsm(3, 5, 7, 900) - from_hmsm(3, 0, 7, 900),
///     TimeDelta::try_seconds(5 * 60).unwrap()
/// );
/// assert_eq!(
///     from_hmsm(3, 5, 7, 900) - from_hmsm(0, 5, 7, 900),
///     TimeDelta::try_seconds(3 * 3600).unwrap()
/// );
/// assert_eq!(
///     from_hmsm(3, 5, 7, 900) - from_hmsm(4, 5, 7, 900),
///     TimeDelta::try_seconds(-3600).unwrap()
/// );
/// assert_eq!(
///     from_hmsm(3, 5, 7, 900) - from_hmsm(2, 4, 6, 800),
///     TimeDelta::try_seconds(3600 + 60 + 1).unwrap() + TimeDelta::try_milliseconds(100).unwrap()
/// );
/// ```
///
/// Leap seconds are handled, but the subtraction assumes that
/// there were no other leap seconds happened.
///
/// ```
/// # use chrono::{TimeDelta, NaiveTime};
/// # let from_hmsm = |h, m, s, milli| { NaiveTime::from_hms_milli_opt(h, m, s, milli).unwrap() };
/// assert_eq!(from_hmsm(3, 0, 59, 1_000) - from_hmsm(3, 0, 59, 0), TimeDelta::try_seconds(1).unwrap());
/// assert_eq!(from_hmsm(3, 0, 59, 1_500) - from_hmsm(3, 0, 59, 0),
///            TimeDelta::try_milliseconds(1500).unwrap());
/// assert_eq!(from_hmsm(3, 0, 59, 1_000) - from_hmsm(3, 0, 0, 0), TimeDelta::try_seconds(60).unwrap());
/// assert_eq!(from_hmsm(3, 0, 0, 0) - from_hmsm(2, 59, 59, 1_000), TimeDelta::try_seconds(1).unwrap());
/// assert_eq!(from_hmsm(3, 0, 59, 1_000) - from_hmsm(2, 59, 59, 1_000),
///            TimeDelta::try_seconds(61).unwrap());
/// ```
impl Sub<NaiveTime> for NaiveTime {
    type Output = TimeDelta;

    #[inline]
    fn sub(self, rhs: NaiveTime) -> TimeDelta {
        self.signed_duration_since(rhs)
    }
}

/// The `Debug` output of the naive time `t` is the same as
/// [`t.format("%H:%M:%S%.f")`](crate::format::strftime).
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
/// use chrono::NaiveTime;
///
/// assert_eq!(format!("{:?}", NaiveTime::from_hms_opt(23, 56, 4).unwrap()), "23:56:04");
/// assert_eq!(
///     format!("{:?}", NaiveTime::from_hms_milli_opt(23, 56, 4, 12).unwrap()),
///     "23:56:04.012"
/// );
/// assert_eq!(
///     format!("{:?}", NaiveTime::from_hms_micro_opt(23, 56, 4, 1234).unwrap()),
///     "23:56:04.001234"
/// );
/// assert_eq!(
///     format!("{:?}", NaiveTime::from_hms_nano_opt(23, 56, 4, 123456).unwrap()),
///     "23:56:04.000123456"
/// );
/// ```
///
/// Leap seconds may also be used.
///
/// ```
/// # use chrono::NaiveTime;
/// assert_eq!(
///     format!("{:?}", NaiveTime::from_hms_milli_opt(6, 59, 59, 1_500).unwrap()),
///     "06:59:60.500"
/// );
/// ```
impl fmt::Debug for NaiveTime {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let (hour, min, sec) = self.hms();
        let (sec, nano) = if self.frac >= 1_000_000_000 {
            (sec + 1, self.frac - 1_000_000_000)
        } else {
            (sec, self.frac)
        };

        use core::fmt::Write;
        write_hundreds(f, hour as u8)?;
        f.write_char(':')?;
        write_hundreds(f, min as u8)?;
        f.write_char(':')?;
        write_hundreds(f, sec as u8)?;

        if nano == 0 {
            Ok(())
        } else if nano % 1_000_000 == 0 {
            write!(f, ".{:03}", nano / 1_000_000)
        } else if nano % 1_000 == 0 {
            write!(f, ".{:06}", nano / 1_000)
        } else {
            write!(f, ".{nano:09}")
        }
    }
}

/// The `Display` output of the naive time `t` is the same as
/// [`t.format("%H:%M:%S%.f")`](crate::format::strftime).
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
/// use chrono::NaiveTime;
///
/// assert_eq!(format!("{}", NaiveTime::from_hms_opt(23, 56, 4).unwrap()), "23:56:04");
/// assert_eq!(
///     format!("{}", NaiveTime::from_hms_milli_opt(23, 56, 4, 12).unwrap()),
///     "23:56:04.012"
/// );
/// assert_eq!(
///     format!("{}", NaiveTime::from_hms_micro_opt(23, 56, 4, 1234).unwrap()),
///     "23:56:04.001234"
/// );
/// assert_eq!(
///     format!("{}", NaiveTime::from_hms_nano_opt(23, 56, 4, 123456).unwrap()),
///     "23:56:04.000123456"
/// );
/// ```
///
/// Leap seconds may also be used.
///
/// ```
/// # use chrono::NaiveTime;
/// assert_eq!(
///     format!("{}", NaiveTime::from_hms_milli_opt(6, 59, 59, 1_500).unwrap()),
///     "06:59:60.500"
/// );
/// ```
impl fmt::Display for NaiveTime {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

/// Parsing a `str` into a `NaiveTime` uses the same format,
/// [`%H:%M:%S%.f`](crate::format::strftime), as in `Debug` and `Display`.
///
/// # Example
///
/// ```
/// use chrono::NaiveTime;
///
/// let t = NaiveTime::from_hms_opt(23, 56, 4).unwrap();
/// assert_eq!("23:56:04".parse::<NaiveTime>(), Ok(t));
///
/// let t = NaiveTime::from_hms_nano_opt(23, 56, 4, 12_345_678).unwrap();
/// assert_eq!("23:56:4.012345678".parse::<NaiveTime>(), Ok(t));
///
/// let t = NaiveTime::from_hms_nano_opt(23, 59, 59, 1_234_567_890).unwrap(); // leap second
/// assert_eq!("23:59:60.23456789".parse::<NaiveTime>(), Ok(t));
///
/// // Seconds are optional
/// let t = NaiveTime::from_hms_opt(23, 56, 0).unwrap();
/// assert_eq!("23:56".parse::<NaiveTime>(), Ok(t));
///
/// assert!("foo".parse::<NaiveTime>().is_err());
/// ```
impl str::FromStr for NaiveTime {
    type Err = ParseError;

    fn from_str(s: &str) -> ParseResult<NaiveTime> {
        const HOUR_AND_MINUTE: &[Item<'static>] = &[
            Item::Numeric(Numeric::Hour, Pad::Zero),
            Item::Space(""),
            Item::Literal(":"),
            Item::Numeric(Numeric::Minute, Pad::Zero),
        ];
        const SECOND_AND_NANOS: &[Item<'static>] = &[
            Item::Space(""),
            Item::Literal(":"),
            Item::Numeric(Numeric::Second, Pad::Zero),
            Item::Fixed(Fixed::Nanosecond),
            Item::Space(""),
        ];
        const TRAILING_WHITESPACE: [Item<'static>; 1] = [Item::Space("")];

        let mut parsed = Parsed::new();
        let s = parse_and_remainder(&mut parsed, s, HOUR_AND_MINUTE.iter())?;
        // Seconds are optional, don't fail if parsing them doesn't succeed.
        let s = parse_and_remainder(&mut parsed, s, SECOND_AND_NANOS.iter()).unwrap_or(s);
        parse(&mut parsed, s, TRAILING_WHITESPACE.iter())?;
        parsed.to_naive_time()
    }
}

/// The default value for a NaiveTime is midnight, 00:00:00 exactly.
///
/// # Example
///
/// ```rust
/// use chrono::NaiveTime;
///
/// let default_time = NaiveTime::default();
/// assert_eq!(default_time, NaiveTime::from_hms_opt(0, 0, 0).unwrap());
/// ```
impl Default for NaiveTime {
    fn default() -> Self {
        NaiveTime::from_hms_opt(0, 0, 0).unwrap()
    }
}
