// This is a part of Chrono.
// See README.md and LICENSE.txt for details.

//! ISO 8601 calendar date with time zone.
#![allow(deprecated)]

#[cfg(feature = "alloc")]
use core::borrow::Borrow;
use core::cmp::Ordering;
use core::ops::{Add, AddAssign, Sub, SubAssign};
use core::{fmt, hash};

#[cfg(feature = "rkyv")]
use rkyv::{Archive, Deserialize, Serialize};

#[cfg(all(feature = "unstable-locales", feature = "alloc"))]
use crate::format::Locale;
#[cfg(feature = "alloc")]
use crate::format::{DelayedFormat, Item, StrftimeItems};
use crate::naive::{IsoWeek, NaiveDate, NaiveTime};
use crate::offset::{TimeZone, Utc};
use crate::{DateTime, Datelike, TimeDelta, Weekday};

/// ISO 8601 calendar date with time zone.
///
/// You almost certainly want to be using a [`NaiveDate`] instead of this type.
///
/// This type primarily exists to aid in the construction of DateTimes that
/// have a timezone by way of the [`TimeZone`] datelike constructors (e.g.
/// [`TimeZone::ymd`]).
///
/// This type should be considered ambiguous at best, due to the inherent lack
/// of precision required for the time zone resolution.
///
/// There are some guarantees on the usage of `Date<Tz>`:
///
/// - If properly constructed via [`TimeZone::ymd`] and others without an error,
///   the corresponding local date should exist for at least a moment.
///   (It may still have a gap from the offset changes.)
///
/// - The `TimeZone` is free to assign *any* [`Offset`](crate::offset::Offset) to the
///   local date, as long as that offset did occur in given day.
///
///   For example, if `2015-03-08T01:59-08:00` is followed by `2015-03-08T03:00-07:00`,
///   it may produce either `2015-03-08-08:00` or `2015-03-08-07:00`
///   but *not* `2015-03-08+00:00` and others.
///
/// - Once constructed as a full `DateTime`, [`DateTime::date`] and other associated
///   methods should return those for the original `Date`. For example, if `dt =
///   tz.ymd_opt(y,m,d).unwrap().hms(h,n,s)` were valid, `dt.date() == tz.ymd_opt(y,m,d).unwrap()`.
///
/// - The date is timezone-agnostic up to one day (i.e. practically always),
///   so the local date and UTC date should be equal for most cases
///   even though the raw calculation between `NaiveDate` and `TimeDelta` may not.
#[deprecated(since = "0.4.23", note = "Use `NaiveDate` or `DateTime<Tz>` instead")]
#[derive(Clone)]
#[cfg_attr(feature = "rkyv", derive(Archive, Deserialize, Serialize))]
pub struct Date<Tz: TimeZone> {
    date: NaiveDate,
    offset: Tz::Offset,
}

/// The minimum possible `Date`.
#[allow(deprecated)]
#[deprecated(since = "0.4.20", note = "Use Date::MIN_UTC instead")]
pub const MIN_DATE: Date<Utc> = Date::<Utc>::MIN_UTC;
/// The maximum possible `Date`.
#[allow(deprecated)]
#[deprecated(since = "0.4.20", note = "Use Date::MAX_UTC instead")]
pub const MAX_DATE: Date<Utc> = Date::<Utc>::MAX_UTC;

impl<Tz: TimeZone> Date<Tz> {
    /// Makes a new `Date` with given *UTC* date and offset.
    /// The local date should be constructed via the `TimeZone` trait.
    #[inline]
    #[must_use]
    pub fn from_utc(date: NaiveDate, offset: Tz::Offset) -> Date<Tz> {
        Date { date, offset }
    }

    /// Makes a new `DateTime` from the current date and given `NaiveTime`.
    /// The offset in the current date is preserved.
    ///
    /// Returns `None` on invalid datetime.
    #[inline]
    #[must_use]
    pub fn and_time(&self, time: NaiveTime) -> Option<DateTime<Tz>> {
        let localdt = self.naive_local().and_time(time);
        self.timezone().from_local_datetime(&localdt).single()
    }

    /// Makes a new `DateTime` from the current date, hour, minute and second.
    /// The offset in the current date is preserved.
    ///
    /// Panics on invalid hour, minute and/or second.
    #[deprecated(since = "0.4.23", note = "Use and_hms_opt() instead")]
    #[inline]
    #[must_use]
    pub fn and_hms(&self, hour: u32, min: u32, sec: u32) -> DateTime<Tz> {
        self.and_hms_opt(hour, min, sec).expect("invalid time")
    }

    /// Makes a new `DateTime` from the current date, hour, minute and second.
    /// The offset in the current date is preserved.
    ///
    /// Returns `None` on invalid hour, minute and/or second.
    #[inline]
    #[must_use]
    pub fn and_hms_opt(&self, hour: u32, min: u32, sec: u32) -> Option<DateTime<Tz>> {
        NaiveTime::from_hms_opt(hour, min, sec).and_then(|time| self.and_time(time))
    }

    /// Makes a new `DateTime` from the current date, hour, minute, second and millisecond.
    /// The millisecond part can exceed 1,000 in order to represent the leap second.
    /// The offset in the current date is preserved.
    ///
    /// Panics on invalid hour, minute, second and/or millisecond.
    #[deprecated(since = "0.4.23", note = "Use and_hms_milli_opt() instead")]
    #[inline]
    #[must_use]
    pub fn and_hms_milli(&self, hour: u32, min: u32, sec: u32, milli: u32) -> DateTime<Tz> {
        self.and_hms_milli_opt(hour, min, sec, milli).expect("invalid time")
    }

    /// Makes a new `DateTime` from the current date, hour, minute, second and millisecond.
    /// The millisecond part can exceed 1,000 in order to represent the leap second.
    /// The offset in the current date is preserved.
    ///
    /// Returns `None` on invalid hour, minute, second and/or millisecond.
    #[inline]
    #[must_use]
    pub fn and_hms_milli_opt(
        &self,
        hour: u32,
        min: u32,
        sec: u32,
        milli: u32,
    ) -> Option<DateTime<Tz>> {
        NaiveTime::from_hms_milli_opt(hour, min, sec, milli).and_then(|time| self.and_time(time))
    }

    /// Makes a new `DateTime` from the current date, hour, minute, second and microsecond.
    /// The microsecond part can exceed 1,000,000 in order to represent the leap second.
    /// The offset in the current date is preserved.
    ///
    /// Panics on invalid hour, minute, second and/or microsecond.
    #[deprecated(since = "0.4.23", note = "Use and_hms_micro_opt() instead")]
    #[inline]
    #[must_use]
    pub fn and_hms_micro(&self, hour: u32, min: u32, sec: u32, micro: u32) -> DateTime<Tz> {
        self.and_hms_micro_opt(hour, min, sec, micro).expect("invalid time")
    }

    /// Makes a new `DateTime` from the current date, hour, minute, second and microsecond.
    /// The microsecond part can exceed 1,000,000 in order to represent the leap second.
    /// The offset in the current date is preserved.
    ///
    /// Returns `None` on invalid hour, minute, second and/or microsecond.
    #[inline]
    #[must_use]
    pub fn and_hms_micro_opt(
        &self,
        hour: u32,
        min: u32,
        sec: u32,
        micro: u32,
    ) -> Option<DateTime<Tz>> {
        NaiveTime::from_hms_micro_opt(hour, min, sec, micro).and_then(|time| self.and_time(time))
    }

    /// Makes a new `DateTime` from the current date, hour, minute, second and nanosecond.
    /// The nanosecond part can exceed 1,000,000,000 in order to represent the leap second.
    /// The offset in the current date is preserved.
    ///
    /// Panics on invalid hour, minute, second and/or nanosecond.
    #[deprecated(since = "0.4.23", note = "Use and_hms_nano_opt() instead")]
    #[inline]
    #[must_use]
    pub fn and_hms_nano(&self, hour: u32, min: u32, sec: u32, nano: u32) -> DateTime<Tz> {
        self.and_hms_nano_opt(hour, min, sec, nano).expect("invalid time")
    }

    /// Makes a new `DateTime` from the current date, hour, minute, second and nanosecond.
    /// The nanosecond part can exceed 1,000,000,000 in order to represent the leap second.
    /// The offset in the current date is preserved.
    ///
    /// Returns `None` on invalid hour, minute, second and/or nanosecond.
    #[inline]
    #[must_use]
    pub fn and_hms_nano_opt(
        &self,
        hour: u32,
        min: u32,
        sec: u32,
        nano: u32,
    ) -> Option<DateTime<Tz>> {
        NaiveTime::from_hms_nano_opt(hour, min, sec, nano).and_then(|time| self.and_time(time))
    }

    /// Makes a new `Date` for the next date.
    ///
    /// Panics when `self` is the last representable date.
    #[deprecated(since = "0.4.23", note = "Use succ_opt() instead")]
    #[inline]
    #[must_use]
    pub fn succ(&self) -> Date<Tz> {
        self.succ_opt().expect("out of bound")
    }

    /// Makes a new `Date` for the next date.
    ///
    /// Returns `None` when `self` is the last representable date.
    #[inline]
    #[must_use]
    pub fn succ_opt(&self) -> Option<Date<Tz>> {
        self.date.succ_opt().map(|date| Date::from_utc(date, self.offset.clone()))
    }

    /// Makes a new `Date` for the prior date.
    ///
    /// Panics when `self` is the first representable date.
    #[deprecated(since = "0.4.23", note = "Use pred_opt() instead")]
    #[inline]
    #[must_use]
    pub fn pred(&self) -> Date<Tz> {
        self.pred_opt().expect("out of bound")
    }

    /// Makes a new `Date` for the prior date.
    ///
    /// Returns `None` when `self` is the first representable date.
    #[inline]
    #[must_use]
    pub fn pred_opt(&self) -> Option<Date<Tz>> {
        self.date.pred_opt().map(|date| Date::from_utc(date, self.offset.clone()))
    }

    /// Retrieves an associated offset from UTC.
    #[inline]
    #[must_use]
    pub fn offset(&self) -> &Tz::Offset {
        &self.offset
    }

    /// Retrieves an associated time zone.
    #[inline]
    #[must_use]
    pub fn timezone(&self) -> Tz {
        TimeZone::from_offset(&self.offset)
    }

    /// Changes the associated time zone.
    /// This does not change the actual `Date` (but will change the string representation).
    #[inline]
    #[must_use]
    pub fn with_timezone<Tz2: TimeZone>(&self, tz: &Tz2) -> Date<Tz2> {
        tz.from_utc_date(&self.date)
    }

    /// Adds given `TimeDelta` to the current date.
    ///
    /// Returns `None` when it will result in overflow.
    #[inline]
    #[must_use]
    pub fn checked_add_signed(self, rhs: TimeDelta) -> Option<Date<Tz>> {
        let date = self.date.checked_add_signed(rhs)?;
        Some(Date { date, offset: self.offset })
    }

    /// Subtracts given `TimeDelta` from the current date.
    ///
    /// Returns `None` when it will result in overflow.
    #[inline]
    #[must_use]
    pub fn checked_sub_signed(self, rhs: TimeDelta) -> Option<Date<Tz>> {
        let date = self.date.checked_sub_signed(rhs)?;
        Some(Date { date, offset: self.offset })
    }

    /// Subtracts another `Date` from the current date.
    /// Returns a `TimeDelta` of integral numbers.
    ///
    /// This does not overflow or underflow at all,
    /// as all possible output fits in the range of `TimeDelta`.
    #[inline]
    #[must_use]
    pub fn signed_duration_since<Tz2: TimeZone>(self, rhs: Date<Tz2>) -> TimeDelta {
        self.date.signed_duration_since(rhs.date)
    }

    /// Returns a view to the naive UTC date.
    #[inline]
    #[must_use]
    pub fn naive_utc(&self) -> NaiveDate {
        self.date
    }

    /// Returns a view to the naive local date.
    ///
    /// This is technically the same as [`naive_utc`](#method.naive_utc)
    /// because the offset is restricted to never exceed one day,
    /// but provided for the consistency.
    #[inline]
    #[must_use]
    pub fn naive_local(&self) -> NaiveDate {
        self.date
    }

    /// Returns the number of whole years from the given `base` until `self`.
    #[must_use]
    pub fn years_since(&self, base: Self) -> Option<u32> {
        self.date.years_since(base.date)
    }

    /// The minimum possible `Date`.
    pub const MIN_UTC: Date<Utc> = Date { date: NaiveDate::MIN, offset: Utc };
    /// The maximum possible `Date`.
    pub const MAX_UTC: Date<Utc> = Date { date: NaiveDate::MAX, offset: Utc };
}

/// Maps the local date to other date with given conversion function.
fn map_local<Tz: TimeZone, F>(d: &Date<Tz>, mut f: F) -> Option<Date<Tz>>
where
    F: FnMut(NaiveDate) -> Option<NaiveDate>,
{
    f(d.naive_local()).and_then(|date| d.timezone().from_local_date(&date).single())
}

impl<Tz: TimeZone> Date<Tz>
where
    Tz::Offset: fmt::Display,
{
    /// Formats the date with the specified formatting items.
    #[cfg(feature = "alloc")]
    #[inline]
    #[must_use]
    pub fn format_with_items<'a, I, B>(&self, items: I) -> DelayedFormat<I>
    where
        I: Iterator<Item = B> + Clone,
        B: Borrow<Item<'a>>,
    {
        DelayedFormat::new_with_offset(Some(self.naive_local()), None, &self.offset, items)
    }

    /// Formats the date with the specified format string.
    /// See the [`crate::format::strftime`] module
    /// on the supported escape sequences.
    #[cfg(feature = "alloc")]
    #[inline]
    #[must_use]
    pub fn format<'a>(&self, fmt: &'a str) -> DelayedFormat<StrftimeItems<'a>> {
        self.format_with_items(StrftimeItems::new(fmt))
    }

    /// Formats the date with the specified formatting items and locale.
    #[cfg(all(feature = "unstable-locales", feature = "alloc"))]
    #[inline]
    #[must_use]
    pub fn format_localized_with_items<'a, I, B>(
        &self,
        items: I,
        locale: Locale,
    ) -> DelayedFormat<I>
    where
        I: Iterator<Item = B> + Clone,
        B: Borrow<Item<'a>>,
    {
        DelayedFormat::new_with_offset_and_locale(
            Some(self.naive_local()),
            None,
            &self.offset,
            items,
            locale,
        )
    }

    /// Formats the date with the specified format string and locale.
    /// See the [`crate::format::strftime`] module
    /// on the supported escape sequences.
    #[cfg(all(feature = "unstable-locales", feature = "alloc"))]
    #[inline]
    #[must_use]
    pub fn format_localized<'a>(
        &self,
        fmt: &'a str,
        locale: Locale,
    ) -> DelayedFormat<StrftimeItems<'a>> {
        self.format_localized_with_items(StrftimeItems::new_with_locale(fmt, locale), locale)
    }
}

impl<Tz: TimeZone> Datelike for Date<Tz> {
    #[inline]
    fn year(&self) -> i32 {
        self.naive_local().year()
    }
    #[inline]
    fn month(&self) -> u32 {
        self.naive_local().month()
    }
    #[inline]
    fn month0(&self) -> u32 {
        self.naive_local().month0()
    }
    #[inline]
    fn day(&self) -> u32 {
        self.naive_local().day()
    }
    #[inline]
    fn day0(&self) -> u32 {
        self.naive_local().day0()
    }
    #[inline]
    fn ordinal(&self) -> u32 {
        self.naive_local().ordinal()
    }
    #[inline]
    fn ordinal0(&self) -> u32 {
        self.naive_local().ordinal0()
    }
    #[inline]
    fn weekday(&self) -> Weekday {
        self.naive_local().weekday()
    }
    #[inline]
    fn iso_week(&self) -> IsoWeek {
        self.naive_local().iso_week()
    }

    #[inline]
    fn with_year(&self, year: i32) -> Option<Date<Tz>> {
        map_local(self, |date| date.with_year(year))
    }

    #[inline]
    fn with_month(&self, month: u32) -> Option<Date<Tz>> {
        map_local(self, |date| date.with_month(month))
    }

    #[inline]
    fn with_month0(&self, month0: u32) -> Option<Date<Tz>> {
        map_local(self, |date| date.with_month0(month0))
    }

    #[inline]
    fn with_day(&self, day: u32) -> Option<Date<Tz>> {
        map_local(self, |date| date.with_day(day))
    }

    #[inline]
    fn with_day0(&self, day0: u32) -> Option<Date<Tz>> {
        map_local(self, |date| date.with_day0(day0))
    }

    #[inline]
    fn with_ordinal(&self, ordinal: u32) -> Option<Date<Tz>> {
        map_local(self, |date| date.with_ordinal(ordinal))
    }

    #[inline]
    fn with_ordinal0(&self, ordinal0: u32) -> Option<Date<Tz>> {
        map_local(self, |date| date.with_ordinal0(ordinal0))
    }
}

// we need them as automatic impls cannot handle associated types
impl<Tz: TimeZone> Copy for Date<Tz> where <Tz as TimeZone>::Offset: Copy {}
unsafe impl<Tz: TimeZone> Send for Date<Tz> where <Tz as TimeZone>::Offset: Send {}

impl<Tz: TimeZone, Tz2: TimeZone> PartialEq<Date<Tz2>> for Date<Tz> {
    fn eq(&self, other: &Date<Tz2>) -> bool {
        self.date == other.date
    }
}

impl<Tz: TimeZone> Eq for Date<Tz> {}

impl<Tz: TimeZone> PartialOrd for Date<Tz> {
    fn partial_cmp(&self, other: &Date<Tz>) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<Tz: TimeZone> Ord for Date<Tz> {
    fn cmp(&self, other: &Date<Tz>) -> Ordering {
        self.date.cmp(&other.date)
    }
}

impl<Tz: TimeZone> hash::Hash for Date<Tz> {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.date.hash(state)
    }
}

impl<Tz: TimeZone> Add<TimeDelta> for Date<Tz> {
    type Output = Date<Tz>;

    #[inline]
    fn add(self, rhs: TimeDelta) -> Date<Tz> {
        self.checked_add_signed(rhs).expect("`Date + TimeDelta` overflowed")
    }
}

impl<Tz: TimeZone> AddAssign<TimeDelta> for Date<Tz> {
    #[inline]
    fn add_assign(&mut self, rhs: TimeDelta) {
        self.date = self.date.checked_add_signed(rhs).expect("`Date + TimeDelta` overflowed");
    }
}

impl<Tz: TimeZone> Sub<TimeDelta> for Date<Tz> {
    type Output = Date<Tz>;

    #[inline]
    fn sub(self, rhs: TimeDelta) -> Date<Tz> {
        self.checked_sub_signed(rhs).expect("`Date - TimeDelta` overflowed")
    }
}

impl<Tz: TimeZone> SubAssign<TimeDelta> for Date<Tz> {
    #[inline]
    fn sub_assign(&mut self, rhs: TimeDelta) {
        self.date = self.date.checked_sub_signed(rhs).expect("`Date - TimeDelta` overflowed");
    }
}

impl<Tz: TimeZone> Sub<Date<Tz>> for Date<Tz> {
    type Output = TimeDelta;

    #[inline]
    fn sub(self, rhs: Date<Tz>) -> TimeDelta {
        self.signed_duration_since(rhs)
    }
}

impl<Tz: TimeZone> fmt::Debug for Date<Tz> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.naive_local().fmt(f)?;
        self.offset.fmt(f)
    }
}

impl<Tz: TimeZone> fmt::Display for Date<Tz>
where
    Tz::Offset: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.naive_local().fmt(f)?;
        self.offset.fmt(f)
    }
}

// Note that implementation of Arbitrary cannot be automatically derived for Date<Tz>, due to
// the nontrivial bound <Tz as TimeZone>::Offset: Arbitrary.
#[cfg(all(feature = "arbitrary", feature = "std"))]
impl<'a, Tz> arbitrary::Arbitrary<'a> for Date<Tz>
where
    Tz: TimeZone,
    <Tz as TimeZone>::Offset: arbitrary::Arbitrary<'a>,
{
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Date<Tz>> {
        let date = NaiveDate::arbitrary(u)?;
        let offset = <Tz as TimeZone>::Offset::arbitrary(u)?;
        Ok(Date::from_utc(date, offset))
    }
}

#[cfg(test)]
mod tests {
    use super::Date;

    use crate::{FixedOffset, NaiveDate, TimeDelta, Utc};

    #[cfg(feature = "clock")]
    use crate::offset::{Local, TimeZone};

    #[test]
    #[cfg(feature = "clock")]
    fn test_years_elapsed() {
        const WEEKS_PER_YEAR: f32 = 52.1775;

        // This is always at least one year because 1 year = 52.1775 weeks.
        let one_year_ago = Utc::today() - TimeDelta::weeks((WEEKS_PER_YEAR * 1.5).ceil() as i64);
        // A bit more than 2 years.
        let two_year_ago = Utc::today() - TimeDelta::weeks((WEEKS_PER_YEAR * 2.5).ceil() as i64);

        assert_eq!(Utc::today().years_since(one_year_ago), Some(1));
        assert_eq!(Utc::today().years_since(two_year_ago), Some(2));

        // If the given DateTime is later than now, the function will always return 0.
        let future = Utc::today() + TimeDelta::weeks(12);
        assert_eq!(Utc::today().years_since(future), None);
    }

    #[test]
    fn test_date_add_assign() {
        let naivedate = NaiveDate::from_ymd_opt(2000, 1, 1).unwrap();
        let date = Date::<Utc>::from_utc(naivedate, Utc);
        let mut date_add = date;

        date_add += TimeDelta::days(5);
        assert_eq!(date_add, date + TimeDelta::days(5));

        let timezone = FixedOffset::east_opt(60 * 60).unwrap();
        let date = date.with_timezone(&timezone);
        let date_add = date_add.with_timezone(&timezone);

        assert_eq!(date_add, date + TimeDelta::days(5));

        let timezone = FixedOffset::west_opt(2 * 60 * 60).unwrap();
        let date = date.with_timezone(&timezone);
        let date_add = date_add.with_timezone(&timezone);

        assert_eq!(date_add, date + TimeDelta::days(5));
    }

    #[test]
    #[cfg(feature = "clock")]
    fn test_date_add_assign_local() {
        let naivedate = NaiveDate::from_ymd_opt(2000, 1, 1).unwrap();

        let date = Local.from_utc_date(&naivedate);
        let mut date_add = date;

        date_add += TimeDelta::days(5);
        assert_eq!(date_add, date + TimeDelta::days(5));
    }

    #[test]
    fn test_date_sub_assign() {
        let naivedate = NaiveDate::from_ymd_opt(2000, 1, 1).unwrap();
        let date = Date::<Utc>::from_utc(naivedate, Utc);
        let mut date_sub = date;

        date_sub -= TimeDelta::days(5);
        assert_eq!(date_sub, date - TimeDelta::days(5));

        let timezone = FixedOffset::east_opt(60 * 60).unwrap();
        let date = date.with_timezone(&timezone);
        let date_sub = date_sub.with_timezone(&timezone);

        assert_eq!(date_sub, date - TimeDelta::days(5));

        let timezone = FixedOffset::west_opt(2 * 60 * 60).unwrap();
        let date = date.with_timezone(&timezone);
        let date_sub = date_sub.with_timezone(&timezone);

        assert_eq!(date_sub, date - TimeDelta::days(5));
    }

    #[test]
    #[cfg(feature = "clock")]
    fn test_date_sub_assign_local() {
        let naivedate = NaiveDate::from_ymd_opt(2000, 1, 1).unwrap();

        let date = Local.from_utc_date(&naivedate);
        let mut date_sub = date;

        date_sub -= TimeDelta::days(5);
        assert_eq!(date_sub, date - TimeDelta::days(5));
    }
}
