use crate::{
    civil::{Date, DateTime, Weekday},
    error::{err, Error},
    util::{
        rangeint::RInto,
        t::{self, ISOWeek, ISOYear, C},
    },
    Zoned,
};

/// A type representing an [ISO 8601 week date].
///
/// The ISO 8601 week date scheme devises a calendar where days are identified
/// by their year, week number and weekday. All years have either precisely
/// 52 or 53 weeks.
///
/// The first week of an ISO 8601 year corresponds to the week containing the
/// first Thursday of the year. For this reason, an ISO 8601 week year can be
/// mismatched with the day's corresponding Gregorian year. For example, the
/// ISO 8601 week date for `1995-01-01` is `1994-W52-7` (with `7` corresponding
/// to Sunday).
///
/// ISO 8601 also considers Monday to be the start of the week, and uses
/// a 1-based numbering system. That is, Monday corresponds to `1` while
/// Sunday corresponds to `7` and is the last day of the week. Weekdays are
/// encapsulated by the [`Weekday`] type, which provides routines for easily
/// converting between different schemes (such as weeks where Sunday is the
/// beginning).
///
/// [ISO 8601 week date]: https://en.wikipedia.org/wiki/ISO_week_date
///
/// # Use case
///
/// Some domains use this method of timekeeping. Otherwise, unless you
/// specifically want a week oriented calendar, it's likely that you'll never
/// need to care about this type.
///
/// # Default value
///
/// For convenience, this type implements the `Default` trait. Its default
/// value is the first day of the zeroth year. i.e., `0000-W1-1`.
///
/// # Example: sample dates
///
/// This example shows a couple ISO 8601 week dates and their corresponding
/// Gregorian equivalents:
///
/// ```
/// use jiff::civil::{ISOWeekDate, Weekday, date};
///
/// let d = date(2019, 12, 30);
/// let weekdate = ISOWeekDate::new(2020, 1, Weekday::Monday).unwrap();
/// assert_eq!(d.iso_week_date(), weekdate);
///
/// let d = date(2024, 3, 9);
/// let weekdate = ISOWeekDate::new(2024, 10, Weekday::Saturday).unwrap();
/// assert_eq!(d.iso_week_date(), weekdate);
/// ```
///
/// # Example: overlapping leap and long years
///
/// A "long" ISO 8601 week year is a year with 53 weeks. That is, it is a year
/// that includes a leap week. This example shows all years in the 20th
/// century that are both Gregorian leap years and long years.
///
/// ```
/// use jiff::civil::date;
///
/// let mut overlapping = vec![];
/// for year in 1900..=1999 {
///     let date = date(year, 1, 1);
///     if date.in_leap_year() && date.iso_week_date().in_long_year() {
///         overlapping.push(year);
///     }
/// }
/// assert_eq!(overlapping, vec![
///     1904, 1908, 1920, 1932, 1936, 1948, 1960, 1964, 1976, 1988, 1992,
/// ]);
/// ```
///
/// # Example: printing all weeks in a year
///
/// The ISO 8601 week calendar can be useful when you want to categorize
/// things into buckets of weeks where all weeks are exactly 7 days, _and_
/// you don't care as much about the precise Gregorian year. Here's an example
/// that prints all of the ISO 8601 weeks in one ISO 8601 week year:
///
/// ```
/// use jiff::{civil::{ISOWeekDate, Weekday}, ToSpan};
///
/// let target_year = 2024;
/// let iso_week_date = ISOWeekDate::new(target_year, 1, Weekday::Monday)?;
/// // Create a series of dates via the Gregorian calendar. But since a
/// // Gregorian week and an ISO 8601 week calendar week are both 7 days,
/// // this works fine.
/// let weeks = iso_week_date
///     .date()
///     .series(1.week())
///     .map(|d| d.iso_week_date())
///     .take_while(|wd| wd.year() == target_year);
/// for start_of_week in weeks {
///     let end_of_week = start_of_week.last_of_week()?;
///     println!(
///         "ISO week {}: {} - {}",
///         start_of_week.week(),
///         start_of_week.date(),
///         end_of_week.date()
///     );
/// }
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Copy, Hash)]
pub struct ISOWeekDate {
    year: ISOYear,
    week: ISOWeek,
    weekday: Weekday,
}

impl ISOWeekDate {
    /// The maximum representable ISO week date.
    ///
    /// The maximum corresponds to the ISO week date of the maximum [`Date`]
    /// value. That is, `-9999-01-01`.
    pub const MIN: ISOWeekDate = ISOWeekDate {
        year: ISOYear::new_unchecked(-9999),
        week: ISOWeek::new_unchecked(1),
        weekday: Weekday::Monday,
    };

    /// The minimum representable ISO week date.
    ///
    /// The minimum corresponds to the ISO week date of the minimum [`Date`]
    /// value. That is, `9999-12-31`.
    pub const MAX: ISOWeekDate = ISOWeekDate {
        year: ISOYear::new_unchecked(9999),
        week: ISOWeek::new_unchecked(52),
        weekday: Weekday::Friday,
    };

    /// The first day of the zeroth year.
    ///
    /// This is guaranteed to be equivalent to `ISOWeekDate::default()`. Note
    /// that this is not equivalent to `Date::default()`.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::civil::{ISOWeekDate, date};
    ///
    /// assert_eq!(ISOWeekDate::ZERO, ISOWeekDate::default());
    /// // The first day of the 0th year in the ISO week calendar is actually
    /// // the third day of the 0th year in the proleptic Gregorian calendar!
    /// assert_eq!(ISOWeekDate::default().date(), date(0, 1, 3));
    /// ```
    pub const ZERO: ISOWeekDate = ISOWeekDate {
        year: ISOYear::new_unchecked(0),
        week: ISOWeek::new_unchecked(1),
        weekday: Weekday::Monday,
    };

    /// Create a new ISO week date from it constituent parts.
    ///
    /// If the given values are out of range (based on what is representable
    /// as a [`Date`]), then this returns an error. This will also return an
    /// error if a leap week is given (week number `53`) for a year that does
    /// not contain a leap week.
    ///
    /// # Example
    ///
    /// This example shows some the boundary conditions involving minimum
    /// and maximum dates:
    ///
    /// ```
    /// use jiff::civil::{ISOWeekDate, Weekday, date};
    ///
    /// // The year 1949 does not contain a leap week.
    /// assert!(ISOWeekDate::new(1949, 53, Weekday::Monday).is_err());
    ///
    /// // Examples of dates at or exceeding the maximum.
    /// let max = ISOWeekDate::new(9999, 52, Weekday::Friday).unwrap();
    /// assert_eq!(max, ISOWeekDate::MAX);
    /// assert_eq!(max.date(), date(9999, 12, 31));
    /// assert!(ISOWeekDate::new(9999, 52, Weekday::Saturday).is_err());
    /// assert!(ISOWeekDate::new(9999, 53, Weekday::Monday).is_err());
    ///
    /// // Examples of dates at or exceeding the minimum.
    /// let min = ISOWeekDate::new(-9999, 1, Weekday::Monday).unwrap();
    /// assert_eq!(min, ISOWeekDate::MIN);
    /// assert_eq!(min.date(), date(-9999, 1, 1));
    /// assert!(ISOWeekDate::new(-10000, 52, Weekday::Sunday).is_err());
    /// ```
    #[inline]
    pub fn new(
        year: i16,
        week: i8,
        weekday: Weekday,
    ) -> Result<ISOWeekDate, Error> {
        let year = ISOYear::try_new("year", year)?;
        let week = ISOWeek::try_new("week", week)?;
        ISOWeekDate::new_ranged(year, week, weekday)
    }

    /// Converts a Gregorian date to an ISO week date.
    ///
    /// The minimum and maximum allowed values of an ISO week date are
    /// set based on the minimum and maximum values of a `Date`. Therefore,
    /// converting to and from `Date` values is non-lossy and infallible.
    ///
    /// This routine is equivalent to [`Date::iso_week_date`]. This routine
    /// is also available via a `From<Date>` trait implementation for
    /// `ISOWeekDate`.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::civil::{ISOWeekDate, Weekday, date};
    ///
    /// let weekdate = ISOWeekDate::from_date(date(1948, 2, 10));
    /// assert_eq!(
    ///     weekdate,
    ///     ISOWeekDate::new(1948, 7, Weekday::Tuesday).unwrap(),
    /// );
    /// ```
    #[inline]
    pub fn from_date(date: Date) -> ISOWeekDate {
        date.iso_week_date()
    }

    // N.B. I tried defining a `ISOWeekDate::constant` for defining ISO week
    // dates as constants, but it was too annoying to do. We could do it if
    // there was a compelling reason for it though.

    /// Returns the year component of this ISO 8601 week date.
    ///
    /// The value returned is guaranteed to be in the range `-9999..=9999`.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::civil::date;
    ///
    /// let weekdate = date(2019, 12, 30).iso_week_date();
    /// assert_eq!(weekdate.year(), 2020);
    /// ```
    #[inline]
    pub fn year(self) -> i16 {
        self.year_ranged().get()
    }

    /// Returns the week component of this ISO 8601 week date.
    ///
    /// The value returned is guaranteed to be in the range `1..=53`. A
    /// value of `53` can only occur for "long" years. That is, years
    /// with a leap week. This occurs precisely in cases for which
    /// [`ISOWeekDate::in_long_year`] returns `true`.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::civil::date;
    ///
    /// let weekdate = date(2019, 12, 30).iso_week_date();
    /// assert_eq!(weekdate.year(), 2020);
    /// assert_eq!(weekdate.week(), 1);
    ///
    /// let weekdate = date(1948, 12, 31).iso_week_date();
    /// assert_eq!(weekdate.year(), 1948);
    /// assert_eq!(weekdate.week(), 53);
    /// ```
    #[inline]
    pub fn week(self) -> i8 {
        self.week_ranged().get()
    }

    /// Returns the day component of this ISO 8601 week date.
    ///
    /// One can use methods on `Weekday` such as
    /// [`Weekday::to_monday_one_offset`]
    /// and
    /// [`Weekday::to_sunday_zero_offset`]
    /// to convert the weekday to a number.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::civil::{date, Weekday};
    ///
    /// let weekdate = date(1948, 12, 31).iso_week_date();
    /// assert_eq!(weekdate.year(), 1948);
    /// assert_eq!(weekdate.week(), 53);
    /// assert_eq!(weekdate.weekday(), Weekday::Friday);
    /// assert_eq!(weekdate.weekday().to_monday_zero_offset(), 4);
    /// assert_eq!(weekdate.weekday().to_monday_one_offset(), 5);
    /// assert_eq!(weekdate.weekday().to_sunday_zero_offset(), 5);
    /// assert_eq!(weekdate.weekday().to_sunday_one_offset(), 6);
    /// ```
    #[inline]
    pub fn weekday(self) -> Weekday {
        self.weekday
    }

    /// Returns the ISO 8601 week date corresponding to the first day in the
    /// week of this week date. The date returned is guaranteed to have a
    /// weekday of [`Weekday::Monday`].
    ///
    /// # Errors
    ///
    /// Since `-9999-01-01` falls on a Monday, it follows that the minimum
    /// support Gregorian date is exactly equivalent to the minimum supported
    /// ISO 8601 week date. This means that this routine can never actually
    /// fail, but only insomuch as the minimums line up. For that reason, and
    /// for consistency with [`ISOWeekDate::last_of_week`], the API is
    /// fallible.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::civil::{ISOWeekDate, Weekday, date};
    ///
    /// let wd = ISOWeekDate::new(2025, 5, Weekday::Wednesday).unwrap();
    /// assert_eq!(wd.date(), date(2025, 1, 29));
    /// assert_eq!(
    ///     wd.first_of_week()?,
    ///     ISOWeekDate::new(2025, 5, Weekday::Monday).unwrap(),
    /// );
    ///
    /// // Works even for the minimum date.
    /// assert_eq!(
    ///     ISOWeekDate::MIN.first_of_week()?,
    ///     ISOWeekDate::new(-9999, 1, Weekday::Monday).unwrap(),
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn first_of_week(self) -> Result<ISOWeekDate, Error> {
        // I believe this can never return an error because `Monday` is in
        // bounds for all possible year-and-week combinations. This is *only*
        // because -9999-01-01 corresponds to -9999-W01-Monday. Which is kinda
        // lucky. And I guess if we ever change the ranges, this could become
        // fallible.
        ISOWeekDate::new_ranged(
            self.year_ranged(),
            self.week_ranged(),
            Weekday::Monday,
        )
    }

    /// Returns the ISO 8601 week date corresponding to the last day in the
    /// week of this week date. The date returned is guaranteed to have a
    /// weekday of [`Weekday::Sunday`].
    ///
    /// # Errors
    ///
    /// This can return an error if the last day of the week exceeds Jiff's
    /// maximum Gregorian date of `9999-12-31`. It turns out this can happen
    /// since `9999-12-31` falls on a Friday.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::civil::{ISOWeekDate, Weekday, date};
    ///
    /// let wd = ISOWeekDate::new(2025, 5, Weekday::Wednesday).unwrap();
    /// assert_eq!(wd.date(), date(2025, 1, 29));
    /// assert_eq!(
    ///     wd.last_of_week()?,
    ///     ISOWeekDate::new(2025, 5, Weekday::Sunday).unwrap(),
    /// );
    ///
    /// // Unlike `first_of_week`, this routine can actually fail on real
    /// // values, although, only when close to the maximum supported date.
    /// assert_eq!(
    ///     ISOWeekDate::MAX.last_of_week().unwrap_err().to_string(),
    ///     "parameter 'weekday' with value 7 is not \
    ///      in the required range of 1..=5",
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn last_of_week(self) -> Result<ISOWeekDate, Error> {
        // This can return an error when in the last week of the maximum year
        // supported by Jiff. That's because the Saturday and Sunday of that
        // week are actually in Gregorian year 10,000.
        ISOWeekDate::new_ranged(
            self.year_ranged(),
            self.week_ranged(),
            Weekday::Sunday,
        )
    }

    /// Returns the ISO 8601 week date corresponding to the first day in the
    /// year of this week date. The date returned is guaranteed to have a
    /// weekday of [`Weekday::Monday`].
    ///
    /// # Errors
    ///
    /// Since `-9999-01-01` falls on a Monday, it follows that the minimum
    /// support Gregorian date is exactly equivalent to the minimum supported
    /// ISO 8601 week date. This means that this routine can never actually
    /// fail, but only insomuch as the minimums line up. For that reason, and
    /// for consistency with [`ISOWeekDate::last_of_year`], the API is
    /// fallible.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::civil::{ISOWeekDate, Weekday, date};
    ///
    /// let wd = ISOWeekDate::new(2025, 5, Weekday::Wednesday).unwrap();
    /// assert_eq!(wd.date(), date(2025, 1, 29));
    /// assert_eq!(
    ///     wd.first_of_year()?,
    ///     ISOWeekDate::new(2025, 1, Weekday::Monday).unwrap(),
    /// );
    ///
    /// // Works even for the minimum date.
    /// assert_eq!(
    ///     ISOWeekDate::MIN.first_of_year()?,
    ///     ISOWeekDate::new(-9999, 1, Weekday::Monday).unwrap(),
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn first_of_year(self) -> Result<ISOWeekDate, Error> {
        // I believe this can never return an error because `Monday` is in
        // bounds for all possible years. This is *only* because -9999-01-01
        // corresponds to -9999-W01-Monday. Which is kinda lucky. And I guess
        // if we ever change the ranges, this could become fallible.
        ISOWeekDate::new_ranged(self.year_ranged(), C(1), Weekday::Monday)
    }

    /// Returns the ISO 8601 week date corresponding to the last day in the
    /// year of this week date. The date returned is guaranteed to have a
    /// weekday of [`Weekday::Sunday`].
    ///
    /// # Errors
    ///
    /// This can return an error if the last day of the year exceeds Jiff's
    /// maximum Gregorian date of `9999-12-31`. It turns out this can happen
    /// since `9999-12-31` falls on a Friday.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::civil::{ISOWeekDate, Weekday, date};
    ///
    /// let wd = ISOWeekDate::new(2025, 5, Weekday::Wednesday).unwrap();
    /// assert_eq!(wd.date(), date(2025, 1, 29));
    /// assert_eq!(
    ///     wd.last_of_year()?,
    ///     ISOWeekDate::new(2025, 52, Weekday::Sunday).unwrap(),
    /// );
    ///
    /// // Works correctly for "long" years.
    /// let wd = ISOWeekDate::new(2026, 5, Weekday::Wednesday).unwrap();
    /// assert_eq!(wd.date(), date(2026, 1, 28));
    /// assert_eq!(
    ///     wd.last_of_year()?,
    ///     ISOWeekDate::new(2026, 53, Weekday::Sunday).unwrap(),
    /// );
    ///
    /// // Unlike `first_of_year`, this routine can actually fail on real
    /// // values, although, only when close to the maximum supported date.
    /// assert_eq!(
    ///     ISOWeekDate::MAX.last_of_year().unwrap_err().to_string(),
    ///     "parameter 'weekday' with value 7 is not \
    ///      in the required range of 1..=5",
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn last_of_year(self) -> Result<ISOWeekDate, Error> {
        // This can return an error when in the maximum year supported by
        // Jiff. That's because the last Saturday and Sunday of that year are
        // actually in Gregorian year 10,000.
        let week = if self.in_long_year() {
            ISOWeek::V::<53, 52, 53>()
        } else {
            ISOWeek::V::<52, 52, 53>()
        };
        ISOWeekDate::new_ranged(self.year_ranged(), week, Weekday::Sunday)
    }

    /// Returns the total number of days in the year of this ISO 8601 week
    /// date.
    ///
    /// It is guaranteed that the value returned is either 364 or 371. The
    /// latter case occurs precisely when [`ISOWeekDate::in_long_year`]
    /// returns `true`.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::civil::{ISOWeekDate, Weekday};
    ///
    /// let weekdate = ISOWeekDate::new(2025, 7, Weekday::Monday).unwrap();
    /// assert_eq!(weekdate.days_in_year(), 364);
    /// let weekdate = ISOWeekDate::new(2026, 7, Weekday::Monday).unwrap();
    /// assert_eq!(weekdate.days_in_year(), 371);
    /// ```
    #[inline]
    pub fn days_in_year(self) -> i16 {
        if self.in_long_year() {
            371
        } else {
            364
        }
    }

    /// Returns the total number of weeks in the year of this ISO 8601 week
    /// date.
    ///
    /// It is guaranteed that the value returned is either 52 or 53. The
    /// latter case occurs precisely when [`ISOWeekDate::in_long_year`]
    /// returns `true`.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::civil::{ISOWeekDate, Weekday};
    ///
    /// let weekdate = ISOWeekDate::new(2025, 7, Weekday::Monday).unwrap();
    /// assert_eq!(weekdate.weeks_in_year(), 52);
    /// let weekdate = ISOWeekDate::new(2026, 7, Weekday::Monday).unwrap();
    /// assert_eq!(weekdate.weeks_in_year(), 53);
    /// ```
    #[inline]
    pub fn weeks_in_year(self) -> i8 {
        if self.in_long_year() {
            53
        } else {
            52
        }
    }

    /// Returns true if and only if the year of this week date is a "long"
    /// year.
    ///
    /// A long year is one that contains precisely 53 weeks. All other years
    /// contain precisely 52 weeks.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::civil::{ISOWeekDate, Weekday};
    ///
    /// let weekdate = ISOWeekDate::new(1948, 7, Weekday::Monday).unwrap();
    /// assert!(weekdate.in_long_year());
    /// let weekdate = ISOWeekDate::new(1949, 7, Weekday::Monday).unwrap();
    /// assert!(!weekdate.in_long_year());
    /// ```
    #[inline]
    pub fn in_long_year(self) -> bool {
        is_long_year(self.year_ranged())
    }

    /// Returns the ISO 8601 date immediately following this one.
    ///
    /// # Errors
    ///
    /// This returns an error when this date is the maximum value.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::civil::{ISOWeekDate, Weekday};
    ///
    /// let wd = ISOWeekDate::new(2025, 5, Weekday::Wednesday).unwrap();
    /// assert_eq!(
    ///     wd.tomorrow()?,
    ///     ISOWeekDate::new(2025, 5, Weekday::Thursday).unwrap(),
    /// );
    ///
    /// // The max doesn't have a tomorrow.
    /// assert!(ISOWeekDate::MAX.tomorrow().is_err());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn tomorrow(self) -> Result<ISOWeekDate, Error> {
        // I suppose we could probably implement this in a more efficient
        // manner but avoiding the roundtrip through Gregorian dates.
        self.date().tomorrow().map(|d| d.iso_week_date())
    }

    /// Returns the ISO 8601 week date immediately preceding this one.
    ///
    /// # Errors
    ///
    /// This returns an error when this date is the minimum value.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::civil::{ISOWeekDate, Weekday};
    ///
    /// let wd = ISOWeekDate::new(2025, 5, Weekday::Wednesday).unwrap();
    /// assert_eq!(
    ///     wd.yesterday()?,
    ///     ISOWeekDate::new(2025, 5, Weekday::Tuesday).unwrap(),
    /// );
    ///
    /// // The min doesn't have a yesterday.
    /// assert!(ISOWeekDate::MIN.yesterday().is_err());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn yesterday(self) -> Result<ISOWeekDate, Error> {
        // I suppose we could probably implement this in a more efficient
        // manner but avoiding the roundtrip through Gregorian dates.
        self.date().yesterday().map(|d| d.iso_week_date())
    }

    /// Converts this ISO week date to a Gregorian [`Date`].
    ///
    /// The minimum and maximum allowed values of an ISO week date are
    /// set based on the minimum and maximum values of a `Date`. Therefore,
    /// converting to and from `Date` values is non-lossy and infallible.
    ///
    /// This routine is equivalent to [`Date::from_iso_week_date`].
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::civil::{ISOWeekDate, Weekday, date};
    ///
    /// let weekdate = ISOWeekDate::new(1948, 7, Weekday::Tuesday).unwrap();
    /// assert_eq!(weekdate.date(), date(1948, 2, 10));
    /// ```
    #[inline]
    pub fn date(self) -> Date {
        Date::from_iso_week_date(self)
    }
}

impl ISOWeekDate {
    /// Creates a new ISO week date from ranged values.
    ///
    /// While the ranged values given eliminate some error cases, not all
    /// combinations of year/week/weekday values are valid ISO week dates
    /// supported by this crate. For example, a week of `53` for short years,
    /// or more niche, a week date that would be bigger than what is supported
    /// by our `Date` type.
    #[inline]
    pub(crate) fn new_ranged(
        year: impl RInto<ISOYear>,
        week: impl RInto<ISOWeek>,
        weekday: Weekday,
    ) -> Result<ISOWeekDate, Error> {
        let year = year.rinto();
        let week = week.rinto();
        // All combinations of years, weeks and weekdays allowed by our
        // range types are valid ISO week dates with one exception: a week
        // number of 53 is only valid for "long" years. Or years with an ISO
        // leap week. It turns out this only happens when the last day of the
        // year is a Thursday.
        //
        // Note that if the ranges in this crate are changed, this could be
        // a little trickier if the range of ISOYear is different from Year.
        debug_assert_eq!(t::Year::MIN, ISOYear::MIN);
        debug_assert_eq!(t::Year::MAX, ISOYear::MAX);
        if week == C(53) && !is_long_year(year) {
            return Err(err!(
                "ISO week number `{week}` is invalid for year `{year}`"
            ));
        }
        // And also, the maximum Date constrains what we can utter with
        // ISOWeekDate so that we can preserve infallible conversions between
        // them. So since 9999-12-31 maps to 9999 W52 Friday, it follows that
        // Saturday and Sunday are not allowed. So reject them.
        //
        // We don't need to worry about the minimum because the minimum date
        // (-9999-01-01) corresponds also to the minimum possible combination
        // of an ISO week date's fields: -9999 W01 Monday. Nice.
        if year == ISOYear::MAX_SELF
            && week == C(52)
            && weekday.to_monday_zero_offset()
                > Weekday::Friday.to_monday_zero_offset()
        {
            return Err(Error::range(
                "weekday",
                weekday.to_monday_one_offset(),
                Weekday::Monday.to_monday_one_offset(),
                Weekday::Friday.to_monday_one_offset(),
            ));
        }
        Ok(ISOWeekDate { year, week, weekday })
    }

    /// Like `ISOWeekDate::new_ranged`, but constrains out-of-bounds values
    /// to their closest valid equivalent.
    ///
    /// For example, given 9999 W52 Saturday, this will return 9999 W52 Friday.
    #[cfg(test)]
    #[inline]
    pub(crate) fn new_ranged_constrain(
        year: impl RInto<ISOYear>,
        week: impl RInto<ISOWeek>,
        mut weekday: Weekday,
    ) -> ISOWeekDate {
        let year = year.rinto();
        let mut week = week.rinto();
        debug_assert_eq!(t::Year::MIN, ISOYear::MIN);
        debug_assert_eq!(t::Year::MAX, ISOYear::MAX);
        if week == C(53) && !is_long_year(year) {
            week = ISOWeek::new(52).unwrap();
        }
        if year == ISOYear::MAX_SELF
            && week == C(52)
            && weekday.to_monday_zero_offset()
                > Weekday::Friday.to_monday_zero_offset()
        {
            weekday = Weekday::Friday;
        }
        ISOWeekDate { year, week, weekday }
    }

    #[inline]
    pub(crate) fn year_ranged(self) -> ISOYear {
        self.year
    }

    #[inline]
    pub(crate) fn week_ranged(self) -> ISOWeek {
        self.week
    }
}

impl Default for ISOWeekDate {
    fn default() -> ISOWeekDate {
        ISOWeekDate::ZERO
    }
}

impl core::fmt::Debug for ISOWeekDate {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        f.debug_struct("ISOWeekDate")
            .field("year", &self.year_ranged().debug())
            .field("week", &self.week_ranged().debug())
            .field("weekday", &self.weekday)
            .finish()
    }
}

impl Eq for ISOWeekDate {}

impl PartialEq for ISOWeekDate {
    #[inline]
    fn eq(&self, other: &ISOWeekDate) -> bool {
        // We roll our own so that we can call 'get' on our ranged integers
        // in order to provoke panics for bugs in dealing with boundary
        // conditions.
        self.weekday == other.weekday
            && self.week.get() == other.week.get()
            && self.year.get() == other.year.get()
    }
}

impl Ord for ISOWeekDate {
    #[inline]
    fn cmp(&self, other: &ISOWeekDate) -> core::cmp::Ordering {
        (self.year.get(), self.week.get(), self.weekday.to_monday_one_offset())
            .cmp(&(
                other.year.get(),
                other.week.get(),
                other.weekday.to_monday_one_offset(),
            ))
    }
}

impl PartialOrd for ISOWeekDate {
    #[inline]
    fn partial_cmp(&self, other: &ISOWeekDate) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl From<Date> for ISOWeekDate {
    #[inline]
    fn from(date: Date) -> ISOWeekDate {
        ISOWeekDate::from_date(date)
    }
}

impl From<DateTime> for ISOWeekDate {
    #[inline]
    fn from(dt: DateTime) -> ISOWeekDate {
        ISOWeekDate::from(dt.date())
    }
}

impl From<Zoned> for ISOWeekDate {
    #[inline]
    fn from(zdt: Zoned) -> ISOWeekDate {
        ISOWeekDate::from(zdt.date())
    }
}

impl<'a> From<&'a Zoned> for ISOWeekDate {
    #[inline]
    fn from(zdt: &'a Zoned) -> ISOWeekDate {
        ISOWeekDate::from(zdt.date())
    }
}

#[cfg(test)]
impl quickcheck::Arbitrary for ISOWeekDate {
    fn arbitrary(g: &mut quickcheck::Gen) -> ISOWeekDate {
        let year = ISOYear::arbitrary(g);
        let week = ISOWeek::arbitrary(g);
        let weekday = Weekday::arbitrary(g);
        ISOWeekDate::new_ranged_constrain(year, week, weekday)
    }

    fn shrink(&self) -> alloc::boxed::Box<dyn Iterator<Item = ISOWeekDate>> {
        alloc::boxed::Box::new(
            (self.year_ranged(), self.week_ranged(), self.weekday())
                .shrink()
                .map(|(year, week, weekday)| {
                    ISOWeekDate::new_ranged_constrain(year, week, weekday)
                }),
        )
    }
}

/// Returns true if the given ISO year is a "long" year or not.
///
/// A "long" year is a year with 53 weeks. Otherwise, it's a "short" year
/// with 52 weeks.
fn is_long_year(year: ISOYear) -> bool {
    // Inspired by: https://en.wikipedia.org/wiki/ISO_week_date#Weeks_per_year
    let last = Date::new_ranged(year.rinto(), C(12).rinto(), C(31).rinto())
        .expect("last day of year is always valid");
    let weekday = last.weekday();
    weekday == Weekday::Thursday
        || (last.in_leap_year() && weekday == Weekday::Friday)
}

#[cfg(not(miri))]
#[cfg(test)]
mod tests {
    use super::*;

    quickcheck::quickcheck! {
        fn prop_all_long_years_have_53rd_week(year: ISOYear) -> bool {
            !is_long_year(year)
                || ISOWeekDate::new(year.get(), 53, Weekday::Sunday).is_ok()
        }

        fn prop_prev_day_is_less(wd: ISOWeekDate) -> quickcheck::TestResult {
            use crate::ToSpan;

            if wd == ISOWeekDate::MIN {
                return quickcheck::TestResult::discard();
            }
            let prev_date = wd.date().checked_add(-1.days()).unwrap();
            quickcheck::TestResult::from_bool(prev_date.iso_week_date() < wd)
        }

        fn prop_next_day_is_greater(wd: ISOWeekDate) -> quickcheck::TestResult {
            use crate::ToSpan;

            if wd == ISOWeekDate::MAX {
                return quickcheck::TestResult::discard();
            }
            let next_date = wd.date().checked_add(1.days()).unwrap();
            quickcheck::TestResult::from_bool(wd < next_date.iso_week_date())
        }
    }
}
