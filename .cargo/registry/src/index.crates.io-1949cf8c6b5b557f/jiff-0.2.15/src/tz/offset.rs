use core::{
    ops::{Add, AddAssign, Neg, Sub, SubAssign},
    time::Duration as UnsignedDuration,
};

use crate::{
    civil,
    duration::{Duration, SDuration},
    error::{err, Error, ErrorContext},
    shared::util::itime::IOffset,
    span::Span,
    timestamp::Timestamp,
    tz::{AmbiguousOffset, AmbiguousTimestamp, AmbiguousZoned, TimeZone},
    util::{
        array_str::ArrayStr,
        rangeint::{self, Composite, RFrom, RInto, TryRFrom},
        t::{self, C},
    },
    RoundMode, SignedDuration, SignedDurationRound, Unit,
};

/// An enum indicating whether a particular datetime  is in DST or not.
///
/// DST stands for "daylight saving time." It is a label used to apply to
/// points in time as a way to contrast it with "standard time." DST is
/// usually, but not always, one hour ahead of standard time. When DST takes
/// effect is usually determined by governments, and the rules can vary
/// depending on the location. DST is typically used as a means to maximize
/// "sunlight" time during typical working hours, and as a cost cutting measure
/// by reducing energy consumption. (The effectiveness of DST and whether it
/// is overall worth it is a separate question entirely.)
///
/// In general, most users should never need to deal with this type. But it can
/// be occasionally useful in circumstances where callers need to know whether
/// DST is active or not for a particular point in time.
///
/// This type has a `From<bool>` trait implementation, where the bool is
/// interpreted as being `true` when DST is active.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord)]
pub enum Dst {
    /// DST is not in effect. In other words, standard time is in effect.
    No,
    /// DST is in effect.
    Yes,
}

impl Dst {
    /// Returns true when this value is equal to `Dst::Yes`.
    pub fn is_dst(self) -> bool {
        matches!(self, Dst::Yes)
    }

    /// Returns true when this value is equal to `Dst::No`.
    ///
    /// `std` in this context refers to "standard time." That is, it is the
    /// offset from UTC used when DST is not in effect.
    pub fn is_std(self) -> bool {
        matches!(self, Dst::No)
    }
}

impl From<bool> for Dst {
    fn from(is_dst: bool) -> Dst {
        if is_dst {
            Dst::Yes
        } else {
            Dst::No
        }
    }
}

/// Represents a fixed time zone offset.
///
/// Negative offsets correspond to time zones west of the prime meridian, while
/// positive offsets correspond to time zones east of the prime meridian.
/// Equivalently, in all cases, `civil-time - offset = UTC`.
///
/// # Display format
///
/// This type implements the `std::fmt::Display` trait. It
/// will convert the offset to a string format in the form
/// `{sign}{hours}[:{minutes}[:{seconds}]]`, where `minutes` and `seconds` are
/// only present when non-zero. For example:
///
/// ```
/// use jiff::tz;
///
/// let o = tz::offset(-5);
/// assert_eq!(o.to_string(), "-05");
/// let o = tz::Offset::from_seconds(-18_000).unwrap();
/// assert_eq!(o.to_string(), "-05");
/// let o = tz::Offset::from_seconds(-18_060).unwrap();
/// assert_eq!(o.to_string(), "-05:01");
/// let o = tz::Offset::from_seconds(-18_062).unwrap();
/// assert_eq!(o.to_string(), "-05:01:02");
///
/// // The min value.
/// let o = tz::Offset::from_seconds(-93_599).unwrap();
/// assert_eq!(o.to_string(), "-25:59:59");
/// // The max value.
/// let o = tz::Offset::from_seconds(93_599).unwrap();
/// assert_eq!(o.to_string(), "+25:59:59");
/// // No offset.
/// let o = tz::offset(0);
/// assert_eq!(o.to_string(), "+00");
/// ```
///
/// # Example
///
/// This shows how to create a zoned datetime with a time zone using a fixed
/// offset:
///
/// ```
/// use jiff::{civil::date, tz, Zoned};
///
/// let offset = tz::offset(-4).to_time_zone();
/// let zdt = date(2024, 7, 8).at(15, 20, 0, 0).to_zoned(offset)?;
/// assert_eq!(zdt.to_string(), "2024-07-08T15:20:00-04:00[-04:00]");
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// Notice that the zoned datetime still includes a time zone annotation. But
/// since there is no time zone identifier, the offset instead is repeated as
/// an additional assertion that a fixed offset datetime was intended.
#[derive(Clone, Copy, Eq, Hash, PartialEq, PartialOrd, Ord)]
pub struct Offset {
    span: t::SpanZoneOffset,
}

impl Offset {
    /// The minimum possible time zone offset.
    ///
    /// This corresponds to the offset `-25:59:59`.
    pub const MIN: Offset = Offset { span: t::SpanZoneOffset::MIN_SELF };

    /// The maximum possible time zone offset.
    ///
    /// This corresponds to the offset `25:59:59`.
    pub const MAX: Offset = Offset { span: t::SpanZoneOffset::MAX_SELF };

    /// The offset corresponding to UTC. That is, no offset at all.
    ///
    /// This is defined to always be equivalent to `Offset::ZERO`, but it is
    /// semantically distinct. This ought to be used when UTC is desired
    /// specifically, while `Offset::ZERO` ought to be used when one wants to
    /// express "no offset." For example, when adding offsets, `Offset::ZERO`
    /// corresponds to the identity.
    pub const UTC: Offset = Offset::ZERO;

    /// The offset corresponding to no offset at all.
    ///
    /// This is defined to always be equivalent to `Offset::UTC`, but it is
    /// semantically distinct. This ought to be used when a zero offset is
    /// desired specifically, while `Offset::UTC` ought to be used when one
    /// wants to express UTC. For example, when adding offsets, `Offset::ZERO`
    /// corresponds to the identity.
    pub const ZERO: Offset = Offset::constant(0);

    /// Creates a new time zone offset in a `const` context from a given number
    /// of hours.
    ///
    /// Negative offsets correspond to time zones west of the prime meridian,
    /// while positive offsets correspond to time zones east of the prime
    /// meridian. Equivalently, in all cases, `civil-time - offset = UTC`.
    ///
    /// The fallible non-const version of this constructor is
    /// [`Offset::from_hours`].
    ///
    /// # Panics
    ///
    /// This routine panics when the given number of hours is out of range.
    /// Namely, `hours` must be in the range `-25..=25`.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::tz::Offset;
    ///
    /// let o = Offset::constant(-5);
    /// assert_eq!(o.seconds(), -18_000);
    /// let o = Offset::constant(5);
    /// assert_eq!(o.seconds(), 18_000);
    /// ```
    ///
    /// Alternatively, one can use the terser `jiff::tz::offset` free function:
    ///
    /// ```
    /// use jiff::tz;
    ///
    /// let o = tz::offset(-5);
    /// assert_eq!(o.seconds(), -18_000);
    /// let o = tz::offset(5);
    /// assert_eq!(o.seconds(), 18_000);
    /// ```
    #[inline]
    pub const fn constant(hours: i8) -> Offset {
        if !t::SpanZoneOffsetHours::contains(hours) {
            panic!("invalid time zone offset hours")
        }
        Offset::constant_seconds((hours as i32) * 60 * 60)
    }

    /// Creates a new time zone offset in a `const` context from a given number
    /// of seconds.
    ///
    /// Negative offsets correspond to time zones west of the prime meridian,
    /// while positive offsets correspond to time zones east of the prime
    /// meridian. Equivalently, in all cases, `civil-time - offset = UTC`.
    ///
    /// The fallible non-const version of this constructor is
    /// [`Offset::from_seconds`].
    ///
    /// # Panics
    ///
    /// This routine panics when the given number of seconds is out of range.
    /// The range corresponds to the offsets `-25:59:59..=25:59:59`. In units
    /// of seconds, that corresponds to `-93,599..=93,599`.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use jiff::tz::Offset;
    ///
    /// let o = Offset::constant_seconds(-18_000);
    /// assert_eq!(o.seconds(), -18_000);
    /// let o = Offset::constant_seconds(18_000);
    /// assert_eq!(o.seconds(), 18_000);
    /// ```
    // This is currently unexported because I find the name too long and
    // very off-putting. I don't think non-hour offsets are used enough to
    // warrant its existence. And I think I'd rather `Offset::hms` be const and
    // exported instead of this monstrosity.
    #[inline]
    pub(crate) const fn constant_seconds(seconds: i32) -> Offset {
        if !t::SpanZoneOffset::contains(seconds) {
            panic!("invalid time zone offset seconds")
        }
        Offset { span: t::SpanZoneOffset::new_unchecked(seconds) }
    }

    /// Creates a new time zone offset from a given number of hours.
    ///
    /// Negative offsets correspond to time zones west of the prime meridian,
    /// while positive offsets correspond to time zones east of the prime
    /// meridian. Equivalently, in all cases, `civil-time - offset = UTC`.
    ///
    /// # Errors
    ///
    /// This routine returns an error when the given number of hours is out of
    /// range. Namely, `hours` must be in the range `-25..=25`.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::tz::Offset;
    ///
    /// let o = Offset::from_hours(-5)?;
    /// assert_eq!(o.seconds(), -18_000);
    /// let o = Offset::from_hours(5)?;
    /// assert_eq!(o.seconds(), 18_000);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn from_hours(hours: i8) -> Result<Offset, Error> {
        let hours = t::SpanZoneOffsetHours::try_new("offset-hours", hours)?;
        Ok(Offset::from_hours_ranged(hours))
    }

    /// Creates a new time zone offset in a `const` context from a given number
    /// of seconds.
    ///
    /// Negative offsets correspond to time zones west of the prime meridian,
    /// while positive offsets correspond to time zones east of the prime
    /// meridian. Equivalently, in all cases, `civil-time - offset = UTC`.
    ///
    /// # Errors
    ///
    /// This routine returns an error when the given number of seconds is out
    /// of range. The range corresponds to the offsets `-25:59:59..=25:59:59`.
    /// In units of seconds, that corresponds to `-93,599..=93,599`.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::tz::Offset;
    ///
    /// let o = Offset::from_seconds(-18_000)?;
    /// assert_eq!(o.seconds(), -18_000);
    /// let o = Offset::from_seconds(18_000)?;
    /// assert_eq!(o.seconds(), 18_000);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn from_seconds(seconds: i32) -> Result<Offset, Error> {
        let seconds = t::SpanZoneOffset::try_new("offset-seconds", seconds)?;
        Ok(Offset::from_seconds_ranged(seconds))
    }

    /// Returns the total number of seconds in this offset.
    ///
    /// The value returned is guaranteed to represent an offset in the range
    /// `-25:59:59..=25:59:59`. Or more precisely, the value will be in units
    /// of seconds in the range `-93,599..=93,599`.
    ///
    /// Negative offsets correspond to time zones west of the prime meridian,
    /// while positive offsets correspond to time zones east of the prime
    /// meridian. Equivalently, in all cases, `civil-time - offset = UTC`.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::tz;
    ///
    /// let o = tz::offset(-5);
    /// assert_eq!(o.seconds(), -18_000);
    /// let o = tz::offset(5);
    /// assert_eq!(o.seconds(), 18_000);
    /// ```
    #[inline]
    pub fn seconds(self) -> i32 {
        self.seconds_ranged().get()
    }

    /// Returns the negation of this offset.
    ///
    /// A negative offset will become positive and vice versa. This is a no-op
    /// if the offset is zero.
    ///
    /// This never panics.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::tz;
    ///
    /// assert_eq!(tz::offset(-5).negate(), tz::offset(5));
    /// // It's also available via the `-` operator:
    /// assert_eq!(-tz::offset(-5), tz::offset(5));
    /// ```
    pub fn negate(self) -> Offset {
        Offset { span: -self.span }
    }

    /// Returns the "sign number" or "signum" of this offset.
    ///
    /// The number returned is `-1` when this offset is negative,
    /// `0` when this offset is zero and `1` when this span is positive.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::tz;
    ///
    /// assert_eq!(tz::offset(5).signum(), 1);
    /// assert_eq!(tz::offset(0).signum(), 0);
    /// assert_eq!(tz::offset(-5).signum(), -1);
    /// ```
    #[inline]
    pub fn signum(self) -> i8 {
        t::Sign::rfrom(self.span.signum()).get()
    }

    /// Returns true if and only if this offset is positive.
    ///
    /// This returns false when the offset is zero or negative.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::tz;
    ///
    /// assert!(tz::offset(5).is_positive());
    /// assert!(!tz::offset(0).is_positive());
    /// assert!(!tz::offset(-5).is_positive());
    /// ```
    pub fn is_positive(self) -> bool {
        self.seconds_ranged() > C(0)
    }

    /// Returns true if and only if this offset is less than zero.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::tz;
    ///
    /// assert!(!tz::offset(5).is_negative());
    /// assert!(!tz::offset(0).is_negative());
    /// assert!(tz::offset(-5).is_negative());
    /// ```
    pub fn is_negative(self) -> bool {
        self.seconds_ranged() < C(0)
    }

    /// Returns true if and only if this offset is zero.
    ///
    /// Or equivalently, when this offset corresponds to [`Offset::UTC`].
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::tz;
    ///
    /// assert!(!tz::offset(5).is_zero());
    /// assert!(tz::offset(0).is_zero());
    /// assert!(!tz::offset(-5).is_zero());
    /// ```
    pub fn is_zero(self) -> bool {
        self.seconds_ranged() == C(0)
    }

    /// Converts this offset into a [`TimeZone`].
    ///
    /// This is a convenience function for calling [`TimeZone::fixed`] with
    /// this offset.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::tz::offset;
    ///
    /// let tz = offset(-4).to_time_zone();
    /// assert_eq!(
    ///     tz.to_datetime(jiff::Timestamp::UNIX_EPOCH).to_string(),
    ///     "1969-12-31T20:00:00",
    /// );
    /// ```
    pub fn to_time_zone(self) -> TimeZone {
        TimeZone::fixed(self)
    }

    /// Converts the given timestamp to a civil datetime using this offset.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::{civil::date, tz, Timestamp};
    ///
    /// assert_eq!(
    ///     tz::offset(-8).to_datetime(Timestamp::UNIX_EPOCH),
    ///     date(1969, 12, 31).at(16, 0, 0, 0),
    /// );
    /// ```
    #[inline]
    pub fn to_datetime(self, timestamp: Timestamp) -> civil::DateTime {
        let idt = timestamp.to_itimestamp().zip2(self.to_ioffset()).map(
            #[allow(unused_mut)]
            |(mut its, ioff)| {
                // This is tricky, but if we have a minimal number of seconds,
                // then the minimum possible nanosecond value is actually 0.
                // So we clamp it in this case. (This encodes the invariant
                // enforced by `Timestamp::new`.)
                #[cfg(debug_assertions)]
                if its.second == t::UnixSeconds::MIN_REPR {
                    its.nanosecond = 0;
                }
                its.to_datetime(ioff)
            },
        );
        civil::DateTime::from_idatetime(idt)
    }

    /// Converts the given civil datetime to a timestamp using this offset.
    ///
    /// # Errors
    ///
    /// This returns an error if this would have returned a timestamp outside
    /// of its minimum and maximum values.
    ///
    /// # Example
    ///
    /// This example shows how to find the timestamp corresponding to
    /// `1969-12-31T16:00:00-08`.
    ///
    /// ```
    /// use jiff::{civil::date, tz, Timestamp};
    ///
    /// assert_eq!(
    ///     tz::offset(-8).to_timestamp(date(1969, 12, 31).at(16, 0, 0, 0))?,
    ///     Timestamp::UNIX_EPOCH,
    /// );
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// This example shows some maximum boundary conditions where this routine
    /// will fail:
    ///
    /// ```
    /// use jiff::{civil::date, tz, Timestamp, ToSpan};
    ///
    /// let dt = date(9999, 12, 31).at(23, 0, 0, 0);
    /// assert!(tz::offset(-8).to_timestamp(dt).is_err());
    ///
    /// // If the offset is big enough, then converting it to a UTC
    /// // timestamp will fit, even when using the maximum civil datetime.
    /// let dt = date(9999, 12, 31).at(23, 59, 59, 999_999_999);
    /// assert_eq!(tz::Offset::MAX.to_timestamp(dt).unwrap(), Timestamp::MAX);
    /// // But adjust the offset down 1 second is enough to go out-of-bounds.
    /// assert!((tz::Offset::MAX - 1.seconds()).to_timestamp(dt).is_err());
    /// ```
    ///
    /// Same as above, but for minimum values:
    ///
    /// ```
    /// use jiff::{civil::date, tz, Timestamp, ToSpan};
    ///
    /// let dt = date(-9999, 1, 1).at(1, 0, 0, 0);
    /// assert!(tz::offset(8).to_timestamp(dt).is_err());
    ///
    /// // If the offset is small enough, then converting it to a UTC
    /// // timestamp will fit, even when using the minimum civil datetime.
    /// let dt = date(-9999, 1, 1).at(0, 0, 0, 0);
    /// assert_eq!(tz::Offset::MIN.to_timestamp(dt).unwrap(), Timestamp::MIN);
    /// // But adjust the offset up 1 second is enough to go out-of-bounds.
    /// assert!((tz::Offset::MIN + 1.seconds()).to_timestamp(dt).is_err());
    /// ```
    #[inline]
    pub fn to_timestamp(
        self,
        dt: civil::DateTime,
    ) -> Result<Timestamp, Error> {
        let its = dt
            .to_idatetime()
            .zip2(self.to_ioffset())
            .map(|(idt, ioff)| idt.to_timestamp(ioff));
        Timestamp::from_itimestamp(its).with_context(|| {
            err!(
                "converting {dt} with offset {offset} to timestamp overflowed",
                offset = self,
            )
        })
    }

    /// Adds the given span of time to this offset.
    ///
    /// Since time zone offsets have second resolution, any fractional seconds
    /// in the duration given are ignored.
    ///
    /// This operation accepts three different duration types: [`Span`],
    /// [`SignedDuration`] or [`std::time::Duration`]. This is achieved via
    /// `From` trait implementations for the [`OffsetArithmetic`] type.
    ///
    /// # Errors
    ///
    /// This returns an error if the result of adding the given span would
    /// exceed the minimum or maximum allowed `Offset` value.
    ///
    /// This also returns an error if the span given contains any non-zero
    /// units bigger than hours.
    ///
    /// # Example
    ///
    /// This example shows how to add one hour to an offset (if the offset
    /// corresponds to standard time, then adding an hour will usually give
    /// you DST time):
    ///
    /// ```
    /// use jiff::{tz, ToSpan};
    ///
    /// let off = tz::offset(-5);
    /// assert_eq!(off.checked_add(1.hours()).unwrap(), tz::offset(-4));
    /// ```
    ///
    /// And note that while fractional seconds are ignored, units less than
    /// seconds aren't ignored if they sum up to a duration at least as big
    /// as one second:
    ///
    /// ```
    /// use jiff::{tz, ToSpan};
    ///
    /// let off = tz::offset(5);
    /// let span = 900.milliseconds()
    ///     .microseconds(50_000)
    ///     .nanoseconds(50_000_000);
    /// assert_eq!(
    ///     off.checked_add(span).unwrap(),
    ///     tz::Offset::from_seconds((5 * 60 * 60) + 1).unwrap(),
    /// );
    /// // Any leftover fractional part is ignored.
    /// let span = 901.milliseconds()
    ///     .microseconds(50_001)
    ///     .nanoseconds(50_000_001);
    /// assert_eq!(
    ///     off.checked_add(span).unwrap(),
    ///     tz::Offset::from_seconds((5 * 60 * 60) + 1).unwrap(),
    /// );
    /// ```
    ///
    /// This example shows some cases where checked addition will fail.
    ///
    /// ```
    /// use jiff::{tz::Offset, ToSpan};
    ///
    /// // Adding units above 'hour' always results in an error.
    /// assert!(Offset::UTC.checked_add(1.day()).is_err());
    /// assert!(Offset::UTC.checked_add(1.week()).is_err());
    /// assert!(Offset::UTC.checked_add(1.month()).is_err());
    /// assert!(Offset::UTC.checked_add(1.year()).is_err());
    ///
    /// // Adding even 1 second to the max, or subtracting 1 from the min,
    /// // will result in overflow and thus an error will be returned.
    /// assert!(Offset::MIN.checked_add(-1.seconds()).is_err());
    /// assert!(Offset::MAX.checked_add(1.seconds()).is_err());
    /// ```
    ///
    /// # Example: adding absolute durations
    ///
    /// This shows how to add signed and unsigned absolute durations to an
    /// `Offset`. Like with `Span`s, any fractional seconds are ignored.
    ///
    /// ```
    /// use std::time::Duration;
    ///
    /// use jiff::{tz::offset, SignedDuration};
    ///
    /// let off = offset(-10);
    ///
    /// let dur = SignedDuration::from_hours(11);
    /// assert_eq!(off.checked_add(dur)?, offset(1));
    /// assert_eq!(off.checked_add(-dur)?, offset(-21));
    ///
    /// // Any leftover time is truncated. That is, only
    /// // whole seconds from the duration are considered.
    /// let dur = Duration::new(3 * 60 * 60, 999_999_999);
    /// assert_eq!(off.checked_add(dur)?, offset(-7));
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn checked_add<A: Into<OffsetArithmetic>>(
        self,
        duration: A,
    ) -> Result<Offset, Error> {
        let duration: OffsetArithmetic = duration.into();
        duration.checked_add(self)
    }

    #[inline]
    fn checked_add_span(self, span: Span) -> Result<Offset, Error> {
        if let Some(err) = span.smallest_non_time_non_zero_unit_error() {
            return Err(err);
        }
        let span_seconds = t::SpanZoneOffset::try_rfrom(
            "span-seconds",
            span.to_invariant_nanoseconds().div_ceil(t::NANOS_PER_SECOND),
        )?;
        let offset_seconds = self.seconds_ranged();
        let seconds =
            offset_seconds.try_checked_add("offset-seconds", span_seconds)?;
        Ok(Offset::from_seconds_ranged(seconds))
    }

    #[inline]
    fn checked_add_duration(
        self,
        duration: SignedDuration,
    ) -> Result<Offset, Error> {
        let duration =
            t::SpanZoneOffset::try_new("duration-seconds", duration.as_secs())
                .with_context(|| {
                    err!(
                        "adding signed duration {duration:?} \
                         to offset {self} overflowed maximum offset seconds"
                    )
                })?;
        let offset_seconds = self.seconds_ranged();
        let seconds = offset_seconds
            .try_checked_add("offset-seconds", duration)
            .with_context(|| {
                err!(
                    "adding signed duration {duration:?} \
                     to offset {self} overflowed"
                )
            })?;
        Ok(Offset::from_seconds_ranged(seconds))
    }

    /// This routine is identical to [`Offset::checked_add`] with the duration
    /// negated.
    ///
    /// # Errors
    ///
    /// This has the same error conditions as [`Offset::checked_add`].
    ///
    /// # Example
    ///
    /// ```
    /// use std::time::Duration;
    ///
    /// use jiff::{tz, SignedDuration, ToSpan};
    ///
    /// let off = tz::offset(-4);
    /// assert_eq!(
    ///     off.checked_sub(1.hours())?,
    ///     tz::offset(-5),
    /// );
    /// assert_eq!(
    ///     off.checked_sub(SignedDuration::from_hours(1))?,
    ///     tz::offset(-5),
    /// );
    /// assert_eq!(
    ///     off.checked_sub(Duration::from_secs(60 * 60))?,
    ///     tz::offset(-5),
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn checked_sub<A: Into<OffsetArithmetic>>(
        self,
        duration: A,
    ) -> Result<Offset, Error> {
        let duration: OffsetArithmetic = duration.into();
        duration.checked_neg().and_then(|oa| oa.checked_add(self))
    }

    /// This routine is identical to [`Offset::checked_add`], except the
    /// result saturates on overflow. That is, instead of overflow, either
    /// [`Offset::MIN`] or [`Offset::MAX`] is returned.
    ///
    /// # Example
    ///
    /// This example shows some cases where saturation will occur.
    ///
    /// ```
    /// use jiff::{tz::Offset, SignedDuration, ToSpan};
    ///
    /// // Adding units above 'day' always results in saturation.
    /// assert_eq!(Offset::UTC.saturating_add(1.weeks()), Offset::MAX);
    /// assert_eq!(Offset::UTC.saturating_add(1.months()), Offset::MAX);
    /// assert_eq!(Offset::UTC.saturating_add(1.years()), Offset::MAX);
    ///
    /// // Adding even 1 second to the max, or subtracting 1 from the min,
    /// // will result in saturationg.
    /// assert_eq!(Offset::MIN.saturating_add(-1.seconds()), Offset::MIN);
    /// assert_eq!(Offset::MAX.saturating_add(1.seconds()), Offset::MAX);
    ///
    /// // Adding absolute durations also saturates as expected.
    /// assert_eq!(Offset::UTC.saturating_add(SignedDuration::MAX), Offset::MAX);
    /// assert_eq!(Offset::UTC.saturating_add(SignedDuration::MIN), Offset::MIN);
    /// assert_eq!(Offset::UTC.saturating_add(std::time::Duration::MAX), Offset::MAX);
    /// ```
    #[inline]
    pub fn saturating_add<A: Into<OffsetArithmetic>>(
        self,
        duration: A,
    ) -> Offset {
        let duration: OffsetArithmetic = duration.into();
        self.checked_add(duration).unwrap_or_else(|_| {
            if duration.is_negative() {
                Offset::MIN
            } else {
                Offset::MAX
            }
        })
    }

    /// This routine is identical to [`Offset::saturating_add`] with the span
    /// parameter negated.
    ///
    /// # Example
    ///
    /// This example shows some cases where saturation will occur.
    ///
    /// ```
    /// use jiff::{tz::Offset, SignedDuration, ToSpan};
    ///
    /// // Adding units above 'day' always results in saturation.
    /// assert_eq!(Offset::UTC.saturating_sub(1.weeks()), Offset::MIN);
    /// assert_eq!(Offset::UTC.saturating_sub(1.months()), Offset::MIN);
    /// assert_eq!(Offset::UTC.saturating_sub(1.years()), Offset::MIN);
    ///
    /// // Adding even 1 second to the max, or subtracting 1 from the min,
    /// // will result in saturationg.
    /// assert_eq!(Offset::MIN.saturating_sub(1.seconds()), Offset::MIN);
    /// assert_eq!(Offset::MAX.saturating_sub(-1.seconds()), Offset::MAX);
    ///
    /// // Adding absolute durations also saturates as expected.
    /// assert_eq!(Offset::UTC.saturating_sub(SignedDuration::MAX), Offset::MIN);
    /// assert_eq!(Offset::UTC.saturating_sub(SignedDuration::MIN), Offset::MAX);
    /// assert_eq!(Offset::UTC.saturating_sub(std::time::Duration::MAX), Offset::MIN);
    /// ```
    #[inline]
    pub fn saturating_sub<A: Into<OffsetArithmetic>>(
        self,
        duration: A,
    ) -> Offset {
        let duration: OffsetArithmetic = duration.into();
        let Ok(duration) = duration.checked_neg() else { return Offset::MIN };
        self.saturating_add(duration)
    }

    /// Returns the span of time from this offset until the other given.
    ///
    /// When the `other` offset is more west (i.e., more negative) of the prime
    /// meridian than this offset, then the span returned will be negative.
    ///
    /// # Properties
    ///
    /// Adding the span returned to this offset will always equal the `other`
    /// offset given.
    ///
    /// # Examples
    ///
    /// ```
    /// use jiff::{tz, ToSpan};
    ///
    /// assert_eq!(
    ///     tz::offset(-5).until(tz::Offset::UTC),
    ///     (5 * 60 * 60).seconds().fieldwise(),
    /// );
    /// // Flipping the operands in this case results in a negative span.
    /// assert_eq!(
    ///     tz::Offset::UTC.until(tz::offset(-5)),
    ///     -(5 * 60 * 60).seconds().fieldwise(),
    /// );
    /// ```
    #[inline]
    pub fn until(self, other: Offset) -> Span {
        let diff = other.seconds_ranged() - self.seconds_ranged();
        Span::new().seconds_ranged(diff.rinto())
    }

    /// Returns the span of time since the other offset given from this offset.
    ///
    /// When the `other` is more east (i.e., more positive) of the prime
    /// meridian than this offset, then the span returned will be negative.
    ///
    /// # Properties
    ///
    /// Adding the span returned to the `other` offset will always equal this
    /// offset.
    ///
    /// # Examples
    ///
    /// ```
    /// use jiff::{tz, ToSpan};
    ///
    /// assert_eq!(
    ///     tz::Offset::UTC.since(tz::offset(-5)),
    ///     (5 * 60 * 60).seconds().fieldwise(),
    /// );
    /// // Flipping the operands in this case results in a negative span.
    /// assert_eq!(
    ///     tz::offset(-5).since(tz::Offset::UTC),
    ///     -(5 * 60 * 60).seconds().fieldwise(),
    /// );
    /// ```
    #[inline]
    pub fn since(self, other: Offset) -> Span {
        self.until(other).negate()
    }

    /// Returns an absolute duration representing the difference in time from
    /// this offset until the given `other` offset.
    ///
    /// When the `other` offset is more west (i.e., more negative) of the prime
    /// meridian than this offset, then the duration returned will be negative.
    ///
    /// Unlike [`Offset::until`], this returns a duration corresponding to a
    /// 96-bit integer of nanoseconds between two offsets.
    ///
    /// # When should I use this versus [`Offset::until`]?
    ///
    /// See the type documentation for [`SignedDuration`] for the section on
    /// when one should use [`Span`] and when one should use `SignedDuration`.
    /// In short, use `Span` (and therefore `Offset::until`) unless you have a
    /// specific reason to do otherwise.
    ///
    /// # Examples
    ///
    /// ```
    /// use jiff::{tz, SignedDuration};
    ///
    /// assert_eq!(
    ///     tz::offset(-5).duration_until(tz::Offset::UTC),
    ///     SignedDuration::from_hours(5),
    /// );
    /// // Flipping the operands in this case results in a negative span.
    /// assert_eq!(
    ///     tz::Offset::UTC.duration_until(tz::offset(-5)),
    ///     SignedDuration::from_hours(-5),
    /// );
    /// ```
    #[inline]
    pub fn duration_until(self, other: Offset) -> SignedDuration {
        SignedDuration::offset_until(self, other)
    }

    /// This routine is identical to [`Offset::duration_until`], but the order
    /// of the parameters is flipped.
    ///
    /// # Examples
    ///
    /// ```
    /// use jiff::{tz, SignedDuration};
    ///
    /// assert_eq!(
    ///     tz::Offset::UTC.duration_since(tz::offset(-5)),
    ///     SignedDuration::from_hours(5),
    /// );
    /// assert_eq!(
    ///     tz::offset(-5).duration_since(tz::Offset::UTC),
    ///     SignedDuration::from_hours(-5),
    /// );
    /// ```
    #[inline]
    pub fn duration_since(self, other: Offset) -> SignedDuration {
        SignedDuration::offset_until(other, self)
    }

    /// Returns a new offset that is rounded according to the given
    /// configuration.
    ///
    /// Rounding an offset has a number of parameters, all of which are
    /// optional. When no parameters are given, then no rounding is done, and
    /// the offset as given is returned. That is, it's a no-op.
    ///
    /// As is consistent with `Offset` itself, rounding only supports units of
    /// hours, minutes or seconds. If any other unit is provided, then an error
    /// is returned.
    ///
    /// The parameters are, in brief:
    ///
    /// * [`OffsetRound::smallest`] sets the smallest [`Unit`] that is allowed
    /// to be non-zero in the offset returned. By default, it is set to
    /// [`Unit::Second`], i.e., no rounding occurs. When the smallest unit is
    /// set to something bigger than seconds, then the non-zero units in the
    /// offset smaller than the smallest unit are used to determine how the
    /// offset should be rounded. For example, rounding `+01:59` to the nearest
    /// hour using the default rounding mode would produce `+02:00`.
    /// * [`OffsetRound::mode`] determines how to handle the remainder
    /// when rounding. The default is [`RoundMode::HalfExpand`], which
    /// corresponds to how you were likely taught to round in school.
    /// Alternative modes, like [`RoundMode::Trunc`], exist too. For example,
    /// a truncating rounding of `+01:59` to the nearest hour would
    /// produce `+01:00`.
    /// * [`OffsetRound::increment`] sets the rounding granularity to
    /// use for the configured smallest unit. For example, if the smallest unit
    /// is minutes and the increment is `15`, then the offset returned will
    /// always have its minute component set to a multiple of `15`.
    ///
    /// # Errors
    ///
    /// In general, there are two main ways for rounding to fail: an improper
    /// configuration like trying to round an offset to the nearest unit other
    /// than hours/minutes/seconds, or when overflow occurs. Overflow can occur
    /// when the offset would exceed the minimum or maximum `Offset` values.
    /// Typically, this can only realistically happen if the offset before
    /// rounding is already close to its minimum or maximum value.
    ///
    /// # Example: rounding to the nearest multiple of 15 minutes
    ///
    /// Most time zone offsets fall on an hour boundary, but some fall on the
    /// half-hour or even 15 minute boundary:
    ///
    /// ```
    /// use jiff::{tz::Offset, Unit};
    ///
    /// let offset = Offset::from_seconds(-(44 * 60 + 30)).unwrap();
    /// let rounded = offset.round((Unit::Minute, 15))?;
    /// assert_eq!(rounded, Offset::from_seconds(-45 * 60).unwrap());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: rounding can fail via overflow
    ///
    /// ```
    /// use jiff::{tz::Offset, Unit};
    ///
    /// assert_eq!(Offset::MAX.to_string(), "+25:59:59");
    /// assert_eq!(
    ///     Offset::MAX.round(Unit::Minute).unwrap_err().to_string(),
    ///     "rounding offset `+25:59:59` resulted in a duration of 26h, \
    ///      which overflows `Offset`",
    /// );
    /// ```
    #[inline]
    pub fn round<R: Into<OffsetRound>>(
        self,
        options: R,
    ) -> Result<Offset, Error> {
        let options: OffsetRound = options.into();
        options.round(self)
    }
}

impl Offset {
    /// This creates an `Offset` via hours/minutes/seconds components.
    ///
    /// Currently, it exists because it's convenient for use in tests.
    ///
    /// I originally wanted to expose this in the public API, but I couldn't
    /// decide on how I wanted to treat signedness. There are a variety of
    /// choices:
    ///
    /// * Require all values to be positive, and ask the caller to use
    /// `-offset` to negate it.
    /// * Require all values to have the same sign. If any differs, either
    /// panic or return an error.
    /// * If any have a negative sign, then behave as if all have a negative
    /// sign.
    /// * Permit any combination of sign and combine them correctly.
    /// Similar to how `std::time::Duration::new(-1s, 1ns)` is turned into
    /// `-999,999,999ns`.
    ///
    /// I think the last option is probably the right behavior, but also the
    /// most annoying to implement. But if someone wants to take a crack at it,
    /// a PR is welcome.
    #[cfg(test)]
    #[inline]
    pub(crate) const fn hms(hours: i8, minutes: i8, seconds: i8) -> Offset {
        let total = (hours as i32 * 60 * 60)
            + (minutes as i32 * 60)
            + (seconds as i32);
        Offset { span: t::SpanZoneOffset::new_unchecked(total) }
    }

    #[inline]
    pub(crate) fn from_hours_ranged(
        hours: impl RInto<t::SpanZoneOffsetHours>,
    ) -> Offset {
        let hours: t::SpanZoneOffset = hours.rinto().rinto();
        Offset::from_seconds_ranged(hours * t::SECONDS_PER_HOUR)
    }

    #[inline]
    pub(crate) fn from_seconds_ranged(
        seconds: impl RInto<t::SpanZoneOffset>,
    ) -> Offset {
        Offset { span: seconds.rinto() }
    }

    /*
    #[inline]
    pub(crate) fn from_ioffset(ioff: Composite<IOffset>) -> Offset {
        let span = rangeint::uncomposite!(ioff, c => (c.second));
        Offset { span: span.to_rint() }
    }
    */

    #[inline]
    pub(crate) fn to_ioffset(self) -> Composite<IOffset> {
        rangeint::composite! {
            (second = self.span) => {
                IOffset { second }
            }
        }
    }

    #[inline]
    pub(crate) const fn from_ioffset_const(ioff: IOffset) -> Offset {
        Offset::from_seconds_unchecked(ioff.second)
    }

    #[inline]
    pub(crate) const fn from_seconds_unchecked(second: i32) -> Offset {
        Offset { span: t::SpanZoneOffset::new_unchecked(second) }
    }

    /*
    #[inline]
    pub(crate) const fn to_ioffset_const(self) -> IOffset {
        IOffset { second: self.span.get_unchecked() }
    }
    */

    #[inline]
    pub(crate) const fn seconds_ranged(self) -> t::SpanZoneOffset {
        self.span
    }

    #[inline]
    pub(crate) fn part_hours_ranged(self) -> t::SpanZoneOffsetHours {
        self.span.div_ceil(t::SECONDS_PER_HOUR).rinto()
    }

    #[inline]
    pub(crate) fn part_minutes_ranged(self) -> t::SpanZoneOffsetMinutes {
        self.span
            .div_ceil(t::SECONDS_PER_MINUTE)
            .rem_ceil(t::MINUTES_PER_HOUR)
            .rinto()
    }

    #[inline]
    pub(crate) fn part_seconds_ranged(self) -> t::SpanZoneOffsetSeconds {
        self.span.rem_ceil(t::SECONDS_PER_MINUTE).rinto()
    }

    #[inline]
    pub(crate) fn to_array_str(&self) -> ArrayStr<9> {
        use core::fmt::Write;

        let mut dst = ArrayStr::new("").unwrap();
        // OK because the string representation of an offset
        // can never exceed 9 bytes. The longest possible, e.g.,
        // is `-25:59:59`.
        write!(&mut dst, "{}", self).unwrap();
        dst
    }
}

impl core::fmt::Debug for Offset {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        let sign = if self.seconds_ranged() < C(0) { "-" } else { "" };
        write!(
            f,
            "{sign}{:02}:{:02}:{:02}",
            self.part_hours_ranged().abs(),
            self.part_minutes_ranged().abs(),
            self.part_seconds_ranged().abs(),
        )
    }
}

impl core::fmt::Display for Offset {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        let sign = if self.span < C(0) { "-" } else { "+" };
        let hours = self.part_hours_ranged().abs().get();
        let minutes = self.part_minutes_ranged().abs().get();
        let seconds = self.part_seconds_ranged().abs().get();
        if hours == 0 && minutes == 0 && seconds == 0 {
            write!(f, "+00")
        } else if hours != 0 && minutes == 0 && seconds == 0 {
            write!(f, "{sign}{hours:02}")
        } else if minutes != 0 && seconds == 0 {
            write!(f, "{sign}{hours:02}:{minutes:02}")
        } else {
            write!(f, "{sign}{hours:02}:{minutes:02}:{seconds:02}")
        }
    }
}

/// Adds a span of time to an offset. This panics on overflow.
///
/// For checked arithmetic, see [`Offset::checked_add`].
impl Add<Span> for Offset {
    type Output = Offset;

    #[inline]
    fn add(self, rhs: Span) -> Offset {
        self.checked_add(rhs)
            .expect("adding span to offset should not overflow")
    }
}

/// Adds a span of time to an offset in place. This panics on overflow.
///
/// For checked arithmetic, see [`Offset::checked_add`].
impl AddAssign<Span> for Offset {
    #[inline]
    fn add_assign(&mut self, rhs: Span) {
        *self = self.add(rhs);
    }
}

/// Subtracts a span of time from an offset. This panics on overflow.
///
/// For checked arithmetic, see [`Offset::checked_sub`].
impl Sub<Span> for Offset {
    type Output = Offset;

    #[inline]
    fn sub(self, rhs: Span) -> Offset {
        self.checked_sub(rhs)
            .expect("subtracting span from offsetsshould not overflow")
    }
}

/// Subtracts a span of time from an offset in place. This panics on overflow.
///
/// For checked arithmetic, see [`Offset::checked_sub`].
impl SubAssign<Span> for Offset {
    #[inline]
    fn sub_assign(&mut self, rhs: Span) {
        *self = self.sub(rhs);
    }
}

/// Computes the span of time between two offsets.
///
/// This will return a negative span when the offset being subtracted is
/// greater (i.e., more east with respect to the prime meridian).
impl Sub for Offset {
    type Output = Span;

    #[inline]
    fn sub(self, rhs: Offset) -> Span {
        self.since(rhs)
    }
}

/// Adds a signed duration of time to an offset. This panics on overflow.
///
/// For checked arithmetic, see [`Offset::checked_add`].
impl Add<SignedDuration> for Offset {
    type Output = Offset;

    #[inline]
    fn add(self, rhs: SignedDuration) -> Offset {
        self.checked_add(rhs)
            .expect("adding signed duration to offset should not overflow")
    }
}

/// Adds a signed duration of time to an offset in place. This panics on
/// overflow.
///
/// For checked arithmetic, see [`Offset::checked_add`].
impl AddAssign<SignedDuration> for Offset {
    #[inline]
    fn add_assign(&mut self, rhs: SignedDuration) {
        *self = self.add(rhs);
    }
}

/// Subtracts a signed duration of time from an offset. This panics on
/// overflow.
///
/// For checked arithmetic, see [`Offset::checked_sub`].
impl Sub<SignedDuration> for Offset {
    type Output = Offset;

    #[inline]
    fn sub(self, rhs: SignedDuration) -> Offset {
        self.checked_sub(rhs).expect(
            "subtracting signed duration from offsetsshould not overflow",
        )
    }
}

/// Subtracts a signed duration of time from an offset in place. This panics on
/// overflow.
///
/// For checked arithmetic, see [`Offset::checked_sub`].
impl SubAssign<SignedDuration> for Offset {
    #[inline]
    fn sub_assign(&mut self, rhs: SignedDuration) {
        *self = self.sub(rhs);
    }
}

/// Adds an unsigned duration of time to an offset. This panics on overflow.
///
/// For checked arithmetic, see [`Offset::checked_add`].
impl Add<UnsignedDuration> for Offset {
    type Output = Offset;

    #[inline]
    fn add(self, rhs: UnsignedDuration) -> Offset {
        self.checked_add(rhs)
            .expect("adding unsigned duration to offset should not overflow")
    }
}

/// Adds an unsigned duration of time to an offset in place. This panics on
/// overflow.
///
/// For checked arithmetic, see [`Offset::checked_add`].
impl AddAssign<UnsignedDuration> for Offset {
    #[inline]
    fn add_assign(&mut self, rhs: UnsignedDuration) {
        *self = self.add(rhs);
    }
}

/// Subtracts an unsigned duration of time from an offset. This panics on
/// overflow.
///
/// For checked arithmetic, see [`Offset::checked_sub`].
impl Sub<UnsignedDuration> for Offset {
    type Output = Offset;

    #[inline]
    fn sub(self, rhs: UnsignedDuration) -> Offset {
        self.checked_sub(rhs).expect(
            "subtracting unsigned duration from offsetsshould not overflow",
        )
    }
}

/// Subtracts an unsigned duration of time from an offset in place. This panics
/// on overflow.
///
/// For checked arithmetic, see [`Offset::checked_sub`].
impl SubAssign<UnsignedDuration> for Offset {
    #[inline]
    fn sub_assign(&mut self, rhs: UnsignedDuration) {
        *self = self.sub(rhs);
    }
}

/// Negate this offset.
///
/// A positive offset becomes negative and vice versa. This is a no-op for the
/// zero offset.
///
/// This never panics.
impl Neg for Offset {
    type Output = Offset;

    #[inline]
    fn neg(self) -> Offset {
        self.negate()
    }
}

/// Converts a `SignedDuration` to a time zone offset.
///
/// If the signed duration has fractional seconds, then it is automatically
/// rounded to the nearest second. (Because an `Offset` has only second
/// precision.)
///
/// # Errors
///
/// This returns an error if the duration overflows the limits of an `Offset`.
///
/// # Example
///
/// ```
/// use jiff::{tz::{self, Offset}, SignedDuration};
///
/// let sdur = SignedDuration::from_secs(-5 * 60 * 60);
/// let offset = Offset::try_from(sdur)?;
/// assert_eq!(offset, tz::offset(-5));
///
/// // Sub-seconds results in rounded.
/// let sdur = SignedDuration::new(-5 * 60 * 60, -500_000_000);
/// let offset = Offset::try_from(sdur)?;
/// assert_eq!(offset, tz::Offset::from_seconds(-(5 * 60 * 60 + 1)).unwrap());
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
impl TryFrom<SignedDuration> for Offset {
    type Error = Error;

    fn try_from(sdur: SignedDuration) -> Result<Offset, Error> {
        let mut seconds = sdur.as_secs();
        let subsec = sdur.subsec_nanos();
        if subsec >= 500_000_000 {
            seconds = seconds.saturating_add(1);
        } else if subsec <= -500_000_000 {
            seconds = seconds.saturating_sub(1);
        }
        let seconds = i32::try_from(seconds).map_err(|_| {
            err!("`SignedDuration` of {sdur} overflows `Offset`")
        })?;
        Offset::from_seconds(seconds)
            .map_err(|_| err!("`SignedDuration` of {sdur} overflows `Offset`"))
    }
}

/// Options for [`Offset::checked_add`] and [`Offset::checked_sub`].
///
/// This type provides a way to ergonomically add one of a few different
/// duration types to a [`Offset`].
///
/// The main way to construct values of this type is with its `From` trait
/// implementations:
///
/// * `From<Span> for OffsetArithmetic` adds (or subtracts) the given span to
/// the receiver offset.
/// * `From<SignedDuration> for OffsetArithmetic` adds (or subtracts)
/// the given signed duration to the receiver offset.
/// * `From<std::time::Duration> for OffsetArithmetic` adds (or subtracts)
/// the given unsigned duration to the receiver offset.
///
/// # Example
///
/// ```
/// use std::time::Duration;
///
/// use jiff::{tz::offset, SignedDuration, ToSpan};
///
/// let off = offset(-10);
/// assert_eq!(off.checked_add(11.hours())?, offset(1));
/// assert_eq!(off.checked_add(SignedDuration::from_hours(11))?, offset(1));
/// assert_eq!(off.checked_add(Duration::from_secs(11 * 60 * 60))?, offset(1));
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Copy, Debug)]
pub struct OffsetArithmetic {
    duration: Duration,
}

impl OffsetArithmetic {
    #[inline]
    fn checked_add(self, offset: Offset) -> Result<Offset, Error> {
        match self.duration.to_signed()? {
            SDuration::Span(span) => offset.checked_add_span(span),
            SDuration::Absolute(sdur) => offset.checked_add_duration(sdur),
        }
    }

    #[inline]
    fn checked_neg(self) -> Result<OffsetArithmetic, Error> {
        let duration = self.duration.checked_neg()?;
        Ok(OffsetArithmetic { duration })
    }

    #[inline]
    fn is_negative(&self) -> bool {
        self.duration.is_negative()
    }
}

impl From<Span> for OffsetArithmetic {
    fn from(span: Span) -> OffsetArithmetic {
        let duration = Duration::from(span);
        OffsetArithmetic { duration }
    }
}

impl From<SignedDuration> for OffsetArithmetic {
    fn from(sdur: SignedDuration) -> OffsetArithmetic {
        let duration = Duration::from(sdur);
        OffsetArithmetic { duration }
    }
}

impl From<UnsignedDuration> for OffsetArithmetic {
    fn from(udur: UnsignedDuration) -> OffsetArithmetic {
        let duration = Duration::from(udur);
        OffsetArithmetic { duration }
    }
}

impl<'a> From<&'a Span> for OffsetArithmetic {
    fn from(span: &'a Span) -> OffsetArithmetic {
        OffsetArithmetic::from(*span)
    }
}

impl<'a> From<&'a SignedDuration> for OffsetArithmetic {
    fn from(sdur: &'a SignedDuration) -> OffsetArithmetic {
        OffsetArithmetic::from(*sdur)
    }
}

impl<'a> From<&'a UnsignedDuration> for OffsetArithmetic {
    fn from(udur: &'a UnsignedDuration) -> OffsetArithmetic {
        OffsetArithmetic::from(*udur)
    }
}

/// Options for [`Offset::round`].
///
/// This type provides a way to configure the rounding of an offset. This
/// includes setting the smallest unit (i.e., the unit to round), the rounding
/// increment and the rounding mode (e.g., "ceil" or "truncate").
///
/// [`Offset::round`] accepts anything that implements
/// `Into<OffsetRound>`. There are a few key trait implementations that
/// make this convenient:
///
/// * `From<Unit> for OffsetRound` will construct a rounding
/// configuration where the smallest unit is set to the one given.
/// * `From<(Unit, i64)> for OffsetRound` will construct a rounding
/// configuration where the smallest unit and the rounding increment are set to
/// the ones given.
///
/// In order to set other options (like the rounding mode), one must explicitly
/// create a `OffsetRound` and pass it to `Offset::round`.
///
/// # Example
///
/// This example shows how to always round up to the nearest half-hour:
///
/// ```
/// use jiff::{tz::{Offset, OffsetRound}, RoundMode, Unit};
///
/// let offset = Offset::from_seconds(4 * 60 * 60 + 17 * 60).unwrap();
/// let rounded = offset.round(
///     OffsetRound::new()
///         .smallest(Unit::Minute)
///         .increment(30)
///         .mode(RoundMode::Expand),
/// )?;
/// assert_eq!(rounded, Offset::from_seconds(4 * 60 * 60 + 30 * 60).unwrap());
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Copy, Debug)]
pub struct OffsetRound(SignedDurationRound);

impl OffsetRound {
    /// Create a new default configuration for rounding a time zone offset via
    /// [`Offset::round`].
    ///
    /// The default configuration does no rounding.
    #[inline]
    pub fn new() -> OffsetRound {
        OffsetRound(SignedDurationRound::new().smallest(Unit::Second))
    }

    /// Set the smallest units allowed in the offset returned. These are the
    /// units that the offset is rounded to.
    ///
    /// # Errors
    ///
    /// The unit must be [`Unit::Hour`], [`Unit::Minute`] or [`Unit::Second`].
    ///
    /// # Example
    ///
    /// A basic example that rounds to the nearest minute:
    ///
    /// ```
    /// use jiff::{tz::Offset, Unit};
    ///
    /// let offset = Offset::from_seconds(-(5 * 60 * 60 + 30)).unwrap();
    /// assert_eq!(offset.round(Unit::Hour)?, Offset::from_hours(-5).unwrap());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn smallest(self, unit: Unit) -> OffsetRound {
        OffsetRound(self.0.smallest(unit))
    }

    /// Set the rounding mode.
    ///
    /// This defaults to [`RoundMode::HalfExpand`], which makes rounding work
    /// like how you were taught in school.
    ///
    /// # Example
    ///
    /// A basic example that rounds to the nearest hour, but changing its
    /// rounding mode to truncation:
    ///
    /// ```
    /// use jiff::{tz::{Offset, OffsetRound}, RoundMode, Unit};
    ///
    /// let offset = Offset::from_seconds(-(5 * 60 * 60 + 30 * 60)).unwrap();
    /// assert_eq!(
    ///     offset.round(OffsetRound::new()
    ///         .smallest(Unit::Hour)
    ///         .mode(RoundMode::Trunc),
    ///     )?,
    ///     // The default round mode does rounding like
    ///     // how you probably learned in school, and would
    ///     // result in rounding to -6 hours. But we
    ///     // change it to truncation here, which makes it
    ///     // round -5.
    ///     Offset::from_hours(-5).unwrap(),
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn mode(self, mode: RoundMode) -> OffsetRound {
        OffsetRound(self.0.mode(mode))
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
    /// after the smallest unit configured (and must not be equivalent to
    /// it). For example, if the smallest unit is [`Unit::Second`], then
    /// *some* of the valid values for the rounding increment are `1`, `2`,
    /// `4`, `5`, `15` and `30`. Namely, any integer that divides evenly into
    /// `60` seconds since there are `60` seconds in the next highest unit
    /// (minutes).
    ///
    /// # Example
    ///
    /// This shows how to round an offset to the nearest 30 minute increment:
    ///
    /// ```
    /// use jiff::{tz::Offset, Unit};
    ///
    /// let offset = Offset::from_seconds(4 * 60 * 60 + 15 * 60).unwrap();
    /// assert_eq!(
    ///     offset.round((Unit::Minute, 30))?,
    ///     Offset::from_seconds(4 * 60 * 60 + 30 * 60).unwrap(),
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn increment(self, increment: i64) -> OffsetRound {
        OffsetRound(self.0.increment(increment))
    }

    /// Does the actual offset rounding.
    fn round(&self, offset: Offset) -> Result<Offset, Error> {
        let smallest = self.0.get_smallest();
        if !(Unit::Second <= smallest && smallest <= Unit::Hour) {
            return Err(err!(
                "rounding `Offset` failed because \
                 a unit of {plural} was provided, but offset rounding \
                 can only use hours, minutes or seconds",
                plural = smallest.plural(),
            ));
        }
        let rounded_sdur = SignedDuration::from(offset).round(self.0)?;
        Offset::try_from(rounded_sdur).map_err(|_| {
            err!(
                "rounding offset `{offset}` resulted in a duration \
                 of {rounded_sdur:?}, which overflows `Offset`",
            )
        })
    }
}

impl Default for OffsetRound {
    fn default() -> OffsetRound {
        OffsetRound::new()
    }
}

impl From<Unit> for OffsetRound {
    fn from(unit: Unit) -> OffsetRound {
        OffsetRound::default().smallest(unit)
    }
}

impl From<(Unit, i64)> for OffsetRound {
    fn from((unit, increment): (Unit, i64)) -> OffsetRound {
        OffsetRound::default().smallest(unit).increment(increment)
    }
}

/// Configuration for resolving disparities between an offset and a time zone.
///
/// A conflict between an offset and a time zone most commonly appears in a
/// datetime string. For example, `2024-06-14T17:30-05[America/New_York]`
/// has a definitive inconsistency between the reported offset (`-05`) and
/// the time zone (`America/New_York`), because at this time in New York,
/// daylight saving time (DST) was in effect. In New York in the year 2024,
/// DST corresponded to the UTC offset `-04`.
///
/// Other conflict variations exist. For example, in 2019, Brazil abolished
/// DST completely. But if one were to create a datetime for 2020 in 2018, that
/// datetime in 2020 would reflect the DST rules as they exist in 2018. That
/// could in turn result in a datetime with an offset that is incorrect with
/// respect to the rules in 2019.
///
/// For this reason, this crate exposes a few ways of resolving these
/// conflicts. It is most commonly used as configuration for parsing
/// [`Zoned`](crate::Zoned) values via
/// [`fmt::temporal::DateTimeParser::offset_conflict`](crate::fmt::temporal::DateTimeParser::offset_conflict). But this configuration can also be used directly via
/// [`OffsetConflict::resolve`].
///
/// The default value is `OffsetConflict::Reject`, which results in an
/// error being returned if the offset and a time zone are not in agreement.
/// This is the default so that Jiff does not automatically make silent choices
/// about whether to prefer the time zone or the offset. The
/// [`fmt::temporal::DateTimeParser::parse_zoned_with`](crate::fmt::temporal::DateTimeParser::parse_zoned_with)
/// documentation shows an example demonstrating its utility in the face
/// of changes in the law, such as the abolition of daylight saving time.
/// By rejecting such things, one can ensure that the original timestamp is
/// preserved or else an error occurs.
///
/// This enum is non-exhaustive so that other forms of offset conflicts may be
/// added in semver compatible releases.
///
/// # Example
///
/// This example shows how to always use the time zone even if the offset is
/// wrong.
///
/// ```
/// use jiff::{civil::date, tz};
///
/// let dt = date(2024, 6, 14).at(17, 30, 0, 0);
/// let offset = tz::offset(-5); // wrong! should be -4
/// let newyork = tz::db().get("America/New_York")?;
///
/// // The default conflict resolution, 'Reject', will error.
/// let result = tz::OffsetConflict::Reject
///     .resolve(dt, offset, newyork.clone());
/// assert!(result.is_err());
///
/// // But we can change it to always prefer the time zone.
/// let zdt = tz::OffsetConflict::AlwaysTimeZone
///     .resolve(dt, offset, newyork.clone())?
///     .unambiguous()?;
/// assert_eq!(zdt.datetime(), date(2024, 6, 14).at(17, 30, 0, 0));
/// // The offset has been corrected automatically.
/// assert_eq!(zdt.offset(), tz::offset(-4));
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Example: parsing
///
/// This example shows how to set the offset conflict resolution configuration
/// while parsing a [`Zoned`](crate::Zoned) datetime. In this example, we
/// always prefer the offset, even if it conflicts with the time zone.
///
/// ```
/// use jiff::{civil::date, fmt::temporal::DateTimeParser, tz};
///
/// static PARSER: DateTimeParser = DateTimeParser::new()
///     .offset_conflict(tz::OffsetConflict::AlwaysOffset);
///
/// let zdt = PARSER.parse_zoned("2024-06-14T17:30-05[America/New_York]")?;
/// // The time *and* offset have been corrected. The offset given was invalid,
/// // so it cannot be kept, but the timestamp returned is equivalent to
/// // `2024-06-14T17:30-05`. It is just adjusted automatically to be correct
/// // in the `America/New_York` time zone.
/// assert_eq!(zdt.datetime(), date(2024, 6, 14).at(18, 30, 0, 0));
/// assert_eq!(zdt.offset(), tz::offset(-4));
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Copy, Debug, Default)]
#[non_exhaustive]
pub enum OffsetConflict {
    /// When the offset and time zone are in conflict, this will always use
    /// the offset to interpret the date time.
    ///
    /// When resolving to a [`AmbiguousZoned`], the time zone attached
    /// to the timestamp will still be the same as the time zone given. The
    /// difference here is that the offset will be adjusted such that it is
    /// correct for the given time zone. However, the timestamp itself will
    /// always match the datetime and offset given (and which is always
    /// unambiguous).
    ///
    /// Basically, you should use this option when you want to keep the exact
    /// time unchanged (as indicated by the datetime and offset), even if it
    /// means a change to civil time.
    AlwaysOffset,
    /// When the offset and time zone are in conflict, this will always use
    /// the time zone to interpret the date time.
    ///
    /// When resolving to an [`AmbiguousZoned`], the offset attached to the
    /// timestamp will always be determined by only looking at the time zone.
    /// This in turn implies that the timestamp returned could be ambiguous,
    /// since this conflict resolution strategy specifically ignores the
    /// offset. (And, we're only at this point because the offset is not
    /// possible for the given time zone, so it can't be used in concert with
    /// the time zone anyway.) This is unlike the `AlwaysOffset` strategy where
    /// the timestamp returned is guaranteed to be unambiguous.
    ///
    /// You should use this option when you want to keep the civil time
    /// unchanged even if it means a change to the exact time.
    AlwaysTimeZone,
    /// Always attempt to use the offset to resolve a datetime to a timestamp,
    /// unless the offset is invalid for the provided time zone. In that case,
    /// use the time zone. When the time zone is used, it's possible for an
    /// ambiguous datetime to be returned.
    ///
    /// See [`ZonedWith::offset_conflict`](crate::ZonedWith::offset_conflict)
    /// for an example of when this strategy is useful.
    PreferOffset,
    /// When the offset and time zone are in conflict, this strategy always
    /// results in conflict resolution returning an error.
    ///
    /// This is the default since a conflict between the offset and the time
    /// zone usually implies an invalid datetime in some way.
    #[default]
    Reject,
}

impl OffsetConflict {
    /// Resolve a potential conflict between an [`Offset`] and a [`TimeZone`].
    ///
    /// # Errors
    ///
    /// This returns an error if this would have returned a timestamp outside
    /// of its minimum and maximum values.
    ///
    /// This can also return an error when using the [`OffsetConflict::Reject`]
    /// strategy. Namely, when using the `Reject` strategy, any offset that is
    /// not compatible with the given datetime and time zone will always result
    /// in an error.
    ///
    /// # Example
    ///
    /// This example shows how each of the different conflict resolution
    /// strategies are applied.
    ///
    /// ```
    /// use jiff::{civil::date, tz};
    ///
    /// let dt = date(2024, 6, 14).at(17, 30, 0, 0);
    /// let offset = tz::offset(-5); // wrong! should be -4
    /// let newyork = tz::db().get("America/New_York")?;
    ///
    /// // Here, we use the offset and ignore the time zone.
    /// let zdt = tz::OffsetConflict::AlwaysOffset
    ///     .resolve(dt, offset, newyork.clone())?
    ///     .unambiguous()?;
    /// // The datetime (and offset) have been corrected automatically
    /// // and the resulting Zoned instant corresponds precisely to
    /// // `2024-06-14T17:30-05[UTC]`.
    /// assert_eq!(zdt.to_string(), "2024-06-14T18:30:00-04:00[America/New_York]");
    ///
    /// // Here, we use the time zone and ignore the offset.
    /// let zdt = tz::OffsetConflict::AlwaysTimeZone
    ///     .resolve(dt, offset, newyork.clone())?
    ///     .unambiguous()?;
    /// // The offset has been corrected automatically and the resulting
    /// // Zoned instant corresponds precisely to `2024-06-14T17:30-04[UTC]`.
    /// // Notice how the civil time remains the same, but the exact instant
    /// // has changed!
    /// assert_eq!(zdt.to_string(), "2024-06-14T17:30:00-04:00[America/New_York]");
    ///
    /// // Here, we prefer the offset, but fall back to the time zone.
    /// // In this example, it has the same behavior as `AlwaysTimeZone`.
    /// let zdt = tz::OffsetConflict::PreferOffset
    ///     .resolve(dt, offset, newyork.clone())?
    ///     .unambiguous()?;
    /// assert_eq!(zdt.to_string(), "2024-06-14T17:30:00-04:00[America/New_York]");
    ///
    /// // The default conflict resolution, 'Reject', will error.
    /// let result = tz::OffsetConflict::Reject
    ///     .resolve(dt, offset, newyork.clone());
    /// assert!(result.is_err());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn resolve(
        self,
        dt: civil::DateTime,
        offset: Offset,
        tz: TimeZone,
    ) -> Result<AmbiguousZoned, Error> {
        self.resolve_with(dt, offset, tz, |off1, off2| off1 == off2)
    }

    /// Resolve a potential conflict between an [`Offset`] and a [`TimeZone`]
    /// using the given definition of equality for an `Offset`.
    ///
    /// The equality predicate is always given a pair of offsets where the
    /// first is the offset given to `resolve_with` and the second is the
    /// offset found in the `TimeZone`.
    ///
    /// # Errors
    ///
    /// This returns an error if this would have returned a timestamp outside
    /// of its minimum and maximum values.
    ///
    /// This can also return an error when using the [`OffsetConflict::Reject`]
    /// strategy. Namely, when using the `Reject` strategy, any offset that is
    /// not compatible with the given datetime and time zone will always result
    /// in an error.
    ///
    /// # Example
    ///
    /// Unlike [`OffsetConflict::resolve`], this routine permits overriding
    /// the definition of equality used for comparing offsets. In
    /// `OffsetConflict::resolve`, exact equality is used. This can be
    /// troublesome in some cases when a time zone has an offset with
    /// fractional minutes, such as `Africa/Monrovia` before 1972.
    ///
    /// Because RFC 3339 and RFC 9557 do not support time zone offsets
    /// with fractional minutes, Jiff will serialize offsets with
    /// fractional minutes by rounding to the nearest minute. This
    /// will result in a different offset than what is actually
    /// used in the time zone. Parsing this _should_ succeed, but
    /// if exact offset equality is used, it won't. This is why a
    /// [`fmt::temporal::DateTimeParser`](crate::fmt::temporal::DateTimeParser)
    /// uses this routine with offset equality that rounds offsets to the
    /// nearest minute before comparison.
    ///
    /// ```
    /// use jiff::{civil::date, tz::{Offset, OffsetConflict, TimeZone}, Unit};
    ///
    /// let dt = date(1968, 2, 1).at(23, 15, 0, 0);
    /// let offset = Offset::from_seconds(-(44 * 60 + 30)).unwrap();
    /// let zdt = dt.in_tz("Africa/Monrovia")?;
    /// assert_eq!(zdt.offset(), offset);
    /// // Notice that the offset has been rounded!
    /// assert_eq!(zdt.to_string(), "1968-02-01T23:15:00-00:45[Africa/Monrovia]");
    ///
    /// // Now imagine parsing extracts the civil datetime, the offset and
    /// // the time zone, and then naively does exact offset comparison:
    /// let tz = TimeZone::get("Africa/Monrovia")?;
    /// // This is the parsed offset, which won't precisely match the actual
    /// // offset used by `Africa/Monrovia` at this time.
    /// let offset = Offset::from_seconds(-45 * 60).unwrap();
    /// let result = OffsetConflict::Reject.resolve(dt, offset, tz.clone());
    /// assert_eq!(
    ///     result.unwrap_err().to_string(),
    ///     "datetime 1968-02-01T23:15:00 could not resolve to a timestamp \
    ///      since 'reject' conflict resolution was chosen, and because \
    ///      datetime has offset -00:45, but the time zone Africa/Monrovia \
    ///      for the given datetime unambiguously has offset -00:44:30",
    /// );
    /// let is_equal = |parsed: Offset, candidate: Offset| {
    ///     parsed == candidate || candidate.round(Unit::Minute).map_or(
    ///         parsed == candidate,
    ///         |candidate| parsed == candidate,
    ///     )
    /// };
    /// let zdt = OffsetConflict::Reject.resolve_with(
    ///     dt,
    ///     offset,
    ///     tz.clone(),
    ///     is_equal,
    /// )?.unambiguous()?;
    /// // Notice that the offset is the actual offset from the time zone:
    /// assert_eq!(zdt.offset(), Offset::from_seconds(-(44 * 60 + 30)).unwrap());
    /// // But when we serialize, the offset gets rounded. If we didn't
    /// // do this, we'd risk the datetime not being parsable by other
    /// // implementations since RFC 3339 and RFC 9557 don't support fractional
    /// // minutes in the offset.
    /// assert_eq!(zdt.to_string(), "1968-02-01T23:15:00-00:45[Africa/Monrovia]");
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// And indeed, notice that parsing uses this same kind of offset equality
    /// to permit zoned datetimes whose offsets would be equivalent after
    /// rounding:
    ///
    /// ```
    /// use jiff::{tz::Offset, Zoned};
    ///
    /// let zdt: Zoned = "1968-02-01T23:15:00-00:45[Africa/Monrovia]".parse()?;
    /// // As above, notice that even though we parsed `-00:45` as the
    /// // offset, the actual offset of our zoned datetime is the correct
    /// // one from the time zone.
    /// assert_eq!(zdt.offset(), Offset::from_seconds(-(44 * 60 + 30)).unwrap());
    /// // And similarly, re-serializing it results in rounding the offset
    /// // again for compatibility with RFC 3339 and RFC 9557.
    /// assert_eq!(zdt.to_string(), "1968-02-01T23:15:00-00:45[Africa/Monrovia]");
    ///
    /// // And we also support parsing the actual fractional minute offset
    /// // as well:
    /// let zdt: Zoned = "1968-02-01T23:15:00-00:44:30[Africa/Monrovia]".parse()?;
    /// assert_eq!(zdt.offset(), Offset::from_seconds(-(44 * 60 + 30)).unwrap());
    /// assert_eq!(zdt.to_string(), "1968-02-01T23:15:00-00:45[Africa/Monrovia]");
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// Rounding does not occur when the parsed offset itself contains
    /// sub-minute precision. In that case, exact equality is used:
    ///
    /// ```
    /// use jiff::{tz::Offset, Zoned};
    ///
    /// let result = "1970-06-01T00-00:45:00[Africa/Monrovia]".parse::<Zoned>();
    /// assert_eq!(
    ///     result.unwrap_err().to_string(),
    ///     "parsing \"1970-06-01T00-00:45:00[Africa/Monrovia]\" failed: \
    ///      datetime 1970-06-01T00:00:00 could not resolve to a timestamp \
    ///      since 'reject' conflict resolution was chosen, and because \
    ///      datetime has offset -00:45, but the time zone Africa/Monrovia \
    ///      for the given datetime unambiguously has offset -00:44:30",
    /// );
    /// ```
    pub fn resolve_with<F>(
        self,
        dt: civil::DateTime,
        offset: Offset,
        tz: TimeZone,
        is_equal: F,
    ) -> Result<AmbiguousZoned, Error>
    where
        F: FnMut(Offset, Offset) -> bool,
    {
        match self {
            // In this case, we ignore any TZ annotation (although still
            // require that it exists) and always use the provided offset.
            OffsetConflict::AlwaysOffset => {
                let kind = AmbiguousOffset::Unambiguous { offset };
                Ok(AmbiguousTimestamp::new(dt, kind).into_ambiguous_zoned(tz))
            }
            // In this case, we ignore any provided offset and always use the
            // time zone annotation.
            OffsetConflict::AlwaysTimeZone => Ok(tz.into_ambiguous_zoned(dt)),
            // In this case, we use the offset if it's correct, but otherwise
            // fall back to the time zone annotation if it's not.
            OffsetConflict::PreferOffset => Ok(
                OffsetConflict::resolve_via_prefer(dt, offset, tz, is_equal),
            ),
            // In this case, if the offset isn't possible for the provided time
            // zone annotation, then we return an error.
            OffsetConflict::Reject => {
                OffsetConflict::resolve_via_reject(dt, offset, tz, is_equal)
            }
        }
    }

    /// Given a parsed datetime, a parsed offset and a parsed time zone, this
    /// attempts to resolve the datetime to a particular instant based on the
    /// 'prefer' strategy.
    ///
    /// In the 'prefer' strategy, we prefer to use the parsed offset to resolve
    /// any ambiguity in the parsed datetime and time zone, but only if the
    /// parsed offset is valid for the parsed datetime and time zone. If the
    /// parsed offset isn't valid, then it is ignored. In the case where it is
    /// ignored, it is possible for an ambiguous instant to be returned.
    fn resolve_via_prefer(
        dt: civil::DateTime,
        given: Offset,
        tz: TimeZone,
        mut is_equal: impl FnMut(Offset, Offset) -> bool,
    ) -> AmbiguousZoned {
        use crate::tz::AmbiguousOffset::*;

        let amb = tz.to_ambiguous_timestamp(dt);
        match amb.offset() {
            // We only look for folds because we consider all offsets for gaps
            // to be invalid. Which is consistent with how they're treated as
            // `OffsetConflict::Reject`. Thus, like any other invalid offset,
            // we fallback to disambiguation (which is handled by the caller).
            Fold { before, after }
                if is_equal(given, before) || is_equal(given, after) =>
            {
                let kind = Unambiguous { offset: given };
                AmbiguousTimestamp::new(dt, kind)
            }
            _ => amb,
        }
        .into_ambiguous_zoned(tz)
    }

    /// Given a parsed datetime, a parsed offset and a parsed time zone, this
    /// attempts to resolve the datetime to a particular instant based on the
    /// 'reject' strategy.
    ///
    /// That is, if the offset is not possibly valid for the given datetime and
    /// time zone, then this returns an error.
    ///
    /// This guarantees that on success, an unambiguous timestamp is returned.
    /// This occurs because if the datetime is ambiguous for the given time
    /// zone, then the parsed offset either matches one of the possible offsets
    /// (and thus provides an unambiguous choice), or it doesn't and an error
    /// is returned.
    fn resolve_via_reject(
        dt: civil::DateTime,
        given: Offset,
        tz: TimeZone,
        mut is_equal: impl FnMut(Offset, Offset) -> bool,
    ) -> Result<AmbiguousZoned, Error> {
        use crate::tz::AmbiguousOffset::*;

        let amb = tz.to_ambiguous_timestamp(dt);
        match amb.offset() {
            Unambiguous { offset } if !is_equal(given, offset) => Err(err!(
                "datetime {dt} could not resolve to a timestamp since \
                 'reject' conflict resolution was chosen, and because \
                 datetime has offset {given}, but the time zone {tzname} for \
                 the given datetime unambiguously has offset {offset}",
                tzname = tz.diagnostic_name(),
            )),
            Unambiguous { .. } => Ok(amb.into_ambiguous_zoned(tz)),
            Gap { before, after } => {
                // In `jiff 0.1`, we reported an error when we found a gap
                // where neither offset matched what was given. But now we
                // report an error whenever we find a gap, as we consider
                // all offsets to be invalid for the gap. This now matches
                // Temporal's behavior which I think is more consistent. And in
                // particular, this makes it more consistent with the behavior
                // of `PreferOffset` when a gap is found (which was also
                // changed to treat all offsets in a gap as invalid).
                //
                // Ref: https://github.com/tc39/proposal-temporal/issues/2892
                Err(err!(
                    "datetime {dt} could not resolve to timestamp \
                     since 'reject' conflict resolution was chosen, and \
                     because datetime has offset {given}, but the time \
                     zone {tzname} for the given datetime falls in a gap \
                     (between offsets {before} and {after}), and all \
                     offsets for a gap are regarded as invalid",
                    tzname = tz.diagnostic_name(),
                ))
            }
            Fold { before, after }
                if !is_equal(given, before) && !is_equal(given, after) =>
            {
                Err(err!(
                    "datetime {dt} could not resolve to timestamp \
                     since 'reject' conflict resolution was chosen, and \
                     because datetime has offset {given}, but the time \
                     zone {tzname} for the given datetime falls in a fold \
                     between offsets {before} and {after}, neither of which \
                     match the offset",
                    tzname = tz.diagnostic_name(),
                ))
            }
            Fold { .. } => {
                let kind = Unambiguous { offset: given };
                Ok(AmbiguousTimestamp::new(dt, kind).into_ambiguous_zoned(tz))
            }
        }
    }
}
