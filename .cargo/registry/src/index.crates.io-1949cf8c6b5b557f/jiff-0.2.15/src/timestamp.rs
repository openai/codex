use core::time::Duration as UnsignedDuration;

use crate::{
    duration::{Duration, SDuration},
    error::{err, Error, ErrorContext},
    fmt::{
        self,
        temporal::{self, DEFAULT_DATETIME_PARSER},
    },
    shared::util::itime::ITimestamp,
    tz::{Offset, TimeZone},
    util::{
        rangeint::{self, Composite, RFrom, RInto},
        round::increment,
        t::{
            self, FractionalNanosecond, NoUnits, NoUnits128, UnixMicroseconds,
            UnixMilliseconds, UnixNanoseconds, UnixSeconds, C,
        },
    },
    zoned::Zoned,
    RoundMode, SignedDuration, Span, SpanRound, Unit,
};

/// An instant in time represented as the number of nanoseconds since the Unix
/// epoch.
///
/// A timestamp is always in the Unix timescale with a UTC offset of zero.
///
/// To obtain civil or "local" datetime units like year, month, day or hour, a
/// timestamp needs to be combined with a [`TimeZone`] to create a [`Zoned`].
/// That can be done with [`Timestamp::in_tz`] or [`Timestamp::to_zoned`].
///
/// The integer count of nanoseconds since the Unix epoch is signed, where
/// the Unix epoch is `1970-01-01 00:00:00Z`. A positive timestamp indicates
/// a point in time after the Unix epoch. A negative timestamp indicates a
/// point in time before the Unix epoch.
///
/// # Parsing and printing
///
/// The `Timestamp` type provides convenient trait implementations of
/// [`std::str::FromStr`] and [`std::fmt::Display`]:
///
/// ```
/// use jiff::Timestamp;
///
/// let ts: Timestamp = "2024-06-19 15:22:45-04".parse()?;
/// assert_eq!(ts.to_string(), "2024-06-19T19:22:45Z");
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// A `Timestamp` can also be parsed from something that _contains_ a
/// timestamp, but with perhaps other data (such as a time zone):
///
/// ```
/// use jiff::Timestamp;
///
/// let ts: Timestamp = "2024-06-19T15:22:45-04[America/New_York]".parse()?;
/// assert_eq!(ts.to_string(), "2024-06-19T19:22:45Z");
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// For more information on the specific format supported, see the
/// [`fmt::temporal`](crate::fmt::temporal) module documentation.
///
/// # Default value
///
/// For convenience, this type implements the `Default` trait. Its default
/// value corresponds to `1970-01-01T00:00:00.000000000`. That is, it is the
/// Unix epoch. One can also access this value via the `Timestamp::UNIX_EPOCH`
/// constant.
///
/// # Leap seconds
///
/// Jiff does not support leap seconds. Jiff behaves as if they don't exist.
/// The only exception is that if one parses a timestamp with a second
/// component of `60`, then it is automatically constrained to `59`:
///
/// ```
/// use jiff::Timestamp;
///
/// let ts: Timestamp = "2016-12-31 23:59:60Z".parse()?;
/// assert_eq!(ts.to_string(), "2016-12-31T23:59:59Z");
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Comparisons
///
/// The `Timestamp` type provides both `Eq` and `Ord` trait implementations
/// to facilitate easy comparisons. When a timestamp `ts1` occurs before a
/// timestamp `ts2`, then `dt1 < dt2`. For example:
///
/// ```
/// use jiff::Timestamp;
///
/// let ts1 = Timestamp::from_second(123_456_789)?;
/// let ts2 = Timestamp::from_second(123_456_790)?;
/// assert!(ts1 < ts2);
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Arithmetic
///
/// This type provides routines for adding and subtracting spans of time, as
/// well as computing the span of time between two `Timestamp` values.
///
/// For adding or subtracting spans of time, one can use any of the following
/// routines:
///
/// * [`Timestamp::checked_add`] or [`Timestamp::checked_sub`] for checked
/// arithmetic.
/// * [`Timestamp::saturating_add`] or [`Timestamp::saturating_sub`] for
/// saturating arithmetic.
///
/// Additionally, checked arithmetic is available via the `Add` and `Sub`
/// trait implementations. When the result overflows, a panic occurs.
///
/// ```
/// use jiff::{Timestamp, ToSpan};
///
/// let ts1: Timestamp = "2024-02-25T15:45Z".parse()?;
/// let ts2 = ts1 - 24.hours();
/// assert_eq!(ts2.to_string(), "2024-02-24T15:45:00Z");
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// One can compute the span of time between two timestamps using either
/// [`Timestamp::until`] or [`Timestamp::since`]. It's also possible to
/// subtract two `Timestamp` values directly via a `Sub` trait implementation:
///
/// ```
/// use jiff::{Timestamp, ToSpan};
///
/// let ts1: Timestamp = "2024-05-03 23:30:00.123Z".parse()?;
/// let ts2: Timestamp = "2024-02-25 07Z".parse()?;
/// // The default is to return spans with units no bigger than seconds.
/// assert_eq!(ts1 - ts2, 5934600.seconds().milliseconds(123).fieldwise());
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// The `until` and `since` APIs are polymorphic and allow re-balancing and
/// rounding the span returned. For example, the default largest unit is
/// seconds (as exemplified above), but we can ask for bigger units (up to
/// hours):
///
/// ```
/// use jiff::{Timestamp, ToSpan, Unit};
///
/// let ts1: Timestamp = "2024-05-03 23:30:00.123Z".parse()?;
/// let ts2: Timestamp = "2024-02-25 07Z".parse()?;
/// assert_eq!(
///     // If you want to deal in units bigger than hours, then you'll have to
///     // convert your timestamp to a [`Zoned`] first.
///     ts1.since((Unit::Hour, ts2))?,
///     1648.hours().minutes(30).milliseconds(123).fieldwise(),
/// );
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// You can also round the span returned:
///
/// ```
/// use jiff::{RoundMode, Timestamp, TimestampDifference, ToSpan, Unit};
///
/// let ts1: Timestamp = "2024-05-03 23:30:59.123Z".parse()?;
/// let ts2: Timestamp = "2024-05-02 07Z".parse()?;
/// assert_eq!(
///     ts1.since(
///         TimestampDifference::new(ts2)
///             .smallest(Unit::Minute)
///             .largest(Unit::Hour),
///     )?,
///     40.hours().minutes(30).fieldwise(),
/// );
/// // `TimestampDifference` uses truncation as a rounding mode by default,
/// // but you can set the rounding mode to break ties away from zero:
/// assert_eq!(
///     ts1.since(
///         TimestampDifference::new(ts2)
///             .smallest(Unit::Minute)
///             .largest(Unit::Hour)
///             .mode(RoundMode::HalfExpand),
///     )?,
///     // Rounds up to 31 minutes.
///     40.hours().minutes(31).fieldwise(),
/// );
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Rounding timestamps
///
/// A `Timestamp` can be rounded based on a [`TimestampRound`] configuration of
/// smallest units, rounding increment and rounding mode. Here's an example
/// showing how to round to the nearest third hour:
///
/// ```
/// use jiff::{Timestamp, TimestampRound, Unit};
///
/// let ts: Timestamp = "2024-06-19 16:27:29.999999999Z".parse()?;
/// assert_eq!(
///     ts.round(TimestampRound::new().smallest(Unit::Hour).increment(3))?,
///     "2024-06-19 15Z".parse::<Timestamp>()?,
/// );
/// // Or alternatively, make use of the `From<(Unit, i64)> for TimestampRound`
/// // trait implementation:
/// assert_eq!(
///     ts.round((Unit::Hour, 3))?.to_string(),
///     "2024-06-19T15:00:00Z",
/// );
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// See [`Timestamp::round`] for more details.
///
/// # An instant in time
///
/// Unlike a [`civil::DateTime`](crate::civil::DateTime), a `Timestamp`
/// _always_ corresponds, unambiguously, to a precise instant in time (to
/// nanosecond precision). This means that attaching a time zone to a timestamp
/// is always unambiguous because there's never any question as to which
/// instant it refers to. This is true even for gaps in civil time.
///
/// For example, in `America/New_York`, clocks were moved ahead one hour
/// at clock time `2024-03-10 02:00:00`. That is, the 2 o'clock hour never
/// appeared on clocks in the `America/New_York` region. Since parsing a
/// timestamp always requires an offset, the time it refers to is unambiguous.
/// We can see this by writing a clock time, `02:30`, that never existed but
/// with two different offsets:
///
/// ```
/// use jiff::Timestamp;
///
/// // All we're doing here is attaching an offset to a civil datetime.
/// // There is no time zone information here, and thus there is no
/// // accounting for ambiguity due to daylight saving time transitions.
/// let before_hour_jump: Timestamp = "2024-03-10 02:30-04".parse()?;
/// let after_hour_jump: Timestamp = "2024-03-10 02:30-05".parse()?;
/// // This shows the instant in time in UTC.
/// assert_eq!(before_hour_jump.to_string(), "2024-03-10T06:30:00Z");
/// assert_eq!(after_hour_jump.to_string(), "2024-03-10T07:30:00Z");
///
/// // Now let's attach each instant to an `America/New_York` time zone.
/// let zdt_before = before_hour_jump.in_tz("America/New_York")?;
/// let zdt_after = after_hour_jump.in_tz("America/New_York")?;
/// // And now we can see that even though the original instant refers to
/// // the 2 o'clock hour, since that hour never existed on the clocks in
/// // `America/New_York`, an instant with a time zone correctly adjusts.
/// assert_eq!(
///     zdt_before.to_string(),
///     "2024-03-10T01:30:00-05:00[America/New_York]",
/// );
/// assert_eq!(
///     zdt_after.to_string(),
///     "2024-03-10T03:30:00-04:00[America/New_York]",
/// );
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// In the example above, there is never a step that is incorrect or has an
/// alternative answer. Every step is unambiguous because we never involve
/// any [`civil`](crate::civil) datetimes.
///
/// But note that if the datetime string you're parsing from lacks an offset,
/// then it *could* be ambiguous even if a time zone is specified. In this
/// case, parsing will always fail:
///
/// ```
/// use jiff::Timestamp;
///
/// let result = "2024-06-30 08:30[America/New_York]".parse::<Timestamp>();
/// assert_eq!(
///     result.unwrap_err().to_string(),
///     "failed to find offset component in \
///      \"2024-06-30 08:30[America/New_York]\", \
///      which is required for parsing a timestamp",
/// );
/// ```
///
/// # Converting a civil datetime to a timestamp
///
/// Sometimes you want to convert the "time on the clock" to a precise instant
/// in time. One way to do this was demonstrated in the previous section, but
/// it only works if you know your current time zone offset:
///
/// ```
/// use jiff::Timestamp;
///
/// let ts: Timestamp = "2024-06-30 08:36-04".parse()?;
/// assert_eq!(ts.to_string(), "2024-06-30T12:36:00Z");
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// The above happened to be the precise instant in time I wrote the example.
/// Since I happened to know the offset, this worked okay. But what if I
/// didn't? We could instead construct a civil datetime and attach a time zone
/// to it. This will create a [`Zoned`] value, from which we can access the
/// timestamp:
///
/// ```
/// use jiff::civil::date;
///
/// let clock = date(2024, 6, 30).at(8, 36, 0, 0).in_tz("America/New_York")?;
/// assert_eq!(clock.timestamp().to_string(), "2024-06-30T12:36:00Z");
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Copy)]
pub struct Timestamp {
    second: UnixSeconds,
    nanosecond: FractionalNanosecond,
}

impl Timestamp {
    /// The minimum representable timestamp.
    ///
    /// The minimum is chosen such that it can be combined with
    /// any legal [`Offset`](crate::tz::Offset) and turned into a
    /// [`civil::DateTime`](crate::civil::DateTime).
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::{civil::date, tz::Offset, Timestamp};
    ///
    /// let dt = Offset::MIN.to_datetime(Timestamp::MIN);
    /// assert_eq!(dt, date(-9999, 1, 1).at(0, 0, 0, 0));
    /// ```
    pub const MIN: Timestamp = Timestamp {
        second: UnixSeconds::MIN_SELF,
        nanosecond: FractionalNanosecond::N::<0>(),
    };

    /// The maximum representable timestamp.
    ///
    /// The maximum is chosen such that it can be combined with
    /// any legal [`Offset`](crate::tz::Offset) and turned into a
    /// [`civil::DateTime`](crate::civil::DateTime).
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::{civil::date, tz::Offset, Timestamp};
    ///
    /// let dt = Offset::MAX.to_datetime(Timestamp::MAX);
    /// assert_eq!(dt, date(9999, 12, 31).at(23, 59, 59, 999_999_999));
    /// ```
    pub const MAX: Timestamp = Timestamp {
        second: UnixSeconds::MAX_SELF,
        nanosecond: FractionalNanosecond::MAX_SELF,
    };

    /// The Unix epoch represented as a timestamp.
    ///
    /// The Unix epoch corresponds to the instant at `1970-01-01T00:00:00Z`.
    /// As a timestamp, it corresponds to `0` nanoseconds.
    ///
    /// A timestamp is positive if and only if it is greater than the Unix
    /// epoch. A timestamp is negative if and only if it is less than the Unix
    /// epoch.
    pub const UNIX_EPOCH: Timestamp = Timestamp {
        second: UnixSeconds::N::<0>(),
        nanosecond: FractionalNanosecond::N::<0>(),
    };

    /// Returns the current system time as a timestamp.
    ///
    /// # Panics
    ///
    /// This panics if the system clock is set to a time value outside of the
    /// range `-009999-01-01T00:00:00Z..=9999-12-31T11:59:59.999999999Z`. The
    /// justification here is that it is reasonable to expect the system clock
    /// to be set to a somewhat sane, if imprecise, value.
    ///
    /// If you want to get the current Unix time fallibly, use
    /// [`Timestamp::try_from`] with a `std::time::SystemTime` as input.
    ///
    /// This may also panic when `SystemTime::now()` itself panics. The most
    /// common context in which this happens is on the `wasm32-unknown-unknown`
    /// target. If you're using that target in the context of the web (for
    /// example, via `wasm-pack`), and you're an application, then you should
    /// enable Jiff's `js` feature. This will automatically instruct Jiff in
    /// this very specific circumstance to execute JavaScript code to determine
    /// the current time from the web browser.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::Timestamp;
    ///
    /// assert!(Timestamp::now() > Timestamp::UNIX_EPOCH);
    /// ```
    #[cfg(feature = "std")]
    pub fn now() -> Timestamp {
        Timestamp::try_from(crate::now::system_time())
            .expect("system time is valid")
    }

    /// Creates a new instant in time represented as a timestamp.
    ///
    /// While a timestamp is logically a count of nanoseconds since the Unix
    /// epoch, this constructor provides a convenience way of constructing
    /// the timestamp from two components: seconds and fractional seconds
    /// expressed as nanoseconds.
    ///
    /// The signs of `second` and `nanosecond` need not be the same.
    ///
    /// # Errors
    ///
    /// This returns an error if the given components would correspond to
    /// an instant outside the supported range. Also, `nanosecond` is limited
    /// to the range `-999,999,999..=999,999,999`.
    ///
    /// # Example
    ///
    /// This example shows the instant in time 123,456,789 seconds after the
    /// Unix epoch:
    ///
    /// ```
    /// use jiff::Timestamp;
    ///
    /// assert_eq!(
    ///     Timestamp::new(123_456_789, 0)?.to_string(),
    ///     "1973-11-29T21:33:09Z",
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: normalized sign
    ///
    /// This example shows how `second` and `nanosecond` are resolved when
    /// their signs differ.
    ///
    /// ```
    /// use jiff::Timestamp;
    ///
    /// let ts = Timestamp::new(2, -999_999_999)?;
    /// assert_eq!(ts.as_second(), 1);
    /// assert_eq!(ts.subsec_nanosecond(), 1);
    ///
    /// let ts = Timestamp::new(-2, 999_999_999)?;
    /// assert_eq!(ts.as_second(), -1);
    /// assert_eq!(ts.subsec_nanosecond(), -1);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: limits
    ///
    /// The minimum timestamp has nanoseconds set to zero, while the maximum
    /// timestamp has nanoseconds set to `999,999,999`:
    ///
    /// ```
    /// use jiff::Timestamp;
    ///
    /// assert_eq!(Timestamp::MIN.subsec_nanosecond(), 0);
    /// assert_eq!(Timestamp::MAX.subsec_nanosecond(), 999_999_999);
    /// ```
    ///
    /// As a consequence, nanoseconds cannot be negative when a timestamp has
    /// minimal seconds:
    ///
    /// ```
    /// use jiff::Timestamp;
    ///
    /// assert!(Timestamp::new(Timestamp::MIN.as_second(), -1).is_err());
    /// // But they can be positive!
    /// let one_ns_more = Timestamp::new(Timestamp::MIN.as_second(), 1)?;
    /// assert_eq!(
    ///     one_ns_more.to_string(),
    ///     "-009999-01-02T01:59:59.000000001Z",
    /// );
    /// // Or, when combined with a minimal offset:
    /// assert_eq!(
    ///     jiff::tz::Offset::MIN.to_datetime(one_ns_more).to_string(),
    ///     "-009999-01-01T00:00:00.000000001",
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn new(second: i64, nanosecond: i32) -> Result<Timestamp, Error> {
        Timestamp::new_ranged(
            UnixSeconds::try_new("second", second)?,
            FractionalNanosecond::try_new("nanosecond", nanosecond)?,
        )
    }

    /// Creates a new `Timestamp` value in a `const` context.
    ///
    /// # Panics
    ///
    /// This routine panics when [`Timestamp::new`] would return an error.
    /// That is, when the given components would correspond to
    /// an instant outside the supported range. Also, `nanosecond` is limited
    /// to the range `-999,999,999..=999,999,999`.
    ///
    /// # Example
    ///
    /// This example shows the instant in time 123,456,789 seconds after the
    /// Unix epoch:
    ///
    /// ```
    /// use jiff::Timestamp;
    ///
    /// assert_eq!(
    ///     Timestamp::constant(123_456_789, 0).to_string(),
    ///     "1973-11-29T21:33:09Z",
    /// );
    /// ```
    #[inline]
    pub const fn constant(mut second: i64, mut nanosecond: i32) -> Timestamp {
        if second == UnixSeconds::MIN_REPR && nanosecond < 0 {
            panic!("nanoseconds must be >=0 when seconds are minimal");
        }
        // We now normalize our seconds and nanoseconds such that they have
        // the same sign (or where one is zero). So for example, when given
        // `-1s 1ns`, then we should turn that into `-999,999,999ns`.
        //
        // But first, if we're already normalized, we're done!
        if second.signum() as i8 == nanosecond.signum() as i8
            || second == 0
            || nanosecond == 0
        {
            return Timestamp {
                second: UnixSeconds::new_unchecked(second),
                nanosecond: FractionalNanosecond::new_unchecked(nanosecond),
            };
        }
        if second < 0 && nanosecond > 0 {
            second += 1;
            nanosecond -= t::NANOS_PER_SECOND.value() as i32;
        } else if second > 0 && nanosecond < 0 {
            second -= 1;
            nanosecond += t::NANOS_PER_SECOND.value() as i32;
        }
        Timestamp {
            second: UnixSeconds::new_unchecked(second),
            nanosecond: FractionalNanosecond::new_unchecked(nanosecond),
        }
    }

    /// Creates a new instant in time from the number of seconds elapsed since
    /// the Unix epoch.
    ///
    /// When `second` is negative, it corresponds to an instant in time before
    /// the Unix epoch. A smaller number corresponds to an instant in time
    /// further into the past.
    ///
    /// # Errors
    ///
    /// This returns an error if the given second corresponds to a timestamp
    /// outside of the [`Timestamp::MIN`] and [`Timestamp::MAX`] boundaries.
    ///
    /// It is a semver guarantee that the only way for this to return an error
    /// is if the given value is out of range. That is, when it is less than
    /// `Timestamp::MIN` or greater than `Timestamp::MAX`.
    ///
    /// # Example
    ///
    /// This example shows the instants in time 1 second immediately after and
    /// before the Unix epoch:
    ///
    /// ```
    /// use jiff::Timestamp;
    ///
    /// assert_eq!(
    ///     Timestamp::from_second(1)?.to_string(),
    ///     "1970-01-01T00:00:01Z",
    /// );
    /// assert_eq!(
    ///     Timestamp::from_second(-1)?.to_string(),
    ///     "1969-12-31T23:59:59Z",
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: saturating construction
    ///
    /// If you need a way to build a `Timestamp` value that saturates to
    /// the minimum and maximum values supported by Jiff, then this is
    /// guaranteed to work:
    ///
    /// ```
    /// use jiff::Timestamp;
    ///
    /// fn from_second_saturating(seconds: i64) -> Timestamp {
    ///     Timestamp::from_second(seconds).unwrap_or_else(|_| {
    ///         if seconds < 0 {
    ///             Timestamp::MIN
    ///         } else {
    ///             Timestamp::MAX
    ///         }
    ///     })
    /// }
    ///
    /// assert_eq!(from_second_saturating(0), Timestamp::UNIX_EPOCH);
    /// assert_eq!(
    ///     from_second_saturating(-999999999999999999),
    ///     Timestamp::MIN
    /// );
    /// assert_eq!(
    ///     from_second_saturating(999999999999999999),
    ///     Timestamp::MAX
    /// );
    /// ```
    #[inline]
    pub fn from_second(second: i64) -> Result<Timestamp, Error> {
        Timestamp::new(second, 0)
    }

    /// Creates a new instant in time from the number of milliseconds elapsed
    /// since the Unix epoch.
    ///
    /// When `millisecond` is negative, it corresponds to an instant in time
    /// before the Unix epoch. A smaller number corresponds to an instant in
    /// time further into the past.
    ///
    /// # Errors
    ///
    /// This returns an error if the given millisecond corresponds to a
    /// timestamp outside of the [`Timestamp::MIN`] and [`Timestamp::MAX`]
    /// boundaries.
    ///
    /// It is a semver guarantee that the only way for this to return an error
    /// is if the given value is out of range. That is, when it is less than
    /// `Timestamp::MIN` or greater than `Timestamp::MAX`.
    ///
    /// # Example
    ///
    /// This example shows the instants in time 1 millisecond immediately after
    /// and before the Unix epoch:
    ///
    /// ```
    /// use jiff::Timestamp;
    ///
    /// assert_eq!(
    ///     Timestamp::from_millisecond(1)?.to_string(),
    ///     "1970-01-01T00:00:00.001Z",
    /// );
    /// assert_eq!(
    ///     Timestamp::from_millisecond(-1)?.to_string(),
    ///     "1969-12-31T23:59:59.999Z",
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: saturating construction
    ///
    /// If you need a way to build a `Timestamp` value that saturates to
    /// the minimum and maximum values supported by Jiff, then this is
    /// guaranteed to work:
    ///
    /// ```
    /// use jiff::Timestamp;
    ///
    /// fn from_millisecond_saturating(millis: i64) -> Timestamp {
    ///     Timestamp::from_millisecond(millis).unwrap_or_else(|_| {
    ///         if millis < 0 {
    ///             Timestamp::MIN
    ///         } else {
    ///             Timestamp::MAX
    ///         }
    ///     })
    /// }
    ///
    /// assert_eq!(from_millisecond_saturating(0), Timestamp::UNIX_EPOCH);
    /// assert_eq!(
    ///     from_millisecond_saturating(-999999999999999999),
    ///     Timestamp::MIN
    /// );
    /// assert_eq!(
    ///     from_millisecond_saturating(999999999999999999),
    ///     Timestamp::MAX
    /// );
    /// ```
    #[inline]
    pub fn from_millisecond(millisecond: i64) -> Result<Timestamp, Error> {
        let millisecond = UnixMilliseconds::try_new128(
            "millisecond timestamp",
            millisecond,
        )?;
        Ok(Timestamp::from_millisecond_ranged(millisecond))
    }

    /// Creates a new instant in time from the number of microseconds elapsed
    /// since the Unix epoch.
    ///
    /// When `microsecond` is negative, it corresponds to an instant in time
    /// before the Unix epoch. A smaller number corresponds to an instant in
    /// time further into the past.
    ///
    /// # Errors
    ///
    /// This returns an error if the given microsecond corresponds to a
    /// timestamp outside of the [`Timestamp::MIN`] and [`Timestamp::MAX`]
    /// boundaries.
    ///
    /// It is a semver guarantee that the only way for this to return an error
    /// is if the given value is out of range. That is, when it is less than
    /// `Timestamp::MIN` or greater than `Timestamp::MAX`.
    ///
    /// # Example
    ///
    /// This example shows the instants in time 1 microsecond immediately after
    /// and before the Unix epoch:
    ///
    /// ```
    /// use jiff::Timestamp;
    ///
    /// assert_eq!(
    ///     Timestamp::from_microsecond(1)?.to_string(),
    ///     "1970-01-01T00:00:00.000001Z",
    /// );
    /// assert_eq!(
    ///     Timestamp::from_microsecond(-1)?.to_string(),
    ///     "1969-12-31T23:59:59.999999Z",
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: saturating construction
    ///
    /// If you need a way to build a `Timestamp` value that saturates to
    /// the minimum and maximum values supported by Jiff, then this is
    /// guaranteed to work:
    ///
    /// ```
    /// use jiff::Timestamp;
    ///
    /// fn from_microsecond_saturating(micros: i64) -> Timestamp {
    ///     Timestamp::from_microsecond(micros).unwrap_or_else(|_| {
    ///         if micros < 0 {
    ///             Timestamp::MIN
    ///         } else {
    ///             Timestamp::MAX
    ///         }
    ///     })
    /// }
    ///
    /// assert_eq!(from_microsecond_saturating(0), Timestamp::UNIX_EPOCH);
    /// assert_eq!(
    ///     from_microsecond_saturating(-999999999999999999),
    ///     Timestamp::MIN
    /// );
    /// assert_eq!(
    ///     from_microsecond_saturating(999999999999999999),
    ///     Timestamp::MAX
    /// );
    /// ```
    #[inline]
    pub fn from_microsecond(microsecond: i64) -> Result<Timestamp, Error> {
        let microsecond = UnixMicroseconds::try_new128(
            "microsecond timestamp",
            microsecond,
        )?;
        Ok(Timestamp::from_microsecond_ranged(microsecond))
    }

    /// Creates a new instant in time from the number of nanoseconds elapsed
    /// since the Unix epoch.
    ///
    /// When `nanosecond` is negative, it corresponds to an instant in time
    /// before the Unix epoch. A smaller number corresponds to an instant in
    /// time further into the past.
    ///
    /// # Errors
    ///
    /// This returns an error if the given nanosecond corresponds to a
    /// timestamp outside of the [`Timestamp::MIN`] and [`Timestamp::MAX`]
    /// boundaries.
    ///
    /// It is a semver guarantee that the only way for this to return an error
    /// is if the given value is out of range. That is, when it is less than
    /// `Timestamp::MIN` or greater than `Timestamp::MAX`.
    ///
    /// # Example
    ///
    /// This example shows the instants in time 1 nanosecond immediately after
    /// and before the Unix epoch:
    ///
    /// ```
    /// use jiff::Timestamp;
    ///
    /// assert_eq!(
    ///     Timestamp::from_nanosecond(1)?.to_string(),
    ///     "1970-01-01T00:00:00.000000001Z",
    /// );
    /// assert_eq!(
    ///     Timestamp::from_nanosecond(-1)?.to_string(),
    ///     "1969-12-31T23:59:59.999999999Z",
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: saturating construction
    ///
    /// If you need a way to build a `Timestamp` value that saturates to
    /// the minimum and maximum values supported by Jiff, then this is
    /// guaranteed to work:
    ///
    /// ```
    /// use jiff::Timestamp;
    ///
    /// fn from_nanosecond_saturating(nanos: i128) -> Timestamp {
    ///     Timestamp::from_nanosecond(nanos).unwrap_or_else(|_| {
    ///         if nanos < 0 {
    ///             Timestamp::MIN
    ///         } else {
    ///             Timestamp::MAX
    ///         }
    ///     })
    /// }
    ///
    /// assert_eq!(from_nanosecond_saturating(0), Timestamp::UNIX_EPOCH);
    /// assert_eq!(
    ///     from_nanosecond_saturating(-9999999999999999999999999999999999),
    ///     Timestamp::MIN
    /// );
    /// assert_eq!(
    ///     from_nanosecond_saturating(9999999999999999999999999999999999),
    ///     Timestamp::MAX
    /// );
    /// ```
    #[inline]
    pub fn from_nanosecond(nanosecond: i128) -> Result<Timestamp, Error> {
        let nanosecond =
            UnixNanoseconds::try_new128("nanosecond timestamp", nanosecond)?;
        Ok(Timestamp::from_nanosecond_ranged(nanosecond))
    }

    /// Creates a new timestamp from a `Duration` with the given sign since the
    /// Unix epoch.
    ///
    /// Positive durations result in a timestamp after the Unix epoch. Negative
    /// durations result in a timestamp before the Unix epoch.
    ///
    /// # Errors
    ///
    /// This returns an error if the given duration corresponds to a timestamp
    /// outside of the [`Timestamp::MIN`] and [`Timestamp::MAX`] boundaries.
    ///
    /// It is a semver guarantee that the only way for this to return an error
    /// is if the given value is out of range. That is, when it is less than
    /// `Timestamp::MIN` or greater than `Timestamp::MAX`.
    ///
    /// # Example
    ///
    /// How one might construct a `Timestamp` from a `SystemTime`:
    ///
    /// ```
    /// use std::time::SystemTime;
    /// use jiff::{SignedDuration, Timestamp};
    ///
    /// let unix_epoch = SystemTime::UNIX_EPOCH;
    /// let now = SystemTime::now();
    /// let duration = SignedDuration::system_until(unix_epoch, now)?;
    /// let ts = Timestamp::from_duration(duration)?;
    /// assert!(ts > Timestamp::UNIX_EPOCH);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// Of course, one should just use [`Timestamp::try_from`] for this
    /// instead. Indeed, the above example is copied almost exactly from the
    /// `TryFrom` implementation.
    ///
    /// # Example: out of bounds
    ///
    /// This example shows how some of the boundary conditions are dealt with.
    ///
    /// ```
    /// use jiff::{SignedDuration, Timestamp};
    ///
    /// // OK, we get the minimum timestamp supported by Jiff:
    /// let duration = SignedDuration::new(-377705023201, 0);
    /// let ts = Timestamp::from_duration(duration)?;
    /// assert_eq!(ts, Timestamp::MIN);
    ///
    /// // We use the minimum number of seconds, but even subtracting
    /// // one more nanosecond after it will result in an error.
    /// let duration = SignedDuration::new(-377705023201, -1);
    /// assert_eq!(
    ///     Timestamp::from_duration(duration).unwrap_err().to_string(),
    ///     "parameter 'seconds and nanoseconds' with value -1 is not \
    ///      in the required range of 0..=1000000000",
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: saturating construction
    ///
    /// If you need a way to build a `Timestamp` value that saturates to
    /// the minimum and maximum values supported by Jiff, then this is
    /// guaranteed to work:
    ///
    /// ```
    /// use jiff::{SignedDuration, Timestamp};
    ///
    /// fn from_duration_saturating(dur: SignedDuration) -> Timestamp {
    ///     Timestamp::from_duration(dur).unwrap_or_else(|_| {
    ///         if dur.is_negative() {
    ///             Timestamp::MIN
    ///         } else {
    ///             Timestamp::MAX
    ///         }
    ///     })
    /// }
    ///
    /// assert_eq!(
    ///     from_duration_saturating(SignedDuration::ZERO),
    ///     Timestamp::UNIX_EPOCH,
    /// );
    /// assert_eq!(
    ///     from_duration_saturating(SignedDuration::from_secs(-999999999999)),
    ///     Timestamp::MIN
    /// );
    /// assert_eq!(
    ///     from_duration_saturating(SignedDuration::from_secs(999999999999)),
    ///     Timestamp::MAX
    /// );
    /// ```
    #[inline]
    pub fn from_duration(
        duration: SignedDuration,
    ) -> Result<Timestamp, Error> {
        // As an optimization, we don't need to go through `Timestamp::new`
        // (or `Timestamp::new_ranged`) here. That's because a `SignedDuration`
        // already guarantees that its seconds and nanoseconds are "coherent."
        // That is, we know we can't have a negative second with a positive
        // nanosecond (or vice versa).
        let second = UnixSeconds::try_new("second", duration.as_secs())?;
        let nanosecond = FractionalNanosecond::try_new(
            "nanosecond",
            duration.subsec_nanos(),
        )?;
        // ... but we do have to check that the *combination* of seconds and
        // nanoseconds aren't out of bounds, which is possible even when both
        // are, on their own, legal values.
        if second == UnixSeconds::MIN_SELF && nanosecond < C(0) {
            return Err(Error::range(
                "seconds and nanoseconds",
                nanosecond,
                0,
                1_000_000_000,
            ));
        }
        Ok(Timestamp { second, nanosecond })
    }

    /// Returns this timestamp as a number of seconds since the Unix epoch.
    ///
    /// This only returns the number of whole seconds. That is, if there are
    /// any fractional seconds in this timestamp, then they are truncated.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::Timestamp;
    ///
    /// let ts = Timestamp::new(5, 123_456_789)?;
    /// assert_eq!(ts.as_second(), 5);
    /// let ts = Timestamp::new(5, 999_999_999)?;
    /// assert_eq!(ts.as_second(), 5);
    ///
    /// let ts = Timestamp::new(-5, -123_456_789)?;
    /// assert_eq!(ts.as_second(), -5);
    /// let ts = Timestamp::new(-5, -999_999_999)?;
    /// assert_eq!(ts.as_second(), -5);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn as_second(self) -> i64 {
        self.as_second_ranged().get()
    }

    /// Returns this timestamp as a number of milliseconds since the Unix
    /// epoch.
    ///
    /// This only returns the number of whole milliseconds. That is, if there
    /// are any fractional milliseconds in this timestamp, then they are
    /// truncated.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::Timestamp;
    ///
    /// let ts = Timestamp::new(5, 123_456_789)?;
    /// assert_eq!(ts.as_millisecond(), 5_123);
    /// let ts = Timestamp::new(5, 999_999_999)?;
    /// assert_eq!(ts.as_millisecond(), 5_999);
    ///
    /// let ts = Timestamp::new(-5, -123_456_789)?;
    /// assert_eq!(ts.as_millisecond(), -5_123);
    /// let ts = Timestamp::new(-5, -999_999_999)?;
    /// assert_eq!(ts.as_millisecond(), -5_999);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn as_millisecond(self) -> i64 {
        self.as_millisecond_ranged().get()
    }

    /// Returns this timestamp as a number of microseconds since the Unix
    /// epoch.
    ///
    /// This only returns the number of whole microseconds. That is, if there
    /// are any fractional microseconds in this timestamp, then they are
    /// truncated.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::Timestamp;
    ///
    /// let ts = Timestamp::new(5, 123_456_789)?;
    /// assert_eq!(ts.as_microsecond(), 5_123_456);
    /// let ts = Timestamp::new(5, 999_999_999)?;
    /// assert_eq!(ts.as_microsecond(), 5_999_999);
    ///
    /// let ts = Timestamp::new(-5, -123_456_789)?;
    /// assert_eq!(ts.as_microsecond(), -5_123_456);
    /// let ts = Timestamp::new(-5, -999_999_999)?;
    /// assert_eq!(ts.as_microsecond(), -5_999_999);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn as_microsecond(self) -> i64 {
        self.as_microsecond_ranged().get()
    }

    /// Returns this timestamp as a number of nanoseconds since the Unix
    /// epoch.
    ///
    /// Since a `Timestamp` has a nanosecond precision, the nanoseconds
    /// returned here represent this timestamp losslessly. That is, the
    /// nanoseconds returned can be used with [`Timestamp::from_nanosecond`] to
    /// create an identical timestamp with no loss of precision.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::Timestamp;
    ///
    /// let ts = Timestamp::new(5, 123_456_789)?;
    /// assert_eq!(ts.as_nanosecond(), 5_123_456_789);
    /// let ts = Timestamp::new(5, 999_999_999)?;
    /// assert_eq!(ts.as_nanosecond(), 5_999_999_999);
    ///
    /// let ts = Timestamp::new(-5, -123_456_789)?;
    /// assert_eq!(ts.as_nanosecond(), -5_123_456_789);
    /// let ts = Timestamp::new(-5, -999_999_999)?;
    /// assert_eq!(ts.as_nanosecond(), -5_999_999_999);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn as_nanosecond(self) -> i128 {
        self.as_nanosecond_ranged().get()
    }

    /// Returns the fractional second component of this timestamp in units
    /// of milliseconds.
    ///
    /// It is guaranteed that this will never return a value that is greater
    /// than 1 second (or less than -1 second).
    ///
    /// This only returns the number of whole milliseconds. That is, if there
    /// are any fractional milliseconds in this timestamp, then they are
    /// truncated.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::Timestamp;
    ///
    /// let ts = Timestamp::new(5, 123_456_789)?;
    /// assert_eq!(ts.subsec_millisecond(), 123);
    /// let ts = Timestamp::new(5, 999_999_999)?;
    /// assert_eq!(ts.subsec_millisecond(), 999);
    ///
    /// let ts = Timestamp::new(-5, -123_456_789)?;
    /// assert_eq!(ts.subsec_millisecond(), -123);
    /// let ts = Timestamp::new(-5, -999_999_999)?;
    /// assert_eq!(ts.subsec_millisecond(), -999);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn subsec_millisecond(self) -> i32 {
        self.subsec_millisecond_ranged().get()
    }

    /// Returns the fractional second component of this timestamp in units of
    /// microseconds.
    ///
    /// It is guaranteed that this will never return a value that is greater
    /// than 1 second (or less than -1 second).
    ///
    /// This only returns the number of whole microseconds. That is, if there
    /// are any fractional microseconds in this timestamp, then they are
    /// truncated.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::Timestamp;
    ///
    /// let ts = Timestamp::new(5, 123_456_789)?;
    /// assert_eq!(ts.subsec_microsecond(), 123_456);
    /// let ts = Timestamp::new(5, 999_999_999)?;
    /// assert_eq!(ts.subsec_microsecond(), 999_999);
    ///
    /// let ts = Timestamp::new(-5, -123_456_789)?;
    /// assert_eq!(ts.subsec_microsecond(), -123_456);
    /// let ts = Timestamp::new(-5, -999_999_999)?;
    /// assert_eq!(ts.subsec_microsecond(), -999_999);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn subsec_microsecond(self) -> i32 {
        self.subsec_microsecond_ranged().get()
    }

    /// Returns the fractional second component of this timestamp in units of
    /// nanoseconds.
    ///
    /// It is guaranteed that this will never return a value that is greater
    /// than 1 second (or less than -1 second).
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::Timestamp;
    ///
    /// let ts = Timestamp::new(5, 123_456_789)?;
    /// assert_eq!(ts.subsec_nanosecond(), 123_456_789);
    /// let ts = Timestamp::new(5, 999_999_999)?;
    /// assert_eq!(ts.subsec_nanosecond(), 999_999_999);
    ///
    /// let ts = Timestamp::new(-5, -123_456_789)?;
    /// assert_eq!(ts.subsec_nanosecond(), -123_456_789);
    /// let ts = Timestamp::new(-5, -999_999_999)?;
    /// assert_eq!(ts.subsec_nanosecond(), -999_999_999);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn subsec_nanosecond(self) -> i32 {
        self.subsec_nanosecond_ranged().get()
    }

    /// Returns this timestamp as a [`SignedDuration`] since the Unix epoch.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::{SignedDuration, Timestamp};
    ///
    /// assert_eq!(
    ///     Timestamp::UNIX_EPOCH.as_duration(),
    ///     SignedDuration::ZERO,
    /// );
    /// assert_eq!(
    ///     Timestamp::new(5, 123_456_789)?.as_duration(),
    ///     SignedDuration::new(5, 123_456_789),
    /// );
    /// assert_eq!(
    ///     Timestamp::new(-5, -123_456_789)?.as_duration(),
    ///     SignedDuration::new(-5, -123_456_789),
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn as_duration(self) -> SignedDuration {
        SignedDuration::from_timestamp(self)
    }

    /// Returns the sign of this timestamp.
    ///
    /// This can return one of three possible values:
    ///
    /// * `0` when this timestamp is precisely equivalent to
    /// [`Timestamp::UNIX_EPOCH`].
    /// * `1` when this timestamp occurs after the Unix epoch.
    /// * `-1` when this timestamp occurs before the Unix epoch.
    ///
    /// The sign returned is guaranteed to match the sign of all "getter"
    /// methods on `Timestamp`. For example, [`Timestamp::as_second`] and
    /// [`Timestamp::subsec_nanosecond`]. This is true even if the signs
    /// of the `second` and `nanosecond` components were mixed when given to
    /// the [`Timestamp::new`] constructor.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::Timestamp;
    ///
    /// let ts = Timestamp::new(5, -999_999_999)?;
    /// assert_eq!(ts.signum(), 1);
    /// // The mixed signs were normalized away!
    /// assert_eq!(ts.as_second(), 4);
    /// assert_eq!(ts.subsec_nanosecond(), 1);
    ///
    /// // The same applies for negative timestamps.
    /// let ts = Timestamp::new(-5, 999_999_999)?;
    /// assert_eq!(ts.signum(), -1);
    /// assert_eq!(ts.as_second(), -4);
    /// assert_eq!(ts.subsec_nanosecond(), -1);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn signum(self) -> i8 {
        if self.is_zero() {
            0
        } else if self.as_second() > 0 || self.subsec_nanosecond() > 0 {
            1
        } else {
            -1
        }
    }

    /// Returns true if and only if this timestamp corresponds to the instant
    /// in time known as the Unix epoch.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::Timestamp;
    ///
    /// assert!(Timestamp::UNIX_EPOCH.is_zero());
    /// ```
    #[inline]
    pub fn is_zero(self) -> bool {
        self.as_second() == 0 && self.subsec_nanosecond() == 0
    }

    /// Creates a [`Zoned`] value by attaching a time zone for the given name
    /// to this instant in time.
    ///
    /// The name given is resolved to a [`TimeZone`] by using the default
    /// [`TimeZoneDatabase`](crate::tz::TimeZoneDatabase) created by
    /// [`tz::db`](crate::tz::db). Indeed, this is a convenience function
    /// for [`Timestamp::to_zoned`] where the time zone database lookup
    /// is done automatically.
    ///
    /// Assuming the time zone name could be resolved to a [`TimeZone`], this
    /// routine is otherwise infallible and never results in any ambiguity
    /// since both a [`Timestamp`] and a [`Zoned`] correspond to precise
    /// instant in time. This is unlike
    /// [`civil::DateTime::to_zoned`](crate::civil::DateTime::to_zoned),
    /// where a civil datetime might correspond to more than one instant in
    /// time (i.e., a fold, typically DST ending) or no instants in time (i.e.,
    /// a gap, typically DST starting).
    ///
    /// # Errors
    ///
    /// This returns an error when the given time zone name could not be found
    /// in the default time zone database.
    ///
    /// # Example
    ///
    /// This is a simple example of converting the instant that is `123,456,789`
    /// seconds after the Unix epoch to an instant that is aware of its time
    /// zone:
    ///
    /// ```
    /// use jiff::Timestamp;
    ///
    /// let ts = Timestamp::new(123_456_789, 0).unwrap();
    /// let zdt = ts.in_tz("America/New_York")?;
    /// assert_eq!(zdt.to_string(), "1973-11-29T16:33:09-05:00[America/New_York]");
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// This can be used to answer questions like, "What time was it at the
    /// Unix epoch in Tasmania?"
    ///
    /// ```
    /// use jiff::Timestamp;
    ///
    /// // Time zone database lookups are case insensitive!
    /// let zdt = Timestamp::UNIX_EPOCH.in_tz("australia/tasmania")?;
    /// assert_eq!(zdt.to_string(), "1970-01-01T11:00:00+11:00[Australia/Tasmania]");
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: errors
    ///
    /// This routine can return an error when the time zone is unrecognized:
    ///
    /// ```
    /// use jiff::Timestamp;
    ///
    /// assert!(Timestamp::UNIX_EPOCH.in_tz("does not exist").is_err());
    /// ```
    #[inline]
    pub fn in_tz(self, time_zone_name: &str) -> Result<Zoned, Error> {
        let tz = crate::tz::db().get(time_zone_name)?;
        Ok(self.to_zoned(tz))
    }

    /// Creates a [`Zoned`] value by attaching the given time zone to this
    /// instant in time.
    ///
    /// This is infallible and never results in any ambiguity since both a
    /// [`Timestamp`] and a [`Zoned`] correspond to precise instant in time.
    /// This is unlike
    /// [`civil::DateTime::to_zoned`](crate::civil::DateTime::to_zoned),
    /// where a civil datetime might correspond to more than one instant in
    /// time (i.e., a fold, typically DST ending) or no instants in time (i.e.,
    /// a gap, typically DST starting).
    ///
    /// In the common case of a time zone being represented as a name string,
    /// like `Australia/Tasmania`, consider using [`Timestamp::in_tz`]
    /// instead.
    ///
    /// # Example
    ///
    /// This example shows how to create a zoned value with a fixed time zone
    /// offset:
    ///
    /// ```
    /// use jiff::{tz::{self, TimeZone}, Timestamp};
    ///
    /// let ts = Timestamp::new(123_456_789, 0).unwrap();
    /// let tz = TimeZone::fixed(tz::offset(-4));
    /// let zdt = ts.to_zoned(tz);
    /// // A time zone annotation is still included in the printable version
    /// // of the Zoned value, but it is fixed to a particular offset.
    /// assert_eq!(zdt.to_string(), "1973-11-29T17:33:09-04:00[-04:00]");
    /// ```
    ///
    /// # Example: POSIX time zone strings
    ///
    /// This example shows how to create a time zone from a POSIX time zone
    /// string that describes the transition to and from daylight saving
    /// time for `America/St_Johns`. In particular, this rule uses non-zero
    /// minutes, which is atypical.
    ///
    /// ```
    /// use jiff::{tz::TimeZone, Timestamp};
    ///
    /// let ts = Timestamp::new(123_456_789, 0)?;
    /// let tz = TimeZone::posix("NST3:30NDT,M3.2.0,M11.1.0")?;
    /// let zdt = ts.to_zoned(tz);
    /// // There isn't any agreed upon mechanism for transmitting a POSIX time
    /// // zone string within an RFC 9557 TZ annotation, so Jiff just emits the
    /// // offset. In practice, POSIX TZ strings are rarely user facing anyway.
    /// // (They are still in widespread use as an implementation detail of the
    /// // IANA Time Zone Database however.)
    /// assert_eq!(zdt.to_string(), "1973-11-29T18:03:09-03:30[-03:30]");
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn to_zoned(self, tz: TimeZone) -> Zoned {
        Zoned::new(self, tz)
    }

    /// Add the given span of time to this timestamp.
    ///
    /// This operation accepts three different duration types: [`Span`],
    /// [`SignedDuration`] or [`std::time::Duration`]. This is achieved via
    /// `From` trait implementations for the [`TimestampArithmetic`] type.
    ///
    /// # Properties
    ///
    /// Given a timestamp `ts1` and a span `s`, and assuming `ts2 = ts1 + s`
    /// exists, it follows then that `ts1 = ts2 - s` for all values of `ts1`
    /// and `s` that sum to a valid `ts2`.
    ///
    /// In short, subtracting the given span from the sum returned by this
    /// function is guaranteed to result in precisely the original timestamp.
    ///
    /// # Errors
    ///
    /// If the sum would overflow the minimum or maximum timestamp values, then
    /// an error is returned.
    ///
    /// This also returns an error if the given duration is a `Span` with any
    /// non-zero units greater than hours. If you want to use bigger units,
    /// convert this timestamp to a `Zoned` and use [`Zoned::checked_add`].
    /// This error occurs because a `Timestamp` has no time zone attached to
    /// it, and thus cannot unambiguously resolve the length of a single day.
    ///
    /// # Example
    ///
    /// This shows how to add `5` hours to the Unix epoch:
    ///
    /// ```
    /// use jiff::{Timestamp, ToSpan};
    ///
    /// let ts = Timestamp::UNIX_EPOCH.checked_add(5.hours())?;
    /// assert_eq!(ts.to_string(), "1970-01-01T05:00:00Z");
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: negative spans are supported
    ///
    /// This shows how to add `-5` hours to the Unix epoch. This is the same
    /// as subtracting `5` hours from the Unix epoch.
    ///
    /// ```
    /// use jiff::{Timestamp, ToSpan};
    ///
    /// let ts = Timestamp::UNIX_EPOCH.checked_add(-5.hours())?;
    /// assert_eq!(ts.to_string(), "1969-12-31T19:00:00Z");
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: available via addition operator
    ///
    /// This routine can be used via the `+` operator. Note though that if it
    /// fails, it will result in a panic.
    ///
    /// ```
    /// use jiff::{Timestamp, ToSpan};
    ///
    /// let ts1 = Timestamp::new(2_999_999_999, 0)?;
    /// assert_eq!(ts1.to_string(), "2065-01-24T05:19:59Z");
    ///
    /// let ts2 = ts1 + 1.hour().minutes(30).nanoseconds(123);
    /// assert_eq!(ts2.to_string(), "2065-01-24T06:49:59.000000123Z");
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: error on overflow
    ///
    /// ```
    /// use jiff::{Timestamp, ToSpan};
    ///
    /// let ts = Timestamp::MAX;
    /// assert_eq!(ts.to_string(), "9999-12-30T22:00:00.999999999Z");
    /// assert!(ts.checked_add(1.nanosecond()).is_err());
    ///
    /// let ts = Timestamp::MIN;
    /// assert_eq!(ts.to_string(), "-009999-01-02T01:59:59Z");
    /// assert!(ts.checked_add(-1.nanosecond()).is_err());
    /// ```
    ///
    /// # Example: adding absolute durations
    ///
    /// This shows how to add signed and unsigned absolute durations to a
    /// `Timestamp`.
    ///
    /// ```
    /// use std::time::Duration;
    ///
    /// use jiff::{SignedDuration, Timestamp};
    ///
    /// let ts1 = Timestamp::new(2_999_999_999, 0)?;
    /// assert_eq!(ts1.to_string(), "2065-01-24T05:19:59Z");
    ///
    /// let dur = SignedDuration::new(60 * 60 + 30 * 60, 123);
    /// assert_eq!(
    ///     ts1.checked_add(dur)?.to_string(),
    ///     "2065-01-24T06:49:59.000000123Z",
    /// );
    ///
    /// let dur = Duration::new(60 * 60 + 30 * 60, 123);
    /// assert_eq!(
    ///     ts1.checked_add(dur)?.to_string(),
    ///     "2065-01-24T06:49:59.000000123Z",
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn checked_add<A: Into<TimestampArithmetic>>(
        self,
        duration: A,
    ) -> Result<Timestamp, Error> {
        let duration: TimestampArithmetic = duration.into();
        duration.checked_add(self)
    }

    #[inline]
    fn checked_add_span(self, span: Span) -> Result<Timestamp, Error> {
        if let Some(err) = span.smallest_non_time_non_zero_unit_error() {
            return Err(err);
        }
        if span.is_zero() {
            return Ok(self);
        }
        // The common case is probably a span without fractional seconds, so
        // we specialize for that since it requires a fair bit less math.
        //
        // Note that this only works when *both* the span and timestamp lack
        // fractional seconds.
        if self.subsec_nanosecond_ranged() == C(0) {
            if let Some(span_seconds) = span.to_invariant_seconds() {
                let time_seconds = self.as_second_ranged();
                let sum = time_seconds
                    .try_checked_add("span", span_seconds)
                    .with_context(|| {
                    err!("adding {span} to {self} overflowed")
                })?;
                return Ok(Timestamp::from_second_ranged(sum));
            }
        }
        let time_nanos = self.as_nanosecond_ranged();
        let span_nanos = span.to_invariant_nanoseconds();
        let sum = time_nanos
            .try_checked_add("span", span_nanos)
            .with_context(|| err!("adding {span} to {self} overflowed"))?;
        Ok(Timestamp::from_nanosecond_ranged(sum))
    }

    #[inline]
    fn checked_add_duration(
        self,
        duration: SignedDuration,
    ) -> Result<Timestamp, Error> {
        let start = self.as_duration();
        let end = start.checked_add(duration).ok_or_else(|| {
            err!("overflow when adding {duration:?} to {self}")
        })?;
        Timestamp::from_duration(end)
    }

    /// This routine is identical to [`Timestamp::checked_add`] with the
    /// duration negated.
    ///
    /// # Errors
    ///
    /// This has the same error conditions as [`Timestamp::checked_add`].
    ///
    /// # Example
    ///
    /// This routine can be used via the `-` operator. Note though that if it
    /// fails, it will result in a panic.
    ///
    /// ```
    /// use jiff::{SignedDuration, Timestamp, ToSpan};
    ///
    /// let ts1 = Timestamp::new(2_999_999_999, 0)?;
    /// assert_eq!(ts1.to_string(), "2065-01-24T05:19:59Z");
    ///
    /// let ts2 = ts1 - 1.hour().minutes(30).nanoseconds(123);
    /// assert_eq!(ts2.to_string(), "2065-01-24T03:49:58.999999877Z");
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: use with [`SignedDuration`] and [`std::time::Duration`]
    ///
    /// ```
    /// use std::time::Duration;
    ///
    /// use jiff::{SignedDuration, Timestamp};
    ///
    /// let ts1 = Timestamp::new(2_999_999_999, 0)?;
    /// assert_eq!(ts1.to_string(), "2065-01-24T05:19:59Z");
    ///
    /// let dur = SignedDuration::new(60 * 60 + 30 * 60, 123);
    /// assert_eq!(
    ///     ts1.checked_sub(dur)?.to_string(),
    ///     "2065-01-24T03:49:58.999999877Z",
    /// );
    ///
    /// let dur = Duration::new(60 * 60 + 30 * 60, 123);
    /// assert_eq!(
    ///     ts1.checked_sub(dur)?.to_string(),
    ///     "2065-01-24T03:49:58.999999877Z",
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn checked_sub<A: Into<TimestampArithmetic>>(
        self,
        duration: A,
    ) -> Result<Timestamp, Error> {
        let duration: TimestampArithmetic = duration.into();
        duration.checked_neg().and_then(|ta| ta.checked_add(self))
    }

    /// This routine is identical to [`Timestamp::checked_add`], except the
    /// result saturates on overflow. That is, instead of overflow, either
    /// [`Timestamp::MIN`] or [`Timestamp::MAX`] is returned.
    ///
    /// # Errors
    ///
    /// This returns an error if the given `Span` contains any non-zero units
    /// greater than hours.
    ///
    /// # Example
    ///
    /// This example shows that arithmetic saturates on overflow.
    ///
    /// ```
    /// use jiff::{SignedDuration, Timestamp, ToSpan};
    ///
    /// assert_eq!(
    ///     Timestamp::MAX,
    ///     Timestamp::MAX.saturating_add(1.nanosecond())?,
    /// );
    /// assert_eq!(
    ///     Timestamp::MIN,
    ///     Timestamp::MIN.saturating_add(-1.nanosecond())?,
    /// );
    /// assert_eq!(
    ///     Timestamp::MAX,
    ///     Timestamp::UNIX_EPOCH.saturating_add(SignedDuration::MAX)?,
    /// );
    /// assert_eq!(
    ///     Timestamp::MIN,
    ///     Timestamp::UNIX_EPOCH.saturating_add(SignedDuration::MIN)?,
    /// );
    /// assert_eq!(
    ///     Timestamp::MAX,
    ///     Timestamp::UNIX_EPOCH.saturating_add(std::time::Duration::MAX)?,
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn saturating_add<A: Into<TimestampArithmetic>>(
        self,
        duration: A,
    ) -> Result<Timestamp, Error> {
        let duration: TimestampArithmetic = duration.into();
        duration.saturating_add(self).context(
            "saturating `Timestamp` arithmetic requires only time units",
        )
    }

    /// This routine is identical to [`Timestamp::saturating_add`] with the
    /// span parameter negated.
    ///
    /// # Errors
    ///
    /// This returns an error if the given `Span` contains any non-zero units
    /// greater than hours.
    ///
    /// # Example
    ///
    /// This example shows that arithmetic saturates on overflow.
    ///
    /// ```
    /// use jiff::{SignedDuration, Timestamp, ToSpan};
    ///
    /// assert_eq!(
    ///     Timestamp::MIN,
    ///     Timestamp::MIN.saturating_sub(1.nanosecond())?,
    /// );
    /// assert_eq!(
    ///     Timestamp::MAX,
    ///     Timestamp::MAX.saturating_sub(-1.nanosecond())?,
    /// );
    /// assert_eq!(
    ///     Timestamp::MIN,
    ///     Timestamp::UNIX_EPOCH.saturating_sub(SignedDuration::MAX)?,
    /// );
    /// assert_eq!(
    ///     Timestamp::MAX,
    ///     Timestamp::UNIX_EPOCH.saturating_sub(SignedDuration::MIN)?,
    /// );
    /// assert_eq!(
    ///     Timestamp::MIN,
    ///     Timestamp::UNIX_EPOCH.saturating_sub(std::time::Duration::MAX)?,
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn saturating_sub<A: Into<TimestampArithmetic>>(
        self,
        duration: A,
    ) -> Result<Timestamp, Error> {
        let duration: TimestampArithmetic = duration.into();
        let Ok(duration) = duration.checked_neg() else {
            return Ok(Timestamp::MIN);
        };
        self.saturating_add(duration)
    }

    /// Returns a span representing the elapsed time from this timestamp until
    /// the given `other` timestamp.
    ///
    /// When `other` occurs before this timestamp, then the span returned will
    /// be negative.
    ///
    /// Depending on the input provided, the span returned is rounded. It may
    /// also be balanced up to bigger units than the default. By default,
    /// the span returned is balanced such that the biggest possible unit is
    /// seconds.
    ///
    /// This operation is configured by providing a [`TimestampDifference`]
    /// value. Since this routine accepts anything that implements
    /// `Into<TimestampDifference>`, once can pass a `Timestamp` directly.
    /// One can also pass a `(Unit, Timestamp)`, where `Unit` is treated as
    /// [`TimestampDifference::largest`].
    ///
    /// # Properties
    ///
    /// It is guaranteed that if the returned span is subtracted from `other`,
    /// and if no rounding is requested, then the original timestamp will be
    /// returned.
    ///
    /// This routine is equivalent to `self.since(other).map(|span| -span)`
    /// if no rounding options are set. If rounding options are set, then
    /// it's equivalent to
    /// `self.since(other_without_rounding_options).map(|span| -span)`,
    /// followed by a call to [`Span::round`] with the appropriate rounding
    /// options set. This is because the negation of a span can result in
    /// different rounding results depending on the rounding mode.
    ///
    /// # Errors
    ///
    /// An error can occur in some cases when the requested configuration
    /// would result in a span that is beyond allowable limits. For example,
    /// the nanosecond component of a span cannot represent the span of
    /// time between the minimum and maximum timestamps supported by Jiff.
    /// Therefore, if one requests a span with its largest unit set to
    /// [`Unit::Nanosecond`], then it's possible for this routine to fail.
    ///
    /// An error can also occur if `TimestampDifference` is misconfigured. For
    /// example, if the smallest unit provided is bigger than the largest unit,
    /// or if the largest unit provided is bigger than hours. (To use bigger
    /// units with an instant in time, use [`Zoned::until`] instead.)
    ///
    /// It is guaranteed that if one provides a timestamp with the default
    /// [`TimestampDifference`] configuration, then this routine will never
    /// fail.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::{Timestamp, ToSpan};
    ///
    /// let earlier: Timestamp = "2006-08-24T22:30:00Z".parse()?;
    /// let later: Timestamp = "2019-01-31 21:00:00Z".parse()?;
    /// assert_eq!(earlier.until(later)?, 392509800.seconds().fieldwise());
    ///
    /// // Flipping the timestamps is fine, but you'll get a negative span.
    /// assert_eq!(later.until(earlier)?, -392509800.seconds().fieldwise());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: using bigger units
    ///
    /// This example shows how to expand the span returned to bigger units.
    /// This makes use of a `From<(Unit, Timestamp)> for TimestampDifference`
    /// trait implementation.
    ///
    /// ```
    /// use jiff::{Timestamp, ToSpan, Unit};
    ///
    /// let ts1: Timestamp = "1995-12-07T03:24:30.000003500Z".parse()?;
    /// let ts2: Timestamp = "2019-01-31 15:30:00Z".parse()?;
    ///
    /// // The default limits durations to using "seconds" as the biggest unit.
    /// let span = ts1.until(ts2)?;
    /// assert_eq!(span.to_string(), "PT730641929.9999965S");
    ///
    /// // But we can ask for units all the way up to hours.
    /// let span = ts1.until((Unit::Hour, ts2))?;
    /// assert_eq!(span.to_string(), "PT202956H5M29.9999965S");
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: rounding the result
    ///
    /// This shows how one might find the difference between two timestamps and
    /// have the result rounded such that sub-seconds are removed.
    ///
    /// In this case, we need to hand-construct a [`TimestampDifference`]
    /// in order to gain full configurability.
    ///
    /// ```
    /// use jiff::{Timestamp, TimestampDifference, ToSpan, Unit};
    ///
    /// let ts1: Timestamp = "1995-12-07 03:24:30.000003500Z".parse()?;
    /// let ts2: Timestamp = "2019-01-31 15:30:00Z".parse()?;
    ///
    /// let span = ts1.until(
    ///     TimestampDifference::from(ts2).smallest(Unit::Second),
    /// )?;
    /// assert_eq!(span.to_string(), "PT730641929S");
    ///
    /// // We can combine smallest and largest units too!
    /// let span = ts1.until(
    ///     TimestampDifference::from(ts2)
    ///         .smallest(Unit::Second)
    ///         .largest(Unit::Hour),
    /// )?;
    /// assert_eq!(span.to_string(), "PT202956H5M29S");
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn until<A: Into<TimestampDifference>>(
        self,
        other: A,
    ) -> Result<Span, Error> {
        let args: TimestampDifference = other.into();
        let span = args.until_with_largest_unit(self)?;
        if args.rounding_may_change_span() {
            span.round(args.round)
        } else {
            Ok(span)
        }
    }

    /// This routine is identical to [`Timestamp::until`], but the order of the
    /// parameters is flipped.
    ///
    /// # Errors
    ///
    /// This has the same error conditions as [`Timestamp::until`].
    ///
    /// # Example
    ///
    /// This routine can be used via the `-` operator. Since the default
    /// configuration is used and because a `Span` can represent the difference
    /// between any two possible timestamps, it will never panic.
    ///
    /// ```
    /// use jiff::{Timestamp, ToSpan};
    ///
    /// let earlier: Timestamp = "2006-08-24T22:30:00Z".parse()?;
    /// let later: Timestamp = "2019-01-31 21:00:00Z".parse()?;
    /// assert_eq!(later - earlier, 392509800.seconds().fieldwise());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn since<A: Into<TimestampDifference>>(
        self,
        other: A,
    ) -> Result<Span, Error> {
        let args: TimestampDifference = other.into();
        let span = -args.until_with_largest_unit(self)?;
        if args.rounding_may_change_span() {
            span.round(args.round)
        } else {
            Ok(span)
        }
    }

    /// Returns an absolute duration representing the elapsed time from this
    /// timestamp until the given `other` timestamp.
    ///
    /// When `other` occurs before this timestamp, then the duration returned
    /// will be negative.
    ///
    /// Unlike [`Timestamp::until`], this always returns a duration
    /// corresponding to a 96-bit integer of nanoseconds between two
    /// timestamps.
    ///
    /// # Fallibility
    ///
    /// This routine never panics or returns an error. Since there are no
    /// configuration options that can be incorrectly provided, no error is
    /// possible when calling this routine. In contrast, [`Timestamp::until`]
    /// can return an error in some cases due to misconfiguration. But like
    /// this routine, [`Timestamp::until`] never panics or returns an error in
    /// its default configuration.
    ///
    /// # When should I use this versus [`Timestamp::until`]?
    ///
    /// See the type documentation for [`SignedDuration`] for the section on
    /// when one should use [`Span`] and when one should use `SignedDuration`.
    /// In short, use `Span` (and therefore `Timestamp::until`) unless you have
    /// a specific reason to do otherwise.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::{Timestamp, SignedDuration};
    ///
    /// let earlier: Timestamp = "2006-08-24T22:30:00Z".parse()?;
    /// let later: Timestamp = "2019-01-31 21:00:00Z".parse()?;
    /// assert_eq!(
    ///     earlier.duration_until(later),
    ///     SignedDuration::from_secs(392509800),
    /// );
    ///
    /// // Flipping the timestamps is fine, but you'll get a negative span.
    /// assert_eq!(
    ///     later.duration_until(earlier),
    ///     SignedDuration::from_secs(-392509800),
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: difference with [`Timestamp::until`]
    ///
    /// The primary difference between this routine and
    /// `Timestamp::until`, other than the return type, is that this
    /// routine is likely to be faster. Namely, it does simple 96-bit
    /// integer math, where as `Timestamp::until` has to do a bit more
    /// work to deal with the different types of units on a `Span`.
    ///
    /// Additionally, since the difference between two timestamps is always
    /// expressed in units of hours or smaller, and units of hours or smaller
    /// are always uniform, there is no "expressive" difference between this
    /// routine and `Timestamp::until`. Because of this, one can always
    /// convert between `Span` and `SignedDuration` as returned by methods
    /// on `Timestamp` without a relative datetime:
    ///
    /// ```
    /// use jiff::{SignedDuration, Span, Timestamp};
    ///
    /// let ts1: Timestamp = "2024-02-28T00:00:00Z".parse()?;
    /// let ts2: Timestamp = "2024-03-01T00:00:00Z".parse()?;
    /// let dur = ts1.duration_until(ts2);
    /// // Guaranteed to never fail because the duration
    /// // between two civil times never exceeds the limits
    /// // of a `Span`.
    /// let span = Span::try_from(dur).unwrap();
    /// assert_eq!(format!("{span:#}"), "172800s");
    /// // Guaranteed to succeed and always return the original
    /// // duration because the units are always hours or smaller,
    /// // and thus uniform. This means a relative datetime is
    /// // never required to do this conversion.
    /// let dur = SignedDuration::try_from(span).unwrap();
    /// assert_eq!(dur, SignedDuration::from_secs(172_800));
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// This conversion guarantee also applies to [`Timestamp::until`] since it
    /// always returns a balanced span. That is, it never returns spans like
    /// `1 second 1000 milliseconds`. (Those cannot be losslessly converted to
    /// a `SignedDuration` since a `SignedDuration` is only represented as a
    /// single 96-bit integer of nanoseconds.)
    #[inline]
    pub fn duration_until(self, other: Timestamp) -> SignedDuration {
        SignedDuration::timestamp_until(self, other)
    }

    /// This routine is identical to [`Timestamp::duration_until`], but the
    /// order of the parameters is flipped.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::{SignedDuration, Timestamp};
    ///
    /// let earlier: Timestamp = "2006-08-24T22:30:00Z".parse()?;
    /// let later: Timestamp = "2019-01-31 21:00:00Z".parse()?;
    /// assert_eq!(
    ///     later.duration_since(earlier),
    ///     SignedDuration::from_secs(392509800),
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn duration_since(self, other: Timestamp) -> SignedDuration {
        SignedDuration::timestamp_until(other, self)
    }

    /// Rounds this timestamp according to the [`TimestampRound`] configuration
    /// given.
    ///
    /// The principal option is [`TimestampRound::smallest`], which allows
    /// one to configure the smallest units in the returned timestamp.
    /// Rounding is what determines whether the specified smallest unit
    /// should keep its current value or whether it should be incremented.
    /// Moreover, the amount it should be incremented can be configured via
    /// [`TimestampRound::increment`]. Finally, the rounding strategy itself
    /// can be configured via [`TimestampRound::mode`].
    ///
    /// Note that this routine is generic and accepts anything that
    /// implements `Into<TimestampRound>`. Some notable implementations are:
    ///
    /// * `From<Unit> for TimestampRound`, which will automatically create a
    /// `TimestampRound::new().smallest(unit)` from the unit provided.
    /// * `From<(Unit, i64)> for TimestampRound`, which will automatically
    /// create a `TimestampRound::new().smallest(unit).increment(number)` from
    /// the unit and increment provided.
    ///
    /// # Errors
    ///
    /// This returns an error if the smallest unit configured on the given
    /// [`TimestampRound`] is bigger than hours.
    ///
    /// The rounding increment, when combined with the smallest unit (which
    /// defaults to [`Unit::Nanosecond`]), must divide evenly into `86,400`
    /// seconds (one 24-hour civil day). For example, increments of both
    /// 45 seconds and 15 minutes are allowed, but 7 seconds and 25 minutes are
    /// both not allowed.
    ///
    /// # Example
    ///
    /// This is a basic example that demonstrates rounding a timestamp to the
    /// nearest hour. This also demonstrates calling this method with the
    /// smallest unit directly, instead of constructing a `TimestampRound`
    /// manually.
    ///
    /// ```
    /// use jiff::{Timestamp, Unit};
    ///
    /// let ts: Timestamp = "2024-06-19 15:30:00Z".parse()?;
    /// assert_eq!(
    ///     ts.round(Unit::Hour)?.to_string(),
    ///     "2024-06-19T16:00:00Z",
    /// );
    /// let ts: Timestamp = "2024-06-19 15:29:59Z".parse()?;
    /// assert_eq!(
    ///     ts.round(Unit::Hour)?.to_string(),
    ///     "2024-06-19T15:00:00Z",
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: changing the rounding mode
    ///
    /// The default rounding mode is [`RoundMode::HalfExpand`], which
    /// breaks ties by rounding away from zero. But other modes like
    /// [`RoundMode::Trunc`] can be used too:
    ///
    /// ```
    /// use jiff::{RoundMode, Timestamp, TimestampRound, Unit};
    ///
    /// // The default will round up to the next hour for any time past the
    /// // 30 minute mark, but using truncation rounding will always round
    /// // down.
    /// let ts: Timestamp = "2024-06-19 15:30:00Z".parse()?;
    /// assert_eq!(
    ///     ts.round(
    ///         TimestampRound::new()
    ///             .smallest(Unit::Hour)
    ///             .mode(RoundMode::Trunc),
    ///     )?.to_string(),
    ///     "2024-06-19T15:00:00Z",
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: rounding to the nearest 5 minute increment
    ///
    /// ```
    /// use jiff::{Timestamp, Unit};
    ///
    /// // rounds down
    /// let ts: Timestamp = "2024-06-19T15:27:29.999999999Z".parse()?;
    /// assert_eq!(
    ///     ts.round((Unit::Minute, 5))?.to_string(),
    ///     "2024-06-19T15:25:00Z",
    /// );
    /// // rounds up
    /// let ts: Timestamp = "2024-06-19T15:27:30Z".parse()?;
    /// assert_eq!(
    ///     ts.round((Unit::Minute, 5))?.to_string(),
    ///     "2024-06-19T15:30:00Z",
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn round<R: Into<TimestampRound>>(
        self,
        options: R,
    ) -> Result<Timestamp, Error> {
        let options: TimestampRound = options.into();
        options.round(self)
    }

    /// Return an iterator of periodic timestamps determined by the given span.
    ///
    /// The given span may be negative, in which case, the iterator will move
    /// backwards through time. The iterator won't stop until either the span
    /// itself overflows, or it would otherwise exceed the minimum or maximum
    /// `Timestamp` value.
    ///
    /// # Example: when to check a glucose monitor
    ///
    /// When my cat had diabetes, my veterinarian installed a glucose monitor
    /// and instructed me to scan it about every 5 hours. This example lists
    /// all of the times I need to scan it for the 2 days following its
    /// installation:
    ///
    /// ```
    /// use jiff::{Timestamp, ToSpan};
    ///
    /// let start: Timestamp = "2023-07-15 16:30:00-04".parse()?;
    /// let end = start.checked_add(48.hours())?;
    /// let mut scan_times = vec![];
    /// for ts in start.series(5.hours()).take_while(|&ts| ts <= end) {
    ///     scan_times.push(ts);
    /// }
    /// assert_eq!(scan_times, vec![
    ///     "2023-07-15 16:30:00-04:00".parse::<Timestamp>()?,
    ///     "2023-07-15 21:30:00-04:00".parse::<Timestamp>()?,
    ///     "2023-07-16 02:30:00-04:00".parse::<Timestamp>()?,
    ///     "2023-07-16 07:30:00-04:00".parse::<Timestamp>()?,
    ///     "2023-07-16 12:30:00-04:00".parse::<Timestamp>()?,
    ///     "2023-07-16 17:30:00-04:00".parse::<Timestamp>()?,
    ///     "2023-07-16 22:30:00-04:00".parse::<Timestamp>()?,
    ///     "2023-07-17 03:30:00-04:00".parse::<Timestamp>()?,
    ///     "2023-07-17 08:30:00-04:00".parse::<Timestamp>()?,
    ///     "2023-07-17 13:30:00-04:00".parse::<Timestamp>()?,
    /// ]);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn series(self, period: Span) -> TimestampSeries {
        TimestampSeries::new(self, period)
    }
}

/// Parsing and formatting APIs.
impl Timestamp {
    /// Parses a timestamp (expressed as broken down time) in `input` matching
    /// the given `format`.
    ///
    /// The format string uses a "printf"-style API where conversion
    /// specifiers can be used as place holders to match components of
    /// a datetime. For details on the specifiers supported, see the
    /// [`fmt::strtime`] module documentation.
    ///
    /// # Errors
    ///
    /// This returns an error when parsing failed. This might happen because
    /// the format string itself was invalid, or because the input didn't match
    /// the format string.
    ///
    /// This also returns an error if there wasn't sufficient information to
    /// construct a timestamp. For example, if an offset wasn't parsed. (The
    /// offset is needed to turn the civil time parsed into a precise instant
    /// in time.)
    ///
    /// # Example
    ///
    /// This example shows how to parse a datetime string into a timestamp:
    ///
    /// ```
    /// use jiff::Timestamp;
    ///
    /// let ts = Timestamp::strptime("%F %H:%M %:z", "2024-07-14 21:14 -04:00")?;
    /// assert_eq!(ts.to_string(), "2024-07-15T01:14:00Z");
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn strptime(
        format: impl AsRef<[u8]>,
        input: impl AsRef<[u8]>,
    ) -> Result<Timestamp, Error> {
        fmt::strtime::parse(format, input).and_then(|tm| tm.to_timestamp())
    }

    /// Formats this timestamp according to the given `format`.
    ///
    /// The format string uses a "printf"-style API where conversion
    /// specifiers can be used as place holders to format components of
    /// a datetime. For details on the specifiers supported, see the
    /// [`fmt::strtime`] module documentation.
    ///
    /// # Errors and panics
    ///
    /// While this routine itself does not error or panic, using the value
    /// returned may result in a panic if formatting fails. See the
    /// documentation on [`fmt::strtime::Display`] for more information.
    ///
    /// To format in a way that surfaces errors without panicking, use either
    /// [`fmt::strtime::format`] or [`fmt::strtime::BrokenDownTime::format`].
    ///
    /// # Example
    ///
    /// This shows how to format a timestamp into a human readable datetime
    /// in UTC:
    ///
    /// ```
    /// use jiff::{civil::date, Timestamp};
    ///
    /// let ts = Timestamp::from_second(86_400)?;
    /// let string = ts.strftime("%a %b %e %I:%M:%S %p UTC %Y").to_string();
    /// assert_eq!(string, "Fri Jan  2 12:00:00 AM UTC 1970");
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn strftime<'f, F: 'f + ?Sized + AsRef<[u8]>>(
        &self,
        format: &'f F,
    ) -> fmt::strtime::Display<'f> {
        fmt::strtime::Display { fmt: format.as_ref(), tm: (*self).into() }
    }

    /// Format a `Timestamp` datetime into a string with the given offset.
    ///
    /// This will format to an RFC 3339 compatible string with an offset.
    ///
    /// This will never use either `Z` (for Zulu time) or `-00:00` as an
    /// offset. This is because Zulu time (and `-00:00`) mean "the time in UTC
    /// is known, but the offset to local time is unknown." Since this routine
    /// accepts an explicit offset, the offset is known. For example,
    /// `Offset::UTC` will be formatted as `+00:00`.
    ///
    /// To format an RFC 3339 string in Zulu time, use the default
    /// [`std::fmt::Display`] trait implementation on `Timestamp`.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::{tz, Timestamp};
    ///
    /// let ts = Timestamp::from_second(1)?;
    /// assert_eq!(
    ///     ts.display_with_offset(tz::offset(-5)).to_string(),
    ///     "1969-12-31T19:00:01-05:00",
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn display_with_offset(
        &self,
        offset: Offset,
    ) -> TimestampDisplayWithOffset {
        TimestampDisplayWithOffset { timestamp: *self, offset }
    }
}

/// Internal APIs using Jiff ranged integers.
impl Timestamp {
    #[inline]
    pub(crate) fn new_ranged(
        second: UnixSeconds,
        nanosecond: FractionalNanosecond,
    ) -> Result<Timestamp, Error> {
        if second == UnixSeconds::MIN_SELF && nanosecond < C(0) {
            return Err(Error::range(
                "seconds and nanoseconds",
                nanosecond,
                0,
                1_000_000_000,
            ));
        }
        // We now normalize our seconds and nanoseconds such that they have
        // the same sign (or where one is zero). So for example, when given
        // `-1s 1ns`, then we should turn that into `-999,999,999ns`.
        //
        // But first, if we're already normalized, we're done!
        if second.signum() == nanosecond.signum()
            || second == C(0)
            || nanosecond == C(0)
        {
            return Ok(Timestamp { second, nanosecond });
        }
        let second = second.without_bounds();
        let nanosecond = nanosecond.without_bounds();
        let [delta_second, delta_nanosecond] = t::NoUnits::vary_many(
            [second, nanosecond],
            |[second, nanosecond]| {
                if second < C(0) && nanosecond > C(0) {
                    [C(1), (-t::NANOS_PER_SECOND).rinto()]
                } else if second > C(0) && nanosecond < C(0) {
                    [C(-1), t::NANOS_PER_SECOND.rinto()]
                } else {
                    [C(0), C(0)]
                }
            },
        );
        Ok(Timestamp {
            second: (second + delta_second).rinto(),
            nanosecond: (nanosecond + delta_nanosecond).rinto(),
        })
    }

    #[inline]
    fn from_second_ranged(second: UnixSeconds) -> Timestamp {
        Timestamp { second, nanosecond: FractionalNanosecond::N::<0>() }
    }

    #[inline]
    fn from_millisecond_ranged(millisecond: UnixMilliseconds) -> Timestamp {
        let second =
            UnixSeconds::rfrom(millisecond.div_ceil(t::MILLIS_PER_SECOND));
        let nanosecond = FractionalNanosecond::rfrom(
            millisecond.rem_ceil(t::MILLIS_PER_SECOND) * t::NANOS_PER_MILLI,
        );
        Timestamp { second, nanosecond }
    }

    #[inline]
    fn from_microsecond_ranged(microsecond: UnixMicroseconds) -> Timestamp {
        let second =
            UnixSeconds::rfrom(microsecond.div_ceil(t::MICROS_PER_SECOND));
        let nanosecond = FractionalNanosecond::rfrom(
            microsecond.rem_ceil(t::MICROS_PER_SECOND) * t::NANOS_PER_MICRO,
        );
        Timestamp { second, nanosecond }
    }

    #[inline]
    pub(crate) fn from_nanosecond_ranged(
        nanosecond: UnixNanoseconds,
    ) -> Timestamp {
        let second =
            UnixSeconds::rfrom(nanosecond.div_ceil(t::NANOS_PER_SECOND));
        let nanosecond = nanosecond.rem_ceil(t::NANOS_PER_SECOND).rinto();
        Timestamp { second, nanosecond }
    }

    #[inline]
    pub(crate) fn from_itimestamp(
        its: Composite<ITimestamp>,
    ) -> Result<Timestamp, Error> {
        let (second, nanosecond) =
            rangeint::uncomposite!(its, c => (c.second, c.nanosecond));
        Ok(Timestamp {
            second: second.try_to_rint("unix-seconds")?,
            nanosecond: nanosecond.to_rint(),
        })
    }

    #[inline]
    pub(crate) fn to_itimestamp(&self) -> Composite<ITimestamp> {
        rangeint::composite! {
            (second = self.second, nanosecond = self.nanosecond) => {
                ITimestamp { second, nanosecond }
            }
        }
    }

    #[inline]
    pub(crate) const fn from_itimestamp_const(its: ITimestamp) -> Timestamp {
        Timestamp {
            second: UnixSeconds::new_unchecked(its.second),
            nanosecond: FractionalNanosecond::new_unchecked(its.nanosecond),
        }
    }

    #[inline]
    pub(crate) const fn to_itimestamp_const(&self) -> ITimestamp {
        ITimestamp {
            second: self.second.get_unchecked(),
            nanosecond: self.nanosecond.get_unchecked(),
        }
    }

    #[inline]
    pub(crate) fn as_second_ranged(self) -> UnixSeconds {
        self.second
    }

    #[inline]
    fn as_millisecond_ranged(self) -> UnixMilliseconds {
        let second = NoUnits::rfrom(self.as_second_ranged());
        let nanosecond = NoUnits::rfrom(self.subsec_nanosecond_ranged());
        // The minimum value of a timestamp has the fractional nanosecond set
        // to 0, but otherwise its minimum value is -999_999_999. So to avoid
        // a case where we return a ranged integer with a minimum value less
        // than the actual minimum, we clamp the fractional part to 0 when the
        // second value is the minimum.
        let [second, nanosecond] =
            NoUnits::vary_many([second, nanosecond], |[second, nanosecond]| {
                if second == UnixSeconds::MIN_SELF && nanosecond < C(0) {
                    [second, C(0).rinto()]
                } else {
                    [second, nanosecond]
                }
            });
        UnixMilliseconds::rfrom(
            (second * t::MILLIS_PER_SECOND)
                + (nanosecond.div_ceil(t::NANOS_PER_MILLI)),
        )
    }

    #[inline]
    fn as_microsecond_ranged(self) -> UnixMicroseconds {
        let second = NoUnits::rfrom(self.as_second_ranged());
        let nanosecond = NoUnits::rfrom(self.subsec_nanosecond_ranged());
        // The minimum value of a timestamp has the fractional nanosecond set
        // to 0, but otherwise its minimum value is -999_999_999. So to avoid
        // a case where we return a ranged integer with a minimum value less
        // than the actual minimum, we clamp the fractional part to 0 when the
        // second value is the minimum.
        let [second, nanosecond] =
            NoUnits::vary_many([second, nanosecond], |[second, nanosecond]| {
                if second == UnixSeconds::MIN_SELF && nanosecond < C(0) {
                    [second, C(0).rinto()]
                } else {
                    [second, nanosecond]
                }
            });
        UnixMicroseconds::rfrom(
            (second * t::MICROS_PER_SECOND)
                + (nanosecond.div_ceil(t::NANOS_PER_MICRO)),
        )
    }

    #[inline]
    pub(crate) fn as_nanosecond_ranged(self) -> UnixNanoseconds {
        let second = NoUnits128::rfrom(self.as_second_ranged());
        let nanosecond = NoUnits128::rfrom(self.subsec_nanosecond_ranged());
        // The minimum value of a timestamp has the fractional nanosecond set
        // to 0, but otherwise its minimum value is -999_999_999. So to avoid
        // a case where we return a ranged integer with a minimum value less
        // than the actual minimum, we clamp the fractional part to 0 when the
        // second value is the minimum.
        let [second, nanosecond] = NoUnits128::vary_many(
            [second, nanosecond],
            |[second, nanosecond]| {
                if second == UnixSeconds::MIN_SELF && nanosecond < C(0) {
                    [second, C(0).rinto()]
                } else {
                    [second, nanosecond]
                }
            },
        );
        UnixNanoseconds::rfrom(second * t::NANOS_PER_SECOND + nanosecond)
    }

    #[inline]
    fn subsec_millisecond_ranged(self) -> t::FractionalMillisecond {
        let millis =
            self.subsec_nanosecond_ranged().div_ceil(t::NANOS_PER_MILLI);
        t::FractionalMillisecond::rfrom(millis)
    }

    #[inline]
    fn subsec_microsecond_ranged(self) -> t::FractionalMicrosecond {
        let micros =
            self.subsec_nanosecond_ranged().div_ceil(t::NANOS_PER_MICRO);
        t::FractionalMicrosecond::rfrom(micros)
    }

    #[inline]
    pub(crate) fn subsec_nanosecond_ranged(self) -> FractionalNanosecond {
        self.nanosecond
    }
}

impl Default for Timestamp {
    #[inline]
    fn default() -> Timestamp {
        Timestamp::UNIX_EPOCH
    }
}

/// Converts a `Timestamp` datetime into a human readable datetime string.
///
/// (This `Debug` representation currently emits the same string as the
/// `Display` representation, but this is not a guarantee.)
///
/// Options currently supported:
///
/// * [`std::fmt::Formatter::precision`] can be set to control the precision
/// of the fractional second component.
///
/// # Example
///
/// ```
/// use jiff::Timestamp;
///
/// let ts = Timestamp::new(1_123_456_789, 123_000_000)?;
/// assert_eq!(
///     format!("{ts:.6?}"),
///     "2005-08-07T23:19:49.123000Z",
/// );
/// // Precision values greater than 9 are clamped to 9.
/// assert_eq!(
///     format!("{ts:.300?}"),
///     "2005-08-07T23:19:49.123000000Z",
/// );
/// // A precision of 0 implies the entire fractional
/// // component is always truncated.
/// assert_eq!(
///     format!("{ts:.0?}"),
///     "2005-08-07T23:19:49Z",
/// );
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
impl core::fmt::Debug for Timestamp {
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        core::fmt::Display::fmt(self, f)
    }
}

/// Converts a `Timestamp` datetime into a RFC 3339 compliant string.
///
/// Since a `Timestamp` never has an offset associated with it and is always
/// in UTC, the string emitted by this trait implementation uses `Z` for "Zulu"
/// time. The significance of Zulu time is prescribed by RFC 9557 and means
/// that "the time in UTC is known, but the offset to local time is unknown."
/// If you need to emit an RFC 3339 compliant string with a specific offset,
/// then use [`Timestamp::display_with_offset`].
///
/// # Formatting options supported
///
/// * [`std::fmt::Formatter::precision`] can be set to control the precision
/// of the fractional second component. When not set, the minimum precision
/// required to losslessly render the value is used.
///
/// # Example
///
/// This shows the default rendering:
///
/// ```
/// use jiff::Timestamp;
///
/// // No fractional seconds.
/// let ts = Timestamp::from_second(1_123_456_789)?;
/// assert_eq!(format!("{ts}"), "2005-08-07T23:19:49Z");
///
/// // With fractional seconds.
/// let ts = Timestamp::new(1_123_456_789, 123_000_000)?;
/// assert_eq!(format!("{ts}"), "2005-08-07T23:19:49.123Z");
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Example: setting the precision
///
/// ```
/// use jiff::Timestamp;
///
/// let ts = Timestamp::new(1_123_456_789, 123_000_000)?;
/// assert_eq!(
///     format!("{ts:.6}"),
///     "2005-08-07T23:19:49.123000Z",
/// );
/// // Precision values greater than 9 are clamped to 9.
/// assert_eq!(
///     format!("{ts:.300}"),
///     "2005-08-07T23:19:49.123000000Z",
/// );
/// // A precision of 0 implies the entire fractional
/// // component is always truncated.
/// assert_eq!(
///     format!("{ts:.0}"),
///     "2005-08-07T23:19:49Z",
/// );
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
impl core::fmt::Display for Timestamp {
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        use crate::fmt::StdFmtWrite;

        let precision =
            f.precision().map(|p| u8::try_from(p).unwrap_or(u8::MAX));
        temporal::DateTimePrinter::new()
            .precision(precision)
            .print_timestamp(self, StdFmtWrite(f))
            .map_err(|_| core::fmt::Error)
    }
}

impl core::str::FromStr for Timestamp {
    type Err = Error;

    #[inline]
    fn from_str(string: &str) -> Result<Timestamp, Error> {
        DEFAULT_DATETIME_PARSER.parse_timestamp(string)
    }
}

impl Eq for Timestamp {}

impl PartialEq for Timestamp {
    #[inline]
    fn eq(&self, rhs: &Timestamp) -> bool {
        self.as_second_ranged().get() == rhs.as_second_ranged().get()
            && self.subsec_nanosecond_ranged().get()
                == rhs.subsec_nanosecond_ranged().get()
    }
}

impl Ord for Timestamp {
    #[inline]
    fn cmp(&self, rhs: &Timestamp) -> core::cmp::Ordering {
        (self.as_second_ranged().get(), self.subsec_nanosecond_ranged().get())
            .cmp(&(
                rhs.as_second_ranged().get(),
                rhs.subsec_nanosecond_ranged().get(),
            ))
    }
}

impl PartialOrd for Timestamp {
    #[inline]
    fn partial_cmp(&self, rhs: &Timestamp) -> Option<core::cmp::Ordering> {
        Some(self.cmp(rhs))
    }
}

impl core::hash::Hash for Timestamp {
    #[inline]
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.as_second_ranged().get().hash(state);
        self.subsec_nanosecond_ranged().get().hash(state);
    }
}

/// Adds a span of time to a timestamp.
///
/// This uses checked arithmetic and panics when it fails. To handle arithmetic
/// without panics, use [`Timestamp::checked_add`]. Note that the failure
/// condition includes overflow and using a `Span` with non-zero units greater
/// than hours.
impl core::ops::Add<Span> for Timestamp {
    type Output = Timestamp;

    #[inline]
    fn add(self, rhs: Span) -> Timestamp {
        self.checked_add_span(rhs).expect("adding span to timestamp failed")
    }
}

/// Adds a span of time to a timestamp in place.
///
/// This uses checked arithmetic and panics when it fails. To handle arithmetic
/// without panics, use [`Timestamp::checked_add`]. Note that the failure
/// condition includes overflow and using a `Span` with non-zero units greater
/// than hours.
impl core::ops::AddAssign<Span> for Timestamp {
    #[inline]
    fn add_assign(&mut self, rhs: Span) {
        *self = *self + rhs
    }
}

/// Subtracts a span of time from a timestamp.
///
/// This uses checked arithmetic and panics when it fails. To handle arithmetic
/// without panics, use [`Timestamp::checked_sub`]. Note that the failure
/// condition includes overflow and using a `Span` with non-zero units greater
/// than hours.
impl core::ops::Sub<Span> for Timestamp {
    type Output = Timestamp;

    #[inline]
    fn sub(self, rhs: Span) -> Timestamp {
        self.checked_add_span(rhs.negate())
            .expect("subtracting span from timestamp failed")
    }
}

/// Subtracts a span of time from a timestamp in place.
///
/// This uses checked arithmetic and panics when it fails. To handle arithmetic
/// without panics, use [`Timestamp::checked_sub`]. Note that the failure
/// condition includes overflow and using a `Span` with non-zero units greater
/// than hours.
impl core::ops::SubAssign<Span> for Timestamp {
    #[inline]
    fn sub_assign(&mut self, rhs: Span) {
        *self = *self - rhs
    }
}

/// Computes the span of time between two timestamps.
///
/// This will return a negative span when the timestamp being subtracted is
/// greater.
///
/// Since this uses the default configuration for calculating a span between
/// two timestamps (no rounding and largest units is seconds), this will never
/// panic or fail in any way.
///
/// To configure the largest unit or enable rounding, use [`Timestamp::since`].
impl core::ops::Sub for Timestamp {
    type Output = Span;

    #[inline]
    fn sub(self, rhs: Timestamp) -> Span {
        self.since(rhs).expect("since never fails when given Timestamp")
    }
}

/// Adds a signed duration of time to a timestamp.
///
/// This uses checked arithmetic and panics on overflow. To handle overflow
/// without panics, use [`Timestamp::checked_add`].
impl core::ops::Add<SignedDuration> for Timestamp {
    type Output = Timestamp;

    #[inline]
    fn add(self, rhs: SignedDuration) -> Timestamp {
        self.checked_add_duration(rhs)
            .expect("adding signed duration to timestamp overflowed")
    }
}

/// Adds a signed duration of time to a timestamp in place.
///
/// This uses checked arithmetic and panics on overflow. To handle overflow
/// without panics, use [`Timestamp::checked_add`].
impl core::ops::AddAssign<SignedDuration> for Timestamp {
    #[inline]
    fn add_assign(&mut self, rhs: SignedDuration) {
        *self = *self + rhs
    }
}

/// Subtracts a signed duration of time from a timestamp.
///
/// This uses checked arithmetic and panics on overflow. To handle overflow
/// without panics, use [`Timestamp::checked_sub`].
impl core::ops::Sub<SignedDuration> for Timestamp {
    type Output = Timestamp;

    #[inline]
    fn sub(self, rhs: SignedDuration) -> Timestamp {
        let rhs = rhs
            .checked_neg()
            .expect("signed duration negation resulted in overflow");
        self.checked_add_duration(rhs)
            .expect("subtracting signed duration from timestamp overflowed")
    }
}

/// Subtracts a signed duration of time from a timestamp in place.
///
/// This uses checked arithmetic and panics on overflow. To handle overflow
/// without panics, use [`Timestamp::checked_sub`].
impl core::ops::SubAssign<SignedDuration> for Timestamp {
    #[inline]
    fn sub_assign(&mut self, rhs: SignedDuration) {
        *self = *self - rhs
    }
}

/// Adds an unsigned duration of time to a timestamp.
///
/// This uses checked arithmetic and panics on overflow. To handle overflow
/// without panics, use [`Timestamp::checked_add`].
impl core::ops::Add<UnsignedDuration> for Timestamp {
    type Output = Timestamp;

    #[inline]
    fn add(self, rhs: UnsignedDuration) -> Timestamp {
        self.checked_add(rhs)
            .expect("adding unsigned duration to timestamp overflowed")
    }
}

/// Adds an unsigned duration of time to a timestamp in place.
///
/// This uses checked arithmetic and panics on overflow. To handle overflow
/// without panics, use [`Timestamp::checked_add`].
impl core::ops::AddAssign<UnsignedDuration> for Timestamp {
    #[inline]
    fn add_assign(&mut self, rhs: UnsignedDuration) {
        *self = *self + rhs
    }
}

/// Subtracts an unsigned duration of time from a timestamp.
///
/// This uses checked arithmetic and panics on overflow. To handle overflow
/// without panics, use [`Timestamp::checked_sub`].
impl core::ops::Sub<UnsignedDuration> for Timestamp {
    type Output = Timestamp;

    #[inline]
    fn sub(self, rhs: UnsignedDuration) -> Timestamp {
        self.checked_sub(rhs)
            .expect("subtracting unsigned duration from timestamp overflowed")
    }
}

/// Subtracts an unsigned duration of time from a timestamp in place.
///
/// This uses checked arithmetic and panics on overflow. To handle overflow
/// without panics, use [`Timestamp::checked_sub`].
impl core::ops::SubAssign<UnsignedDuration> for Timestamp {
    #[inline]
    fn sub_assign(&mut self, rhs: UnsignedDuration) {
        *self = *self - rhs
    }
}

impl From<Zoned> for Timestamp {
    #[inline]
    fn from(zdt: Zoned) -> Timestamp {
        zdt.timestamp()
    }
}

impl<'a> From<&'a Zoned> for Timestamp {
    #[inline]
    fn from(zdt: &'a Zoned) -> Timestamp {
        zdt.timestamp()
    }
}

#[cfg(feature = "std")]
impl From<Timestamp> for std::time::SystemTime {
    #[inline]
    fn from(time: Timestamp) -> std::time::SystemTime {
        let unix_epoch = std::time::SystemTime::UNIX_EPOCH;
        let sdur = time.as_duration();
        let dur = sdur.unsigned_abs();
        // These are guaranteed to succeed because we assume that SystemTime
        // uses at least 64 bits for the time, and our durations are capped via
        // the range on UnixSeconds.
        if sdur.is_negative() {
            unix_epoch.checked_sub(dur).expect("duration too big (negative)")
        } else {
            unix_epoch.checked_add(dur).expect("duration too big (positive)")
        }
    }
}

#[cfg(feature = "std")]
impl TryFrom<std::time::SystemTime> for Timestamp {
    type Error = Error;

    #[inline]
    fn try_from(
        system_time: std::time::SystemTime,
    ) -> Result<Timestamp, Error> {
        let unix_epoch = std::time::SystemTime::UNIX_EPOCH;
        let dur = SignedDuration::system_until(unix_epoch, system_time)?;
        Timestamp::from_duration(dur)
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for Timestamp {
    #[inline]
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for Timestamp {
    #[inline]
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Timestamp, D::Error> {
        use serde::de;

        struct TimestampVisitor;

        impl<'de> de::Visitor<'de> for TimestampVisitor {
            type Value = Timestamp;

            fn expecting(
                &self,
                f: &mut core::fmt::Formatter,
            ) -> core::fmt::Result {
                f.write_str("a timestamp string")
            }

            #[inline]
            fn visit_bytes<E: de::Error>(
                self,
                value: &[u8],
            ) -> Result<Timestamp, E> {
                DEFAULT_DATETIME_PARSER
                    .parse_timestamp(value)
                    .map_err(de::Error::custom)
            }

            #[inline]
            fn visit_str<E: de::Error>(
                self,
                value: &str,
            ) -> Result<Timestamp, E> {
                self.visit_bytes(value.as_bytes())
            }
        }

        deserializer.deserialize_str(TimestampVisitor)
    }
}

#[cfg(test)]
impl quickcheck::Arbitrary for Timestamp {
    fn arbitrary(g: &mut quickcheck::Gen) -> Timestamp {
        use quickcheck::Arbitrary;

        let seconds: UnixSeconds = Arbitrary::arbitrary(g);
        let mut nanoseconds: FractionalNanosecond = Arbitrary::arbitrary(g);
        // nanoseconds must be zero for the minimum second value,
        // so just clamp it to 0.
        if seconds == UnixSeconds::MIN_SELF && nanoseconds < C(0) {
            nanoseconds = C(0).rinto();
        }
        Timestamp::new_ranged(seconds, nanoseconds).unwrap_or_default()
    }

    fn shrink(&self) -> alloc::boxed::Box<dyn Iterator<Item = Self>> {
        let second = self.as_second_ranged();
        let nanosecond = self.subsec_nanosecond_ranged();
        alloc::boxed::Box::new((second, nanosecond).shrink().filter_map(
            |(second, nanosecond)| {
                if second == UnixSeconds::MIN_SELF && nanosecond > C(0) {
                    None
                } else {
                    Timestamp::new_ranged(second, nanosecond).ok()
                }
            },
        ))
    }
}

/// A type for formatting a [`Timestamp`] with a specific offset.
///
/// This type is created by the [`Timestamp::display_with_offset`] method.
///
/// Like the [`std::fmt::Display`] trait implementation for `Timestamp`, this
/// always emits an RFC 3339 compliant string. Unlike `Timestamp`'s `Display`
/// trait implementation, which always uses `Z` or "Zulu" time, this always
/// uses an offfset.
///
/// # Formatting options supported
///
/// * [`std::fmt::Formatter::precision`] can be set to control the precision
/// of the fractional second component.
///
/// # Example
///
/// ```
/// use jiff::{tz, Timestamp};
///
/// let offset = tz::offset(-5);
/// let ts = Timestamp::new(1_123_456_789, 123_000_000)?;
/// assert_eq!(
///     format!("{ts:.6}", ts = ts.display_with_offset(offset)),
///     "2005-08-07T18:19:49.123000-05:00",
/// );
/// // Precision values greater than 9 are clamped to 9.
/// assert_eq!(
///     format!("{ts:.300}", ts = ts.display_with_offset(offset)),
///     "2005-08-07T18:19:49.123000000-05:00",
/// );
/// // A precision of 0 implies the entire fractional
/// // component is always truncated.
/// assert_eq!(
///     format!("{ts:.0}", ts = ts.display_with_offset(tz::Offset::UTC)),
///     "2005-08-07T23:19:49+00:00",
/// );
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Copy, Debug)]
pub struct TimestampDisplayWithOffset {
    timestamp: Timestamp,
    offset: Offset,
}

impl core::fmt::Display for TimestampDisplayWithOffset {
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        use crate::fmt::StdFmtWrite;

        let precision =
            f.precision().map(|p| u8::try_from(p).unwrap_or(u8::MAX));
        temporal::DateTimePrinter::new()
            .precision(precision)
            .print_timestamp_with_offset(
                &self.timestamp,
                self.offset,
                StdFmtWrite(f),
            )
            .map_err(|_| core::fmt::Error)
    }
}

/// An iterator over periodic timestamps, created by [`Timestamp::series`].
///
/// It is exhausted when the next value would exceed a [`Span`] or
/// [`Timestamp`] value.
#[derive(Clone, Debug)]
pub struct TimestampSeries {
    ts: Timestamp,
    duration: Option<SignedDuration>,
}

impl TimestampSeries {
    #[inline]
    fn new(ts: Timestamp, period: Span) -> TimestampSeries {
        let duration = SignedDuration::try_from(period).ok();
        TimestampSeries { ts, duration }
    }
}

impl Iterator for TimestampSeries {
    type Item = Timestamp;

    #[inline]
    fn next(&mut self) -> Option<Timestamp> {
        let duration = self.duration?;
        let this = self.ts;
        self.ts = self.ts.checked_add_duration(duration).ok()?;
        Some(this)
    }
}

/// Options for [`Timestamp::checked_add`] and [`Timestamp::checked_sub`].
///
/// This type provides a way to ergonomically add one of a few different
/// duration types to a [`Timestamp`].
///
/// The main way to construct values of this type is with its `From` trait
/// implementations:
///
/// * `From<Span> for TimestampArithmetic` adds (or subtracts) the given span
/// to the receiver timestamp.
/// * `From<SignedDuration> for TimestampArithmetic` adds (or subtracts)
/// the given signed duration to the receiver timestamp.
/// * `From<std::time::Duration> for TimestampArithmetic` adds (or subtracts)
/// the given unsigned duration to the receiver timestamp.
///
/// # Example
///
/// ```
/// use std::time::Duration;
///
/// use jiff::{SignedDuration, Timestamp, ToSpan};
///
/// let ts: Timestamp = "2024-02-28T00:00:00Z".parse()?;
/// assert_eq!(
///     ts.checked_add(48.hours())?,
///     "2024-03-01T00:00:00Z".parse()?,
/// );
/// assert_eq!(
///     ts.checked_add(SignedDuration::from_hours(48))?,
///     "2024-03-01T00:00:00Z".parse()?,
/// );
/// assert_eq!(
///     ts.checked_add(Duration::from_secs(48 * 60 * 60))?,
///     "2024-03-01T00:00:00Z".parse()?,
/// );
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Copy, Debug)]
pub struct TimestampArithmetic {
    duration: Duration,
}

impl TimestampArithmetic {
    #[inline]
    fn checked_add(self, ts: Timestamp) -> Result<Timestamp, Error> {
        match self.duration.to_signed()? {
            SDuration::Span(span) => ts.checked_add_span(span),
            SDuration::Absolute(sdur) => ts.checked_add_duration(sdur),
        }
    }

    #[inline]
    fn saturating_add(self, ts: Timestamp) -> Result<Timestamp, Error> {
        let Ok(signed) = self.duration.to_signed() else {
            return Ok(Timestamp::MAX);
        };
        let result = match signed {
            SDuration::Span(span) => {
                if let Some(err) = span.smallest_non_time_non_zero_unit_error()
                {
                    return Err(err);
                }
                ts.checked_add_span(span)
            }
            SDuration::Absolute(sdur) => ts.checked_add_duration(sdur),
        };
        Ok(result.unwrap_or_else(|_| {
            if self.is_negative() {
                Timestamp::MIN
            } else {
                Timestamp::MAX
            }
        }))
    }

    #[inline]
    fn checked_neg(self) -> Result<TimestampArithmetic, Error> {
        let duration = self.duration.checked_neg()?;
        Ok(TimestampArithmetic { duration })
    }

    #[inline]
    fn is_negative(&self) -> bool {
        self.duration.is_negative()
    }
}

impl From<Span> for TimestampArithmetic {
    fn from(span: Span) -> TimestampArithmetic {
        let duration = Duration::from(span);
        TimestampArithmetic { duration }
    }
}

impl From<SignedDuration> for TimestampArithmetic {
    fn from(sdur: SignedDuration) -> TimestampArithmetic {
        let duration = Duration::from(sdur);
        TimestampArithmetic { duration }
    }
}

impl From<UnsignedDuration> for TimestampArithmetic {
    fn from(udur: UnsignedDuration) -> TimestampArithmetic {
        let duration = Duration::from(udur);
        TimestampArithmetic { duration }
    }
}

impl<'a> From<&'a Span> for TimestampArithmetic {
    fn from(span: &'a Span) -> TimestampArithmetic {
        TimestampArithmetic::from(*span)
    }
}

impl<'a> From<&'a SignedDuration> for TimestampArithmetic {
    fn from(sdur: &'a SignedDuration) -> TimestampArithmetic {
        TimestampArithmetic::from(*sdur)
    }
}

impl<'a> From<&'a UnsignedDuration> for TimestampArithmetic {
    fn from(udur: &'a UnsignedDuration) -> TimestampArithmetic {
        TimestampArithmetic::from(*udur)
    }
}

/// Options for [`Timestamp::since`] and [`Timestamp::until`].
///
/// This type provides a way to configure the calculation of
/// spans between two [`Timestamp`] values. In particular, both
/// `Timestamp::since` and `Timestamp::until` accept anything that implements
/// `Into<TimestampDifference>`. There are a few key trait implementations that
/// make this convenient:
///
/// * `From<Timestamp> for TimestampDifference` will construct a
/// configuration consisting of just the timestamp. So for example,
/// `timestamp1.until(timestamp2)` will return the span from `timestamp1` to
/// `timestamp2`.
/// * `From<Zoned> for TimestampDifference` will construct a configuration
/// consisting of the timestamp from the given zoned datetime. So for example,
/// `timestamp.since(zoned)` returns the span from `zoned.to_timestamp()` to
/// `timestamp`.
/// * `From<(Unit, Timestamp)>` is a convenient way to specify the largest
/// units that should be present on the span returned. By default, the largest
/// units are seconds. Using this trait implementation is equivalent to
/// `TimestampDifference::new(timestamp).largest(unit)`.
/// * `From<(Unit, Zoned)>` is like the one above, but with the time from
/// the given zoned datetime.
///
/// One can also provide a `TimestampDifference` value directly. Doing so
/// is necessary to use the rounding features of calculating a span. For
/// example, setting the smallest unit (defaults to [`Unit::Nanosecond`]), the
/// rounding mode (defaults to [`RoundMode::Trunc`]) and the rounding increment
/// (defaults to `1`). The defaults are selected such that no rounding occurs.
///
/// Rounding a span as part of calculating it is provided as a convenience.
/// Callers may choose to round the span as a distinct step via
/// [`Span::round`].
///
/// # Example
///
/// This example shows how to round a span between two timestamps to the
/// nearest half-hour, with ties breaking away from zero.
///
/// ```
/// use jiff::{RoundMode, Timestamp, TimestampDifference, ToSpan, Unit};
///
/// let ts1 = "2024-03-15 08:14:00.123456789Z".parse::<Timestamp>()?;
/// let ts2 = "2024-03-22 15:00Z".parse::<Timestamp>()?;
/// let span = ts1.until(
///     TimestampDifference::new(ts2)
///         .smallest(Unit::Minute)
///         .largest(Unit::Hour)
///         .mode(RoundMode::HalfExpand)
///         .increment(30),
/// )?;
/// assert_eq!(format!("{span:#}"), "175h");
///
/// // One less minute, and because of the HalfExpand mode, the span would
/// // get rounded down.
/// let ts2 = "2024-03-22 14:59Z".parse::<Timestamp>()?;
/// let span = ts1.until(
///     TimestampDifference::new(ts2)
///         .smallest(Unit::Minute)
///         .largest(Unit::Hour)
///         .mode(RoundMode::HalfExpand)
///         .increment(30),
/// )?;
/// assert_eq!(span, 174.hours().minutes(30).fieldwise());
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Copy, Debug)]
pub struct TimestampDifference {
    timestamp: Timestamp,
    round: SpanRound<'static>,
}

impl TimestampDifference {
    /// Create a new default configuration for computing the span between
    /// the given timestamp and some other time (specified as the receiver in
    /// [`Timestamp::since`] or [`Timestamp::until`]).
    #[inline]
    pub fn new(timestamp: Timestamp) -> TimestampDifference {
        // We use truncation rounding by default since it seems that's
        // what is generally expected when computing the difference between
        // datetimes.
        //
        // See: https://github.com/tc39/proposal-temporal/issues/1122
        let round = SpanRound::new().mode(RoundMode::Trunc);
        TimestampDifference { timestamp, round }
    }

    /// Set the smallest units allowed in the span returned.
    ///
    /// # Errors
    ///
    /// The smallest units must be no greater than the largest units. If this
    /// is violated, then computing a span with this configuration will result
    /// in an error.
    ///
    /// # Example
    ///
    /// This shows how to round a span between two timestamps to units no less
    /// than seconds.
    ///
    /// ```
    /// use jiff::{RoundMode, Timestamp, TimestampDifference, ToSpan, Unit};
    ///
    /// let ts1 = "2024-03-15 08:14:02.5001Z".parse::<Timestamp>()?;
    /// let ts2 = "2024-03-15T08:16:03.0001Z".parse::<Timestamp>()?;
    /// let span = ts1.until(
    ///     TimestampDifference::new(ts2)
    ///         .smallest(Unit::Second)
    ///         .mode(RoundMode::HalfExpand),
    /// )?;
    /// assert_eq!(span, 121.seconds().fieldwise());
    ///
    /// // Because of the rounding mode, a small less-than-1-second increase in
    /// // the first timestamp can change the result of rounding.
    /// let ts1 = "2024-03-15 08:14:02.5002Z".parse::<Timestamp>()?;
    /// let span = ts1.until(
    ///     TimestampDifference::new(ts2)
    ///         .smallest(Unit::Second)
    ///         .mode(RoundMode::HalfExpand),
    /// )?;
    /// assert_eq!(span, 120.seconds().fieldwise());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn smallest(self, unit: Unit) -> TimestampDifference {
        TimestampDifference { round: self.round.smallest(unit), ..self }
    }

    /// Set the largest units allowed in the span returned.
    ///
    /// When a largest unit is not specified, computing a span between
    /// timestamps behaves as if it were set to [`Unit::Second`]. Unless
    /// [`TimestampDifference::smallest`] is bigger than `Unit::Second`, then
    /// the largest unit is set to the smallest unit.
    ///
    /// # Errors
    ///
    /// The largest units, when set, must be at least as big as the smallest
    /// units (which defaults to [`Unit::Nanosecond`]). If this is violated,
    /// then computing a span with this configuration will result in an error.
    ///
    /// # Example
    ///
    /// This shows how to round a span between two timestamps to units no
    /// bigger than seconds.
    ///
    /// ```
    /// use jiff::{Timestamp, TimestampDifference, ToSpan, Unit};
    ///
    /// let ts1 = "2024-03-15 08:14Z".parse::<Timestamp>()?;
    /// let ts2 = "2030-11-22 08:30Z".parse::<Timestamp>()?;
    /// let span = ts1.until(
    ///     TimestampDifference::new(ts2).largest(Unit::Second),
    /// )?;
    /// assert_eq!(format!("{span:#}"), "211076160s");
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn largest(self, unit: Unit) -> TimestampDifference {
        TimestampDifference { round: self.round.largest(unit), ..self }
    }

    /// Set the rounding mode.
    ///
    /// This defaults to [`RoundMode::Trunc`] since it's plausible that
    /// rounding "up" in the context of computing the span between
    /// two timestamps could be surprising in a number of cases. The
    /// [`RoundMode::HalfExpand`] mode corresponds to typical rounding you
    /// might have learned about in school. But a variety of other rounding
    /// modes exist.
    ///
    /// # Example
    ///
    /// This shows how to always round "up" towards positive infinity.
    ///
    /// ```
    /// use jiff::{RoundMode, Timestamp, TimestampDifference, ToSpan, Unit};
    ///
    /// let ts1 = "2024-03-15 08:10Z".parse::<Timestamp>()?;
    /// let ts2 = "2024-03-15 08:11Z".parse::<Timestamp>()?;
    /// let span = ts1.until(
    ///     TimestampDifference::new(ts2)
    ///         .smallest(Unit::Hour)
    ///         .mode(RoundMode::Ceil),
    /// )?;
    /// // Only one minute elapsed, but we asked to always round up!
    /// assert_eq!(span, 1.hour().fieldwise());
    ///
    /// // Since `Ceil` always rounds toward positive infinity, the behavior
    /// // flips for a negative span.
    /// let span = ts1.since(
    ///     TimestampDifference::new(ts2)
    ///         .smallest(Unit::Hour)
    ///         .mode(RoundMode::Ceil),
    /// )?;
    /// assert_eq!(span, 0.hour().fieldwise());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn mode(self, mode: RoundMode) -> TimestampDifference {
        TimestampDifference { round: self.round.mode(mode), ..self }
    }

    /// Set the rounding increment for the smallest unit.
    ///
    /// The default value is `1`. Other values permit rounding the smallest
    /// unit to the nearest integer increment specified. For example, if the
    /// smallest unit is set to [`Unit::Minute`], then a rounding increment of
    /// `30` would result in rounding in increments of a half hour. That is,
    /// the only minute value that could result would be `0` or `30`.
    ///
    /// # Errors
    ///
    /// The rounding increment must divide evenly into the next highest unit
    /// after the smallest unit configured (and must not be equivalent to it).
    /// For example, if the smallest unit is [`Unit::Nanosecond`], then *some*
    /// of the valid values for the rounding increment are `1`, `2`, `4`, `5`,
    /// `100` and `500`. Namely, any integer that divides evenly into `1,000`
    /// nanoseconds since there are `1,000` nanoseconds in the next highest
    /// unit (microseconds).
    ///
    /// The error will occur when computing the span, and not when setting
    /// the increment here.
    ///
    /// # Example
    ///
    /// This shows how to round the span between two timestamps to the nearest
    /// 5 minute increment.
    ///
    /// ```
    /// use jiff::{RoundMode, Timestamp, TimestampDifference, ToSpan, Unit};
    ///
    /// let ts1 = "2024-03-15 08:19Z".parse::<Timestamp>()?;
    /// let ts2 = "2024-03-15 12:52Z".parse::<Timestamp>()?;
    /// let span = ts1.until(
    ///     TimestampDifference::new(ts2)
    ///         .smallest(Unit::Minute)
    ///         .increment(5)
    ///         .mode(RoundMode::HalfExpand),
    /// )?;
    /// assert_eq!(span.to_string(), "PT275M");
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn increment(self, increment: i64) -> TimestampDifference {
        TimestampDifference { round: self.round.increment(increment), ..self }
    }

    /// Returns true if and only if this configuration could change the span
    /// via rounding.
    #[inline]
    fn rounding_may_change_span(&self) -> bool {
        self.round.rounding_may_change_span_ignore_largest()
    }

    /// Returns the span of time from `ts1` to the timestamp in this
    /// configuration. The biggest units allowed are determined by the
    /// `smallest` and `largest` settings, but defaults to `Unit::Second`.
    #[inline]
    fn until_with_largest_unit(&self, t1: Timestamp) -> Result<Span, Error> {
        let t2 = self.timestamp;
        let largest = self
            .round
            .get_largest()
            .unwrap_or_else(|| self.round.get_smallest().max(Unit::Second));
        if largest >= Unit::Day {
            return Err(err!(
                "unit {largest} is not supported when computing the \
                 difference between timestamps (must use units smaller \
                 than 'day')",
                largest = largest.singular(),
            ));
        }
        let nano1 = t1.as_nanosecond_ranged().without_bounds();
        let nano2 = t2.as_nanosecond_ranged().without_bounds();
        let diff = nano2 - nano1;
        // This can fail when `largest` is nanoseconds since not all intervals
        // can be represented by a single i64 in units of nanoseconds.
        Span::from_invariant_nanoseconds(largest, diff)
    }
}

impl From<Timestamp> for TimestampDifference {
    #[inline]
    fn from(ts: Timestamp) -> TimestampDifference {
        TimestampDifference::new(ts)
    }
}

impl From<Zoned> for TimestampDifference {
    #[inline]
    fn from(zdt: Zoned) -> TimestampDifference {
        TimestampDifference::new(Timestamp::from(zdt))
    }
}

impl<'a> From<&'a Zoned> for TimestampDifference {
    #[inline]
    fn from(zdt: &'a Zoned) -> TimestampDifference {
        TimestampDifference::from(Timestamp::from(zdt))
    }
}

impl From<(Unit, Timestamp)> for TimestampDifference {
    #[inline]
    fn from((largest, ts): (Unit, Timestamp)) -> TimestampDifference {
        TimestampDifference::from(ts).largest(largest)
    }
}

impl From<(Unit, Zoned)> for TimestampDifference {
    #[inline]
    fn from((largest, zdt): (Unit, Zoned)) -> TimestampDifference {
        TimestampDifference::from((largest, Timestamp::from(zdt)))
    }
}

impl<'a> From<(Unit, &'a Zoned)> for TimestampDifference {
    #[inline]
    fn from((largest, zdt): (Unit, &'a Zoned)) -> TimestampDifference {
        TimestampDifference::from((largest, Timestamp::from(zdt)))
    }
}

/// Options for [`Timestamp::round`].
///
/// This type provides a way to configure the rounding of a timestamp. In
/// particular, `Timestamp::round` accepts anything that implements the
/// `Into<TimestampRound>` trait. There are some trait implementations that
/// therefore make calling `Timestamp::round` in some common cases more
/// ergonomic:
///
/// * `From<Unit> for TimestampRound` will construct a rounding
/// configuration that rounds to the unit given. Specifically,
/// `TimestampRound::new().smallest(unit)`.
/// * `From<(Unit, i64)> for TimestampRound` is like the one above, but also
/// specifies the rounding increment for [`TimestampRound::increment`].
///
/// Note that in the default configuration, no rounding occurs.
///
/// # Example
///
/// This example shows how to round a timestamp to the nearest second:
///
/// ```
/// use jiff::{Timestamp, Unit};
///
/// let ts: Timestamp = "2024-06-20 16:24:59.5Z".parse()?;
/// assert_eq!(
///     ts.round(Unit::Second)?.to_string(),
///     // The second rounds up and causes minutes to increase.
///     "2024-06-20T16:25:00Z",
/// );
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// The above makes use of the fact that `Unit` implements
/// `Into<TimestampRound>`. If you want to change the rounding mode to, say,
/// truncation, then you'll need to construct a `TimestampRound` explicitly
/// since there are no convenience `Into` trait implementations for
/// [`RoundMode`].
///
/// ```
/// use jiff::{RoundMode, Timestamp, TimestampRound, Unit};
///
/// let ts: Timestamp = "2024-06-20 16:24:59.5Z".parse()?;
/// assert_eq!(
///     ts.round(
///         TimestampRound::new().smallest(Unit::Second).mode(RoundMode::Trunc),
///     )?.to_string(),
///     // The second just gets truncated as if it wasn't there.
///     "2024-06-20T16:24:59Z",
/// );
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Copy, Debug)]
pub struct TimestampRound {
    smallest: Unit,
    mode: RoundMode,
    increment: i64,
}

impl TimestampRound {
    /// Create a new default configuration for rounding a [`Timestamp`].
    #[inline]
    pub fn new() -> TimestampRound {
        TimestampRound {
            smallest: Unit::Nanosecond,
            mode: RoundMode::HalfExpand,
            increment: 1,
        }
    }

    /// Set the smallest units allowed in the timestamp returned after
    /// rounding.
    ///
    /// Any units below the smallest configured unit will be used, along with
    /// the rounding increment and rounding mode, to determine the value of the
    /// smallest unit. For example, when rounding `2024-06-20T03:25:30Z` to the
    /// nearest minute, the `30` second unit will result in rounding the minute
    /// unit of `25` up to `26` and zeroing out everything below minutes.
    ///
    /// This defaults to [`Unit::Nanosecond`].
    ///
    /// # Errors
    ///
    /// The smallest units must be no greater than [`Unit::Hour`].
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::{Timestamp, TimestampRound, Unit};
    ///
    /// let ts: Timestamp = "2024-06-20T03:25:30Z".parse()?;
    /// assert_eq!(
    ///     ts.round(TimestampRound::new().smallest(Unit::Minute))?.to_string(),
    ///     "2024-06-20T03:26:00Z",
    /// );
    /// // Or, utilize the `From<Unit> for TimestampRound` impl:
    /// assert_eq!(
    ///     ts.round(Unit::Minute)?.to_string(),
    ///     "2024-06-20T03:26:00Z",
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn smallest(self, unit: Unit) -> TimestampRound {
        TimestampRound { smallest: unit, ..self }
    }

    /// Set the rounding mode.
    ///
    /// This defaults to [`RoundMode::HalfExpand`], which rounds away from
    /// zero. It matches the kind of rounding you might have been taught in
    /// school.
    ///
    /// # Example
    ///
    /// This shows how to always round timestamps up towards positive infinity.
    ///
    /// ```
    /// use jiff::{RoundMode, Timestamp, TimestampRound, Unit};
    ///
    /// let ts: Timestamp = "2024-06-20 03:25:01Z".parse()?;
    /// assert_eq!(
    ///     ts.round(
    ///         TimestampRound::new()
    ///             .smallest(Unit::Minute)
    ///             .mode(RoundMode::Ceil),
    ///     )?.to_string(),
    ///     "2024-06-20T03:26:00Z",
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn mode(self, mode: RoundMode) -> TimestampRound {
        TimestampRound { mode, ..self }
    }

    /// Set the rounding increment for the smallest unit.
    ///
    /// The default value is `1`. Other values permit rounding the smallest
    /// unit to the nearest integer increment specified. For example, if the
    /// smallest unit is set to [`Unit::Minute`], then a rounding increment of
    /// `30` would result in rounding in increments of a half hour. That is,
    /// the only minute value that could result would be `0` or `30`.
    ///
    /// # Errors
    ///
    /// The rounding increment, when combined with the smallest unit (which
    /// defaults to [`Unit::Nanosecond`]), must divide evenly into `86,400`
    /// seconds (one 24-hour civil day). For example, increments of both
    /// 45 seconds and 15 minutes are allowed, but 7 seconds and 25 minutes are
    /// both not allowed.
    ///
    /// # Example
    ///
    /// This example shows how to round a timestamp to the nearest 10 minute
    /// increment.
    ///
    /// ```
    /// use jiff::{RoundMode, Timestamp, TimestampRound, Unit};
    ///
    /// let ts: Timestamp = "2024-06-20 03:24:59Z".parse()?;
    /// assert_eq!(
    ///     ts.round((Unit::Minute, 10))?.to_string(),
    ///     "2024-06-20T03:20:00Z",
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn increment(self, increment: i64) -> TimestampRound {
        TimestampRound { increment, ..self }
    }

    /// Does the actual rounding.
    pub(crate) fn round(
        &self,
        timestamp: Timestamp,
    ) -> Result<Timestamp, Error> {
        let increment =
            increment::for_timestamp(self.smallest, self.increment)?;
        let nanosecond = timestamp.as_nanosecond_ranged().without_bounds();
        let rounded = self.mode.round_by_unit_in_nanoseconds(
            nanosecond,
            self.smallest,
            increment,
        );
        let nanosecond = UnixNanoseconds::rfrom(rounded);
        Ok(Timestamp::from_nanosecond_ranged(nanosecond))
    }
}

impl Default for TimestampRound {
    #[inline]
    fn default() -> TimestampRound {
        TimestampRound::new()
    }
}

impl From<Unit> for TimestampRound {
    #[inline]
    fn from(unit: Unit) -> TimestampRound {
        TimestampRound::default().smallest(unit)
    }
}

impl From<(Unit, i64)> for TimestampRound {
    #[inline]
    fn from((unit, increment): (Unit, i64)) -> TimestampRound {
        TimestampRound::from(unit).increment(increment)
    }
}

#[cfg(test)]
mod tests {
    use alloc::string::ToString;

    use std::io::Cursor;

    use crate::{
        civil::{self, datetime},
        tz::Offset,
        ToSpan,
    };

    use super::*;

    fn mktime(seconds: i64, nanos: i32) -> Timestamp {
        Timestamp::new(seconds, nanos).unwrap()
    }

    fn mkdt(
        year: i16,
        month: i8,
        day: i8,
        hour: i8,
        minute: i8,
        second: i8,
        nano: i32,
    ) -> civil::DateTime {
        let date = civil::Date::new(year, month, day).unwrap();
        let time = civil::Time::new(hour, minute, second, nano).unwrap();
        civil::DateTime::from_parts(date, time)
    }

    #[test]
    fn to_datetime_specific_examples() {
        let tests = [
            ((UnixSeconds::MIN_REPR, 0), (-9999, 1, 2, 1, 59, 59, 0)),
            (
                (UnixSeconds::MIN_REPR + 1, -999_999_999),
                (-9999, 1, 2, 1, 59, 59, 1),
            ),
            ((-1, 1), (1969, 12, 31, 23, 59, 59, 1)),
            ((UnixSeconds::MAX_REPR, 0), (9999, 12, 30, 22, 0, 0, 0)),
            ((UnixSeconds::MAX_REPR - 1, 0), (9999, 12, 30, 21, 59, 59, 0)),
            (
                (UnixSeconds::MAX_REPR - 1, 999_999_999),
                (9999, 12, 30, 21, 59, 59, 999_999_999),
            ),
            (
                (UnixSeconds::MAX_REPR, 999_999_999),
                (9999, 12, 30, 22, 0, 0, 999_999_999),
            ),
            ((-2, -1), (1969, 12, 31, 23, 59, 57, 999_999_999)),
            ((-86398, -1), (1969, 12, 31, 0, 0, 1, 999_999_999)),
            ((-86399, -1), (1969, 12, 31, 0, 0, 0, 999_999_999)),
            ((-86400, -1), (1969, 12, 30, 23, 59, 59, 999_999_999)),
        ];
        for (t, dt) in tests {
            let timestamp = mktime(t.0, t.1);
            let datetime = mkdt(dt.0, dt.1, dt.2, dt.3, dt.4, dt.5, dt.6);
            assert_eq!(
                Offset::UTC.to_datetime(timestamp),
                datetime,
                "timestamp: {t:?}"
            );
            assert_eq!(
                timestamp,
                datetime.to_zoned(TimeZone::UTC).unwrap().timestamp(),
                "datetime: {datetime:?}"
            );
        }
    }

    #[test]
    fn to_datetime_many_seconds_in_some_days() {
        let days = [
            i64::from(t::UnixEpochDay::MIN_REPR),
            -1000,
            -5,
            23,
            2000,
            i64::from(t::UnixEpochDay::MAX_REPR),
        ];
        let seconds = [
            -86_400, -10, -9, -8, -7, -6, -5, -4, -3, -2, -1, 0, 1, 2, 3, 4,
            5, 6, 7, 8, 9, 10, 86_400,
        ];
        let nanos = [0, 1, 5, 999_999_999];
        for day in days {
            let midpoint = day * 86_400;
            for second in seconds {
                let second = midpoint + second;
                if !UnixSeconds::contains(second) {
                    continue;
                }
                for nano in nanos {
                    if second == UnixSeconds::MIN_REPR && nano != 0 {
                        continue;
                    }
                    let t = Timestamp::new(second, nano).unwrap();
                    let Ok(got) =
                        Offset::UTC.to_datetime(t).to_zoned(TimeZone::UTC)
                    else {
                        continue;
                    };
                    assert_eq!(t, got.timestamp());
                }
            }
        }
    }

    #[test]
    fn invalid_time() {
        assert!(Timestamp::new(UnixSeconds::MIN_REPR, -1).is_err());
        assert!(Timestamp::new(UnixSeconds::MIN_REPR, -999_999_999).is_err());
        // These are greater than the minimum and thus okay!
        assert!(Timestamp::new(UnixSeconds::MIN_REPR, 1).is_ok());
        assert!(Timestamp::new(UnixSeconds::MIN_REPR, 999_999_999).is_ok());
    }

    #[cfg(target_pointer_width = "64")]
    #[test]
    fn timestamp_size() {
        #[cfg(debug_assertions)]
        {
            assert_eq!(40, core::mem::size_of::<Timestamp>());
        }
        #[cfg(not(debug_assertions))]
        {
            assert_eq!(16, core::mem::size_of::<Timestamp>());
        }
    }

    #[test]
    fn nanosecond_roundtrip_boundaries() {
        let inst = Timestamp::MIN;
        let nanos = inst.as_nanosecond_ranged();
        assert_eq!(C(0), nanos % t::NANOS_PER_SECOND);
        let got = Timestamp::from_nanosecond_ranged(nanos);
        assert_eq!(inst, got);

        let inst = Timestamp::MAX;
        let nanos = inst.as_nanosecond_ranged();
        assert_eq!(
            FractionalNanosecond::MAX_SELF,
            nanos % t::NANOS_PER_SECOND
        );
        let got = Timestamp::from_nanosecond_ranged(nanos);
        assert_eq!(inst, got);
    }

    #[test]
    fn timestamp_saturating_add() {
        insta::assert_snapshot!(
            Timestamp::MIN.saturating_add(Span::new().days(1)).unwrap_err(),
            @"saturating `Timestamp` arithmetic requires only time units: operation can only be performed with units of hours or smaller, but found non-zero day units (operations on `Timestamp`, `tz::Offset` and `civil::Time` don't support calendar units in a `Span`)",
        )
    }

    #[test]
    fn timestamp_saturating_sub() {
        insta::assert_snapshot!(
            Timestamp::MAX.saturating_sub(Span::new().days(1)).unwrap_err(),
            @"saturating `Timestamp` arithmetic requires only time units: operation can only be performed with units of hours or smaller, but found non-zero day units (operations on `Timestamp`, `tz::Offset` and `civil::Time` don't support calendar units in a `Span`)",
        )
    }

    quickcheck::quickcheck! {
        fn prop_unix_seconds_roundtrip(t: Timestamp) -> quickcheck::TestResult {
            let dt = t.to_zoned(TimeZone::UTC).datetime();
            let Ok(got) = dt.to_zoned(TimeZone::UTC) else {
                return quickcheck::TestResult::discard();
            };
            quickcheck::TestResult::from_bool(t == got.timestamp())
        }

        fn prop_nanos_roundtrip_unix_ranged(t: Timestamp) -> bool {
            let nanos = t.as_nanosecond_ranged();
            let got = Timestamp::from_nanosecond_ranged(nanos);
            t == got
        }

        fn prop_nanos_roundtrip_unix(t: Timestamp) -> bool {
            let nanos = t.as_nanosecond();
            let got = Timestamp::from_nanosecond(nanos).unwrap();
            t == got
        }

        fn timestamp_constant_and_new_are_same1(t: Timestamp) -> bool {
            let got = Timestamp::constant(t.as_second(), t.subsec_nanosecond());
            t == got
        }

        fn timestamp_constant_and_new_are_same2(
            secs: i64,
            nanos: i32
        ) -> quickcheck::TestResult {
            let Ok(ts) = Timestamp::new(secs, nanos) else {
                return quickcheck::TestResult::discard();
            };
            let got = Timestamp::constant(secs, nanos);
            quickcheck::TestResult::from_bool(ts == got)
        }
    }

    /// A `serde` deserializer compatibility test.
    ///
    /// Serde YAML used to be unable to deserialize `jiff` types,
    /// as deserializing from bytes is not supported by the deserializer.
    ///
    /// - <https://github.com/BurntSushi/jiff/issues/138>
    /// - <https://github.com/BurntSushi/jiff/discussions/148>
    #[test]
    fn timestamp_deserialize_yaml() {
        let expected = datetime(2024, 10, 31, 16, 33, 53, 123456789)
            .to_zoned(TimeZone::UTC)
            .unwrap()
            .timestamp();

        let deserialized: Timestamp =
            serde_yaml::from_str("2024-10-31T16:33:53.123456789+00:00")
                .unwrap();

        assert_eq!(deserialized, expected);

        let deserialized: Timestamp = serde_yaml::from_slice(
            "2024-10-31T16:33:53.123456789+00:00".as_bytes(),
        )
        .unwrap();

        assert_eq!(deserialized, expected);

        let cursor = Cursor::new(b"2024-10-31T16:33:53.123456789+00:00");
        let deserialized: Timestamp = serde_yaml::from_reader(cursor).unwrap();

        assert_eq!(deserialized, expected);
    }

    #[test]
    fn timestamp_precision_loss() {
        let ts1: Timestamp =
            "2025-01-25T19:32:21.783444592+01:00".parse().unwrap();
        let span = 1.second();
        let ts2 = ts1 + span;
        assert_eq!(ts2.to_string(), "2025-01-25T18:32:22.783444592Z");
        assert_eq!(ts1, ts2 - span, "should be reversible");
    }
}
