use crate::util::rangeint::{ri128, ri16, ri32, ri64, ri8, RInto};

/// A type alias for the sign of a number.
///
/// It can be -1 for a negative sign, 1 for a positive sign or 0 for no sign.
pub(crate) type Sign = ri8<-1, 1>;

/// A type alias for a ranged integer with no units.
///
/// In particular, the range of this type is just the range of an `i64`. This
/// is useful when too many things with different units need to be combined at
/// once, and it's just too painful to keep them straight. In cases like that,
/// it's useful to just convert everything to `NoUnits`, do the necessary math,
/// and then convert back to the appropriate ranged types.
///
/// Note that we don't actually lose much by doing this, since the computed
/// min/max values are retained even when converting *to and from* this type.
/// In general, this type is just about making some math easier by making
/// everything uniform.
pub(crate) type NoUnits = ri64<{ i64::MIN as i128 }, { i64::MAX as i128 }>;

/// A type alias for a ranged 96-bit integer with no units.
///
/// This is like `NoUnits`, but useful in contexts where one wants to limit
/// values to what can be represented to 96 bits.
pub(crate) type NoUnits96 = ri128<{ -(1 << 95) }, { (1 << 95) - 1 }>;

/// A type alias for a ranged 128-bit integer with no units.
///
/// This is like `NoUnits`, but useful in contexts where one wants to limit
/// values to what can be represented by an `i128`.
pub(crate) type NoUnits128 = ri128<{ i128::MIN }, { i128::MAX }>;

/// A type alias for a ranged 32-bit integer with no units.
///
/// This is like `NoUnits`, but useful in contexts where one wants to limit
/// values to what can be represented by an `i32`.
pub(crate) type NoUnits32 = ri32<{ i32::MIN as i128 }, { i32::MAX as i128 }>;

/// A type alias for a ranged 16-bit integer with no units.
///
/// This is like `NoUnits`, but useful in contexts where one wants to limit
/// values to what can be represented by an `i16`.
pub(crate) type NoUnits16 = ri16<{ i16::MIN as i128 }, { i16::MAX as i128 }>;

/*
/// A type alias for a ranged 8-bit integer with no units.
///
/// This is like `NoUnits`, but useful in contexts where one wants to limit
/// values to what can be represented by an `i8`.
pub(crate) type NoUnits8 = ri8<{ i8::MIN as i128 }, { i8::MAX as i128 }>;
*/

/// The range of years supported by jiff.
///
/// This is ultimately where some of the other ranges (like `UnixSeconds`)
/// were determined from. That is, the range of years is the primary point at
/// which the space of supported time instants is derived from. If one wanted
/// to expand this range, you'd need to change it here and then compute the
/// corresponding min/max values for `UnixSeconds`.
pub(crate) type Year = ri16<-9999, 9999>;

/// The range of CE years supported by jiff.
pub(crate) type YearCE = ri16<1, { Year::MAX }>;

/// The range of BCE years supported by jiff.
pub(crate) type YearBCE = ri16<1, { Year::MAX + 1 }>;

/// The range of Unix seconds supported by Jiff.
///
/// This range should correspond to the first second of `Year::MIN` up through
/// (and including) the last second of `Year::MAX`. Actually computing that is
/// non-trivial, however, it can be computed easily enough using Unix programs
/// like `date`:
///
/// ```text
/// $ TZ=0 date -d 'Mon Jan  1 12:00:00 AM  -9999' +'%s'
/// date: invalid date ‘Mon Jan  1 12:00:00 AM  -9999’
/// $ TZ=0 date -d 'Fri Dec 31 23:59:59  9999' +'%s'
/// 253402300799
/// ```
///
/// Well, almost easily enough. `date` apparently doesn't support negative
/// years. But it does support negative timestamps:
///
/// ```text
/// $ TZ=0 date -d '@-377705116800'
/// Mon Jan  1 12:00:00 AM  -9999
/// $ TZ=0 date -d '@253402300799'
/// Fri Dec 31 11:59:59 PM  9999
/// ```
///
/// With that said, we actually end up restricting the range a bit more than
/// what's above. Namely, what's above is what we support for civil datetimes.
/// Because of time zones, we need to choose whether all `Timestamp` values
/// can be infallibly converted to `civil::DateTime` values, or whether all
/// `civil::DateTime` values can be infallibly converted to `Timestamp` values.
/// I chose the former because getting a civil datetime is important for
/// formatting. If I didn't choose the former, there would be some instants
/// that could not be formatted. Thus, we make room by shrinking the range of
/// allowed instants by precisely the maximum supported time zone offset.
pub(crate) type UnixSeconds = ri64<
    { -377705116800 - SpanZoneOffset::MIN },
    { 253402300799 - SpanZoneOffset::MAX },
>;

/// Like UnixSeconds, but expressed in units of milliseconds.
pub(crate) type UnixMilliseconds = ri64<
    { UnixSeconds::MIN * MILLIS_PER_SECOND.bound() },
    {
        (UnixSeconds::MAX * MILLIS_PER_SECOND.bound())
            + (FractionalNanosecond::MAX / NANOS_PER_MILLI.bound())
    },
>;

/// Like UnixSeconds, but expressed in units of microseconds.
pub(crate) type UnixMicroseconds = ri64<
    { UnixSeconds::MIN * MICROS_PER_SECOND.bound() },
    {
        (UnixSeconds::MAX * MICROS_PER_SECOND.bound())
            + (FractionalNanosecond::MAX / NANOS_PER_MICRO.bound())
    },
>;

/// Like UnixSeconds, but expressed in units of nanoseconds.
pub(crate) type UnixNanoseconds = ri128<
    { UnixSeconds::MIN * NANOS_PER_SECOND.bound() },
    {
        UnixSeconds::MAX * NANOS_PER_SECOND.bound() + FractionalNanosecond::MAX
    },
>;

/// The range of possible month values.
pub(crate) type Month = ri8<1, 12>;

/// The range of a weekday, offset from zero.
pub(crate) type WeekdayZero = ri8<0, 6>;

/// The range of a weekday, offset from one.
pub(crate) type WeekdayOne = ri8<1, 7>;

/// The range of possible day values.
///
/// Obviously this range is not valid for every month. Therefore, code working
/// with days needs to be careful to check that it is valid for whatever month
/// is being used.
pub(crate) type Day = ri8<1, 31>;

pub(crate) type DayOfYear = ri16<1, 366>;

pub(crate) type ISOYear = ri16<-9999, 9999>;

pub(crate) type ISOWeek = ri8<1, 53>;

pub(crate) type WeekNum = ri8<0, 53>;

/// The range of possible hour values.
pub(crate) type Hour = ri8<0, 23>;

/// The range of possible minute values.
pub(crate) type Minute = ri8<0, 59>;

/// The range of possible second values not accounting for leap seconds.
pub(crate) type Second = ri8<0, 59>;

/// The range of possible millisecond values.
pub(crate) type Millisecond = ri16<0, 999>;

/// The range of possible microsecond values.
pub(crate) type Microsecond = ri16<0, 999>;

/// The range of possible nanosecond values.
pub(crate) type Nanosecond = ri16<0, 999>;

/// The range of possible nanosecond values.
pub(crate) type SubsecNanosecond = ri32<0, { NANOS_PER_SECOND.bound() - 1 }>;

/// A range representing each possible second in a single civil day.
pub(crate) type CivilDaySecond =
    ri32<0, { SECONDS_PER_CIVIL_DAY.bound() - 1 }>;

/// A range representing each possible nanosecond in a single civil day.
pub(crate) type CivilDayNanosecond =
    ri64<0, { NANOS_PER_CIVIL_DAY.bound() - 1 }>;

/// The number of seconds permitted in a single day.
///
/// This is mostly just a "sensible" cap on what is possible. We allow one day
/// to span up to 7 civil days.
///
/// It must also be at least 1 second long.
pub(crate) type ZonedDaySeconds =
    ri64<1, { 7 * SECONDS_PER_CIVIL_DAY.bound() }>;

/// The number of nanoseconds permitted in a single day.
///
/// This is mostly just a "sensible" cap on what is possible. We allow one day
/// to span up to 7 civil days.
///
/// It must also be at least 1 second long.
pub(crate) type ZonedDayNanoseconds = ri64<
    { ZonedDaySeconds::MIN * NANOS_PER_SECOND.bound() },
    { ZonedDaySeconds::MAX * NANOS_PER_SECOND.bound() },
>;

/// The number of days from the Unix epoch for the Gregorian calendar.
///
/// The range supported is based on the range of Unix timestamps that we
/// support.
///
/// While I had originally used the "rate die" concept from Calendrical
/// Calculations, I found [Howard Hinnant's formulation][date-algorithms]
/// much more straight-forward. And while I didn't benchmark them, it also
/// appears faster.
///
/// [date-algorithms]: http://howardhinnant.github.io/date_algorithms.html
pub(crate) type UnixEpochDay = ri32<
    {
        (UnixSeconds::MIN + SpanZoneOffset::MIN)
            .div_euclid(SECONDS_PER_CIVIL_DAY.bound())
    },
    {
        (UnixSeconds::MAX + SpanZoneOffset::MAX)
            .div_euclid(SECONDS_PER_CIVIL_DAY.bound())
    },
>;

/// A precise min/max of the allowed range of a duration in years.
pub(crate) type SpanYears = ri16<{ -(Year::LEN - 1) }, { Year::LEN - 1 }>;

/// A precise min/max of the allowed range of a duration in months.
pub(crate) type SpanMonths = ri32<
    { SpanYears::MIN * MONTHS_PER_YEAR.bound() },
    { SpanYears::MAX * MONTHS_PER_YEAR.bound() },
>;

/// A range of the allowed number of weeks.
///
/// This is an upper bound and not actually a precise maximum. I believe a
/// precise max could be fractional and not an integer.
pub(crate) type SpanWeeks = ri32<{ SpanDays::MIN / 7 }, { SpanDays::MAX / 7 }>;

/// A range of the allowed number of days.
pub(crate) type SpanDays =
    ri32<{ SpanHours::MIN / 24 }, { SpanHours::MAX / 24 }>;

/// A range of the allowed number of hours.
///
/// Like days, this is an upper bound because some days (because of DST) have
/// 25 hours.
pub(crate) type SpanHours =
    ri32<{ SpanMinutes::MIN / 60 }, { SpanMinutes::MAX / 60 }>;

/// A range of the allowed number of minutes.
pub(crate) type SpanMinutes =
    ri64<{ SpanSeconds::MIN / 60 }, { SpanSeconds::MAX / 60 }>;

/// The maximum number of seconds that can be expressed with a span.
///
/// All of our span types (except for years and months, since they have
/// variable length even in civil datetimes) are defined in terms of this
/// constant. The way it's defined is a little odd, so let's break it down.
///
/// Firstly, a span of seconds should be able to represent at least
/// the complete span supported by `Timestamp`. Thus, it's based off of
/// `UnixSeconds::LEN`. That is, a span should be able to represent the value
/// `UnixSeconds::MAX - UnixSeconds::MIN`.
///
/// Secondly, a span should also be able to account for any amount of possible
/// time that a time zone offset might add or subtract to an `Timestamp`. This
/// also means it can account for any difference between two `civil::DateTime`
/// values.
///
/// Thirdly, we would like our span to be divisible by `SECONDS_PER_CIVIL_DAY`.
/// This isn't strictly required, but it makes defining boundaries a little
/// smoother. If it weren't divisible, then the lower bounds on some types
/// would need to be adjusted by one.
///
/// Note that neither the existence of this constant nor defining our spans
/// based on it impacts the correctness of doing arithmetic on zoned instants.
/// Artihemetic on zoned instants still uses "civil" spans, but the length
/// of time for some units (like a day) might vary. The arithmetic for zoned
/// instants accounts for this explicitly. But it still must obey the limits
/// set here.
const SPAN_CIVIL_SECONDS: i128 = next_multiple_of(
    UnixSeconds::LEN + SpanZoneOffset::MAX + SECONDS_PER_CIVIL_DAY.bound(),
    SECONDS_PER_CIVIL_DAY.bound(),
);

/// A range of the allowed number of seconds.
pub(crate) type SpanSeconds =
    ri64<{ -SPAN_CIVIL_SECONDS }, SPAN_CIVIL_SECONDS>;

/// A range of the allowed number of milliseconds.
pub(crate) type SpanMilliseconds =
    ri64<{ SpanSeconds::MIN * 1_000 }, { SpanSeconds::MAX * 1_000 }>;

/// A range of the allowed number of microseconds.
pub(crate) type SpanMicroseconds =
    ri64<{ SpanMilliseconds::MIN * 1_000 }, { SpanMilliseconds::MAX * 1_000 }>;

/// A range of the allowed number of nanoseconds.
///
/// For this, we cannot cover the full span of supported time instants since
/// `UnixSeconds::MAX * NANOSECONDS_PER_SECOND` cannot fit into 64-bits. We
/// could use a `i128`, but it doesn't seem worth it.
///
/// Also note that our min is equal to -max, so that the total number of values
/// in this range is one less than the number of distinct `i64` values. We do
/// that so that the absolute value is always defined.
pub(crate) type SpanNanoseconds =
    ri64<{ (i64::MIN + 1) as i128 }, { i64::MAX as i128 }>;

/// The range of allowable fractional milliseconds.
///
/// That is, this corresponds to the range of milliseconds allowable within a
/// single second. It can be either positive or negative.
pub(crate) type FractionalMillisecond = ri32<
    { -(MILLIS_PER_SECOND.bound() - 1) },
    { MILLIS_PER_SECOND.bound() - 1 },
>;

/// The range of allowable fractional microseconds.
///
/// That is, this corresponds to the range of microseconds allowable within a
/// single second. It can be either positive or negative.
pub(crate) type FractionalMicrosecond = ri32<
    { -(MICROS_PER_SECOND.bound() - 1) },
    { MICROS_PER_SECOND.bound() - 1 },
>;

/// The range of allowable fractional nanoseconds.
///
/// That is, this corresponds to the range of nanoseconds allowable within a
/// single second. It can be either positive or negative.
pub(crate) type FractionalNanosecond = ri32<
    { -(NANOS_PER_SECOND.bound() - 1) },
    { NANOS_PER_SECOND.bound() - 1 },
>;

/// The range of allowable seconds and lower in a span, in units of seconds.
///
/// This corresponds to when the min/max of seconds, milliseconds, microseconds
/// and nanoseconds are added together in a span. This is useful for describing
/// the limit on the total number of possible seconds when all of these units
/// are combined. This is necessary as part of printing/parsing spans because
/// the ISO 8601 duration format doesn't support individual millisecond,
/// microsecond and nanosecond components. So they all need to be smushed into
/// seconds and a possible fractional part.
pub(crate) type SpanSecondsOrLower = ri64<
    {
        SpanSeconds::MIN
            + (SpanMilliseconds::MIN / MILLIS_PER_SECOND.bound())
            + (SpanMicroseconds::MIN / MICROS_PER_SECOND.bound())
            + (SpanNanoseconds::MIN / NANOS_PER_SECOND.bound())
    },
    {
        SpanSeconds::MAX
            + (SpanMilliseconds::MAX / MILLIS_PER_SECOND.bound())
            + (SpanMicroseconds::MAX / MICROS_PER_SECOND.bound())
            + (SpanNanoseconds::MAX / NANOS_PER_SECOND.bound())
    },
>;

/// The range of allowable seconds and lower in a span, in units of
/// nanoseconds.
///
/// See `SpanSecondsOrLower`. This exists for the same reason. Namely, when
/// serializing a `Span` to an ISO 8601 duration string, we need to combine
/// seconds and lower into a single fractional seconds value.
pub(crate) type SpanSecondsOrLowerNanoseconds = ri128<
    {
        (SpanSeconds::MIN * NANOS_PER_SECOND.bound())
            + (SpanMilliseconds::MIN * NANOS_PER_MILLI.bound())
            + (SpanMicroseconds::MIN * NANOS_PER_MICRO.bound())
            + SpanNanoseconds::MIN
    },
    {
        (SpanSeconds::MAX * NANOS_PER_SECOND.bound())
            + (SpanMilliseconds::MAX * NANOS_PER_MILLI.bound())
            + (SpanMicroseconds::MAX * NANOS_PER_MICRO.bound())
            + SpanNanoseconds::MAX
    },
>;

/// The span of seconds permitted for expressing the offset of a time zone.
pub(crate) type SpanZoneOffset =
    ri32<{ -SPAN_ZONE_OFFSET_TOTAL_SECONDS }, SPAN_ZONE_OFFSET_TOTAL_SECONDS>;

/// The max number of seconds that can be expressed in a time zone offset.
///
/// This is computed here based on the span offset types below for convenience
/// use in the `SpanZoneOffset` definition above.
const SPAN_ZONE_OFFSET_TOTAL_SECONDS: i128 =
    (SpanZoneOffsetHours::MAX * 60 * 60)
        + (SpanZoneOffsetMinutes::MAX * 60)
        + SpanZoneOffsetSeconds::MAX;

/// The number of hours allowed in a time zone offset.
///
/// This number was somewhat arbitrarily chosen. In part because it's
/// bigger than any current offset by a wide margin, and in part because
/// POSIX `TZ` strings require the ability to store offsets in the range
/// `-24:59:59..=25:59:59`. Note though that we make the range a little bigger
/// with `-25:59:59..=25:59:59` so that negating an offset always produces a
/// valid offset.
///
/// Note that RFC 8536 actually allows offsets to be much bigger, namely, in
/// the range `(-2^31, 2^31)`, where both ends are _exclusive_ (`-2^31` is
/// explicitly disallowed, and `2^31` overflows a signed 32-bit integer). But
/// RFC 8536 does say that it *should* be in the range `[-89999, 93599]`, which
/// matches POSIX. In order to keep our offset small, we stick roughly to what
/// POSIX requires.
pub(crate) type SpanZoneOffsetHours = ri8<-25, 25>;

/// The number of minutes allowed in a time zone offset.
pub(crate) type SpanZoneOffsetMinutes = ri8<-59, 59>;

/// The number of seconds allowed in a time zone offset.
pub(crate) type SpanZoneOffsetSeconds = ri8<-59, 59>;

/// The number of months in a year.
pub(crate) const MONTHS_PER_YEAR: Constant = Constant(12);

/// The number of days in a week.
pub(crate) const DAYS_PER_CIVIL_WEEK: Constant = Constant(7);

/// The number of whole hours in one day.
pub(crate) const HOURS_PER_CIVIL_DAY: Constant = Constant(24);

/// The number of minutes in a civil day.
pub(crate) const MINUTES_PER_CIVIL_DAY: Constant =
    Constant(HOURS_PER_CIVIL_DAY.value() * MINUTES_PER_HOUR.value());

/// The number of minutes in an hour.
pub(crate) const MINUTES_PER_HOUR: Constant = Constant(60);

/// The number of seconds in a civil week.
///
/// Some weeks will have more or less seconds because of DST transitions. But
/// such things are ignored when dealing with civil time, and so this constant
/// is still useful.
pub(crate) const SECONDS_PER_CIVIL_WEEK: Constant = Constant(
    DAYS_PER_CIVIL_WEEK.value()
        * HOURS_PER_CIVIL_DAY.value()
        * SECONDS_PER_HOUR.value(),
);

/// The number of seconds in a civil day.
///
/// Some days will have more or less seconds because of DST transitions. But
/// such things are ignored when dealing with civil time, and so this constant
/// is still useful.
pub(crate) const SECONDS_PER_CIVIL_DAY: Constant =
    Constant(HOURS_PER_CIVIL_DAY.value() * SECONDS_PER_HOUR.value());

/// The number of seconds in a single hour.
pub(crate) const SECONDS_PER_HOUR: Constant =
    Constant(SECONDS_PER_MINUTE.value() * 60);

/// The number of seconds in a single minute.
pub(crate) const SECONDS_PER_MINUTE: Constant = Constant(60);

/// The number of microseconds in a civil day.
pub(crate) const MILLIS_PER_CIVIL_DAY: Constant =
    Constant(SECONDS_PER_CIVIL_DAY.value() * MILLIS_PER_SECOND.value());

/// The number of milliseconds in a single second.
pub(crate) const MILLIS_PER_SECOND: Constant = Constant(1_000);

/// The number of microseconds in a civil day.
pub(crate) const MICROS_PER_CIVIL_DAY: Constant =
    Constant(SECONDS_PER_CIVIL_DAY.value() * MICROS_PER_SECOND.value());

/// The number of microseconds in a single second.
pub(crate) const MICROS_PER_SECOND: Constant = Constant(1_000_000);

/// The number of microseconds in a single millisecond.
pub(crate) const MICROS_PER_MILLI: Constant = Constant(1_000);

/// The number of nanoseconds in a civil week.
///
/// Some weeks will have more or less seconds because of DST transitions. But
/// such things are ignored when dealing with civil time, and so this constant
/// is still useful.
pub(crate) const NANOS_PER_CIVIL_WEEK: Constant =
    Constant(SECONDS_PER_CIVIL_WEEK.value() * NANOS_PER_SECOND.value());

/// The number of nanoseconds in a civil day.
///
/// Some days will have more or less seconds because of DST transitions. But
/// such things are ignored when dealing with civil time, and so this constant
/// is still useful.
pub(crate) const NANOS_PER_CIVIL_DAY: Constant =
    Constant(SECONDS_PER_CIVIL_DAY.value() * NANOS_PER_SECOND.value());

/// The number of nanoseconds in a single hour.
pub(crate) const NANOS_PER_HOUR: Constant =
    Constant(SECONDS_PER_HOUR.value() * NANOS_PER_SECOND.value());

/// The number of nanoseconds in a single minute.
pub(crate) const NANOS_PER_MINUTE: Constant =
    Constant(SECONDS_PER_MINUTE.value() * NANOS_PER_SECOND.value());

/// The number of nanoseconds in a single second.
pub(crate) const NANOS_PER_SECOND: Constant = Constant(1_000_000_000);

/// The number of nanoseconds in a single millisecond.
pub(crate) const NANOS_PER_MILLI: Constant = Constant(1_000_000);

/// The number of nanoseconds in a single microsecond.
pub(crate) const NANOS_PER_MICRO: Constant = Constant(1_000);

pub(crate) fn sign<T: Ord>(t1: T, t2: T) -> Sign {
    use core::cmp::Ordering::*;
    match t1.cmp(&t2) {
        Less => Sign::N::<-1>(),
        Equal => Sign::N::<0>(),
        Greater => Sign::N::<1>(),
    }
}

/// A constant value for use in arithmetic in this crate.
///
/// This type is basically a bunch of shenanigans to make constants work in
/// a sensible way with our range integers. Essentially, we really want
/// constants to satisfy the following criteria:
///
/// 1. Defined in one place.
/// 2. Composable in that we can define constants in terms of other constants.
/// 3. Easy to use with any kind of range integer type.
/// 4. Specially constructed when used with ranged integers. That is, a ranged
/// integer value build from a constant should have computed min/max bounds
/// equivalent to the constant itself. (Normally, a `rN::new` will set the
/// computed min/max bounds to the MIN/MAX bounds overall, since it is assumed
/// that `rN::new` accepts a value that can vary to any legal value in the
/// range. But a constant needs tight bounds because, well, it can literally
/// never vary.)
///
/// # Trait implementations
///
/// It'd be nice to impl `Add/Sub/Mul/Div` for `Constant` itself, but they
/// can't be used in a const context... which is where it would be most useful.
/// Otherwise, we just define `Add/Sub/Mul/Div` impls for all of the ranged
/// integer types so that constants can be used on the left-hand side of
/// arithmetic expressions. (The ranged integer types have impls that are
/// generic enough to support arithmetic with constants on the right hand
/// side.)
///
/// We do a similar thing for the `Partial{Eq,Ord}` traits. The ranged integers
/// already have impls for `Constant` on the right-hand side. Below are the
/// impls for `Constant` on the left-hand side.
///
/// All of the trait impls that deal with constants and ranged integers are
/// implemented with the ranged integer types.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord)]
pub(crate) struct Constant(pub(crate) i64);

/// A short-hand creating a generic `Constant` value as a ranged integer.
///
/// Callers do need to ensure that the `MIN` and `MAX` bounds are specified (or
/// more likely inferred), but otherwise, the `ri64` returned will be usable
/// in most contexts even with other ranged integers (like `ri8`).
#[allow(non_snake_case)]
pub(crate) fn C(
    constant: i64,
) -> ri64<{ i64::MIN as i128 }, { i64::MAX as i128 }> {
    Constant(constant).rinto()
}

#[allow(non_snake_case)]
pub(crate) fn C128(constant: i64) -> ri128<{ i128::MIN }, { i128::MAX }> {
    Constant(constant).rinto()
}

impl Constant {
    /// Return the primitive value of this constant.
    pub(crate) const fn value(self) -> i64 {
        self.0
    }

    /// Return this constant as a bound intended to be used in const generics.
    pub(crate) const fn bound(self) -> i128 {
        self.value() as i128
    }
}

impl core::ops::Neg for Constant {
    type Output = Constant;

    fn neg(self) -> Constant {
        Constant(-self.0)
    }
}

impl From<Constant> for i8 {
    fn from(c: Constant) -> i8 {
        #[cfg(not(debug_assertions))]
        {
            c.value() as i8
        }
        #[cfg(debug_assertions)]
        {
            i8::try_from(c.value()).unwrap_or_else(|_| {
                panic!("{c:?} is out of range {:?}..={:?}", i8::MIN, i8::MAX);
            })
        }
    }
}

impl From<Constant> for i16 {
    fn from(c: Constant) -> i16 {
        #[cfg(not(debug_assertions))]
        {
            c.value() as i16
        }
        #[cfg(debug_assertions)]
        {
            i16::try_from(c.value()).unwrap_or_else(|_| {
                panic!(
                    "{c:?} is out of range {:?}..={:?}",
                    i16::MIN,
                    i16::MAX
                );
            })
        }
    }
}

impl From<Constant> for i32 {
    fn from(c: Constant) -> i32 {
        #[cfg(not(debug_assertions))]
        {
            c.value() as i32
        }
        #[cfg(debug_assertions)]
        {
            i32::try_from(c.value()).unwrap_or_else(|_| {
                panic!(
                    "{c:?} is out of range {:?}..={:?}",
                    i32::MIN,
                    i32::MAX
                );
            })
        }
    }
}

impl From<Constant> for i64 {
    fn from(c: Constant) -> i64 {
        c.value()
    }
}

impl From<Constant> for i128 {
    fn from(c: Constant) -> i128 {
        i128::from(c.value())
    }
}

/// Computes the next multiple of `rhs` that is greater than or equal to `lhs`.
///
/// Taken from:
/// https://github.com/rust-lang/rust/blob/eff958c59e8c07ba0515e164b825c9001b242294/library/core/src/num/int_macros.rs
const fn next_multiple_of(lhs: i128, rhs: i128) -> i128 {
    // This would otherwise fail when calculating `r` when self == T::MIN.
    if rhs == -1 {
        return lhs;
    }

    let r = lhs % rhs;
    let m = if (r > 0 && rhs < 0) || (r < 0 && rhs > 0) { r + rhs } else { r };
    if m == 0 {
        lhs
    } else {
        lhs + (rhs - m)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn divisible() {
        // We requires that our span of seconds is divisible by an even number
        // of days. When it's not divisible, some of the boundary conditions
        // get a little trickier, but I do not believe it's necessary for
        // correctness. Without this assertion, some of the minimum values for
        // our range types above need to be one less. (I believe.)
        assert_eq!(0, SpanSeconds::MAX_REPR % 86_400);
    }
}
