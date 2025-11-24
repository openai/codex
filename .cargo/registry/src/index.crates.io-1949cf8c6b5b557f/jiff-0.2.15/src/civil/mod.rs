/*!
Facilities for dealing with inexact dates and times.

# Overview

The essential types in this module are:

* [`Date`] is a specific day in the Gregorian calendar.
* [`Time`] is a specific wall clock time.
* [`DateTime`] is a combination of a day and a time.

Moreover, the [`date`](date()) and [`time`](time()) free functions can be used
to conveniently create values of any of three types above:

```
use jiff::civil::{date, time};

assert_eq!(date(2024, 7, 31).to_string(), "2024-07-31");
assert_eq!(time(15, 20, 0, 123).to_string(), "15:20:00.000000123");
assert_eq!(
    date(2024, 7, 31).at(15, 20, 0, 123).to_string(),
    "2024-07-31T15:20:00.000000123",
);
assert_eq!(
    time(15, 20, 0, 123).on(2024, 7, 31).to_string(),
    "2024-07-31T15:20:00.000000123",
);
```

# What is "civil" time?

A civil datetime is a calendar date and a clock time. It also goes by the
names "naive," "local" or "plain." The most important thing to understand
about civil time is that it does not correspond to a precise instant in
time. This is in contrast to types like [`Timestamp`](crate::Timestamp) and
[`Zoned`](crate::Zoned), which _do_ correspond to a precise instant in time (to
nanosecond precision).

Because a civil datetime _never_ has a time zone associated with it, and
because some time zones have transitions that skip or repeat clock times, it
follows that not all civil datetimes precisely map to a single instant in time.
For example, `2024-03-10 02:30` never existed on a clock in `America/New_York`
because the 2 o'clock hour was skipped when the clocks were "moved forward"
for daylight saving time. Conversely, `2024-11-03 01:30` occurred twice in
`America/New_York` because the 1 o'clock hour was repeated when clocks were
"moved backward" for daylight saving time. (When time is skipped, it's called a
"gap." When time is repeated, it's called a "fold.")

In contrast, an instant in time (that is, `Timestamp` or `Zoned`) can _always_
be converted to a civil datetime. And, when a civil datetime is combined
with its time zone identifier _and_ its offset, the resulting machine readable
string is unambiguous 100% of the time:

```
use jiff::{civil::date, tz::TimeZone};

let tz = TimeZone::get("America/New_York")?;
let dt = date(2024, 11, 3).at(1, 30, 0, 0);
// It's ambiguous, so asking for an unambiguous instant presents an error!
assert!(tz.to_ambiguous_zoned(dt).unambiguous().is_err());
// Gives you the earlier time in a fold, i.e., before DST ends:
assert_eq!(
    tz.to_ambiguous_zoned(dt).earlier()?.to_string(),
    "2024-11-03T01:30:00-04:00[America/New_York]",
);
// Gives you the later time in a fold, i.e., after DST ends.
// Notice the offset change from the previous example!
assert_eq!(
    tz.to_ambiguous_zoned(dt).later()?.to_string(),
    "2024-11-03T01:30:00-05:00[America/New_York]",
);
// "Just give me something reasonable"
assert_eq!(
    tz.to_ambiguous_zoned(dt).compatible()?.to_string(),
    "2024-11-03T01:30:00-04:00[America/New_York]",
);

# Ok::<(), Box<dyn std::error::Error>>(())
```

# When should I use civil time?

Here is a likely non-exhaustive list of reasons why you might want to use
civil time:

* When you want or need to deal with calendar and clock units as an
intermediate step before and/or after associating it with a time zone. For
example, perhaps you need to parse strings like `2000-01-01T00:00:00` from a
CSV file that have no time zone or offset information, but the time zone is
implied through some out-of-band mechanism.
* When time zone is actually irrelevant. For example, a fitness tracking app
that reminds you to work-out at 6am local time, regardless of which time zone
you're in.
* When you need to perform arithmetic that deliberately ignores daylight
saving time.
* When interacting with legacy systems or systems that specifically do not
support time zones.
*/

pub use self::{
    date::{Date, DateArithmetic, DateDifference, DateSeries, DateWith},
    datetime::{
        DateTime, DateTimeArithmetic, DateTimeDifference, DateTimeRound,
        DateTimeSeries, DateTimeWith,
    },
    iso_week_date::ISOWeekDate,
    time::{
        Time, TimeArithmetic, TimeDifference, TimeRound, TimeSeries, TimeWith,
    },
    weekday::{Weekday, WeekdaysForward, WeekdaysReverse},
};

mod date;
mod datetime;
mod iso_week_date;
mod time;
mod weekday;

/// The era corresponding to a particular year.
///
/// The BCE era corresponds to years less than or equal to `0`, while the CE
/// era corresponds to years greater than `0`.
///
/// In particular, this crate allows years to be negative and also to be `0`,
/// which is contrary to the common practice of excluding the year `0` when
/// writing dates for the Gregorian calendar. Moreover, common practice eschews
/// negative years in favor of labeling a year with an era notation. That is,
/// the year `1 BCE` is year `0` in this crate. The year `2 BCE` is the year
/// `-1` in this crate.
///
/// To get the year in its era format, use [`Date::era_year`].
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Era {
    /// The "before common era" era.
    ///
    /// This corresponds to all years less than or equal to `0`.
    ///
    /// This is precisely equivalent to the "BC" or "before Christ" era.
    BCE,
    /// The "common era" era.
    ///
    /// This corresponds to all years greater than `0`.
    ///
    /// This is precisely equivalent to the "AD" or "anno Domini" or "in the
    /// year of the Lord" era.
    CE,
}

/// Creates a new `DateTime` value in a `const` context.
///
/// This is a convenience free function for [`DateTime::constant`]. It is
/// intended to provide a terse syntax for constructing `DateTime` values from
/// parameters that are known to be valid.
///
/// # Panics
///
/// This routine panics when [`DateTime::new`] would return an error. That
/// is, when the given components do not correspond to a valid datetime.
/// Namely, all of the following must be true:
///
/// * The year must be in the range `-9999..=9999`.
/// * The month must be in the range `1..=12`.
/// * The day must be at least `1` and must be at most the number of days
/// in the corresponding month. So for example, `2024-02-29` is valid but
/// `2023-02-29` is not.
/// * `0 <= hour <= 23`
/// * `0 <= minute <= 59`
/// * `0 <= second <= 59`
/// * `0 <= subsec_nanosecond <= 999,999,999`
///
/// Similarly, when used in a const context, invalid parameters will prevent
/// your Rust program from compiling.
///
/// # Example
///
/// ```
/// use jiff::civil::DateTime;
///
/// let d = DateTime::constant(2024, 2, 29, 21, 30, 5, 123_456_789);
/// assert_eq!(d.date().year(), 2024);
/// assert_eq!(d.date().month(), 2);
/// assert_eq!(d.date().day(), 29);
/// assert_eq!(d.time().hour(), 21);
/// assert_eq!(d.time().minute(), 30);
/// assert_eq!(d.time().second(), 5);
/// assert_eq!(d.time().millisecond(), 123);
/// assert_eq!(d.time().microsecond(), 456);
/// assert_eq!(d.time().nanosecond(), 789);
/// ```
#[inline]
pub const fn datetime(
    year: i16,
    month: i8,
    day: i8,
    hour: i8,
    minute: i8,
    second: i8,
    subsec_nanosecond: i32,
) -> DateTime {
    DateTime::constant(
        year,
        month,
        day,
        hour,
        minute,
        second,
        subsec_nanosecond,
    )
}

/// Creates a new `Date` value in a `const` context.
///
/// This is a convenience free function for [`Date::constant`]. It is intended
/// to provide a terse syntax for constructing `Date` values from parameters
/// that are known to be valid.
///
/// # Panics
///
/// This routine panics when [`Date::new`] would return an error. That is,
/// when the given year-month-day does not correspond to a valid date.
/// Namely, all of the following must be true:
///
/// * The year must be in the range `-9999..=9999`.
/// * The month must be in the range `1..=12`.
/// * The day must be at least `1` and must be at most the number of days
/// in the corresponding month. So for example, `2024-02-29` is valid but
/// `2023-02-29` is not.
///
/// Similarly, when used in a const context, invalid parameters will prevent
/// your Rust program from compiling.
///
/// # Example
///
/// ```
/// use jiff::civil::date;
///
/// let d = date(2024, 2, 29);
/// assert_eq!(d.year(), 2024);
/// assert_eq!(d.month(), 2);
/// assert_eq!(d.day(), 29);
/// ```
#[inline]
pub const fn date(year: i16, month: i8, day: i8) -> Date {
    Date::constant(year, month, day)
}

/// Creates a new `Time` value in a `const` context.
///
/// This is a convenience free function for [`Time::constant`]. It is intended
/// to provide a terse syntax for constructing `Time` values from parameters
/// that are known to be valid.
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
pub const fn time(
    hour: i8,
    minute: i8,
    second: i8,
    subsec_nanosecond: i32,
) -> Time {
    Time::constant(hour, minute, second, subsec_nanosecond)
}
