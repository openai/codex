use core::time::Duration as UnsignedDuration;

use crate::{
    civil::{
        Date, DateTime, DateTimeRound, DateTimeWith, Era, ISOWeekDate, Time,
        Weekday,
    },
    duration::{Duration, SDuration},
    error::{err, Error, ErrorContext},
    fmt::{
        self,
        temporal::{self, DEFAULT_DATETIME_PARSER},
    },
    tz::{AmbiguousOffset, Disambiguation, Offset, OffsetConflict, TimeZone},
    util::{
        rangeint::{RInto, TryRFrom},
        round::increment,
        t::{self, ZonedDayNanoseconds, C},
    },
    RoundMode, SignedDuration, Span, SpanRound, Timestamp, Unit,
};

/// A time zone aware instant in time.
///
/// A `Zoned` value can be thought of as the combination of following types,
/// all rolled into one:
///
/// * A [`Timestamp`] for indicating the precise instant in time.
/// * A [`DateTime`] for indicating the "civil" calendar date and clock time.
/// * A [`TimeZone`] for indicating how to apply time zone transitions while
/// performing arithmetic.
///
/// In particular, a `Zoned` is specifically designed for dealing with
/// datetimes in a time zone aware manner. Here are some highlights:
///
/// * Arithmetic automatically adjusts for daylight saving time (DST), using
/// the rules defined by [RFC 5545].
/// * Creating new `Zoned` values from other `Zoned` values via [`Zoned::with`]
/// by changing clock time (e.g., `02:30`) can do so without worrying that the
/// time will be invalid due to DST transitions.
/// * An approximate superset of the [`DateTime`] API is offered on `Zoned`,
/// but where each of its operations take time zone into account when
/// appropriate. For example, [`DateTime::start_of_day`] always returns a
/// datetime set to midnight, but [`Zoned::start_of_day`] returns the first
/// instant of a day, which might not be midnight if there is a time zone
/// transition at midnight.
/// * When using a `Zoned`, it is easy to switch between civil datetime (the
/// day you see on the calendar and the time you see on the clock) and Unix
/// time (a precise instant in time). Indeed, a `Zoned` can be losslessy
/// converted to any other datetime type in this crate: [`Timestamp`],
/// [`DateTime`], [`Date`] and [`Time`].
/// * A `Zoned` value can be losslessly serialized and deserialized, via
/// [serde], by adhering to [RFC 8536]. An example of a serialized zoned
/// datetime is `2024-07-04T08:39:00-04:00[America/New_York]`.
/// * Since a `Zoned` stores a [`TimeZone`] itself, multiple time zone aware
/// operations can be chained together without repeatedly specifying the time
/// zone.
///
/// [RFC 5545]: https://datatracker.ietf.org/doc/html/rfc5545
/// [RFC 8536]: https://datatracker.ietf.org/doc/html/rfc8536
/// [serde]: https://serde.rs/
///
/// # Parsing and printing
///
/// The `Zoned` type provides convenient trait implementations of
/// [`std::str::FromStr`] and [`std::fmt::Display`]:
///
/// ```
/// use jiff::Zoned;
///
/// let zdt: Zoned = "2024-06-19 15:22[America/New_York]".parse()?;
/// // Notice that the second component and the offset have both been added.
/// assert_eq!(zdt.to_string(), "2024-06-19T15:22:00-04:00[America/New_York]");
///
/// // While in the above case the datetime is unambiguous, in some cases, it
/// // can be ambiguous. In these cases, an offset is required to correctly
/// // roundtrip a zoned datetime. For example, on 2024-11-03 in New York, the
/// // 1 o'clock hour was repeated twice, corresponding to the end of daylight
/// // saving time.
/// //
/// // So because of the ambiguity, this time could be in offset -04 (the first
/// // time 1 o'clock is on the clock) or it could be -05 (the second time
/// // 1 o'clock is on the clock, corresponding to the end of DST).
/// //
/// // By default, parsing uses a "compatible" strategy for resolving all cases
/// // of ambiguity: in forward transitions (gaps), the later time is selected.
/// // And in backward transitions (folds), the earlier time is selected.
/// let zdt: Zoned = "2024-11-03 01:30[America/New_York]".parse()?;
/// // As we can see, since this was a fold, the earlier time was selected
/// // because the -04 offset is the first time 1 o'clock appears on the clock.
/// assert_eq!(zdt.to_string(), "2024-11-03T01:30:00-04:00[America/New_York]");
/// // But if we changed the offset and re-serialized, the only thing that
/// // changes is, indeed, the offset. This demonstrates that the offset is
/// // key to ensuring lossless serialization.
/// let zdt = zdt.with().offset(jiff::tz::offset(-5)).build()?;
/// assert_eq!(zdt.to_string(), "2024-11-03T01:30:00-05:00[America/New_York]");
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// A `Zoned` can also be parsed from just a time zone aware date (but the
/// time zone annotation is still required). In this case, the time is set to
/// midnight:
///
/// ```
/// use jiff::Zoned;
///
/// let zdt: Zoned = "2024-06-19[America/New_York]".parse()?;
/// assert_eq!(zdt.to_string(), "2024-06-19T00:00:00-04:00[America/New_York]");
/// // ... although it isn't always midnight, in the case of a time zone
/// // transition at midnight!
/// let zdt: Zoned = "2015-10-18[America/Sao_Paulo]".parse()?;
/// assert_eq!(zdt.to_string(), "2015-10-18T01:00:00-02:00[America/Sao_Paulo]");
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// For more information on the specific format supported, see the
/// [`fmt::temporal`](crate::fmt::temporal) module documentation.
///
/// # Leap seconds
///
/// Jiff does not support leap seconds. Jiff behaves as if they don't exist.
/// The only exception is that if one parses a datetime with a second component
/// of `60`, then it is automatically constrained to `59`:
///
/// ```
/// use jiff::{civil::date, Zoned};
///
/// let zdt: Zoned = "2016-12-31 23:59:60[Australia/Tasmania]".parse()?;
/// assert_eq!(zdt.datetime(), date(2016, 12, 31).at(23, 59, 59, 0));
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Comparisons
///
/// The `Zoned` type provides both `Eq` and `Ord` trait implementations to
/// facilitate easy comparisons. When a zoned datetime `zdt1` occurs before a
/// zoned datetime `zdt2`, then `zdt1 < zdt2`. For example:
///
/// ```
/// use jiff::civil::date;
///
/// let zdt1 = date(2024, 3, 11).at(1, 25, 15, 0).in_tz("America/New_York")?;
/// let zdt2 = date(2025, 1, 31).at(0, 30, 0, 0).in_tz("America/New_York")?;
/// assert!(zdt1 < zdt2);
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// Note that `Zoned` comparisons only consider the precise instant in time.
/// The civil datetime or even the time zone are completely ignored. So it's
/// possible for a zoned datetime to be less than another even if it's civil
/// datetime is bigger:
///
/// ```
/// use jiff::civil::date;
///
/// let zdt1 = date(2024, 7, 4).at(12, 0, 0, 0).in_tz("America/New_York")?;
/// let zdt2 = date(2024, 7, 4).at(11, 0, 0, 0).in_tz("America/Los_Angeles")?;
/// assert!(zdt1 < zdt2);
/// // But if we only compare civil datetime, the result is flipped:
/// assert!(zdt1.datetime() > zdt2.datetime());
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// The same applies for equality as well. Two `Zoned` values are equal, even
/// if they have different time zones, when the instant in time is identical:
///
/// ```
/// use jiff::civil::date;
///
/// let zdt1 = date(2024, 7, 4).at(12, 0, 0, 0).in_tz("America/New_York")?;
/// let zdt2 = date(2024, 7, 4).at(9, 0, 0, 0).in_tz("America/Los_Angeles")?;
/// assert_eq!(zdt1, zdt2);
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// (Note that this is diifferent from
/// [Temporal's `ZonedDateTime.equals`][temporal-equals] comparison, which will
/// take time zone into account for equality. This is because `Eq` and `Ord`
/// trait implementations must be consistent in Rust. If you need Temporal's
/// behavior, then use `zdt1 == zdt2 && zdt1.time_zone() == zdt2.time_zone()`.)
///
/// [temporal-equals]: https://tc39.es/proposal-temporal/docs/zoneddatetime.html#equals
///
/// # Arithmetic
///
/// This type provides routines for adding and subtracting spans of time, as
/// well as computing the span of time between two `Zoned` values. These
/// operations take time zones into account.
///
/// For adding or subtracting spans of time, one can use any of the following
/// routines:
///
/// * [`Zoned::checked_add`] or [`Zoned::checked_sub`] for checked
/// arithmetic.
/// * [`Zoned::saturating_add`] or [`Zoned::saturating_sub`] for
/// saturating arithmetic.
///
/// Additionally, checked arithmetic is available via the `Add` and `Sub`
/// trait implementations. When the result overflows, a panic occurs.
///
/// ```
/// use jiff::{civil::date, ToSpan};
///
/// let start = date(2024, 2, 25).at(15, 45, 0, 0).in_tz("America/New_York")?;
/// // `Zoned` doesn't implement `Copy`, so we use `&start` instead of `start`.
/// let one_week_later = &start + 1.weeks();
/// assert_eq!(one_week_later.datetime(), date(2024, 3, 3).at(15, 45, 0, 0));
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// One can compute the span of time between two zoned datetimes using either
/// [`Zoned::until`] or [`Zoned::since`]. It's also possible to subtract
/// two `Zoned` values directly via a `Sub` trait implementation:
///
/// ```
/// use jiff::{civil::date, ToSpan};
///
/// let zdt1 = date(2024, 5, 3).at(23, 30, 0, 0).in_tz("America/New_York")?;
/// let zdt2 = date(2024, 2, 25).at(7, 0, 0, 0).in_tz("America/New_York")?;
/// assert_eq!(&zdt1 - &zdt2, 1647.hours().minutes(30).fieldwise());
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// The `until` and `since` APIs are polymorphic and allow re-balancing and
/// rounding the span returned. For example, the default largest unit is hours
/// (as exemplified above), but we can ask for bigger units:
///
/// ```
/// use jiff::{civil::date, ToSpan, Unit};
///
/// let zdt1 = date(2024, 5, 3).at(23, 30, 0, 0).in_tz("America/New_York")?;
/// let zdt2 = date(2024, 2, 25).at(7, 0, 0, 0).in_tz("America/New_York")?;
/// assert_eq!(
///     zdt1.since((Unit::Year, &zdt2))?,
///     2.months().days(7).hours(16).minutes(30).fieldwise(),
/// );
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// Or even round the span returned:
///
/// ```
/// use jiff::{civil::date, RoundMode, ToSpan, Unit, ZonedDifference};
///
/// let zdt1 = date(2024, 5, 3).at(23, 30, 0, 0).in_tz("America/New_York")?;
/// let zdt2 = date(2024, 2, 25).at(7, 0, 0, 0).in_tz("America/New_York")?;
/// assert_eq!(
///     zdt1.since(
///         ZonedDifference::new(&zdt2)
///             .smallest(Unit::Day)
///             .largest(Unit::Year),
///     )?,
///     2.months().days(7).fieldwise(),
/// );
/// // `ZonedDifference` uses truncation as a rounding mode by default,
/// // but you can set the rounding mode to break ties away from zero:
/// assert_eq!(
///     zdt1.since(
///         ZonedDifference::new(&zdt2)
///             .smallest(Unit::Day)
///             .largest(Unit::Year)
///             .mode(RoundMode::HalfExpand),
///     )?,
///     // Rounds up to 8 days.
///     2.months().days(8).fieldwise(),
/// );
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Rounding
///
/// A `Zoned` can be rounded based on a [`ZonedRound`] configuration of
/// smallest units, rounding increment and rounding mode. Here's an example
/// showing how to round to the nearest third hour:
///
/// ```
/// use jiff::{civil::date, Unit, ZonedRound};
///
/// let zdt = date(2024, 6, 19)
///     .at(16, 27, 29, 999_999_999)
///     .in_tz("America/New_York")?;
/// assert_eq!(
///     zdt.round(ZonedRound::new().smallest(Unit::Hour).increment(3))?,
///     date(2024, 6, 19).at(15, 0, 0, 0).in_tz("America/New_York")?,
/// );
/// // Or alternatively, make use of the `From<(Unit, i64)> for ZonedRound`
/// // trait implementation:
/// assert_eq!(
///     zdt.round((Unit::Hour, 3))?,
///     date(2024, 6, 19).at(15, 0, 0, 0).in_tz("America/New_York")?,
/// );
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// See [`Zoned::round`] for more details.
#[derive(Clone)]
pub struct Zoned {
    inner: ZonedInner,
}

/// The representation of a `Zoned`.
///
/// This uses 4 different things: a timestamp, a datetime, an offset and a
/// time zone. This in turn makes `Zoned` a bit beefy (40 bytes on x86-64),
/// but I think this is probably the right trade off. (At time of writing,
/// 2024-07-04.)
///
/// Technically speaking, the only essential fields here are timestamp and time
/// zone. The datetime and offset can both be unambiguously _computed_ from the
/// combination of a timestamp and a time zone. Indeed, just the timestamp and
/// the time zone was my initial representation. But as I developed the API of
/// this type, it became clearer that we should probably store the datetime and
/// offset as well.
///
/// The main issue here is that in order to compute the datetime from a
/// timestamp and a time zone, you need to do two things:
///
/// 1. First, compute the offset. This means doing a binary search on the TZif
/// data for the transition (or closest transition) matching the timestamp.
/// 2. Second, use the offset (from UTC) to convert the timestamp into a civil
/// datetime. This involves a "Unix time to Unix epoch days" conversion that
/// requires some heavy arithmetic.
///
/// So if we don't store the datetime or offset, then we need to compute them
/// any time we need them. And the Temporal design really pushes heavily in
/// favor of treating the "instant in time" and "civil datetime" as two sides
/// to the same coin. That means users are very encouraged to just use whatever
/// they need. So if we are always computing the offset and datetime whenever
/// we need them, we're potentially punishing users for working with civil
/// datetimes. It just doesn't feel like the right trade-off.
///
/// Instead, my idea here is that, ultimately, `Zoned` is meant to provide
/// a one-stop shop for "doing the right thing." Presenting that unified
/// abstraction comes with costs. And that if we want to expose cheaper ways
/// of performing at least some of the operations on `Zoned` by making fewer
/// assumptions, then we should probably endeavor to do that by exposing a
/// lower level API. I'm not sure what that would look like, so I think it
/// should be driven by use cases.
///
/// Some other things I considered:
///
/// * Use `Zoned(Arc<ZonedInner>)` to make `Zoned` pointer-sized. But I didn't
/// like this because it implies creating any new `Zoned` value requires an
/// allocation. Since a `TimeZone` internally uses an `Arc`, all it requires
/// today is a chunky memcpy and an atomic ref count increment.
/// * Use `OnceLock` shenanigans for the datetime and offset fields. This would
/// make `Zoned` even beefier and I wasn't totally clear how much this would
/// save us. And it would impose some (probably small) cost on every datetime
/// or offset access.
/// * Use a radically different design that permits a `Zoned` to be `Copy`.
/// I personally find it deeply annoying that `Zoned` is both the "main"
/// datetime type in Jiff and also the only one that doesn't implement `Copy`.
/// I explored some designs, but I couldn't figure out how to make it work in
/// a satisfying way. The main issue here is `TimeZone`. A `TimeZone` is a huge
/// chunk of data and the ergonomics of the `Zoned` API require being able to
/// access a `TimeZone` without the caller providing it explicitly. So to me,
/// the only real alternative here is to use some kind of integer handle into
/// a global time zone database. But now you all of a sudden need to worry
/// about synchronization for every time zone access and plausibly also garbage
/// collection. And this also complicates matters for using custom time zone
/// databases. So I ultimately came down on "Zoned is not Copy" as the least
/// awful choice. *heavy sigh*
#[derive(Clone)]
struct ZonedInner {
    timestamp: Timestamp,
    datetime: DateTime,
    offset: Offset,
    time_zone: TimeZone,
}

impl Zoned {
    /// Returns the current system time in this system's time zone.
    ///
    /// If the system's time zone could not be found, then [`TimeZone::UTC`]
    /// is used instead. When this happens, a `WARN` level log message will
    /// be emitted. (To see it, one will need to install a logger that is
    /// compatible with the `log` crate and enable Jiff's `logging` Cargo
    /// feature.)
    ///
    /// To create a `Zoned` value for the current time in a particular
    /// time zone other than the system default time zone, use
    /// `Timestamp::now().to_zoned(time_zone)`. In particular, using
    /// [`Timestamp::now`] avoids the work required to fetch the system time
    /// zone if you did `Zoned::now().with_time_zone(time_zone)`.
    ///
    /// # Panics
    ///
    /// This panics if the system clock is set to a time value outside of the
    /// range `-009999-01-01T00:00:00Z..=9999-12-31T11:59:59.999999999Z`. The
    /// justification here is that it is reasonable to expect the system clock
    /// to be set to a somewhat sane, if imprecise, value.
    ///
    /// If you want to get the current Unix time fallibly, use
    /// [`Zoned::try_from`] with a `std::time::SystemTime` as input.
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
    /// use jiff::{Timestamp, Zoned};
    ///
    /// assert!(Zoned::now().timestamp() > Timestamp::UNIX_EPOCH);
    /// ```
    #[cfg(feature = "std")]
    #[inline]
    pub fn now() -> Zoned {
        Zoned::try_from(crate::now::system_time())
            .expect("system time is valid")
    }

    /// Creates a new `Zoned` value from a specific instant in a particular
    /// time zone. The time zone determines how to render the instant in time
    /// into civil time. (Also known as "clock," "wall," "local" or "naive"
    /// time.)
    ///
    /// To create a new zoned datetime from another with a particular field
    /// value, use the methods on [`ZonedWith`] via [`Zoned::with`].
    ///
    /// # Construction from civil time
    ///
    /// A `Zoned` value can also be created from a civil time via the following
    /// methods:
    ///
    /// * [`DateTime::in_tz`] does a Time Zone Database lookup given a time
    /// zone name string.
    /// * [`DateTime::to_zoned`] accepts a `TimeZone`.
    /// * [`Date::in_tz`] does a Time Zone Database lookup given a time zone
    /// name string and attempts to use midnight as the clock time.
    /// * [`Date::to_zoned`] accepts a `TimeZone` and attempts to use midnight
    /// as the clock time.
    ///
    /// Whenever one is converting from civil time to a zoned
    /// datetime, it is possible for the civil time to be ambiguous.
    /// That is, it might be a clock reading that could refer to
    /// multiple possible instants in time, or it might be a clock
    /// reading that never exists. The above routines will use a
    /// [`Disambiguation::Compatible`]
    /// strategy to automatically resolve these corner cases.
    ///
    /// If one wants to control how ambiguity is resolved (including
    /// by returning an error), use [`TimeZone::to_ambiguous_zoned`]
    /// and select the desired strategy via a method on
    /// [`AmbiguousZoned`](crate::tz::AmbiguousZoned).
    ///
    /// # Example: What was the civil time in Tasmania at the Unix epoch?
    ///
    /// ```
    /// use jiff::{tz::TimeZone, Timestamp, Zoned};
    ///
    /// let tz = TimeZone::get("Australia/Tasmania")?;
    /// let zdt = Zoned::new(Timestamp::UNIX_EPOCH, tz);
    /// assert_eq!(
    ///     zdt.to_string(),
    ///     "1970-01-01T11:00:00+11:00[Australia/Tasmania]",
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: What was the civil time in New York when World War 1 ended?
    ///
    /// ```
    /// use jiff::civil::date;
    ///
    /// let zdt1 = date(1918, 11, 11).at(11, 0, 0, 0).in_tz("Europe/Paris")?;
    /// let zdt2 = zdt1.in_tz("America/New_York")?;
    /// assert_eq!(
    ///     zdt2.to_string(),
    ///     "1918-11-11T06:00:00-05:00[America/New_York]",
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn new(timestamp: Timestamp, time_zone: TimeZone) -> Zoned {
        let offset = time_zone.to_offset(timestamp);
        let datetime = offset.to_datetime(timestamp);
        let inner = ZonedInner { timestamp, datetime, offset, time_zone };
        Zoned { inner }
    }

    /// A crate internal constructor for building a `Zoned` from its
    /// constituent parts.
    ///
    /// This should basically never be exposed, because it can be quite tricky
    /// to get the parts correct.
    ///
    /// See `civil::DateTime::to_zoned` for a use case for this routine. (Why
    /// do you think? Perf!)
    #[inline]
    pub(crate) fn from_parts(
        timestamp: Timestamp,
        time_zone: TimeZone,
        offset: Offset,
        datetime: DateTime,
    ) -> Zoned {
        let inner = ZonedInner { timestamp, datetime, offset, time_zone };
        Zoned { inner }
    }

    /// Create a builder for constructing a new `DateTime` from the fields of
    /// this datetime.
    ///
    /// See the methods on [`ZonedWith`] for the different ways one can set
    /// the fields of a new `Zoned`.
    ///
    /// Note that this doesn't support changing the time zone. If you want a
    /// `Zoned` value of the same instant but in a different time zone, use
    /// [`Zoned::in_tz`] or [`Zoned::with_time_zone`]. If you want a `Zoned`
    /// value of the same civil datetime (assuming it isn't ambiguous) but in
    /// a different time zone, then use [`Zoned::datetime`] followed by
    /// [`DateTime::in_tz`] or [`DateTime::to_zoned`].
    ///
    /// # Example
    ///
    /// The builder ensures one can chain together the individual components
    /// of a zoned datetime without it failing at an intermediate step. For
    /// example, if you had a date of `2024-10-31T00:00:00[America/New_York]`
    /// and wanted to change both the day and the month, and each setting was
    /// validated independent of the other, you would need to be careful to set
    /// the day first and then the month. In some cases, you would need to set
    /// the month first and then the day!
    ///
    /// But with the builder, you can set values in any order:
    ///
    /// ```
    /// use jiff::civil::date;
    ///
    /// let zdt1 = date(2024, 10, 31).at(0, 0, 0, 0).in_tz("America/New_York")?;
    /// let zdt2 = zdt1.with().month(11).day(30).build()?;
    /// assert_eq!(
    ///     zdt2,
    ///     date(2024, 11, 30).at(0, 0, 0, 0).in_tz("America/New_York")?,
    /// );
    ///
    /// let zdt1 = date(2024, 4, 30).at(0, 0, 0, 0).in_tz("America/New_York")?;
    /// let zdt2 = zdt1.with().day(31).month(7).build()?;
    /// assert_eq!(
    ///     zdt2,
    ///     date(2024, 7, 31).at(0, 0, 0, 0).in_tz("America/New_York")?,
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn with(&self) -> ZonedWith {
        ZonedWith::new(self.clone())
    }

    /// Return a new zoned datetime with precisely the same instant in a
    /// different time zone.
    ///
    /// The zoned datetime returned is guaranteed to have an equivalent
    /// [`Timestamp`]. However, its civil [`DateTime`] may be different.
    ///
    /// # Example: What was the civil time in New York when World War 1 ended?
    ///
    /// ```
    /// use jiff::{civil::date, tz::TimeZone};
    ///
    /// let from = TimeZone::get("Europe/Paris")?;
    /// let to = TimeZone::get("America/New_York")?;
    /// let zdt1 = date(1918, 11, 11).at(11, 0, 0, 0).to_zoned(from)?;
    /// // Switch zdt1 to a different time zone, but keeping the same instant
    /// // in time. The civil time changes, but not the instant!
    /// let zdt2 = zdt1.with_time_zone(to);
    /// assert_eq!(
    ///     zdt2.to_string(),
    ///     "1918-11-11T06:00:00-05:00[America/New_York]",
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn with_time_zone(&self, time_zone: TimeZone) -> Zoned {
        Zoned::new(self.timestamp(), time_zone)
    }

    /// Return a new zoned datetime with precisely the same instant in a
    /// different time zone.
    ///
    /// The zoned datetime returned is guaranteed to have an equivalent
    /// [`Timestamp`]. However, its civil [`DateTime`] may be different.
    ///
    /// The name given is resolved to a [`TimeZone`] by using the default
    /// [`TimeZoneDatabase`](crate::tz::TimeZoneDatabase) created by
    /// [`tz::db`](crate::tz::db). Indeed, this is a convenience function for
    /// [`DateTime::to_zoned`] where the time zone database lookup is done
    /// automatically.
    ///
    /// # Errors
    ///
    /// This returns an error when the given time zone name could not be found
    /// in the default time zone database.
    ///
    /// # Example: What was the civil time in New York when World War 1 ended?
    ///
    /// ```
    /// use jiff::civil::date;
    ///
    /// let zdt1 = date(1918, 11, 11).at(11, 0, 0, 0).in_tz("Europe/Paris")?;
    /// // Switch zdt1 to a different time zone, but keeping the same instant
    /// // in time. The civil time changes, but not the instant!
    /// let zdt2 = zdt1.in_tz("America/New_York")?;
    /// assert_eq!(
    ///     zdt2.to_string(),
    ///     "1918-11-11T06:00:00-05:00[America/New_York]",
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn in_tz(&self, name: &str) -> Result<Zoned, Error> {
        let tz = crate::tz::db().get(name)?;
        Ok(self.with_time_zone(tz))
    }

    /// Returns the time zone attached to this [`Zoned`] value.
    ///
    /// A time zone is more than just an offset. A time zone is a series of
    /// rules for determining the civil time for a corresponding instant.
    /// Indeed, a zoned datetime uses its time zone to perform zone-aware
    /// arithmetic, rounding and serialization.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::Zoned;
    ///
    /// let zdt: Zoned = "2024-07-03 14:31[america/new_york]".parse()?;
    /// assert_eq!(zdt.time_zone().iana_name(), Some("America/New_York"));
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn time_zone(&self) -> &TimeZone {
        &self.inner.time_zone
    }

    /// Returns the year for this zoned datetime.
    ///
    /// The value returned is guaranteed to be in the range `-9999..=9999`.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::civil::date;
    ///
    /// let zdt1 = date(2024, 3, 9).at(7, 30, 0, 0).in_tz("America/New_York")?;
    /// assert_eq!(zdt1.year(), 2024);
    ///
    /// let zdt2 = date(-2024, 3, 9).at(7, 30, 0, 0).in_tz("America/New_York")?;
    /// assert_eq!(zdt2.year(), -2024);
    ///
    /// let zdt3 = date(0, 3, 9).at(7, 30, 0, 0).in_tz("America/New_York")?;
    /// assert_eq!(zdt3.year(), 0);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn year(&self) -> i16 {
        self.date().year()
    }

    /// Returns the year and its era.
    ///
    /// This crate specifically allows years to be negative or `0`, where as
    /// years written for the Gregorian calendar are always positive and
    /// greater than `0`. In the Gregorian calendar, the era labels `BCE` and
    /// `CE` are used to disambiguate between years less than or equal to `0`
    /// and years greater than `0`, respectively.
    ///
    /// The crate is designed this way so that years in the latest era (that
    /// is, `CE`) are aligned with years in this crate.
    ///
    /// The year returned is guaranteed to be in the range `1..=10000`.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::civil::{Era, date};
    ///
    /// let zdt = date(2024, 10, 3).at(7, 30, 0, 0).in_tz("America/New_York")?;
    /// assert_eq!(zdt.era_year(), (2024, Era::CE));
    ///
    /// let zdt = date(1, 10, 3).at(7, 30, 0, 0).in_tz("America/New_York")?;
    /// assert_eq!(zdt.era_year(), (1, Era::CE));
    ///
    /// let zdt = date(0, 10, 3).at(7, 30, 0, 0).in_tz("America/New_York")?;
    /// assert_eq!(zdt.era_year(), (1, Era::BCE));
    ///
    /// let zdt = date(-1, 10, 3).at(7, 30, 0, 0).in_tz("America/New_York")?;
    /// assert_eq!(zdt.era_year(), (2, Era::BCE));
    ///
    /// let zdt = date(-10, 10, 3).at(7, 30, 0, 0).in_tz("America/New_York")?;
    /// assert_eq!(zdt.era_year(), (11, Era::BCE));
    ///
    /// let zdt = date(-9_999, 10, 3).at(7, 30, 0, 0).in_tz("America/New_York")?;
    /// assert_eq!(zdt.era_year(), (10_000, Era::BCE));
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn era_year(&self) -> (i16, Era) {
        self.date().era_year()
    }

    /// Returns the month for this zoned datetime.
    ///
    /// The value returned is guaranteed to be in the range `1..=12`.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::civil::date;
    ///
    /// let zdt = date(2024, 3, 9).at(7, 30, 0, 0).in_tz("America/New_York")?;
    /// assert_eq!(zdt.month(), 3);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn month(&self) -> i8 {
        self.date().month()
    }

    /// Returns the day for this zoned datetime.
    ///
    /// The value returned is guaranteed to be in the range `1..=31`.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::civil::date;
    ///
    /// let zdt = date(2024, 2, 29).at(7, 30, 0, 0).in_tz("America/New_York")?;
    /// assert_eq!(zdt.day(), 29);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn day(&self) -> i8 {
        self.date().day()
    }

    /// Returns the "hour" component of this zoned datetime.
    ///
    /// The value returned is guaranteed to be in the range `0..=23`.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::civil::date;
    ///
    /// let zdt = date(2000, 1, 2)
    ///     .at(3, 4, 5, 123_456_789)
    ///     .in_tz("America/New_York")?;
    /// assert_eq!(zdt.hour(), 3);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn hour(&self) -> i8 {
        self.time().hour()
    }

    /// Returns the "minute" component of this zoned datetime.
    ///
    /// The value returned is guaranteed to be in the range `0..=59`.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::civil::date;
    ///
    /// let zdt = date(2000, 1, 2)
    ///     .at(3, 4, 5, 123_456_789)
    ///     .in_tz("America/New_York")?;
    /// assert_eq!(zdt.minute(), 4);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn minute(&self) -> i8 {
        self.time().minute()
    }

    /// Returns the "second" component of this zoned datetime.
    ///
    /// The value returned is guaranteed to be in the range `0..=59`.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::civil::date;
    ///
    /// let zdt = date(2000, 1, 2)
    ///     .at(3, 4, 5, 123_456_789)
    ///     .in_tz("America/New_York")?;
    /// assert_eq!(zdt.second(), 5);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn second(&self) -> i8 {
        self.time().second()
    }

    /// Returns the "millisecond" component of this zoned datetime.
    ///
    /// The value returned is guaranteed to be in the range `0..=999`.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::civil::date;
    ///
    /// let zdt = date(2000, 1, 2)
    ///     .at(3, 4, 5, 123_456_789)
    ///     .in_tz("America/New_York")?;
    /// assert_eq!(zdt.millisecond(), 123);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn millisecond(&self) -> i16 {
        self.time().millisecond()
    }

    /// Returns the "microsecond" component of this zoned datetime.
    ///
    /// The value returned is guaranteed to be in the range `0..=999`.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::civil::date;
    ///
    /// let zdt = date(2000, 1, 2)
    ///     .at(3, 4, 5, 123_456_789)
    ///     .in_tz("America/New_York")?;
    /// assert_eq!(zdt.microsecond(), 456);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn microsecond(&self) -> i16 {
        self.time().microsecond()
    }

    /// Returns the "nanosecond" component of this zoned datetime.
    ///
    /// The value returned is guaranteed to be in the range `0..=999`.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::civil::date;
    ///
    /// let zdt = date(2000, 1, 2)
    ///     .at(3, 4, 5, 123_456_789)
    ///     .in_tz("America/New_York")?;
    /// assert_eq!(zdt.nanosecond(), 789);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn nanosecond(&self) -> i16 {
        self.time().nanosecond()
    }

    /// Returns the fractional nanosecond for this `Zoned` value.
    ///
    /// If you want to set this value on `Zoned`, then use
    /// [`ZonedWith::subsec_nanosecond`] via [`Zoned::with`].
    ///
    /// The value returned is guaranteed to be in the range `0..=999_999_999`.
    ///
    /// Note that this returns the fractional second associated with the civil
    /// time on this `Zoned` value. This is distinct from the fractional
    /// second on the underlying timestamp. A timestamp, for example, may be
    /// negative to indicate time before the Unix epoch. But a civil datetime
    /// can only have a negative year, while the remaining values are all
    /// semantically positive. See the examples below for how this can manifest
    /// in practice.
    ///
    /// # Example
    ///
    /// This shows the relationship between constructing a `Zoned` value
    /// with routines like `with().millisecond()` and accessing the entire
    /// fractional part as a nanosecond:
    ///
    /// ```
    /// use jiff::civil::date;
    ///
    /// let zdt1 = date(2000, 1, 2)
    ///     .at(3, 4, 5, 123_456_789)
    ///     .in_tz("America/New_York")?;
    /// assert_eq!(zdt1.subsec_nanosecond(), 123_456_789);
    ///
    /// let zdt2 = zdt1.with().millisecond(333).build()?;
    /// assert_eq!(zdt2.subsec_nanosecond(), 333_456_789);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: nanoseconds from a timestamp
    ///
    /// This shows how the fractional nanosecond part of a `Zoned` value
    /// manifests from a specific timestamp.
    ///
    /// ```
    /// use jiff::Timestamp;
    ///
    /// // 1,234 nanoseconds after the Unix epoch.
    /// let zdt = Timestamp::new(0, 1_234)?.in_tz("UTC")?;
    /// assert_eq!(zdt.subsec_nanosecond(), 1_234);
    /// // N.B. The timestamp's fractional second and the civil datetime's
    /// // fractional second happen to be equal here:
    /// assert_eq!(zdt.timestamp().subsec_nanosecond(), 1_234);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: fractional seconds can differ between timestamps and civil time
    ///
    /// This shows how a timestamp can have a different fractional second
    /// value than its corresponding `Zoned` value because of how the sign
    /// is handled:
    ///
    /// ```
    /// use jiff::{civil, Timestamp};
    ///
    /// // 1,234 nanoseconds before the Unix epoch.
    /// let zdt = Timestamp::new(0, -1_234)?.in_tz("UTC")?;
    /// // The timestamp's fractional second is what was given:
    /// assert_eq!(zdt.timestamp().subsec_nanosecond(), -1_234);
    /// // But the civil datetime's fractional second is equal to
    /// // `1_000_000_000 - 1_234`. This is because civil datetimes
    /// // represent times in strictly positive values, like it
    /// // would read on a clock.
    /// assert_eq!(zdt.subsec_nanosecond(), 999998766);
    /// // Looking at the other components of the time value might help.
    /// assert_eq!(zdt.hour(), 23);
    /// assert_eq!(zdt.minute(), 59);
    /// assert_eq!(zdt.second(), 59);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn subsec_nanosecond(&self) -> i32 {
        self.time().subsec_nanosecond()
    }

    /// Returns the weekday corresponding to this zoned datetime.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::civil::{Weekday, date};
    ///
    /// // The Unix epoch was on a Thursday.
    /// let zdt = date(1970, 1, 1).at(7, 30, 0, 0).in_tz("America/New_York")?;
    /// assert_eq!(zdt.weekday(), Weekday::Thursday);
    /// // One can also get the weekday as an offset in a variety of schemes.
    /// assert_eq!(zdt.weekday().to_monday_zero_offset(), 3);
    /// assert_eq!(zdt.weekday().to_monday_one_offset(), 4);
    /// assert_eq!(zdt.weekday().to_sunday_zero_offset(), 4);
    /// assert_eq!(zdt.weekday().to_sunday_one_offset(), 5);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn weekday(&self) -> Weekday {
        self.date().weekday()
    }

    /// Returns the ordinal day of the year that this zoned datetime resides
    /// in.
    ///
    /// For leap years, this always returns a value in the range `1..=366`.
    /// Otherwise, the value is in the range `1..=365`.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::civil::date;
    ///
    /// let zdt = date(2006, 8, 24).at(7, 30, 0, 0).in_tz("America/New_York")?;
    /// assert_eq!(zdt.day_of_year(), 236);
    ///
    /// let zdt = date(2023, 12, 31).at(7, 30, 0, 0).in_tz("America/New_York")?;
    /// assert_eq!(zdt.day_of_year(), 365);
    ///
    /// let zdt = date(2024, 12, 31).at(7, 30, 0, 0).in_tz("America/New_York")?;
    /// assert_eq!(zdt.day_of_year(), 366);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn day_of_year(&self) -> i16 {
        self.date().day_of_year()
    }

    /// Returns the ordinal day of the year that this zoned datetime resides
    /// in, but ignores leap years.
    ///
    /// That is, the range of possible values returned by this routine is
    /// `1..=365`, even if this date resides in a leap year. If this date is
    /// February 29, then this routine returns `None`.
    ///
    /// The value `365` always corresponds to the last day in the year,
    /// December 31, even for leap years.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::civil::date;
    ///
    /// let zdt = date(2006, 8, 24).at(7, 30, 0, 0).in_tz("America/New_York")?;
    /// assert_eq!(zdt.day_of_year_no_leap(), Some(236));
    ///
    /// let zdt = date(2023, 12, 31).at(7, 30, 0, 0).in_tz("America/New_York")?;
    /// assert_eq!(zdt.day_of_year_no_leap(), Some(365));
    ///
    /// let zdt = date(2024, 12, 31).at(7, 30, 0, 0).in_tz("America/New_York")?;
    /// assert_eq!(zdt.day_of_year_no_leap(), Some(365));
    ///
    /// let zdt = date(2024, 2, 29).at(7, 30, 0, 0).in_tz("America/New_York")?;
    /// assert_eq!(zdt.day_of_year_no_leap(), None);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn day_of_year_no_leap(&self) -> Option<i16> {
        self.date().day_of_year_no_leap()
    }

    /// Returns the beginning of the day, corresponding to `00:00:00` civil
    /// time, that this datetime resides in.
    ///
    /// While in nearly all cases the time returned will be `00:00:00`, it is
    /// possible for the time to be different from midnight if there is a time
    /// zone transition at midnight.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::{civil::date, Zoned};
    ///
    /// let zdt = date(2015, 10, 18).at(12, 0, 0, 0).in_tz("America/New_York")?;
    /// assert_eq!(
    ///     zdt.start_of_day()?.to_string(),
    ///     "2015-10-18T00:00:00-04:00[America/New_York]",
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: start of day may not be midnight
    ///
    /// In some time zones, gap transitions may begin at midnight. This implies
    /// that `00:xx:yy` does not exist on a clock in that time zone for that
    /// day.
    ///
    /// ```
    /// use jiff::{civil::date, Zoned};
    ///
    /// let zdt = date(2015, 10, 18).at(12, 0, 0, 0).in_tz("America/Sao_Paulo")?;
    /// assert_eq!(
    ///     zdt.start_of_day()?.to_string(),
    ///     // not midnight!
    ///     "2015-10-18T01:00:00-02:00[America/Sao_Paulo]",
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: error because of overflow
    ///
    /// In some cases, it's possible for `Zoned` value to be able to represent
    /// an instant in time later in the day for a particular time zone, but not
    /// earlier in the day. This can only occur near the minimum datetime value
    /// supported by Jiff.
    ///
    /// ```
    /// use jiff::{civil::date, tz::{TimeZone, Offset}, Zoned};
    ///
    /// // While -9999-01-03T04:00:00+25:59:59 is representable as a Zoned
    /// // value, the start of the corresponding day is not!
    /// let tz = TimeZone::fixed(Offset::MAX);
    /// let zdt = date(-9999, 1, 3).at(4, 0, 0, 0).to_zoned(tz.clone())?;
    /// assert!(zdt.start_of_day().is_err());
    /// // The next day works fine since -9999-01-04T00:00:00+25:59:59 is
    /// // representable.
    /// let zdt = date(-9999, 1, 4).at(15, 0, 0, 0).to_zoned(tz)?;
    /// assert_eq!(
    ///     zdt.start_of_day()?.datetime(),
    ///     date(-9999, 1, 4).at(0, 0, 0, 0),
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn start_of_day(&self) -> Result<Zoned, Error> {
        self.datetime().start_of_day().to_zoned(self.time_zone().clone())
    }

    /// Returns the end of the day, corresponding to `23:59:59.999999999` civil
    /// time, that this datetime resides in.
    ///
    /// While in nearly all cases the time returned will be
    /// `23:59:59.999999999`, it is possible for the time to be different if
    /// there is a time zone transition covering that time.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::civil::date;
    ///
    /// let zdt = date(2024, 7, 3)
    ///     .at(7, 30, 10, 123_456_789)
    ///     .in_tz("America/New_York")?;
    /// assert_eq!(
    ///     zdt.end_of_day()?,
    ///     date(2024, 7, 3)
    ///         .at(23, 59, 59, 999_999_999)
    ///         .in_tz("America/New_York")?,
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: error because of overflow
    ///
    /// In some cases, it's possible for `Zoned` value to be able to represent
    /// an instant in time earlier in the day for a particular time zone, but
    /// not later in the day. This can only occur near the maximum datetime
    /// value supported by Jiff.
    ///
    /// ```
    /// use jiff::{civil::date, tz::{TimeZone, Offset}, Zoned};
    ///
    /// // While 9999-12-30T01:30-04 is representable as a Zoned
    /// // value, the start of the corresponding day is not!
    /// let tz = TimeZone::get("America/New_York")?;
    /// let zdt = date(9999, 12, 30).at(1, 30, 0, 0).to_zoned(tz.clone())?;
    /// assert!(zdt.end_of_day().is_err());
    /// // The previous day works fine since 9999-12-29T23:59:59.999999999-04
    /// // is representable.
    /// let zdt = date(9999, 12, 29).at(1, 30, 0, 0).to_zoned(tz.clone())?;
    /// assert_eq!(
    ///     zdt.end_of_day()?,
    ///     date(9999, 12, 29)
    ///         .at(23, 59, 59, 999_999_999)
    ///         .in_tz("America/New_York")?,
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn end_of_day(&self) -> Result<Zoned, Error> {
        let end_of_civil_day = self.datetime().end_of_day();
        let ambts = self.time_zone().to_ambiguous_timestamp(end_of_civil_day);
        // I'm not sure if there are any real world cases where this matters,
        // but this is basically the reverse of `compatible`, so we write
        // it out ourselves. Basically, if the last civil datetime is in a
        // gap, then we want the earlier instant since the later instant must
        // necessarily be in the next day. And if the last civil datetime is
        // in a fold, then we want the later instant since both the earlier
        // and later instants are in the same calendar day and the later one
        // must be, well, later. In contrast, compatible mode takes the later
        // instant in a gap and the earlier instant in a fold. So we flip that
        // here.
        let offset = match ambts.offset() {
            AmbiguousOffset::Unambiguous { offset } => offset,
            AmbiguousOffset::Gap { after, .. } => after,
            AmbiguousOffset::Fold { after, .. } => after,
        };
        offset
            .to_timestamp(end_of_civil_day)
            .map(|ts| ts.to_zoned(self.time_zone().clone()))
    }

    /// Returns the first date of the month that this zoned datetime resides
    /// in.
    ///
    /// In most cases, the time in the zoned datetime returned remains
    /// unchanged. In some cases, the time may change if the time
    /// on the previous date was unambiguous (always true, since a
    /// `Zoned` is a precise instant in time) and the same clock time
    /// on the returned zoned datetime is ambiguous. In this case, the
    /// [`Disambiguation::Compatible`]
    /// strategy will be used to turn it into a precise instant. If you want to
    /// use a different disambiguation strategy, then use [`Zoned::datetime`]
    /// to get the civil datetime, then use [`DateTime::first_of_month`],
    /// then use [`TimeZone::to_ambiguous_zoned`] and apply your preferred
    /// disambiguation strategy.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::civil::date;
    ///
    /// let zdt = date(2024, 2, 29).at(7, 30, 0, 0).in_tz("America/New_York")?;
    /// assert_eq!(
    ///     zdt.first_of_month()?,
    ///     date(2024, 2, 1).at(7, 30, 0, 0).in_tz("America/New_York")?,
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn first_of_month(&self) -> Result<Zoned, Error> {
        self.datetime().first_of_month().to_zoned(self.time_zone().clone())
    }

    /// Returns the last date of the month that this zoned datetime resides in.
    ///
    /// In most cases, the time in the zoned datetime returned remains
    /// unchanged. In some cases, the time may change if the time
    /// on the previous date was unambiguous (always true, since a
    /// `Zoned` is a precise instant in time) and the same clock time
    /// on the returned zoned datetime is ambiguous. In this case, the
    /// [`Disambiguation::Compatible`]
    /// strategy will be used to turn it into a precise instant. If you want to
    /// use a different disambiguation strategy, then use [`Zoned::datetime`]
    /// to get the civil datetime, then use [`DateTime::last_of_month`],
    /// then use [`TimeZone::to_ambiguous_zoned`] and apply your preferred
    /// disambiguation strategy.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::civil::date;
    ///
    /// let zdt = date(2024, 2, 5).at(7, 30, 0, 0).in_tz("America/New_York")?;
    /// assert_eq!(
    ///     zdt.last_of_month()?,
    ///     date(2024, 2, 29).at(7, 30, 0, 0).in_tz("America/New_York")?,
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn last_of_month(&self) -> Result<Zoned, Error> {
        self.datetime().last_of_month().to_zoned(self.time_zone().clone())
    }

    /// Returns the ordinal number of the last day in the month in which this
    /// zoned datetime resides.
    ///
    /// This is phrased as "the ordinal number of the last day" instead of "the
    /// number of days" because some months may be missing days due to time
    /// zone transitions. However, this is extraordinarily rare.
    ///
    /// This is guaranteed to always return one of the following values,
    /// depending on the year and the month: 28, 29, 30 or 31.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::civil::date;
    ///
    /// let zdt = date(2024, 2, 10).at(7, 30, 0, 0).in_tz("America/New_York")?;
    /// assert_eq!(zdt.days_in_month(), 29);
    ///
    /// let zdt = date(2023, 2, 10).at(7, 30, 0, 0).in_tz("America/New_York")?;
    /// assert_eq!(zdt.days_in_month(), 28);
    ///
    /// let zdt = date(2024, 8, 15).at(7, 30, 0, 0).in_tz("America/New_York")?;
    /// assert_eq!(zdt.days_in_month(), 31);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: count of days in month
    ///
    /// In `Pacific/Apia`, December 2011 did not have a December 30. Instead,
    /// the calendar [skipped from December 29 right to December 31][samoa].
    ///
    /// If you really do need the count of days in a month in a time zone
    /// aware fashion, then it's possible to achieve through arithmetic:
    ///
    /// ```
    /// use jiff::{civil::date, RoundMode, ToSpan, Unit, ZonedDifference};
    ///
    /// let first_of_month = date(2011, 12, 1).in_tz("Pacific/Apia")?;
    /// assert_eq!(first_of_month.days_in_month(), 31);
    /// let one_month_later = first_of_month.checked_add(1.month())?;
    ///
    /// let options = ZonedDifference::new(&one_month_later)
    ///     .largest(Unit::Hour)
    ///     .smallest(Unit::Hour)
    ///     .mode(RoundMode::HalfExpand);
    /// let span = first_of_month.until(options)?;
    /// let days = ((span.get_hours() as f64) / 24.0).round() as i64;
    /// // Try the above in a different time zone, like America/New_York, and
    /// // you'll get 31 here.
    /// assert_eq!(days, 30);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// [samoa]: https://en.wikipedia.org/wiki/Time_in_Samoa#2011_time_zone_change
    #[inline]
    pub fn days_in_month(&self) -> i8 {
        self.date().days_in_month()
    }

    /// Returns the first date of the year that this zoned datetime resides in.
    ///
    /// In most cases, the time in the zoned datetime returned remains
    /// unchanged. In some cases, the time may change if the time
    /// on the previous date was unambiguous (always true, since a
    /// `Zoned` is a precise instant in time) and the same clock time
    /// on the returned zoned datetime is ambiguous. In this case, the
    /// [`Disambiguation::Compatible`]
    /// strategy will be used to turn it into a precise instant. If you want to
    /// use a different disambiguation strategy, then use [`Zoned::datetime`]
    /// to get the civil datetime, then use [`DateTime::first_of_year`],
    /// then use [`TimeZone::to_ambiguous_zoned`] and apply your preferred
    /// disambiguation strategy.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::civil::date;
    ///
    /// let zdt = date(2024, 2, 29).at(7, 30, 0, 0).in_tz("America/New_York")?;
    /// assert_eq!(
    ///     zdt.first_of_year()?,
    ///     date(2024, 1, 1).at(7, 30, 0, 0).in_tz("America/New_York")?,
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn first_of_year(&self) -> Result<Zoned, Error> {
        self.datetime().first_of_year().to_zoned(self.time_zone().clone())
    }

    /// Returns the last date of the year that this zoned datetime resides in.
    ///
    /// In most cases, the time in the zoned datetime returned remains
    /// unchanged. In some cases, the time may change if the time
    /// on the previous date was unambiguous (always true, since a
    /// `Zoned` is a precise instant in time) and the same clock time
    /// on the returned zoned datetime is ambiguous. In this case, the
    /// [`Disambiguation::Compatible`]
    /// strategy will be used to turn it into a precise instant. If you want to
    /// use a different disambiguation strategy, then use [`Zoned::datetime`]
    /// to get the civil datetime, then use [`DateTime::last_of_year`],
    /// then use [`TimeZone::to_ambiguous_zoned`] and apply your preferred
    /// disambiguation strategy.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::civil::date;
    ///
    /// let zdt = date(2024, 2, 5).at(7, 30, 0, 0).in_tz("America/New_York")?;
    /// assert_eq!(
    ///     zdt.last_of_year()?,
    ///     date(2024, 12, 31).at(7, 30, 0, 0).in_tz("America/New_York")?,
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn last_of_year(&self) -> Result<Zoned, Error> {
        self.datetime().last_of_year().to_zoned(self.time_zone().clone())
    }

    /// Returns the ordinal number of the last day in the year in which this
    /// zoned datetime resides.
    ///
    /// This is phrased as "the ordinal number of the last day" instead of "the
    /// number of days" because some years may be missing days due to time
    /// zone transitions. However, this is extraordinarily rare.
    ///
    /// This is guaranteed to always return either `365` or `366`.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::civil::date;
    ///
    /// let zdt = date(2024, 7, 10).at(7, 30, 0, 0).in_tz("America/New_York")?;
    /// assert_eq!(zdt.days_in_year(), 366);
    ///
    /// let zdt = date(2023, 7, 10).at(7, 30, 0, 0).in_tz("America/New_York")?;
    /// assert_eq!(zdt.days_in_year(), 365);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn days_in_year(&self) -> i16 {
        self.date().days_in_year()
    }

    /// Returns true if and only if the year in which this zoned datetime
    /// resides is a leap year.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::civil::date;
    ///
    /// let zdt = date(2024, 1, 1).at(7, 30, 0, 0).in_tz("America/New_York")?;
    /// assert!(zdt.in_leap_year());
    ///
    /// let zdt = date(2023, 12, 31).at(7, 30, 0, 0).in_tz("America/New_York")?;
    /// assert!(!zdt.in_leap_year());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn in_leap_year(&self) -> bool {
        self.date().in_leap_year()
    }

    /// Returns the zoned datetime with a date immediately following this one.
    ///
    /// In most cases, the time in the zoned datetime returned remains
    /// unchanged. In some cases, the time may change if the time
    /// on the previous date was unambiguous (always true, since a
    /// `Zoned` is a precise instant in time) and the same clock time
    /// on the returned zoned datetime is ambiguous. In this case, the
    /// [`Disambiguation::Compatible`]
    /// strategy will be used to turn it into a precise instant. If you want to
    /// use a different disambiguation strategy, then use [`Zoned::datetime`]
    /// to get the civil datetime, then use [`DateTime::tomorrow`],
    /// then use [`TimeZone::to_ambiguous_zoned`] and apply your preferred
    /// disambiguation strategy.
    ///
    /// # Errors
    ///
    /// This returns an error when one day following this zoned datetime would
    /// exceed the maximum `Zoned` value.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::{civil::date, Timestamp};
    ///
    /// let zdt = date(2024, 2, 28).at(7, 30, 0, 0).in_tz("America/New_York")?;
    /// assert_eq!(
    ///     zdt.tomorrow()?,
    ///     date(2024, 2, 29).at(7, 30, 0, 0).in_tz("America/New_York")?,
    /// );
    ///
    /// // The max doesn't have a tomorrow.
    /// assert!(Timestamp::MAX.in_tz("America/New_York")?.tomorrow().is_err());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: ambiguous datetimes are automatically resolved
    ///
    /// ```
    /// use jiff::{civil::date, Timestamp};
    ///
    /// let zdt = date(2024, 3, 9).at(2, 30, 0, 0).in_tz("America/New_York")?;
    /// assert_eq!(
    ///     zdt.tomorrow()?,
    ///     date(2024, 3, 10).at(3, 30, 0, 0).in_tz("America/New_York")?,
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn tomorrow(&self) -> Result<Zoned, Error> {
        self.datetime().tomorrow()?.to_zoned(self.time_zone().clone())
    }

    /// Returns the zoned datetime with a date immediately preceding this one.
    ///
    /// In most cases, the time in the zoned datetime returned remains
    /// unchanged. In some cases, the time may change if the time
    /// on the previous date was unambiguous (always true, since a
    /// `Zoned` is a precise instant in time) and the same clock time
    /// on the returned zoned datetime is ambiguous. In this case, the
    /// [`Disambiguation::Compatible`]
    /// strategy will be used to turn it into a precise instant. If you want to
    /// use a different disambiguation strategy, then use [`Zoned::datetime`]
    /// to get the civil datetime, then use [`DateTime::yesterday`],
    /// then use [`TimeZone::to_ambiguous_zoned`] and apply your preferred
    /// disambiguation strategy.
    ///
    /// # Errors
    ///
    /// This returns an error when one day preceding this zoned datetime would
    /// be less than the minimum `Zoned` value.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::{civil::date, Timestamp};
    ///
    /// let zdt = date(2024, 3, 1).at(7, 30, 0, 0).in_tz("America/New_York")?;
    /// assert_eq!(
    ///     zdt.yesterday()?,
    ///     date(2024, 2, 29).at(7, 30, 0, 0).in_tz("America/New_York")?,
    /// );
    ///
    /// // The min doesn't have a yesterday.
    /// assert!(Timestamp::MIN.in_tz("America/New_York")?.yesterday().is_err());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: ambiguous datetimes are automatically resolved
    ///
    /// ```
    /// use jiff::{civil::date, Timestamp};
    ///
    /// let zdt = date(2024, 11, 4).at(1, 30, 0, 0).in_tz("America/New_York")?;
    /// assert_eq!(
    ///     zdt.yesterday()?.to_string(),
    ///     // Consistent with the "compatible" disambiguation strategy, the
    ///     // "first" 1 o'clock hour is selected. You can tell this because
    ///     // the offset is -04, which corresponds to DST time in New York.
    ///     // The second 1 o'clock hour would have offset -05.
    ///     "2024-11-03T01:30:00-04:00[America/New_York]",
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn yesterday(&self) -> Result<Zoned, Error> {
        self.datetime().yesterday()?.to_zoned(self.time_zone().clone())
    }

    /// Returns the "nth" weekday from the beginning or end of the month in
    /// which this zoned datetime resides.
    ///
    /// The `nth` parameter can be positive or negative. A positive value
    /// computes the "nth" weekday from the beginning of the month. A negative
    /// value computes the "nth" weekday from the end of the month. So for
    /// example, use `-1` to "find the last weekday" in this date's month.
    ///
    /// In most cases, the time in the zoned datetime returned remains
    /// unchanged. In some cases, the time may change if the time
    /// on the previous date was unambiguous (always true, since a
    /// `Zoned` is a precise instant in time) and the same clock time
    /// on the returned zoned datetime is ambiguous. In this case, the
    /// [`Disambiguation::Compatible`]
    /// strategy will be used to turn it into a precise instant. If you want to
    /// use a different disambiguation strategy, then use [`Zoned::datetime`]
    /// to get the civil datetime, then use [`DateTime::nth_weekday_of_month`],
    /// then use [`TimeZone::to_ambiguous_zoned`] and apply your preferred
    /// disambiguation strategy.
    ///
    /// # Errors
    ///
    /// This returns an error when `nth` is `0`, or if it is `5` or `-5` and
    /// there is no 5th weekday from the beginning or end of the month. This
    /// could also return an error if the corresponding datetime could not be
    /// represented as an instant for this `Zoned`'s time zone. (This can only
    /// happen close the boundaries of an [`Timestamp`].)
    ///
    /// # Example
    ///
    /// This shows how to get the nth weekday in a month, starting from the
    /// beginning of the month:
    ///
    /// ```
    /// use jiff::civil::{Weekday, date};
    ///
    /// let zdt = date(2017, 3, 1).at(7, 30, 0, 0).in_tz("America/New_York")?;
    /// let second_friday = zdt.nth_weekday_of_month(2, Weekday::Friday)?;
    /// assert_eq!(
    ///     second_friday,
    ///     date(2017, 3, 10).at(7, 30, 0, 0).in_tz("America/New_York")?,
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// This shows how to do the reverse of the above. That is, the nth _last_
    /// weekday in a month:
    ///
    /// ```
    /// use jiff::civil::{Weekday, date};
    ///
    /// let zdt = date(2024, 3, 1).at(7, 30, 0, 0).in_tz("America/New_York")?;
    /// let last_thursday = zdt.nth_weekday_of_month(-1, Weekday::Thursday)?;
    /// assert_eq!(
    ///     last_thursday,
    ///     date(2024, 3, 28).at(7, 30, 0, 0).in_tz("America/New_York")?,
    /// );
    ///
    /// let second_last_thursday = zdt.nth_weekday_of_month(
    ///     -2,
    ///     Weekday::Thursday,
    /// )?;
    /// assert_eq!(
    ///     second_last_thursday,
    ///     date(2024, 3, 21).at(7, 30, 0, 0).in_tz("America/New_York")?,
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// This routine can return an error if there isn't an `nth` weekday
    /// for this month. For example, March 2024 only has 4 Mondays:
    ///
    /// ```
    /// use jiff::civil::{Weekday, date};
    ///
    /// let zdt = date(2024, 3, 25).at(7, 30, 0, 0).in_tz("America/New_York")?;
    /// let fourth_monday = zdt.nth_weekday_of_month(4, Weekday::Monday)?;
    /// assert_eq!(
    ///     fourth_monday,
    ///     date(2024, 3, 25).at(7, 30, 0, 0).in_tz("America/New_York")?,
    /// );
    /// // There is no 5th Monday.
    /// assert!(zdt.nth_weekday_of_month(5, Weekday::Monday).is_err());
    /// // Same goes for counting backwards.
    /// assert!(zdt.nth_weekday_of_month(-5, Weekday::Monday).is_err());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn nth_weekday_of_month(
        &self,
        nth: i8,
        weekday: Weekday,
    ) -> Result<Zoned, Error> {
        self.datetime()
            .nth_weekday_of_month(nth, weekday)?
            .to_zoned(self.time_zone().clone())
    }

    /// Returns the "nth" weekday from this zoned datetime, not including
    /// itself.
    ///
    /// The `nth` parameter can be positive or negative. A positive value
    /// computes the "nth" weekday starting at the day after this date and
    /// going forwards in time. A negative value computes the "nth" weekday
    /// starting at the day before this date and going backwards in time.
    ///
    /// For example, if this zoned datetime's weekday is a Sunday and the first
    /// Sunday is asked for (that is, `zdt.nth_weekday(1, Weekday::Sunday)`),
    /// then the result is a week from this zoned datetime corresponding to the
    /// following Sunday.
    ///
    /// In most cases, the time in the zoned datetime returned remains
    /// unchanged. In some cases, the time may change if the time
    /// on the previous date was unambiguous (always true, since a
    /// `Zoned` is a precise instant in time) and the same clock time
    /// on the returned zoned datetime is ambiguous. In this case, the
    /// [`Disambiguation::Compatible`]
    /// strategy will be used to turn it into a precise instant. If you want to
    /// use a different disambiguation strategy, then use [`Zoned::datetime`]
    /// to get the civil datetime, then use [`DateTime::nth_weekday`],
    /// then use [`TimeZone::to_ambiguous_zoned`] and apply your preferred
    /// disambiguation strategy.
    ///
    /// # Errors
    ///
    /// This returns an error when `nth` is `0`, or if it would otherwise
    /// result in a date that overflows the minimum/maximum values of
    /// `Zoned`.
    ///
    /// # Example
    ///
    /// This example shows how to find the "nth" weekday going forwards in
    /// time:
    ///
    /// ```
    /// use jiff::civil::{Weekday, date};
    ///
    /// // Use a Sunday in March as our start date.
    /// let zdt = date(2024, 3, 10).at(7, 30, 0, 0).in_tz("America/New_York")?;
    /// assert_eq!(zdt.weekday(), Weekday::Sunday);
    ///
    /// // The first next Monday is tomorrow!
    /// let next_monday = zdt.nth_weekday(1, Weekday::Monday)?;
    /// assert_eq!(
    ///     next_monday,
    ///     date(2024, 3, 11).at(7, 30, 0, 0).in_tz("America/New_York")?,
    /// );
    ///
    /// // But the next Sunday is a week away, because this doesn't
    /// // include the current weekday.
    /// let next_sunday = zdt.nth_weekday(1, Weekday::Sunday)?;
    /// assert_eq!(
    ///     next_sunday,
    ///     date(2024, 3, 17).at(7, 30, 0, 0).in_tz("America/New_York")?,
    /// );
    ///
    /// // "not this Thursday, but next Thursday"
    /// let next_next_thursday = zdt.nth_weekday(2, Weekday::Thursday)?;
    /// assert_eq!(
    ///     next_next_thursday,
    ///     date(2024, 3, 21).at(7, 30, 0, 0).in_tz("America/New_York")?,
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// This example shows how to find the "nth" weekday going backwards in
    /// time:
    ///
    /// ```
    /// use jiff::civil::{Weekday, date};
    ///
    /// // Use a Sunday in March as our start date.
    /// let zdt = date(2024, 3, 10).at(7, 30, 0, 0).in_tz("America/New_York")?;
    /// assert_eq!(zdt.weekday(), Weekday::Sunday);
    ///
    /// // "last Saturday" was yesterday!
    /// let last_saturday = zdt.nth_weekday(-1, Weekday::Saturday)?;
    /// assert_eq!(
    ///     last_saturday,
    ///     date(2024, 3, 9).at(7, 30, 0, 0).in_tz("America/New_York")?,
    /// );
    ///
    /// // "last Sunday" was a week ago.
    /// let last_sunday = zdt.nth_weekday(-1, Weekday::Sunday)?;
    /// assert_eq!(
    ///     last_sunday,
    ///     date(2024, 3, 3).at(7, 30, 0, 0).in_tz("America/New_York")?,
    /// );
    ///
    /// // "not last Thursday, but the one before"
    /// let prev_prev_thursday = zdt.nth_weekday(-2, Weekday::Thursday)?;
    /// assert_eq!(
    ///     prev_prev_thursday,
    ///     date(2024, 2, 29).at(7, 30, 0, 0).in_tz("America/New_York")?,
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// This example shows that overflow results in an error in either
    /// direction:
    ///
    /// ```
    /// use jiff::{civil::Weekday, Timestamp};
    ///
    /// let zdt = Timestamp::MAX.in_tz("America/New_York")?;
    /// assert_eq!(zdt.weekday(), Weekday::Thursday);
    /// assert!(zdt.nth_weekday(1, Weekday::Saturday).is_err());
    ///
    /// let zdt = Timestamp::MIN.in_tz("America/New_York")?;
    /// assert_eq!(zdt.weekday(), Weekday::Monday);
    /// assert!(zdt.nth_weekday(-1, Weekday::Sunday).is_err());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: getting the start of the week
    ///
    /// Given a date, one can use `nth_weekday` to determine the start of the
    /// week in which the date resides in. This might vary based on whether
    /// the weeks start on Sunday or Monday. This example shows how to handle
    /// both.
    ///
    /// ```
    /// use jiff::civil::{Weekday, date};
    ///
    /// let zdt = date(2024, 3, 15).at(7, 30, 0, 0).in_tz("America/New_York")?;
    /// // For weeks starting with Sunday.
    /// let start_of_week = zdt.tomorrow()?.nth_weekday(-1, Weekday::Sunday)?;
    /// assert_eq!(
    ///     start_of_week,
    ///     date(2024, 3, 10).at(7, 30, 0, 0).in_tz("America/New_York")?,
    /// );
    /// // For weeks starting with Monday.
    /// let start_of_week = zdt.tomorrow()?.nth_weekday(-1, Weekday::Monday)?;
    /// assert_eq!(
    ///     start_of_week,
    ///     date(2024, 3, 11).at(7, 30, 0, 0).in_tz("America/New_York")?,
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// In the above example, we first get the date after the current one
    /// because `nth_weekday` does not consider itself when counting. This
    /// works as expected even at the boundaries of a week:
    ///
    /// ```
    /// use jiff::civil::{Time, Weekday, date};
    ///
    /// // The start of the week.
    /// let zdt = date(2024, 3, 10).at(0, 0, 0, 0).in_tz("America/New_York")?;
    /// let start_of_week = zdt.tomorrow()?.nth_weekday(-1, Weekday::Sunday)?;
    /// assert_eq!(
    ///     start_of_week,
    ///     date(2024, 3, 10).at(0, 0, 0, 0).in_tz("America/New_York")?,
    /// );
    /// // The end of the week.
    /// let zdt = date(2024, 3, 16)
    ///     .at(23, 59, 59, 999_999_999)
    ///     .in_tz("America/New_York")?;
    /// let start_of_week = zdt
    ///     .tomorrow()?
    ///     .nth_weekday(-1, Weekday::Sunday)?
    ///     .with().time(Time::midnight()).build()?;
    /// assert_eq!(
    ///     start_of_week,
    ///     date(2024, 3, 10).at(0, 0, 0, 0).in_tz("America/New_York")?,
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn nth_weekday(
        &self,
        nth: i32,
        weekday: Weekday,
    ) -> Result<Zoned, Error> {
        self.datetime()
            .nth_weekday(nth, weekday)?
            .to_zoned(self.time_zone().clone())
    }

    /// Returns the precise instant in time referred to by this zoned datetime.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::civil::date;
    ///
    /// let zdt = date(2024, 3, 14).at(18, 45, 0, 0).in_tz("America/New_York")?;
    /// assert_eq!(zdt.timestamp().as_second(), 1_710_456_300);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn timestamp(&self) -> Timestamp {
        self.inner.timestamp
    }

    /// Returns the civil datetime component of this zoned datetime.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::civil::date;
    ///
    /// let zdt = date(2024, 3, 14).at(18, 45, 0, 0).in_tz("America/New_York")?;
    /// assert_eq!(zdt.datetime(), date(2024, 3, 14).at(18, 45, 0, 0));
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn datetime(&self) -> DateTime {
        self.inner.datetime
    }

    /// Returns the civil date component of this zoned datetime.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::civil::date;
    ///
    /// let zdt = date(2024, 3, 14).at(18, 45, 0, 0).in_tz("America/New_York")?;
    /// assert_eq!(zdt.date(), date(2024, 3, 14));
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn date(&self) -> Date {
        self.datetime().date()
    }

    /// Returns the civil time component of this zoned datetime.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::civil::{date, time};
    ///
    /// let zdt = date(2024, 3, 14).at(18, 45, 0, 0).in_tz("America/New_York")?;
    /// assert_eq!(zdt.time(), time(18, 45, 0, 0));
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn time(&self) -> Time {
        self.datetime().time()
    }

    /// Construct a civil [ISO 8601 week date] from this zoned datetime.
    ///
    /// The [`ISOWeekDate`] type describes itself in more detail, but in
    /// brief, the ISO week date calendar system eschews months in favor of
    /// weeks.
    ///
    /// This routine is equivalent to
    /// [`ISOWeekDate::from_date(zdt.date())`](ISOWeekDate::from_date).
    ///
    /// [ISO 8601 week date]: https://en.wikipedia.org/wiki/ISO_week_date
    ///
    /// # Example
    ///
    /// This shows a number of examples demonstrating the conversion from a
    /// Gregorian date to an ISO 8601 week date:
    ///
    /// ```
    /// use jiff::civil::{Date, Time, Weekday, date};
    ///
    /// let zdt = date(1995, 1, 1).at(18, 45, 0, 0).in_tz("US/Eastern")?;
    /// let weekdate = zdt.iso_week_date();
    /// assert_eq!(weekdate.year(), 1994);
    /// assert_eq!(weekdate.week(), 52);
    /// assert_eq!(weekdate.weekday(), Weekday::Sunday);
    ///
    /// let zdt = date(1996, 12, 31).at(18, 45, 0, 0).in_tz("US/Eastern")?;
    /// let weekdate = zdt.iso_week_date();
    /// assert_eq!(weekdate.year(), 1997);
    /// assert_eq!(weekdate.week(), 1);
    /// assert_eq!(weekdate.weekday(), Weekday::Tuesday);
    ///
    /// let zdt = date(2019, 12, 30).at(18, 45, 0, 0).in_tz("US/Eastern")?;
    /// let weekdate = zdt.iso_week_date();
    /// assert_eq!(weekdate.year(), 2020);
    /// assert_eq!(weekdate.week(), 1);
    /// assert_eq!(weekdate.weekday(), Weekday::Monday);
    ///
    /// let zdt = date(2024, 3, 9).at(18, 45, 0, 0).in_tz("US/Eastern")?;
    /// let weekdate = zdt.iso_week_date();
    /// assert_eq!(weekdate.year(), 2024);
    /// assert_eq!(weekdate.week(), 10);
    /// assert_eq!(weekdate.weekday(), Weekday::Saturday);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn iso_week_date(self) -> ISOWeekDate {
        self.date().iso_week_date()
    }

    /// Returns the time zone offset of this zoned datetime.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::civil::date;
    ///
    /// let zdt = date(2024, 2, 14).at(18, 45, 0, 0).in_tz("America/New_York")?;
    /// // -05 because New York is in "standard" time at this point.
    /// assert_eq!(zdt.offset(), jiff::tz::offset(-5));
    ///
    /// let zdt = date(2024, 7, 14).at(18, 45, 0, 0).in_tz("America/New_York")?;
    /// // But we get -04 once "summer" or "daylight saving time" starts.
    /// assert_eq!(zdt.offset(), jiff::tz::offset(-4));
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn offset(&self) -> Offset {
        self.inner.offset
    }

    /// Add the given span of time to this zoned datetime. If the sum would
    /// overflow the minimum or maximum zoned datetime values, then an error is
    /// returned.
    ///
    /// This operation accepts three different duration types: [`Span`],
    /// [`SignedDuration`] or [`std::time::Duration`]. This is achieved via
    /// `From` trait implementations for the [`ZonedArithmetic`] type.
    ///
    /// # Properties
    ///
    /// This routine is _not_ reversible because some additions may
    /// be ambiguous. For example, adding `1 month` to the zoned
    /// datetime `2024-03-31T00:00:00[America/New_York]` will produce
    /// `2024-04-30T00:00:00[America/New_York]` since April has
    /// only 30 days in a month. Moreover, subtracting `1 month`
    /// from `2024-04-30T00:00:00[America/New_York]` will produce
    /// `2024-03-30T00:00:00[America/New_York]`, which is not the date we
    /// started with.
    ///
    /// A similar argument applies for days, since with zoned datetimes,
    /// different days can be different lengths.
    ///
    /// If spans of time are limited to units of hours (or less), then this
    /// routine _is_ reversible. This also implies that all operations with a
    /// [`SignedDuration`] or a [`std::time::Duration`] are reversible.
    ///
    /// # Errors
    ///
    /// If the span added to this zoned datetime would result in a zoned
    /// datetime that exceeds the range of a `Zoned`, then this will return an
    /// error.
    ///
    /// # Example
    ///
    /// This shows a few examples of adding spans of time to various zoned
    /// datetimes. We make use of the [`ToSpan`](crate::ToSpan) trait for
    /// convenient creation of spans.
    ///
    /// ```
    /// use jiff::{civil::date, ToSpan};
    ///
    /// let zdt = date(1995, 12, 7)
    ///     .at(3, 24, 30, 3_500)
    ///     .in_tz("America/New_York")?;
    /// let got = zdt.checked_add(20.years().months(4).nanoseconds(500))?;
    /// assert_eq!(
    ///     got,
    ///     date(2016, 4, 7).at(3, 24, 30, 4_000).in_tz("America/New_York")?,
    /// );
    ///
    /// let zdt = date(2019, 1, 31).at(15, 30, 0, 0).in_tz("America/New_York")?;
    /// let got = zdt.checked_add(1.months())?;
    /// assert_eq!(
    ///     got,
    ///     date(2019, 2, 28).at(15, 30, 0, 0).in_tz("America/New_York")?,
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: available via addition operator
    ///
    /// This routine can be used via the `+` operator. Note though that if it
    /// fails, it will result in a panic. Note that we use `&zdt + ...` instead
    /// of `zdt + ...` since `Add` is implemented for `&Zoned` and not `Zoned`.
    /// This is because `Zoned` is not `Copy`.
    ///
    /// ```
    /// use jiff::{civil::date, ToSpan};
    ///
    /// let zdt = date(1995, 12, 7)
    ///     .at(3, 24, 30, 3_500)
    ///     .in_tz("America/New_York")?;
    /// let got = &zdt + 20.years().months(4).nanoseconds(500);
    /// assert_eq!(
    ///     got,
    ///     date(2016, 4, 7).at(3, 24, 30, 4_000).in_tz("America/New_York")?,
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: zone aware arithmetic
    ///
    /// This example demonstrates the difference between "add 1 day" and
    /// "add 24 hours." In the former case, 1 day might not correspond to 24
    /// hours if there is a time zone transition in the intervening period.
    /// However, adding 24 hours always means adding exactly 24 hours.
    ///
    /// ```
    /// use jiff::{civil::date, ToSpan};
    ///
    /// let zdt = date(2024, 3, 10).at(0, 0, 0, 0).in_tz("America/New_York")?;
    ///
    /// let one_day_later = zdt.checked_add(1.day())?;
    /// assert_eq!(
    ///     one_day_later.to_string(),
    ///     "2024-03-11T00:00:00-04:00[America/New_York]",
    /// );
    ///
    /// let twenty_four_hours_later = zdt.checked_add(24.hours())?;
    /// assert_eq!(
    ///     twenty_four_hours_later.to_string(),
    ///     "2024-03-11T01:00:00-04:00[America/New_York]",
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: automatic disambiguation
    ///
    /// This example demonstrates what happens when adding a span
    /// of time results in an ambiguous zoned datetime. Zone aware
    /// arithmetic uses automatic disambiguation corresponding to the
    /// [`Disambiguation::Compatible`]
    /// strategy for resolving an ambiguous datetime to a precise instant.
    /// For example, in the case below, there is a gap in the clocks for 1
    /// hour starting at `2024-03-10 02:00:00` in `America/New_York`. The
    /// "compatible" strategy chooses the later time in a gap:.
    ///
    /// ```
    /// use jiff::{civil::date, ToSpan};
    ///
    /// let zdt = date(2024, 3, 9).at(2, 30, 0, 0).in_tz("America/New_York")?;
    /// let one_day_later = zdt.checked_add(1.day())?;
    /// assert_eq!(
    ///     one_day_later.to_string(),
    ///     "2024-03-10T03:30:00-04:00[America/New_York]",
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// And this example demonstrates the "compatible" strategy when arithmetic
    /// results in an ambiguous datetime in a fold. In this case, we make use
    /// of the fact that the 1 o'clock hour was repeated on `2024-11-03`.
    ///
    /// ```
    /// use jiff::{civil::date, ToSpan};
    ///
    /// let zdt = date(2024, 11, 2).at(1, 30, 0, 0).in_tz("America/New_York")?;
    /// let one_day_later = zdt.checked_add(1.day())?;
    /// assert_eq!(
    ///     one_day_later.to_string(),
    ///     // This corresponds to the first iteration of the 1 o'clock hour,
    ///     // i.e., when DST is still in effect. It's the earlier time.
    ///     "2024-11-03T01:30:00-04:00[America/New_York]",
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: negative spans are supported
    ///
    /// ```
    /// use jiff::{civil::date, ToSpan};
    ///
    /// let zdt = date(2024, 3, 31)
    ///     .at(19, 5, 59, 999_999_999)
    ///     .in_tz("America/New_York")?;
    /// assert_eq!(
    ///     zdt.checked_add(-1.months())?,
    ///     date(2024, 2, 29).
    ///         at(19, 5, 59, 999_999_999)
    ///         .in_tz("America/New_York")?,
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: error on overflow
    ///
    /// ```
    /// use jiff::{civil::date, ToSpan};
    ///
    /// let zdt = date(2024, 3, 31).at(13, 13, 13, 13).in_tz("America/New_York")?;
    /// assert!(zdt.checked_add(9000.years()).is_err());
    /// assert!(zdt.checked_add(-19000.years()).is_err());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: adding absolute durations
    ///
    /// This shows how to add signed and unsigned absolute durations to a
    /// `Zoned`.
    ///
    /// ```
    /// use std::time::Duration;
    ///
    /// use jiff::{civil::date, SignedDuration};
    ///
    /// let zdt = date(2024, 2, 29).at(0, 0, 0, 0).in_tz("US/Eastern")?;
    ///
    /// let dur = SignedDuration::from_hours(25);
    /// assert_eq!(
    ///     zdt.checked_add(dur)?,
    ///     date(2024, 3, 1).at(1, 0, 0, 0).in_tz("US/Eastern")?,
    /// );
    /// assert_eq!(
    ///     zdt.checked_add(-dur)?,
    ///     date(2024, 2, 27).at(23, 0, 0, 0).in_tz("US/Eastern")?,
    /// );
    ///
    /// let dur = Duration::from_secs(25 * 60 * 60);
    /// assert_eq!(
    ///     zdt.checked_add(dur)?,
    ///     date(2024, 3, 1).at(1, 0, 0, 0).in_tz("US/Eastern")?,
    /// );
    /// // One cannot negate an unsigned duration,
    /// // but you can subtract it!
    /// assert_eq!(
    ///     zdt.checked_sub(dur)?,
    ///     date(2024, 2, 27).at(23, 0, 0, 0).in_tz("US/Eastern")?,
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn checked_add<A: Into<ZonedArithmetic>>(
        &self,
        duration: A,
    ) -> Result<Zoned, Error> {
        let duration: ZonedArithmetic = duration.into();
        duration.checked_add(self)
    }

    #[inline]
    fn checked_add_span(&self, span: Span) -> Result<Zoned, Error> {
        let span_calendar = span.only_calendar();
        // If our duration only consists of "time" (hours, minutes, etc), then
        // we can short-circuit and do timestamp math. This also avoids dealing
        // with ambiguity and time zone bullshit.
        if span_calendar.is_zero() {
            return self
                .timestamp()
                .checked_add(span)
                .map(|ts| ts.to_zoned(self.time_zone().clone()))
                .with_context(|| {
                    err!(
                        "failed to add span {span} to timestamp {timestamp} \
                         from zoned datetime {zoned}",
                        timestamp = self.timestamp(),
                        zoned = self,
                    )
                });
        }
        let span_time = span.only_time();
        let dt =
            self.datetime().checked_add(span_calendar).with_context(|| {
                err!(
                    "failed to add span {span_calendar} to datetime {dt} \
                     from zoned datetime {zoned}",
                    dt = self.datetime(),
                    zoned = self,
                )
            })?;

        let tz = self.time_zone();
        let mut ts =
            tz.to_ambiguous_timestamp(dt).compatible().with_context(|| {
                err!(
                    "failed to convert civil datetime {dt} to timestamp \
                     with time zone {tz}",
                    tz = self.time_zone().diagnostic_name(),
                )
            })?;
        ts = ts.checked_add(span_time).with_context(|| {
            err!(
                "failed to add span {span_time} to timestamp {ts} \
                 (which was created from {dt})"
            )
        })?;
        Ok(ts.to_zoned(tz.clone()))
    }

    #[inline]
    fn checked_add_duration(
        &self,
        duration: SignedDuration,
    ) -> Result<Zoned, Error> {
        self.timestamp()
            .checked_add(duration)
            .map(|ts| ts.to_zoned(self.time_zone().clone()))
    }

    /// This routine is identical to [`Zoned::checked_add`] with the
    /// duration negated.
    ///
    /// # Errors
    ///
    /// This has the same error conditions as [`Zoned::checked_add`].
    ///
    /// # Example
    ///
    /// This routine can be used via the `-` operator. Note though that if it
    /// fails, it will result in a panic. Note that we use `&zdt - ...` instead
    /// of `zdt - ...` since `Sub` is implemented for `&Zoned` and not `Zoned`.
    /// This is because `Zoned` is not `Copy`.
    ///
    /// ```
    /// use std::time::Duration;
    ///
    /// use jiff::{civil::date, SignedDuration, ToSpan};
    ///
    /// let zdt = date(1995, 12, 7)
    ///     .at(3, 24, 30, 3_500)
    ///     .in_tz("America/New_York")?;
    /// let got = &zdt - 20.years().months(4).nanoseconds(500);
    /// assert_eq!(
    ///     got,
    ///     date(1975, 8, 7).at(3, 24, 30, 3_000).in_tz("America/New_York")?,
    /// );
    ///
    /// let dur = SignedDuration::new(24 * 60 * 60, 500);
    /// assert_eq!(
    ///     &zdt - dur,
    ///     date(1995, 12, 6).at(3, 24, 30, 3_000).in_tz("America/New_York")?,
    /// );
    ///
    /// let dur = Duration::new(24 * 60 * 60, 500);
    /// assert_eq!(
    ///     &zdt - dur,
    ///     date(1995, 12, 6).at(3, 24, 30, 3_000).in_tz("America/New_York")?,
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn checked_sub<A: Into<ZonedArithmetic>>(
        &self,
        duration: A,
    ) -> Result<Zoned, Error> {
        let duration: ZonedArithmetic = duration.into();
        duration.checked_neg().and_then(|za| za.checked_add(self))
    }

    /// This routine is identical to [`Zoned::checked_add`], except the
    /// result saturates on overflow. That is, instead of overflow, either
    /// [`Timestamp::MIN`] or [`Timestamp::MAX`] (in this `Zoned` value's time
    /// zone) is returned.
    ///
    /// # Properties
    ///
    /// The properties of this routine are identical to [`Zoned::checked_add`],
    /// except that if saturation occurs, then the result is not reversible.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::{civil::date, SignedDuration, Timestamp, ToSpan};
    ///
    /// let zdt = date(2024, 3, 31).at(13, 13, 13, 13).in_tz("America/New_York")?;
    /// assert_eq!(Timestamp::MAX, zdt.saturating_add(9000.years()).timestamp());
    /// assert_eq!(Timestamp::MIN, zdt.saturating_add(-19000.years()).timestamp());
    /// assert_eq!(Timestamp::MAX, zdt.saturating_add(SignedDuration::MAX).timestamp());
    /// assert_eq!(Timestamp::MIN, zdt.saturating_add(SignedDuration::MIN).timestamp());
    /// assert_eq!(Timestamp::MAX, zdt.saturating_add(std::time::Duration::MAX).timestamp());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn saturating_add<A: Into<ZonedArithmetic>>(
        &self,
        duration: A,
    ) -> Zoned {
        let duration: ZonedArithmetic = duration.into();
        self.checked_add(duration).unwrap_or_else(|_| {
            let ts = if duration.is_negative() {
                Timestamp::MIN
            } else {
                Timestamp::MAX
            };
            ts.to_zoned(self.time_zone().clone())
        })
    }

    /// This routine is identical to [`Zoned::saturating_add`] with the span
    /// parameter negated.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::{civil::date, SignedDuration, Timestamp, ToSpan};
    ///
    /// let zdt = date(2024, 3, 31).at(13, 13, 13, 13).in_tz("America/New_York")?;
    /// assert_eq!(Timestamp::MIN, zdt.saturating_sub(19000.years()).timestamp());
    /// assert_eq!(Timestamp::MAX, zdt.saturating_sub(-9000.years()).timestamp());
    /// assert_eq!(Timestamp::MIN, zdt.saturating_sub(SignedDuration::MAX).timestamp());
    /// assert_eq!(Timestamp::MAX, zdt.saturating_sub(SignedDuration::MIN).timestamp());
    /// assert_eq!(Timestamp::MIN, zdt.saturating_sub(std::time::Duration::MAX).timestamp());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn saturating_sub<A: Into<ZonedArithmetic>>(
        &self,
        duration: A,
    ) -> Zoned {
        let duration: ZonedArithmetic = duration.into();
        let Ok(duration) = duration.checked_neg() else {
            return Timestamp::MIN.to_zoned(self.time_zone().clone());
        };
        self.saturating_add(duration)
    }

    /// Returns a span representing the elapsed time from this zoned datetime
    /// until the given `other` zoned datetime.
    ///
    /// When `other` occurs before this datetime, then the span returned will
    /// be negative.
    ///
    /// Depending on the input provided, the span returned is rounded. It may
    /// also be balanced up to bigger units than the default. By default, the
    /// span returned is balanced such that the biggest possible unit is hours.
    /// This default is an API guarantee. Users can rely on the default not
    /// returning any calendar units in the default configuration.
    ///
    /// This operation is configured by providing a [`ZonedDifference`]
    /// value. Since this routine accepts anything that implements
    /// `Into<ZonedDifference>`, once can pass a `&Zoned` directly.
    /// One can also pass a `(Unit, &Zoned)`, where `Unit` is treated as
    /// [`ZonedDifference::largest`].
    ///
    /// # Properties
    ///
    /// It is guaranteed that if the returned span is subtracted from `other`,
    /// and if no rounding is requested, and if the largest unit requested
    /// is at most `Unit::Hour`, then the original zoned datetime will be
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
    /// time between the minimum and maximum zoned datetime supported by Jiff.
    /// Therefore, if one requests a span with its largest unit set to
    /// [`Unit::Nanosecond`], then it's possible for this routine to fail.
    ///
    /// An error can also occur if `ZonedDifference` is misconfigured. For
    /// example, if the smallest unit provided is bigger than the largest unit.
    ///
    /// An error can also occur if units greater than `Unit::Hour` are
    /// requested _and_ if the time zones in the provided zoned datetimes
    /// are distinct. (See [`TimeZone`]'s section on equality for details on
    /// how equality is determined.) This error occurs because the length of
    /// a day may vary depending on the time zone. To work around this
    /// restriction, convert one or both of the zoned datetimes into the same
    /// time zone.
    ///
    /// It is guaranteed that if one provides a datetime with the default
    /// [`ZonedDifference`] configuration, then this routine will never
    /// fail.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::{civil::date, ToSpan};
    ///
    /// let earlier = date(2006, 8, 24).at(22, 30, 0, 0).in_tz("America/New_York")?;
    /// let later = date(2019, 1, 31).at(21, 0, 0, 0).in_tz("America/New_York")?;
    /// assert_eq!(
    ///     earlier.until(&later)?,
    ///     109_031.hours().minutes(30).fieldwise(),
    /// );
    ///
    /// // Flipping the dates is fine, but you'll get a negative span.
    /// assert_eq!(
    ///     later.until(&earlier)?,
    ///     -109_031.hours().minutes(30).fieldwise(),
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: using bigger units
    ///
    /// This example shows how to expand the span returned to bigger units.
    /// This makes use of a `From<(Unit, &Zoned)> for ZonedDifference`
    /// trait implementation.
    ///
    /// ```
    /// use jiff::{civil::date, Unit, ToSpan};
    ///
    /// let zdt1 = date(1995, 12, 07).at(3, 24, 30, 3500).in_tz("America/New_York")?;
    /// let zdt2 = date(2019, 01, 31).at(15, 30, 0, 0).in_tz("America/New_York")?;
    ///
    /// // The default limits durations to using "hours" as the biggest unit.
    /// let span = zdt1.until(&zdt2)?;
    /// assert_eq!(span.to_string(), "PT202956H5M29.9999965S");
    ///
    /// // But we can ask for units all the way up to years.
    /// let span = zdt1.until((Unit::Year, &zdt2))?;
    /// assert_eq!(format!("{span:#}"), "23y 1mo 24d 12h 5m 29s 999ms 996µs 500ns");
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: rounding the result
    ///
    /// This shows how one might find the difference between two zoned
    /// datetimes and have the result rounded such that sub-seconds are
    /// removed.
    ///
    /// In this case, we need to hand-construct a [`ZonedDifference`]
    /// in order to gain full configurability.
    ///
    /// ```
    /// use jiff::{civil::date, Unit, ToSpan, ZonedDifference};
    ///
    /// let zdt1 = date(1995, 12, 07).at(3, 24, 30, 3500).in_tz("America/New_York")?;
    /// let zdt2 = date(2019, 01, 31).at(15, 30, 0, 0).in_tz("America/New_York")?;
    ///
    /// let span = zdt1.until(
    ///     ZonedDifference::from(&zdt2).smallest(Unit::Second),
    /// )?;
    /// assert_eq!(format!("{span:#}"), "202956h 5m 29s");
    ///
    /// // We can combine smallest and largest units too!
    /// let span = zdt1.until(
    ///     ZonedDifference::from(&zdt2)
    ///         .smallest(Unit::Second)
    ///         .largest(Unit::Year),
    /// )?;
    /// assert_eq!(span.to_string(), "P23Y1M24DT12H5M29S");
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: units biggers than days inhibit reversibility
    ///
    /// If you ask for units bigger than hours, then adding the span returned
    /// to the `other` zoned datetime is not guaranteed to result in the
    /// original zoned datetime. For example:
    ///
    /// ```
    /// use jiff::{civil::date, Unit, ToSpan};
    ///
    /// let zdt1 = date(2024, 3, 2).at(0, 0, 0, 0).in_tz("America/New_York")?;
    /// let zdt2 = date(2024, 5, 1).at(0, 0, 0, 0).in_tz("America/New_York")?;
    ///
    /// let span = zdt1.until((Unit::Month, &zdt2))?;
    /// assert_eq!(span, 1.month().days(29).fieldwise());
    /// let maybe_original = zdt2.checked_sub(span)?;
    /// // Not the same as the original datetime!
    /// assert_eq!(
    ///     maybe_original,
    ///     date(2024, 3, 3).at(0, 0, 0, 0).in_tz("America/New_York")?,
    /// );
    ///
    /// // But in the default configuration, hours are always the biggest unit
    /// // and reversibility is guaranteed.
    /// let span = zdt1.until(&zdt2)?;
    /// assert_eq!(span.to_string(), "PT1439H");
    /// let is_original = zdt2.checked_sub(span)?;
    /// assert_eq!(is_original, zdt1);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// This occurs because spans are added as if by adding the biggest units
    /// first, and then the smaller units. Because months vary in length,
    /// their meaning can change depending on how the span is added. In this
    /// case, adding one month to `2024-03-02` corresponds to 31 days, but
    /// subtracting one month from `2024-05-01` corresponds to 30 days.
    #[inline]
    pub fn until<'a, A: Into<ZonedDifference<'a>>>(
        &self,
        other: A,
    ) -> Result<Span, Error> {
        let args: ZonedDifference = other.into();
        let span = args.until_with_largest_unit(self)?;
        if args.rounding_may_change_span() {
            span.round(args.round.relative(self))
        } else {
            Ok(span)
        }
    }

    /// This routine is identical to [`Zoned::until`], but the order of the
    /// parameters is flipped.
    ///
    /// # Errors
    ///
    /// This has the same error conditions as [`Zoned::until`].
    ///
    /// # Example
    ///
    /// This routine can be used via the `-` operator. Since the default
    /// configuration is used and because a `Span` can represent the difference
    /// between any two possible zoned datetimes, it will never panic. Note
    /// that we use `&zdt1 - &zdt2` instead of `zdt1 - zdt2` since `Sub` is
    /// implemented for `&Zoned` and not `Zoned`. This is because `Zoned` is
    /// not `Copy`.
    ///
    /// ```
    /// use jiff::{civil::date, ToSpan};
    ///
    /// let earlier = date(2006, 8, 24).at(22, 30, 0, 0).in_tz("America/New_York")?;
    /// let later = date(2019, 1, 31).at(21, 0, 0, 0).in_tz("America/New_York")?;
    /// assert_eq!(&later - &earlier, 109_031.hours().minutes(30).fieldwise());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn since<'a, A: Into<ZonedDifference<'a>>>(
        &self,
        other: A,
    ) -> Result<Span, Error> {
        let args: ZonedDifference = other.into();
        let span = -args.until_with_largest_unit(self)?;
        if args.rounding_may_change_span() {
            span.round(args.round.relative(self))
        } else {
            Ok(span)
        }
    }

    /// Returns an absolute duration representing the elapsed time from this
    /// zoned datetime until the given `other` zoned datetime.
    ///
    /// When `other` occurs before this zoned datetime, then the duration
    /// returned will be negative.
    ///
    /// Unlike [`Zoned::until`], this always returns a duration
    /// corresponding to a 96-bit integer of nanoseconds between two
    /// zoned datetimes.
    ///
    /// # Fallibility
    ///
    /// This routine never panics or returns an error. Since there are no
    /// configuration options that can be incorrectly provided, no error is
    /// possible when calling this routine. In contrast, [`Zoned::until`]
    /// can return an error in some cases due to misconfiguration. But like
    /// this routine, [`Zoned::until`] never panics or returns an error in
    /// its default configuration.
    ///
    /// # When should I use this versus [`Zoned::until`]?
    ///
    /// See the type documentation for [`SignedDuration`] for the section on
    /// when one should use [`Span`] and when one should use `SignedDuration`.
    /// In short, use `Span` (and therefore `Timestamp::until`) unless you have
    /// a specific reason to do otherwise.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::{civil::date, SignedDuration};
    ///
    /// let earlier = date(2006, 8, 24).at(22, 30, 0, 0).in_tz("US/Eastern")?;
    /// let later = date(2019, 1, 31).at(21, 0, 0, 0).in_tz("US/Eastern")?;
    /// assert_eq!(
    ///     earlier.duration_until(&later),
    ///     SignedDuration::from_hours(109_031) + SignedDuration::from_mins(30),
    /// );
    ///
    /// // Flipping the dates is fine, but you'll get a negative span.
    /// assert_eq!(
    ///     later.duration_until(&earlier),
    ///     -SignedDuration::from_hours(109_031) + -SignedDuration::from_mins(30),
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: difference with [`Zoned::until`]
    ///
    /// The main difference between this routine and `Zoned::until` is that
    /// the latter can return units other than a 96-bit integer of nanoseconds.
    /// While a 96-bit integer of nanoseconds can be converted into other units
    /// like hours, this can only be done for uniform units. (Uniform units are
    /// units for which each individual unit always corresponds to the same
    /// elapsed time regardless of the datetime it is relative to.) This can't
    /// be done for units like years, months or days.
    ///
    /// ```
    /// use jiff::{civil::date, SignedDuration, Span, SpanRound, ToSpan, Unit};
    ///
    /// let zdt1 = date(2024, 3, 10).at(0, 0, 0, 0).in_tz("US/Eastern")?;
    /// let zdt2 = date(2024, 3, 11).at(0, 0, 0, 0).in_tz("US/Eastern")?;
    ///
    /// let span = zdt1.until((Unit::Day, &zdt2))?;
    /// assert_eq!(format!("{span:#}"), "1d");
    ///
    /// let duration = zdt1.duration_until(&zdt2);
    /// // This day was only 23 hours long!
    /// assert_eq!(duration, SignedDuration::from_hours(23));
    /// // There's no way to extract years, months or days from the signed
    /// // duration like one might extract hours (because every hour
    /// // is the same length). Instead, you actually have to convert
    /// // it to a span and then balance it by providing a relative date!
    /// let options = SpanRound::new().largest(Unit::Day).relative(&zdt1);
    /// let span = Span::try_from(duration)?.round(options)?;
    /// assert_eq!(format!("{span:#}"), "1d");
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: getting an unsigned duration
    ///
    /// If you're looking to find the duration between two zoned datetimes as
    /// a [`std::time::Duration`], you'll need to use this method to get a
    /// [`SignedDuration`] and then convert it to a `std::time::Duration`:
    ///
    /// ```
    /// use std::time::Duration;
    ///
    /// use jiff::civil::date;
    ///
    /// let zdt1 = date(2024, 7, 1).at(0, 0, 0, 0).in_tz("US/Eastern")?;
    /// let zdt2 = date(2024, 8, 1).at(0, 0, 0, 0).in_tz("US/Eastern")?;
    /// let duration = Duration::try_from(zdt1.duration_until(&zdt2))?;
    /// assert_eq!(duration, Duration::from_secs(31 * 24 * 60 * 60));
    ///
    /// // Note that unsigned durations cannot represent all
    /// // possible differences! If the duration would be negative,
    /// // then the conversion fails:
    /// assert!(Duration::try_from(zdt2.duration_until(&zdt1)).is_err());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn duration_until(&self, other: &Zoned) -> SignedDuration {
        SignedDuration::zoned_until(self, other)
    }

    /// This routine is identical to [`Zoned::duration_until`], but the
    /// order of the parameters is flipped.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::{civil::date, SignedDuration};
    ///
    /// let earlier = date(2006, 8, 24).at(22, 30, 0, 0).in_tz("US/Eastern")?;
    /// let later = date(2019, 1, 31).at(21, 0, 0, 0).in_tz("US/Eastern")?;
    /// assert_eq!(
    ///     later.duration_since(&earlier),
    ///     SignedDuration::from_hours(109_031) + SignedDuration::from_mins(30),
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn duration_since(&self, other: &Zoned) -> SignedDuration {
        SignedDuration::zoned_until(other, self)
    }

    /// Rounds this zoned datetime according to the [`ZonedRound`]
    /// configuration given.
    ///
    /// The principal option is [`ZonedRound::smallest`], which allows one to
    /// configure the smallest units in the returned zoned datetime. Rounding
    /// is what determines whether that unit should keep its current value
    /// or whether it should be incremented. Moreover, the amount it should
    /// be incremented can be configured via [`ZonedRound::increment`].
    /// Finally, the rounding strategy itself can be configured via
    /// [`ZonedRound::mode`].
    ///
    /// Note that this routine is generic and accepts anything that
    /// implements `Into<ZonedRound>`. Some notable implementations are:
    ///
    /// * `From<Unit> for ZonedRound`, which will automatically create a
    /// `ZonedRound::new().smallest(unit)` from the unit provided.
    /// * `From<(Unit, i64)> for ZonedRound`, which will automatically
    /// create a `ZonedRound::new().smallest(unit).increment(number)` from
    /// the unit and increment provided.
    ///
    /// # Errors
    ///
    /// This returns an error if the smallest unit configured on the given
    /// [`ZonedRound`] is bigger than days. An error is also returned if
    /// the rounding increment is greater than 1 when the units are days.
    /// (Currently, rounding to the nearest week, month or year is not
    /// supported.)
    ///
    /// When the smallest unit is less than days, the rounding increment must
    /// divide evenly into the next highest unit after the smallest unit
    /// configured (and must not be equivalent to it). For example, if the
    /// smallest unit is [`Unit::Nanosecond`], then *some* of the valid values
    /// for the rounding increment are `1`, `2`, `4`, `5`, `100` and `500`.
    /// Namely, any integer that divides evenly into `1,000` nanoseconds since
    /// there are `1,000` nanoseconds in the next highest unit (microseconds).
    ///
    /// This can also return an error in some cases where rounding would
    /// require arithmetic that exceeds the maximum zoned datetime value.
    ///
    /// # Example
    ///
    /// This is a basic example that demonstrates rounding a zoned datetime
    /// to the nearest day. This also demonstrates calling this method with
    /// the smallest unit directly, instead of constructing a `ZonedRound`
    /// manually.
    ///
    /// ```
    /// use jiff::{civil::date, Unit};
    ///
    /// // rounds up
    /// let zdt = date(2024, 6, 19).at(15, 0, 0, 0).in_tz("America/New_York")?;
    /// assert_eq!(
    ///     zdt.round(Unit::Day)?,
    ///     date(2024, 6, 20).at(0, 0, 0, 0).in_tz("America/New_York")?,
    /// );
    ///
    /// // rounds down
    /// let zdt = date(2024, 6, 19).at(10, 0, 0, 0).in_tz("America/New_York")?;
    /// assert_eq!(
    ///     zdt.round(Unit::Day)?,
    ///     date(2024, 6, 19).at(0, 0, 0, 0).in_tz("America/New_York")?,
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
    /// use jiff::{civil::date, RoundMode, Unit, Zoned, ZonedRound};
    ///
    /// let zdt = date(2024, 6, 19).at(15, 0, 0, 0).in_tz("America/New_York")?;
    /// assert_eq!(
    ///     zdt.round(Unit::Day)?,
    ///     date(2024, 6, 20).at(0, 0, 0, 0).in_tz("America/New_York")?,
    /// );
    /// // The default will round up to the next day for any time past noon (as
    /// // shown above), but using truncation rounding will always round down.
    /// assert_eq!(
    ///     zdt.round(
    ///         ZonedRound::new().smallest(Unit::Day).mode(RoundMode::Trunc),
    ///     )?,
    ///     date(2024, 6, 19).at(0, 0, 0, 0).in_tz("America/New_York")?,
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: rounding to the nearest 5 minute increment
    ///
    /// ```
    /// use jiff::{civil::date, Unit};
    ///
    /// // rounds down
    /// let zdt = date(2024, 6, 19)
    ///     .at(15, 27, 29, 999_999_999)
    ///     .in_tz("America/New_York")?;
    /// assert_eq!(
    ///     zdt.round((Unit::Minute, 5))?,
    ///     date(2024, 6, 19).at(15, 25, 0, 0).in_tz("America/New_York")?,
    /// );
    /// // rounds up
    /// let zdt = date(2024, 6, 19)
    ///     .at(15, 27, 30, 0)
    ///     .in_tz("America/New_York")?;
    /// assert_eq!(
    ///     zdt.round((Unit::Minute, 5))?,
    ///     date(2024, 6, 19).at(15, 30, 0, 0).in_tz("America/New_York")?,
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: behavior near time zone transitions
    ///
    /// When rounding this zoned datetime near time zone transitions (such as
    /// DST), the "sensible" thing is done by default. Namely, rounding will
    /// jump to the closest instant, even if the change in civil clock time is
    /// large. For example, when rounding up into a gap, the civil clock time
    /// will jump over the gap, but the corresponding change in the instant is
    /// as one might expect:
    ///
    /// ```
    /// use jiff::{Unit, Zoned};
    ///
    /// let zdt1: Zoned = "2024-03-10T01:59:00-05[America/New_York]".parse()?;
    /// let zdt2 = zdt1.round(Unit::Hour)?;
    /// assert_eq!(
    ///     zdt2.to_string(),
    ///     "2024-03-10T03:00:00-04:00[America/New_York]",
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// Similarly, when rounding inside a fold, rounding will respect whether
    /// it's the first or second time the clock has repeated the hour. For the
    /// DST transition in New York on `2024-11-03` from offset `-04` to `-05`,
    /// here is an example that rounds the first 1 o'clock hour:
    ///
    /// ```
    /// use jiff::{Unit, Zoned};
    ///
    /// let zdt1: Zoned = "2024-11-03T01:59:01-04[America/New_York]".parse()?;
    /// let zdt2 = zdt1.round(Unit::Minute)?;
    /// assert_eq!(
    ///     zdt2.to_string(),
    ///     "2024-11-03T01:59:00-04:00[America/New_York]",
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// And now the second 1 o'clock hour. Notice how the rounded result stays
    /// in the second 1 o'clock hour.
    ///
    /// ```
    /// use jiff::{Unit, Zoned};
    ///
    /// let zdt1: Zoned = "2024-11-03T01:59:01-05[America/New_York]".parse()?;
    /// let zdt2 = zdt1.round(Unit::Minute)?;
    /// assert_eq!(
    ///     zdt2.to_string(),
    ///     "2024-11-03T01:59:00-05:00[America/New_York]",
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: rounding to nearest day takes length of day into account
    ///
    /// Some days are shorter than 24 hours, and so rounding down will occur
    /// even when the time is past noon:
    ///
    /// ```
    /// use jiff::{Unit, Zoned};
    ///
    /// let zdt1: Zoned = "2025-03-09T12:15-04[America/New_York]".parse()?;
    /// let zdt2 = zdt1.round(Unit::Day)?;
    /// assert_eq!(
    ///     zdt2.to_string(),
    ///     "2025-03-09T00:00:00-05:00[America/New_York]",
    /// );
    ///
    /// // For 23 hour days, 12:30 is the tipping point to round up in the
    /// // default rounding configuration:
    /// let zdt1: Zoned = "2025-03-09T12:30-04[America/New_York]".parse()?;
    /// let zdt2 = zdt1.round(Unit::Day)?;
    /// assert_eq!(
    ///     zdt2.to_string(),
    ///     "2025-03-10T00:00:00-04:00[America/New_York]",
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// And some days are longer than 24 hours, and so rounding _up_ will occur
    /// even when the time is before noon:
    ///
    /// ```
    /// use jiff::{Unit, Zoned};
    ///
    /// let zdt1: Zoned = "2025-11-02T11:45-05[America/New_York]".parse()?;
    /// let zdt2 = zdt1.round(Unit::Day)?;
    /// assert_eq!(
    ///     zdt2.to_string(),
    ///     "2025-11-03T00:00:00-05:00[America/New_York]",
    /// );
    ///
    /// // For 25 hour days, 11:30 is the tipping point to round up in the
    /// // default rounding configuration. So 11:29 will round down:
    /// let zdt1: Zoned = "2025-11-02T11:29-05[America/New_York]".parse()?;
    /// let zdt2 = zdt1.round(Unit::Day)?;
    /// assert_eq!(
    ///     zdt2.to_string(),
    ///     "2025-11-02T00:00:00-04:00[America/New_York]",
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: overflow error
    ///
    /// This example demonstrates that it's possible for this operation to
    /// result in an error from zoned datetime arithmetic overflow.
    ///
    /// ```
    /// use jiff::{Timestamp, Unit};
    ///
    /// let zdt = Timestamp::MAX.in_tz("America/New_York")?;
    /// assert!(zdt.round(Unit::Day).is_err());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// This occurs because rounding to the nearest day for the maximum
    /// timestamp would result in rounding up to the next day. But the next day
    /// is greater than the maximum, and so this returns an error.
    #[inline]
    pub fn round<R: Into<ZonedRound>>(
        &self,
        options: R,
    ) -> Result<Zoned, Error> {
        let options: ZonedRound = options.into();
        options.round(self)
    }

    /*
    /// Return an iterator of periodic zoned datetimes determined by the given
    /// span.
    ///
    /// The given span may be negative, in which case, the iterator will move
    /// backwards through time. The iterator won't stop until either the span
    /// itself overflows, or it would otherwise exceed the minimum or maximum
    /// `Zoned` value.
    ///
    /// # Example: when to check a glucose monitor
    ///
    /// When my cat had diabetes, my veterinarian installed a glucose monitor
    /// and instructed me to scan it about every 5 hours. This example lists
    /// all of the times I need to scan it for the 2 days following its
    /// installation:
    ///
    /// ```
    /// use jiff::{civil::datetime, ToSpan};
    ///
    /// let start = datetime(2023, 7, 15, 16, 30, 0, 0).in_tz("America/New_York")?;
    /// let end = start.checked_add(2.days())?;
    /// let mut scan_times = vec![];
    /// for zdt in start.series(5.hours()).take_while(|zdt| zdt <= end) {
    ///     scan_times.push(zdt.datetime());
    /// }
    /// assert_eq!(scan_times, vec![
    ///     datetime(2023, 7, 15, 16, 30, 0, 0),
    ///     datetime(2023, 7, 15, 21, 30, 0, 0),
    ///     datetime(2023, 7, 16, 2, 30, 0, 0),
    ///     datetime(2023, 7, 16, 7, 30, 0, 0),
    ///     datetime(2023, 7, 16, 12, 30, 0, 0),
    ///     datetime(2023, 7, 16, 17, 30, 0, 0),
    ///     datetime(2023, 7, 16, 22, 30, 0, 0),
    ///     datetime(2023, 7, 17, 3, 30, 0, 0),
    ///     datetime(2023, 7, 17, 8, 30, 0, 0),
    ///     datetime(2023, 7, 17, 13, 30, 0, 0),
    /// ]);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example
    ///
    /// BREADCRUMBS: Maybe just remove ZonedSeries for now..?
    ///
    /// ```
    /// use jiff::{civil::date, ToSpan};
    ///
    /// let zdt = date(2011, 12, 28).in_tz("Pacific/Apia")?;
    /// let mut it = zdt.series(1.day());
    /// assert_eq!(it.next(), Some(date(2011, 12, 28).in_tz("Pacific/Apia")?));
    /// assert_eq!(it.next(), Some(date(2011, 12, 29).in_tz("Pacific/Apia")?));
    /// assert_eq!(it.next(), Some(date(2011, 12, 30).in_tz("Pacific/Apia")?));
    /// assert_eq!(it.next(), Some(date(2011, 12, 31).in_tz("Pacific/Apia")?));
    /// assert_eq!(it.next(), Some(date(2012, 01, 01).in_tz("Pacific/Apia")?));
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn series(self, period: Span) -> ZonedSeries {
        ZonedSeries { start: self, period, step: 0 }
    }
    */

    #[inline]
    fn into_parts(self) -> (Timestamp, DateTime, Offset, TimeZone) {
        let inner = self.inner;
        let ZonedInner { timestamp, datetime, offset, time_zone } = inner;
        (timestamp, datetime, offset, time_zone)
    }
}

/// Parsing and formatting using a "printf"-style API.
impl Zoned {
    /// Parses a zoned datetime in `input` matching the given `format`.
    ///
    /// The format string uses a "printf"-style API where conversion
    /// specifiers can be used as place holders to match components of
    /// a datetime. For details on the specifiers supported, see the
    /// [`fmt::strtime`] module documentation.
    ///
    /// # Warning
    ///
    /// The `strtime` module APIs do not require an IANA time zone identifier
    /// to parse a `Zoned`. If one is not used, then if you format a zoned
    /// datetime in a time zone like `America/New_York` and then parse it back
    /// again, the zoned datetime you get back will be a "fixed offset" zoned
    /// datetime. This in turn means it will not perform daylight saving time
    /// safe arithmetic.
    ///
    /// However, the `%Q` directive may be used to both format and parse an
    /// IANA time zone identifier. It is strongly recommended to use this
    /// directive whenever one is formatting or parsing `Zoned` values.
    ///
    /// # Errors
    ///
    /// This returns an error when parsing failed. This might happen because
    /// the format string itself was invalid, or because the input didn't match
    /// the format string.
    ///
    /// This also returns an error if there wasn't sufficient information to
    /// construct a zoned datetime. For example, if an offset wasn't parsed.
    ///
    /// # Example
    ///
    /// This example shows how to parse a zoned datetime:
    ///
    /// ```
    /// use jiff::Zoned;
    ///
    /// let zdt = Zoned::strptime("%F %H:%M %:Q", "2024-07-14 21:14 US/Eastern")?;
    /// assert_eq!(zdt.to_string(), "2024-07-14T21:14:00-04:00[US/Eastern]");
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn strptime(
        format: impl AsRef<[u8]>,
        input: impl AsRef<[u8]>,
    ) -> Result<Zoned, Error> {
        fmt::strtime::parse(format, input).and_then(|tm| tm.to_zoned())
    }

    /// Formats this zoned datetime according to the given `format`.
    ///
    /// The format string uses a "printf"-style API where conversion
    /// specifiers can be used as place holders to format components of
    /// a datetime. For details on the specifiers supported, see the
    /// [`fmt::strtime`] module documentation.
    ///
    /// # Warning
    ///
    /// The `strtime` module APIs do not support parsing or formatting with
    /// IANA time zone identifiers. This means that if you format a zoned
    /// datetime in a time zone like `America/New_York` and then parse it back
    /// again, the zoned datetime you get back will be a "fixed offset" zoned
    /// datetime. This in turn means it will not perform daylight saving time
    /// safe arithmetic.
    ///
    /// The `strtime` modules APIs are useful for ad hoc formatting and
    /// parsing, but they shouldn't be used as an interchange format. For
    /// an interchange format, the default `std::fmt::Display` and
    /// `std::str::FromStr` trait implementations on `Zoned` are appropriate.
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
    /// While the output of the Unix `date` command is likely locale specific,
    /// this is what it looks like on my system:
    ///
    /// ```
    /// use jiff::civil::date;
    ///
    /// let zdt = date(2024, 7, 15).at(16, 24, 59, 0).in_tz("America/New_York")?;
    /// let string = zdt.strftime("%a %b %e %I:%M:%S %p %Z %Y").to_string();
    /// assert_eq!(string, "Mon Jul 15 04:24:59 PM EDT 2024");
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn strftime<'f, F: 'f + ?Sized + AsRef<[u8]>>(
        &self,
        format: &'f F,
    ) -> fmt::strtime::Display<'f> {
        fmt::strtime::Display { fmt: format.as_ref(), tm: self.into() }
    }
}

impl Default for Zoned {
    #[inline]
    fn default() -> Zoned {
        Zoned::new(Timestamp::default(), TimeZone::UTC)
    }
}

/// Converts a `Zoned` datetime into a human readable datetime string.
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
/// use jiff::civil::date;
///
/// let zdt = date(2024, 6, 15).at(7, 0, 0, 123_000_000).in_tz("US/Eastern")?;
/// assert_eq!(
///     format!("{zdt:.6?}"),
///     "2024-06-15T07:00:00.123000-04:00[US/Eastern]",
/// );
/// // Precision values greater than 9 are clamped to 9.
/// assert_eq!(
///     format!("{zdt:.300?}"),
///     "2024-06-15T07:00:00.123000000-04:00[US/Eastern]",
/// );
/// // A precision of 0 implies the entire fractional
/// // component is always truncated.
/// assert_eq!(
///     format!("{zdt:.0?}"),
///     "2024-06-15T07:00:00-04:00[US/Eastern]",
/// );
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
impl core::fmt::Debug for Zoned {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        core::fmt::Display::fmt(self, f)
    }
}

/// Converts a `Zoned` datetime into a RFC 9557 compliant string.
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
/// use jiff::civil::date;
///
/// // No fractional seconds:
/// let zdt = date(2024, 6, 15).at(7, 0, 0, 0).in_tz("US/Eastern")?;
/// assert_eq!(format!("{zdt}"), "2024-06-15T07:00:00-04:00[US/Eastern]");
///
/// // With fractional seconds:
/// let zdt = date(2024, 6, 15).at(7, 0, 0, 123_000_000).in_tz("US/Eastern")?;
/// assert_eq!(format!("{zdt}"), "2024-06-15T07:00:00.123-04:00[US/Eastern]");
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Example: setting the precision
///
/// ```
/// use jiff::civil::date;
///
/// let zdt = date(2024, 6, 15).at(7, 0, 0, 123_000_000).in_tz("US/Eastern")?;
/// assert_eq!(
///     format!("{zdt:.6}"),
///     "2024-06-15T07:00:00.123000-04:00[US/Eastern]",
/// );
/// // Precision values greater than 9 are clamped to 9.
/// assert_eq!(
///     format!("{zdt:.300}"),
///     "2024-06-15T07:00:00.123000000-04:00[US/Eastern]",
/// );
/// // A precision of 0 implies the entire fractional
/// // component is always truncated.
/// assert_eq!(
///     format!("{zdt:.0}"),
///     "2024-06-15T07:00:00-04:00[US/Eastern]",
/// );
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
impl core::fmt::Display for Zoned {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        use crate::fmt::StdFmtWrite;

        let precision =
            f.precision().map(|p| u8::try_from(p).unwrap_or(u8::MAX));
        temporal::DateTimePrinter::new()
            .precision(precision)
            .print_zoned(self, StdFmtWrite(f))
            .map_err(|_| core::fmt::Error)
    }
}

/// Parses a zoned timestamp from the Temporal datetime format.
///
/// See the [`fmt::temporal`](crate::fmt::temporal) for more information on
/// the precise format.
///
/// Note that this is only enabled when the `std` feature
/// is enabled because it requires access to a global
/// [`TimeZoneDatabase`](crate::tz::TimeZoneDatabase).
impl core::str::FromStr for Zoned {
    type Err = Error;

    fn from_str(string: &str) -> Result<Zoned, Error> {
        DEFAULT_DATETIME_PARSER.parse_zoned(string)
    }
}

impl Eq for Zoned {}

impl PartialEq for Zoned {
    #[inline]
    fn eq(&self, rhs: &Zoned) -> bool {
        self.timestamp().eq(&rhs.timestamp())
    }
}

impl<'a> PartialEq<Zoned> for &'a Zoned {
    #[inline]
    fn eq(&self, rhs: &Zoned) -> bool {
        (**self).eq(rhs)
    }
}

impl Ord for Zoned {
    #[inline]
    fn cmp(&self, rhs: &Zoned) -> core::cmp::Ordering {
        self.timestamp().cmp(&rhs.timestamp())
    }
}

impl PartialOrd for Zoned {
    #[inline]
    fn partial_cmp(&self, rhs: &Zoned) -> Option<core::cmp::Ordering> {
        Some(self.cmp(rhs))
    }
}

impl<'a> PartialOrd<Zoned> for &'a Zoned {
    #[inline]
    fn partial_cmp(&self, rhs: &Zoned) -> Option<core::cmp::Ordering> {
        (**self).partial_cmp(rhs)
    }
}

impl core::hash::Hash for Zoned {
    #[inline]
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.timestamp().hash(state);
    }
}

#[cfg(feature = "std")]
impl TryFrom<std::time::SystemTime> for Zoned {
    type Error = Error;

    #[inline]
    fn try_from(system_time: std::time::SystemTime) -> Result<Zoned, Error> {
        let timestamp = Timestamp::try_from(system_time)?;
        Ok(Zoned::new(timestamp, TimeZone::system()))
    }
}

#[cfg(feature = "std")]
impl From<Zoned> for std::time::SystemTime {
    #[inline]
    fn from(time: Zoned) -> std::time::SystemTime {
        time.timestamp().into()
    }
}

/// Adds a span of time to a zoned datetime.
///
/// This uses checked arithmetic and panics on overflow. To handle overflow
/// without panics, use [`Zoned::checked_add`].
impl<'a> core::ops::Add<Span> for &'a Zoned {
    type Output = Zoned;

    #[inline]
    fn add(self, rhs: Span) -> Zoned {
        self.checked_add(rhs)
            .expect("adding span to zoned datetime overflowed")
    }
}

/// Adds a span of time to a zoned datetime in place.
///
/// This uses checked arithmetic and panics on overflow. To handle overflow
/// without panics, use [`Zoned::checked_add`].
impl core::ops::AddAssign<Span> for Zoned {
    #[inline]
    fn add_assign(&mut self, rhs: Span) {
        *self = &*self + rhs
    }
}

/// Subtracts a span of time from a zoned datetime.
///
/// This uses checked arithmetic and panics on overflow. To handle overflow
/// without panics, use [`Zoned::checked_sub`].
impl<'a> core::ops::Sub<Span> for &'a Zoned {
    type Output = Zoned;

    #[inline]
    fn sub(self, rhs: Span) -> Zoned {
        self.checked_sub(rhs)
            .expect("subtracting span from zoned datetime overflowed")
    }
}

/// Subtracts a span of time from a zoned datetime in place.
///
/// This uses checked arithmetic and panics on overflow. To handle overflow
/// without panics, use [`Zoned::checked_sub`].
impl core::ops::SubAssign<Span> for Zoned {
    #[inline]
    fn sub_assign(&mut self, rhs: Span) {
        *self = &*self - rhs
    }
}

/// Computes the span of time between two zoned datetimes.
///
/// This will return a negative span when the zoned datetime being subtracted
/// is greater.
///
/// Since this uses the default configuration for calculating a span between
/// two zoned datetimes (no rounding and largest units is hours), this will
/// never panic or fail in any way. It is guaranteed that the largest non-zero
/// unit in the `Span` returned will be hours.
///
/// To configure the largest unit or enable rounding, use [`Zoned::since`].
impl<'a> core::ops::Sub for &'a Zoned {
    type Output = Span;

    #[inline]
    fn sub(self, rhs: &'a Zoned) -> Span {
        self.since(rhs).expect("since never fails when given Zoned")
    }
}

/// Adds a signed duration of time to a zoned datetime.
///
/// This uses checked arithmetic and panics on overflow. To handle overflow
/// without panics, use [`Zoned::checked_add`].
impl<'a> core::ops::Add<SignedDuration> for &'a Zoned {
    type Output = Zoned;

    #[inline]
    fn add(self, rhs: SignedDuration) -> Zoned {
        self.checked_add(rhs)
            .expect("adding signed duration to zoned datetime overflowed")
    }
}

/// Adds a signed duration of time to a zoned datetime in place.
///
/// This uses checked arithmetic and panics on overflow. To handle overflow
/// without panics, use [`Zoned::checked_add`].
impl core::ops::AddAssign<SignedDuration> for Zoned {
    #[inline]
    fn add_assign(&mut self, rhs: SignedDuration) {
        *self = &*self + rhs
    }
}

/// Subtracts a signed duration of time from a zoned datetime.
///
/// This uses checked arithmetic and panics on overflow. To handle overflow
/// without panics, use [`Zoned::checked_sub`].
impl<'a> core::ops::Sub<SignedDuration> for &'a Zoned {
    type Output = Zoned;

    #[inline]
    fn sub(self, rhs: SignedDuration) -> Zoned {
        self.checked_sub(rhs).expect(
            "subtracting signed duration from zoned datetime overflowed",
        )
    }
}

/// Subtracts a signed duration of time from a zoned datetime in place.
///
/// This uses checked arithmetic and panics on overflow. To handle overflow
/// without panics, use [`Zoned::checked_sub`].
impl core::ops::SubAssign<SignedDuration> for Zoned {
    #[inline]
    fn sub_assign(&mut self, rhs: SignedDuration) {
        *self = &*self - rhs
    }
}

/// Adds an unsigned duration of time to a zoned datetime.
///
/// This uses checked arithmetic and panics on overflow. To handle overflow
/// without panics, use [`Zoned::checked_add`].
impl<'a> core::ops::Add<UnsignedDuration> for &'a Zoned {
    type Output = Zoned;

    #[inline]
    fn add(self, rhs: UnsignedDuration) -> Zoned {
        self.checked_add(rhs)
            .expect("adding unsigned duration to zoned datetime overflowed")
    }
}

/// Adds an unsigned duration of time to a zoned datetime in place.
///
/// This uses checked arithmetic and panics on overflow. To handle overflow
/// without panics, use [`Zoned::checked_add`].
impl core::ops::AddAssign<UnsignedDuration> for Zoned {
    #[inline]
    fn add_assign(&mut self, rhs: UnsignedDuration) {
        *self = &*self + rhs
    }
}

/// Subtracts an unsigned duration of time from a zoned datetime.
///
/// This uses checked arithmetic and panics on overflow. To handle overflow
/// without panics, use [`Zoned::checked_sub`].
impl<'a> core::ops::Sub<UnsignedDuration> for &'a Zoned {
    type Output = Zoned;

    #[inline]
    fn sub(self, rhs: UnsignedDuration) -> Zoned {
        self.checked_sub(rhs).expect(
            "subtracting unsigned duration from zoned datetime overflowed",
        )
    }
}

/// Subtracts an unsigned duration of time from a zoned datetime in place.
///
/// This uses checked arithmetic and panics on overflow. To handle overflow
/// without panics, use [`Zoned::checked_sub`].
impl core::ops::SubAssign<UnsignedDuration> for Zoned {
    #[inline]
    fn sub_assign(&mut self, rhs: UnsignedDuration) {
        *self = &*self - rhs
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for Zoned {
    #[inline]
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for Zoned {
    #[inline]
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Zoned, D::Error> {
        use serde::de;

        struct ZonedVisitor;

        impl<'de> de::Visitor<'de> for ZonedVisitor {
            type Value = Zoned;

            fn expecting(
                &self,
                f: &mut core::fmt::Formatter,
            ) -> core::fmt::Result {
                f.write_str("a zoned datetime string")
            }

            #[inline]
            fn visit_bytes<E: de::Error>(
                self,
                value: &[u8],
            ) -> Result<Zoned, E> {
                DEFAULT_DATETIME_PARSER
                    .parse_zoned(value)
                    .map_err(de::Error::custom)
            }

            #[inline]
            fn visit_str<E: de::Error>(self, value: &str) -> Result<Zoned, E> {
                self.visit_bytes(value.as_bytes())
            }
        }

        deserializer.deserialize_str(ZonedVisitor)
    }
}

#[cfg(test)]
impl quickcheck::Arbitrary for Zoned {
    fn arbitrary(g: &mut quickcheck::Gen) -> Zoned {
        let timestamp = Timestamp::arbitrary(g);
        let tz = TimeZone::UTC; // TODO: do something better here?
        Zoned::new(timestamp, tz)
    }

    fn shrink(&self) -> alloc::boxed::Box<dyn Iterator<Item = Self>> {
        let timestamp = self.timestamp();
        alloc::boxed::Box::new(
            timestamp
                .shrink()
                .map(|timestamp| Zoned::new(timestamp, TimeZone::UTC)),
        )
    }
}

/*
/// An iterator over periodic zoned datetimes, created by [`Zoned::series`].
///
/// It is exhausted when the next value would exceed a [`Span`] or [`Zoned`]
/// value.
#[derive(Clone, Debug)]
pub struct ZonedSeries {
    start: Zoned,
    period: Span,
    step: i64,
}

impl Iterator for ZonedSeries {
    type Item = Zoned;

    #[inline]
    fn next(&mut self) -> Option<Zoned> {
        // let this = self.start.clone();
        // self.start = self.start.checked_add(self.period).ok()?;
        // Some(this)
        // This is how civil::DateTime series works. But this has a problem
        // for Zoned when there are time zone transitions that skip an entire
        // day. For example, Pacific/Api doesn't have a December 30, 2011.
        // For that case, the code above works better. But if you do it that
        // way, then you get the `jan31 + 1 month = feb28` and
        // `feb28 + 1 month = march28` problem. Where you would instead
        // expect jan31, feb28, mar31... I think.
        //
        // So I'm not quite sure how to resolve this particular conundrum.
        // And this is why ZonedSeries is currently not available.
        let span = self.period.checked_mul(self.step).ok()?;
        self.step = self.step.checked_add(1)?;
        let zdt = self.start.checked_add(span).ok()?;
        Some(zdt)
    }
}
*/

/// Options for [`Timestamp::checked_add`] and [`Timestamp::checked_sub`].
///
/// This type provides a way to ergonomically add one of a few different
/// duration types to a [`Timestamp`].
///
/// The main way to construct values of this type is with its `From` trait
/// implementations:
///
/// * `From<Span> for ZonedArithmetic` adds (or subtracts) the given span
/// to the receiver timestamp.
/// * `From<SignedDuration> for ZonedArithmetic` adds (or subtracts)
/// the given signed duration to the receiver timestamp.
/// * `From<std::time::Duration> for ZonedArithmetic` adds (or subtracts)
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
pub struct ZonedArithmetic {
    duration: Duration,
}

impl ZonedArithmetic {
    #[inline]
    fn checked_add(self, zdt: &Zoned) -> Result<Zoned, Error> {
        match self.duration.to_signed()? {
            SDuration::Span(span) => zdt.checked_add_span(span),
            SDuration::Absolute(sdur) => zdt.checked_add_duration(sdur),
        }
    }

    #[inline]
    fn checked_neg(self) -> Result<ZonedArithmetic, Error> {
        let duration = self.duration.checked_neg()?;
        Ok(ZonedArithmetic { duration })
    }

    #[inline]
    fn is_negative(&self) -> bool {
        self.duration.is_negative()
    }
}

impl From<Span> for ZonedArithmetic {
    fn from(span: Span) -> ZonedArithmetic {
        let duration = Duration::from(span);
        ZonedArithmetic { duration }
    }
}

impl From<SignedDuration> for ZonedArithmetic {
    fn from(sdur: SignedDuration) -> ZonedArithmetic {
        let duration = Duration::from(sdur);
        ZonedArithmetic { duration }
    }
}

impl From<UnsignedDuration> for ZonedArithmetic {
    fn from(udur: UnsignedDuration) -> ZonedArithmetic {
        let duration = Duration::from(udur);
        ZonedArithmetic { duration }
    }
}

impl<'a> From<&'a Span> for ZonedArithmetic {
    fn from(span: &'a Span) -> ZonedArithmetic {
        ZonedArithmetic::from(*span)
    }
}

impl<'a> From<&'a SignedDuration> for ZonedArithmetic {
    fn from(sdur: &'a SignedDuration) -> ZonedArithmetic {
        ZonedArithmetic::from(*sdur)
    }
}

impl<'a> From<&'a UnsignedDuration> for ZonedArithmetic {
    fn from(udur: &'a UnsignedDuration) -> ZonedArithmetic {
        ZonedArithmetic::from(*udur)
    }
}

/// Options for [`Zoned::since`] and [`Zoned::until`].
///
/// This type provides a way to configure the calculation of spans between two
/// [`Zoned`] values. In particular, both `Zoned::since` and `Zoned::until`
/// accept anything that implements `Into<ZonedDifference>`. There are a few
/// key trait implementations that make this convenient:
///
/// * `From<&Zoned> for ZonedDifference` will construct a configuration
/// consisting of just the zoned datetime. So for example, `zdt1.since(zdt2)`
/// returns the span from `zdt2` to `zdt1`.
/// * `From<(Unit, &Zoned)>` is a convenient way to specify the largest units
/// that should be present on the span returned. By default, the largest units
/// are days. Using this trait implementation is equivalent to
/// `ZonedDifference::new(&zdt).largest(unit)`.
///
/// One can also provide a `ZonedDifference` value directly. Doing so
/// is necessary to use the rounding features of calculating a span. For
/// example, setting the smallest unit (defaults to [`Unit::Nanosecond`]), the
/// rounding mode (defaults to [`RoundMode::Trunc`]) and the rounding increment
/// (defaults to `1`). The defaults are selected such that no rounding occurs.
///
/// Rounding a span as part of calculating it is provided as a convenience.
/// Callers may choose to round the span as a distinct step via
/// [`Span::round`], but callers may need to provide a reference date
/// for rounding larger units. By coupling rounding with routines like
/// [`Zoned::since`], the reference date can be set automatically based on
/// the input to `Zoned::since`.
///
/// # Example
///
/// This example shows how to round a span between two zoned datetimes to the
/// nearest half-hour, with ties breaking away from zero.
///
/// ```
/// use jiff::{RoundMode, ToSpan, Unit, Zoned, ZonedDifference};
///
/// let zdt1 = "2024-03-15 08:14:00.123456789[America/New_York]".parse::<Zoned>()?;
/// let zdt2 = "2030-03-22 15:00[America/New_York]".parse::<Zoned>()?;
/// let span = zdt1.until(
///     ZonedDifference::new(&zdt2)
///         .smallest(Unit::Minute)
///         .largest(Unit::Year)
///         .mode(RoundMode::HalfExpand)
///         .increment(30),
/// )?;
/// assert_eq!(span, 6.years().days(7).hours(7).fieldwise());
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Copy, Debug)]
pub struct ZonedDifference<'a> {
    zoned: &'a Zoned,
    round: SpanRound<'static>,
}

impl<'a> ZonedDifference<'a> {
    /// Create a new default configuration for computing the span between the
    /// given zoned datetime and some other zoned datetime (specified as the
    /// receiver in [`Zoned::since`] or [`Zoned::until`]).
    #[inline]
    pub fn new(zoned: &'a Zoned) -> ZonedDifference<'a> {
        // We use truncation rounding by default since it seems that's
        // what is generally expected when computing the difference between
        // datetimes.
        //
        // See: https://github.com/tc39/proposal-temporal/issues/1122
        let round = SpanRound::new().mode(RoundMode::Trunc);
        ZonedDifference { zoned, round }
    }

    /// Set the smallest units allowed in the span returned.
    ///
    /// When a largest unit is not specified and the smallest unit is hours
    /// or greater, then the largest unit is automatically set to be equal to
    /// the smallest unit.
    ///
    /// # Errors
    ///
    /// The smallest units must be no greater than the largest units. If this
    /// is violated, then computing a span with this configuration will result
    /// in an error.
    ///
    /// # Example
    ///
    /// This shows how to round a span between two zoned datetimes to the
    /// nearest number of weeks.
    ///
    /// ```
    /// use jiff::{RoundMode, ToSpan, Unit, Zoned, ZonedDifference};
    ///
    /// let zdt1 = "2024-03-15 08:14[America/New_York]".parse::<Zoned>()?;
    /// let zdt2 = "2030-11-22 08:30[America/New_York]".parse::<Zoned>()?;
    /// let span = zdt1.until(
    ///     ZonedDifference::new(&zdt2)
    ///         .smallest(Unit::Week)
    ///         .largest(Unit::Week)
    ///         .mode(RoundMode::HalfExpand),
    /// )?;
    /// assert_eq!(format!("{span:#}"), "349w");
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn smallest(self, unit: Unit) -> ZonedDifference<'a> {
        ZonedDifference { round: self.round.smallest(unit), ..self }
    }

    /// Set the largest units allowed in the span returned.
    ///
    /// When a largest unit is not specified and the smallest unit is hours
    /// or greater, then the largest unit is automatically set to be equal to
    /// the smallest unit. Otherwise, when the largest unit is not specified,
    /// it is set to hours.
    ///
    /// Once a largest unit is set, there is no way to change this rounding
    /// configuration back to using the "automatic" default. Instead, callers
    /// must create a new configuration.
    ///
    /// # Errors
    ///
    /// The largest units, when set, must be at least as big as the smallest
    /// units (which defaults to [`Unit::Nanosecond`]). If this is violated,
    /// then computing a span with this configuration will result in an error.
    ///
    /// # Example
    ///
    /// This shows how to round a span between two zoned datetimes to units no
    /// bigger than seconds.
    ///
    /// ```
    /// use jiff::{ToSpan, Unit, Zoned, ZonedDifference};
    ///
    /// let zdt1 = "2024-03-15 08:14[America/New_York]".parse::<Zoned>()?;
    /// let zdt2 = "2030-11-22 08:30[America/New_York]".parse::<Zoned>()?;
    /// let span = zdt1.until(
    ///     ZonedDifference::new(&zdt2).largest(Unit::Second),
    /// )?;
    /// assert_eq!(span.to_string(), "PT211079760S");
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn largest(self, unit: Unit) -> ZonedDifference<'a> {
        ZonedDifference { round: self.round.largest(unit), ..self }
    }

    /// Set the rounding mode.
    ///
    /// This defaults to [`RoundMode::Trunc`] since it's plausible that
    /// rounding "up" in the context of computing the span between
    /// two zoned datetimes could be surprising in a number of cases. The
    /// [`RoundMode::HalfExpand`] mode corresponds to typical rounding you
    /// might have learned about in school. But a variety of other rounding
    /// modes exist.
    ///
    /// # Example
    ///
    /// This shows how to always round "up" towards positive infinity.
    ///
    /// ```
    /// use jiff::{RoundMode, ToSpan, Unit, Zoned, ZonedDifference};
    ///
    /// let zdt1 = "2024-03-15 08:10[America/New_York]".parse::<Zoned>()?;
    /// let zdt2 = "2024-03-15 08:11[America/New_York]".parse::<Zoned>()?;
    /// let span = zdt1.until(
    ///     ZonedDifference::new(&zdt2)
    ///         .smallest(Unit::Hour)
    ///         .mode(RoundMode::Ceil),
    /// )?;
    /// // Only one minute elapsed, but we asked to always round up!
    /// assert_eq!(span, 1.hour().fieldwise());
    ///
    /// // Since `Ceil` always rounds toward positive infinity, the behavior
    /// // flips for a negative span.
    /// let span = zdt1.since(
    ///     ZonedDifference::new(&zdt2)
    ///         .smallest(Unit::Hour)
    ///         .mode(RoundMode::Ceil),
    /// )?;
    /// assert_eq!(span, 0.hour().fieldwise());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn mode(self, mode: RoundMode) -> ZonedDifference<'a> {
        ZonedDifference { round: self.round.mode(mode), ..self }
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
    /// When the smallest unit is less than days, the rounding increment must
    /// divide evenly into the next highest unit after the smallest unit
    /// configured (and must not be equivalent to it). For example, if the
    /// smallest unit is [`Unit::Nanosecond`], then *some* of the valid values
    /// for the rounding increment are `1`, `2`, `4`, `5`, `100` and `500`.
    /// Namely, any integer that divides evenly into `1,000` nanoseconds since
    /// there are `1,000` nanoseconds in the next highest unit (microseconds).
    ///
    /// The error will occur when computing the span, and not when setting
    /// the increment here.
    ///
    /// # Example
    ///
    /// This shows how to round the span between two zoned datetimes to the
    /// nearest 5 minute increment.
    ///
    /// ```
    /// use jiff::{RoundMode, ToSpan, Unit, Zoned, ZonedDifference};
    ///
    /// let zdt1 = "2024-03-15 08:19[America/New_York]".parse::<Zoned>()?;
    /// let zdt2 = "2024-03-15 12:52[America/New_York]".parse::<Zoned>()?;
    /// let span = zdt1.until(
    ///     ZonedDifference::new(&zdt2)
    ///         .smallest(Unit::Minute)
    ///         .increment(5)
    ///         .mode(RoundMode::HalfExpand),
    /// )?;
    /// assert_eq!(format!("{span:#}"), "4h 35m");
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn increment(self, increment: i64) -> ZonedDifference<'a> {
        ZonedDifference { round: self.round.increment(increment), ..self }
    }

    /// Returns true if and only if this configuration could change the span
    /// via rounding.
    #[inline]
    fn rounding_may_change_span(&self) -> bool {
        self.round.rounding_may_change_span_ignore_largest()
    }

    /// Returns the span of time from `dt1` to the datetime in this
    /// configuration. The biggest units allowed are determined by the
    /// `smallest` and `largest` settings, but defaults to `Unit::Day`.
    #[inline]
    fn until_with_largest_unit(&self, zdt1: &Zoned) -> Result<Span, Error> {
        let zdt2 = self.zoned;

        let sign = t::sign(zdt2, zdt1);
        if sign == C(0) {
            return Ok(Span::new());
        }

        let largest = self
            .round
            .get_largest()
            .unwrap_or_else(|| self.round.get_smallest().max(Unit::Hour));
        if largest < Unit::Day {
            return zdt1.timestamp().until((largest, zdt2.timestamp()));
        }
        if zdt1.time_zone() != zdt2.time_zone() {
            return Err(err!(
                "computing the span between zoned datetimes, with \
                 {largest} units, requires that the time zones are \
                 equivalent, but {zdt1} and {zdt2} have distinct \
                 time zones",
                largest = largest.singular(),
            ));
        }
        let tz = zdt1.time_zone();

        let (dt1, mut dt2) = (zdt1.datetime(), zdt2.datetime());

        let mut day_correct: t::SpanDays = C(0).rinto();
        if -sign == dt1.time().until_nanoseconds(dt2.time()).signum() {
            day_correct += C(1);
        }

        let mut mid = dt2
            .date()
            .checked_add(Span::new().days_ranged(day_correct * -sign))
            .with_context(|| {
                err!(
                    "failed to add {days} days to date in {dt2}",
                    days = day_correct * -sign,
                )
            })?
            .to_datetime(dt1.time());
        let mut zmid: Zoned = mid.to_zoned(tz.clone()).with_context(|| {
            err!(
                "failed to convert intermediate datetime {mid} \
                     to zoned timestamp in time zone {tz}",
                tz = tz.diagnostic_name(),
            )
        })?;
        if t::sign(zdt2, &zmid) == -sign {
            if sign == C(-1) {
                panic!("this should be an error");
            }
            day_correct += C(1);
            mid = dt2
                .date()
                .checked_add(Span::new().days_ranged(day_correct * -sign))
                .with_context(|| {
                    err!(
                        "failed to add {days} days to date in {dt2}",
                        days = day_correct * -sign,
                    )
                })?
                .to_datetime(dt1.time());
            zmid = mid.to_zoned(tz.clone()).with_context(|| {
                err!(
                    "failed to convert intermediate datetime {mid} \
                         to zoned timestamp in time zone {tz}",
                    tz = tz.diagnostic_name(),
                )
            })?;
            if t::sign(zdt2, &zmid) == -sign {
                panic!("this should be an error too");
            }
        }
        let remainder_nano = zdt2.timestamp().as_nanosecond_ranged()
            - zmid.timestamp().as_nanosecond_ranged();
        dt2 = mid;

        let date_span = dt1.date().until((largest, dt2.date()))?;
        Ok(Span::from_invariant_nanoseconds(
            Unit::Hour,
            remainder_nano.rinto(),
        )
        .expect("difference between time always fits in span")
        .years_ranged(date_span.get_years_ranged())
        .months_ranged(date_span.get_months_ranged())
        .weeks_ranged(date_span.get_weeks_ranged())
        .days_ranged(date_span.get_days_ranged()))
    }
}

impl<'a> From<&'a Zoned> for ZonedDifference<'a> {
    #[inline]
    fn from(zdt: &'a Zoned) -> ZonedDifference<'a> {
        ZonedDifference::new(zdt)
    }
}

impl<'a> From<(Unit, &'a Zoned)> for ZonedDifference<'a> {
    #[inline]
    fn from((largest, zdt): (Unit, &'a Zoned)) -> ZonedDifference<'a> {
        ZonedDifference::new(zdt).largest(largest)
    }
}

/// Options for [`Zoned::round`].
///
/// This type provides a way to configure the rounding of a zoned datetime. In
/// particular, `Zoned::round` accepts anything that implements the
/// `Into<ZonedRound>` trait. There are some trait implementations that
/// therefore make calling `Zoned::round` in some common cases more
/// ergonomic:
///
/// * `From<Unit> for ZonedRound` will construct a rounding
/// configuration that rounds to the unit given. Specifically,
/// `ZonedRound::new().smallest(unit)`.
/// * `From<(Unit, i64)> for ZonedRound` is like the one above, but also
/// specifies the rounding increment for [`ZonedRound::increment`].
///
/// Note that in the default configuration, no rounding occurs.
///
/// # Example
///
/// This example shows how to round a zoned datetime to the nearest second:
///
/// ```
/// use jiff::{civil::date, Unit, Zoned};
///
/// let zdt: Zoned = "2024-06-20 16:24:59.5[America/New_York]".parse()?;
/// assert_eq!(
///     zdt.round(Unit::Second)?,
///     // The second rounds up and causes minutes to increase.
///     date(2024, 6, 20).at(16, 25, 0, 0).in_tz("America/New_York")?,
/// );
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// The above makes use of the fact that `Unit` implements
/// `Into<ZonedRound>`. If you want to change the rounding mode to, say,
/// truncation, then you'll need to construct a `ZonedRound` explicitly
/// since there are no convenience `Into` trait implementations for
/// [`RoundMode`].
///
/// ```
/// use jiff::{civil::date, RoundMode, Unit, Zoned, ZonedRound};
///
/// let zdt: Zoned = "2024-06-20 16:24:59.5[America/New_York]".parse()?;
/// assert_eq!(
///     zdt.round(
///         ZonedRound::new().smallest(Unit::Second).mode(RoundMode::Trunc),
///     )?,
///     // The second just gets truncated as if it wasn't there.
///     date(2024, 6, 20).at(16, 24, 59, 0).in_tz("America/New_York")?,
/// );
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Copy, Debug)]
pub struct ZonedRound {
    round: DateTimeRound,
}

impl ZonedRound {
    /// Create a new default configuration for rounding a [`Zoned`].
    #[inline]
    pub fn new() -> ZonedRound {
        ZonedRound { round: DateTimeRound::new() }
    }

    /// Set the smallest units allowed in the zoned datetime returned after
    /// rounding.
    ///
    /// Any units below the smallest configured unit will be used, along
    /// with the rounding increment and rounding mode, to determine
    /// the value of the smallest unit. For example, when rounding
    /// `2024-06-20T03:25:30[America/New_York]` to the nearest minute, the `30`
    /// second unit will result in rounding the minute unit of `25` up to `26`
    /// and zeroing out everything below minutes.
    ///
    /// This defaults to [`Unit::Nanosecond`].
    ///
    /// # Errors
    ///
    /// The smallest units must be no greater than [`Unit::Day`]. And when the
    /// smallest unit is `Unit::Day`, the rounding increment must be equal to
    /// `1`. Otherwise an error will be returned from [`Zoned::round`].
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::{civil::date, Unit, ZonedRound};
    ///
    /// let zdt = date(2024, 6, 20).at(3, 25, 30, 0).in_tz("America/New_York")?;
    /// assert_eq!(
    ///     zdt.round(ZonedRound::new().smallest(Unit::Minute))?,
    ///     date(2024, 6, 20).at(3, 26, 0, 0).in_tz("America/New_York")?,
    /// );
    /// // Or, utilize the `From<Unit> for ZonedRound` impl:
    /// assert_eq!(
    ///     zdt.round(Unit::Minute)?,
    ///     date(2024, 6, 20).at(3, 26, 0, 0).in_tz("America/New_York")?,
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn smallest(self, unit: Unit) -> ZonedRound {
        ZonedRound { round: self.round.smallest(unit) }
    }

    /// Set the rounding mode.
    ///
    /// This defaults to [`RoundMode::HalfExpand`], which rounds away from
    /// zero. It matches the kind of rounding you might have been taught in
    /// school.
    ///
    /// # Example
    ///
    /// This shows how to always round zoned datetimes up towards positive
    /// infinity.
    ///
    /// ```
    /// use jiff::{civil::date, RoundMode, Unit, Zoned, ZonedRound};
    ///
    /// let zdt: Zoned = "2024-06-20 03:25:01[America/New_York]".parse()?;
    /// assert_eq!(
    ///     zdt.round(
    ///         ZonedRound::new()
    ///             .smallest(Unit::Minute)
    ///             .mode(RoundMode::Ceil),
    ///     )?,
    ///     date(2024, 6, 20).at(3, 26, 0, 0).in_tz("America/New_York")?,
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn mode(self, mode: RoundMode) -> ZonedRound {
        ZonedRound { round: self.round.mode(mode) }
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
    /// When the smallest unit is `Unit::Day`, then the rounding increment must
    /// be `1` or else [`Zoned::round`] will return an error.
    ///
    /// For other units, the rounding increment must divide evenly into the
    /// next highest unit above the smallest unit set. The rounding increment
    /// must also not be equal to the next highest unit. For example, if the
    /// smallest unit is [`Unit::Nanosecond`], then *some* of the valid values
    /// for the rounding increment are `1`, `2`, `4`, `5`, `100` and `500`.
    /// Namely, any integer that divides evenly into `1,000` nanoseconds since
    /// there are `1,000` nanoseconds in the next highest unit (microseconds).
    ///
    /// # Example
    ///
    /// This example shows how to round a zoned datetime to the nearest 10
    /// minute increment.
    ///
    /// ```
    /// use jiff::{civil::date, RoundMode, Unit, Zoned, ZonedRound};
    ///
    /// let zdt: Zoned = "2024-06-20 03:24:59[America/New_York]".parse()?;
    /// assert_eq!(
    ///     zdt.round((Unit::Minute, 10))?,
    ///     date(2024, 6, 20).at(3, 20, 0, 0).in_tz("America/New_York")?,
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn increment(self, increment: i64) -> ZonedRound {
        ZonedRound { round: self.round.increment(increment) }
    }

    /// Does the actual rounding.
    ///
    /// Most of the work is farmed out to civil datetime rounding.
    pub(crate) fn round(&self, zdt: &Zoned) -> Result<Zoned, Error> {
        let start = zdt.datetime();
        if self.round.get_smallest() == Unit::Day {
            return self.round_days(zdt);
        }
        let end = self.round.round(start)?;
        // Like in the ZonedWith API, in order to avoid small changes to clock
        // time hitting a 1 hour disambiguation shift, we use offset conflict
        // resolution to do our best to "prefer" the offset we already have.
        let amb = OffsetConflict::PreferOffset.resolve(
            end,
            zdt.offset(),
            zdt.time_zone().clone(),
        )?;
        amb.compatible()
    }

    /// Does rounding when the smallest unit is equal to days. We don't reuse
    /// civil datetime rounding for this since the length of a day for a zoned
    /// datetime might not be 24 hours.
    ///
    /// Ref: https://tc39.es/proposal-temporal/#sec-temporal.zoneddatetime.prototype.round
    fn round_days(&self, zdt: &Zoned) -> Result<Zoned, Error> {
        debug_assert_eq!(self.round.get_smallest(), Unit::Day);

        // Rounding by days requires an increment of 1. We just re-use the
        // civil datetime rounding checks, which has the same constraint
        // although it does check for other things that aren't relevant here.
        increment::for_datetime(Unit::Day, self.round.get_increment())?;

        // FIXME: We should be doing this with a &TimeZone, but will need a
        // refactor so that we do zone-aware arithmetic using just a Timestamp
        // and a &TimeZone. Fixing just this should just be some minor annoying
        // work. The grander refactor is something like an `Unzoned` type, but
        // I'm not sure that's really worth it. ---AG
        let start = zdt.start_of_day().with_context(move || {
            err!("failed to find start of day for {zdt}")
        })?;
        let end = start
            .checked_add(Span::new().days_ranged(C(1).rinto()))
            .with_context(|| {
                err!("failed to add 1 day to {start} to find length of day")
            })?;
        let span = start
            .timestamp()
            .until((Unit::Nanosecond, end.timestamp()))
            .with_context(|| {
                err!(
                    "failed to compute span in nanoseconds \
                     from {start} until {end}"
                )
            })?;
        let nanos = span.get_nanoseconds_ranged();
        let day_length =
            ZonedDayNanoseconds::try_rfrom("nanoseconds-per-zoned-day", nanos)
                .with_context(|| {
                    err!(
                        "failed to convert span between {start} until {end} \
                         to nanoseconds",
                    )
                })?;
        let progress = zdt.timestamp().as_nanosecond_ranged()
            - start.timestamp().as_nanosecond_ranged();
        let rounded = self.round.get_mode().round(progress, day_length);
        let nanos = start
            .timestamp()
            .as_nanosecond_ranged()
            .try_checked_add("timestamp-nanos", rounded)?;
        Ok(Timestamp::from_nanosecond_ranged(nanos)
            .to_zoned(zdt.time_zone().clone()))
    }
}

impl Default for ZonedRound {
    #[inline]
    fn default() -> ZonedRound {
        ZonedRound::new()
    }
}

impl From<Unit> for ZonedRound {
    #[inline]
    fn from(unit: Unit) -> ZonedRound {
        ZonedRound::default().smallest(unit)
    }
}

impl From<(Unit, i64)> for ZonedRound {
    #[inline]
    fn from((unit, increment): (Unit, i64)) -> ZonedRound {
        ZonedRound::from(unit).increment(increment)
    }
}

/// A builder for setting the fields on a [`Zoned`].
///
/// This builder is constructed via [`Zoned::with`].
///
/// # Example
///
/// The builder ensures one can chain together the individual components of a
/// zoned datetime without it failing at an intermediate step. For example,
/// if you had a date of `2024-10-31T00:00:00[America/New_York]` and wanted
/// to change both the day and the month, and each setting was validated
/// independent of the other, you would need to be careful to set the day first
/// and then the month. In some cases, you would need to set the month first
/// and then the day!
///
/// But with the builder, you can set values in any order:
///
/// ```
/// use jiff::civil::date;
///
/// let zdt1 = date(2024, 10, 31).at(0, 0, 0, 0).in_tz("America/New_York")?;
/// let zdt2 = zdt1.with().month(11).day(30).build()?;
/// assert_eq!(
///     zdt2,
///     date(2024, 11, 30).at(0, 0, 0, 0).in_tz("America/New_York")?,
/// );
///
/// let zdt1 = date(2024, 4, 30).at(0, 0, 0, 0).in_tz("America/New_York")?;
/// let zdt2 = zdt1.with().day(31).month(7).build()?;
/// assert_eq!(
///     zdt2,
///     date(2024, 7, 31).at(0, 0, 0, 0).in_tz("America/New_York")?,
/// );
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Debug)]
pub struct ZonedWith {
    original: Zoned,
    datetime_with: DateTimeWith,
    offset: Option<Offset>,
    disambiguation: Disambiguation,
    offset_conflict: OffsetConflict,
}

impl ZonedWith {
    #[inline]
    fn new(original: Zoned) -> ZonedWith {
        let datetime_with = original.datetime().with();
        ZonedWith {
            original,
            datetime_with,
            offset: None,
            disambiguation: Disambiguation::default(),
            offset_conflict: OffsetConflict::PreferOffset,
        }
    }

    /// Create a new `Zoned` from the fields set on this configuration.
    ///
    /// An error occurs when the fields combine to an invalid zoned datetime.
    ///
    /// For any fields not set on this configuration, the values are taken from
    /// the [`Zoned`] that originally created this configuration. When no
    /// values are set, this routine is guaranteed to succeed and will always
    /// return the original zoned datetime without modification.
    ///
    /// # Example
    ///
    /// This creates a zoned datetime corresponding to the last day in the year
    /// at noon:
    ///
    /// ```
    /// use jiff::civil::date;
    ///
    /// let zdt = date(2023, 1, 1).at(12, 0, 0, 0).in_tz("America/New_York")?;
    /// assert_eq!(
    ///     zdt.with().day_of_year_no_leap(365).build()?,
    ///     date(2023, 12, 31).at(12, 0, 0, 0).in_tz("America/New_York")?,
    /// );
    ///
    /// // It also works with leap years for the same input:
    /// let zdt = date(2024, 1, 1).at(12, 0, 0, 0).in_tz("America/New_York")?;
    /// assert_eq!(
    ///     zdt.with().day_of_year_no_leap(365).build()?,
    ///     date(2024, 12, 31).at(12, 0, 0, 0).in_tz("America/New_York")?,
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: error for invalid zoned datetime
    ///
    /// If the fields combine to form an invalid datetime, then an error is
    /// returned:
    ///
    /// ```
    /// use jiff::civil::date;
    ///
    /// let zdt = date(2024, 11, 30).at(15, 30, 0, 0).in_tz("America/New_York")?;
    /// assert!(zdt.with().day(31).build().is_err());
    ///
    /// let zdt = date(2024, 2, 29).at(15, 30, 0, 0).in_tz("America/New_York")?;
    /// assert!(zdt.with().year(2023).build().is_err());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn build(self) -> Result<Zoned, Error> {
        let dt = self.datetime_with.build()?;
        let (_, _, offset, time_zone) = self.original.into_parts();
        let offset = self.offset.unwrap_or(offset);
        let ambiguous = self.offset_conflict.resolve(dt, offset, time_zone)?;
        ambiguous.disambiguate(self.disambiguation)
    }

    /// Set the year, month and day fields via the `Date` given.
    ///
    /// This overrides any previous year, month or day settings.
    ///
    /// # Example
    ///
    /// This shows how to create a new zoned datetime with a different date:
    ///
    /// ```
    /// use jiff::civil::date;
    ///
    /// let zdt1 = date(2005, 11, 5).at(15, 30, 0, 0).in_tz("America/New_York")?;
    /// let zdt2 = zdt1.with().date(date(2017, 10, 31)).build()?;
    /// // The date changes but the time remains the same.
    /// assert_eq!(
    ///     zdt2,
    ///     date(2017, 10, 31).at(15, 30, 0, 0).in_tz("America/New_York")?,
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn date(self, date: Date) -> ZonedWith {
        ZonedWith { datetime_with: self.datetime_with.date(date), ..self }
    }

    /// Set the hour, minute, second, millisecond, microsecond and nanosecond
    /// fields via the `Time` given.
    ///
    /// This overrides any previous hour, minute, second, millisecond,
    /// microsecond, nanosecond or subsecond nanosecond settings.
    ///
    /// # Example
    ///
    /// This shows how to create a new zoned datetime with a different time:
    ///
    /// ```
    /// use jiff::civil::{date, time};
    ///
    /// let zdt1 = date(2005, 11, 5).at(15, 30, 0, 0).in_tz("America/New_York")?;
    /// let zdt2 = zdt1.with().time(time(23, 59, 59, 123_456_789)).build()?;
    /// // The time changes but the date remains the same.
    /// assert_eq!(
    ///     zdt2,
    ///     date(2005, 11, 5)
    ///         .at(23, 59, 59, 123_456_789)
    ///         .in_tz("America/New_York")?,
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn time(self, time: Time) -> ZonedWith {
        ZonedWith { datetime_with: self.datetime_with.time(time), ..self }
    }

    /// Set the year field on a [`Zoned`].
    ///
    /// One can access this value via [`Zoned::year`].
    ///
    /// This overrides any previous year settings.
    ///
    /// # Errors
    ///
    /// This returns an error when [`ZonedWith::build`] is called if the
    /// given year is outside the range `-9999..=9999`. This can also return an
    /// error if the resulting date is otherwise invalid.
    ///
    /// # Example
    ///
    /// This shows how to create a new zoned datetime with a different year:
    ///
    /// ```
    /// use jiff::civil::date;
    ///
    /// let zdt1 = date(2005, 11, 5).at(15, 30, 0, 0).in_tz("America/New_York")?;
    /// assert_eq!(zdt1.year(), 2005);
    /// let zdt2 = zdt1.with().year(2007).build()?;
    /// assert_eq!(zdt2.year(), 2007);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: only changing the year can fail
    ///
    /// For example, while `2024-02-29T01:30:00[America/New_York]` is valid,
    /// `2023-02-29T01:30:00[America/New_York]` is not:
    ///
    /// ```
    /// use jiff::civil::date;
    ///
    /// let zdt = date(2024, 2, 29).at(1, 30, 0, 0).in_tz("America/New_York")?;
    /// assert!(zdt.with().year(2023).build().is_err());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn year(self, year: i16) -> ZonedWith {
        ZonedWith { datetime_with: self.datetime_with.year(year), ..self }
    }

    /// Set the year of a zoned datetime via its era and its non-negative
    /// numeric component.
    ///
    /// One can access this value via [`Zoned::era_year`].
    ///
    /// # Errors
    ///
    /// This returns an error when [`ZonedWith::build`] is called if the
    /// year is outside the range for the era specified. For [`Era::BCE`], the
    /// range is `1..=10000`. For [`Era::CE`], the range is `1..=9999`.
    ///
    /// # Example
    ///
    /// This shows that `CE` years are equivalent to the years used by this
    /// crate:
    ///
    /// ```
    /// use jiff::civil::{Era, date};
    ///
    /// let zdt1 = date(2005, 11, 5).at(8, 0, 0, 0).in_tz("America/New_York")?;
    /// assert_eq!(zdt1.year(), 2005);
    /// let zdt2 = zdt1.with().era_year(2007, Era::CE).build()?;
    /// assert_eq!(zdt2.year(), 2007);
    ///
    /// // CE years are always positive and can be at most 9999:
    /// assert!(zdt1.with().era_year(-5, Era::CE).build().is_err());
    /// assert!(zdt1.with().era_year(10_000, Era::CE).build().is_err());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// But `BCE` years always correspond to years less than or equal to `0`
    /// in this crate:
    ///
    /// ```
    /// use jiff::civil::{Era, date};
    ///
    /// let zdt1 = date(-27, 7, 1).at(8, 22, 30, 0).in_tz("America/New_York")?;
    /// assert_eq!(zdt1.year(), -27);
    /// assert_eq!(zdt1.era_year(), (28, Era::BCE));
    ///
    /// let zdt2 = zdt1.with().era_year(509, Era::BCE).build()?;
    /// assert_eq!(zdt2.year(), -508);
    /// assert_eq!(zdt2.era_year(), (509, Era::BCE));
    ///
    /// let zdt2 = zdt1.with().era_year(10_000, Era::BCE).build()?;
    /// assert_eq!(zdt2.year(), -9_999);
    /// assert_eq!(zdt2.era_year(), (10_000, Era::BCE));
    ///
    /// // BCE years are always positive and can be at most 10000:
    /// assert!(zdt1.with().era_year(-5, Era::BCE).build().is_err());
    /// assert!(zdt1.with().era_year(10_001, Era::BCE).build().is_err());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: overrides `ZonedWith::year`
    ///
    /// Setting this option will override any previous `ZonedWith::year`
    /// option:
    ///
    /// ```
    /// use jiff::civil::{Era, date};
    ///
    /// let zdt1 = date(2024, 7, 2).at(10, 27, 10, 123).in_tz("America/New_York")?;
    /// let zdt2 = zdt1.with().year(2000).era_year(1900, Era::CE).build()?;
    /// assert_eq!(
    ///     zdt2,
    ///     date(1900, 7, 2).at(10, 27, 10, 123).in_tz("America/New_York")?,
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// Similarly, `ZonedWith::year` will override any previous call to
    /// `ZonedWith::era_year`:
    ///
    /// ```
    /// use jiff::civil::{Era, date};
    ///
    /// let zdt1 = date(2024, 7, 2).at(19, 0, 1, 1).in_tz("America/New_York")?;
    /// let zdt2 = zdt1.with().era_year(1900, Era::CE).year(2000).build()?;
    /// assert_eq!(
    ///     zdt2,
    ///     date(2000, 7, 2).at(19, 0, 1, 1).in_tz("America/New_York")?,
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn era_year(self, year: i16, era: Era) -> ZonedWith {
        ZonedWith {
            datetime_with: self.datetime_with.era_year(year, era),
            ..self
        }
    }

    /// Set the month field on a [`Zoned`].
    ///
    /// One can access this value via [`Zoned::month`].
    ///
    /// This overrides any previous month settings.
    ///
    /// # Errors
    ///
    /// This returns an error when [`ZonedWith::build`] is called if the
    /// given month is outside the range `1..=12`. This can also return an
    /// error if the resulting date is otherwise invalid.
    ///
    /// # Example
    ///
    /// This shows how to create a new zoned datetime with a different month:
    ///
    /// ```
    /// use jiff::civil::date;
    ///
    /// let zdt1 = date(2005, 11, 5)
    ///     .at(18, 3, 59, 123_456_789)
    ///     .in_tz("America/New_York")?;
    /// assert_eq!(zdt1.month(), 11);
    ///
    /// let zdt2 = zdt1.with().month(6).build()?;
    /// assert_eq!(zdt2.month(), 6);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: only changing the month can fail
    ///
    /// For example, while `2024-10-31T00:00:00[America/New_York]` is valid,
    /// `2024-11-31T00:00:00[America/New_York]` is not:
    ///
    /// ```
    /// use jiff::civil::date;
    ///
    /// let zdt = date(2024, 10, 31).at(0, 0, 0, 0).in_tz("America/New_York")?;
    /// assert!(zdt.with().month(11).build().is_err());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn month(self, month: i8) -> ZonedWith {
        ZonedWith { datetime_with: self.datetime_with.month(month), ..self }
    }

    /// Set the day field on a [`Zoned`].
    ///
    /// One can access this value via [`Zoned::day`].
    ///
    /// This overrides any previous day settings.
    ///
    /// # Errors
    ///
    /// This returns an error when [`ZonedWith::build`] is called if the
    /// given given day is outside of allowable days for the corresponding year
    /// and month fields.
    ///
    /// # Example
    ///
    /// This shows some examples of setting the day, including a leap day:
    ///
    /// ```
    /// use jiff::civil::date;
    ///
    /// let zdt1 = date(2024, 2, 5).at(21, 59, 1, 999).in_tz("America/New_York")?;
    /// assert_eq!(zdt1.day(), 5);
    /// let zdt2 = zdt1.with().day(10).build()?;
    /// assert_eq!(zdt2.day(), 10);
    /// let zdt3 = zdt1.with().day(29).build()?;
    /// assert_eq!(zdt3.day(), 29);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: changing only the day can fail
    ///
    /// This shows some examples that will fail:
    ///
    /// ```
    /// use jiff::civil::date;
    ///
    /// let zdt1 = date(2023, 2, 5)
    ///     .at(22, 58, 58, 9_999)
    ///     .in_tz("America/New_York")?;
    /// // 2023 is not a leap year
    /// assert!(zdt1.with().day(29).build().is_err());
    ///
    /// // September has 30 days, not 31.
    /// let zdt1 = date(2023, 9, 5).in_tz("America/New_York")?;
    /// assert!(zdt1.with().day(31).build().is_err());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn day(self, day: i8) -> ZonedWith {
        ZonedWith { datetime_with: self.datetime_with.day(day), ..self }
    }

    /// Set the day field on a [`Zoned`] via the ordinal number of a day
    /// within a year.
    ///
    /// When used, any settings for month are ignored since the month is
    /// determined by the day of the year.
    ///
    /// The valid values for `day` are `1..=366`. Note though that `366` is
    /// only valid for leap years.
    ///
    /// This overrides any previous day settings.
    ///
    /// # Errors
    ///
    /// This returns an error when [`ZonedWith::build`] is called if the
    /// given day is outside the allowed range of `1..=366`, or when a value of
    /// `366` is given for a non-leap year.
    ///
    /// # Example
    ///
    /// This demonstrates that if a year is a leap year, then `60` corresponds
    /// to February 29:
    ///
    /// ```
    /// use jiff::civil::date;
    ///
    /// let zdt = date(2024, 1, 1)
    ///     .at(23, 59, 59, 999_999_999)
    ///     .in_tz("America/New_York")?;
    /// assert_eq!(
    ///     zdt.with().day_of_year(60).build()?,
    ///     date(2024, 2, 29)
    ///         .at(23, 59, 59, 999_999_999)
    ///         .in_tz("America/New_York")?,
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// But for non-leap years, day 60 is March 1:
    ///
    /// ```
    /// use jiff::civil::date;
    ///
    /// let zdt = date(2023, 1, 1)
    ///     .at(23, 59, 59, 999_999_999)
    ///     .in_tz("America/New_York")?;
    /// assert_eq!(
    ///     zdt.with().day_of_year(60).build()?,
    ///     date(2023, 3, 1)
    ///         .at(23, 59, 59, 999_999_999)
    ///         .in_tz("America/New_York")?,
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// And using `366` for a non-leap year will result in an error, since
    /// non-leap years only have 365 days:
    ///
    /// ```
    /// use jiff::civil::date;
    ///
    /// let zdt = date(2023, 1, 1).at(0, 0, 0, 0).in_tz("America/New_York")?;
    /// assert!(zdt.with().day_of_year(366).build().is_err());
    /// // The maximal year is not a leap year, so it returns an error too.
    /// let zdt = date(9999, 1, 1).at(0, 0, 0, 0).in_tz("America/New_York")?;
    /// assert!(zdt.with().day_of_year(366).build().is_err());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn day_of_year(self, day: i16) -> ZonedWith {
        ZonedWith {
            datetime_with: self.datetime_with.day_of_year(day),
            ..self
        }
    }

    /// Set the day field on a [`Zoned`] via the ordinal number of a day
    /// within a year, but ignoring leap years.
    ///
    /// When used, any settings for month are ignored since the month is
    /// determined by the day of the year.
    ///
    /// The valid values for `day` are `1..=365`. The value `365` always
    /// corresponds to the last day of the year, even for leap years. It is
    /// impossible for this routine to return a zoned datetime corresponding to
    /// February 29. (Unless there is a relevant time zone transition that
    /// provokes disambiguation that shifts the datetime into February 29.)
    ///
    /// This overrides any previous day settings.
    ///
    /// # Errors
    ///
    /// This returns an error when [`ZonedWith::build`] is called if the
    /// given day is outside the allowed range of `1..=365`.
    ///
    /// # Example
    ///
    /// This demonstrates that `60` corresponds to March 1, regardless of
    /// whether the year is a leap year or not:
    ///
    /// ```
    /// use jiff::civil::date;
    ///
    /// let zdt = date(2023, 1, 1)
    ///     .at(23, 59, 59, 999_999_999)
    ///     .in_tz("America/New_York")?;
    /// assert_eq!(
    ///     zdt.with().day_of_year_no_leap(60).build()?,
    ///     date(2023, 3, 1)
    ///         .at(23, 59, 59, 999_999_999)
    ///         .in_tz("America/New_York")?,
    /// );
    ///
    /// let zdt = date(2024, 1, 1)
    ///     .at(23, 59, 59, 999_999_999)
    ///     .in_tz("America/New_York")?;
    /// assert_eq!(
    ///     zdt.with().day_of_year_no_leap(60).build()?,
    ///     date(2024, 3, 1)
    ///         .at(23, 59, 59, 999_999_999)
    ///         .in_tz("America/New_York")?,
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// And using `365` for any year will always yield the last day of the
    /// year:
    ///
    /// ```
    /// use jiff::civil::date;
    ///
    /// let zdt = date(2023, 1, 1)
    ///     .at(23, 59, 59, 999_999_999)
    ///     .in_tz("America/New_York")?;
    /// assert_eq!(
    ///     zdt.with().day_of_year_no_leap(365).build()?,
    ///     zdt.last_of_year()?,
    /// );
    ///
    /// let zdt = date(2024, 1, 1)
    ///     .at(23, 59, 59, 999_999_999)
    ///     .in_tz("America/New_York")?;
    /// assert_eq!(
    ///     zdt.with().day_of_year_no_leap(365).build()?,
    ///     zdt.last_of_year()?,
    /// );
    ///
    /// // Careful at the boundaries. The last day of the year isn't
    /// // representable with all time zones. For example:
    /// let zdt = date(9999, 1, 1)
    ///     .at(23, 59, 59, 999_999_999)
    ///     .in_tz("America/New_York")?;
    /// assert!(zdt.with().day_of_year_no_leap(365).build().is_err());
    /// // But with other time zones, it works okay:
    /// let zdt = date(9999, 1, 1)
    ///     .at(23, 59, 59, 999_999_999)
    ///     .to_zoned(jiff::tz::TimeZone::fixed(jiff::tz::Offset::MAX))?;
    /// assert_eq!(
    ///     zdt.with().day_of_year_no_leap(365).build()?,
    ///     zdt.last_of_year()?,
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// A value of `366` is out of bounds, even for leap years:
    ///
    /// ```
    /// use jiff::civil::date;
    ///
    /// let zdt = date(2024, 1, 1).at(5, 30, 0, 0).in_tz("America/New_York")?;
    /// assert!(zdt.with().day_of_year_no_leap(366).build().is_err());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn day_of_year_no_leap(self, day: i16) -> ZonedWith {
        ZonedWith {
            datetime_with: self.datetime_with.day_of_year_no_leap(day),
            ..self
        }
    }

    /// Set the hour field on a [`Zoned`].
    ///
    /// One can access this value via [`Zoned::hour`].
    ///
    /// This overrides any previous hour settings.
    ///
    /// # Errors
    ///
    /// This returns an error when [`ZonedWith::build`] is called if the
    /// given hour is outside the range `0..=23`.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::civil::time;
    ///
    /// let zdt1 = time(15, 21, 59, 0).on(2010, 6, 1).in_tz("America/New_York")?;
    /// assert_eq!(zdt1.hour(), 15);
    /// let zdt2 = zdt1.with().hour(3).build()?;
    /// assert_eq!(zdt2.hour(), 3);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn hour(self, hour: i8) -> ZonedWith {
        ZonedWith { datetime_with: self.datetime_with.hour(hour), ..self }
    }

    /// Set the minute field on a [`Zoned`].
    ///
    /// One can access this value via [`Zoned::minute`].
    ///
    /// This overrides any previous minute settings.
    ///
    /// # Errors
    ///
    /// This returns an error when [`ZonedWith::build`] is called if the
    /// given minute is outside the range `0..=59`.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::civil::time;
    ///
    /// let zdt1 = time(15, 21, 59, 0).on(2010, 6, 1).in_tz("America/New_York")?;
    /// assert_eq!(zdt1.minute(), 21);
    /// let zdt2 = zdt1.with().minute(3).build()?;
    /// assert_eq!(zdt2.minute(), 3);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn minute(self, minute: i8) -> ZonedWith {
        ZonedWith { datetime_with: self.datetime_with.minute(minute), ..self }
    }

    /// Set the second field on a [`Zoned`].
    ///
    /// One can access this value via [`Zoned::second`].
    ///
    /// This overrides any previous second settings.
    ///
    /// # Errors
    ///
    /// This returns an error when [`ZonedWith::build`] is called if the
    /// given second is outside the range `0..=59`.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::civil::time;
    ///
    /// let zdt1 = time(15, 21, 59, 0).on(2010, 6, 1).in_tz("America/New_York")?;
    /// assert_eq!(zdt1.second(), 59);
    /// let zdt2 = zdt1.with().second(3).build()?;
    /// assert_eq!(zdt2.second(), 3);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn second(self, second: i8) -> ZonedWith {
        ZonedWith { datetime_with: self.datetime_with.second(second), ..self }
    }

    /// Set the millisecond field on a [`Zoned`].
    ///
    /// One can access this value via [`Zoned::millisecond`].
    ///
    /// This overrides any previous millisecond settings.
    ///
    /// Note that this only sets the millisecond component. It does
    /// not change the microsecond or nanosecond components. To set
    /// the fractional second component to nanosecond precision, use
    /// [`ZonedWith::subsec_nanosecond`].
    ///
    /// # Errors
    ///
    /// This returns an error when [`ZonedWith::build`] is called if the
    /// given millisecond is outside the range `0..=999`, or if both this and
    /// [`ZonedWith::subsec_nanosecond`] are set.
    ///
    /// # Example
    ///
    /// This shows the relationship between [`Zoned::millisecond`] and
    /// [`Zoned::subsec_nanosecond`]:
    ///
    /// ```
    /// use jiff::civil::time;
    ///
    /// let zdt1 = time(15, 21, 35, 0).on(2010, 6, 1).in_tz("America/New_York")?;
    /// let zdt2 = zdt1.with().millisecond(123).build()?;
    /// assert_eq!(zdt2.subsec_nanosecond(), 123_000_000);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn millisecond(self, millisecond: i16) -> ZonedWith {
        ZonedWith {
            datetime_with: self.datetime_with.millisecond(millisecond),
            ..self
        }
    }

    /// Set the microsecond field on a [`Zoned`].
    ///
    /// One can access this value via [`Zoned::microsecond`].
    ///
    /// This overrides any previous microsecond settings.
    ///
    /// Note that this only sets the microsecond component. It does
    /// not change the millisecond or nanosecond components. To set
    /// the fractional second component to nanosecond precision, use
    /// [`ZonedWith::subsec_nanosecond`].
    ///
    /// # Errors
    ///
    /// This returns an error when [`ZonedWith::build`] is called if the
    /// given microsecond is outside the range `0..=999`, or if both this and
    /// [`ZonedWith::subsec_nanosecond`] are set.
    ///
    /// # Example
    ///
    /// This shows the relationship between [`Zoned::microsecond`] and
    /// [`Zoned::subsec_nanosecond`]:
    ///
    /// ```
    /// use jiff::civil::time;
    ///
    /// let zdt1 = time(15, 21, 35, 0).on(2010, 6, 1).in_tz("America/New_York")?;
    /// let zdt2 = zdt1.with().microsecond(123).build()?;
    /// assert_eq!(zdt2.subsec_nanosecond(), 123_000);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn microsecond(self, microsecond: i16) -> ZonedWith {
        ZonedWith {
            datetime_with: self.datetime_with.microsecond(microsecond),
            ..self
        }
    }

    /// Set the nanosecond field on a [`Zoned`].
    ///
    /// One can access this value via [`Zoned::nanosecond`].
    ///
    /// This overrides any previous nanosecond settings.
    ///
    /// Note that this only sets the nanosecond component. It does
    /// not change the millisecond or microsecond components. To set
    /// the fractional second component to nanosecond precision, use
    /// [`ZonedWith::subsec_nanosecond`].
    ///
    /// # Errors
    ///
    /// This returns an error when [`ZonedWith::build`] is called if the
    /// given nanosecond is outside the range `0..=999`, or if both this and
    /// [`ZonedWith::subsec_nanosecond`] are set.
    ///
    /// # Example
    ///
    /// This shows the relationship between [`Zoned::nanosecond`] and
    /// [`Zoned::subsec_nanosecond`]:
    ///
    /// ```
    /// use jiff::civil::time;
    ///
    /// let zdt1 = time(15, 21, 35, 0).on(2010, 6, 1).in_tz("America/New_York")?;
    /// let zdt2 = zdt1.with().nanosecond(123).build()?;
    /// assert_eq!(zdt2.subsec_nanosecond(), 123);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn nanosecond(self, nanosecond: i16) -> ZonedWith {
        ZonedWith {
            datetime_with: self.datetime_with.nanosecond(nanosecond),
            ..self
        }
    }

    /// Set the subsecond nanosecond field on a [`Zoned`].
    ///
    /// If you want to access this value on `Zoned`, then use
    /// [`Zoned::subsec_nanosecond`].
    ///
    /// This overrides any previous subsecond nanosecond settings.
    ///
    /// Note that this sets the entire fractional second component to
    /// nanosecond precision, and overrides any individual millisecond,
    /// microsecond or nanosecond settings. To set individual components,
    /// use [`ZonedWith::millisecond`], [`ZonedWith::microsecond`] or
    /// [`ZonedWith::nanosecond`].
    ///
    /// # Errors
    ///
    /// This returns an error when [`ZonedWith::build`] is called if the
    /// given subsecond nanosecond is outside the range `0..=999,999,999`,
    /// or if both this and one of [`ZonedWith::millisecond`],
    /// [`ZonedWith::microsecond`] or [`ZonedWith::nanosecond`] are set.
    ///
    /// # Example
    ///
    /// This shows the relationship between constructing a `Zoned` value
    /// with subsecond nanoseconds and its individual subsecond fields:
    ///
    /// ```
    /// use jiff::civil::time;
    ///
    /// let zdt1 = time(15, 21, 35, 0).on(2010, 6, 1).in_tz("America/New_York")?;
    /// let zdt2 = zdt1.with().subsec_nanosecond(123_456_789).build()?;
    /// assert_eq!(zdt2.millisecond(), 123);
    /// assert_eq!(zdt2.microsecond(), 456);
    /// assert_eq!(zdt2.nanosecond(), 789);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn subsec_nanosecond(self, subsec_nanosecond: i32) -> ZonedWith {
        ZonedWith {
            datetime_with: self
                .datetime_with
                .subsec_nanosecond(subsec_nanosecond),
            ..self
        }
    }

    /// Set the offset to use in the new zoned datetime.
    ///
    /// This can be used in some cases to explicitly disambiguate a datetime
    /// that could correspond to multiple instants in time.
    ///
    /// How the offset is used to construct a new zoned datetime
    /// depends on the offset conflict resolution strategy
    /// set via [`ZonedWith::offset_conflict`]. The default is
    /// [`OffsetConflict::PreferOffset`], which will always try to use the
    /// offset to resolve a datetime to an instant, unless the offset is
    /// incorrect for this zoned datetime's time zone. In which case, only the
    /// time zone is used to select the correct offset (which may involve using
    /// the disambiguation strategy set via [`ZonedWith::disambiguation`]).
    ///
    /// # Example
    ///
    /// This example shows parsing the first time the 1 o'clock hour appeared
    /// on a clock in New York on 2024-11-03, and then changing only the
    /// offset to flip it to the second time 1 o'clock appeared on the clock:
    ///
    /// ```
    /// use jiff::{tz, Zoned};
    ///
    /// let zdt1: Zoned = "2024-11-03 01:30-04[America/New_York]".parse()?;
    /// let zdt2 = zdt1.with().offset(tz::offset(-5)).build()?;
    /// assert_eq!(
    ///     zdt2.to_string(),
    ///     // Everything stays the same, except for the offset.
    ///     "2024-11-03T01:30:00-05:00[America/New_York]",
    /// );
    ///
    /// // If we use an invalid offset for the America/New_York time zone,
    /// // then it will be ignored and the disambiguation strategy set will
    /// // be used.
    /// let zdt3 = zdt1.with().offset(tz::offset(-12)).build()?;
    /// assert_eq!(
    ///     zdt3.to_string(),
    ///     // The default disambiguation is Compatible.
    ///     "2024-11-03T01:30:00-04:00[America/New_York]",
    /// );
    /// // But we could change the disambiguation strategy to reject such
    /// // cases!
    /// let result = zdt1
    ///     .with()
    ///     .offset(tz::offset(-12))
    ///     .disambiguation(tz::Disambiguation::Reject)
    ///     .build();
    /// assert!(result.is_err());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn offset(self, offset: Offset) -> ZonedWith {
        ZonedWith { offset: Some(offset), ..self }
    }

    /// Set the conflict resolution strategy for when an offset is inconsistent
    /// with the time zone.
    ///
    /// See the documentation on [`OffsetConflict`] for more details about the
    /// different strategies one can choose.
    ///
    /// Unlike parsing (where the default is `OffsetConflict::Reject`), the
    /// default for `ZonedWith` is [`OffsetConflict::PreferOffset`], which
    /// avoids daylight saving time disambiguation causing unexpected 1-hour
    /// shifts after small changes to clock time.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::Zoned;
    ///
    /// // Set to the "second" time 1:30 is on the clocks in New York on
    /// // 2024-11-03. The offset in the datetime string makes this
    /// // unambiguous.
    /// let zdt1 = "2024-11-03T01:30-05[America/New_York]".parse::<Zoned>()?;
    /// // Now we change the minute field:
    /// let zdt2 = zdt1.with().minute(34).build()?;
    /// assert_eq!(
    ///     zdt2.to_string(),
    ///     // Without taking the offset of the `Zoned` value into account,
    ///     // this would have defaulted to using the "compatible"
    ///     // disambiguation strategy, which would have selected the earlier
    ///     // offset of -04 instead of sticking with the later offset of -05.
    ///     "2024-11-03T01:34:00-05:00[America/New_York]",
    /// );
    ///
    /// // But note that if we change the clock time such that the previous
    /// // offset is no longer valid (by moving back before DST ended), then
    /// // the default strategy will automatically adapt and change the offset.
    /// let zdt2 = zdt1.with().hour(0).build()?;
    /// assert_eq!(
    ///     zdt2.to_string(),
    ///     "2024-11-03T00:30:00-04:00[America/New_York]",
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn offset_conflict(self, strategy: OffsetConflict) -> ZonedWith {
        ZonedWith { offset_conflict: strategy, ..self }
    }

    /// Set the disambiguation strategy for when a zoned datetime falls into a
    /// time zone transition "fold" or "gap."
    ///
    /// The most common manifestation of such time zone transitions is daylight
    /// saving time. In most cases, the transition into daylight saving time
    /// moves the civil time ("the time you see on the clock") ahead one hour.
    /// This is called a "gap" because an hour on the clock is skipped. While
    /// the transition out of daylight saving time moves the civil time back
    /// one hour. This is called a "fold" because an hour on the clock is
    /// repeated.
    ///
    /// In the case of a gap, an ambiguous datetime manifests as a time that
    /// never appears on a clock. (For example, `02:30` on `2024-03-10` in New
    /// York.) In the case of a fold, an ambiguous datetime manifests as a
    /// time that repeats itself. (For example, `01:30` on `2024-11-03` in New
    /// York.) So when a fold occurs, you don't know whether it's the "first"
    /// occurrence of that time or the "second."
    ///
    /// Time zone transitions are not just limited to daylight saving time,
    /// although those are the most common. In other cases, a transition occurs
    /// because of a change in the offset of the time zone itself. (See the
    /// examples below.)
    ///
    /// # Example: time zone offset change
    ///
    /// In this example, we explore a time zone offset change in Hawaii that
    /// occurred on `1947-06-08`. Namely, Hawaii went from a `-10:30` offset
    /// to a `-10:00` offset at `02:00`. This results in a 30 minute gap in
    /// civil time.
    ///
    /// ```
    /// use jiff::{civil::date, tz, ToSpan, Zoned};
    ///
    /// // This datetime is unambiguous...
    /// let zdt1 = "1943-06-02T02:05[Pacific/Honolulu]".parse::<Zoned>()?;
    /// // but... 02:05 didn't exist on clocks on 1947-06-08.
    /// let zdt2 = zdt1
    ///     .with()
    ///     .disambiguation(tz::Disambiguation::Later)
    ///     .year(1947)
    ///     .day(8)
    ///     .build()?;
    /// // Our parser is configured to select the later time, so we jump to
    /// // 02:35. But if we used `Disambiguation::Earlier`, then we'd get
    /// // 01:35.
    /// assert_eq!(zdt2.datetime(), date(1947, 6, 8).at(2, 35, 0, 0));
    /// assert_eq!(zdt2.offset(), tz::offset(-10));
    ///
    /// // If we subtract 10 minutes from 02:35, notice that we (correctly)
    /// // jump to 01:55 *and* our offset is corrected to -10:30.
    /// let zdt3 = zdt2.checked_sub(10.minutes())?;
    /// assert_eq!(zdt3.datetime(), date(1947, 6, 8).at(1, 55, 0, 0));
    /// assert_eq!(zdt3.offset(), tz::offset(-10).saturating_sub(30.minutes()));
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: offset conflict resolution and disambiguation
    ///
    /// This example shows how the disambiguation configuration can
    /// interact with the default offset conflict resolution strategy of
    /// [`OffsetConflict::PreferOffset`]:
    ///
    /// ```
    /// use jiff::{civil::date, tz, Zoned};
    ///
    /// // This datetime is unambiguous.
    /// let zdt1 = "2024-03-11T02:05[America/New_York]".parse::<Zoned>()?;
    /// assert_eq!(zdt1.offset(), tz::offset(-4));
    /// // But the same time on March 10 is ambiguous because there is a gap!
    /// let zdt2 = zdt1
    ///     .with()
    ///     .disambiguation(tz::Disambiguation::Earlier)
    ///     .day(10)
    ///     .build()?;
    /// assert_eq!(zdt2.datetime(), date(2024, 3, 10).at(1, 5, 0, 0));
    /// assert_eq!(zdt2.offset(), tz::offset(-5));
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// Namely, while we started with an offset of `-04`, it (along with all
    /// other offsets) are considered invalid during civil time gaps due to
    /// time zone transitions (such as the beginning of daylight saving time in
    /// most locations).
    ///
    /// The default disambiguation strategy is
    /// [`Disambiguation::Compatible`], which in the case of gaps, chooses the
    /// time after the gap:
    ///
    /// ```
    /// use jiff::{civil::date, tz, Zoned};
    ///
    /// // This datetime is unambiguous.
    /// let zdt1 = "2024-03-11T02:05[America/New_York]".parse::<Zoned>()?;
    /// assert_eq!(zdt1.offset(), tz::offset(-4));
    /// // But the same time on March 10 is ambiguous because there is a gap!
    /// let zdt2 = zdt1
    ///     .with()
    ///     .day(10)
    ///     .build()?;
    /// assert_eq!(zdt2.datetime(), date(2024, 3, 10).at(3, 5, 0, 0));
    /// assert_eq!(zdt2.offset(), tz::offset(-4));
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// Alternatively, one can choose to always respect the offset, and thus
    /// civil time for the provided time zone will be adjusted to match the
    /// instant prescribed by the offset. In this case, no disambiguation is
    /// performed:
    ///
    /// ```
    /// use jiff::{civil::date, tz, Zoned};
    ///
    /// // This datetime is unambiguous. But `2024-03-10T02:05` is!
    /// let zdt1 = "2024-03-11T02:05[America/New_York]".parse::<Zoned>()?;
    /// assert_eq!(zdt1.offset(), tz::offset(-4));
    /// // But the same time on March 10 is ambiguous because there is a gap!
    /// let zdt2 = zdt1
    ///     .with()
    ///     .offset_conflict(tz::OffsetConflict::AlwaysOffset)
    ///     .day(10)
    ///     .build()?;
    /// // Why do we get this result? Because `2024-03-10T02:05-04` is
    /// // `2024-03-10T06:05Z`. And in `America/New_York`, the civil time
    /// // for that timestamp is `2024-03-10T01:05-05`.
    /// assert_eq!(zdt2.datetime(), date(2024, 3, 10).at(1, 5, 0, 0));
    /// assert_eq!(zdt2.offset(), tz::offset(-5));
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn disambiguation(self, strategy: Disambiguation) -> ZonedWith {
        ZonedWith { disambiguation: strategy, ..self }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use alloc::string::ToString;

    use crate::{
        civil::{date, datetime},
        span::span_eq,
        tz, ToSpan,
    };

    use super::*;

    #[test]
    fn until_with_largest_unit() {
        if crate::tz::db().is_definitively_empty() {
            return;
        }

        let zdt1: Zoned = date(1995, 12, 7)
            .at(3, 24, 30, 3500)
            .in_tz("Asia/Kolkata")
            .unwrap();
        let zdt2: Zoned =
            date(2019, 1, 31).at(15, 30, 0, 0).in_tz("Asia/Kolkata").unwrap();
        let span = zdt1.until(&zdt2).unwrap();
        span_eq!(
            span,
            202956
                .hours()
                .minutes(5)
                .seconds(29)
                .milliseconds(999)
                .microseconds(996)
                .nanoseconds(500)
        );
        let span = zdt1.until((Unit::Year, &zdt2)).unwrap();
        span_eq!(
            span,
            23.years()
                .months(1)
                .days(24)
                .hours(12)
                .minutes(5)
                .seconds(29)
                .milliseconds(999)
                .microseconds(996)
                .nanoseconds(500)
        );

        let span = zdt2.until((Unit::Year, &zdt1)).unwrap();
        span_eq!(
            span,
            -23.years()
                .months(1)
                .days(24)
                .hours(12)
                .minutes(5)
                .seconds(29)
                .milliseconds(999)
                .microseconds(996)
                .nanoseconds(500)
        );
        let span = zdt1.until((Unit::Nanosecond, &zdt2)).unwrap();
        span_eq!(span, 730641929999996500i64.nanoseconds());

        let zdt1: Zoned =
            date(2020, 1, 1).at(0, 0, 0, 0).in_tz("America/New_York").unwrap();
        let zdt2: Zoned = date(2020, 4, 24)
            .at(21, 0, 0, 0)
            .in_tz("America/New_York")
            .unwrap();
        let span = zdt1.until(&zdt2).unwrap();
        span_eq!(span, 2756.hours());
        let span = zdt1.until((Unit::Year, &zdt2)).unwrap();
        span_eq!(span, 3.months().days(23).hours(21));

        let zdt1: Zoned = date(2000, 10, 29)
            .at(0, 0, 0, 0)
            .in_tz("America/Vancouver")
            .unwrap();
        let zdt2: Zoned = date(2000, 10, 29)
            .at(23, 0, 0, 5)
            .in_tz("America/Vancouver")
            .unwrap();
        let span = zdt1.until((Unit::Day, &zdt2)).unwrap();
        span_eq!(span, 24.hours().nanoseconds(5));
    }

    #[cfg(target_pointer_width = "64")]
    #[test]
    fn zoned_size() {
        #[cfg(debug_assertions)]
        {
            #[cfg(feature = "alloc")]
            {
                assert_eq!(96, core::mem::size_of::<Zoned>());
            }
            #[cfg(all(target_pointer_width = "64", not(feature = "alloc")))]
            {
                assert_eq!(96, core::mem::size_of::<Zoned>());
            }
        }
        #[cfg(not(debug_assertions))]
        {
            #[cfg(feature = "alloc")]
            {
                assert_eq!(40, core::mem::size_of::<Zoned>());
            }
            #[cfg(all(target_pointer_width = "64", not(feature = "alloc")))]
            {
                // This asserts the same value as the alloc value above, but
                // it wasn't always this way, which is why it's written out
                // separately. Moreover, in theory, I'd be open to regressing
                // this value if it led to an improvement in alloc-mode. But
                // more likely, it would be nice to decrease this size in
                // non-alloc modes.
                assert_eq!(40, core::mem::size_of::<Zoned>());
            }
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
    fn zoned_deserialize_yaml() {
        if crate::tz::db().is_definitively_empty() {
            return;
        }

        let expected = datetime(2024, 10, 31, 16, 33, 53, 123456789)
            .in_tz("UTC")
            .unwrap();

        let deserialized: Zoned =
            serde_yaml::from_str("2024-10-31T16:33:53.123456789+00:00[UTC]")
                .unwrap();

        assert_eq!(deserialized, expected);

        let deserialized: Zoned = serde_yaml::from_slice(
            "2024-10-31T16:33:53.123456789+00:00[UTC]".as_bytes(),
        )
        .unwrap();

        assert_eq!(deserialized, expected);

        let cursor = Cursor::new(b"2024-10-31T16:33:53.123456789+00:00[UTC]");
        let deserialized: Zoned = serde_yaml::from_reader(cursor).unwrap();

        assert_eq!(deserialized, expected);
    }

    /// This is a regression test for a case where changing a zoned datetime
    /// to have a time of midnight ends up producing a counter-intuitive
    /// result.
    ///
    /// See: <https://github.com/BurntSushi/jiff/issues/211>
    #[test]
    fn zoned_with_time_dst_after_gap() {
        if crate::tz::db().is_definitively_empty() {
            return;
        }

        let zdt1: Zoned = "2024-03-31T12:00[Atlantic/Azores]".parse().unwrap();
        assert_eq!(
            zdt1.to_string(),
            "2024-03-31T12:00:00+00:00[Atlantic/Azores]"
        );

        let zdt2 = zdt1.with().time(Time::midnight()).build().unwrap();
        assert_eq!(
            zdt2.to_string(),
            "2024-03-31T01:00:00+00:00[Atlantic/Azores]"
        );
    }

    /// Similar to `zoned_with_time_dst_after_gap`, but tests what happens
    /// when moving from/to both sides of the gap.
    ///
    /// See: <https://github.com/BurntSushi/jiff/issues/211>
    #[test]
    fn zoned_with_time_dst_us_eastern() {
        if crate::tz::db().is_definitively_empty() {
            return;
        }

        let zdt1: Zoned = "2024-03-10T01:30[US/Eastern]".parse().unwrap();
        assert_eq!(zdt1.to_string(), "2024-03-10T01:30:00-05:00[US/Eastern]");
        let zdt2 = zdt1.with().hour(2).build().unwrap();
        assert_eq!(zdt2.to_string(), "2024-03-10T03:30:00-04:00[US/Eastern]");

        let zdt1: Zoned = "2024-03-10T03:30[US/Eastern]".parse().unwrap();
        assert_eq!(zdt1.to_string(), "2024-03-10T03:30:00-04:00[US/Eastern]");
        let zdt2 = zdt1.with().hour(2).build().unwrap();
        assert_eq!(zdt2.to_string(), "2024-03-10T03:30:00-04:00[US/Eastern]");

        // I originally thought that this was difference from Temporal. Namely,
        // I thought that Temporal ignored the disambiguation setting (and the
        // bad offset). But it doesn't. I was holding it wrong.
        //
        // See: https://github.com/tc39/proposal-temporal/issues/3078
        let zdt1: Zoned = "2024-03-10T01:30[US/Eastern]".parse().unwrap();
        assert_eq!(zdt1.to_string(), "2024-03-10T01:30:00-05:00[US/Eastern]");
        let zdt2 = zdt1
            .with()
            .offset(tz::offset(10))
            .hour(2)
            .disambiguation(Disambiguation::Earlier)
            .build()
            .unwrap();
        assert_eq!(zdt2.to_string(), "2024-03-10T01:30:00-05:00[US/Eastern]");

        // This should also respect the disambiguation setting even without
        // explicitly specifying an invalid offset. This is becaue `02:30-05`
        // is regarded as invalid since `02:30` isn't a valid civil time on
        // this date in this time zone.
        let zdt1: Zoned = "2024-03-10T01:30[US/Eastern]".parse().unwrap();
        assert_eq!(zdt1.to_string(), "2024-03-10T01:30:00-05:00[US/Eastern]");
        let zdt2 = zdt1
            .with()
            .hour(2)
            .disambiguation(Disambiguation::Earlier)
            .build()
            .unwrap();
        assert_eq!(zdt2.to_string(), "2024-03-10T01:30:00-05:00[US/Eastern]");
    }

    #[test]
    fn zoned_precision_loss() {
        if crate::tz::db().is_definitively_empty() {
            return;
        }

        let zdt1: Zoned = "2025-01-25T19:32:21.783444592+01:00[Europe/Paris]"
            .parse()
            .unwrap();
        let span = 1.second();
        let zdt2 = &zdt1 + span;
        assert_eq!(
            zdt2.to_string(),
            "2025-01-25T19:32:22.783444592+01:00[Europe/Paris]"
        );
        assert_eq!(zdt1, &zdt2 - span, "should be reversible");
    }

    // See: https://github.com/BurntSushi/jiff/issues/290
    #[test]
    fn zoned_roundtrip_regression() {
        if crate::tz::db().is_definitively_empty() {
            return;
        }

        let zdt: Zoned =
            "2063-03-31T10:00:00+11:00[Australia/Sydney]".parse().unwrap();
        assert_eq!(zdt.offset(), super::Offset::constant(11));
        let roundtrip = zdt.time_zone().to_zoned(zdt.datetime()).unwrap();
        assert_eq!(zdt, roundtrip);
    }

    // See: https://github.com/BurntSushi/jiff/issues/305
    #[test]
    fn zoned_round_dst_day_length() {
        if crate::tz::db().is_definitively_empty() {
            return;
        }

        let zdt1: Zoned =
            "2025-03-09T12:15[America/New_York]".parse().unwrap();
        let zdt2 = zdt1.round(Unit::Day).unwrap();
        // Since this day is only 23 hours long, it should round down instead
        // of up (as it would on a normal 24 hour day). Interestingly, the bug
        // was causing this to not only round up, but to a datetime that wasn't
        // the start of a day. Specifically, 2025-03-10T01:00:00-04:00.
        assert_eq!(
            zdt2.to_string(),
            "2025-03-09T00:00:00-05:00[America/New_York]"
        );
    }

    #[test]
    fn zoned_round_errors() {
        if crate::tz::db().is_definitively_empty() {
            return;
        }

        let zdt: Zoned = "2025-03-09T12:15[America/New_York]".parse().unwrap();

        insta::assert_snapshot!(
            zdt.round(Unit::Year).unwrap_err(),
            @"datetime rounding does not support years"
        );
        insta::assert_snapshot!(
            zdt.round(Unit::Month).unwrap_err(),
            @"datetime rounding does not support months"
        );
        insta::assert_snapshot!(
            zdt.round(Unit::Week).unwrap_err(),
            @"datetime rounding does not support weeks"
        );

        let options = ZonedRound::new().smallest(Unit::Day).increment(2);
        insta::assert_snapshot!(
            zdt.round(options).unwrap_err(),
            @"increment 2 for rounding datetime to days must be 1) less than 2, 2) divide into it evenly and 3) greater than zero"
        );
    }

    // This tests that if we get a time zone offset with an explicit second
    // component, then it must *exactly* match the correct offset for that
    // civil time.
    //
    // See: https://github.com/tc39/proposal-temporal/issues/3099
    // See: https://github.com/tc39/proposal-temporal/pull/3107
    #[test]
    fn time_zone_offset_seconds_exact_match() {
        if crate::tz::db().is_definitively_empty() {
            return;
        }

        let zdt: Zoned =
            "1970-06-01T00:00:00-00:45[Africa/Monrovia]".parse().unwrap();
        assert_eq!(
            zdt.to_string(),
            "1970-06-01T00:00:00-00:45[Africa/Monrovia]"
        );

        let zdt: Zoned =
            "1970-06-01T00:00:00-00:44:30[Africa/Monrovia]".parse().unwrap();
        assert_eq!(
            zdt.to_string(),
            "1970-06-01T00:00:00-00:45[Africa/Monrovia]"
        );

        insta::assert_snapshot!(
            "1970-06-01T00:00:00-00:44:40[Africa/Monrovia]".parse::<Zoned>().unwrap_err(),
            @r#"parsing "1970-06-01T00:00:00-00:44:40[Africa/Monrovia]" failed: datetime 1970-06-01T00:00:00 could not resolve to a timestamp since 'reject' conflict resolution was chosen, and because datetime has offset -00:44:40, but the time zone Africa/Monrovia for the given datetime unambiguously has offset -00:44:30"#,
        );

        insta::assert_snapshot!(
            "1970-06-01T00:00:00-00:45:00[Africa/Monrovia]".parse::<Zoned>().unwrap_err(),
            @r#"parsing "1970-06-01T00:00:00-00:45:00[Africa/Monrovia]" failed: datetime 1970-06-01T00:00:00 could not resolve to a timestamp since 'reject' conflict resolution was chosen, and because datetime has offset -00:45, but the time zone Africa/Monrovia for the given datetime unambiguously has offset -00:44:30"#,
        );
    }

    // These are some interesting tests because the time zones have transitions
    // that are very close to one another (within 14 days!). I picked these up
    // from a bug report to Temporal. Their reference implementation uses
    // different logic to examine time zone transitions than Jiff. In contrast,
    // Jiff uses the IANA time zone database directly. So it was unaffected.
    //
    // [1]: https://github.com/tc39/proposal-temporal/issues/3110
    #[test]
    fn weird_time_zone_transitions() {
        if crate::tz::db().is_definitively_empty() {
            return;
        }

        let zdt: Zoned =
            "2000-10-08T01:00:00-01:00[America/Noronha]".parse().unwrap();
        let sod = zdt.start_of_day().unwrap();
        assert_eq!(
            sod.to_string(),
            "2000-10-08T01:00:00-01:00[America/Noronha]"
        );

        let zdt: Zoned =
            "2000-10-08T03:00:00-03:00[America/Boa_Vista]".parse().unwrap();
        let sod = zdt.start_of_day().unwrap();
        assert_eq!(
            sod.to_string(),
            "2000-10-08T01:00:00-03:00[America/Boa_Vista]",
        );
    }
}
