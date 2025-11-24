use core::time::Duration as UnsignedDuration;

use crate::{
    civil::{Date, DateTime},
    duration::{Duration, SDuration},
    error::{err, Error, ErrorContext},
    fmt::{
        self,
        temporal::{self, DEFAULT_DATETIME_PARSER},
    },
    shared::util::itime::{ITime, ITimeNanosecond, ITimeSecond},
    util::{
        rangeint::{self, Composite, RFrom, RInto, TryRFrom},
        round::increment,
        t::{
            self, CivilDayNanosecond, CivilDaySecond, Hour, Microsecond,
            Millisecond, Minute, Nanosecond, Second, SubsecNanosecond, C,
        },
    },
    RoundMode, SignedDuration, Span, SpanRound, Unit, Zoned,
};

/// A representation of civil "wall clock" time.
///
/// Conceptually, a `Time` value corresponds to the typical hours and minutes
/// that you might see on a clock. This type also contains the second and
/// fractional subsecond (to nanosecond precision) associated with a time.
///
/// # Civil time
///
/// A `Time` value behaves as if it corresponds precisely to a single
/// nanosecond within a day, where all days have `86,400` seconds. That is,
/// any given `Time` value corresponds to a nanosecond in the inclusive range
/// `[0, 86399999999999]`, where `0` corresponds to `00:00:00.000000000`
/// ([`Time::MIN`]) and `86399999999999` corresponds to `23:59:59.999999999`
/// ([`Time::MAX`]). Moreover, in civil time, all hours have the same number of
/// minutes, all minutes have the same number of seconds and all seconds have
/// the same number of nanoseconds.
///
/// # Parsing and printing
///
/// The `Time` type provides convenient trait implementations of
/// [`std::str::FromStr`] and [`std::fmt::Display`]:
///
/// ```
/// use jiff::civil::Time;
///
/// let t: Time = "15:22:45".parse()?;
/// assert_eq!(t.to_string(), "15:22:45");
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// A civil `Time` can also be parsed from something that _contains_ a
/// time, but with perhaps other data (such as an offset or time zone):
///
/// ```
/// use jiff::civil::Time;
///
/// let t: Time = "2024-06-19T15:22:45-04[America/New_York]".parse()?;
/// assert_eq!(t.to_string(), "15:22:45");
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
/// value is midnight. i.e., `00:00:00.000000000`.
///
/// # Leap seconds
///
/// Jiff does not support leap seconds. Jiff behaves as if they don't exist.
/// The only exception is that if one parses a time with a second component
/// of `60`, then it is automatically constrained to `59`:
///
/// ```
/// use jiff::civil::{Time, time};
///
/// let t: Time = "23:59:60".parse()?;
/// assert_eq!(t, time(23, 59, 59, 0));
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Comparisons
///
/// The `Time` type provides both `Eq` and `Ord` trait implementations to
/// facilitate easy comparisons. When a time `t1` occurs before a time `t2`,
/// then `t1 < t2`. For example:
///
/// ```
/// use jiff::civil::time;
///
/// let t1 = time(7, 30, 1, 0);
/// let t2 = time(8, 10, 0, 0);
/// assert!(t1 < t2);
/// ```
///
/// As mentioned above, `Time` values are not associated with timezones, and
/// thus transitions such as DST are not taken into account when comparing
/// `Time` values.
///
/// # Arithmetic
///
/// This type provides routines for adding and subtracting spans of time, as
/// well as computing the span of time between two `Time` values.
///
/// For adding or subtracting spans of time, one can use any of the following
/// routines:
///
/// * [`Time::wrapping_add`] or [`Time::wrapping_sub`] for wrapping arithmetic.
/// * [`Time::checked_add`] or [`Time::checked_sub`] for checked arithmetic.
/// * [`Time::saturating_add`] or [`Time::saturating_sub`] for saturating
/// arithmetic.
///
/// Additionally, wrapping arithmetic is available via the `Add` and `Sub`
/// trait implementations:
///
/// ```
/// use jiff::{civil::time, ToSpan};
///
/// let t = time(20, 10, 1, 0);
/// let span = 1.hours().minutes(49).seconds(59);
/// assert_eq!(t + span, time(22, 0, 0, 0));
///
/// // Overflow will result in wrap-around unless using checked
/// // arithmetic explicitly.
/// let t = time(23, 59, 59, 999_999_999);
/// assert_eq!(time(0, 0, 0, 0), t + 1.nanoseconds());
/// ```
///
/// Wrapping arithmetic is used by default because it corresponds to how clocks
/// showing the time of day behave in practice.
///
/// One can compute the span of time between two times using either
/// [`Time::until`] or [`Time::since`]. It's also possible to subtract two
/// `Time` values directly via a `Sub` trait implementation:
///
/// ```
/// use jiff::{civil::time, ToSpan};
///
/// let time1 = time(22, 0, 0, 0);
/// let time2 = time(20, 10, 1, 0);
/// assert_eq!(
///     time1 - time2,
///     1.hours().minutes(49).seconds(59).fieldwise(),
/// );
/// ```
///
/// The `until` and `since` APIs are polymorphic and allow re-balancing and
/// rounding the span returned. For example, the default largest unit is hours
/// (as exemplified above), but we can ask for smaller units:
///
/// ```
/// use jiff::{civil::time, ToSpan, Unit};
///
/// let time1 = time(23, 30, 0, 0);
/// let time2 = time(7, 0, 0, 0);
/// assert_eq!(
///     time1.since((Unit::Minute, time2))?,
///     990.minutes().fieldwise(),
/// );
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// Or even round the span returned:
///
/// ```
/// use jiff::{civil::{TimeDifference, time}, RoundMode, ToSpan, Unit};
///
/// let time1 = time(23, 30, 0, 0);
/// let time2 = time(23, 35, 59, 0);
/// assert_eq!(
///     time1.until(
///         TimeDifference::new(time2).smallest(Unit::Minute),
///     )?,
///     5.minutes().fieldwise(),
/// );
/// // `TimeDifference` uses truncation as a rounding mode by default,
/// // but you can set the rounding mode to break ties away from zero:
/// assert_eq!(
///     time1.until(
///         TimeDifference::new(time2)
///             .smallest(Unit::Minute)
///             .mode(RoundMode::HalfExpand),
///     )?,
///     // Rounds up to 6 minutes.
///     6.minutes().fieldwise(),
/// );
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Rounding
///
/// A `Time` can be rounded based on a [`TimeRound`] configuration of smallest
/// units, rounding increment and rounding mode. Here's an example showing how
/// to round to the nearest third hour:
///
/// ```
/// use jiff::{civil::{TimeRound, time}, Unit};
///
/// let t = time(16, 27, 29, 999_999_999);
/// assert_eq!(
///     t.round(TimeRound::new().smallest(Unit::Hour).increment(3))?,
///     time(15, 0, 0, 0),
/// );
/// // Or alternatively, make use of the `From<(Unit, i64)> for TimeRound`
/// // trait implementation:
/// assert_eq!(t.round((Unit::Hour, 3))?, time(15, 0, 0, 0));
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// See [`Time::round`] for more details.
#[derive(Clone, Copy, Eq, Hash, PartialEq, PartialOrd, Ord)]
pub struct Time {
    hour: Hour,
    minute: Minute,
    second: Second,
    subsec_nanosecond: SubsecNanosecond,
}

impl Time {
    /// The minimum representable time value.
    ///
    /// This corresponds to `00:00:00.000000000`.
    pub const MIN: Time = Time::midnight();

    /// The maximum representable time value.
    ///
    /// This corresponds to `23:59:59.999999999`.
    pub const MAX: Time = Time::constant(23, 59, 59, 999_999_999);

    /// Creates a new `Time` value from its component hour, minute, second and
    /// fractional subsecond (up to nanosecond precision) values.
    ///
    /// To set the component values of a time after creating it, use
    /// [`TimeWith`] via [`Time::with`] to build a new [`Time`] from the fields
    /// of an existing time.
    ///
    /// # Errors
    ///
    /// This returns an error unless *all* of the following conditions are
    /// true:
    ///
    /// * `0 <= hour <= 23`
    /// * `0 <= minute <= 59`
    /// * `0 <= second <= 59`
    /// * `0 <= subsec_nanosecond <= 999,999,999`
    ///
    /// # Example
    ///
    /// This shows an example of a valid time:
    ///
    /// ```
    /// use jiff::civil::Time;
    ///
    /// let t = Time::new(21, 30, 5, 123_456_789).unwrap();
    /// assert_eq!(t.hour(), 21);
    /// assert_eq!(t.minute(), 30);
    /// assert_eq!(t.second(), 5);
    /// assert_eq!(t.millisecond(), 123);
    /// assert_eq!(t.microsecond(), 456);
    /// assert_eq!(t.nanosecond(), 789);
    /// ```
    ///
    /// This shows an example of an invalid time:
    ///
    /// ```
    /// use jiff::civil::Time;
    ///
    /// assert!(Time::new(21, 30, 60, 0).is_err());
    /// ```
    #[inline]
    pub fn new(
        hour: i8,
        minute: i8,
        second: i8,
        subsec_nanosecond: i32,
    ) -> Result<Time, Error> {
        let hour = Hour::try_new("hour", hour)?;
        let minute = Minute::try_new("minute", minute)?;
        let second = Second::try_new("second", second)?;
        let subsec_nanosecond =
            SubsecNanosecond::try_new("subsec_nanosecond", subsec_nanosecond)?;
        Ok(Time::new_ranged(hour, minute, second, subsec_nanosecond))
    }

    /// Creates a new `Time` value in a `const` context.
    ///
    /// # Panics
    ///
    /// This panics if the given values do not correspond to a valid `Time`.
    /// All of the following conditions must be true:
    ///
    /// * `0 <= hour <= 23`
    /// * `0 <= minute <= 59`
    /// * `0 <= second <= 59`
    /// * `0 <= subsec_nanosecond <= 999,999,999`
    ///
    /// Similarly, when used in a const context, invalid parameters will
    /// prevent your Rust program from compiling.
    ///
    /// # Example
    ///
    /// This shows an example of a valid time in a `const` context:
    ///
    /// ```
    /// use jiff::civil::Time;
    ///
    /// const BEDTIME: Time = Time::constant(21, 30, 5, 123_456_789);
    /// assert_eq!(BEDTIME.hour(), 21);
    /// assert_eq!(BEDTIME.minute(), 30);
    /// assert_eq!(BEDTIME.second(), 5);
    /// assert_eq!(BEDTIME.millisecond(), 123);
    /// assert_eq!(BEDTIME.microsecond(), 456);
    /// assert_eq!(BEDTIME.nanosecond(), 789);
    /// assert_eq!(BEDTIME.subsec_nanosecond(), 123_456_789);
    /// ```
    #[inline]
    pub const fn constant(
        hour: i8,
        minute: i8,
        second: i8,
        subsec_nanosecond: i32,
    ) -> Time {
        if !Hour::contains(hour) {
            panic!("invalid hour");
        }
        if !Minute::contains(minute) {
            panic!("invalid minute");
        }
        if !Second::contains(second) {
            panic!("invalid second");
        }
        if !SubsecNanosecond::contains(subsec_nanosecond) {
            panic!("invalid nanosecond");
        }
        let hour = Hour::new_unchecked(hour);
        let minute = Minute::new_unchecked(minute);
        let second = Second::new_unchecked(second);
        let subsec_nanosecond =
            SubsecNanosecond::new_unchecked(subsec_nanosecond);
        Time { hour, minute, second, subsec_nanosecond }
    }

    /// Returns the first moment of time in a day.
    ///
    /// Specifically, this has the `hour`, `minute`, `second`, `millisecond`,
    /// `microsecond` and `nanosecond` fields all set to `0`.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::civil::Time;
    ///
    /// let t = Time::midnight();
    /// assert_eq!(t.hour(), 0);
    /// assert_eq!(t.minute(), 0);
    /// assert_eq!(t.second(), 0);
    /// assert_eq!(t.millisecond(), 0);
    /// assert_eq!(t.microsecond(), 0);
    /// assert_eq!(t.nanosecond(), 0);
    /// ```
    #[inline]
    pub const fn midnight() -> Time {
        Time::constant(0, 0, 0, 0)
    }

    /// Create a builder for constructing a `Time` from the fields of this
    /// time.
    ///
    /// See the methods on [`TimeWith`] for the different ways one can set the
    /// fields of a new `Time`.
    ///
    /// # Example
    ///
    /// Unlike [`Date`], a [`Time`] is valid for all possible valid values
    /// of its fields. That is, there is no way for two valid field values
    /// to combine into an invalid `Time`. So, for `Time`, this builder does
    /// have as much of a benefit versus an API design with methods like
    /// `Time::with_hour` and `Time::with_minute`. Nevertheless, this builder
    /// permits settings multiple fields at the same time and performing only
    /// one validity check. Moreover, this provides a consistent API with other
    /// date and time types in this crate.
    ///
    /// ```
    /// use jiff::civil::time;
    ///
    /// let t1 = time(0, 0, 24, 0);
    /// let t2 = t1.with().hour(15).minute(30).millisecond(10).build()?;
    /// assert_eq!(t2, time(15, 30, 24, 10_000_000));
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn with(self) -> TimeWith {
        TimeWith::new(self)
    }

    /// Returns the "hour" component of this time.
    ///
    /// The value returned is guaranteed to be in the range `0..=23`.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::civil::time;
    ///
    /// let t = time(13, 35, 56, 123_456_789);
    /// assert_eq!(t.hour(), 13);
    /// ```
    #[inline]
    pub fn hour(self) -> i8 {
        self.hour_ranged().get()
    }

    /// Returns the "minute" component of this time.
    ///
    /// The value returned is guaranteed to be in the range `0..=59`.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::civil::time;
    ///
    /// let t = time(13, 35, 56, 123_456_789);
    /// assert_eq!(t.minute(), 35);
    /// ```
    #[inline]
    pub fn minute(self) -> i8 {
        self.minute_ranged().get()
    }

    /// Returns the "second" component of this time.
    ///
    /// The value returned is guaranteed to be in the range `0..=59`.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::civil::time;
    ///
    /// let t = time(13, 35, 56, 123_456_789);
    /// assert_eq!(t.second(), 56);
    /// ```
    #[inline]
    pub fn second(self) -> i8 {
        self.second_ranged().get()
    }

    /// Returns the "millisecond" component of this time.
    ///
    /// The value returned is guaranteed to be in the range `0..=999`.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::civil::time;
    ///
    /// let t = time(13, 35, 56, 123_456_789);
    /// assert_eq!(t.millisecond(), 123);
    /// ```
    #[inline]
    pub fn millisecond(self) -> i16 {
        self.millisecond_ranged().get()
    }

    /// Returns the "microsecond" component of this time.
    ///
    /// The value returned is guaranteed to be in the range `0..=999`.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::civil::time;
    ///
    /// let t = time(13, 35, 56, 123_456_789);
    /// assert_eq!(t.microsecond(), 456);
    /// ```
    #[inline]
    pub fn microsecond(self) -> i16 {
        self.microsecond_ranged().get()
    }

    /// Returns the "nanosecond" component of this time.
    ///
    /// The value returned is guaranteed to be in the range `0..=999`.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::civil::time;
    ///
    /// let t = time(13, 35, 56, 123_456_789);
    /// assert_eq!(t.nanosecond(), 789);
    /// ```
    #[inline]
    pub fn nanosecond(self) -> i16 {
        self.nanosecond_ranged().get()
    }

    /// Returns the fractional nanosecond for this `Time` value.
    ///
    /// If you want to set this value on `Time`, then use
    /// [`TimeWith::subsec_nanosecond`] via [`Time::with`].
    ///
    /// The value returned is guaranteed to be in the range `0..=999_999_999`.
    ///
    /// # Example
    ///
    /// This shows the relationship between constructing a `Time` value
    /// with routines like `with().millisecond()` and accessing the entire
    /// fractional part as a nanosecond:
    ///
    /// ```
    /// use jiff::civil::time;
    ///
    /// let t = time(15, 21, 35, 0).with().millisecond(987).build()?;
    /// assert_eq!(t.subsec_nanosecond(), 987_000_000);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: nanoseconds from a timestamp
    ///
    /// This shows how the fractional nanosecond part of a `Time` value
    /// manifests from a specific timestamp.
    ///
    /// ```
    /// use jiff::{civil, Timestamp};
    ///
    /// // 1,234 nanoseconds after the Unix epoch.
    /// let zdt = Timestamp::new(0, 1_234)?.in_tz("UTC")?;
    /// let time = zdt.datetime().time();
    /// assert_eq!(time.subsec_nanosecond(), 1_234);
    ///
    /// // 1,234 nanoseconds before the Unix epoch.
    /// let zdt = Timestamp::new(0, -1_234)?.in_tz("UTC")?;
    /// let time = zdt.datetime().time();
    /// // The nanosecond is equal to `1_000_000_000 - 1_234`.
    /// assert_eq!(time.subsec_nanosecond(), 999998766);
    /// // Looking at the other components of the time value might help.
    /// assert_eq!(time.hour(), 23);
    /// assert_eq!(time.minute(), 59);
    /// assert_eq!(time.second(), 59);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn subsec_nanosecond(self) -> i32 {
        self.subsec_nanosecond_ranged().get()
    }

    /// Given a [`Date`], this constructs a [`DateTime`] value with its time
    /// component equal to this time.
    ///
    /// This is a convenience function for [`DateTime::from_parts`].
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::civil::{DateTime, date, time};
    ///
    /// let d = date(2010, 3, 14);
    /// let t = time(2, 30, 0, 0);
    /// assert_eq!(DateTime::from_parts(d, t), t.to_datetime(d));
    /// ```
    #[inline]
    pub const fn to_datetime(self, date: Date) -> DateTime {
        DateTime::from_parts(date, self)
    }

    /// A convenience function for constructing a [`DateTime`] from this time
    /// on the date given by its components.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::civil::time;
    ///
    /// assert_eq!(
    ///     time(2, 30, 0, 0).on(2010, 3, 14).to_string(),
    ///     "2010-03-14T02:30:00",
    /// );
    /// ```
    ///
    /// One can also flip the order by making use of [`Date::at`]:
    ///
    /// ```
    /// use jiff::civil::date;
    ///
    /// assert_eq!(
    ///     date(2010, 3, 14).at(2, 30, 0, 0).to_string(),
    ///     "2010-03-14T02:30:00",
    /// );
    /// ```
    #[inline]
    pub const fn on(self, year: i16, month: i8, day: i8) -> DateTime {
        DateTime::from_parts(Date::constant(year, month, day), self)
    }

    /// Add the given span to this time and wrap around on overflow.
    ///
    /// This operation accepts three different duration types: [`Span`],
    /// [`SignedDuration`] or [`std::time::Duration`]. This is achieved via
    /// `From` trait implementations for the [`TimeArithmetic`] type.
    ///
    /// # Properties
    ///
    /// Given times `t1` and `t2`, and a span `s`, with `t2 = t1 + s`, it
    /// follows then that `t1 = t2 - s` for all values of `t1` and `s` that sum
    /// to `t2`.
    ///
    /// In short, subtracting the given span from the sum returned by this
    /// function is guaranteed to result in precisely the original time.
    ///
    /// # Example: available via addition operator
    ///
    /// This routine can be used via the `+` operator.
    ///
    /// ```
    /// use jiff::{civil::time, ToSpan};
    ///
    /// let t = time(20, 10, 1, 0);
    /// assert_eq!(
    ///     t + 1.hours().minutes(49).seconds(59),
    ///     time(22, 0, 0, 0),
    /// );
    /// ```
    ///
    /// # Example: add nanoseconds to a `Time`
    ///
    /// ```
    /// use jiff::{civil::time, ToSpan};
    ///
    /// let t = time(22, 35, 1, 0);
    /// assert_eq!(
    ///     time(22, 35, 3, 500_000_000),
    ///     t.wrapping_add(2_500_000_000i64.nanoseconds()),
    /// );
    /// ```
    ///
    /// # Example: add span with multiple units
    ///
    /// ```
    /// use jiff::{civil::time, ToSpan};
    ///
    /// let t = time(20, 10, 1, 0);
    /// assert_eq!(
    ///     time(22, 0, 0, 0),
    ///     t.wrapping_add(1.hours().minutes(49).seconds(59)),
    /// );
    /// ```
    ///
    /// # Example: adding an empty span is a no-op
    ///
    /// ```
    /// use jiff::{civil::time, Span};
    ///
    /// let t = time(20, 10, 1, 0);
    /// assert_eq!(t, t.wrapping_add(Span::new()));
    /// ```
    ///
    /// # Example: addition wraps on overflow
    ///
    /// ```
    /// use jiff::{civil::time, SignedDuration, ToSpan};
    ///
    /// let t = time(23, 59, 59, 999_999_999);
    /// assert_eq!(
    ///     t.wrapping_add(1.nanoseconds()),
    ///     time(0, 0, 0, 0),
    /// );
    /// assert_eq!(
    ///     t.wrapping_add(SignedDuration::from_nanos(1)),
    ///     time(0, 0, 0, 0),
    /// );
    /// assert_eq!(
    ///     t.wrapping_add(std::time::Duration::from_nanos(1)),
    ///     time(0, 0, 0, 0),
    /// );
    /// ```
    ///
    /// Similarly, if there are any non-zero units greater than hours in the
    /// given span, then they also result in wrapping behavior (i.e., they are
    /// ignored):
    ///
    /// ```
    /// use jiff::{civil::time, ToSpan};
    ///
    /// // doesn't matter what our time value is in this example
    /// let t = time(0, 0, 0, 0);
    /// assert_eq!(t, t.wrapping_add(1.days()));
    /// ```
    #[inline]
    pub fn wrapping_add<A: Into<TimeArithmetic>>(self, duration: A) -> Time {
        let duration: TimeArithmetic = duration.into();
        duration.wrapping_add(self)
    }

    #[inline]
    fn wrapping_add_span(self, span: Span) -> Time {
        let mut sum = self.to_nanosecond().without_bounds();
        sum = sum.wrapping_add(
            span.get_hours_ranged()
                .without_bounds()
                .wrapping_mul(t::NANOS_PER_HOUR),
        );
        sum = sum.wrapping_add(
            span.get_minutes_ranged()
                .without_bounds()
                .wrapping_mul(t::NANOS_PER_MINUTE),
        );
        sum = sum.wrapping_add(
            span.get_seconds_ranged()
                .without_bounds()
                .wrapping_mul(t::NANOS_PER_SECOND),
        );
        sum = sum.wrapping_add(
            span.get_milliseconds_ranged()
                .without_bounds()
                .wrapping_mul(t::NANOS_PER_MILLI),
        );
        sum = sum.wrapping_add(
            span.get_microseconds_ranged()
                .without_bounds()
                .wrapping_mul(t::NANOS_PER_MICRO),
        );
        sum = sum.wrapping_add(span.get_nanoseconds_ranged().without_bounds());
        let civil_day_nanosecond = sum % t::NANOS_PER_CIVIL_DAY;
        Time::from_nanosecond(civil_day_nanosecond.rinto())
    }

    #[inline]
    fn wrapping_add_signed_duration(self, duration: SignedDuration) -> Time {
        let start = t::NoUnits128::rfrom(self.to_nanosecond());
        let duration = t::NoUnits128::new_unchecked(duration.as_nanos());
        let end = start.wrapping_add(duration) % t::NANOS_PER_CIVIL_DAY;
        Time::from_nanosecond(end.rinto())
    }

    #[inline]
    fn wrapping_add_unsigned_duration(
        self,
        duration: UnsignedDuration,
    ) -> Time {
        let start = t::NoUnits128::rfrom(self.to_nanosecond());
        // OK because 96-bit unsigned integer can't overflow i128.
        let duration = i128::try_from(duration.as_nanos()).unwrap();
        let duration = t::NoUnits128::new_unchecked(duration);
        let duration = duration % t::NANOS_PER_CIVIL_DAY;
        let end = start.wrapping_add(duration) % t::NANOS_PER_CIVIL_DAY;
        Time::from_nanosecond(end.rinto())
    }

    /// This routine is identical to [`Time::wrapping_add`] with the duration
    /// negated.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::{civil::time, SignedDuration, ToSpan};
    ///
    /// let t = time(0, 0, 0, 0);
    /// assert_eq!(
    ///     t.wrapping_sub(1.nanoseconds()),
    ///     time(23, 59, 59, 999_999_999),
    /// );
    /// assert_eq!(
    ///     t.wrapping_sub(SignedDuration::from_nanos(1)),
    ///     time(23, 59, 59, 999_999_999),
    /// );
    /// assert_eq!(
    ///     t.wrapping_sub(std::time::Duration::from_nanos(1)),
    ///     time(23, 59, 59, 999_999_999),
    /// );
    ///
    /// assert_eq!(
    ///     t.wrapping_sub(SignedDuration::MIN),
    ///     time(15, 30, 8, 999_999_999),
    /// );
    /// assert_eq!(
    ///     t.wrapping_sub(SignedDuration::MAX),
    ///     time(8, 29, 52, 1),
    /// );
    /// assert_eq!(
    ///     t.wrapping_sub(std::time::Duration::MAX),
    ///     time(16, 59, 44, 1),
    /// );
    /// ```
    #[inline]
    pub fn wrapping_sub<A: Into<TimeArithmetic>>(self, duration: A) -> Time {
        let duration: TimeArithmetic = duration.into();
        duration.wrapping_sub(self)
    }

    #[inline]
    fn wrapping_sub_unsigned_duration(
        self,
        duration: UnsignedDuration,
    ) -> Time {
        let start = t::NoUnits128::rfrom(self.to_nanosecond());
        // OK because 96-bit unsigned integer can't overflow i128.
        let duration = i128::try_from(duration.as_nanos()).unwrap();
        let duration = t::NoUnits128::new_unchecked(duration);
        let end = start.wrapping_sub(duration) % t::NANOS_PER_CIVIL_DAY;
        Time::from_nanosecond(end.rinto())
    }

    /// Add the given span to this time and return an error if the result would
    /// otherwise overflow.
    ///
    /// This operation accepts three different duration types: [`Span`],
    /// [`SignedDuration`] or [`std::time::Duration`]. This is achieved via
    /// `From` trait implementations for the [`TimeArithmetic`] type.
    ///
    /// # Properties
    ///
    /// Given a time `t1` and a span `s`, and assuming `t2 = t1 + s` exists, it
    /// follows then that `t1 = t2 - s` for all values of `t1` and `s` that sum
    /// to a valid `t2`.
    ///
    /// In short, subtracting the given span from the sum returned by this
    /// function is guaranteed to result in precisely the original time.
    ///
    /// # Errors
    ///
    /// If the sum would overflow the minimum or maximum timestamp values, then
    /// an error is returned.
    ///
    /// If the given span has any non-zero units greater than hours, then an
    /// error is returned.
    ///
    /// # Example: add nanoseconds to a `Time`
    ///
    /// ```
    /// use jiff::{civil::time, ToSpan};
    ///
    /// let t = time(22, 35, 1, 0);
    /// assert_eq!(
    ///     time(22, 35, 3, 500_000_000),
    ///     t.checked_add(2_500_000_000i64.nanoseconds())?,
    /// );
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: add span with multiple units
    ///
    /// ```
    /// use jiff::{civil::time, ToSpan};
    ///
    /// let t = time(20, 10, 1, 0);
    /// assert_eq!(
    ///     time(22, 0, 0, 0),
    ///     t.checked_add(1.hours().minutes(49).seconds(59))?,
    /// );
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: adding an empty span is a no-op
    ///
    /// ```
    /// use jiff::{civil::time, Span};
    ///
    /// let t = time(20, 10, 1, 0);
    /// assert_eq!(t, t.checked_add(Span::new())?);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: error on overflow
    ///
    /// ```
    /// use jiff::{civil::time, ToSpan};
    ///
    /// // okay
    /// let t = time(23, 59, 59, 999_999_998);
    /// assert_eq!(
    ///     t.with().nanosecond(999).build()?,
    ///     t.checked_add(1.nanoseconds())?,
    /// );
    ///
    /// // not okay
    /// let t = time(23, 59, 59, 999_999_999);
    /// assert!(t.checked_add(1.nanoseconds()).is_err());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// Similarly, if there are any non-zero units greater than hours in the
    /// given span, then they also result in overflow (and thus an error):
    ///
    /// ```
    /// use jiff::{civil::time, ToSpan};
    ///
    /// // doesn't matter what our time value is in this example
    /// let t = time(0, 0, 0, 0);
    /// assert!(t.checked_add(1.days()).is_err());
    /// ```
    ///
    /// # Example: adding absolute durations
    ///
    /// This shows how to add signed and unsigned absolute durations to a
    /// `Time`. As with adding a `Span`, any overflow that occurs results in
    /// an error.
    ///
    /// ```
    /// use std::time::Duration;
    ///
    /// use jiff::{civil::time, SignedDuration};
    ///
    /// let t = time(23, 0, 0, 0);
    ///
    /// let dur = SignedDuration::from_mins(30);
    /// assert_eq!(t.checked_add(dur)?, time(23, 30, 0, 0));
    /// assert_eq!(t.checked_add(-dur)?, time(22, 30, 0, 0));
    ///
    /// let dur = Duration::new(0, 1);
    /// assert_eq!(t.checked_add(dur)?, time(23, 0, 0, 1));
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn checked_add<A: Into<TimeArithmetic>>(
        self,
        duration: A,
    ) -> Result<Time, Error> {
        let duration: TimeArithmetic = duration.into();
        duration.checked_add(self)
    }

    #[inline]
    fn checked_add_span(self, span: Span) -> Result<Time, Error> {
        let (time, span) = self.overflowing_add(span)?;
        if let Some(err) = span.smallest_non_time_non_zero_unit_error() {
            return Err(err);
        }
        Ok(time)
    }

    #[inline]
    fn checked_add_duration(
        self,
        duration: SignedDuration,
    ) -> Result<Time, Error> {
        let original = duration;
        let start = t::NoUnits128::rfrom(self.to_nanosecond());
        let duration = t::NoUnits128::new_unchecked(duration.as_nanos());
        // This can never fail because the maximum duration fits into a
        // 96-bit integer, and adding any 96-bit integer to any 64-bit
        // integer can never overflow a 128-bit integer.
        let end = start.try_checked_add("nanoseconds", duration).unwrap();
        let end = CivilDayNanosecond::try_rfrom("nanoseconds", end)
            .with_context(|| {
                err!(
                    "adding signed duration {duration:?}, equal to
                     {nanos} nanoseconds, to {time} overflowed",
                    duration = original,
                    nanos = original.as_nanos(),
                    time = self,
                )
            })?;
        Ok(Time::from_nanosecond(end))
    }

    /// This routine is identical to [`Time::checked_add`] with the duration
    /// negated.
    ///
    /// # Errors
    ///
    /// This has the same error conditions as [`Time::checked_add`].
    ///
    /// # Example
    ///
    /// ```
    /// use std::time::Duration;
    ///
    /// use jiff::{civil::time, SignedDuration, ToSpan};
    ///
    /// let t = time(22, 0, 0, 0);
    /// assert_eq!(
    ///     t.checked_sub(1.hours().minutes(49).seconds(59))?,
    ///     time(20, 10, 1, 0),
    /// );
    /// assert_eq!(
    ///     t.checked_sub(SignedDuration::from_hours(1))?,
    ///     time(21, 0, 0, 0),
    /// );
    /// assert_eq!(
    ///     t.checked_sub(Duration::from_secs(60 * 60))?,
    ///     time(21, 0, 0, 0),
    /// );
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn checked_sub<A: Into<TimeArithmetic>>(
        self,
        duration: A,
    ) -> Result<Time, Error> {
        let duration: TimeArithmetic = duration.into();
        duration.checked_neg().and_then(|ta| ta.checked_add(self))
    }

    /// This routine is identical to [`Time::checked_add`], except the
    /// result saturates on overflow. That is, instead of overflow, either
    /// [`Time::MIN`] or [`Time::MAX`] is returned.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::{civil::{Time, time}, SignedDuration, ToSpan};
    ///
    /// // no saturation
    /// let t = time(23, 59, 59, 999_999_998);
    /// assert_eq!(
    ///     t.with().nanosecond(999).build()?,
    ///     t.saturating_add(1.nanoseconds()),
    /// );
    ///
    /// // saturates
    /// let t = time(23, 59, 59, 999_999_999);
    /// assert_eq!(Time::MAX, t.saturating_add(1.nanoseconds()));
    /// assert_eq!(Time::MAX, t.saturating_add(SignedDuration::MAX));
    /// assert_eq!(Time::MIN, t.saturating_add(SignedDuration::MIN));
    /// assert_eq!(Time::MAX, t.saturating_add(std::time::Duration::MAX));
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// Similarly, if there are any non-zero units greater than hours in the
    /// given span, then they also result in overflow (and thus saturation):
    ///
    /// ```
    /// use jiff::{civil::{Time, time}, ToSpan};
    ///
    /// // doesn't matter what our time value is in this example
    /// let t = time(0, 0, 0, 0);
    /// assert_eq!(Time::MAX, t.saturating_add(1.days()));
    /// ```
    #[inline]
    pub fn saturating_add<A: Into<TimeArithmetic>>(self, duration: A) -> Time {
        let duration: TimeArithmetic = duration.into();
        self.checked_add(duration).unwrap_or_else(|_| {
            if duration.is_negative() {
                Time::MIN
            } else {
                Time::MAX
            }
        })
    }

    /// This routine is identical to [`Time::saturating_add`] with the duration
    /// negated.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::{civil::{Time, time}, SignedDuration, ToSpan};
    ///
    /// // no saturation
    /// let t = time(0, 0, 0, 1);
    /// assert_eq!(
    ///     t.with().nanosecond(0).build()?,
    ///     t.saturating_sub(1.nanoseconds()),
    /// );
    ///
    /// // saturates
    /// let t = time(0, 0, 0, 0);
    /// assert_eq!(Time::MIN, t.saturating_sub(1.nanoseconds()));
    /// assert_eq!(Time::MIN, t.saturating_sub(SignedDuration::MAX));
    /// assert_eq!(Time::MAX, t.saturating_sub(SignedDuration::MIN));
    /// assert_eq!(Time::MIN, t.saturating_sub(std::time::Duration::MAX));
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn saturating_sub<A: Into<TimeArithmetic>>(self, duration: A) -> Time {
        let duration: TimeArithmetic = duration.into();
        let Ok(duration) = duration.checked_neg() else { return Time::MIN };
        self.saturating_add(duration)
    }

    /// Adds the given span to the this time value, and returns the resulting
    /// time with any overflowing amount in the span returned.
    ///
    /// This isn't part of the public API because it seems a little odd, and
    /// I'm unsure of its use case. Overall this routine is a bit specialized
    /// and I'm not sure how generally useful it is. But it is used in crucial
    /// points in other parts of this crate.
    ///
    /// If you want this public, please file an issue and discuss your use
    /// case: https://github.com/BurntSushi/jiff/issues/new
    #[inline]
    pub(crate) fn overflowing_add(
        self,
        span: Span,
    ) -> Result<(Time, Span), Error> {
        if let Some(err) = span.smallest_non_time_non_zero_unit_error() {
            return Err(err);
        }
        let span_nanos = span.to_invariant_nanoseconds();
        let time_nanos = self.to_nanosecond();
        let sum = span_nanos + time_nanos;
        let days = t::SpanDays::try_new(
            "overflowing-days",
            sum.div_floor(t::NANOS_PER_CIVIL_DAY),
        )?;
        let time_nanos = sum.rem_floor(t::NANOS_PER_CIVIL_DAY);
        let time = Time::from_nanosecond(time_nanos.rinto());
        Ok((time, Span::new().days_ranged(days)))
    }

    /// Like `overflowing_add`, but with `SignedDuration`.
    ///
    /// This is used for datetime arithmetic, when adding to the time
    /// component overflows into days (always 24 hours).
    #[inline]
    pub(crate) fn overflowing_add_duration(
        self,
        duration: SignedDuration,
    ) -> Result<(Time, SignedDuration), Error> {
        if self.subsec_nanosecond() != 0 || duration.subsec_nanos() != 0 {
            return self.overflowing_add_duration_general(duration);
        }
        let start = t::NoUnits::rfrom(self.to_second());
        let duration_secs = t::NoUnits::new_unchecked(duration.as_secs());
        // This can fail if the duration is near its min or max values, and
        // thus we fall back to the more general (but slower) implementation
        // that uses 128-bit integers.
        let Some(sum) = start.checked_add(duration_secs) else {
            return self.overflowing_add_duration_general(duration);
        };
        let days = t::SpanDays::try_new(
            "overflowing-days",
            sum.div_floor(t::SECONDS_PER_CIVIL_DAY),
        )?;
        let time_secs = sum.rem_floor(t::SECONDS_PER_CIVIL_DAY);
        let time = Time::from_second(time_secs.rinto());
        // OK because of the constraint imposed by t::SpanDays.
        let hours = i64::from(days).checked_mul(24).unwrap();
        Ok((time, SignedDuration::from_hours(hours)))
    }

    /// Like `overflowing_add`, but with `SignedDuration`.
    ///
    /// This is used for datetime arithmetic, when adding to the time
    /// component overflows into days (always 24 hours).
    #[inline(never)]
    #[cold]
    fn overflowing_add_duration_general(
        self,
        duration: SignedDuration,
    ) -> Result<(Time, SignedDuration), Error> {
        let start = t::NoUnits128::rfrom(self.to_nanosecond());
        let duration = t::NoUnits96::new_unchecked(duration.as_nanos());
        // This can never fail because the maximum duration fits into a
        // 96-bit integer, and adding any 96-bit integer to any 64-bit
        // integer can never overflow a 128-bit integer.
        let sum = start.try_checked_add("nanoseconds", duration).unwrap();
        let days = t::SpanDays::try_new(
            "overflowing-days",
            sum.div_floor(t::NANOS_PER_CIVIL_DAY),
        )?;
        let time_nanos = sum.rem_floor(t::NANOS_PER_CIVIL_DAY);
        let time = Time::from_nanosecond(time_nanos.rinto());
        // OK because of the constraint imposed by t::SpanDays.
        let hours = i64::from(days).checked_mul(24).unwrap();
        Ok((time, SignedDuration::from_hours(hours)))
    }

    /// Returns a span representing the elapsed time from this time until
    /// the given `other` time.
    ///
    /// When `other` is earlier than this time, the span returned will be
    /// negative.
    ///
    /// Depending on the input provided, the span returned is rounded. It may
    /// also be balanced down to smaller units than the default. By default,
    /// the span returned is balanced such that the biggest possible unit is
    /// hours.
    ///
    /// This operation is configured by providing a [`TimeDifference`]
    /// value. Since this routine accepts anything that implements
    /// `Into<TimeDifference>`, once can pass a `Time` directly. One
    /// can also pass a `(Unit, Time)`, where `Unit` is treated as
    /// [`TimeDifference::largest`].
    ///
    /// # Properties
    ///
    /// As long as no rounding is requested, it is guaranteed that adding the
    /// span returned to the `other` time will always equal this time.
    ///
    /// # Errors
    ///
    /// An error can occur if `TimeDifference` is misconfigured. For example,
    /// if the smallest unit provided is bigger than the largest unit, or if
    /// the largest unit is bigger than [`Unit::Hour`].
    ///
    /// It is guaranteed that if one provides a time with the default
    /// [`TimeDifference`] configuration, then this routine will never fail.
    ///
    /// # Examples
    ///
    /// ```
    /// use jiff::{civil::time, ToSpan};
    ///
    /// let t1 = time(22, 35, 1, 0);
    /// let t2 = time(22, 35, 3, 500_000_000);
    /// assert_eq!(t1.until(t2)?, 2.seconds().milliseconds(500).fieldwise());
    /// // Flipping the dates is fine, but you'll get a negative span.
    /// assert_eq!(t2.until(t1)?, -2.seconds().milliseconds(500).fieldwise());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: using smaller units
    ///
    /// This example shows how to contract the span returned to smaller units.
    /// This makes use of a `From<(Unit, Time)> for TimeDifference`
    /// trait implementation.
    ///
    /// ```
    /// use jiff::{civil::time, Unit, ToSpan};
    ///
    /// let t1 = time(3, 24, 30, 3500);
    /// let t2 = time(15, 30, 0, 0);
    ///
    /// // The default limits spans to using "hours" as the biggest unit.
    /// let span = t1.until(t2)?;
    /// assert_eq!(span.to_string(), "PT12H5M29.9999965S");
    ///
    /// // But we can ask for smaller units, like capping the biggest unit
    /// // to minutes instead of hours.
    /// let span = t1.until((Unit::Minute, t2))?;
    /// assert_eq!(span.to_string(), "PT725M29.9999965S");
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn until<A: Into<TimeDifference>>(
        self,
        other: A,
    ) -> Result<Span, Error> {
        let args: TimeDifference = other.into();
        let span = args.until_with_largest_unit(self)?;
        if args.rounding_may_change_span() {
            span.round(args.round)
        } else {
            Ok(span)
        }
    }

    /// This routine is identical to [`Time::until`], but the order of the
    /// parameters is flipped.
    ///
    /// # Errors
    ///
    /// This has the same error conditions as [`Time::until`].
    ///
    /// # Example
    ///
    /// This routine can be used via the `-` operator. Since the default
    /// configuration is used and because a `Span` can represent the difference
    /// between any two possible times, it will never panic.
    ///
    /// ```
    /// use jiff::{civil::time, ToSpan};
    ///
    /// let earlier = time(1, 0, 0, 0);
    /// let later = time(22, 30, 0, 0);
    /// assert_eq!(later - earlier, 21.hours().minutes(30).fieldwise());
    /// ```
    #[inline]
    pub fn since<A: Into<TimeDifference>>(
        self,
        other: A,
    ) -> Result<Span, Error> {
        let args: TimeDifference = other.into();
        let span = -args.until_with_largest_unit(self)?;
        if args.rounding_may_change_span() {
            span.round(args.round)
        } else {
            Ok(span)
        }
    }

    /// Returns an absolute duration representing the elapsed time from this
    /// time until the given `other` time.
    ///
    /// When `other` occurs before this time, then the duration returned will
    /// be negative.
    ///
    /// Unlike [`Time::until`], this returns a duration corresponding to a
    /// 96-bit integer of nanoseconds between two times. In this case of
    /// computing durations between civil times where all days are assumed to
    /// be 24 hours long, the duration returned will always be less than 24
    /// hours.
    ///
    /// # Fallibility
    ///
    /// This routine never panics or returns an error. Since there are no
    /// configuration options that can be incorrectly provided, no error is
    /// possible when calling this routine. In contrast, [`Time::until`] can
    /// return an error in some cases due to misconfiguration. But like this
    /// routine, [`Time::until`] never panics or returns an error in its
    /// default configuration.
    ///
    /// # When should I use this versus [`Time::until`]?
    ///
    /// See the type documentation for [`SignedDuration`] for the section on
    /// when one should use [`Span`] and when one should use `SignedDuration`.
    /// In short, use `Span` (and therefore `Time::until`) unless you have a
    /// specific reason to do otherwise.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::{civil::time, SignedDuration};
    ///
    /// let t1 = time(22, 35, 1, 0);
    /// let t2 = time(22, 35, 3, 500_000_000);
    /// assert_eq!(t1.duration_until(t2), SignedDuration::new(2, 500_000_000));
    /// // Flipping the time is fine, but you'll get a negative duration.
    /// assert_eq!(t2.duration_until(t1), -SignedDuration::new(2, 500_000_000));
    /// ```
    ///
    /// # Example: difference with [`Time::until`]
    ///
    /// Since the difference between two civil times is always expressed in
    /// units of hours or smaller, and units of hours or smaller are always
    /// uniform, there is no "expressive" difference between this routine and
    /// `Time::until`. The only difference is that this routine returns a
    /// `SignedDuration` and `Time::until` returns a [`Span`]. Moreover, since
    /// the difference is always less than 24 hours, the return values can
    /// always be infallibly and losslessly converted between each other:
    ///
    /// ```
    /// use jiff::{civil::time, SignedDuration, Span};
    ///
    /// let t1 = time(22, 35, 1, 0);
    /// let t2 = time(22, 35, 3, 500_000_000);
    /// let dur = t1.duration_until(t2);
    /// // Guaranteed to never fail because the duration
    /// // between two civil times never exceeds the limits
    /// // of a `Span`.
    /// let span = Span::try_from(dur).unwrap();
    /// assert_eq!(span, Span::new().seconds(2).milliseconds(500).fieldwise());
    /// // Guaranteed to succeed and always return the original
    /// // duration because the units are always hours or smaller,
    /// // and thus uniform. This means a relative datetime is
    /// // never required to do this conversion.
    /// let dur = SignedDuration::try_from(span).unwrap();
    /// assert_eq!(dur, SignedDuration::new(2, 500_000_000));
    /// ```
    ///
    /// This conversion guarantee also applies to [`Time::until`] since it
    /// always returns a balanced span. That is, it never returns spans like
    /// `1 second 1000 milliseconds`. (Those cannot be losslessly converted to
    /// a `SignedDuration` since a `SignedDuration` is only represented as a
    /// single 96-bit integer of nanoseconds.)
    ///
    /// # Example: getting an unsigned duration
    ///
    /// If you're looking to find the duration between two times as a
    /// [`std::time::Duration`], you'll need to use this method to get a
    /// [`SignedDuration`] and then convert it to a `std::time::Duration`:
    ///
    /// ```
    /// use std::time::Duration;
    ///
    /// use jiff::{civil::time, SignedDuration, Span};
    ///
    /// let t1 = time(22, 35, 1, 0);
    /// let t2 = time(22, 35, 3, 500_000_000);
    /// let dur = Duration::try_from(t1.duration_until(t2))?;;
    /// assert_eq!(dur, Duration::new(2, 500_000_000));
    ///
    /// // Note that unsigned durations cannot represent all
    /// // possible differences! If the duration would be negative,
    /// // then the conversion fails:
    /// assert!(Duration::try_from(t2.duration_until(t1)).is_err());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn duration_until(self, other: Time) -> SignedDuration {
        SignedDuration::time_until(self, other)
    }

    /// This routine is identical to [`Time::duration_until`], but the order of
    /// the parameters is flipped.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::{civil::time, SignedDuration};
    ///
    /// let earlier = time(1, 0, 0, 0);
    /// let later = time(22, 30, 0, 0);
    /// assert_eq!(
    ///     later.duration_since(earlier),
    ///     SignedDuration::from_secs((21 * 60 * 60) + (30 * 60)),
    /// );
    /// ```
    #[inline]
    pub fn duration_since(self, other: Time) -> SignedDuration {
        SignedDuration::time_until(other, self)
    }

    /// Rounds this time according to the [`TimeRound`] configuration given.
    ///
    /// The principal option is [`TimeRound::smallest`], which allows one
    /// to configure the smallest units in the returned time. Rounding
    /// is what determines whether that unit should keep its current value
    /// or whether it should be incremented. Moreover, the amount it should
    /// be incremented can be configured via [`TimeRound::increment`].
    /// Finally, the rounding strategy itself can be configured via
    /// [`TimeRound::mode`].
    ///
    /// Note that this routine is generic and accepts anything that
    /// implements `Into<TimeRound>`. Some notable implementations are:
    ///
    /// * `From<Unit> for Round`, which will automatically create a
    /// `TimeRound::new().smallest(unit)` from the unit provided.
    /// * `From<(Unit, i64)> for Round`, which will automatically create a
    /// `TimeRound::new().smallest(unit).increment(number)` from the unit
    /// and increment provided.
    ///
    /// # Errors
    ///
    /// This returns an error if the smallest unit configured on the given
    /// [`TimeRound`] is bigger than hours.
    ///
    /// The rounding increment must divide evenly into the next highest unit
    /// after the smallest unit configured (and must not be equivalent to it).
    /// For example, if the smallest unit is [`Unit::Nanosecond`], then *some*
    /// of the valid values for the rounding increment are `1`, `2`, `4`, `5`,
    /// `100` and `500`. Namely, any integer that divides evenly into `1,000`
    /// nanoseconds since there are `1,000` nanoseconds in the next highest
    /// unit (microseconds).
    ///
    /// This can never fail because of overflow for any input. The only
    /// possible errors are "configuration" errors.
    ///
    /// # Example
    ///
    /// This is a basic example that demonstrates rounding a datetime to the
    /// nearest second. This also demonstrates calling this method with the
    /// smallest unit directly, instead of constructing a `TimeRound` manually.
    ///
    /// ```
    /// use jiff::{civil::time, Unit};
    ///
    /// let t = time(15, 45, 10, 123_456_789);
    /// assert_eq!(
    ///     t.round(Unit::Second)?,
    ///     time(15, 45, 10, 0),
    /// );
    /// let t = time(15, 45, 10, 500_000_001);
    /// assert_eq!(
    ///     t.round(Unit::Second)?,
    ///     time(15, 45, 11, 0),
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
    /// use jiff::{civil::{TimeRound, time}, RoundMode, Unit};
    ///
    /// let t = time(15, 45, 10, 999_999_999);
    /// assert_eq!(
    ///     t.round(Unit::Second)?,
    ///     time(15, 45, 11, 0),
    /// );
    /// // The default will round up to the next second for any fraction
    /// // greater than or equal to 0.5. But truncation will always round
    /// // toward zero.
    /// assert_eq!(
    ///     t.round(
    ///         TimeRound::new().smallest(Unit::Second).mode(RoundMode::Trunc),
    ///     )?,
    ///     time(15, 45, 10, 0),
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: rounding to the nearest 5 minute increment
    ///
    /// ```
    /// use jiff::{civil::time, Unit};
    ///
    /// // rounds down
    /// let t = time(15, 27, 29, 999_999_999);
    /// assert_eq!(t.round((Unit::Minute, 5))?, time(15, 25, 0, 0));
    /// // rounds up
    /// let t = time(15, 27, 30, 0);
    /// assert_eq!(t.round((Unit::Minute, 5))?, time(15, 30, 0, 0));
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: rounding wraps around on overflow
    ///
    /// This example demonstrates that it's possible for this operation to
    /// overflow, and as a result, have the time wrap around.
    ///
    /// ```
    /// use jiff::{civil::Time, Unit};
    ///
    /// let t = Time::MAX;
    /// assert_eq!(t.round(Unit::Hour)?, Time::MIN);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn round<R: Into<TimeRound>>(self, options: R) -> Result<Time, Error> {
        let options: TimeRound = options.into();
        options.round(self)
    }

    /// Return an iterator of periodic times determined by the given span.
    ///
    /// The given span may be negative, in which case, the iterator will move
    /// backwards through time. The iterator won't stop until either the span
    /// itself overflows, or it would otherwise exceed the minimum or maximum
    /// `Time` value.
    ///
    /// # Example: visiting every third hour
    ///
    /// This shows how to visit each third hour of a 24 hour time interval:
    ///
    /// ```
    /// use jiff::{civil::{Time, time}, ToSpan};
    ///
    /// let start = Time::MIN;
    /// let mut every_third_hour = vec![];
    /// for t in start.series(3.hours()) {
    ///     every_third_hour.push(t);
    /// }
    /// assert_eq!(every_third_hour, vec![
    ///     time(0, 0, 0, 0),
    ///     time(3, 0, 0, 0),
    ///     time(6, 0, 0, 0),
    ///     time(9, 0, 0, 0),
    ///     time(12, 0, 0, 0),
    ///     time(15, 0, 0, 0),
    ///     time(18, 0, 0, 0),
    ///     time(21, 0, 0, 0),
    /// ]);
    /// ```
    ///
    /// Or go backwards every 6.5 hours:
    ///
    /// ```
    /// use jiff::{civil::{Time, time}, ToSpan};
    ///
    /// let start = time(23, 0, 0, 0);
    /// let times: Vec<Time> = start.series(-6.hours().minutes(30)).collect();
    /// assert_eq!(times, vec![
    ///     time(23, 0, 0, 0),
    ///     time(16, 30, 0, 0),
    ///     time(10, 0, 0, 0),
    ///     time(3, 30, 0, 0),
    /// ]);
    /// ```
    #[inline]
    pub fn series(self, period: Span) -> TimeSeries {
        TimeSeries { start: self, period, step: 0 }
    }
}

/// Parsing and formatting using a "printf"-style API.
impl Time {
    /// Parses a civil time in `input` matching the given `format`.
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
    /// construct a civil time. For example, if an offset wasn't parsed.
    ///
    /// # Example
    ///
    /// This example shows how to parse a civil time:
    ///
    /// ```
    /// use jiff::civil::Time;
    ///
    /// // Parse with a 12-hour clock.
    /// let time = Time::strptime("%I:%M%P", "4:30pm")?;
    /// assert_eq!(time.to_string(), "16:30:00");
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn strptime(
        format: impl AsRef<[u8]>,
        input: impl AsRef<[u8]>,
    ) -> Result<Time, Error> {
        fmt::strtime::parse(format, input).and_then(|tm| tm.to_time())
    }

    /// Formats this civil time according to the given `format`.
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
    /// This example shows how to format a civil time in a 12 hour clock with
    /// no padding for the hour:
    ///
    /// ```
    /// use jiff::civil::time;
    ///
    /// let t = time(16, 30, 59, 0);
    /// let string = t.strftime("%-I:%M%P").to_string();
    /// assert_eq!(string, "4:30pm");
    /// ```
    ///
    /// Note that one can round a `Time` before formatting. For example, to
    /// round to the nearest minute:
    ///
    /// ```
    /// use jiff::{civil::time, Unit};
    ///
    /// let t = time(16, 30, 59, 0);
    /// let string = t.round(Unit::Minute)?.strftime("%-I:%M%P").to_string();
    /// assert_eq!(string, "4:31pm");
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
}

/// Crate internal APIs.
///
/// Many of these are mirrors of the public API, but on ranged types. These
/// are often much more convenient to use in composition with other parts of
/// the crate that also use ranged integer types. And this often permits the
/// routines to be infallible and (possibly) zero-cost.
impl Time {
    #[inline]
    pub(crate) fn new_ranged(
        hour: impl RInto<Hour>,
        minute: impl RInto<Minute>,
        second: impl RInto<Second>,
        subsec_nanosecond: impl RInto<SubsecNanosecond>,
    ) -> Time {
        Time {
            hour: hour.rinto(),
            minute: minute.rinto(),
            second: second.rinto(),
            subsec_nanosecond: subsec_nanosecond.rinto(),
        }
    }

    /// Set the fractional parts of this time to the given units via ranged
    /// types.
    #[inline]
    fn with_subsec_parts_ranged(
        self,
        millisecond: impl RInto<Millisecond>,
        microsecond: impl RInto<Microsecond>,
        nanosecond: impl RInto<Nanosecond>,
    ) -> Time {
        let millisecond = SubsecNanosecond::rfrom(millisecond.rinto());
        let microsecond = SubsecNanosecond::rfrom(microsecond.rinto());
        let nanosecond = SubsecNanosecond::rfrom(nanosecond.rinto());
        let mut subsec_nanosecond =
            millisecond * t::MICROS_PER_MILLI * t::NANOS_PER_MICRO;
        subsec_nanosecond += microsecond * t::NANOS_PER_MICRO;
        subsec_nanosecond += nanosecond;
        Time { subsec_nanosecond: subsec_nanosecond.rinto(), ..self }
    }

    #[inline]
    pub(crate) fn hour_ranged(self) -> Hour {
        self.hour
    }

    #[inline]
    pub(crate) fn minute_ranged(self) -> Minute {
        self.minute
    }

    #[inline]
    pub(crate) fn second_ranged(self) -> Second {
        self.second
    }

    #[inline]
    pub(crate) fn millisecond_ranged(self) -> Millisecond {
        let micros = self.subsec_nanosecond_ranged() / t::NANOS_PER_MICRO;
        let millis = micros / t::MICROS_PER_MILLI;
        millis.rinto()
    }

    #[inline]
    pub(crate) fn microsecond_ranged(self) -> Microsecond {
        let micros = self.subsec_nanosecond_ranged() / t::NANOS_PER_MICRO;
        let only_micros = micros % t::MICROS_PER_MILLI;
        only_micros.rinto()
    }

    #[inline]
    pub(crate) fn nanosecond_ranged(self) -> Nanosecond {
        let only_nanos = self.subsec_nanosecond_ranged() % t::NANOS_PER_MICRO;
        only_nanos.rinto()
    }

    #[inline]
    pub(crate) fn subsec_nanosecond_ranged(self) -> SubsecNanosecond {
        self.subsec_nanosecond
    }

    #[inline]
    pub(crate) fn until_nanoseconds(self, other: Time) -> t::SpanNanoseconds {
        let t1 = t::SpanNanoseconds::rfrom(self.to_nanosecond());
        let t2 = t::SpanNanoseconds::rfrom(other.to_nanosecond());
        t2 - t1
    }

    /// Converts this time value to the number of seconds that has elapsed
    /// since `00:00:00`. This completely ignores seconds. Callers should
    /// likely ensure that the fractional second component is zero.
    ///
    /// The maximum possible value that can be returned represents the time
    /// `23:59:59`.
    #[inline]
    pub(crate) fn to_second(&self) -> CivilDaySecond {
        self.to_itime().map(|x| x.to_second().second).to_rint()
    }

    /// Converts the given second to a time value. The second should correspond
    /// to the number of seconds that have elapsed since `00:00:00`. The
    /// fractional second component of the `Time` returned is always `0`.
    #[cfg_attr(feature = "perf-inline", inline(always))]
    pub(crate) fn from_second(second: CivilDaySecond) -> Time {
        let second = rangeint::composite!((second) => {
            ITimeSecond { second }
        });
        Time::from_itime(second.map(|x| x.to_time()))
    }

    /// Converts this time value to the number of nanoseconds that has elapsed
    /// since `00:00:00.000000000`.
    ///
    /// The maximum possible value that can be returned represents the time
    /// `23:59:59.999999999`.
    #[inline]
    pub(crate) fn to_nanosecond(&self) -> CivilDayNanosecond {
        self.to_itime().map(|x| x.to_nanosecond().nanosecond).to_rint()
    }

    /// Converts the given nanosecond to a time value. The nanosecond should
    /// correspond to the number of nanoseconds that have elapsed since
    /// `00:00:00.000000000`.
    #[cfg_attr(feature = "perf-inline", inline(always))]
    pub(crate) fn from_nanosecond(nanosecond: CivilDayNanosecond) -> Time {
        let nano = rangeint::composite!((nanosecond) => {
            ITimeNanosecond { nanosecond }
        });
        Time::from_itime(nano.map(|x| x.to_time()))
    }

    #[inline]
    pub(crate) fn to_itime(&self) -> Composite<ITime> {
        rangeint::composite! {
            (
                hour = self.hour,
                minute = self.minute,
                second = self.second,
                subsec_nanosecond = self.subsec_nanosecond,
            ) => {
                ITime { hour, minute, second, subsec_nanosecond }
            }
        }
    }

    #[inline]
    pub(crate) fn from_itime(itime: Composite<ITime>) -> Time {
        let (hour, minute, second, subsec_nanosecond) = rangeint::uncomposite!(
            itime,
            c => (c.hour, c.minute, c.second, c.subsec_nanosecond),
        );
        Time {
            hour: hour.to_rint(),
            minute: minute.to_rint(),
            second: second.to_rint(),
            subsec_nanosecond: subsec_nanosecond.to_rint(),
        }
    }

    #[inline]
    pub(crate) const fn to_itime_const(&self) -> ITime {
        ITime {
            hour: self.hour.get_unchecked(),
            minute: self.minute.get_unchecked(),
            second: self.second.get_unchecked(),
            subsec_nanosecond: self.subsec_nanosecond.get_unchecked(),
        }
    }
}

impl Default for Time {
    #[inline]
    fn default() -> Time {
        Time::midnight()
    }
}

/// Converts a `Time` into a human readable time string.
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
/// use jiff::civil::time;
///
/// let t = time(7, 0, 0, 123_000_000);
/// assert_eq!(format!("{t:.6?}"), "07:00:00.123000");
/// // Precision values greater than 9 are clamped to 9.
/// assert_eq!(format!("{t:.300?}"), "07:00:00.123000000");
/// // A precision of 0 implies the entire fractional
/// // component is always truncated.
/// assert_eq!(format!("{t:.0?}"), "07:00:00");
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
impl core::fmt::Debug for Time {
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        core::fmt::Display::fmt(self, f)
    }
}

/// Converts a `Time` into an ISO 8601 compliant string.
///
/// # Formatting options supported
///
/// * [`std::fmt::Formatter::precision`] can be set to control the precision
/// of the fractional second component. When not set, the minimum precision
/// required to losslessly render the value is used.
///
/// # Example
///
/// ```
/// use jiff::civil::time;
///
/// // No fractional seconds:
/// let t = time(7, 0, 0, 0);
/// assert_eq!(format!("{t}"), "07:00:00");
///
/// // With fractional seconds:
/// let t = time(7, 0, 0, 123_000_000);
/// assert_eq!(format!("{t}"), "07:00:00.123");
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Example: setting the precision
///
/// ```
/// use jiff::civil::time;
///
/// let t = time(7, 0, 0, 123_000_000);
/// assert_eq!(format!("{t:.6}"), "07:00:00.123000");
/// // Precision values greater than 9 are clamped to 9.
/// assert_eq!(format!("{t:.300}"), "07:00:00.123000000");
/// // A precision of 0 implies the entire fractional
/// // component is always truncated.
/// assert_eq!(format!("{t:.0}"), "07:00:00");
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
impl core::fmt::Display for Time {
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        use crate::fmt::StdFmtWrite;

        let precision =
            f.precision().map(|p| u8::try_from(p).unwrap_or(u8::MAX));
        temporal::DateTimePrinter::new()
            .precision(precision)
            .print_time(self, StdFmtWrite(f))
            .map_err(|_| core::fmt::Error)
    }
}

impl core::str::FromStr for Time {
    type Err = Error;

    #[inline]
    fn from_str(string: &str) -> Result<Time, Error> {
        DEFAULT_DATETIME_PARSER.parse_time(string)
    }
}

/// Adds a span of time. This uses wrapping arithmetic.
///
/// For checked arithmetic, see [`Time::checked_add`].
impl core::ops::Add<Span> for Time {
    type Output = Time;

    #[inline]
    fn add(self, rhs: Span) -> Time {
        self.wrapping_add(rhs)
    }
}

/// Adds a span of time in place. This uses wrapping arithmetic.
///
/// For checked arithmetic, see [`Time::checked_add`].
impl core::ops::AddAssign<Span> for Time {
    #[inline]
    fn add_assign(&mut self, rhs: Span) {
        *self = *self + rhs;
    }
}

/// Subtracts a span of time. This uses wrapping arithmetic.
///
/// For checked arithmetic, see [`Time::checked_sub`].
impl core::ops::Sub<Span> for Time {
    type Output = Time;

    #[inline]
    fn sub(self, rhs: Span) -> Time {
        self.wrapping_sub(rhs)
    }
}

/// Subtracts a span of time in place. This uses wrapping arithmetic.
///
/// For checked arithmetic, see [`Time::checked_sub`].
impl core::ops::SubAssign<Span> for Time {
    #[inline]
    fn sub_assign(&mut self, rhs: Span) {
        *self = *self - rhs;
    }
}

/// Computes the span of time between two times.
///
/// This will return a negative span when the time being subtracted is greater.
///
/// Since this uses the default configuration for calculating a span between
/// two times (no rounding and largest units is hours), this will never panic
/// or fail in any way.
///
/// To configure the largest unit or enable rounding, use [`Time::since`].
impl core::ops::Sub for Time {
    type Output = Span;

    #[inline]
    fn sub(self, rhs: Time) -> Span {
        self.since(rhs).expect("since never fails when given Time")
    }
}

/// Adds a signed duration of time. This uses wrapping arithmetic.
///
/// For checked arithmetic, see [`Time::checked_add`].
impl core::ops::Add<SignedDuration> for Time {
    type Output = Time;

    #[inline]
    fn add(self, rhs: SignedDuration) -> Time {
        self.wrapping_add(rhs)
    }
}

/// Adds a signed duration of time in place. This uses wrapping arithmetic.
///
/// For checked arithmetic, see [`Time::checked_add`].
impl core::ops::AddAssign<SignedDuration> for Time {
    #[inline]
    fn add_assign(&mut self, rhs: SignedDuration) {
        *self = *self + rhs;
    }
}

/// Subtracts a signed duration of time. This uses wrapping arithmetic.
///
/// For checked arithmetic, see [`Time::checked_sub`].
impl core::ops::Sub<SignedDuration> for Time {
    type Output = Time;

    #[inline]
    fn sub(self, rhs: SignedDuration) -> Time {
        self.wrapping_sub(rhs)
    }
}

/// Subtracts a signed duration of time in place. This uses wrapping arithmetic.
///
/// For checked arithmetic, see [`Time::checked_sub`].
impl core::ops::SubAssign<SignedDuration> for Time {
    #[inline]
    fn sub_assign(&mut self, rhs: SignedDuration) {
        *self = *self - rhs;
    }
}

/// Adds an unsigned duration of time. This uses wrapping arithmetic.
///
/// For checked arithmetic, see [`Time::checked_add`].
impl core::ops::Add<UnsignedDuration> for Time {
    type Output = Time;

    #[inline]
    fn add(self, rhs: UnsignedDuration) -> Time {
        self.wrapping_add(rhs)
    }
}

/// Adds an unsigned duration of time in place. This uses wrapping arithmetic.
///
/// For checked arithmetic, see [`Time::checked_add`].
impl core::ops::AddAssign<UnsignedDuration> for Time {
    #[inline]
    fn add_assign(&mut self, rhs: UnsignedDuration) {
        *self = *self + rhs;
    }
}

/// Subtracts an unsigned duration of time. This uses wrapping arithmetic.
///
/// For checked arithmetic, see [`Time::checked_sub`].
impl core::ops::Sub<UnsignedDuration> for Time {
    type Output = Time;

    #[inline]
    fn sub(self, rhs: UnsignedDuration) -> Time {
        self.wrapping_sub(rhs)
    }
}

/// Subtracts an unsigned duration of time in place. This uses wrapping
/// arithmetic.
///
/// For checked arithmetic, see [`Time::checked_sub`].
impl core::ops::SubAssign<UnsignedDuration> for Time {
    #[inline]
    fn sub_assign(&mut self, rhs: UnsignedDuration) {
        *self = *self - rhs;
    }
}

impl From<DateTime> for Time {
    #[inline]
    fn from(dt: DateTime) -> Time {
        dt.time()
    }
}

impl From<Zoned> for Time {
    #[inline]
    fn from(zdt: Zoned) -> Time {
        zdt.datetime().time()
    }
}

impl<'a> From<&'a Zoned> for Time {
    #[inline]
    fn from(zdt: &'a Zoned) -> Time {
        zdt.datetime().time()
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for Time {
    #[inline]
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for Time {
    #[inline]
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Time, D::Error> {
        use serde::de;

        struct TimeVisitor;

        impl<'de> de::Visitor<'de> for TimeVisitor {
            type Value = Time;

            fn expecting(
                &self,
                f: &mut core::fmt::Formatter,
            ) -> core::fmt::Result {
                f.write_str("a time string")
            }

            #[inline]
            fn visit_bytes<E: de::Error>(
                self,
                value: &[u8],
            ) -> Result<Time, E> {
                DEFAULT_DATETIME_PARSER
                    .parse_time(value)
                    .map_err(de::Error::custom)
            }

            #[inline]
            fn visit_str<E: de::Error>(self, value: &str) -> Result<Time, E> {
                self.visit_bytes(value.as_bytes())
            }
        }

        deserializer.deserialize_str(TimeVisitor)
    }
}

#[cfg(test)]
impl quickcheck::Arbitrary for Time {
    fn arbitrary(g: &mut quickcheck::Gen) -> Time {
        let hour = Hour::arbitrary(g);
        let minute = Minute::arbitrary(g);
        let second = Second::arbitrary(g);
        let subsec_nanosecond = SubsecNanosecond::arbitrary(g);
        Time::new_ranged(hour, minute, second, subsec_nanosecond)
    }

    fn shrink(&self) -> alloc::boxed::Box<dyn Iterator<Item = Time>> {
        alloc::boxed::Box::new(
            (
                self.hour_ranged(),
                self.minute_ranged(),
                self.second_ranged(),
                self.subsec_nanosecond_ranged(),
            )
                .shrink()
                .map(
                    |(hour, minute, second, subsec_nanosecond)| {
                        Time::new_ranged(
                            hour,
                            minute,
                            second,
                            subsec_nanosecond,
                        )
                    },
                ),
        )
    }
}

/// An iterator over periodic times, created by [`Time::series`].
///
/// It is exhausted when the next value would exceed a [`Span`] or [`Time`]
/// value.
#[derive(Clone, Debug)]
pub struct TimeSeries {
    start: Time,
    period: Span,
    step: i64,
}

impl Iterator for TimeSeries {
    type Item = Time;

    #[inline]
    fn next(&mut self) -> Option<Time> {
        let span = self.period.checked_mul(self.step).ok()?;
        self.step = self.step.checked_add(1)?;
        let time = self.start.checked_add(span).ok()?;
        Some(time)
    }
}

/// Options for [`Time::checked_add`] and [`Time::checked_sub`].
///
/// This type provides a way to ergonomically add one of a few different
/// duration types to a [`Time`].
///
/// The main way to construct values of this type is with its `From` trait
/// implementations:
///
/// * `From<Span> for TimeArithmetic` adds (or subtracts) the given span to the
/// receiver time.
/// * `From<SignedDuration> for TimeArithmetic` adds (or subtracts)
/// the given signed duration to the receiver time.
/// * `From<std::time::Duration> for TimeArithmetic` adds (or subtracts)
/// the given unsigned duration to the receiver time.
///
/// # Example
///
/// ```
/// use std::time::Duration;
///
/// use jiff::{civil::time, SignedDuration, ToSpan};
///
/// let t = time(0, 0, 0, 0);
/// assert_eq!(t.checked_add(2.hours())?, time(2, 0, 0, 0));
/// assert_eq!(t.checked_add(SignedDuration::from_hours(2))?, time(2, 0, 0, 0));
/// assert_eq!(t.checked_add(Duration::from_secs(2 * 60 * 60))?, time(2, 0, 0, 0));
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Copy, Debug)]
pub struct TimeArithmetic {
    duration: Duration,
}

impl TimeArithmetic {
    #[inline]
    fn wrapping_add(self, time: Time) -> Time {
        match self.duration {
            Duration::Span(span) => time.wrapping_add_span(span),
            Duration::Signed(sdur) => time.wrapping_add_signed_duration(sdur),
            Duration::Unsigned(udur) => {
                time.wrapping_add_unsigned_duration(udur)
            }
        }
    }

    #[inline]
    fn wrapping_sub(self, time: Time) -> Time {
        match self.duration {
            Duration::Span(span) => time.wrapping_add_span(span.negate()),
            Duration::Signed(sdur) => {
                if let Some(sdur) = sdur.checked_neg() {
                    time.wrapping_add_signed_duration(sdur)
                } else {
                    let udur = UnsignedDuration::new(
                        i64::MIN.unsigned_abs(),
                        sdur.subsec_nanos().unsigned_abs(),
                    );
                    time.wrapping_add_unsigned_duration(udur)
                }
            }
            Duration::Unsigned(udur) => {
                time.wrapping_sub_unsigned_duration(udur)
            }
        }
    }

    #[inline]
    fn checked_add(self, time: Time) -> Result<Time, Error> {
        match self.duration.to_signed()? {
            SDuration::Span(span) => time.checked_add_span(span),
            SDuration::Absolute(sdur) => time.checked_add_duration(sdur),
        }
    }

    #[inline]
    fn checked_neg(self) -> Result<TimeArithmetic, Error> {
        let duration = self.duration.checked_neg()?;
        Ok(TimeArithmetic { duration })
    }

    #[inline]
    fn is_negative(&self) -> bool {
        self.duration.is_negative()
    }
}

impl From<Span> for TimeArithmetic {
    fn from(span: Span) -> TimeArithmetic {
        let duration = Duration::from(span);
        TimeArithmetic { duration }
    }
}

impl From<SignedDuration> for TimeArithmetic {
    fn from(sdur: SignedDuration) -> TimeArithmetic {
        let duration = Duration::from(sdur);
        TimeArithmetic { duration }
    }
}

impl From<UnsignedDuration> for TimeArithmetic {
    fn from(udur: UnsignedDuration) -> TimeArithmetic {
        let duration = Duration::from(udur);
        TimeArithmetic { duration }
    }
}

impl<'a> From<&'a Span> for TimeArithmetic {
    fn from(span: &'a Span) -> TimeArithmetic {
        TimeArithmetic::from(*span)
    }
}

impl<'a> From<&'a SignedDuration> for TimeArithmetic {
    fn from(sdur: &'a SignedDuration) -> TimeArithmetic {
        TimeArithmetic::from(*sdur)
    }
}

impl<'a> From<&'a UnsignedDuration> for TimeArithmetic {
    fn from(udur: &'a UnsignedDuration) -> TimeArithmetic {
        TimeArithmetic::from(*udur)
    }
}

/// Options for [`Time::since`] and [`Time::until`].
///
/// This type provides a way to configure the calculation of spans between two
/// [`Time`] values. In particular, both `Time::since` and `Time::until` accept
/// anything that implements `Into<TimeDifference>`. There are a few key trait
/// implementations that make this convenient:
///
/// * `From<Time> for TimeDifference` will construct a configuration consisting
/// of just the time. So for example, `time1.until(time2)` will return the span
/// from `time1` to `time2`.
/// * `From<DateTime> for TimeDifference` will construct a configuration
/// consisting of just the time from the given datetime. So for example,
/// `time.since(datetime)` returns the span from `datetime.time()` to `time`.
/// * `From<(Unit, Time)>` is a convenient way to specify the largest units
/// that should be present on the span returned. By default, the largest units
/// are hours. Using this trait implementation is equivalent to
/// `TimeDifference::new(time).largest(unit)`.
/// * `From<(Unit, DateTime)>` is like the one above, but with the time from
/// the given datetime.
///
/// One can also provide a `TimeDifference` value directly. Doing so
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
/// This example shows how to round a span between two datetimes to the nearest
/// half-hour, with ties breaking away from zero.
///
/// ```
/// use jiff::{civil::{Time, TimeDifference}, RoundMode, ToSpan, Unit};
///
/// let t1 = "08:14:00.123456789".parse::<Time>()?;
/// let t2 = "15:00".parse::<Time>()?;
/// let span = t1.until(
///     TimeDifference::new(t2)
///         .smallest(Unit::Minute)
///         .mode(RoundMode::HalfExpand)
///         .increment(30),
/// )?;
/// assert_eq!(span, 7.hours().fieldwise());
///
/// // One less minute, and because of the HalfExpand mode, the span would
/// // get rounded down.
/// let t2 = "14:59".parse::<Time>()?;
/// let span = t1.until(
///     TimeDifference::new(t2)
///         .smallest(Unit::Minute)
///         .mode(RoundMode::HalfExpand)
///         .increment(30),
/// )?;
/// assert_eq!(span, 6.hours().minutes(30).fieldwise());
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Copy, Debug)]
pub struct TimeDifference {
    time: Time,
    round: SpanRound<'static>,
}

impl TimeDifference {
    /// Create a new default configuration for computing the span between
    /// the given time and some other time (specified as the receiver in
    /// [`Time::since`] or [`Time::until`]).
    #[inline]
    pub fn new(time: Time) -> TimeDifference {
        // We use truncation rounding by default since it seems that's
        // what is generally expected when computing the difference between
        // datetimes.
        //
        // See: https://github.com/tc39/proposal-temporal/issues/1122
        let round = SpanRound::new().mode(RoundMode::Trunc);
        TimeDifference { time, round }
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
    /// This shows how to round a span between two times to units no less than
    /// seconds.
    ///
    /// ```
    /// use jiff::{civil::{Time, TimeDifference}, RoundMode, ToSpan, Unit};
    ///
    /// let t1 = "08:14:02.5001".parse::<Time>()?;
    /// let t2 = "08:30:03.0001".parse::<Time>()?;
    /// let span = t1.until(
    ///     TimeDifference::new(t2)
    ///         .smallest(Unit::Second)
    ///         .mode(RoundMode::HalfExpand),
    /// )?;
    /// assert_eq!(span, 16.minutes().seconds(1).fieldwise());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn smallest(self, unit: Unit) -> TimeDifference {
        TimeDifference { round: self.round.smallest(unit), ..self }
    }

    /// Set the largest units allowed in the span returned.
    ///
    /// When a largest unit is not specified, computing a span between times
    /// behaves as if it were set to [`Unit::Hour`].
    ///
    /// # Errors
    ///
    /// The largest units, when set, must be at least as big as the smallest
    /// units (which defaults to [`Unit::Nanosecond`]). If this is violated,
    /// then computing a span with this configuration will result in an error.
    ///
    /// # Example
    ///
    /// This shows how to round a span between two times to units no
    /// bigger than seconds.
    ///
    /// ```
    /// use jiff::{civil::{Time, TimeDifference}, ToSpan, Unit};
    ///
    /// let t1 = "08:14".parse::<Time>()?;
    /// let t2 = "08:30".parse::<Time>()?;
    /// let span = t1.until(TimeDifference::new(t2).largest(Unit::Second))?;
    /// assert_eq!(span, 960.seconds().fieldwise());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn largest(self, unit: Unit) -> TimeDifference {
        TimeDifference { round: self.round.largest(unit), ..self }
    }

    /// Set the rounding mode.
    ///
    /// This defaults to [`RoundMode::Trunc`] since it's plausible that
    /// rounding "up" in the context of computing the span between two times
    /// could be surprising in a number of cases. The [`RoundMode::HalfExpand`]
    /// mode corresponds to typical rounding you might have learned about in
    /// school. But a variety of other rounding modes exist.
    ///
    /// # Example
    ///
    /// This shows how to always round "up" towards positive infinity.
    ///
    /// ```
    /// use jiff::{civil::{Time, TimeDifference}, RoundMode, ToSpan, Unit};
    ///
    /// let t1 = "08:10".parse::<Time>()?;
    /// let t2 = "08:11".parse::<Time>()?;
    /// let span = t1.until(
    ///     TimeDifference::new(t2)
    ///         .smallest(Unit::Hour)
    ///         .mode(RoundMode::Ceil),
    /// )?;
    /// // Only one minute elapsed, but we asked to always round up!
    /// assert_eq!(span, 1.hour().fieldwise());
    ///
    /// // Since `Ceil` always rounds toward positive infinity, the behavior
    /// // flips for a negative span.
    /// let span = t1.since(
    ///     TimeDifference::new(t2)
    ///         .smallest(Unit::Hour)
    ///         .mode(RoundMode::Ceil),
    /// )?;
    /// assert_eq!(span, 0.hour().fieldwise());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn mode(self, mode: RoundMode) -> TimeDifference {
        TimeDifference { round: self.round.mode(mode), ..self }
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
    /// This shows how to round the span between two times to the nearest 5
    /// minute increment.
    ///
    /// ```
    /// use jiff::{civil::{Time, TimeDifference}, RoundMode, ToSpan, Unit};
    ///
    /// let t1 = "08:19".parse::<Time>()?;
    /// let t2 = "12:52".parse::<Time>()?;
    /// let span = t1.until(
    ///     TimeDifference::new(t2)
    ///         .smallest(Unit::Minute)
    ///         .increment(5)
    ///         .mode(RoundMode::HalfExpand),
    /// )?;
    /// assert_eq!(span, 4.hour().minutes(35).fieldwise());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn increment(self, increment: i64) -> TimeDifference {
        TimeDifference { round: self.round.increment(increment), ..self }
    }

    /// Returns true if and only if this configuration could change the span
    /// via rounding.
    #[inline]
    fn rounding_may_change_span(&self) -> bool {
        self.round.rounding_may_change_span_ignore_largest()
    }

    /// Returns the span of time from `t1` to the time in this configuration.
    /// The biggest units allowed are determined by the `smallest` and
    /// `largest` settings, but defaults to `Unit::Hour`.
    #[inline]
    fn until_with_largest_unit(&self, t1: Time) -> Result<Span, Error> {
        let t2 = self.time;
        if t1 == t2 {
            return Ok(Span::new());
        }
        let largest = self.round.get_largest().unwrap_or(Unit::Hour);
        if largest > Unit::Hour {
            return Err(err!(
                "rounding the span between two times must use hours \
                 or smaller for its units, but found {units}",
                units = largest.plural(),
            ));
        }
        let start = t1.to_nanosecond();
        let end = t2.to_nanosecond();
        let span =
            Span::from_invariant_nanoseconds(largest, (end - start).rinto())
                .expect("difference in civil times is always in bounds");
        Ok(span)
    }
}

impl From<Time> for TimeDifference {
    #[inline]
    fn from(time: Time) -> TimeDifference {
        TimeDifference::new(time)
    }
}

impl From<DateTime> for TimeDifference {
    #[inline]
    fn from(dt: DateTime) -> TimeDifference {
        TimeDifference::from(Time::from(dt))
    }
}

impl From<Zoned> for TimeDifference {
    #[inline]
    fn from(zdt: Zoned) -> TimeDifference {
        TimeDifference::from(Time::from(zdt))
    }
}

impl<'a> From<&'a Zoned> for TimeDifference {
    #[inline]
    fn from(zdt: &'a Zoned) -> TimeDifference {
        TimeDifference::from(zdt.datetime())
    }
}

impl From<(Unit, Time)> for TimeDifference {
    #[inline]
    fn from((largest, time): (Unit, Time)) -> TimeDifference {
        TimeDifference::from(time).largest(largest)
    }
}

impl From<(Unit, DateTime)> for TimeDifference {
    #[inline]
    fn from((largest, dt): (Unit, DateTime)) -> TimeDifference {
        TimeDifference::from((largest, Time::from(dt)))
    }
}

impl From<(Unit, Zoned)> for TimeDifference {
    #[inline]
    fn from((largest, zdt): (Unit, Zoned)) -> TimeDifference {
        TimeDifference::from((largest, Time::from(zdt)))
    }
}

impl<'a> From<(Unit, &'a Zoned)> for TimeDifference {
    #[inline]
    fn from((largest, zdt): (Unit, &'a Zoned)) -> TimeDifference {
        TimeDifference::from((largest, zdt.datetime()))
    }
}

/// Options for [`Time::round`].
///
/// This type provides a way to configure the rounding of a civil time.
/// In particular, `Time::round` accepts anything that implements the
/// `Into<TimeRound>` trait. There are some trait implementations that
/// therefore make calling `Time::round` in some common cases more ergonomic:
///
/// * `From<Unit> for TimeRound` will construct a rounding configuration that
/// rounds to the unit given. Specifically, `TimeRound::new().smallest(unit)`.
/// * `From<(Unit, i64)> for TimeRound` is like the one above, but also
/// specifies the rounding increment for [`TimeRound::increment`].
///
/// Note that in the default configuration, no rounding occurs.
///
/// # Example
///
/// This example shows how to round a time to the nearest second:
///
/// ```
/// use jiff::{civil::{Time, time}, Unit};
///
/// let t: Time = "16:24:59.5".parse()?;
/// assert_eq!(
///     t.round(Unit::Second)?,
///     // The second rounds up and causes minutes to increase.
///     time(16, 25, 0, 0),
/// );
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// The above makes use of the fact that `Unit` implements
/// `Into<TimeRound>`. If you want to change the rounding mode to, say,
/// truncation, then you'll need to construct a `TimeRound` explicitly
/// since there are no convenience `Into` trait implementations for
/// [`RoundMode`].
///
/// ```
/// use jiff::{civil::{Time, TimeRound, time}, RoundMode, Unit};
///
/// let t: Time = "2024-06-20 16:24:59.5".parse()?;
/// assert_eq!(
///     t.round(
///         TimeRound::new().smallest(Unit::Second).mode(RoundMode::Trunc),
///     )?,
///     // The second just gets truncated as if it wasn't there.
///     time(16, 24, 59, 0),
/// );
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Copy, Debug)]
pub struct TimeRound {
    smallest: Unit,
    mode: RoundMode,
    increment: i64,
}

impl TimeRound {
    /// Create a new default configuration for rounding a [`Time`].
    #[inline]
    pub fn new() -> TimeRound {
        TimeRound {
            smallest: Unit::Nanosecond,
            mode: RoundMode::HalfExpand,
            increment: 1,
        }
    }

    /// Set the smallest units allowed in the time returned after rounding.
    ///
    /// Any units below the smallest configured unit will be used, along with
    /// the rounding increment and rounding mode, to determine the value of the
    /// smallest unit. For example, when rounding `03:25:30` to the
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
    /// use jiff::{civil::{TimeRound, time}, Unit};
    ///
    /// let t = time(3, 25, 30, 0);
    /// assert_eq!(
    ///     t.round(TimeRound::new().smallest(Unit::Minute))?,
    ///     time(3, 26, 0, 0),
    /// );
    /// // Or, utilize the `From<Unit> for TimeRound` impl:
    /// assert_eq!(t.round(Unit::Minute)?, time(3, 26, 0, 0));
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn smallest(self, unit: Unit) -> TimeRound {
        TimeRound { smallest: unit, ..self }
    }

    /// Set the rounding mode.
    ///
    /// This defaults to [`RoundMode::HalfExpand`], which rounds away from
    /// zero. It matches the kind of rounding you might have been taught in
    /// school.
    ///
    /// # Example
    ///
    /// This shows how to always round times up towards positive infinity.
    ///
    /// ```
    /// use jiff::{civil::{Time, TimeRound, time}, RoundMode, Unit};
    ///
    /// let t: Time = "03:25:01".parse()?;
    /// assert_eq!(
    ///     t.round(
    ///         TimeRound::new()
    ///             .smallest(Unit::Minute)
    ///             .mode(RoundMode::Ceil),
    ///     )?,
    ///     time(3, 26, 0, 0),
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn mode(self, mode: RoundMode) -> TimeRound {
        TimeRound { mode, ..self }
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
    /// The rounding increment must divide evenly into the
    /// next highest unit above the smallest unit set. The rounding increment
    /// must also not be equal to the next highest unit. For example, if the
    /// smallest unit is [`Unit::Nanosecond`], then *some* of the valid values
    /// for the rounding increment are `1`, `2`, `4`, `5`, `100` and `500`.
    /// Namely, any integer that divides evenly into `1,000` nanoseconds since
    /// there are `1,000` nanoseconds in the next highest unit (microseconds).
    ///
    /// # Example
    ///
    /// This example shows how to round a time to the nearest 10 minute
    /// increment.
    ///
    /// ```
    /// use jiff::{civil::{Time, TimeRound, time}, RoundMode, Unit};
    ///
    /// let t: Time = "03:24:59".parse()?;
    /// assert_eq!(t.round((Unit::Minute, 10))?, time(3, 20, 0, 0));
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn increment(self, increment: i64) -> TimeRound {
        TimeRound { increment, ..self }
    }

    /// Does the actual rounding.
    pub(crate) fn round(&self, t: Time) -> Result<Time, Error> {
        let increment = increment::for_time(self.smallest, self.increment)?;
        let nanos = t.to_nanosecond();
        let rounded = self.mode.round_by_unit_in_nanoseconds(
            nanos,
            self.smallest,
            increment,
        );
        let limit =
            t::NoUnits128::rfrom(t::CivilDayNanosecond::MAX_SELF) + C(1);
        Ok(Time::from_nanosecond((rounded % limit).rinto()))
    }
}

impl Default for TimeRound {
    #[inline]
    fn default() -> TimeRound {
        TimeRound::new()
    }
}

impl From<Unit> for TimeRound {
    #[inline]
    fn from(unit: Unit) -> TimeRound {
        TimeRound::default().smallest(unit)
    }
}

impl From<(Unit, i64)> for TimeRound {
    #[inline]
    fn from((unit, increment): (Unit, i64)) -> TimeRound {
        TimeRound::from(unit).increment(increment)
    }
}

/// A builder for setting the fields on a [`Time`].
///
/// This builder is constructed via [`Time::with`].
///
/// # Example
///
/// Unlike [`Date`], a [`Time`] is valid for all possible valid values of its
/// fields. That is, there is no way for two valid field values to combine
/// into an invalid `Time`. So, for `Time`, this builder does have as much of
/// a benefit versus an API design with methods like `Time::with_hour` and
/// `Time::with_minute`. Nevertheless, this builder permits settings multiple
/// fields at the same time and performing only one validity check. Moreover,
/// this provides a consistent API with other date and time types in this
/// crate.
///
/// ```
/// use jiff::civil::time;
///
/// let t1 = time(0, 0, 24, 0);
/// let t2 = t1.with().hour(15).minute(30).millisecond(10).build()?;
/// assert_eq!(t2, time(15, 30, 24, 10_000_000));
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Copy, Debug)]
pub struct TimeWith {
    original: Time,
    hour: Option<i8>,
    minute: Option<i8>,
    second: Option<i8>,
    millisecond: Option<i16>,
    microsecond: Option<i16>,
    nanosecond: Option<i16>,
    subsec_nanosecond: Option<i32>,
}

impl TimeWith {
    #[inline]
    fn new(original: Time) -> TimeWith {
        TimeWith {
            original,
            hour: None,
            minute: None,
            second: None,
            millisecond: None,
            microsecond: None,
            nanosecond: None,
            subsec_nanosecond: None,
        }
    }

    /// Create a new `Time` from the fields set on this configuration.
    ///
    /// An error occurs when the fields combine to an invalid time. This only
    /// occurs when at least one field has an invalid value, or if at least
    /// one of `millisecond`, `microsecond` or `nanosecond` is set _and_
    /// `subsec_nanosecond` is set. Otherwise, if all fields are valid, then
    /// the entire `Time` is guaranteed to be valid.
    ///
    /// For any fields not set on this configuration, the values are taken from
    /// the [`Time`] that originally created this configuration. When no values
    /// are set, this routine is guaranteed to succeed and will always return
    /// the original time without modification.
    ///
    /// # Example
    ///
    /// This creates a time but with its fractional nanosecond component
    /// stripped:
    ///
    /// ```
    /// use jiff::civil::time;
    ///
    /// let t = time(14, 27, 30, 123_456_789);
    /// assert_eq!(t.with().subsec_nanosecond(0).build()?, time(14, 27, 30, 0));
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: error for invalid time
    ///
    /// ```
    /// use jiff::civil::time;
    ///
    /// let t = time(14, 27, 30, 0);
    /// assert!(t.with().hour(24).build().is_err());
    /// ```
    ///
    /// # Example: error for ambiguous sub-second value
    ///
    /// ```
    /// use jiff::civil::time;
    ///
    /// let t = time(14, 27, 30, 123_456_789);
    /// // Setting both the individual sub-second fields and the entire
    /// // fractional component could lead to a misleading configuration. So
    /// // if it's done, it results in an error in all cases. Callers must
    /// // choose one or the other.
    /// assert!(t.with().microsecond(1).subsec_nanosecond(0).build().is_err());
    /// ```
    #[inline]
    pub fn build(self) -> Result<Time, Error> {
        let hour = match self.hour {
            None => self.original.hour_ranged(),
            Some(hour) => Hour::try_new("hour", hour)?,
        };
        let minute = match self.minute {
            None => self.original.minute_ranged(),
            Some(minute) => Minute::try_new("minute", minute)?,
        };
        let second = match self.second {
            None => self.original.second_ranged(),
            Some(second) => Second::try_new("second", second)?,
        };
        let millisecond = match self.millisecond {
            None => self.original.millisecond_ranged(),
            Some(millisecond) => {
                Millisecond::try_new("millisecond", millisecond)?
            }
        };
        let microsecond = match self.microsecond {
            None => self.original.microsecond_ranged(),
            Some(microsecond) => {
                Microsecond::try_new("microsecond", microsecond)?
            }
        };
        let nanosecond = match self.nanosecond {
            None => self.original.nanosecond_ranged(),
            Some(nanosecond) => Nanosecond::try_new("nanosecond", nanosecond)?,
        };
        let subsec_nanosecond = match self.subsec_nanosecond {
            None => self.original.subsec_nanosecond_ranged(),
            Some(subsec_nanosecond) => {
                if self.millisecond.is_some() {
                    return Err(err!(
                        "cannot set both TimeWith::millisecond \
                         and TimeWith::subsec_nanosecond",
                    ));
                }
                if self.microsecond.is_some() {
                    return Err(err!(
                        "cannot set both TimeWith::microsecond \
                         and TimeWith::subsec_nanosecond",
                    ));
                }
                if self.nanosecond.is_some() {
                    return Err(err!(
                        "cannot set both TimeWith::nanosecond \
                         and TimeWith::subsec_nanosecond",
                    ));
                }
                SubsecNanosecond::try_new(
                    "subsec_nanosecond",
                    subsec_nanosecond,
                )?
            }
        };
        if self.subsec_nanosecond.is_some() {
            Ok(Time::new_ranged(hour, minute, second, subsec_nanosecond))
        } else {
            Ok(Time::new_ranged(hour, minute, second, C(0))
                .with_subsec_parts_ranged(
                    millisecond,
                    microsecond,
                    nanosecond,
                ))
        }
    }

    /// Set the hour field on a [`Time`].
    ///
    /// One can access this value via [`Time::hour`].
    ///
    /// This overrides any previous hour settings.
    ///
    /// # Errors
    ///
    /// This returns an error when [`TimeWith::build`] is called if the given
    /// hour is outside the range `0..=23`.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::civil::time;
    ///
    /// let t1 = time(15, 21, 59, 0);
    /// assert_eq!(t1.hour(), 15);
    /// let t2 = t1.with().hour(3).build()?;
    /// assert_eq!(t2.hour(), 3);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn hour(self, hour: i8) -> TimeWith {
        TimeWith { hour: Some(hour), ..self }
    }

    /// Set the minute field on a [`Time`].
    ///
    /// One can access this value via [`Time::minute`].
    ///
    /// This overrides any previous minute settings.
    ///
    /// # Errors
    ///
    /// This returns an error when [`TimeWith::build`] is called if the given
    /// minute is outside the range `0..=59`.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::civil::time;
    ///
    /// let t1 = time(15, 21, 59, 0);
    /// assert_eq!(t1.minute(), 21);
    /// let t2 = t1.with().minute(3).build()?;
    /// assert_eq!(t2.minute(), 3);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn minute(self, minute: i8) -> TimeWith {
        TimeWith { minute: Some(minute), ..self }
    }

    /// Set the second field on a [`Time`].
    ///
    /// One can access this value via [`Time::second`].
    ///
    /// This overrides any previous second settings.
    ///
    /// # Errors
    ///
    /// This returns an error when [`TimeWith::build`] is called if the given
    /// second is outside the range `0..=59`.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::civil::time;
    ///
    /// let t1 = time(15, 21, 59, 0);
    /// assert_eq!(t1.second(), 59);
    /// let t2 = t1.with().second(3).build()?;
    /// assert_eq!(t2.second(), 3);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn second(self, second: i8) -> TimeWith {
        TimeWith { second: Some(second), ..self }
    }

    /// Set the millisecond field on a [`Time`].
    ///
    /// One can access this value via [`Time::millisecond`].
    ///
    /// This overrides any previous millisecond settings.
    ///
    /// Note that this only sets the millisecond component. It does
    /// not change the microsecond or nanosecond components. To set
    /// the fractional second component to nanosecond precision, use
    /// [`TimeWith::subsec_nanosecond`].
    ///
    /// # Errors
    ///
    /// This returns an error when [`TimeWith::build`] is called if the given
    /// millisecond is outside the range `0..=999`, or if both this and
    /// [`TimeWith::subsec_nanosecond`] are set.
    ///
    /// # Example
    ///
    /// This shows the relationship between [`Time::millisecond`] and
    /// [`Time::subsec_nanosecond`]:
    ///
    /// ```
    /// use jiff::civil::time;
    ///
    /// let t = time(15, 21, 35, 0).with().millisecond(123).build()?;
    /// assert_eq!(t.subsec_nanosecond(), 123_000_000);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn millisecond(self, millisecond: i16) -> TimeWith {
        TimeWith { millisecond: Some(millisecond), ..self }
    }

    /// Set the microsecond field on a [`Time`].
    ///
    /// One can access this value via [`Time::microsecond`].
    ///
    /// This overrides any previous microsecond settings.
    ///
    /// Note that this only sets the microsecond component. It does
    /// not change the millisecond or nanosecond components. To set
    /// the fractional second component to nanosecond precision, use
    /// [`TimeWith::subsec_nanosecond`].
    ///
    /// # Errors
    ///
    /// This returns an error when [`TimeWith::build`] is called if the given
    /// microsecond is outside the range `0..=999`, or if both this and
    /// [`TimeWith::subsec_nanosecond`] are set.
    ///
    /// # Example
    ///
    /// This shows the relationship between [`Time::microsecond`] and
    /// [`Time::subsec_nanosecond`]:
    ///
    /// ```
    /// use jiff::civil::time;
    ///
    /// let t = time(15, 21, 35, 0).with().microsecond(123).build()?;
    /// assert_eq!(t.subsec_nanosecond(), 123_000);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn microsecond(self, microsecond: i16) -> TimeWith {
        TimeWith { microsecond: Some(microsecond), ..self }
    }

    /// Set the nanosecond field on a [`Time`].
    ///
    /// One can access this value via [`Time::nanosecond`].
    ///
    /// This overrides any previous nanosecond settings.
    ///
    /// Note that this only sets the nanosecond component. It does
    /// not change the millisecond or microsecond components. To set
    /// the fractional second component to nanosecond precision, use
    /// [`TimeWith::subsec_nanosecond`].
    ///
    /// # Errors
    ///
    /// This returns an error when [`TimeWith::build`] is called if the given
    /// nanosecond is outside the range `0..=999`, or if both this and
    /// [`TimeWith::subsec_nanosecond`] are set.
    ///
    /// # Example
    ///
    /// This shows the relationship between [`Time::nanosecond`] and
    /// [`Time::subsec_nanosecond`]:
    ///
    /// ```
    /// use jiff::civil::time;
    ///
    /// let t = time(15, 21, 35, 0).with().nanosecond(123).build()?;
    /// assert_eq!(t.subsec_nanosecond(), 123);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn nanosecond(self, nanosecond: i16) -> TimeWith {
        TimeWith { nanosecond: Some(nanosecond), ..self }
    }

    /// Set the subsecond nanosecond field on a [`Time`].
    ///
    /// If you want to access this value on `Time`, then use
    /// [`Time::subsec_nanosecond`].
    ///
    /// This overrides any previous subsecond nanosecond settings.
    ///
    /// Note that this sets the entire fractional second component to
    /// nanosecond precision, and overrides any individual millisecond,
    /// microsecond or nanosecond settings. To set individual components,
    /// use [`TimeWith::millisecond`], [`TimeWith::microsecond`] or
    /// [`TimeWith::nanosecond`].
    ///
    /// # Errors
    ///
    /// This returns an error when [`TimeWith::build`] is called if the given
    /// subsecond nanosecond is outside the range `0..=999,999,999`, or if both
    /// this and one of [`TimeWith::millisecond`], [`TimeWith::microsecond`] or
    /// [`TimeWith::nanosecond`] are set.
    ///
    /// # Example
    ///
    /// This shows the relationship between constructing a `Time` value with
    /// subsecond nanoseconds and its individual subsecond fields:
    ///
    /// ```
    /// use jiff::civil::time;
    ///
    /// let t1 = time(15, 21, 35, 0);
    /// let t2 = t1.with().subsec_nanosecond(123_456_789).build()?;
    /// assert_eq!(t2.millisecond(), 123);
    /// assert_eq!(t2.microsecond(), 456);
    /// assert_eq!(t2.nanosecond(), 789);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn subsec_nanosecond(self, subsec_nanosecond: i32) -> TimeWith {
        TimeWith { subsec_nanosecond: Some(subsec_nanosecond), ..self }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use crate::{civil::time, span::span_eq, ToSpan};

    use super::*;

    #[test]
    fn min() {
        let t = Time::MIN;
        assert_eq!(t.hour(), 0);
        assert_eq!(t.minute(), 0);
        assert_eq!(t.second(), 0);
        assert_eq!(t.subsec_nanosecond(), 0);
    }

    #[test]
    fn max() {
        let t = Time::MAX;
        assert_eq!(t.hour(), 23);
        assert_eq!(t.minute(), 59);
        assert_eq!(t.second(), 59);
        assert_eq!(t.subsec_nanosecond(), 999_999_999);
    }

    #[test]
    fn invalid() {
        assert!(Time::new(24, 0, 0, 0).is_err());
        assert!(Time::new(23, 60, 0, 0).is_err());
        assert!(Time::new(23, 59, 60, 0).is_err());
        assert!(Time::new(23, 59, 61, 0).is_err());
        assert!(Time::new(-1, 0, 0, 0).is_err());
        assert!(Time::new(0, -1, 0, 0).is_err());
        assert!(Time::new(0, 0, -1, 0).is_err());

        assert!(Time::new(0, 0, 0, 1_000_000_000).is_err());
        assert!(Time::new(0, 0, 0, -1).is_err());
        assert!(Time::new(23, 59, 59, 1_000_000_000).is_err());
        assert!(Time::new(23, 59, 59, -1).is_err());
    }

    #[test]
    fn rounding_cross_midnight() {
        let t1 = time(23, 59, 59, 999_999_999);

        let t2 = t1.round(Unit::Nanosecond).unwrap();
        assert_eq!(t2, t1);

        let t2 = t1.round(Unit::Millisecond).unwrap();
        assert_eq!(t2, time(0, 0, 0, 0));

        let t2 = t1.round(Unit::Microsecond).unwrap();
        assert_eq!(t2, time(0, 0, 0, 0));

        let t2 = t1.round(Unit::Millisecond).unwrap();
        assert_eq!(t2, time(0, 0, 0, 0));

        let t2 = t1.round(Unit::Second).unwrap();
        assert_eq!(t2, time(0, 0, 0, 0));

        let t2 = t1.round(Unit::Minute).unwrap();
        assert_eq!(t2, time(0, 0, 0, 0));

        let t2 = t1.round(Unit::Hour).unwrap();
        assert_eq!(t2, time(0, 0, 0, 0));

        let t1 = time(22, 15, 0, 0);
        assert_eq!(
            time(22, 30, 0, 0),
            t1.round(TimeRound::new().smallest(Unit::Minute).increment(30))
                .unwrap()
        );
    }

    #[cfg(not(miri))]
    quickcheck::quickcheck! {
        fn prop_ordering_same_as_civil_nanosecond(
            civil_nanosecond1: CivilDayNanosecond,
            civil_nanosecond2: CivilDayNanosecond
        ) -> bool {
            let t1 = Time::from_nanosecond(civil_nanosecond1);
            let t2 = Time::from_nanosecond(civil_nanosecond2);
            t1.cmp(&t2) == civil_nanosecond1.cmp(&civil_nanosecond2)
        }

        fn prop_checked_add_then_sub(
            time: Time,
            nano_span: CivilDayNanosecond
        ) -> quickcheck::TestResult {
            let span = Span::new().nanoseconds(nano_span.get());
            let Ok(sum) = time.checked_add(span) else {
                return quickcheck::TestResult::discard()
            };
            let diff = sum.checked_sub(span).unwrap();
            quickcheck::TestResult::from_bool(time == diff)
        }

        fn prop_wrapping_add_then_sub(
            time: Time,
            nano_span: CivilDayNanosecond
        ) -> bool {
            let span = Span::new().nanoseconds(nano_span.get());
            let sum = time.wrapping_add(span);
            let diff = sum.wrapping_sub(span);
            time == diff
        }

        fn prop_checked_add_equals_wrapping_add(
            time: Time,
            nano_span: CivilDayNanosecond
        ) -> quickcheck::TestResult {
            let span = Span::new().nanoseconds(nano_span.get());
            let Ok(sum_checked) = time.checked_add(span) else {
                return quickcheck::TestResult::discard()
            };
            let sum_wrapped = time.wrapping_add(span);
            quickcheck::TestResult::from_bool(sum_checked == sum_wrapped)
        }

        fn prop_checked_sub_equals_wrapping_sub(
            time: Time,
            nano_span: CivilDayNanosecond
        ) -> quickcheck::TestResult {
            let span = Span::new().nanoseconds(nano_span.get());
            let Ok(diff_checked) = time.checked_sub(span) else {
                return quickcheck::TestResult::discard()
            };
            let diff_wrapped = time.wrapping_sub(span);
            quickcheck::TestResult::from_bool(diff_checked == diff_wrapped)
        }

        fn prop_until_then_add(t1: Time, t2: Time) -> bool {
            let span = t1.until(t2).unwrap();
            t1.checked_add(span).unwrap() == t2
        }

        fn prop_until_then_sub(t1: Time, t2: Time) -> bool {
            let span = t1.until(t2).unwrap();
            t2.checked_sub(span).unwrap() == t1
        }

        fn prop_since_then_add(t1: Time, t2: Time) -> bool {
            let span = t1.since(t2).unwrap();
            t2.checked_add(span).unwrap() == t1
        }

        fn prop_since_then_sub(t1: Time, t2: Time) -> bool {
            let span = t1.since(t2).unwrap();
            t1.checked_sub(span).unwrap() == t2
        }

        fn prop_until_is_since_negated(t1: Time, t2: Time) -> bool {
            t1.until(t2).unwrap().get_nanoseconds()
                == t1.since(t2).unwrap().negate().get_nanoseconds()
        }
    }

    #[test]
    fn overflowing_add() {
        let t1 = time(23, 30, 0, 0);
        let (t2, span) = t1.overflowing_add(5.hours()).unwrap();
        assert_eq!(t2, time(4, 30, 0, 0));
        span_eq!(span, 1.days());
    }

    #[test]
    fn overflowing_add_overflows() {
        let t1 = time(23, 30, 0, 0);
        let span = Span::new()
            .hours(t::SpanHours::MAX_REPR)
            .minutes(t::SpanMinutes::MAX_REPR)
            .seconds(t::SpanSeconds::MAX_REPR)
            .milliseconds(t::SpanMilliseconds::MAX_REPR)
            .microseconds(t::SpanMicroseconds::MAX_REPR)
            .nanoseconds(t::SpanNanoseconds::MAX_REPR);
        assert!(t1.overflowing_add(span).is_err());
    }

    #[test]
    fn time_size() {
        #[cfg(debug_assertions)]
        {
            assert_eq!(24, core::mem::size_of::<Time>());
        }
        #[cfg(not(debug_assertions))]
        {
            assert_eq!(8, core::mem::size_of::<Time>());
        }
    }

    // This test checks that a wrapping subtraction with the minimum signed
    // duration is as expected.
    #[test]
    fn wrapping_sub_signed_duration_min() {
        let max = -SignedDuration::MIN.as_nanos();
        let got = time(15, 30, 8, 999_999_999).to_nanosecond();
        let expected = max.rem_euclid(t::NANOS_PER_CIVIL_DAY.bound());
        assert_eq!(i128::from(got.get()), expected);
    }

    // This test checks that a wrapping subtraction with the maximum signed
    // duration is as expected.
    #[test]
    fn wrapping_sub_signed_duration_max() {
        let max = -SignedDuration::MAX.as_nanos();
        let got = time(8, 29, 52, 1).to_nanosecond();
        let expected = max.rem_euclid(t::NANOS_PER_CIVIL_DAY.bound());
        assert_eq!(i128::from(got.get()), expected);
    }

    // This test checks that a wrapping subtraction with the maximum unsigned
    // duration is as expected.
    #[test]
    fn wrapping_sub_unsigned_duration_max() {
        let max =
            -i128::try_from(std::time::Duration::MAX.as_nanos()).unwrap();
        let got = time(16, 59, 44, 1).to_nanosecond();
        let expected = max.rem_euclid(t::NANOS_PER_CIVIL_DAY.bound());
        assert_eq!(i128::from(got.get()), expected);
    }

    /// # `serde` deserializer compatibility test
    ///
    /// Serde YAML used to be unable to deserialize `jiff` types,
    /// as deserializing from bytes is not supported by the deserializer.
    ///
    /// - <https://github.com/BurntSushi/jiff/issues/138>
    /// - <https://github.com/BurntSushi/jiff/discussions/148>
    #[test]
    fn civil_time_deserialize_yaml() {
        let expected = time(16, 35, 4, 987654321);

        let deserialized: Time =
            serde_yaml::from_str("16:35:04.987654321").unwrap();

        assert_eq!(deserialized, expected);

        let deserialized: Time =
            serde_yaml::from_slice("16:35:04.987654321".as_bytes()).unwrap();

        assert_eq!(deserialized, expected);

        let cursor = Cursor::new(b"16:35:04.987654321");
        let deserialized: Time = serde_yaml::from_reader(cursor).unwrap();

        assert_eq!(deserialized, expected);
    }
}
